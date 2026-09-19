//! What the crew get on with, and the order the player wants it done in.
//!
//! Every errand a Bim takes on *of its own accord* is one of these jobs. The
//! player gives each a number from [`HIGHEST`] to [`LOWEST`], and a Bim with
//! nothing pressing works through whatever is going in that order. They all
//! start equal, so out of the box this changes nothing and the ship behaves
//! exactly as it did before anybody touched the panel.
//!
//! What it does **not** touch is the body. Sleep and the heads are not work —
//! there is no row for them and no number to set — and a Bim past its hunger
//! is fed whatever the cook row says, because a priority list is a statement
//! about what to do next and not a licence to starve. See
//! `Game::consider_errand`, which is the only place any of this is read.
//!
//! No strings cross the boundary, so the ship knows [`Job::Clean`] and the
//! host knows "Cleaning". **Adding a job is four edits**: a variant here,
//! appended; a name in `WORK_NAMES` and a fixture to ring in `WORK_SPOTS`
//! (`crates/app/src/names.rs` and `crew.rs`, whose length tests pin both
//! against [`Job::ALL`]); and the range in `scratchpad/priority.rs` that
//! checks every code names a job. Miss the name and the row renders blank;
//! miss the probe and it passes on a job nobody can read.

/// The jobs, in the order they are listed and in the order the codes run.
///
/// The code is the whole identity across the boundary — the host indexes its
/// name table with it — so variants are appended rather than inserted.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// Sweeping the deck.
    Clean,
    /// Sowing an empty tray in the bay.
    Plant,
    /// Lifting a ripe one out of it.
    Cut,
    /// Carrying things: what was lifted out of a tray to the cold store,
    /// and materials from a shelf to a construction site. The first has no
    /// errand of its own — it is the back half of a [`Job::Cut`], so a
    /// cutting waits on whichever of the two is set later. The second is an
    /// errand in its own right: a load off a shelf, walked to the site and
    /// put down there, one trip at a time, while the world says a site
    /// still wants something (`game::Build`).
    Haul,
    /// Cooking: a meal for a hungry Bim, and stew for the cold store while
    /// the shelf holds fewer than the manager asked for — one vegetable and
    /// one block of tofu, chopped, cooked and put away in a tub. One row,
    /// because both are the galley, and a Bim that is hungry eats before it
    /// cooks for the shelf whatever the number says.
    Cook,
    /// Standing at the helm to control the ship. On offer while the ship is
    /// away from a berth and nobody is posted at the helm; whoever takes it
    /// is posted there — a standing order, like the player's own "take the
    /// helm" — and let go when the ship is tied up again. The room only
    /// knows the helm through `Game::set_helm`, which the world calls: the
    /// classic room has no helm and never offers this.
    Helm,
    /// Making something at a bench — the smelter, the workbench — while the
    /// world has an order for it: the hold short of a product the player
    /// asked to keep, the inputs aboard, and the station powered. One row
    /// for every bench, because what a Bim does at any of them is stand
    /// there; which recipe is the order's. See `game::Order`.
    Craft,
    /// A walk outside in a suit to gather ore, while the ship is holding
    /// at a belt with a suit aboard and room for what comes back. The world
    /// says when — `game::Eva` — and what a walk yields is the belt's.
    Mine,
    /// Putting a part of the ship together at a construction site the
    /// player laid out, once everything it is made of has been carried
    /// there — [`Job::Haul`] is the carrying. The world says what there is
    /// to build and what each site still wants (`game::Build`); the room
    /// walks a Bim to a shelf and to the site, in a suit if the site is
    /// outside the hull.
    Build,
    /// Dressing a wound with one of the ship's bandages — its own first,
    /// while it bleeds, else the crewmate with the most open wounds — on
    /// offer while a bandage is to hand and somebody aboard is bleeding
    /// (`Game::medical_on_offer`). The one row that is also an
    /// **interruption**: set to [`HIGHEST`] it displaces whatever the Bim
    /// is on the moment there is a wound to dress, rather than waiting for
    /// the errand to finish; set to [`NEVER`] nobody doctors of their own
    /// accord, and the player's own bandage orders still work.
    Medical,
}

impl Job {
    pub const ALL: [Job; 10] = [
        Job::Clean,
        Job::Plant,
        Job::Cut,
        Job::Haul,
        Job::Cook,
        Job::Helm,
        Job::Craft,
        Job::Mine,
        Job::Build,
        Job::Medical,
    ];

    /// 0, then one per job. The host names them.
    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Job> {
        Job::ALL.get(code as usize).copied()
    }
}

/// The most important a job can be set to, and the least. Small is urgent:
/// a 1 is done before a 2, which is how every list of this shape reads.
pub const HIGHEST: u32 = 1;
pub const LOWEST: u32 = 5;
/// Below the top: **never**. A job at this is not work at all — it is never
/// offered, whatever else is going. The one exception is a meal for a Bim
/// past its hunger, which is not a licence to starve (see the module note).
pub const NEVER: u32 = 0;
/// What everything starts at — the middle, so the first click in either
/// direction says something.
pub const DEFAULT: u32 = 3;

/// One number per job. The player's standing instruction to the ship rather
/// than to a Bim: there is one list and both crew work to it, the same as the
/// timetable and the action thresholds.
pub struct Priorities {
    level: [u32; Job::ALL.len()],
}

impl Priorities {
    /// Every row at [`DEFAULT`] but the medical one, which starts at
    /// [`HIGHEST`]: a wound is dressed the moment there is a bandage for
    /// it, and a crewmate dying is treated before the deck is swept,
    /// unless the player says otherwise. No seeded probe bleeds, so the
    /// default costs nothing there.
    pub fn new() -> Priorities {
        let mut level = [DEFAULT; Job::ALL.len()];
        level[Job::Medical as usize] = HIGHEST;
        Priorities { level }
    }

    pub fn of(&self, job: Job) -> u32 {
        self.level[job as usize]
    }

    /// Whether the job is switched off altogether.
    pub fn never(&self, job: Job) -> bool {
        self.of(job) == NEVER
    }

    /// Set one, clamped to the range — [`NEVER`] to [`LOWEST`]. Out-of-range
    /// is clamped rather than refused: the number crosses the boundary as a
    /// bare `u32` and a silent no-op would leave the panel showing something
    /// the ship is not doing.
    pub fn set(&mut self, job: Job, level: u32) {
        self.level[job as usize] = level.clamp(NEVER, LOWEST);
    }

    /// What a click on the box does: **one off the number** — a step more
    /// important — through the top to never, and round to the bottom again
    /// from there. The cycling lives here rather than in the host so that
    /// the range has exactly one definition.
    pub fn cycle(&mut self, job: Job) -> u32 {
        let next = if self.of(job) <= NEVER {
            LOWEST
        } else {
            self.of(job) - 1
        };
        self.set(job, next);
        next
    }

    /// The other way round: one onto the number, and from the bottom to
    /// never. A right click.
    pub fn cycle_back(&mut self, job: Job) -> u32 {
        let next = if self.of(job) >= LOWEST {
            NEVER
        } else {
            self.of(job) + 1
        };
        self.set(job, next);
        next
    }

    /// Which of two jobs is done first. Ties keep the order they were offered
    /// in, which is what makes an untouched list behave exactly as the fixed
    /// order did before there was a list at all.
    pub fn before(&self, a: Job, b: Job) -> bool {
        self.of(a) < self.of(b)
    }
}

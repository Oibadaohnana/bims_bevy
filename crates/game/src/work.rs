//! What the crew get on with, and the order the player wants it done in.
//!
//! Every errand a Bim takes on *of its own accord* is one of these jobs. The
//! player gives each a number from [`HIGHEST`] to [`LOWEST`], and a Bim with
//! nothing pressing works through whatever is going in that order. See
//! `Game::consider_errand`, which is the only place any of this is read.
//!
//! No strings cross the boundary, so the ship knows [`Job::Build`] and the
//! host knows "Building". **Adding a job is three edits**: a variant here,
//! appended; a name in `WORK_NAMES` and a fixture to ring in `WORK_SPOTS`
//! (the app's, which no panel shows since the HUD of feature 107). Miss
//! the name and the row renders blank. Task 113 took the carry to the
//! workbench (`Haul`) and the making at a bench (`Craft`) away with the
//! workbench and the crafting, and closed the codes up.

/// The jobs, in the order they are listed and in the order the codes run.
///
/// The code is the whole identity across the boundary — the host indexes its
/// name table with it — so variants are appended rather than inserted.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Job {
    /// Putting a part of the ship together at a construction site the
    /// player laid out. Nothing is carried to one: a part is paid for out
    /// of the crew's pool (feature 95), so the errand is the walk and the
    /// work. The world says what there is to build (`game::Build`); the
    /// room walks a Bim to the site, in a suit if it is outside the hull.
    Build,
    /// Reviving a downed crewmate (task 120, `Game::revive_on_offer`). The
    /// one row that is also an **interruption**: set to [`HIGHEST`] it
    /// displaces whatever the Bim is on the moment there is a crewmate to
    /// revive, rather than waiting for the errand to finish; set to
    /// [`NEVER`] nobody revives of their own accord, and the player's own
    /// orders still work.
    Medical,
}

impl Job {
    pub const ALL: [Job; 2] = [Job::Build, Job::Medical];

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
/// offered, whatever else is going.
pub const NEVER: u32 = 0;
/// What everything starts at — the middle, so the first click in either
/// direction says something.
pub const DEFAULT: u32 = 3;

/// One number per job. The player's standing instruction to the ship rather
/// than to a Bim: there is one list and the whole crew work to it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Priorities {
    level: [u32; Job::ALL.len()],
}

impl Priorities {
    /// Every row at [`DEFAULT`] but the medical one, which starts at
    /// [`HIGHEST`]: a downed crewmate is revived before anything else,
    /// unless the player says otherwise.
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

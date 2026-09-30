//! Scripted jobs the Bim carries out, one step at a time.
//!
//! A task is a straight line of [`Step`]s. Each step either sends the Bim
//! somewhere and waits for it to arrive, or holds it in place for a while
//! playing an animation. Anything the step changes about the world — a door
//! ordered, a kit taken out of a pack — happens on the way in or on the way
//! out, so the state and what you can see never disagree.

use crate::character::{Action, Character, Held};
use crate::clock;
use crate::math::{PI, Vec2, vec2};
use crate::nav::Maps;
use crate::room::{Room, Switch, TILE};

/// How far a patient may move, in tiles, from where it stood when the
/// walk over to it was planned before the helper plans the walk again
/// from where it is now. See `Task::patient_at`.
pub const FOLLOW_SLACK: f32 = 1.5;

/// How near a kit's tile, in tiles, an engineer has to be to work at it:
/// the walk over ends the moment the body is this close with a clear
/// line to the tile's middle, and the bar fills only from there.
pub const DEPLOY_REACH: f32 = 2.0;

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Step {
    // Working a door is a little errand: walk to its panel, and put a hand
    // on it. What to do to which door is on the task, not on the step.
    GoToSwitch,
    FlipSwitch,

    // A walk outside: a suit out of its locker, over to the deck inside the
    // port, out through the airlock, and back in and the suit hung up
    // again after. The one errand that leaves the hull is a construction
    // site beyond it, which turns off at `StepOut` and back at
    // `WalkToPort` (`Kind::fork`).
    GoToSuitLocker,
    TakeSuit,
    GoToGangway,
    StepOut,
    WalkToPort,
    StepIn,
    BackToSuitLocker,
    PutSuitBack,

    // Building. Standing beside a construction site and putting it
    // together for as long as the world says, `Kind::Build`. A site
    // outside the hull is the same errand with the suit and the airlock
    // either side of the walk to it, on the outside's grid: the walk out
    // borrows the walk outside's steps above, from `GoToSuitLocker` to
    // `StepOut` and from `WalkToPort` to `PutSuitBack`. Nothing is
    // carried — a part is paid for out of the crew's pool — so the room
    // changes no ship and says only, on `Room::built`, that the site is
    // done; the world takes the price and puts the part down.
    GoToSite,
    Construct,

    // A revive (task 120): over to a downed crewmate and hands on it for
    // the helper's revive time, riding in `rest_minutes` like a build's.
    // The room has not got the bodies, so `Revive` only says, on the way
    // out, who was brought round by whom (`Room::revived`), and `Game`
    // does it: the patient's health is a `Bim`'s. See `Kind::Revive`.
    GoToPatient,
    Revive,

    // A walk to a spot on the deck and nothing more — `Kind::Walk`, a
    // move the player gave with Shift held, waiting its turn on the queue
    // behind whatever the Bim is on. It is never *run* as a chain: the
    // game takes it off the queue and gives the walk the way a right-click
    // does (`Game::pump_queue`), so the plain's windows are walked leg by
    // leg like any other order.
    GoToSpot,

    // Laying a kit — `Kind::Deploy`: towards a tile beside the one it goes
    // on (`deploy_stand`), stopping once within `DEPLOY_REACH` of it
    // (`in_deploy_reach`), and the minutes at it, `rest_minutes` like a
    // build's. `Deploy` only says, on the way out, that it was laid
    // (`Room::deployed`); the world owns what was laid.
    GoToDeploySpot,
    Deploy,

    Done,
}

impl Step {
    fn next(self) -> Step {
        use Step::*;
        match self {
            GoToSwitch => FlipSwitch,
            GoToSuitLocker => TakeSuit,
            TakeSuit => GoToGangway,
            GoToGangway => StepOut,
            // Nothing is done out there on its own account: the one errand
            // that steps out is a build, and it forks here.
            StepOut => WalkToPort,
            WalkToPort => StepIn,
            StepIn => BackToSuitLocker,
            BackToSuitLocker => PutSuitBack,
            // The building errand as it runs inside the hull; the outside
            // variant turns off through the airlock in `Kind::steps` and
            // `Task::next_step`.
            GoToSite => Construct,
            GoToPatient => Revive,
            GoToSpot => Done,
            GoToDeploySpot => Deploy,
            FlipSwitch | PutSuitBack | Construct | Revive | Deploy | Done => Done,
        }
    }

    /// How long a stationary step lasts. Walking steps run until the Bim
    /// arrives, so their length here is ignored, and the steps that run for
    /// as long as the task was told — see `Task::duration` — are not here
    /// either.
    fn duration(self) -> f32 {
        use Step::*;
        match self {
            FlipSwitch => 0.5,
            TakeSuit | PutSuitBack => 1.5,
            StepOut | StepIn => 1.0,
            _ => 0.0,
        }
    }

    fn is_walk(self) -> bool {
        use Step::*;
        matches!(
            self,
            GoToSwitch
                | GoToSuitLocker
                | GoToGangway
                | WalkToPort
                | BackToSuitLocker
                | GoToSite
                | GoToPatient
                | GoToSpot
                | GoToDeploySpot
        )
    }
}

/// Which errand is running. The host labels its status line off this.
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Kind {
    /// Walking over to a door and working its panel by hand: none of them
    /// answer from across the room.
    Switch(Switch),
    /// Putting construction site `site` together, standing beside it, for
    /// the minutes riding in `rest_minutes`.
    /// `outside` is whether the site is beyond the hull, in which case the
    /// walk goes out through the airlock in a suit and back in again
    /// after — decided when the errand is begun, off whether any tile
    /// beside the site can be stood on from the deck. Nothing is carried:
    /// a part is paid for out of the crew's pool since feature 95. The
    /// room says it is done on `Room::built`; the world takes the price
    /// and puts the part down.
    Build { site: u32, outside: bool },
    /// Reviving `patient`, a downed crewmate (task 120): over to where it
    /// lies — `Room::crew`, as the walk is entered — and standing beside
    /// it, hands on it, for the helper's revive time riding in
    /// `rest_minutes` (`Skill::revive`: ten seconds, four for a medic).
    /// `Revive` hands the pair back on `Room::revived` and the game checks
    /// the two are still together, and the patient still downed, when the
    /// hands come off.
    Revive { patient: usize },
    /// A walk to a spot on the deck, given with Shift held so it waits its
    /// turn behind what the Bim is on: the spot rides on `Saved::target`.
    /// `post` is whether the Bim stands there once it arrives — a
    /// crewmate's under the alarm, a walk to a desk — the way the live
    /// order would have posted it. Never a running chain: see
    /// `Step::GoToSpot` and `Game::pump_queue`.
    Walk { post: bool },
    /// Laying one of an engineer's deployables on the deck tile `(x, y)`
    /// — a room tile, in tiles — `kind` the world's code for what it is
    /// (feature 74, task 127), which the room only carries back.
    /// The walk towards it — over as soon as the Bim is within
    /// [`DEPLOY_REACH`] tiles of it with a clear line, at the latest
    /// beside it — and the minutes riding in `rest_minutes` of working
    /// steps there, with `effort` on them the way a build's are; the
    /// walk counts nothing towards its bar. The room says it was laid on
    /// `Room::deployed`; the world puts the deployable down and spends
    /// the charge then, so a deploy given up has spent nothing. A hit
    /// does not put it down, and it is never put down onto the queue:
    /// anything that displaces it drops it (`Game::interrupt`).
    Deploy { x: i32, y: i32, kind: u32 },
}

impl Kind {
    /// Where this chain begins. Needed to walk a chain from the top, which is
    /// how a half-finished one works out where to pick itself up.
    fn first_step(self) -> Step {
        match self {
            Kind::Switch(_) => Step::GoToSwitch,
            Kind::Build { outside: true, .. } => Step::GoToSuitLocker,
            Kind::Build { outside: false, .. } => Step::GoToSite,
            Kind::Revive { .. } => Step::GoToPatient,
            Kind::Walk { .. } => Step::GoToSpot,
            Kind::Deploy { .. } => Step::GoToDeploySpot,
        }
    }

    /// The middle of the tile a deploy is laying its kit on, in room
    /// units, or `None` for every other errand.
    pub fn deploy_tile(self) -> Option<Vec2> {
        match self {
            Kind::Deploy { x, y, .. } => {
                Some(vec2((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE))
            }
            _ => None,
        }
    }

    /// The crewmate a revive walks to, or `None` for every other errand.
    pub fn patient(self) -> Option<usize> {
        match self {
            Kind::Revive { patient } => Some(patient),
            _ => None,
        }
    }

    /// The site a building errand is about, or `None` for every other
    /// errand.
    pub fn site(self) -> Option<u32> {
        match self {
            Kind::Build { site, .. } => Some(site),
            _ => None,
        }
    }

    /// Whether this errand goes out through the airlock.
    fn goes_outside(self) -> bool {
        matches!(self, Kind::Build { outside: true, .. })
    }

    /// The step after `here` where this chain turns off the straight line
    /// `Step::next` draws — the forks that are the same for `steps` and for
    /// `Task::next_step`, so the two cannot disagree about the shape of a
    /// chain. `None` where the line is followed.
    fn fork(self, here: Step) -> Option<Step> {
        Some(match (self, here) {
            // A build outside goes out through the airlock with the Bim,
            // and the Bim comes back in when the part is down.
            (Kind::Build { outside: true, .. }, Step::StepOut) => Step::GoToSite,
            (Kind::Build { outside: true, .. }, Step::Construct) => Step::WalkToPort,
            _ => return None,
        })
    }

    /// Every step of the chain, in order.
    fn steps(self) -> impl Iterator<Item = Step> {
        let mut at = Some(self.first_step());
        core::iter::from_fn(move || {
            let here = at?;
            let next = self.fork(here).unwrap_or_else(|| here.next());
            at = if next == Step::Done { None } else { Some(next) };
            Some(here)
        })
    }
}

/// A chain put down part-way through, and everything needed to pick it up:
/// which step it had reached and how far into it, and what it was holding.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Saved {
    /// Which Bim this chain belongs to. A chain is never handed over — it
    /// goes back on the queue of the Bim that put it down.
    who: usize,
    kind: Kind,
    step: Step,
    elapsed: f32,
    rest_minutes: f32,
    /// The spot a queued walk is bound for, or the spot a walk had chosen.
    target: Option<Vec2>,
    main: Held,
    tool: Held,
    /// An order the player gave with Shift held, waiting its turn — never
    /// begun, so it is *begun* when its turn comes, the way the live order
    /// would have been, rather than resumed: what cannot be begun is
    /// dropped rather than walked through into nothing. See
    /// `Game::order_later` and `Game::pump_queue`. Off for a chain that was
    /// put down.
    ordered: bool,
}

impl Saved {
    /// An errand that has not started yet, dressed as one that was put down at
    /// its first step. It is the same thing to everything downstream — the
    /// queue, the agenda, the resume — which is what lets work be *added* to
    /// the queue rather than only ever put back into it.
    pub fn fresh(who: usize, kind: Kind, rest_minutes: f32) -> Saved {
        Saved {
            who,
            kind,
            step: kind.first_step(),
            elapsed: 0.0,
            rest_minutes,
            target: None,
            main: Held::Nothing,
            tool: Held::Nothing,
            ordered: false,
        }
    }

    /// An order given for later: [`Saved::fresh`], flagged `ordered`, with
    /// the spot a walk is bound for on `target` (`None` for every other
    /// errand).
    pub fn ordered(who: usize, kind: Kind, rest_minutes: f32, target: Option<Vec2>) -> Saved {
        Saved {
            target,
            ordered: true,
            ..Saved::fresh(who, kind, rest_minutes)
        }
    }

    /// Whether this is an order waiting its turn rather than a chain put
    /// down. See the field.
    pub fn is_ordered(&self) -> bool {
        self.ordered
    }

    /// Whose chain this is.
    pub fn who(&self) -> usize {
        self.who
    }

    /// The spot a queued walk is bound for: `None` for a chain that has not
    /// chosen one.
    pub fn target(&self) -> Option<Vec2> {
        self.target
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// How far through the chain this was, for the host's readout.
    pub fn progress(&self) -> f32 {
        progress_of(self.kind, self.step, self.elapsed, self.rest_minutes)
    }

    pub fn rest_minutes(&self) -> f32 {
        self.rest_minutes
    }

    /// Where picking this chain up again would first walk the Bim to.
    pub fn resume_station(&self, room: &Room, from: Vec2) -> Option<Vec2> {
        destination(
            self.kind,
            rewind(self.kind, self.step),
            room,
            from,
            self.target,
        )
    }

    /// Put back what `set_scripted(false)` threw away.
    fn restore(&self, ch: &mut Character) {
        ch.hold_main(self.main);
        ch.hold_tool(self.tool);
    }
}

/// Where a walking step is headed, or `None` for a step that stands still.
///
/// Out here rather than inside `enter` so that the question "could the Bim
/// actually get there?" can be asked before a chain is started, with the same
/// answer the chain itself would get.
fn destination(
    kind: Kind,
    step: Step,
    room: &Room,
    from: Vec2,
    target: Option<Vec2>,
) -> Option<Vec2> {
    use Step::*;
    match step {
        GoToSwitch => match kind {
            Kind::Switch(which) => Some(room.switch_station(which, from)),
            _ => None,
        },
        // A room without a suit locker or a port has nowhere to send the
        // Bim, and the errand is not begun — `Game::can_go_outside` asks.
        GoToSuitLocker | BackToSuitLocker => room.suit_locker_station(),
        GoToGangway => room.gangway,
        // Back to the spot outside the port.
        WalkToPort => room.outside,
        // A tile beside the site, chosen as the step is entered — from
        // wherever the Bim is standing, on whichever grid it is standing
        // on.
        GoToSite => target,
        // Beside the patient, chosen as the walk is entered from where the
        // patient lies then.
        GoToPatient => target,
        // And beside the weapon on the deck, likewise; and the spot a
        // queued walk was given for.
        GoToSpot | GoToDeploySpot => target,
        _ => None,
    }
}

/// Where a chain would first have to walk to, were it started now. `None`
/// when it starts on the spot — or picks its spot as it goes — and so cannot
/// be blocked at the outset.
pub fn first_station(kind: Kind, room: &Room, from: Vec2) -> Option<Vec2> {
    destination(kind, kind.first_step(), room, from, None)
}

/// The steps of an errand outside that happen beyond the door: from
/// stepping out to stepping in, for a build at a site outside the hull.
/// Nothing for an errand that stays aboard.
fn outside_half(kind: Kind, step: Step) -> bool {
    if !kind.goes_outside() {
        return false;
    }
    matches!(
        step,
        Step::StepOut | Step::GoToSite | Step::Construct | Step::WalkToPort
    )
}

/// The construction site an errand is about, as the world described it
/// this step, or `None` when the world no longer wants it — cancelled, or
/// built by somebody else.
fn site_of(room: &Room, site: u32) -> Option<&crate::game::Build> {
    room.builds.iter().find(|b| b.site == site)
}

/// The middle of a site: of the box round its tiles.
fn site_middle(build: &crate::game::Build) -> Vec2 {
    let mut lo = vec2(f32::MAX, f32::MAX);
    let mut hi = vec2(f32::MIN, f32::MIN);
    for tile in &build.tiles {
        lo = vec2(lo.x.min(tile.min.x), lo.y.min(tile.min.y));
        hi = vec2(hi.x.max(tile.max.x), hi.y.max(tile.max.y));
    }
    (lo + hi) * 0.5
}

/// Where to stand to work site `site` from `from`, on the grid a body
/// `outside` or not is on: the nearest tile beside its footprint — four
/// ways, never a corner — that the grid has a route to, and failing that
/// the nearest tile *of* the footprint, since deck plating and conduit go
/// under the Bim's own feet. `None` when the site is not there, or nothing
/// beside it can be stood on from here — which, asked of the deck's grid,
/// is what says a site is outside the hull.
///
/// One place, asked when the errand is begun and again as the walk to the
/// site is entered: the two have to agree or the chain loops.
pub fn site_stand(room: &Room, maps: &Maps, site: u32, from: Vec2, outside: bool) -> Option<Vec2> {
    let build = site_of(room, site)?;
    let nav = if outside {
        maps.outside()?
    } else {
        maps.deck()
    };
    let t = TILE;
    let mut ring: Vec<Vec2> = Vec::new();
    let mut own: Vec<Vec2> = Vec::new();
    for tile in &build.tiles {
        own.push(tile.center());
        for (dx, dy) in [(0.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
            let p = tile.center() + vec2(dx * t, dy * t);
            if !build.tiles.iter().any(|r| r.contains(p)) && !ring.contains(&p) {
                ring.push(p);
            }
        }
    }
    let pick = |mut spots: Vec<Vec2>| {
        spots.sort_by(|a, b| {
            (*a - from)
                .len()
                .partial_cmp(&(*b - from).len())
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        spots
            .into_iter()
            .find(|&p| nav.interior().contains(p) && nav.is_free(p) && nav.can_reach(from, p))
    };
    pick(ring).or_else(|| pick(own))
}

/// Whether a body at `at` is near enough the kit's tile `tile` to work
/// at it: within [`DEPLOY_REACH`] tiles, with a clear line on the deck.
pub fn in_deploy_reach(maps: &Maps, tile: Vec2, at: Vec2) -> bool {
    (tile - at).len() <= DEPLOY_REACH * TILE && maps.deck().line_clear(at, tile)
}

/// Where to stand to lay a kit on the tile whose middle is `tile`, from
/// `from`: the nearest of the four tiles beside it — never a corner, like
/// a site — that the deck's grid has a route to, and failing that the
/// tile itself, since a sandbag is laid at the feet. `None` with no
/// way to any of it. Asked when the order is given and again as the
/// walk is entered, like a site's, so the two agree.
pub fn deploy_stand(maps: &Maps, tile: Vec2, from: Vec2) -> Option<Vec2> {
    let nav = maps.deck();
    let t = TILE;
    let mut ring: Vec<Vec2> = [(0.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (0.0, 1.0)]
        .into_iter()
        .map(|(dx, dy)| tile + vec2(dx * t, dy * t))
        .collect();
    ring.sort_by(|a, b| {
        (*a - from)
            .len()
            .partial_cmp(&(*b - from).len())
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    ring.into_iter()
        .chain(core::iter::once(tile))
        .find(|&p| nav.interior().contains(p) && nav.is_free(p) && nav.can_reach(from, p))
}

/// Where to stand to revive `patient`, for `who` standing at `from`: the
/// nearest free cell to a point a tile from the patient towards the
/// helper — beside the body, not on it.
/// `None` for a patient the room has no position for (dead, outside) or
/// no route to.
///
/// Asked as the walk is entered, from where the patient stands *then*
/// (`Room::crew`), so a chain picked up off the queue walks to where the
/// patient has got to rather than where it was — and by
/// `Game::medical_on_offer` before the errand is offered at all, so a
/// patient nobody can get to is not a chain started and given up every
/// step.
pub fn patient_stand(
    room: &Room,
    maps: &Maps,
    who: usize,
    patient: usize,
    from: Vec2,
) -> Option<Vec2> {
    if patient == who {
        return Some(from);
    }
    let at = room.patient_at(patient)?;
    let nav = maps.deck();
    let toward = from - at;
    let step = if toward.len() > 1e-3 {
        toward * (TILE / toward.len())
    } else {
        vec2(TILE, 0.0)
    };
    let stand = nav.nearest_free(at + step);
    nav.can_reach(from, stand).then_some(stand)
}

/// The step to start at when picking `target` up again.
///
/// Standing steps assume the Bim is already in the right place, so resuming
/// rewinds to the most recent walking step and lets the Bim walk back to the
/// bench first.
fn rewind(kind: Kind, target: Step) -> Step {
    // An errand outside put down out there — or at the door — starts its
    // outside half again from the gangway: the body was brought in when
    // the chain was put down, and the spot beside the site is chosen
    // afresh once it is out again. See `Task::resume`.
    if outside_half(kind, target) {
        return Step::GoToGangway;
    }
    let mut back_at = kind.first_step();
    for step in kind.steps() {
        if step.is_walk() {
            back_at = step;
        }
        if step == target {
            break;
        }
    }
    back_at
}

/// A rough length for a step, for the progress readout only. Walking steps
/// have no fixed length — it depends where the Bim happens to be standing —
/// so they get a nominal figure. Being a second out only nudges a bar.
const NOMINAL_WALK: f32 = 3.0;

fn weight(step: Step, rest_minutes: f32) -> f32 {
    if step.is_walk() {
        NOMINAL_WALK
    } else if matches!(step, Step::Construct | Step::Revive | Step::Deploy) {
        clock::seconds(rest_minutes)
    } else {
        step.duration().max(0.05)
    }
}

/// How far through a chain a given step is, 0 to 1, weighted by how long each
/// step takes rather than by how many there are — otherwise a twenty-minute
/// treatment would read as half done the moment the helper arrived.
///
/// A deploy's walk weighs nothing: its bar is the work within reach of the
/// tile alone, and stands empty on the tile until the engineer is there.
fn progress_of(kind: Kind, step: Step, elapsed: f32, rest_minutes: f32) -> f32 {
    let weight = |s: Step| {
        if matches!(kind, Kind::Deploy { .. }) && s.is_walk() {
            0.0
        } else {
            weight(s, rest_minutes)
        }
    };
    let mut total = 0.0;
    let mut before = None;
    for s in kind.steps() {
        if s == step && before.is_none() {
            before = Some(total);
        }
        total += weight(s);
    }
    if total <= 0.0 {
        return 1.0;
    }
    let Some(before) = before else {
        return 1.0;
    };
    let here = weight(step);
    let within = if here > 0.0 {
        (elapsed / here).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((before + within * here) / total).clamp(0.0, 1.0)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Task {
    /// Which Bim is doing this.
    who: usize,
    kind: Kind,
    step: Step,
    /// Time spent in the current step.
    elapsed: f32,
    /// How long the working step runs, in game minutes — a build, a
    /// revive, a kit laid. Zero for every other errand.
    rest_minutes: f32,
    /// The spot a walk that picks its own is going to: beside a site, a
    /// patient, a tile a kit goes on. Chosen
    /// as the walk is entered.
    target: Option<Vec2>,
    /// Where the patient stood when the walk over to it was planned
    /// (`GoToPatient`). A patient that has since moved more than
    /// [`FOLLOW_SLACK`] from there — running from a fight, say — has the
    /// walk planned again from where it is now (`Task::update`), so the
    /// helper arrives beside the patient and not at an empty spot. Not on
    /// `Saved`: a walk picked up again plans afresh anyway.
    patient_at: Option<Vec2>,
    /// Set when a walk has nowhere to go. The chain is given up on the next
    /// tick rather than pretending it arrived.
    blocked: bool,
    /// Set while walking back to a chain that was put down: the step to drop
    /// into, with its progress, once the Bim is in position again.
    resume: Option<Saved>,
}

impl Task {
    /// `rest_minutes` is how long the working step runs; an errand with no
    /// such step passes zero.
    fn starting_at(
        who: usize,
        kind: Kind,
        step: Step,
        rest_minutes: f32,
        ch: &mut Character,
        room: &mut Room,
        maps: &Maps,
    ) -> Task {
        ch.set_scripted(true);
        let mut task = Task {
            who,
            kind,
            step,
            elapsed: 0.0,
            rest_minutes,
            target: None,
            patient_at: None,
            blocked: false,
            resume: None,
        };
        task.enter(ch, room, maps);
        task
    }

    /// Walk over to a door's panel and work it. Every door the Bim can
    /// operate goes through here, so nothing can be changed without the Bim
    /// being there to change it.
    pub fn work_switch(
        who: usize,
        which: Switch,
        ch: &mut Character,
        room: &mut Room,
        maps: &Maps,
    ) -> Task {
        Task::starting_at(
            who,
            Kind::Switch(which),
            Step::GoToSwitch,
            0.0,
            ch,
            room,
            maps,
        )
    }

    /// Off to put site `site` together, for `minutes` beside it, out
    /// through the airlock and back if it is `outside` the hull.
    pub fn build(
        who: usize,
        site: u32,
        outside: bool,
        minutes: f32,
        ch: &mut Character,
        room: &mut Room,
        maps: &Maps,
    ) -> Task {
        let kind = Kind::Build { site, outside };
        Task::starting_at(who, kind, kind.first_step(), minutes, ch, room, maps)
    }

    /// Off to revive `patient` — over to wherever it lies, and `seconds`
    /// with hands on it (a minute of the room's clock is a second).
    pub fn revive(
        who: usize,
        patient: usize,
        seconds: f32,
        ch: &mut Character,
        room: &mut Room,
        maps: &Maps,
    ) -> Task {
        Task::starting_at(
            who,
            Kind::Revive { patient },
            Step::GoToPatient,
            seconds * clock::MINUTES_PER_SECOND,
            ch,
            room,
            maps,
        )
    }

    /// Off to lay deployable `kind` (the world's code) on the tile whose
    /// middle is `tile`, for `minutes` of working steps within reach of it.
    pub fn deploy(
        who: usize,
        tile: Vec2,
        kind: u32,
        minutes: f32,
        ch: &mut Character,
        room: &mut Room,
        maps: &Maps,
    ) -> Task {
        let t = TILE;
        let kind = Kind::Deploy {
            x: (tile.x / t).floor() as i32,
            y: (tile.y / t).floor() as i32,
            kind,
        };
        Task::starting_at(who, kind, kind.first_step(), minutes, ch, room, maps)
    }

    /// The step after this one: the straight line, or the fork a build
    /// outside takes through the airlock.
    fn next_step(&self) -> Step {
        self.kind
            .fork(self.step)
            .unwrap_or_else(|| self.step.next())
    }

    /// How long the step running now lasts. The working steps run for as
    /// long as the Bim was told; everything else is a fixed length.
    fn duration(&self) -> f32 {
        match self.step {
            Step::Construct | Step::Revive | Step::Deploy => clock::seconds(self.rest_minutes),
            step => step.duration(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.step == Step::Done
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn rest_minutes(&self) -> f32 {
        self.rest_minutes
    }

    /// How far through the chain the Bim is, 0 to 1.
    pub fn progress(&self) -> f32 {
        let at = self.resume.as_ref().map_or(self.step, |s| s.step);
        let elapsed = self.resume.as_ref().map_or(self.elapsed, |s| s.elapsed);
        progress_of(self.kind, at, elapsed, self.rest_minutes)
    }

    /// Give this chain up for good, the way a blocked one is: whatever is in
    /// the Bim's hands goes back where it came from. For a Bim leaving the
    /// room altogether.
    pub fn abandon(self, ch: &mut Character, room: &mut Room) {
        Task::let_go(true, ch, room);
        ch.set_scripted(false);
    }

    /// Put this chain down and hand back everything needed to resume it.
    /// Consumes the task: a suspended chain lives in the queue, not here.
    pub fn suspend(self, ch: &mut Character, room: &mut Room) -> Saved {
        // A chain already walking back to where it left off is saved at the
        // step it was heading for, not at the walk.
        let saved = match self.resume {
            Some(saved) => saved,
            None => Saved {
                who: self.who,
                kind: self.kind,
                step: self.step,
                elapsed: self.elapsed,
                rest_minutes: self.rest_minutes,
                target: self.target,
                main: ch.main_held(),
                tool: ch.tool_held(),
                ordered: false,
            },
        };
        // `false`: a suspended chain is kept, not given up, and a kit in the
        // hands is on `saved.main` and comes back with the chain.
        Task::let_go(false, ch, room);
        ch.set_scripted(false);
        saved
    }

    /// Leave the world in a state the Bim can walk away from.
    fn let_go(_for_good: bool, ch: &mut Character, room: &mut Room) {
        // A walk outside given up brings the body back in through the door,
        // whatever it was doing out there: a suspended one resumes from the
        // gangway and goes out again, and an abandoned one is simply back.
        if ch.is_outside()
            && let Some(gangway) = room.gangway
        {
            ch.come_inside(gangway);
        }
    }

    /// Pick a chain back up. The Bim walks to the last place the chain had it
    /// standing, and only then drops back into the step it was on.
    pub fn resume(saved: Saved, ch: &mut Character, room: &mut Room, maps: &Maps) -> Task {
        let back_at = rewind(saved.kind, saved.step);
        ch.set_scripted(true);
        let mut task = Task {
            who: saved.who,
            kind: saved.kind,
            step: back_at,
            elapsed: 0.0,
            rest_minutes: saved.rest_minutes,
            target: saved.target,
            patient_at: None,
            blocked: false,
            // Interrupted on the walk itself: there is nothing to drop into
            // afterwards, it simply walks it again. A walk outside put down
            // beyond the door is the same: it goes out again and picks its
            // spot afresh.
            resume: if back_at == saved.step || outside_half(saved.kind, saved.step) {
                None
            } else {
                Some(saved)
            },
        };
        task.enter(ch, room, maps);
        task
    }

    pub fn step(&self) -> Step {
        self.step
    }

    /// Whether the hands are on a downed crewmate this instant: the revive
    /// itself, not the walk to the patient. A Bim reviving has no hand free
    /// for a weapon — `Game::tick_combat` holsters it for the while.
    pub fn is_reviving(&self) -> bool {
        self.step == Step::Revive
    }

    /// Whether the hands are on a kit this instant: the laying itself,
    /// not the walk over. An engineer building fires nothing —
    /// `Game::tick_combat` holsters the weapon for the while.
    pub fn is_laying(&self) -> bool {
        self.step == Step::Deploy
    }

    /// How far through the hands-on part of a revive this chain is, from
    /// nought to one — the walk over not counted — or `None` while it is
    /// not kneeling at the patient. What the bar over a downed body reads.
    pub fn revive_share(&self) -> Option<f32> {
        if self.step != Step::Revive {
            return None;
        }
        let total = self.duration();
        Some(if total > 0.0 {
            (self.elapsed / total).clamp(0.0, 1.0)
        } else {
            1.0
        })
    }

    /// Set the Bim and the room up for whichever step we just moved into.
    fn enter(&mut self, ch: &mut Character, room: &mut Room, maps: &Maps) {
        use Step::*;
        self.elapsed = 0.0;

        // The walks that pick their own spot pick it now, from wherever the
        // Bim stands, on the grid it is on. A spot there is no longer any
        // way to — a site cancelled, a patient dead, a weapon picked up —
        // gives the errand up the way a locked door gives one up.
        match self.step {
            Step::GoToSite => {
                self.target = self
                    .kind
                    .site()
                    .and_then(|site| site_stand(room, maps, site, ch.pos, ch.is_outside()));
                if self.target.is_none() {
                    self.blocked = true;
                    return;
                }
            }
            // Beside the patient, wherever it stands now. Nowhere — the
            // patient dead since the order, or outside — and the errand is
            // given up like any other walk with nowhere to go.
            Step::GoToPatient => {
                let Some(patient) = self.kind.patient() else {
                    unreachable!("GoToPatient is a Revive's step")
                };
                self.target = patient_stand(room, maps, self.who, patient, ch.pos);
                self.patient_at = room.patient_at(patient);
                if self.target.is_none() {
                    self.blocked = true;
                    return;
                }
            }
            // Beside the tile the kit goes on, from wherever the Bim stands.
            Step::GoToDeploySpot => {
                self.target = self
                    .kind
                    .deploy_tile()
                    .and_then(|tile| deploy_stand(maps, tile, ch.pos));
                if self.target.is_none() {
                    self.blocked = true;
                    return;
                }
            }
            _ => {}
        }

        if let Some(to) = destination(self.kind, self.step, room, ch.pos, self.target) {
            // Routed around the furniture, same as a player order. The grid is
            // chosen here rather than by the caller because the step before
            // this one may have just stepped out of the airlock, which puts
            // the body on the outside's grid.
            let nav = maps.for_who(self.who, ch.is_outside(), ch.is_afield());
            if !ch.walk_to(nav, to, ch.is_afield()) {
                // Nowhere to walk — a door has been locked across the way.
                // The chain gives up here rather than carrying on: an empty
                // route leaves `arrived` true straight away, and a chain that
                // takes that for arrival runs itself through every remaining
                // step in one frame and flings the Bim across the deck the
                // moment one of them sets a position.
                self.blocked = true;
                return;
            }
            ch.set_action(Action::None);
            return;
        }

        // Standing steps: face the work and start the right animation.
        match self.step {
            // A door can be in any wall, so the Bim turns to whichever one
            // it walked up to rather than to a fixed heading.
            FlipSwitch => {
                if let Kind::Switch(which) = self.kind {
                    ch.face(room.switch_facing(which, ch.pos));
                }
                ch.set_action(Action::Reach);
            }
            // At the locker, facing it; at the port, facing out; and back
            // in, facing the deck.
            TakeSuit | PutSuitBack => {
                ch.face(room.suit_locker_facing());
                ch.set_action(Action::Reach);
            }
            StepOut => {
                ch.face(room.port_facing());
                ch.set_action(Action::Reach);
            }
            StepIn => {
                if let Some(gangway) = room.gangway {
                    ch.come_inside(gangway);
                }
                ch.face(room.port_facing() + PI);
                ch.set_action(Action::Reach);
            }
            // At the site, turned to it, the arms going at the work.
            Construct => {
                if let Some(build) = self.kind.site().and_then(|site| site_of(room, site)) {
                    let d = site_middle(build) - ch.pos;
                    ch.face(d.y.atan2(d.x));
                }
                ch.set_action(Action::Chop);
            }
            // Turned to the patient, kneeling at it.
            Revive => {
                if let Some(patient) = self.kind.patient()
                    && patient != self.who
                    && let Some(at) = room.patient_at(patient)
                {
                    let d = at - ch.pos;
                    if d.len() > 1e-3 {
                        ch.face(d.y.atan2(d.x));
                    }
                }
                ch.set_action(Action::Revive);
            }
            // Turned to the tile the kit goes on, hands at it.
            Deploy => {
                if let Some(tile) = self.kind.deploy_tile() {
                    let d = tile - ch.pos;
                    if d.len() > 1e-3 {
                        ch.face(d.y.atan2(d.x));
                    }
                }
                ch.set_action(Action::Chop);
            }
            _ => ch.set_action(Action::None),
        }
    }

    /// Everything that changes as a step finishes.
    fn leave(&mut self, ch: &mut Character, room: &mut Room) {
        use Step::*;
        match self.step {
            // Out through the door: held beyond the hull, in the suit,
            // until `StepIn` brings the body back. Nothing is carried out
            // there and nothing comes back.
            StepOut => {
                if let Some(outside) = room.outside {
                    ch.go_outside(outside, room.port_facing(), false);
                }
            }
            // Built: the world puts the part down. Off the room's list at
            // once, so the same site is not offered again before the world
            // has spoken.
            Construct => {
                if let Some(site) = self.kind.site() {
                    room.built.push((site, self.who));
                    room.builds.retain(|b| b.site != site);
                }
            }
            // Hands off the patient: the room says who was brought round
            // by whom, and the game — which has the body — does it, if the
            // two are still together and the patient still down.
            Revive => {
                if let Kind::Revive { patient } = self.kind {
                    room.revived.push((self.who, patient));
                }
            }
            // Laid: the room says who laid what where, and the world puts
            // the deployable down and takes the kit out of the pack.
            Deploy => {
                if let (Kind::Deploy { kind, .. }, Some(tile)) =
                    (self.kind, self.kind.deploy_tile())
                {
                    room.deployed.push((self.who, tile, kind));
                }
            }
            // The player asked for the door as it is when the Bim's hand
            // reaches the panel, so the order is given now.
            FlipSwitch => {
                if let Kind::Switch(which) = self.kind {
                    room.work_switch(which);
                }
            }
            _ => {}
        }
    }

    /// `effort` is how fast it is getting on with things, 1 for a Bim in good
    /// order: a commander's aura, an engineer's build. It stretches every
    /// step of the errand.
    pub fn update(
        &mut self,
        dt: f32,
        ch: &mut Character,
        room: &mut Room,
        maps: &Maps,
        effort: f32,
    ) {
        if self.step == Step::Done {
            return;
        }
        let dt = dt * effort;

        // Nowhere to walk: give the errand up, and put the world back in a
        // state the Bim can be left in.
        if self.blocked {
            // This chain is over for good, so anything in the Bim's hands is
            // handed over with it.
            Task::let_go(true, ch, room);
            ch.set_scripted(false);
            self.step = Step::Done;
            return;
        }

        self.elapsed += dt;

        // A patient that has walked off since the walk over to it was
        // planned — more than `FOLLOW_SLACK` from where it stood then —
        // is followed: the walk is planned again from where it is now,
        // and nowhere to reach it is the errand given up like any other.
        // The Bim's own spot is where it already is, so only somebody
        // else's moves.
        if self.step == Step::GoToPatient
            && let Some(patient) = self.kind.patient()
            && patient != self.who
            && let Some(was) = self.patient_at
            && let Some(now) = room.patient_at(patient)
            && (now - was).len() > FOLLOW_SLACK * TILE
        {
            self.enter(ch, room, maps);
            return;
        }

        // Near enough the kit's tile to work at it: the walk over is done
        // where the body stands, beside the tile or not.
        let in_reach = self.step == Step::GoToDeploySpot
            && self.resume.is_none()
            && self
                .kind
                .deploy_tile()
                .is_some_and(|tile| in_deploy_reach(maps, tile, ch.pos));
        if in_reach && !ch.arrived() {
            ch.halt();
        }

        let finished = if self.step.is_walk() {
            ch.arrived() || in_reach
        } else {
            // Standing steps also wait for the turn-on-the-spot to settle, so
            // the Bim is never seen reaching into a bench sideways.
            self.elapsed >= self.duration() && ch.facing_settled()
        };

        if finished {
            // Back in position after picking a chain up again: carry on from
            // the step it was put down on, with the progress it had.
            if let Some(saved) = self.resume.take() {
                saved.restore(ch);
                self.step = saved.step;
                self.enter(ch, room, maps);
                self.elapsed = saved.elapsed;
                return;
            }
            self.leave(ch, room);
            self.step = self.next_step();
            if self.step == Step::Done {
                ch.set_scripted(false);
                return;
            }
            self.enter(ch, room, maps);
        }
    }
}

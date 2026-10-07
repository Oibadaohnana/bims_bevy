//! Where the shape buffer's time goes, when somebody asks (task 122).
//!
//! The app's `perf` times a frame's named parts from outside; the shape
//! buffer (`ship::Session::render`) is one of them, and what it is made of
//! — each room's own picture, the trace and the light map inside the
//! crew's, the painter over both — is in crates the app's timers cannot
//! reach into. So the timers for those are here, in the lowest crate that
//! has any of them, and the app turns them on and reads them back beside
//! its own.
//!
//! Off, which is every run but a `BIMS_PERF=1` one, a [`scope`] is an
//! atomic load and a branch and the clock is never read — which is also
//! what keeps it harmless on a target whose `Instant` would panic. Std
//! alone, so a probe compiles it like any other module.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// A timed part of the shape buffer. The order is the order of the report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    /// The crew's room drawing itself (`Game::render` on `world.aboard`),
    /// the trace and the light map included.
    CrewRoom,
    /// Inside a room's picture: what the crew see traced again
    /// (`Game::observe`).
    Observe,
    /// Inside a room's picture: the smooth light map (`Sight::light_map`).
    LightMap,
    /// Inside the light map: each body that moved marched again
    /// (`Sight::view_of`), before the picture is composed.
    LightViews,
    /// The station's room drawing itself (`world.residents`).
    StationRoom,
    /// The ship's own bookkeeping between the rooms and the painter: the
    /// frame count, the airlock, the cameras, the plain's pictures and the
    /// blueprint's answer.
    ShipState,
    /// The ship painter over all of it (`world_paint::paint`).
    WorldPaint,
    /// Inside the painter: the stations and their rooms' pictures placed.
    Stations,
    /// Inside the stations: a station's own picture built, its hull, its
    /// lights and what stands at it, before it is turned into place.
    StationPicture,
    /// Inside a station's picture: its grid, what its room draws and its
    /// ground worked out.
    StationPrep,
    /// Inside a station's picture: its rim and its tiles (`hull_tiles`).
    StationHull,
    /// Inside a station's picture: its lights, its lamps' glass, a relic
    /// cache and the machines' ship.
    StationLights,
    /// Inside the stations: the shade along a station's walls.
    StationShade,
    /// Inside the painter: the planet's plain.
    Plain,
}

impl Part {
    pub const ALL: [Part; 14] = [
        Part::CrewRoom,
        Part::Observe,
        Part::LightMap,
        Part::LightViews,
        Part::StationRoom,
        Part::ShipState,
        Part::WorldPaint,
        Part::Stations,
        Part::StationPicture,
        Part::StationPrep,
        Part::StationHull,
        Part::StationLights,
        Part::StationShade,
        Part::Plain,
    ];

    /// What the row is called, and how deep it sits under the shape buffer.
    pub fn row(self) -> (&'static str, usize) {
        match self {
            Part::CrewRoom => ("crew room", 0),
            Part::Observe => ("observe", 1),
            Part::LightMap => ("light map", 1),
            Part::LightViews => ("views marched", 2),
            Part::StationRoom => ("station room", 0),
            Part::ShipState => ("ship state", 0),
            Part::WorldPaint => ("world paint", 0),
            Part::Stations => ("stations", 1),
            Part::StationPicture => ("station picture", 2),
            Part::StationPrep => ("grid and skip", 3),
            Part::StationHull => ("hull tiles", 3),
            Part::StationLights => ("lights and lamps", 3),
            Part::StationShade => ("station shade", 2),
            Part::Plain => ("plain", 1),
        }
    }
}

const PARTS: usize = Part::ALL.len();

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);
static NANOS: [AtomicU64; PARTS] = [ZERO; PARTS];
static ON: AtomicBool = AtomicBool::new(false);

/// Start timing, from nought — or stop.
pub fn record(on: bool) {
    for slot in &NANOS {
        slot.store(0, Ordering::Relaxed);
    }
    MARCHES.store(0, Ordering::Relaxed);
    MARCH_FRAMES.store(0, Ordering::Relaxed);
    MARCH_MOST.store(0, Ordering::Relaxed);
    ON.store(on, Ordering::Relaxed);
}

/// What a part has added up to since [`record`], in milliseconds.
pub fn millis(part: Part) -> f64 {
    NANOS[part as usize].load(Ordering::Relaxed) as f64 / 1.0e6
}

/// Time a part until the guard is dropped.
pub fn scope(part: Part) -> Scope {
    Scope(ON.load(Ordering::Relaxed).then(|| (part, Instant::now())))
}

pub struct Scope(Option<(Part, Instant)>);

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((part, at)) = self.0 {
            NANOS[part as usize].fetch_add(at.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }
}

static MARCHES: AtomicU64 = AtomicU64::new(0);
static MARCH_FRAMES: AtomicU64 = AtomicU64::new(0);
static MARCH_MOST: AtomicU64 = AtomicU64::new(0);

/// One body's view marched again by the light map: how many a frame is
/// what splitting the march body by body could be worth.
pub fn marched() {
    if ON.load(Ordering::Relaxed) {
        MARCHES.fetch_add(1, Ordering::Relaxed);
    }
}

/// One light map that marched `n` views again, `n` over nought: how the
/// marches fall across frames, which is what a split body by body is
/// bounded by.
pub fn marched_together(n: u64) {
    if ON.load(Ordering::Relaxed) {
        MARCH_FRAMES.fetch_add(1, Ordering::Relaxed);
        MARCH_MOST.fetch_max(n, Ordering::Relaxed);
    }
}

/// How many views were marched since [`record`].
pub fn marches() -> u64 {
    MARCHES.load(Ordering::Relaxed)
}

/// How many light maps marched anything, and the most views one marched.
pub fn march_frames() -> (u64, u64) {
    (
        MARCH_FRAMES.load(Ordering::Relaxed),
        MARCH_MOST.load(Ordering::Relaxed),
    )
}

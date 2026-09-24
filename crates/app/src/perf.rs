//! Where a frame goes, when somebody asks (feature 96).
//!
//! `BIMS_PERF=1` turns on a handful of timers round the named parts of a
//! frame — the world's steps, the panels' layout, the shape buffer, the
//! tessellation — and a smoke run prints what they add up to when it
//! exits, beside the frame time it already prints. Off, which is every
//! ordinary run, a [`scope`] is an atomic load and a branch: nothing is
//! timed and nothing is printed, so the normal build carries no clock.
//!
//!   BIMS_PERF=1 BIMS_SMOKE_FREE=1 BIMS_SMOKE_FRAMES=400 \
//!     ./hidden target/release/bims droids
//!
//! The first [`WARMUP`] frames are thrown away and the timers started
//! again after them: a window's first frames are Bevy coming up, the
//! canvas being fitted and the room being laid out, and none of that is
//! what a frame costs once the game is running.
//!
//! The report is a tree, since the scopes nest: `frame` is the screen's
//! whole system and the rows under it are parts of it, so they sum to a
//! little under it and the rest is the screen's own arithmetic. What is
//! left between `frame` and the wall clock is Bevy and egui — input,
//! egui's own tessellation of the panels, and handing the meshes to the
//! GPU — which is not ours to put a scope inside of.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// The frames a run throws away before the timers are started again.
pub const WARMUP: u32 = 100;

/// A part of a frame that is timed. The order is the order of the report
/// and `depth` is how far it is indented under the one above it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Frame,
    Wire,
    Step,
    Prep,
    Canvas,
    Panels,
    Render,
    Tessellate,
    Overlay,
}

impl Phase {
    pub const ALL: [Phase; 9] = [
        Phase::Frame,
        Phase::Wire,
        Phase::Step,
        Phase::Prep,
        Phase::Canvas,
        Phase::Panels,
        Phase::Render,
        Phase::Tessellate,
        Phase::Overlay,
    ];

    /// What the row is called, and how deep it sits under `frame`.
    fn row(self) -> (&'static str, usize) {
        match self {
            Phase::Frame => ("frame", 0),
            Phase::Wire => ("wire", 1),
            Phase::Step => ("world steps", 1),
            Phase::Prep => ("panel prep", 1),
            Phase::Canvas => ("canvas ui", 1),
            Phase::Panels => ("panels ui", 1),
            Phase::Render => ("shape buffer", 1),
            Phase::Tessellate => ("tessellate", 1),
            Phase::Overlay => ("overlay words", 1),
        }
    }
}

/// A thing counted rather than timed: how big the picture was.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Count {
    Shapes,
    Floats,
    Steps,
}

impl Count {
    pub const ALL: [Count; 3] = [Count::Shapes, Count::Floats, Count::Steps];

    fn row(self) -> &'static str {
        match self {
            Count::Shapes => "shapes drawn",
            Count::Floats => "floats replayed",
            Count::Steps => "world steps",
        }
    }
}

const PHASES: usize = Phase::ALL.len();
const COUNTS: usize = Count::ALL.len();

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);
static NANOS: [AtomicU64; PHASES] = [ZERO; PHASES];
static HITS: [AtomicU64; PHASES] = [ZERO; PHASES];
static TALLY: [AtomicU64; COUNTS] = [ZERO; COUNTS];
static RECORDING: AtomicBool = AtomicBool::new(false);
static START: OnceLock<std::sync::Mutex<Option<(Instant, u32)>>> = OnceLock::new();

/// `BIMS_PERF=1`: whether a frame is timed at all. Read once.
pub fn wanted() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_PERF").as_deref() == Ok("1"))
}

fn recording() -> bool {
    RECORDING.load(Ordering::Relaxed)
}

/// Throw away what the first frames cost and start the clock again. The
/// frame this was called at is remembered, so the report knows how many
/// frames the totals are spread over.
pub fn begin(at_frame: u32) {
    for slot in NANOS.iter().chain(HITS.iter()).chain(TALLY.iter()) {
        slot.store(0, Ordering::Relaxed);
    }
    *START
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap() = Some((Instant::now(), at_frame));
    RECORDING.store(true, Ordering::Relaxed);
}

/// Time a part of a frame until the guard is dropped.
pub fn scope(phase: Phase) -> Scope {
    Scope(recording().then(|| (phase, Instant::now())))
}

pub struct Scope(Option<(Phase, Instant)>);

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((phase, at)) = self.0 {
            let i = phase as usize;
            NANOS[i].fetch_add(at.elapsed().as_nanos() as u64, Ordering::Relaxed);
            HITS[i].fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Add to one of the counted things.
pub fn tally(what: Count, n: u64) {
    if recording() {
        TALLY[what as usize].fetch_add(n, Ordering::Relaxed);
    }
}

/// The report, as the lines a smoke run prints. `wall_ms` is what the
/// whole run took by the clock, so the share is of a real frame rather
/// than of the part of it that is ours.
pub fn report(at_frame: u32) -> Vec<String> {
    let Some((since, from)) = *START
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
    else {
        return Vec::new();
    };
    RECORDING.store(false, Ordering::Relaxed);
    let frames = at_frame.saturating_sub(from).max(1) as f64;
    let wall_ms = since.elapsed().as_secs_f64() * 1000.0;
    let per_frame = wall_ms / frames;
    let mut lines = vec![format!(
        "perf: {frames:.0} frames after warm-up, {per_frame:.2} ms a frame by the clock"
    )];
    lines.push(format!(
        "perf: {:<20}{:>10}{:>10}{:>8}",
        "part", "total ms", "ms/frame", "share"
    ));
    let mut named = 0.0;
    for phase in Phase::ALL {
        let ms = NANOS[phase as usize].load(Ordering::Relaxed) as f64 / 1.0e6;
        let (name, depth) = phase.row();
        if depth > 0 {
            named += ms;
        }
        lines.push(format!(
            "perf: {:<20}{:>10.1}{:>10.3}{:>7.1}%",
            format!("{:indent$}{name}", "", indent = depth * 2),
            ms,
            ms / frames,
            100.0 * ms / wall_ms.max(1.0e-9),
        ));
    }
    let frame_ms = NANOS[Phase::Frame as usize].load(Ordering::Relaxed) as f64 / 1.0e6;
    for (name, ms) in [
        ("  the screen's own", (frame_ms - named).max(0.0)),
        ("bevy and egui", (wall_ms - frame_ms).max(0.0)),
    ] {
        lines.push(format!(
            "perf: {:<20}{:>10.1}{:>10.3}{:>7.1}%",
            name,
            ms,
            ms / frames,
            100.0 * ms / wall_ms.max(1.0e-9),
        ));
    }
    for what in Count::ALL {
        let n = TALLY[what as usize].load(Ordering::Relaxed) as f64;
        lines.push(format!(
            "perf: {:<20}{:>10.0}{:>10.1} a frame",
            what.row(),
            n,
            n / frames
        ));
    }
    lines
}

//! The infesting race, as the world keeps it (feature 83).
//!
//! The machines themselves are `bims::droid` — what one is, what its
//! body is, and what it looks like. This is the other half: **which
//! stations are held, how many machines come, and when the next lot
//! arrive.**
//!
//! Since feature 92 it also holds the **crisis**: [`origin`], the one
//! star the machines begin at, and [`turns_on`], the day any star falls
//! — `first + DROID_SPREAD_DAYS * hops` along the galaxy's hyperlanes
//! (`worldgen::Galaxy::lanes`). Those two are the whole of the spread
//! rule, and neither keeps any state; `World::spread_crisis` is what
//! turns a day that has come into a station in the machines' hands.
//!
//! # A held station has waves, and the count is fixed at the first dock
//!
//! [`Infestation`] is one station's: how many waves are still to come,
//! which one is aboard, and the clock minute the next is due. It is
//! **saved and in `world_checksum`**, because it is the size of the
//! fight and two clients have to agree about it.
//!
//! - The **wave count** is worked out once, the first time the crew dock
//!   there, and never again. Wave one is aboard at that moment, stood
//!   about the station's rooms.
//! - The **wave size** is worked out as each wave appears, so a crew
//!   that has grown richer between waves meets more machines.
//! - When the last machine of a wave is destroyed and waves are left,
//!   the next arrives [`data::DROID_REINFORCE_STEPS`] of the **mission
//!   clock** later (feature 103). **Never while one is still standing.**
//! - Leaving a station before its last wave is destroyed puts it back as
//!   the crew met it (`crate::run`): the next visit is a fresh fight, its
//!   count worked out again at the day it is fought on.
//!
//! # How many
//!
//! Integers only, and none of `station::scaled`'s doubling: a wave grows
//! by *addition* with the crew, the calendar, the worth and the levels,
//! so a crew ten times as rich meets the formula's wave and not a
//! thousand machines. See [`wave_size`] and [`wave_count`].

use crate::data;
use economy::Money;
use worldgen::Galaxy;
use worldgen::rng::{Purpose, Rng};

/// One droid-held station's state. Which waves are left, which is
/// aboard, and when the next is due.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Infestation {
    pub station: u32,
    /// How many waves are still to arrive after the one aboard. Worked
    /// out at the crew's first dock ([`Infestation::settle`]) and only
    /// ever counted down.
    pub waves_left: u32,
    /// Which wave is aboard, counting from one; nought before the first
    /// has ever been laid.
    pub wave: u32,
    /// The step of the **mission clock** the next wave arrives at (feature
    /// 103), or `None` while one is still standing or there are none left.
    pub next_wave: Option<u64>,
    /// Whether the count has been worked out yet: the crew's first dock
    /// settles it, and nothing settles it twice.
    pub settled: bool,
    /// Whether the last machine of the last wave has been destroyed —
    /// said once, and what the crisis step reads
    /// (`World::droid_station_cleared`).
    pub cleared: bool,
}

impl Infestation {
    /// A station newly held: nothing worked out yet.
    pub fn new(station: u32) -> Infestation {
        Infestation {
            station,
            waves_left: 0,
            wave: 0,
            next_wave: None,
            settled: false,
            cleared: false,
        }
    }

    /// The crew have docked here for the first time: the wave count is
    /// fixed now and never worked out again, and wave one is aboard.
    /// Does nothing the second time.
    pub fn settle(&mut self, waves: u32) {
        if self.settled {
            return;
        }
        self.settled = true;
        // `waves` counts the one that is aboard, so what is *left* is
        // one fewer.
        self.waves_left = waves.saturating_sub(1);
        self.wave = 1;
        self.next_wave = None;
    }

    /// Whether any wave is still to come after the one aboard.
    pub fn more_to_come(&self) -> bool {
        self.waves_left > 0
    }
}

/// How many machines a wave is:
///
/// `DROID_WAVE_BASE + crew + days + worth_steps + levels / 3`, capped at
/// [`data::DROID_WAVE_MAX`].
///
/// - `crew` is how many are aboard,
/// - `days` is whole [`data::ENEMIES_DAYS`] the world has run,
/// - `worth_steps` is [`worth_steps`] — halves of the starting worth the
///   crew have grown by,
/// - `levels` is the crew's class levels less one each, added up.
///
/// Integers throughout, and nothing here doubles: a wave grows by
/// addition, which is what keeps a rich crew's fight a fight rather than
/// a slideshow.
pub fn wave_size(crew: u32, days: u32, worth_steps: u32, levels: u32) -> u32 {
    data::DROID_WAVE_BASE
        .saturating_add(crew)
        .saturating_add(days)
        .saturating_add(worth_steps)
        .saturating_add(levels / 3)
        .min(data::DROID_WAVE_MAX)
}

/// How many waves a held station has all told, the one aboard counted:
///
/// `DROID_WAVES_BASE + days + worth_steps / 2 + levels / 10`.
///
/// Worked out once, at the crew's first dock, and never again.
pub fn wave_count(days: u32, worth_steps: u32, levels: u32) -> u32 {
    data::DROID_WAVES_BASE
        .saturating_add(days)
        .saturating_add(worth_steps / 2)
        .saturating_add(levels / 10)
}

/// How many halves of its starting worth the crew have grown by:
/// `(worth − start) / (start / 2)`, and nought when the crew are no
/// richer than they began or the start was nothing. Whole euros in and a
/// whole number out, so a server counts the same.
pub fn worth_steps(worth: Money, start_worth: Money) -> u32 {
    let step = start_worth / 2;
    if step == 0 || worth <= start_worth {
        return 0;
    }
    ((worth - start_worth) / step) as u32
}

/// Whole [`data::ENEMIES_DAYS`] the world has run: the same calendar
/// step a garrison grows by.
pub fn day_steps(days_gone: u32) -> u32 {
    days_gone / data::ENEMIES_DAYS
}

// --- the crisis (feature 92) ---------------------------------------------

/// Where the machines began: the one star the crisis spreads out from.
///
/// Rolled once, at [`crate::World::start`], off its own stream — the
/// galaxy seed, the crew's starting star and [`Purpose::DroidOrigin`] —
/// so the same galaxy started from the same dock puts the machines in the
/// same place whoever asks, and moving anything else about the world
/// never moves them.
///
/// What is rolled among is **every star at least
/// [`data::DROID_ORIGIN_MIN_HOPS`] hops away** by the lane graph: which
/// star it is matters far less than how far off it is, since the whole
/// point of the roll is the months between the first news of the machines
/// and the first wave at the crew's own dock. A galaxy too small or too
/// stringy to have one that far away gives up the minimum and takes the
/// furthest it has — never the nearest, which would open the game with
/// the crisis next door.
pub fn origin(galaxy: &Galaxy, start_star: u32) -> u32 {
    let hops = galaxy.hops_from(start_star);
    let mut rng = Rng::stream(
        galaxy.seed,
        start_star,
        galaxy.generator_version,
        Purpose::DroidOrigin,
    );
    let far = candidates(&hops, data::DROID_ORIGIN_MIN_HOPS);
    let far = if far.is_empty() {
        // Nothing far enough: as far as this galaxy goes, which is at
        // least the star the crew are at and so never empty.
        let furthest = hops.iter().copied().filter(|&h| h != u16::MAX).max();
        candidates(&hops, furthest.unwrap_or(0))
    } else {
        far
    };
    let pick = rng.below(far.len() as u32) as usize;
    far.get(pick).copied().unwrap_or(start_star)
}

/// Every star at least `least` hops off, in id order. Unreachable stars
/// are left out — after [`worldgen::Galaxy::lanes`]'s completion there are
/// none, but a table asked about a star of another galaxy is all of them.
fn candidates(hops: &[u16], least: u16) -> Vec<u32> {
    hops.iter()
        .enumerate()
        .filter(|&(_, &h)| h != u16::MAX && h >= least)
        .map(|(id, _)| id as u32)
        .collect()
}

/// The day a star `hops` from the origin turns, counting from the day the
/// world opened: `first + DROID_SPREAD_DAYS * hops`, and [`u32::MAX`] —
/// never — for a star the lanes do not reach.
///
/// `first` is nought in the game — the crisis is there from the start
/// (feature 102) — and the `crisis` probe moves it (`BIMS_CRISIS_DAY`).
pub fn turns_on(first: u32, hops: u16) -> u32 {
    if hops == u16::MAX {
        return u32::MAX;
    }
    first.saturating_add(data::DROID_SPREAD_DAYS.saturating_mul(hops as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wave_is_the_sum_and_stops_at_the_cap() {
        // Nothing but the base and the crew to begin with.
        assert_eq!(wave_size(0, 0, 0, 0), data::DROID_WAVE_BASE);
        assert_eq!(wave_size(3, 0, 0, 0), data::DROID_WAVE_BASE + 3);
        // A month, a doubling of worth, and nine levels between them.
        assert_eq!(wave_size(1, 1, 2, 9), data::DROID_WAVE_BASE + 1 + 1 + 2 + 3);
        // Levels count in threes, so eight of them are two.
        assert_eq!(wave_size(0, 0, 0, 8), data::DROID_WAVE_BASE + 2);
        // And the cap holds however rich the crew are.
        assert_eq!(wave_size(50, 50, 50, 300), data::DROID_WAVE_MAX);
    }

    #[test]
    fn the_worth_steps_are_halves_and_never_a_doubling() {
        assert_eq!(worth_steps(100, 100), 0, "no richer, no steps");
        assert_eq!(worth_steps(50, 100), 0, "poorer, no steps");
        assert_eq!(worth_steps(150, 100), 1);
        assert_eq!(worth_steps(200, 100), 2);
        // Ten times as rich is eighteen steps, not ten doublings: the
        // wave is capped at sixteen and nothing has exploded.
        assert_eq!(worth_steps(1000, 100), 18);
        assert_eq!(wave_size(1, 0, 18, 0), data::DROID_WAVE_MAX);
        assert_eq!(worth_steps(100, 0), 0, "a start of nothing has no step");
    }

    #[test]
    fn the_wave_count_is_the_other_sum() {
        assert_eq!(wave_count(0, 0, 0), data::DROID_WAVES_BASE);
        assert_eq!(wave_count(2, 0, 0), data::DROID_WAVES_BASE + 2);
        assert_eq!(wave_count(0, 5, 0), data::DROID_WAVES_BASE + 2, "halves");
        assert_eq!(wave_count(0, 0, 25), data::DROID_WAVES_BASE + 2, "tenths");
    }

    #[test]
    fn a_count_is_settled_once_and_never_again() {
        let mut it = Infestation::new(7);
        assert!(!it.settled);
        it.settle(3);
        assert_eq!(it.wave, 1, "wave one is aboard at the first dock");
        assert_eq!(it.waves_left, 2, "and two more to come");
        // Richer, older, and it makes no difference: the count is fixed.
        it.settle(9);
        assert_eq!(it.waves_left, 2);
        assert!(it.more_to_come());
    }

    #[test]
    fn the_days_step_is_the_garrison_s_calendar() {
        assert_eq!(day_steps(0), 0);
        assert_eq!(day_steps(data::ENEMIES_DAYS - 1), 0);
        assert_eq!(day_steps(data::ENEMIES_DAYS), 1);
        assert_eq!(day_steps(data::ENEMIES_DAYS * 3 + 5), 3);
    }
}

// --- where a wave stands -------------------------------------------------

use shipdesign::dock::Port;
use shipdesign::parts::PartKind;
use shipdesign::{Layer, ShipDesign, TILE};

/// Every airlock of a design that opens onto space, in **part order** —
/// `shipdesign::dock::port` is the first of these, and it is the one the
/// crew dock at. The same "outward" rule: the side of the airlock with
/// nothing of the frame beyond it along its whole length.
pub fn airlocks(design: &ShipDesign) -> Vec<Port> {
    let grid = design.grid();
    let t = TILE as f64;
    design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Airlock)
        .filter_map(|airlock| {
            let tiles = airlock.tiles();
            let outward = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .find(|&(dx, dy)| {
                    tiles.iter().all(|&(x, y)| {
                        grid.get(Layer::Structure, (x as i32 + dx, y as i32 + dy)) == 0
                    })
                })?;
            let n = tiles.len() as f64;
            let centre = tiles.iter().fold((0.0, 0.0), |acc, &(x, y)| {
                (
                    acc.0 + (x as f64 + 0.5) * t / n,
                    acc.1 + (y as f64 + 0.5) * t / n,
                )
            });
            Some(Port {
                part_id: airlock.id,
                centre,
                outward,
            })
        })
        .collect()
}

/// The airlock a reinforcement ship ties up at: the one **farthest from
/// the port** — the first airlock, where the crew dock — and, where two
/// are equally far, the lower index. A station with one airlock has
/// nowhere else to put it, and the machines come in through the crew's
/// own door.
pub fn arrival_airlock(design: &ShipDesign) -> Option<Port> {
    let ports = airlocks(design);
    let first = *ports.first()?;
    ports
        .iter()
        .copied()
        .enumerate()
        .max_by(|(i, a), (j, b)| {
            let da = (a.centre.0 - first.centre.0).hypot(a.centre.1 - first.centre.1);
            let db = (b.centre.0 - first.centre.0).hypot(b.centre.1 - first.centre.1);
            // Farther wins; equally far, the lower index does, so the
            // max-by comparison prefers the earlier on a tie.
            da.total_cmp(&db).then(j.cmp(i))
        })
        .map(|(_, port)| port)
}

/// The spot `tiles` inside an airlock, in the design's own world units:
/// where a wave is posted, the way a raider's boarders are posted at the
/// ship's gangway.
pub fn inside_of(port: &Port, tiles: f64) -> (f64, f64) {
    let reach = tiles * TILE as f64;
    (
        port.centre.0 - port.outward.0 as f64 * reach,
        port.centre.1 - port.outward.1 as f64 * reach,
    )
}

/// Free deck tiles of a design, spread about it: every floor tile with
/// nothing standing on it, taken evenly across the list so a wave stood
/// about the station's rooms is not a wave in one corner. In design
/// world units.
pub fn spots_about(design: &ShipDesign, n: usize) -> Vec<(f64, f64)> {
    if n == 0 {
        return Vec::new();
    }
    let grid = design.grid();
    let t = TILE as f64;
    let free: Vec<(f64, f64)> = design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Floor)
        .map(|p| (p.origin.0 as i32, p.origin.1 as i32))
        .filter(|&tile| grid.get(Layer::Object, tile) == 0)
        .map(|(x, y)| ((x as f64 + 0.5) * t, (y as f64 + 0.5) * t))
        .collect();
    if free.is_empty() {
        return vec![(0.0, 0.0); n];
    }
    // Evenly across the list: the parts are in build order, which runs
    // over the design, so every `free.len() / n`-th tile is a spread.
    (0..n)
        .map(|i| free[(i * free.len()) / n.max(1) % free.len()])
        .collect()
}

/// The spot a wave arriving on a **surface** is posted at: just inside
/// the gate it comes through — north for an odd wave, south for an even
/// one — in the town design's own world units, with the way it faces.
/// `None` for a design that is not a town.
///
/// The lander itself is drawn on the plain beyond that gate, which is
/// the crew's room's to draw: the town's own room is the deck alone and
/// has no plain in it, so the machines are posted at the gate rather
/// than beside the lander and walk in from there.
pub fn gate_spot(side: u32, wave: u32) -> ((f64, f64), (f64, f64)) {
    let t = TILE as f64;
    let x = (crate::surface::GATE_X0 as f64 + crate::surface::GATE_WIDTH as f64 / 2.0) * t;
    let north = wave % 2 == 1;
    // The wall stands on the outermost deck tiles: row 1 in the north
    // and `side - 2` in the south. A tile inside each is the street.
    let (y, facing) = if north {
        ((2.5) * t, (0.0, 1.0))
    } else {
        ((side as f64 - 3.5) * t, (0.0, -1.0))
    };
    ((x, y), facing)
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use shipdesign::TILE;

    /// On a surface a wave walks in **through a gate**, north for an odd
    /// wave and south for an even one, and stands a tile inside the wall
    /// rather than on it. The gates are the west cross street where it
    /// meets the north wall and the south (`crate::surface`).
    #[test]
    fn a_surface_wave_comes_through_the_north_gate_and_then_the_south() {
        let side = data::SURFACE_SIDE;
        let t = TILE as f64;
        let gate_x = (crate::surface::GATE_X0 as f64 + crate::surface::GATE_WIDTH as f64 / 2.0) * t;

        // Wave two is even: the south gate, facing north into the town.
        let ((x, y), facing) = gate_spot(side, 2);
        assert!((x - gate_x).abs() < 1e-6, "in the gate's own column");
        assert_eq!(facing, (0.0, -1.0), "facing north, into the town");
        assert!(
            y > (side as f64 - 4.0) * t && y < (side as f64 - 2.0) * t,
            "just inside the south wall: {y}"
        );

        // Wave three is odd: the north gate, facing south.
        let ((x3, y3), facing3) = gate_spot(side, 3);
        assert!((x3 - gate_x).abs() < 1e-6);
        assert_eq!(facing3, (0.0, 1.0), "facing south, into the town");
        assert!(y3 > t && y3 < 3.0 * t, "just inside the north wall: {y3}");

        // And they alternate: every odd wave north, every even one south.
        for wave in 1..8u32 {
            let (_, face) = gate_spot(side, wave);
            let north = wave % 2 == 1;
            assert_eq!(face.1 > 0.0, north, "wave {wave}");
        }
        // The two are never the same spot.
        assert_ne!(gate_spot(side, 1).0, gate_spot(side, 2).0);
    }
}

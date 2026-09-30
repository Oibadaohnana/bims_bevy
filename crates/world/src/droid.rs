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
//! - The **wave size** is worked out as each wave appears. Nothing in a
//!   mission moves it — the world clock stands still until the next trip
//!   — so every wave of one fight is the same size.
//! - When the last machine of a wave is destroyed and waves are left,
//!   the next arrives [`data::DROID_REINFORCE_STEPS`] of the **mission
//!   clock** later (feature 103). **Never while one is still standing.**
//! - Leaving a station before its last wave is destroyed puts it back as
//!   the crew met it (`crate::run`): the next visit is a fresh fight, its
//!   count worked out again at the day it is fought on.
//!
//! # How many
//!
//! **A wave's size is world time and the number of players, and nothing
//! else** (feature 105); **a site's count of waves is the tier its
//! machines come at**, one, two or four ([`data::DROID_TIER_WAVES`]).
//! Integers only, and nothing doubles: a wave grows by *addition*,
//! one machine a player and one every [`data::ENEMIES_HOURS`] of the
//! world clock. What the crew own, what they have learnt, and how many
//! bots, mercenaries and recruits walk with them are none of the
//! machines' business — growing stronger makes the fight easier, and
//! money kept is not punished. Only a jump moves the world clock — a day
//! each, [`data::JUMP_MINUTES`], and nothing for a trip within a system
//! (the map rework) — so the machines grow with the systems crossed. See
//! [`wave_size`] and [`wave_count`].

use crate::data;
use bims::combat::Tier;
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
    /// The Machine Heart's fight (feature 108, [`crate::heart::HeartFight`]):
    /// `None` at every station but its fortress, and there until the crew
    /// first dock. Put back with the rest of this when the crew leave
    /// before the core is down.
    #[cfg_attr(feature = "serde", serde(default))]
    pub heart: Option<crate::heart::HeartFight>,
    /// Whether a **relic cache** still lies on the station's research desk
    /// (feature 106, `crate::relic::cache_rolled`): rolled when the
    /// machines take the site, and gone when a crew member opens it. Put
    /// back with the rest of the site when the crew leave it uncleared.
    pub cache: bool,
    /// Whether the site is the **Manufacturers'** rather than the
    /// machines' (feature 109, [`crate::manufacturer`]): the same fight's
    /// bookkeeping — the waves, the clock, the clear, the cache — with
    /// their people on the deck where the machines would stand. A site of
    /// theirs is not infested: the crisis never takes it, it is never a
    /// jammer, and it frees no system when it is cleared. Hashed only where
    /// it is set, so nothing about a machines' site moved.
    #[cfg_attr(feature = "serde", serde(default))]
    pub manufacturers: bool,
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
            heart: None,
            cache: false,
            manufacturers: false,
        }
    }

    /// A site the Manufacturers hold (feature 109): nothing worked out
    /// yet, as for a station newly held by the machines.
    pub fn manufacturers(station: u32) -> Infestation {
        Infestation {
            manufacturers: true,
            ..Infestation::new(station)
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
/// `DROID_WAVE_BASE + players + time_steps`, with no cap (task 132: the
/// sixteen it stopped at went, to be balanced another way).
///
/// - `players` is how many **player Bims** there are (`World::players`)
///   — never the bots, the mercenaries, the recruits or anybody else who
///   walks with them;
/// - `time_steps` is [`time_steps`] — whole [`data::ENEMIES_HOURS`] the
///   world clock has run.
///
/// Integers throughout, and nothing here doubles.
pub fn wave_size(players: u32, time_steps: u32) -> u32 {
    data::DROID_WAVE_BASE
        .saturating_add(players)
        .saturating_add(time_steps)
}

/// How many waves a held station has all told, the one aboard counted:
/// [`data::DROID_TIER_WAVES`] at the `tier` its machines come at — one at
/// tier one, two at tier two, four at tier three. Not the clock and not
/// the players: a crew of four and a later day meet bigger waves, not
/// more of them.
///
/// Worked out once, at the crew's first dock, and never again.
pub fn wave_count(tier: Tier) -> u32 {
    by_tier(data::DROID_TIER_WAVES, tier)
}

/// The entry of a table of three — tier one, two, three — for `tier`.
fn by_tier(table: [u32; 3], tier: Tier) -> u32 {
    match tier {
        Tier::One => table[0],
        Tier::Two => table[1],
        Tier::Three => table[2],
    }
}

/// Whole [`data::ENEMIES_HOURS`] in `hours_gone` of the world clock
/// (`World::hours_gone`): the time step a wave and a station's count of
/// waves grow by.
pub fn time_steps(hours_gone: u32) -> u32 {
    hours_gone / data::ENEMIES_HOURS
}

/// The dials of the wave formula, for tuning while the game runs: the
/// app reads them from `scaling.ron` and hands them to
/// `World::set_wave_scaling` whenever the file changes. The default is
/// the constants in [`data`], so a world never told is the formula
/// above to the machine.
///
/// A wave is `base + per_player × players + per_step × steps`, `steps`
/// being whole `step_days` of the world clock, and a held station has
/// `tier_waves` waves by the tier its machines come at.
/// Every wave of the run's first mission is `first_mission_ease`
/// fewer. Integers only, like the formula.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct WaveScaling {
    /// Machines in every wave before anything is counted.
    pub base: u32,
    /// Machines added for each player Bim.
    pub per_player: u32,
    /// Machines added for each time step gone.
    pub per_step: u32,
    /// How many days of the world clock one time step is (one at the
    /// least). A jump is a day, so this is jumps too.
    pub step_days: u32,
    /// Waves a held station has, by the tier its machines come at — tier
    /// one, two, three (`(1, 2, 4)` in the file).
    pub tier_waves: [u32; 3],
    /// Machines fewer in every wave of the run's first mission.
    pub first_mission_ease: u32,
}

impl WaveScaling {
    /// The constants of [`data`]: the game as it plays untuned.
    pub const DEFAULT: WaveScaling = WaveScaling {
        base: data::DROID_WAVE_BASE,
        per_player: 1,
        per_step: 1,
        step_days: data::ENEMIES_HOURS / 24,
        tier_waves: data::DROID_TIER_WAVES,
        first_mission_ease: data::FIRST_MISSION_WAVE_EASE,
    };

    /// Whole time steps in `hours_gone` of the world clock.
    pub fn steps(&self, hours_gone: u32) -> u32 {
        hours_gone / self.step_days.max(1).saturating_mul(24)
    }

    /// How many machines a wave is for `players` at `hours_gone`.
    pub fn size(&self, players: u32, hours_gone: u32) -> u32 {
        self.base
            .saturating_add(self.per_player.saturating_mul(players))
            .saturating_add(self.per_step.saturating_mul(self.steps(hours_gone)))
    }

    /// How many waves a held station has whose machines come at `tier`.
    pub fn count(&self, tier: Tier) -> u32 {
        by_tier(self.tier_waves, tier)
    }
}

impl Default for WaveScaling {
    fn default() -> WaveScaling {
        WaveScaling::DEFAULT
    }
}

/// The run's difficulty, as the game setup picked it: three of the wave
/// formula's dials — the base machines a wave, how many each player adds
/// and how many each time step adds — laid over whatever
/// [`WaveScaling`] the tuning file says (`World::set_difficulty`). The
/// rest of the formula (the step's days, the waves a site, the first
/// mission's ease) stays the file's. Saved with the world, so a load or
/// a restart plays at the difficulty the run was begun at, and dealt to
/// every machine of a lobby with the rest of the settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Difficulty {
    /// Machines in every wave before anything is counted.
    pub base: u32,
    /// Machines added for each player Bim.
    pub per_player: u32,
    /// Machines added for each time step gone.
    pub per_step: u32,
}

impl Difficulty {
    /// The three dials as `scaling` has them.
    pub fn of(scaling: WaveScaling) -> Difficulty {
        Difficulty {
            base: scaling.base,
            per_player: scaling.per_player,
            per_step: scaling.per_step,
        }
    }

    /// `scaling` with these three in place of its own.
    pub fn over(self, scaling: WaveScaling) -> WaveScaling {
        WaveScaling {
            base: self.base,
            per_player: self.per_player,
            per_step: self.per_step,
            ..scaling
        }
    }
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

    /// The untuned dials are the formula to the machine, and a tuned one
    /// moves what it says.
    #[test]
    fn the_default_scaling_is_the_formula_and_a_dial_moves_it() {
        let d = WaveScaling::DEFAULT;
        for players in 1..=4 {
            for hours in (0..=data::ENEMIES_HOURS * 12).step_by(37) {
                assert_eq!(
                    d.size(players, hours),
                    wave_size(players, time_steps(hours))
                );
            }
        }
        for tier in Tier::ALL {
            assert_eq!(d.count(tier), wave_count(tier));
        }
        let tuned = WaveScaling {
            base: 5,
            per_player: 2,
            per_step: 3,
            step_days: 1,
            tier_waves: [3, 5, 7],
            first_mission_ease: 0,
        };
        // Two players, three days in.
        assert_eq!(tuned.size(2, 3 * 24 + 5), 5 + 2 * 2 + 3 * 3);
        assert_eq!(tuned.count(Tier::One), 3);
        assert_eq!(tuned.count(Tier::Two), 5);
        assert_eq!(tuned.count(Tier::Three), 7);
        // A step of nought days is a step of one, not a division by nought.
        let zero = WaveScaling {
            step_days: 0,
            ..tuned
        };
        assert_eq!(zero.steps(7 * 24 + 5), 7);
    }

    #[test]
    fn a_wave_is_the_sum_and_has_no_cap() {
        // Nothing but the base and the players to begin with.
        assert_eq!(wave_size(0, 0), data::DROID_WAVE_BASE);
        assert_eq!(wave_size(3, 0), data::DROID_WAVE_BASE + 3);
        // A player and two steps of the clock.
        assert_eq!(wave_size(1, 2), data::DROID_WAVE_BASE + 1 + 2);
        // And nothing stops it however long the run (task 132).
        assert_eq!(wave_size(4, 500), data::DROID_WAVE_BASE + 4 + 500);
    }

    /// One wave at tier one, two at tier two and four at tier three.
    #[test]
    fn the_wave_count_is_the_tier_s() {
        assert_eq!(wave_count(Tier::One), 1);
        assert_eq!(wave_count(Tier::Two), 2);
        assert_eq!(wave_count(Tier::Three), 4);
    }

    /// The size never falls as the clock runs on, for any number of
    /// players, and has risen by the end of a run's worth of the clock.
    #[test]
    fn the_size_never_falls_as_the_clock_runs_and_rises_over_a_run() {
        let run_hours = 24 * 120;
        for players in 1..=4 {
            let mut size = 0;
            for hours in (0..=run_hours).step_by(7) {
                let steps = time_steps(hours);
                assert!(wave_size(players, steps) >= size, "{players} at {hours}h");
                size = wave_size(players, steps);
            }
            assert!(size > wave_size(players, 0), "{players}: the waves grew");
        }
    }

    #[test]
    fn a_wave_grows_with_the_players() {
        for steps in [0, 3, 8] {
            for players in 1..4 {
                assert!(wave_size(players + 1, steps) > wave_size(players, steps));
            }
        }
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
    fn the_time_step_is_every_enemies_hours() {
        let h = data::ENEMIES_HOURS;
        assert_eq!(time_steps(0), 0);
        assert_eq!(time_steps(h - 1), 0);
        assert_eq!(time_steps(h), 1);
        assert_eq!(time_steps(h * 3 + 5), 3);
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

/// The airlock a reinforcement wave `wave` ties up at, at station `id`:
/// **every** airlock but the crew's in turn, so a fight is not one door
/// held — wave one at the farthest from the port ([`arrival_airlock`]),
/// the next at the next farthest, and round again (ties to the lower
/// index). A generated station's airlocks open into rooms as well as
/// corridors, so the waves come aboard in different parts of it. The
/// Machine Heart's fortress (feature 108) keeps its own turn, the east,
/// the north and the south lobbies by index. A station with no airlock
/// but the port has the machines come in through the crew's own door.
pub fn arrival_airlock_at(design: &ShipDesign, id: u32, wave: u32) -> Option<Port> {
    let ports = airlocks(design);
    let first = *ports.first()?;
    if crate::heart::is_heart(id) {
        let others = ports.len().checked_sub(1).filter(|&n| n > 0)?;
        return ports.get(1 + wave as usize % others).copied();
    }
    let far = |p: &Port| (p.centre.0 - first.centre.0).hypot(p.centre.1 - first.centre.1);
    let mut others: Vec<(usize, Port)> = ports.iter().copied().enumerate().skip(1).collect();
    if others.is_empty() {
        return Some(first);
    }
    others.sort_by(|(i, a), (j, b)| far(b).total_cmp(&far(a)).then(i.cmp(j)));
    let n = others.len();
    Some(others[(wave as usize + n - 1) % n].1)
}

/// The spot `tiles` inside an airlock, in the design's own world units:
/// where a reinforcement wave is posted, off the airlock its ship tied
/// up at.
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

#[cfg(test)]
mod gate_tests {
    use crate::surface::{Gate, Wall, gate_for_wave};
    use shipdesign::TILE;

    /// On a surface a wave walks in **through a gate** — the town's gates
    /// in turn, the first for wave one (feature 112; until then, north for
    /// an odd wave and south for an even one, which is what a town with a
    /// north gate and a south gate still gets) — and stands a tile inside
    /// the wall rather than on it, facing into the town.
    #[test]
    fn a_surface_wave_comes_through_the_town_s_gates_in_turn() {
        let side = crate::data::SURFACE_SIDE as f64;
        let t = TILE as f64;
        let north = Gate {
            wall: Wall::North,
            from: 30,
            width: 6,
        };
        let east = Gate {
            wall: Wall::East,
            from: 44,
            width: 8,
        };
        let south = Gate {
            wall: Wall::South,
            from: 52,
            width: 7,
        };

        // Two gates, the template's: odd waves north, even waves south.
        let pair = [north, south];
        for wave in 1..8u32 {
            let gate = gate_for_wave(&pair, wave).unwrap();
            assert_eq!(gate.wall == Wall::North, wave % 2 == 1, "wave {wave}");
        }
        let spot = north.spot();
        assert!((spot.x - 33.0 * t).abs() < 1e-6, "in the gate's own column");
        assert!(spot.y > t && spot.y < 3.0 * t, "just inside the north wall");
        assert_eq!(north.inward(), (0.0, 1.0));
        let spot = south.spot();
        assert!(spot.y > (side - 4.0) * t && spot.y < (side - 2.0) * t);
        assert_eq!(south.inward(), (0.0, -1.0));

        // Three: round them in order, and an east gate faces west.
        let three = [north, east, south];
        let walls: Vec<Wall> = (1..7u32)
            .map(|w| gate_for_wave(&three, w).unwrap().wall)
            .collect();
        assert_eq!(
            walls,
            [
                Wall::North,
                Wall::East,
                Wall::South,
                Wall::North,
                Wall::East,
                Wall::South
            ]
        );
        let spot = east.spot();
        assert!(spot.x > (side - 4.0) * t && spot.x < (side - 2.0) * t);
        assert!((spot.y - 48.0 * t).abs() < 1e-6, "in the gate's own row");
        assert_eq!(east.inward(), (-1.0, 0.0));
        // The lander beyond each, outside the ground.
        let (at, out) = east.beyond(5.0 * t);
        assert!(at.x > side * t && out.x > 0.0);
        let (at, out) = north.beyond(5.0 * t);
        assert!(at.y < 0.0 && out.y < 0.0);
        assert!(gate_for_wave(&[], 1).is_none());
    }
}

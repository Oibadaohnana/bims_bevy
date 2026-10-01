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
//! **The machines scale on the run day and the players, and nothing else**
//! (task 147, [`WaveScaling`]): a wave is the players' share — which
//! grows by `day_scaling` every `scaling_days` — and at a defence the
//! bots' (the crew's bots and a defence's defenders); a site has a wave more every `wave_days`; and each enemy's
//! tier is dealt by the day's tier-two and tier-three shares. What the
//! crew own, what they have learnt, and how many bots, mercenaries and
//! recruits walk with them are none of the machines' business. Only a
//! jump moves the world clock — a day each, [`data::JUMP_MINUTES`] — so
//! the machines grow with the systems crossed.

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

/// The dials of the wave formula (task 147), and the whole of how the
/// machines scale: **how many** a wave is, **how many waves** a site has
/// and **what tier** each enemy comes at. Nothing else moves any of it —
/// no base, no ease, no floor at the crew's numbers, no distance. The app
/// reads them from `scaling.ron` and hands them to
/// `World::set_wave_scaling` whenever the file changes, and the game
/// setup's Difficulty lays its own over them for a run
/// (`World::set_difficulty`). The default is the constants in [`data`].
///
/// Every rule reads the **run day** — the day the top bar shows, one on
/// the day the world opens (`World::run_day`):
///
/// - a wave is `(per_player + day_scaling × steps) × players +
///   ⌈per_bot × bots⌉` (the crew's bots and a
///   defence's defenders), `steps` being whole `scaling_days` in
///   the run day ([`WaveScaling::size`]);
/// - a site has `1 + whole wave_days` waves ([`WaveScaling::waves`]);
/// - the share of enemies at tier two is `day / tier2_days`, all of them
///   from that day on, and the same for tier three; the share of the
///   Manufacturers who carry any gear (tier one and up) is
///   `day / tier1_days` ([`WaveScaling::machine_tiers`],
///   [`WaveScaling::gear_tiers`]).
///
/// A step or wave length of nought days never grows; a tier timing of
/// nought is that tier for everybody from the first day.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct WaveScaling {
    /// Machines added for each player Bim (bots do not count).
    pub enemies_per_player: u32,
    /// How much `enemies_per_player` grows every `scaling_days` (the "y").
    pub day_scaling: u32,
    /// How many days one step of `day_scaling` is (the "x").
    pub scaling_days: u32,
    /// Machines added for each bot: every crew member alive who is not a
    /// player (bots, hands and joiners — not a commander's reinforcements)
    /// and at a defence every defender the site fields; the product
    /// rounded up, `1.5` and three bots is five. (`enemies_per_defender`
    /// in a file or a save written before.)
    #[cfg_attr(feature = "serde", serde(alias = "enemies_per_defender"))]
    pub enemies_per_bot: f32,
    /// Every this many days a site has one wave more (the "z").
    pub wave_days: u32,
    /// The day every Manufacturer carries tier-one gear (a gun and
    /// armour); before it the share that do is `day / tier1_days`, and
    /// the rest carry the laser pistol alone.
    pub tier1_days: u32,
    /// The day every enemy is tier two at the least — machines and the
    /// Manufacturers' gear alike; before it the share is
    /// `day / tier2_days`.
    pub tier2_days: u32,
    /// The same for tier three.
    pub tier3_days: u32,
}

impl WaveScaling {
    /// The constants of [`data`]: the game as it plays untuned.
    pub const DEFAULT: WaveScaling = WaveScaling {
        enemies_per_player: data::ENEMIES_PER_PLAYER,
        day_scaling: data::DAY_SCALING,
        scaling_days: data::SCALING_DAYS,
        enemies_per_bot: data::ENEMIES_PER_BOT,
        wave_days: data::WAVE_DAYS,
        tier1_days: data::TIER1_DAYS,
        tier2_days: data::TIER2_DAYS,
        tier3_days: data::TIER3_DAYS,
    };

    /// Whole `scaling_days` in run day `day`; nought with no step.
    pub fn steps(&self, day: u32) -> u32 {
        day.checked_div(self.scaling_days).unwrap_or(0)
    }

    /// Machines a player brings on run day `day`:
    /// `enemies_per_player + day_scaling × steps`.
    pub fn per_player_on(&self, day: u32) -> u32 {
        self.enemies_per_player
            .saturating_add(self.day_scaling.saturating_mul(self.steps(day)))
    }

    /// Machines `bots` bring, `enemies_per_bot` each, the
    /// product rounded up — worked in hundredths, so two machines agree
    /// to the machine.
    pub fn for_bots(&self, bots: u32) -> u32 {
        let hundredths = (f64::from(self.enemies_per_bot.max(0.0)) * 100.0).round() as u64;
        let whole = (hundredths * u64::from(bots)).div_ceil(100);
        whole.min(u64::from(u32::MAX)) as u32
    }

    /// How many machines a wave is for `players` and `bots` on run
    /// day `day`. The world makes it one at the least.
    pub fn size(&self, players: u32, bots: u32, day: u32) -> u32 {
        self.per_player_on(day)
            .saturating_mul(players)
            .saturating_add(self.for_bots(bots))
    }

    /// How many waves a site has all told on run day `day`, the first
    /// counted: one, and one more every `wave_days`.
    pub fn waves(&self, day: u32) -> u32 {
        1u32.saturating_add(day.checked_div(self.wave_days).unwrap_or(0))
    }

    /// How many machines a wave of `n` have tier two at the least and
    /// tier three, on run day `day`: each share of `n` in whole machines,
    /// the tier-three ones counted among the tier-two ones.
    fn tier_counts(&self, n: u32, day: u32) -> (u32, u32) {
        let three = share_of(n, day, self.tier3_days);
        let two = share_of(n, day, self.tier2_days).max(three);
        (two, three)
    }

    /// The tier of each machine of a wave of `n` on run day `day`, in the
    /// wave's order: tier three for the first of the day's tier-three
    /// share, tier two for the next of the tier-two share, and tier one
    /// for the rest.
    pub fn machine_tiers(&self, n: u32, day: u32) -> Vec<Tier> {
        let (two, three) = self.tier_counts(n, day);
        (0..n)
            .map(|i| {
                if i < three {
                    Tier::Three
                } else if i < two {
                    Tier::Two
                } else {
                    Tier::One
                }
            })
            .collect()
    }

    /// The tier of the gear each of `n` Manufacturers carries on run day
    /// `day`, in their order: as [`WaveScaling::machine_tiers`] for tiers
    /// two and three, then tier one for the next of the `tier1_days`
    /// share, and `None` — the laser pistol and no armour — for the rest.
    pub fn gear_tiers(&self, n: u32, day: u32) -> Vec<Option<Tier>> {
        let (two, three) = self.tier_counts(n, day);
        let one = share_of(n, day, self.tier1_days).max(two);
        (0..n)
            .map(|i| {
                if i < three {
                    Some(Tier::Three)
                } else if i < two {
                    Some(Tier::Two)
                } else if i < one {
                    Some(Tier::One)
                } else {
                    None
                }
            })
            .collect()
    }

    /// The tier at least half the machines come at on run day `day`: what
    /// a site's tier is said as (the map, the chart, the checksum).
    pub fn usual_tier(&self, day: u32) -> Tier {
        let half = |days: u32| u64::from(day) * 2 >= u64::from(days);
        if half(self.tier3_days) {
            Tier::Three
        } else if half(self.tier2_days) {
            Tier::Two
        } else {
            Tier::One
        }
    }
}

impl Default for WaveScaling {
    fn default() -> WaveScaling {
        WaveScaling::DEFAULT
    }
}

/// `day / days` of `n` in whole machines — rounded down, so a tier reaches
/// one machine once its share is a whole one — never more than `n`; all
/// of `n` for `days` nought.
fn share_of(n: u32, day: u32, days: u32) -> u32 {
    if days == 0 || day >= days {
        return n;
    }
    (u64::from(n) * u64::from(day) / u64::from(days)) as u32
}

/// The run's difficulty, as the game setup picked it: every dial of the
/// formula ([`WaveScaling`]), laid over the tuning file's for the run
/// (`World::set_difficulty`). Saved with the world, so a load or a
/// restart plays at the difficulty the run was begun at, and dealt to
/// every machine of a lobby with the rest of the settings.
pub type Difficulty = WaveScaling;

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

    fn dials() -> WaveScaling {
        WaveScaling {
            enemies_per_player: 2,
            day_scaling: 1,
            scaling_days: 5,
            enemies_per_bot: 1.0,
            wave_days: 10,
            tier1_days: 5,
            tier2_days: 20,
            tier3_days: 40,
        }
    }

    /// The player's own example (task 147): day two, a defence with
    /// three bots, one player at two a player, a step of five days
    /// not yet come — five machines.
    #[test]
    fn a_wave_is_per_player_with_the_day_s_growth_and_per_bot() {
        let d = dials();
        assert_eq!(d.size(1, 3, 2), 3 + 2);
        // The step comes on day five, and raises every player's share.
        assert_eq!(d.size(1, 0, 4), 2);
        assert_eq!(d.size(1, 0, 5), 3);
        assert_eq!(d.size(3, 0, 5), 9);
        assert_eq!(d.size(2, 0, 12), 2 * (2 + 2));
        // A decimal per bot is rounded up on the whole.
        let half = WaveScaling {
            enemies_per_bot: 1.5,
            ..d
        };
        assert_eq!(half.for_bots(3), 5);
        assert_eq!(half.for_bots(2), 3);
        assert_eq!(half.for_bots(0), 0);
        let third = WaveScaling {
            enemies_per_bot: 0.34,
            ..d
        };
        assert_eq!(third.for_bots(3), 2, "1.02 is two");
        // No step length is no growth, not a division by nought.
        let flat = WaveScaling {
            scaling_days: 0,
            ..d
        };
        assert_eq!(flat.size(1, 0, 90), 2);
    }

    #[test]
    fn a_site_has_a_wave_more_every_wave_days() {
        let d = dials();
        assert_eq!(d.waves(1), 1);
        assert_eq!(d.waves(9), 1);
        assert_eq!(d.waves(10), 2);
        assert_eq!(d.waves(25), 3);
        let one = WaveScaling { wave_days: 0, ..d };
        assert_eq!(one.waves(200), 1);
    }

    /// Twenty days to tier two: half the machines on day ten, all of them
    /// on day twenty; tier three the same over forty, counted first.
    #[test]
    fn the_tier_shares_grow_with_the_day() {
        let d = dials();
        let count = |n, day, tier| {
            d.machine_tiers(n, day)
                .iter()
                .filter(|&&t| t == tier)
                .count() as u32
        };
        assert_eq!(count(10, 1, Tier::One), 10, "half a machine is none yet");
        assert_eq!(count(10, 10, Tier::Two) + count(10, 10, Tier::Three), 5);
        assert_eq!(count(10, 20, Tier::Two) + count(10, 20, Tier::Three), 10);
        assert_eq!(count(10, 20, Tier::Three), 5);
        assert_eq!(count(10, 40, Tier::Three), 10);
        assert_eq!(count(10, 90, Tier::Three), 10);
        // One machine at a time: a wave of five on day four has one.
        assert_eq!(count(5, 4, Tier::Two), 1);
        assert_eq!(
            d.machine_tiers(3, 30),
            [Tier::Three, Tier::Three, Tier::Two]
        );
        // A timing of nought is the tier from the first day.
        let at_once = WaveScaling { tier3_days: 0, ..d };
        assert!(
            at_once
                .machine_tiers(4, 1)
                .iter()
                .all(|&t| t == Tier::Three)
        );
        assert_eq!(d.usual_tier(9), Tier::One);
        assert_eq!(d.usual_tier(10), Tier::Two);
        assert_eq!(d.usual_tier(20), Tier::Three);
    }

    /// The Manufacturers' gear: the pistol alone for the share not yet
    /// geared, tier one up to `tier1_days`, then two and three as the
    /// machines'.
    #[test]
    fn the_manufacturers_gear_up_by_the_day() {
        let d = dials();
        assert!(d.gear_tiers(4, 1).iter().all(|t| t.is_none()), "the pistol");
        let tiers = d.gear_tiers(4, 2);
        assert_eq!(tiers.iter().filter(|t| t.is_some()).count(), 1, "{tiers:?}");
        assert!(d.gear_tiers(4, 5).iter().all(|t| t.is_some()));
        assert_eq!(
            d.gear_tiers(4, 10),
            [
                Some(Tier::Three),
                Some(Tier::Two),
                Some(Tier::One),
                Some(Tier::One)
            ]
        );
        assert!(d.gear_tiers(4, 40).iter().all(|&t| t == Some(Tier::Three)));
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

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
//! **The enemies scale on the run day and the players, and nothing else**
//! (task 147, [`WaveScaling`]): the run is four areas of so many days
//! each — area 0, then tiers one, two and three — a wave is the players'
//! share, which grows by each area's `growth_per_day`, and the bots' (the
//! crew's bots and a defence's defenders); a site has its area's waves;
//! and who comes — the Manufacturers or the machines — and each enemy's
//! tier are the day's shares along each area. What the
//! crew own, what they have learnt, and how many bots and
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
    /// Whether the site is the **Manufacturers'** rather than the
    /// machines' (feature 109, [`crate::manufacturer`]): the same fight's
    /// bookkeeping — the waves, the clock, the clear — with
    /// their people on the deck where the machines would stand. A site of
    /// theirs is not infested: the crisis never takes it, it is never a
    /// jammer, and it frees no system when it is cleared. Hashed only where
    /// it is set, so nothing about a machines' site moved.
    #[cfg_attr(feature = "serde", serde(default))]
    pub manufacturers: bool,
    /// **Sabotage** (October 2026, `crate::world` `sabotage.rs`): the
    /// charge, the way out and how far the crew have got. `None` at every
    /// other attack. Put back with the rest of this when the crew leave.
    #[cfg_attr(feature = "serde", serde(default))]
    pub sabotage: Option<Sabotage>,
    /// **A nest hunt** (October 2026, `crate::world` `nests.rs`): the
    /// nests grown into the site's walls. `None` at every other attack.
    #[cfg_attr(feature = "serde", serde(default))]
    pub nests: Option<Nests>,
}

/// A nest hunt's nests: where each is grown (a tile of the site's design,
/// and the way its bay faces, a unit step), which are destroyed, when the
/// standing ones next build and how many machines they have built.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Nests {
    pub spots: Vec<(u32, u32)>,
    pub facing: Vec<(i32, i32)>,
    pub down: Vec<bool>,
    pub next_build: u64,
    pub built: u32,
}

/// Where a Sabotage stands: the charge to plant, held, then the run for
/// the way out.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SabotagePhase {
    /// The charge not planted yet: the crew fight their way to it.
    #[default]
    Plant,
    /// Planted: held against the machines making to disarm it.
    Hold,
    /// It can no longer be disarmed: the crew run for the way out before
    /// it blows.
    Escape,
    /// It blew, the crew out.
    Done,
    /// The machines disarmed it: the run is lost.
    Disarmed,
}

impl SabotagePhase {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// A Sabotage's state (October 2026): the charge's tile of the site's
/// design, the airlock the crew get out by (its index among the design's
/// airlocks), the phase, and its counts in steps — the planting's work,
/// the machines' disarming, what is left of the hold or the escape, and
/// when the next wave lands while the charge is armed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sabotage {
    pub charge: (u32, u32),
    pub extraction: u32,
    pub phase: SabotagePhase,
    pub planted: u32,
    pub disarm: u32,
    pub left: u64,
    pub next_wave: u64,
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
            manufacturers: false,
            sabotage: None,
            nests: None,
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

/// The dials of the wave formula, and the whole of how the enemies scale
/// (task 147; laid out by **areas** since October 2026, the player's: "I
/// only want to define tier_n_area -> that sets how long (in days) the
/// area is"). Nothing else moves any of it — no base, no ease, no floor
/// at the crew's numbers, no distance. The app reads them from
/// `scaling.ron` and hands them to `World::set_wave_scaling` whenever the
/// file changes, and the game setup's Difficulty lays its own over them
/// for a run (`World::set_difficulty`). The default is the constants in
/// [`data`].
///
/// **The run is four areas, one after the other**, each so many days (a
/// day is a row of the floor): [`WaveScaling::area_0`], an easing the map
/// does not show (its rows are drawn tier one), then the tier-one, the
/// tier-two and the tier-three areas. Tier two's door — the floor's
/// guaranteed trader — is the tier-two area's first day and not one of
/// its `days`; after the tier-three area come the trader under the Heart
/// and the Heart, neither counted either ([`WaveScaling::heart_day`]).
///
/// Every rule reads the **run day** — the day the top bar shows, one on
/// the day the world opens (`World::run_day`):
///
/// - **a wave** is `enemies_per_player` and every day gone since the run
///   began times the `growth_per_day` of the area it fell in — carried
///   over from area to area, so the count never jumps — times the
///   players, rounded down, and `⌈enemies_per_bot × bots⌉` on top
///   ([`WaveScaling::size`]);
/// - **a site's waves**, the extras on top of a wave, an elite's Guardians
///   and Bombers and a defence's defenders are the day's area's own
///   numbers ([`Area`]);
/// - **who comes**, each a straight line in whole enemies
///   ([`WaveScaling::machines_in`], [`WaveScaling::gear_tiers`],
///   [`WaveScaling::machine_tiers`]): area 0 is the Manufacturers alone,
///   the share of them with a gun and armour (tier one) rising from none
///   on day one to all on the tier-one area's first day; the machines'
///   share of a wave rises from none on that day to all on the tier-two
///   area's first; the share at tier two from halfway through the
///   tier-one area to all on the tier-two area's first day, and tier
///   three's the same from halfway through the tier-two area to all on
///   the tier-three area's first.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct WaveScaling {
    /// Enemies each player Bim brings on day one (bots do not count).
    pub enemies_per_player: u32,
    /// Enemies added for each bot: every crew member alive who is not a
    /// player (bots, hands and joiners — not a commander's reinforcements)
    /// and at a defence every defender the site fields — none at all at
    /// an Area defend; the product
    /// rounded up, `1.5` and three bots is five. (`enemies_per_defender`
    /// in a file or a save written before.)
    #[cfg_attr(feature = "serde", serde(alias = "enemies_per_defender"))]
    pub enemies_per_bot: f32,
    /// The easing before the tier-one area: the Manufacturers alone, no
    /// elite, drawn tier one on the map.
    pub area_0: Area,
    pub tier_1_area: Area,
    /// Its `days` not counting tier two's door, its first day.
    pub tier_2_area: Area,
    /// Its `days` not counting the trader under the Heart or the Heart.
    pub tier_3_area: Area,
}

/// One area's numbers ([`WaveScaling`]).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct Area {
    /// How many days (rows of the floor) the area is.
    pub days: u32,
    /// How much each player's share of a wave grows every day of the
    /// area, carried into the next.
    pub growth_per_day: f32,
    /// How many waves a site has all told (one at the least).
    pub waves: u32,
    /// Bombers on top of every wave — machines, so never in area 0.
    pub bombers: u32,
    /// And Lancers.
    pub lancers: u32,
    /// An elite fight's Guardians for each player Bim, in its Guardian
    /// wave. Area 0 has no elite.
    pub guardians: u32,
    /// An elite fight's Bombers on top of its Guardian wave, besides the
    /// Guardians.
    pub elites: u32,
    /// The armed defenders who stand with a site the crew defend.
    pub defenders: u32,
}

impl Area {
    /// An area of `days` with nothing in it.
    pub const NONE: Area = Area {
        days: 0,
        growth_per_day: 0.0,
        waves: 1,
        bombers: 0,
        lancers: 0,
        guardians: 0,
        elites: 0,
        defenders: 0,
    };

    /// The growth in hundredths of an enemy a day, so two machines agree
    /// to the enemy.
    pub(crate) fn growth_hundredths(&self) -> u64 {
        (f64::from(self.growth_per_day.max(0.0)) * 100.0).round() as u64
    }

    /// The growth set to `h` hundredths of an enemy a day — read back by
    /// [`Area::growth_hundredths`] as exactly `h` (an ascension's,
    /// `crate::ascension::scale`).
    pub(crate) fn set_growth_hundredths(&mut self, h: u64) {
        self.growth_per_day = h as f32 / 100.0;
    }
}

impl Default for Area {
    fn default() -> Area {
        Area::NONE
    }
}

impl WaveScaling {
    /// The constants of [`data`]: the game as it plays untuned.
    pub const DEFAULT: WaveScaling = WaveScaling {
        enemies_per_player: data::ENEMIES_PER_PLAYER,
        enemies_per_bot: data::ENEMIES_PER_BOT,
        area_0: data::AREA_0,
        tier_1_area: data::TIER_1_AREA,
        tier_2_area: data::TIER_2_AREA,
        tier_3_area: data::TIER_3_AREA,
    };

    /// The default's numbers with tier two's door on run day `two` and
    /// tier three from day `three` (the tests'): no area 0, a tier-one
    /// area up to the door and a tier-two area up to tier three — so
    /// `two` one at the least and `three` past it.
    pub fn with_tier_days(two: u32, three: u32) -> WaveScaling {
        let mut s = WaveScaling::DEFAULT;
        s.area_0.days = 0;
        s.tier_1_area.days = two.saturating_sub(1);
        s.tier_2_area.days = three.saturating_sub(two.max(1) + 1);
        s
    }

    /// The four areas, in the order they come.
    pub fn areas(&self) -> [&Area; 4] {
        [
            &self.area_0,
            &self.tier_1_area,
            &self.tier_2_area,
            &self.tier_3_area,
        ]
    }

    /// The first day of the tier-one area.
    pub fn tier_one_day(&self) -> u32 {
        self.area_0.days.saturating_add(1)
    }

    /// The first day of the tier-two area: tier two's door, the floor's
    /// guaranteed trader.
    pub fn tier_two_day(&self) -> u32 {
        self.tier_one_day().saturating_add(self.tier_1_area.days)
    }

    /// The first day of the tier-three area: past the door and the
    /// tier-two area's `days`.
    pub fn tier_three_day(&self) -> u32 {
        self.tier_two_day()
            .saturating_add(1)
            .saturating_add(self.tier_2_area.days)
    }

    /// The Heart's day — the floor's last row: past the tier-three area's
    /// `days` and the trader under the Heart.
    pub fn heart_day(&self) -> u32 {
        self.tier_three_day()
            .saturating_add(self.tier_3_area.days)
            .saturating_add(1)
    }

    /// Which area run day `day` is in, nought to three: the door is the
    /// tier-two area's and the trader under the Heart, the Heart and
    /// any day past it the tier-three area's.
    pub fn area_index(&self, day: u32) -> usize {
        if day < self.tier_one_day() {
            0
        } else if day < self.tier_two_day() {
            1
        } else if day < self.tier_three_day() {
            2
        } else {
            3
        }
    }

    /// The area run day `day` is in.
    pub fn area_on(&self, day: u32) -> &Area {
        self.areas()[self.area_index(day)]
    }

    /// The zone's tier on run day `day`: tier three from the tier-three
    /// area's first day, two from the tier-two area's (its door), one
    /// before — area 0 included. What the floor marks its rows.
    pub fn zone_on(&self, day: u32) -> Tier {
        if day >= self.tier_three_day() {
            Tier::Three
        } else if day >= self.tier_two_day() {
            Tier::Two
        } else {
            Tier::One
        }
    }

    /// Enemies a player brings on run day `day`, in hundredths:
    /// `enemies_per_player`, and each area's `growth_per_day` for every
    /// day of it gone by `day` — area 0's from day one, so on day one
    /// none.
    fn per_player_hundredths(&self, day: u32) -> u64 {
        let starts = [
            1,
            self.tier_one_day(),
            self.tier_two_day(),
            self.tier_three_day(),
        ];
        let mut sum = u64::from(self.enemies_per_player) * 100;
        for (k, area) in self.areas().iter().enumerate() {
            let start = starts[k];
            // The last area runs on past the Heart.
            let end = starts.get(k + 1).copied().unwrap_or(u32::MAX);
            let gone = day.min(end).saturating_sub(start);
            sum = sum.saturating_add(area.growth_hundredths().saturating_mul(u64::from(gone)));
        }
        sum
    }

    /// [`WaveScaling::per_player_hundredths`] for an ascension's
    /// arithmetic (`crate::ascension::scale`).
    pub(crate) fn per_player_hundredths_on(&self, day: u32) -> u64 {
        self.per_player_hundredths(day)
    }

    /// Enemies a player brings on run day `day`, rounded down.
    pub fn per_player_on(&self, day: u32) -> u32 {
        (self.per_player_hundredths(day) / 100).min(u64::from(u32::MAX)) as u32
    }

    /// Enemies `bots` bring, `enemies_per_bot` each, the
    /// product rounded up — worked in hundredths, so two machines agree
    /// to the machine.
    pub fn for_bots(&self, bots: u32) -> u32 {
        let hundredths = (f64::from(self.enemies_per_bot.max(0.0)) * 100.0).round() as u64;
        let whole = (hundredths * u64::from(bots)).div_ceil(100);
        whole.min(u64::from(u32::MAX)) as u32
    }

    /// How many enemies a wave is for `players` and `bots` on run day
    /// `day`: the players' share rounded down on the whole, the bots'
    /// on top. The world makes it one at the least.
    pub fn size(&self, players: u32, bots: u32, day: u32) -> u32 {
        let players = (self
            .per_player_hundredths(day)
            .saturating_mul(u64::from(players))
            / 100)
            .min(u64::from(u32::MAX)) as u32;
        players.saturating_add(self.for_bots(bots))
    }

    /// How many waves a site has all told on run day `day`, the first
    /// counted: the area's, one at the least.
    pub fn waves(&self, day: u32) -> u32 {
        self.area_on(day).waves.max(1)
    }

    /// The Bombers and the Lancers on top of a wave on run day `day`: the
    /// area's — none in area 0, which has no machines.
    pub fn extras(&self, day: u32) -> (u32, u32) {
        match self.area_index(day) {
            0 => (0, 0),
            _ => {
                let area = self.area_on(day);
                (area.bombers, area.lancers)
            }
        }
    }

    /// How many Guardians an elite fight's Guardian wave holds on run day
    /// `day` for `players` player Bims: the area's for each.
    pub fn elite_guardians(&self, day: u32, players: u32) -> u32 {
        self.area_on(day).guardians.saturating_mul(players)
    }

    /// How many Bombers come on top of an elite fight's Guardian wave on
    /// run day `day`: the area's `elites`.
    pub fn elite_bombers(&self, day: u32) -> u32 {
        self.area_on(day).elites
    }

    /// The armed defenders a defended site fields on run day `day`.
    pub fn defenders(&self, day: u32) -> u32 {
        self.area_on(day).defenders
    }

    /// How many of a wave of `n` are machines on run day `day`: none
    /// through area 0, then a straight line from none on the tier-one
    /// area's first day to all on the tier-two area's; the rest are the
    /// Manufacturers' people.
    pub fn machines_in(&self, n: u32, day: u32) -> u32 {
        ramp(n, day, 2 * self.tier_one_day(), 2 * self.tier_two_day())
    }

    /// How many of `n` enemies are tier two at the least and tier three on
    /// run day `day`: tier two's share from halfway through the tier-one
    /// area to all on the tier-two area's first day, tier three's from
    /// halfway through the tier-two area to all on its first day, the
    /// tier-three ones counted among the tier-two ones.
    fn tier_counts(&self, n: u32, day: u32) -> (u32, u32) {
        let (one, two, three) = (
            self.tier_one_day(),
            self.tier_two_day(),
            self.tier_three_day(),
        );
        let three = ramp(n, day, two + three, 2 * three);
        let two = ramp(n, day, one + two, 2 * two).max(three);
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
    /// two and three, then tier one for the next of the share geared —
    /// none on day one, all on the tier-one area's first day — and `None`
    /// (the laser pistol and no armour) for the rest.
    pub fn gear_tiers(&self, n: u32, day: u32) -> Vec<Option<Tier>> {
        let (two, three) = self.tier_counts(n, day);
        let one = ramp(n, day, 2, 2 * self.tier_one_day()).max(two);
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
}

impl Default for WaveScaling {
    fn default() -> WaveScaling {
        WaveScaling::DEFAULT
    }
}

/// A straight line's share of `n` in whole enemies, rounded down, on run
/// day `day`: none up to `from2`, all from `to2` — both in **half days**,
/// twice the day, so a ramp may start halfway through an area — and
/// never more than `n`. All of `n` where the line has no length.
fn ramp(n: u32, day: u32, from2: u32, to2: u32) -> u32 {
    let at = u64::from(day) * 2;
    let (from, to) = (u64::from(from2), u64::from(to2));
    if at >= to {
        return n;
    }
    if at <= from {
        return 0;
    }
    (u64::from(n) * (at - from) / (to - from)) as u32
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

    /// The player's layout (October 2026): area 0 five days, tier one
    /// fourteen, tier two nine, tier three three; three a player growing
    /// half an enemy a day in every area.
    fn dials() -> WaveScaling {
        let area = |days, waves, defenders| Area {
            days,
            growth_per_day: 0.5,
            waves,
            bombers: 1,
            lancers: 1,
            guardians: 1,
            elites: 1,
            defenders,
        };
        WaveScaling {
            enemies_per_player: 3,
            enemies_per_bot: 1.0,
            area_0: area(5, 1, 2),
            tier_1_area: area(14, 1, 3),
            tier_2_area: area(9, 2, 4),
            tier_3_area: area(3, 3, 5),
        }
    }

    /// Rows 1-5 area 0, 6-19 tier one, 20 the door, 21-29 tier two, 30-32
    /// tier three, 33 the trader under the Heart, 34 the Heart.
    #[test]
    fn the_areas_lie_one_after_the_other() {
        let d = dials();
        assert_eq!(d.tier_one_day(), 6);
        assert_eq!(d.tier_two_day(), 20);
        assert_eq!(d.tier_three_day(), 30);
        assert_eq!(d.heart_day(), 34);
        for (day, area, zone) in [
            (1, 0, Tier::One),
            (5, 0, Tier::One),
            (6, 1, Tier::One),
            (19, 1, Tier::One),
            (20, 2, Tier::Two),
            (29, 2, Tier::Two),
            (30, 3, Tier::Three),
            (34, 3, Tier::Three),
            (90, 3, Tier::Three),
        ] {
            assert_eq!(d.area_index(day), area, "day {day}");
            assert_eq!(d.zone_on(day), zone, "day {day}");
        }
        assert_eq!(d.waves(1), 1);
        assert_eq!(d.waves(20), 2);
        assert_eq!(d.waves(31), 3);
        assert_eq!(d.defenders(19), 3);
        assert_eq!(d.extras(3), (0, 0), "no machines in area 0");
        assert_eq!(d.extras(6), (1, 1));
        assert_eq!(d.elite_guardians(25, 2), 2);
        // An area of nought waves is one.
        let mut none = d;
        none.area_0.waves = 0;
        assert_eq!(none.waves(1), 1);
    }

    /// The count carries over from area to area: day one is the base, and
    /// every day after adds its own area's growth.
    #[test]
    fn a_wave_grows_by_each_area_s_day_and_carries_over() {
        let mut d = dials();
        assert_eq!(d.size(1, 0, 1), 3);
        assert_eq!(d.size(1, 0, 2), 3, "3.5 is three");
        assert_eq!(d.size(2, 0, 2), 7, "7.0 for two players");
        assert_eq!(d.size(1, 0, 6), 5, "area 0's five days");
        d.tier_1_area.growth_per_day = 1.0;
        // Tier one's first day carries area 0's end; its own growth after.
        assert_eq!(d.size(1, 0, 6), 5);
        assert_eq!(d.size(1, 0, 8), 7);
        assert_eq!(d.size(1, 0, 20), 5 + 14, "the door: tier one's 14 days");
        assert_eq!(d.size(1, 0, 22), 20, "and tier two's half a day each");
        assert_eq!(d.size(1, 3, 1), 3 + 3);
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
    }

    /// Area 0 is the Manufacturers alone; the machines' share rises
    /// through tier one to all of the wave at tier two's door.
    #[test]
    fn the_machines_come_in_through_the_tier_one_area() {
        let d = dials();
        for day in 1..=6 {
            assert_eq!(d.machines_in(10, day), 0, "day {day}");
        }
        assert_eq!(d.machines_in(14, 13), 7, "half way");
        assert_eq!(d.machines_in(10, 19), 9);
        assert_eq!(d.machines_in(10, 20), 10);
        assert_eq!(d.machines_in(10, 40), 10);
    }

    /// Tier two from halfway through the tier-one area (day 13) to all at
    /// the door (day 20); tier three from halfway through the tier-two
    /// area (day 25) to all on day 30.
    #[test]
    fn the_tier_shares_grow_along_the_areas() {
        let d = dials();
        let count = |n, day, tier| {
            d.machine_tiers(n, day)
                .iter()
                .filter(|&&t| t >= tier)
                .count() as u32
        };
        assert_eq!(count(10, 13, Tier::Two), 0);
        assert_eq!(count(14, 16, Tier::Two), 6, "6 of 14 half days");
        assert_eq!(count(10, 19, Tier::Two), 8);
        assert_eq!(count(10, 20, Tier::Two), 10, "no tier one at the door");
        assert_eq!(count(10, 25, Tier::Three), 0);
        assert_eq!(count(10, 27, Tier::Three), 4);
        assert_eq!(count(10, 30, Tier::Three), 10);
        assert_eq!(d.machine_tiers(3, 28), [Tier::Three, Tier::Two, Tier::Two]);
        // A tier-one area of nought days is tier two from day one of it.
        let mut quick = d;
        quick.tier_1_area.days = 0;
        assert!(quick.machine_tiers(4, 6).iter().all(|&t| t == Tier::Two));
    }

    /// The Manufacturers' gear: the pistol for all on day one, a gun and
    /// armour for a growing share through area 0, all of them from the
    /// tier-one area on, then two and three as the machines'.
    #[test]
    fn the_manufacturers_gear_up_through_area_0() {
        let d = dials();
        assert!(d.gear_tiers(4, 1).iter().all(|t| t.is_none()), "the pistol");
        let geared = |day| d.gear_tiers(10, day).iter().filter(|t| t.is_some()).count();
        assert_eq!(geared(3), 4);
        assert_eq!(geared(5), 8);
        assert_eq!(geared(6), 10);
        assert_eq!(
            d.gear_tiers(4, 28),
            [
                Some(Tier::Three),
                Some(Tier::Three),
                Some(Tier::Two),
                Some(Tier::Two)
            ]
        );
        assert!(d.gear_tiers(4, 40).iter().all(|&t| t == Some(Tier::Three)));
        // No area 0: geared from day one.
        let mut none = d;
        none.area_0.days = 0;
        assert!(none.gear_tiers(4, 1).iter().all(|t| t.is_some()));
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

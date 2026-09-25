//! Relics (feature 106): passive items a player's Bim wins and keeps for
//! the rest of the run, in the manner of Slay the Spire, and the unlocks
//! that put more of them in the pool between runs.
//!
//! # What a relic is
//!
//! A [`Relic`] is held by **one player's Bim** — never a bot's — and is
//! never moved, dropped or sold: it is on the Bim that was given it until
//! the run ends, through a death and a buyback like a level. A relic is
//! in a run once at most: the run's pool ([`Relics::pool`]) loses one the
//! moment it is **offered**, whether anybody takes it or not.
//!
//! # What a relic does
//!
//! Every relic is a row of [`RELICS`]: an id, a tier, whether it is in a
//! new profile's pool, and an [`Effect`] made of three kinds of hook, so a
//! new relic is a row and at most a few lines where its hook is read:
//!
//! - a **stat modifier** ([`Effect::Stat`]): a percentage on a [`Stat`] —
//!   weapon damage, accuracy, move speed, armour, class cooldowns, healing
//!   received, bounty — under a [`When`] (always, or while another
//!   player's Bim is down). `World::skill_of`, the cooldowns, the beam,
//!   the medkit and the bounty read [`stat_percent`];
//! - a **trigger** ([`Effect::On`], a [`Hook`]): on a kill, a hit taken,
//!   going down, a mission's start or an ability used, an [`Action`] —
//!   getting up again, cooldowns taken off, a spell untouchable — under
//!   **conditions**: once a mission, and below a share of health;
//! - and the one that fits neither, [`Effect::EveryNthShot`], which the
//!   room counts (`bims::combat::Skill::overcharge`).
//!
//! No number is written here: every one is a constant in [`crate::data`].
//! No word either: the names and the descriptions are the app's
//! (`names::relic_name`).
//!
//! # Where relics come from
//!
//! A site cleared **with machines in it** offers [`data::RELIC_OFFER`]
//! relics of its tier ([`offer`]), and a held site may hide a **cache**
//! ([`data::RELIC_CACHE_CHANCE`]) that gives one. Either way the crew
//! choose together ([`RelicChoice`]): a player proposes a relic and the
//! player's Bim to have it, every connected player accepts, and a new
//! proposal clears every acceptance — the world map's vote over again.
//! A cache's relic is **pending** until the site is cleared, and lost if
//! the crew leave first.
//!
//! # Unlocks
//!
//! A [`Profile`] — the app keeps it on disk, beside the saves — is which
//! relics and classes a player has unlocked and how many runs they have
//! won. A won run unlocks [`data::RELICS_UNLOCKED_PER_WIN`] relics, the
//! first still locked in [`Relic::ALL`]'s order ([`Profile::record_run`]).
//! The host's profile is the run's pool, fixed at the start.

use crate::class::Class;
use crate::data;

/// A relic, by its place in [`RELICS`]. The code is what crosses the seam
/// and goes into the checksum; a relic added later goes on the end.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relic {
    FocusingLens = 0,
    ServoBraces = 1,
    FieldPlating = 2,
    CoolantLoop = 3,
    SteadyGrip = 4,
    TraumaKit = 5,
    SecondWind = 6,
    SalvageBeacon = 7,
    OverchargeCell = 8,
    LastStand = 9,
    KillRelay = 10,
    PhaseHarness = 11,
}

impl Relic {
    /// Every relic, in list order: the order a win unlocks them in.
    pub const ALL: [Relic; 12] = [
        Relic::FocusingLens,
        Relic::ServoBraces,
        Relic::FieldPlating,
        Relic::CoolantLoop,
        Relic::SteadyGrip,
        Relic::TraumaKit,
        Relic::SecondWind,
        Relic::SalvageBeacon,
        Relic::OverchargeCell,
        Relic::LastStand,
        Relic::KillRelay,
        Relic::PhaseHarness,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Relic> {
        Relic::ALL.get(code as usize).copied()
    }

    /// Its row of [`RELICS`].
    pub fn def(self) -> &'static RelicDef {
        &RELICS[self as usize]
    }

    /// One, two or three: the tier of site that offers it.
    pub fn tier(self) -> u8 {
        self.def().tier
    }

    /// Whether a new profile has it in its pool.
    pub fn starts_unlocked(self) -> bool {
        self.def().first
    }

    /// What it does.
    pub fn effect(self) -> Effect {
        self.def().effect
    }
}

/// A number a relic moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stat {
    /// What every bolt and blow of its weapon does.
    Damage,
    /// Its odds of hitting.
    Accuracy,
    /// Its pace, at all times.
    MoveSpeed,
    /// The protection of what it wears.
    Armour,
    /// How long its **class's** cooldowns are — the engineer's kits, the
    /// soldier's grenades, the taunt and the rally; never the medicine,
    /// which is everybody's. A minus is shorter.
    Cooldowns,
    /// What a medkit, a bandage and a medic's beam put back into it.
    HealingReceived,
    /// What the Republic pays for a machine it destroyed.
    Bounty,
}

/// When a stat modifier holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    Always,
    /// While another player's Bim is down — out cold.
    OtherPlayerDown,
}

/// What sets a relic's action off.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    /// A mission begins.
    MissionStart,
    /// A machine it hit last is destroyed.
    Kill,
    /// A hit lands on it.
    HitTaken,
    /// It goes down — out cold.
    Downed,
    /// It uses one of its class's two keys.
    AbilityUse,
}

/// What a relic's trigger does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    /// Up again `after` seconds of the mission clock, at `health_percent`
    /// of its health, if it is still down and alive by then.
    GetUp { after: f64, health_percent: u32 },
    /// Every class cooldown running on it that many seconds shorter.
    CooldownsLess { seconds: f64 },
    /// No hit takes anything from it for that many seconds.
    Untouchable { seconds: f32 },
}

/// A trigger, its conditions and its action.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hook {
    pub trigger: Trigger,
    /// Fires once a mission at most.
    pub once_per_mission: bool,
    /// Fires only with its health under this share, in per cent.
    pub below_health: Option<u32>,
    pub action: Action,
}

/// What a relic does: one of the three kinds of hook.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    Stat {
        stat: Stat,
        percent: i32,
        when: When,
    },
    /// Every `every`th shot of its weapon does `damage_percent` more.
    EveryNthShot { every: u32, damage_percent: i32 },
    On(Hook),
}

/// A relic's row: the data the whole of it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RelicDef {
    pub relic: Relic,
    pub tier: u8,
    /// In a new profile's pool, rather than unlocked by a win.
    pub first: bool,
    pub effect: Effect,
}

const fn stat(relic: Relic, tier: u8, first: bool, stat: Stat, percent: i32) -> RelicDef {
    RelicDef {
        relic,
        tier,
        first,
        effect: Effect::Stat {
            stat,
            percent,
            when: When::Always,
        },
    }
}

/// Every relic there is, in [`Relic::ALL`]'s order.
pub const RELICS: [RelicDef; 12] = [
    stat(
        Relic::FocusingLens,
        1,
        true,
        Stat::Damage,
        data::FOCUSING_LENS_DAMAGE_PERCENT,
    ),
    stat(
        Relic::ServoBraces,
        1,
        true,
        Stat::MoveSpeed,
        data::SERVO_BRACES_SPEED_PERCENT,
    ),
    stat(
        Relic::FieldPlating,
        1,
        true,
        Stat::Armour,
        data::FIELD_PLATING_ARMOUR_PERCENT,
    ),
    stat(
        Relic::CoolantLoop,
        1,
        true,
        Stat::Cooldowns,
        -data::COOLANT_LOOP_COOLDOWN_PERCENT,
    ),
    stat(
        Relic::SteadyGrip,
        1,
        false,
        Stat::Accuracy,
        data::STEADY_GRIP_ACCURACY_PERCENT,
    ),
    stat(
        Relic::TraumaKit,
        1,
        false,
        Stat::HealingReceived,
        data::TRAUMA_KIT_HEALING_PERCENT,
    ),
    RelicDef {
        relic: Relic::SecondWind,
        tier: 2,
        first: true,
        effect: Effect::On(Hook {
            trigger: Trigger::Downed,
            once_per_mission: true,
            below_health: None,
            action: Action::GetUp {
                after: data::SECOND_WIND_SECONDS,
                health_percent: data::SECOND_WIND_HEALTH_PERCENT,
            },
        }),
    },
    stat(
        Relic::SalvageBeacon,
        2,
        true,
        Stat::Bounty,
        data::SALVAGE_BEACON_BOUNTY_PERCENT,
    ),
    RelicDef {
        relic: Relic::OverchargeCell,
        tier: 2,
        first: false,
        effect: Effect::EveryNthShot {
            every: data::OVERCHARGE_CELL_EVERY,
            damage_percent: data::OVERCHARGE_CELL_DAMAGE_PERCENT,
        },
    },
    RelicDef {
        relic: Relic::LastStand,
        tier: 3,
        first: true,
        effect: Effect::Stat {
            stat: Stat::Damage,
            percent: data::LAST_STAND_DAMAGE_PERCENT,
            when: When::OtherPlayerDown,
        },
    },
    RelicDef {
        relic: Relic::KillRelay,
        tier: 3,
        first: true,
        effect: Effect::On(Hook {
            trigger: Trigger::Kill,
            once_per_mission: false,
            below_health: None,
            action: Action::CooldownsLess {
                seconds: data::KILL_RELAY_SECONDS,
            },
        }),
    },
    RelicDef {
        relic: Relic::PhaseHarness,
        tier: 3,
        first: false,
        effect: Effect::On(Hook {
            trigger: Trigger::HitTaken,
            once_per_mission: true,
            below_health: Some(data::PHASE_HARNESS_BELOW_PERCENT),
            action: Action::Untouchable {
                seconds: data::PHASE_HARNESS_SECONDS,
            },
        }),
    },
];

/// The percentage a Bim holding `held` has on `stat`, every relic's
/// modifier summed — nought with none. `other_down` is whether another
/// player's Bim is down, for [`When::OtherPlayerDown`].
pub fn stat_percent(held: &[Relic], stat: Stat, other_down: bool) -> i32 {
    held.iter()
        .map(|r| match r.effect() {
            Effect::Stat {
                stat: s,
                percent,
                when,
            } if s == stat => match when {
                When::Always => percent,
                When::OtherPlayerDown if other_down => percent,
                When::OtherPlayerDown => 0,
            },
            _ => 0,
        })
        .sum()
}

/// A percentage as a factor: ten is 1.1, minus ten 0.9 — never under
/// nought.
pub fn factor(percent: i32) -> f64 {
    (1.0 + f64::from(percent) / 100.0).max(0.0)
}

/// Every hook a Bim holding `held` has on `trigger`, with the relic.
pub fn hooks(held: &[Relic], trigger: Trigger) -> impl Iterator<Item = (Relic, Hook)> + '_ {
    held.iter().filter_map(move |&r| match r.effect() {
        Effect::On(hook) if hook.trigger == trigger => Some((r, hook)),
        _ => None,
    })
}

/// The overcharge a Bim holding `held` shoots with: every how many shots,
/// and what that shot's damage is multiplied by — `(0, 1.0)` with none.
pub fn overcharge(held: &[Relic]) -> (u32, f32) {
    held.iter()
        .find_map(|r| match r.effect() {
            Effect::EveryNthShot {
                every,
                damage_percent,
            } => Some((every, factor(damage_percent) as f32)),
            _ => None,
        })
        .unwrap_or((0, 1.0))
}

/// What a new profile's pool is: every relic marked for it, in list order.
pub fn starting_pool() -> Vec<Relic> {
    Relic::ALL
        .into_iter()
        .filter(|r| r.starts_unlocked())
        .collect()
}

/// A set of relics as bits, one a code — how a pool crosses the lobby.
pub fn mask_of(relics: &[Relic]) -> u64 {
    relics.iter().fold(0, |m, r| m | 1 << r.code())
}

/// The relics of a mask, in list order.
pub fn relics_of_mask(mask: u64) -> Vec<Relic> {
    Relic::ALL
        .into_iter()
        .filter(|r| mask & (1 << r.code()) != 0)
        .collect()
}

/// `n` relics drawn from `pool` for a site of `tier`, each draw off
/// `seed`: a draw takes from the site's tier while it has any left, and
/// from the next tier down when it has none, and nothing when no tier at
/// or below the site's has any. None twice, and in the order drawn.
pub fn offer(pool: &[Relic], tier: u8, n: usize, seed: u64) -> Vec<Relic> {
    let mut left: Vec<Relic> = pool.to_vec();
    let mut drawn = Vec::new();
    for i in 0..n {
        let Some(t) = (1..=tier.max(1))
            .rev()
            .find(|&t| left.iter().any(|r| r.tier() == t))
        else {
            break;
        };
        let of_tier: Vec<Relic> = left.iter().copied().filter(|r| r.tier() == t).collect();
        let roll = worldgen::rng::mix(seed ^ (i as u64).wrapping_mul(0x_9E37_79B9_7F4A_7C15));
        let pick = of_tier[(roll % of_tier.len() as u64) as usize];
        left.retain(|&r| r != pick);
        drawn.push(pick);
    }
    drawn
}

/// A number off the galaxy's seed for one site and one purpose: the same
/// on every machine, and drawn from no stream a fight draws from, so a
/// roll here moves nothing else.
fn site_roll(galaxy_seed: u64, star: u32, station: u32, salt: u64) -> u32 {
    let seed = worldgen::rng::mix(galaxy_seed ^ salt)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station));
    worldgen::rng::Rng::new(seed).below(100)
}

/// Whether a site the machines take hides a relic cache, at
/// [`data::RELIC_CACHE_CHANCE`] in a hundred: rolled once a site, off the
/// galaxy's seed.
pub fn cache_rolled(galaxy_seed: u64, star: u32, station: u32) -> bool {
    site_roll(galaxy_seed, star, station, 0x_5245_4C49_4343) < data::RELIC_CACHE_CHANCE
}

/// Whether a site's machines come at tier two, `hops` from the crew's own
/// star, once the world clock is past [`data::ENEMY_TIER2_HOURS`]: odds
/// of `hops` in [`data::ENEMY_TIER2_SURE_HOPS`], and always from there on.
/// Rolled once a site, off the galaxy's seed, so the map's quote and the
/// wave on arrival agree.
pub fn tier_two_rolled(galaxy_seed: u64, star: u32, station: u32, hops: u16) -> bool {
    let sure = data::ENEMY_TIER2_SURE_HOPS.max(1);
    if hops >= sure {
        return true;
    }
    let odds = u32::from(hops) * 100 / u32::from(sure);
    site_roll(galaxy_seed, star, station, 0x_5449_4552_3254) < odds
}

/// Where a relic choice came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Source {
    /// A site cleared with machines in it: chosen on the reward screen,
    /// after the departure check and before the map.
    Reward,
    /// A cache opened on a held site's deck: chosen in the mission with
    /// the game running, and pending until the site is cleared.
    Cache,
}

impl Source {
    pub fn code(self) -> u32 {
        match self {
            Source::Reward => 0,
            Source::Cache => 1,
        }
    }
}

/// A relic, or none, put to the crew for one player's Bim, and who has
/// said yes: the proposer counts as having, and a new proposal starts the
/// count again.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelicProposal {
    /// The relic, or `None` for taking none.
    pub relic: Option<Relic>,
    /// The player slot whose Bim would have it.
    pub to: u32,
    /// The player slot that put it.
    pub by: u32,
    /// One a player slot.
    pub accepted: Vec<bool>,
}

impl RelicProposal {
    /// Whether every player still at the keyboard has said yes; a slot
    /// past `connected` counts as connected.
    pub fn carried(&self, connected: &[bool]) -> bool {
        self.accepted
            .iter()
            .enumerate()
            .all(|(slot, &yes)| yes || !connected.get(slot).copied().unwrap_or(true))
    }
}

/// The relics on offer and the proposal on the table.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelicChoice {
    pub source: Source,
    /// The site's tier the offer was drawn at.
    pub tier: u8,
    pub options: Vec<Relic>,
    pub proposal: Option<RelicProposal>,
}

/// Everything the run keeps about relics. Saved and in `world_checksum`
/// whole, as part of [`crate::Run`].
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Relics {
    /// What is still to be offered, in list order: the host's unlocked
    /// relics at the start, one less every relic offered.
    pub pool: Vec<Relic>,
    /// One a player slot: the relics that player's Bim holds, in the order
    /// it was given them.
    pub held: Vec<Vec<Relic>>,
    /// Relics given out of a cache on a site not cleared yet, with the
    /// player slot each is for: kept the step the site is cleared, lost if
    /// the crew leave before.
    pub pending: Vec<(u32, Relic)>,
    /// The choice being made, if one is.
    pub choice: Option<RelicChoice>,
    /// One a player slot: the once-a-mission relics that have fired this
    /// mission.
    pub fired: Vec<Vec<Relic>>,
    /// One a player slot: the mission minute its Bim went down, while it
    /// is down and a relic is waiting to get it up.
    pub down_since: Vec<Option<f64>>,
    /// How many offers have been drawn this run: what the next is seeded
    /// with, beside the site.
    pub offers: u32,
}

impl Relics {
    /// A run's relics: the pool it opens with, `players` Bims holding none.
    pub fn new(pool: Vec<Relic>, players: u32) -> Relics {
        let players = players as usize;
        Relics {
            pool,
            held: vec![Vec::new(); players],
            pending: Vec::new(),
            choice: None,
            fired: vec![Vec::new(); players],
            down_since: vec![None; players],
            offers: 0,
        }
    }

    /// What a player's Bim holds.
    pub fn of(&self, slot: u32) -> &[Relic] {
        self.held.get(slot as usize).map_or(&[], |h| h.as_slice())
    }

    /// Whether a relic is in the run at all — held, pending or on offer
    /// now — which is what keeps it out of every later offer.
    pub fn in_play(&self, relic: Relic) -> bool {
        self.held.iter().flatten().any(|&r| r == relic)
            || self.pending.iter().any(|&(_, r)| r == relic)
            || self
                .choice
                .as_ref()
                .is_some_and(|c| c.options.contains(&relic))
    }

    /// Give a relic to a player's Bim for good.
    pub fn give(&mut self, slot: u32, relic: Relic) {
        let at = slot as usize;
        if self.held.len() <= at {
            self.held.resize(at + 1, Vec::new());
        }
        if !self.held[at].contains(&relic) {
            self.held[at].push(relic);
        }
    }

    /// Whether a once-a-mission relic has fired this mission.
    pub fn has_fired(&self, slot: u32, relic: Relic) -> bool {
        self.fired
            .get(slot as usize)
            .is_some_and(|f| f.contains(&relic))
    }

    pub(crate) fn mark_fired(&mut self, slot: u32, relic: Relic) {
        let at = slot as usize;
        if self.fired.len() <= at {
            self.fired.resize(at + 1, Vec::new());
        }
        if !self.fired[at].contains(&relic) {
            self.fired[at].push(relic);
        }
    }

    /// A mission begins: every once-a-mission relic ready again and
    /// nobody waiting to get up.
    pub(crate) fn new_mission(&mut self, players: u32) {
        let players = players as usize;
        self.fired = vec![Vec::new(); players];
        self.down_since = vec![None; players];
        if self.held.len() < players {
            self.held.resize(players, Vec::new());
        }
    }
}

/// What a player has unlocked between runs, and how many runs they have
/// won — the app keeps it as `bims/profile.ron` beside the saves. A code a
/// relic or class, so a profile written by a build with more of either
/// still reads.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Profile {
    /// The relics unlocked, by code, in list order.
    pub relics: Vec<u32>,
    /// The classes unlocked, by code.
    pub classes: Vec<u32>,
    /// Runs won.
    pub wins: u32,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile::new()
    }
}

impl Profile {
    /// A new profile: the starting pool and every class not marked
    /// unlockable.
    pub fn new() -> Profile {
        Profile {
            relics: starting_pool().into_iter().map(Relic::code).collect(),
            classes: Class::ALL
                .into_iter()
                .filter(|c| !c.unlockable())
                .map(Class::code)
                .collect(),
            wins: 0,
        }
    }

    /// The relics unlocked, in list order: a run's pool, when this is the
    /// host's.
    pub fn pool(&self) -> Vec<Relic> {
        Relic::ALL
            .into_iter()
            .filter(|r| self.relics.contains(&r.code()))
            .collect()
    }

    /// Whether a class may be picked.
    pub fn class_unlocked(&self, class: Class) -> bool {
        !class.unlockable() || self.classes.contains(&class.code())
    }

    /// A run over: won, [`data::RELICS_UNLOCKED_PER_WIN`] relics unlocked —
    /// the first still locked, in list order — and the win counted; lost,
    /// nothing. The relics unlocked now.
    pub fn record_run(&mut self, won: bool) -> Vec<Relic> {
        if !won {
            return Vec::new();
        }
        self.wins = self.wins.saturating_add(1);
        let fresh: Vec<Relic> = Relic::ALL
            .into_iter()
            .filter(|r| !self.relics.contains(&r.code()))
            .take(data::RELICS_UNLOCKED_PER_WIN)
            .collect();
        for r in &fresh {
            self.relics.push(r.code());
        }
        self.relics.sort_unstable();
        fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_in_code_order_and_every_tier_is_one_to_three() {
        for (i, def) in RELICS.iter().enumerate() {
            assert_eq!(def.relic.code() as usize, i);
            assert!((1..=3).contains(&def.tier));
            assert_eq!(Relic::from_code(i as u32), Some(def.relic));
        }
        assert_eq!(Relic::from_code(RELICS.len() as u32), None);
    }

    #[test]
    fn the_starting_pool_is_the_table_s_first_column() {
        use Relic::*;
        assert_eq!(
            starting_pool(),
            vec![
                FocusingLens,
                ServoBraces,
                FieldPlating,
                CoolantLoop,
                SecondWind,
                SalvageBeacon,
                LastStand,
                KillRelay
            ]
        );
    }

    #[test]
    fn an_offer_takes_the_site_s_tier_then_the_next_lower_and_never_twice() {
        let pool = Relic::ALL.to_vec();
        for seed in 0..200 {
            let three = offer(&pool, 3, 3, seed);
            assert_eq!(three.len(), 3);
            assert!(three.iter().all(|r| r.tier() == 3), "{three:?}");
            let mut sorted = three.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 3, "none twice");
        }
        // Tier two has three (Second Wind, Salvage Beacon, Overcharge
        // Cell): the fourth draw falls to tier one.
        let four = offer(&pool, 2, 4, 7);
        assert_eq!(four.iter().filter(|r| r.tier() == 2).count(), 3);
        assert_eq!(four[3].tier(), 1);
        // Nothing at or below the tier: nothing, even with tier three left.
        let high = vec![Relic::KillRelay, Relic::LastStand];
        assert!(offer(&high, 1, 3, 1).is_empty());
        assert!(offer(&[], 3, 3, 1).is_empty());
        // Short of three: as many as there are.
        assert_eq!(offer(&[Relic::FocusingLens], 3, 3, 1), vec![Relic::FocusingLens]);
    }

    #[test]
    fn a_win_unlocks_the_first_locked_relics_in_list_order_and_a_loss_nothing() {
        let mut profile = Profile::new();
        assert!(profile.record_run(false).is_empty());
        assert_eq!(profile.wins, 0);
        assert_eq!(profile.pool(), starting_pool());
        let first = profile.record_run(true);
        let locked: Vec<Relic> = Relic::ALL
            .into_iter()
            .filter(|r| !r.starts_unlocked())
            .collect();
        assert_eq!(first, locked[..data::RELICS_UNLOCKED_PER_WIN].to_vec());
        assert_eq!(profile.wins, 1);
        let second = profile.record_run(true);
        assert_eq!(
            second,
            locked[data::RELICS_UNLOCKED_PER_WIN..]
                .iter()
                .copied()
                .take(data::RELICS_UNLOCKED_PER_WIN)
                .collect::<Vec<_>>()
        );
        // Everything unlocked: a win unlocks nothing more, and counts.
        for _ in 0..10 {
            profile.record_run(true);
        }
        assert_eq!(profile.pool(), Relic::ALL.to_vec());
        assert!(profile.record_run(true).is_empty());
    }

    #[test]
    fn every_class_is_unlocked_in_a_new_profile() {
        let profile = Profile::new();
        for class in Class::ALL {
            assert!(profile.class_unlocked(class));
        }
    }

    #[test]
    fn a_pool_crosses_as_a_mask_and_back() {
        let pool = starting_pool();
        assert_eq!(relics_of_mask(mask_of(&pool)), pool);
        assert_eq!(relics_of_mask(mask_of(&Relic::ALL)), Relic::ALL.to_vec());
    }

    #[test]
    fn stat_modifiers_sum_and_last_stand_waits_on_a_downed_friend() {
        let held = [Relic::FocusingLens, Relic::LastStand];
        assert_eq!(
            stat_percent(&held, Stat::Damage, false),
            data::FOCUSING_LENS_DAMAGE_PERCENT
        );
        assert_eq!(
            stat_percent(&held, Stat::Damage, true),
            data::FOCUSING_LENS_DAMAGE_PERCENT + data::LAST_STAND_DAMAGE_PERCENT
        );
        assert_eq!(stat_percent(&held, Stat::Armour, true), 0);
        assert!(stat_percent(&[Relic::CoolantLoop], Stat::Cooldowns, false) < 0);
    }
}

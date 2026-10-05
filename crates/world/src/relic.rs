//! Relics (feature 106, rebuilt October 2026): what a crew wins off an
//! elite's fight and keeps for the rest of the run, and the profile kept
//! between runs.
//!
//! # What a relic is
//!
//! A [`Relic`] is **the crew's**, never one Bim's: it is held once, by
//! the run ([`Relics::held`]), and moves something for everybody it
//! names — every Bim, the players' own, or the bots (every crew member no
//! player steers: bots, hired hands, a commander's reinforcements), or
//! the run itself (the bounty, the experience, the waves, the trader). It
//! is never moved, dropped or sold, and there is **no cap** on how many
//! the crew hold.
//!
//! **Every relic has a price.** A row of [`RELICS`] is a list of
//! [`Modifier`]s — a [`Stat`] moved by a share for a [`Who`] — and every
//! row has at least one that helps and one that hurts
//! ([`Modifier::helps`]), so taking one is a choice and taking none is
//! one too. None of them slows a revive or cuts the healing, which would
//! be a cost the medic and the commander's Medivac paid alone.
//!
//! No number is written here: every one is a constant in [`crate::data`].
//! No word either: the names and the lines are the app's
//! (`names::relic_name`).
//!
//! # Where relics come from
//!
//! **Only an elite's fight drops them** (`crate::elite`): the clear offers
//! [`data::RELIC_OFFER`] drawn evenly off what the crew do not hold yet
//! ([`offer`]), and the crew choose one — or none — together
//! ([`RelicChoice`]): a player proposes, every connected player accepts,
//! and a new proposal clears every acceptance, the world map's vote over
//! again. No trader sells one and no cache hides one.

use crate::class::Class;
use crate::data;

/// A relic, by its place in [`RELICS`]. The code is what crosses the seam
/// and goes into the checksum; a relic added later goes on the end.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relic {
    GlassCannon = 0,
    HeavyPlating = 1,
    HairTrigger = 2,
    OverclockedCores = 3,
    BountyContract = 4,
    HuntersPact = 5,
    DrillSergeant = 6,
    LoneWolves = 7,
    BlackMarket = 8,
    Adrenaline = 9,
    SalvageBurn = 10,
    NaniteMesh = 11,
    CleanSweep = 12,
}

impl Relic {
    /// Every relic, in list order.
    pub const ALL: [Relic; 13] = [
        Relic::GlassCannon,
        Relic::HeavyPlating,
        Relic::HairTrigger,
        Relic::OverclockedCores,
        Relic::BountyContract,
        Relic::HuntersPact,
        Relic::DrillSergeant,
        Relic::LoneWolves,
        Relic::BlackMarket,
        Relic::Adrenaline,
        Relic::SalvageBurn,
        Relic::NaniteMesh,
        Relic::CleanSweep,
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

    /// What it does: its boons and its costs, in the order the app says
    /// them.
    pub fn modifiers(self) -> &'static [Modifier] {
        self.def().modifiers
    }
}

/// A number a relic moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stat {
    /// What every bolt and blow of a Bim's weapon does, in per cent.
    Damage,
    /// How fast a Bim pulls the trigger, in per cent.
    FireRate,
    /// A Bim's pace, at all times, in per cent.
    MoveSpeed,
    /// What a Bim takes of every hit that lands on it, in per cent. A
    /// minus is less.
    DamageTaken,
    /// How long a Bim's **class's** cooldowns are, in per cent. A minus is
    /// shorter.
    Cooldowns,
    /// What the crew's bolts and blows do **to an enemy** — a machine or
    /// one of the Manufacturers' people (October 2026) — in per cent.
    MachineDamage,
    /// What the Republic pays for every enemy down, in per cent.
    Bounty,
    /// The experience every enemy down is worth, in per cent.
    Experience,
    /// How many machines come in every wave, in per cent, rounded up.
    WaveSize,
    /// What a trader asks of the crew, in per cent. A minus is cheaper.
    TraderPrices,
    /// Hit points every Bim on its feet gets back a second — a number of
    /// them, not a share.
    Regen,
    /// The hit points every enemy — a machine or one of the
    /// Manufacturers' people — is laid with, in per cent.
    EnemyHealth,
    /// The experience a site paid every crew member since its last clear,
    /// in per cent, paid again at the clear when no player's Bim went
    /// down there since (October 2026, *Clean Sweep*,
    /// `World::pay_the_clean_sweep`).
    CleanExperience,
    /// What an enemy a bot took down pays, in per cent of its bounty: not
    /// a share moved but a floor — the most any relic held says, over
    /// `Rewards::bot_bounty_percent` when it is more ([`bot_bounty`]).
    BotBounty,
}

impl Stat {
    /// Whether more of it is better for the crew: false for what a minus
    /// helps (the damage taken, the cooldowns, the waves, the prices).
    pub fn more_is_better(self) -> bool {
        !matches!(
            self,
            Stat::DamageTaken
                | Stat::Cooldowns
                | Stat::WaveSize
                | Stat::TraderPrices
                | Stat::EnemyHealth
        )
    }
}

/// Whom a modifier moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    /// Every crew member — and, for a stat of the run's own (the bounty,
    /// the experience, the waves, the trader), the run.
    Everyone,
    /// The players' own Bims.
    Players,
    /// Every crew member no player steers: the bots, the hired hands, the
    /// townsfolk who joined, a commander's reinforcements.
    Bots,
}

impl Who {
    /// Whether it takes in a crew member that is (`bot`) or is not a bot.
    pub fn covers(self, bot: bool) -> bool {
        match self {
            Who::Everyone => true,
            Who::Players => !bot,
            Who::Bots => bot,
        }
    }
}

/// One thing a relic does: `stat` moved by `amount` — per cent, or hit
/// points a second for [`Stat::Regen`] — for `who`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Modifier {
    pub who: Who,
    pub stat: Stat,
    pub amount: i32,
}

impl Modifier {
    /// Whether it is a boon, rather than the price.
    pub fn helps(self) -> bool {
        (self.amount > 0) == self.stat.more_is_better()
    }
}

/// A relic's row: the data the whole of it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RelicDef {
    pub relic: Relic,
    pub modifiers: &'static [Modifier],
}

const fn m(who: Who, stat: Stat, amount: i32) -> Modifier {
    Modifier { who, stat, amount }
}

const fn row(relic: Relic, modifiers: &'static [Modifier]) -> RelicDef {
    RelicDef { relic, modifiers }
}

use Stat as S;
use Who as W;

/// Every relic there is, in [`Relic::ALL`]'s order: its boons first, then
/// its price.
pub const RELICS: [RelicDef; 13] = [
    row(
        Relic::GlassCannon,
        &[
            m(W::Everyone, S::Damage, data::GLASS_CANNON_DAMAGE),
            m(W::Everyone, S::DamageTaken, data::GLASS_CANNON_TAKEN),
        ],
    ),
    row(
        Relic::HeavyPlating,
        &[
            m(W::Everyone, S::DamageTaken, -data::HEAVY_PLATING_TAKEN),
            m(W::Everyone, S::MoveSpeed, -data::HEAVY_PLATING_SPEED),
        ],
    ),
    row(
        Relic::HairTrigger,
        &[
            m(W::Everyone, S::FireRate, data::HAIR_TRIGGER_FIRE_RATE),
            m(W::Everyone, S::Cooldowns, data::HAIR_TRIGGER_COOLDOWNS),
        ],
    ),
    row(
        Relic::OverclockedCores,
        &[
            m(
                W::Everyone,
                S::Cooldowns,
                -data::OVERCLOCKED_CORES_COOLDOWNS,
            ),
            m(W::Everyone, S::Damage, -data::OVERCLOCKED_CORES_DAMAGE),
        ],
    ),
    row(
        Relic::BountyContract,
        &[
            m(W::Everyone, S::Bounty, data::BOUNTY_CONTRACT_BOUNTY),
            m(W::Everyone, S::MachineDamage, -data::BOUNTY_CONTRACT_DAMAGE),
        ],
    ),
    row(
        Relic::HuntersPact,
        &[
            m(W::Everyone, S::Experience, data::HUNTERS_PACT_EXPERIENCE),
            m(W::Everyone, S::WaveSize, data::HUNTERS_PACT_WAVES),
        ],
    ),
    row(
        Relic::DrillSergeant,
        &[
            m(W::Bots, S::Damage, data::DRILL_SERGEANT_BOT_DAMAGE),
            m(W::Bots, S::DamageTaken, -data::DRILL_SERGEANT_BOT_TAKEN),
            m(W::Bots, S::BotBounty, data::DRILL_SERGEANT_BOT_BOUNTY),
            m(W::Players, S::Damage, -data::DRILL_SERGEANT_PLAYER_DAMAGE),
        ],
    ),
    row(
        Relic::LoneWolves,
        &[
            m(W::Players, S::Damage, data::LONE_WOLVES_PLAYER_DAMAGE),
            m(W::Players, S::MoveSpeed, data::LONE_WOLVES_PLAYER_SPEED),
            m(W::Bots, S::Damage, -data::LONE_WOLVES_BOT_DAMAGE),
        ],
    ),
    row(
        Relic::BlackMarket,
        &[
            m(W::Everyone, S::TraderPrices, -data::BLACK_MARKET_PRICES),
            m(W::Everyone, S::EnemyHealth, data::BLACK_MARKET_ENEMY_HEALTH),
        ],
    ),
    row(
        Relic::Adrenaline,
        &[
            m(W::Everyone, S::MoveSpeed, data::ADRENALINE_SPEED),
            m(W::Everyone, S::DamageTaken, data::ADRENALINE_TAKEN),
        ],
    ),
    row(
        Relic::SalvageBurn,
        &[
            m(W::Everyone, S::MachineDamage, data::SALVAGE_BURN_DAMAGE),
            m(W::Everyone, S::Bounty, -data::SALVAGE_BURN_BOUNTY),
        ],
    ),
    row(
        Relic::NaniteMesh,
        &[
            m(W::Everyone, S::Regen, data::NANITE_MESH_REGEN),
            m(W::Everyone, S::Damage, -data::NANITE_MESH_DAMAGE),
        ],
    ),
    row(
        Relic::CleanSweep,
        &[
            m(
                W::Everyone,
                S::CleanExperience,
                data::CLEAN_SWEEP_EXPERIENCE,
            ),
            m(W::Everyone, S::DamageTaken, data::CLEAN_SWEEP_TAKEN),
        ],
    ),
];

/// What the relics `held` put on `stat` for a crew member that is (`bot`)
/// or is not a bot: every modifier of theirs on it that covers it, summed
/// — nought with none.
pub fn percent(held: &[Relic], stat: Stat, bot: bool) -> i32 {
    held.iter()
        .flat_map(|r| r.modifiers())
        .filter(|m| m.stat == stat && m.who.covers(bot))
        .map(|m| m.amount)
        .sum()
}

/// What the relics `held` put on a stat of the run's own — the bounty,
/// the experience, the waves, the trader's prices: every modifier on it,
/// whoever it names.
pub fn crew_percent(held: &[Relic], stat: Stat) -> i32 {
    held.iter()
        .flat_map(|r| r.modifiers())
        .filter(|m| m.stat == stat)
        .map(|m| m.amount)
        .sum()
}

/// What a bot's kill pays, in per cent of the enemy's bounty, with the
/// relics `held`: `dial` (`Rewards::bot_bounty_percent`), or the most a
/// relic's [`Stat::BotBounty`] says when that is more.
pub fn bot_bounty(held: &[Relic], dial: u32) -> u32 {
    held.iter()
        .flat_map(|r| r.modifiers())
        .filter(|m| m.stat == Stat::BotBounty)
        .map(|m| m.amount.max(0) as u32)
        .fold(dial, u32::max)
}

/// A percentage as a factor: ten is 1.1, minus ten 0.9 — never under
/// nought.
pub fn factor(percent: i32) -> f64 {
    (1.0 + f64::from(percent) / 100.0).max(0.0)
}

/// `n` relics drawn evenly off `drawable`, off `seed`: none twice, in the
/// order drawn, as many as there are when there are fewer. The list is the
/// caller's and is not touched.
pub fn offer(drawable: &[Relic], n: usize, seed: u64) -> Vec<Relic> {
    let mut left: Vec<Relic> = drawable.to_vec();
    let mut drawn = Vec::new();
    for i in 0..n {
        if left.is_empty() {
            break;
        }
        let roll = worldgen::rng::mix(seed ^ (i as u64).wrapping_mul(0x_9E37_79B9_7F4A_7C15));
        let pick = left.remove((roll % left.len() as u64) as usize);
        drawn.push(pick);
    }
    drawn
}

/// A relic, or none, put to the crew, and who has said yes: the proposer
/// counts as having, and a new proposal starts the count again.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelicProposal {
    /// The relic, or `None` for taking none.
    pub relic: Option<Relic>,
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

/// The relics on offer off an elite's clear and the proposal on the table.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelicChoice {
    pub options: Vec<Relic>,
    pub proposal: Option<RelicProposal>,
}

impl RelicChoice {
    /// A choice of `options`, nothing proposed yet.
    pub fn new(options: Vec<Relic>) -> RelicChoice {
        RelicChoice {
            options,
            proposal: None,
        }
    }
}

/// Everything the run keeps about relics. Saved and in `world_checksum`
/// whole, as part of [`crate::Run`].
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Relics {
    /// What the crew hold, in the order they took them.
    pub held: Vec<Relic>,
    /// The choice being made, if one is.
    pub choice: Option<RelicChoice>,
    /// How many offers have been drawn this run: what the next is seeded
    /// with, beside the site.
    pub offers: u32,
}

impl Relics {
    /// What may still be offered: every relic the crew do not hold, in
    /// list order.
    pub fn drawable(&self) -> Vec<Relic> {
        Relic::ALL
            .into_iter()
            .filter(|r| !self.held.contains(r))
            .collect()
    }

    /// A relic the crew take, for good.
    pub fn give(&mut self, relic: Relic) {
        if !self.held.contains(&relic) {
            self.held.push(relic);
        }
    }
}

/// What a player has unlocked between runs, and how many runs they have
/// won — the app keeps it as `bims/profile.ron` beside the saves. A code a
/// class, so a profile written by a build with more of them still reads.
/// (It kept the unlocked relics too, until every relic was in every run;
/// an old file's list is read past.)
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Profile {
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
    /// A new profile: every class not marked unlockable.
    pub fn new() -> Profile {
        Profile {
            classes: Class::ALL
                .into_iter()
                .filter(|c| !c.unlockable())
                .map(Class::code)
                .collect(),
            wins: 0,
        }
    }

    /// Whether a class may be picked.
    pub fn class_unlocked(&self, class: Class) -> bool {
        !class.unlockable() || self.classes.contains(&class.code())
    }

    /// A run over: won, the win counted; lost, nothing.
    pub fn record_run(&mut self, won: bool) {
        if won {
            self.wins = self.wins.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_in_code_order() {
        for (i, def) in RELICS.iter().enumerate() {
            assert_eq!(def.relic.code() as usize, i);
            assert_eq!(Relic::from_code(i as u32), Some(def.relic));
        }
        assert_eq!(Relic::from_code(RELICS.len() as u32), None);
    }

    /// The player's rule: every relic is a boon **with a price**, and
    /// none of them is paid in revives or healing.
    #[test]
    fn every_relic_helps_and_costs() {
        for relic in Relic::ALL {
            let mods = relic.modifiers();
            assert!(mods.iter().any(|m| m.helps()), "{relic:?} helps nobody");
            assert!(mods.iter().any(|m| !m.helps()), "{relic:?} costs nothing");
            assert!(mods.iter().all(|m| m.amount != 0), "{relic:?}");
            assert!(
                mods.iter().all(|m| m.helps() || m.stat != Stat::Regen),
                "{relic:?} costs the healing"
            );
        }
    }

    #[test]
    fn a_modifier_covers_whom_it_names() {
        let held = [Relic::DrillSergeant, Relic::GlassCannon];
        assert_eq!(
            percent(&held, Stat::Damage, true),
            data::DRILL_SERGEANT_BOT_DAMAGE + data::GLASS_CANNON_DAMAGE
        );
        assert_eq!(
            percent(&held, Stat::Damage, false),
            data::GLASS_CANNON_DAMAGE - data::DRILL_SERGEANT_PLAYER_DAMAGE
        );
        assert_eq!(percent(&held, Stat::FireRate, false), 0);
        assert_eq!(bot_bounty(&[], 5), 5);
        assert_eq!(bot_bounty(&held, 5), data::DRILL_SERGEANT_BOT_BOUNTY as u32);
        assert_eq!(bot_bounty(&held, 120), 120, "a floor, never a cut");
        assert_eq!(
            crew_percent(&[Relic::BountyContract, Relic::SalvageBurn], Stat::Bounty),
            data::BOUNTY_CONTRACT_BOUNTY - data::SALVAGE_BURN_BOUNTY
        );
        assert_eq!(
            crew_percent(&[Relic::BlackMarket], Stat::EnemyHealth),
            data::BLACK_MARKET_ENEMY_HEALTH
        );
        assert_eq!(percent(&[], Stat::Damage, false), 0);
    }

    #[test]
    fn an_offer_draws_none_twice_evenly_and_leaves_the_list() {
        let all = Relic::ALL.to_vec();
        let mut seen = [0u32; Relic::ALL.len()];
        for seed in 0..6_000 {
            let three = offer(&all, 3, seed);
            assert_eq!(three.len(), 3);
            let mut sorted = three.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 3, "none twice: {three:?}");
            for r in three {
                seen[r.code() as usize] += 1;
            }
        }
        // Each its share of eighteen thousand draws, give or take.
        let share = 18_000 / Relic::ALL.len() as u32;
        for (code, n) in seen.iter().enumerate() {
            assert!((share - 200..=share + 200).contains(n), "relic {code}: {n}");
        }
        assert_eq!(all, Relic::ALL.to_vec(), "the list is the caller's");
        assert_eq!(offer(&all, 3, 42), offer(&all, 3, 42));
        assert_eq!(offer(&[Relic::Adrenaline], 3, 1), vec![Relic::Adrenaline]);
        assert!(offer(&[], 3, 1).is_empty());
    }

    #[test]
    fn what_is_held_is_never_offered_again() {
        let mut relics = Relics::default();
        relics.give(Relic::HairTrigger);
        relics.give(Relic::HairTrigger);
        assert_eq!(relics.held, vec![Relic::HairTrigger]);
        assert!(!relics.drawable().contains(&Relic::HairTrigger));
        assert_eq!(relics.drawable().len(), Relic::ALL.len() - 1);
    }

    #[test]
    fn a_win_is_counted_and_a_loss_is_not() {
        let mut profile = Profile::new();
        profile.record_run(false);
        assert_eq!(profile.wins, 0);
        profile.record_run(true);
        assert_eq!(profile.wins, 1);
        for class in Class::ALL {
            assert!(profile.class_unlocked(class));
        }
    }
}

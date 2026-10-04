//! The **items** of a player's Bim, Dota 2's way (October 2026): six
//! slots on the loadout ([`ITEM_SLOTS`], `Gear::items`), each holding a
//! [`Module`] or nothing — bought at a trader, upgraded there a tier at a
//! time, sold back there for half what it cost, moved in the Armory like
//! a weapon. A bot never carries one.
//!
//! The room reads what it must of them itself, off the loadout: the
//! health a *Reactor Heart* or a *Pressure Seal* adds to the bar
//! ([`Gear::max_health`]). The
//! rest is the world's — the crit, the regeneration, the blink, the
//! ultimate's extra rank, and since step two the cooldowns cut, the fire
//! rate and reach, the leech, the arc and the three actives (mender,
//! reset, shell) — through the numbers here, so every number an item has
//! is in this file and nowhere else.
//!
//! The words — a name and a line on what each does — are the app's
//! (`names.rs`); what crosses is the kind's code and the tier's.

use crate::combat::{Gear, Tier};
use crate::health::MAX_HEALTH;

/// How many items a Bim carries: six, from the first day (four until
/// the player's word later in October 2026).
pub const ITEM_SLOTS: usize = 6;

/// What an item is. The code crosses the seam and goes into the
/// checksum; a kind added later goes on the end.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModuleKind {
    /// *Blink Drive* (Dota's Blink Dagger): active, the Bim put down at a
    /// spot it can see within its reach, at once; not for
    /// [`BLINK_HIT_LOCK_SECONDS`] after a hit.
    BlinkDrive = 0,
    /// *Executioner* (Crystalys, then Daedalus): passive, a chance a
    /// weapon hit is critical.
    Executioner = 1,
    /// *Reactor Heart* (Vanguard, then the Heart of Tarrasque): passive,
    /// more health and regeneration, faster after a while unhurt.
    ReactorHeart = 2,
    /// *Override Core* (Aghanim's Scepter): passive, the class's ultimate
    /// one rank higher than bought — up to a fifth rank no point buys.
    /// Made at one tier.
    OverrideCore = 3,
    // Step two (October 2026): three relics brought into item form, and
    // six more after Dota's.
    /// *Coolant Loop* (the relic it was): passive, the class's cooldowns
    /// shorter.
    CoolantLoop = 4,
    /// *Pressure Seal* (the relic it was, Ring of Health): passive, more
    /// health on the bar (it gave hit points back all the time until
    /// October 2026, the player's word).
    PressureSeal = 5,
    /// *Steady Grip* (the relic it was, Hyperstone): passive, the trigger
    /// pulled faster.
    SteadyGrip = 6,
    /// *Long Barrel* (Dragon Lance): passive, the weapon reaches further.
    LongBarrel = 7,
    /// *Leech Capacitor* (Satanic's lifesteal): passive, a share of the
    /// weapon's damage on an enemy back as hit points.
    LeechCapacitor = 8,
    /// *Arc Coil* (Maelstrom): passive, every few weapon hits on an enemy
    /// arc to the enemies round it, machines and Manufacturers alike.
    ArcCoil = 9,
    /// *Field Mender* (Mekansm): active, every crewmate near healed.
    FieldMender = 10,
    /// *Reset Capacitor* (Refresher Orb): active, every class cooldown
    /// ready and every charge full.
    ResetCapacitor = 11,
    /// *Ablative Shell* (Black King Bar): active, a few seconds of less
    /// damage and no armour stripped.
    AblativeShell = 12,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 13] = [
        ModuleKind::BlinkDrive,
        ModuleKind::Executioner,
        ModuleKind::ReactorHeart,
        ModuleKind::OverrideCore,
        ModuleKind::CoolantLoop,
        ModuleKind::PressureSeal,
        ModuleKind::SteadyGrip,
        ModuleKind::LongBarrel,
        ModuleKind::LeechCapacitor,
        ModuleKind::ArcCoil,
        ModuleKind::FieldMender,
        ModuleKind::ResetCapacitor,
        ModuleKind::AblativeShell,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<ModuleKind> {
        ModuleKind::ALL.get(code as usize).copied()
    }

    /// Whether it is made at `tier`: every kind at every tier but the
    /// *Override Core*, which is one thing and made at tier one alone —
    /// never on a shelf at another, never upgraded — and the *Reset
    /// Capacitor*, made at tier three alone (October 2026): on no shelf
    /// before the run's tier-three day, never upgraded.
    pub fn made_at(self, tier: Tier) -> bool {
        match self {
            ModuleKind::OverrideCore => tier == Tier::One,
            ModuleKind::ResetCapacitor => tier == Tier::Three,
            _ => true,
        }
    }

    /// The lowest tier it is made at: three for the *Reset Capacitor*,
    /// one for every other kind.
    pub fn min_tier(self) -> Tier {
        Tier::ALL
            .into_iter()
            .find(|&t| self.made_at(t))
            .unwrap_or(Tier::One)
    }

    /// Whether it has a tier worth saying: all but the *Override Core*.
    /// The *Reset Capacitor*'s, its one, is said: a tier-three thing.
    pub fn tiered(self) -> bool {
        self != ModuleKind::OverrideCore
    }

    /// Whether its key does something: the *Blink Drive*, the *Field
    /// Mender*, the *Reset Capacitor* and the *Ablative Shell*. The rest
    /// work on their own.
    pub fn active(self) -> bool {
        matches!(
            self,
            ModuleKind::BlinkDrive
                | ModuleKind::FieldMender
                | ModuleKind::ResetCapacitor
                | ModuleKind::AblativeShell
        )
    }

    /// The kind at `tier`, nothing paid for it.
    pub fn at(self, tier: Tier) -> Module {
        Module {
            kind: self,
            tier,
            paid: 0,
        }
    }
}

/// One item: a kind at a tier, and what its owner paid for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Module {
    pub kind: ModuleKind,
    pub tier: Tier,
    /// What it cost its owner at the traders, every upgrade added on:
    /// half of it comes back on a sale (the world's `World::sell_value`).
    /// Nought for one never bought (a probe's, a save's from before),
    /// which sells for half its tier's price instead. The room never
    /// reads it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub paid: u64,
}

/// A number by tier, one to three.
fn by_tier<T: Copy>(table: [T; 3], tier: Tier) -> T {
    table[(tier.code().clamp(1, 3) - 1) as usize]
}

// --- the Blink Drive ---------------------------------------------------------

/// How far a blink puts the Bim, in tiles, by tier. A spot farther off
/// is taken as the farthest the drive reaches along the way to it.
pub const BLINK_RANGE_TILES: [f32; 3] = [6.0, 8.0, 10.0];
/// Seconds of the mission clock before the drive blinks again, by tier.
pub const BLINK_COOLDOWN_SECONDS: [f32; 3] = [14.0, 12.0, 10.0];
/// Seconds after a hit taken before the drive will blink — Dota's rule,
/// so it puts a Bim where it wants to fight rather than out of a fight
/// it is losing.
pub const BLINK_HIT_LOCK_SECONDS: f32 = 3.0;

// --- the Executioner -----------------------------------------------------------

/// The chance a weapon hit is critical, by tier.
pub const EXECUTIONER_CHANCE: [f32; 3] = [0.12, 0.18, 0.25];
/// What a critical hit does, as a multiple of the weapon's flat damage,
/// by tier — the soldier's Weak Spot's way of counting.
pub const EXECUTIONER_DAMAGE: [f32; 3] = [1.6, 1.9, 2.25];

// --- the Reactor Heart ------------------------------------------------------------

/// Hit points on top of the bar, by tier.
pub const HEART_HEALTH: [f32; 3] = [25.0, 40.0, 60.0];
/// Per cent of the Bim's whole bar a second it puts back while it is on
/// its feet, by tier: every heal is a share of the bar since October 2026
/// (the player's word), so the first tier is half a per cent a second.
pub const HEART_REGEN: [f32; 3] = [0.5, 1.0, 1.0];
/// And after [`HEART_QUIET_SECONDS`] without a hit, instead, by tier.
pub const HEART_QUIET_REGEN: [f32; 3] = [1.5, 3.0, 5.0];
/// How long without a hit before the quicker regeneration starts.
pub const HEART_QUIET_SECONDS: f32 = 6.0;

// --- step two (October 2026) ---------------------------------------------------

/// *Coolant Loop*: per cent off the class's cooldowns, by tier. Several
/// add, never past [`COOLDOWN_CUT_MOST`].
pub const COOLANT_LOOP_PERCENT: [i32; 3] = [10, 15, 20];
/// The most the items take off a cooldown, in per cent.
pub const COOLDOWN_CUT_MOST: i32 = 50;
/// *Pressure Seal*: hit points on top of the bar, by tier — a little
/// under the *Reactor Heart*'s at about half its price, and no
/// regeneration (it gave 0.5 / 1 / 1.5 per cent of the bar a second back
/// until October 2026: "rework pressure seal to give hp instead of
/// regen").
pub const PRESSURE_SEAL_HEALTH: [f32; 3] = [20.0, 30.0, 45.0];
/// *Steady Grip*: per cent on the fire rate, by tier.
pub const STEADY_GRIP_PERCENT: [i32; 3] = [10, 15, 20];
/// *Long Barrel*: tiles on the weapon's range, by tier.
pub const LONG_BARREL_TILES: [f32; 3] = [2.0, 3.0, 4.0];
/// *Leech Capacitor*: the share of a weapon hit's damage on an enemy back
/// as the holder's hit points, by tier.
pub const LEECH_SHARE: [f32; 3] = [0.08, 0.12, 0.16];
/// *Arc Coil*: every this many weapon hits on enemies, the last arcs.
pub const ARC_EVERY: u32 = 4;
/// How many other enemies an arc reaches, by tier.
pub const ARC_TARGETS: [usize; 3] = [2, 3, 4];
/// What an arc does to each, by tier.
pub const ARC_DAMAGE: [f32; 3] = [15.0, 25.0, 40.0];
/// How far an arc jumps from the enemy struck, in tiles.
pub const ARC_REACH_TILES: f32 = 4.0;
/// *Field Mender*: per cent of each one's whole bar to every crewmate
/// within its reach, the holder included, by tier.
pub const MENDER_HEAL: [f32; 3] = [30.0, 45.0, 60.0];
/// Its reach, in tiles.
pub const MENDER_TILES: f32 = 5.0;
/// Its cooldown, in seconds, by tier.
pub const MENDER_COOLDOWN_SECONDS: [f32; 3] = [45.0, 40.0, 35.0];
/// *Reset Capacitor*: its cooldown, in seconds, by tier — eighty, made
/// at tier three alone (October 2026; it was 180 / 150 / 120 at every
/// tier). The table stays by tier so a saved one of a lower tier reads.
pub const RESET_COOLDOWN_SECONDS: [f32; 3] = [80.0, 80.0, 80.0];
/// *Ablative Shell*: how long it lasts, in seconds, by tier.
pub const SHELL_SECONDS: [f32; 3] = [4.0, 5.0, 6.0];
/// What the damage taken is multiplied by while it lasts.
pub const SHELL_DAMAGE_TAKEN: f32 = 0.6;
/// Its cooldown, in seconds, by tier.
pub const SHELL_COOLDOWN_SECONDS: [f32; 3] = [60.0, 55.0, 50.0];

impl Module {
    /// An active item's cooldown, in seconds; nought for a passive one.
    pub fn cooldown(self) -> f32 {
        let table = match self.kind {
            ModuleKind::BlinkDrive => BLINK_COOLDOWN_SECONDS,
            ModuleKind::FieldMender => MENDER_COOLDOWN_SECONDS,
            ModuleKind::ResetCapacitor => RESET_COOLDOWN_SECONDS,
            ModuleKind::AblativeShell => SHELL_COOLDOWN_SECONDS,
            _ => return 0.0,
        };
        by_tier(table, self.tier)
    }

    /// The per cent a *Coolant Loop* takes off the class's cooldowns.
    pub fn cooldown_cut(self) -> i32 {
        if self.kind == ModuleKind::CoolantLoop {
            by_tier(COOLANT_LOOP_PERCENT, self.tier)
        } else {
            0
        }
    }

    /// The per cent a *Steady Grip* puts on the fire rate.
    pub fn fire_rate_percent(self) -> i32 {
        if self.kind == ModuleKind::SteadyGrip {
            by_tier(STEADY_GRIP_PERCENT, self.tier)
        } else {
            0
        }
    }

    /// The tiles a *Long Barrel* puts on the weapon's range.
    pub fn range_tiles(self) -> f32 {
        if self.kind == ModuleKind::LongBarrel {
            by_tier(LONG_BARREL_TILES, self.tier)
        } else {
            0.0
        }
    }

    /// The share of a hit's damage a *Leech Capacitor* gives back.
    pub fn leech(self) -> f32 {
        if self.kind == ModuleKind::LeechCapacitor {
            by_tier(LEECH_SHARE, self.tier)
        } else {
            0.0
        }
    }

    /// An *Arc Coil*'s arc: how many enemies it reaches and what it does
    /// to each.
    pub fn arc(self) -> Option<(usize, f32)> {
        (self.kind == ModuleKind::ArcCoil).then(|| {
            (
                by_tier(ARC_TARGETS, self.tier),
                by_tier(ARC_DAMAGE, self.tier),
            )
        })
    }

    /// A *Field Mender*'s heal.
    pub fn mend(self) -> f32 {
        if self.kind == ModuleKind::FieldMender {
            by_tier(MENDER_HEAL, self.tier)
        } else {
            0.0
        }
    }

    /// How long an *Ablative Shell* lasts, in seconds.
    pub fn shell_seconds(self) -> f32 {
        if self.kind == ModuleKind::AblativeShell {
            by_tier(SHELL_SECONDS, self.tier)
        } else {
            0.0
        }
    }

    /// Whether it is a *Blink Drive*.
    pub fn is_blink(self) -> bool {
        self.kind == ModuleKind::BlinkDrive
    }

    /// A blink's reach, in tiles; nought for anything but a drive.
    pub fn blink_range(self) -> f32 {
        if self.is_blink() {
            by_tier(BLINK_RANGE_TILES, self.tier)
        } else {
            0.0
        }
    }

    /// A blink's cooldown, in seconds; nought for anything but a drive.
    pub fn blink_cooldown(self) -> f32 {
        if self.is_blink() {
            by_tier(BLINK_COOLDOWN_SECONDS, self.tier)
        } else {
            0.0
        }
    }

    /// Its crit — the chance and the multiple — for an *Executioner*.
    pub fn crit(self) -> Option<(f32, f32)> {
        (self.kind == ModuleKind::Executioner).then(|| {
            (
                by_tier(EXECUTIONER_CHANCE, self.tier),
                by_tier(EXECUTIONER_DAMAGE, self.tier),
            )
        })
    }

    /// The health it adds to the bar: a *Reactor Heart*'s or a
    /// *Pressure Seal*'s.
    pub fn health_bonus(self) -> f32 {
        match self.kind {
            ModuleKind::ReactorHeart => by_tier(HEART_HEALTH, self.tier),
            ModuleKind::PressureSeal => by_tier(PRESSURE_SEAL_HEALTH, self.tier),
            _ => 0.0,
        }
    }

    /// Its regeneration — hit points a second, and after a while unhurt
    /// — for a *Reactor Heart*.
    pub fn regen(self) -> Option<(f32, f32)> {
        (self.kind == ModuleKind::ReactorHeart).then(|| {
            (
                by_tier(HEART_REGEN, self.tier),
                by_tier(HEART_QUIET_REGEN, self.tier),
            )
        })
    }
}

impl Gear {
    /// The items carried, the empty slots left out.
    pub fn modules(&self) -> impl Iterator<Item = Module> + '_ {
        self.items.iter().flatten().copied()
    }

    /// Whether it carries an item of `kind`.
    pub fn carries(&self, kind: ModuleKind) -> bool {
        self.modules().any(|m| m.kind == kind)
    }

    /// The first empty item slot, nought to three.
    pub fn free_item_slot(&self) -> Option<usize> {
        self.items.iter().position(Option::is_none)
    }

    /// A whole bar for this body: [`MAX_HEALTH`] and every *Reactor
    /// Heart*'s and *Pressure Seal*'s health on top.
    pub fn max_health(&self) -> f32 {
        MAX_HEALTH + self.modules().map(Module::health_bonus).sum::<f32>()
    }

    /// The crit its items give, Dota's way with more than one: each rolls
    /// on its own, so the chance is that any of them comes up, and what
    /// it does is the biggest of their multiples. `None` with no
    /// *Executioner*.
    pub fn item_crit(&self) -> Option<(f32, f32)> {
        combine_crits(self.modules().filter_map(Module::crit))
    }

    /// The per cent its *Coolant Loop*s take off the class's cooldowns,
    /// summed, never past [`COOLDOWN_CUT_MOST`].
    pub fn item_cooldown_cut(&self) -> i32 {
        self.modules()
            .map(Module::cooldown_cut)
            .sum::<i32>()
            .min(COOLDOWN_CUT_MOST)
    }

    /// The per cent its *Steady Grip*s put on the fire rate, summed.
    pub fn item_fire_rate_percent(&self) -> i32 {
        self.modules().map(Module::fire_rate_percent).sum()
    }

    /// The tiles its *Long Barrel*s put on the weapon's range, summed.
    pub fn item_range_tiles(&self) -> f32 {
        self.modules().map(Module::range_tiles).sum()
    }

    /// The share its *Leech Capacitor*s give back, summed.
    pub fn item_leech(&self) -> f32 {
        self.modules().map(Module::leech).sum()
    }

    /// Its best *Arc Coil*'s arc.
    pub fn item_arc(&self) -> Option<(usize, f32)> {
        self.modules()
            .filter_map(Module::arc)
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The regeneration its items give, summed — hit points a second, and
    /// after a while unhurt.
    pub fn item_regen(&self) -> (f32, f32) {
        self.modules()
            .filter_map(Module::regen)
            .fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y))
    }
}

/// Crits that each roll on their own, as one: the chance any comes up,
/// and the biggest multiple among them. `None` for none.
pub fn combine_crits(crits: impl Iterator<Item = (f32, f32)>) -> Option<(f32, f32)> {
    // Either comes up: a + b - ab, folded, so one alone is its own chance
    // to the bit.
    let mut any: Option<(f32, f32)> = None;
    for (chance, multiple) in crits {
        let c = chance.clamp(0.0, 1.0);
        any = Some(match any {
            None => (c, multiple),
            Some((was, most)) => (was + c - was * c, most.max(multiple)),
        });
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heart_raises_the_bar_and_two_stack() {
        let mut gear = Gear::issued();
        assert_eq!(gear.max_health(), MAX_HEALTH);
        gear.items[0] = Some(ModuleKind::ReactorHeart.at(Tier::Two));
        assert_eq!(gear.max_health(), MAX_HEALTH + HEART_HEALTH[1]);
        gear.items[3] = Some(ModuleKind::ReactorHeart.at(Tier::One));
        assert_eq!(
            gear.max_health(),
            MAX_HEALTH + HEART_HEALTH[1] + HEART_HEALTH[0]
        );
        assert_eq!(gear.free_item_slot(), Some(1));
    }

    #[test]
    fn a_seal_raises_the_bar_and_mends_nothing() {
        let mut gear = Gear::issued();
        gear.items[0] = Some(ModuleKind::PressureSeal.at(Tier::Three));
        assert_eq!(gear.max_health(), MAX_HEALTH + PRESSURE_SEAL_HEALTH[2]);
        assert_eq!(gear.item_regen(), (0.0, 0.0));
    }

    #[test]
    fn two_crits_roll_apart_and_the_bigger_multiple_counts() {
        assert_eq!(combine_crits(std::iter::empty()), None);
        let (chance, damage) = combine_crits([(0.2, 2.25), (0.25, 1.6)].into_iter()).unwrap();
        assert!((chance - (1.0 - 0.8 * 0.75)).abs() < 1e-6);
        assert_eq!(damage, 2.25);
    }

    #[test]
    fn only_the_override_core_and_the_reset_capacitor_are_made_at_one_tier() {
        for kind in ModuleKind::ALL {
            assert_eq!(ModuleKind::from_code(kind.code()), Some(kind));
            let tiers = Tier::ALL.iter().filter(|&&t| kind.made_at(t)).count();
            let one = matches!(kind, ModuleKind::OverrideCore | ModuleKind::ResetCapacitor);
            assert_eq!(tiers, if one { 1 } else { 3 });
            assert!(kind.made_at(kind.min_tier()));
        }
        assert_eq!(ModuleKind::ResetCapacitor.min_tier(), Tier::Three);
    }
}

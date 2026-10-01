//! The **items** of a player's Bim, Dota 2's way (October 2026): four
//! slots on the loadout ([`ITEM_SLOTS`], `Gear::items`), each holding a
//! [`Module`] or nothing — bought at a trader, moved in the Armory like a
//! weapon, combined two of a tier into the next. A bot never carries one.
//!
//! The room reads what it must of them itself, off the loadout: the
//! health a *Reactor Heart* adds to the bar ([`Gear::max_health`]). The
//! rest is the world's — the crit, the regeneration, the blink and the
//! ultimate's extra rank — through the numbers here, so every number an
//! item has is in this file and nowhere else.
//!
//! The words — a name and a line on what each does — are the app's
//! (`names.rs`); what crosses is the kind's code and the tier's.

use crate::combat::{Gear, Tier};
use crate::health::MAX_HEALTH;

/// How many items a Bim carries: four, from the first day.
pub const ITEM_SLOTS: usize = 4;

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
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 4] = [
        ModuleKind::BlinkDrive,
        ModuleKind::Executioner,
        ModuleKind::ReactorHeart,
        ModuleKind::OverrideCore,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<ModuleKind> {
        ModuleKind::ALL.get(code as usize).copied()
    }

    /// Whether it is made at `tier`: every kind at every tier but the
    /// *Override Core*, which is one thing and made at tier one alone —
    /// never on a shelf at another, never combined.
    pub fn made_at(self, tier: Tier) -> bool {
        match self {
            ModuleKind::OverrideCore => tier == Tier::One,
            _ => true,
        }
    }

    /// Whether it has a tier worth saying: all but the *Override Core*.
    pub fn tiered(self) -> bool {
        self != ModuleKind::OverrideCore
    }

    /// Whether its key does something: the *Blink Drive*. The rest work
    /// on their own.
    pub fn active(self) -> bool {
        self == ModuleKind::BlinkDrive
    }

    /// The kind at `tier`.
    pub fn at(self, tier: Tier) -> Module {
        Module { kind: self, tier }
    }
}

/// One item: a kind at a tier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Module {
    pub kind: ModuleKind,
    pub tier: Tier,
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
/// Hit points a second it puts back while the Bim is on its feet, by tier.
pub const HEART_REGEN: [f32; 3] = [0.5, 1.0, 1.0];
/// And after [`HEART_QUIET_SECONDS`] without a hit, instead, by tier.
pub const HEART_QUIET_REGEN: [f32; 3] = [1.5, 3.0, 5.0];
/// How long without a hit before the quicker regeneration starts.
pub const HEART_QUIET_SECONDS: f32 = 6.0;

impl Module {
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

    /// The health it adds to the bar: a *Reactor Heart*'s.
    pub fn health_bonus(self) -> f32 {
        if self.kind == ModuleKind::ReactorHeart {
            by_tier(HEART_HEALTH, self.tier)
        } else {
            0.0
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
    /// Heart*'s health on top.
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
    fn two_crits_roll_apart_and_the_bigger_multiple_counts() {
        assert_eq!(combine_crits(std::iter::empty()), None);
        let (chance, damage) = combine_crits([(0.2, 2.25), (0.25, 1.6)].into_iter()).unwrap();
        assert!((chance - (1.0 - 0.8 * 0.75)).abs() < 1e-6);
        assert_eq!(damage, 2.25);
    }

    #[test]
    fn only_the_override_core_is_made_at_one_tier() {
        for kind in ModuleKind::ALL {
            assert_eq!(ModuleKind::from_code(kind.code()), Some(kind));
            let tiers = Tier::ALL.iter().filter(|&&t| kind.made_at(t)).count();
            assert_eq!(tiers, if kind.tiered() { 3 } else { 1 });
        }
    }
}

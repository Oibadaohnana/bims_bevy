//! The **items** (October 2026), Dota 2's: what a player's Bim carries in
//! its four item slots beside its weapon and its armour. What each one
//! does in a fight is `bims::module` (the numbers) and the world's step
//! (`item_use.rs`, a child of `crate::world`); this is the rules no room
//! needs — what one costs, which tier a trader sells at, and two of a
//! kind combined.
//!
//! # Where they come from
//!
//! A trader's **item shelf** ([`shop`]) has every kind at the tier the
//! day has reached — tier one until the scaling's tier-two day, tier two
//! until its tier-three day, tier three after ([`shop_tier`], the days
//! the machines' tiers run on, `scaling.ron`) — one of each kind a visit,
//! sold out until the next (`Trader::items_sold`). A tier above the day's is had by
//! **combining** two of a kind at one tier, at the trader, as a gun or a
//! piece is ([`crate::trader::combined`]). The *Override Core* is one
//! thing at one tier and combines into nothing.
//!
//! # Who carries one
//!
//! A player's own Bim, in any of its four slots, and nobody else: a bot is
//! refused one (`Refusal::BotsCarryNoItems`). Moved in the Armory as gear
//! is — between missions, or aboard on arriving — and kept through a
//! death like the rest of the loadout.

use bims::combat::Tier;
use bims::module::{Module, ModuleKind};
use economy::Money;

use crate::data;
use crate::droid::WaveScaling;

/// What the items keep through a mission, on the run (saved): when each
/// active item is ready again, and when each player's Bim was last hit —
/// what a *Blink Drive* and a *Reactor Heart* wait on. Cleared at every
/// mission's start, so every item is ready then.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItemClocks {
    /// `(who, item slot, mission minute)`: ready again at that minute.
    pub ready_at: Vec<(u32, u32, f64)>,
    /// The mission minute each player's Bim was last hit, by slot;
    /// `None` not this mission.
    pub hurt_at: Vec<Option<f64>>,
    /// Each player's weapon hits on machines this mission, by slot: what
    /// an *Arc Coil* counts to [`bims::module::ARC_EVERY`] by.
    #[cfg_attr(feature = "serde", serde(default))]
    pub arc_hits: Vec<u32>,
    /// `(who, mission minute)`: an *Ablative Shell* on until then.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shell_until: Vec<(u32, f64)>,
}

impl ItemClocks {
    /// A mission begun: every item ready, nobody hit yet.
    pub fn new_mission(&mut self) {
        self.ready_at.clear();
        self.hurt_at.clear();
        self.arc_hits.clear();
        self.shell_until.clear();
    }
}

/// The tier a trader's items — and its one gun and one piece — are sold
/// at on `day` of the run: one, then two from the scaling's tier-two day,
/// then three from its tier-three day. The machines reach a tier by a
/// share of them first; the trader all at once on the day.
pub fn shop_tier(scaling: &WaveScaling, day: u32) -> Tier {
    if day >= scaling.tier3_days {
        Tier::Three
    } else if day >= scaling.tier2_days {
        Tier::Two
    } else {
        Tier::One
    }
}

/// The trader's item shelf on a day at `tier`: every kind, at `tier` or
/// — for a kind not made there, the *Override Core* — at the tier it is
/// made at; a kind made only above `tier`, the *Reset Capacitor* before
/// tier three, is not on it.
pub fn shop(tier: Tier) -> Vec<Module> {
    ModuleKind::ALL
        .into_iter()
        .filter(|kind| kind.min_tier() <= tier)
        .map(|kind| {
            let at = if kind.made_at(tier) {
                tier
            } else {
                kind.min_tier()
            };
            kind.at(at)
        })
        .collect()
}

/// What an item costs at a trader before the dials, the licence and the
/// players' share: [`data::ITEM_PRICE`] by kind and tier.
pub fn price(item: Module) -> Money {
    let row = data::ITEM_PRICE[item.kind.code() as usize];
    row[(item.tier.code().clamp(1, 3) - 1) as usize]
}

/// Two items into one a tier up, if they are two of one kind at one tier
/// and the kind is made a tier up: `Ok(None)` for two that are not a
/// pair, `Err(())` for a pair at the top.
pub fn combined(a: Module, b: Module) -> Result<Option<Module>, ()> {
    if a != b {
        return Ok(None);
    }
    match a.tier.next() {
        Some(next) if a.kind.made_at(next) => Ok(Some(a.kind.at(next))),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shop_s_tier_is_the_day_s_by_the_scaling() {
        let scaling = WaveScaling::DEFAULT;
        assert_eq!(shop_tier(&scaling, 0), Tier::One);
        assert_eq!(shop_tier(&scaling, scaling.tier2_days - 1), Tier::One);
        assert_eq!(shop_tier(&scaling, scaling.tier2_days), Tier::Two);
        assert_eq!(shop_tier(&scaling, scaling.tier3_days), Tier::Three);
        let shelf = shop(Tier::Three);
        assert_eq!(shelf.len(), ModuleKind::ALL.len());
        for item in shelf {
            let want = if item.kind.tiered() {
                Tier::Three
            } else {
                Tier::One
            };
            assert_eq!(item.tier, want, "{item:?}");
            assert!(price(item) > 0);
        }
        // The Reset Capacitor is made at tier three alone: on no shelf
        // before it.
        for tier in [Tier::One, Tier::Two] {
            let shelf = shop(tier);
            assert_eq!(shelf.len(), ModuleKind::ALL.len() - 1);
            assert!(shelf.iter().all(|m| m.kind != ModuleKind::ResetCapacitor));
        }
    }

    #[test]
    fn two_of_a_kind_combine_a_tier_up_and_the_core_never() {
        let blink = |t| ModuleKind::BlinkDrive.at(t);
        assert_eq!(
            combined(blink(Tier::One), blink(Tier::One)),
            Ok(Some(blink(Tier::Two)))
        );
        assert_eq!(combined(blink(Tier::Three), blink(Tier::Three)), Err(()));
        assert_eq!(combined(blink(Tier::One), blink(Tier::Two)), Ok(None));
        let core = ModuleKind::OverrideCore.at(Tier::One);
        assert_eq!(combined(core, core), Err(()));
        let reset = ModuleKind::ResetCapacitor.at(Tier::Three);
        assert_eq!(combined(reset, reset), Err(()));
        // Each tier costs more than the one under it.
        for kind in ModuleKind::ALL
            .into_iter()
            .filter(|&k| Tier::ALL.into_iter().all(|t| k.made_at(t)))
        {
            assert!(price(kind.at(Tier::One)) < price(kind.at(Tier::Two)));
            assert!(price(kind.at(Tier::Two)) < price(kind.at(Tier::Three)));
        }
    }
}

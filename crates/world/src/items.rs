//! The **items** (October 2026), Dota 2's: what a player's Bim carries in
//! its six item slots beside its weapon and its armour. What each one
//! does in a fight is `bims::module` (the numbers) and the world's step
//! (`item_use.rs`, a child of `crate::world`); this is the rules no room
//! needs — what one costs, which tier a trader sells at, and the tier an
//! upgrade takes one to.
//!
//! # Where they come from
//!
//! A trader's **item shelf** ([`shop`]) has every kind at the lowest tier
//! it is made at — tier one, the *Reset Capacitor* tier three — whatever
//! the day or the floor's zone (October 2026, the player's word: a
//! tier-two or tier-three zone's trader sells as a tier-one zone's does)
//! — one of each kind a visit, sold out until the next
//! (`Trader::items_sold`). A kind the player's own
//! Bim already carries is offered as its **upgrade** instead ([`upgraded`]):
//! the next tier whatever the day, at the next tier's price, made in the
//! slot it is in — so every trader after the first buy sells it a tier
//! up, to tier three. The *Override Core* is one thing at one tier and
//! has no upgrade. Nothing is combined (October 2026), and an item is
//! never bought into the armory. Sold back at a trader, one fetches half
//! of what was paid for it (`Module::paid`, `World::sell_value`).
//!
//! # Who carries one
//!
//! A player's own Bim, in any of its six slots, and nobody else: a bot is
//! refused one (`Refusal::BotsCarryNoItems`). Moved in the Armory as gear
//! is — between missions, or aboard on arriving — and kept through a
//! death like the rest of the loadout.

use bims::module::{Module, ModuleKind};
use economy::Money;

use crate::data;

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
    /// Every *Smoke Launcher*'s cloud hanging (October 2026).
    #[cfg_attr(feature = "serde", serde(default))]
    pub smoke: Vec<Smoke>,
    /// Every *Tether Link* holding.
    #[cfg_attr(feature = "serde", serde(default))]
    pub tethers: Vec<Tether>,
    /// Every *Decoy Projector*'s ghost walking or standing.
    #[cfg_attr(feature = "serde", serde(default))]
    pub ghosts: Vec<Ghost>,
    /// `(who, mission minute)`: an *Adrenal Injector*'s rush on until then.
    #[cfg_attr(feature = "serde", serde(default))]
    pub adrenal_until: Vec<(u32, f64)>,
}

/// A *Smoke Launcher*'s cloud, in the crew's room's units.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Smoke {
    pub x: f32,
    pub y: f32,
    /// Its radius, in room units.
    pub radius: f32,
    /// The mission minutes it went up at and hangs until.
    pub from: f64,
    pub until: f64,
}

/// A *Tether Link*: who carries it, the crewmate linked, the share of the
/// crewmate's hits the carrier takes, and the mission minute it lets go.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tether {
    pub carrier: u32,
    pub mate: u32,
    pub share: f32,
    pub until: f64,
}

/// A *Decoy Projector*'s ghost: whose, where it is (the crew's room's
/// units), the walk still ahead of it, its pace in room units a second,
/// which way it faces and its stride, for the picture, and the mission
/// minutes it went up at and lasts until.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ghost {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub path: Vec<(f32, f32)>,
    pub pace: f32,
    pub heading: f32,
    pub stride: f32,
    pub from: f64,
    pub until: f64,
}

impl ItemClocks {
    /// A mission begun: every item ready, nobody hit yet, no cloud, link
    /// or ghost left over.
    pub fn new_mission(&mut self) {
        self.ready_at.clear();
        self.hurt_at.clear();
        self.arc_hits.clear();
        self.shell_until.clear();
        self.smoke.clear();
        self.tethers.clear();
        self.ghosts.clear();
        self.adrenal_until.clear();
    }
}

/// The trader's item shelf (October 2026): every kind at the lowest tier
/// it is made at — tier one, the *Reset Capacitor* (made at tier three
/// alone) at three — the same on every day and in every zone. A higher
/// tier is had only by upgrading one carried ([`upgraded`]).
pub fn shop() -> Vec<Module> {
    ModuleKind::ALL
        .into_iter()
        .map(|kind| kind.at(kind.min_tier()))
        .collect()
}

/// What an item costs at a trader before the dials and the relics:
/// [`data::ITEM_PRICE`] by kind and tier.
pub fn price(item: Module) -> Money {
    let row = data::ITEM_PRICE[item.kind.code() as usize];
    row[(item.tier.code().clamp(1, 3) - 1) as usize]
}

/// What an item line at a trader sells a player (October 2026,
/// `World::item_offer`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemOffer {
    /// A kind its own Bim carries none of, off today's shelf.
    Buy(Module),
    /// The one its own Bim carries in item slot `at` (nought to three),
    /// a tier up.
    Upgrade { at: u32, from: Module, to: Module },
    /// The one its own Bim carries, at its top already.
    Top(Module),
}

/// What an item held is upgraded to at a trader: its kind a tier up, what
/// was paid for it kept, if the kind is made there — `None` at the top,
/// and for the *Override Core*, made at one tier alone.
pub fn upgraded(item: Module) -> Option<Module> {
    let next = item.tier.next().filter(|&t| item.kind.made_at(t))?;
    Some(Module { tier: next, ..item })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::Tier;

    #[test]
    fn the_shop_sells_every_kind_at_its_lowest_tier() {
        let shelf = shop();
        assert_eq!(shelf.len(), ModuleKind::ALL.len());
        for item in shelf {
            let want = if item.kind == ModuleKind::ResetCapacitor {
                Tier::Three
            } else {
                Tier::One
            };
            assert_eq!(item.tier, want, "{item:?}");
            assert!(price(item) > 0);
        }
    }

    #[test]
    fn an_item_upgrades_a_tier_at_a_time_and_the_core_never() {
        let blink = |t| ModuleKind::BlinkDrive.at(t);
        let bought = Module {
            paid: 900,
            ..blink(Tier::One)
        };
        assert_eq!(
            upgraded(bought),
            Some(Module {
                paid: 900,
                ..blink(Tier::Two)
            })
        );
        assert_eq!(upgraded(blink(Tier::Two)), Some(blink(Tier::Three)));
        assert_eq!(upgraded(blink(Tier::Three)), None);
        assert_eq!(upgraded(ModuleKind::OverrideCore.at(Tier::One)), None);
        assert_eq!(upgraded(ModuleKind::ResetCapacitor.at(Tier::Three)), None);
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

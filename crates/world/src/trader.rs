//! The trader (task 114): the one place gear is bought and sold, visited
//! entirely on the world map.
//!
//! This is the rules, none of them the world's to walk: **which sites are
//! traders** and **what is on a trader's shelf**. `trading.rs` (a child of
//! `world`, like `mission.rs`) is where they meet the world's fields — the
//! visit, the purchases, an item's upgrade and a sale. Nothing is
//! combined since October 2026.
//!
//! # Which sites
//!
//! **Stateless, like the Manufacturers' and the crisis.** A **system** has
//! a trader or not — one in ten ([`rolled`], [`data::TRADER_SYSTEM_CHANCE`]
//! in a hundred off the galaxy's seed), or one of the [`near_sites`] the
//! start makes up so there are at least [`data::TRADER_NEAR_SITES`] within
//! [`data::TRADER_NEAR_HOPS`] lanes of home — and where it has one, its
//! trader is the lowest-numbered station that is [`eligible`] ([`pick`]):
//! a desk to trade across (never a derelict), not the crew's home, not the
//! Manufacturers', not one of the machines' derived stations, never a
//! town. Functions of the galaxy and the crew's own star, so two clients
//! agree about every site without a word, and a save carries none of it.
//! The galaxy chart marks every system with one (`World::trader_stars`).
//!
//! Never the jammer's station, so that a trader is never a site to clear:
//! the trader is picked **first**, and the machines' jammer stands on the
//! lowest-numbered station that is not it ([`jammer_candidate`]) — or on
//! one of their own where there is none, a system whose one station is
//! its trader among them. The crisis passes a trader by
//! (`World::spread_crisis`) and it is **closed** instead, while its system
//! is infested and not liberated.
//!
//! # The shelf
//!
//! [`data::TRADER_WEAPONS`] weapon and [`data::TRADER_ARMOUR`] piece — one
//! each since October 2026 — any kind made at the **tier the day has
//! reached** (`crate::items::shop_tier`, off the scaling's tier days:
//! never a minigun at tier one or a rail lance below three, task 115),
//! rolled off the galaxy's seed, the site, the player and the visit on a
//! stream of their own ([`roll_shelf`]) — every visit afresh. Beside it
//! the **items** (`crate::items::shop`): every kind at the day's tier,
//! one of each a visit.

use bims::combat::{ArmourKind, Tier};
use physics::ResourceId;
use worldgen::{Galaxy, StationBlueprint, StationKind};

use crate::run::Site;
use crate::{armour, data, heart, jammer};

/// Whether a station could be a trader at all: a desk to trade across,
/// and none of the stations a trader must never be. `home` and
/// `manufacturers` say whether it is the crew's home and whether it is the
/// Manufacturers'. The jammer is not asked: a system's trader is picked
/// first, and the jammer stands on another station ([`jammer_candidate`]).
pub fn eligible(station: &StationBlueprint, home: bool, manufacturers: bool) -> bool {
    station.kind != StationKind::Derelict
        && !jammer::is_derived(station.id)
        && !heart::is_heart(station.id)
        && !home
        && !manufacturers
}

/// The station the machines' jammer stands on in a system of `stations`
/// the day it falls: the lowest id of those not derived, not the
/// Manufacturers' and not the system's `trader` (`World::jammer_station`'s
/// own rule, through `World::jammer_site_among`), `None` where there is
/// none — and then the machines build one of their own. `manufacturers`
/// says whether a station is theirs.
pub fn jammer_candidate(
    stations: &[StationBlueprint],
    manufacturers: impl Fn(&StationBlueprint) -> bool,
    trader: Option<u32>,
) -> Option<u32> {
    stations
        .iter()
        .filter(|s| !jammer::is_derived(s.id) && !heart::is_heart(s.id))
        .filter(|s| !manufacturers(s))
        .filter(|s| Some(s.id) != trader)
        .map(|s| s.id)
        .min()
}

/// A number off the galaxy's seed for one site and one purpose, in a
/// hundred: the same on every machine, and off no stream a fight draws
/// from.
fn site_roll(galaxy_seed: u64, star: u32, station: u32, salt: u64) -> u32 {
    let seed = worldgen::rng::mix(galaxy_seed ^ salt)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station));
    worldgen::rng::Rng::new(seed).below(100)
}

/// Whether the galaxy's own roll gives a star's system a trader: odds of
/// [`data::TRADER_SYSTEM_CHANCE`] in a hundred, once a **system** — it was
/// once a station, and a system of six stations was six chances.
pub fn rolled(galaxy_seed: u64, star: u32) -> bool {
    site_roll(galaxy_seed, star, 0, 0x_5452_4144_4553_5953) < data::TRADER_SYSTEM_CHANCE
}

/// Which station of a system would be its trader: the lowest id of those
/// `eligible` says could be one, `None` where none could. A system has
/// one trader at most.
pub fn pick(
    stations: &[StationBlueprint],
    eligible: impl Fn(&StationBlueprint) -> bool,
) -> Option<u32> {
    stations.iter().filter(|s| eligible(s)).map(|s| s.id).min()
}

/// The traders made up on top of the roll, so a run has somewhere to buy
/// near home: where fewer than [`data::TRADER_NEAR_SITES`] systems within
/// [`data::TRADER_NEAR_HOPS`] lanes of the crew's own star — its own system
/// counted — were rolled one, that many more of the others there, picked in
/// an order off the galaxy's seed, each at its system's [`pick`].
/// `eligible` answers for a station of a star. `(star, station)` pairs,
/// sorted.
pub fn near_sites(
    galaxy: &Galaxy,
    home: u32,
    eligible: impl Fn(u32, &worldgen::StarSystem, &StationBlueprint) -> bool,
) -> Vec<(u32, u32)> {
    let hops = galaxy.hops_from(home);
    let mut rolled_near = 0usize;
    let mut rest: Vec<(u64, u32, u32)> = Vec::new();
    for (star, &h) in hops.iter().enumerate() {
        if h > data::TRADER_NEAR_HOPS {
            continue;
        }
        let star = star as u32;
        let Some(system) = galaxy.system(star) else {
            continue;
        };
        let Some(station) = pick(&system.stations, |s| eligible(star, &system, s)) else {
            continue;
        };
        if rolled(galaxy.seed, star) {
            rolled_near += 1;
        } else {
            let order = worldgen::rng::mix(
                galaxy.seed
                    ^ 0x_5452_4E45_4152
                    ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station)),
            );
            rest.push((order, star, station));
        }
    }
    let wanted = data::TRADER_NEAR_SITES.saturating_sub(rolled_near);
    rest.sort_unstable();
    let mut picked: Vec<(u32, u32)> = rest
        .into_iter()
        .take(wanted)
        .map(|(_, star, station)| (star, station))
        .collect();
    picked.sort_unstable();
    picked
}

/// Whether a system's [`pick`] is a trader: its star [`rolled`], or it is
/// one of the `near` sites [`near_sites`] made up.
pub fn holds(galaxy_seed: u64, near: &[(u32, u32)], star: u32, station: u32) -> bool {
    rolled(galaxy_seed, star) || near.binary_search(&(star, station)).is_ok()
}

/// One thing on a trader's shelf: a gun or a piece of armour — by its
/// resource, which names the kind — and its tier. A piece is made, and
/// numbered off the holdings, the moment it is bought.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShelfItem {
    pub resource: ResourceId,
    pub tier: Tier,
}

impl ShelfItem {
    /// Whether it is a weapon; a piece of armour otherwise.
    pub fn is_weapon(self) -> bool {
        armour::weapon_of(self.resource).is_some()
    }

    /// The weapon it is, if it is one.
    pub fn weapon(self) -> Option<bims::combat::Weapon> {
        armour::weapon_of(self.resource).map(|kind| kind.at(self.tier))
    }

    /// The kind of armour it is, if it is a piece.
    pub fn armour(self) -> Option<ArmourKind> {
        armour::kind_of(self.resource)
    }
}

/// A trader's shelf for player `owner` on a visit: [`data::TRADER_WEAPONS`]
/// weapons, then [`data::TRADER_ARMOUR`] pieces, each drawn from
/// [`shelf_candidates`] at `tier` — the tier the day has reached
/// (`crate::items::shop_tier`, October 2026; any tier before) — off the
/// galaxy's seed, the site, the player and `visit` (the world clock's
/// minute of the arrival) on a stream of the shelf's own: the same on
/// every machine, and another shelf every visit.
pub fn roll_shelf(
    galaxy_seed: u64,
    star: u32,
    station: u32,
    owner: u32,
    visit: u64,
    tier: Tier,
) -> Vec<ShelfItem> {
    let seed = worldgen::rng::mix(galaxy_seed ^ 0x_5348_454C_4600)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station))
        ^ worldgen::rng::mix(0x_4F57_4E45_5200 + u64::from(owner))
        ^ worldgen::rng::mix(visit);
    shelf_off(seed, tier)
}

/// A trader's shelf **rolled again** (task 118, *Restock Codes*): the
/// draws of [`roll_shelf`] off a seed that also mixes `again` — the world
/// clock's minute of the visit — so a restock is the same on every
/// machine and another visit's is another shelf.
pub fn reroll_shelf(
    galaxy_seed: u64,
    star: u32,
    station: u32,
    again: u64,
    tier: Tier,
) -> Vec<ShelfItem> {
    let seed = worldgen::rng::mix(galaxy_seed ^ 0x_5245_5354_4F43)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station))
        ^ worldgen::rng::mix(again);
    shelf_off(seed, tier)
}

/// The shelf's draws off one seed, at `tier`.
fn shelf_off(seed: u64, tier: Tier) -> Vec<ShelfItem> {
    let mut rng = worldgen::rng::Rng::new(seed);
    let mut shelf = Vec::with_capacity(data::TRADER_WEAPONS + data::TRADER_ARMOUR);
    // One draw a thing, over every kind the list makes at the tier: a
    // minigun at tier one or a lance below three is never a candidate at
    // all (task 115), rather than drawn and moved up.
    let mut draw = |list: &[ResourceId], n: usize, rng: &mut worldgen::rng::Rng| {
        let candidates = shelf_candidates(list, Some(tier));
        if candidates.is_empty() {
            return;
        }
        for _ in 0..n {
            shelf.push(candidates[rng.below(candidates.len() as u32) as usize]);
        }
    };
    draw(&worldgen::data::WEAPONS, data::TRADER_WEAPONS, &mut rng);
    draw(&worldgen::data::ARMOUR, data::TRADER_ARMOUR, &mut rng);
    shelf
}

/// Every thing of `list` a shelf may hold: each kind at each tier it is
/// made at ([`bims::combat::WeaponKind::made_at`] for a gun; the armour is
/// made at every tier) — at `tier` alone where one is given — the kinds
/// in the list's order and the tiers upward.
pub fn shelf_candidates(list: &[ResourceId], tier: Option<Tier>) -> Vec<ShelfItem> {
    list.iter()
        .flat_map(|&resource| {
            Tier::ALL
                .into_iter()
                .filter(move |&t| tier.is_none_or(|want| want == t))
                .filter(move |&t| armour::weapon_of(resource).is_none_or(|kind| kind.made_at(t)))
                .map(move |t| ShelfItem { resource, tier: t })
        })
        .collect()
}

/// What the world keeps about one trader the crew have been to: what is
/// left on its shelf — a slot a thing, `None` once bought, so a slot's
/// number is the same on every visit. Saved, and in
/// `world_checksum`. **Every player has a trader of their own** at a
/// trader's site — its own shelf, bought from with their own money — so one player's buying never takes from another's: `owner`
/// is whose.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trader {
    pub site: Site,
    /// The player this one sells to.
    #[cfg_attr(feature = "serde", serde(default))]
    pub owner: u32,
    pub shelf: Vec<Option<ShelfItem>>,
    /// The items bought off the item shelf this visit, by kind's code
    /// (October 2026): one of a kind a visit, sold out until the next.
    #[cfg_attr(feature = "serde", serde(default))]
    pub items_sold: Vec<u32>,
}

impl Trader {
    /// Player `owner`'s trader met for the first time: its shelf rolled
    /// for the visit at `tier` ([`roll_shelf`]).
    pub fn new(galaxy_seed: u64, site: Site, owner: u32, visit: u64, tier: Tier) -> Trader {
        let mut trader = Trader {
            site,
            owner,
            shelf: Vec::new(),
            items_sold: Vec::new(),
        };
        trader.restock(galaxy_seed, visit, tier);
        trader
    }

    /// The shelf rolled afresh for a visit (October 2026): one weapon and
    /// one piece at the day's tier, whatever was bought the visit before,
    /// and every item on sale again.
    pub fn restock(&mut self, galaxy_seed: u64, visit: u64, tier: Tier) {
        self.items_sold.clear();
        self.shelf = roll_shelf(
            galaxy_seed,
            self.site.star,
            self.site.station,
            self.owner,
            visit,
            tier,
        )
        .into_iter()
        .map(Some)
        .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::WeaponKind;

    #[test]
    fn a_shelf_is_the_seed_s_and_holds_what_it_should() {
        let roll =
            |star, station, owner, visit, tier| roll_shelf(7, star, station, owner, visit, tier);
        let a = roll(3, 2, 0, 0, Tier::One);
        assert_eq!(a, roll(3, 2, 0, 0, Tier::One));
        // Another site, another player or another visit: another shelf,
        // over enough of them that one alike by chance does not decide it.
        assert!((0..20).any(|n| roll(3, 2 + n, 0, 0, Tier::One) != a));
        assert!((0..20).any(|n| roll(3, 2, 1 + n, 0, Tier::One) != a));
        assert!((0..20).any(|n| roll(3, 2, 0, 1 + n, Tier::One) != a));
        // One gun and one piece (October 2026), at the tier asked.
        assert_eq!(a.len(), data::TRADER_WEAPONS + data::TRADER_ARMOUR);
        assert_eq!((data::TRADER_WEAPONS, data::TRADER_ARMOUR), (1, 1));
        assert!(a[..data::TRADER_WEAPONS].iter().all(|i| i.is_weapon()));
        assert!(
            a[data::TRADER_WEAPONS..]
                .iter()
                .all(|i| i.armour().is_some())
        );
        // Over many shelves every kind turns up, and only at the tier.
        for tier in Tier::ALL {
            let mut kinds = std::collections::BTreeSet::new();
            for station in 0..400 {
                for item in roll_shelf(11, 1, station, 0, 0, tier) {
                    assert_eq!(item.tier, tier, "{item:?}");
                    kinds.insert(item.resource as u32);
                }
            }
            assert_eq!(
                kinds.len(),
                shelf_candidates(&worldgen::data::WEAPONS, Some(tier)).len() + 1
            );
        }
    }

    /// Task 115: no shelf over many seeds holds a minigun at tier one or a
    /// rail lance below three, and both do turn up.
    #[test]
    fn no_shelf_holds_a_weapon_below_its_lowest_tier_and_both_new_kinds_turn_up() {
        let (mut miniguns, mut lances) = (0, 0);
        for seed in 0..40u64 {
            for station in 0..50 {
                let tier = Tier::ALL[(station % 3) as usize];
                for item in roll_shelf(seed, (seed % 7) as u32, station, 0, 0, tier) {
                    let Some(w) = item.weapon() else {
                        continue;
                    };
                    assert!(
                        w.kind.made_at(w.tier),
                        "seed {seed} station {station}: {w:?}"
                    );
                    match w.kind {
                        WeaponKind::Minigun => miniguns += 1,
                        WeaponKind::RailLance => lances += 1,
                        _ => {}
                    }
                }
            }
        }
        assert!(
            miniguns > 20 && lances > 20,
            "{miniguns} miniguns, {lances} lances"
        );
        // The candidates: seven kinds, the minigun at two tiers and the
        // lance at one, so 5 x 3 + 2 + 1; and the one armour at every tier.
        assert_eq!(shelf_candidates(&worldgen::data::WEAPONS, None).len(), 18);
        assert_eq!(shelf_candidates(&worldgen::data::ARMOUR, None).len(), 3);
        assert_eq!(
            shelf_candidates(&worldgen::data::WEAPONS, Some(Tier::One)).len(),
            5
        );
        assert_eq!(
            shelf_candidates(&worldgen::data::WEAPONS, Some(Tier::Three)).len(),
            7
        );
    }
}

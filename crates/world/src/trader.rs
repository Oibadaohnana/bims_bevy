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
//! [`data::TRADER_NEAR_HOPS`] lanes of home and, from every system,
//! another system's trader within [`data::TRADER_EVERY_HOPS`] ([`cover`],
//! October 2026) — and where it has one, its
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
//! **Every** gun but the laser pistol ([`data::TRADER_WEAPONS`]) and the
//! armour ([`data::TRADER_ARMOUR`]), always (October 2026), each kind at
//! its own tier ([`shelf`], `World::shelf_tier`): **tier one** until the
//! player buys that kind, then one past the best of it bought — the day
//! lifts nothing (October 2026) — and never below
//! the lowest its kind is made at (a minigun at two, a rail lance at
//! three, task 115) — put up afresh every visit. Beside it
//! the **items** (`crate::items::shop`): every kind at the day's tier,
//! one of each a visit.

use bims::combat::{ArmourKind, Tier, WeaponKind};
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

/// The traders made up on top of the roll: so a run has somewhere to buy
/// near home — where fewer than [`data::TRADER_NEAR_SITES`] systems within
/// [`data::TRADER_NEAR_HOPS`] lanes of the crew's own star, its own system
/// counted, were rolled one, that many more of the others there, picked in
/// an order off the galaxy's seed — and so there is **a trader every
/// [`data::TRADER_EVERY_HOPS`] hops** ([`cover`]). Each at its system's
/// [`pick`]; `eligible` answers for a station of a star. `(star, station)`
/// pairs, sorted.
pub fn near_sites(
    galaxy: &Galaxy,
    home: u32,
    eligible: impl Fn(u32, &worldgen::StarSystem, &StationBlueprint) -> bool,
) -> Vec<(u32, u32)> {
    // Every system's would-be trader, generated once.
    let picks: Vec<Option<u32>> = (0..galaxy.stars.len() as u32)
        .map(|star| {
            let system = galaxy.system(star)?;
            pick(&system.stations, |s| eligible(star, &system, s))
        })
        .collect();
    let hops = galaxy.hops_from(home);
    let mut rolled_near = 0usize;
    let mut rest: Vec<(u64, u32, u32)> = Vec::new();
    for (star, &h) in hops.iter().enumerate() {
        if h > data::TRADER_NEAR_HOPS {
            continue;
        }
        let Some(station) = picks[star] else {
            continue;
        };
        let star = star as u32;
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
    let mut traders: Vec<bool> = (0..picks.len())
        .map(|star| picks[star].is_some() && rolled(galaxy.seed, star as u32))
        .collect();
    for &(star, _) in &picked {
        traders[star as usize] = true;
    }
    for star in cover(galaxy, &picks, &mut traders, data::TRADER_EVERY_HOPS) {
        if let Some(station) = picks[star as usize] {
            picked.push((star, station));
        }
    }
    picked.sort_unstable();
    picked
}

/// The stars made traders so that from every star — a trader's own
/// included — **another** star's trader is at most `reach` lanes away:
/// leaving a trader, the next is always within `reach`. `picks` is each
/// star's would-be trader (`None` where it can have none), `traders` which
/// stars have one already, and is marked as the made-up ones are added.
///
/// Greedy and deterministic: while some star is short of one, the star
/// that can take a trader and would serve the most stars short of one is
/// made one — a tie to the earlier in an order off the galaxy's seed, so
/// they do not bunch at the low ids — until every star is served, or none
/// that could take one would serve anybody more (a star hemmed in by
/// systems that can have none, which is left without). The stars added,
/// in the order they were.
pub fn cover(galaxy: &Galaxy, picks: &[Option<u32>], traders: &mut [bool], reach: u16) -> Vec<u32> {
    // Each star's ball of `reach`, itself included; a ball is symmetric,
    // so a star's ball is also every star it would serve.
    let balls: Vec<Vec<u32>> = (0..picks.len() as u32)
        .map(|star| within(galaxy, star, reach))
        .collect();
    let order = |star: u32| {
        worldgen::rng::mix(galaxy.seed ^ 0x_5452_4556_4552 ^ worldgen::rng::mix(u64::from(star)))
    };
    let mut added = Vec::new();
    loop {
        let short: Vec<bool> = balls
            .iter()
            .enumerate()
            .map(|(star, ball)| {
                !ball
                    .iter()
                    .any(|&t| t as usize != star && traders[t as usize])
            })
            .collect();
        if !short.contains(&true) {
            break;
        }
        let best = (0..picks.len())
            .filter(|&star| picks[star].is_some() && !traders[star])
            .map(|star| {
                let serves = balls[star]
                    .iter()
                    .filter(|&&s| s as usize != star && short[s as usize])
                    .count();
                (serves, std::cmp::Reverse(order(star as u32)), star)
            })
            .max();
        match best {
            Some((serves, _, star)) if serves > 0 => {
                traders[star] = true;
                added.push(star as u32);
            }
            _ => break,
        }
    }
    added
}

/// Every star within `reach` lanes of `from`, itself included, in the
/// order a breadth-first walk meets them.
fn within(galaxy: &Galaxy, from: u32, reach: u16) -> Vec<u32> {
    let mut hops = vec![u16::MAX; galaxy.stars.len()];
    let Some(start) = hops.get_mut(from as usize) else {
        return Vec::new();
    };
    *start = 0;
    let mut seen = vec![from];
    let mut at = 0;
    while let Some(&star) = seen.get(at) {
        at += 1;
        let next_hops = hops[star as usize] + 1;
        if next_hops > reach {
            continue;
        }
        for &next in galaxy.lanes(star) {
            if hops[next as usize] == u16::MAX {
                hops[next as usize] = next_hops;
                seen.push(next);
            }
        }
    }
    seen
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

/// A trader's shelf for one player (October 2026): **every** gun a shelf
/// sells ([`shelf_kinds`] of [`worldgen::data::WEAPONS`]: never the laser
/// pistol), then every piece of armour, in the lists' order — each at the
/// tier `tier_of` gives its kind (`World::shelf_tier`), put up to the
/// lowest tier the kind is made at where that is higher: a minigun is
/// always on the shelf, at tier two until one is bought.
pub fn shelf(tier_of: impl Fn(ResourceId) -> Tier) -> Vec<ShelfItem> {
    shelf_kinds(&worldgen::data::WEAPONS)
        .chain(shelf_kinds(&worldgen::data::ARMOUR))
        .map(|resource| {
            let mut tier = tier_of(resource);
            while armour::weapon_of(resource).is_some_and(|kind| !kind.made_at(tier)) {
                match tier.next() {
                    Some(up) => tier = up,
                    None => break,
                }
            }
            ShelfItem { resource, tier }
        })
        .collect()
}

/// The kinds of `list` a shelf sells: all but the laser pistol (October
/// 2026) — every Bim sets out with one, and none is bought or sold.
pub fn shelf_kinds(list: &[ResourceId]) -> impl Iterator<Item = ResourceId> + '_ {
    list.iter()
        .copied()
        .filter(|&resource| armour::weapon_of(resource) != Some(WeaponKind::LaserPistol))
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
    /// Player `owner`'s trader met for the first time, its shelf `shelf`
    /// ([`shelf`]).
    pub fn new(site: Site, owner: u32, shelf: Vec<ShelfItem>) -> Trader {
        let mut trader = Trader {
            site,
            owner,
            shelf: Vec::new(),
            items_sold: Vec::new(),
        };
        trader.restock(shelf);
        trader
    }

    /// The shelf put up afresh for a visit (October 2026): every kind
    /// again at `shelf`'s tiers, whatever was bought the visit before, and
    /// every item on sale again.
    pub fn restock(&mut self, shelf: Vec<ShelfItem>) {
        self.items_sold.clear();
        self.shelf = shelf.into_iter().map(Some).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every gun but the pistol, then the armour, each at the tier asked
    /// or the lowest its kind is made at; the slots line up with
    /// [`data::TRADER_WEAPONS`] and [`data::TRADER_ARMOUR`].
    #[test]
    fn a_shelf_holds_every_gun_and_the_armour_at_their_tiers() {
        for day in Tier::ALL {
            let a = shelf(|_| day);
            assert_eq!(a.len(), data::TRADER_WEAPONS + data::TRADER_ARMOUR);
            assert_eq!((data::TRADER_WEAPONS, data::TRADER_ARMOUR), (6, 1));
            assert!(a[..data::TRADER_WEAPONS].iter().all(|i| i.is_weapon()));
            assert!(
                a[data::TRADER_WEAPONS..]
                    .iter()
                    .all(|i| i.armour().is_some())
            );
            let kinds: Vec<_> = a.iter().filter_map(|i| i.weapon()).collect();
            for kind in WeaponKind::ALL {
                let on = kinds.iter().filter(|w| w.kind == kind).count();
                assert_eq!(on, usize::from(kind != WeaponKind::LaserPistol), "{kind:?}");
            }
            for item in &a {
                let lowest = item.weapon().map_or(Tier::One, |w| w.kind.min_tier());
                assert_eq!(item.tier, day.max(lowest), "{item:?}");
            }
        }
        // One kind a tier up leaves the others where they were.
        let up = shelf(|r| {
            if r == ResourceId::Shotgun {
                Tier::Two
            } else {
                Tier::One
            }
        });
        for item in up {
            let want = match item.resource {
                ResourceId::Shotgun | ResourceId::Minigun => Tier::Two,
                ResourceId::RailLance => Tier::Three,
                _ => Tier::One,
            };
            assert_eq!(item.tier, want, "{item:?}");
        }
    }
}

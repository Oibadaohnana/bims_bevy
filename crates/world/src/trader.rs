//! The trader (task 114): the one place gear is bought, visited entirely on
//! the world map.
//!
//! This is the rules, none of them the world's to walk: **which sites are
//! traders**, **what is on a trader's shelf**, and **what two things
//! combine into**. `trading.rs` (a child of `world`, like `mission.rs`) is
//! where they meet the world's fields — the visit, the purchases, the
//! relic's vote and the combining.
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
//! [`data::TRADER_WEAPONS`] weapons and [`data::TRADER_ARMOUR`] pieces, any
//! kind at any tier it is made at ([`shelf_candidates`]: never a minigun
//! at tier one or a rail lance below three, task 115, nor arc greaves at
//! tier one or a Reflective plate below three, task 116), rolled off the galaxy's seed and the site on a stream
//! of their own ([`roll_shelf`]) — once a trader a run, and never again:
//! the world keeps what is left of it ([`Trader`]).

use bims::combat::{ArmourKind, Item, Tier};
use physics::ResourceId;
use worldgen::{Galaxy, StationBlueprint, StationKind};

use crate::relic::Relic;
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

/// A trader's shelf: [`data::TRADER_WEAPONS`] weapons, then
/// [`data::TRADER_ARMOUR`] pieces, each drawn from [`shelf_candidates`] —
/// any kind of its list at any tier it is made at — off the galaxy's seed and the site on a stream of the shelf's own. The
/// same every time it is asked, so it is rolled once a run by being kept
/// the first time the crew arrive ([`Trader::new`]).
pub fn roll_shelf(galaxy_seed: u64, star: u32, station: u32) -> Vec<ShelfItem> {
    let seed = worldgen::rng::mix(galaxy_seed ^ 0x_5348_454C_4600)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station));
    shelf_off(seed)
}

/// Player `owner`'s shelf at a trader: the first player's is
/// [`roll_shelf`]'s, every other's a roll of its own off a seed that also
/// mixes the slot.
pub fn roll_shelf_for(galaxy_seed: u64, star: u32, station: u32, owner: u32) -> Vec<ShelfItem> {
    if owner == 0 {
        return roll_shelf(galaxy_seed, star, station);
    }
    let seed = worldgen::rng::mix(galaxy_seed ^ 0x_5348_454C_4600)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station))
        ^ worldgen::rng::mix(0x_4F57_4E45_5200 + u64::from(owner));
    shelf_off(seed)
}

/// A trader's shelf **rolled again** (task 118, *Restock Codes*): the
/// draws of [`roll_shelf`] off a seed that also mixes `again` — the world
/// clock's minute of the visit — so a restock is the same on every
/// machine and another visit's is another shelf.
pub fn reroll_shelf(galaxy_seed: u64, star: u32, station: u32, again: u64) -> Vec<ShelfItem> {
    let seed = worldgen::rng::mix(galaxy_seed ^ 0x_5245_5354_4F43)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station))
        ^ worldgen::rng::mix(again);
    shelf_off(seed)
}

/// The shelf's draws off one seed.
fn shelf_off(seed: u64) -> Vec<ShelfItem> {
    let mut rng = worldgen::rng::Rng::new(seed);
    let mut shelf = Vec::with_capacity(data::TRADER_WEAPONS + data::TRADER_ARMOUR);
    // One draw a thing, over every pair of a kind and a tier the kind is
    // made at: a minigun at tier one or a lance below three is never a
    // candidate at all (task 115), rather than drawn and moved up.
    let mut draw = |list: &[ResourceId], n: usize, rng: &mut worldgen::rng::Rng| {
        let candidates = shelf_candidates(list);
        for _ in 0..n {
            shelf.push(candidates[rng.below(candidates.len() as u32) as usize]);
        }
    };
    draw(&worldgen::data::WEAPONS, data::TRADER_WEAPONS, &mut rng);
    draw(&worldgen::data::ARMOUR, data::TRADER_ARMOUR, &mut rng);
    shelf
}

/// Every thing of `list` a shelf may hold: each kind at each tier it is
/// made at ([`bims::combat::WeaponKind::made_at`] for a gun,
/// [`ArmourKind::made_at`] for a piece — never arc greaves at tier one or
/// a Reflective plate below three, task 116), the kinds in the list's
/// order and the tiers upward.
pub fn shelf_candidates(list: &[ResourceId]) -> Vec<ShelfItem> {
    list.iter()
        .flat_map(|&resource| {
            Tier::ALL
                .into_iter()
                .filter(move |&tier| {
                    armour::weapon_of(resource).is_none_or(|kind| kind.made_at(tier))
                        && armour::kind_of(resource).is_none_or(|kind| kind.made_at(tier))
                })
                .map(move |tier| ShelfItem { resource, tier })
        })
        .collect()
}

/// What the world keeps about one trader the crew have been to: what is
/// left on its shelf — a slot a thing, `None` once bought, so a slot's
/// number is the same on every visit — and its relic, drawn the first time
/// the crew arrived and there until bought. Saved, and in
/// `world_checksum`. **Every player has a trader of their own** at a
/// trader's site — its own shelf and its own relic, bought with their own
/// money — so one player's buying never takes from another's: `owner`
/// is whose.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trader {
    pub site: Site,
    /// The player this one sells to.
    #[cfg_attr(feature = "serde", serde(default))]
    pub owner: u32,
    pub shelf: Vec<Option<ShelfItem>>,
    /// The relic, until it is bought: drawn by the day's odds the first
    /// time the crew arrived (task 117). It stays in the run's pool and out
    /// of every other draw while it is here, leaves the pool when bought,
    /// and goes back in the running — off this table — the first draw after
    /// the trader closes.
    pub relic: Option<Relic>,
}

impl Trader {
    /// Player `owner`'s trader met for the first time: its shelf rolled
    /// (a roll of the player's own, [`roll_shelf_for`]), its relic the one
    /// drawn for it.
    pub fn new(galaxy_seed: u64, site: Site, relic: Option<Relic>, owner: u32) -> Trader {
        Trader {
            site,
            owner,
            shelf: roll_shelf_for(galaxy_seed, site.star, site.station, owner)
                .into_iter()
                .map(Some)
                .collect(),
            relic,
        }
    }
}

/// What a relic costs at a trader: [`data::RELIC_PRICE`] by the relic's
/// own tier.
pub fn relic_price(relic: Relic) -> economy::Money {
    let at = (relic.tier().max(1) - 1) as usize;
    data::RELIC_PRICE
        .get(at)
        .copied()
        .unwrap_or(data::RELIC_PRICE[data::RELIC_PRICE.len() - 1])
}

/// Why two things will not combine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CombineError {
    /// Not two of one kind at one tier: two weapons of different kinds, a
    /// weapon and a piece, two pieces for different parts, or two tiers.
    NotAPair,
    /// Two at tier three: there is no tier past it.
    TopTier,
}

/// What two things combine into (the workbench's upgrade, at a trader now):
/// two weapons of one kind at one tier, or two pieces of one kind at one
/// tier, make one of that kind a tier up — a piece whole, numbered `id`.
/// Tier three combines into nothing.
pub fn combined(a: Item, b: Item, id: u32) -> Result<Item, CombineError> {
    match (a, b) {
        (Item::Weapon(x), Item::Weapon(y)) if x.kind == y.kind && x.tier == y.tier => {
            let next = next_tier(x.tier).ok_or(CombineError::TopTier)?;
            Ok(Item::Weapon(x.kind.at(next)))
        }
        (Item::Armour(x), Item::Armour(y)) if x.kind == y.kind && x.tier == y.tier => {
            let next = next_tier(x.tier).ok_or(CombineError::TopTier)?;
            Ok(Item::Armour(bims::combat::Piece::new(id, x.kind, next)))
        }
        _ => Err(CombineError::NotAPair),
    }
}

/// The tier after `tier`, and none after three.
pub fn next_tier(tier: Tier) -> Option<Tier> {
    Tier::from_code(tier.code() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::WeaponKind;

    #[test]
    fn a_shelf_is_the_seed_s_and_holds_what_it_should() {
        let a = roll_shelf(7, 3, 2);
        assert_eq!(a, roll_shelf(7, 3, 2));
        assert_ne!(a, roll_shelf(7, 3, 5), "another site, another shelf");
        assert_eq!(a.len(), data::TRADER_WEAPONS + data::TRADER_ARMOUR);
        assert!(a[..data::TRADER_WEAPONS].iter().all(|i| i.is_weapon()));
        assert!(
            a[data::TRADER_WEAPONS..]
                .iter()
                .all(|i| i.armour().is_some())
        );
        // Over many shelves every kind and every tier turns up.
        let mut kinds = std::collections::BTreeSet::new();
        let mut tiers = std::collections::BTreeSet::new();
        for station in 0..200 {
            for item in roll_shelf(11, 1, station) {
                kinds.insert(item.resource as u32);
                tiers.insert(item.tier.code());
            }
        }
        assert_eq!(kinds.len(), 12);
        assert_eq!(tiers.len(), 3);
    }

    /// Task 116: no shelf over many seeds holds arc greaves at tier one or
    /// a Reflective plate below three, and both do turn up.
    #[test]
    fn no_shelf_holds_a_piece_below_its_lowest_tier_and_both_new_kinds_turn_up() {
        let (mut greaves, mut plates) = (0, 0);
        for seed in 0..40u64 {
            for station in 0..50 {
                for item in roll_shelf(seed, (seed % 7) as u32, station) {
                    let Some(kind) = item.armour() else {
                        continue;
                    };
                    assert!(
                        kind.made_at(item.tier),
                        "seed {seed} station {station}: {kind:?} at {:?}",
                        item.tier
                    );
                    match kind {
                        ArmourKind::ArcGreaves => greaves += 1,
                        ArmourKind::ReflectivePlate => plates += 1,
                        _ => {}
                    }
                }
            }
        }
        assert!(
            greaves > 50 && plates > 50,
            "{greaves} greaves, {plates} plates"
        );
    }

    /// Task 116: two tier-two pairs of arc greaves make a tier-three pair,
    /// and a Reflective plate — tier three and nothing else — combines into
    /// nothing.
    #[test]
    fn two_arc_greaves_combine_and_a_plate_does_not() {
        use bims::combat::Piece;
        let greaves = |id, t| Item::Armour(Piece::new(id, ArmourKind::ArcGreaves, t));
        assert_eq!(
            combined(greaves(1, Tier::Two), greaves(2, Tier::Two), 9),
            Ok(greaves(9, Tier::Three))
        );
        assert_eq!(
            combined(greaves(1, Tier::Three), greaves(2, Tier::Three), 9),
            Err(CombineError::TopTier)
        );
        let plate = |id| Item::Armour(Piece::new(id, ArmourKind::ReflectivePlate, Tier::Three));
        assert_eq!(combined(plate(1), plate(2), 9), Err(CombineError::TopTier));
        // Nor with a kevlar of its tier: the two are other kinds.
        let kevlar = Item::Armour(Piece::new(3, ArmourKind::BasicKevlar, Tier::Three));
        assert_eq!(combined(plate(1), kevlar, 9), Err(CombineError::NotAPair));
    }

    /// Task 115: no shelf over many seeds holds a minigun at tier one or a
    /// rail lance below three, and both do turn up.
    #[test]
    fn no_shelf_holds_a_weapon_below_its_lowest_tier_and_both_new_kinds_turn_up() {
        let (mut miniguns, mut lances) = (0, 0);
        for seed in 0..40u64 {
            for station in 0..50 {
                for item in roll_shelf(seed, (seed % 7) as u32, station) {
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
            miniguns > 50 && lances > 50,
            "{miniguns} miniguns, {lances} lances"
        );
        // The candidates: seven kinds, the minigun at two tiers and the
        // lance at one, so 5 x 3 + 2 + 1; and five pieces, the arc greaves
        // at two tiers and the plate at one (task 116), so 3 x 3 + 2 + 1.
        assert_eq!(shelf_candidates(&worldgen::data::WEAPONS).len(), 18);
        assert_eq!(shelf_candidates(&worldgen::data::ARMOUR).len(), 12);
    }

    /// Task 115: two tier-two miniguns make a tier-three one, and a lance
    /// — tier three and nothing else — combines into nothing.
    #[test]
    fn two_miniguns_combine_and_a_lance_does_not() {
        let mini = |t| Item::Weapon(WeaponKind::Minigun.at(t));
        assert_eq!(
            combined(mini(Tier::Two), mini(Tier::Two), 9),
            Ok(mini(Tier::Three))
        );
        let lance = Item::Weapon(WeaponKind::RailLance.basic());
        assert_eq!(combined(lance, lance, 9), Err(CombineError::TopTier));
    }

    #[test]
    fn two_of_a_kind_combine_a_tier_up_and_three_does_not() {
        let rifle = |t| Item::Weapon(WeaponKind::AutoRifle.at(t));
        assert_eq!(
            combined(rifle(Tier::One), rifle(Tier::One), 9),
            Ok(rifle(Tier::Two))
        );
        assert_eq!(
            combined(rifle(Tier::Three), rifle(Tier::Three), 9),
            Err(CombineError::TopTier)
        );
        assert_eq!(
            combined(rifle(Tier::One), rifle(Tier::Two), 9),
            Err(CombineError::NotAPair)
        );
        let helm = |id, t| Item::Armour(bims::combat::Piece::new(id, ArmourKind::BasicHelm, t));
        let Ok(Item::Armour(made)) = combined(helm(1, Tier::Two), helm(2, Tier::Two), 9) else {
            panic!("two helms make a helm");
        };
        assert_eq!(
            (made.id, made.kind, made.tier),
            (9, ArmourKind::BasicHelm, Tier::Three)
        );
        assert_eq!(
            combined(helm(1, Tier::One), rifle(Tier::One), 9),
            Err(CombineError::NotAPair)
        );
    }

    #[test]
    fn a_relic_costs_its_tier_s_price() {
        for relic in Relic::ALL {
            assert_eq!(
                relic_price(relic),
                data::RELIC_PRICE[(relic.tier() - 1) as usize]
            );
        }
    }
}

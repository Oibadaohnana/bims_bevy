//! The Manufacturers (feature 109): the human faction that built the
//! machines and defends what it made — the crew's enemy from the first day
//! of a run, where the machines are months away.
//!
//! This is the rules, none of them the world's to walk: **which sites are
//! theirs**, **who stands in a garrison of theirs** and **what each of them
//! carries**. `garrison.rs` (a child of `world`, like `mission.rs`) is where
//! they meet the world's fields — the site's [`crate::droid::Infestation`],
//! which a site of theirs has the way a held station has one, and the
//! residents' room they are laid in.
//!
//! # Which sites
//!
//! **Stateless, like the crisis.** A site is theirs when it is an orbital
//! station ([`eligible`]: not a town, never one of the machines' derived
//! stations) outside the crew's own system and either rolled
//! ([`rolled`], [`data::MANUFACTURER_SITE_CHANCE`] in a hundred off the
//! galaxy's seed) or one of the [`near_sites`] the start makes up so there
//! are at least [`data::MANUFACTURER_NEAR_SITES`] within
//! [`data::MANUFACTURER_NEAR_HOPS`] lanes of home. Both are functions of
//! the galaxy and the crew's own star, so two clients agree about every
//! site without a word, and a save carries none of it.
//!
//! # Who, and with what
//!
//! Before [`data::MANUFACTURER_DROIDS_LOST_DAY`] a site is a **fixed
//! garrison**: the machines' own wave size, each body rolled a Trooper at
//! the day's [`trooper_percent`] and one of their people otherwise, and no
//! reinforcement. From that day on it is their people alone, in the
//! machines' own waves. What a Manufacturer carries is [`gear`], by the
//! day: the pistol, then a tier-one gun, then tier-one armour as well, then
//! whatever tier the machines at that site would come at. All of it is
//! rolled off a seed the site and the world clock make, so it is fixed for
//! a visit (the clock stands still in a mission) and rolled afresh on the
//! next (every trip moves it).

use bims::combat::{Gear, Tier};
use worldgen::{Galaxy, StationBlueprint};

use crate::{data, heart, jammer};

/// Whether a station could be theirs at all: any orbital station the
/// generator made. Never a town — a settlement is not a blueprint, so it
/// never reaches this — and never the machines' own derived jammer or
/// their fortress.
pub fn eligible(station: &StationBlueprint) -> bool {
    !jammer::is_derived(station.id) && !heart::is_heart(station.id)
}

/// Whether the galaxy's own roll makes a site theirs: odds of
/// [`data::MANUFACTURER_SITE_CHANCE`] in a hundred, off the galaxy's seed,
/// the star and the station — no stream a fight draws from.
pub fn rolled(galaxy_seed: u64, star: u32, station: u32) -> bool {
    site_roll(galaxy_seed, star, station, 0x_4D41_4E55_4641_4354) < data::MANUFACTURER_SITE_CHANCE
}

/// The sites made theirs on top of the roll so a run has somebody to fight
/// from the first day: where fewer than [`data::MANUFACTURER_NEAR_SITES`]
/// eligible stations within [`data::MANUFACTURER_NEAR_HOPS`] lanes of the
/// crew's own star (and not in it) were rolled, that many more of the
/// others there, picked in an order off the galaxy's seed. `(star,
/// station)` pairs, sorted. Empty where the roll already did it — and
/// where there is nothing near to pick from. `primaries_only` keeps to the
/// one station each system offers (task 135, `World::offered_station`);
/// off only under the tests' dial that keeps whole systems.
pub fn near_sites(galaxy: &Galaxy, home: u32, primaries_only: bool) -> Vec<(u32, u32)> {
    let hops = galaxy.hops_from(home);
    let mut rolled_near = 0usize;
    let mut rest: Vec<(u64, u32, u32)> = Vec::new();
    for (star, &h) in hops.iter().enumerate() {
        if h == 0 || h > data::MANUFACTURER_NEAR_HOPS {
            continue;
        }
        let star = star as u32;
        let Some(system) = galaxy.system(star) else {
            continue;
        };
        // Only the station the system offers (task 135): the rest are never
        // met — bar under the tests' dial.
        let primary = crate::world::offered::primary(&system.stations);
        for station in system
            .stations
            .iter()
            .filter(|s| eligible(s) && (!primaries_only || Some(s.id) == primary))
        {
            if rolled(galaxy.seed, star, station.id) {
                rolled_near += 1;
            } else {
                let order = worldgen::rng::mix(
                    galaxy.seed
                        ^ 0x_4E45_4152
                        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station.id)),
                );
                rest.push((order, star, station.id));
            }
        }
    }
    let wanted = data::MANUFACTURER_NEAR_SITES.saturating_sub(rolled_near);
    rest.sort_unstable();
    let mut picked: Vec<(u32, u32)> = rest
        .into_iter()
        .take(wanted)
        .map(|(_, star, station)| (star, station))
        .collect();
    picked.sort_unstable();
    picked
}

/// Whether a station is theirs: [`eligible`], outside the crew's own
/// system, and [`rolled`] or one of the `near` sites [`near_sites`] made
/// up.
pub fn holds(
    galaxy_seed: u64,
    home: u32,
    near: &[(u32, u32)],
    star: u32,
    station: &StationBlueprint,
) -> bool {
    star != home
        && eligible(station)
        && (rolled(galaxy_seed, star, station.id) || near.contains(&(star, station.id)))
}

/// The share of a garrison that is a Trooper on `day`, in per cent: the
/// row of [`data::MANUFACTURER_TROOPER_PERCENT`] the day has reached.
pub fn trooper_percent(day: u32) -> u32 {
    data::MANUFACTURER_TROOPER_PERCENT
        .iter()
        .rev()
        .find(|&&(from, _)| day >= from)
        .map_or(0, |&(_, percent)| percent)
}

/// Whether they still have the machines on `day`: before
/// [`data::MANUFACTURER_DROIDS_LOST_DAY`], a fixed garrison with Troopers
/// in it; from it on, waves of their own people.
pub fn has_droids(day: u32) -> bool {
    day < data::MANUFACTURER_DROIDS_LOST_DAY
}

/// A garrison of `n` on `day`, body by body: `true` for a Trooper, `false`
/// for one of their people, each rolled on its own at
/// [`trooper_percent`] off `seed`.
pub fn garrison(n: u32, day: u32, seed: u64) -> Vec<bool> {
    let percent = trooper_percent(day);
    let mut rng = worldgen::rng::Rng::new(seed ^ 0x_5452_4F4F_5045_5253);
    (0..n).map(|_| rng.below(100) < percent).collect()
}

/// The tier of what they carry on `day` at a site whose machines would
/// come at `droid_tier`: tier one while they still have the machines —
/// the pistol days counted as tier one — and the machines' own after.
/// What their bounty is paid by and a site of theirs offers relics at.
pub fn gear_tier(day: u32, droid_tier: Tier) -> Tier {
    if has_droids(day) {
        Tier::One
    } else {
        droid_tier
    }
}

/// What one of them carries on `day`, off `seed`, with any armour numbered
/// from `piece_ids`: the laser pistol alone before
/// [`data::MANUFACTURER_ANY_GUN_DAY`]; a tier-one gun from it; tier-one
/// armour besides from [`data::MANUFACTURER_ARMOUR_DAY`]; and from
/// [`data::MANUFACTURER_DROIDS_LOST_DAY`] a gun and armour at
/// `droid_tier`, the tier the machines at that site would come at.
pub fn gear(day: u32, droid_tier: Tier, seed: u64, piece_ids: u32) -> Gear {
    let (weapon, armour) = if day < data::MANUFACTURER_ANY_GUN_DAY {
        (None, None)
    } else if day < data::MANUFACTURER_ARMOUR_DAY {
        (Some(Tier::One), None)
    } else if has_droids(day) {
        (Some(Tier::One), Some(Tier::One))
    } else {
        (Some(droid_tier), Some(droid_tier))
    };
    Gear::manufacturer(seed, weapon, armour, piece_ids)
}

/// A number under a hundred for a site, off the galaxy's seed and a salt.
fn site_roll(galaxy_seed: u64, star: u32, station: u32, salt: u64) -> u32 {
    let seed = worldgen::rng::mix(galaxy_seed ^ salt)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station));
    worldgen::rng::Rng::new(seed).below(100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::WeaponKind;

    #[test]
    fn the_schedule_is_the_one_asked_for() {
        // --- the trooper share by day ---
        for (day, share) in [
            (0, 0),
            (4, 0),
            (5, 10),
            (6, 10),
            (7, 25),
            (8, 50),
            (9, 60),
            (10, 0),
            (40, 0),
        ] {
            assert_eq!(trooper_percent(day), share, "day {day}");
        }
        // --- what they carry by day ---
        for day in 0..3 {
            for seed in 0..50 {
                let g = gear(day, Tier::Three, seed, 1);
                assert_eq!(g.weapon, Some(WeaponKind::LaserPistol.basic()));
                assert!(g.head.is_none() && g.body.is_none() && g.legs.is_none());
            }
        }
        let mut kinds = std::collections::BTreeSet::new();
        for day in 3..6 {
            for seed in 0..200 {
                let g = gear(day, Tier::Three, seed, 1);
                let w = g.weapon.unwrap();
                assert_eq!(w.tier, Tier::One);
                assert_ne!(w.kind, WeaponKind::Schword, "never a blade");
                kinds.insert(w.kind as u32);
                assert!(g.head.is_none() && g.body.is_none() && g.legs.is_none());
            }
        }
        assert!(kinds.len() >= 3, "a tier-one gun of more than one kind");
        for day in 6..10 {
            let g = gear(day, Tier::Three, 7, 1);
            assert_eq!(g.weapon.unwrap().tier, Tier::One);
            for piece in [g.head, g.body, g.legs] {
                assert_eq!(piece.unwrap().tier, Tier::One);
            }
        }
        for tier in Tier::ALL {
            let g = gear(12, tier, 7, 1);
            assert_eq!(g.weapon.unwrap().tier, tier);
            for piece in [g.head, g.body, g.legs] {
                assert_eq!(piece.unwrap().tier, tier);
            }
            assert_eq!(gear_tier(12, tier), tier);
            assert_eq!(gear_tier(9, tier), Tier::One);
        }
        // --- the garrison's rolls come out near the share ---
        let troopers: usize = (0..200u64)
            .map(|seed| garrison(10, 8, seed).iter().filter(|&&t| t).count())
            .sum();
        assert!(
            (800..1200).contains(&troopers),
            "{troopers} of 2000 at day 8"
        );
        assert!(garrison(16, 2, 3).iter().all(|&t| !t));
        assert!(garrison(16, 10, 3).iter().all(|&t| !t));
    }

    #[test]
    fn a_galaxy_has_them_near_home_and_about_one_site_in_ten() {
        // The share is pooled over the seeds: a galaxy's few hundred sites
        // within ten lanes are too few to hold one galaxy to a tight band.
        let (mut theirs, mut all) = (0u32, 0u32);
        for seed in [1u64, 7, 42, 0x_5749_4e44_4f57_0001] {
            let galaxy = Galaxy::new(seed, worldgen::GalaxyType::SpiralTwoArm);
            let Some((home, _)) = crate::spawn_anywhere(&galaxy, 0) else {
                continue;
            };
            let near = near_sites(&galaxy, home, true);
            let hops = galaxy.hops_from(home);
            let mut close = 0;
            for (star, &h) in hops.iter().enumerate() {
                if h > 10 {
                    continue;
                }
                let star = star as u32;
                let Some(system) = galaxy.system(star) else {
                    continue;
                };
                for s in system.stations.iter().filter(|s| eligible(s)) {
                    let held = holds(galaxy.seed, home, &near, star, s);
                    if h == 0 {
                        assert!(!held, "none in the crew's own system");
                    }
                    if held && (1..=data::MANUFACTURER_NEAR_HOPS).contains(&h) {
                        close += 1;
                    }
                    if h > data::MANUFACTURER_NEAR_HOPS && h <= 10 {
                        all += 1;
                        theirs += u32::from(held);
                    }
                }
            }
            assert!(
                close >= data::MANUFACTURER_NEAR_SITES,
                "seed {seed}: {close} near"
            );
            // The same galaxy and home, the same answer.
            assert_eq!(near, near_sites(&galaxy, home, true));
        }
        let percent = theirs * 100 / all.max(1);
        assert!((7..=13).contains(&percent), "{percent}% of {all}");
    }
}

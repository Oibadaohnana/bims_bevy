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
//! **Who stands in a wave is the same everywhere** (October 2026, the
//! player's: mixed waves at every site): a share of it the machines and
//! the rest their people, the share the day's
//! (`crate::droid::WaveScaling::machines_in`) — their people alone
//! through the scaling's area 0, the machines' share rising through the
//! tier-one area to the whole wave at tier two's door. What one of them
//! carries is [`gear`], at the tier the run day deals it
//! (`crate::droid::WaveScaling::gear_tiers`): the laser pistol alone for
//! the share not yet geared, and a gun and armour at its tier for the
//! rest. All of it is rolled off a seed the site and the world clock
//! make, so it is fixed for a visit (the clock stands still in a mission)
//! and rolled afresh on the next (every trip moves it).

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
        // Never in an elite's system: its station is the machines'.
        if crate::elite::rolled(galaxy.seed, star) {
            continue;
        }
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
        && !crate::elite::rolled(galaxy_seed, star)
        && eligible(station)
        && (rolled(galaxy_seed, star, station.id) || near.contains(&(star, station.id)))
}

/// What one of them carries, off `seed`, with its armour numbered
/// `piece_ids` (task 147): with no `tier` the laser pistol and nothing
/// else; with one a gun (never the schword) and armour at it. The tier is
/// the run day's (`World::manufacturer_gear_tiers`).
pub fn gear(tier: Option<Tier>, seed: u64, piece_ids: u32) -> Gear {
    Gear::manufacturer(seed, tier, tier, piece_ids)
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
    fn they_carry_the_pistol_or_a_gun_and_armour_at_the_tier() {
        // --- what they carry, by the tier the day deals them ---
        for seed in 0..50 {
            let g = gear(None, seed, 1);
            assert_eq!(g.weapon, Some(WeaponKind::LaserPistol.basic()));
            assert!(g.armour.is_none());
        }
        let mut kinds = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let g = gear(Some(Tier::One), seed, 1);
            let w = g.weapon.unwrap();
            assert_eq!(w.tier, Tier::One);
            assert_ne!(w.kind, WeaponKind::Schword, "never a blade");
            kinds.insert(w.kind as u32);
        }
        assert!(kinds.len() >= 3, "a tier-one gun of more than one kind");
        for tier in Tier::ALL {
            let g = gear(Some(tier), 7, 1);
            assert_eq!(g.weapon.unwrap().tier, tier);
            assert_eq!(g.armour.unwrap().tier, tier);
        }
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

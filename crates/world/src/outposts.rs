//! The machines' outposts (task 136): the sites of a system the machines
//! hold **from the first day**, crisis or none, so the map is attack and
//! defence evenly and not every site a defence.
//!
//! The rule is a line of arithmetic, like the crisis's own: a system's
//! **candidates** — every station and settlement that is neither its
//! trader, the Manufacturers', the Machine Heart's fortress nor a derived
//! jammer — in id order, every other one the machines', a coin off the
//! galaxy's seed and the star deciding whether the first is. Two sites,
//! one of each; three, one or two. **The crew's home is never one**: in
//! the home system the coin is the one that leaves it a defence, so the
//! run's first mission is the defence it was tuned as.
//!
//! An outpost is held the way a station the crisis took is — an
//! [`Infestation`], laid by [`World::infest`] at the doors the
//! Manufacturers' sites are laid at (`World::settle_jammer`: the start, a
//! jump, the spread and every load) — so it is an Attack site
//! ([`World::site_kind`]), cleared or not, and everything the machines'
//! stations have (waves, the clear, the bounty, the relic reward, the
//! cache) it has. Never over a site the crew have met otherwise: one with
//! a defence, won, lost or running, or a town they held. The map's quote
//! of a system never visited asks [`World::outposts_of`] for the same
//! answer.
//!
//! In a system the machines have (the crisis's flip) every site but the
//! trader is theirs anyway, outposts or not: only attack.

use super::*;

/// Salt for the coin, "OUTPOSTS".
const OUTPOST_SALT: u64 = 0x_4f55_5450_4f53_5453;

/// Which of `candidates` (sorted, deduplicated) the machines hold from the
/// start: every other one, the coin off `galaxy_seed` and `star` choosing
/// whether the first is — or, where `home` is among them, the coin that
/// leaves `home` out.
pub fn held(galaxy_seed: u64, star: u32, home: Option<u32>, candidates: &[u32]) -> Vec<u32> {
    let coin = match home.and_then(|h| candidates.iter().position(|&c| c == h)) {
        Some(at) => (at % 2) as u32,
        None => {
            let seed = worldgen::rng::mix(galaxy_seed ^ OUTPOST_SALT)
                ^ worldgen::rng::mix(u64::from(star));
            worldgen::rng::Rng::new(seed).below(2)
        }
    };
    candidates
        .iter()
        .enumerate()
        .filter(|&(at, _)| (at as u32 + coin) % 2 == 1)
        .map(|(_, &id)| id)
        .collect()
}

impl World {
    /// The machines' outposts of `star`'s system, whose generated system
    /// is `system` ([`held`] over its candidates), in id order. None under
    /// the tests' quiet dial ([`World::set_quiet_sites_for_probe`]).
    pub fn outposts_of(&self, star: u32, system: &StarSystem) -> Vec<u32> {
        if self.quiet_sites {
            return Vec::new();
        }
        self.outposts_by_rule(star, system)
    }

    /// [`World::outposts_of`] whatever the dial says.
    fn outposts_by_rule(&self, star: u32, system: &StarSystem) -> Vec<u32> {
        // The candidates are the two fights the system offers (task 135):
        // its station and its town. Where the station is the
        // Manufacturers', that is the attack and the town the defence.
        // Under the tests' dial that keeps whole systems, every station and
        // town but the trader's, the Manufacturers' and the derived.
        // An elite is its system's attack (`crate::elite`): no outpost
        // beside it, and under the dial none dealt to it.
        let elite = self.elite_station(star, &system.stations);
        if elite.is_some() && !self.whole_systems {
            return Vec::new();
        }
        let candidates = if self.whole_systems {
            let mut all: Vec<u32> = system
                .stations
                .iter()
                .filter(|s| !heart::is_heart(s.id) && !jammer::is_derived(s.id))
                .filter(|s| Some(s.id) != elite)
                .filter(|s| !self.is_trader_station(star, &system.stations, s))
                .filter(|s| !self.is_manufacturer_site(star, s))
                .map(|s| s.id)
                .collect();
            all.extend(
                system
                    .bodies
                    .iter()
                    .filter(|b| surface::landable(b.kind))
                    .map(|b| surface::surface_id(b.id)),
            );
            all.sort_unstable();
            all
        } else {
            self.offered_fights(star, system)
        };
        if !self.whole_systems
            && system
                .stations
                .iter()
                .any(|s| candidates.contains(&s.id) && self.is_manufacturer_site(star, s))
        {
            return Vec::new();
        }
        let home = (star == self.home_star).then_some(self.home);
        held(self.galaxy_seed, star, home, &candidates)
    }

    /// Every outpost of this system laid as the machines' (task 136):
    /// [`World::infest`], but never over a site with a defence or a town
    /// held — nor, as `infest` already refuses, one held already or a
    /// trader.
    pub(super) fn settle_outposts(&mut self) {
        let system = self.system.clone();
        for id in self.outposts_of(self.star_id, &system) {
            if self.is_droid_held(id) || self.defense(id).is_some() || self.town_held(id) {
                continue;
            }
            self.infest(id);
        }
    }

    /// The quiet dial going on: every outpost of this system not yet
    /// fought over given back ([`World::give_back_outposts`]).
    pub(super) fn drop_outposts(&mut self) {
        self.give_back_outposts(None);
    }

    /// The outposts of this system — or the one `only` names — not yet
    /// fought over (no wave settled, not cleared) given back to their
    /// people, and a room open on one opened again with them. Nothing in
    /// a system the machines have, where the crisis holds them anyway.
    /// The quiet dial's, and [`World::land_for_probe`]'s for its town, so
    /// a probe that lands to stand in a friendly town still does.
    pub(super) fn give_back_outposts(&mut self, only: Option<u32>) {
        if self.infested(self.star_id) {
            return;
        }
        let system = self.system.clone();
        let outposts = self.outposts_by_rule(self.star_id, &system);
        let fresh = |it: &Infestation| {
            outposts.contains(&it.station)
                && only.is_none_or(|id| id == it.station)
                && !it.manufacturers
                && !it.settled
                && !it.cleared
        };
        let dropped: Vec<u32> = self
            .infested
            .iter()
            .filter(|it| fresh(it))
            .map(|it| it.station)
            .collect();
        if dropped.is_empty() {
            return;
        }
        self.infested.retain(|it| !fresh(it));
        if let Some(id) = self.residents.as_ref().map(|r| r.station)
            && dropped.contains(&id)
        {
            self.reopen_residents(id);
        }
        self.apply_stances();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{REFERENCE_MONEY, open_simulation_world};
    use crate::run::SiteKind;
    use shipdesign::fixture::flyer;

    /// Every other candidate, the same answer on every call, and the home
    /// never one: two candidates are one of each, four are two.
    #[test]
    fn every_other_candidate_is_held_and_home_never_is() {
        let four = [3, 5, 9, 12];
        for seed in 0..32 {
            for star in 0..8 {
                let got = held(seed, star, None, &four);
                assert!(got == [5, 12] || got == [3, 9], "{got:?}");
                assert_eq!(got, held(seed, star, None, &four));
                assert_eq!(held(seed, star, None, &[4, 7]).len(), 1);
            }
        }
        for home in four {
            for seed in 0..8 {
                let got = held(seed, 1, Some(home), &four);
                assert_eq!(got.len(), 2);
                assert!(!got.contains(&home), "home {home} held: {got:?}");
            }
        }
        // Both coins come up over a few stars.
        let firsts: Vec<bool> = (0..16)
            .map(|star| held(7, star, None, &[1, 2]) == [1])
            .collect();
        assert!(firsts.contains(&true) && firsts.contains(&false));
    }

    /// The map offers as many sites to attack as to defend, system by
    /// system — off by one at most where a system's count is odd — and the
    /// crew's home is a defence.
    #[test]
    fn the_map_offers_attack_and_defence_evenly_and_home_is_a_defence() {
        let world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
        assert_eq!(world.site_kind(world.home), SiteKind::Defend);
        // The home is one of its system's defences, and the one site the
        // map will not quote (the crew are there).
        let mut by_star: Vec<(u32, i32, i32)> = vec![(world.star_id, 0, 1)];
        for (site, quote) in world.travel_quotes() {
            let Ok(quote) = quote else { continue };
            if quote.trader || quote.manufacturers || quote.infested || quote.heart.is_some() {
                continue;
            }
            if heart::is_heart(site.station) || jammer::is_derived(site.station) {
                continue;
            }
            let at = match by_star.iter().position(|&(s, _, _)| s == site.star) {
                Some(at) => at,
                None => {
                    by_star.push((site.star, 0, 0));
                    by_star.len() - 1
                }
            };
            match quote.kind {
                SiteKind::Attack => by_star[at].1 += 1,
                SiteKind::Defend => by_star[at].2 += 1,
                SiteKind::Trader => {}
            }
        }
        let attacks: i32 = by_star.iter().map(|&(_, a, _)| a).sum();
        let defences: i32 = by_star.iter().map(|&(_, _, d)| d).sum();
        assert!(attacks > 0 && defences > 0, "{by_star:?}");
        for &(star, a, d) in &by_star {
            assert!(
                (a - d).abs() <= 1,
                "star {star}: {a} to attack, {d} to defend"
            );
        }
    }

    /// In a system the machines have, every site but a trader is an attack.
    #[test]
    fn a_system_the_machines_have_is_only_attacks() {
        let mut world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
        world.set_droid_origin_for_probe(world.star_id);
        world.set_crisis_first_day_for_probe(0);
        let sites = world.sites_at(world.star_id);
        for site in sites {
            if world.is_trader_here(site.station) {
                continue;
            }
            assert_eq!(world.site_kind(site.station), SiteKind::Attack, "{site:?}");
        }
    }
}

//! What a system offers (task 135): **one station and one planet's
//! town**, whatever the generator put there — and, besides, its trader
//! where it has one, the machines' derived jammer or the Machine Heart's
//! fortress where those stand. The crew's home system is the home station
//! and the town alone.
//!
//! The station is the system's **primary**: the lowest-numbered station
//! the generator made ([`primary`]), or the crew's home in theirs. The
//! trader is picked among the rest (`World::trader_of`), so a system with
//! a trader keeps two stations; the Manufacturers' made-up sites near home
//! are primaries (`manufacturer::near_sites`). The town is the lowest
//! landable body's ([`town_body`]). Every other station and settlement is
//! taken out of the system the moment it is settled
//! ([`World::settle_offered`], at the top of `World::settle_jammer`: the
//! start, a jump, the spread and every load), and a system of another star
//! is trimmed the same way before its sites are listed or quoted
//! ([`World::trim_system`]). The bodies stay: a planet with no town on it
//! is scenery.
//!
//! **One fight a system.** Once a mission begins at a system's Attack or
//! Defend site the other is refused for the rest of the run
//! ([`Refusal::OtherSiteChosen`], `Run::chosen`): the crew choose which of
//! the two to fight. A trader and the Heart are never refused so.

use super::*;
use worldgen::StationBlueprint;

/// The station a system is offered by: the lowest-numbered the generator
/// made — never a derived jammer or the fortress, which are laid later
/// and stand beside it.
pub fn primary(stations: &[StationBlueprint]) -> Option<u32> {
    stations
        .iter()
        .filter(|s| !jammer::is_derived(s.id) && !heart::is_heart(s.id))
        .map(|s| s.id)
        .min()
}

/// The body whose town a system offers: the lowest-numbered with ground
/// to stand one on. Every system has one since worldgen's
/// `GENERATOR_VERSION` 8.
pub fn town_body(system: &StarSystem) -> Option<u32> {
    system
        .bodies
        .iter()
        .filter(|b| surface::landable(b.kind))
        .map(|b| b.id)
        .min()
}

impl World {
    /// The one station `star`'s system offers besides its trader: the
    /// crew's home in theirs, the [`primary`] of `stations` anywhere else.
    pub fn offered_station(&self, star: u32, stations: &[StationBlueprint]) -> Option<u32> {
        if star == self.home_star {
            Some(self.home)
        } else {
            primary(stations)
        }
    }

    /// `star`'s generated `system` cut to what it offers: the offered
    /// station, the trader, and whatever derived station or fortress it
    /// already holds. Doing it twice changes nothing.
    pub fn trim_system(&self, star: u32, system: &mut StarSystem) {
        if self.whole_systems {
            return;
        }
        let keep = self.offered_station(star, &system.stations);
        let trader = self.trader_of(star, &system.stations);
        system.stations.retain(|s| {
            jammer::is_derived(s.id)
                || heart::is_heart(s.id)
                || Some(s.id) == keep
                || Some(s.id) == trader
        });
    }

    /// The towns `system` offers: the one on its [`town_body`].
    pub fn offered_surfaces(&self, system: &StarSystem) -> Vec<Surface> {
        if self.whole_systems {
            return Surface::all_of(system, self.galaxy_seed);
        }
        let body = town_body(system);
        Surface::all_of(system, self.galaxy_seed)
            .into_iter()
            .filter(|s| Some(s.body) == body)
            .collect()
    }

    /// Every site `star`'s system offers that is a mission — its station,
    /// unless that is its trader, and its town — in id order: what one
    /// fight a system chooses between. The machines' outposts are dealt
    /// among these, the Manufacturers' station left out.
    pub fn offered_fights(&self, star: u32, system: &StarSystem) -> Vec<u32> {
        let mut sites: Vec<u32> = self
            .offered_station(star, &system.stations)
            .filter(|&id| self.trader_of(star, &system.stations) != Some(id))
            .into_iter()
            .collect();
        sites.extend(town_body(system).map(surface::surface_id));
        sites.sort_unstable();
        sites
    }

    /// This system cut to what it offers ([`World::trim_system`]): the
    /// stations, the settlements and the chart together. Called first
    /// wherever a system is settled.
    pub(super) fn settle_offered(&mut self) {
        if self.whole_systems {
            return;
        }
        let mut system = self.system.clone();
        self.trim_system(self.star_id, &mut system);
        self.system = system;
        let kept: Vec<u32> = self.system.stations.iter().map(|s| s.id).collect();
        self.stations.retain(|s| kept.contains(&s.id));
        let body = town_body(&self.system);
        self.surfaces.retain(|s| Some(s.body) == body);
        self.discovered
            .retain(|n| !matches!(n, Node::Station(id) if !kept.contains(id)));
    }

    /// Whether a trip to `site`, of `system`, is refused because the crew
    /// already chose the other fight of it ([`Refusal::OtherSiteChosen`]).
    pub(super) fn other_site_chosen(&self, site: run::Site, system: &StarSystem) -> bool {
        !self.whole_systems
            && self
                .run
                .chosen
                .iter()
                .any(|c| c.star == site.star && c.station != site.station)
            && self
                .offered_fights(site.star, system)
                .contains(&site.station)
    }

    /// Whether a site of this system is the fight the crew passed over:
    /// they fought its other one ([`Refusal::OtherSiteChosen`]). What the
    /// system map greys its word by.
    pub fn passed_over(&self, station: u32) -> bool {
        self.other_site_chosen(
            run::Site {
                star: self.star_id,
                station,
            },
            &self.system,
        )
    }

    /// The mission just begun at `station` of this system, if it is one of
    /// the two fights it offers, is the one chosen there.
    pub(super) fn choose_site(&mut self, station: u32) {
        if self.whole_systems {
            return;
        }
        let site = run::Site {
            star: self.star_id,
            station,
        };
        if self.run.chosen.iter().any(|c| c.star == site.star) {
            return;
        }
        if !self
            .offered_fights(self.star_id, &self.system)
            .contains(&station)
        {
            return;
        }
        self.run.chosen.push(site);
        self.run.chosen.sort_by_key(|c| (c.star, c.station));
    }

    /// The tests' dial (task 135): `true` is every system as the generator
    /// made it — every station and town, the trader and the Manufacturers'
    /// made-up sites picked among all of them, and no one-fight rule — for
    /// the tests whose subject is not what a system offers, written against
    /// whole systems. The shared fixtures set it right after the start;
    /// what the start trimmed is put back, the stations the world already
    /// holds (the spawn replanned as the hub) kept as they are.
    pub fn set_whole_systems_for_probe(&mut self, whole: bool) {
        // The machines' outposts laid by the rule as it stood, not yet
        // fought over, given back first: the settling below lays them by
        // the rule as it now is.
        self.drop_outposts();
        self.whole_systems = whole;
        let galaxy = self.galaxy();
        self.manufacturer_near = crate::manufacturer::near_sites(&galaxy, self.home_star, !whole);
        self.trader_near = self.trader_near_sites(&galaxy);
        if whole && let Some(full) = galaxy.system(self.star_id) {
            let mut blueprints = full.stations.clone();
            let mut stations = Station::all_of(&full);
            for station in &mut stations {
                if let Some(held) = self.stations.iter().find(|s| s.id == station.id) {
                    *station = held.clone();
                }
            }
            // A derived jammer or the fortress, laid past everything the
            // generator numbers, stays where it is.
            for held in &self.stations {
                if !stations.iter().any(|s| s.id == held.id) {
                    stations.push(held.clone());
                }
            }
            for bp in &self.system.stations {
                if !blueprints.iter().any(|b| b.id == bp.id) {
                    blueprints.push(bp.clone());
                }
            }
            stations.sort_by_key(|s| s.id);
            blueprints.sort_by_key(|b| b.id);
            self.stations = stations;
            self.system.stations = blueprints;
            let mut surfaces = Surface::all_of(&full, self.galaxy_seed);
            for surface in &mut surfaces {
                if let Some(at) = self.surfaces.iter().position(|s| s.body == surface.body) {
                    *surface = self.surfaces.remove(at);
                }
            }
            self.surfaces = surfaces;
            let charted = self.discovered.len() >= 2;
            if charted {
                self.discovered.extend(self.system.nodes());
                self.discovered.sort_by_key(node_key);
                self.discovered.dedup();
            }
        }
        self.settle_jammer();
    }
}

//! What a system offers: **one mission** — its station or one planet's
//! town, an attack or a defence — or, instead, its trader; besides them,
//! the machines' derived jammer or the Machine Heart's fortress where
//! those stand. Task 135 made it one station and one town a system, one
//! of them fought; the galaxy-only map cut it to the one, so the galaxy
//! chart says what every star holds and the crew go there off the chart.
//!
//! The station is the system's **primary**: the lowest-numbered station
//! the generator made ([`primary`]), or the crew's home in theirs. The
//! town is the lowest landable body's ([`town_body`]). Which of the two
//! is the mission is a coin off the galaxy's seed and the star
//! ([`town_fight`]) — the station wherever there is no town, the town
//! wherever there is no station, and the station whatever the coin says
//! at home, at an elite and where the Manufacturers hold it
//! ([`World::offered_fight`]). Whether it is an attack or a defence is
//! the outposts' own coin (`outposts.rs`).
//!
//! A system with a **trader** (`World::trader_of`, its primary since the
//! galaxy-only map) offers the trader and no mission. Every other station
//! and settlement is taken out of the system the moment it is settled
//! ([`World::settle_offered`], at the top of `World::settle_jammer`: the
//! start, a jump, the spread and every load), and a system of another star
//! is trimmed the same way before its sites are listed or quoted
//! ([`World::trim_system`]). The bodies stay: a planet with no town on it
//! is scenery. Every rule here answers the same over a system trimmed as
//! over it whole, so trimming twice changes nothing.
//!
//! **One fight a system** (task 135, `Run::chosen`,
//! [`Refusal::OtherSiteChosen`]) is still the rule, and refuses nothing
//! while a system offers one fight. A trader and the Heart are never
//! refused so.

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

/// Salt for the coin that picks a system's one fight, "MISSION".
const MISSION_SALT: u64 = 0x_004d_4953_5349_4f4e;

/// Whether `star`'s one fight is its town rather than its station, where
/// it has both and nothing holds it to the station: a coin off the
/// galaxy's seed and the star, of a salt of its own, so it falls
/// independently of the outposts' attack-or-defence.
pub fn town_fight(galaxy_seed: u64, star: u32) -> bool {
    let seed = worldgen::rng::mix(galaxy_seed ^ MISSION_SALT) ^ worldgen::rng::mix(u64::from(star));
    worldgen::rng::Rng::new(seed).below(2) == 1
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

    /// The one mission `star`'s system, generated as `system` (trimmed or
    /// not), offers: its station or its town ([`town_fight`]), the station
    /// at home, at an elite and where it is the Manufacturers' — `None`
    /// where the system's trader is all it offers.
    pub fn offered_fight(&self, star: u32, system: &StarSystem) -> Option<u32> {
        // The machines' origin offers the Heart's fortress and nothing
        // else ([`World::mission_site`]).
        if !self.whole_systems && star == self.droid_origin {
            return None;
        }
        if self.trader_of(star, &system.stations).is_some() {
            return None;
        }
        let town = town_body(system).map(surface::surface_id);
        let Some(station) = self.offered_station(star, &system.stations) else {
            return town;
        };
        let Some(town) = town else {
            return Some(station);
        };
        // Stateless rolls only, never a dial: the answer must not move
        // under the tests' quiet dial, or the system trimmed by one answer
        // would be read by the other.
        let held = star == self.home_star
            || crate::elite::holds(self.galaxy_seed, self.home_star, star)
            || self.elite_forced == Some(run::Site { star, station })
            || system
                .station(station)
                .is_some_and(|s| self.is_manufacturer_site(star, s));
        if !held && town_fight(self.galaxy_seed, star) {
            Some(town)
        } else {
            Some(station)
        }
    }

    /// `star`'s generated `system` cut to what it offers: the mission's
    /// station or the trader, and whatever derived station or fortress it
    /// already holds. Doing it twice changes nothing.
    pub fn trim_system(&self, star: u32, system: &mut StarSystem) {
        if self.whole_systems {
            return;
        }
        let keep = self.offered_fight(star, system);
        let trader = self.trader_of(star, &system.stations);
        system.stations.retain(|s| {
            jammer::is_derived(s.id)
                || heart::is_heart(s.id)
                || Some(s.id) == keep
                || Some(s.id) == trader
        });
    }

    /// The towns `system` offers: the one on its [`town_body`], where that
    /// is its mission.
    pub fn offered_surfaces(&self, system: &StarSystem) -> Vec<Surface> {
        if self.whole_systems {
            return Surface::all_of(system, self.galaxy_seed);
        }
        let fight = self.offered_fight(system.star_id, system);
        Surface::all_of(system, self.galaxy_seed)
            .into_iter()
            .filter(|s| Some(surface::surface_id(s.body)) == fight)
            .collect()
    }

    /// Every site `star`'s system offers that is a mission: its one
    /// ([`World::offered_fight`]), none where it is a trader's — what one
    /// fight a system chooses between, and what the machines' outposts are
    /// dealt among.
    pub fn offered_fights(&self, star: u32, system: &StarSystem) -> Vec<u32> {
        self.offered_fight(star, system).into_iter().collect()
    }

    /// **The one site of `star`'s system**, whatever it is: the Machine
    /// Heart's fortress at the machines' origin, else its mission
    /// ([`World::offered_fight`]), else its trader. What the galaxy chart
    /// marks and goes to, and — the system being the machines' — its
    /// jammer ([`World::jammer_site_of`]), so a system never offers two.
    pub fn mission_site(&self, star: u32, system: &StarSystem) -> Option<u32> {
        if !self.whole_systems && star == self.droid_origin {
            return Some(heart::heart_id(star));
        }
        self.offered_fight(star, system)
            .or_else(|| self.trader_of(star, &system.stations))
    }

    /// Where the jammer of `star`'s system, generated as `system`, stands
    /// once the machines have it: the system's one site
    /// ([`World::mission_site`]) — a station, a town, a trader or the
    /// Heart, an attack whatever it was. Under the tests' whole-systems
    /// dial the rule as it was: the lowest orbital that is neither the
    /// Manufacturers' nor the trader, else the machines' own derived one.
    pub fn jammer_site_of(&self, star: u32, system: &StarSystem) -> Option<u32> {
        if self.whole_systems {
            return Some(
                self.jammer_site_among(star, &system.stations)
                    .unwrap_or_else(|| jammer::jammer_id(star)),
            );
        }
        self.mission_site(star, system)
    }

    /// Whether an infested system's trader is fought for: in a run (not
    /// under the tests' whole-systems dial) the machines take it as they
    /// take any site — it is the system's one site, so its jammer — and
    /// once it is cleared it trades again.
    pub fn traders_fall(&self) -> bool {
        !self.whole_systems
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
        let fight = self.offered_fight(self.star_id, &self.system);
        self.surfaces
            .retain(|s| Some(surface::surface_id(s.body)) == fight);
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

    // --- elites (`crate::elite`) ------------------------------------------------

    /// The elite of `star`'s system, whose stations are `stations`, if it
    /// holds one ([`crate::elite::holds`]): the station it offers. None under
    /// the tests' quiet dial, as the outposts are none.
    pub fn elite_station(&self, star: u32, stations: &[StationBlueprint]) -> Option<u32> {
        if let Some(forced) = self.elite_forced.filter(|f| f.star == star) {
            return Some(forced.station);
        }
        if !self.holds_elite(star) {
            return None;
        }
        primary(stations)
    }

    /// Whether a station of this system is its elite.
    pub fn is_elite_here(&self, id: u32) -> bool {
        self.elite_station(self.star_id, &self.system.stations) == Some(id)
    }

    /// Whether a site anywhere is an elite: this system's off the world,
    /// any other's off the galaxy (generated here).
    pub fn is_elite(&self, site: run::Site) -> bool {
        if site.star == self.star_id {
            return self.is_elite_here(site.station);
        }
        if !self.holds_elite(site.star) {
            return false;
        }
        self.galaxy().system(site.star).is_some_and(|system| {
            self.elite_station(site.star, &system.stations) == Some(site.station)
        })
    }

    /// Whether `star`'s system holds an elite ([`crate::elite::holds`]),
    /// none under the tests' quiet dial — or one a probe forced there.
    pub fn holds_elite(&self, star: u32) -> bool {
        // Never the machines' origin in a run: its one site is the Heart.
        self.elite_forced.is_some_and(|f| f.star == star)
            || (!self.quiet_sites
                && (self.whole_systems || star != self.droid_origin)
                && crate::elite::holds(self.galaxy_seed, self.home_star, star))
    }

    /// The probes' dial: `station` of this system an elite whatever the
    /// roll says — its waves, its Guardian and its relics.
    pub fn set_elite_for_probe(&mut self, station: u32) {
        self.elite_forced = Some(run::Site {
            star: self.star_id,
            station,
        });
    }

    /// Every star whose system holds an elite, in id order — what the
    /// galaxy chart marks. A roll the crew are told, charted or not.
    pub fn elite_stars(&self, stars: u32) -> Vec<u32> {
        (0..stars).filter(|&star| self.holds_elite(star)).collect()
    }

    /// The nearest elite by the lanes, as `(star, station)`: this system's,
    /// else the fewest hops off, the lower star on a tie.
    pub fn nearest_elite_site(&self) -> Option<(u32, u32)> {
        let galaxy = self.galaxy();
        let hops = galaxy.hops_from(self.star_id);
        let mut stars: Vec<(u16, u32)> = hops
            .iter()
            .enumerate()
            .filter(|&(star, &h)| h != u16::MAX && self.holds_elite(star as u32))
            .map(|(star, &h)| (h, star as u32))
            .collect();
        stars.sort_unstable();
        stars.into_iter().find_map(|(_, star)| {
            let system = galaxy.system(star)?;
            self.elite_station(star, &system.stations)
                .map(|station| (star, station))
        })
    }

    /// The `BIMS_ELITE` probe and the tests': the crew travel to the nearest
    /// elite ([`World::nearest_elite_site`]) through a site of every star on
    /// the way, a trip being one lane, and a mission there begins. The
    /// station, or `None` with nowhere to go.
    pub fn elite_dock_for_probe(&mut self) -> Option<u32> {
        let (star, station) = self.nearest_elite_site()?;
        let route = if star == self.star_id {
            vec![star]
        } else {
            self.route_to(star)?
        };
        for &via in route.iter().skip(1).take(route.len().saturating_sub(2)) {
            let site = self
                .sites_at(via)
                .into_iter()
                .find(|s| self.travel_quote(*s).is_ok())?;
            self.probe_trip(site)?;
        }
        self.probe_trip(run::Site { star, station })?;
        Some(station)
    }

    /// This system's elite laid as the machines' from the first day,
    /// through [`World::infest`] — never over a site the crew have met
    /// otherwise (a defence, a town held). At the doors the outposts are
    /// laid at (`settle_jammer`).
    pub(super) fn settle_elite(&mut self) {
        let Some(id) = self.elite_station(self.star_id, &self.system.stations) else {
            return;
        };
        if self.is_droid_held(id) || self.defense(id).is_some() || self.town_held(id) {
            return;
        }
        self.infest(id);
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

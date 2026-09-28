//! The world's side of the Manufacturers (feature 109, the rules are
//! [`crate::manufacturer`]): which of this system's sites are theirs, their
//! people laid on the deck, and the handful of places their fight is not
//! the machines'.
//!
//! A child of `crate::world`, so it reaches the world's private fields the
//! way `mission.rs` and `fortress.rs` do.
//!
//! **A site of theirs is held the way a station the machines took is**: it
//! has an [`Infestation`] — flagged [`Infestation::manufacturers`] — so the
//! stance is hostile, nobody lives there, nothing is traded or hired, the
//! waves and their clock, the clear, the pending bounty, the relic reward
//! and the cache, and the site put back as it was met when it is left
//! uncleared, are all the machines' own machinery unchanged. What differs
//! is who stands on the deck — [`World::lay_manufacturers`] — and the
//! numbers: one wave while they still have the machines, the machines' own
//! count after, and [`data::MANUFACTURER_REINFORCE_STEPS`] between them.
//!
//! And what it is **not**: infested. The crisis never takes a site of
//! theirs ([`World::infest`] refuses it), it is never a system's jammer
//! ([`World::jammer_station`]), and a system with nothing else in it has
//! the machines' derived jammer laid beside it.

use super::*;
use crate::manufacturer;

/// Off a site's wave seed for a defence's wave of theirs (task 131), so
/// it never rolls what a garrison of theirs on the same day would.
const DEFENSE_SALT: u64 = 0x_4445_4645_4E43_4531;

impl World {
    /// Whether a station of **this** system is one the Manufacturers hold
    /// by the rule ([`manufacturer::holds`]) — whatever the crew have done
    /// there since. False for a settlement and for the machines' derived
    /// stations.
    pub fn is_manufacturer_station(&self, id: u32) -> bool {
        self.system
            .station(id)
            .is_some_and(|s| self.is_manufacturer_site(self.star_id, s))
    }

    /// Whether a station of any system is the Manufacturers': what the
    /// map's quote asks of a system it generated.
    pub fn is_manufacturer_site(&self, star: u32, station: &worldgen::StationBlueprint) -> bool {
        manufacturer::holds(
            self.galaxy_seed,
            self.home_star,
            &self.manufacturer_near,
            star,
            station,
        )
    }

    /// Whether the site at `id` is held by the Manufacturers now: it has
    /// their [`Infestation`].
    pub fn is_manufacturer_held(&self, id: u32) -> bool {
        self.infestation(id).is_some_and(|it| it.manufacturers)
    }

    /// Every station of this system that is theirs given its hold: an
    /// [`Infestation`] of theirs, sorted in by id like the machines'. Never
    /// one twice, and never over what is there already — a memory brings
    /// back the one the crew left, cleared or not. Called at the same doors
    /// as the jammer (`World::settle_jammer`): the start, a jump, the
    /// spread and every load.
    pub(super) fn settle_manufacturers(&mut self) {
        let theirs: Vec<u32> = self
            .system
            .stations
            .iter()
            .filter(|s| self.is_manufacturer_site(self.star_id, s))
            .map(|s| s.id)
            .collect();
        let mut added = false;
        for id in theirs {
            if self.infestation(id).is_some() {
                continue;
            }
            let mut it = Infestation::manufacturers(id);
            // A site of theirs may hide a relic cache on its research desk
            // as a held station may (feature 106): rolled here, once.
            it.cache = crate::relic::cache_rolled(self.galaxy_seed, self.star_id, id);
            self.infested.push(it);
            added = true;
        }
        if added {
            self.infested.sort_by_key(|it| it.station);
            self.apply_stances();
        }
    }

    /// How many of the Manufacturers on the residents' deck are on their
    /// feet: alive and not downed. One down is out of the fight — nobody
    /// revives it, and it dies when its countdown runs out — and holds no
    /// wave and no clear up.
    pub(super) fn manufacturers_standing(&self) -> u32 {
        let Some(residents) = &self.residents else {
            return 0;
        };
        let room = &residents.aboard.room;
        (0..room.crew_count() as usize)
            .filter(|&who| room.is_manufacturer(who) && room.is_alive(who) && !room.is_downed(who))
            .count() as u32
    }

    /// How many waves the site at `id` has all told, fixed at the crew's
    /// first dock: **one** at a site of the Manufacturers' while they still
    /// have the machines — a fixed garrison, nothing coming after it — and
    /// the machines' own count ([`World::droid_wave_count`]) otherwise.
    pub(super) fn wave_count_here(&self, id: u32) -> u32 {
        if self.is_manufacturer_held(id) && manufacturer::has_droids(self.days_gone()) {
            return 1;
        }
        self.droid_wave_count()
    }

    /// How long after a wave is down the next arrives at `id`, in steps of
    /// the mission clock: [`data::MANUFACTURER_REINFORCE_STEPS`] at a site
    /// of the Manufacturers' and the machines' own elsewhere — and the
    /// probes' dial over both, which shortens the machines' to a minute.
    pub(super) fn reinforce_steps_here(&self, id: u32) -> u64 {
        if self.is_manufacturer_held(id) && self.droid_reinforce == data::DROID_REINFORCE_STEPS {
            return data::MANUFACTURER_REINFORCE_STEPS;
        }
        self.droid_reinforce
    }

    /// What tier the Manufacturers at the site alongside carry
    /// ([`manufacturer::gear_tier`]): tier one while they have the machines,
    /// the pistol days among them, and the machines' own tier after. What a
    /// site of theirs offers its relics at.
    pub fn manufacturer_tier(&self) -> Tier {
        manufacturer::gear_tier(self.days_gone(), self.droid_tier())
    }

    /// The seed a site's people are rolled off for one wave: the galaxy,
    /// the star, the station, the wave and the **world clock** — which
    /// stands still through a mission and moves with every trip, so a
    /// visit's garrison is fixed for the visit and a fresh one the next.
    fn garrison_seed(&self, station: u32, wave: u32) -> u64 {
        worldgen::rng::mix(
            self.galaxy_seed
                ^ worldgen::rng::mix(u64::from(self.star_id) << 32 | u64::from(station))
                ^ worldgen::rng::mix(self.clock_minutes.floor() as u64 ^ 0x_4741_5252 << 32)
                ^ u64::from(wave),
        )
    }

    /// The wave that is aboard a site of the Manufacturers', laid on the
    /// residents' deck if it is not there yet — the first dock's garrison,
    /// or a reinforcement at the airlock its ship tied up at. Nothing on a
    /// site already cleared, and nothing twice: the room keeps which wave
    /// it holds ([`Residents::manufacturers_laid`]).
    ///
    /// **The garrison** (wave one) is the machines' own wave size, stood
    /// about the station's rooms as their first wave is, each body rolled a
    /// Trooper at the day's share while they still have the machines and
    /// one of their people otherwise. **A reinforcement** is their people
    /// alone. Their people go on the deck before the Troopers, since a
    /// body index past the Bims is a machine's.
    pub(super) fn lay_manufacturers(&mut self) {
        let Some(id) = self.residents.as_ref().map(|r| r.station) else {
            return;
        };
        let Some(it) = self.infestation(id).cloned() else {
            return;
        };
        let laid = self.residents.as_ref().map_or(0, |r| r.manufacturers_laid);
        if it.wave == 0 || it.cleared || laid >= it.wave {
            return;
        }
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let day = self.days_gone();
        let n = self.droid_wave_size();
        let tier = self.droid_tier();
        let seed = self.garrison_seed(id, it.wave);
        let first = it.wave == 1;
        let troopers = if first && manufacturer::has_droids(day) {
            manufacturer::garrison(n, day, seed)
        } else {
            vec![false; n as usize]
        };
        let placed = if first {
            let Some(residents) = &self.residents else {
                return;
            };
            let spots: Vec<bims::math::Vec2> = droidplan::spots_about(&station.design, n as usize)
                .into_iter()
                .map(|(x, y)| residents.aboard.to_room(dvec2(x, y)))
                .collect();
            Some((spots, 0.0))
        } else {
            self.arrival_spots(&station, n, it.wave)
        };
        let Some((spots, facing)) = placed else {
            return;
        };
        self.stand_manufacturers(&troopers, &spots, facing, day, tier, seed, it.wave);
    }

    /// A wave of theirs **attacking a site the crew are defending**
    /// (task 131): what lands in place of the machines' wave while they
    /// still have the machines (before [`data::MANUFACTURER_DROIDS_LOST_DAY`]).
    /// `n` bodies at the wave's arrival spots, each rolled a Trooper at the
    /// day's share ([`manufacturer::trooper_percent`]: none on day nought,
    /// a tenth from day five, up to three in five on day nine) and one of
    /// their people otherwise, their people armed by the day
    /// ([`manufacturer::gear`]). In a friendly room each of them is an
    /// *intruder* (`bims::game::Game::is_intruder`) and fights the
    /// machines' way. Off the site's own seed for the wave, with a salt of
    /// its own, so a garrison and a defence on one day never roll alike.
    pub(super) fn lay_defense_manufacturers(&mut self, id: u32, n: u32) {
        let Some(wave) = self.defense(id).map(|d| d.wave) else {
            return;
        };
        if wave == 0 || n == 0 {
            return;
        }
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let day = self.days_gone();
        let tier = self.droid_tier();
        let seed = self.garrison_seed(id, wave) ^ DEFENSE_SALT;
        let troopers = manufacturer::garrison(n, day, seed);
        let Some((spots, facing)) = self.arrival_spots(&station, n, wave) else {
            return;
        };
        self.stand_manufacturers(&troopers, &spots, facing, day, tier, seed, wave);
    }

    /// Whether the waves attacking a site the crew defend are the
    /// Manufacturers' today (task 131): while they still have the
    /// machines, before [`data::MANUFACTURER_DROIDS_LOST_DAY`]; the
    /// machines' own from then on — or always the machines' once a probe
    /// said so ([`World::set_defense_by_machines_for_probe`]).
    pub fn defense_by_manufacturers(&self) -> bool {
        !self.defense_by_machines_forced && manufacturer::has_droids(self.days_gone())
    }

    /// One wave of theirs stood on the residents' deck at `spots`: their
    /// people first — a body index past the Bims is a machine's — then the
    /// Troopers beside them, a body `i` of the wave a Trooper where
    /// `troopers[i]`. The room keeps which wave it holds
    /// ([`Residents::manufacturers_laid`]).
    #[allow(clippy::too_many_arguments)]
    fn stand_manufacturers(
        &mut self,
        troopers: &[bool],
        spots: &[bims::math::Vec2],
        facing: f32,
        day: u32,
        tier: Tier,
        seed: u64,
        wave: u32,
    ) {
        let n = troopers.len() as u32;
        let spot = |i: usize| spots.get(i).copied().unwrap_or(bims::math::Vec2::ZERO);
        let stagger = |i: usize| bims::game::PLAN_EVERY * (i as f32) / (n.max(1) as f32);
        let Some(residents) = &mut self.residents else {
            return;
        };
        let room = &mut residents.aboard.room;
        // Their people first.
        for (i, _) in troopers.iter().enumerate().filter(|(_, t)| !**t) {
            let own = worldgen::rng::mix(seed ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            // The room's pieces are its own; a thousand a body clear of
            // any other body's.
            let pieces = 10_000 + 1_000 * room.crew_count();
            let gear = manufacturer::gear(day, tier, own, pieces);
            room.enlist_manufacturer(spot(i), gear, own, stagger(i));
        }
        // Then the machines beside them: Troopers, armed by their place
        // among the Troopers as a wave's are.
        let machines: Vec<bims::droid::Droid> = troopers
            .iter()
            .enumerate()
            .filter(|(_, t)| **t)
            .enumerate()
            .map(|(k, (i, _))| {
                let mut droid = bims::droid::Droid::new(
                    bims::droid::DroidKind::Trooper,
                    tier,
                    k,
                    wave,
                    spot(i),
                    facing,
                    seed ^ (i as u64) << 8 ^ u64::from(wave),
                );
                droid.plan_wait = stagger(i);
                droid.breach_wait = droid.plan_wait;
                droid
            })
            .collect();
        room.adopt_droids(machines, bims::math::Vec2::ZERO);
        residents.aboard.crew = room.body_count();
        residents.manufacturers_laid = wave;
    }

    /// The nearest site of the Manufacturers' by the lanes, as `(star,
    /// station)`: this system's first, then the stars a hop off, then two,
    /// each star's stations in id order. `None` with none within
    /// [`data::MANUFACTURER_NEAR_HOPS`] of here.
    pub fn nearest_manufacturer_site(&self) -> Option<(u32, u32)> {
        let galaxy = self.galaxy();
        let hops = galaxy.hops_from(self.star_id);
        let mut stars: Vec<(u16, u32)> = hops
            .iter()
            .enumerate()
            .filter(|&(_, &h)| h <= data::MANUFACTURER_NEAR_HOPS)
            .map(|(star, &h)| (h, star as u32))
            .collect();
        stars.sort_unstable();
        stars.into_iter().find_map(|(_, star)| {
            let system = galaxy.system(star)?;
            system
                .stations
                .iter()
                .find(|s| self.is_manufacturer_site(star, s))
                .map(|s| (star, s.id))
        })
    }

    /// The `manufacturers` probe (feature 109) and the tests': the crew
    /// travel to the nearest site of theirs ([`World::nearest_manufacturer_site`])
    /// — through a site of every star on the way, a trip being one lane —
    /// and the world clock is then put **at `day`**, before the first step
    /// lays the garrison, so what stands there is what that day calls for.
    /// The station, or `None` with nowhere to go.
    pub fn manufacturer_dock_for_probe(&mut self, day: u32) -> Option<u32> {
        let (star, station) = self.nearest_manufacturer_site()?;
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
        // The trips took days of their own; the day asked for may be
        // before the crew could have got there, and the rooms' clocks —
        // what the day on the screen is read off — are put back with it.
        let minutes = f64::from(day) * time::DAY;
        if minutes < self.clock_minutes {
            self.clock_minutes = minutes;
            self.aboard.room.set_clock_for_probe(minutes as f32);
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.set_clock_for_probe(minutes as f32);
            }
        } else {
            self.set_day_for_probe(day);
        }
        Some(station)
    }

    /// One trip for a probe: off the site the crew are at, onto the map,
    /// and away to `site` as a vote carried would take them.
    fn probe_trip(&mut self, site: run::Site) -> Option<()> {
        let mut events = Vec::new();
        if self.run.phase == run::Phase::Mission {
            self.leave_mission(&mut events);
        }
        self.run.phase = run::Phase::Map;
        let quote = self.travel_quote(site).ok()?;
        self.travel(quote, &mut events);
        Some(())
    }
}

impl World {
    /// A held site's [`Infestation`] to be changed by hand — its clock put
    /// on, say — for a test that will not wait four hours of the mission
    /// clock.
    pub fn infestation_mut_for_probe(&mut self, id: u32) -> Option<&mut Infestation> {
        self.infested.iter_mut().find(|it| it.station == id)
    }
}

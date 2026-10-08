//! The world's side of the Manufacturers (feature 109, the rules are
//! [`crate::manufacturer`]): which of this system's sites are theirs, and
//! every wave laid on a deck — a share of it the machines and the rest
//! their people, at every site alike (October 2026, [`World::lay_wave`]).
//!
//! A child of `crate::world`, so it reaches the world's private fields the
//! way `mission.rs` and `fortress.rs` do.
//!
//! **A site of theirs is held the way a station the machines took is**: it
//! has an [`Infestation`] — flagged [`Infestation::manufacturers`] — so the
//! stance is hostile, nobody lives there, nothing is traded, the
//! waves and their clock, the clear, the pending bounty, the relic reward,
//! and the site put back as it was met when it is left
//! uncleared, are all the machines' own machinery unchanged, and so is who
//! stands on its deck: the day's mix, as anywhere.
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
            self.infested.push(Infestation::manufacturers(id));
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
    /// first dock: the day's area's ([`World::droid_wave_count`]), an
    /// elite's at least [`data::ELITE_WAVES`].
    pub(super) fn wave_count_here(&self, id: u32) -> u32 {
        // An elite comes in at least two (`crate::elite`), bar a count the
        // probes forced.
        if self.is_elite_here(id) {
            return match self.droid_waves_forced {
                Some(_) => self.droid_wave_count(),
                None => self.droid_wave_count().max(data::ELITE_WAVES),
            };
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

    /// The seed a site's people are rolled off for one wave: the galaxy,
    /// the star, the station, the wave and the **world clock** — which
    /// stands still through a mission and moves with every trip, so a
    /// visit's garrison is fixed for the visit and a fresh one the next.
    pub(super) fn garrison_seed(&self, station: u32, wave: u32) -> u64 {
        worldgen::rng::mix(
            self.galaxy_seed
                ^ worldgen::rng::mix(u64::from(self.star_id) << 32 | u64::from(station))
                ^ worldgen::rng::mix(self.clock_minutes.floor() as u64 ^ 0x_4741_5252 << 32)
                ^ u64::from(wave),
        )
    }

    /// How many of a wave of `n` are machines (October 2026, the player's:
    /// mixed waves at every site): the day's share
    /// ([`droidplan::WaveScaling::machines_in`]) — none through area 0,
    /// all of it from tier two's door — and at an elite's Guardian wave
    /// at least its Guardians, since a Guardian is a machine. All of it
    /// at the Heart and wherever a probe forced the machines, their
    /// kinds or their tier.
    pub fn machines_of(&self, n: u32, wave: u32) -> u32 {
        if self.at_the_heart()
            || self.machines_forced
            || self.droid_kinds_forced.is_some()
            || self.droid_tier.is_some()
        {
            return n;
        }
        let machines = self.scaling().machines_in(n, self.run_day());
        if self.elite_guardian_wave(wave) {
            let guardians = self.area_now().1.guardians.saturating_mul(self.players());
            return machines.max(guardians.min(n));
        }
        machines
    }

    /// A wave of `n` laid on the residents' deck at `id` (October 2026:
    /// every site's, attack or defence alike): [`World::machines_of`] of
    /// it the machines, their kinds the day's ([`World::wave_kinds_for`],
    /// the area's Bombers and Lancers on top) each at its own tier, and
    /// the rest the Manufacturers' people, each armed at the day's share
    /// ([`World::manufacturer_gear_tiers`]). `first` stands it about the
    /// station's rooms, else it arrives at the wave's airlock or gate
    /// ([`World::arrival_spots`]); `seeking` sends its machines looking
    /// for the crew, a reinforcement's. Their people are rolled off
    /// `seed`, the machines off the station's own. The room keeps which
    /// wave it holds ([`Residents::manufacturers_laid`]), so a room built
    /// afresh lays it again and nothing lays it twice.
    pub(super) fn lay_wave(
        &mut self,
        id: u32,
        n: u32,
        wave: u32,
        first: bool,
        seeking: bool,
        seed: u64,
    ) {
        self.lay_wave_with(id, n, wave, None, false, first, seeking, seed);
    }

    /// [`World::lay_wave`] of one landing of a mission's budget (October
    /// 2026, `run::Landing`): its base bodies, exactly its Bombers and
    /// Lancers on top in place of the area's, and every body of it marked
    /// unpaid where it is the trickle.
    pub(super) fn lay_wave_as(
        &mut self,
        id: u32,
        wave: u32,
        landing: &run::Landing,
        first: bool,
        seeking: bool,
        seed: u64,
    ) {
        let extras = Some((landing.bombers, landing.lancers));
        self.lay_wave_with(
            id,
            landing.base,
            wave,
            extras,
            landing.unpaid,
            first,
            seeking,
            seed,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn lay_wave_with(
        &mut self,
        id: u32,
        n: u32,
        wave: u32,
        extras: Option<(u32, u32)>,
        unpaid: bool,
        first: bool,
        seeking: bool,
        seed: u64,
    ) {
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let machines = self.machines_of(n, wave);
        let people = n - machines;
        let kinds = self.wave_kinds_with(machines, wave, extras);
        let total = people as usize + kinds.len();
        let placed = if first {
            let Some(residents) = &self.residents else {
                return;
            };
            let spots: Vec<bims::math::Vec2> = droidplan::spots_about(&station.design, total)
                .into_iter()
                .map(|(x, y)| residents.aboard.to_room(dvec2(x, y)))
                .collect();
            Some((spots, 0.0))
        } else {
            // A wave whose every way in is welded burns through the one
            // its turn names (`World::burn_through`).
            self.burn_through(&station, wave);
            self.arrival_spots(&station, total as u32, wave)
        };
        let Some((spots, facing)) = placed else {
            return;
        };
        let (theirs, rest) = spots.split_at((people as usize).min(spots.len()));
        let mut droids = self.build_wave(kinds, wave, rest, facing, station.map_seed);
        for d in &mut droids {
            d.seeking = seeking;
            d.unpaid = unpaid;
        }
        // What each of the wave pays in experience (October 2026) — bar a
        // mission with a budget, priced once when it opened.
        if self.run.budget.is_none() {
            self.price_the_wave(id, total as u32);
        }
        let bims_before = self
            .residents
            .as_ref()
            .map_or(0, |r| r.aboard.room.crew_count() as usize);
        self.stand_people(people, theirs, seed);
        if let Some(residents) = &mut self.residents {
            if unpaid {
                for who in bims_before..residents.aboard.room.crew_count() as usize {
                    residents.aboard.room.set_unpaid(who);
                }
            }
            residents
                .aboard
                .room
                .adopt_droids(droids, bims::math::Vec2::ZERO);
            residents.aboard.crew = residents.aboard.room.body_count();
            residents.manufacturers_laid = wave;
        }
    }

    /// A wave landing on a site the crew are defending: [`World::lay_wave`]
    /// at the wave's airlock or gate, its people off the site's own seed
    /// for the wave with a salt of its own (task 131), so a garrison and a
    /// defence on one day never roll alike.
    pub(super) fn lay_defense_wave(&mut self, id: u32, n: u32) {
        let Some(wave) = self.defense(id).map(|d| d.wave) else {
            return;
        };
        if wave == 0 || n == 0 {
            return;
        }
        let seed = self.garrison_seed(id, wave) ^ DEFENSE_SALT;
        self.lay_wave(id, n, wave, false, false, seed);
    }

    /// [`World::lay_defense_wave`] of one landing of a mission's budget.
    pub(super) fn lay_defense_wave_as(&mut self, id: u32, landing: &run::Landing) {
        let Some(wave) = self.defense(id).map(|d| d.wave) else {
            return;
        };
        if wave == 0 || landing.base == 0 {
            return;
        }
        let seed = self.garrison_seed(id, wave) ^ DEFENSE_SALT;
        self.lay_wave_as(id, wave, landing, false, false, seed);
    }

    /// The wave a held site has aboard laid: [`World::lay_wave`], wave one
    /// about the station's rooms and a reinforcement at its airlock,
    /// looking for the crew.
    pub(super) fn lay_held_wave(&mut self, id: u32, n: u32, wave: u32) {
        let seed = self.garrison_seed(id, wave);
        self.lay_wave(id, n, wave, wave == 1, wave > 1, seed);
    }

    /// [`World::lay_held_wave`] of one landing of a mission's budget.
    pub(super) fn lay_held_wave_as(&mut self, id: u32, wave: u32, landing: &run::Landing) {
        let seed = self.garrison_seed(id, wave);
        self.lay_wave_as(id, wave, landing, wave == 1, wave > 1, seed);
    }

    /// `people` of the Manufacturers' people stood on the residents' deck
    /// at `spots`, each armed at the day's share. A body index past the
    /// Bims is a machine's, so they go in after the Bims and before any
    /// machine: a wave laid with machines still standing (an Area defend's
    /// waves stack, October 2026) moves every machine's body index on by
    /// as many, and what the world keeps a body is moved on with it.
    pub(super) fn stand_people(&mut self, people: u32, spots: &[bims::math::Vec2], seed: u64) {
        if people == 0 {
            return;
        }
        let geared = self.manufacturer_gear_tiers(people);
        let spot = |i: usize| spots.get(i).copied().unwrap_or(bims::math::Vec2::ZERO);
        let stagger = |i: usize| bims::game::PLAN_EVERY * (i as f32) / (people.max(1) as f32);
        // The crew's relics on every body laid (*Black Market*).
        let toughen = self.enemy_health_factor();
        let Some(residents) = &mut self.residents else {
            return;
        };
        let room = &residents.aboard.room;
        let bims_before = room.crew_count() as usize;
        if bims_before < residents.down.len() {
            for _ in 0..people {
                residents.down.insert(bims_before, false);
                residents.xp_down.insert(bims_before, false);
                residents.last_hit_by.insert(bims_before, None);
                residents.grave.insert(bims_before, false);
                residents.defender.insert(bims_before, false);
            }
        }
        let room = &mut residents.aboard.room;
        for i in 0..people as usize {
            let own = worldgen::rng::mix(seed ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            // The room's pieces are its own; a thousand a body clear of
            // any other body's.
            let pieces = 10_000 + 1_000 * room.crew_count();
            let gear = manufacturer::gear(geared.get(i).copied().flatten(), own, pieces);
            let who = room.enlist_manufacturer(spot(i), gear, own, stagger(i));
            if let Some(factor) = toughen {
                // The bar's extra rides where a crew member's level
                // does, so an item picked up keeps it.
                room.set_level_health(who, room.max_health(who) * (factor - 1.0));
            }
        }
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
    pub(super) fn probe_trip(&mut self, site: run::Site) -> Option<()> {
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

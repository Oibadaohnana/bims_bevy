//! The world's half of the Machine Heart (feature 108): the fortress laid
//! into the origin's system, the core, the conduits and the fabricators put
//! on its deck, the phase read off the room every step and told back to
//! it, the machines built, the win — and what the map says about it on
//! arrival. The types and the rules are `crate::heart`; this is where they
//! meet the world's private fields, a child module of `world` the way
//! `mission.rs` and `relics.rs` are.

use super::*;
use crate::heart::{self, HeartFight, HeartPhase, HeartPreview};
use bims::droid::{Droid, DroidKind};

/// The Machine Heart's fight as it stands, for the strip along the top of
/// the screen: the phase, what the core has left of what it had, and how
/// many of the conduits and the fabricators are still standing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HeartStatus {
    pub station: u32,
    pub phase: HeartPhase,
    pub core_health: f32,
    pub core_max: f32,
    pub conduits_left: u32,
    pub conduits: u32,
    pub fabricators_left: u32,
}

/// The run in numbers, for the victory screen (feature 108): the days the
/// world clock ran — which only travel moves — the sites cleared of
/// machines, the systems liberated (their jammer cleared), the machines
/// destroyed, the deaths, and the crew's relics.
#[derive(Clone, PartialEq, Debug)]
pub struct RunSummary {
    pub days: f64,
    pub sites_cleared: u32,
    pub systems_liberated: u32,
    pub machines_destroyed: u32,
    pub deaths: u64,
    pub relics: Vec<crate::relic::Relic>,
}

impl World {
    /// The run in numbers (feature 108): what the victory screen says.
    pub fn run_summary(&self) -> RunSummary {
        RunSummary {
            days: self.clock_minutes / time::DAY,
            sites_cleared: self.run.sites_cleared,
            systems_liberated: self.run.systems_liberated,
            machines_destroyed: self.run.machines_destroyed,
            deaths: self.run.deaths,
            relics: self.run.relics.held.clone(),
        }
    }

    // --- the fortress, as a station ---------------------------------------------

    /// The Machine Heart's fortress in this system, if it is the origin's
    /// and the fortress has been laid.
    pub fn heart_station(&self) -> Option<u32> {
        let id = heart::heart_id(self.star_id);
        self.stations.iter().any(|s| s.id == id).then_some(id)
    }

    /// The fortress put into this system, or taken out of it: **the one
    /// place either happens**, called at the end of
    /// [`World::settle_jammer`] — so wherever a system is settled: the
    /// start, a jump, every load and the step a system falls. The origin's
    /// system has one; no other system does. Never saved: stripped and
    /// laid again off the star's own stream every time, the way a derived
    /// jammer is, and charted the moment it is laid — the crisis is no
    /// secret, and nor is where it began.
    pub(super) fn settle_heart(&mut self) {
        let id = heart::heart_id(self.star_id);
        self.system.stations.retain(|s| !heart::is_heart(s.id));
        self.stations.retain(|s| !heart::is_heart(s.id));
        self.discovered
            .retain(|n| !matches!(n, Node::Station(id) if heart::is_heart(*id)));
        if self.star_id != self.droid_origin {
            return;
        }
        let blueprint = heart::blueprint(&self.system, self.galaxy_seed, self.star_id);
        let at = blueprint.position;
        self.system.stations.push(blueprint.clone());
        // Its id is past the generator's and past a derived jammer's, so
        // the end of the list is id order.
        self.stations.push(Station::build(&blueprint, at));
        self.discovered.push(Node::Station(id));
        self.discovered.sort_by_key(node_key);
        self.discovered.dedup();
    }

    /// Whether the crew have seen the machines' origin (feature 108): been
    /// in its system or in any system a lane from it. What puts the origin
    /// and its fortress on the galaxy chart.
    pub fn origin_seen(&self) -> bool {
        // Off the crisis's own hop table, which is kept: the chart asks
        // every frame, and the galaxy is a thousand stars to generate.
        self.stars_visited()
            .into_iter()
            .any(|star| self.hops_from_origin(star) <= 1)
    }

    // --- the fight ----------------------------------------------------------------

    /// The fight at the fortress alongside, if the crew are at it and its
    /// count has been settled.
    pub fn heart_fight(&self) -> Option<&HeartFight> {
        let id = self.residents.as_ref()?.station;
        self.infestation(id)?.heart.as_ref()
    }

    /// The fight at the fortress alongside as the strip along the top
    /// shows it (feature 108), read off the room and the world: `None`
    /// away from it, and before anything has been laid.
    pub fn heart_status(&self) -> Option<HeartStatus> {
        let residents = self.residents.as_ref()?;
        let fight = self.infestation(residents.station)?.heart.as_ref()?;
        if !fight.laid {
            return None;
        }
        let room = &residents.aboard.room;
        let mut status = HeartStatus {
            station: residents.station,
            phase: fight.phase,
            core_health: 0.0,
            core_max: fight.core_health,
            conduits_left: 0,
            conduits: fight.conduits,
            fabricators_left: 0,
        };
        for d in room.droids() {
            match d.kind {
                DroidKind::Core => {
                    status.core_health = if d.destroyed {
                        0.0
                    } else {
                        d.body.health(bims::droid::DroidPart::Chassis)
                    };
                    status.core_max = d.body.max(bims::droid::DroidPart::Chassis);
                }
                DroidKind::Conduit if !d.destroyed => status.conduits_left += 1,
                DroidKind::Fabricator if !d.destroyed => status.fabricators_left += 1,
                _ => {}
            }
        }
        Some(status)
    }

    /// The Machine Heart's machines for the fortress `station`, built
    /// where [`heart::places`] puts them — the core first, then the
    /// fabricators, then the conduits — at tier three, in the residents'
    /// room's own units. A conduit laid in a fight already past its seal is
    /// laid a wreck, so a room built afresh mid-fight is the fight it was.
    fn heart_machines(&self, station: &Station, fight: &HeartFight) -> Vec<Droid> {
        let Some(residents) = &self.residents else {
            return Vec::new();
        };
        let places = heart::places(&station.design, fight.conduits, data::HEART_FABRICATORS);
        let t = shipdesign::TILE as f64;
        let at = |(x, y): (u32, u32)| {
            residents
                .aboard
                .to_room(dvec2((x as f64 + 0.5) * t, (y as f64 + 0.5) * t))
        };
        let core_at = at(places.core);
        let tier = Tier::Three;
        let seed = station.map_seed;
        let mut out = Vec::new();
        let mut core = Droid::structure(
            DroidKind::Core,
            tier,
            fight.core_health,
            core_at,
            bims::math::vec2(1.0, 0.0),
            seed ^ 0x_C0DE,
        );
        // Sealed from the first step it stands, not from the first step
        // the world tells it so.
        core.heart.sealed = fight.phase == HeartPhase::Sealed;
        out.push(core);
        for (i, &tile) in places.fabricators.iter().enumerate() {
            let p = at(tile);
            // Facing out from the core: what it builds leaves by the bay
            // in its front, away from the middle.
            let out_dir = (p - core_at).normalize_or_zero();
            out.push(Droid::structure(
                DroidKind::Fabricator,
                tier,
                data::HEART_FABRICATOR_HEALTH,
                p,
                out_dir,
                seed ^ 0x_FAB0 ^ (i as u64) << 12,
            ));
        }
        for (i, &tile) in places.conduits.iter().enumerate() {
            let p = at(tile);
            let mut d = Droid::structure(
                DroidKind::Conduit,
                tier,
                data::HEART_CONDUIT_HEALTH,
                p,
                (core_at - p).normalize_or_zero(),
                seed ^ 0x_C0D0 ^ (i as u64) << 12,
            );
            if fight.phase != HeartPhase::Sealed {
                d.destroy();
            }
            out.push(d);
        }
        out
    }

    /// The fortress's fight begun, the first time the crew dock at it
    /// (feature 108): its count settled off how many players there are,
    /// beside the waves' (`Infestation::settle`). Nothing the second time.
    pub(super) fn settle_heart_fight(&mut self, id: u32) {
        if !heart::is_heart(id) {
            return;
        }
        let players = self.players();
        if let Some(it) = self.infestation_mut(id)
            && it.heart.is_none()
        {
            it.heart = Some(HeartFight::new(players));
        }
    }

    /// The Machine Heart's machines to go on a fresh deck ahead of the wave
    /// (feature 108) — `None` anywhere but the fortress, and nothing once
    /// they are on it. Called by `settle_droids`, which lays them first so
    /// they keep the front of the list.
    pub(super) fn heart_machines_to_lay(&mut self, station: &Station) -> Vec<Droid> {
        let Some(fight) = self.infestation(station.id).and_then(|it| it.heart.clone()) else {
            return Vec::new();
        };
        let machines = self.heart_machines(station, &fight);
        // A conduit laid a wreck was shot down before and has had its
        // wave; it brings no other.
        let wrecks = machines
            .iter()
            .filter(|d| d.kind == DroidKind::Conduit && d.destroyed)
            .count() as u32;
        if let Some(it) = self.infestation_mut(station.id)
            && let Some(f) = it.heart.as_mut()
        {
            f.laid = true;
            f.links_down = f.links_down.max(wrecks);
        }
        machines
    }

    /// The Machine Heart's step (feature 108), after the rooms have
    /// stepped and before the loss is checked: the phase read off the room
    /// — the last conduit down exposes the core, the core under
    /// [`data::HEART_OVERLOAD_FRACTION`] of its health overloads it, the
    /// core at nothing is the run won — the machines the fabricators are
    /// due to build put on the deck, and what the phase allows told to the
    /// core for the room's next step.
    pub(super) fn heart_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(id) = self.residents.as_ref().map(|r| r.station) else {
            return;
        };
        let Some(fight) = self.infestation(id).and_then(|it| it.heart.clone()) else {
            return;
        };
        if !fight.laid || fight.phase == HeartPhase::Destroyed {
            return;
        }
        let Some(status) = self.heart_status() else {
            return;
        };
        let now = self.run.mission_steps;
        // Every conduit shot down since the last step brings a wave and its
        // Guardians in by the airlocks, on top of whatever is still
        // standing: one Guardian for the first, two for the second, and so
        // on (October 2026).
        let down = self.residents.as_ref().map_or(0, |r| {
            r.aboard
                .room
                .droids()
                .iter()
                .filter(|d| d.kind == DroidKind::Conduit && d.destroyed)
                .count() as u32
        });
        let mut links_down = fight.links_down;
        while links_down < down {
            links_down += 1;
            self.conduit_wave(id, links_down, events);
        }
        let mut phase = fight.phase;
        let mut next_build = fight.next_build;
        if phase == HeartPhase::Sealed && status.conduits_left == 0 {
            phase = HeartPhase::Exposed;
            next_build = Some(now + data::HEART_FABRICATOR_INTERVAL);
            events.push(WorldEvent::HeartExposed { station: id });
        }
        if phase == HeartPhase::Exposed
            && status.core_health < status.core_max * data::HEART_OVERLOAD_FRACTION
        {
            phase = HeartPhase::Overload;
            // The next build comes on the faster clock, never later.
            let sooner = now + self.build_interval(HeartPhase::Overload);
            next_build = Some(next_build.map_or(sooner, |due| due.min(sooner)));
            events.push(WorldEvent::HeartOverload { station: id });
        }
        if status.core_health <= 0.0 {
            phase = HeartPhase::Destroyed;
            next_build = None;
        }
        let mut built = fight.built;
        if matches!(phase, HeartPhase::Exposed | HeartPhase::Overload)
            && next_build.is_some_and(|due| now >= due)
        {
            built += self.fabricate(id, fight.built);
            next_build = Some(now + self.build_interval(phase));
        }
        if let Some(it) = self.infestation_mut(id) {
            if let Some(f) = it.heart.as_mut() {
                f.phase = phase;
                f.next_build = next_build;
                f.built = built;
                f.links_down = links_down;
            }
            if phase == HeartPhase::Destroyed && !it.cleared {
                // The last machine that matters is down: the station is
                // won back, and the run with it.
                it.cleared = true;
                it.waves_left = 0;
                it.next_wave = None;
                events.push(WorldEvent::HeartDestroyed { station: id });
                events.push(WorldEvent::DroidStationCleared { station: id });
                self.run_won(events);
            }
        }
        self.tell_the_heart(phase);
    }

    /// The wave and the Guardians for the `link`-th conduit shot down
    /// (October 2026, the player's: "for the heart everytime you destroy a
    /// link a wave should spawn"): a wave of the day's size and kinds
    /// ([`World::droid_wave_size`], [`World::wave_kinds_for`] — the
    /// tier-three area's Bombers and Lancers on top), every machine at
    /// tier three, and [`heart::guardians_for_link`] Guardians with it —
    /// one for the first, two for the second, and so on — all in by the
    /// next airlock in turn and looking for the crew, like a
    /// reinforcement, **added** to the deck, never clearing it, and
    /// counted as a wave of the station's (`Infestation::wave`). The
    /// fortress has no waves by the clock.
    fn conduit_wave(&mut self, id: u32, link: u32, events: &mut Vec<WorldEvent>) {
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let Some(wave) = self.infestation_mut(id).map(|it| {
            it.wave += 1;
            it.wave
        }) else {
            return;
        };
        let mut kinds = self.wave_kinds_for(self.droid_wave_size(), wave);
        let guardians = heart::guardians_for_link(link);
        kinds.extend(std::iter::repeat_n(DroidKind::Guardian, guardians as usize));
        let Some((spots, facing)) = self.arrival_spots(&station, kinds.len() as u32, wave) else {
            return;
        };
        let mut arriving = self.build_wave(kinds, wave, &spots, facing, station.map_seed ^ 0x_6A2D);
        for d in &mut arriving {
            d.seeking = true;
        }
        if let Some(residents) = &mut self.residents {
            residents
                .aboard
                .room
                .adopt_droids(arriving, bims::math::Vec2::ZERO);
            residents.aboard.crew = residents.aboard.room.body_count();
        }
        events.push(WorldEvent::DroidReinforcements { station: id });
    }

    /// How long between two builds of the fabricators, in steps of the
    /// mission clock: [`data::HEART_FABRICATOR_INTERVAL`], divided by
    /// [`data::HEART_OVERLOAD_SPAWN_FACTOR`] in the overload.
    fn build_interval(&self, phase: HeartPhase) -> u64 {
        if phase == HeartPhase::Overload {
            (data::HEART_FABRICATOR_INTERVAL / data::HEART_OVERLOAD_SPAWN_FACTOR.max(1)).max(1)
        } else {
            data::HEART_FABRICATOR_INTERVAL
        }
    }

    /// One machine built at every fabricator still standing and not
    /// stunned (a Stun Shot holds one's build over, October 2026), out of its
    /// bay, at tier three and with the wave aboard: a Trooper, a Husk, a
    /// Trooper, a Warden and round again, by how many have been built so
    /// every client builds the same. How many were built.
    fn fabricate(&mut self, id: u32, built: u32) -> u32 {
        let wave = self.infestation(id).map_or(1, |it| it.wave);
        let toughen = self.enemy_health_factor();
        let Some(residents) = self.residents.as_mut() else {
            return 0;
        };
        let room = &mut residents.aboard.room;
        let bays: Vec<(usize, bims::math::Vec2, bims::math::Vec2)> = room
            .droids()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == DroidKind::Fabricator && !d.destroyed && !d.is_stunned())
            .map(|(i, d)| (i, d.pos, d.facing()))
            .collect();
        const KINDS: [DroidKind; 4] = [
            DroidKind::Trooper,
            DroidKind::Husk,
            DroidKind::Trooper,
            DroidKind::Warden,
        ];
        let mut made = Vec::with_capacity(bays.len());
        for (n, &(i, at, facing)) in bays.iter().enumerate() {
            let k = built as usize + n;
            let kind = KINDS[k % KINDS.len()];
            let spot = at + facing * (shipdesign::TILE as f32 * 1.2);
            let mut droid = Droid::new(
                kind,
                Tier::Three,
                k,
                wave,
                spot,
                facing.angle(),
                u64::from(id) << 20 ^ k as u64 ^ 0x_FAB,
            );
            if let Some(factor) = toughen {
                droid.body.toughen(factor);
            }
            droid.plan_wait = 0.0;
            made.push(droid);
            if let Some(h) = room.heart_state_mut(i) {
                h.made = bims::droid::HeartState::MADE_FLASH;
            }
        }
        let count = made.len() as u32;
        room.adopt_droids(made, bims::math::Vec2::ZERO);
        residents.aboard.crew = room.body_count();
        count
    }

    /// What the phase allows, told to the core and the fabricators for the
    /// room's next step: sealed or not, how many beams, how hard, how fast.
    fn tell_the_heart(&mut self, phase: HeartPhase) {
        let Some(residents) = self.residents.as_mut() else {
            return;
        };
        let room = &mut residents.aboard.room;
        let (sealed, emitters, pace) = match phase {
            HeartPhase::Sealed => (true, 0, 1.0),
            HeartPhase::Exposed => (false, 1, 1.0),
            HeartPhase::Overload => (false, 2, data::HEART_OVERLOAD_SWEEP_FACTOR),
            HeartPhase::Destroyed => (false, 0, 1.0),
        };
        for i in 0..room.droid_count() as usize {
            let kind = room.droid(i).map(|d| d.kind);
            let Some(h) = room.heart_state_mut(i) else {
                continue;
            };
            match kind {
                Some(DroidKind::Core) => {
                    h.sealed = sealed;
                    h.emitters = emitters;
                    h.damage = data::HEART_BEAM_DAMAGE_FACTOR;
                    h.pace = pace;
                    h.overload = phase == HeartPhase::Overload;
                }
                // A fabricator's lamp is lit while it builds.
                Some(DroidKind::Fabricator) => {
                    h.emitters =
                        u8::from(matches!(phase, HeartPhase::Exposed | HeartPhase::Overload));
                }
                _ => {}
            }
        }
    }

    // --- the map ------------------------------------------------------------------

    /// What the fortress would be on arrival (feature 108): the conduits,
    /// the core and the Guardians its conduits send, which the players
    /// decide and the clock does not (October 2026: no waves there) — the
    /// numbers the fight will be built with. `None` for any site but a
    /// fortress.
    pub fn heart_preview(&self, station: u32) -> Option<HeartPreview> {
        let star = heart::heart_star(station)?;
        if star != self.droid_origin {
            return None;
        }
        let players = self.players();
        let conduits = heart::conduits_for(players);
        Some(HeartPreview {
            conduits,
            core_health: heart::core_health_for(players),
            guardians: heart::guardians_for(conduits),
        })
    }

    // --- the probes -----------------------------------------------------------------

    /// A point of the residents' room on the crew's joined deck — its shift
    /// off, then the station's frame, as `visit` carries a shot across —
    /// for a probe that wants a crew member stood somewhere in the
    /// fortress. `None` while the rooms are not joined.
    pub fn residents_point_on_deck_for_probe(
        &self,
        p: bims::math::Vec2,
    ) -> Option<bims::math::Vec2> {
        let residents = self.residents.as_ref()?;
        let (origin, ex, ey) = self.aboard.station_frame?;
        let p = dvec2(p.x as f64, p.y as f64).sub(residents.aboard.offset);
        let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
        Some(bims::math::vec2(at.x as f32, at.y as f32))
    }

    /// The `heart` command (feature 108): the machines' origin put at the
    /// crew's own star, the fortress laid, every station of the system in
    /// the machines' hands and the ship docked at the fortress.
    pub fn heart_dock_for_probe(&mut self) -> bool {
        self.droid_origin = self.star_id;
        self.settle_crisis();
        let Some(id) = self.heart_station() else {
            return false;
        };
        self.infest_here_for_probe();
        // Docked again from the start, as `arena_dock_for_probe` docks:
        // the joined deck is the fortress's, and the residents' room is
        // opened on it.
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        self.run.site = Some(id);
        true
    }

    /// The fight at the fortress alongside wound on to `phase` (feature
    /// 108, `BIMS_HEART_PHASE`): every conduit destroyed for the exposed
    /// core, and for the overload the core's health put just under the
    /// fraction as well. False away from a laid fortress.
    pub fn set_heart_phase_for_probe(&mut self, phase: HeartPhase) -> bool {
        if phase == HeartPhase::Sealed {
            return self.heart_status().is_some();
        }
        // Wound on, not fought: the conduits bring no waves.
        let id = self.residents.as_ref().map(|r| r.station);
        if let Some(f) = id
            .and_then(|id| self.infestation_mut(id))
            .and_then(|it| it.heart.as_mut())
        {
            f.links_down = f.conduits;
        }
        let Some(residents) = self.residents.as_mut() else {
            return false;
        };
        let room = &mut residents.aboard.room;
        for i in 0..room.droid_count() as usize {
            let Some(d) = room.droid(i) else {
                continue;
            };
            match d.kind {
                DroidKind::Conduit if !d.destroyed => {
                    room.strike_droid(i, bims::droid::DroidPart::Chassis, 1e9);
                }
                DroidKind::Core if phase == HeartPhase::Overload => {
                    let max = d.body.max(bims::droid::DroidPart::Chassis);
                    let now = d.body.health(bims::droid::DroidPart::Chassis);
                    let want = max * data::HEART_OVERLOAD_FRACTION * 0.98;
                    // Unsealed for the one blow, so it lands.
                    if let Some(h) = room.heart_state_mut(i) {
                        h.sealed = false;
                    }
                    room.strike_droid(i, bims::droid::DroidPart::Chassis, (now - want).max(0.0));
                }
                _ => {}
            }
        }
        true
    }
}

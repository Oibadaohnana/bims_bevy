//! The defences' missions of October 2026 (the player's D1, D3 and D5),
//! their states `crate::objective::Guard` on the site's `Defense`:
//!
//! - **Bomb disposal**: [`data::BOMB_CHARGES`] charges (one more from the
//!   third player) spread over the site from its port outward, all on one
//!   timer of [`data::BOMB_STEPS`] (three minutes) from the defence's
//!   start. Each is [`data::DEFUSE_SECONDS`] of hands (the Use key; an
//!   engineer's two a step); the machines make for the charges. Every one
//!   defused: the waves stop and the deck cleared is the site held. The
//!   timer out with a charge left: **the station goes up, and the run
//!   with it**. Waves every [`data::BOMB_WAVE_STEPS`], stacking.
//! - **Hold the doors**: the site laid out with a vault (`Feature::Vault`):
//!   an antechamber with two outer doors, a partition door, the core and
//!   in it the site's **commander** — a crew bot in the commander's kit,
//!   posted there. Every door is sealed (`Game::seal_door`); the machines
//!   make for the nearer outer door, then the inner one, then him, and a
//!   door they stand at for [`data::DOOR_BREAK_STEPS`] unbroken is broken
//!   in (the count starts again whenever nobody of theirs is at it). Waves
//!   every [`data::DOORS_WAVE_STEPS`] fixed, stacking, each three quarters
//!   of the day's and one more a player, for [`data::DOORS_STEPS`]; then
//!   the Republic's soldiers come instead, a wave's worth every as often,
//!   at the day's tier, until nothing of the enemy stands — and the deck
//!   clear is the site held. **The commander down is the run lost.**
//! - **Protect the commander**: the site's commander in **his room** — a
//!   station laid out with one (`Feature::Command`: the room nearest the
//!   port, a desk of crates across it, so the waves come in far from it), a town's watch house
//!   (`surface::WATCH_ROOM`) — posted at the tile of it farthest from its
//!   doors, where he stays the whole fight and fights from (the player's:
//!   "he will just stay for the whole fight and defend himself"), every
//!   enemy making for him, for [`data::CHIEF_STEPS`] (two minutes); waves
//!   every [`data::CHIEF_WAVE_STEPS`]. Downed he is revived as any of the
//!   crew; **dead, the site falls**. A town's defence that is no Area
//!   defend is this one.
//!
//! Either commander holds his post: the crew's room is told it as his
//! objective (`say_the_commander_s_post`), which he walks back to and
//! shoots from once the alarm has dropped every post, and he is posted
//! there again whenever an order took him more than [`HOLD_SLACK`] tiles
//! off it.
//!
//! The commander is gone at the mission's end, as a prisoner never freed
//! is. A child of `crate::world`, as `mission.rs` is. **Nothing here draws
//! from a stream.**

use super::*;
use crate::objective::{Bombs, Chief, Doors, Gate, Guard};
use crate::run::Mission;

/// The room's deploy code a charge's defusing goes as.
pub const DEFUSE_CODE: u32 = 104;
/// How long the room's errand runs: longer than any defusing.
const ERRAND_MINUTES: f32 = 600.0;
/// How far off his post a commander may stand, in tiles, before he is
/// posted there again — and how often that is asked, in steps.
const HOLD_SLACK: f32 = 2.0;
const HOLD_EVERY: u64 = 60;

impl World {
    /// The guard of the defence at the site the crew are at, with its id.
    pub(super) fn guard_here(&self) -> Option<(u32, Guard)> {
        let id = self.ship.state.station()?;
        let d = self.defense(id)?;
        if d.over() {
            return None;
        }
        Some((id, d.guard.clone()?))
    }

    fn set_guard(&mut self, id: u32, g: Guard) {
        if let Some(d) = self.defense_mut(id) {
            d.guard = Some(g);
        }
    }

    /// The guard laid as the defence at `id` begins, where its mission is
    /// one of these three — the commander enlisted on the crew's deck.
    pub(super) fn begin_guard(&mut self, id: u32) -> Option<Guard> {
        let mission = self.mission_here(id);
        if !matches!(mission, Mission::Bombs | Mission::Doors | Mission::Chief) {
            return None;
        }
        let station = self.station(id)?.clone();
        let design = &station.design;
        let from = match station.port() {
            Some(port) => dvec2(port.centre.0, port.centre.1),
            None => dvec2(0.0, 0.0),
        };
        match mission {
            Mission::Bombs => {
                let n = data::BOMB_CHARGES + u32::from(self.players() >= 3);
                let charges = attacks::spots(design, None, n as usize, from, false);
                if charges.is_empty() {
                    return None;
                }
                Some(Guard::Bombs(Bombs {
                    work: vec![0; charges.len()],
                    defused: vec![false; charges.len()],
                    charges,
                    left: data::BOMB_STEPS,
                }))
            }
            Mission::Doors => {
                let fitted = station
                    .fitted
                    .clone()
                    .filter(|f| f.feature == crate::stationgen::Feature::Vault)?;
                let [ante, core] = [*fitted.rooms.first()?, *fitted.rooms.get(1)?];
                let gate = |door: (u32, u32), from_room: [u32; 4]| -> Option<Gate> {
                    let outside = outside_of(design, door, from_room)?;
                    Some(Gate {
                        door,
                        outside,
                        held: 0,
                        breached: false,
                    })
                };
                // The outer doors are worked from outside the vault, the
                // inner one from the antechamber's side.
                let whole = [
                    ante[0].min(core[0]),
                    ante[1].min(core[1]),
                    ante[2].max(core[2]),
                    ante[3].max(core[3]),
                ];
                let outer: Vec<Gate> = fitted
                    .doors
                    .iter()
                    .take(2)
                    .filter_map(|&d| gate(d, whole))
                    .collect();
                let inner = gate(*fitted.doors.get(2)?, core)?;
                let core_tile = attacks::spots(design, Some(core), 1, from, false)
                    .first()
                    .copied()?;
                let vip = self.enlist_commander(core_tile)?;
                Some(Guard::Doors(Doors {
                    outer,
                    inner,
                    core: core_tile,
                    vip,
                    left: data::DOORS_STEPS,
                    soldiers_at: 0,
                }))
            }
            Mission::Chief => {
                // His room: the station's built for him, a town's watch
                // house; a station that could not be built so has him
                // at the open tile nearest the port.
                let room = station
                    .fitted
                    .as_ref()
                    .filter(|f| f.feature == crate::stationgen::Feature::Command)
                    .and_then(|f| f.rooms.first().copied())
                    .or_else(|| surface::surface_body(id).map(|_| surface::WATCH_ROOM));
                let post = match room {
                    Some(room) => command_post(design, room, from)?,
                    None => *attacks::spots(design, None, 1, from, true).first()?,
                };
                let vip = self.enlist_commander(post)?;
                Some(Guard::Chief(Chief {
                    vip,
                    room: room.unwrap_or([post.0, post.1, post.0, post.1]),
                    post,
                    left: data::CHIEF_STEPS,
                }))
            }
            _ => None,
        }
    }

    /// The site's commander on the crew's deck at a design tile: a crew bot
    /// with a rifle and armour at the zone's tier, posted there. His crew
    /// index.
    fn enlist_commander(&mut self, tile: (u32, u32)) -> Option<u32> {
        let t = shipdesign::TILE as f64;
        let p = self
            .aboard
            .from_station(dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t))?;
        let at = bims::math::vec2(p.x as f32, p.y as f32);
        let tier = self.zone_tier();
        let gear = bims::combat::Gear {
            weapon: Some(WeaponKind::AutoRifle.at(tier.max(WeaponKind::AutoRifle.min_tier()))),
            armour: Some(self.holdings.new_piece(ArmourKind::Armour, tier)),
            ..Default::default()
        };
        let seed =
            worldgen::rng::mix(self.galaxy_seed ^ u64::from(tile.0) << 20 ^ u64::from(tile.1));
        let who = self.aboard.room.enlist_reinforcement(at, gear, seed) as u32;
        self.crew_down.push(false);
        self.crew_locked.push(false);
        self.aboard.room.post_at(who as usize, at);
        self.size_for_the_reinforcements();
        Some(who)
    }

    /// Whether crew member `who` is the site's commander: kept off the
    /// crew's bots, in the commander's kit, gone at the mission's end.
    pub(crate) fn is_vip(&self, who: u32) -> bool {
        self.guard_here()
            .and_then(|(_, g)| g.vip())
            .is_some_and(|v| v == who)
    }

    /// The commander's post, said to the crew's room every step as his
    /// objective (by crew index; nobody else's), so the alarm dropping
    /// every post leaves him making for it and shooting from it. Nothing
    /// with no commander here.
    pub(super) fn say_the_commander_s_post(&mut self) {
        let held = self
            .guard_here()
            .filter(|_| self.aboard.is_joined() && self.in_mission())
            .and_then(|(_, g)| match g {
                Guard::Doors(d) => Some((d.vip, d.core)),
                Guard::Chief(c) => Some((c.vip, c.post)),
                Guard::Bombs(_) => None,
            });
        let mut spots = Vec::new();
        if let Some((vip, post)) = held
            && let Some(at) = self.tile_on_deck(post)
            && vip < self.aboard.room.crew_count()
        {
            spots = vec![None; self.aboard.room.crew_count() as usize];
            spots[vip as usize] = Some(at);
        }
        self.aboard.room.set_objectives(&spots);
    }

    /// The commander posted at his post again where an order took him
    /// more than [`HOLD_SLACK`] tiles off it, asked every [`HOLD_EVERY`]
    /// steps; never while he is down.
    fn hold_the_commander(&mut self, vip: u32, post: (u32, u32)) {
        let who = vip as usize;
        if self.run.mission_steps % HOLD_EVERY != 0
            || who >= self.aboard.room.crew_count() as usize
            || !self.aboard.room.is_alive(who)
            || self.aboard.room.is_down(who)
        {
            return;
        }
        let Some(at) = self.tile_on_deck(post) else {
            return;
        };
        let off = (self.aboard.room.bim_pos(who) - at).len();
        if off > HOLD_SLACK * shipdesign::TILE as f32 {
            self.aboard.room.post_at(who, at);
        }
    }

    /// A crew member dropped: the commander's index moved down past it.
    pub(super) fn vip_after_drop(&mut self, gone: u32) {
        for d in &mut self.defenses {
            if let Some(vip) = d.guard.as_mut().and_then(|g| g.vip_mut())
                && *vip > gone
            {
                *vip -= 1;
            }
        }
    }

    /// A mission's end: the commander gone with the site.
    pub(super) fn leave_the_commander(&mut self) {
        let Some(id) = self.ship.state.station().or(self.run.site) else {
            return;
        };
        let Some(vip) = self
            .defense(id)
            .and_then(|d| d.guard.as_ref())
            .and_then(|g| g.vip())
        else {
            return;
        };
        if vip >= self.players() && vip < self.aboard.crew_count() {
            self.drop_crew_member(vip);
        }
        // Nobody to drop twice.
        if let Some(d) = self.defense_mut(id)
            && let Some(Guard::Doors(_) | Guard::Chief(_)) = d.guard
        {
            d.guard = None;
        }
    }

    /// The wave a defence of these three lands, of `n` the day's: Hold the
    /// doors' three quarters of it and one more a player.
    pub(super) fn guard_wave_size(&self, id: u32, n: u32) -> u32 {
        match self.defense(id).and_then(|d| d.guard.as_ref()) {
            Some(Guard::Doors(_)) => (n * 3 / 4 + self.players()).max(1),
            _ => n,
        }
    }

    /// How far apart a defence of these three's waves land.
    pub(super) fn guard_gap(&self, id: u32) -> Option<u64> {
        match self.defense(id).and_then(|d| d.guard.as_ref())? {
            Guard::Bombs(_) => Some(data::BOMB_WAVE_STEPS),
            Guard::Doors(_) => Some(data::DOORS_WAVE_STEPS),
            Guard::Chief(_) => Some(data::CHIEF_WAVE_STEPS),
        }
    }

    /// The enemies standing in the residents' room within `reach` tiles of
    /// a design point of the site: the machines and their people up.
    fn enemies_near(&self, p: DVec2, reach: f32) -> u32 {
        let Some(residents) = self.residents.as_ref() else {
            return 0;
        };
        let at = residents.aboard.to_room(p);
        let reach = reach * shipdesign::TILE as f32;
        let room = &residents.aboard.room;
        let machines = (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .filter(|d| !d.destroyed && (d.pos - at).len() <= reach)
            .count();
        let people = (0..room.crew_count() as usize)
            .filter(|&w| {
                room.is_manufacturer(w)
                    && room.is_alive(w)
                    && !room.is_down(w)
                    && (room.body_pos(w) - at).len() <= reach
            })
            .count();
        (machines + people) as u32
    }

    /// Whether anything of the enemy stands on the residents' deck.
    fn enemy_standing_here(&self) -> bool {
        let Some(residents) = self.residents.as_ref() else {
            return false;
        };
        let room = &residents.aboard.room;
        (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .any(|d| !d.destroyed && !d.kind.is_structure())
            || (0..room.crew_count() as usize)
                .any(|w| room.is_manufacturer(w) && room.is_alive(w) && !room.is_down(w))
    }

    // --- the step ------------------------------------------------------------

    /// The three's step, after the defence's waves: the timers, the hands
    /// on a charge, the doors, the commander, the Republic's soldiers.
    pub(super) fn guard_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, g)) = self.guard_here() else {
            return;
        };
        if !self.aboard.is_joined() || !self.in_mission() {
            return;
        }
        // Nothing runs before the first wave is down on the deck.
        if self.defense(id).is_none_or(|d| d.wave == 0) {
            if let Guard::Doors(d) = &g {
                self.seal_the_gates(d);
            }
            return;
        }
        match g {
            Guard::Bombs(b) => self.bombs_step(id, b, events),
            Guard::Doors(d) => self.doors_step(id, d, events),
            Guard::Chief(c) => self.chief_step(id, c, events),
        }
    }

    fn bombs_step(&mut self, id: u32, mut b: Bombs, events: &mut Vec<WorldEvent>) {
        for i in 0..b.charges.len() {
            if b.defused[i] {
                continue;
            }
            let Some(at) = self.tile_on_deck(b.charges[i]) else {
                continue;
            };
            let (hands, work) = self.hands_on(DEFUSE_CODE, at);
            if work == 0 {
                continue;
            }
            b.work[i] += work;
            if b.work[i] >= data::DEFUSE_SECONDS * 60 {
                b.defused[i] = true;
                for who in hands {
                    self.aboard.room.end_deploy(who);
                }
                let left = b.defused.iter().filter(|&&d| !d).count() as u32;
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 20,
                    n: left,
                    who: u32::MAX,
                });
            }
        }
        if !b.all_defused() {
            b.left = b.left.saturating_sub(1);
            if b.left == 0 {
                // The station goes up, and the run with it.
                if let Some(d) = self.defense_mut(id) {
                    d.lost = true;
                }
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 21,
                    n: 0,
                    who: u32::MAX,
                });
                let crew = self.aboard.room.crew_count() as usize;
                for who in 0..crew {
                    if self.aboard.room.is_alive(who) {
                        self.aboard.room.kill_now(who);
                    }
                }
                if !self.run.won {
                    self.lost = true;
                }
            }
        }
        self.set_guard(id, Guard::Bombs(b));
    }

    /// The vault's doors sealed while they stand, in both rooms.
    fn seal_the_gates(&mut self, d: &Doors) {
        for gate in d.outer.iter().chain(std::iter::once(&d.inner)) {
            self.seal_door_at(gate.door, !gate.breached);
        }
    }

    /// A door of the site at its first design tile sealed shut, or let
    /// go, in both rooms.
    pub(super) fn seal_door_at(&mut self, door: (u32, u32), sealed: bool) {
        let t = shipdesign::TILE as f64;
        let in_design = dvec2((door.0 as f64 + 0.5) * t, (door.1 as f64 + 0.5) * t);
        if let Some(q) = self.aboard.from_station(in_design) {
            let at = bims::math::vec2(q.x as f32, q.y as f32);
            if let Some(i) = self.aboard.room.door_index_at(at) {
                self.aboard.room.seal_door(i, sealed);
                if !sealed {
                    self.aboard.room.order_door_now(i, bims::door::Order::Open);
                }
            }
        }
        if let Some(residents) = self.residents.as_mut() {
            let at = residents.aboard.to_room(in_design);
            if let Some(i) = residents.aboard.room.door_index_at(at) {
                residents.aboard.room.seal_door(i, sealed);
                if !sealed {
                    residents
                        .aboard
                        .room
                        .order_door_now(i, bims::door::Order::Open);
                }
            }
        }
    }

    fn doors_step(&mut self, id: u32, mut d: Doors, events: &mut Vec<WorldEvent>) {
        let t = shipdesign::TILE as f64;
        let mid = |tile: (u32, u32)| dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t);
        // The machines at a door: an unbroken count, from nought again
        // whenever nobody of theirs is at it.
        let outer_down = d.outer.iter().any(|g| g.breached);
        let mut broke = None;
        for (k, gate) in d.outer.iter_mut().enumerate() {
            if gate.breached {
                continue;
            }
            if self_enemies_at(self, mid(gate.outside)) {
                gate.held += 1;
            } else {
                gate.held = 0;
            }
            if gate.held >= data::DOOR_BREAK_STEPS {
                gate.breached = true;
                broke = Some(k as u32);
            }
        }
        if outer_down && !d.inner.breached {
            if self_enemies_at(self, mid(d.inner.outside)) {
                d.inner.held += 1;
            } else {
                d.inner.held = 0;
            }
            if d.inner.held >= data::DOOR_BREAK_STEPS {
                d.inner.breached = true;
                broke = Some(2);
            }
        }
        if let Some(k) = broke {
            events.push(WorldEvent::Objective {
                station: id,
                what: 22,
                n: k,
                who: u32::MAX,
            });
        }
        self.seal_the_gates(&d);
        self.hold_the_commander(d.vip, d.core);
        // The commander down: the run lost.
        let vip = d.vip as usize;
        if vip < self.aboard.room.crew_count() as usize
            && (!self.aboard.room.is_alive(vip) || self.aboard.room.is_down(vip))
        {
            if let Some(def) = self.defense_mut(id) {
                def.lost = true;
            }
            events.push(WorldEvent::Objective {
                station: id,
                what: 23,
                n: 0,
                who: d.vip,
            });
            if !self.run.won {
                self.lost = true;
            }
            self.set_guard(id, Guard::Doors(d));
            return;
        }
        // The hold's time, and the Republic's soldiers once it is up.
        if d.left > 0 {
            d.left -= 1;
            if d.left == 0 {
                d.soldiers_at = self.run.mission_steps;
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 24,
                    n: 0,
                    who: u32::MAX,
                });
            }
        } else if self.run.mission_steps >= d.soldiers_at && self.enemy_standing_here() {
            let n = self.landing_wave_size();
            self.call_the_republic(id, n);
            d.soldiers_at = self.run.mission_steps + data::DOORS_WAVE_STEPS;
            events.push(WorldEvent::Objective {
                station: id,
                what: 25,
                n,
                who: u32::MAX,
            });
        }
        self.set_guard(id, Guard::Doors(d));
    }

    /// `n` of the Republic's soldiers in at the site's airlock: the
    /// commanders' reinforcements' kind — a rifle and armour at the zone's
    /// tier, gone when they fall and at the mission's end.
    fn call_the_republic(&mut self, id: u32, n: u32) {
        let Some(port) = self.station(id).and_then(|s| s.port()) else {
            return;
        };
        let inside = droidplan::inside_of(&port, data::ASHORE_TILES + 1.0);
        let Some(p) = self.aboard.from_station(dvec2(inside.0, inside.1)) else {
            return;
        };
        let at = bims::math::vec2(p.x as f32, p.y as f32);
        let tile = shipdesign::TILE as f32;
        let mut spots = self.aboard.room.free_tiles_near(at, 4.0 * tile);
        if spots.is_empty() {
            spots.push(at);
        }
        let tier = self.zone_tier();
        for k in 0..n as usize {
            let gear = bims::combat::Gear {
                weapon: Some(WeaponKind::AutoRifle.at(tier.max(WeaponKind::AutoRifle.min_tier()))),
                armour: Some(self.holdings.new_piece(ArmourKind::Armour, tier)),
                ..Default::default()
            };
            let seed = worldgen::rng::mix(
                self.galaxy_seed ^ self.run.mission_steps ^ (k as u64 + 1) << 32 ^ 0x_5245_5055,
            );
            self.enlist_republic(0, spots[k % spots.len()], gear, seed, false);
        }
        self.size_for_the_reinforcements();
    }

    fn chief_step(&mut self, id: u32, mut c: Chief, events: &mut Vec<WorldEvent>) {
        let vip = c.vip as usize;
        if vip < self.aboard.room.crew_count() as usize && !self.aboard.room.is_alive(vip) {
            // Dead: the site falls.
            if let Some(def) = self.defense_mut(id) {
                def.lost = true;
            }
            self.run.pending_bounty = 0;
            events.push(WorldEvent::Objective {
                station: id,
                what: 26,
                n: 0,
                who: c.vip,
            });
            events.push(WorldEvent::TownFell { station: id });
            self.set_guard(id, Guard::Chief(c));
            return;
        }
        // He holds his room.
        self.hold_the_commander(c.vip, c.post);
        if c.left > 0 {
            c.left -= 1;
            if c.left == 0 {
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 27,
                    n: 0,
                    who: u32::MAX,
                });
            }
        }
        self.set_guard(id, Guard::Chief(c));
    }

    /// Where the enemies make for at these three: the nearest charge left;
    /// the nearer outer door, the inner one, then the commander; or the
    /// commander. A spot each by body index in the residents' room.
    pub(super) fn guard_spots(&self) -> Vec<Option<bims::math::Vec2>> {
        let Some((_, g)) = self.guard_here() else {
            return Vec::new();
        };
        let Some(residents) = self.residents.as_ref() else {
            return Vec::new();
        };
        let t = shipdesign::TILE as f64;
        let mid = |tile: (u32, u32)| dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t);
        let room = &residents.aboard.room;
        let vip_at = |vip: u32| -> Option<bims::math::Vec2> {
            let p = self.aboard.room.bim_pos(vip as usize);
            let q = self.aboard.to_station(dvec2(p.x as f64, p.y as f64))?;
            Some(residents.aboard.to_room(q))
        };
        let targets: Vec<bims::math::Vec2> = match &g {
            Guard::Bombs(b) => b
                .charges
                .iter()
                .zip(&b.defused)
                .filter(|(_, d)| !**d)
                .map(|(&c, _)| residents.aboard.to_room(mid(c)))
                .collect(),
            Guard::Doors(d) => {
                let open: Vec<&Gate> = d.outer.iter().filter(|g| !g.breached).collect();
                if d.outer.iter().any(|g| g.breached) {
                    if d.inner.breached {
                        vip_at(d.vip).into_iter().collect()
                    } else {
                        vec![residents.aboard.to_room(mid(d.inner.outside))]
                    }
                } else {
                    open.iter()
                        .map(|g| residents.aboard.to_room(mid(g.outside)))
                        .collect()
                }
            }
            Guard::Chief(c) => vip_at(c.vip).into_iter().collect(),
        };
        if targets.is_empty() {
            return Vec::new();
        }
        let bims = room.crew_count() as usize;
        (0..room.body_count() as usize)
            .map(|body| {
                let enemy = body >= bims || room.is_manufacturer(body);
                if !enemy {
                    return None;
                }
                let at = if body >= bims {
                    room.droid(body - bims).map(|d| d.pos)?
                } else {
                    room.body_pos(body)
                };
                let near = targets
                    .iter()
                    .copied()
                    .min_by(|a, b| (*a - at).len().total_cmp(&(*b - at).len()))?;
                Some(near + defense::enemy_spot(body) * (shipdesign::TILE as f32 * 0.5))
            })
            .collect()
    }

    /// What the Use key does at Bomb disposal: the nearest charge left in
    /// reach. `None` anywhere else.
    pub(super) fn can_defuse(&self, slot: u32) -> Option<usize> {
        let (_, Guard::Bombs(b)) = self.guard_here()? else {
            return None;
        };
        let me = self.aboard.room.bim_pos(slot as usize);
        let reach = attacks::USE_REACH * shipdesign::TILE as f32;
        b.charges
            .iter()
            .enumerate()
            .filter(|&(i, _)| !b.defused[i])
            .filter_map(|(i, &c)| Some((i, self.tile_on_deck(c)?)))
            .filter(|&(_, at)| (at - me).len() <= reach)
            .min_by(|a, b| (a.1 - me).len().total_cmp(&(b.1 - me).len()))
            .map(|(i, _)| i)
    }

    /// The defusing begun at charge `i`.
    pub(super) fn defuse(&mut self, slot: u32, i: usize) -> Result<(), Refusal> {
        let Some((_, Guard::Bombs(b))) = self.guard_here() else {
            return Err(Refusal::NothingToUse);
        };
        let at = self
            .tile_on_deck(b.charges[i])
            .ok_or(Refusal::NothingToUse)?;
        if self
            .aboard
            .room
            .deploy(slot as usize, at, DEFUSE_CODE, ERRAND_MINUTES)
        {
            Ok(())
        } else {
            Err(Refusal::NothingToUse)
        }
    }

    /// Why the run was lost to one of these, if it was: 1 the station
    /// gone up, 2 the vault's commander down.
    pub fn guard_failed(&self) -> Option<u32> {
        let id = self.ship.state.station()?;
        let d = self.defense(id)?;
        if !d.lost {
            return None;
        }
        match d.guard.as_ref()? {
            Guard::Bombs(b) if !b.all_defused() && b.left == 0 => Some(1),
            Guard::Doors(_) => Some(2),
            _ => None,
        }
    }

    /// The guard of the defence here, for the tests.
    pub fn guard_now(&self) -> Option<Guard> {
        self.guard_here().map(|(_, g)| g)
    }

    /// The guard's timer put at `steps`, for the tests.
    pub fn set_guard_time_for_probe(&mut self, steps: u64) {
        let Some((id, mut g)) = self.guard_here() else {
            return;
        };
        match &mut g {
            Guard::Bombs(b) => b.left = steps,
            Guard::Doors(d) => d.left = steps,
            Guard::Chief(c) => c.left = steps,
        }
        self.set_guard(id, g);
    }

    /// These three as the app draws and says them.
    pub(super) fn guard_look(&self) -> Option<ObjectiveLook> {
        let (_, g) = self.guard_here()?;
        let t = shipdesign::TILE as f64;
        let mid = |tile: (u32, u32)| dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t);
        let vip_mark = |vip: u32| {
            let p = self.aboard.room.bim_pos(vip as usize);
            let at = self.aboard.to_station(dvec2(p.x as f64, p.y as f64))?;
            let share = self.aboard.room.health(vip as usize)
                / self.aboard.room.max_health(vip as usize).max(1.0);
            Some(Mark {
                at,
                kind: MarkKind::Vip {
                    down: self.aboard.room.is_down(vip as usize),
                },
                progress: share,
            })
        };
        let seconds = |steps: u64| Some(steps as f32 / 60.0);
        let mut marks = Vec::new();
        let look = match g {
            Guard::Bombs(b) => {
                for (i, &c) in b.charges.iter().enumerate() {
                    marks.push(Mark {
                        at: mid(c),
                        kind: MarkKind::Charge {
                            defused: b.defused[i],
                        },
                        progress: (b.work[i] as f32 / (data::DEFUSE_SECONDS * 60) as f32).min(1.0),
                    });
                }
                ObjectiveLook {
                    mission: Mission::Bombs,
                    marks,
                    done: b.defused.iter().filter(|&&d| d).count() as u32,
                    total: b.charges.len() as u32,
                    phase: 0,
                    wave_in: None,
                    time_left: (!b.all_defused()).then(|| seconds(b.left)).flatten(),
                }
            }
            Guard::Doors(d) => {
                for gate in d.outer.iter().chain(std::iter::once(&d.inner)) {
                    marks.push(Mark {
                        at: mid(gate.door),
                        kind: MarkKind::Gate {
                            breached: gate.breached,
                        },
                        progress: (gate.held as f32 / data::DOOR_BREAK_STEPS as f32).min(1.0),
                    });
                }
                marks.extend(vip_mark(d.vip));
                let stage = if d.inner.breached {
                    2
                } else if d.outer.iter().any(|g| g.breached) {
                    1
                } else {
                    0
                };
                ObjectiveLook {
                    mission: Mission::Doors,
                    marks,
                    done: d.outer.iter().filter(|g| g.breached).count() as u32
                        + u32::from(d.inner.breached),
                    total: 3,
                    phase: if d.left == 0 { 3 } else { stage },
                    wave_in: None,
                    time_left: (d.left > 0).then(|| seconds(d.left)).flatten(),
                }
            }
            Guard::Chief(c) => {
                marks.extend(vip_mark(c.vip));
                ObjectiveLook {
                    mission: Mission::Chief,
                    marks,
                    done: 0,
                    total: 1,
                    phase: u32::from(c.left == 0),
                    wave_in: None,
                    time_left: (c.left > 0).then(|| seconds(c.left)).flatten(),
                }
            }
        };
        Some(look)
    }
}

/// Whether anything of the enemy stands at a design point of the site,
/// within a tile and a half.
fn self_enemies_at(world: &World, p: DVec2) -> bool {
    world.enemies_near(p, 1.5) > 0
}

/// Where a commander holds his room (inner tiles, inclusive): its open
/// tile — free deck with free deck all round — farthest from the nearest
/// of its doors (the doors with a tile on its ring), any free tile of it
/// where none is open; farthest from `from` with no door found. Ties to
/// the lower row, then column.
fn command_post(design: &ShipDesign, room: [u32; 4], from: DVec2) -> Option<(u32, u32)> {
    let t = shipdesign::TILE as f64;
    let mid = |x: f64, y: f64| dvec2((x + 0.5) * t, (y + 0.5) * t);
    let [x0, y0, x1, y1] = room;
    let on_ring = |(x, y): (u32, u32)| {
        let (x, y) = (x as i64, y as i64);
        let inside_ring = x >= x0 as i64 - 1
            && x <= x1 as i64 + 1
            && y >= y0 as i64 - 1
            && y <= y1 as i64 + 1;
        let inside = x >= x0 as i64 && x <= x1 as i64 && y >= y0 as i64 && y <= y1 as i64;
        inside_ring && !inside
    };
    let doors: Vec<DVec2> = design
        .parts
        .iter()
        .filter(|p| p.kind == shipdesign::PartKind::Door)
        .filter_map(|p| {
            let tiles = p.tiles();
            if !tiles.iter().any(|&t| on_ring(t)) {
                return None;
            }
            let n = tiles.len() as f64;
            let (sx, sy) = tiles
                .iter()
                .fold((0.0, 0.0), |(a, b), &(x, y)| (a + x as f64, b + y as f64));
            Some(mid(sx / n, sy / n))
        })
        .collect();
    let grid = design.grid();
    let free = |x: i64, y: i64| {
        x >= 0
            && y >= 0
            && grid.get(shipdesign::Layer::Floor, (x as i32, y as i32)) != 0
            && grid.get(shipdesign::Layer::Object, (x as i32, y as i32)) == 0
    };
    let open = |x: i64, y: i64| (-1..=1).all(|dx| (-1..=1).all(|dy| free(x + dx, y + dy)));
    let score = |x: u32, y: u32| {
        let p = mid(x as f64, y as f64);
        if doors.is_empty() {
            p.sub(from).length()
        } else {
            doors
                .iter()
                .map(|&d| p.sub(d).length())
                .fold(f64::INFINITY, f64::min)
        }
    };
    let best = |want: &dyn Fn(i64, i64) -> bool| {
        let mut best: Option<((u32, u32), f64)> = None;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !want(x as i64, y as i64) {
                    continue;
                }
                let s = score(x, y);
                if best.is_none_or(|(_, b)| s > b) {
                    best = Some(((x, y), s));
                }
            }
        }
        best.map(|(tile, _)| tile)
    };
    best(&open).or_else(|| best(&free))
}

/// The free tile beside a door's two tiles that is outside `room` (deck
/// tiles, inclusive): where it is worked or broken from.
fn outside_of(design: &ShipDesign, door: (u32, u32), room: [u32; 4]) -> Option<(u32, u32)> {
    let inside = |a: i32, b: i32| {
        a >= room[0] as i32 && a <= room[2] as i32 && b >= room[1] as i32 && b <= room[3] as i32
    };
    let free = |a: i32, b: i32| {
        let grid = design.grid();
        a >= 0
            && b >= 0
            && grid.get(shipdesign::Layer::Floor, (a, b)) != 0
            && grid.get(shipdesign::Layer::Object, (a, b)) == 0
    };
    let (x, y) = (door.0 as i32, door.1 as i32);
    [(x, y), (x + 1, y), (x, y + 1)]
        .into_iter()
        .flat_map(|(a, b)| [(a + 1, b), (a - 1, b), (a, b + 1), (a, b - 1)])
        .find(|&(a, b)| !inside(a, b) && free(a, b))
        .map(|(a, b)| (a as u32, b as u32))
}

//! **Sabotage** (October 2026, a mission the map shapes): an attack whose
//! fight is a charge — planted deep in the site, held, then run from.
//!
//! - **The charge** stands on the free deck tile farthest from the port
//!   whose eight neighbours are free deck too ([`charge_spot`]), and the
//!   **way out** is the airlock but the port farthest from it
//!   ([`extraction_of`]): the escape crosses the station, and the port —
//!   the way the crew came — is no way out. Laid when the crew first dock
//!   at a site whose mission it is ([`World::settle_sabotage`]); a site
//!   with no airlock but the port is a plain attack.
//! - **Plant**: the crew fight the site's waves as at any attack, but the
//!   last of them down clears nothing. `Command::Plant` (the Use key, V)
//!   puts the Bim's hands on the charge — the room's deploy errand of
//!   [`PLANT_CODE`], counted by the world as a weld is
//!   ([`World::settle_plants`]: a step a step, an engineer's two) — and at
//!   [`data::PLANT_SECONDS`] it is planted.
//! - **Hold** ([`data::SABOTAGE_HOLD_STEPS`], 45 s): the ship casts off
//!   (*Back to ship* is refused, `Refusal::ShipCastOff`), a wave lands at
//!   once and every [`data::SABOTAGE_WAVE_STEPS`] after it, the last down
//!   or not, and every enemy makes for the charge
//!   ([`World::sabotage_objectives`]). Each step every machine standing and
//!   every one of their people on its feet within
//!   [`data::DISARM_REACH_TILES`] of it counts a step of disarming (three
//!   at most): at [`data::DISARM_STEPS`] it is disarmed and **the run is
//!   lost**.
//! - **Escape** ([`data::SABOTAGE_ESCAPE_STEPS`], 75 s): the charge can no
//!   longer be disarmed and the waves come on, looking for the crew. When
//!   it blows, every crew member within [`data::EXTRACTION_REACH_TILES`]
//!   of the way out gets away — downed or not — and every other is left
//!   behind, dead; the site is cleared, every player's *Back to ship* is
//!   pressed for them and the departure takes the rest.
//!
//! The state is the `Infestation`'s (`droid::Sabotage`), saved, hashed and
//! put back with it. A child of `crate::world`, as `mission.rs` is.
//! **Nothing here draws from a stream.**

use super::*;
use crate::droid::{Sabotage, SabotagePhase};
use crate::run::Mission;
use shipdesign::Layer;

/// The room's deploy code the planting goes as: past every
/// `DeployKind`'s and the weld's; never laid.
pub const PLANT_CODE: u32 = 101;
/// How near the charge's tile the Bim has to stand, in tiles: under the
/// room's deploy reach, as a weld's is.
pub const PLANT_REACH: f32 = 1.75;
/// How long the room's errand runs: longer than any planting.
const ERRAND_MINUTES: f32 = 600.0;

/// A Sabotage as the app draws it: the charge's tile middle and the way
/// out — its doorway's middle and a unit step out — in the site's own
/// design units, the phase, how far the planting and the disarming have
/// got, nought to one, and the seconds the hold or the escape has left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SabotageLook {
    pub charge: DVec2,
    pub way_out: DVec2,
    pub way_out_out: DVec2,
    pub phase: SabotagePhase,
    pub planted: f32,
    pub disarmed: f32,
    pub seconds_left: f32,
}

/// The charge's tile: the free deck tile farthest from `from` whose eight
/// neighbours are free deck too — nothing standing there, a body able to
/// stand round it — the lower row, then column, on a tie.
pub fn charge_spot(design: &ShipDesign, from: (f64, f64)) -> Option<(u32, u32)> {
    let grid = design.grid();
    let t = shipdesign::TILE as f64;
    let free = |x: i32, y: i32| {
        grid.get(Layer::Floor, (x, y)) != 0 && grid.get(Layer::Object, (x, y)) == 0
    };
    let mut best: Option<((u32, u32), f64)> = None;
    for part in design.parts.iter().filter(|p| p.kind == PartKind::Floor) {
        let (x, y) = (part.origin.0 as i32, part.origin.1 as i32);
        if !(-1..=1).all(|dx| (-1..=1).all(|dy| free(x + dx, y + dy))) {
            continue;
        }
        let (cx, cy) = ((x as f64 + 0.5) * t, (y as f64 + 0.5) * t);
        let d = (cx - from.0).hypot(cy - from.1);
        let tile = (x as u32, y as u32);
        let better = match best {
            None => true,
            Some((at, far)) => {
                d > far + 1e-6 || ((d - far).abs() <= 1e-6 && (tile.1, tile.0) < (at.1, at.0))
            }
        };
        if better {
            best = Some((tile, d));
        }
    }
    best.map(|(tile, _)| tile)
}

/// The way out: the airlock but the port farthest from the charge, by its
/// index among the design's airlocks; the lower index on a tie. `None`
/// with no airlock but the port.
pub fn extraction_of(design: &ShipDesign, charge: (u32, u32)) -> Option<u32> {
    let t = shipdesign::TILE as f64;
    let (cx, cy) = ((charge.0 as f64 + 0.5) * t, (charge.1 as f64 + 0.5) * t);
    droidplan::airlocks(design)
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, p)| (i as u32, (p.centre.0 - cx).hypot(p.centre.1 - cy)))
        .fold(None, |best: Option<(u32, f64)>, (i, d)| match best {
            Some((_, far)) if far + 1e-6 >= d => best,
            _ => Some((i, d)),
        })
        .map(|(i, _)| i)
}

impl World {
    /// The Sabotage of the site alongside, with its id.
    fn sabotage_here(&self) -> Option<(u32, Sabotage)> {
        let id = self.ship.state.alongside()?;
        let s = self.infestation(id)?.sabotage?;
        Some((id, s))
    }

    /// Whether the site alongside is a Sabotage whose charge is armed —
    /// held or run from — when the ship has cast off and *Back to ship* is
    /// refused.
    pub fn ship_cast_off(&self) -> bool {
        self.sabotage_here()
            .is_some_and(|(_, s)| matches!(s.phase, SabotagePhase::Hold | SabotagePhase::Escape))
    }

    /// Whether the run was lost to the charge disarmed.
    pub fn sabotage_failed(&self) -> bool {
        self.sabotage_here()
            .is_some_and(|(_, s)| s.phase == SabotagePhase::Disarmed)
    }

    /// The Sabotage of the site alongside as the app draws it.
    pub fn sabotage_look(&self) -> Option<SabotageLook> {
        let (id, s) = self.sabotage_here()?;
        let station = self.station(id)?;
        let port = *droidplan::airlocks(&station.design).get(s.extraction as usize)?;
        let t = shipdesign::TILE as f64;
        let left = match s.phase {
            SabotagePhase::Hold | SabotagePhase::Escape => s.left as f32 / 60.0,
            _ => 0.0,
        };
        Some(SabotageLook {
            charge: dvec2((s.charge.0 as f64 + 0.5) * t, (s.charge.1 as f64 + 0.5) * t),
            way_out: dvec2(port.centre.0, port.centre.1),
            way_out_out: dvec2(port.outward.0 as f64, port.outward.1 as f64),
            phase: s.phase,
            planted: s.planted as f32 / (data::PLANT_SECONDS * 60).max(1) as f32,
            disarmed: s.disarm as f32 / data::DISARM_STEPS.max(1) as f32,
            seconds_left: left,
        })
    }

    /// A Sabotage laid at `id` the step the crew first dock there, where
    /// its mission is one and it has a way out but the port.
    pub(super) fn settle_sabotage(&mut self, id: u32) {
        if self.mission_here(id) != Mission::Sabotage {
            return;
        }
        if self
            .infestation(id)
            .is_none_or(|it| it.sabotage.is_some() || it.cleared || it.heart.is_some())
        {
            return;
        }
        let Some(station) = self.station(id) else {
            return;
        };
        let Some(port) = station.port() else {
            return;
        };
        let Some(charge) = charge_spot(&station.design, port.centre) else {
            return;
        };
        let Some(extraction) = extraction_of(&station.design, charge) else {
            return;
        };
        if let Some(it) = self.infestation_mut(id) {
            it.sabotage = Some(Sabotage {
                charge,
                extraction,
                phase: SabotagePhase::Plant,
                planted: 0,
                disarm: 0,
                left: 0,
                next_wave: 0,
            });
        }
    }

    /// The charge's tile middle in the crew's room.
    fn charge_on_deck(&self, s: &Sabotage) -> Option<bims::math::Vec2> {
        let t = shipdesign::TILE as f64;
        let p = self.aboard.from_station(dvec2(
            (s.charge.0 as f64 + 0.5) * t,
            (s.charge.1 as f64 + 0.5) * t,
        ))?;
        let t = shipdesign::TILE as f32;
        Some(bims::math::vec2(
            ((p.x as f32 / t).floor() + 0.5) * t,
            ((p.y as f32 / t).floor() + 0.5) * t,
        ))
    }

    /// What planting the charge asks of that player's Bim: fit to act
    /// (`OutOfReach`), a Sabotage alongside whose charge is still to plant
    /// and the Bim within [`PLANT_REACH`] tiles of it (`NoChargeNear`).
    /// The charge's tile on the crew's deck back.
    pub fn can_plant(&self, slot: u32) -> Result<bims::math::Vec2, Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        let Some((_, s)) = self.sabotage_here() else {
            return Err(Refusal::NoChargeNear);
        };
        if s.phase != SabotagePhase::Plant || !self.in_mission() {
            return Err(Refusal::NoChargeNear);
        }
        let at = self.charge_on_deck(&s).ok_or(Refusal::NoChargeNear)?;
        let me = self.aboard.room.bim_pos(slot as usize);
        if (at - me).len() > PLANT_REACH * shipdesign::TILE as f32
            || !self.aboard.room.deploy_tile_ok(slot as usize, at)
        {
            return Err(Refusal::NoChargeNear);
        }
        Ok(at)
    }

    /// The planting begun — see `Command::Plant`.
    pub(super) fn plant(&mut self, slot: u32) -> Result<(), Refusal> {
        let at = self.can_plant(slot)?;
        if !self
            .aboard
            .room
            .deploy(slot as usize, at, PLANT_CODE, ERRAND_MINUTES)
        {
            return Err(Refusal::NoChargeNear);
        }
        Ok(())
    }

    /// The planting this step, after the rooms: a step of work for every
    /// Bim whose hands are on the charge (an engineer's two), and at
    /// [`data::PLANT_SECONDS`] the charge planted — the hold begun, a wave
    /// due at once, every hand let go.
    pub(super) fn settle_plants(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, s)) = self.sabotage_here() else {
            return;
        };
        if s.phase != SabotagePhase::Plant {
            return;
        }
        let Some(at) = self.charge_on_deck(&s) else {
            return;
        };
        let half = shipdesign::TILE as f32 * 0.5;
        let mut hands = Vec::new();
        let mut work = 0;
        for who in 0..self.aboard.room.crew_count() as usize {
            let Some((tile, code, laying)) = self.aboard.room.deploy_work(who) else {
                continue;
            };
            if code != PLANT_CODE || (tile - at).len() > half {
                continue;
            }
            hands.push(who);
            if laying {
                work += if self.is_engineer(who as u32) { 2 } else { 1 };
            }
        }
        if work == 0 {
            return;
        }
        let now = self.run.mission_steps;
        let mut planted = false;
        if let Some(sab) = self.infestation_mut(id).and_then(|it| it.sabotage.as_mut()) {
            sab.planted += work;
            if sab.planted >= data::PLANT_SECONDS * 60 {
                sab.phase = SabotagePhase::Hold;
                sab.left = data::SABOTAGE_HOLD_STEPS;
                sab.next_wave = now;
                planted = true;
            }
        }
        if planted {
            // The site's own waves are done with: what comes now is the
            // charge's.
            if let Some(it) = self.infestation_mut(id) {
                it.waves_left = 0;
                it.next_wave = None;
            }
            for who in hands {
                self.aboard.room.end_deploy(who);
            }
            events.push(WorldEvent::SabotageStage {
                station: id,
                phase: SabotagePhase::Hold.code(),
            });
        }
    }

    /// The enemies within reach of the charge in the residents' room: the
    /// machines standing and their people on their feet.
    fn at_the_charge(&self, s: &Sabotage) -> u32 {
        let Some(residents) = self.residents.as_ref() else {
            return 0;
        };
        let t = shipdesign::TILE as f64;
        let charge = residents.aboard.to_room(dvec2(
            (s.charge.0 as f64 + 0.5) * t,
            (s.charge.1 as f64 + 0.5) * t,
        ));
        let reach = data::DISARM_REACH_TILES * shipdesign::TILE as f32;
        let room = &residents.aboard.room;
        let machines = (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .filter(|d| !d.destroyed && (d.pos - charge).len() <= reach)
            .count();
        let people = (0..room.crew_count() as usize)
            .filter(|&who| {
                room.is_manufacturer(who)
                    && room.is_alive(who)
                    && !room.is_down(who)
                    && (room.body_pos(who) - charge).len() <= reach
            })
            .count();
        (machines + people) as u32
    }

    /// The Sabotage's step, after the waves: the hold's and the escape's
    /// clocks, their waves, the disarming, and the blast.
    pub(super) fn sabotage_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, s)) = self.sabotage_here() else {
            return;
        };
        if !self.aboard.is_joined()
            || !matches!(s.phase, SabotagePhase::Hold | SabotagePhase::Escape)
        {
            return;
        }
        let now = self.run.mission_steps;
        // The waves on their clock, the last down or not.
        if now >= s.next_wave {
            let full = self.landing_wave_size();
            let wave = if let Some(it) = self.infestation_mut(id) {
                it.wave += 1;
                it.wave
            } else {
                return;
            };
            self.lay_held_wave(id, full, wave);
            events.push(WorldEvent::DroidReinforcements { station: id });
            if let Some(sab) = self.infestation_mut(id).and_then(|it| it.sabotage.as_mut()) {
                sab.next_wave = now + data::SABOTAGE_WAVE_STEPS;
            }
        }
        let near = if s.phase == SabotagePhase::Hold {
            self.at_the_charge(&s).min(3)
        } else {
            0
        };
        let mut stage = None;
        if let Some(sab) = self.infestation_mut(id).and_then(|it| it.sabotage.as_mut()) {
            sab.disarm += near;
            sab.left = sab.left.saturating_sub(1);
            if sab.phase == SabotagePhase::Hold && sab.disarm >= data::DISARM_STEPS {
                sab.phase = SabotagePhase::Disarmed;
                stage = Some(SabotagePhase::Disarmed);
            } else if sab.left == 0 && sab.phase == SabotagePhase::Hold {
                sab.phase = SabotagePhase::Escape;
                sab.left = data::SABOTAGE_ESCAPE_STEPS;
                stage = Some(SabotagePhase::Escape);
            } else if sab.left == 0 && sab.phase == SabotagePhase::Escape {
                sab.phase = SabotagePhase::Done;
                stage = Some(SabotagePhase::Done);
            }
        }
        match stage {
            Some(SabotagePhase::Disarmed) => {
                events.push(WorldEvent::SabotageStage {
                    station: id,
                    phase: SabotagePhase::Disarmed.code(),
                });
                if !self.run.won {
                    self.lost = true;
                }
            }
            Some(SabotagePhase::Escape) => events.push(WorldEvent::SabotageStage {
                station: id,
                phase: SabotagePhase::Escape.code(),
            }),
            Some(SabotagePhase::Done) => self.blow_the_charge(id, events),
            _ => {}
        }
    }

    /// The charge blows: every crew member within reach of the way out
    /// gets away, every other is left behind, dead; the site is cleared
    /// and every player's *Back to ship* pressed, so the departure takes
    /// the crew home at the end of the step.
    fn blow_the_charge(&mut self, id: u32, events: &mut Vec<WorldEvent>) {
        let way_out = self.sabotage_look().and_then(|look| {
            let inside = look
                .way_out
                .sub(look.way_out_out.scale(1.5 * shipdesign::TILE as f64));
            let p = self.aboard.from_station(inside)?;
            Some(bims::math::vec2(p.x as f32, p.y as f32))
        });
        let reach = data::EXTRACTION_REACH_TILES * shipdesign::TILE as f32;
        let crew = self.aboard.room.crew_count() as usize;
        for who in 0..crew {
            if !self.aboard.room.is_alive(who) || self.is_reinforcement(who as u32) {
                continue;
            }
            let out = way_out.is_some_and(|at| (self.aboard.room.bim_pos(who) - at).len() <= reach);
            if !out {
                self.aboard.room.kill_now(who);
                events.push(WorldEvent::LeftBehind { who: who as u32 });
            }
        }
        if let Some(it) = self.infestation_mut(id) {
            it.cleared = true;
        }
        events.push(WorldEvent::SabotageStage {
            station: id,
            phase: SabotagePhase::Done.code(),
        });
        events.push(WorldEvent::DroidStationCleared { station: id });
        let players = self.players() as usize;
        self.run
            .returning
            .resize(players.max(self.run.returning.len()), false);
        for slot in 0..players {
            self.run.returning[slot] = true;
        }
        self.run.recalled = true;
    }

    /// Where the enemies make for while the charge is held: a spot each
    /// round it, by body index in the residents' room. Nothing otherwise.
    pub(super) fn sabotage_objectives(&self) -> Vec<Option<bims::math::Vec2>> {
        let Some((_, s)) = self.sabotage_here() else {
            return Vec::new();
        };
        if s.phase != SabotagePhase::Hold {
            return Vec::new();
        }
        let Some(residents) = self.residents.as_ref() else {
            return Vec::new();
        };
        let t = shipdesign::TILE as f64;
        let charge = residents.aboard.to_room(dvec2(
            (s.charge.0 as f64 + 0.5) * t,
            (s.charge.1 as f64 + 0.5) * t,
        ));
        let room = &residents.aboard.room;
        let bims = room.crew_count() as usize;
        (0..room.body_count() as usize)
            .map(|body| {
                let enemy = body >= bims || room.is_manufacturer(body);
                enemy.then(|| charge + defense::enemy_spot(body) * (shipdesign::TILE as f32 * 0.6))
            })
            .collect()
    }

    /// Plant the charge outright and begin the hold, for the tests.
    pub fn plant_for_probe(&mut self) {
        if let Some(id) = self.ship.state.alongside() {
            self.settle_sabotage(id);
        }
        let Some((id, _)) = self.sabotage_here() else {
            return;
        };
        let now = self.run.mission_steps;
        if let Some(it) = self.infestation_mut(id) {
            it.waves_left = 0;
            it.next_wave = None;
            if let Some(sab) = it.sabotage.as_mut() {
                sab.planted = data::PLANT_SECONDS * 60;
                sab.phase = SabotagePhase::Hold;
                sab.left = data::SABOTAGE_HOLD_STEPS;
                sab.next_wave = now;
            }
        }
    }
}

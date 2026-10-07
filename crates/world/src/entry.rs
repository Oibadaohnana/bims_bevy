//! The ways in, and the crew welding them shut (October 2026).
//!
//! A wave comes aboard by a site's **ways in** in turn — a station's
//! airlocks but the port, the farthest first (`droid::arrival_airlock_at`),
//! a town's gates (`surface::gate_for_wave`) — and the crew can **weld** one
//! shut ([`Command::Weld`]): the Bim's hands are the room's, a deploy errand
//! of [`WELD_CODE`] (`Game::deploy`), and the work is the world's — every
//! step a Bim's hands are on a way in counts a step of welding to it, an
//! engineer's two (`Run::weld_work`, [`World::settle_welds`]), so two Bims
//! weld one twice as fast; at [`WELD_SECONDS`] of it — a breach's
//! [`data::BREACH_WELD_SECONDS`] — it is welded and every Bim on it let go.
//! A wave whose turn names a welded way in comes in by the
//! next unwelded one in turn instead; with every one welded it **burns
//! through** the one its turn names, and that weld is gone
//! ([`World::burn_through`], [`WorldEvent::WeldBurnt`]). So the crew choose
//! where the fight comes from, and holding that is a weld a wave.
//!
//! What is welded is the mission's (`Run::welded`, saved, hashed where any
//! is), by way in's index — an airlock's among the design's airlocks
//! (`droid::airlocks`), a gate's among the town's — and cleared at every
//! mission's start and end. The Machine Heart's fortress keeps its own
//! turn and is never welded. A child of `crate::world`, as `mission.rs`
//! is. **Nothing here draws from a stream.**

use super::*;
use shipdesign::dock::Port;

/// The room's deploy code a weld goes as (`Game::deploy`): past every
/// `DeployKind`'s, and never laid as one — the world counts the work
/// ([`World::settle_welds`]) and puts the errand down when it is done.
pub const WELD_CODE: u32 = 100;
/// How near the Bim's way in has to be to be welded, in tiles from the
/// Bim to the tile inside it: under the room's `task::DEPLOY_REACH` (two),
/// so the work starts where the Bim stands — a player's own Bim is walked
/// by its player alone.
pub const WELD_REACH: f32 = 1.75;
/// Seconds of one Bim's hands on an ordinary way in to weld it shut; an
/// engineer's count twice.
pub const WELD_SECONDS: u32 = 4;
/// How long the room's errand runs, in game minutes: longer than any weld,
/// so the world's count always ends it first.
const ERRAND_MINUTES: f32 = 600.0;
/// Steps of the world a second at 1×.
const STEPS_A_SECOND: u32 = 60;

/// A way in as the app draws it: where its doorway's middle is on the
/// crew's deck, which way is out, whether it is welded, and whether the
/// next wave comes by it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntryLook {
    pub index: u32,
    /// The middle of the doorway — an airlock's two tiles, a gate's
    /// opening a tile inside the wall — in the crew's room's units.
    pub at: bims::math::Vec2,
    /// A unit step out of the site.
    pub outward: bims::math::Vec2,
    /// How wide it is, in tiles.
    pub width: f32,
    pub welded: bool,
    pub next: bool,
    /// How far through its weld it is, nought to one: nought when nobody
    /// has put a hand to it.
    pub progress: f32,
    /// The doorway's middle and the way out in the site's own design
    /// units, for a painter drawing in the station's frame.
    pub site_at: DVec2,
    pub site_out: DVec2,
}

/// A way in of a site, in its own design's units: the middle of its
/// doorway, a unit step out, its width in tiles.
#[derive(Clone, Copy, Debug)]
pub(super) struct WayIn {
    pub(super) index: u32,
    pub(super) at: DVec2,
    pub(super) outward: DVec2,
    pub(super) width: f32,
}

impl World {
    /// A station's ways in, in the order the waves take them: its
    /// airlocks but the port, the farthest from the port first (ties to
    /// the lower index) — `droid::arrival_airlock_at`'s turn — each by its
    /// index among the design's airlocks; a town's gates in their order.
    /// None at the Machine Heart's fortress, which keeps its own turn, and
    /// none at a station with only the port.
    pub(super) fn ways_in(station: &Station) -> Vec<WayIn> {
        if crate::heart::is_heart(station.id) {
            return Vec::new();
        }
        if crate::surface::surface_body(station.id).is_some() {
            return station
                .gates
                .iter()
                .enumerate()
                .map(|(i, gate)| {
                    let (ox, oy) = gate.inward();
                    WayIn {
                        index: i as u32,
                        at: gate.spot(),
                        outward: dvec2(-ox, -oy),
                        width: gate.width as f32,
                    }
                })
                .collect();
        }
        let ports = droidplan::airlocks(&station.design);
        let Some(first) = ports.first().copied() else {
            return Vec::new();
        };
        let far = |p: &Port| (p.centre.0 - first.centre.0).hypot(p.centre.1 - first.centre.1);
        let mut others: Vec<(usize, Port)> = ports.iter().copied().enumerate().skip(1).collect();
        others.sort_by(|(i, a), (j, b)| far(b).total_cmp(&far(a)).then(i.cmp(j)));
        others
            .into_iter()
            .map(|(i, p)| WayIn {
                index: i as u32,
                at: dvec2(p.centre.0, p.centre.1),
                outward: dvec2(p.outward.0 as f64, p.outward.1 as f64),
                width: 2.0,
            })
            .collect()
    }

    /// Which of `ways` wave `wave` comes in by, with what is welded: the
    /// one its turn names, or the next unwelded after it in turn — and,
    /// every one welded, the one its turn names, burnt through (`true`).
    /// Its place in `ways`.
    fn turn_of(&self, ways: &[WayIn], wave: u32) -> Option<(usize, bool)> {
        let n = ways.len();
        if n == 0 {
            return None;
        }
        let turn = (wave as usize + n - 1) % n;
        (0..n)
            .map(|k| (turn + k) % n)
            .find(|&k| !self.run.welded.contains(&ways[k].index))
            .map(|k| (k, false))
            .or(Some((turn, true)))
    }

    /// The airlock wave `wave` comes aboard `station` by: the turn's
    /// (`droid::arrival_airlock_at`) with the welds read — the next
    /// unwelded in turn where its own is welded. A station with only the
    /// port, and the Heart's fortress, as `arrival_airlock_at` says.
    pub(super) fn arrival_port(&self, station: &Station, wave: u32) -> Option<Port> {
        let ways = Self::ways_in(station);
        let Some((k, _)) = self.turn_of(&ways, wave) else {
            return droidplan::arrival_airlock_at(&station.design, station.id, wave);
        };
        droidplan::airlocks(&station.design)
            .get(ways[k].index as usize)
            .copied()
    }

    /// The gate wave `wave` comes into a town by: the turn's
    /// (`surface::gate_for_wave`) with the welds read, as
    /// [`World::arrival_port`] does an airlock.
    pub(super) fn arrival_gate(
        &self,
        station: &Station,
        wave: u32,
    ) -> Option<crate::surface::Gate> {
        let ways = Self::ways_in(station);
        let (k, _) = self.turn_of(&ways, wave)?;
        station.gates.get(ways[k].index as usize).copied()
    }

    /// A wave `wave` landing at `station`: every way in welded, it burns
    /// through the one its turn names, and that weld is gone. Nothing
    /// otherwise.
    pub(super) fn burn_through(&mut self, station: &Station, wave: u32) {
        // At Seal the breaches nothing burns through: every breach welded
        // is no wave at all.
        if self.is_breaches(station.id) {
            return;
        }
        let ways = Self::ways_in(station);
        if let Some((k, true)) = self.turn_of(&ways, wave) {
            let index = ways[k].index;
            self.run.welded.retain(|&e| e != index);
            self.run.burnt.push(index);
        }
    }

    /// The site the crew are at in a mission, if it is the one alongside.
    fn site_for_welds(&self) -> Option<Station> {
        if !self.in_mission() {
            return None;
        }
        let id = self.ship.state.alongside()?;
        self.station(id).cloned()
    }

    /// Which wave lands next at `station`: one past the one aboard.
    fn next_wave_at(&self, station: u32) -> u32 {
        let aboard = match self.infestation(station) {
            Some(it) => it.wave,
            None => self.defense(station).map(|d| d.wave).unwrap_or(0),
        };
        aboard + 1
    }

    /// A design point of the site as a point of the crew's room.
    fn crew_point(&self, p: DVec2) -> Option<bims::math::Vec2> {
        let q = self.aboard.from_station(p)?;
        Some(bims::math::vec2(q.x as f32, q.y as f32))
    }

    /// Every way in of the site the crew are at, as the app draws it — on
    /// the crew's deck, welded or not, and which the next wave takes (none
    /// while the site has no more to come). Empty between missions, at the
    /// Heart and at a site with no way in but the port.
    pub fn entries(&self) -> Vec<EntryLook> {
        let Some(station) = self.site_for_welds() else {
            return Vec::new();
        };
        let ways = Self::ways_in(&station);
        let more = self.site_threatened(station.id)
            || self
                .infestation(station.id)
                .is_some_and(|it| !it.cleared && it.waves_left > 0)
            || self.defense(station.id).is_some_and(|d| d.more_to_come());
        let next = more
            .then(|| self.turn_of(&ways, self.next_wave_at(station.id)))
            .flatten()
            .map(|(k, _)| k);
        let turned = |v: DVec2| -> Option<bims::math::Vec2> {
            let a = self.crew_point(dvec2(0.0, 0.0))?;
            let b = self.crew_point(v)?;
            Some((b - a).normalize_or_zero())
        };
        ways.iter()
            .enumerate()
            .filter_map(|(k, w)| {
                Some(EntryLook {
                    index: w.index,
                    at: self.crew_point(w.at)?,
                    outward: turned(w.outward)?,
                    width: w.width,
                    welded: self.run.welded.contains(&w.index),
                    next: next == Some(k),
                    progress: self.weld_share(w.index),
                    site_at: w.at,
                    site_out: w.outward,
                })
            })
            .collect()
    }

    /// The tile of the crew's room a way in is welded from: the deck a
    /// tile inside its doorway.
    fn weld_tile(&self, way: &WayIn) -> Option<bims::math::Vec2> {
        let inside = way.at.sub(way.outward.scale(shipdesign::TILE as f64));
        let p = self.crew_point(inside)?;
        let t = shipdesign::TILE as f32;
        Some(bims::math::vec2(
            ((p.x / t).floor() + 0.5) * t,
            ((p.y / t).floor() + 0.5) * t,
        ))
    }

    /// The way in that player's Bim would weld: the nearest unwelded one
    /// whose weld tile is within [`WELD_REACH`] tiles of it, with that
    /// tile. Refused `OutOfReach` (not fit to act, or no mission on) and
    /// `NoWayInNear`. What the app greys the key with.
    pub fn can_weld(&self, slot: u32) -> Result<(u32, bims::math::Vec2), Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        let Some(station) = self.site_for_welds() else {
            return Err(Refusal::OutOfReach);
        };
        let me = self.aboard.room.bim_pos(slot as usize);
        let reach = WELD_REACH * shipdesign::TILE as f32;
        Self::ways_in(&station)
            .iter()
            .filter(|w| !self.run.welded.contains(&w.index))
            .filter_map(|w| Some((w.index, self.weld_tile(w)?)))
            .filter(|&(_, tile)| (tile - me).len() <= reach)
            .filter(|&(_, tile)| self.aboard.room.deploy_tile_ok(slot as usize, tile))
            .min_by(|a, b| {
                (a.1 - me)
                    .len()
                    .total_cmp(&(b.1 - me).len())
                    .then(a.0.cmp(&b.0))
            })
            .ok_or(Refusal::NoWayInNear)
    }

    /// The weld begun — see [`Command::Weld`]: the errand handed to the
    /// room, the way in's index back.
    pub(super) fn weld(&mut self, slot: u32) -> Result<u32, Refusal> {
        let (index, tile) = self.can_weld(slot)?;
        if !self
            .aboard
            .room
            .deploy(slot as usize, tile, WELD_CODE, ERRAND_MINUTES)
        {
            return Err(Refusal::NoWayInNear);
        }
        Ok(index)
    }

    /// Steps of welding a way in of `station` wants: a breach's at Seal
    /// the breaches, else an ordinary one's.
    fn weld_steps(&self, station: u32) -> u32 {
        let seconds = if self.is_breaches(station) {
            data::BREACH_WELD_SECONDS
        } else {
            WELD_SECONDS
        };
        seconds * STEPS_A_SECOND
    }

    /// How far through its weld way in `index` of the site alongside is,
    /// nought to one.
    pub fn weld_share(&self, index: u32) -> f32 {
        let Some(station) = self.ship.state.alongside() else {
            return 0.0;
        };
        let done = self
            .run
            .weld_work
            .iter()
            .find(|&&(e, _)| e == index)
            .map_or(0, |&(_, work)| work);
        (done as f32 / self.weld_steps(station).max(1) as f32).min(1.0)
    }

    /// The welding this step, after the rooms: a step of work to the way
    /// in under every Bim's hands on a weld (an engineer's two), and every
    /// way in whose work is done welded — said once, with the first Bim on
    /// it — and the Bims on it let go.
    pub(super) fn settle_welds(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(station) = self.site_for_welds() else {
            return;
        };
        let ways = Self::ways_in(&station);
        let half = shipdesign::TILE as f32 * 0.5;
        let tiles: Vec<(u32, bims::math::Vec2)> = ways
            .iter()
            .filter_map(|w| Some((w.index, self.weld_tile(w)?)))
            .collect();
        let mut hands: Vec<(usize, u32)> = Vec::new();
        for who in 0..self.aboard.room.crew_count() as usize {
            let Some((at, code, laying)) = self.aboard.room.deploy_work(who) else {
                continue;
            };
            if code != WELD_CODE {
                continue;
            }
            let Some(&(index, _)) = tiles.iter().find(|(_, tile)| (*tile - at).len() <= half)
            else {
                continue;
            };
            if self.run.welded.contains(&index) {
                // Welded by others meanwhile: nothing left to do here.
                self.aboard.room.end_deploy(who);
                continue;
            }
            hands.push((who, index));
            if !laying {
                continue;
            }
            let step = if self.is_engineer(who as u32) { 2 } else { 1 };
            match self.run.weld_work.iter_mut().find(|(e, _)| *e == index) {
                Some((_, work)) => *work += step,
                None => self.run.weld_work.push((index, step)),
            }
        }
        let needed = self.weld_steps(station.id);
        let done: Vec<u32> = self
            .run
            .weld_work
            .iter()
            .filter(|&&(_, work)| work >= needed)
            .map(|&(e, _)| e)
            .collect();
        for index in done {
            self.run.weld_work.retain(|&(e, _)| e != index);
            self.run.welded.push(index);
            self.run.welded.sort_unstable();
            let mut first = None;
            for &(who, at) in &hands {
                if at == index {
                    first.get_or_insert(who);
                    self.aboard.room.end_deploy(who);
                }
            }
            events.push(WorldEvent::Welding {
                who: first.unwrap_or(0) as u32,
                entry: index,
                done: true,
            });
        }
    }

    /// The burns since last asked, each said once: the step's events get
    /// [`WorldEvent::WeldBurnt`] for every weld a wave burnt through.
    pub(super) fn say_burns(&mut self, events: &mut Vec<WorldEvent>) {
        for entry in std::mem::take(&mut self.run.burnt) {
            events.push(WorldEvent::WeldBurnt { entry });
        }
    }

    /// For a look (`BIMS_WELDS=1`): the site's first way in welded and
    /// the second half way through its weld.
    pub fn show_welds_for_probe(&mut self) {
        let Some(station) = self.site_for_welds() else {
            return;
        };
        let ways = Self::ways_in(&station);
        if let Some(first) = ways.first() {
            self.weld_for_probe(first.index);
        }
        if let Some(second) = ways.get(1) {
            let half = self.weld_steps(station.id) / 2;
            self.run.weld_work.retain(|&(e, _)| e != second.index);
            self.run.weld_work.push((second.index, half));
        }
    }

    /// Weld way in `index` of the site alongside outright, for the tests.
    pub fn weld_for_probe(&mut self, index: u32) {
        if !self.run.welded.contains(&index) {
            self.run.welded.push(index);
            self.run.welded.sort_unstable();
        }
    }
}

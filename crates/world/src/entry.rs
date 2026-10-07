//! The ways in, and the crew welding them shut (October 2026).
//!
//! A wave comes aboard by a site's **ways in** in turn — a station's
//! airlocks but the port, the farthest first (`droid::arrival_airlock_at`),
//! a town's gates (`surface::gate_for_wave`) — and the crew can **weld** one
//! shut ([`Command::Weld`]): the walk and the work are the room's, a deploy
//! errand of [`WELD_CODE`] (`Game::deploy`), so a hit drops it and the app's
//! bar shows it. A wave whose turn names a welded way in comes in by the
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
/// `DeployKind`'s, and never laid as one — `World::step` hands it to
/// [`World::finish_weld`].
pub const WELD_CODE: u32 = 100;
/// How near the Bim's way in has to be to be welded, in tiles from the
/// Bim to the tile inside it: under the room's `task::DEPLOY_REACH` (two),
/// so the work starts where the Bim stands — a player's own Bim is walked
/// by its player alone.
pub const WELD_REACH: f32 = 1.75;
/// How long a weld takes, in game minutes of working steps — seconds at
/// 1× — and an engineer's.
pub const WELD_MINUTES: f32 = 4.0;
pub const ENGINEER_WELD_MINUTES: f32 = 2.0;

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
    /// The doorway's middle and the way out in the site's own design
    /// units, for a painter drawing in the station's frame.
    pub site_at: DVec2,
    pub site_out: DVec2,
}

/// A way in of a site, in its own design's units: the middle of its
/// doorway, a unit step out, its width in tiles.
#[derive(Clone, Copy, Debug)]
struct WayIn {
    index: u32,
    at: DVec2,
    outward: DVec2,
    width: f32,
}

impl World {
    /// A station's ways in, in the order the waves take them: its
    /// airlocks but the port, the farthest from the port first (ties to
    /// the lower index) — `droid::arrival_airlock_at`'s turn — each by its
    /// index among the design's airlocks; a town's gates in their order.
    /// None at the Machine Heart's fortress, which keeps its own turn, and
    /// none at a station with only the port.
    fn ways_in(station: &Station) -> Vec<WayIn> {
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
        let minutes = if self.is_engineer(slot) {
            ENGINEER_WELD_MINUTES
        } else {
            WELD_MINUTES
        };
        if !self
            .aboard
            .room
            .deploy(slot as usize, tile, WELD_CODE, minutes)
        {
            return Err(Refusal::NoWayInNear);
        }
        Ok(index)
    }

    /// The room says `who` finished a weld at `at`: the way in whose weld
    /// tile it is, welded — if the mission still runs there and it is not
    /// welded already.
    pub(super) fn finish_weld(
        &mut self,
        who: usize,
        at: bims::math::Vec2,
        events: &mut Vec<WorldEvent>,
    ) {
        let Some(station) = self.site_for_welds() else {
            return;
        };
        let half = shipdesign::TILE as f32 * 0.5;
        let Some(way) = Self::ways_in(&station).into_iter().find(|w| {
            self.weld_tile(w)
                .is_some_and(|tile| (tile - at).len() <= half)
        }) else {
            return;
        };
        if self.run.welded.contains(&way.index) {
            return;
        }
        self.run.welded.push(way.index);
        self.run.welded.sort_unstable();
        events.push(WorldEvent::Welding {
            who: who as u32,
            entry: way.index,
            done: true,
        });
    }

    /// The burns since last asked, each said once: the step's events get
    /// [`WorldEvent::WeldBurnt`] for every weld a wave burnt through.
    pub(super) fn say_burns(&mut self, events: &mut Vec<WorldEvent>) {
        for entry in std::mem::take(&mut self.run.burnt) {
            events.push(WorldEvent::WeldBurnt { entry });
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

//! **Evacuation** (October 2026, a mission the map shapes): a defence
//! whose fight is getting the site's people to the crew's ship.
//!
//! - **Who**: the site's own people and [`data::EVACUEES`] refugees beside
//!   them (`Residents::open`'s `evacuees`, past the bunks) — every body of
//!   the site's room that is not a defender, a grave or one of the
//!   Manufacturers'. They take no arms: the room keeps them sheltering, and
//!   the world posts them by **the flag** instead of a bunk.
//! - **The flag**: one, standing where the site's first person stands when
//!   the defence begins. `Command::Flag` (the Use key, V) takes it up from
//!   within [`data::FLAG_REACH_TILES`] — a player's Bim, never a bot — and
//!   puts it down where the Bim stands when pressed again; the carrier
//!   still shoots. Downed or unfit, the carrier drops it where it stands.
//!   Every person not yet aboard is posted a little way round it
//!   ([`World::evacuation_step`]) — round the carrier while it is carried,
//!   so they follow; where it lies when it is dropped, so they hold there.
//! - **Aboard**: a person standing on the crew's ship (its own design
//!   under it on the crew's deck) is aboard for good, and posted no more.
//! - **The waves** come on a clock, [`data::EVAC_WAVE_STEPS`] apart and
//!   stacking, until it is over; the machines go for the crew and the
//!   people alike.
//! - **Held** when every one still alive is aboard (one at the least):
//!   `TownHeld`, and the bounty waiting for the clear cut to the share of
//!   the people saved. **Failed** when none is left alive and none aboard:
//!   the defence lost, nothing paid.
//!
//! - **The map** (October 2026, the player's: "the evacuation need its own
//!   map generation … spawn right next to the hostages … fight through
//!   enemies to get to the ship … horizontally long … Enemies always
//!   should spawn between you and the ship"): a station's Evacuation is
//!   laid out long east–west (`stationgen::Feature::Evacuation`), its
//!   shelter the room farthest from the port; a town's has the reachable
//!   tile farthest from its pad. The flag, the people and the crew are
//!   all stood there as the defence begins ([`World::shelter_tile`]), and
//!   every wave lands on the walk from the flag — or from a player's Bim
//!   nearer the ship by it — to the ship, part of the way along
//!   ([`World::evacuation_arrival`]), so the crew fight their way home.
//!
//! The state is the defence's (`defense::Evacuation`), saved and hashed.
//! A child of `crate::world`, as `mission.rs` is. **Nothing here draws
//! from a stream.**

use super::*;
use crate::defense::Evacuation;

/// How far along the walk to the ship a wave lands, in tiles of walk
/// from the flag (or the player's Bim nearest the ship): half the walk,
/// at least [`AHEAD_LEAST`] and at most [`AHEAD_MOST`] — never past the
/// ship's door. Far enough that the crew walk into it rather than shoot
/// it as it lands.
const AHEAD_MOST: usize = 30;
const AHEAD_LEAST: usize = 10;

/// Whether a tile of a design can be walked: deck with nothing on it
/// that blocks a body (a door does not).
fn walkable(design: &ShipDesign, x: i32, y: i32) -> bool {
    let grid = design.grid();
    if grid.get(shipdesign::Layer::Floor, (x, y)) == 0 {
        return false;
    }
    let object = grid.get(shipdesign::Layer::Object, (x, y));
    object == 0
        || design
            .part(object)
            .is_none_or(|p| !p.kind.def().blocks_movement)
}

/// A site's tiles by how many steps of walk — four ways, a tile a step —
/// they are from the tile just inside its port.
struct Walk {
    side: i32,
    steps: Vec<u32>,
}

impl Walk {
    fn from_port(design: &ShipDesign) -> Option<Walk> {
        let port = droidplan::airlocks(design).into_iter().next()?;
        let t = shipdesign::TILE as f64;
        let (x, y) = droidplan::inside_of(&port, 1.0);
        let start = ((x / t).floor() as i32, (y / t).floor() as i32);
        let side = design.build_area as i32;
        let mut steps = vec![u32::MAX; (side * side) as usize];
        let inside = |(x, y): (i32, i32)| x >= 0 && y >= 0 && x < side && y < side;
        if !inside(start) || !walkable(design, start.0, start.1) {
            return None;
        }
        let mut queue = std::collections::VecDeque::new();
        steps[(start.1 * side + start.0) as usize] = 0;
        queue.push_back(start);
        while let Some((x, y)) = queue.pop_front() {
            let here = steps[(y * side + x) as usize];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (x + dx, y + dy);
                if !inside(next) || !walkable(design, next.0, next.1) {
                    continue;
                }
                let i = (next.1 * side + next.0) as usize;
                if steps[i] == u32::MAX {
                    steps[i] = here + 1;
                    queue.push_back(next);
                }
            }
        }
        Some(Walk { side, steps })
    }

    /// How many steps a tile is from the port; `None` for one not reached.
    fn at(&self, (x, y): (i32, i32)) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.side || y >= self.side {
            return None;
        }
        let s = self.steps[(y * self.side + x) as usize];
        (s != u32::MAX).then_some(s)
    }

    /// The walk from `from` to the port, a tile a step, `from` first: each
    /// step to the first neighbour one step nearer, east, west, south,
    /// north.
    fn path_from(&self, from: (i32, i32)) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let Some(mut left) = self.at(from) else {
            return out;
        };
        let mut at = from;
        out.push(at);
        while left > 0 {
            let Some(next) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .map(|(dx, dy)| (at.0 + dx, at.1 + dy))
                .find(|&n| self.at(n) == Some(left - 1))
            else {
                break;
            };
            at = next;
            left -= 1;
            out.push(at);
        }
        out
    }

    /// The tile reached farthest from the port with free deck all round
    /// it, the lower row and then column on a tie.
    fn farthest_open(&self, design: &ShipDesign) -> Option<(u32, u32)> {
        let open =
            |x: i32, y: i32| (-1..=1).all(|dx| (-1..=1).all(|dy| walkable(design, x + dx, y + dy)));
        let mut best: Option<((i32, i32), u32)> = None;
        for y in 0..self.side {
            for x in 0..self.side {
                let Some(s) = self.at((x, y)) else {
                    continue;
                };
                if best.is_some_and(|(_, b)| s <= b) || !open(x, y) {
                    continue;
                }
                best = Some(((x, y), s));
            }
        }
        best.map(|((x, y), _)| (x as u32, y as u32))
    }
}

/// A design tile's middle in design units.
fn tile_middle((x, y): (u32, u32)) -> DVec2 {
    let t = shipdesign::TILE as f64;
    dvec2((x as f64 + 0.5) * t, (y as f64 + 0.5) * t)
}

/// The design tile a point in design units is on.
fn tile_of(p: DVec2) -> (i32, i32) {
    let t = shipdesign::TILE as f64;
    ((p.x / t).floor() as i32, (p.y / t).floor() as i32)
}

/// The Evacuation's flag as the app draws it: where it stands in the
/// site's own design units, and whether it is carried; the people aboard
/// and how many there were.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvacuationLook {
    pub flag: DVec2,
    pub carried: bool,
    pub aboard: u32,
    pub total: u32,
    pub alive: u32,
}

impl World {
    /// The Evacuation at the site the crew are at, with its id.
    fn evacuation_here(&self) -> Option<(u32, &Evacuation)> {
        let id = self.ship.state.station()?;
        Some((id, self.defense(id)?.evacuation.as_ref()?))
    }

    /// The people an Evacuation is for, by body index in the site's room:
    /// alive, not a defender, a grave or one of the Manufacturers'.
    fn evacuees(&self) -> Vec<usize> {
        let Some(residents) = self.residents.as_ref() else {
            return Vec::new();
        };
        let room = &residents.aboard.room;
        (0..room.crew_count() as usize)
            .filter(|&who| {
                residents.is_own(who) && !room.is_manufacturer(who) && room.is_alive(who)
            })
            .collect()
    }

    /// Where a body of the site's room stands on the crew's deck.
    fn on_crew_deck(&self, who: usize) -> Option<bims::math::Vec2> {
        let residents = self.residents.as_ref()?;
        let p = residents.aboard.position(who as u32);
        let q = self.aboard.from_station(p)?;
        Some(bims::math::vec2(q.x as f32, q.y as f32))
    }

    /// Whether a point of the crew's deck is on the crew's ship.
    pub(super) fn on_the_ship(&self, at: bims::math::Vec2) -> bool {
        let (foreign, p) = self.aboard.design_of(dvec2(at.x as f64, at.y as f64));
        if foreign {
            return false;
        }
        let t = shipdesign::TILE as f64;
        let tile = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        self.ship
            .design
            .grid()
            .get(shipdesign::Layer::Structure, tile)
            != 0
    }

    /// Where an Evacuation at `id` begins, a tile of the site's design:
    /// the middle of the shelter its map was built with, else the tile
    /// farthest from the port by walk with free deck all round (a town,
    /// or a station no long layout kept the contract for).
    pub fn shelter_tile(&self, id: u32) -> Option<(u32, u32)> {
        let site = self.station(id)?;
        let design = &site.design;
        let shelter = site
            .fitted
            .as_ref()
            .filter(|f| f.feature == crate::stationgen::Feature::Evacuation)
            .and_then(|f| f.rooms.first().copied());
        if let Some(room) = shelter {
            let middle = tile_middle(((room[0] + room[2]) / 2, (room[1] + room[3]) / 2));
            if let Some(&tile) = attacks::spots(design, Some(room), 1, middle, true).first() {
                return Some(tile);
            }
        }
        Walk::from_port(design)?.farthest_open(design)
    }

    /// The Evacuation laid as the defence `id` begins: the people counted
    /// and the flag in the shelter (or, with none, where the first of
    /// them stands). `None` where there is nobody to bring out.
    pub(super) fn begin_evacuation(&self) -> Option<Evacuation> {
        let people = self.evacuees();
        let first = *people.first()?;
        let sheltered = self
            .ship
            .state
            .station()
            .and_then(|id| self.shelter_tile(id))
            .and_then(|tile| self.aboard.from_station(tile_middle(tile)))
            .map(|p| bims::math::vec2(p.x as f32, p.y as f32));
        let at = sheltered.or_else(|| self.on_crew_deck(first))?;
        Some(Evacuation {
            total: people.len() as u32,
            aboard: Vec::new(),
            holder: None,
            at: (at.x.round() as i32, at.y.round() as i32),
        })
    }

    /// The Evacuation as the app draws it.
    pub fn evacuation_look(&self) -> Option<EvacuationLook> {
        let (_, e) = self.evacuation_here()?;
        let at = match e.holder {
            Some(who) => self.aboard.room.bim_pos(who as usize),
            None => bims::math::vec2(e.at.0 as f32, e.at.1 as f32),
        };
        let flag = self
            .aboard
            .to_station(dvec2(at.x as f64, at.y as f64))
            .unwrap_or(dvec2(0.0, 0.0));
        Some(EvacuationLook {
            flag,
            carried: e.holder.is_some(),
            aboard: e.aboard.len() as u32,
            total: e.total,
            alive: self.evacuees().len() as u32,
        })
    }

    /// What the flag asks of that player's Bim: fit to act (`OutOfReach`),
    /// an Evacuation under way and the flag its own to put down, or lying
    /// within reach to take up (`NoFlagNear`). True to take it up.
    pub fn can_flag(&self, slot: u32) -> Result<bool, Refusal> {
        if !self.fit_to_act(slot) || slot >= self.players() {
            return Err(Refusal::OutOfReach);
        }
        let Some((id, e)) = self.evacuation_here() else {
            return Err(Refusal::NoFlagNear);
        };
        if self.defense(id).is_some_and(|d| d.over()) {
            return Err(Refusal::NoFlagNear);
        }
        match e.holder {
            Some(who) if who == slot => Ok(false),
            Some(_) => Err(Refusal::NoFlagNear),
            None => {
                let me = self.aboard.room.bim_pos(slot as usize);
                let at = bims::math::vec2(e.at.0 as f32, e.at.1 as f32);
                if (at - me).len() <= data::FLAG_REACH_TILES * shipdesign::TILE as f32 {
                    Ok(true)
                } else {
                    Err(Refusal::NoFlagNear)
                }
            }
        }
    }

    /// The flag taken up, or put down where the Bim stands — see
    /// `Command::Flag`. True when it was taken up.
    pub(super) fn flag(&mut self, slot: u32) -> Result<bool, Refusal> {
        let take = self.can_flag(slot)?;
        let me = self.aboard.room.bim_pos(slot as usize);
        let Some(id) = self.ship.state.station() else {
            return Err(Refusal::NoFlagNear);
        };
        if let Some(e) = self.defense_mut(id).and_then(|d| d.evacuation.as_mut()) {
            if take {
                e.holder = Some(slot);
            } else {
                e.holder = None;
                e.at = (me.x.round() as i32, me.y.round() as i32);
            }
        }
        Ok(take)
    }

    /// The Evacuation's step, after the waves: the flag with its carrier
    /// (dropped by one down or unfit), whoever has reached the ship
    /// counted aboard, the rest posted round the flag, and the end — held
    /// or failed.
    pub(super) fn evacuation_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, e)) = self.evacuation_here().map(|(id, e)| (id, e.clone())) else {
            return;
        };
        if !self.aboard.is_joined() || self.defense(id).is_some_and(|d| d.over()) {
            return;
        }
        // The flag with its carrier.
        let mut e = e;
        if let Some(who) = e.holder {
            let at = self.aboard.room.bim_pos(who as usize);
            e.at = (at.x.round() as i32, at.y.round() as i32);
            if !self.fit_to_act(who) {
                e.holder = None;
                events.push(WorldEvent::FlagCarried { who, taken: false });
            }
        }
        // Who is aboard now.
        let people = self.evacuees();
        for &who in &people {
            if e.aboard.contains(&(who as u32)) {
                continue;
            }
            if self
                .on_crew_deck(who)
                .is_some_and(|at| self.on_the_ship(at))
            {
                e.aboard.push(who as u32);
                e.aboard.sort_unstable();
                events.push(WorldEvent::Evacuated {
                    station: id,
                    aboard: e.aboard.len() as u32,
                });
            }
        }
        // The rest posted round the flag, in the site's room.
        let flag = bims::math::vec2(e.at.0 as f32, e.at.1 as f32);
        let spot_in_site = self.aboard.to_station(dvec2(flag.x as f64, flag.y as f64));
        if let (Some(spot), Some(residents)) = (spot_in_site, self.residents.as_mut()) {
            let middle = residents.aboard.to_room(spot);
            let tile = shipdesign::TILE as f32;
            for (k, &who) in people
                .iter()
                .filter(|&&who| !e.aboard.contains(&(who as u32)))
                .enumerate()
            {
                let at = middle + defense::enemy_spot(k + 1) * (tile * 1.2);
                let room = &mut residents.aboard.room;
                let far = room
                    .post_of(who)
                    .is_none_or(|post| (post - at).len() > tile * 1.5);
                if far {
                    room.post_at(who, at);
                }
            }
        }
        // The end: every one alive aboard, or none left at all.
        let alive = people.len() as u32;
        let saved = e
            .aboard
            .iter()
            .filter(|&&w| people.contains(&(w as usize)))
            .count() as u32;
        let total = e.total.max(1);
        let held = saved > 0 && saved == alive;
        let failed = alive == 0;
        if let Some(d) = self.defense_mut(id) {
            d.evacuation = Some(e);
            if held {
                d.won = true;
            } else if failed {
                d.lost = true;
            }
        }
        if held {
            // The reward the share of them saved.
            self.run.pending_bounty = self.run.pending_bounty * u64::from(saved) / u64::from(total);
            self.bonus_wave_fought();
            events.push(WorldEvent::TownHeld { station: id });
        } else if failed {
            self.run.pending_bounty = 0;
            events.push(WorldEvent::TownFell { station: id });
        }
    }

    /// Where the flag stands at `id`'s Evacuation, in the site's own
    /// units: where the crew are stood as it begins.
    pub(super) fn evacuation_start(&self, id: u32) -> Option<(f64, f64)> {
        let e = self.defense(id)?.evacuation.as_ref()?;
        let p = self
            .aboard
            .to_station(dvec2(e.at.0 as f64, e.at.1 as f64))?;
        Some((p.x, p.y))
    }

    /// An Evacuation's people stood round its flag, the step it begins —
    /// in the shelter with the crew rather than about the site.
    pub(super) fn stand_the_evacuees(&mut self, id: u32) {
        let Some(at) = self.evacuation_start(id) else {
            return;
        };
        let people = self.evacuees();
        let Some(residents) = self.residents.as_mut().filter(|r| r.station == id) else {
            return;
        };
        let middle = residents.aboard.to_room(dvec2(at.0, at.1));
        let tile = shipdesign::TILE as f32;
        for (k, &who) in people.iter().enumerate() {
            let spot = middle + defense::enemy_spot(k + 1) * (tile * 1.2);
            let room = &mut residents.aboard.room;
            room.stand_at(who, spot);
            room.post_at(who, spot);
        }
    }

    /// Where a wave at an Evacuation lands, in the site's own units, and
    /// which way it faces: on the walk to the ship from whichever is
    /// nearest the ship by it of the flag and the players' Bims fit to act
    /// on the site's deck, part of the way along ([`AHEAD_MOST`]) — so it
    /// always stands between the crew and the ship. `None` where the site
    /// is no Evacuation under way or nothing is on its deck, and the wave
    /// comes in by its airlock or gate as anywhere.
    pub(super) fn evacuation_arrival(&self, station: &Station) -> Option<((f64, f64), f32)> {
        let (id, e) = self.evacuation_here()?;
        if id != station.id || self.defense(id).is_some_and(|d| d.over()) {
            return None;
        }
        let design = &station.design;
        let walk = Walk::from_port(design)?;
        let flag = match e.holder {
            Some(who) => self.aboard.room.bim_pos(who as usize),
            None => bims::math::vec2(e.at.0 as f32, e.at.1 as f32),
        };
        let mut points = vec![flag];
        for slot in 0..self.players() {
            if self.fit_to_act(slot) {
                points.push(self.aboard.room.bim_pos(slot as usize));
            }
        }
        let mut from: Option<((i32, i32), u32)> = None;
        for p in points {
            let Some(q) = self.aboard.to_station(dvec2(p.x as f64, p.y as f64)) else {
                continue;
            };
            let tile = tile_of(q);
            let Some(s) = walk.at(tile) else {
                continue;
            };
            if from.is_none_or(|(_, b)| s < b) {
                from = Some((tile, s));
            }
        }
        let (from, _) = from?;
        let path = walk.path_from(from);
        let last = path.len().checked_sub(1)?;
        let ahead = (path.len() / 2).clamp(AHEAD_LEAST, AHEAD_MOST).min(last);
        let spot = path[ahead];
        let at = tile_middle((spot.0 as u32, spot.1 as u32));
        let back = tile_middle((from.0 as u32, from.1 as u32)).sub(at);
        let facing = bims::math::vec2(back.x as f32, back.y as f32).angle();
        Some(((at.x, at.y), facing))
    }

    /// The flag put in that player's Bim's hands outright, for the tests.
    pub fn give_flag_for_probe(&mut self, slot: u32) {
        let Some(id) = self.ship.state.station() else {
            return;
        };
        if let Some(e) = self.defense_mut(id).and_then(|d| d.evacuation.as_mut()) {
            e.holder = Some(slot);
        }
    }
}

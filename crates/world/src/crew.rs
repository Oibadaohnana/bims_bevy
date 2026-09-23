//! The crew: the room's simulation, aboard the ship.
//!
//! This is the fifth stage of [`crate::World::step`], and it is not a copy
//! of the room — it *is* the room. `bims::game::Game` is the Bims with
//! their needs and errands, the galley, the heads, the bay and the deck, and
//! `bims::aboard` lays one out from the accepted [`ShipDesign`] so that
//! every fixture is where the designer put it. One update per world step,
//! on the world's clock: the room's own `update` takes real seconds at 1x,
//! and a world step is exactly one sixtieth of one.
//!
//! # Where a Bim is
//!
//! In **design world units**, about the design's origin, `y` growing down
//! the grid — the coordinates the ship's own parts are in, so that the
//! ship's position, heading and acceleration do not reach them. The room
//! draws itself in those units too, and the ship painter turns the whole
//! picture with the hull.
//!
//! # Bim *i* at bunk *i*
//!
//! Bunks in id order, and ids only ever climb and are never reissued, so
//! the pairing is the same on every client and survives every edit that
//! did not touch the bunks. `bims::aboard::starts` is that rule.
//!
//! # What is not here yet
//!
//! The room has [`bims::room::BERTHS`] beds and as many seats, so at most
//! two of a crew are simulated — a Bim's index is its berth and its seat,
//! and the room has two of each. The cold store is stocked from the cargo
//! when the world opens and is the room's from then on; what the crew eat
//! and grow does not yet come back off the manifest.

use bims::bim::Bim;
use bims::character::Uniform;
use bims::combat::Gear;
use bims::game::{Container, Game as Room};
use bims::manager::Stock;
use bims::math::{Rect, vec2};
use bims::sight::Fog;
use economy::Money;
use shipdesign::parts::{Layer, TILE};
use shipdesign::{ShipDesign, dock};
use worldgen::math::{DVec2, dvec2};

use crate::data;
use crate::docking::Joined;
use crate::memory::Grave;
use crate::station::{Berth, Station};

/// A world step, in the room's own unit: real seconds at 1x.
const STEP_SECONDS: f32 = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;

/// The room aboard, and the crew in it.
///
/// Docked, it is the ship **and the station** as one deck — see
/// [`crate::docking`] — with the ship's crew in it and the station's
/// residents in a room of their own ([`Residents`]), and the ship's own
/// grid sitting `offset` into the room's. Everything that reads a position through here gets it in the
/// **ship's** frame, whichever room it is; only the painter and the pointer
/// need the offset, to put the room's picture and the room's coordinates
/// where the ship is.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aboard {
    pub room: Room,
    /// Where the ship's design origin sits in the room's grid, in design
    /// units. Nought for a ship on its own.
    pub offset: DVec2,
    /// How many of the room's Bims are the ship's crew: all of them, now
    /// that a station's residents keep their own room. Kept as a count so
    /// a second crew — another player's — has somewhere to go.
    pub crew: u32,
    /// The design the room is laid out on: the ship's, or the joined one.
    pub design: ShipDesign,
    /// Where the two sides of the airlock lead, in the room's units, while
    /// the rooms are joined: the corridor just inside the station's door,
    /// and the deck just inside the ship's. Where the station's people are
    /// sent before the ship casts off, and where its own are called back
    /// to. Neither for a ship on its own.
    pub ashore: Option<DVec2>,
    pub gangway: Option<DVec2>,
    /// Where a station design point lands in the room, while joined: the
    /// image of the station's origin and its two axes as unit steps in the
    /// room's grid. How the residents, walking about in their own room,
    /// are put on this deck for its doors to open for them — see
    /// [`Aboard::visit`].
    pub station_frame: Option<(DVec2, DVec2, DVec2)>,
    /// The station's box on the joined deck, in the room's units, while
    /// joined: what tells the station's fixtures from the ship's in a
    /// layout of the whole. See [`Aboard::leave_the_station_s`].
    pub station_box: Option<(DVec2, DVec2)>,
}

impl Aboard {
    /// The room laid out from `design`, with `crew` Bims at their bunks.
    pub fn new(design: &ShipDesign, crew: u32, seed: u64) -> Aboard {
        let mut room = bims::aboard::game_aboard(design, crew as usize, seed);
        // Drawn once before the first step, so the ship view has the room in
        // it from its first frame rather than from its first step.
        room.render();
        let crew = room_crew(&room);
        Aboard {
            room,
            offset: DVec2::ZERO,
            crew,
            design: design.clone(),
            ashore: None,
            gangway: None,
            station_frame: None,
            station_box: None,
        }
    }

    /// The ship and its station as one deck, with the ship's crew in it —
    /// each where they were standing in the ship's room, now in the joined
    /// one. The crew come from `Game::take_crew`, so nobody is mid-errand:
    /// a chain aimed at a fixture in a room that no longer exists is not a
    /// chain worth keeping. The station's residents are not moved: they
    /// stay in their own room, with their own fixtures.
    /// On a planet, `terrain` is the plain the town stands on, and the
    /// room is laid out on it — the ground round the deck walked and seen
    /// (`bims::aboard::layout_of_on`) — with the station's frame in the
    /// room, in whole tiles, as the plane's.
    pub fn joined(
        joined: Joined,
        ship: &ShipDesign,
        station: &ShipDesign,
        crew: Vec<Bim>,
        seed: u64,
        minutes: f64,
        terrain: Option<bims::terrain::Terrain>,
    ) -> Aboard {
        let plane = terrain.map(|terrain| {
            let t = TILE as f64;
            let tiles = |v: DVec2| ((v.x / t).round() as i32, (v.y / t).round() as i32);
            let unit = |v: DVec2| (v.x.round() as i32, v.y.round() as i32);
            bims::terrain::Plane::new(
                terrain,
                tiles(joined.station_origin),
                unit(joined.station_ex),
                unit(joined.station_ey),
                (0, 0, 0, 0),
            )
        });
        let mut layout = bims::aboard::layout_of_on(&joined.design, plane.as_ref());
        // The station's box on the joined deck: its four corners through
        // the join, since a station docked side on is turned.
        let side = station.build_area as f64 * TILE as f64;
        let corners = [
            joined.from_station(dvec2(0.0, 0.0)),
            joined.from_station(dvec2(side, 0.0)),
            joined.from_station(dvec2(0.0, side)),
            joined.from_station(dvec2(side, side)),
        ];
        let (mut lo, mut hi) = (corners[0], corners[0]);
        for c in &corners {
            lo = dvec2(lo.x.min(c.x), lo.y.min(c.y));
            hi = dvec2(hi.x.max(c.x), hi.y.max(c.y));
        }
        let station_box = Some((lo, hi));
        Self::leave_the_station_s(station_box, &mut layout);
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut room = Room::with_layout(layout, seed, &[], w, h);
        let shift = joined.ship_shift();
        // Either side of the passage: a few tiles in from each door, along
        // the way it opens, read backwards.
        let inside = |port: dock::Port| {
            let reach = crate::data::ASHORE_TILES * TILE as f64;
            dvec2(
                port.centre.0 - port.outward.0 as f64 * reach,
                port.centre.1 - port.outward.1 as f64 * reach,
            )
        };
        let gangway = dock::port(ship).map(|p| inside(p).add(shift));
        let ashore = dock::port(station).map(|p| joined.from_station(inside(p)));
        room.adopt(crew, vec2(shift.x as f32, shift.y as f32));
        let crew = room_crew(&room);
        room.wind_clock(minutes as f32);
        room.render();
        Aboard {
            room,
            offset: shift,
            crew,
            design: joined.design,
            ashore,
            gangway,
            station_frame: Some((joined.station_origin, joined.station_ex, joined.station_ey)),
            station_box,
        }
    }

    /// The same two hulls as one deck the other way round, for the station's
    /// people: `joined` from [`crate::docking::join_mirror`], the station at
    /// its own coordinates plus the shift and the ship turned in, with
    /// `everybody` — the residents out of their old room through
    /// `Game::take_crew`, so nobody is mid-errand — put back where they
    /// stood, shifted. The ship's box is the foreign one here, and its
    /// fixtures are left to the crew (`leave_the_station_s`, which knows
    /// only a box). No gangway and no ashore: nobody walks these home.
    pub fn mirrored(
        joined: Joined,
        ship: &ShipDesign,
        everybody: Vec<Bim>,
        seed: u64,
        minutes: f64,
    ) -> Aboard {
        let mut layout = bims::aboard::layout_of(&joined.design);
        let side = ship.build_area as f64 * TILE as f64;
        let corners = [
            joined.from_station(dvec2(0.0, 0.0)),
            joined.from_station(dvec2(side, 0.0)),
            joined.from_station(dvec2(0.0, side)),
            joined.from_station(dvec2(side, side)),
        ];
        let (mut lo, mut hi) = (corners[0], corners[0]);
        for c in &corners {
            lo = dvec2(lo.x.min(c.x), lo.y.min(c.y));
            hi = dvec2(hi.x.max(c.x), hi.y.max(c.y));
        }
        let station_box = Some((lo, hi));
        Self::leave_the_station_s(station_box, &mut layout);
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut room = Room::with_layout(layout, seed, &[], w, h);
        let shift = joined.ship_shift();
        let count = everybody.len() as u32;
        room.adopt(everybody, vec2(shift.x as f32, shift.y as f32));
        room.wind_clock(minutes as f32);
        room.render();
        Aboard {
            room,
            offset: shift,
            crew: count,
            design: joined.design,
            ashore: None,
            gangway: None,
            station_frame: Some((joined.station_origin, joined.station_ex, joined.station_ey)),
            station_box,
        }
    }

    /// The station's fixtures are the residents' to work and to draw —
    /// their own room does both — so on the joined deck they are furniture
    /// to walk round and nothing more: not a bay the crew would go and
    /// tend, not a still painted over the residents' live picture of the
    /// same hob. Told apart by where they stand, since the two hulls never
    /// overlap. Nothing to do for a ship on its own. Asked at the join and
    /// at every relayout after, since a part built relays the whole deck.
    fn leave_the_station_s(station_box: Option<(DVec2, DVec2)>, layout: &mut bims::room::Layout) {
        let Some((lo, hi)) = station_box else {
            return;
        };
        let on_station = |r: &bims::math::Rect| {
            let m = r.center();
            (m.x as f64) >= lo.x
                && (m.x as f64) <= hi.x
                && (m.y as f64) >= lo.y
                && (m.y as f64) <= hi.y
        };
        layout.extras.retain(|(_, r)| !on_station(r));
        let more = &mut layout.more;
        more.worktops.retain(|r| !on_station(r));
        more.hobs.retain(|r| !on_station(r));
        more.fridges.retain(|r| !on_station(r));
        more.dishwashers.retain(|r| !on_station(r));
        more.lockers.retain(|r| !on_station(r));
        more.showers.retain(|(r, _)| !on_station(r));
        more.bays.retain(|(r, _)| !on_station(r));
        more.fields.retain(|(r, _)| !on_station(r));
        more.heads.retain(|(r, _)| !on_station(r));
    }

    /// The sky over the other hull: daylight over the station's box on
    /// this deck, so a town's ground is lit whatever its lamps say
    /// (`Game::set_daylight`). Asked at a landing, of the crew's joined
    /// room; nothing for a ship on its own, and a station's deck is
    /// under a roof.
    pub fn daylight_over_station(&mut self) {
        // On the plain the whole box is under the sky, the ground round
        // the ship included.
        if self.room.plane().is_some() {
            let over = self.room.interior();
            self.room.set_daylight(Some(over));
            return;
        }
        let over = self.station_box.map(|(lo, hi)| {
            Rect::from_corners(
                vec2(lo.x as f32, lo.y as f32),
                vec2(hi.x as f32, hi.y as f32),
            )
        });
        self.room.set_daylight(over);
    }

    /// Where the station's people are, in the station's own units, put on
    /// this deck as bodies for the doors to open for. They are not in the
    /// room — they walk in their own — but the room draws the doors, and a
    /// resident walking through a shut-looking door would be a door lying.
    /// And which of them are `down` — dead or out cold in their own room,
    /// index for index — since one that is is a body under a right-click
    /// on this deck (`HIT_VISITOR`), for looting. Told *after* the
    /// positions every time: `set_visitors` clears the flags.
    pub fn visit(&mut self, residents: &[DVec2], down: &[bool]) {
        let Some((origin, ex, ey)) = self.station_frame else {
            return;
        };
        let visitors: Vec<bims::math::Vec2> = residents
            .iter()
            .map(|p| {
                let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
                vec2(at.x as f32, at.y as f32)
            })
            .collect();
        self.room.set_visitors(visitors);
        self.room.set_visitors_down(down);
    }

    /// A point of the station's own design as a point of the joined deck,
    /// in the room's units: `station_frame` applied, the inverse of
    /// [`Aboard::to_station`]. `None` for a ship on its own.
    pub fn from_station(&self, p: DVec2) -> Option<DVec2> {
        let (origin, ex, ey) = self.station_frame?;
        Some(origin.add(ex.scale(p.x)).add(ey.scale(p.y)))
    }

    /// The station's people as targets, at those same positions, in the
    /// room's units: one an index, `None` for one that is down. What the
    /// room's Bims in combat mode shoot at — see `bims::combat`.
    pub fn hostiles(&self, residents: &[DVec2], alive: &[bool]) -> Vec<Option<bims::math::Vec2>> {
        let Some((origin, ex, ey)) = self.station_frame else {
            return Vec::new();
        };
        residents
            .iter()
            .enumerate()
            .map(|(who, p)| {
                if !alive.get(who).copied().unwrap_or(false) {
                    return None;
                }
                let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
                Some(vec2(at.x as f32, at.y as f32))
            })
            .collect()
    }

    /// A point of the joined deck, in the room's units, as a point of the
    /// station's own design — the inverse of `station_frame`, which is
    /// two unit axes about an origin, so the answer is two projections.
    /// `None` for a ship on its own, which has no station frame to be in.
    pub fn to_station(&self, p: DVec2) -> Option<DVec2> {
        let (origin, ex, ey) = self.station_frame?;
        let d = p.sub(origin);
        Some(dvec2(d.x * ex.x + d.y * ex.y, d.x * ey.x + d.y * ey.y))
    }

    /// A point of the room as a point of the design it came from: the
    /// **foreign** one's, through the frame, for a point in the foreign
    /// box — the station's on the crew's deck, the ship's on the
    /// residents' mirrored one — else the room's own design's, the shift
    /// off. Whether it was the foreign one comes back with it. What a
    /// lamp is remembered by across a relayout (`World::lamps`).
    pub fn design_of(&self, p: DVec2) -> (bool, DVec2) {
        if let (Some((lo, hi)), Some(q)) = (self.station_box, self.to_station(p))
            && p.x >= lo.x
            && p.x <= hi.x
            && p.y >= lo.y
            && p.y <= hi.y
        {
            return (true, q);
        }
        (false, p.sub(self.offset))
    }

    /// The other way: a design point back into the room — the foreign
    /// design's through the frame, `None` while there is none, or the
    /// room's own, shifted.
    pub fn room_of(&self, foreign: bool, p: DVec2) -> Option<DVec2> {
        if foreign {
            self.from_station(p)
        } else {
            Some(p.add(self.offset))
        }
    }

    /// Where the crew stand, as the station's people would find them: in
    /// the station's own design units, one an index, `None` for one that
    /// is dead, out cold — a body down is nobody's target — or outside in
    /// a suit. On the ship's own deck too, since September 2026: the
    /// residents' room holds the ship as well (`Residents::join`), so its
    /// people can walk the passage after a crew member that runs aboard.
    /// What a hostile station's room is handed as its targets, so
    /// its people shoot at where the crew actually are — which, for one
    /// peeking round a corner, is the peek it leans out to
    /// (`Game::exposed_at`), not the wall it stands behind; `crew_peeking`
    /// says which, index for index. Empty for a ship on its own.
    pub fn crew_ashore(&self) -> Vec<Option<DVec2>> {
        if self.station_frame.is_none() {
            return Vec::new();
        }
        (0..self.crew as usize)
            .map(|who| {
                if !self.room.is_alive(who)
                    || self.room.is_unconscious(who)
                    || self.room.is_outside(who)
                {
                    return None;
                }
                let p = self.room.exposed_at(who);
                self.to_station(dvec2(p.x as f64, p.y as f64))
            })
            .collect()
    }

    /// Which of the crew are peeking from cover, index for index with
    /// `crew_ashore`: a bolt reaching one is dodged half the time
    /// (`bims::combat::DODGE_IN_COVER`), and the other room is told so
    /// after its targets (`Game::set_hostiles_peeking`).
    pub fn crew_peeking(&self) -> Vec<bool> {
        (0..self.crew as usize)
            .map(|who| self.room.peek(who).is_some())
            .collect()
    }

    /// Which of the station's people, at those same positions, the crew can
    /// see from where they stand on the joined deck: one flag each, for
    /// the station's own room to draw them by.
    pub fn seen(&self, residents: &[DVec2]) -> Vec<bool> {
        let Some((origin, ex, ey)) = self.station_frame else {
            return vec![false; residents.len()];
        };
        residents
            .iter()
            .map(|p| {
                let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
                self.room.seen_at(at.x as f32, at.y as f32)
            })
            .collect()
    }

    /// Take the room apart again: the ship's crew back into a room of the
    /// ship alone. Anybody still on the station's deck is stood at their
    /// bunk — `adopt` does that for a position the new room has no floor
    /// under — which is the ship leaving without waiting. The crew are
    /// `everybody`, already out of the joined room through
    /// `Game::take_crew`: the world takes them itself, so that it can read
    /// what the leaving banked in the old room — a sheaf still in hand —
    /// off it before the room goes.
    pub fn unjoined(
        self,
        everybody: Vec<Bim>,
        ship: &ShipDesign,
        seed: u64,
        minutes: f64,
    ) -> Aboard {
        let offset = self.offset;
        let layout = bims::aboard::layout_of(ship);
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut room = Room::with_layout(layout, seed, &[], w, h);
        room.adopt(everybody, vec2(-offset.x as f32, -offset.y as f32));
        room.wind_clock(minutes as f32);
        room.render();
        let crew = room_crew(&room);
        Aboard {
            room,
            offset: DVec2::ZERO,
            crew,
            design: ship.clone(),
            ashore: None,
            gangway: None,
            station_frame: None,
            station_box: None,
        }
    }

    /// The room laid out again on `design` — the ship as it now is, or the
    /// joined deck as it now is — under the crew, who keep their errands
    /// and where they stand. What a part being built calls for, as against
    /// docking, which takes the room apart: a wall is not a reason to
    /// abandon everybody's afternoon. The ship's offset into the room does
    /// not move — a join's shift is the station's corners against the
    /// ship's build area, and neither changes when a part goes on.
    pub fn relayout(&mut self, design: ShipDesign) {
        let mut layout = bims::aboard::layout_of_on(&design, self.room.plane());
        Self::leave_the_station_s(self.station_box, &mut layout);
        self.room.relayout(layout);
        self.design = design;
    }

    /// Whether one of them is on the ship: standing on a tile of the
    /// ship's own frame, or in the passage between the two collars. Asked
    /// of the ship's design rather than the joined one, because the joined
    /// one is the station too.
    pub fn on_ship(&self, who: u32, ship: &ShipDesign) -> bool {
        let p = self.position(who);
        let t = TILE as f64;
        let tile = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        if ship.grid().get(Layer::Structure, tile) != 0 {
            return true;
        }
        // The passage: the tile beyond each airlock tile, the way it opens.
        dock::port(ship)
            .and_then(|port| ship.part(port.part_id).map(|a| (port, a.tiles())))
            .is_some_and(|(port, tiles)| {
                tiles
                    .iter()
                    .any(|&(x, y)| (x as i32 + port.outward.0, y as i32 + port.outward.1) == tile)
            })
    }

    /// Everybody to their own side of the airlock: the station's people
    /// ashore, the ship's back aboard. Sent once each, and again only when
    /// an errand has taken one back across — a Bim handed a fresh route
    /// every step never moves. The station's people are *posted* ashore,
    /// since they are let go with the station anyway; the crew are only
    /// walked back, or they would stand at the airlock for the rest of the
    /// voyage. Whether they are all there yet is [`Aboard::everybody_home`];
    /// nothing here waits.
    pub fn send_everybody_home(&mut self, ship: &ShipDesign) {
        let near = |a: Option<bims::math::Vec2>, b: bims::math::Vec2| {
            a.is_some_and(|a| (a - b).len() <= TILE as f32)
        };
        for who in 0..self.count() {
            let crew = who < self.crew;
            let on_ship = self.on_ship(who, ship);
            let (belongs, to) = if crew {
                (on_ship, self.gangway)
            } else {
                (!on_ship, self.ashore)
            };
            let Some(to) = to else {
                continue;
            };
            let to = vec2(to.x as f32, to.y as f32);
            if belongs {
                continue;
            }
            let who = who as usize;
            if crew {
                // Already on the way: leave it be.
                if near(self.room.destination_for_probe(who), to) {
                    continue;
                }
                self.room.walk_to(who, to);
            } else {
                // Posted there already — to within the snap a route makes —
                // and on its way or standing: leave it be.
                if near(self.room.post_of(who), to) && !self.room.is_busy(who) {
                    continue;
                }
                self.room.send_to(who, to);
            }
        }
    }

    /// Whether the crew are all on the ship and the station's people all
    /// off it. True of a ship on its own.
    pub fn everybody_home(&self, ship: &ShipDesign) -> bool {
        (0..self.count()).all(|who| self.on_ship(who, ship) == (who < self.crew))
    }

    /// Whether this is the ship and a station as one room.
    pub fn is_joined(&self) -> bool {
        self.offset != DVec2::ZERO || self.count() > self.crew
    }

    /// The ship's own crew: the first `crew` of the room's Bims.
    pub fn crew_count(&self) -> u32 {
        self.crew
    }

    /// One step of the crew: what stage 5 does. The simulation only — the
    /// room is drawn by [`Aboard::render`] when the ship is, not every step.
    pub fn step(&mut self) {
        self.room.simulate(STEP_SECONDS);
    }

    /// Tell the room where the helm is while the ship wants somebody at it,
    /// in the ship's design units, or that it does not. The room offers the
    /// helm as a job off this — see `bims::work::Job::Helm`.
    pub fn set_helm(&mut self, seat: Option<DVec2>) {
        self.room.set_helm(seat.map(|s| {
            let at = s.add(self.offset);
            vec2(at.x as f32, at.y as f32)
        }));
    }

    /// Draw the room as it stands. Called by the ship painter once a frame.
    pub fn render(&mut self) {
        self.room.render();
    }

    /// How many bodies are simulated. Not always the crew count — see
    /// the module note — and, at a station the machines hold, the Bims
    /// **and the machines** (feature 83, `bims::droid`): one index
    /// space, the Bims first, which is what the world hands the other
    /// room as its targets and reads its hits back against.
    pub fn count(&self) -> u32 {
        self.room.body_count()
    }

    /// Where one of them is, in the **ship's** design world units — the
    /// room's, less the ship's offset into it. A machine's position as
    /// readily as a Bim's.
    pub fn position(&self, who: u32) -> DVec2 {
        if who >= self.count() {
            return DVec2::ZERO;
        }
        let p = self.room.body_pos(who as usize);
        dvec2(p.x as f64, p.y as f64).sub(self.offset)
    }

    /// A point of the design this room was laid out from, as a point of
    /// the room: the offset put on. The inverse of [`Aboard::to_design`].
    pub fn to_room(&self, design: DVec2) -> bims::math::Vec2 {
        let at = design.add(self.offset);
        vec2(at.x as f32, at.y as f32)
    }

    /// A point of the room as a point of its design: the offset taken off.
    pub fn to_design(&self, room: bims::math::Vec2) -> DVec2 {
        dvec2(room.x as f64, room.y as f64).sub(self.offset)
    }

    /// Where a shot at one of them is aimed, in the same units: the peek
    /// it leans out to while it aims from cover, else where it stands
    /// (`Game::exposed_at`). What the other room is handed as the
    /// target's position, so a Bim looking down a corridor is shot at
    /// where it looks from rather than at the wall it stands behind.
    pub fn exposed(&self, who: u32) -> DVec2 {
        if who >= self.count() {
            return DVec2::ZERO;
        }
        let p = self.room.exposed_at(who as usize);
        dvec2(p.x as f64, p.y as f64).sub(self.offset)
    }

    /// Whether one of them is standing on a deck tile of the room's design.
    pub fn on_deck(&self, who: u32) -> bool {
        let p = self.room.bim_pos(who as usize);
        let t = shipdesign::TILE as f32;
        let tile = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        self.design.grid().get(shipdesign::Layer::Floor, tile) != 0
    }

    /// The room's clock, in game minutes. It is the world's clock read
    /// through the room, and the two agreeing is what
    /// `the_crew_keep_the_world_s_clock` holds.
    pub fn minutes(&self) -> f64 {
        self.room.clock_minutes() as f64
    }

    /// Every container the crew can reach into, by the room's own
    /// indices: each bench (the armoury among them, told by its part
    /// code), each shelf, each cold store. What a stow or a fetch is
    /// checked for reach against — see `World::container_takes`. The room
    /// keeps the lists private and answers by index, so this counts up
    /// until it stops answering.
    pub fn containers(&self) -> Vec<Container> {
        let mut out: Vec<Container> = (0..self.room.benches().len())
            .map(Container::Bench)
            .collect();
        for make in [Container::Shelf, Container::Fridge, Container::Desk] {
            let mut i = 0;
            while self.room.container_frame(make(i)).is_some() {
                out.push(make(i));
                i += 1;
            }
        }
        out
    }
}

fn room_crew(room: &Room) -> u32 {
    room.crew_count()
}

impl core::fmt::Debug for Aboard {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Aboard({} crew, {:.1} min)",
            self.count(),
            self.minutes()
        )
    }
}

/// The people living on a station: the room again, laid out on the
/// station's design, opened when the ship comes near and closed when it
/// leaves. See `World::settle_residents`.
///
/// Opened rather than kept: a station's room is not simulated while nobody
/// is there to see it, and when the ship comes back the residents start
/// afresh at their bunks. What they were doing an hour ago is not state the
/// world carries — it is the honest limit of this step, and it is why a
/// derelict, with nobody aboard, never gets a room at all.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Residents {
    pub station: u32,
    pub aboard: Aboard,
    /// Which of them the world has already said are down, by index, so
    /// `WorldEvent::EnemyDown` is said once. A shot to the head kills at
    /// the top of the body's next tick with the health total still well
    /// above nought, so "down" is asked of the room each step rather
    /// than read off the hit — see `World::visit`.
    pub down: Vec<bool>,
    /// Which of them the crew have already been given experience for
    /// going down — out cold or dead — and for dying, by index, so each
    /// enemy counts once for each (feature 74, `crate::class`).
    pub xp_down: Vec<bool>,
    pub xp_dead: Vec<bool>,
    /// Which crew member last landed a hit on each of them, by index —
    /// a bolt's, a blow's or a burst's shooter, `None` for a sentry's —
    /// so the soldier's *rampage* knows who downed whom (feature 75).
    pub last_hit_by: Vec<Option<usize>>,
    /// Which of them are mercenaries for hire, by index, and what a month
    /// of each costs (`crate::mercenary::priced`); `None` for one of the
    /// station's own. Derived from the seed and the crew's worth when the
    /// room opens, like the gear, so nothing new is hashed.
    pub fee: Vec<Option<Money>>,
    /// Which of them were laid out dead when the room opened, by index
    /// (feature 85): a body the station already had, out of
    /// `World::graves`, rather than somebody who died while this room was
    /// open. Its death is already in `World::losses`, so
    /// `World::close_residents` must not count it a second time.
    pub grave: Vec<bool>,
}

impl Residents {
    /// The station's room, with its people at their bunks and its clock
    /// wound on to the world's, so a station reached at noon is not at
    /// breakfast.
    /// `count` is the station's own people and `mercenaries` how many
    /// hired hands live among them (`crate::mercenary::how_many`): the
    /// last that many bodies, in the mercenary's coverall and kit and
    /// priced, as many as the bunks will take after the residents.
    ///
    /// **A station's room holds no more than it has bunks** — a garrison
    /// bigger than an orbital's four is four, which is what the arena's
    /// twenty are for — the cut made here, since the room itself takes
    /// whatever it is given (`Game::with_layout`): the ship's crew is the
    /// world's count and may run past the ship's bunks. A design with no
    /// bunk at all still has the room's one stand-in berth.
    pub fn open(
        station: u32,
        design: &ShipDesign,
        count: u32,
        mercenaries: u32,
        seed: u64,
        minutes: f64,
        graves: &[Grave],
    ) -> Residents {
        let bunks = design.count(shipdesign::PartKind::Bunk).max(1);
        let count = count.min(bunks);
        let mercenaries = mercenaries.min(bunks - count);
        // And the dead this station has already (feature 85), on the end:
        // bodies, not people, so the bunks have nothing to say about how
        // many of them there are.
        let mut aboard = Aboard::new(design, count + mercenaries + graves.len() as u32, seed);
        aboard.room.wind_clock(minutes as f32);
        // Looked at from outside: the crew see none of it, and nobody in
        // it is drawn, until the ship docks and the rooms are joined.
        aboard.room.set_fog(Fog::All);
        // The station's coverall, so they can be told from the crew, and
        // a weapon each — rolled off the station's own seed and the seat,
        // so the same station arms the same people the same way every
        // time it is reached, and `world_checksum` has nothing new to
        // hash: the kind is a function of what it already holds.
        let mut fee = vec![None; aboard.count() as usize];
        for who in 0..count + mercenaries {
            if who < count {
                aboard.room.set_uniform(who as usize, Uniform::Station);
                aboard
                    .room
                    .issue(who as usize, Gear::issued_for(seed ^ who as u64));
            } else {
                // A mercenary: the olive coverall, its own kit off its own
                // seed — the pieces numbered from a thousand a head so no
                // two bodies' collide in this room — and its price.
                let n = who - count;
                let merc_seed = crate::mercenary::seed_for(seed, n);
                let gear = Gear::hired_for(merc_seed, 1_000 * (who + 1));
                aboard.room.set_uniform(who as usize, Uniform::Mercenary);
                aboard.room.issue(who as usize, gear);
                fee[who as usize] = Some(crate::mercenary::priced(merc_seed, &gear));
            }
        }
        // The dead this station already has, laid where they fell
        // (feature 85): the coverall they wore, what was left on them and
        // the face they had, and then the body put down on the spot the
        // grave names. Their deaths are the station's losses already, so
        // nothing here is counted again.
        for (n, grave) in graves.iter().enumerate() {
            let who = (count + mercenaries) as usize + n;
            let uniform = if grave.hired {
                Uniform::Mercenary
            } else {
                Uniform::Station
            };
            aboard.room.set_uniform(who, uniform);
            aboard.room.set_look(who, grave.look);
            aboard.room.issue(who, grave.gear);
            aboard
                .room
                .lay_out_dead(who, vec2(grave.x as f32, grave.y as f32));
        }
        // Their own manager's goals, a head each — the crew's Management tab
        // is the crew's, and reaches nobody ashore — and a larder already
        // at them, since they have been living here. The dead eat nothing.
        let each = count + mercenaries;
        let (veg, tofu, stew) = (
            data::RESIDENT_VEG_EACH * each,
            data::RESIDENT_TOFU_EACH * each,
            data::RESIDENT_STEW_EACH * each,
        );
        aboard.room.set_target(Stock::Veg, veg);
        aboard.room.set_target(Stock::Tofu, tofu);
        aboard.room.set_target(Stock::Stew, stew);
        // No fibre: the residents grow none unasked, and nobody asks.
        aboard.room.set_stock(veg, tofu, stew, 0);
        // A couple of bandages in the station's locker, so its people can
        // dress a wound of their own. The world keeps no hold for a
        // station, so this is the count for as long as the room is open.
        aboard.room.set_bandages(data::RESIDENT_BANDAGES);
        aboard.room.set_medkits(data::RESIDENT_MEDKITS);
        aboard.room.render();
        // A town is under a sky: its whole ground is lit by day, whatever
        // its lamps reach, and the lamps are for the houses.
        if crate::surface::surface_body(station).is_some() {
            aboard.room.set_daylight(Some(ground_of(design)));
        }
        // A body laid out is down already, and the crew were paid for it
        // the day they shot it: flagged on all three, so nothing is said
        // about it again and nobody is paid twice.
        let mut down = vec![false; aboard.count() as usize];
        let mut grave = vec![false; aboard.count() as usize];
        for who in (count + mercenaries) as usize..aboard.count() as usize {
            down[who] = true;
            grave[who] = true;
        }
        Residents {
            station,
            aboard,
            xp_down: down.clone(),
            xp_dead: down.clone(),
            last_hit_by: vec![None; down.len()],
            down,
            fee,
            grave,
        }
    }

    /// The station and the ship as one deck for its people, the ship turned
    /// into the station's frame (`docking::join_mirror`), so that they can
    /// walk the passage onto the ship: what `World::join_rooms` does to
    /// this room as it makes the crew's. Everybody comes across where
    /// they stood — errands dropped, like the crew's at a dock — with the
    /// room's own counts: the larder, its targets, the bandages and the
    /// kits. Nothing happens when the two have no ports to join by.
    pub fn join(
        &mut self,
        ship: &ShipDesign,
        com: DVec2,
        station: &Station,
        berth: &Berth,
        minutes: f64,
    ) {
        let Some(joined) = crate::docking::join_mirror(ship, com, station, berth) else {
            return;
        };
        let seed = station.map_seed ^ 0x5A17;
        let everybody = self.aboard.room.take_crew();
        // And the machines with them (feature 83): a fresh room has none,
        // and a wave standing at the door does not vanish because the
        // crew docked. They are shifted the way `adopt` shifts the Bims.
        let machines = self.aboard.room.take_droids();
        // The station's people watch their own airlock: the tile just
        // inside its door, in this room's units — the station's design
        // plus the shift its deck took — so a crew member coming through
        // it is seen (`Game::set_watched`, `bims::game::AIRLOCK_WATCH`)
        // and a boarding is what starts the fight at a hostile station.
        let inside = station.port().map(|port| {
            let t = TILE as f64;
            dvec2(
                port.centre.0 - port.outward.0 as f64 * t,
                port.centre.1 - port.outward.1 as f64 * t,
            )
        });
        let mut fresh = Aboard::mirrored(joined, ship, everybody, seed, minutes);
        // The machines across, by the same shift the Bims took.
        let shift = fresh.offset;
        fresh
            .room
            .adopt_droids(machines, vec2(shift.x as f32, shift.y as f32));
        fresh.crew = fresh.room.body_count();
        let watched = inside.map(|p| fresh.to_room(p));
        fresh.room.set_watched(watched);
        // A town's ground under the sky again: the station's own area,
        // shifted to where the mirror laid it. Not `station_box`, which
        // is the ship's here, and the ship has a roof.
        if crate::surface::surface_body(self.station).is_some() {
            let ground = ground_at(&station.design, fresh.offset);
            fresh.room.set_daylight(Some(ground));
        }
        self.replace_room(fresh);
    }

    /// The station's own room again, the ship gone: everybody back where
    /// they stood, less the shift, and one still aboard the ship — its
    /// deck is no longer here — at its bunk (`Game::adopt`).
    pub fn unjoin(&mut self, station: &Station, minutes: f64) {
        if self.aboard.station_frame.is_none() {
            return;
        }
        let offset = self.aboard.offset;
        let seed = station.map_seed ^ 0x5A17;
        let everybody = self.aboard.room.take_crew();
        let machines = self.aboard.room.take_droids();
        let layout = bims::aboard::layout_of(&station.design);
        let (w, h) = (layout.bounds.width(), layout.bounds.height());
        let mut room = Room::with_layout(layout, seed, &[], w, h);
        room.adopt(everybody, vec2(-offset.x as f32, -offset.y as f32));
        // And the machines back off the joined deck onto the station's
        // own, by the same shift (feature 83).
        room.adopt_droids(machines, vec2(-offset.x as f32, -offset.y as f32));
        let count = room.body_count();
        room.wind_clock(minutes as f32);
        room.render();
        // A town's own ground under the sky again, as at the open.
        if crate::surface::surface_body(self.station).is_some() {
            room.set_daylight(Some(ground_of(&station.design)));
        }
        let fresh = Aboard {
            room,
            offset: DVec2::ZERO,
            crew: count,
            design: station.design.clone(),
            ashore: None,
            gangway: None,
            station_frame: None,
            station_box: None,
        };
        self.replace_room(fresh);
    }

    /// A fresh room in the old one's place, with what the old room held
    /// that is not a body's: the larder and its targets, the medicine, the
    /// fog and whether the doors are drawn. `down` and `fee` are by index
    /// and the indices are kept (`adopt` keeps the order).
    fn replace_room(&mut self, mut fresh: Aboard) {
        let old = &self.aboard.room;
        for which in [Stock::Veg, Stock::Tofu, Stock::Stew, Stock::Fibre] {
            fresh.room.set_target(which, old.target(which));
        }
        fresh.room.set_stock(
            old.store_veg(),
            old.store_tofu(),
            old.store_stew(),
            old.store_fibre(),
        );
        fresh.room.set_bandages(old.bandages());
        fresh.room.set_medkits(old.medkits());
        fresh.room.set_fog(old.fog());
        fresh.room.set_doors_drawn(old.doors_drawn());
        fresh.room.render();
        self.aboard = fresh;
    }

    /// A settlement's guard to its post: the first of its people, sent to
    /// stand at [`crate::surface::GUARD_POST`] outside the watch house —
    /// a post, so it goes off to eat and sleep and comes back to it
    /// (`Game::send_to`). Nothing at a station, which has no watch.
    /// Called whenever the room is built afresh — opened, joined,
    /// unjoined — since a fresh room drops every post; and false when
    /// there is nobody to post or no way there.
    pub fn post_guard(&mut self, station: &Station) -> bool {
        if station.plan != crate::station::Plan::Surface
            || self.aboard.count() <= crate::surface::GUARD
        {
            return false;
        }
        let at = self.aboard.to_room(crate::surface::guard_post());
        self.aboard.room.send_to(crate::surface::GUARD as usize, at)
    }

    /// A raider's boarders to the ship: every one of them alive and without
    /// a post is posted at the ship's **gangway** — the deck a few tiles
    /// inside the ship's airlock, [`data::ASHORE_TILES`] in the way the
    /// door opens, in this room's units through the mirror's frame — so
    /// they walk the passage onto the ship and hold its door, and force
    /// the airlock if it is locked against them (`Game::post_at`,
    /// `Game::breach`). A post, so a fight drops it (`Game::muster`) and
    /// the world asks again every step the raider is tied up: one that
    /// went back to its bunk after a fight is sent again. Nothing unless
    /// the ship is on this room's deck. How many were sent.
    pub fn post_boarders(&mut self, ship: &ShipDesign) -> u32 {
        let Some((origin, ex, ey)) = self.aboard.station_frame else {
            return 0;
        };
        let Some(port) = dock::port(ship) else {
            return 0;
        };
        let reach = crate::data::ASHORE_TILES * TILE as f64;
        let inside = dvec2(
            port.centre.0 - port.outward.0 as f64 * reach,
            port.centre.1 - port.outward.1 as f64 * reach,
        );
        let at = origin.add(ex.scale(inside.x)).add(ey.scale(inside.y));
        let at = vec2(at.x as f32, at.y as f32);
        let mut sent = 0;
        for who in 0..self.aboard.count() as usize {
            let room = &mut self.aboard.room;
            if !room.is_alive(who) || room.has_post(who) {
                continue;
            }
            if room.post_at(who, at) {
                sent += 1;
            }
        }
        sent
    }

    /// Which of them may be spoken to — a mercenary for hire, alive and on
    /// its feet — index for index, for the joined deck's
    /// `Game::set_visitors_hailable`.
    /// A machine is never one: `fee` is short of the body count until
    /// `visit` grows it, and a body with no entry has no fee.
    pub fn hailable(&self) -> Vec<bool> {
        (0..self.aboard.count() as usize)
            .map(|who| {
                self.fee.get(who).copied().flatten().is_some() && !self.aboard.room.is_down(who)
            })
            .collect()
    }
}

impl core::fmt::Debug for Residents {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Residents(station {}, {:?})", self.station, self.aboard)
    }
}

/// A design's whole area as a rect in the room's units, from its origin:
/// what the daylight covers on a town's own room, whose ground is the
/// whole of the design (`Game::set_daylight`).
fn ground_of(design: &ShipDesign) -> Rect {
    ground_at(design, DVec2::ZERO)
}

/// The same, laid at `at` — where a mirror put the station's origin.
fn ground_at(design: &ShipDesign, at: DVec2) -> Rect {
    let side = design.build_area as f32 * TILE as f32;
    Rect::from_min_size(vec2(at.x as f32, at.y as f32), vec2(side, side))
}

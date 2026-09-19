//! A station is a place: a hull on a grid, at a position, with a door.
//!
//! The world generator hands over a [`StationBlueprint`] — a kind, a
//! position and a `map_seed` — and this module turns it into a
//! [`ShipDesign`] the way the designer would have: the same parts, the same
//! rules, through [`shipdesign::apply`]. That buys three things at once.
//! The ship painter draws a station with the code it draws the ship with,
//! so a station sits in the same picture as the hull rather than beside it
//! as an icon. The room (`bims::aboard`) lays a station out exactly as it
//! lays a ship out, so the people living there are the room's Bims with the
//! room's needs and errands. And [`shipdesign::dock::port`] finds the
//! station's airlock the way it finds the ship's, which is what makes
//! docking airlock to airlock one piece of arithmetic asked twice.
//!
//! # Deterministic, and the same on both targets
//!
//! Everything here is drawn from the blueprint's `map_seed` through
//! `worldgen::rng`, in integers, and placed through `apply` — so a station
//! is the same station on a native server and in a browser, and the ship
//! docks in the same place on both to the unit.
//!
//! # A station does not turn
//!
//! Its heading is nought, always. North is up on every station's grid, so a
//! station's design coordinates go into the system through
//! [`flight::angle::rotate_design`] at zero: the flip between the grid's
//! y-down and the system's y-up and nothing else.
//!
//! # What a station is not, yet
//!
//! It is not a solid the ship cannot fly through. The ship docks *beside*
//! it — [`Station::berth`] puts the hull outside the station's, airlock to
//! airlock — and a trip to it ends outside its hull, but a trip *past* one
//! is a straight line whatever is in the way. Collision is the next step
//! and this is the shape it will need: a hull with a real extent, at a real
//! position.

use flight::angle;
use shipdesign::dock::{self, Port};
use shipdesign::parts::TILE;
use shipdesign::{Budget, Edit, Money, PartKind, Rotation, ShipDesign, apply};
use worldgen::math::{DVec2, dvec2};
use worldgen::rng::Rng;
use worldgen::system::StationBlueprint;
use worldgen::{Node, StarSystem, StationKind, Stock};

use crate::data;

/// Layouts already built, by kind and seed. A layout is a pure function of
/// the two, and building one is two thousand edits through `apply`, each of
/// which rebuilds the grid — cheap enough once, and a world opens every
/// station of its system at once. Looked up only, never walked, so the
/// order in it decides nothing.
static BUILT: std::sync::Mutex<Vec<((StationKind, u64), ShipDesign)>> =
    std::sync::Mutex::new(Vec::new());

/// How many tiles across a station's build area is, by kind. The hull fills
/// it bar a one-tile margin. Big next to a ship — the playtest ship is twenty
/// — because a station is where ships go, not a ship.
pub fn side_of(kind: StationKind) -> u32 {
    match kind {
        StationKind::Orbital => 64,
        StationKind::Refinery => 56,
        StationKind::MiningOutpost => 52,
        StationKind::Derelict => 54,
        StationKind::Relay => 48,
    }
}

/// How many people live aboard, by kind. The room simulates at most
/// [`bims::room::BERTHS`], a relay is a lonely posting, and nobody is left
/// on a derelict — which is why one never gets a room at all.
pub fn residents_of(kind: StationKind) -> u32 {
    match kind {
        StationKind::Derelict => 0,
        StationKind::Relay => 1,
        _ => bims::room::BERTHS as u32,
    }
}

/// The odds a friendly station has a research key on its desk when the
/// world opens: four in five.
pub const KEY_CHANCE: u32 = 80;

/// Whether a station's research desk holds a key, off its seed — a stream
/// of its own, so the layout's rolls are what they were. Only asked of a
/// station that is neither an enemy's nor a derelict.
pub fn key_rolled(map_seed: u64) -> bool {
    Rng::new(map_seed ^ 0x_4B45_5931).below(100) < KEY_CHANCE
}

/// How many enemies a hostile station holds against a crew of `crew`,
/// whose ship and hold are worth `worth` now and were worth `start_worth`
/// when the world opened: [`data::ENEMIES_BASE`] and one a crewmate,
/// doubled every time the worth has risen by another half of what it
/// started at, and never more than [`data::ENEMIES_MAX`]. Whole euros in
/// and a whole number out, so a server counts the same crowd.
///
/// A crew that has not got richer meets the baseline; a ship worth nothing
/// at the start has no half to grow by, and its crew meet it for good.
pub fn enemies_of(crew: u32, worth: Money, start_worth: Money) -> u32 {
    let baseline = data::ENEMIES_BASE.saturating_add(crew);
    let step = start_worth / 2;
    if step == 0 || worth <= start_worth {
        return baseline.min(data::ENEMIES_MAX);
    }
    let doublings = (worth - start_worth) / step;
    let mut count = baseline;
    for _ in 0..doublings {
        if count >= data::ENEMIES_MAX {
            break;
        }
        count *= 2;
    }
    count.min(data::ENEMIES_MAX)
}

/// Where a ship goes to be docked at a station: its centre of mass and its
/// heading, with the two airlocks' outer faces touching.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Berth {
    pub position: DVec2,
    pub heading: f64,
}

/// One station of the system, as a place.
#[derive(Clone, PartialEq, Debug)]
pub struct Station {
    pub id: u32,
    pub kind: StationKind,
    /// The layout: what the painter draws, what the room lays out, and what
    /// the port is found in.
    pub design: ShipDesign,
    /// Where design tile (0, 0) sits in the system. The grid's centre is on
    /// the blueprint's position.
    pub anchor: DVec2,
    /// The seed the residents' room is opened with.
    pub map_seed: u64,
    /// What is on its shelves; `World::buy` asks it and nothing else does.
    pub stock: Stock,
    /// Whether the people aboard are enemies, as the generator rolled it —
    /// the blueprint's word, carried here for the painter and for
    /// `World::start`. The rule a caller wants is `World::stance`: the
    /// spawn is home whatever this says, and nothing else overrides it.
    pub hostile: bool,
    /// Whether a tier-one research key was found on its research desk
    /// when the world opened: rolled off the seed at [`KEY_CHANCE`] for a
    /// station that is neither an enemy's nor a derelict. The blueprint's
    /// word; whether the key is still there is `World::station_keys`, and
    /// the spawn has one whatever this says.
    pub key: bool,
}

impl Station {
    /// Build the station the blueprint describes, standing at `at`.
    pub fn build(blueprint: &StationBlueprint, at: DVec2) -> Station {
        let design = layout(blueprint.kind, blueprint.map_seed);
        let half = design.build_area as f64 * TILE as f64 / 2.0;
        Station {
            id: blueprint.id,
            kind: blueprint.kind,
            anchor: at.sub(angle::rotate_design(dvec2(half, half), 0.0)),
            design,
            map_seed: blueprint.map_seed,
            stock: blueprint.stock,
            hostile: blueprint.hostile,
            key: !blueprint.hostile
                && blueprint.kind != StationKind::Derelict
                && key_rolled(blueprint.map_seed),
        }
    }

    /// Every station of a system, in id order.
    pub fn all_of(system: &StarSystem) -> Vec<Station> {
        system
            .stations
            .iter()
            .filter_map(|blueprint| {
                let at = system.absolute_position(Node::Station(blueprint.id))?;
                Some(Station::build(blueprint, at))
            })
            .collect()
    }

    pub fn residents(&self) -> u32 {
        residents_of(self.kind)
    }

    /// A design point — world units about the grid's origin, `y` down — in
    /// the system.
    pub fn to_system(&self, design: DVec2) -> DVec2 {
        self.anchor.add(angle::rotate_design(design, 0.0))
    }

    /// The inverse: a point of the system as a point of the design. A
    /// station never turns, so it is the anchor taken off.
    pub fn from_system(&self, system: DVec2) -> DVec2 {
        angle::unrotate_design(system.sub(self.anchor), 0.0)
    }

    /// The middle of the grid, in the system: the blueprint's position.
    pub fn centre(&self) -> DVec2 {
        let half = self.design.build_area as f64 * TILE as f64 / 2.0;
        self.to_system(dvec2(half, half))
    }

    /// How far from the centre the hull reaches, at most: the half-diagonal
    /// of the grid. A circle the hull is certainly inside, for "how near is
    /// the ship to the station" without walking the tiles.
    pub fn radius(&self) -> f64 {
        self.design.build_area as f64 * TILE as f64 * core::f64::consts::FRAC_1_SQRT_2
    }

    /// How far a point is from the station's hull, at least: distance to the
    /// centre less the radius, and never negative.
    pub fn clearance(&self, from: DVec2) -> f64 {
        (from.distance(self.centre()) - self.radius()).max(0.0)
    }

    pub fn port(&self) -> Option<Port> {
        dock::port(&self.design)
    }

    /// The outer face of the station's airlock, in the system, and the way
    /// it opens, as a unit vector. The way out for a ship pushing off.
    pub fn face(&self) -> Option<(DVec2, DVec2)> {
        let port = self.port()?;
        let (fx, fy) = port.face();
        let outward = dvec2(port.outward.0 as f64, port.outward.1 as f64);
        Some((
            self.to_system(dvec2(fx, fy)),
            angle::rotate_design(outward, 0.0),
        ))
    }

    /// Where `ship` docks: its centre of mass and heading with its airlock's
    /// outer face on the station's, opening the other way.
    ///
    /// A ship with no port cannot dock, and gets a berth all the same — held
    /// off the station's door by its own size, pointing north — because a
    /// world still opens with a ship alongside whether or not it can go
    /// aboard, and "beside the door" is where alongside is.
    pub fn berth(&self, ship: &ShipDesign, centre_of_mass: DVec2) -> Option<Berth> {
        let (face, outward) = self.face()?;
        let Some(port) = dock::port(ship) else {
            let reach = ship.build_area as f64 * TILE as f64 * core::f64::consts::FRAC_1_SQRT_2;
            return Some(Berth {
                position: face.add(outward.scale(reach + TILE as f64)),
                heading: 0.0,
            });
        };
        // The ship's door has to open the way the station's does not. Its
        // outward step has a bearing at heading nought; the heading is what
        // turns that bearing onto the opposite of the station's.
        let step = dvec2(port.outward.0 as f64, port.outward.1 as f64);
        let at_zero = angle::bearing(angle::rotate_design(step, 0.0));
        let heading = angle::wrap(angle::bearing(outward.scale(-1.0)) - at_zero);
        // Then the ship is placed so that its door's outer face lands on the
        // station's: the face is a design offset from the centre of mass,
        // turned through that heading.
        let (fx, fy) = port.face();
        let offset = angle::rotate_design(dvec2(fx, fy).sub(centre_of_mass), heading);
        Some(Berth {
            position: face.sub(offset),
            heading,
        })
    }
}

// --- the layout ---------------------------------------------------------------

/// The station's design, from its kind and its seed.
///
/// One plan for all of them, sized by kind and dressed by the seed. The
/// hull is a square with its corners cut back three tiles in
/// [`PartKind::DiagonalOutsideWall`] pieces, the port in the west skin and
/// the array on the north. Inside, two corridors three tiles wide cross in
/// the middle — the west one runs in from the port — and the four
/// quarters between them are rooms: the galley and mess to the north-west,
/// the crew's quarters with the heads along their north wall to the
/// north-east, hydroponics to the south-west, engineering to the
/// south-east. Every room has a doorway onto each corridor, two tiles
/// wide, and every fixture stands with two clear tiles in front of it,
/// because the room's navigation cannot walk a one-tile gap (see
/// `crates/shipdesign`'s module note) — a bulkhead here is only ever
/// where a body can still get round it with a tile to spare. A derelict
/// is the same hull with holes in it and nobody home.
///
/// The seed decides how many bays, shelves and batteries there are and
/// nothing else about the shape, so two seeds are two stations without
/// either being a different building.
pub fn layout(kind: StationKind, map_seed: u64) -> ShipDesign {
    if let Ok(built) = BUILT.lock()
        && let Some((_, design)) = built.iter().find(|(key, _)| *key == (kind, map_seed))
    {
        return design.clone();
    }
    let design = build_layout(kind, side_of(kind), 1, map_seed);
    if let Ok(mut built) = BUILT.lock() {
        built.push(((kind, map_seed), design.clone()));
    }
    design
}

/// The arena: the same plan as [`layout`], the same kind and seed, laid
/// out [`data::ARENA_SIDE`] tiles across — bigger than any kind of
/// station — with the quarters' bunks in [`data::ARENA_BUNK_COLUMNS`]
/// columns, since the room sleeps at most as many as it has bunks and a
/// garrison of [`data::ENEMIES_MAX`] wants a bunk each. What the
/// `combat` command rebuilds its dock as (`World::arena_dock_for_probe`):
/// longer corridors and bigger rooms to fight through, and a crowd to
/// fight. Not cached, since one is built a run.
pub fn arena(kind: StationKind, map_seed: u64) -> ShipDesign {
    build_layout(kind, data::ARENA_SIDE, data::ARENA_BUNK_COLUMNS, map_seed)
}

/// The hub's half-size, in hull tiles: a thirteen-tile square where the
/// four arms meet, eleven of deck inside its skin.
const HUB: u32 = 6;
/// An arm's half-width in hull tiles: seven across, a corridor five wide
/// inside its two walls.
const ARM: u32 = 3;
/// A docking lobby's half-width and depth, in hull tiles: the nine-by-seven
/// block at the end of each arm with the airlock in its outer skin. The
/// west one — the port's — is the reactor room as well, thirteen tall and
/// eleven deep, the corridor running through the middle of it.
const LOBBY: u32 = 4;
const LOBBY_DEPTH: u32 = 7;
const PORT_LOBBY: u32 = 6;
const PORT_LOBBY_DEPTH: u32 = 11;
/// How far past the lobby an arm's rooms begin, and how far short of the
/// hub they end: a tile of void between a room and the block beside it,
/// so the skin seals and the room is a room rather than a corner opened
/// into the hub.
const ROOM_GAP: u32 = 2;
/// How far out from the hub's skin each arm's barricade of sandbags stands.
const BARRICADE_OUT: u32 = 4;

/// A block of hull, tile ranges inclusive: the plan is a union of these.
#[derive(Clone, Copy)]
struct Block {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Block {
    fn new(x0: u32, y0: u32, x1: u32, y1: u32) -> Block {
        Block { x0, y0, x1, y1 }
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 as i32 && x <= self.x1 as i32 && y >= self.y0 as i32 && y <= self.y1 as i32
    }

    /// The deck inside the skin.
    fn inner(&self) -> Block {
        Block::new(self.x0 + 1, self.y0 + 1, self.x1 - 1, self.y1 - 1)
    }
}

/// The plan: a hub and four arms to a docking lobby each — the port in
/// the west one, the array on the north — with the rooms hung off the
/// arms the way the picture had them. Two rooms deep either side of the
/// north arm: the mess (the galley) inside and the crew's quarters beyond
/// it to the west, the heads inside and the laboratory (the bay) beyond
/// to the east; two deep either side of the south arm: the rec room and
/// the research room (more bays) to the west, the storage and the cargo
/// (the shelves) to the east; and the port's lobby, bigger than the
/// others, is the reactor room, with the trading desk by the door. Every
/// room has a two-tile doorway — the inner rooms onto their arm, the
/// outer rooms through the partition into the inner — and
/// a barricade of sandbags stands across three of each arm's five tiles
/// a few tiles out from the hub, the gap past it on alternate sides. A
/// tile of void between a room and the block beside it keeps the skin
/// sealed, since the hull is the union of the blocks: every tile in one
/// is frame and deck, and one with any of its eight neighbours outside
/// them all is outside wall instead. A derelict is the same hull with
/// holes in its skin and nobody home. Sized by `side`; forty-eight is
/// the smallest the rooms fit at.
fn build_layout(kind: StationKind, side: u32, bunk_columns: u32, map_seed: u64) -> ShipDesign {
    let budget = Budget::new(Money::MAX);
    let mut design = ShipDesign::new(side);
    let mut rng = Rng::new(map_seed);

    let put = |design: &mut ShipDesign, kind: PartKind, origin: (u32, u32), rotation| {
        if let Ok(next) = apply(
            design,
            &budget,
            Edit::Place {
                kind,
                origin,
                rotation,
            },
        ) {
            *design = next;
        }
    };
    let take = |design: &mut ShipDesign, tile: (u32, u32)| {
        let standing = design
            .grid()
            .get(shipdesign::Layer::Object, (tile.0 as i32, tile.1 as i32));
        if standing != 0
            && let Ok(next) = apply(design, &budget, Edit::Remove { part_id: standing })
        {
            *design = next;
        }
    };

    let mid = side / 2;
    let last = side - 2;
    // The hub, the four arms and their lobbies.
    let hub = Block::new(mid - HUB, mid - HUB, mid + HUB, mid + HUB);
    let lobby_w = Block::new(1, mid - PORT_LOBBY, PORT_LOBBY_DEPTH, mid + PORT_LOBBY);
    let lobby_e = Block::new(last - LOBBY_DEPTH + 1, mid - LOBBY, last, mid + LOBBY);
    let lobby_n = Block::new(mid - LOBBY, 1, mid + LOBBY, LOBBY_DEPTH);
    let lobby_s = Block::new(mid - LOBBY, last - LOBBY_DEPTH + 1, mid + LOBBY, last);
    let arm_w = Block::new(PORT_LOBBY_DEPTH, mid - ARM, mid - HUB, mid + ARM);
    let arm_e = Block::new(mid + HUB, mid - ARM, last - LOBBY_DEPTH + 1, mid + ARM);
    let arm_n = Block::new(mid - ARM, LOBBY_DEPTH, mid + ARM, mid - HUB);
    let arm_s = Block::new(mid - ARM, mid + HUB, mid + ARM, last - LOBBY_DEPTH + 1);
    // The rooms off the north and south arms, two deep a side: from a
    // tile past the lobby to a tile short of the hub, each as wide as
    // half the space between the arm and the hull's edge allows.
    let span_n = Block::new(0, LOBBY_DEPTH + ROOM_GAP, 0, mid - HUB - ROOM_GAP);
    let span_s = Block::new(
        0,
        mid + HUB + ROOM_GAP,
        0,
        last - LOBBY_DEPTH + 1 - ROOM_GAP,
    );
    let room_w = (mid - ARM - 2) / 2;
    let inner_w = Block::new(mid - ARM - room_w, 0, mid - ARM, 0);
    let outer_w = Block::new(mid - ARM - 2 * room_w + 1, 0, mid - ARM - room_w, 0);
    let inner_e = Block::new(mid + ARM, 0, mid + ARM + room_w, 0);
    let outer_e = Block::new(mid + ARM + room_w, 0, mid + ARM + 2 * room_w - 1, 0);
    let at = |cols: Block, rows: Block| Block::new(cols.x0, rows.y0, cols.x1, rows.y1);
    let mess = at(inner_w, span_n);
    let quarters = at(outer_w, span_n);
    let heads = at(inner_e, span_n);
    let lab = at(outer_e, span_n);
    let rec = at(inner_w, span_s);
    let research = at(outer_w, span_s);
    let storage = at(inner_e, span_s);
    let cargo = at(outer_e, span_s);
    let blocks = [
        hub, lobby_w, lobby_e, lobby_n, lobby_s, arm_w, arm_e, arm_n, arm_s, mess, quarters, heads,
        lab, rec, research, storage, cargo,
    ];
    let inside = |x: i32, y: i32| blocks.iter().any(|b| b.contains(x, y));
    let skin = |x: i32, y: i32| {
        inside(x, y)
            && (-1..=1).any(|dx| (-1..=1).any(|dy| (dx != 0 || dy != 0) && !inside(x + dx, y + dy)))
    };

    // Frame, deck and skin over the union.
    let mut skins = Vec::new();
    for y in 1..=last {
        for x in 1..=last {
            if !inside(x as i32, y as i32) {
                continue;
            }
            put(&mut design, PartKind::Structure, (x, y), Rotation::R0);
            if skin(x as i32, y as i32) {
                put(&mut design, PartKind::OutsideWall, (x, y), Rotation::R0);
                skins.push((x, y));
            } else {
                put(&mut design, PartKind::Floor, (x, y), Rotation::R0);
            }
        }
    }

    // The port: two tiles of the west lobby's skin, decked, with the
    // airlock on them — the first airlock, which is what a ship docks at
    // — and a docking bay's airlock at the end of each of the other arms.
    // The array in the north lobby's skin beside its airlock.
    let airlocks = [
        ((1, mid - 1), Rotation::R0),
        ((last, mid - 1), Rotation::R0),
        ((mid - 1, 1), Rotation::R90),
        ((mid - 1, last), Rotation::R90),
    ];
    let mut kept = vec![(mid + 3, 1)];
    for &((x, y), rotation) in &airlocks {
        let tiles = if rotation == Rotation::R0 {
            [(x, y), (x, y + 1)]
        } else {
            [(x, y), (x + 1, y)]
        };
        for tile in tiles {
            take(&mut design, tile);
            put(&mut design, PartKind::Floor, tile, Rotation::R0);
            kept.push(tile);
        }
        put(&mut design, PartKind::Airlock, (x, y), rotation);
    }
    take(&mut design, (mid + 3, 1));
    put(
        &mut design,
        PartKind::SensorArray,
        (mid + 3, 1),
        Rotation::R0,
    );

    // The port lobby is the reactor room too: the trading desk against its
    // north wall by the port, worked from the row below — the first thing
    // a crew coming aboard meets, clear of the spot the station's people
    // are sent home to — the reactor beyond it along the same wall with
    // two tiles of gangway between, and life support, the batteries and
    // the tank along the south wall; the corridor runs through the middle.
    let lobby = lobby_w.inner();
    put(
        &mut design,
        PartKind::TradingDesk,
        (3, lobby.y0),
        Rotation::R0,
    );
    put(&mut design, PartKind::Reactor, (7, lobby.y0), Rotation::R0);
    put(
        &mut design,
        PartKind::LifeSupport,
        (8, lobby.y1 - 1),
        Rotation::R0,
    );
    let batteries = rng.below(3);
    for i in 0..batteries {
        put(
            &mut design,
            PartKind::Battery,
            (6, lobby.y1 - i),
            Rotation::R0,
        );
    }

    // The partitions: a wall down every edge a room shares with its arm
    // or with the room beside it, with a two-tile doorway in each — the
    // inner rooms' onto the arm, the outer rooms' through the partition —
    // placed towards the hub for the west rooms and towards the lobby for
    // the east, so no two face each other across the corridor, and clear
    // of the barricades. The wall tiles are the shared hull column or row
    // itself, which the union made deck.
    let mut doors: Vec<((u32, u32), Rotation)> = Vec::new();
    let mut walls: Vec<(u32, u32)> = Vec::new();
    // A wall down column `x` for rows `y0..=y1`, less a doorway at `door`
    // (two tiles, `door` and `door + 1`), the door upright.
    let mut column = |x: u32, y0: u32, y1: u32, door: u32| {
        for y in y0..=y1 {
            if y == door || y == door + 1 {
                continue;
            }
            walls.push((x, y));
        }
        doors.push(((x, door), Rotation::R0));
    };
    let door_n_w = span_n.y1 - 4;
    let door_n_e = span_n.y0 + 3;
    let door_s_w = span_s.y0 + 3;
    let door_s_e = span_s.y1 - 4;
    column(mess.x1, mess.y0 + 1, mess.y1 - 1, door_n_w);
    column(quarters.x1, mess.y0 + 1, mess.y1 - 1, door_n_w);
    column(heads.x0, heads.y0 + 1, heads.y1 - 1, door_n_e);
    column(lab.x0, heads.y0 + 1, heads.y1 - 1, door_n_e);
    column(rec.x1, rec.y0 + 1, rec.y1 - 1, door_s_w);
    column(research.x1, rec.y0 + 1, rec.y1 - 1, door_s_w);
    column(storage.x0, storage.y0 + 1, storage.y1 - 1, door_s_e);
    column(cargo.x0, storage.y0 + 1, storage.y1 - 1, door_s_e);
    for &(x, y) in &walls {
        put(&mut design, PartKind::Wall, (x, y), Rotation::R0);
    }
    for &(origin, rotation) in &doors {
        put(&mut design, PartKind::Door, origin, rotation);
    }

    // Cover in the hallways: a line of sandbags across three of each
    // arm's five tiles, `BARRICADE_OUT` tiles out from the hub's skin,
    // from alternate walls so the gap past each is on the other side — a
    // body behind one peeks round its end and is dodged half the shots at
    // it (`bims::sight`). Clear of every doorway's two tiles of approach:
    // the west rooms' doors are on the far side of each arm from its
    // barricade, and the east rooms' further out.
    let out = HUB + BARRICADE_OUT;
    for i in 0..3u32 {
        put(
            &mut design,
            PartKind::Sandbags,
            (mid - out, mid + i),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Sandbags,
            (mid + out, mid - ARM + 1 + i),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Sandbags,
            (mid - ARM + 1 + i, mid - out),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Sandbags,
            (mid + i, mid + out),
            Rotation::R0,
        );
    }

    // The mess: the galley along the north wall from the corner, worked
    // from the row below, and tables with a chair a side under it, as
    // many as the room is deep for.
    let m = mess.inner();
    for (kind, x) in [
        (PartKind::ColdStore, m.x0 + 1),
        (PartKind::Worktop, m.x0 + 2),
        (PartKind::Hob, m.x0 + 4),
        (PartKind::Dishwasher, m.x0 + 5),
    ] {
        put(&mut design, kind, (x, m.y0), Rotation::R0);
    }
    let mut table_y = m.y0 + 3;
    while table_y + 1 <= m.y1 - 1 {
        put(
            &mut design,
            PartKind::Table,
            (m.x0 + 2, table_y),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Chair,
            (m.x0 + 2, table_y + 1),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Chair,
            (m.x0 + 3, table_y + 1),
            Rotation::R0,
        );
        table_y += 4;
    }

    // The crew's quarters: bunks down the west skin, a column every three
    // tiles for a station that wants more of them (the arena) — the bunk,
    // its use tile, and a tile of gangway before the next — never nearer
    // the partition than two tiles of gangway.
    let q = quarters.inner();
    for column in 0..bunk_columns {
        let Some(x) = q.x0.checked_add(3 * column) else {
            break;
        };
        if x + 3 > q.x1 {
            break;
        }
        let mut bunk_y = q.y0 + 1;
        while bunk_y + 1 <= q.y1 - 1 {
            put(&mut design, PartKind::Bunk, (x, bunk_y), Rotation::R180);
            bunk_y += 3;
        }
    }

    // The heads: along the north wall from the corner away from the arm,
    // worked from the row below.
    let h = heads.inner();
    for (kind, x) in [
        (PartKind::Toilet, h.x1 - 3),
        (PartKind::Basin, h.x1 - 2),
        (PartKind::Shower, h.x1 - 1),
    ] {
        put(&mut design, kind, (x, h.y0), Rotation::R0);
    }

    // The research room: the research desk against its north wall from
    // the corner, worked from the row below — the desk the station's key
    // sits on, and what the crew go ashore for — with its trays starting
    // a row lower than the laboratory's so the desk's spot has deck on
    // its far side (the room's navigation will not walk a spot between
    // two solids).
    let rr = research.inner();
    put(
        &mut design,
        PartKind::ResearchDesk,
        (rr.x0 + 1, rr.y0),
        Rotation::R0,
    );

    // The laboratory and the research room: runs of six trays, one every
    // three rows from two below the north wall so the row a run is worked
    // from and the row behind it are clear, as many as the seed likes and
    // the rooms hold — more on a bigger station, which feeds more. The
    // broom locker against the laboratory's north wall by the partition.
    let bays = 1 + rng.below(3) + (side - 40) / 6;
    let mut placed = 0;
    for (room, first_row) in [(lab.inner(), 2), (research.inner(), 3)] {
        let columns = ((room.x1 - room.x0 + 1) / 7).max(1);
        let mut i = 0;
        while placed < bays {
            let (column, row) = (i % columns, i / columns);
            let at = (room.x0 + 1 + column * 7, room.y0 + first_row + row * 3);
            if at.1 + 2 > room.y1 || at.0 + 5 > room.x1 {
                break;
            }
            put(&mut design, PartKind::HydroBay, at, Rotation::R0);
            placed += 1;
            i += 1;
        }
    }
    put(
        &mut design,
        PartKind::BroomLocker,
        (lab.inner().x0, lab.inner().y0),
        Rotation::R0,
    );

    // The rec room: a table with a chair a side, as many as the room is
    // deep for, down its west side.
    let r = rec.inner();
    let mut table_y = r.y0 + 1;
    while table_y + 1 <= r.y1 - 1 {
        put(
            &mut design,
            PartKind::Table,
            (r.x0 + 1, table_y),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Chair,
            (r.x0 + 1, table_y + 1),
            Rotation::R0,
        );
        put(
            &mut design,
            PartKind::Chair,
            (r.x0 + 2, table_y + 1),
            Rotation::R0,
        );
        table_y += 4;
    }

    // The storage and the cargo: shelves along the north walls, two tiles
    // apart, and a second row four tiles down in a room deep enough.
    let shelves = 2 + rng.below(4) + (side - 40) / 4;
    let mut placed = 0;
    for room in [storage.inner(), cargo.inner()] {
        let mut shelf_y = room.y0;
        while placed < shelves && shelf_y + 2 <= room.y1 {
            let mut shelf_x = room.x0 + 2;
            while placed < shelves && shelf_x + 1 <= room.x1 - 1 {
                put(
                    &mut design,
                    PartKind::Shelf,
                    (shelf_x, shelf_y),
                    Rotation::R0,
                );
                placed += 1;
                shelf_x += 2;
            }
            shelf_y += 4;
        }
    }

    // Light: a wall light in every room's inner corners and one every six
    // tiles along its long walls, on whatever tile is still free — last,
    // so a lamp never takes a fixture's tile. A tile no light reaches is
    // dark, and a dark deck is one the crew see ten tiles across
    // (`bims::sight`); a lamp at a corner where two blocks open into each
    // other has no wall at its back and is only a warning.
    for block in &blocks {
        let i = block.inner();
        let mut lamps: Vec<(u32, u32)> =
            vec![(i.x0, i.y0), (i.x1, i.y0), (i.x0, i.y1), (i.x1, i.y1)];
        let mut x = i.x0 + 6;
        while x < i.x1 {
            lamps.push((x, i.y0));
            lamps.push((x, i.y1));
            x += 6;
        }
        let mut y = i.y0 + 6;
        while y < i.y1 {
            lamps.push((i.x0, y));
            lamps.push((i.x1, y));
            y += 6;
        }
        for at in lamps {
            put(&mut design, PartKind::WallLight, at, Rotation::R0);
        }
    }

    // A derelict has lost some of its skin — not the port, not the other
    // airlocks and not the array, which is what a passing ship still picks
    // up. A roll that lands on one of those takes nothing, and is still a
    // roll, so the stream stays in step.
    if kind == StationKind::Derelict {
        let holes = 6 + rng.below(6);
        for _ in 0..holes {
            let tile = skins[rng.below(skins.len() as u32) as usize];
            if kept.contains(&tile) {
                continue;
            }
            take(&mut design, tile);
        }
    }

    design
}

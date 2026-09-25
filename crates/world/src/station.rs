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
//! `worldgen::rng`, in integers, and placed through [`Placer`] — which
//! answers exactly what `apply` would, part for part, without rebuilding
//! the grid for every one of a town's twenty thousand parts — so a
//! station is the same station on a native server and in a browser, and
//! the ship docks in the same place on both to the unit.
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

use economy::market::{Bias, Market, MarketKind};
use flight::angle;
use shipdesign::dock::{self, Port};
use shipdesign::parts::TILE;
use shipdesign::{Layer, PartKind, PlacedPart, Rotation, ShipDesign};
use worldgen::math::{DVec2, dvec2};
use worldgen::rng::Rng;
use worldgen::system::StationBlueprint;
use worldgen::{Node, StarSystem, StationKind, Stock};

use crate::data;

/// Layouts already built, by kind, plan and seed. A layout is a pure
/// function of the three, and building one is thousands of parts through
/// the [`Placer`] — cheap, but a world opens every station of its system
/// at once and the painter and the tests ask again. Looked up only, never
/// walked, so the order in it decides nothing.
static BUILT: std::sync::Mutex<Vec<((StationKind, Plan, u64), ShipDesign)>> =
    std::sync::Mutex::new(Vec::new());

/// How many tiles across a [`Plan::Hub`] station's build area is, by kind.
/// The hull fills it bar a one-tile margin. Big next to a ship — the
/// playtest ship is twenty — because a station is where ships go, not a
/// ship. The other plans are sized by [`Plan::side`].
pub fn side_of(kind: StationKind) -> u32 {
    match kind {
        StationKind::Orbital => 64,
        StationKind::Refinery => 56,
        StationKind::MiningOutpost => 52,
        StationKind::Derelict => 54,
        StationKind::Relay => 48,
    }
}

/// How many people live aboard at most, by kind: the hub's
/// [`bims::room::BERTHS`], a relay is a lonely posting, and nobody is left
/// on a derelict — which is why one never gets a room at all. The number a
/// station actually houses is its plan's, [`Plan::residents`], never more
/// than this; "does anybody live there" is this being nought.
pub fn residents_of(kind: StationKind) -> u32 {
    match kind {
        StationKind::Derelict => 0,
        StationKind::Relay => 1,
        _ => bims::room::BERTHS as u32,
    }
}

/// Which desk a station keeps, for `economy::market` to quote: the
/// station's kind, bar the settlement on a planet's surface — a
/// [`Plan::Surface`] is a market of its own kind whatever `StationKind`
/// it is laid out as ([`crate::surface::SURFACE_KIND`]) — and none at a
/// derelict, which stocks nothing and buys nothing. The one place the
/// generator's kinds and the market's are mapped; `ship::Session` asks
/// it for the design phase's desk too.
pub fn market_kind(kind: StationKind, plan: Plan) -> Option<MarketKind> {
    if plan == Plan::Surface {
        return Some(MarketKind::Settlement);
    }
    match kind {
        StationKind::Orbital => Some(MarketKind::Orbital),
        StationKind::Refinery => Some(MarketKind::Refinery),
        StationKind::MiningOutpost => Some(MarketKind::MiningOutpost),
        StationKind::Relay => Some(MarketKind::Relay),
        StationKind::Derelict => None,
    }
}

/// Where a ship goes to be docked at a station: its centre of mass and its
/// heading, with the two airlocks' outer faces touching.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Berth {
    pub position: DVec2,
    pub heading: f64,
}

/// One station of the system, as a place.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Station {
    pub id: u32,
    pub kind: StationKind,
    /// Which building it is — rolled off the seed, bar the spawn, which
    /// `World::start` makes a [`Plan::Hub`] whatever it rolled.
    pub plan: Plan,
    /// The layout: what the painter draws, what the room lays out, and what
    /// the port is found in.
    pub design: ShipDesign,
    /// How many people live here: the plan's number for a station
    /// ([`Plan::residents`]), and a town's own roll for a planet's surface
    /// (`crate::surface::Surface::population`, five to thirty). What
    /// [`Station::residents`] answers, and what the layout was sized to.
    pub population: u32,
    /// Where design tile (0, 0) sits in the system. The grid's centre is on
    /// the blueprint's position.
    pub anchor: DVec2,
    /// The seed the residents' room is opened with.
    pub map_seed: u64,
    /// What is on its shelves: what the desk sells.
    pub stock: Stock,
    /// Its desk's own lean on every price, as the generator rolled it —
    /// except the spawn's, which `World::start` sets to nothing, so an
    /// opening pool buys the same at a kind of station whatever the seed
    /// rolled. What [`Station::market`] quotes through.
    pub bias: Bias,
    /// Whether the people aboard were rolled enemies, as the generator
    /// rolled it — the blueprint's word, carried here for the spawn,
    /// which is never one. **Nobody's stance**: every human is friendly (features 102
    /// and 104), and whose a station is to the crew is `World::stance`
    /// — the machines' where they hold it, home at home, and a
    /// stranger's everywhere else.
    pub hostile: bool,
}

impl Station {
    /// Build the station the blueprint describes, standing at `at`, on the
    /// plan its seed rolls.
    pub fn build(blueprint: &StationBlueprint, at: DVec2) -> Station {
        Station::build_as(blueprint, at, Plan::rolled(blueprint.map_seed))
    }

    /// Build the station the blueprint describes, standing at `at`, on
    /// `plan` whatever its seed rolled.
    pub fn build_as(blueprint: &StationBlueprint, at: DVec2, plan: Plan) -> Station {
        let design = layout(blueprint.kind, plan, blueprint.map_seed);
        let half = design.build_area as f64 * TILE as f64 / 2.0;
        Station {
            id: blueprint.id,
            kind: blueprint.kind,
            plan,
            anchor: at.sub(angle::rotate_design(dvec2(half, half), 0.0)),
            design,
            population: plan.residents(blueprint.kind),
            map_seed: blueprint.map_seed,
            stock: blueprint.stock,
            bias: blueprint.bias,
            hostile: blueprint.hostile,
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

    /// Lay the station out again on `plan`, standing where it stood: the
    /// anchor is recomputed from the old centre, since the build area
    /// changes with the plan. `World::start` makes the spawn a hub with
    /// it.
    pub fn replan(&mut self, plan: Plan) {
        let centre = self.centre();
        self.plan = plan;
        self.design = layout(self.kind, plan, self.map_seed);
        self.population = plan.residents(self.kind);
        let half = self.design.build_area as f64 * TILE as f64 / 2.0;
        self.anchor = centre.sub(angle::rotate_design(dvec2(half, half), 0.0));
    }

    /// How many people live here: [`Station::population`] — the plan's
    /// number for a station, a town's roll for a surface.
    pub fn residents(&self) -> u32 {
        self.population
    }

    /// Its desk, to quote from: the kind's ([`market_kind`]) with this
    /// station's own lean. `None` at a derelict, which has nobody to
    /// keep one — `World::sell` refuses there, and `World::buy` never
    /// gets that far since it stocks nothing.
    pub fn market(&self) -> Option<Market> {
        market_kind(self.kind, self.plan).map(|kind| Market::new(kind, self.bias))
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
    /// it opens, as a unit vector: what a berth is set against.
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

/// Which building a station is: one of six floor plans, rolled off the
/// station's seed ([`Plan::rolled`]) so that two docks are two different
/// places, and the same dock the same place every time. The plan decides
/// the shape, how wide the corridors are, how big the hull is
/// ([`Plan::side`]) and how many people live there ([`Plan::residents`]);
/// the seed then dresses it — how many bays, shelves and batteries.
///
/// The [`Plan::Hub`] is the one the game was tuned on and the one the
/// spawn is built as whatever its seed rolled (`World::start`, the way
/// the spawn is home whatever its stance rolled): every fixture test walks
/// it, the arena is it laid out bigger, and a crew's first dock should be
/// the familiar one. The other five are what a crew meets *elsewhere*.
///
/// Every plan keeps the port in the west skin and the array in the north,
/// a straight run of deck at least nine tiles in from the port for the
/// crew to come ashore onto and a fight to be staged in
/// (`World::stage_droid_fight_for_probe`), a reactor room by the port with the
/// trading desk in it, and the same rooms with the same fixtures laid the
/// same way — the galley, the quarters, the heads, the research desk and
/// the bays, the shelves — so the room's people live in any of them the
/// way they live in the hub. What differs is how the rooms hang together.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Plan {
    /// A hub and four arms, a docking lobby at the end of each, rooms hung
    /// two deep off the north and south arms; corridors five wide with a
    /// barricade in each. Sized by kind, 48 to 64. Two live here.
    Hub,
    /// The smallest: a squat bar with the reactor room at the port end and
    /// one corridor two wide down its middle, two rooms to the north and
    /// three to the south of it. 34 and up. A lonely posting for one.
    Pod,
    /// Two fat bands crossing, a hall where they meet: the port band is
    /// the reactor room, and a corridor two wide runs up the middle of
    /// each of the other three between a room a side. 44 and up. Three.
    Cross,
    /// Long and thin: the reactor room at the port end and a corridor
    /// three wide the length of the hull, four rooms a side. 60 and up.
    /// Four.
    Spine,
    /// A square ring round a void, its corridor two wide along the inner
    /// edge and the rooms outside it: the reactor room in the west side
    /// with the port, a docking lobby opposite it in the east, and the
    /// crew's quarters in a corner big enough for ten bunks. 48 and up.
    /// Six.
    Ring,
    /// A comb: the reactor lobby, a spine corridor two wide, and three
    /// arms east off it — the middle one three wide with cover, each
    /// ending in a docking bay — with the rooms between the arms. 52 and
    /// up. Five.
    Comb,
    /// Not a station's at all but a planet's: the town the ship lands at,
    /// laid out on the ground — open, no skin — built like a fort: a wall
    /// round the deck with the pad in its west side and a gate in its
    /// north and its south, the watch house and the trading house beside
    /// the pad, a hall, houses along its streets, fields and the wild
    /// over the rest inside the wall
    /// (`crate::surface`). Never rolled: a body's surface is this whatever
    /// its seed says, and no station is. Sized [`data::SURFACE_SIDE`]
    /// whatever its kind; how many live there is the surface's own roll
    /// ([`Station::population`]), and [`Plan::residents`] answers the most
    /// a town holds, since only `layout` and the tests ask it.
    Surface,
}

/// The salt the plan is rolled with: a stream of its own off the seed, so
/// the layout's own rolls — the bays, the shelves — are what they were.
const PLAN_SALT: u64 = 0x_504C_414E_0000_0000;

impl Plan {
    /// The six a station can be. [`Plan::Surface`] is not among them: it
    /// is a planet's, never a station's, and never rolled.
    pub const ALL: [Plan; 6] = [
        Plan::Hub,
        Plan::Pod,
        Plan::Cross,
        Plan::Spine,
        Plan::Ring,
        Plan::Comb,
    ];

    /// The plan a station's seed rolls: one of the six, evenly.
    pub fn rolled(map_seed: u64) -> Plan {
        Plan::ALL[Rng::new(map_seed ^ PLAN_SALT).below(Plan::ALL.len() as u32) as usize]
    }

    /// How many tiles across the build area is. The hub is sized by kind
    /// ([`side_of`]); the others have a size each, an orbital's a little
    /// bigger than a relay's, since their rooms are laid at fixed offsets
    /// from the port and the far ones grow with the hull.
    pub fn side(self, kind: StationKind) -> u32 {
        let grown = match kind {
            StationKind::Orbital => 8,
            StationKind::Refinery => 4,
            StationKind::MiningOutpost | StationKind::Derelict => 2,
            StationKind::Relay => 0,
        };
        match self {
            Plan::Hub => side_of(kind),
            Plan::Pod => 34 + grown,
            Plan::Cross => 44 + grown,
            Plan::Spine => 60 + grown,
            Plan::Ring => 48 + grown,
            Plan::Comb => 52 + grown,
            Plan::Surface => data::SURFACE_SIDE,
        }
    }

    /// How many people live aboard: nobody on a derelict, one on a relay
    /// whatever the plan, else the plan's number — always short of the
    /// bunks by two or more, so there is a bed for a mercenary for hire.
    /// A surface's is the **most** a town holds
    /// ([`data::SURFACE_POPULATION`]): the town's own number is rolled
    /// and kept on [`Station::population`], and this is what `layout`
    /// builds the biggest town to.
    pub fn residents(self, kind: StationKind) -> u32 {
        let of_plan = match self {
            Plan::Hub => bims::room::BERTHS as u32,
            Plan::Pod => 1,
            Plan::Cross => 3,
            Plan::Spine => 4,
            Plan::Ring => 6,
            Plan::Comb => 5,
            Plan::Surface => return data::SURFACE_POPULATION.1,
        };
        residents_of(kind).min(of_plan)
    }

    /// How wide the corridors are, in tiles. Two is the narrowest the
    /// room's navigation walks (`crates/game`'s `BODY_MARGIN` on a tile),
    /// and too narrow for a barricade: a sandbag across one of its two
    /// tiles is a wall.
    pub fn corridor(self) -> u32 {
        match self {
            Plan::Hub => 5,
            Plan::Pod | Plan::Cross | Plan::Ring | Plan::Comb => 2,
            Plan::Spine => 3,
            // Open ground: the whole of it is corridor.
            Plan::Surface => data::SURFACE_SIDE,
        }
    }
}

/// The station's design, from its kind, its plan and its seed.
///
/// One furnisher for all of them ([`furnish`]), on the floor plan the plan
/// draws (`hub`, `pod`, `cross`, `spine`, `ring`, `comb`). The hull is
/// the union of the plan's blocks — every tile in any of them frame and
/// deck, one with any of its eight neighbours outside them all outside
/// wall — with the port in the west skin and the array in the north.
/// Every room has a doorway two tiles wide and every fixture stands with
/// two clear tiles in front of it, because the room's navigation cannot
/// walk a one-tile gap (see `crates/shipdesign`'s module note). A derelict
/// is the same hull with holes in it and nobody home.
///
/// The seed decides how many bays, shelves and batteries there are and
/// nothing else about the shape, so two seeds of one plan are two
/// stations without either being a different building.
pub fn layout(kind: StationKind, plan: Plan, map_seed: u64) -> ShipDesign {
    if let Ok(built) = BUILT.lock()
        && let Some((_, design)) = built.iter().find(|(key, _)| *key == (kind, plan, map_seed))
    {
        return design.clone();
    }
    let design = build_layout(kind, plan, plan.side(kind), 1, map_seed);
    if let Ok(mut built) = BUILT.lock() {
        built.push(((kind, plan, map_seed), design.clone()));
    }
    design
}

/// A planet's town, from its seed, its biome and how many live there:
/// `crate::surface::floor` furnished, and the wild laid round it. Not
/// cached here — `crate::surface::Surface::station` keeps the one it
/// built, and a town is built when somebody lands, not when the world
/// opens. [`layout`] of [`Plan::Surface`] is the biggest temperate town
/// on the same seed, for the map and the tests.
pub fn layout_surface(map_seed: u64, biome: crate::surface::Biome, population: u32) -> ShipDesign {
    let side = data::SURFACE_SIDE;
    let floor = crate::surface::floor(side, biome, population, map_seed);
    furnish(crate::surface::SURFACE_KIND, side, floor, map_seed)
}

/// The arena: the hub plan, the same kind and seed, laid out
/// [`data::ARENA_SIDE`] tiles across — bigger than any kind of station —
/// with the quarters' bunks in [`data::ARENA_BUNK_COLUMNS`] columns. What
/// the `droids` command rebuilds its dock as before the machines have it
/// (`World::arena_dock_for_probe`): longer corridors and bigger rooms to
/// fight through. Not cached, since one is built a run.
pub fn arena(kind: StationKind, map_seed: u64) -> ShipDesign {
    build_layout(
        kind,
        Plan::Hub,
        data::ARENA_SIDE,
        data::ARENA_BUNK_COLUMNS,
        map_seed,
    )
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
/// The other plans' reactor room: the port lobby's block, eleven wide and
/// as tall as the plan makes it, with the trading desk, the reactor, life
/// support and the batteries at the hub's offsets in it.
const REACTOR_ROOM: u32 = 10;
/// A ring station's four sides are this thick, skin to inner skin: nine
/// tiles of room, a wall, the corridor two wide, and the skin round the
/// void.
const RING_SIDE: u32 = 14;

/// A block of hull, tile ranges inclusive: the plan is a union of these.
/// A room is one too, its ring the walls and its [`Block::inner`] the deck.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Block {
    pub(crate) x0: u32,
    pub(crate) y0: u32,
    pub(crate) x1: u32,
    pub(crate) y1: u32,
}

impl Block {
    pub(crate) const fn new(x0: u32, y0: u32, x1: u32, y1: u32) -> Block {
        Block { x0, y0, x1, y1 }
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 as i32 && x <= self.x1 as i32 && y >= self.y0 as i32 && y <= self.y1 as i32
    }

    /// The deck inside the skin.
    pub(crate) fn inner(&self) -> Block {
        Block::new(self.x0 + 1, self.y0 + 1, self.x1 - 1, self.y1 - 1)
    }
}

/// A floor plan: where the hull is and what each room is for. What a
/// plan's function draws and [`furnish`] fills. The rooms are blocks with
/// their walls on (`inner` is the deck); the lobby is deck only.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Floor {
    /// The hull, as the union of these.
    pub(crate) hull: Vec<Block>,
    /// The airlocks, the port first — the first airlock is what a ship
    /// docks at (`shipdesign::dock::port`).
    pub(crate) airlocks: Vec<((u32, u32), Rotation)>,
    /// The sensor array's tile, in the north skin.
    pub(crate) array: (u32, u32),
    /// The reactor room's deck: the trading desk, the reactor, life
    /// support and the batteries stand at fixed offsets in it, so it is
    /// at least eight wide and eleven tall.
    pub(crate) lobby: Block,
    /// The partitions, and the doorways in them.
    pub(crate) walls: Vec<(u32, u32)>,
    pub(crate) doors: Vec<((u32, u32), Rotation)>,
    /// Where the sandbags stand.
    pub(crate) cover: Vec<(u32, u32)>,
    pub(crate) mess: Block,
    pub(crate) quarters: Block,
    pub(crate) heads: Block,
    pub(crate) research: Block,
    pub(crate) lab: Option<Block>,
    pub(crate) rec: Option<Block>,
    pub(crate) stores: Vec<Block>,
    pub(crate) bunk_columns: u32,
    /// Where the lamps hang: a ring of them in each, in this order.
    pub(crate) lit: Vec<Block>,
    /// Where the big plant stands.
    pub(crate) hall: (u32, u32),
    /// Whether the hull has no skin: a planet's surface is open ground,
    /// and its edge is where the world ends rather than a wall. Every
    /// tile of the hull is deck then, the airlocks' included.
    pub(crate) open: bool,
    /// Where standing lights are planted on the deck, after everything
    /// but the wild: a surface's ground is lit by these, since nothing
    /// there has a wall to hang a lamp from. A tile already taken is
    /// skipped.
    pub(crate) standing_lights: Vec<(u32, u32)>,
    /// Whatever else the plan wants placed, in this order, after the
    /// comforts and before the standing lights: a town's houses' bunks,
    /// its bathhouses' fittings, its greenhouses' bays and its fields
    /// (`crate::surface`). One that is refused — a tile taken by a lamp,
    /// say — is skipped, so a plan lays these clear of where the lamps
    /// hang.
    pub(crate) extra: Vec<(PartKind, (u32, u32), Rotation)>,
    /// How many columns of tables the mess holds, four tiles apart, each
    /// with a chair a side under it and as many rows as the room is deep
    /// for. One on every station; a town's hall seats its whole
    /// population.
    pub(crate) mess_columns: u32,
    /// The wild round a town, laid last of all — after the standing
    /// lights — by `crate::surface::wild` in this biome, out to the edge
    /// of the deck. `None` on a station, which has nothing outside its
    /// skin.
    pub(crate) wild: Option<crate::surface::Biome>,
    /// Ground the wild keeps off: a town's streets, the yard before its
    /// pad and its field lots — the ways a Bim walks, which nothing may
    /// grow across. Empty on a station.
    pub(crate) clear: Vec<Block>,
}

/// A wall round `room` — every tile of its ring that is deck; the skin
/// refuses one — less the two tiles of each doorway, the doors upright
/// in a column and flat in a row.
pub(crate) fn enclose(
    room: Block,
    doorways: &[((u32, u32), Rotation)],
    walls: &mut Vec<(u32, u32)>,
    doors: &mut Vec<((u32, u32), Rotation)>,
) {
    let is_door = |x: u32, y: u32| {
        doorways.iter().any(|&((dx, dy), rotation)| {
            (x, y) == (dx, dy)
                || (rotation == Rotation::R0 && (x, y) == (dx, dy + 1))
                || (rotation == Rotation::R90 && (x, y) == (dx + 1, dy))
        })
    };
    for y in room.y0..=room.y1 {
        for x in room.x0..=room.x1 {
            let on_ring = x == room.x0 || x == room.x1 || y == room.y0 || y == room.y1;
            if on_ring && !is_door(x, y) {
                walls.push((x, y));
            }
        }
    }
    doors.extend_from_slice(doorways);
}

/// The hub: four arms to a docking lobby each — the port in the west one,
/// the array on the north — with the rooms hung off the arms the way the
/// picture had them. Two rooms deep either side of the north arm: the
/// mess (the galley) inside and the crew's quarters beyond it to the
/// west, the heads inside and the laboratory (the bay) beyond to the
/// east; two deep either side of the south arm: the rec room and the
/// research room (more bays) to the west, the storage and the cargo (the
/// shelves) to the east; and the port's lobby, bigger than the others, is
/// the reactor room, with the trading desk by the door. Every room has a
/// two-tile doorway — the inner rooms onto their arm, the outer rooms
/// through the partition into the inner — and a barricade of sandbags
/// stands across three of each arm's five tiles a few tiles out from the
/// hub, the gap past it on alternate sides. A tile of void between a
/// room and the block beside it keeps the skin sealed, since the hull is
/// the union of the blocks. Forty-eight is the smallest the rooms fit at.
fn hub(side: u32, bunk_columns: u32) -> Floor {
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
    let blocks = vec![
        hub, lobby_w, lobby_e, lobby_n, lobby_s, arm_w, arm_e, arm_n, arm_s, mess, quarters, heads,
        lab, rec, research, storage, cargo,
    ];

    // The port: two tiles of the west lobby's skin, and a docking bay's
    // airlock at the end of each of the other arms. The array in the
    // north lobby's skin beside its airlock.
    let airlocks = vec![
        ((1, mid - 1), Rotation::R0),
        ((last, mid - 1), Rotation::R0),
        ((mid - 1, 1), Rotation::R90),
        ((mid - 1, last), Rotation::R90),
    ];

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

    // Cover in the hallways: a line of sandbags across three of each
    // arm's five tiles, `BARRICADE_OUT` tiles out from the hub's skin,
    // from alternate walls so the gap past each is on the other side — a
    // body behind one peeks round its end and is dodged half the shots at
    // it (`bims::sight`). Clear of every doorway's two tiles of approach:
    // the west rooms' doors are on the far side of each arm from its
    // barricade, and the east rooms' further out.
    let out = HUB + BARRICADE_OUT;
    let mut cover = Vec::new();
    for i in 0..3u32 {
        cover.push((mid - out, mid + i));
        cover.push((mid + out, mid - ARM + 1 + i));
        cover.push((mid - ARM + 1 + i, mid - out));
        cover.push((mid + i, mid + out));
    }

    Floor {
        lit: blocks.clone(),
        hull: blocks,
        airlocks,
        array: (mid + 3, 1),
        lobby: lobby_w.inner(),
        walls,
        doors,
        cover,
        mess,
        quarters,
        heads,
        research,
        lab: Some(lab),
        rec: Some(rec),
        stores: vec![storage, cargo],
        bunk_columns,
        hall: (mid, mid),
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
    }
}

/// The pod: a bar twenty tall across the width of the build area, the
/// reactor room at its west end with the port in its skin, and one
/// corridor two wide from the reactor room's door to a second airlock in
/// the east skin. North of the corridor the mess and the crew's quarters,
/// south of it the heads, the research room (the desk and the bays; the
/// broom locker is here too) and the storage. No cover: a sandbag in a
/// corridor two wide is a wall.
fn pod(side: u32) -> Floor {
    let mid = side / 2;
    let last = side - 2;
    let hull = vec![Block::new(1, mid - 10, last, mid + 9)];
    let lobby = Block::new(1, mid - 10, REACTOR_ROOM, mid + 9);
    let corridor = Block::new(REACTOR_ROOM, mid - 2, last, mid + 1);
    // North of the corridor: the mess, then the quarters to the east skin.
    let mess = Block::new(REACTOR_ROOM, mid - 10, 19, mid - 2);
    let quarters = Block::new(19, mid - 10, last, mid - 2);
    // South of it: the heads, the research room, the storage.
    let heads = Block::new(REACTOR_ROOM, mid + 1, 17, mid + 9);
    let research = Block::new(17, mid + 1, 25, mid + 9);
    let storage = Block::new(25, mid + 1, last, mid + 9);

    let mut walls = Vec::new();
    let mut doors = Vec::new();
    enclose(
        lobby,
        &[((REACTOR_ROOM, mid - 1), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    // Each room's doorway is in its corridor wall, clear of what stands
    // inside: the galley's tables, the bunks' column, the shelves' row.
    enclose(
        mess,
        &[((12, mid - 2), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        quarters,
        &[((25, mid - 2), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        heads,
        &[((11, mid + 1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        research,
        &[((22, mid + 1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        storage,
        &[((26, mid + 1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );

    Floor {
        hull,
        airlocks: vec![
            ((1, mid - 1), Rotation::R0),
            ((last, mid - 1), Rotation::R0),
        ],
        array: (mid + 3, mid - 10),
        lobby: lobby.inner(),
        walls,
        doors,
        cover: Vec::new(),
        mess,
        quarters,
        heads,
        research,
        lab: None,
        rec: None,
        stores: vec![storage],
        bunk_columns: 2,
        lit: vec![lobby, corridor, mess, quarters, heads, research, storage],
        hall: (lobby.x0 + 4, mid - 3),
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
    }
}

/// The cross: a band nineteen tall across the build area and one as wide
/// down it, a hall seventeen square where they meet. The west arm is the
/// reactor room, opening into the hall by the port's rows; the other
/// three have a corridor two wide up the middle, from the hall to an
/// airlock in the end skin, and a room either side of it: the mess and
/// the quarters up the north arm, the heads and the storage along the
/// east, the rec room and the research room (the desk, the bays, the
/// broom locker) down the south. Cover is four sandbags in the hall's
/// corners, since none fits in the arms.
fn cross(side: u32) -> Floor {
    let mid = side / 2;
    let last = side - 2;
    let hull = vec![
        Block::new(1, mid - 9, last, mid + 9),
        Block::new(mid - 9, 1, mid + 9, last),
    ];
    let lobby = Block::new(1, mid - 9, mid - 9, mid + 9);
    let hall = Block::new(mid - 9, mid - 9, mid + 9, mid + 9);
    let arm_n = Block::new(mid - 2, 1, mid + 1, mid - 9);
    let arm_e = Block::new(mid + 9, mid - 2, last, mid + 1);
    let arm_s = Block::new(mid - 2, mid + 9, mid + 1, last);
    let mess = Block::new(mid - 9, 1, mid - 2, mid - 9);
    let quarters = Block::new(mid + 1, 1, mid + 9, mid - 9);
    let heads = Block::new(mid + 9, mid - 9, last, mid - 2);
    let storage = Block::new(mid + 9, mid + 1, last, mid + 9);
    let rec = Block::new(mid - 9, mid + 9, mid - 2, last);
    let research = Block::new(mid + 1, mid + 9, mid + 9, last);

    let mut walls = Vec::new();
    let mut doors = Vec::new();
    enclose(
        lobby,
        &[((mid - 9, mid - 1), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    // The mess and the rec room open onto their arm; the quarters onto
    // the hall, since a door in their arm wall would open onto the bunks;
    // the research room onto its arm by the desk; the heads and the
    // storage onto the east arm, the storage's door before its shelves.
    enclose(
        mess,
        &[((mid - 2, 5), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        quarters,
        &[((mid + 3, mid - 9), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        heads,
        &[((mid + 11, mid - 2), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        storage,
        &[((mid + 10, mid + 1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        rec,
        &[((mid - 2, mid + 12), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        research,
        &[((mid + 1, mid + 11), Rotation::R0)],
        &mut walls,
        &mut doors,
    );

    Floor {
        hull,
        airlocks: vec![
            ((1, mid - 1), Rotation::R0),
            ((last, mid - 1), Rotation::R0),
            ((mid - 1, 1), Rotation::R90),
            ((mid - 1, last), Rotation::R90),
        ],
        array: (mid + 3, 1),
        lobby: lobby.inner(),
        walls,
        doors,
        cover: vec![
            (mid - 4, mid - 4),
            (mid + 4, mid - 4),
            (mid - 4, mid + 4),
            (mid + 4, mid + 4),
        ],
        mess,
        quarters,
        heads,
        research,
        lab: None,
        rec: Some(rec),
        stores: vec![storage],
        bunk_columns: 2,
        lit: vec![
            lobby, hall, arm_n, arm_e, arm_s, mess, quarters, heads, storage, rec, research,
        ],
        hall: (mid, mid),
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
    }
}

/// The spine: a bar twenty-one tall the width of the build area, the
/// reactor room at the port end and a corridor three wide from its door
/// to an airlock in the east skin, with four rooms a side: the mess, the
/// quarters (three columns of bunks), the heads and the laboratory to
/// the north; the rec room, the research room, the storage and the cargo
/// to the south. Two sandbags stand in the corridor from alternate walls,
/// each leaving two tiles past it.
fn spine(side: u32) -> Floor {
    let mid = side / 2;
    let last = side - 2;
    let hull = vec![Block::new(1, mid - 10, last, mid + 10)];
    let lobby = Block::new(1, mid - 10, REACTOR_ROOM, mid + 10);
    let corridor = Block::new(REACTOR_ROOM, mid - 2, last, mid + 2);
    // The rooms share the span from the lobby's wall to the east skin in
    // ninths, wall to wall.
    let w = last - REACTOR_ROOM;
    let at = |ninths: u32| REACTOR_ROOM + w * ninths / 9;
    let (b1, b2, b3) = (at(2), at(5), at(6));
    let (c1, c2, c3) = (at(2), at(5), at(7));
    let (n0, n1) = (mid - 10, mid - 2);
    let (s0, s1) = (mid + 2, mid + 10);
    let mess = Block::new(REACTOR_ROOM, n0, b1, n1);
    let quarters = Block::new(b1, n0, b2, n1);
    let heads = Block::new(b2, n0, b3, n1);
    let lab = Block::new(b3, n0, last, n1);
    let rec = Block::new(REACTOR_ROOM, s0, c1, s1);
    let research = Block::new(c1, s0, c2, s1);
    let storage = Block::new(c2, s0, c3, s1);
    let cargo = Block::new(c3, s0, last, s1);

    let mut walls = Vec::new();
    let mut doors = Vec::new();
    enclose(
        lobby,
        &[((REACTOR_ROOM, mid - 1), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    // Every door is in the room's corridor wall, none facing another
    // across it, each clear of what stands inside.
    enclose(
        mess,
        &[((mess.x0 + 2, n1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        quarters,
        &[((b1 + 2, n1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        heads,
        &[((b2 + 1, n1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        lab,
        &[((b3 + 1, n1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        rec,
        &[((rec.x0 + 6, s0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        research,
        &[((c1 + 5, s0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        storage,
        &[((c2 + 8, s0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        cargo,
        &[((last - 3, s0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );

    Floor {
        hull,
        airlocks: vec![
            ((1, mid - 1), Rotation::R0),
            ((last, mid - 1), Rotation::R0),
        ],
        array: (mid + 3, mid - 10),
        lobby: lobby.inner(),
        walls,
        doors,
        cover: vec![((b1 + b2) / 2, mid - 1), ((c3 + last) / 2, mid + 1)],
        mess,
        quarters,
        heads,
        research,
        lab: Some(lab),
        rec: Some(rec),
        stores: vec![storage, cargo],
        bunk_columns: 3,
        lit: vec![
            lobby, corridor, mess, quarters, heads, lab, rec, research, storage, cargo,
        ],
        hall: (lobby.x0 + 4, mid - 3),
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
    }
}

/// The ring: four sides fourteen thick round a void, the corridor two
/// wide along each side's inner edge, turning in a two-by-two at each
/// corner, and the rooms outside it against the skin. The west side is
/// the quarters (two columns of bunks, five each), the reactor room with
/// the port, and the rec room; the north side the mess and the heads; the
/// east side the laboratory, a docking lobby with the second airlock, and
/// a store; the south side the research room and the cargo. No cover.
fn ring(side: u32) -> Floor {
    let mid = side / 2;
    let last = side - 2;
    let t = RING_SIDE;
    let hull = vec![
        Block::new(1, 1, last, t),
        Block::new(1, last - t + 1, last, last),
        Block::new(1, 1, t, last),
        Block::new(last - t + 1, 1, last, last),
    ];
    // The walls the rooms stand behind: `near` on the north and west
    // sides, `far` on the south and east; the corridor runs between each
    // and its side's inner skin.
    let near = t - 3;
    let far = last - t + 4;
    let split = last / 2;
    let quarters = Block::new(1, 1, near, mid - 6);
    let lobby = Block::new(1, mid - 6, near, mid + 6);
    let rec = Block::new(1, mid + 6, near, last);
    let mess = Block::new(near, 1, split, near);
    let heads = Block::new(split, 1, far, near);
    let lab = Block::new(far, 1, last, mid - 6);
    let bay = Block::new(far, mid - 6, last, mid + 6);
    let store = Block::new(far, mid + 6, last, last);
    let research = Block::new(near, far, split, last);
    let cargo = Block::new(split, far, far, last);
    let run_n = Block::new(near, near, far, t);
    let run_s = Block::new(near, last - t + 1, far, far);
    let run_w = Block::new(near, near, t, far);
    let run_e = Block::new(last - t + 1, near, far, far);

    let mut walls = Vec::new();
    let mut doors = Vec::new();
    enclose(
        quarters,
        &[((near, near + 3), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        lobby,
        &[((near, mid - 1), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        rec,
        &[((near, mid + 8), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        mess,
        &[((near + 6, near), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        heads,
        &[((split + 2, near), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        lab,
        &[((far, near + 3), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        bay,
        &[((far, mid - 2), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        store,
        &[((far, mid + 8), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        research,
        &[((near + 8, far), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        cargo,
        &[((split + 1, far), Rotation::R90)],
        &mut walls,
        &mut doors,
    );

    Floor {
        hull,
        airlocks: vec![
            ((1, mid - 1), Rotation::R0),
            ((last, mid - 1), Rotation::R0),
        ],
        array: (mid + 3, 1),
        lobby: lobby.inner(),
        walls,
        doors,
        cover: Vec::new(),
        mess,
        quarters,
        heads,
        research,
        lab: Some(lab),
        rec: Some(rec),
        stores: vec![cargo, store],
        bunk_columns: 2,
        lit: vec![
            run_n, run_s, run_w, run_e, quarters, lobby, rec, mess, heads, lab, bay, store,
            research, cargo,
        ],
        hall: (bay.x0 + 5, mid),
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
    }
}

/// The comb: the hub's port lobby, a spine corridor two wide behind it,
/// and three arms east off the spine to a docking bay each — the outer
/// two corridors two wide, the middle one three with a sandbag from
/// either wall — with the rooms in the bands between them: the mess and
/// the quarters (three columns of bunks) above the top arm, the heads
/// and the laboratory between it and the middle, the rec room and the
/// research room between the middle and the bottom, the storage and the
/// cargo below that. The arms run eight tiles past the rooms, which is
/// what makes it a comb.
fn comb(side: u32) -> Floor {
    let mid = side / 2;
    let last = side - 2;
    // The arms' walls: the top arm's rows 13 and 14, the middle arm's
    // the port's three, the bottom arm's thirteen and twelve from the
    // south skin.
    let (a0, a1) = (12, 15);
    let (b0, b1) = (mid - 2, mid + 2);
    let (c0, c1) = (last - 14, last - 11);
    let teeth = last - 8;
    let lobby = Block::new(1, mid - 6, REACTOR_ROOM, mid + 6);
    let spine = Block::new(REACTOR_ROOM, a0, REACTOR_ROOM + 3, c1);
    let body = Block::new(REACTOR_ROOM + 3, 1, teeth, last);
    let arm_a = Block::new(REACTOR_ROOM + 3, a0, last, a1);
    let arm_b = Block::new(REACTOR_ROOM + 3, b0, last, b1);
    let arm_c = Block::new(REACTOR_ROOM + 3, c0, last, c1);
    let hull = vec![lobby, spine, body, arm_a, arm_b, arm_c];
    let x0 = REACTOR_ROOM + 3;
    let mess = Block::new(x0, 1, 25, a0);
    let quarters = Block::new(25, 1, teeth, a0);
    let heads = Block::new(x0, a1, 22, b0);
    let lab = Block::new(22, a1, teeth, b0);
    let rec = Block::new(x0, b1, 23, c0);
    let research = Block::new(23, b1, teeth, c0);
    let storage = Block::new(x0, c1, 28, last);
    let cargo = Block::new(28, c1, teeth, last);

    let mut walls = Vec::new();
    let mut doors = Vec::new();
    enclose(
        lobby,
        &[((REACTOR_ROOM, mid - 1), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    // The rooms above the top arm and between it and the middle open
    // onto the top arm, the rest onto the arm below them; the cargo's
    // door is in its far corner, past the last shelf.
    enclose(mess, &[((19, a0), Rotation::R90)], &mut walls, &mut doors);
    enclose(
        quarters,
        &[((27, a0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(heads, &[((15, a1), Rotation::R90)], &mut walls, &mut doors);
    enclose(lab, &[((30, a1), Rotation::R90)], &mut walls, &mut doors);
    enclose(rec, &[((19, b1), Rotation::R90)], &mut walls, &mut doors);
    enclose(
        research,
        &[((31, b1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        storage,
        &[((14, c1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        cargo,
        &[((teeth - 2, c1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );

    Floor {
        hull,
        airlocks: vec![
            ((1, mid - 1), Rotation::R0),
            ((last, a0 + 1), Rotation::R0),
            ((last, mid - 1), Rotation::R0),
            ((last, c0 + 1), Rotation::R0),
        ],
        array: (mid + 3, 1),
        lobby: lobby.inner(),
        walls,
        doors,
        cover: vec![(25, mid - 1), (last - 13, mid + 1)],
        mess,
        quarters,
        heads,
        research,
        lab: Some(lab),
        rec: Some(rec),
        stores: vec![storage, cargo],
        bunk_columns: 3,
        lit: vec![
            lobby, spine, arm_a, arm_b, arm_c, mess, quarters, heads, lab, rec, research, storage,
            cargo,
        ],
        hall: (lobby.x0 + 4, mid - 3),
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
    }
}

/// The plan's floor, furnished: [`furnish`] on what the plan draws.
fn build_layout(
    kind: StationKind,
    plan: Plan,
    side: u32,
    bunk_columns: u32,
    map_seed: u64,
) -> ShipDesign {
    build_placer(kind, plan, side, bunk_columns, map_seed).design
}

/// [`build_layout`], handing back the placer it furnished through — for
/// the test that replays a furnishing through `apply`.
pub(crate) fn build_placer(
    kind: StationKind,
    plan: Plan,
    side: u32,
    bunk_columns: u32,
    map_seed: u64,
) -> Placer {
    let floor = match plan {
        Plan::Hub => hub(side, bunk_columns),
        Plan::Pod => pod(side),
        Plan::Cross => cross(side),
        Plan::Spine => spine(side),
        Plan::Ring => ring(side),
        Plan::Comb => comb(side),
        // The biggest temperate town: what `BIMS_NAV_MAP=Surface` prints
        // and the tests walk. A planet's own is `layout_surface`.
        Plan::Surface => crate::surface::floor(
            side,
            crate::surface::Biome::Temperate,
            data::SURFACE_POPULATION.1,
            map_seed,
        ),
    };
    furnish_placer(kind, side, floor, map_seed)
}

/// A design being furnished, with the occupancy of its four layers kept
/// beside it: the grid [`shipdesign::apply`] builds afresh from every part
/// for every edit, kept up to date a part at a time instead. A town is
/// twenty thousand parts, and `apply` walking every one of them for every
/// one of them was the better part of a minute; this is linear in the
/// parts. It answers **exactly** what a run of `apply`s would have — the
/// same refusals, tile for tile, the same ids in the same order — so every
/// station's `design_hash` is what it was before the placer went in;
/// `furnish_through_the_placer_is_furnish_through_apply` replays a
/// furnishing through `apply` and asks for equality, part for part. What
/// it does not do is keep a second source of truth *out* of here: the
/// occupancy is private, the design is what comes out, and nothing reads
/// the layers after `furnish` returns.
pub(crate) struct Placer {
    pub(crate) design: ShipDesign,
    side: u32,
    /// The part in each tile of each layer, in [`Layer::ALL`] order, `0`
    /// for empty — `shipdesign::Grid` again, kept rather than rebuilt.
    layers: [Vec<u32>; Layer::ALL.len()],
    /// Every put and take asked of it, in order, for the test that
    /// replays them through `apply`.
    #[cfg(test)]
    pub(crate) attempts: Vec<Attempt>,
}

/// One thing asked of a [`Placer`], for the replay.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum Attempt {
    Put(PartKind, (u32, u32), Rotation),
    Take((u32, u32)),
}

impl Placer {
    /// An empty design `side` tiles square, nothing on any layer.
    pub(crate) fn new(side: u32) -> Placer {
        let cells = (side as usize) * (side as usize);
        Placer {
            design: ShipDesign::new(side),
            side,
            layers: [
                vec![0; cells],
                vec![0; cells],
                vec![0; cells],
                vec![0; cells],
            ],
            #[cfg(test)]
            attempts: Vec::new(),
        }
    }

    fn index(&self, (x, y): (u32, u32)) -> usize {
        (y as usize) * (self.side as usize) + (x as usize)
    }

    /// Whether a tile is on the design at all.
    pub(crate) fn inside(&self, (x, y): (i32, i32)) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.side && (y as u32) < self.side
    }

    /// The part in `tile` on `layer`, or `0` — off the design too, as
    /// `Grid::get` answers.
    pub(crate) fn get(&self, layer: Layer, tile: (i32, i32)) -> u32 {
        if !self.inside(tile) {
            return 0;
        }
        let i = self.index((tile.0 as u32, tile.1 as u32));
        self.layers[layer as usize][i]
    }

    /// The kind of the part on the object layer of `tile`, if any.
    pub(crate) fn object_at(&self, tile: (i32, i32)) -> Option<PartKind> {
        let id = self.get(Layer::Object, tile);
        (id != 0)
            .then(|| self.design.part(id).map(|p| p.kind))
            .flatten()
    }

    /// Whether a body cannot stand on `tile`: off the design, no deck
    /// under it, or something on the object layer that blocks movement.
    /// `shipdesign::validate::walkable` asked of the occupancy; the
    /// wild's flood is walked over the complement.
    pub(crate) fn blocked(&self, tile: (i32, i32)) -> bool {
        if self.get(Layer::Floor, tile) == 0 {
            return true;
        }
        let object = self.get(Layer::Object, tile);
        object != 0
            && self
                .design
                .part(object)
                .is_some_and(|p| p.kind.def().blocks_movement)
    }

    /// Place a part, or refuse it the way `shipdesign::design::place`
    /// would: off the build area, its own layer taken on any tile of the
    /// footprint, what it requires missing under any of them, or — for
    /// something hung — no wall at its back. The budget every furnishing
    /// runs on is `Money::MAX`, which affords anything, so that refusal
    /// is not asked. Accepted, it gets `next_id` and goes on the end, as
    /// `apply` appends it.
    pub(crate) fn put(&mut self, kind: PartKind, origin: (u32, u32), rotation: Rotation) -> bool {
        #[cfg(test)]
        self.attempts.push(Attempt::Put(kind, origin, rotation));
        let def = kind.def();
        let (w, h) = shipdesign::parts::footprint(kind, rotation);
        // In bounds, in u64 like `place`, so a wild origin cannot wrap.
        let far_x = origin.0 as u64 + w as u64;
        let far_y = origin.1 as u64 + h as u64;
        if far_x > self.side as u64 || far_y > self.side as u64 {
            return false;
        }
        let tiles: Vec<(u32, u32)> = shipdesign::parts::covered(kind, rotation)
            .into_iter()
            .map(|(dx, dy)| (origin.0 + dx, origin.1 + dy))
            .collect();
        for &(x, y) in &tiles {
            let tile = (x as i32, y as i32);
            if self.get(def.layer, tile) != 0 {
                return false;
            }
            if let Some(under) = def.requires
                && self.get(under, tile) == 0
            {
                return false;
            }
        }
        if shipdesign::hangs_on_wall(kind) && !self.wall_at_back(origin, rotation) {
            return false;
        }
        let id = self.design.next_id;
        self.design.parts.push(PlacedPart {
            id,
            kind,
            origin,
            rotation,
        });
        self.design.next_id += 1;
        for tile in tiles {
            let i = self.index(tile);
            self.layers[def.layer as usize][i] = id;
        }
        true
    }

    /// Take the part on the object layer of `tile` off, if there is one.
    /// `shipdesign::design::remove`'s two refusals cannot arise for an
    /// object: nothing requires the object layer under it, and no hold
    /// in a furnishing has anything in it. Removal keeps the id order,
    /// as `apply`'s does.
    pub(crate) fn take(&mut self, tile: (u32, u32)) -> bool {
        #[cfg(test)]
        self.attempts.push(Attempt::Take(tile));
        let id = self.get(Layer::Object, (tile.0 as i32, tile.1 as i32));
        if id == 0 {
            return false;
        }
        let Some(part) = self.design.part(id).copied() else {
            return false;
        };
        for t in part.tiles() {
            let i = self.index(t);
            self.layers[Layer::Object as usize][i] = 0;
        }
        self.design.parts.retain(|p| p.id != id);
        true
    }

    /// Whether something hung at `origin` turned `rotation` has its wall:
    /// [`shipdesign::wall_at_back`] asked of the occupancy rather than of
    /// a grid rebuilt for it — the tile behind holds a part that blocks
    /// movement and is not a door.
    pub(crate) fn wall_at_back(&self, origin: (u32, u32), rotation: Rotation) -> bool {
        let (dx, dy) = shipdesign::wall_light_back(rotation);
        let at = (origin.0 as i32 + dx, origin.1 as i32 + dy);
        let id = self.get(Layer::Object, at);
        id != 0
            && self
                .design
                .part(id)
                .is_some_and(|p| p.kind.def().blocks_movement && p.kind != PartKind::Door)
    }

    /// The rotation a lamp or a picture at `tile` hangs at, if any side
    /// has a wall: [`shipdesign::wall_light_rotation`]'s rule — the first
    /// of `Rotation::ALL` whose back is a wall — on the occupancy.
    pub(crate) fn hung(&self, tile: (u32, u32)) -> Option<Rotation> {
        Rotation::ALL
            .into_iter()
            .find(|&r| self.wall_at_back(tile, r))
    }

    /// The same furnishing again, every put and take through `apply` on a
    /// fresh design, refusals skipped as `furnish` skips them. What the
    /// placer's design is pinned against.
    #[cfg(test)]
    pub(crate) fn replay(&self) -> ShipDesign {
        use shipdesign::{Budget, Edit, Money, apply};
        let budget = Budget::new(Money::MAX);
        let mut design = ShipDesign::new(self.side);
        for &attempt in &self.attempts {
            match attempt {
                Attempt::Put(kind, origin, rotation) => {
                    if let Ok(next) = apply(
                        &design,
                        &budget,
                        Edit::Place {
                            kind,
                            origin,
                            rotation,
                        },
                    ) {
                        design = next;
                    }
                }
                Attempt::Take(tile) => {
                    let standing = design
                        .grid()
                        .get(Layer::Object, (tile.0 as i32, tile.1 as i32));
                    if standing != 0
                        && let Ok(next) =
                            apply(&design, &budget, Edit::Remove { part_id: standing })
                    {
                        design = next;
                    }
                }
            }
        }
        design
    }
}

/// The hull over the floor's blocks and every fixture in its rooms, in
/// one order for every plan. The galley along the mess's north wall with
/// tables under it; bunks down the quarters' west wall, a column every
/// three tiles; the heads along their north wall; the research desk
/// against the research room's; runs of six trays in the laboratory and
/// the research room, as many as the seed likes and the rooms hold;
/// tables down the rec room; shelves along the stores' north walls; a
/// wall light in every lit block's corners and along its walls; the
/// comforts; whatever else the plan asks for; the standing lights; and
/// the wild, on a planet. Every fixture stands so that its use spot has
/// deck beyond it, because the room's navigation will not walk a spot
/// between two solids.
fn furnish(kind: StationKind, side: u32, floor: Floor, map_seed: u64) -> ShipDesign {
    furnish_placer(kind, side, floor, map_seed).design
}

/// [`furnish`], handing back the placer it furnished through — for the
/// test that replays it.
pub(crate) fn furnish_placer(kind: StationKind, side: u32, floor: Floor, map_seed: u64) -> Placer {
    let mut placer = Placer::new(side);
    let mut rng = Rng::new(map_seed);

    let last = side - 2;
    let inside = |x: i32, y: i32| floor.hull.iter().any(|b| b.contains(x, y));
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
            placer.put(PartKind::Structure, (x, y), Rotation::R0);
            if !floor.open && skin(x as i32, y as i32) {
                placer.put(PartKind::OutsideWall, (x, y), Rotation::R0);
                skins.push((x, y));
            } else {
                placer.put(PartKind::Floor, (x, y), Rotation::R0);
            }
        }
    }

    // The airlocks: two tiles of skin each, decked, with the airlock on
    // them — the first is the port, which is what a ship docks at. The
    // array in the north skin.
    let mut kept = vec![floor.array];
    for &((x, y), rotation) in &floor.airlocks {
        let tiles = if rotation == Rotation::R0 {
            [(x, y), (x, y + 1)]
        } else {
            [(x, y), (x + 1, y)]
        };
        for tile in tiles {
            placer.take(tile);
            placer.put(PartKind::Floor, tile, Rotation::R0);
            kept.push(tile);
        }
        placer.put(PartKind::Airlock, (x, y), rotation);
    }
    placer.take(floor.array);
    placer.put(PartKind::SensorArray, floor.array, Rotation::R0);

    // The reactor room: the trading desk against its north wall by the
    // port, worked from the row below — the first thing a crew coming
    // aboard meets, clear of the spot the station's people are sent home
    // to — the reactor beyond it along the same wall with two tiles of
    // gangway between, and life support, the batteries and the tank along
    // the south wall; the corridor runs through the middle.
    let lobby = floor.lobby;
    placer.put(
        PartKind::TradingDesk,
        (lobby.x0 + 1, lobby.y0),
        Rotation::R0,
    );
    placer.put(PartKind::Reactor, (lobby.x0 + 5, lobby.y0), Rotation::R0);
    placer.put(
        PartKind::LifeSupport,
        (lobby.x0 + 6, lobby.y1 - 1),
        Rotation::R0,
    );
    let batteries = rng.below(3);
    for i in 0..batteries {
        placer.put(
            PartKind::Battery,
            (lobby.x0 + 4, lobby.y1 - i),
            Rotation::R0,
        );
    }

    // The partitions and their doors.
    for &(x, y) in &floor.walls {
        placer.put(PartKind::Wall, (x, y), Rotation::R0);
    }
    for &(origin, rotation) in &floor.doors {
        placer.put(PartKind::Door, origin, rotation);
    }

    // Cover: low, walked and seen over, ducked behind (`bims::sight`).
    for &at in &floor.cover {
        placer.put(PartKind::Sandbags, at, Rotation::R0);
    }

    // The mess: the galley along the north wall from the corner, worked
    // from the row below, and tables with a chair a side under it, in
    // `mess_columns` columns four tiles apart, as many rows as the room
    // is deep for. One column on every station; a town's hall seats the
    // whole town.
    let m = floor.mess.inner();
    for (kind, x) in [
        (PartKind::ColdStore, m.x0 + 1),
        (PartKind::Worktop, m.x0 + 2),
        (PartKind::Hob, m.x0 + 4),
        (PartKind::Dishwasher, m.x0 + 5),
    ] {
        placer.put(kind, (x, m.y0), Rotation::R0);
    }
    for column in 0..floor.mess_columns {
        let x = m.x0 + 2 + 4 * column;
        let mut table_y = m.y0 + 3;
        while table_y + 1 <= m.y1 - 1 {
            placer.put(PartKind::Table, (x, table_y), Rotation::R0);
            placer.put(PartKind::Chair, (x, table_y + 1), Rotation::R0);
            placer.put(PartKind::Chair, (x + 1, table_y + 1), Rotation::R0);
            table_y += 4;
        }
    }

    // The crew's quarters: bunks down the west wall, a column every three
    // tiles for a plan that wants more of them — the bunk, its use tile,
    // and a tile of gangway before the next — never nearer the partition
    // than two tiles of gangway.
    let q = floor.quarters.inner();
    for column in 0..floor.bunk_columns {
        let Some(x) = q.x0.checked_add(3 * column) else {
            break;
        };
        if x + 3 > q.x1 {
            break;
        }
        let mut bunk_y = q.y0 + 1;
        while bunk_y + 1 <= q.y1 - 1 {
            placer.put(PartKind::Bunk, (x, bunk_y), Rotation::R180);
            bunk_y += 3;
        }
    }

    // The heads: along the north wall from the corner away from the
    // door, worked from the row below.
    let h = floor.heads.inner();
    for (kind, x) in [
        (PartKind::Toilet, h.x1 - 3),
        (PartKind::Basin, h.x1 - 2),
        (PartKind::Shower, h.x1 - 1),
    ] {
        placer.put(kind, (x, h.y0), Rotation::R0);
    }

    // The research room: the research desk against its north wall from
    // the corner, worked from the row below — the desk the station's key
    // sits on, and what the crew go ashore for — with its trays starting
    // a row lower than the laboratory's so the desk's spot has deck on
    // its far side (the room's navigation will not walk a spot between
    // two solids).
    let rr = floor.research.inner();
    placer.put(PartKind::ResearchDesk, (rr.x0 + 1, rr.y0), Rotation::R0);

    // The laboratory and the research room: runs of six trays, one every
    // three rows from two below the north wall so the row a run is worked
    // from and the row behind it are clear, as many as the seed likes and
    // the rooms hold — more on a bigger station, which feeds more. The
    // broom locker against the laboratory's north wall by the partition,
    // or the research room's where there is no laboratory.
    let bays = 1 + rng.below(3) + side.saturating_sub(40) / 6;
    let mut placed = 0;
    let mut bay_rooms = Vec::new();
    if let Some(lab) = floor.lab {
        bay_rooms.push((lab.inner(), 2));
    }
    bay_rooms.push((rr, 3));
    for (room, first_row) in bay_rooms {
        let columns = ((room.x1 - room.x0 + 1) / 7).max(1);
        let mut i = 0;
        while placed < bays {
            let (column, row) = (i % columns, i / columns);
            let at = (room.x0 + 1 + column * 7, room.y0 + first_row + row * 3);
            if at.1 + 2 > room.y1 || at.0 + 5 > room.x1 {
                break;
            }
            placer.put(PartKind::HydroBay, at, Rotation::R0);
            placed += 1;
            i += 1;
        }
    }
    let locker = floor.lab.map(|lab| lab.inner()).unwrap_or(rr);
    placer.put(PartKind::BroomLocker, (locker.x0, locker.y0), Rotation::R0);

    // The rec room: a table with a chair a side, as many as the room is
    // deep for, down its west side.
    if let Some(rec) = floor.rec {
        let r = rec.inner();
        let mut table_y = r.y0 + 1;
        while table_y + 1 <= r.y1 - 1 {
            placer.put(PartKind::Table, (r.x0 + 1, table_y), Rotation::R0);
            placer.put(PartKind::Chair, (r.x0 + 1, table_y + 1), Rotation::R0);
            placer.put(PartKind::Chair, (r.x0 + 2, table_y + 1), Rotation::R0);
            table_y += 4;
        }
    }

    // The stores: shelves along the north walls, two tiles apart, and a
    // second row four tiles down in a room deep enough.
    let shelves = 2 + rng.below(4) + side.saturating_sub(40) / 4;
    let mut placed = 0;
    for room in floor.stores.iter().map(|b| b.inner()) {
        let mut shelf_y = room.y0;
        while placed < shelves && shelf_y + 2 <= room.y1 {
            let mut shelf_x = room.x0 + 2;
            while placed < shelves && shelf_x + 1 <= room.x1 - 1 {
                placer.put(PartKind::Shelf, (shelf_x, shelf_y), Rotation::R0);
                placed += 1;
                shelf_x += 2;
            }
            shelf_y += 4;
        }
    }

    // Light: a wall light in every lit block's inner corners and one
    // every six tiles along its long walls, on whatever tile is still
    // free — last, so a lamp never takes a fixture's tile — each turned
    // to the wall at its back (`Placer::hung`), and none where two
    // blocks open into each other and there is no wall to hang from. A
    // tile no light reaches is dark, and a dark deck is one the crew see
    // ten tiles across (`bims::sight`).
    for block in &floor.lit {
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
            if let Some(hung) = placer.hung(at) {
                placer.put(PartKind::WallLight, at, hung);
            }
        }
    }

    // Comforts, since people live here: a big plant in the hall, a small
    // one in the mess and the rec room a tile in from their far corners,
    // and a picture on the north wall of the quarters and the rec room,
    // hung like a lamp. After the lamps, so a comfort never takes a
    // lamp's tile — one whose tile is taken is left out, and the lamp
    // stays. Every tile within reach of one scores higher to a Bim
    // standing on it (`shipdesign::comfort`, `bims::filth`), which is
    // what makes a station somewhere to live rather than a corridor with
    // bunks off it.
    let r = floor.rec.map(|rec| rec.inner());
    let mut comforts = vec![
        (PartKind::BigPlant, floor.hall),
        (PartKind::SmallPlant, (m.x1 - 1, m.y1)),
    ];
    if let Some(r) = r {
        comforts.push((PartKind::SmallPlant, (r.x1 - 1, r.y1)));
    }
    comforts.push((PartKind::Picture, (q.x0 + 1, q.y0)));
    if let Some(r) = r {
        comforts.push((PartKind::Picture, (r.x0 + 4, r.y0)));
    }
    for (kind, at) in comforts {
        let turn = if shipdesign::hangs_on_wall(kind) {
            placer.hung(at)
        } else {
            Some(Rotation::R0)
        };
        if let Some(turn) = turn {
            placer.put(kind, at, turn);
        }
    }

    // Whatever else the plan wants: a town's houses, bathhouses,
    // greenhouses and fields, laid clear of the lamps by the plan since a
    // refusal here is skipped.
    for &(kind, at, rotation) in &floor.extra {
        placer.put(kind, at, rotation);
    }

    // Standing lights, where the plan plants them: a surface's open
    // ground, which has no wall to hang a lamp from. After everything
    // else but the wild, so a light never takes a fixture's tile.
    for &at in &floor.standing_lights {
        placer.put(PartKind::StandingLight, at, Rotation::R0);
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
            placer.take(tile);
        }
    }

    // And the wild round a town, last of all: everything the town is not,
    // out to the edge of the ground.
    if let Some(biome) = floor.wild {
        crate::surface::wild(&mut placer, &floor, biome, &mut rng);
    }

    placer
}

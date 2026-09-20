//! A planet's surface is a place, and landing on it is a docking.
//!
//! A rocky planet or an ice world has a **settlement** on it — the town
//! the ship lands at — and the settlement is a [`Station`] like any other:
//! a [`ShipDesign`](shipdesign::ShipDesign) laid out on the ground
//! ([`Plan::Surface`]), with a port the ship's airlock mates to, a trading
//! desk, people living in it, a side and a shelf. Everything the world
//! knows how to do at a station — dock, join the rooms, open the
//! residents' room, trade, fight, hire — it does on the surface unchanged,
//! because the surface *is* a station to it, found through
//! [`World::station`](crate::World::station) by an id of its own:
//! [`surface_id`] of the body, well clear of any station the generator
//! numbers. What is different is the ground: the hull is **open** — no
//! skin, every tile deck, the edge of the deck the edge of the world — and
//! the painter draws the planet under it instead of the stars.
//!
//! # Rolled here, not in the generator
//!
//! The generator does not know a planet has a surface; the galaxy checksum
//! is what it was. A surface's seed, its side ([`Surface::hostile`], at
//! `worldgen`'s own share) and its shelf come off a stream of their own
//! ([`Purpose::Settlement`]) from the galaxy seed, the star and the body,
//! so two clients roll the same town on the same planet and nothing else
//! in the galaxy moves. The side goes on the world's `hostile` list with
//! the stations', so `World::stance` reads it and `world_checksum` eats
//! it.
//!
//! # Built when it is wanted
//!
//! A surface is [`data::SURFACE_SIDE`] tiles of deck and every one of them
//! is an `apply`, which is more than the biggest station; a system has a
//! few landable bodies and a world opens every station of its system at
//! once. So a [`Surface`] carries its roll and builds its [`Station`] the
//! first time somebody asks for it — [`Surface::station`], through a
//! `OnceLock` — which is at a landing, or when the map asks.

use std::sync::OnceLock;

use shipdesign::Rotation;
use shipdesign::parts::TILE;
use worldgen::math::{DVec2, dvec2};
use worldgen::rng::{Purpose, Rng, mix, seed_for};
use worldgen::{BodyKind, StarSystem, StationKind, Stock};

use crate::data;
use crate::station::{Block, Floor, Plan, Station, enclose, layout};

/// The bit that marks a station id as a surface's. The generator numbers
/// its stations from nought and never gets near this.
pub const SURFACE_BASE: u32 = 0x4000_0000;

/// The station id of a body's surface.
pub fn surface_id(body: u32) -> u32 {
    SURFACE_BASE | body
}

/// The body whose surface a station id names, if it names one.
pub fn surface_body(id: u32) -> Option<u32> {
    (id & SURFACE_BASE != 0).then_some(id & !SURFACE_BASE)
}

/// Whether a ship can land on a body of this kind: the ones with ground.
pub fn landable(kind: BodyKind) -> bool {
    matches!(kind, BodyKind::RockyPlanet | BodyKind::IceWorld)
}

/// The kind a surface's station is built as. An orbital's: what is on
/// the shelf is the orbital's list — fibre where there is ground to grow
/// it — and how many may live there is its ceiling.
pub const SURFACE_KIND: StationKind = StationKind::Orbital;

/// Which of a settlement's people is its guard: the first. Posted at
/// [`GUARD_POST`] whenever the surface's room is opened or rebuilt
/// (`crate::crew::Residents::post_guard`), and back there after every
/// errand (`bims::game::Game::send_to`).
pub const GUARD: u32 = 0;

/// The tile the guard stands on: outside the watch house's door, facing
/// the pad, between its two sandbags. Kept clear by the plan.
pub const GUARD_POST: (u32, u32) = (5, 40);

/// The middle of the guard's tile, in the surface's design units.
pub fn guard_post() -> DVec2 {
    let t = TILE as f64;
    dvec2(
        (GUARD_POST.0 as f64 + 0.5) * t,
        (GUARD_POST.1 as f64 + 0.5) * t,
    )
}

/// One landable body's settlement, as rolled: its seed, its side and its
/// shelf, and its station once it has been built.
#[derive(Debug)]
pub struct Surface {
    pub body: u32,
    pub kind: BodyKind,
    /// [`surface_id`] of the body.
    pub id: u32,
    /// The body's position: the middle of the ground.
    pub position: DVec2,
    pub map_seed: u64,
    /// Whether its people are enemies, as rolled; the rule is
    /// `World::stance`, off the `hostile` list this is put on.
    pub hostile: bool,
    pub stock: Stock,
    built: OnceLock<Station>,
}

impl Surface {
    /// Every landable body of a system, in body order, rolled off the
    /// galaxy seed, the star and each body's id: the same planet is the
    /// same town for two players.
    pub fn all_of(system: &StarSystem, galaxy_seed: u64) -> Vec<Surface> {
        let base = seed_for(
            galaxy_seed,
            system.star_id,
            worldgen::GENERATOR_VERSION,
            Purpose::Settlement,
        );
        system
            .bodies
            .iter()
            .filter(|body| landable(body.kind))
            .map(|body| {
                let stream = Rng::new(base ^ mix(body.id as u64));
                // Each off its own branch, like a station's in the
                // generator, so reworking one does not move the others.
                let map_seed = stream.branch(0x_4d41_5000_0000_0000).next_u64();
                let hostile = stream
                    .branch(0x_484f_5354_0000_0000)
                    .chance(worldgen::data::HOSTILE_SHARE);
                let stock = Stock::roll(SURFACE_KIND, &mut stream.branch(0x_5354_4f43_4b00_0000));
                Surface {
                    body: body.id,
                    kind: body.kind,
                    id: surface_id(body.id),
                    position: body.position,
                    map_seed,
                    hostile,
                    stock,
                    built: OnceLock::new(),
                }
            })
            .collect()
    }

    /// The settlement as a station, built the first time it is asked for
    /// and kept: [`Plan::Surface`] on the surface's seed, its grid
    /// centred on the body, no key on its desk.
    pub fn station(&self) -> &Station {
        self.built.get_or_init(|| {
            let design = layout(SURFACE_KIND, Plan::Surface, self.map_seed);
            let half = design.build_area as f64 * TILE as f64 / 2.0;
            Station {
                id: self.id,
                kind: SURFACE_KIND,
                plan: Plan::Surface,
                anchor: self
                    .position
                    .sub(flight::angle::rotate_design(dvec2(half, half), 0.0)),
                design,
                map_seed: self.map_seed,
                stock: self.stock,
                hostile: self.hostile,
                key: false,
            }
        })
    }

    /// Whether the station has been built yet — for the tests, which
    /// want a world to open without building every town in the system.
    pub fn is_built(&self) -> bool {
        self.built.get().is_some()
    }
}

// --- the plan ---------------------------------------------------------------

/// Where the trading house stands and where the watch house does, in
/// hull tiles from the west edge — the pad's — and the north.
const HOUSE_X: u32 = 8;
const HOUSE_Y: u32 = 6;
/// A room of the trading house is this many tiles across, walls
/// included, and the two rows of rooms this many tall.
const ROOM: u32 = 12;
const WATCH_Y: u32 = 36;
const WATCH_SIDE: u32 = 8;

/// The settlement's floor: the whole build area bar its rim is ground —
/// open, no skin, deck to the edge — with the port in the west edge at
/// the middle, where the pad is; the **trading house** a block of six
/// rooms in two rows to the north-east of the pad — the trading hall
/// (the reactor room: the desk, the generator, life support, the
/// batteries), the mess and the quarters along the north row, each with
/// a door of its own onto the ground, the heads, the research room and
/// the store along the south row the same — and the **watch house**, a
/// small building south of it by the pad with the settlement's sensor
/// dish on its roof, its door towards the pad, two sandbags before it
/// and the guard's post ([`GUARD_POST`]) between them. Standing lights
/// round the ground, since nothing out there has a wall to hang a lamp
/// from, and a big plant in the yard. Every door is two tiles and every
/// fixture stands clear as the station plans' do, for the room's
/// navigation (`crates/world/CLAUDE.md`, the walkability contract).
pub(crate) fn floor(side: u32) -> Floor {
    debug_assert_eq!(side, data::SURFACE_SIDE);
    let last = side - 2;
    let mid = side / 2;
    let hull = vec![Block::new(1, 1, last, last)];

    // The trading house: two rows of three rooms, sharing their walls.
    let (x0, y0) = (HOUSE_X, HOUSE_Y);
    let lobby = Block::new(x0, y0, x0 + ROOM, y0 + ROOM);
    let mess = Block::new(x0 + ROOM, y0, x0 + 2 * ROOM, y0 + ROOM);
    let quarters = Block::new(x0 + 2 * ROOM, y0, x0 + 3 * ROOM, y0 + ROOM);
    let y1 = y0 + ROOM;
    let heads = Block::new(x0, y1, x0 + 10, y1 + ROOM);
    let research = Block::new(x0 + 10, y1, x0 + 2 * ROOM, y1 + ROOM);
    let store = Block::new(x0 + 2 * ROOM, y1, x0 + 3 * ROOM, y1 + ROOM);
    // The watch house, south of it by the pad.
    let watch = Block::new(x0, WATCH_Y, x0 + WATCH_SIDE, WATCH_Y + WATCH_SIDE);

    let mut walls = Vec::new();
    let mut doors = Vec::new();
    // Each room's door in the wall that faces the ground, clear of what
    // stands inside: the hall's and the heads' towards the pad, the
    // mess's and the quarters' in their north walls, the research
    // room's and the store's in their south.
    enclose(
        lobby,
        &[((lobby.x0, lobby.y0 + 6), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        mess,
        &[((mess.x0 + 8, mess.y0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        quarters,
        &[((quarters.x0 + 8, quarters.y0), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        heads,
        &[((heads.x0, heads.y0 + 6), Rotation::R0)],
        &mut walls,
        &mut doors,
    );
    enclose(
        research,
        &[((research.x0 + 10, research.y1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        store,
        &[((store.x0 + 1, store.y1), Rotation::R90)],
        &mut walls,
        &mut doors,
    );
    enclose(
        watch,
        &[((watch.x0, watch.y0 + 4), Rotation::R0)],
        &mut walls,
        &mut doors,
    );

    // Two sandbags before the watch house's door, the post between them.
    let cover = vec![
        (GUARD_POST.0 - 1, GUARD_POST.1 - 2),
        (GUARD_POST.0 - 1, GUARD_POST.1 + 3),
    ];

    // Standing lights round the ground and between the buildings, none
    // within two tiles of a door or of the post.
    let standing_lights = vec![
        (3, 3),
        (mid - 2, 3),
        (last - 4, 3),
        (last - 4, 15),
        (last - 4, mid - 1),
        (last - 4, 40),
        (last - 4, last - 2),
        (mid - 2, last - 2),
        (3, last - 2),
        (4, 21),
        (4, 33),
        (mid - 2, 35),
        (40, 36),
        (22, 46),
        (36, 48),
        (14, 50),
    ];

    Floor {
        hull,
        airlocks: vec![((1, mid - 1), Rotation::R0)],
        // The dish on the watch house's roof.
        array: (watch.x0 + 4, watch.y0),
        lobby: lobby.inner(),
        walls,
        doors,
        cover,
        mess,
        quarters,
        heads,
        research,
        lab: None,
        rec: None,
        stores: vec![store],
        bunk_columns: 2,
        lit: vec![lobby, mess, quarters, heads, research, store, watch],
        hall: (23, 34),
        open: true,
        standing_lights,
    }
}

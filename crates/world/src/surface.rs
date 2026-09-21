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
//! A surface is [`data::SURFACE_SIDE`] tiles of deck and twenty thousand
//! parts on it, more than the biggest station by far; a system has a few
//! landable bodies and a world opens every station of its system at once.
//! So a [`Surface`] carries its roll and builds its [`Station`] the first
//! time somebody asks for it — [`Surface::station`], through a `OnceLock`
//! — which is at a landing, or when the map asks.
//!
//! # A town, and the wild round it
//!
//! What is built is a **town** sized by its population and shaped by its
//! biome ([`floor`]): the pad, the watch house and the trading house by
//! it, a gathering hall, houses along three streets, bathhouses, fields
//! or greenhouses, and then the wild ([`wild`]) — forest, rock, water —
//! over every tile of ground the town does not use, out to the edge of
//! the deck. A Bim leaving the ship can walk any way; what stops it is a
//! tree, a boulder, a house wall or the water, never a line drawn on the
//! ground, and the streets run out through the wild to the edge of the
//! world so there is always a way out. The rules are the station plans'
//! (`crates/world/CLAUDE.md`, the walkability contract): every door two
//! tiles, every use spot with deck beyond it, and nothing left that the
//! room's navigation cannot reach from the pad.

use std::sync::OnceLock;

use economy::market::Bias;
use shipdesign::parts::TILE;
use shipdesign::{PartKind, Rotation};
use worldgen::math::{DVec2, dvec2};
use worldgen::rng::{Purpose, Rng, mix, seed_for};
use worldgen::{BodyKind, StarSystem, StationKind, Stock};

use crate::data;
use crate::station::{Block, Floor, Placer, Plan, Station, enclose, layout_surface};

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

/// What kind of ground a settlement stands on: the planet's **biome**,
/// which decides what its people grow their food in — fields in the
/// soil of a desert or a temperate world, hydroponic bays under glass on
/// an arctic one — what the wild round the town is made of, and what
/// the painter draws the ground, the walls and the trees as. An ice
/// world is arctic; a rocky planet rolls desert or temperate. The codes
/// cross the seam and are never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Biome {
    Desert = 0,
    Temperate = 1,
    Arctic = 2,
}

impl Biome {
    pub const ALL: [Biome; 3] = [Biome::Desert, Biome::Temperate, Biome::Arctic];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Biome> {
        Biome::ALL.get(code as usize).copied()
    }
}

/// Which of a settlement's people is its guard: the first. Posted at
/// [`GUARD_POST`] whenever the surface's room is opened or rebuilt
/// (`crate::crew::Residents::post_guard`), and back there after every
/// errand (`bims::game::Game::send_to`).
pub const GUARD: u32 = 0;

/// The tile the guard stands on: outside the watch house's door, facing
/// the pad, between its two sandbags — a few tiles south of the pad,
/// whatever the side. Kept clear by the plan, and by the wild.
pub const GUARD_POST: (u32, u32) = (5, data::SURFACE_SIDE / 2 + 8);

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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// What ground the town stands on: arctic on an ice world, desert or
    /// temperate on a rocky planet, rolled evenly.
    pub biome: Biome,
    /// How many live there, [`data::SURFACE_POPULATION`] inclusive — what
    /// the town is sized by, and what its station's `residents()` answers.
    pub population: u32,
    pub stock: Stock,
    /// The desk's lean on every price, rolled here as a station's is by
    /// the generator; quoted as an `economy::market::MarketKind::Settlement`'s.
    pub bias: Bias,
    /// Left out of a save: it is a function of the roll above, and the
    /// first ask after a load builds it again.
    #[cfg_attr(feature = "serde", serde(skip))]
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
                // And the desk's lean, off "BIAS" as the generator's is.
                let bias = worldgen::data::price_bias(&mut stream.branch(0x_4249_4153_0000_0000));
                // The ground: an ice world is arctic; a rocky planet rolls
                // desert or temperate evenly, off "BIOME".
                let biome = if body.kind == BodyKind::IceWorld {
                    Biome::Arctic
                } else if stream.branch(0x_4249_4f4d_4500_0000).below(2) == 0 {
                    Biome::Desert
                } else {
                    Biome::Temperate
                };
                // And how many live there, off "POPL".
                let (least, most) = data::SURFACE_POPULATION;
                let population = least
                    + stream
                        .branch(0x_504f_504c_0000_0000)
                        .below(most - least + 1);
                Surface {
                    body: body.id,
                    kind: body.kind,
                    id: surface_id(body.id),
                    position: body.position,
                    map_seed,
                    hostile,
                    biome,
                    population,
                    stock,
                    bias,
                    built: OnceLock::new(),
                }
            })
            .collect()
    }

    /// The town as a station, built the first time it is asked for and
    /// kept: [`Plan::Surface`] on the surface's seed, in its biome, sized
    /// to its population ([`layout_surface`]), its grid centred on the
    /// body, no key on its desk.
    /// The ground beyond the town: the plain's rule, off the same seed
    /// the town is, so a planet is the same planet everywhere. See
    /// `bims::terrain`.
    pub fn terrain(&self) -> bims::terrain::Terrain {
        bims::terrain::Terrain::new(
            self.map_seed ^ 0x_504c_4149_4e00_0000,
            self.biome.code() as u8,
            data::SURFACE_SIDE,
        )
    }

    pub fn station(&self) -> &Station {
        self.built.get_or_init(|| {
            let design = layout_surface(self.map_seed, self.biome, self.population);
            let half = design.build_area as f64 * TILE as f64 / 2.0;
            Station {
                id: self.id,
                kind: SURFACE_KIND,
                plan: Plan::Surface,
                anchor: self
                    .position
                    .sub(flight::angle::rotate_design(dvec2(half, half), 0.0)),
                design,
                population: self.population,
                map_seed: self.map_seed,
                stock: self.stock,
                bias: self.bias,
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
//
// The ground is `SURFACE_SIDE` tiles square, deck from tile 1 to tile
// `SURFACE_SIDE - 2` and void at the rim; the numbers below are tiles from
// its top-left. The pad is in the west edge at the middle, and everything
// is laid out from it: the yard before it, the trading house north of the
// yard and the watch house south of it, and three streets — the main
// street east from the pad, and one to the north and one to the south of
// it — crossed by two more running north to south. What the streets cut
// the ground into is the lots the buildings stand in, each fronting the
// street its door opens on.

/// The first and the last tile of deck.
const FIRST: u32 = 1;
const LAST: u32 = data::SURFACE_SIDE - 2;
/// The main street: eight tiles either side of the pad's rows, from the
/// yard to the east edge.
const MAIN_Y0: u32 = data::SURFACE_SIDE / 2 - 4;
const MAIN_Y1: u32 = data::SURFACE_SIDE / 2 + 3;
/// The other streets are this wide; the north and south streets start at
/// these rows, the west cross street at this column, and the east one
/// three tiles past the hall, wherever the hall ends.
const STREET: u32 = 6;
const NORTH_Y0: u32 = 20;
const SOUTH_Y0: u32 = 70;
const CROSS_A_X0: u32 = 30;
/// The first column a building stands in; west of it is the pad's yard,
/// this many rows of it kept open before the pad.
const TOWN_X0: u32 = 8;
const YARD_Y0: u32 = 28;
const YARD_Y1: u32 = 62;
/// The four bands of lots between the streets, top row to bottom row.
const LOT_N: (u32, u32) = (8, 19);
const LOT_M: (u32, u32) = (26, 43);
const LOT_S: (u32, u32) = (52, 69);
const LOT_F: (u32, u32) = (76, 88);
/// The watch house: a small square by the pad with its door onto the
/// yard, the guard's post three tiles before it.
const WATCH_X0: u32 = GUARD_POST.0 + 3;
const WATCH_Y0: u32 = GUARD_POST.1 - 4;
const WATCH_SIDE: u32 = 8;
/// The trading house: the trading hall (the reactor room, eleven wide
/// and fourteen tall for the desk, the reactor, life support and the
/// batteries at `furnish`'s offsets) with its door onto the yard, the
/// research room beside it — eight rows inside, which is room for the
/// desk and one run of trays and no more — with its door onto the west
/// cross street, and the store under that, opening onto the main street.
const LOBBY: Block = Block::new(TOWN_X0, 30, TOWN_X0 + 10, MAIN_Y0 - 1);
const RESEARCH: Block = Block::new(TOWN_X0 + 10, 30, CROSS_A_X0 - 1, 39);
const STORE: Block = Block::new(TOWN_X0 + 10, 39, CROSS_A_X0 - 1, MAIN_Y0 - 1);
/// The gathering hall stands here, fronting the main street, as wide as
/// its tables want and this many rows of them deep.
const HALL_X0: u32 = CROSS_A_X0 + STREET;
const HALL_ROWS: u32 = 3;
/// The forest, the rock or the ice at the edge of the world is this many
/// tiles deep, in [`SECTORS`] stretches round the perimeter, each of
/// which is a gap — thin enough to walk through — at [`GAP_CHANCE`].
const RING: u32 = 6;
const SECTORS: usize = 32;
const GAP_CHANCE: f64 = 0.3;
/// The town's own rolls — where a house stands, what stands in it, the
/// shape of the wild — come off the seed salted, so the furnisher's rolls
/// (the batteries, the shelves) are what the station plans' are.
const TOWN_SALT: u64 = 0x_544f_574e_0000_0000;

/// One kind of building along a street.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Building {
    /// Bunks in `columns` columns three tiles apart, `bunks` to a column
    /// every three rows, the quarters' pattern: the bunk, its use tile
    /// beside it, a tile of gangway.
    House { columns: u32, bunks: u32 },
    /// A toilet, a basin and a shower along the north wall.
    Bath,
    /// Three runs of hydroponic trays down the west side and an aisle
    /// two wide down the east, for a town that cannot farm.
    Greenhouse,
}

impl Building {
    /// Tiles across and down, walls included. A house is four inside for
    /// one column — the bunk, its spot, and two of gangway for the door
    /// — and seven for two; its height leaves the corner under the last
    /// bunk free, since the lamps go in before the bunks and take the
    /// inner corners.
    fn size(self) -> (u32, u32) {
        match self {
            Building::House { columns, bunks } => (3 * columns + 3, 3 * bunks + 3),
            Building::Bath => (8, 6),
            Building::Greenhouse => (11, 12),
        }
    }

    /// Where the door's first tile is along the street wall, from the
    /// block's corner: past the corner lamp and clear of what stands
    /// inside — a house's bunk column, a bathhouse's fittings, which are
    /// at the far end of the wall, a greenhouse's trays.
    fn door_dx(self) -> u32 {
        match self {
            Building::House { .. } => 2,
            Building::Bath => 1,
            Building::Greenhouse => 8,
        }
    }

    /// What to try instead when this does not fit: the next smaller house.
    fn smaller(self) -> Option<Building> {
        match self {
            Building::House {
                columns: 2,
                bunks: 3,
            } => Some(Building::House {
                columns: 2,
                bunks: 2,
            }),
            Building::House {
                columns: 2,
                bunks: 2,
            } => Some(Building::House {
                columns: 1,
                bunks: 3,
            }),
            Building::House {
                columns: 1,
                bunks: 3,
            } => Some(Building::House {
                columns: 1,
                bunks: 2,
            }),
            _ => None,
        }
    }
}

/// One side of one street a row of buildings fronts: the columns it
/// spans, the row the street wall stands in, whether the street lies to
/// the south of the buildings (their doors in their south walls) and how
/// many rows away from the street a building may reach.
#[derive(Clone, Copy)]
struct Frontage {
    x0: u32,
    x1: u32,
    wall: u32,
    south: bool,
    deep: u32,
}

/// What the plan gathers as it lays the town out.
#[derive(Default)]
struct Town {
    walls: Vec<(u32, u32)>,
    doors: Vec<((u32, u32), Rotation)>,
    lit: Vec<Block>,
    extra: Vec<(PartKind, (u32, u32), Rotation)>,
    clear: Vec<Block>,
    bunks: u32,
    bays: u32,
    strips: u32,
}

impl Town {
    /// A walled room with one two-tile door, lit.
    fn room(&mut self, block: Block, door: ((u32, u32), Rotation)) {
        enclose(block, &[door], &mut self.walls, &mut self.doors);
        self.lit.push(block);
    }

    /// A building of `kind` standing on `block`, its door in the south
    /// wall when the street is to the south and in the north wall
    /// otherwise, and what stands inside it as extras. Every fixture is
    /// laid clear of where `furnish` hangs the lamps — the inner corners
    /// and every sixth tile along the walls — since the lamps go in
    /// first and an extra on a lamp's tile is dropped.
    fn build(&mut self, kind: Building, block: Block, south: bool, rng: &mut Rng) {
        let inner = block.inner();
        let door_y = if south { block.y1 } else { block.y0 };
        self.room(block, ((block.x0 + kind.door_dx(), door_y), Rotation::R90));
        match kind {
            Building::House { columns, bunks } => {
                for column in 0..columns {
                    for row in 0..bunks {
                        self.extra.push((
                            PartKind::Bunk,
                            (inner.x0 + 3 * column, inner.y0 + 1 + 3 * row),
                            Rotation::R180,
                        ));
                    }
                }
                self.bunks += columns * bunks;
                // A comfort in some: a picture on the north wall, a plant
                // in the far corner — neither in a bunk's column, neither
                // blocking a step.
                if rng.chance(0.6) {
                    self.extra
                        .push((PartKind::Picture, (inner.x1 - 1, inner.y0), Rotation::R0));
                }
                if rng.chance(0.35) {
                    self.extra
                        .push((PartKind::SmallPlant, (inner.x1 - 1, inner.y1), Rotation::R0));
                }
            }
            Building::Bath => {
                for (kind, x) in [
                    (PartKind::Toilet, inner.x1 - 3),
                    (PartKind::Basin, inner.x1 - 2),
                    (PartKind::Shower, inner.x1 - 1),
                ] {
                    self.extra.push((kind, (x, inner.y0), Rotation::R0));
                }
            }
            Building::Greenhouse => {
                for run in 0..3 {
                    self.extra.push((
                        PartKind::HydroBay,
                        (inner.x0 + 1, inner.y0 + 2 + 3 * run),
                        Rotation::R0,
                    ));
                }
                self.bays += 3;
            }
        }
    }

    /// Which house to build next: mostly three bunks in a column, some
    /// of four in two columns, a few of two — more small ones in a small
    /// town, where the streets have room to spare, and in a big one
    /// lodging houses of six, two columns of three, which is the most
    /// bunks a tile of street buys, since the streets are what runs out.
    fn pick_house(&self, population: u32, rng: &mut Rng) -> Building {
        let roll = rng.below(100);
        let (six, three, four) = if population >= 40 {
            (45, 75, 92)
        } else if population >= 30 {
            (0, 55, 85)
        } else {
            (0, 40, 60)
        };
        if roll < six {
            Building::House {
                columns: 2,
                bunks: 3,
            }
        } else if roll < three {
            Building::House {
                columns: 1,
                bunks: 3,
            }
        } else if roll < four {
            Building::House {
                columns: 2,
                bunks: 2,
            }
        } else {
            Building::House {
                columns: 1,
                bunks: 2,
            }
        }
    }

    /// Buildings along a frontage from its west end, a gap of two or
    /// three tiles between them and each set back from the street by a
    /// tile or not: bathhouses while any are wanted, then houses until
    /// the town has its bunks. A building that will not fit — too wide
    /// for what is left of the frontage, too deep for the lot — is tried
    /// smaller, and the frontage is done when nothing fits.
    fn fill(
        &mut self,
        frontage: Frontage,
        baths: &mut u32,
        bunks_wanted: u32,
        population: u32,
        rng: &mut Rng,
    ) {
        let mut x = frontage.x0;
        loop {
            if *baths == 0 && self.bunks >= bunks_wanted {
                return;
            }
            let mut want = if *baths > 0 {
                Building::Bath
            } else {
                self.pick_house(population, rng)
            };
            let placed = loop {
                let (w, h) = want.size();
                if x + w - 1 <= frontage.x1 && h <= frontage.deep {
                    break Some((want, w, h));
                }
                match want.smaller() {
                    Some(smaller) => want = smaller,
                    None => break None,
                }
            };
            let Some((kind, w, h)) = placed else {
                return;
            };
            let setback = if kind == Building::Bath {
                0
            } else {
                rng.below(2).min(frontage.deep - h)
            };
            let block = if frontage.south {
                Block::new(
                    x,
                    frontage.wall - setback - h + 1,
                    x + w - 1,
                    frontage.wall - setback,
                )
            } else {
                Block::new(
                    x,
                    frontage.wall + setback,
                    x + w - 1,
                    frontage.wall + setback + h - 1,
                )
            };
            self.build(kind, block, frontage.south, rng);
            if kind == Building::Bath {
                *baths -= 1;
            }
            x = block.x1 + 3 + rng.below(2);
        }
    }

    /// Fields across a lot from its west end: blocks of four strips six
    /// wide, one every three rows so the row a strip is worked from and
    /// the row behind it are clear, two tiles of path between blocks, as
    /// many as the lot holds and the town wants. The lot is kept clear
    /// of the wild from the street to the last strip. Hands back the
    /// column the lot is free from.
    fn fields(&mut self, lot: Block, wanted: u32) -> u32 {
        let by = lot.y0 + 1;
        let mut bx = lot.x0 + 1;
        while self.strips < wanted && bx + 5 <= lot.x1 - 1 {
            for strip in 0..4 {
                self.extra
                    .push((PartKind::Field, (bx, by + 1 + 3 * strip), Rotation::R0));
            }
            self.clear
                .push(Block::new(bx - 1, lot.y0, bx + 7, (by + 11).min(lot.y1)));
            self.strips += 4;
            bx += 8;
        }
        bx
    }

    /// Greenhouses across a lot the same way, each against the street
    /// with its door onto it, until the town has its bays.
    fn greenhouses(&mut self, lot: Block, south: bool, wanted: u32, rng: &mut Rng) -> u32 {
        let (w, h) = Building::Greenhouse.size();
        let mut x = lot.x0;
        while self.bays < wanted && x + w - 1 <= lot.x1 {
            let block = if south {
                Block::new(x, lot.y1 - h + 1, x + w - 1, lot.y1)
            } else {
                Block::new(x, lot.y0, x + w - 1, lot.y0 + h - 1)
            };
            self.build(Building::Greenhouse, block, south, rng);
            x += w + 2;
        }
        x
    }
}

/// The town's floor plan, sized by its population and shaped by its
/// biome, deterministic in the four.
///
/// The whole build area bar its rim is ground — `Floor::open`, no skin,
/// deck to the edge — with the port at the middle of the west edge,
/// where the **pad** is. The **watch house** stands south of the pad
/// with the sensor dish on its roof, its door towards the pad, two
/// sandbags before it and the guard's post ([`GUARD_POST`]) between
/// them; the **trading house** north of it — the trading hall with the
/// desk, the reactor, life support and the batteries, the research room
/// with the research desk and one run of trays, and the store with its
/// shelves. The **main street** runs east from the pad; a street to the
/// north and one to the south run parallel, and two cross streets cut
/// the three. On the main street, in the middle, the **gathering
/// hall**: the galley along its north wall and tables in as many columns
/// as seat the whole town three rows deep. Then, along the streets from
/// the middle out, the **bathhouses** — one for every twelve people —
/// and the **houses**, two to four bunks each, a bunk for everybody and
/// two over for mercenaries, with a gap and a setback rolled for each so
/// no two towns are the same street. Beyond the east cross street and
/// along the south the **fields** — strips of soil worked like a bay at
/// half its pace, a strip for every two people — or, on an arctic
/// world, **greenhouses** of trays instead, a bay for every four. A big
/// plant on the main street before the hall, standing lights along every
/// street, a wall light in every building. And, after all of that,
/// [`wild`].
///
/// Every door is two tiles and every fixture stands clear as the station
/// plans' do, for the room's navigation (`crates/world/CLAUDE.md`, the
/// walkability contract): the tests walk every town from the pad.
pub(crate) fn floor(side: u32, biome: Biome, population: u32, seed: u64) -> Floor {
    debug_assert_eq!(side, data::SURFACE_SIDE);
    let last = side - 2;
    let mid = side / 2;
    let mut rng = Rng::new(seed ^ TOWN_SALT);
    let mut town = Town::default();

    // What stands by the pad, the same in every town.
    let watch = Block::new(
        WATCH_X0,
        WATCH_Y0,
        WATCH_X0 + WATCH_SIDE,
        WATCH_Y0 + WATCH_SIDE,
    );
    town.room(LOBBY, ((LOBBY.x0, LOBBY.y0 + 6), Rotation::R0));
    town.room(RESEARCH, ((RESEARCH.x1, RESEARCH.y0 + 6), Rotation::R0));
    town.room(STORE, ((STORE.x0 + 3, STORE.y1), Rotation::R90));
    town.room(watch, ((watch.x0, watch.y0 + 4), Rotation::R0));

    // The hall, as wide as its tables: a chair each, two a table, three
    // rows of tables four tiles apart, and the columns to make the number.
    let tables = population.div_ceil(2);
    let columns = tables.div_ceil(HALL_ROWS).max(1);
    let (hall_w, hall_h) = (4 * columns + 2, 4 * HALL_ROWS + 4);
    let hall = Block::new(HALL_X0, MAIN_Y0 - hall_h, HALL_X0 + hall_w - 1, MAIN_Y0 - 1);
    town.room(hall, ((hall.x0 + hall_w / 2 - 1, hall.y1), Rotation::R90));
    // The east cross street, three tiles past the hall.
    let xb = hall.x1 + 3;
    let east = xb + STREET;

    // The first house — the quarters, two columns of two — east of the
    // watch house, and the first bathhouse across the main street from
    // the hall; the rest come off the frontages.
    let quarters = Block::new(TOWN_X0 + 11, LOT_S.0, TOWN_X0 + 19, LOT_S.0 + 8);
    town.room(quarters, ((quarters.x0 + 4, quarters.y0), Rotation::R90));
    town.bunks += 4;
    let heads = Block::new(HALL_X0, LOT_S.0, HALL_X0 + 7, LOT_S.0 + 5);
    town.room(heads, ((heads.x0 + 1, heads.y0), Rotation::R90));

    // The streets and the yard: nothing grows on them.
    town.clear.extend([
        Block::new(FIRST + 1, MAIN_Y0, last, MAIN_Y1),
        Block::new(TOWN_X0, NORTH_Y0, last, NORTH_Y0 + STREET - 1),
        Block::new(TOWN_X0, SOUTH_Y0, last, SOUTH_Y0 + STREET - 1),
        Block::new(CROSS_A_X0, FIRST + 1, CROSS_A_X0 + STREET - 1, last),
        Block::new(xb, FIRST + 1, xb + STREET - 1, last),
        Block::new(FIRST + 1, YARD_Y0, TOWN_X0 - 1, YARD_Y1),
    ]);

    // Food: the lots beyond the east cross street, top to bottom, then
    // the south lot between the cross streets — fields, or greenhouses
    // where nothing grows outside — and what is left of each lot is a
    // frontage for houses after the streets' own.
    let food_lots = [
        (Block::new(east, LOT_N.0, last - 1, LOT_N.1), true),
        (Block::new(east, LOT_S.0, last - 1, LOT_S.1), false),
        (Block::new(east, LOT_F.0, last - 1, LOT_F.1), false),
        (Block::new(HALL_X0, LOT_F.0, xb - 1, LOT_F.1), false),
    ];
    let mut leftovers = Vec::new();
    for (lot, south) in food_lots {
        let next = match biome {
            // The research room's one run counts.
            Biome::Arctic => town.greenhouses(lot, south, population.div_ceil(4) - 1, &mut rng),
            _ => town.fields(lot, population.div_ceil(2)),
        };
        if next + 5 < lot.x1 {
            leftovers.push(Frontage {
                x0: next,
                x1: lot.x1,
                wall: if south { lot.y1 } else { lot.y0 },
                south,
                deep: (lot.y1 - lot.y0 + 1).min(13),
            });
        }
    }

    // The frontages, nearest the middle first: the main street's south
    // side across from the hall (bathhouses; nothing deeper fits before
    // the row behind), the south street's north side, the north street's
    // north side west and east of the cross street, the south street's
    // north side by the quarters, its south side, the east lot's two
    // streets, and whatever the food left.
    let mut frontages = vec![
        Frontage {
            x0: heads.x1 + 3,
            x1: xb - 1,
            wall: LOT_S.0,
            south: false,
            deep: 6,
        },
        Frontage {
            x0: HALL_X0,
            x1: xb - 1,
            wall: LOT_S.1,
            south: true,
            deep: 12,
        },
        Frontage {
            x0: TOWN_X0,
            x1: CROSS_A_X0 - 1,
            wall: LOT_N.1,
            south: true,
            deep: 12,
        },
        Frontage {
            x0: HALL_X0,
            x1: xb - 1,
            wall: LOT_N.1,
            south: true,
            deep: 12,
        },
        Frontage {
            x0: quarters.x0,
            x1: CROSS_A_X0 - 1,
            wall: LOT_S.1,
            south: true,
            deep: 9,
        },
        Frontage {
            x0: TOWN_X0,
            x1: CROSS_A_X0 - 1,
            wall: LOT_F.0,
            south: false,
            deep: 13,
        },
        Frontage {
            x0: east,
            x1: last - 1,
            wall: LOT_M.1,
            south: true,
            deep: 9,
        },
        Frontage {
            x0: east,
            x1: last - 1,
            wall: LOT_M.0,
            south: false,
            deep: 9,
        },
    ];
    frontages.extend(leftovers);
    let mut baths = population.div_ceil(12).saturating_sub(1);
    let bunks_wanted = population + 2;
    for frontage in frontages {
        if baths == 0 && town.bunks >= bunks_wanted {
            break;
        }
        town.fill(frontage, &mut baths, bunks_wanted, population, &mut rng);
    }

    // Standing lights along every street and two in the yard, none within
    // two tiles of a door's approach or the post; the big plant on the
    // main street before the hall.
    let standing_lights = vec![
        (12, MAIN_Y0 + 2),
        (24, MAIN_Y1 - 2),
        (38, MAIN_Y0 + 2),
        (52, MAIN_Y1 - 2),
        (70, MAIN_Y0 + 2),
        (86, MAIN_Y1 - 2),
        (14, NORTH_Y0 + 2),
        (44, NORTH_Y0 + 3),
        (74, NORTH_Y0 + 2),
        (14, SOUTH_Y0 + 2),
        (44, SOUTH_Y0 + 3),
        (74, SOUTH_Y0 + 2),
        (CROSS_A_X0 + 2, 12),
        (CROSS_A_X0 + 3, 60),
        (CROSS_A_X0 + 2, 84),
        (xb + 2, 12),
        (xb + 3, 60),
        (xb + 2, 84),
        (4, YARD_Y0 + 5),
        (4, YARD_Y1 + 1),
    ];

    Floor {
        hull: vec![Block::new(FIRST, FIRST, last, last)],
        airlocks: vec![((FIRST, mid - 1), Rotation::R0)],
        // The dish on the watch house's roof.
        array: (watch.x0 + 4, watch.y0),
        lobby: LOBBY.inner(),
        walls: town.walls,
        doors: town.doors,
        // Two sandbags before the watch house's door, the post between.
        cover: vec![
            (GUARD_POST.0 - 1, GUARD_POST.1 - 2),
            (GUARD_POST.0 - 1, GUARD_POST.1 + 3),
        ],
        mess: hall,
        quarters,
        heads,
        research: RESEARCH,
        lab: None,
        rec: None,
        stores: vec![STORE],
        bunk_columns: 2,
        lit: town.lit,
        hall: (hall.x0 + hall_w / 2, MAIN_Y0 + 3),
        open: true,
        standing_lights,
        extra: town.extra,
        mess_columns: columns,
        wild: Some(biome),
        clear: town.clear,
    }
}

// --- the wild ---------------------------------------------------------------

/// How thick the scatter is, by where a tile lies: in the ring at the
/// edge of the world, in a gap of that ring, and then by how far it is
/// from the nearest building or field — far out, in between, or in the
/// town.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Zone {
    Ring,
    Gap,
    Far,
    Mid,
    Near,
}

/// The chance a tile of a zone grows a tree, a shrub or a boulder, in a
/// biome — a forest at the edge of a temperate world with clearings in
/// it, rock at a desert's, ice and firs at an arctic one; sparse in the
/// town, thicker outside it.
fn rates(biome: Biome, zone: Zone) -> (f64, f64, f64) {
    match (biome, zone) {
        (Biome::Temperate, Zone::Ring) => (0.70, 0.05, 0.006),
        (Biome::Temperate, Zone::Gap) => (0.10, 0.04, 0.004),
        (Biome::Temperate, Zone::Far) => (0.20, 0.05, 0.006),
        (Biome::Temperate, Zone::Mid) => (0.06, 0.045, 0.003),
        (Biome::Temperate, Zone::Near) => (0.012, 0.02, 0.001),
        (Biome::Desert, Zone::Ring) => (0.0, 0.10, 0.10),
        (Biome::Desert, Zone::Gap) => (0.0, 0.06, 0.02),
        (Biome::Desert, Zone::Far) => (0.0, 0.05, 0.015),
        (Biome::Desert, Zone::Mid) => (0.0, 0.03, 0.006),
        (Biome::Desert, Zone::Near) => (0.0, 0.012, 0.002),
        (Biome::Arctic, Zone::Ring) => (0.30, 0.005, 0.30),
        (Biome::Arctic, Zone::Gap) => (0.06, 0.005, 0.06),
        (Biome::Arctic, Zone::Far) => (0.09, 0.01, 0.06),
        (Biome::Arctic, Zone::Mid) => (0.03, 0.005, 0.02),
        (Biome::Arctic, Zone::Near) => (0.008, 0.003, 0.005),
    }
}

/// What a pocket nobody can reach is filled with when no wild part is
/// near enough to copy: the biome's own ground cover.
fn ground_kind(biome: Biome) -> PartKind {
    match biome {
        Biome::Temperate => PartKind::Tree,
        Biome::Desert | Biome::Arctic => PartKind::Boulder,
    }
}

/// A clump of one kind about a centre: every tile within `r2` of it
/// grows the kind at `chance`.
struct Clump {
    at: (i32, i32),
    r2: i32,
    kind: PartKind,
    chance: f64,
}

/// A lake, a pool or a frozen lake: an ellipse of water.
#[derive(Clone, Copy)]
struct Lake {
    at: (i32, i32),
    rx: i32,
    ry: i32,
}

impl Lake {
    fn holds(&self, x: i32, y: i32) -> bool {
        let (dx, dy) = (x - self.at.0, y - self.at.1);
        dx * dx * self.ry * self.ry + dy * dy * self.rx * self.rx
            <= self.rx * self.rx * self.ry * self.ry
    }
}

/// How far a tile is from the edge of the deck.
fn rim(x: i32, y: i32) -> i32 {
    let (first, last) = (FIRST as i32, LAST as i32);
    (x - first).min(last - x).min(y - first).min(last - y)
}

/// Which stretch of the perimeter a tile of the ring is on: the tiles
/// along the edge it is nearest, counted clockwise from the north-west
/// corner in [`SECTORS`] stretches. Integers throughout, since a town
/// has to come out the same on every machine.
fn sector(x: i32, y: i32) -> usize {
    let (first, last) = (FIRST as i32, LAST as i32);
    let (n, e, s, w) = (y - first, last - x, last - y, x - first);
    let m = n.min(e).min(s).min(w);
    let along = if m == n {
        x - first
    } else if m == e {
        (last - first) + (y - first)
    } else if m == s {
        2 * (last - first) + (last - x)
    } else {
        3 * (last - first) + (last - y)
    };
    let span = 4 * (last - first) + 4;
    (along.max(0) * SECTORS as i32 / span) as usize % SECTORS
}

/// A breadth-first distance over the grid from every tile `source` says,
/// four ways, carrying the source's mark: `u32::MAX` and `None` where
/// nothing is reached. Tile order throughout, so two machines agree.
fn distances(
    side: i32,
    source: impl Fn(i32, i32) -> Option<PartKind>,
) -> (Vec<u32>, Vec<Option<PartKind>>) {
    let n = (side * side) as usize;
    let mut dist = vec![u32::MAX; n];
    let mut mark = vec![None; n];
    let mut queue = std::collections::VecDeque::new();
    for y in 0..side {
        for x in 0..side {
            if let Some(kind) = source(x, y) {
                let i = (y * side + x) as usize;
                dist[i] = 0;
                mark[i] = Some(kind);
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        let i = (y * side + x) as usize;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= side || ny >= side {
                continue;
            }
            let j = (ny * side + nx) as usize;
            if dist[j] != u32::MAX {
                continue;
            }
            dist[j] = dist[i] + 1;
            mark[j] = mark[i];
            queue.push_back((nx, ny));
        }
    }
    (dist, mark)
}

/// The wild: everything the town is not, out to the edge of the deck, so
/// that a Bim can walk any way from the ship and what stops it is the
/// ground — a forest, a cliff, the water — never the edge of a map.
///
/// By biome: a temperate world has a forest of `Tree`s round the edge
/// with gaps in it, clumps of trees further in, a scatter of `Shrub`s,
/// a lake and a few `Boulder`s; a desert has lines of `Boulder`s for
/// cliffs and outcrops of them, `Shrub`s the painter draws as cacti, and
/// one oasis — a pool with a few palms about it; an arctic world has
/// outcrops of rock, a frozen lake, a few firs and hardly a shrub. The
/// scatter is thicker at the edge and thinner in the town
/// ([`rates`]), and the streets run out through it, so there is always a
/// way to the edge of the world.
///
/// Nothing grows where a Bim has to stand or pass: on a use spot, a
/// door's tiles or the two beyond either face of it, the guard's post
/// or a standing light, nor within one tile (all eight neighbours) of
/// any of them; nor inside a building, on a street, in the yard or in a
/// field lot. And nothing is left that cannot be reached: after the
/// scatter, a flood from the pad's inside tile four ways over every tile
/// nothing blocks (a door is open) finds what it reaches, and every free
/// tile it did not reach is filled with the wild kind nearest it. A
/// one-tile straight gap is walked by the room's navigation and a
/// diagonal-only gap is not (`crates/game/CLAUDE.md`), and the four-way
/// flood says exactly that, so what it leaves free the room can walk.
pub(crate) fn wild(placer: &mut Placer, floor: &Floor, biome: Biome, rng: &mut Rng) {
    let side = placer.design.build_area as i32;
    let n = (side * side) as usize;
    let index = |x: i32, y: i32| (y * side + x) as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < side && y < side;

    // Where nothing may grow.
    let mut keep = vec![false; n];
    let keep_around = |keep: &mut Vec<bool>, x: i32, y: i32| {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if inside(x + dx, y + dy) {
                    keep[index(x + dx, y + dy)] = true;
                }
            }
        }
    };
    for part in &placer.design.parts {
        for (sx, sy) in part.use_spots() {
            keep_around(&mut keep, sx, sy);
        }
        match part.kind {
            PartKind::Door | PartKind::Airlock => {
                // A door lies along its wall: one wide and two tall in a
                // column, two wide and one tall in a row. Its tiles, and
                // the two beyond each face.
                let across = matches!(part.rotation, Rotation::R0 | Rotation::R180);
                for (x, y) in part.tiles() {
                    let (x, y) = (x as i32, y as i32);
                    keep_around(&mut keep, x, y);
                    if across {
                        keep_around(&mut keep, x - 1, y);
                        keep_around(&mut keep, x + 1, y);
                    } else {
                        keep_around(&mut keep, x, y - 1);
                        keep_around(&mut keep, x, y + 1);
                    }
                }
            }
            PartKind::StandingLight => {
                keep_around(&mut keep, part.origin.0 as i32, part.origin.1 as i32);
            }
            _ => {}
        }
    }
    keep_around(&mut keep, GUARD_POST.0 as i32, GUARD_POST.1 as i32);
    for block in floor.lit.iter().chain(&floor.clear) {
        for y in block.y0..=block.y1 {
            for x in block.x0..=block.x1 {
                if inside(x as i32, y as i32) {
                    keep[index(x as i32, y as i32)] = true;
                }
            }
        }
    }
    let eligible = |placer: &Placer, x: i32, y: i32| {
        inside(x, y)
            && !keep[index(x, y)]
            && placer.get(shipdesign::Layer::Floor, (x, y)) != 0
            && placer.get(shipdesign::Layer::Object, (x, y)) == 0
    };

    // How far every tile is from the town — a building or a field.
    let built: Vec<bool> = {
        let mut built = vec![false; n];
        for block in &floor.lit {
            for y in block.y0..=block.y1 {
                for x in block.x0..=block.x1 {
                    if inside(x as i32, y as i32) {
                        built[index(x as i32, y as i32)] = true;
                    }
                }
            }
        }
        for part in &placer.design.parts {
            if part.kind == PartKind::Field {
                for (x, y) in part.tiles() {
                    built[index(x as i32, y as i32)] = true;
                }
            }
        }
        built
    };
    let (dist, _) = distances(side, |x, y| built[index(x, y)].then_some(PartKind::Wall));

    // The ring's gaps, a roll a stretch.
    let gaps: Vec<bool> = (0..SECTORS).map(|_| rng.chance(GAP_CHANCE)).collect();
    let zone = |x: i32, y: i32| {
        if rim(x, y) < RING as i32 {
            if gaps[sector(x, y)] {
                Zone::Gap
            } else {
                Zone::Ring
            }
        } else {
            match dist[index(x, y)] {
                d if d >= 12 => Zone::Far,
                d if d >= 6 => Zone::Mid,
                _ => Zone::Near,
            }
        }
    };

    // The features, rolled before the scatter: clumps, cliffs, water.
    // A spot for one is somewhere on the ground, a few tiles from the
    // nearest building and free to grow on; a roll that finds none in a
    // few tries is a town without that feature, which is fine.
    let spot = |rng: &mut Rng, placer: &Placer, margin: i32, off: u32| -> Option<(i32, i32)> {
        for _ in 0..80 {
            let x = rng.below(side as u32) as i32;
            let y = rng.below(side as u32) as i32;
            if rim(x, y) >= margin && dist[index(x, y)] >= off && eligible(placer, x, y) {
                return Some((x, y));
            }
        }
        None
    };
    let mut clumps: Vec<Clump> = Vec::new();
    let mut lines: Vec<(i32, i32)> = Vec::new();
    let lake: Option<Lake>;
    let mut palms: Vec<(i32, i32)> = Vec::new();
    match biome {
        Biome::Temperate => {
            let count = 14 + rng.below(8);
            for _ in 0..count {
                let r = 2 + rng.below(3) as i32;
                if let Some(at) = spot(rng, placer, 1, 4) {
                    clumps.push(Clump {
                        at,
                        r2: r * r,
                        kind: PartKind::Tree,
                        chance: 0.65,
                    });
                }
            }
            let (rx, ry) = (3 + rng.below(3) as i32, 2 + rng.below(3) as i32);
            lake = spot(rng, placer, rx.max(ry) + 2, 4).map(|at| Lake { at, rx, ry });
        }
        Biome::Desert => {
            let count = 6 + rng.below(5);
            for _ in 0..count {
                let r = 1 + rng.below(2) as i32;
                if let Some(at) = spot(rng, placer, 1, 4) {
                    clumps.push(Clump {
                        at,
                        r2: r * r,
                        kind: PartKind::Boulder,
                        chance: 0.75,
                    });
                }
            }
            // Cliffs: lines of rock across the open ground, each a run
            // of tiles in one of the eight directions.
            let count = 4 + rng.below(4);
            for _ in 0..count {
                let Some((mut x, mut y)) = spot(rng, placer, 1, 6) else {
                    continue;
                };
                let dir = [
                    (1, 0),
                    (1, 1),
                    (0, 1),
                    (-1, 1),
                    (-1, 0),
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                ][rng.below(8) as usize];
                let len = 7 + rng.below(10);
                for _ in 0..len {
                    lines.push((x, y));
                    x += dir.0;
                    y += dir.1;
                }
            }
            let (rx, ry) = (2 + rng.below(2) as i32, 1 + rng.below(2) as i32);
            lake = spot(rng, placer, rx + 3, 4).map(|at| Lake { at, rx, ry });
            if let Some(pool) = lake {
                for (dx, dy) in [
                    (1, 0),
                    (1, 1),
                    (0, 1),
                    (-1, 1),
                    (-1, 0),
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                ] {
                    if rng.chance(0.7) {
                        palms.push((
                            pool.at.0 + dx * (pool.rx + 1),
                            pool.at.1 + dy * (pool.ry + 1),
                        ));
                    }
                }
            }
        }
        Biome::Arctic => {
            let count = 8 + rng.below(5);
            for _ in 0..count {
                let r = 1 + rng.below(3) as i32;
                if let Some(at) = spot(rng, placer, 1, 4) {
                    clumps.push(Clump {
                        at,
                        r2: r * r,
                        kind: PartKind::Boulder,
                        chance: 0.7,
                    });
                }
            }
            let count = 4 + rng.below(5);
            for _ in 0..count {
                let r = 2 + rng.below(2) as i32;
                if let Some(at) = spot(rng, placer, 1, 4) {
                    clumps.push(Clump {
                        at,
                        r2: r * r,
                        kind: PartKind::Tree,
                        chance: 0.5,
                    });
                }
            }
            let (rx, ry) = (4 + rng.below(4) as i32, 3 + rng.below(3) as i32);
            lake = spot(rng, placer, rx.max(ry) + 2, 4).map(|at| Lake { at, rx, ry });
        }
    }

    // The scatter: one roll an eligible tile, in tile order.
    let mut chosen: Vec<Option<PartKind>> = vec![None; n];
    for y in 0..side {
        for x in 0..side {
            if !eligible(placer, x, y) {
                continue;
            }
            if lake.is_some_and(|l| l.holds(x, y)) {
                chosen[index(x, y)] = Some(PartKind::Water);
                continue;
            }
            if palms.contains(&(x, y)) {
                chosen[index(x, y)] = Some(PartKind::Tree);
                continue;
            }
            let (mut tree, shrub, mut boulder) = rates(biome, zone(x, y));
            if lines.contains(&(x, y)) {
                boulder = 0.9;
            }
            for clump in &clumps {
                let (dx, dy) = (x - clump.at.0, y - clump.at.1);
                if dx * dx + dy * dy <= clump.r2 {
                    match clump.kind {
                        PartKind::Tree => tree = tree.max(clump.chance),
                        _ => boulder = boulder.max(clump.chance),
                    }
                }
            }
            let roll = rng.unit();
            chosen[index(x, y)] = if roll < tree {
                Some(PartKind::Tree)
            } else if roll < tree + shrub {
                Some(PartKind::Shrub)
            } else if roll < tree + shrub + boulder {
                Some(PartKind::Boulder)
            } else {
                None
            };
        }
    }
    for y in 0..side {
        for x in 0..side {
            if let Some(kind) = chosen[index(x, y)] {
                placer.put(kind, (x as u32, y as u32), Rotation::R0);
            }
        }
    }

    // The flood from the pad, over everything nothing blocks.
    let start = shipdesign::port(&placer.design)
        .map(|port| {
            let t = TILE as f64;
            (
                ((port.centre.0 - port.outward.0 as f64 * t) / t) as i32,
                ((port.centre.1 - port.outward.1 as f64 * t) / t) as i32,
            )
        })
        .unwrap_or((FIRST as i32 + 1, (side / 2) - 1));
    let mut reached = vec![false; n];
    let mut stack = vec![start];
    reached[index(start.0, start.1)] = true;
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if !inside(nx, ny) || reached[index(nx, ny)] || placer.blocked((nx, ny)) {
                continue;
            }
            reached[index(nx, ny)] = true;
            stack.push((nx, ny));
        }
    }

    // And every free tile it did not reach filled with the nearest wild.
    let is_wild = |kind: PartKind| {
        matches!(
            kind,
            PartKind::Tree | PartKind::Shrub | PartKind::Boulder | PartKind::Water
        )
    };
    let (_, nearest) = distances(side, |x, y| {
        placer.object_at((x, y)).filter(|&kind| is_wild(kind))
    });
    let mut pockets = Vec::new();
    for y in 0..side {
        for x in 0..side {
            if reached[index(x, y)]
                || placer.get(shipdesign::Layer::Floor, (x, y)) == 0
                || placer.get(shipdesign::Layer::Object, (x, y)) != 0
            {
                continue;
            }
            pockets.push((x, y, nearest[index(x, y)].unwrap_or(ground_kind(biome))));
        }
    }
    for (x, y, kind) in pockets {
        placer.put(kind, (x as u32, y as u32), Rotation::R0);
    }
}

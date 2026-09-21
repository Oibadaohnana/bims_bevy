//! The ground beyond a settlement's deck: a planet's plain.
//!
//! A landed ship stands on a plane [`EXTENT`] tiles a side, and the town
//! is a patch of it — the deck the settlement is laid out on, in the
//! middle. Everything else is **terrain**: a function of the planet's
//! seed, the biome and the tile, worked out afresh wherever it is asked
//! for and never stored whole, since a hundred million tiles is neither
//! a `Vec` nor a save. What confines a Bim out there is what the ground
//! gives — a cliff, water, forest too dense to push through — and not
//! the edge of anything: the plane's own edge is a rim of cliff
//! [`RIM`] tiles deep, further off than anybody will walk.
//!
//! [`Terrain`] is the rule, in the settlement's own tile frame (the deck
//! is `0..side` both ways and the plane is centred on it), and every
//! roll in it is integer arithmetic on a hash, so a planet is the same
//! planet on every machine. [`Plane`] is the room's view of it: the same
//! rule read in the room's tiles through the frame the join put the
//! station in, a cache of the chunks looked at, and the fog — which
//! tiles the crew see now ([`Plane::observe`], traced tile by tile out
//! to [`VIEW`] the way `crate::sight` traces the deck) and which they
//! have ever seen. The room's dense structures — the navigation grids,
//! the sight mask, the filth — stay the deck's: the plane is walked on
//! a window a body carries with it (`nav::Nav::outside`, built about
//! it by `Game::refresh_afield`) and seen through this.

use crate::math::{Rect, Vec2, vec2};
use std::collections::BTreeMap;

/// The plane's side, in tiles. Ten thousand: a settlement's deck is
/// ninety-six, and the pad is a morning's walk from anything.
pub const EXTENT: i32 = 10_000;
/// How far the crew see over open ground, in tiles, and how far from
/// the camera's middle the ground is drawn: the world is loaded this far
/// and no further.
pub const VIEW: i32 = 60;
/// A chunk of the cache, in tiles a side.
pub const CHUNK: i32 = 32;
/// How far out from the deck the ground is kept clear, in tiles: open
/// ground all round the town and the pad, with a tree or a rock here
/// and there — on an even lattice, so no two ever touch and nothing
/// out there is ever walled off by the scatter.
pub const CLEARING: i32 = 48;
/// The cliff round the edge of the plane, in tiles.
pub const RIM: i32 = 48;
/// How far past the joined deck's box the room's own grids reach, in
/// tiles: the ground round the ship and the town's outskirts are walked
/// on the fine grid like the deck, and everything beyond on a window.
pub const DECK_MARGIN: i32 = 12;

/// The biomes, as `world::surface::Biome` numbers them.
pub const DESERT: u8 = 0;
pub const TEMPERATE: u8 = 1;
pub const ARCTIC: u8 = 2;

/// What a tile of the plane is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Ground {
    Open = 0,
    Tree = 1,
    Shrub = 2,
    Boulder = 3,
    Water = 4,
    Cliff = 5,
    Forest = 6,
}

impl Ground {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Ground {
        match code {
            1 => Ground::Tree,
            2 => Ground::Shrub,
            3 => Ground::Boulder,
            4 => Ground::Water,
            5 => Ground::Cliff,
            6 => Ground::Forest,
            _ => Ground::Open,
        }
    }

    /// Whether a body walks round it. Everything but open ground.
    pub fn blocks(self) -> bool {
        self != Ground::Open
    }

    /// Whether a line of sight stops at it: a cliff and a forest — the
    /// big things. A lone tree, a rock, a shrub and water are seen past;
    /// on open ground a single tree throwing a sixty-tile shadow was a
    /// picture of stripes, not of a tree.
    pub fn opaque(self) -> bool {
        matches!(self, Ground::Cliff | Ground::Forest)
    }
}

/// The rule: what the ground is at a tile of the settlement's frame.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Terrain {
    seed: u64,
    biome: u8,
    /// The deck's side, in tiles: `0..side` both ways is the town's.
    side: i32,
}

/// One tile's worth of thresholds, by biome. `e` is the elevation and
/// `m` the moisture, nought to 255 each; a feature is where the two fall
/// in its band, and the scatter between them is a chance in 256 a tile.
struct Rules {
    cliff_from: i32,
    water_below: i32,
    water_wet_from: i32,
    forest_wet_from: i32,
    forest_lo: i32,
    forest_hi: i32,
    /// The forest's fringe, this much moisture short of the forest: trees
    /// at this chance in 256.
    fringe: i32,
    fringe_tree: u32,
    tree: u32,
    shrub: u32,
    boulder: u32,
}

const TEMPERATE_RULES: Rules = Rules {
    cliff_from: 172,
    water_below: 85,
    water_wet_from: 150,
    forest_wet_from: 187,
    forest_lo: 85,
    forest_hi: 172,
    fringe: 14,
    fringe_tree: 120,
    tree: 18,
    shrub: 10,
    boulder: 3,
};

const DESERT_RULES: Rules = Rules {
    cliff_from: 176,
    water_below: 72,
    water_wet_from: 196,
    // No forest in a desert: a band nothing falls in.
    forest_wet_from: 256,
    forest_lo: 0,
    forest_hi: 0,
    fringe: 0,
    fringe_tree: 0,
    tree: 2,
    shrub: 9,
    boulder: 7,
};

const ARCTIC_RULES: Rules = Rules {
    cliff_from: 176,
    water_below: 88,
    water_wet_from: 140,
    forest_wet_from: 190,
    forest_lo: 88,
    forest_hi: 176,
    fringe: 16,
    fringe_tree: 110,
    tree: 12,
    shrub: 3,
    boulder: 8,
};

/// The lattice scatter of the clearing, a chance in 256 on the even
/// tiles.
const CLEARING_SCATTER: u32 = 10;
/// The scree before the rim: boulders at this chance in 256 over the
/// [`RIM`] tiles inside it.
const SCREE: u32 = 70;

/// A number off the seed and a lattice point, well mixed.
fn mix(seed: u64, x: i32, y: i32) -> u32 {
    let mut h = seed
        ^ (x as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 32;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 29;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    h as u32
}

/// Smooth value noise, nought to 255, off a lattice `scale` tiles apart:
/// the corner values blended by a smoothstep in fixed point. Integer
/// throughout, so two machines agree to the bit.
fn noise(seed: u64, x: i32, y: i32, scale: i32) -> i64 {
    const ONE: i64 = 1024;
    let (cx, cy) = (x.div_euclid(scale), y.div_euclid(scale));
    let (fx, fy) = (x.rem_euclid(scale), y.rem_euclid(scale));
    let corner = |dx: i32, dy: i32| (mix(seed, cx + dx, cy + dy) & 0xFF) as i64;
    let smooth = |t: i32| -> i64 {
        let t = t as i64 * ONE / scale as i64;
        t * t * (3 * ONE - 2 * t) / (ONE * ONE)
    };
    let (sx, sy) = (smooth(fx), smooth(fy));
    let top = corner(0, 0) * (ONE - sx) + corner(1, 0) * sx;
    let bottom = corner(0, 1) * (ONE - sx) + corner(1, 1) * sx;
    (top * (ONE - sy) + bottom * sy) / (ONE * ONE)
}

/// Three octaves of it — broad, middling, fine — nought to 255.
fn field(seed: u64, x: i32, y: i32) -> i32 {
    let v = noise(seed, x, y, 256) * 4
        + noise(seed ^ 0x51, x, y, 64) * 2
        + noise(seed ^ 0xA3, x, y, 16);
    (v / 7) as i32
}

impl Terrain {
    pub fn new(seed: u64, biome: u8, side: u32) -> Terrain {
        Terrain {
            seed,
            biome,
            side: side as i32,
        }
    }

    pub fn biome(&self) -> u8 {
        self.biome
    }

    pub fn side(&self) -> i32 {
        self.side
    }

    /// The plane's box in the settlement's tiles, exclusive at the top:
    /// [`EXTENT`] a side about the deck's middle.
    pub fn extent(&self) -> (i32, i32, i32, i32) {
        let mid = self.side / 2;
        (
            mid - EXTENT / 2,
            mid - EXTENT / 2,
            mid + EXTENT / 2,
            mid + EXTENT / 2,
        )
    }

    fn rules(&self) -> &'static Rules {
        match self.biome {
            DESERT => &DESERT_RULES,
            ARCTIC => &ARCTIC_RULES,
            _ => &TEMPERATE_RULES,
        }
    }

    /// What the ground is at a tile of the settlement's frame. Open on
    /// the deck itself — the town's parts are the design's — cliff past
    /// the plane's edge and round its rim, clear about the town, and the
    /// biome's rule everywhere else.
    pub fn at(&self, x: i32, y: i32) -> Ground {
        if x >= 0 && y >= 0 && x < self.side && y < self.side {
            return Ground::Open;
        }
        let (x0, y0, x1, y1) = self.extent();
        if x < x0 || y < y0 || x >= x1 || y >= y1 {
            return Ground::Cliff;
        }
        let to_edge = (x - x0).min(y - y0).min(x1 - 1 - x).min(y1 - 1 - y);
        if to_edge < RIM {
            return Ground::Cliff;
        }
        let roll = mix(self.seed ^ 0x7A11, x, y);
        let chance = |odds: u32| (roll & 0xFF) < odds;
        if to_edge < 2 * RIM {
            return if chance(SCREE) {
                Ground::Boulder
            } else {
                Ground::Open
            };
        }
        // The clearing: how far the tile is from the deck's box, each way.
        let dx = if x < 0 { -x } else { x - self.side + 1 };
        let dy = if y < 0 { -y } else { y - self.side + 1 };
        let to_deck = dx.max(dy).max(0);
        if to_deck < CLEARING {
            if x.rem_euclid(2) == 0 && y.rem_euclid(2) == 0 && chance(CLEARING_SCATTER) {
                return match self.biome {
                    DESERT => {
                        if roll & 0x100 != 0 {
                            Ground::Boulder
                        } else {
                            Ground::Shrub
                        }
                    }
                    _ => Ground::Tree,
                };
            }
            return Ground::Open;
        }
        let rules = self.rules();
        let e = field(self.seed ^ 0xE1E7, x, y);
        let m = field(self.seed ^ 0x3013, x, y);
        if e >= rules.cliff_from {
            return Ground::Cliff;
        }
        if e < rules.water_below && m >= rules.water_wet_from {
            return Ground::Water;
        }
        if e >= rules.forest_lo && e < rules.forest_hi {
            if m >= rules.forest_wet_from {
                return Ground::Forest;
            }
            if m >= rules.forest_wet_from - rules.fringe && chance(rules.fringe_tree) {
                return Ground::Tree;
            }
        }
        // The scatter, one roll: the bands laid end to end.
        let r = roll & 0xFF;
        if r < rules.tree {
            Ground::Tree
        } else if r < rules.tree + rules.shrub {
            Ground::Shrub
        } else if r < rules.tree + rules.shrub + rules.boulder {
            Ground::Boulder
        } else {
            Ground::Open
        }
    }
}

/// A chunk of tiles, one code each.
type Chunk = Box<[u8]>;
/// A chunk's worth of flags.
type Bits = [u64; (CHUNK * CHUNK / 64) as usize];
const NO_BITS: Bits = [0; (CHUNK * CHUNK / 64) as usize];

fn chunk_of(x: i32, y: i32) -> ((i32, i32), usize) {
    (
        (x.div_euclid(CHUNK), y.div_euclid(CHUNK)),
        (y.rem_euclid(CHUNK) * CHUNK + x.rem_euclid(CHUNK)) as usize,
    )
}

fn bit(bits: &Bits, i: usize) -> bool {
    bits[i / 64] & (1 << (i % 64)) != 0
}

fn set_bit(bits: &mut Bits, i: usize) {
    bits[i / 64] |= 1 << (i % 64);
}

/// The plane as the room holds it. See the module note.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plane {
    terrain: Terrain,
    /// The station's frame in the room's tiles: where its tile (0, 0)
    /// sits, and its two axes as unit steps. A room tile `(rx, ry)` is
    /// the station's `((rx, ry) - origin) . ex, . ey`.
    origin: (i32, i32),
    ex: (i32, i32),
    ey: (i32, i32),
    /// The deck's box in the room's tiles — the room's interior, which
    /// the sight mask covers — exclusive at the top. Nothing in it is the
    /// plane's to see.
    deck: (i32, i32, i32, i32),
    /// The tiles looked at so far, by the station's chunk. Not saved: a
    /// function of the seed.
    #[cfg_attr(feature = "serde", serde(skip))]
    chunks: BTreeMap<(i32, i32), Chunk>,
    /// Every tile a crew member has ever seen, by the station's chunk.
    explored: BTreeMap<(i32, i32), Bits>,
    /// What they see now, traced from where they stand.
    #[cfg_attr(feature = "serde", serde(skip))]
    seen: BTreeMap<(i32, i32), Bits>,
    #[cfg_attr(feature = "serde", serde(skip))]
    eyes_at: Vec<(i32, i32)>,
    #[cfg_attr(feature = "serde", serde(skip))]
    traced: bool,
}

/// What the picture draws over a tile of the plane: nothing, the grey
/// of ground looked at and not in view, or the black of ground never
/// seen.
pub const VEIL_NONE: u8 = 0;
pub const VEIL_GREY: u8 = 1;
pub const VEIL_BLACK: u8 = 2;

impl Plane {
    /// `origin`, `ex` and `ey` are the station's frame in the room's
    /// tiles; `deck` the room's interior in tiles, exclusive at the top.
    pub fn new(
        terrain: Terrain,
        origin: (i32, i32),
        ex: (i32, i32),
        ey: (i32, i32),
        deck: (i32, i32, i32, i32),
    ) -> Plane {
        Plane {
            terrain,
            origin,
            ex,
            ey,
            deck,
            chunks: BTreeMap::new(),
            explored: BTreeMap::new(),
            seen: BTreeMap::new(),
            eyes_at: Vec::new(),
            traced: false,
        }
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    pub fn biome(&self) -> u8 {
        self.terrain.biome
    }

    /// The deck's box, in the room's tiles.
    pub fn deck(&self) -> (i32, i32, i32, i32) {
        self.deck
    }

    /// The deck's box again, when the room's box is settled: what
    /// `aboard::layout_of_on` grows it to.
    pub fn set_deck(&mut self, deck: (i32, i32, i32, i32)) {
        self.deck = deck;
    }

    pub fn on_deck(&self, rx: i32, ry: i32) -> bool {
        let (x0, y0, x1, y1) = self.deck;
        rx >= x0 && ry >= y0 && rx < x1 && ry < y1
    }

    /// A room tile as the station's.
    pub fn to_station(&self, rx: i32, ry: i32) -> (i32, i32) {
        let (dx, dy) = (rx - self.origin.0, ry - self.origin.1);
        (
            dx * self.ex.0 + dy * self.ex.1,
            dx * self.ey.0 + dy * self.ey.1,
        )
    }

    /// A station tile as the room's.
    pub fn from_station(&self, sx: i32, sy: i32) -> (i32, i32) {
        (
            self.origin.0 + sx * self.ex.0 + sy * self.ey.0,
            self.origin.1 + sx * self.ex.1 + sy * self.ey.1,
        )
    }

    /// The tile a room point is in.
    pub fn tile_of(p: Vec2, tile: f32) -> (i32, i32) {
        ((p.x / tile).floor() as i32, (p.y / tile).floor() as i32)
    }

    /// The ground at a room tile, off the cache or worked out.
    pub fn at_room(&self, rx: i32, ry: i32) -> Ground {
        let (sx, sy) = self.to_station(rx, ry);
        self.at_station(sx, sy)
    }

    pub fn at_station(&self, sx: i32, sy: i32) -> Ground {
        let (key, i) = chunk_of(sx, sy);
        match self.chunks.get(&key) {
            Some(chunk) => Ground::from_code(chunk[i]),
            None => self.terrain.at(sx, sy),
        }
    }

    /// Work out and keep every chunk a room box touches, inclusive.
    pub fn load(&mut self, rx0: i32, ry0: i32, rx1: i32, ry1: i32) {
        let corners = [
            self.to_station(rx0, ry0),
            self.to_station(rx1, ry0),
            self.to_station(rx0, ry1),
            self.to_station(rx1, ry1),
        ];
        let sx0 = corners.iter().map(|c| c.0).min().unwrap_or(0);
        let sy0 = corners.iter().map(|c| c.1).min().unwrap_or(0);
        let sx1 = corners.iter().map(|c| c.0).max().unwrap_or(0);
        let sy1 = corners.iter().map(|c| c.1).max().unwrap_or(0);
        for cy in sy0.div_euclid(CHUNK)..=sy1.div_euclid(CHUNK) {
            for cx in sx0.div_euclid(CHUNK)..=sx1.div_euclid(CHUNK) {
                if self.chunks.contains_key(&(cx, cy)) {
                    continue;
                }
                let mut chunk = vec![0u8; (CHUNK * CHUNK) as usize];
                for y in 0..CHUNK {
                    for x in 0..CHUNK {
                        chunk[(y * CHUNK + x) as usize] =
                            self.terrain.at(cx * CHUNK + x, cy * CHUNK + y).code();
                    }
                }
                self.chunks.insert((cx, cy), chunk.into_boxed_slice());
            }
        }
    }

    /// Drop every cached chunk further than `keep` tiles from all of
    /// `about`, in room tiles: what nobody is near any more.
    pub fn trim(&mut self, about: &[(i32, i32)], keep: i32) {
        let near: Vec<(i32, i32)> = about.iter().map(|&(x, y)| self.to_station(x, y)).collect();
        self.chunks.retain(|&(cx, cy), _| {
            let (mx, my) = (cx * CHUNK + CHUNK / 2, cy * CHUNK + CHUNK / 2);
            near.iter()
                .any(|&(sx, sy)| (sx - mx).abs() <= keep && (sy - my).abs() <= keep)
        });
    }

    /// The blocking tiles of a room box, inclusive, as rects in room
    /// units — a run of them along a row is one rect — for a grid and a
    /// body's push-out. Tiles `skip` says yes to are left out: the deck's
    /// own, where what stands is the layout's.
    pub fn solids_in(
        &self,
        rx0: i32,
        ry0: i32,
        rx1: i32,
        ry1: i32,
        tile: f32,
        skip: &dyn Fn(i32, i32) -> bool,
    ) -> Vec<Rect> {
        self.runs_in(rx0, ry0, rx1, ry1, tile, skip, |g| g.blocks())
    }

    /// The same for what stops a line of sight.
    pub fn opaque_in(
        &self,
        rx0: i32,
        ry0: i32,
        rx1: i32,
        ry1: i32,
        tile: f32,
        skip: &dyn Fn(i32, i32) -> bool,
    ) -> Vec<Rect> {
        self.runs_in(rx0, ry0, rx1, ry1, tile, skip, |g| g.opaque())
    }

    fn runs_in(
        &self,
        rx0: i32,
        ry0: i32,
        rx1: i32,
        ry1: i32,
        tile: f32,
        skip: &dyn Fn(i32, i32) -> bool,
        pick: impl Fn(Ground) -> bool,
    ) -> Vec<Rect> {
        let mut out = Vec::new();
        for ry in ry0..=ry1 {
            let mut run: Option<i32> = None;
            for rx in rx0..=rx1 + 1 {
                let on = rx <= rx1 && !skip(rx, ry) && pick(self.at_room(rx, ry));
                match (run, on) {
                    (None, true) => run = Some(rx),
                    (Some(from), false) => {
                        out.push(Rect::from_corners(
                            vec2(from as f32 * tile, ry as f32 * tile),
                            vec2(rx as f32 * tile, (ry + 1) as f32 * tile),
                        ));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// Trace what the crew see of the plane from these eyes, in room
    /// units, if any of them has crossed a tile since the last trace:
    /// every tile off the deck within [`VIEW`] of an eye whose line from
    /// the eye's tile crosses nothing opaque — the plane's own, and on
    /// the deck whatever `blocked` says, which is the sight mask's cells.
    /// What is seen stays explored. True when traced again.
    pub fn observe(
        &mut self,
        eyes: &[Vec2],
        tile: f32,
        blocked: &dyn Fn(i32, i32) -> bool,
    ) -> bool {
        let eyes_at: Vec<(i32, i32)> = eyes.iter().map(|&p| Plane::tile_of(p, tile)).collect();
        if self.traced && eyes_at == self.eyes_at {
            return false;
        }
        self.eyes_at = eyes_at.clone();
        self.traced = true;
        self.seen.clear();
        let side = 2 * VIEW + 1;
        for &(ex, ey) in &eyes_at {
            self.load(ex - VIEW, ey - VIEW, ex + VIEW, ey + VIEW);
            // The window round the eye, opaque or not, once a tile.
            let mut opaque = vec![false; (side * side) as usize];
            for dy in 0..side {
                for dx in 0..side {
                    let (x, y) = (ex - VIEW + dx, ey - VIEW + dy);
                    opaque[(dy * side + dx) as usize] = if self.on_deck(x, y) {
                        blocked(x, y)
                    } else {
                        self.at_room(x, y).opaque()
                    };
                }
            }
            let at = |x: i32, y: i32| ((y - ey + VIEW) * side + (x - ex + VIEW)) as usize;
            let mut clear = vec![false; (side * side) as usize];
            for y in ey - VIEW..=ey + VIEW {
                for x in ex - VIEW..=ex + VIEW {
                    let (dx, dy) = (x - ex, y - ey);
                    if dx * dx + dy * dy > VIEW * VIEW {
                        continue;
                    }
                    clear[at(x, y)] = line_clear(&opaque, &at, (ex, ey), (x, y));
                }
            }
            // A tile beside a seen one is seen: the trace is a line to a
            // tile's middle, and its edges come out pinholed and stepped a
            // tile at a time otherwise.
            for y in ey - VIEW..=ey + VIEW {
                for x in ex - VIEW..=ex + VIEW {
                    if self.on_deck(x, y) {
                        continue;
                    }
                    let seen = clear[at(x, y)]
                        || (x > ex - VIEW
                            && y > ey - VIEW
                            && x < ex + VIEW
                            && y < ey + VIEW
                            && [
                                (-1, 0),
                                (1, 0),
                                (0, -1),
                                (0, 1),
                                (-1, -1),
                                (1, -1),
                                (-1, 1),
                                (1, 1),
                            ]
                            .iter()
                            .any(|&(dx, dy)| clear[at(x + dx, y + dy)]));
                    if !seen {
                        continue;
                    }
                    let (sx, sy) = self.to_station(x, y);
                    let (key, i) = chunk_of(sx, sy);
                    set_bit(self.seen.entry(key).or_insert(NO_BITS), i);
                    set_bit(self.explored.entry(key).or_insert(NO_BITS), i);
                }
            }
        }
        true
    }

    /// What the picture draws over a room tile of the plane.
    pub fn veil_at_room(&self, rx: i32, ry: i32) -> u8 {
        let (sx, sy) = self.to_station(rx, ry);
        let (key, i) = chunk_of(sx, sy);
        if self.seen.get(&key).is_some_and(|b| bit(b, i)) {
            VEIL_NONE
        } else if self.explored.get(&key).is_some_and(|b| bit(b, i)) {
            VEIL_GREY
        } else {
            VEIL_BLACK
        }
    }

    /// Whether anybody sees the tile a room point is in, off the deck.
    pub fn seen_at(&self, p: Vec2, tile: f32) -> bool {
        let (rx, ry) = Plane::tile_of(p, tile);
        self.veil_at_room(rx, ry) == VEIL_NONE
    }

    /// How many tiles have ever been seen, for the probes.
    pub fn explored_count(&self) -> usize {
        self.explored
            .values()
            .map(|b| b.iter().map(|w| w.count_ones() as usize).sum::<usize>())
            .sum()
    }
}

/// Whether the line between two tiles' middles crosses no opaque tile
/// before the second: the same traversal as `Sight::clear_line`, on a
/// window of flags — the tile the line stops at is seen, so a cliff is
/// seen from the ground before it.
fn line_clear(
    opaque: &[bool],
    at: &dyn Fn(i32, i32) -> usize,
    from: (i32, i32),
    to: (i32, i32),
) -> bool {
    let (mut x, mut y) = from;
    if (x, y) == to {
        return true;
    }
    let (dx, dy) = (to.0 - x, to.1 - y);
    let (sx, sy) = (dx.signum(), dy.signum());
    let (ax, ay) = (dx.abs(), dy.abs());
    // Bresenham on the longer axis, the error in the other's units.
    let mut err = ax - ay;
    let most = ax + ay + 1;
    for _ in 0..most {
        let e2 = 2 * err;
        if e2 > -ay {
            err -= ay;
            x += sx;
        }
        if e2 < ax {
            err += ax;
            y += sy;
        }
        if (x, y) == to {
            return true;
        }
        if opaque[at(x, y)] {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_biome() -> [Terrain; 3] {
        [
            Terrain::new(0x5EED, DESERT, 96),
            Terrain::new(0x5EED, TEMPERATE, 96),
            Terrain::new(0x5EED, ARCTIC, 96),
        ]
    }

    #[test]
    fn the_deck_is_open_the_clearing_walkable_and_the_rim_cliff() {
        for t in every_biome() {
            for y in 0..96 {
                assert_eq!(t.at(0, y), Ground::Open);
                assert_eq!(t.at(95, y), Ground::Open);
            }
            // The clearing: never two blocked tiles touching, and open at
            // its every odd tile.
            for y in -CLEARING + 1..96 + CLEARING - 1 {
                for x in -CLEARING + 1..0 {
                    let g = t.at(x, y);
                    if x.rem_euclid(2) != 0 || y.rem_euclid(2) != 0 {
                        assert_eq!(g, Ground::Open, "{x},{y}");
                    }
                    if g.blocks() {
                        for (dx, dy) in [(1, 0), (0, 1), (1, 1), (-1, 1)] {
                            assert!(!t.at(x + dx, y + dy).blocks(), "{x},{y} touches {dx},{dy}");
                        }
                    }
                }
            }
            let (x0, y0, x1, y1) = t.extent();
            assert_eq!(x1 - x0, EXTENT);
            assert_eq!(t.at(x0, y0), Ground::Cliff);
            assert_eq!(t.at(x1 - 1, y1 - 1), Ground::Cliff);
            assert_eq!(t.at(x0 + RIM - 1, 48), Ground::Cliff);
            assert_eq!(t.at(x0 - 1, 48), Ground::Cliff);
        }
    }

    #[test]
    fn the_same_seed_is_the_same_ground_and_another_is_not() {
        let a = Terrain::new(7, TEMPERATE, 96);
        let b = Terrain::new(7, TEMPERATE, 96);
        let c = Terrain::new(8, TEMPERATE, 96);
        let mut differ = 0;
        for y in -400..400 {
            for x in 200..600 {
                assert_eq!(a.at(x, y), b.at(x, y));
                if a.at(x, y) != c.at(x, y) {
                    differ += 1;
                }
            }
        }
        assert!(differ > 1000, "{differ}");
    }

    #[test]
    fn the_plain_is_wide_open_and_has_every_feature() {
        // A flood from the deck over a thousand-tile square, four ways:
        // most of it reachable, and every kind of ground met on the way.
        for t in every_biome() {
            let (x0, y0, x1, y1) = (-500, -500, 600, 600);
            let w = (x1 - x0) as usize;
            let h = (y1 - y0) as usize;
            let mut reached = vec![false; w * h];
            let mut stack = vec![(48, 48)];
            let mut open = 0usize;
            let mut kinds = [0usize; 7];
            for y in y0..y1 {
                for x in x0..x1 {
                    let g = t.at(x, y);
                    kinds[g.code() as usize] += 1;
                    if !g.blocks() {
                        open += 1;
                    }
                }
            }
            reached[(48 - y0) as usize * w + (48 - x0) as usize] = true;
            let mut count = 0usize;
            while let Some((x, y)) = stack.pop() {
                count += 1;
                for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    if nx < x0 || ny < y0 || nx >= x1 || ny >= y1 {
                        continue;
                    }
                    let i = (ny - y0) as usize * w + (nx - x0) as usize;
                    if reached[i] || t.at(nx, ny).blocks() {
                        continue;
                    }
                    reached[i] = true;
                    stack.push((nx, ny));
                }
            }
            assert!(
                count * 10 > open * 8,
                "biome {}: {count} of {open} open tiles reached",
                t.biome
            );
            assert!(
                open * 10 > w * h * 5,
                "biome {}: {open} open of {}",
                t.biome,
                w * h
            );
            assert!(
                kinds[Ground::Cliff.code() as usize] > 0,
                "biome {} has cliffs",
                t.biome
            );
            assert!(
                kinds[Ground::Water.code() as usize] > 0,
                "biome {} has water",
                t.biome
            );
            if t.biome != DESERT {
                assert!(
                    kinds[Ground::Forest.code() as usize] > 0,
                    "biome {} has forest",
                    t.biome
                );
            }
        }
    }

    #[test]
    fn the_plane_reads_the_station_through_its_frame_and_sees_to_the_view() {
        let t = Terrain::new(3, TEMPERATE, 96);
        // The station turned a quarter and shifted: room (10, 20) is the
        // station's (0, 0), the station's +x down the room's +y.
        let mut plane = Plane::new(t.clone(), (10, 20), (0, 1), (-1, 0), (0, 0, 20, 30));
        assert_eq!(plane.to_station(10, 20), (0, 0));
        assert_eq!(plane.to_station(10, 25), (5, 0));
        assert_eq!(plane.to_station(7, 20), (0, 3));
        assert_eq!(plane.from_station(5, 0), (10, 25));
        for rx in -50..50 {
            for ry in -50..50 {
                let (sx, sy) = plane.to_station(rx, ry);
                assert_eq!(plane.from_station(sx, sy), (rx, ry));
                assert_eq!(plane.at_room(rx, ry), t.at(sx, sy));
            }
        }
        plane.load(-100, -100, 100, 100);
        for rx in -100..100 {
            for ry in -100..100 {
                let (sx, sy) = plane.to_station(rx, ry);
                assert_eq!(plane.at_room(rx, ry), t.at(sx, sy));
            }
        }
        // An eye at a room tile off the deck sees open ground to the
        // view's edge and nothing past it.
        let tile = 52.0;
        let eye = vec2(30.5 * tile, 68.5 * tile);
        assert!(plane.observe(&[eye], tile, &|_, _| true));
        assert!(!plane.observe(&[eye], tile, &|_, _| true));
        assert!(plane.seen_at(eye, tile));
        assert_eq!(plane.veil_at_room(30, 68 + VIEW + 1), VEIL_BLACK);
        assert!(plane.explored_count() > 1000);
        let seen_now = plane.explored_count();
        // Moved off: what was seen is grey, and more is known.
        let eye2 = vec2(30.5 * tile, (68 + VIEW) as f32 * tile + 0.5 * tile);
        assert!(plane.observe(&[eye2], tile, &|_, _| true));
        let grey = (9..=60)
            .filter(|&y| plane.veil_at_room(30, y) == VEIL_GREY)
            .count();
        assert!(grey > 20, "{grey} grey");
        assert!(plane.explored_count() > seen_now);
    }
}

#[cfg(test)]
mod calibrate {
    use super::*;
    #[test]
    #[ignore]
    fn histogram() {
        let mut e = [0usize; 256];
        let mut m = [0usize; 256];
        let n = 1000 * 1000;
        for y in 500..1500 {
            for x in 500..1500 {
                e[field(0xE1E7, x, y) as usize] += 1;
                m[field(0x3013, x, y) as usize] += 1;
            }
        }
        let pct = |h: &[usize; 256], p: f64| {
            let mut acc = 0usize;
            for (v, &c) in h.iter().enumerate() {
                acc += c;
                if acc as f64 >= p * n as f64 {
                    return v;
                }
            }
            255
        };
        for p in [0.01, 0.05, 0.1, 0.2, 0.5, 0.8, 0.9, 0.95, 0.99] {
            println!("p{p}: e {} m {}", pct(&e, p), pct(&m, p));
        }
    }
}

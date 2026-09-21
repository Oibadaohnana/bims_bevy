//! What the crew can see.
//!
//! Every Bim sees all the way round — there is no cone — and what stops
//! its eyes is what stands in the way: the walls, the hull, the tall parts,
//! a door with its leaves shut. Sight is worked out on the room's tile
//! grid and it is **traced**: a tile is seen when the straight line from a
//! crew member's eyes to the tile's middle crosses nothing opaque on the
//! way, and the tile it stops at is seen too, so a wall is seen from the
//! room it walls. The crew share it — a tile one of them sees, all of
//! them see — because the crew are one crew and a screen is one screen.
//!
//! The mask is a picture and a fact at once: [`Game::render`] draws a fog
//! over every tile nobody sees, and the world asks [`Sight::seen_at`] for
//! whether a body on the station's deck is in anybody's view. It is
//! recomputed only when something that could change it has — an eye
//! crossed a tile, a door shut or opened — since a trace of every tile
//! from every eye is a few hundred thousand steps in a joined room.
//!
//! # Peeking round a wall
//!
//! A Bim standing **against** a wall — the tile beside it opaque — leans
//! out and looks along it: as well as from where it stands, it sees from
//! the free tile either side of it along the wall, and what those extra
//! eyes add is whatever lies **beyond the wall's line**. So a Bim pressed
//! to the corner of a room sees the whole of the room on the other side
//! of the corner, where one standing a tile back sees only the wedge the
//! corner leaves. [`Sight::eyes_from`] is the rule, and [`Sight::sees_from`]
//! asks it for one body and one target — a shot is fired from whichever
//! eye saw the enemy, so a peeking Bim shoots from the peek.
//!
//! # Whose the tiles are
//!
//! Every tile has a [`Stance`]: the crew's own ship is friendly, and a
//! station's tiles are whatever the world says the station is. What the
//! fog looks like over an unseen tile follows from that. A friendly
//! structure is under a **semi** fog — the deck stays readable, since the
//! crew know their own ship — while a neutral or hostile one is **black**
//! where no line of sight has ever reached, and **grey** where one has
//! and none does now: the structure shows there and no body does. The
//! grey is what has been looked at, and only that — a wall is seen from
//! the room it walls, the way the trace works, so a room's outline comes
//! in as its inside is looked at and nothing behind a bulkhead shows
//! until somebody has seen past it. The grey stays once earned: what has
//! been looked at is known, and only who is standing there is forgotten.

use crate::draw::{Color, DrawList};
use crate::math::{Rect, Vec2, vec2};

/// The fog over what nobody sees of a friendly structure: the deck under
/// it stays readable, since the ship is the crew's own and they know where
/// the walls are, but whatever is standing there is not drawn.
const FOG: Color = Color::rgba(0.02, 0.04, 0.03, 0.62);
/// What has been looked at of somebody else's structure and is not in
/// view now: the walls and the fixtures show through, and nobody standing
/// among them does.
const FOG_GREY: Color = Color::rgba(0.06, 0.07, 0.08, 0.80);
/// The rest of somebody else's structure: nothing.
const FOG_BLACK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);

/// How far a Bim sees in the dark, in tiles: a tile a light does not reach
/// is seen only from this close. Lit tiles are seen as far as the line is
/// clear. See [`Light`].
pub const DARK_RANGE: f32 = 10.0;

/// A light in the room: where it is and how far it reaches, in room units.
/// A tile is **lit** when the straight line from some light to its middle
/// crosses nothing opaque (the walls and the tall parts — never a door,
/// which is not there to a light the way it is to an eye) and is within the
/// light's reach. Lights are always on until they are shot out — see
/// [`Lamp`]. A room never handed any lights is lit throughout — the
/// classic room, which has no lighting to speak of — and a designed deck
/// handed none is dark everywhere.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Light {
    pub at: Vec2,
    pub reach: f32,
}

/// What a lamp takes before it goes out: two pistol bolts leave it
/// failing, a third puts it out, and a shotgun's or a sniper's one does
/// on its own.
pub const LAMP_HEALTH: f32 = 16.0;
/// Below this share of its health a lamp is **failing**: it flickers now
/// and then, on its own.
pub const LAMP_FAILING: f32 = 0.2;
/// How near a bolt has to pass a lamp to hit it, in room units: the
/// glass, near enough. A bolt aimed down the middle of a gangway misses
/// the lamps on its walls; one that goes wide may not.
pub const LAMP_RADIUS: f32 = 10.0;
/// How long a lamp flickers for after a hit, and for one of its failing
/// flickers, in seconds at 1x.
const LAMP_HIT_FLICKER: f32 = 0.6;
const LAMP_FAIL_FLICKER: f32 = 0.35;
/// How many times a second a flickering lamp picks a new brightness, and
/// how often a failing one starts a flicker: the odds per second.
const FLICKER_RATE: f32 = 24.0;
const FAIL_FLICKER_ODDS: f32 = 0.3;

/// One lamp of the room, as the fight sees it: where it is, what it has
/// left, and how bright it is shown. A bolt that passes within
/// [`LAMP_RADIUS`] of it stops there and takes its damage off the lamp
/// ([`Sight::damage_lamp`]); at nought it is **out** — its light gone
/// from the tile mask and the picture alike, so the deck round it goes
/// dark and a Bim sees only its ten tiles there. A hit sets it
/// flickering for a moment, and one below [`LAMP_FAILING`] flickers now
/// and then for good. The flicker is the **picture's** alone: the tile
/// mask and the eyes read the lamp as on until it is out, so nothing
/// the fight decides depends on a frame's brightness, and it is worked
/// out from the clock and the lamp's index rather than rolled, so it
/// draws nothing off any stream.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lamp {
    pub at: Vec2,
    /// What it has left of [`LAMP_HEALTH`]; nought or under is out.
    pub health: f32,
    /// Whether it has power: off, it is as dark as one shot out — its
    /// light off the tile mask and the picture — but whole, and comes
    /// back the moment the power does. The world's to set
    /// (`Sight::set_lamp_powered`): a lamp on no live network, or any
    /// lamp in a brownout. On to start with, since a room never told is
    /// the classic room, which has no reactor to lose.
    pub powered: bool,
    /// How bright it is shown, nought to one: one steady, nought dark,
    /// between while it flickers.
    pub level: f32,
    /// Seconds of flicker left.
    flicker: f32,
    /// The last second-long window a failing flicker was rolled for.
    window: u32,
}

impl Lamp {
    /// Whether it is broken: shot to nothing. Never true of a lamp that
    /// is merely unpowered.
    pub fn is_out(&self) -> bool {
        self.health <= 0.0
    }

    /// Whether it gives no light: out, or unpowered. What the tile mask,
    /// the field and the picture read.
    pub fn is_dark(&self) -> bool {
        self.is_out() || !self.powered
    }

    pub fn is_failing(&self) -> bool {
        !self.is_out() && self.health <= LAMP_HEALTH * LAMP_FAILING
    }
}

/// A pseudo-random unit number from a lamp's index and a slot of time:
/// the flicker's dice, which are the same every time for the same lamp
/// and the same instant. SplitMix64's mixer, which is plenty for a
/// picture.
fn noise(lamp: usize, slot: u32) -> f32 {
    let mut z = ((lamp as u64) << 32 ^ slot as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// How close behind low cover a body has to stand to be covered by it,
/// in tiles from the body to the sandbags' middle: the tile beside it,
/// diagonals included, and no further.
pub const COVER_REACH: f32 = 1.5;

/// How far an opaque fog rectangle reaches past its tiles on each side, in
/// room units, so that two meeting edge to edge show no seam.
const OVERLAP: f32 = 2.0;

/// Whose a structure is, to the crew. What the fog over it looks like.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Stance {
    /// The crew's own ship, and anywhere else they are welcome.
    #[default]
    Friendly = 0,
    Neutral = 1,
    /// Whoever lives there is an enemy.
    Hostile = 2,
}

impl Stance {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// One tile, as the grid sees it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Cell {
    /// A line of sight stops here: a wall, the hull, a tall part, a shut
    /// door, or the outside.
    opaque: bool,
    /// The fog is drawn here when it is not seen: a tile of the hull, or
    /// anywhere inside the classic room's box.
    fogged: bool,
    /// Somebody else's: a station's tile on a joined deck, under the
    /// foreign stance rather than the room's own.
    foreign: bool,
    /// Low cover: sandbags, seen and walked over, that a body close
    /// behind ducks under — see `Sight::covered`.
    cover: bool,
    /// Opaque, but furniture rather than wall: a shelf, a cabinet, a
    /// table. A line of sight stops here like at a wall; the light
    /// picture shades behind it softly — see `Sight::set_tall`.
    soft: bool,
}

/// One place a body looks from: where, and — for a peek — what it may
/// add, as the direction of the wall it is peeking past from the body's
/// own tile.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Eye {
    pub at: Vec2,
    /// The body's own tile and the wall's direction from it, for a peek;
    /// `None` for the body's own eyes, which add everything they see.
    beyond: Option<((i32, i32), (i32, i32))>,
}

impl Eye {
    /// Whether this is a peek beside a wall rather than the body's own
    /// eyes. What the enemy's tactics ask, to tell cover from the open.
    pub fn is_peek(&self) -> bool {
        self.beyond.is_some()
    }

    /// Whether this eye may add the tile: any, for the body's own; past
    /// the wall's line, for a peek.
    pub fn admits(&self, tile: (i32, i32)) -> bool {
        match self.beyond {
            None => true,
            Some(((bx, by), (dx, dy))) => (tile.0 - bx) * dx + (tile.1 - by) * dy >= 1,
        }
    }
}

/// How a fogged tile is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Veil {
    Semi,
    Grey,
    Black,
}

/// What everybody aboard can see, put together. See the module note.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sight {
    /// The grid's corner and its tile, in room units.
    origin: Vec2,
    tile: f32,
    columns: i32,
    rows: i32,
    /// What is always in the way: the walls and the tall parts.
    fixed: Vec<Cell>,
    /// The same with the shut doors added, for the trace to read.
    cells: Vec<Cell>,
    /// Whether anybody sees each tile.
    seen: Vec<bool>,
    /// Whether a light reaches each tile — see [`Light`] — and the lights
    /// themselves, for the picture. With no lights, every tile is lit.
    lit: Vec<bool>,
    lights: Vec<Light>,
    /// The lamps, one a light: what each has left and how bright it is
    /// shown. See [`Lamp`].
    lamps: Vec<Lamp>,
    /// Which lamps' health changed since a host last asked
    /// (`take_lamp_changes`).
    lamp_changes: Vec<usize>,
    /// The lamps' own clock, for the flicker's dice.
    lamp_seconds: f32,
    /// Never handed lights at all: lit throughout. See [`Light`].
    lit_everywhere: bool,
    /// The sky: every tile whose middle lies in here is lit whatever the
    /// lamps say, on the mask and in the picture. The world's word, for a
    /// settlement's ground (`set_daylight`); `None` aboard and on a
    /// station, where every light is a lamp.
    daylight: Option<Rect>,
    /// How far an eye sees at all, in room units, lit or not: `None`
    /// aboard and on a station, where a lit tile is seen as far as the
    /// line is clear; a plain's sight range on a planet (`set_range`),
    /// for the mask and the picture alike.
    range: Option<f32>,
    /// The smooth picture — see [`LightMap`] — and the light field it is
    /// built on, worked out once per layout. Left out of a save with the
    /// fields, the lamps' boxes and the views under it: all of them are
    /// pictures of what is saved, and an empty light field is what makes
    /// `light_map` draw the lot again.
    #[cfg_attr(feature = "serde", serde(skip))]
    map: LightMap,
    /// Whether the picture wants composing again though no eye moved:
    /// a stance changed, or the layout.
    map_stale: bool,
    /// The light over the deck as the eyes read it — every lamp that is
    /// not out at full — and as it is shown, with the flicker in. The
    /// sum of the lamps' own fields, each cached over its reach so a
    /// lamp going out or flickering is a box summed again and not every
    /// lamp marched again. See [`Sight::light_field`].
    #[cfg_attr(feature = "serde", serde(skip))]
    light_field: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(skip))]
    shown_field: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(skip))]
    lamp_fields: Vec<LampField>,
    light_field_stale: bool,
    /// Where the shown field changed since the picture was composed: a
    /// lamp that flickered or went out.
    #[cfg_attr(feature = "serde", serde(skip))]
    field_dirty: Option<Box>,
    /// What each body's eyes reach, pixel by pixel, as last marched —
    /// see [`Sight::light_map`] — so a body that has not moved is not
    /// marched again. `views_stale` throws them all away: the cells the
    /// rays stop at changed.
    #[cfg_attr(feature = "serde", serde(skip))]
    views: Vec<View>,
    views_stale: bool,
    /// Every pixel a line of sight has ever reached: the picture's grey
    /// over a stranger's structure. A flag a pixel, so it goes into a
    /// save as a string of noughts and ones (`math::bools`).
    #[cfg_attr(feature = "serde", serde(with = "crate::math::bools"))]
    explored_px: Vec<bool>,
    /// Whether each tile was ever seen: what has been looked at stays
    /// known — the grey is the fog of war's "explored", and only the
    /// bodies in it are forgotten.
    explored: Vec<bool>,
    /// What the mask was last worked out from: the eyes' tiles and the
    /// shut doors. When these have not moved, neither has the mask.
    eyes_at: Vec<(i32, i32)>,
    /// The shut doors `cells` carries now — see `set_shut`.
    shut: Vec<Rect>,
    /// Whether the doors have moved since the mask was traced.
    stale: bool,
    /// Whether anything has been traced yet. Until it has, nothing is
    /// seen, which is right for a room nobody is looking into.
    traced: bool,
    /// Whose the room's own tiles are, and whose the foreign ones.
    own: Stance,
    foreign: Stance,
}

impl Sight {
    /// A grid over `bounds` at `tile`, where a line of sight stops at
    /// every one of `opaque` and outside `interior`, and the fog is drawn
    /// over `fogged` — or, with none given, over the whole of `bounds`.
    pub fn new(bounds: Rect, interior: Rect, tile: f32, opaque: &[Rect], fogged: &[Rect]) -> Sight {
        let columns = (bounds.width() / tile).ceil().max(1.0) as i32;
        let rows = (bounds.height() / tile).ceil().max(1.0) as i32;
        let mut sight = Sight {
            origin: bounds.min,
            tile,
            columns,
            rows,
            fixed: Vec::new(),
            cells: Vec::new(),
            seen: vec![false; (columns * rows) as usize],
            lit: vec![true; (columns * rows) as usize],
            lights: Vec::new(),
            lamps: Vec::new(),
            lamp_changes: Vec::new(),
            lamp_seconds: 0.0,
            lit_everywhere: true,
            daylight: None,
            range: None,
            map: LightMap::default(),
            map_stale: true,
            light_field: Vec::new(),
            shown_field: Vec::new(),
            lamp_fields: Vec::new(),
            light_field_stale: true,
            field_dirty: None,
            views: Vec::new(),
            views_stale: true,
            explored_px: Vec::new(),
            explored: vec![false; (columns * rows) as usize],
            eyes_at: Vec::new(),
            shut: Vec::new(),
            stale: false,
            traced: false,
            own: Stance::Friendly,
            foreign: Stance::Neutral,
        };
        let mut fixed = vec![
            Cell {
                opaque: false,
                fogged: fogged.is_empty(),
                foreign: false,
                cover: false,
                soft: false,
            };
            (columns * rows) as usize
        ];
        // Outside the deck's box is the outside: nothing to see there and
        // nothing to see through.
        for y in 0..rows {
            for x in 0..columns {
                if !interior.contains(sight.middle(x, y)) {
                    fixed[sight.index(x, y)].opaque = true;
                }
            }
        }
        for rect in opaque {
            sight.mark(&mut fixed, rect, &mut |c| c.opaque = true);
        }
        for rect in fogged {
            sight.mark(&mut fixed, rect, &mut |c| c.fogged = true);
        }
        sight.cells = fixed.clone();
        sight.fixed = fixed;
        sight
    }

    /// Whose the room's own tiles are. A station's room is whatever the
    /// station is; the ship's is the crew's.
    pub fn set_stance(&mut self, stance: Stance) {
        self.own = stance;
        self.map_stale = true;
    }

    /// Which tiles are somebody else's — a station's on a joined deck,
    /// by its box — and whose. `None` makes every tile the room's own.
    pub fn set_foreign(&mut self, rect: Option<Rect>, stance: Stance) {
        self.mark_foreign(rect, stance, false);
    }

    /// The other way about: every tile outside `rect` is somebody else's.
    pub fn set_foreign_outside(&mut self, rect: Rect, stance: Stance) {
        self.mark_foreign(Some(rect), stance, true);
    }

    fn mark_foreign(&mut self, rect: Option<Rect>, stance: Stance, outside: bool) {
        self.foreign = stance;
        self.map_stale = true;
        for c in &mut self.fixed {
            c.foreign = false;
        }
        if let Some(rect) = rect {
            for y in 0..self.rows {
                for x in 0..self.columns {
                    if rect.contains(self.middle(x, y)) != outside {
                        let i = self.index(x, y);
                        self.fixed[i].foreign = true;
                    }
                }
            }
        }
        // The trace's copy carries the doors; the flag is the same either
        // way, so it is copied across rather than traced afresh.
        for (c, f) in self.cells.iter_mut().zip(&self.fixed) {
            c.foreign = f.foreign;
        }
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.columns + x) as usize
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.columns && y < self.rows
    }

    /// Which tile a point is in. Off the grid is a tile off the grid, which
    /// `inside` says no to.
    pub fn tile_of(&self, p: Vec2) -> (i32, i32) {
        (
            ((p.x - self.origin.x) / self.tile).floor() as i32,
            ((p.y - self.origin.y) / self.tile).floor() as i32,
        )
    }

    fn middle(&self, x: i32, y: i32) -> Vec2 {
        self.origin + vec2((x as f32 + 0.5) * self.tile, (y as f32 + 0.5) * self.tile)
    }

    /// Whether a line of sight stops in the tile, as of the last trace:
    /// off the grid counts as stopped.
    fn opaque_at(&self, x: i32, y: i32) -> bool {
        !self.inside(x, y) || self.cells[self.index(x, y)].opaque
    }

    /// The same of a tile of the room's grid — `(0, 0)` the tile at the
    /// room's origin, whatever corner this grid starts from.
    pub fn opaque_room_tile(&self, rx: i32, ry: i32) -> bool {
        let ox = (self.origin.x / self.tile).round() as i32;
        let oy = (self.origin.y / self.tile).round() as i32;
        self.opaque_at(rx - ox, ry - oy)
    }

    /// Every tile a rectangle covers a real share of: the part of the tile
    /// it takes has to be at least half the rectangle's own width and
    /// height, capped at half a tile. A tile-aligned part covers its tiles
    /// and none beside them, since the sliver it shares with a neighbour is
    /// nothing wide; a thin wall straddling a tile line goes to the tile
    /// most of it is in.
    fn mark(&self, cells: &mut [Cell], rect: &Rect, f: &mut dyn FnMut(&mut Cell)) {
        let (x0, y0) = self.tile_of(rect.min);
        let (x1, y1) = self.tile_of(rect.max - vec2(1e-3, 1e-3));
        let need_w = (rect.width() * 0.5).min(self.tile * 0.5);
        let need_h = (rect.height() * 0.5).min(self.tile * 0.5);
        for y in y0.max(0)..=y1.min(self.rows - 1) {
            for x in x0.max(0)..=x1.min(self.columns - 1) {
                let tile = Rect::from_min_size(
                    self.origin + vec2(x as f32 * self.tile, y as f32 * self.tile),
                    vec2(self.tile, self.tile),
                );
                let w = rect.max.x.min(tile.max.x) - rect.min.x.max(tile.min.x);
                let h = rect.max.y.min(tile.max.y) - rect.min.y.max(tile.min.y);
                if w >= need_w && h >= need_h && w > 0.0 && h > 0.0 {
                    f(&mut cells[self.index(x, y)]);
                }
            }
        }
    }

    /// Where a body standing at `p` looks from: its own eyes, and — with a
    /// wall against it — the peek either side along that wall. See the
    /// module note. Read against the last trace's doors.
    pub fn eyes_from(&self, p: Vec2) -> Vec<Eye> {
        let mut eyes = vec![Eye {
            at: p,
            beyond: None,
        }];
        let (bx, by) = self.tile_of(p);
        if !self.inside(bx, by) {
            return eyes;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if !self.opaque_at(bx + dx, by + dy) {
                continue;
            }
            // The wall is there; the tiles either side of the body along
            // it are where it leans out to.
            for (sx, sy) in [(dy, -dx), (-dy, dx)] {
                let (x, y) = (bx + sx, by + sy);
                if self.opaque_at(x, y) {
                    continue;
                }
                let at = self.middle(x, y);
                let eye = Eye {
                    at,
                    beyond: Some(((bx, by), (dx, dy))),
                };
                if !eyes.contains(&eye) {
                    eyes.push(eye);
                }
            }
        }
        eyes
    }

    /// Whether a body at `from` sees the tile a point is in, and from
    /// which eye — its own, or a peek beside a wall. `None` when it does
    /// not.
    pub fn sees_from(&self, from: Vec2, target: Vec2) -> Option<Vec2> {
        let tile = self.tile_of(target);
        if !self.inside(tile.0, tile.1) {
            return None;
        }
        if !self.in_the_light(from, tile) {
            return None;
        }
        self.eyes_from(from)
            .into_iter()
            .find(|eye| eye.admits(tile) && self.clear_line(eye.at, tile))
            .map(|eye| eye.at)
    }

    /// Put these shut doors over the fixed picture, for every line of
    /// sight to read — the trace's and a shot's alike — if they are not
    /// the ones there already. True when they were not. On its own this
    /// is all a room nobody looks through needs: its people aim through
    /// `cells` whether or not a mask is ever traced.
    pub fn set_shut(&mut self, shut: &[Rect]) -> bool {
        if shut == self.shut {
            return false;
        }
        self.shut = shut.to_vec();
        self.stale = true;
        self.views_stale = true;
        let mut cells = self.fixed.clone();
        for door in shut {
            self.mark(&mut cells, door, &mut |c| c.opaque = true);
        }
        self.cells = cells;
        true
    }

    /// Put the lights in, and work out what they reach: a tile within a
    /// light's reach with a clear line from it, over the fixed picture —
    /// the walls and the tall parts, and no door, since the leaves of a
    /// door are not a thing a light waits for. Handed none, the deck is
    /// dark throughout; never handed any, it is lit (`new`). Once per
    /// layout; the lights do not move. Every lamp starts whole: what a
    /// fight did to one is the world's to remember across a relayout
    /// (`set_lamp_health`).
    pub fn set_lights(&mut self, lights: &[Light]) {
        self.lights = lights.to_vec();
        self.lamps = lights
            .iter()
            .map(|l| Lamp {
                at: l.at,
                health: LAMP_HEALTH,
                powered: true,
                level: 1.0,
                flicker: 0.0,
                window: 0,
            })
            .collect();
        self.lamp_changes.clear();
        self.lit_everywhere = false;
        self.light_field_stale = true;
        self.relight();
    }

    /// The tile mask of the lamps that give light, worked out again:
    /// what `set_lights` does, and what a lamp going out — or losing its
    /// power — does over.
    fn relight(&mut self) {
        self.stale = true;
        self.views_stale = true;
        for l in self.lit.iter_mut() {
            *l = false;
        }
        for (light, lamp) in self.lights.iter().zip(&self.lamps) {
            if lamp.is_dark() {
                continue;
            }
            let (lx, ly) = self.tile_of(light.at);
            let span = (light.reach / self.tile).ceil() as i32 + 1;
            for y in (ly - span).max(0)..=(ly + span).min(self.rows - 1) {
                for x in (lx - span).max(0)..=(lx + span).min(self.columns - 1) {
                    let i = self.index(x, y);
                    if self.lit[i] || (self.middle(x, y) - light.at).len() > light.reach {
                        continue;
                    }
                    if self.clear_line_over(&self.fixed, light.at, (x, y)) {
                        self.lit[i] = true;
                    }
                }
            }
        }
        // The sky, after the lamps: a tile under it is lit whether a
        // lamp reaches it or not, and a wall shades nothing from it.
        if let Some(over) = self.daylight {
            for y in 0..self.rows {
                for x in 0..self.columns {
                    if over.contains(self.middle(x, y)) {
                        let i = self.index(x, y);
                        self.lit[i] = true;
                    }
                }
            }
        }
    }

    /// Daylight over `over`, or none: every tile whose middle lies in it
    /// is lit whatever the lamps say — the mask and the picture both —
    /// and the lamps still light the rest. The world's to set, for a
    /// settlement's ground: a town is under a sky, and its lamps are for
    /// the houses. Kept across `set_lights` and a relayout
    /// (`Room::relayout` carries it), since it is not the layout's to
    /// know. A room never handed lights is lit throughout already
    /// (`lit_everywhere`), and the sky changes nothing there.
    pub fn set_daylight(&mut self, over: Option<Rect>) {
        if self.daylight == over {
            return;
        }
        self.daylight = over;
        if self.lit_everywhere {
            return;
        }
        self.relight();
        // The fields are summed again with the sky in, whole: the
        // picture takes it up the next time it is asked for.
        self.light_field_stale = true;
    }

    /// The daylight, as set.
    /// How far an eye sees at all, or `None` for as far as the line is
    /// clear. See `range`. The mask is traced again on the next look.
    pub fn set_range(&mut self, range: Option<f32>) {
        self.range = range;
        self.stale = true;
        self.views_stale = true;
        self.map_stale = true;
    }

    pub fn range(&self) -> Option<f32> {
        self.range
    }

    pub fn daylight(&self) -> Option<Rect> {
        self.daylight
    }

    /// The lights, as put in.
    pub fn lights(&self) -> &[Light] {
        &self.lights
    }

    /// The lamps, one a light, index for index. See [`Lamp`].
    pub fn lamps(&self) -> &[Lamp] {
        &self.lamps
    }

    /// The lamp on the tile a point is in, if there is one — a light
    /// part's tile, say — with its index.
    pub fn lamp_at(&self, p: Vec2) -> Option<(usize, &Lamp)> {
        let tile = self.tile_of(p);
        self.lamps
            .iter()
            .enumerate()
            .find(|(_, l)| self.tile_of(l.at) == tile)
    }

    /// A bolt landed on lamp `i` for `damage`: what it has left comes
    /// down, it flickers for a moment, and at nought it goes out. True
    /// when this is the hit that put it out.
    pub fn damage_lamp(&mut self, i: usize, damage: f32) -> bool {
        let Some(lamp) = self.lamps.get_mut(i) else {
            return false;
        };
        if lamp.is_out() {
            return false;
        }
        lamp.health = (lamp.health - damage).max(0.0);
        lamp.flicker = LAMP_HIT_FLICKER;
        self.lamp_changes.push(i);
        if self.lamps[i].is_out() {
            self.lamp_switched(i);
            true
        } else {
            false
        }
    }

    /// Lamp `i` as the world remembers it: what it has left, set outright
    /// — at the room's building, or from the other room a fight is
    /// mirrored in. Out at nought or under, no flicker.
    pub fn set_lamp_health(&mut self, i: usize, health: f32) {
        let Some(lamp) = self.lamps.get_mut(i) else {
            return;
        };
        if lamp.health == health {
            return;
        }
        let was_out = lamp.is_out();
        lamp.health = health.max(0.0);
        if lamp.is_out() != was_out {
            self.lamp_switched(i);
        }
    }

    /// Which lamps' health changed since this was last asked, oldest
    /// first: the world's cue to remember it, and to carry it across.
    pub fn take_lamp_changes(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.lamp_changes)
    }

    /// Lamp `i` has power, or has not: unpowered it is dark — off the
    /// tile mask and the picture like one shot out — and whole, so it
    /// comes straight back when the power does. The world's, every step,
    /// off the ship's wiring and its brownout; nothing here decides it.
    /// No flicker either way: a reactor does not gutter.
    pub fn set_lamp_powered(&mut self, i: usize, powered: bool) {
        let Some(lamp) = self.lamps.get_mut(i) else {
            return;
        };
        if lamp.powered == powered {
            return;
        }
        let was_dark = lamp.is_dark();
        lamp.powered = powered;
        if lamp.is_dark() != was_dark {
            self.lamp_switched(i);
        }
    }

    /// Lamp `i` went dark, or came back: nothing shown or all of it, its
    /// light off the tile mask or on it, its share of the field taken
    /// out or put back over its reach, and every eye marched again,
    /// since what a lamp lit is what was seen by it.
    fn lamp_switched(&mut self, i: usize) {
        self.lamps[i].level = if self.lamps[i].is_dark() { 0.0 } else { 1.0 };
        self.lamps[i].flicker = 0.0;
        self.relight();
        if let Some(field) = self.lamp_fields.get(i) {
            let over = field.over();
            self.sum_fields(over, true);
            self.field_dirty = Box::join(self.field_dirty, Some(over));
        }
    }

    /// The lamps' clock: a flickering lamp picks a new brightness
    /// [`FLICKER_RATE`] times a second until its flicker runs out, and a
    /// failing one starts a flicker at [`FAIL_FLICKER_ODDS`] a second.
    /// Every change of brightness is the shown field summed again over
    /// the lamp's reach, for the picture to take up.
    pub fn tick_lamps(&mut self, dt: f32) {
        if self.lamps.is_empty() {
            return;
        }
        self.lamp_seconds += dt;
        let t = self.lamp_seconds;
        for i in 0..self.lamps.len() {
            let lamp = &mut self.lamps[i];
            if lamp.is_dark() {
                continue;
            }
            if lamp.flicker <= 0.0 && lamp.is_failing() {
                let window = t as u32;
                if window != lamp.window {
                    lamp.window = window;
                    if noise(i ^ 0x5EED, window) < FAIL_FLICKER_ODDS {
                        lamp.flicker = LAMP_FAIL_FLICKER;
                    }
                }
            }
            let level = if lamp.flicker > 0.0 {
                lamp.flicker -= dt;
                let n = noise(i, (t * FLICKER_RATE) as u32);
                // Off, or nearly, half the time; else on.
                if n < 0.5 { 0.1 + n * 0.5 } else { 1.0 }
            } else {
                1.0
            };
            if (level - lamp.level).abs() < 1.0 / 255.0 {
                continue;
            }
            lamp.level = level;
            if let Some(field) = self.lamp_fields.get(i) {
                let over = field.over();
                self.sum_fields(over, false);
                self.field_dirty = Box::join(self.field_dirty, Some(over));
            }
        }
    }

    /// Whether a light reaches the tile a point is in.
    pub fn lit_at(&self, p: Vec2) -> bool {
        let (x, y) = self.tile_of(p);
        self.inside(x, y) && self.lit[self.index(x, y)]
    }

    /// Whether an eye at `from` can make the tile out at all: lit, or
    /// within [`DARK_RANGE`] of the eye. The dark rule, on top of the
    /// line being clear.
    fn in_the_light(&self, from: Vec2, tile: (i32, i32)) -> bool {
        let away = (self.middle(tile.0, tile.1) - from).len();
        if self.range.is_some_and(|r| away > r) {
            return false;
        }
        self.lit[self.index(tile.0, tile.1)] || away <= DARK_RANGE * self.tile
    }

    /// Mark these rectangles as low cover — sandbags: nothing to sight or
    /// to a walk, but a body within [`COVER_REACH`] behind one, on the
    /// side a bolt comes from, ducks under it. Part of the fixed picture,
    /// so it survives every `set_shut`.
    pub fn set_cover(&mut self, cover: &[Rect]) {
        let mut fixed = std::mem::take(&mut self.fixed);
        for c in fixed.iter_mut() {
            c.cover = false;
        }
        for rect in cover {
            self.mark(&mut fixed, rect, &mut |c| c.cover = true);
        }
        for (c, f) in self.cells.iter_mut().zip(&fixed) {
            c.cover = f.cover;
        }
        self.fixed = fixed;
    }

    /// Mark these rectangles as furniture among the opaque cells — the
    /// tall parts, as against the walls — for the picture: a light's
    /// direct fall stops at them like at a wall, and a softer fill goes
    /// past them, so a shelf casts a shade and a bulkhead casts the dark.
    /// Nothing to the rule, which reads `opaque` alone. Part of the fixed
    /// picture, so it survives every `set_shut`; before it is called,
    /// every opaque cell is a wall.
    pub fn set_tall(&mut self, tall: &[Rect]) {
        let mut fixed = std::mem::take(&mut self.fixed);
        for c in fixed.iter_mut() {
            c.soft = false;
        }
        for rect in tall {
            self.mark(&mut fixed, rect, &mut |c| c.soft = c.opaque);
        }
        for (c, f) in self.cells.iter_mut().zip(&fixed) {
            c.soft = f.soft;
        }
        self.fixed = fixed;
        self.light_field_stale = true;
        self.views_stale = true;
    }

    /// Whether a tile is low cover.
    pub fn cover_at(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.cells[self.index(x, y)].cover
    }

    /// Whether a body at `body` is in cover from something at `from`: a
    /// tile of low cover lies on the straight line between them, within
    /// [`COVER_REACH`] of the body, and the body is not standing on the
    /// sandbags itself. The same traversal as [`Sight::clear_line`], read
    /// for cover rather than for a wall; where the line goes past the reach
    /// the answer is no, so a body far behind a barricade is in the open
    /// — a bolt comes over it.
    pub fn covered(&self, body: Vec2, from: Vec2) -> bool {
        let (mut x, mut y) = self.tile_of(body);
        let (tx, ty) = self.tile_of(from);
        if (x, y) == (tx, ty) || self.cover_at(x, y) {
            return false;
        }
        let d = from - body;
        let step_x: i32 = if d.x > 0.0 { 1 } else { -1 };
        let step_y: i32 = if d.y > 0.0 { 1 } else { -1 };
        let next_x = self.origin.x + (x + if d.x > 0.0 { 1 } else { 0 }) as f32 * self.tile;
        let next_y = self.origin.y + (y + if d.y > 0.0 { 1 } else { 0 }) as f32 * self.tile;
        let (mut t_x, delta_x) = if d.x.abs() > 1e-6 {
            ((next_x - body.x) / d.x, self.tile / d.x.abs())
        } else {
            (f32::INFINITY, f32::INFINITY)
        };
        let (mut t_y, delta_y) = if d.y.abs() > 1e-6 {
            ((next_y - body.y) / d.y, self.tile / d.y.abs())
        } else {
            (f32::INFINITY, f32::INFINITY)
        };
        let reach = COVER_REACH * self.tile;
        let most = (tx - x).abs() + (ty - y).abs() + 2;
        for _ in 0..most {
            let t = if t_x < t_y {
                let t = t_x;
                x += step_x;
                t_x += delta_x;
                t
            } else {
                let t = t_y;
                y += step_y;
                t_y += delta_y;
                t
            };
            // Past the reach, or at the shooter: nothing between counts.
            if t >= 1.0 || (x, y) == (tx, ty) || (self.middle(x, y) - body).len() > reach {
                return false;
            }
            if self.cover_at(x, y) {
                return true;
            }
        }
        false
    }

    /// Work the mask out again from these eyes and these shut doors, if
    /// anything about them has changed since last time. True when it was.
    pub fn observe(&mut self, eyes: &[Vec2], shut: &[Rect]) -> bool {
        let eyes_at: Vec<(i32, i32)> = eyes.iter().map(|&p| self.tile_of(p)).collect();
        self.set_shut(shut);
        if self.traced && !self.stale && eyes_at == self.eyes_at {
            return false;
        }
        self.eyes_at = eyes_at;
        self.stale = false;
        self.traced = true;

        for s in self.seen.iter_mut() {
            *s = false;
        }
        for &body in eyes {
            for eye in self.eyes_from(body) {
                let (ex, ey) = self.tile_of(eye.at);
                if !self.inside(ex, ey) {
                    continue;
                }
                for y in 0..self.rows {
                    for x in 0..self.columns {
                        let i = self.index(x, y);
                        if self.seen[i] || !eye.admits((x, y)) || !self.in_the_light(body, (x, y)) {
                            continue;
                        }
                        if self.clear_line(eye.at, (x, y)) {
                            self.seen[i] = true;
                        }
                    }
                }
            }
        }
        // What has been seen stays known.
        for (e, &s) in self.explored.iter_mut().zip(&self.seen) {
            *e |= s;
        }
        true
    }

    /// Whether the straight line from `from` to the middle of tile `to`
    /// crosses no opaque tile before it gets there. The tiles the line
    /// passes through are walked one at a time, whichever grid line it
    /// crosses next — the standard traversal — so a line squeezing between
    /// two opaque tiles set corner to corner still has to pass through one
    /// of them, and is stopped.
    pub fn clear_line(&self, from: Vec2, to: (i32, i32)) -> bool {
        self.clear_line_over(&self.cells, from, to)
    }

    /// The same over any set of cells: the fixed picture for a light, the
    /// picture with the doors in for an eye.
    fn clear_line_over(&self, cells: &[Cell], from: Vec2, to: (i32, i32)) -> bool {
        let (mut x, mut y) = self.tile_of(from);
        let (tx, ty) = to;
        if (x, y) == (tx, ty) {
            return true;
        }
        let target = self.middle(tx, ty);
        let d = target - from;
        let step_x: i32 = if d.x > 0.0 { 1 } else { -1 };
        let step_y: i32 = if d.y > 0.0 { 1 } else { -1 };
        // How far along the line (as a fraction of it) the next vertical
        // and horizontal grid lines are, and how much further each one
        // after that is.
        let next_x = self.origin.x + (x + if d.x > 0.0 { 1 } else { 0 }) as f32 * self.tile;
        let next_y = self.origin.y + (y + if d.y > 0.0 { 1 } else { 0 }) as f32 * self.tile;
        let (mut t_x, delta_x) = if d.x.abs() > 1e-6 {
            ((next_x - from.x) / d.x, self.tile / d.x.abs())
        } else {
            (f32::INFINITY, f32::INFINITY)
        };
        let (mut t_y, delta_y) = if d.y.abs() > 1e-6 {
            ((next_y - from.y) / d.y, self.tile / d.y.abs())
        } else {
            (f32::INFINITY, f32::INFINITY)
        };
        // Never more steps than tiles on the way, whatever the arithmetic.
        let most = (tx - x).abs() + (ty - y).abs() + 2;
        for _ in 0..most {
            if t_x < t_y {
                x += step_x;
                t_x += delta_x;
            } else {
                y += step_y;
                t_y += delta_y;
            }
            if (x, y) == (tx, ty) {
                return true;
            }
            if !self.inside(x, y) || cells[self.index(x, y)].opaque {
                return false;
            }
        }
        false
    }

    /// Where the straight line from `from` to `to` first enters an opaque
    /// tile, if it does before it gets there: the point on the tile's
    /// edge. The same traversal as [`Sight::clear_line`], to a point
    /// rather than a tile's middle — what stops a shot.
    pub fn first_opaque_along(&self, from: Vec2, to: Vec2) -> Option<Vec2> {
        let (mut x, mut y) = self.tile_of(from);
        let (tx, ty) = self.tile_of(to);
        if (x, y) == (tx, ty) {
            return None;
        }
        let d = to - from;
        let step_x: i32 = if d.x > 0.0 { 1 } else { -1 };
        let step_y: i32 = if d.y > 0.0 { 1 } else { -1 };
        let next_x = self.origin.x + (x + if d.x > 0.0 { 1 } else { 0 }) as f32 * self.tile;
        let next_y = self.origin.y + (y + if d.y > 0.0 { 1 } else { 0 }) as f32 * self.tile;
        let (mut t_x, delta_x) = if d.x.abs() > 1e-6 {
            ((next_x - from.x) / d.x, self.tile / d.x.abs())
        } else {
            (f32::INFINITY, f32::INFINITY)
        };
        let (mut t_y, delta_y) = if d.y.abs() > 1e-6 {
            ((next_y - from.y) / d.y, self.tile / d.y.abs())
        } else {
            (f32::INFINITY, f32::INFINITY)
        };
        let most = (tx - x).abs() + (ty - y).abs() + 2;
        for _ in 0..most {
            // Where the line crosses into the next tile, as a fraction of
            // the whole of it.
            let t = if t_x < t_y {
                let t = t_x;
                x += step_x;
                t_x += delta_x;
                t
            } else {
                let t = t_y;
                y += step_y;
                t_y += delta_y;
                t
            };
            if t >= 1.0 {
                return None;
            }
            if self.opaque_at(x, y) {
                return Some(from + d * t);
            }
            if (x, y) == (tx, ty) {
                return None;
            }
        }
        None
    }

    /// Whether anybody sees the tile a point is in.
    pub fn seen_at(&self, p: Vec2) -> bool {
        let (x, y) = self.tile_of(p);
        self.inside(x, y) && self.seen[self.index(x, y)]
    }

    /// What is drawn over the tile a point is in, as of the last trace:
    /// 0 nothing, 1 the semi fog of a friendly structure, 2 the grey ring
    /// round what is seen of a stranger's, 3 the black beyond it. For the
    /// probes.
    pub fn veil_at(&self, p: Vec2) -> u32 {
        let (x, y) = self.tile_of(p);
        if !self.inside(x, y) {
            return 0;
        }
        match self.veil(self.index(x, y), false) {
            None => 0,
            Some(Veil::Semi) => 1,
            Some(Veil::Grey) => 2,
            Some(Veil::Black) => 3,
        }
    }

    /// How the fog over a tile is drawn, if it is: by whose the tile is,
    /// and — for somebody else's — whether it has ever been seen. With
    /// `all`, nothing is seen and nothing has been.
    fn veil(&self, i: usize, all: bool) -> Option<Veil> {
        let c = &self.cells[i];
        if !c.fogged || (!all && self.seen[i]) {
            return None;
        }
        let stance = if c.foreign { self.foreign } else { self.own };
        Some(match stance {
            Stance::Friendly => Veil::Semi,
            _ if !all && self.explored[i] => Veil::Grey,
            _ => Veil::Black,
        })
    }

    /// The fog on the tile grid, over every fogged tile nobody sees — or,
    /// with `all`, over every fogged tile, for a room nobody is looking
    /// into. Through the crew's own eyes the picture is the [`LightMap`]'s
    /// and this is not drawn. Each kind of veil is one pass: neighbouring
    /// tiles are drawn as one rectangle wherever they can be, so the fog
    /// is a few shapes rather than a thousand, and a run that matches the
    /// run under it is one shape taller.
    pub fn draw(&self, list: &mut DrawList, all: bool) {
        for (veil, colour) in [
            (Veil::Semi, FOG),
            (Veil::Grey, FOG_GREY),
            (Veil::Black, FOG_BLACK),
        ] {
            self.draw_runs(list, colour, |i| self.veil(i, all) == Some(veil));
        }
    }

    fn draw_runs(&self, list: &mut DrawList, colour: Color, fogged: impl Fn(usize) -> bool) {
        let fogged = |x: i32, y: i32| fogged(self.index(x, y));
        // Runs along each row, then the same runs in consecutive rows
        // stacked.
        let mut open: Vec<(i32, i32, i32)> = Vec::new(); // (x0, x1, y0)
        for y in 0..self.rows {
            let mut runs: Vec<(i32, i32)> = Vec::new();
            let mut x = 0;
            while x < self.columns {
                if fogged(x, y) {
                    let x0 = x;
                    while x < self.columns && fogged(x, y) {
                        x += 1;
                    }
                    runs.push((x0, x));
                } else {
                    x += 1;
                }
            }
            let mut next: Vec<(i32, i32, i32)> = Vec::new();
            for &(x0, x1) in &runs {
                match open.iter().position(|&(a, b, _)| a == x0 && b == x1) {
                    Some(i) => next.push(open.swap_remove(i)),
                    None => next.push((x0, x1, y)),
                }
            }
            for (x0, x1, y0) in open.drain(..) {
                self.fog_rect(list, colour, x0, x1, y0, y);
            }
            open = next;
        }
        for (x0, x1, y0) in open {
            self.fog_rect(list, colour, x0, x1, y0, self.rows);
        }
    }

    fn fog_rect(&self, list: &mut DrawList, colour: Color, x0: i32, x1: i32, y0: i32, y1: i32) {
        let min = self.origin + vec2(x0 as f32 * self.tile, y0 as f32 * self.tile);
        let size = vec2((x1 - x0) as f32 * self.tile, (y1 - y0) as f32 * self.tile);
        // An opaque rectangle overlaps its neighbours by a hair: every
        // edge is feathered a pixel wide, and two black rectangles meeting
        // edge to edge showed the seam as a lighter line. A translucent
        // one may not, since the overlap would be twice as dark.
        let grow = if colour.a >= 1.0 { OVERLAP } else { 0.0 };
        list.rect(min + size * 0.5, size + vec2(grow, grow), 0.0, 0.0, colour);
    }

    pub fn tiles(&self) -> i32 {
        self.columns * self.rows
    }
}

/// How many pixels of the light map a tile is across. Eight: fine enough
/// that a shadow's edge, drawn magnified and filtered, reads as a line
/// and not as steps, and coarse enough that a joined deck is under half a
/// million pixels to march.
pub const MAP_PX_PER_TILE: i32 = 8;
/// How many rays an eye or a light is marched along. Four thousand: a ray
/// and its neighbour are a pixel apart eighty tiles out, past a plain's
/// sight range with room to spare, so nothing between them is missed —
/// two thousand streaked at the far side of a landed room's box.
const RAYS: u32 = 4096;
/// The fog over what the crew do not see of their own deck.
const MAP_FOG: f32 = 0.62;
/// The fog over what they have looked at of a stranger's deck and do not
/// see now: the structure shows through it, and no body does.
const MAP_GREY: f32 = 0.80;
/// The dark over what they see of it that no light reaches — deep enough
/// that a lamp's pool reads against it, short of the fog so the deck
/// stays readable.
const MAP_DARK: f32 = 0.50;
/// How much of a light's reach is full brightness before it fades, and
/// how the rest falls off — steeply at first, so a pool has a bright
/// heart and a soft rim, the way a lamp's does.
const LIGHT_CORE: f32 = 0.35;
const LIGHT_FALL: f32 = 1.6;
/// How much of a lamp's light gets past furniture — a shelf, a cabinet,
/// a table — into the shade behind it: the shadow, and it is a light one.
/// Nothing gets past a wall.
const SHADOW_FILL: f32 = 0.55;
/// The lamplight over a lit pixel at full brightness: the warm wash the
/// host tints the deck with, nought to one, faded with the light — and
/// how much of it shows through the fog over what the crew know, since
/// the lamps are always on and the crew know where they hang.
const GLOW: f32 = 0.24;
const GLOW_UNDER_FOG: f32 = 0.5;

/// The smooth picture of the crew's sight and the lamps: two bytes a
/// pixel — the darkness to draw over the room, nought where the crew see
/// a lit tile, the dark's where they see an unlit one, the fog's where
/// they see nothing of their own deck, the grey's where they see nothing
/// of a stranger's they have looked at, black where they have never
/// looked; and the lamplight, the warm wash where a light falls. Marched,
/// not traced: from every eye and every light a fan of [`RAYS`] rays is
/// walked pixel by pixel until it meets an opaque cell, so what is lit
/// and what is seen have the straight edges of the walls that stop them
/// and not the tile grid's steps, and a lamp throws a cone past a
/// doorway and a shade behind a cabinet. The tile mask stays the
/// **rule** — the fight and the world read it — and this is the picture
/// of it, by the same lines.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightMap {
    /// The grid's corner, in room units, and a pixel's side.
    pub origin: Vec2,
    pub px: f32,
    pub width: usize,
    pub height: usize,
    /// Darkness, nought to 255, row by row.
    pub alpha: Vec<u8>,
    /// Lamplight, nought to 255, row by row: how strongly the host tints
    /// the pixel with its lamplight colour.
    pub glow: Vec<u8>,
    /// Bumped every time the map is worked out again, so a host can tell a
    /// new picture from the one it has already uploaded.
    pub version: u64,
    /// Where this version differs from the one before it, as `(x, y,
    /// width, height)` in pixels — what a host holding the previous
    /// version has to take again — or `None` for the whole picture.
    pub changed: Option<(usize, usize, usize, usize)>,
}

impl LightMap {
    /// The map's extent in room units.
    pub fn size(&self) -> Vec2 {
        vec2(self.width as f32 * self.px, self.height as f32 * self.px)
    }
}

/// What one body's eyes reach, as last marched: the eyes it was marched
/// from, a flag a pixel — seen from this body or not — and the box the
/// seen pixels lie in. Kept so that a body standing still is not marched
/// again while another walks, and so that the picture is composed again
/// only where a view changed.
#[derive(Clone, Debug)]
struct View {
    eyes: Vec<Eye>,
    seen: Vec<bool>,
    reached: Option<Box>,
}

/// A box of map pixels, both ends in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Box {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Box {
    fn with(self, x: usize, y: usize) -> Box {
        Box {
            x0: self.x0.min(x),
            y0: self.y0.min(y),
            x1: self.x1.max(x),
            y1: self.y1.max(y),
        }
    }

    fn join(a: Option<Box>, b: Option<Box>) -> Option<Box> {
        match (a, b) {
            (Some(a), Some(b)) => Some(a.with(b.x0, b.y0).with(b.x1, b.y1)),
            (a, None) => a,
            (None, b) => b,
        }
    }
}

/// One lamp's light over its reach, cached: the box of map pixels it
/// can fall on and a byte a pixel of it, so a lamp that flickers or goes
/// out is its box summed again and not every lamp marched again.
#[derive(Clone, Debug, Default)]
struct LampField {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    pixels: Vec<u8>,
}

impl LampField {
    /// The box it covers, both ends in.
    fn over(&self) -> Box {
        Box {
            x0: self.x0,
            y0: self.y0,
            x1: (self.x0 + self.w).saturating_sub(1),
            y1: (self.y0 + self.h).saturating_sub(1),
        }
    }
}

impl Sight {
    /// The light over the fixed picture, one byte a pixel, nought dark to
    /// 255 lit — worked out once per layout, since the lights do not move
    /// and no door stops one. Every lamp is marched twice: its direct
    /// fall, which every opaque cell stops, and a fill of [`SHADOW_FILL`]
    /// of it, which the furniture (`Cell::soft`) lets past and the walls
    /// do not — so behind a shelf is a shade and behind a bulkhead the
    /// dark. Full brightness to [`LIGHT_CORE`] of the reach, fading to
    /// the edge. Each lamp's fall is kept on its own ([`LampField`]) and
    /// the field is their **sum**, saturating — light adds, so the deck
    /// between four lamps is lit by all four, where the brightest alone
    /// left a dark star between them — read by [`Sight::light_map`].
    fn build_light_fields(&mut self) {
        let (w, h) = self.map_dims();
        self.light_field = vec![0u8; w * h];
        self.shown_field = vec![0u8; w * h];
        self.lamp_fields.clear();
        if self.lit_everywhere {
            for v in self.light_field.iter_mut() {
                *v = 255;
            }
            self.shown_field.clone_from(&self.light_field);
            return;
        }
        let px = self.tile / MAP_PX_PER_TILE as f32;
        let mut fields: Vec<LampField> = Vec::with_capacity(self.lights.len());
        for light in &self.lights {
            // The box its reach can fall in, clipped to the map.
            let ox = ((light.at.x - self.origin.x) / px).floor() as i64;
            let oy = ((light.at.y - self.origin.y) / px).floor() as i64;
            let r = (light.reach / px).ceil() as i64 + 1;
            let x0 = (ox - r).clamp(0, w as i64) as usize;
            let y0 = (oy - r).clamp(0, h as i64) as usize;
            let x1 = (ox + r + 1).clamp(0, w as i64) as usize;
            let y1 = (oy + r + 1).clamp(0, h as i64) as usize;
            let mut field = LampField {
                x0,
                y0,
                w: x1 - x0,
                h: y1 - y0,
                pixels: vec![0u8; (x1 - x0) * (y1 - y0)],
            };
            for (through_soft, share) in [(false, 1.0), (true, SHADOW_FILL)] {
                self.march(
                    light.at,
                    Some(light.reach),
                    &self.fixed,
                    through_soft,
                    &mut |i, _, d| {
                        let (x, y) = (i % w, i / w);
                        if x < x0 || x >= x1 || y < y0 || y >= y1 {
                            return;
                        }
                        let t = d / light.reach;
                        let bright = if t <= LIGHT_CORE {
                            1.0
                        } else {
                            ((1.0 - t) / (1.0 - LIGHT_CORE))
                                .clamp(0.0, 1.0)
                                .powf(LIGHT_FALL)
                        };
                        let v = (bright * share * 255.0) as u8;
                        let j = (y - y0) * field.w + (x - x0);
                        if field.pixels[j] < v {
                            field.pixels[j] = v;
                        }
                    },
                );
            }
            fields.push(field);
        }
        self.lamp_fields = fields;
        let whole = Box {
            x0: 0,
            y0: 0,
            x1: w.saturating_sub(1),
            y1: h.saturating_sub(1),
        };
        self.sum_fields(whole, true);
    }

    /// The lamps' fields summed again over `over`: the shown field with
    /// each lamp at its level, and — `eyes` — the one the eyes read, with
    /// each lamp that is not out at full. Saturating at 255.
    fn sum_fields(&mut self, over: Box, eyes: bool) {
        let w = self.map_dims().0;
        if self.lit_everywhere || self.light_field.is_empty() {
            return;
        }
        let (x0, y0, x1, y1) = (over.x0, over.y0, over.x1, over.y1);
        let width = x1 + 1 - x0;
        let mut shown = vec![0u32; width * (y1 + 1 - y0)];
        let mut nominal = vec![0u32; if eyes { shown.len() } else { 0 }];
        for (field, lamp) in self.lamp_fields.iter().zip(&self.lamps) {
            if lamp.is_dark() {
                continue;
            }
            let level = (lamp.level.clamp(0.0, 1.0) * 256.0) as u32;
            let fx0 = field.x0.max(x0);
            let fy0 = field.y0.max(y0);
            let fx1 = (field.x0 + field.w).min(x1 + 1);
            let fy1 = (field.y0 + field.h).min(y1 + 1);
            if fx1 <= fx0 || fy1 <= fy0 {
                continue;
            }
            for y in fy0..fy1 {
                let from = (y - field.y0) * field.w + (fx0 - field.x0);
                let row = &field.pixels[from..from + (fx1 - fx0)];
                let to = (y - y0) * width + (fx0 - x0);
                for (k, &v) in row.iter().enumerate() {
                    shown[to + k] += (v as u32 * level) >> 8;
                    if eyes {
                        nominal[to + k] += v as u32;
                    }
                }
            }
        }
        for y in y0..=y1 {
            let from = (y - y0) * width;
            let to = y * w + x0;
            for k in 0..width {
                self.shown_field[to + k] = shown[from + k].min(255) as u8;
                if eyes {
                    self.light_field[to + k] = nominal[from + k].min(255) as u8;
                }
            }
        }
        // The sky over the lot: full in both fields wherever the daylight
        // falls, so the picture agrees with the mask (`relight`) and a
        // lamp going out under it changes nothing there.
        if let Some(sky) = self.daylight {
            let px = self.tile / MAP_PX_PER_TILE as f32;
            let sx0 = (((sky.min.x - self.origin.x) / px).floor().max(0.0) as usize).max(x0);
            let sy0 = (((sky.min.y - self.origin.y) / px).floor().max(0.0) as usize).max(y0);
            let sx1 = (((sky.max.x - self.origin.x) / px).ceil().max(0.0) as usize).min(x1 + 1);
            let sy1 = (((sky.max.y - self.origin.y) / px).ceil().max(0.0) as usize).min(y1 + 1);
            for y in sy0..sy1 {
                let at_y = self.origin.y + (y as f32 + 0.5) * px;
                for x in sx0..sx1 {
                    let at_x = self.origin.x + (x as f32 + 0.5) * px;
                    if !sky.contains(vec2(at_x, at_y)) {
                        continue;
                    }
                    self.shown_field[y * w + x] = 255;
                    if eyes {
                        self.light_field[y * w + x] = 255;
                    }
                }
            }
        }
    }

    /// The map's pixels across and down.
    fn map_dims(&self) -> (usize, usize) {
        (
            (self.columns * MAP_PX_PER_TILE) as usize,
            (self.rows * MAP_PX_PER_TILE) as usize,
        )
    }

    /// Walk [`RAYS`] rays out from `from`, calling `f` with each pixel
    /// reached — its index, its tile, and how far along the ray it is in
    /// room units — until the ray meets an opaque cell of `cells` (one
    /// that is furniture, `soft`, too, unless `through_soft`), leaves the
    /// grid, or has gone `reach`. Each ray is the grid traversal, in
    /// pixels: it steps to whichever pixel edge comes next, so every
    /// pixel the line passes through is visited once and none is skipped
    /// — the same walk as [`Sight::clear_line`] at eight times the
    /// resolution, and integer arithmetic bar one add a step.
    fn march(
        &self,
        from: Vec2,
        reach: Option<f32>,
        cells: &[Cell],
        through_soft: bool,
        f: &mut dyn FnMut(usize, (i32, i32), f32),
    ) {
        let (w, h) = self.map_dims();
        let (w, h) = (w as i32, h as i32);
        let px = self.tile / MAP_PX_PER_TILE as f32;
        // The eye in pixel units.
        let ox = (from.x - self.origin.x) / px;
        let oy = (from.y - self.origin.y) / px;
        let far = reach.map(|r| r / px).unwrap_or((w.max(h) as f32) * 1.5);
        let (fx, fy) = self.tile_of(from);
        let stops = |c: &Cell| c.opaque && !(through_soft && c.soft);
        let from_opaque = self.inside(fx, fy) && stops(&cells[self.index(fx, fy)]);
        let shift = MAP_PX_PER_TILE.trailing_zeros();
        for r in 0..RAYS {
            let a = r as f32 / RAYS as f32 * core::f32::consts::TAU;
            let (dx, dy) = (a.cos(), a.sin());
            let mut x = ox.floor() as i32;
            let mut y = oy.floor() as i32;
            let step_x: i32 = if dx > 0.0 { 1 } else { -1 };
            let step_y: i32 = if dy > 0.0 { 1 } else { -1 };
            // How far along the ray (in pixels) the next vertical and
            // horizontal pixel edges are, and how much further each one
            // after that is.
            let (mut t_x, delta_x) = if dx.abs() > 1e-6 {
                let next = if dx > 0.0 { x + 1 } else { x } as f32;
                ((next - ox) / dx, 1.0 / dx.abs())
            } else {
                (f32::INFINITY, f32::INFINITY)
            };
            let (mut t_y, delta_y) = if dy.abs() > 1e-6 {
                let next = if dy > 0.0 { y + 1 } else { y } as f32;
                ((next - oy) / dy, 1.0 / dy.abs())
            } else {
                (f32::INFINITY, f32::INFINITY)
            };
            // The pixel the eye is in, then every one the ray crosses into.
            let mut t = 0.0;
            loop {
                if x < 0 || y < 0 || x >= w || y >= h || t > far {
                    break;
                }
                let (tx, ty) = (x >> shift, y >> shift);
                let i = (y * w + x) as usize;
                let opaque = stops(&cells[self.index(tx, ty)]);
                // A ray reaches into the wall that stops it — the wall is
                // seen, and lit, from the room — and no further; one that
                // starts inside a wall (an eye pressed to it) gets out.
                if opaque && !(from_opaque && (tx, ty) == (fx, fy)) {
                    f(i, (tx, ty), t * px);
                    break;
                }
                f(i, (tx, ty), t * px);
                if t_x < t_y {
                    t = t_x;
                    x += step_x;
                    t_x += delta_x;
                } else {
                    t = t_y;
                    y += step_y;
                    t_y += delta_y;
                }
            }
        }
    }

    /// Whether a body's eyes are where they were last marched from: the
    /// same eyes, each within half a pixel of where it was.
    fn view_holds(&self, view: &View, eyes: &[Eye]) -> bool {
        let px = self.tile / MAP_PX_PER_TILE as f32;
        view.eyes.len() == eyes.len()
            && view
                .eyes
                .iter()
                .zip(eyes)
                .all(|(a, b)| a.beyond == b.beyond && (a.at - b.at).len() < px * 0.5)
    }

    /// March one body's eyes over the cells as they are, into `seen`,
    /// and say which pixels it reached as a box.
    fn view_of(&self, body: Vec2, eyes: &[Eye], seen: &mut [bool]) -> Option<Box> {
        for s in seen.iter_mut() {
            *s = false;
        }
        let range = DARK_RANGE * self.tile;
        let w = self.map_dims().0;
        let mut reached: Option<Box> = None;
        for eye in eyes {
            // A peek adds only what lies past its wall; the body's own eyes
            // everything. The dark rule is measured from the body, as the
            // trace measures it.
            self.march(eye.at, self.range, &self.cells, false, &mut |i, tile, _| {
                if !seen[i]
                    && eye.admits(tile)
                    && (self.light_field[i] > 0
                        || (self.middle(tile.0, tile.1) - body).len() <= range)
                {
                    seen[i] = true;
                    let (x, y) = (i % w, i / w);
                    reached = Some(match reached {
                        None => Box {
                            x0: x,
                            y0: y,
                            x1: x,
                            y1: y,
                        },
                        Some(b) => b.with(x, y),
                    });
                }
            });
        }
        reached
    }

    /// The picture of the mask from these bodies' eyes, worked out again
    /// only where it has to be: a body that has not moved since it was
    /// last marched keeps its view, one that has is marched afresh, and
    /// every one is when the cells changed — a door, the layout — and the
    /// picture is composed again over the box the changed views cover
    /// and nowhere else. True when the map changed, which is when its
    /// `version` moved. See [`LightMap`].
    pub fn light_map(&mut self, bodies: &[Vec2]) -> bool {
        if self.light_field.is_empty() || self.light_field_stale {
            self.build_light_fields();
            self.light_field_stale = false;
            self.field_dirty = None;
            self.map_stale = true;
        }
        let (w, h) = self.map_dims();
        if self.explored_px.len() != w * h {
            self.explored_px = vec![false; w * h];
            self.map_stale = true;
        }
        if self.views_stale {
            self.views.clear();
            self.views_stale = false;
            self.map_stale = true;
        }
        // Where the picture changed: nowhere yet, a box, or everywhere.
        // A lamp that flickered or went out is a box of it already.
        let mut dirty: Option<Box> = self.field_dirty.take();
        let mut changed = self.map_stale || dirty.is_some();
        if self.views.len() > bodies.len() {
            for view in self.views.drain(bodies.len()..) {
                dirty = Box::join(dirty, view.reached);
                changed = true;
            }
        }
        for (b, &body) in bodies.iter().enumerate() {
            let eyes = self.eyes_from(body);
            if self.views.get(b).is_some_and(|v| self.view_holds(v, &eyes)) {
                continue;
            }
            let (mut seen, was) = match self.views.get_mut(b) {
                Some(v) => (std::mem::take(&mut v.seen), v.reached),
                None => (vec![false; w * h], None),
            };
            let reached = self.view_of(body, &eyes, &mut seen);
            dirty = Box::join(Box::join(dirty, was), reached);
            let view = View {
                eyes,
                seen,
                reached,
            };
            if b < self.views.len() {
                self.views[b] = view;
            } else {
                self.views.push(view);
            }
            changed = true;
        }
        if !changed {
            return false;
        }
        let over = if self.map_stale {
            Box {
                x0: 0,
                y0: 0,
                x1: w - 1,
                y1: h - 1,
            }
        } else {
            match dirty {
                Some(b) => b,
                None => return false,
            }
        };
        self.map_stale = false;
        self.compose(over);
        true
    }

    /// Put the views, the memory and the light field together into the
    /// picture over the tiles `over` touches. Tile by tile so the cell is
    /// looked up once a tile rather than once a pixel.
    fn compose(&mut self, over: Box) {
        let (w, h) = self.map_dims();
        let px = self.tile / MAP_PX_PER_TILE as f32;
        let n = MAP_PX_PER_TILE as usize;
        let whole = self.map.width != w || self.map.height != h;
        if whole {
            self.map.alpha = vec![0u8; w * h];
            self.map.glow = vec![0u8; w * h];
        }
        // No lamps at all — the classic room — is lit and has no lamplight
        // to wash the deck with.
        let wash = if self.lit_everywhere { 0.0 } else { GLOW };
        let fog = (MAP_FOG * 255.0) as u8;
        let grey = (MAP_GREY * 255.0) as u8;
        let (alpha, glow) = (&mut self.map.alpha, &mut self.map.glow);
        for ty in over.y0 / n..=over.y1 / n {
            for tx in over.x0 / n..=over.x1 / n {
                let c = &self.cells[ty * self.columns as usize + tx];
                let friendly =
                    (if c.foreign { self.foreign } else { self.own }) == Stance::Friendly;
                for py in ty * n..(ty + 1) * n {
                    for i in py * w + tx * n..py * w + (tx + 1) * n {
                        // The unfogged outside is nobody's: nothing drawn.
                        if !c.fogged {
                            alpha[i] = 0;
                            glow[i] = 0;
                            continue;
                        }
                        let seen = self.views.iter().any(|v| v.seen[i]);
                        // As shown, flicker and all: the eyes read the
                        // other field, and only for what they reach.
                        let light = self.shown_field[i] as f32 / 255.0;
                        if seen {
                            self.explored_px[i] = true;
                            alpha[i] = (MAP_DARK * (1.0 - light) * 255.0) as u8;
                            glow[i] = (wash * light * 255.0) as u8;
                        } else if friendly {
                            alpha[i] = fog;
                            glow[i] = (wash * GLOW_UNDER_FOG * light * 255.0) as u8;
                        } else if self.explored_px[i] {
                            alpha[i] = grey;
                            glow[i] = (wash * GLOW_UNDER_FOG * light * 255.0) as u8;
                        } else {
                            alpha[i] = 255;
                            glow[i] = 0;
                        }
                    }
                }
            }
        }
        self.map.origin = self.origin;
        self.map.px = px;
        self.map.width = w;
        self.map.height = h;
        self.map.version += 1;
        // What a host holding the previous version has to take again:
        // whole tiles, since that is what was composed.
        self.map.changed =
            if whole || (over.x0 == 0 && over.y0 == 0 && over.x1 + 1 >= w && over.y1 + 1 >= h) {
                None
            } else {
                let (x0, y0) = (over.x0 / n * n, over.y0 / n * n);
                let (x1, y1) = (
                    ((over.x1 / n + 1) * n).min(w),
                    ((over.y1 / n + 1) * n).min(h),
                );
                Some((x0, y0, x1 - x0, y1 - y0))
            };
    }

    /// The map as last worked out.
    pub fn map(&self) -> &LightMap {
        &self.map
    }
}

/// What a room draws of the fog, and whether its bodies are drawn: whose
/// eyes the picture is through.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Fog {
    /// The crew's own: what they see is lit, the rest is fogged, and every
    /// body is the crew's, so every body is drawn.
    #[default]
    Crew,
    /// Nobody's: a room looked at from outside, a station's with the ship
    /// alongside. All of it fogged, and nobody in it drawn.
    All,
    /// Somebody else's: a station's room under a joined deck, whose fog is
    /// the joined room's to draw. No fog here, and a body is drawn only
    /// when the world says it is in the crew's view (`Game::set_seen`).
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: f32 = 52.0;

    fn middle(x: f32, y: f32) -> Vec2 {
        vec2((x + 0.5) * TILE, (y + 0.5) * TILE)
    }

    /// Sandbags at (5, 5) in an open room: a body just south of them is
    /// covered from the north and from nowhere else, one standing on them
    /// is in the open, and one two tiles back is past the reach — a bolt
    /// comes over.
    #[test]
    fn a_body_close_behind_sandbags_is_covered_from_across_them_and_from_nowhere_else() {
        let room = Rect::from_min_size(Vec2::ZERO, vec2(12.0 * TILE, 12.0 * TILE));
        let mut sight = Sight::new(room, room, TILE, &[], &[]);
        let bags = Rect::from_min_size(vec2(5.0 * TILE, 5.0 * TILE), vec2(TILE, TILE));
        sight.set_cover(&[bags]);
        assert!(sight.cover_at(5, 5) && !sight.cover_at(5, 6));
        // Nothing to sight or to the trace: the tile beyond is seen.
        assert!(sight.clear_line(middle(5.0, 6.0), (5, 3)));

        let behind = middle(5.0, 6.0);
        assert!(
            sight.covered(behind, middle(5.0, 1.0)),
            "from straight across"
        );
        assert!(
            sight.covered(behind, middle(4.0, 1.0)),
            "and from a little off the line"
        );
        assert!(
            !sight.covered(behind, middle(10.0, 6.0)),
            "not from the side"
        );
        assert!(!sight.covered(behind, middle(5.0, 10.0)), "not from behind");
        assert!(
            !sight.covered(middle(5.0, 5.0), middle(5.0, 1.0)),
            "standing on them is the open"
        );
        assert!(
            !sight.covered(middle(5.0, 8.0), middle(5.0, 1.0)),
            "two tiles back is past the reach"
        );
        // Diagonally behind counts too: the sandbags' middle is within the
        // reach and on the line.
        assert!(
            sight.covered(middle(6.0, 6.0), middle(3.0, 3.0)),
            "corner to corner"
        );
        // The shut doors do not wipe the cover: it is part of the fixed picture.
        sight.set_shut(&[Rect::from_min_size(vec2(0.0, 0.0), vec2(TILE, TILE))]);
        assert!(sight.covered(behind, middle(5.0, 1.0)));
    }
}

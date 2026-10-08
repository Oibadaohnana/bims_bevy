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
//! # One fog over the whole map
//!
//! The fog is Dota's (task 128): the whole of a structure — the crew's
//! own ship, a friendly station, a stranger's, a hostile one, a town —
//! is always drawn, and every fogged pixel nobody sees is under the one
//! fog, [`MAP_FOG`], whether or not anybody has ever looked at it. There
//! is no black and no memory of what has been looked at. What the fog
//! hides is **who is standing there**: a body not the crew's is drawn
//! only in sight (`Game::body_seen`), and what the crew see is brighter
//! than the fog — nought over a lit pixel, the dark's shade over an unlit
//! one, both under the fog's.

use crate::draw::{Color, DrawList};
use crate::math::{Rect, Vec2, vec2};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

/// The fog over what nobody sees, on the tile grid (`Fog::All`): the deck
/// under it stays readable, whoever's it is, but whatever is standing
/// there is not drawn. The light map's is [`MAP_FOG`], the same alpha.
const FOG: Color = Color::rgba(0.02, 0.04, 0.03, MAP_FOG);

/// How far a Bim sees in the dark, in tiles: a tile a light does not reach
/// is seen only from this close. Lit tiles are seen as far as the line is
/// clear, up to [`VIEW_RANGE`], and a tile partly lit — the soft rim of a lamp's pool — from
/// further the more it is lit: `DARK_RANGE / (1 − light)`, so a tile half
/// lit is seen from twice as far (task 152: ten tiles, every tile lit or
/// not, before it; fifteen, then eight when the player could still see
/// too far through the dark).
pub const DARK_RANGE: f32 = 8.0;
/// How far anybody sees at all, lit tiles too, in tiles — on every map,
/// by day or night (October 2026, the player's word: "reduce the vision
/// even in light to what you can see"; a little past the game view's
/// [`crate::balance::MAX_RANGE`], so nothing in reach of a gun is hidden).
/// Before, a lit tile was seen as far as the line was clear, forty tiles
/// on a dark map and a plain's `VIEW` (60) out of doors.
pub const VIEW_RANGE: f32 = 14.0;
/// How wide a lamp's soft rim is, in tiles: full light to this short of
/// its reach, then fading smoothly to nothing at the reach, so the edge of
/// a pool is clear — which tiles it lights and which it does not — without
/// the tile grid's hard step. A tile at the middle of the rim counts half
/// lit (task 152). Before, a lamp was full only to a third of its reach
/// and faded the whole way after, with no edge to read.
pub const LIGHT_EDGE: f32 = 2.0;
/// How much of a lamp's light gets past furniture — a shelf, a cabinet,
/// a table — into the shade behind it, on the rule and in the picture
/// alike: the shadow, and it is a light one. Nothing gets past a wall.
const SHADOW_FILL: f32 = 0.45;
/// How much light a bolt in flight throws on the tile it is in, and on
/// the four beside it, for the rule (task 152): a laser lights its way.
const FLARE_ON: u8 = 255;
const FLARE_BESIDE: u8 = 128;

/// How much of a lamp's light falls `d` from it, nought to one, for a lamp
/// of `reach` on a grid of `tile` — [`LIGHT_EDGE`]'s soft rim: one inside
/// it, a smoothstep down to nought at the reach. The rule's tiles and the
/// picture's pixels are both read off this, so the picture's edge is the
/// rule's.
pub fn lamp_fall(d: f32, reach: f32, tile: f32) -> f32 {
    if d >= reach {
        return 0.0;
    }
    let edge = (LIGHT_EDGE * tile).min(reach);
    let t = ((reach - d) / edge).min(1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A light in the room: where it is and how far it reaches, in room units.
/// A tile is **lit** when the straight line from some light to its middle
/// crosses nothing opaque (the walls and the tall parts — never a door,
/// which is not there to a light the way it is to an eye) and is within the
/// light's reach. Lights are always on until they are shot out — see
/// [`Lamp`]. A room never handed any lights is lit throughout — a bare
/// room (`Game::bare`), which has no lighting to speak of — and a
/// designed deck handed none is dark everywhere.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Light {
    pub at: Vec2,
    pub reach: f32,
}

/// What a lamp takes before it goes out: two pistol bolts put it out
/// since the pistol went to 8 a shot with its magazine (three at 6, and
/// two left it failing at 7.2, in October 2026), and a shotgun's or a
/// sniper's one does on its own.
pub const LAMP_HEALTH: f32 = 16.0;
/// Below this share of its health a lamp is **failing**: it flickers now
/// and then, on its own.
pub const LAMP_FAILING: f32 = 0.2;
/// How near a bolt has to pass a lamp to hit it, in room units: the
/// glass, near enough. A bolt aimed down the middle of a gangway misses
/// the lamps on its walls; one that goes wide may not.
pub const LAMP_RADIUS: f32 = 10.0;
/// How near a bolt has to pass a fuel drum to hole it, in room units:
/// most of a tile's width, so a bolt down the middle of a gangway past a
/// drum on its wall flies on and one aimed at a body beside it may not.
pub const TANK_RADIUS: f32 = 18.0;

/// What a crate takes before it is matchwood and no cover: the damage of
/// the bolts it stopped and the bursts that reached it.
pub const CRATE_HEALTH: f32 = 120.0;
/// What a fuel drum takes before it bursts.
pub const TANK_HEALTH: f32 = 40.0;

/// A crate or a fuel drum of the room's layout (`shipdesign::Crate`,
/// `FuelTank`) as the fight has left it: where it stands, its tile's
/// rectangle, which it is, and what it has left. A crate is low cover
/// until it is shot to nothing (the bolts it stopped,
/// `Combat::cover_hits`, and the bursts that reach it), and then its
/// tile is no cover at all; a drum takes the bolts that pass within
/// [`TANK_RADIUS`] and at nothing **bursts** (`Game::burst`). What is
/// broken stays where it stood, walked round as before: the walk never
/// changes, only the cover and the drum. The world remembers the
/// damage by tile (`World::props`), as it does a lamp's.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Prop {
    pub at: Vec2,
    pub rect: Rect,
    /// A fuel drum, else a crate.
    pub tank: bool,
    pub health: f32,
}

impl Prop {
    /// What a whole one of its kind has.
    pub fn whole(tank: bool) -> f32 {
        if tank { TANK_HEALTH } else { CRATE_HEALTH }
    }

    /// Shot to nothing: a crate no cover, a drum burst.
    pub fn is_broken(&self) -> bool {
        self.health <= 0.0
    }
}

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
    /// How bright it is shown, nought to one: one steady, nought dark,
    /// between while it flickers.
    pub level: f32,
    /// Seconds of flicker left.
    flicker: f32,
    /// The last second-long window a failing flicker was rolled for.
    window: u32,
    /// Switched off (task 152): a dark station's, whole and giving no
    /// light — `Sight::set_lamps_off`. Shot at like a lit one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub off: bool,
}

impl Lamp {
    /// Whether it is broken: shot to nothing.
    pub fn is_out(&self) -> bool {
        self.health <= 0.0
    }

    /// Whether it gives no light: out, or switched off.
    pub fn is_dark(&self) -> bool {
        self.is_out() || self.off
    }

    pub fn is_failing(&self) -> bool {
        !self.is_out() && self.health <= LAMP_HEALTH * LAMP_FAILING
    }

    /// Whether it is flickering now: hit a moment ago, or one of a
    /// failing lamp's flickers. What the picture spits sparks off.
    pub fn is_flickering(&self) -> bool {
        !self.is_dark() && self.flicker > 0.0
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
/// in whole tiles from the body's tile to the sandbags' tile, across,
/// down or diagonally: the tile beside the bags or one tile back from
/// them, and no further.
pub const COVER_REACH: i32 = 2;

/// Whose a structure is, to the crew: the world's word, which decides
/// whether its people are at war with the crew. The fog is the same over
/// every stance (task 128).
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
    /// anywhere inside a bare room's box.
    fogged: bool,
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
    /// How much light falls on each tile, nought dark to 255 lit — see
    /// [`Light`], [`lamp_fall`] — and the lights themselves, for the
    /// picture. With no lights, every tile is lit.
    light: Vec<u8>,
    /// The light the bolts in flight throw this step (`set_flares`), a
    /// byte a tile like `light`, and which tiles it is on, so the next
    /// step clears only those. Read by the rule beside the lamps'; the
    /// picture draws a bolt's own glow instead. Saved: what the next step
    /// clears and compares against is the step before's, and a world read
    /// back without them (a guest's resync) traces its mask again where
    /// the original did not.
    #[cfg_attr(feature = "serde", serde(default))]
    flare: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(default))]
    flared: Vec<usize>,
    lights: Vec<Light>,
    /// The lamps, one a light: what each has left and how bright it is
    /// shown. See [`Lamp`].
    lamps: Vec<Lamp>,
    /// Which lamps' health changed since a host last asked
    /// (`take_lamp_changes`).
    lamp_changes: Vec<usize>,
    /// The crates and the fuel drums, and which of them changed since a
    /// host last asked (`take_prop_changes`). See [`Prop`].
    #[cfg_attr(feature = "serde", serde(default))]
    props: Vec<Prop>,
    #[cfg_attr(feature = "serde", serde(default))]
    prop_changes: Vec<usize>,
    /// The lamps' own clock, for the flicker's dice.
    lamp_seconds: f32,
    /// Never handed lights at all: lit throughout. See [`Light`].
    lit_everywhere: bool,
    /// The sky: every tile whose middle lies in here is lit whatever the
    /// lamps say, on the mask and in the picture. The world's word, for a
    /// settlement's ground (`set_daylight`); `None` aboard and on a
    /// station, where every light is a lamp.
    daylight: Option<Rect>,
    /// Night on a planet (task 152): the dark and the fog are drawn
    /// deeper (`MAP_DARK_NIGHT`, `MAP_FOG_NIGHT`). The picture's alone —
    /// the rule reads the light, and night is the sky taken away
    /// (`set_daylight`). The world's to set, kept across a relayout.
    #[cfg_attr(feature = "serde", serde(default))]
    night: bool,
    /// Where the lamps are switched off (task 152): a dark station's
    /// box, every lamp whose middle lies in it giving no light. The
    /// world's word (`set_lamps_off`), kept across a relayout.
    #[cfg_attr(feature = "serde", serde(default))]
    lamps_off: Option<Rect>,
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
    /// Moved with `views_stale`: which cells the views were marched
    /// over, for a picture kept elsewhere — the plain's,
    /// `terrain::Plane::picture` — to tell that the walls, the doors or
    /// the lights have moved under its own views.
    #[cfg_attr(feature = "serde", serde(skip))]
    cells_version: u64,
    /// This sight's name to a host drawing its map (task 121), and what
    /// is kept for that host. Pictures, both, and out of a save.
    #[cfg_attr(feature = "serde", serde(skip))]
    picture: PictureId,
    #[cfg_attr(feature = "serde", serde(skip))]
    host: HostLight,
    /// What the mask was last worked out from: the eyes' tiles and the
    /// shut doors. When these have not moved, neither has the mask.
    eyes_at: Vec<(i32, i32)>,
    /// The shut doors `cells` carries now — see `set_shut`.
    shut: Vec<Rect>,
    /// The low cover as the layout laid it (`set_cover`) and as the
    /// world has laid it since (`set_laid_cover`) — an engineer's
    /// sandbags, feature 74 — kept apart so either can be set again
    /// without losing the other. Both are marked into `fixed` together.
    layout_cover: Vec<Rect>,
    laid_cover: Vec<Rect>,
    /// Whether the doors have moved since the mask was traced.
    stale: bool,
    /// Whether anything has been traced yet. Until it has, nothing is
    /// seen, which is right for a room nobody is looking into.
    traced: bool,
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
            light: vec![255; (columns * rows) as usize],
            flare: Vec::new(),
            flared: Vec::new(),
            lights: Vec::new(),
            lamps: Vec::new(),
            lamp_changes: Vec::new(),
            props: Vec::new(),
            prop_changes: Vec::new(),
            lamp_seconds: 0.0,
            lit_everywhere: true,
            daylight: None,
            night: false,
            lamps_off: None,
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
            cells_version: 0,
            picture: PictureId::default(),
            host: HostLight::default(),
            eyes_at: Vec::new(),
            shut: Vec::new(),
            layout_cover: Vec::new(),
            laid_cover: Vec::new(),
            stale: false,
            traced: false,
        };
        let mut fixed = vec![
            Cell {
                opaque: false,
                fogged: fogged.is_empty(),
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
        self.sees_from_as(from, target, false)
    }

    /// [`Sight::sees_from`] with the dark no object: a clear line is
    /// enough, lit or not, however far. What an engineer's sentry sees
    /// with — a sensor, not an eye — so a machine standing where the
    /// lamps are shot out is no safer from it than one in the light.
    pub fn sees_from_in_the_dark(&self, from: Vec2, target: Vec2) -> Option<Vec2> {
        self.sees_from_as(from, target, true)
    }

    fn sees_from_as(&self, from: Vec2, target: Vec2, in_the_dark: bool) -> Option<Vec2> {
        let tile = self.tile_of(target);
        if !self.inside(tile.0, tile.1) {
            return None;
        }
        if !in_the_dark && !self.in_the_light(from, tile) {
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
        self.cells_version += 1;
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
                level: 1.0,
                flicker: 0.0,
                window: 0,
                off: false,
            })
            .collect();
        self.mark_lamps_off();
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
        self.cells_version += 1;
        // Summed, saturating, as the picture's field is (`sum_fields`):
        // light adds, so a tile between two lamps' rims is lit by both.
        let mut sum = vec![0u32; self.light.len()];
        for (light, lamp) in self.lights.iter().zip(&self.lamps) {
            if lamp.is_dark() {
                continue;
            }
            let (lx, ly) = self.tile_of(light.at);
            let span = (light.reach / self.tile).ceil() as i32 + 1;
            for y in (ly - span).max(0)..=(ly + span).min(self.rows - 1) {
                for x in (lx - span).max(0)..=(lx + span).min(self.columns - 1) {
                    let fall =
                        lamp_fall((self.middle(x, y) - light.at).len(), light.reach, self.tile);
                    if fall <= 0.0 {
                        continue;
                    }
                    // Straight, past nothing in the way; or through the
                    // furniture's shade at [`SHADOW_FILL`]; or not at all.
                    let share = if self.clear_line_over(&self.fixed, light.at, (x, y)) {
                        1.0
                    } else if self
                        .clear_line_by(&self.fixed, light.at, (x, y), |c| c.opaque && !c.soft)
                    {
                        SHADOW_FILL
                    } else {
                        continue;
                    };
                    sum[self.index(x, y)] += (fall * share * 255.0) as u32;
                }
            }
        }
        for (l, s) in self.light.iter_mut().zip(sum) {
            *l = s.min(255) as u8;
        }
        // The sky, after the lamps: a tile under it is lit whether a
        // lamp reaches it or not, and a wall shades nothing from it.
        if let Some(over) = self.daylight {
            for y in 0..self.rows {
                for x in 0..self.columns {
                    if over.contains(self.middle(x, y)) {
                        let i = self.index(x, y);
                        self.light[i] = 255;
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

    /// Every lamp whose middle lies in `over` switched off, or none (task
    /// 152): a dark station — whole, shot at like any, giving no light,
    /// on the mask and in the picture alike. Kept across `set_lights`.
    pub fn set_lamps_off(&mut self, over: Option<Rect>) {
        if self.lamps_off == over {
            return;
        }
        self.lamps_off = over;
        self.mark_lamps_off();
        self.range_moved();
        if !self.lit_everywhere {
            self.relight();
            self.light_field_stale = true;
        }
    }

    pub fn lamps_off(&self) -> Option<Rect> {
        self.lamps_off
    }

    /// Each lamp's switch off `lamps_off`, its level with it.
    fn mark_lamps_off(&mut self) {
        let over = self.lamps_off;
        for lamp in &mut self.lamps {
            lamp.off = over.is_some_and(|r| r.contains(lamp.at));
            lamp.level = if lamp.is_dark() { 0.0 } else { 1.0 };
            lamp.flicker = 0.0;
        }
    }

    /// Night or not, for the picture (task 152): the whole map is composed
    /// again at the next look with the night's dark and fog.
    pub fn set_night(&mut self, night: bool) {
        if self.night != night {
            self.night = night;
            self.range_moved();
            self.map_stale = true;
            // Built again whole, so a host drawing it takes it up too.
            self.light_field_stale = true;
        }
    }

    pub fn night(&self) -> bool {
        self.night
    }

    /// The daylight, as set.
    /// How far an eye sees at all, or `None` for as far as the line is
    /// clear. See `range`. The mask is traced again on the next look.
    pub fn set_range(&mut self, range: Option<f32>) {
        self.range = range;
        self.range_moved();
    }

    /// The eyes' reach changed: the mask traced again, the views marched
    /// again and the picture composed again at the next look.
    fn range_moved(&mut self) {
        self.stale = true;
        self.views_stale = true;
        self.cells_version += 1;
        self.map_stale = true;
    }

    pub fn range(&self) -> Option<f32> {
        self.range
    }

    /// How far an eye sees at all, as the rule and the picture read it:
    /// the range set, held to [`VIEW_RANGE`] lit or dark.
    pub fn view_range(&self) -> Option<f32> {
        let cap = VIEW_RANGE * self.tile;
        Some(self.range.map_or(cap, |r| r.min(cap)))
    }

    /// Which cells a line of sight is read over now: a number that moves
    /// whenever the walls, the tall parts, the lights or the shut doors
    /// do, so a picture marched over them can tell it is stale.
    pub fn cells_version(&self) -> u64 {
        self.cells_version
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

    /// The layout's crates and fuel drums, `(the tile's rectangle, a
    /// drum)`, every one whole: once a layout, like the lights. What a
    /// fight did to one is the world's to put back (`set_prop_health`).
    pub fn set_props(&mut self, props: &[(Rect, bool)]) {
        self.props = props
            .iter()
            .map(|&(rect, tank)| Prop {
                at: rect.center(),
                rect,
                tank,
                health: Prop::whole(tank),
            })
            .collect();
        self.prop_changes.clear();
        self.mark_cover();
    }

    /// The crates and the drums, index for index with the layout's.
    pub fn props(&self) -> &[Prop] {
        &self.props
    }

    /// The crate or the drum on a tile, with its index.
    pub fn prop_on(&self, tile: (i32, i32)) -> Option<usize> {
        self.props.iter().position(|p| self.tile_of(p.at) == tile)
    }

    /// Prop `i` takes `damage`: true when this is what broke it — a
    /// crate's tile no longer cover, a drum to burst.
    pub fn damage_prop(&mut self, i: usize, damage: f32) -> bool {
        let Some(prop) = self.props.get_mut(i) else {
            return false;
        };
        if prop.is_broken() || damage <= 0.0 {
            return false;
        }
        prop.health = (prop.health - damage).max(0.0);
        self.prop_changes.push(i);
        if self.props[i].is_broken() {
            if !self.props[i].tank {
                self.mark_cover();
            }
            true
        } else {
            false
        }
    }

    /// Prop `i` as the world remembers it, set outright: at the room's
    /// building, or from the other room a fight is mirrored in.
    pub fn set_prop_health(&mut self, i: usize, health: f32) {
        let Some(prop) = self.props.get_mut(i) else {
            return;
        };
        if prop.health == health {
            return;
        }
        let was = prop.is_broken();
        prop.health = health.max(0.0);
        if prop.is_broken() != was && !prop.tank {
            self.mark_cover();
        }
    }

    /// Which props' health changed since this was last asked, oldest
    /// first: the world's cue to remember it.
    pub fn take_prop_changes(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.prop_changes)
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

    /// Whether the tile a point is in counts as lit: half its light or
    /// more, a lamp's, the sky's or a bolt's.
    pub fn lit_at(&self, p: Vec2) -> bool {
        self.light_at(p) >= 0.5
    }

    /// How much light falls on the tile a point is in, nought to one —
    /// the lamps', the sky's and the bolts' in flight. Off the grid is
    /// dark.
    pub fn light_at(&self, p: Vec2) -> f32 {
        let (x, y) = self.tile_of(p);
        if !self.inside(x, y) {
            return 0.0;
        }
        self.tile_light(self.index(x, y)) as f32 / 255.0
    }

    /// How much of the lamps' and the sky's light falls on the tile a
    /// point is in, nought to one — the bolts' own left out: how dark it
    /// is round a bolt, for its glow (`Game::draw_bolt_glow`).
    pub fn lamplight_at(&self, p: Vec2) -> f32 {
        let (x, y) = self.tile_of(p);
        if !self.inside(x, y) {
            return 0.0;
        }
        self.light[self.index(x, y)] as f32 / 255.0
    }

    /// A tile's light as the rule reads it: the lamps' and the sky's, or
    /// a bolt's if that is more.
    fn tile_light(&self, i: usize) -> u8 {
        self.light[i].max(self.flare.get(i).copied().unwrap_or(0))
    }

    /// The bolts in flight this step, where each is (task 152): a laser
    /// lights the tile it is in fully and the four beside it half — no
    /// further through a wall — for the rule to read, so a body a bolt
    /// passes in the dark is seen as far as a lit one. The picture draws
    /// each bolt's own glow (`Combat::draw`). Called every step; the mask
    /// is traced again only when the tiles lit changed.
    pub fn set_flares(&mut self, bolts: &[Vec2]) {
        let mut lit: Vec<(usize, u8)> = Vec::new();
        for &p in bolts {
            let (x, y) = self.tile_of(p);
            if !self.inside(x, y) {
                continue;
            }
            lit.push((self.index(x, y), FLARE_ON));
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if self.inside(nx, ny) && !self.cells[self.index(nx, ny)].opaque {
                    lit.push((self.index(nx, ny), FLARE_BESIDE));
                }
            }
        }
        if lit.is_empty() && self.flared.is_empty() {
            return;
        }
        if self.flare.len() != self.light.len() {
            self.flare = vec![0; self.light.len()];
        }
        let before: Vec<(usize, u8)> = self.flared.iter().map(|&i| (i, self.flare[i])).collect();
        for &i in &self.flared {
            self.flare[i] = 0;
        }
        self.flared.clear();
        for (i, v) in lit {
            if self.flare[i] == 0 {
                self.flared.push(i);
            }
            self.flare[i] = self.flare[i].max(v);
        }
        let now: Vec<(usize, u8)> = self.flared.iter().map(|&i| (i, self.flare[i])).collect();
        if before != now {
            self.stale = true;
        }
    }

    /// Whether an eye at `from` could make out a body at `at` at all —
    /// [`Sight::in_the_light`] for a point, the line not asked about.
    /// What a Trooper's stand is weighed by: a spot it would not see its
    /// target from in the dark is no spot to shoot from.
    pub fn makes_out(&self, from: Vec2, at: Vec2) -> bool {
        let tile = self.tile_of(at);
        self.inside(tile.0, tile.1) && self.in_the_light(from, tile)
    }

    /// Whether an eye at `from` can make the tile out at all: within
    /// [`DARK_RANGE`] of the eye in the dark, and as much further as the
    /// tile is lit — `DARK_RANGE / (1 − light)`, so a lit tile is seen
    /// out to [`VIEW_RANGE`] and one half lit from twice as far. The dark rule, on
    /// top of the line being clear.
    fn in_the_light(&self, from: Vec2, tile: (i32, i32)) -> bool {
        let away = (self.middle(tile.0, tile.1) - from).len();
        if self.view_range().is_some_and(|r| away > r) {
            return false;
        }
        let dark = 255 - self.tile_light(self.index(tile.0, tile.1)) as u32;
        away * dark as f32 <= DARK_RANGE * self.tile * 255.0
    }

    /// Mark these rectangles as low cover — sandbags: nothing to sight or
    /// to a walk, but a body within [`COVER_REACH`] behind one, on the
    /// side a bolt comes from, ducks under it. Part of the fixed picture,
    /// so it survives every `set_shut`. The layout's; whatever the world
    /// has laid since (`set_laid_cover`) stays marked beside it.
    pub fn set_cover(&mut self, cover: &[Rect]) {
        self.layout_cover = cover.to_vec();
        self.mark_cover();
    }

    /// The low cover laid at run time — an engineer's deployed sandbags
    /// (feature 74) — over and above the layout's, the whole list every
    /// time: what is not on it any more is unmarked. A fresh `Sight` has
    /// none, so the world says it again after every relayout, join and
    /// unjoin. Nothing to do when the list is what it was.
    pub fn set_laid_cover(&mut self, laid: &[Rect]) {
        if self.laid_cover == laid {
            return;
        }
        self.laid_cover = laid.to_vec();
        self.mark_cover();
    }

    /// The cover laid at run time, as last set.
    pub fn laid_cover(&self) -> &[Rect] {
        &self.laid_cover
    }

    fn mark_cover(&mut self) {
        let mut fixed = std::mem::take(&mut self.fixed);
        for c in fixed.iter_mut() {
            c.cover = false;
        }
        // A crate shot to nothing is no cover: its tile is left out.
        let rects: Vec<Rect> = self
            .layout_cover
            .iter()
            .filter(|r| {
                !self
                    .props
                    .iter()
                    .any(|p| !p.tank && p.is_broken() && p.rect == **r)
            })
            .chain(self.laid_cover.iter())
            .copied()
            .collect();
        for rect in &rects {
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
        self.cells_version += 1;
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
        self.cover_between(body, from).is_some()
    }

    /// [`Sight::covered`] with the tile of cover that does it, for the
    /// bolt it stopped to be taken off the bags (`Combat::cover_hits`).
    pub fn cover_between(&self, body: Vec2, from: Vec2) -> Option<(i32, i32)> {
        self.cover_along(body, from, Some(COVER_REACH))
    }

    /// The same at any distance: whether sandbags lie anywhere on the
    /// line from `body` to `from`, and where. What a sentry *dug in*
    /// asks (feature 74), for which bags across the room count as
    /// cover; a body wants them within reach.
    pub fn cover_anywhere_between(&self, body: Vec2, from: Vec2) -> Option<(i32, i32)> {
        self.cover_along(body, from, None)
    }

    fn cover_along(&self, body: Vec2, from: Vec2, reach: Option<i32>) -> Option<(i32, i32)> {
        let (mut x, mut y) = self.tile_of(body);
        let (bx, by) = (x, y);
        let (tx, ty) = self.tile_of(from);
        if (x, y) == (tx, ty) || self.cover_at(x, y) {
            return None;
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
            if t >= 1.0
                || (x, y) == (tx, ty)
                || reach.is_some_and(|reach| (x - bx).abs().max((y - by).abs()) > reach)
            {
                return None;
            }
            if self.cover_at(x, y) {
                return Some((x, y));
            }
        }
        None
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
        self.clear_line_by(cells, from, to, |c| c.opaque)
    }

    /// The same, stopped wherever `stops` says: a lamp's shade behind the
    /// furniture reads the walls alone.
    fn clear_line_by(
        &self,
        cells: &[Cell],
        from: Vec2,
        to: (i32, i32),
        stops: impl Fn(&Cell) -> bool,
    ) -> bool {
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
            if !self.inside(x, y) || stops(&cells[self.index(x, y)]) {
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

    /// Whether the tile a point is in is under the fog, as of the last
    /// trace — fogged and nobody sees it, or, with `all`, fogged at all
    /// (a room nobody is looking into). Off the grid is never.
    pub fn hidden_at(&self, p: Vec2, all: bool) -> bool {
        let (x, y) = self.tile_of(p);
        self.inside(x, y) && self.veiled(self.index(x, y), all)
    }

    /// What is drawn over the tile a point is in, as of the last trace:
    /// 0 nothing, 1 the fog — the one veil there is, whoever's the tile
    /// is and whether or not anybody has looked at it. For the probes.
    pub fn veil_at(&self, p: Vec2) -> u32 {
        let (x, y) = self.tile_of(p);
        if !self.inside(x, y) {
            return 0;
        }
        self.veiled(self.index(x, y), false) as u32
    }

    /// Whether the fog is over a tile: fogged, and nobody sees it. With
    /// `all`, nobody sees anything.
    fn veiled(&self, i: usize, all: bool) -> bool {
        self.cells[i].fogged && (all || !self.seen[i])
    }

    /// The fog on the tile grid, over every fogged tile nobody sees — or,
    /// with `all`, over every fogged tile, for a room nobody is looking
    /// into: the one fog colour, whoever's the tiles are, so the
    /// structure shows through and nobody standing in it is drawn.
    /// Through the crew's own eyes the picture is the [`LightMap`]'s and
    /// this is not drawn. Neighbouring tiles are drawn as one rectangle
    /// wherever they can be, so the fog is a few shapes rather than a
    /// thousand, and a run that matches the run under it is one shape
    /// taller.
    pub fn draw(&self, list: &mut DrawList, all: bool) {
        self.draw_runs(list, FOG, |i| self.veiled(i, all));
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
        // Edge to edge: the fog is translucent, and an overlap would be
        // twice as dark along the seam.
        list.rect(min + size * 0.5, size, 0.0, 0.0, colour);
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
pub(crate) const RAYS: u32 = 4096;
/// The fog over everything the crew do not see — their own deck, a
/// stranger's, the plain — whether or not anybody has looked at it: the
/// structure shows through it, and no body does (task 128).
pub(crate) const MAP_FOG: f32 = 0.72;
/// The dark over what they see that no light reaches — deep enough that
/// a lamp's pool reads against it, short of the fog so what is seen is
/// always brighter than what is not. Both went up with task 152 (from
/// 0.62 and 0.50), so the edge of a pool is plain to see; then the two
/// were pulled apart (0.66 and 0.56 before), so the edge of how far a
/// body sees in the dark — [`DARK_RANGE`] — is plain to see too: what
/// is seen and unlit keeps nearly twice the light of the fog, where it
/// kept a third more and the ring of sight was lost in the dark.
pub(crate) const MAP_DARK: f32 = 0.48;
/// The same two at night on a planet (task 152): the open ground under
/// no sky is darker than a station's corridor, and what nobody sees is
/// darker still — a night, where the lamps' pools are the town. Pulled
/// apart with the day's (0.86 and 0.78 before, the seen ground barely
/// over the fog): what is seen keeps three times the fog's light, so
/// the eight tiles round a body read as a ring in the night.
pub(crate) const MAP_FOG_NIGHT: f32 = 0.90;
pub(crate) const MAP_DARK_NIGHT: f32 = 0.66;
/// The lamplight over a lit pixel at full brightness: the warm wash the
/// host tints the deck with, nought to one, faded with the light — and
/// how much of it shows through the fog over what the crew know, since
/// the lamps are always on and the crew know where they hang.
const GLOW: f32 = 0.28;
const GLOW_UNDER_FOG: f32 = 0.5;

/// The fog's alpha and the dark's, by day or by night (task 152).
pub(crate) fn fog_alpha(night: bool) -> f32 {
    if night { MAP_FOG_NIGHT } else { MAP_FOG }
}

pub(crate) fn dark_alpha(night: bool) -> f32 {
    if night { MAP_DARK_NIGHT } else { MAP_DARK }
}

/// The smooth picture of the crew's sight and the lamps: two bytes a
/// pixel — the darkness to draw over the room, nought where the crew see
/// a lit tile, the dark's where they see an unlit one, the fog's where
/// they see nothing, whoever's the deck is; and the lamplight, the warm
/// wash where a light falls. Marched,
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
    /// For a host that draws the map itself (`set_host_draws`, task
    /// 121): what to draw it from. The two planes are then empty unless
    /// the host is checking; the extent above is kept either way.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub inputs: Option<Arc<LightInputs>>,
}

impl LightMap {
    /// The map's extent in room units.
    pub fn size(&self) -> Vec2 {
        vec2(self.width as f32 * self.px, self.height as f32 * self.px)
    }
}

// --- a host that draws the light map itself (task 121) ---------------------
//
// The march and the composing can be done by the host
// on its GPU, from the same numbers: `set_host_draws` says so, and then
// `Sight::light_map` works out only what the host needs — which bodies'
// eyes moved, from where each eye's rays start, the cells, the lamps'
// light, the tables the composing reads — and hands it over as
// `LightMap::inputs`, leaving the two planes empty. The host's picture is
// the one `light_map_on_cpu` makes, byte for byte: the rays' starts are
// divided here, so the walk the host does is additions and comparisons;
// every float the composing reads is a table made here. The rule — the
// tile mask the fight reads — is not in any of this and never moves.

/// Whether the host draws the light map: not at all, instead of this
/// crate, or beside it for checking the two against each other.
const HOST_NO: u8 = 0;
const HOST_YES: u8 = 1;
const HOST_CHECKING: u8 = 2;
static HOST_DRAWS: AtomicU8 = AtomicU8::new(HOST_NO);

/// Say whether the host draws the crew's light map on its own
/// (`on`), and whether this crate is to work it out as well so the two
/// can be compared (`checking`). Off — which is every test, every probe
/// and every host that does not ask — the map is worked out here as it
/// always was. A picture's switch: nothing the simulation reads moves.
pub fn set_host_draws(on: bool, checking: bool) {
    let mode = match (on, checking) {
        (false, _) => HOST_NO,
        (true, false) => HOST_YES,
        (true, true) => HOST_CHECKING,
    };
    HOST_DRAWS.store(mode, Ordering::Relaxed);
}

fn host_draws() -> u8 {
    HOST_DRAWS.load(Ordering::Relaxed)
}

/// A name for one [`Sight`] as the host keeps its picture: every sight
/// made, cloned or read from a save has a name of its own, so a host
/// holding the views and the revisions of one never takes another's
/// inputs for a change to them.
#[derive(Debug)]
pub struct PictureId(pub u64);

impl PictureId {
    fn fresh() -> PictureId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        PictureId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for PictureId {
    fn default() -> PictureId {
        PictureId::fresh()
    }
}

impl Clone for PictureId {
    fn clone(&self) -> PictureId {
        PictureId::fresh()
    }
}

/// A cell as the host's composing reads it, a byte a tile: a line of
/// sight stops there, the fog is drawn over it.
pub const CELL_OPAQUE: u8 = 1;
pub const CELL_FOGGED: u8 = 2;
/// And for the host's softening alone (task 140) — nothing it marches or
/// composes the fog by: the cell is in the way in the fixed picture, the
/// walls and the tall parts and no door (what a lamp's light stops at),
/// and it is furniture rather than wall (`set_tall`).
pub const CELL_FIXED: u8 = 4;
pub const CELL_SOFT: u8 = 8;

/// Everything a host needs to draw the crew's light map itself, as of one
/// frame (see the note above `set_host_draws`). The large parts are
/// shared and made again only when they change; each carries a revision
/// that moves when it does, so a host that missed a frame catches up by
/// comparing, not by counting.
#[derive(Clone, Debug)]
pub struct LightInputs {
    /// Which sight this is ([`PictureId`]).
    pub sight: u64,
    /// The map, in pixels, and the grid under it, in tiles.
    pub width: usize,
    pub height: usize,
    pub columns: usize,
    pub rows: usize,
    /// A byte a tile: [`CELL_OPAQUE`], [`CELL_FOGGED`].
    pub cells: Arc<Vec<u8>>,
    pub cells_rev: u64,
    /// The lamps' light a pixel: as the eyes read it (every lamp not out
    /// at full) and as it is shown (with the flicker in).
    pub light_field: Arc<Vec<u8>>,
    pub shown_field: Arc<Vec<u8>>,
    pub fields_rev: u64,
    /// The composing's tables, a byte for each of the 256 levels of the
    /// shown light: the dark over what is seen, the lamplight over it,
    /// and the lamplight under the fog; and the fog's own.
    pub dark: [u8; 256],
    pub glow_seen: [u8; 256],
    pub glow_fog: [u8; 256],
    pub fog: u8,
    /// The march's dark rule, [`dark_reach`]: for each darkness (255
    /// less the light the eyes read), how far from its body — squared,
    /// in pixels — a pixel that dark is seen.
    pub reach: [u32; 256],
    /// One a body whose eyes the picture is through, in the bodies'
    /// order.
    pub views: Vec<Arc<ViewInputs>>,
    /// Beside a host that is checking: the map this crate worked out
    /// itself this frame — darkness and lamplight.
    pub cpu: Option<Arc<(Vec<u8>, Vec<u8>)>>,
}

/// One body's eyes, as last marched: a revision that moves whenever they
/// are marched again, the map pixel the body stands in — what the dark
/// rule is measured from ([`dark_reach`]) — and each eye.
#[derive(Clone, Debug, Default)]
pub struct ViewInputs {
    pub rev: u64,
    pub body: (i32, i32),
    pub eyes: Vec<EyeInputs>,
}

/// One eye's fan of rays, ready to walk: the pixel it starts in, the tile
/// the eye is in and whether that tile stops a line of sight (an eye
/// pressed to a wall gets out of it), how far a ray goes in pixels, the
/// peek's wall (its body's tile and direction) if it is a peek, and each
/// ray's first two edge crossings — the one division a ray makes, made
/// here. See [`ray_table`] for the rest of a ray.
#[derive(Clone, Debug, Default)]
pub struct EyeInputs {
    pub start: (i32, i32),
    pub from_tile: (i32, i32),
    pub from_opaque: bool,
    pub far: f32,
    pub beyond: Option<((i32, i32), (i32, i32))>,
    pub t0: Vec<[f32; 2]>,
}

/// Every ray of a fan as [`march_rays`] walks it: its direction, and how
/// far along it one pixel's step across and one down are — infinite for
/// a ray that never crosses that way.
pub fn ray_table() -> Vec<[f32; 4]> {
    ray_directions()
        .iter()
        .map(|&(dx, dy)| {
            let across = if dx.abs() > 1e-6 {
                1.0 / dx.abs()
            } else {
                f32::INFINITY
            };
            let down = if dy.abs() > 1e-6 {
                1.0 / dy.abs()
            } else {
                f32::INFINITY
            };
            [dx, dy, across, down]
        })
        .collect()
}

/// Where each ray of a fan from `(ox, oy)` — in pixels — first crosses a
/// pixel edge across and down, as [`march_rays`] works it out.
fn ray_starts(ox: f32, oy: f32) -> Vec<[f32; 2]> {
    let x = ox.floor() as i32;
    let y = oy.floor() as i32;
    ray_directions()
        .iter()
        .map(|&(dx, dy)| {
            let t_x = if dx.abs() > 1e-6 {
                let next = if dx > 0.0 { x + 1 } else { x } as f32;
                (next - ox) / dx
            } else {
                f32::INFINITY
            };
            let t_y = if dy.abs() > 1e-6 {
                let next = if dy > 0.0 { y + 1 } else { y } as f32;
                (next - oy) / dy
            } else {
                f32::INFINITY
            };
            [t_x, t_y]
        })
        .collect()
}

/// What this crate keeps for a host that draws the map: the eyes it last
/// handed over for each body and the parts it shares, so each is made
/// again only when it changed.
#[derive(Clone, Debug, Default)]
struct HostLight {
    eyes: Vec<Vec<Eye>>,
    views: Vec<Arc<ViewInputs>>,
    revs: u64,
    cells: Arc<Vec<u8>>,
    cells_rev: u64,
    light_field: Arc<Vec<u8>>,
    shown_field: Arc<Vec<u8>>,
    fields_rev: u64,
    cells_version: u64,
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
    /// dark. Full brightness to [`LIGHT_EDGE`] short of the reach, then
    /// the soft rim ([`lamp_fall`], the rule's own). Each lamp's fall is kept on its own ([`LampField`]) and
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
                    |i, _, d| {
                        let (x, y) = (i % w, i / w);
                        if x < x0 || x >= x1 || y < y0 || y >= y1 {
                            return;
                        }
                        let bright = lamp_fall(d, light.reach, self.tile);
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
    /// grid, or has gone `reach`. The walk itself is [`march_rays`].
    ///
    /// `f` is taken by type and not as a `&mut dyn FnMut`: it is called
    /// once a *pixel* — some hundreds of thousands of times for one pair
    /// of eyes on a station's deck — and an indirect call there was a
    /// third of what a fight's frame cost (feature 96).
    fn march<F: FnMut(usize, (i32, i32), f32)>(
        &self,
        from: Vec2,
        reach: Option<f32>,
        cells: &[Cell],
        through_soft: bool,
        mut f: F,
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
        let opaque = |tx: i32, ty: i32| stops(&cells[self.index(tx, ty)]);
        march_rays(
            (ox, oy),
            far,
            (w, h),
            (fx, fy),
            from_opaque,
            opaque,
            |i, tile, t| f(i, tile, t * px),
        );
    }

    /// The map pixel a body stands in, which the picture's dark rule is
    /// measured from.
    fn body_pixel(&self, body: Vec2) -> (i32, i32) {
        let px = self.tile / MAP_PX_PER_TILE as f32;
        (
            ((body.x - self.origin.x) / px).floor() as i32,
            ((body.y - self.origin.y) / px).floor() as i32,
        )
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
        seen.fill(false);
        let reach = dark_reach();
        let w = self.map_dims().0;
        let (bx, by) = self.body_pixel(body);
        let mut reached: Option<Box> = None;
        for eye in eyes {
            // A peek adds only what lies past its wall; the body's own eyes
            // everything. The dark rule is measured from the body, as the
            // trace measures it, a pixel at a time: within the dark range
            // as far as the pixel's light stretches it (`dark_reach`).
            self.march(
                eye.at,
                self.view_range(),
                &self.cells,
                false,
                |i, tile, _| {
                    if !seen[i] && eye.admits(tile) && {
                        let (dx, dy) = ((i % w) as i64 - bx as i64, (i / w) as i64 - by as i64);
                        (dx * dx + dy * dy) as u64
                            <= reach[255 - self.light_field[i] as usize] as u64
                    } {
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
                },
            );
        }
        reached
    }

    /// The picture of the mask from these bodies' eyes — worked out here
    /// ([`Sight::light_map_on_cpu`]), or, for a host that draws it itself
    /// (`set_host_draws`, task 121), what the host draws it from
    /// ([`LightMap::inputs`]), or both for a host checking the two. True
    /// when the map changed, which is when its `version` moved.
    pub fn light_map(&mut self, bodies: &[Vec2]) -> bool {
        match host_draws() {
            HOST_YES => self.light_inputs(bodies, false),
            HOST_CHECKING => {
                let changed = self.light_map_on_cpu(bodies);
                self.light_inputs(bodies, true);
                changed
            }
            _ => self.light_map_on_cpu(bodies),
        }
    }

    /// The cells as the host is handed them: a byte a tile, the flags
    /// the light map's passes read.
    fn cell_bytes(&self) -> Vec<u8> {
        self.cells
            .iter()
            .zip(&self.fixed)
            .map(|(c, f)| {
                let bit = |on: bool, bit: u8| if on { bit } else { 0 };
                bit(c.opaque, CELL_OPAQUE)
                    | bit(c.fogged, CELL_FOGGED)
                    | bit(f.opaque, CELL_FIXED)
                    | bit(f.opaque && f.soft, CELL_SOFT)
            })
            .collect()
    }

    /// This sight takes the place of `shown`, the one drawn until now —
    /// a guest's rollback (task 156): a copy of the host's world put where
    /// its own guess was. What was handed to the host's light map is
    /// `shown`'s, so it is carried over and every revision goes on
    /// climbing from it: the cells and the lamps' fields are compared
    /// with what the host holds and handed over again where they differ
    /// (a view with them, its eyes being marched over other cells), and
    /// the CPU's map is composed afresh a version past `shown`'s, which
    /// the host takes whole. Picture only; nothing a step reads.
    pub fn adopt_picture(&mut self, shown: &mut Sight) {
        self.host = std::mem::take(&mut shown.host);
        self.picture = PictureId(shown.picture.0);
        if *self.host.cells == self.cell_bytes() {
            self.host.cells_version = self.cells_version;
        } else {
            self.host.cells_version = self.cells_version.wrapping_add(1);
        }
        if !self.light_field_stale
            && (*self.host.light_field != self.light_field
                || *self.host.shown_field != self.shown_field)
        {
            self.light_field_stale = true;
        }
        self.map.version = shown.map.version.wrapping_add(2);
        self.map.changed = None;
        self.map_stale = true;
    }

    /// What a host drawing the map needs this frame (task 121): the
    /// lamps' fields built as `light_map_on_cpu` builds them, each body's
    /// eyes compared with the ones last handed over by the same rule
    /// (`view_holds`) and handed over again only when they moved — with
    /// each ray's first crossings divided here — and the cells and the
    /// tables. Checking, the planes `light_map_on_cpu` just made go
    /// alongside, and nothing it keeps is touched here.
    fn light_inputs(&mut self, bodies: &[Vec2], checking: bool) -> bool {
        let mut fields_changed = false;
        if !checking {
            if self.light_field.is_empty() || self.light_field_stale {
                self.build_light_fields();
                self.light_field_stale = false;
                fields_changed = true;
            }
            // Read nowhere else when the host draws: a lamp that
            // flickered is a field changed.
            if self.field_dirty.take().is_some() {
                fields_changed = true;
            }
        } else if *self.host.light_field != self.light_field
            || *self.host.shown_field != self.shown_field
        {
            fields_changed = true;
        }
        let (w, h) = self.map_dims();
        let mut changed = false;
        if fields_changed || self.host.light_field.len() != w * h {
            self.host.light_field = Arc::new(self.light_field.clone());
            self.host.shown_field = Arc::new(self.shown_field.clone());
            self.host.fields_rev += 1;
            changed = true;
        }
        // The cells: a door, the layout. Made again only when their version
        // moved — everything that changes a cell moves it (`set_shut`,
        // `set_tall`, `relight`, `set_range`) — or for a sight the host has
        // none of yet, and compared, since they are a byte a tile. They
        // were made afresh every frame until task 140 put two bits more in.
        if self.host.cells.len() != self.cells.len()
            || self.host.cells_version != self.cells_version
        {
            let cells = self.cell_bytes();
            if *self.host.cells != cells {
                self.host.cells = Arc::new(cells);
                self.host.cells_rev += 1;
                changed = true;
            }
        }
        // Every view is marched again whenever `light_map_on_cpu` would
        // throw its views away — a door, the layout, a lamp going out, the
        // range — which is whenever the cells' version moved with it.
        if self.host.cells_version != self.cells_version {
            self.host.cells_version = self.cells_version;
            self.host.eyes.clear();
            self.host.views.clear();
            changed = true;
        }
        if self.host.views.len() > bodies.len() {
            self.host.views.truncate(bodies.len());
            self.host.eyes.truncate(bodies.len());
            changed = true;
        }
        for (b, &body) in bodies.iter().enumerate() {
            let eyes = self.eyes_from(body);
            if self.host.eyes.get(b).is_some_and(|held| {
                held.len() == eyes.len()
                    && held.iter().zip(&eyes).all(|(a, e)| {
                        a.beyond == e.beyond
                            && (a.at - e.at).len() < self.tile / MAP_PX_PER_TILE as f32 * 0.5
                    })
            }) {
                continue;
            }
            self.host.revs += 1;
            let view = Arc::new(self.view_inputs(body, &eyes, self.host.revs));
            if b < self.host.views.len() {
                self.host.views[b] = view;
                self.host.eyes[b] = eyes;
            } else {
                self.host.views.push(view);
                self.host.eyes.push(eyes);
            }
            changed = true;
        }
        let wash = if self.lit_everywhere { 0.0 } else { GLOW };
        let mut dark = [0u8; 256];
        let mut glow_seen = [0u8; 256];
        let mut glow_fog = [0u8; 256];
        for level in 0..256 {
            // `compose`'s own arithmetic, a level at a time.
            let light = level as u8 as f32 / 255.0;
            dark[level] = (dark_alpha(self.night) * (1.0 - light) * 255.0) as u8;
            glow_seen[level] = (wash * light * 255.0) as u8;
            glow_fog[level] = (wash * GLOW_UNDER_FOG * light * 255.0) as u8;
        }
        let px = self.tile / MAP_PX_PER_TILE as f32;
        let cpu = checking.then(|| Arc::new((self.map.alpha.clone(), self.map.glow.clone())));
        self.map.inputs = Some(Arc::new(LightInputs {
            sight: self.picture.0,
            width: w,
            height: h,
            columns: self.columns as usize,
            rows: self.rows as usize,
            cells: self.host.cells.clone(),
            cells_rev: self.host.cells_rev,
            light_field: self.host.light_field.clone(),
            shown_field: self.host.shown_field.clone(),
            fields_rev: self.host.fields_rev,
            dark,
            glow_seen,
            glow_fog,
            fog: (fog_alpha(self.night) * 255.0) as u8,
            reach: dark_reach(),
            views: self.host.views.clone(),
            cpu,
        }));
        if !checking {
            self.map_stale = false;
            self.map.origin = self.origin;
            self.map.px = px;
            self.map.width = w;
            self.map.height = h;
            self.map.changed = None;
            if changed {
                self.map.version += 1;
            }
        }
        changed
    }

    /// One body's eyes as the host walks them: `march`'s arithmetic up to
    /// the walk, and the pixel the body is in, which `view_of`'s dark rule
    /// is measured from.
    fn view_inputs(&self, body: Vec2, eyes: &[Eye], rev: u64) -> ViewInputs {
        let (w, h) = self.map_dims();
        let (w, h) = (w as i32, h as i32);
        let px = self.tile / MAP_PX_PER_TILE as f32;
        let far = self
            .view_range()
            .map(|r| r / px)
            .unwrap_or((w.max(h) as f32) * 1.5);
        let eyes = eyes
            .iter()
            .map(|eye| {
                let ox = (eye.at.x - self.origin.x) / px;
                let oy = (eye.at.y - self.origin.y) / px;
                let (fx, fy) = self.tile_of(eye.at);
                EyeInputs {
                    start: (ox.floor() as i32, oy.floor() as i32),
                    from_tile: (fx, fy),
                    from_opaque: self.inside(fx, fy) && self.cells[self.index(fx, fy)].opaque,
                    far,
                    beyond: eye.beyond,
                    t0: ray_starts(ox, oy),
                }
            })
            .collect();
        ViewInputs {
            rev,
            body: self.body_pixel(body),
            eyes,
        }
    }

    /// The picture of the mask from these bodies' eyes, worked out again
    /// only where it has to be: a body that has not moved since it was
    /// last marched keeps its view, one that has is marched afresh, and
    /// every one is when the cells changed — a door, the layout — and the
    /// picture is composed again over the box the changed views cover
    /// and nowhere else. True when the map changed, which is when its
    /// `version` moved. See [`LightMap`].
    fn light_map_on_cpu(&mut self, bodies: &[Vec2]) -> bool {
        if self.light_field.is_empty() || self.light_field_stale {
            self.build_light_fields();
            self.light_field_stale = false;
            self.field_dirty = None;
            self.map_stale = true;
        }
        let (w, h) = self.map_dims();
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
        let views_timed = crate::timing::scope(crate::timing::Part::LightViews);
        let mut marched = 0;
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
            crate::timing::marched();
            marched += 1;
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
        drop(views_timed);
        if marched > 0 {
            crate::timing::marched_together(marched);
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

    /// Put the views and the light field together into the
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
        // No lamps at all — a bare room — is lit and has no lamplight
        // to wash the deck with.
        let wash = if self.lit_everywhere { 0.0 } else { GLOW };
        let dark = dark_alpha(self.night);
        let fog = (fog_alpha(self.night) * 255.0) as u8;
        let (alpha, glow) = (&mut self.map.alpha, &mut self.map.glow);
        for ty in over.y0 / n..=over.y1 / n {
            for tx in over.x0 / n..=over.x1 / n {
                let c = &self.cells[ty * self.columns as usize + tx];
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
                            alpha[i] = (dark * (1.0 - light) * 255.0) as u8;
                            glow[i] = (wash * light * 255.0) as u8;
                        } else {
                            // Nobody sees it: the one fog, whoever's it is
                            // and whether or not anybody ever looked.
                            alpha[i] = fog;
                            glow[i] = (wash * GLOW_UNDER_FOG * light * 255.0) as u8;
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

/// The picture's dark rule, a squared distance in map pixels for each
/// darkness — 255 less the light the eyes read at the pixel: how far from
/// the body a pixel that dark is made out. [`DARK_RANGE`] in the dark,
/// `DARK_RANGE / (1 − light)` beside a lamp — [`Sight::in_the_light`]'s
/// rule — and anywhere in full light. Integers, so the host's march
/// (`lightmap.wgsl`, task 121) reads the same table and agrees to the
/// pixel.
pub fn dark_reach() -> [u32; 256] {
    let range = (DARK_RANGE * MAP_PX_PER_TILE as f32) as f64 * 255.0;
    let mut reach = [u32::MAX; 256];
    for (dark, r) in reach.iter_mut().enumerate().skip(1) {
        *r = (range / dark as f64).powi(2).min(u32::MAX as f64) as u32;
    }
    reach
}

/// Walk [`RAYS`] rays out from `from` — in pixels, over a grid `dims`
/// pixels across and down with [`MAP_PX_PER_TILE`] to a tile — calling
/// `f` with each pixel reached: its index, its tile, and how far along
/// the ray it is in pixels — until the ray meets a tile `opaque` says
/// yes to, leaves the grid, or has gone `far`. `from_tile` is the tile
/// the eye is in and `from_opaque` whether that tile itself is opaque:
/// a ray reaches into the wall that stops it — the wall is seen, and
/// lit, from the room — and no further; one that starts inside a wall
/// (an eye pressed to it) gets out. Each ray is the grid traversal, in
/// pixels: it steps to whichever pixel edge comes next, so every pixel
/// the line passes through is visited once and none is skipped — the
/// same walk as [`Sight::clear_line`] at eight times the resolution,
/// and integer arithmetic bar one add a step. The deck's light map and
/// the plain's picture (`terrain::Plane::picture`) are both marched by
/// this, so their shadows have the same edges.
/// The [`RAYS`] directions a fan is walked along, worked out once.
///
/// They are the same four thousand angles every time — the fan is a
/// constant — and a body's eyes on a station's deck are marched some
/// hundreds of thousands of pixels, so the two trig calls a ray were
/// eight thousand of them for every eye that moved half a pixel
/// (feature 96). The table is a tenth of the cost of the march it saves
/// and is built the first time anything is marched.

fn ray_directions() -> &'static [(f32, f32)] {
    static DIRS: std::sync::OnceLock<Vec<(f32, f32)>> = std::sync::OnceLock::new();
    DIRS.get_or_init(|| {
        (0..RAYS)
            .map(|r| {
                let a = r as f32 / RAYS as f32 * core::f32::consts::TAU;
                (a.cos(), a.sin())
            })
            .collect()
    })
}

pub(crate) fn march_rays(
    from: (f32, f32),
    far: f32,
    dims: (i32, i32),
    from_tile: (i32, i32),
    from_opaque: bool,
    opaque: impl Fn(i32, i32) -> bool,
    mut f: impl FnMut(usize, (i32, i32), f32),
) {
    let (ox, oy) = from;
    let (w, h) = dims;
    let shift = MAP_PX_PER_TILE.trailing_zeros();
    for &(dx, dy) in ray_directions() {
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
            if opaque(tx, ty) && !(from_opaque && (tx, ty) == from_tile) {
                f(i, (tx, ty), t);
                break;
            }
            f(i, (tx, ty), t);
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

    /// The light map is a **picture**, and a change made to march it
    /// faster must not move a pixel of it (feature 96). A walled room
    /// with a doorway, two lamps and a body in it, marched: the two
    /// planes are hashed and the numbers written down. Nothing about
    /// what the map *means* is asserted here — the tests above do that
    /// — only that it is the map it was.
    #[test]
    fn the_light_map_is_the_same_picture_it_was() {
        let room = Rect::from_min_size(Vec2::ZERO, vec2(16.0 * TILE, 10.0 * TILE));
        // A wall down the middle with one tile of doorway in it, so the
        // rays are stopped, turn a corner and fan out past it.
        let wall: Vec<Rect> = (0..10)
            .filter(|y| *y != 4)
            .map(|y| Rect::from_min_size(vec2(8.0 * TILE, y as f32 * TILE), vec2(TILE, TILE)))
            .collect();
        let mut sight = Sight::new(room, room, TILE, &wall, &[room]);
        sight.set_lights(&[
            Light {
                at: middle(3.0, 2.0),
                reach: 7.0 * TILE,
            },
            Light {
                at: middle(12.0, 7.0),
                reach: 9.0 * TILE,
            },
        ]);
        assert!(sight.light_map(&[middle(4.0, 4.0), middle(13.0, 2.0)]));
        let map = sight.map();
        assert_eq!((map.width, map.height), (16 * 8, 10 * 8));
        let hash = |plane: &[u8]| -> u64 {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for &b in plane {
                h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
            h
        };
        assert_eq!(hash(&map.alpha), ALPHA_PINNED, "the darkness moved");
        assert_eq!(hash(&map.glow), GLOW_PINNED, "the lamplight moved");
        // And a second call with nobody moved changes nothing at all.
        let version = map.version;
        assert!(!sight.light_map(&[middle(4.0, 4.0), middle(13.0, 2.0)]));
        assert_eq!(sight.map().version, version);
    }

    /// The two planes of [`the_light_map_is_the_same_picture_it_was`],
    /// hashed. They are not a rule — they are what the march came out at
    /// — so a change that moves them on purpose writes the new numbers
    /// down and says in its commit what moved.
    ///
    /// Moved by task 152, on purpose: a lamp's light is full to two tiles
    /// short of its reach and a soft rim after (`lamp_fall`, not the old
    /// third and a long fall), the furniture lets less past, the dark
    /// and the lamplight are deeper, and a pixel is seen as far as its
    /// light stretches the fifteen tiles (`dark_reach`) — was 8 708 404 861
    /// 411 554 907 and 18 347 543 318 836 694 730.
    ///
    /// The darkness moved again, on purpose, when the seen dark and the
    /// fog were pulled apart so the edge of sight in the dark shows
    /// (`MAP_DARK` 0.48, `MAP_FOG` 0.72) — was 7 876 032 499 620 110 001.
    const ALPHA_PINNED: u64 = 504_339_205_329_301_406;
    const GLOW_PINNED: u64 = 4_961_022_665_338_550_551;

    /// What a host drawing the light map keeps (task 121), and the walk
    /// and the composing it does — the GPU's `lightmap.wgsl`, written out
    /// in Rust step for step: the same integer steps, the same float
    /// additions and comparisons, an infinite crossing as the largest
    /// float (as the host uploads it), every other float out of a table.
    struct Host {
        sight: u64,
        seen: Vec<Vec<bool>>,
        revs: Vec<u64>,
        alpha: Vec<u8>,
        glow: Vec<u8>,
    }

    impl Host {
        fn draw(&mut self, inp: &LightInputs) {
            let (w, h) = (inp.width, inp.height);
            if self.sight != inp.sight || self.alpha.len() != w * h {
                self.sight = inp.sight;
                self.seen.clear();
                self.revs.clear();
            }
            let rays = ray_table();
            let finite = |v: f32| if v.is_finite() { v } else { f32::MAX };
            self.seen.resize(inp.views.len(), Vec::new());
            self.revs.resize(inp.views.len(), 0);
            for (b, view) in inp.views.iter().enumerate() {
                if self.revs[b] == view.rev {
                    continue;
                }
                self.revs[b] = view.rev;
                let seen = &mut self.seen[b];
                *seen = vec![false; w * h];
                for eye in &view.eyes {
                    for (ray, t0) in rays.iter().zip(&eye.t0) {
                        let (mut x, mut y) = eye.start;
                        let sx = if ray[0] > 0.0 { 1 } else { -1 };
                        let sy = if ray[1] > 0.0 { 1 } else { -1 };
                        let (dlx, dly) = (finite(ray[2]), finite(ray[3]));
                        let (mut tx, mut ty) = (finite(t0[0]), finite(t0[1]));
                        let mut t = 0.0f32;
                        loop {
                            if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || t > eye.far {
                                break;
                            }
                            let tile = (x >> 3, y >> 3);
                            let ti = tile.1 as usize * inp.columns + tile.0 as usize;
                            let i = y as usize * w + x as usize;
                            let stop = inp.cells[ti] & CELL_OPAQUE != 0
                                && !(eye.from_opaque && tile == eye.from_tile);
                            let admits = match eye.beyond {
                                None => true,
                                Some(((bx, by), (dx, dy))) => {
                                    (tile.0 - bx) * dx + (tile.1 - by) * dy >= 1
                                }
                            };
                            let (dx, dy) = (x - view.body.0, y - view.body.1);
                            let d2 = (dx * dx + dy * dy) as u32;
                            if admits && d2 <= inp.reach[255 - inp.light_field[i] as usize] {
                                seen[i] = true;
                            }
                            if stop {
                                break;
                            }
                            if tx < ty {
                                t = tx;
                                x += sx;
                                tx += dlx;
                            } else {
                                t = ty;
                                y += sy;
                                ty += dly;
                            }
                        }
                    }
                }
            }
            self.alpha = vec![0; w * h];
            self.glow = vec![0; w * h];
            for i in 0..w * h {
                let (x, y) = (i % w, i / w);
                let c = inp.cells[(y / 8) * inp.columns + x / 8];
                if c & CELL_FOGGED == 0 {
                    continue;
                }
                let light = inp.shown_field[i] as usize;
                if self.seen.iter().any(|s| s[i]) {
                    self.alpha[i] = inp.dark[light];
                    self.glow[i] = inp.glow_seen[light];
                } else {
                    self.alpha[i] = inp.fog;
                    self.glow[i] = inp.glow_fog[light];
                }
            }
        }
    }

    /// The host's walk and composing (above) come out at the very map
    /// `light_map_on_cpu` makes, frame after frame: bodies walking, one
    /// pressed to a wall and peeking, a door shutting, a lamp shot out, a
    /// closet nobody ever sees under the fog — each frame both are worked
    /// out, from the same sight twice over, and compared byte for byte.
    #[test]
    fn a_host_marching_the_inputs_draws_the_map_this_crate_draws() {
        let room = Rect::from_min_size(Vec2::ZERO, vec2(20.0 * TILE, 12.0 * TILE));
        let mut wall: Vec<Rect> = (0..12)
            .filter(|y| *y != 4 && *y != 8)
            .map(|y| Rect::from_min_size(vec2(10.0 * TILE, y as f32 * TILE), vec2(TILE, TILE)))
            .collect();
        // A closet walled in at the far corner, which nobody ever sees.
        wall.push(Rect::from_min_size(
            vec2(16.0 * TILE, 8.0 * TILE),
            vec2(4.0 * TILE, TILE),
        ));
        wall.push(Rect::from_min_size(
            vec2(16.0 * TILE, 9.0 * TILE),
            vec2(TILE, 3.0 * TILE),
        ));
        let make = || {
            let mut sight = Sight::new(room, room, TILE, &wall, &[room]);
            sight.set_lights(&[
                Light {
                    at: middle(3.0, 2.0),
                    reach: 7.0 * TILE,
                },
                Light {
                    at: middle(15.0, 7.0),
                    reach: 9.0 * TILE,
                },
            ]);
            sight
        };
        let mut cpu = make();
        let mut gpu = make();
        let mut host = Host {
            sight: 0,
            seen: Vec::new(),
            revs: Vec::new(),
            alpha: Vec::new(),
            glow: Vec::new(),
        };
        let door = Rect::from_min_size(vec2(10.0 * TILE, 8.0 * TILE), vec2(TILE, TILE));
        let frames: Vec<Vec<Vec2>> = (0..24)
            .map(|f| {
                let f = f as f32;
                vec![
                    // Through the doorway into the stranger's half and back out.
                    vec2((4.0 + 0.9 * f.min(24.0 - f)) * TILE, 4.5 * TILE),
                    // Standing still, then pressed to the wall and peeking.
                    if f < 10.0 {
                        middle(2.0, 9.0)
                    } else {
                        middle(9.0, 6.0)
                    },
                    // Pacing a little, less than half a pixel at times.
                    vec2(6.0 * TILE + (f * 0.3).sin() * 0.2, 10.5 * TILE),
                ]
            })
            .collect();
        for (f, bodies) in frames.iter().enumerate() {
            if f == 8 {
                cpu.set_shut(&[door]);
                gpu.set_shut(&[door]);
            }
            if f == 14 {
                cpu.damage_lamp(1, 1000.0);
                gpu.damage_lamp(1, 1000.0);
            }
            // The last few frames with one body fewer.
            let bodies = if f >= 20 { &bodies[..2] } else { &bodies[..] };
            cpu.light_map_on_cpu(bodies);
            gpu.light_inputs(bodies, false);
            let inputs = gpu.map().inputs.clone().expect("the host's inputs");
            host.draw(&inputs);
            let map = cpu.map();
            assert_eq!(host.alpha, map.alpha, "the darkness at frame {f}");
            assert_eq!(host.glow, map.glow, "the lamplight at frame {f}");
            assert_eq!((gpu.map().width, gpu.map().height), (map.width, map.height));
            // The scene is doing what it is for: a peek once the body is
            // against the wall, and the fog over what nobody sees.
            if f >= 10 && f < 20 {
                assert!(
                    inputs.views[1].eyes.iter().any(|e| e.beyond.is_some()),
                    "{:?}",
                    inputs.views[1]
                        .eyes
                        .iter()
                        .map(|e| (e.start, e.beyond))
                        .collect::<Vec<_>>()
                );
            }
            if f == 23 {
                assert!(host.alpha.contains(&inputs.fog) && !host.alpha.contains(&255));
            }
        }
    }

    /// **What the crew see is brighter than the fog** (task 128), and
    /// that is the rule, not a tuning: a pixel seen at no light at all, a
    /// pixel seen under a lamp at full, and a pixel nobody sees, composed
    /// on one deck — each seen alpha strictly under the unseen one. And
    /// the fog is one fog: the same deck as the world hands a friendly
    /// room and a hostile one (`Game::set_stance`, a station's box on a
    /// joined deck `Game::set_foreign`) is the same picture byte for byte,
    /// with nothing black in it.
    #[test]
    fn what_the_crew_see_is_brighter_than_the_fog_on_any_deck() {
        // Twenty tiles by six, walled across at x = 12: a lamp at the
        // west end reaching four tiles, a body at x = 9 — the lamp's pool
        // in sight, the deck round the body beyond the lamp's reach and
        // within the dark range, and everything past the wall unseen.
        let room = Rect::from_min_size(Vec2::ZERO, vec2(20.0 * TILE, 6.0 * TILE));
        let wall: Vec<Rect> = (0..6)
            .map(|y| Rect::from_min_size(vec2(12.0 * TILE, y as f32 * TILE), vec2(TILE, TILE)))
            .collect();
        let mut sight = Sight::new(room, room, TILE, &wall, &[room]);
        sight.set_lights(&[Light {
            at: middle(1.0, 3.0),
            reach: 4.0 * TILE,
        }]);
        let body = middle(9.0, 3.0);
        sight.observe(&[body], &[]);
        sight.light_map(&[body]);
        let map = sight.map();
        let px = |x: f32, y: f32| -> usize {
            let (x, y) = ((x * TILE / map.px) as usize, (y * TILE / map.px) as usize);
            y * map.width + x
        };
        let lit = px(1.5, 3.5);
        let dark = px(8.5, 3.5);
        let unseen = px(16.5, 3.5);
        assert_eq!(sight.shown_field[lit], 255, "the lamp's pool is at full");
        assert_eq!(sight.shown_field[dark], 0, "the body's deck has no light");
        assert!(sight.seen_at(middle(1.0, 3.0)) && sight.seen_at(middle(8.0, 3.0)));
        assert!(!sight.seen_at(middle(16.0, 3.0)));
        let (a_lit, a_dark, a_fog) = (map.alpha[lit], map.alpha[dark], map.alpha[unseen]);
        assert!(
            a_lit < a_fog,
            "seen and lit {a_lit} against the fog {a_fog}"
        );
        assert!(
            a_dark < a_fog,
            "seen in the dark {a_dark} against the fog {a_fog}"
        );

        // The same deck through the room, friendly and hostile: the
        // playtest ship's layout as the world lays a deck out, with one
        // body aboard, and everything the stance could reach set.
        let picture = |stance: Stance, foreign: Option<Stance>| {
            let layout = crate::aboard::layout_of(&shipdesign::fixture::playtest_ship());
            let (w, h) = (layout.bounds.width(), layout.bounds.height());
            let at = layout.bounds.min + vec2(6.5 * TILE, 6.5 * TILE);
            let mut game = crate::game::Game::with_layout(layout, 7, &[at], w, h);
            game.set_stance(stance);
            if let Some(foreign) = foreign {
                let half = Rect::from_min_size(Vec2::ZERO, vec2(w * 0.5, h));
                game.set_foreign(Some(half), foreign);
            }
            game.render();
            let map = game.light_map().expect("a deck through the crew's eyes");
            (map.alpha.clone(), map.glow.clone())
        };
        let friendly = picture(Stance::Friendly, None);
        let fog = (MAP_FOG * 255.0) as u8;
        assert!(
            friendly.0.contains(&fog),
            "something aboard is under the fog"
        );
        assert!(
            friendly.0.iter().all(|&a| a <= fog),
            "nothing darker than the fog"
        );
        assert!(
            friendly.0.iter().any(|&a| a < fog),
            "something aboard is seen"
        );
        assert_eq!(picture(Stance::Hostile, None), friendly, "a hostile deck");
        assert_eq!(
            picture(Stance::Neutral, None),
            friendly,
            "a stranger's deck"
        );
        assert_eq!(
            picture(Stance::Friendly, Some(Stance::Hostile)),
            friendly,
            "a hostile station's box on the deck"
        );
    }

    fn middle(x: f32, y: f32) -> Vec2 {
        vec2((x + 0.5) * TILE, (y + 0.5) * TILE)
    }

    /// Nobody sees past [`VIEW_RANGE`] tiles, lit tiles too, by day or
    /// night, lamps on or off; a plain's own range under it is kept.
    #[test]
    fn nobody_sees_past_fourteen_tiles_lit_or_not() {
        let room = Rect::from_min_size(Vec2::ZERO, vec2(60.0 * TILE, 6.0 * TILE));
        let mut sight = Sight::new(room, room, TILE, &[], &[room]);
        sight.set_lights(&[Light {
            at: middle(12.0, 3.0),
            reach: 9.0 * TILE,
        }]);
        let eye = middle(2.0, 3.0);
        let (near, far) = (middle(15.0, 3.0), middle(17.0, 3.0));
        assert!(sight.lit_at(near) && sight.lit_at(far));
        assert_eq!(sight.view_range(), Some(VIEW_RANGE * TILE));
        assert!(sight.sees_from(eye, near).is_some(), "thirteen tiles, lit");
        assert!(sight.sees_from(eye, far).is_none(), "fifteen tiles, lit");
        sight.set_night(true);
        assert!(sight.sees_from(eye, far).is_none(), "fifteen, at night");
        sight.set_night(false);
        let corner = Rect::from_min_size(Vec2::ZERO, vec2(TILE, TILE));
        sight.set_lamps_off(Some(corner));
        assert!(sight.sees_from(eye, far).is_none(), "fifteen, a dark map");
        sight.set_lamps_off(None);
        // A plain's own range under the cap is kept, over it held.
        sight.set_range(Some(10.0 * TILE));
        assert_eq!(sight.view_range(), Some(10.0 * TILE));
        sight.set_range(Some(60.0 * TILE));
        assert_eq!(sight.view_range(), Some(VIEW_RANGE * TILE));
    }

    /// Sandbags at (5, 5) in an open room: a body just south of them is
    /// covered from the north and from nowhere else, one standing on them
    /// is in the open, one a tile further back is still behind them, and
    /// one three tiles back is past the reach — a bolt comes over.
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
            sight.covered(middle(5.0, 7.0), middle(5.0, 1.0)),
            "one tile back from the bags is still behind them"
        );
        assert!(
            sight.covered(middle(7.0, 7.0), middle(2.0, 2.0)),
            "and one tile back on the diagonal"
        );
        assert!(
            !sight.covered(middle(5.0, 8.0), middle(5.0, 1.0)),
            "three tiles back is past the reach"
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

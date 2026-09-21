//! Drawing the game: the ship at tile scale, and the system on a map.
//!
//! # The one piece of geometry that matters
//!
//! The camera never rotates, so the **ship** does. A point in the design grid
//! lands on screen at
//!
//! ```text
//! screen = R(heading) · (design − centre_of_mass)
//! ```
//!
//! where `R` turns clockwise in a y-down coordinate system — which is exactly
//! what the canvas's own `rotate()` does, so each tile is emitted with `rot`
//! set to the heading and the host turns it.
//!
//! That is not a second opinion about which way round the ship is. It falls
//! out of `flight::angle::rotate_design` and the screen's y-flip — a rotation
//! composed with the flip between the grid's y-down and the system's y-up. The
//! tests next door are what keep the two in step:
//! `a_screen_point_maps_back_to_the_tile_it_is_over` reads this arithmetic
//! backwards at four headings, and it fails the moment a sign here moves.
//!
//! # What is never turned with the ship
//!
//! The starfield and anything drawn because it is *out there* — a station
//! alongside, everything on the map. Those are in the world, and the world
//! does not tip over when the ship does.
//!
//! # Head up
//!
//! The one exception is the player's to switch on: `Game::head_up` turns the
//! *camera* instead, by the heading undone, so the ship is drawn the way it
//! was laid out and the sky and the map turn round it. Nothing above changes
//! shape — the ship goes through `Game::ship_turn`, which is the heading plus
//! the camera's turn, and everything out there is pushed square to the
//! window as before and then turned by `Game::camera_turn` after the fact
//! (`DrawList::turn_from`). North up, both turns are what they always were.

use bims::sight::Stance;
use flight::Phase;
use shipdesign::parts::{Layer, PartKind, Rotation, TILE};
use shipdesign::{Grid, ShipDesign};
use world::{Biome, ShipState};
use worldgen::math::{DVec2, dvec2};
use worldgen::{BodyKind, Node, StationKind};

use crate::draw::{Color, DrawList};
use crate::game::{Game, Overlay, ViewMode};
use crate::hull;
use crate::paint::PART_COLORS;
use crate::starfield::FIELD;

const VOID: Color = Color::rgb(0.02, 0.03, 0.04);
const FRAME: Color = Color::rgb(0.10, 0.11, 0.13);
const DECK: Color = Color::rgb(0.13, 0.15, 0.18);
const GLOW: Color = Color::rgb(0.38, 0.86, 0.95);
const STAR: Color = Color::rgb(0.98, 0.88, 0.55);
const MUTED: Color = Color::rgba(0.55, 0.85, 0.95, 0.22);
/// The electricity overlay: a part on a live network, and one that is not.
const LIVE: Color = Color::rgb(0.50, 0.90, 0.60);
const DEAD: Color = Color::rgb(0.98, 0.45, 0.32);
/// A station's stance (`World::stance`), on the map and on its far plate.
/// Hostile is the enemy red the lobby rings a hostile station in on the
/// system diagram (`lobby::draw::ENEMY` — the same three numbers, since the
/// two crates share no palette and the player has already learnt the colour
/// there); every other station somebody lives on — home and the neutral
/// ones alike — is a friendly blue, so the map says at a glance which
/// corner of the system is theirs and which is not. A derelict is
/// nobody's and gets neither. The blue is not the aim ring's cyan
/// (`GLOW`), so the two rings read apart on one station.
const ENEMY: Color = Color::rgb(1.0, 0.28, 0.22);
const FRIEND: Color = Color::rgb(0.36, 0.55, 1.0);
/// How strongly a hostile station's far plate is washed in that red: enough
/// to tell it from a neutral stranger's black at a glance, faint enough
/// that the icon on it still reads.
const ENEMY_TINT: f32 = 0.22;
/// The stance ring on the map, as a share of the icon size: outside the
/// aim ring (which is the icon size across), so the two never sit on each
/// other when the helm is pointed at an enemy's station.
const STANCE_RING: f32 = 1.3;

/// What the rocks of a mining site are drawn in, by `world::Rock` code:
/// stone, iron ore and galvum. Stone is the dull brown of the belt's icon,
/// iron a silver that catches the light, galvum the purple nothing else
/// aboard is. Told apart at a glance, which is the point: which asteroid
/// is the rare one shows through its skin.
pub static ROCK_COLORS: [Color; 3] = [
    Color::rgb(0.42, 0.36, 0.30),
    Color::rgb(0.72, 0.74, 0.78),
    Color::rgb(0.62, 0.32, 0.82),
];
/// The seam between one rock tile and the next, and the lit edge on each.
const ROCK_SEAM: Color = Color::rgba(0.0, 0.0, 0.0, 0.35);
const ROCK_LIT: Color = Color::rgba(1.0, 1.0, 1.0, 0.10);
/// A rock marked to be mined: ringed in the warm colour an order is.
const MARK: Color = Color::rgb(0.98, 0.72, 0.35);
/// A construction site: the blueprint's blue, and how far through the
/// part's own picture shows for a site and for the blueprint in hand.
const BLUEPRINT: Color = Color::rgb(0.45, 0.72, 1.0);
const SITE_FADE: f32 = 0.55;
const GHOST_FADE: f32 = 0.50;
/// Where whoever uses a part would stand, as the designer marks it.
const SPOT: Color = Color::rgba(0.98, 0.82, 0.35, 0.85);
/// The lights round a research desk with a key on it: the spot's gold,
/// pulsing, and a wash of it over the desk so it is seen from across the
/// room.
const KEY_LIGHT: Color = Color::rgb(1.0, 0.86, 0.40);
const KEY_WASH: Color = Color::rgba(1.0, 0.86, 0.40, 0.22);
/// How many frames one pulse of them takes.
const KEY_PULSE: f32 = 90.0;
/// The ground, when the ship is on a planet (`World::landed`), by the
/// settlement's biome (`world::Biome`): desert sand, a warm ochre;
/// temperate grass, a muted green; arctic snow, a cold blue-grey — each
/// dark enough that the ship's deck and the bodies on the ground read on
/// it, and each **the same colour the town's outdoor floor is drawn in**
/// (`ground_floor`), so where the settlement's deck ends is invisible
/// and the wild ring is what marks the edge of the ground. Beside each
/// its darker tone: the patches `ground` scatters over the backdrop, so
/// it is ground and not a colour. The pad under the ship: paving, with
/// a lighter border.
const SAND: Color = Color::rgb(0.58, 0.46, 0.30);
const SAND_DARK: Color = Color::rgb(0.50, 0.39, 0.25);
const GRASS: Color = Color::rgb(0.36, 0.46, 0.28);
const GRASS_DARK: Color = Color::rgb(0.29, 0.38, 0.22);
const SNOW: Color = Color::rgb(0.64, 0.70, 0.76);
const SNOW_DARK: Color = Color::rgb(0.55, 0.62, 0.69);
/// The floor inside a town's buildings: boards, warmer than a deck.
const FLOORBOARD: Color = Color::rgb(0.36, 0.28, 0.20);
/// What is scattered over the ground between the wild: a tuft of grass
/// and, rarely, a flower on it; a ripple in the sand or a pebble; a
/// drift of snow.
const TUFT: Color = Color::rgba(0.16, 0.26, 0.12, 0.7);
const FLOWER: Color = Color::rgb(0.94, 0.82, 0.70);
const RIPPLE: Color = Color::rgba(1.0, 0.94, 0.80, 0.30);
const PEBBLE: Color = Color::rgb(0.46, 0.36, 0.24);
const DRIFT: Color = Color::rgba(0.80, 0.88, 0.96, 0.55);
/// Roughly one outdoor tile in this many carries a decoration.
const DECORATED_ONE_IN: u32 = 6;
const PAD: Color = Color::rgb(0.24, 0.25, 0.27);
const PAD_EDGE: Color = Color::rgba(1.0, 1.0, 1.0, 0.22);
/// How far past the hull the pad reaches, in tiles.
const PAD_MARGIN: f32 = 1.5;
/// How much bigger the planet is drawn at the end of a landing than at
/// the top of it, as a factor the ship's frame grows through — the planet
/// coming up under the ship — and how much of the landing's end and the
/// lift-off's beginning is black: the world loading, and the screen
/// saying so.
const LANDING_GROWTH: f32 = 40.0;
const LANDING_BLACK: f64 = 0.25;
const LIFT_BLACK: f64 = 0.2;

/// One colour per lobby slot. The route line is drawn in the colour of
/// whoever set the destination, which is the whole of what
/// `destination_set_by` is for.
pub static PLAYER_COLORS: [Color; 4] = [
    Color::rgb(0.38, 0.86, 0.95),
    Color::rgb(0.98, 0.72, 0.35),
    Color::rgb(0.55, 0.90, 0.60),
    Color::rgb(0.85, 0.58, 0.92),
];

pub fn player_color(slot: u32) -> Color {
    PLAYER_COLORS[(slot as usize) % PLAYER_COLORS.len()]
}

/// The whole frame, whichever view is up.
pub fn paint(game: &Game, list: &mut DrawList) {
    list.clear();
    match game.mode {
        ViewMode::Ship => paint_ship(game, list),
        ViewMode::Map => paint_map(game, list),
    }
}

// --- the ship ---------------------------------------------------------------

/// A design point, in the camera's units about the ship.
fn on_screen(design: DVec2, centre: DVec2, heading: f64) -> (f32, f32) {
    let d = design.sub(centre);
    let (s, c) = (heading.sin(), heading.cos());
    ((d.x * c - d.y * s) as f32, (d.x * s + d.y * c) as f32)
}

/// Where one of the crew lands in the camera's units, for the host's name
/// over their head. The same arithmetic the room's picture is turned with.
pub fn crew_on_screen(game: &Game, who: u32) -> (f32, f32) {
    on_screen(
        game.world.aboard.position(who),
        game.world.ship.dynamics.centre_of_mass,
        game.ship_turn(),
    )
}

/// The four corners of the room's light map — its origin, then clockwise
/// — in the camera's units, through the same turn the room's picture
/// goes through, so the fog lands on the deck it was traced over. `None`
/// for a room with no map (nobody's eyes).
pub fn light_map_on_screen(game: &Game) -> Option<[(f32, f32); 4]> {
    let map = game.world.aboard.room.light_map()?;
    let (w, h) = (map.size().x as f64, map.size().y as f64);
    let o = dvec2(map.origin.x as f64, map.origin.y as f64).sub(game.world.aboard.offset);
    let centre = game.world.ship.dynamics.centre_of_mass;
    let turn = game.ship_turn();
    Some([
        on_screen(o, centre, turn),
        on_screen(o.add(dvec2(w, 0.0)), centre, turn),
        on_screen(o.add(dvec2(w, h)), centre, turn),
        on_screen(o.add(dvec2(0.0, h)), centre, turn),
    ])
}

/// One number over a part in the electricity view: where it lands in the
/// camera's units — over the top of its footprint, the same arithmetic the
/// crew's names use — what it draws **now** and what it draws at most.
/// The two differ only for an engine: nothing while it is not lit, its
/// throttled share while it is, against the full draw of the table. The
/// host writes the words; this is the numbers.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PowerLabel {
    pub x: f32,
    pub y: f32,
    pub now: f64,
    pub full: f64,
    /// Whether the part is on a live network at all. A dark consumer is
    /// still labelled — with what it would draw — so the view says what
    /// wiring it would cost.
    pub live: bool,
}

/// Every consumer's draw, for the numbers the electricity view puts over
/// the drainers. The engines are in it — they are what drains most — with
/// what they draw at this moment of the plan, so the numbers agree with
/// the exhaust. Asked once a frame while the view is up, and never
/// otherwise; it is a union-find over the grid.
pub fn power_labels(game: &Game) -> Vec<PowerLabel> {
    let design = &game.world.ship.design;
    let live: Vec<u32> = shipdesign::networks(design)
        .into_iter()
        .filter(|net| net.live())
        .flat_map(|net| net.parts)
        .collect();
    let firing = game.firing();
    let dynamics = &game.world.ship.dynamics;
    let centre = dynamics.centre_of_mass;
    let turn = game.ship_turn();
    let mut out = Vec::new();
    for part in &design.parts {
        let def = part.kind.def();
        if !(def.draws() || def.pushes()) {
            continue;
        }
        let is_live = live.contains(&part.id);
        let (now, full) = if def.pushes() {
            let (lit, throttle) = match part.rotation.facing() {
                physics::Facing::Forward => (firing.forward, dynamics.forward_throttle),
                physics::Facing::Backward => (firing.backward, dynamics.backward_throttle),
                _ => (false, 0.0),
            };
            let now = if lit && is_live {
                def.thrust_power * throttle
            } else {
                0.0
            };
            (now, def.thrust_power)
        } else {
            (-def.power, -def.power)
        };
        // Over the middle of the footprint's top edge, in design units,
        // then through the ship's turn like everything else drawn on it.
        let tiles = part.tiles();
        let (x0, x1) = tiles
            .iter()
            .fold((u32::MAX, 0), |(lo, hi), &(x, _)| (lo.min(x), hi.max(x)));
        let y0 = tiles.iter().map(|&(_, y)| y).min().unwrap_or(0);
        let at = dvec2(
            (x0 as f64 + x1 as f64 + 1.0) * 0.5 * TILE as f64,
            y0 as f64 * TILE as f64,
        );
        let (x, y) = on_screen(at, centre, turn);
        out.push(PowerLabel {
            x,
            y,
            now,
            full,
            live: is_live,
        });
    }
    out
}

/// The middle of a tile, in design world units.
fn tile_middle(x: u32, y: u32) -> DVec2 {
    dvec2(
        (x as f64 + 0.5) * TILE as f64,
        (y as f64 + 0.5) * TILE as f64,
    )
}

fn paint_ship(game: &Game, list: &mut DrawList) {
    let camera = &game.ship_view;
    let scale = camera.scale().max(1e-9);

    // The void first, big enough to cover the canvas at any pan — or, on
    // a planet, the ground: there is no space down there, and the planet
    // is the whole of the surroundings.
    let half_w = camera.width / scale;
    let half_h = camera.height / scale;
    let landed = game
        .world
        .landed()
        .and_then(|body| game.world.surface(body))
        .map(|surface| surface.biome);
    let backdrop = match landed {
        Some(biome) => ground_color(biome),
        None => VOID,
    };
    list.rect(0.0, 0.0, half_w * 3.0, half_h * 3.0, 0.0, backdrop);

    // Everything out there, drawn square to the window and then turned with
    // the camera — which is not at all unless the view is head up. The
    // planet the ship is at is the ground under it; the stations are drawn
    // where they are, already turned, by `stations`. On the ground there
    // are no stars and no planet in the sky: the ground itself, patched.
    let out_there = list.len();
    if let Some(biome) = landed {
        ground(game, list, biome);
    } else {
        starfield(game, list);
        local_node(game, list);
    }
    list.turn_from(out_there, game.camera_turn() as f32);
    // The plain the town stands on, under it and the ship: the ground
    // beyond the deck, and the fog over what the crew have not seen of it.
    plain(game, list);
    let visitors = stations(game, list);

    // The ship, drawn in its own frame — design units about the design's
    // origin, the grid it was laid out in — and turned with it at the end.
    // One turn for the whole picture, so a picture made of many shapes only
    // has to be right the once.
    let design = &game.world.ship.design;
    let grid = design.grid();
    let firing = game.firing();
    let centre = game.world.ship.dynamics.centre_of_mass;
    let centre = (centre.x as f32, centre.y as f32);
    let mut ship = DrawList::default();

    // The pad the ship stands on, on a planet: paving under the whole
    // hull, in the ship's frame like the rocks, and under everything.
    if landed.is_some() {
        pad(design, &mut ship);
    }
    // The rocks of the mining site, if the ship is at one. In the ship's
    // frame — they were laid out on its tile grid — so they go through the
    // same turn the hull does; and under it, since they are outside it.
    rocks(game, &mut ship);

    // Under everything: the rim that makes the hull a body against the
    // stars, and the exhaust, which shows where it clears the stern and
    // never over the deck.
    hull::shadow(&mut ship, design, &grid);
    hull::exhaust(&mut ship, design, &grid, firing, centre, game.frame);

    // The parts the room aboard draws for itself — the galley, the heads,
    // the table, the bunks, the bay, the locker — are left to it: it has the
    // pictures. The airlock is drawn open while the ship is docked, because
    // it is mated to the station's and a way through.
    let rooms = bims::aboard::drawn_by_room(design);
    let mated = game.mated_airlock().map(|id| (id, game.airlock_ajar));
    hull_tiles(&mut ship, design, &grid, firing, &rooms, mated, None);
    // What is laid out to be built, over the deck it will stand on: each
    // site as the part's own picture, shown through, in the blueprint's
    // blue, with how much of it has arrived along the bottom. And the
    // blueprint in the player's hand, over the tile the pointer is on.
    sites(game, &grid, &mut ship);
    blueprint(game, &grid, &mut ship);
    hull::lights(&mut ship, design, &grid, game.frame);
    lamp_faces(&mut ship, game, design, None);
    // The reactors' glow over their pictures: brighter the harder they
    // work, which under a burn is the engines drawing on them.
    let load = game.world.power().load() as f32;
    for part in &design.parts {
        if part.kind.def().supplies() {
            crate::fittings::reactor_glow(&mut ship, part, load, game.frame);
        }
    }

    // The tile under the pointer, rung. Part of the ship, so turned with it
    // — and a rock under the pointer while the player is marking rocks,
    // which is on the same grid.
    let tile = TILE as f32;
    let over_rock = game.marking
        && game
            .hover
            .is_some_and(|(x, y)| game.world.site_here().is_some_and(|s| s.at(x, y).is_some()));
    if let Some((x, y)) = game.hover
        && (design.holds((x, y)) || over_rock)
    {
        let m = signed_tile_middle(x, y);
        ship.push(
            crate::draw::KIND_RECT,
            m.x as f32,
            m.y as f32,
            tile,
            tile,
            0.0,
            3.0,
            2.0,
            GLOW,
        );
    }

    let turn = game.ship_turn() as f32;
    list.append_turned(ship.shapes(), centre, turn);
    // The room aboard — its fixtures, the deck's mess, the crew and what
    // they are carrying — as the room drew it, over the hull and turned the
    // same way. The room's draw buffer is the same twelve floats a shape as
    // this one, since the format is shared across all three cdylibs. Over
    // the ship's picture rather than in it because it is a separate buffer;
    // over the lights and the hover ring too, which touches nothing the
    // room draws.
    // The room's picture is in the room's own units, which are the ship's
    // shifted by the ship's offset into them — nothing while the ship is
    // on its own, the join's shift while it is docked.
    let offset = game.world.aboard.offset;
    let room_centre = (centre.0 + offset.x as f32, centre.1 + offset.y as f32);
    list.append_turned(game.world.aboard.room.shapes(), room_centre, turn);
    // The station's people, over the ship's picture: one that has come
    // through the passage is standing on this deck, and one that has not
    // is on the station's, where nothing of the ship's is drawn.
    list.append(visitors.shapes());
    // The electricity overlay, over the lot — the room's fixtures included,
    // since a galley that draws power is rung as much as a reactor is — in
    // the ship's frame and turned with it like the hull.
    if game.overlay == Overlay::Electricity {
        let mut over = DrawList::default();
        electricity(&mut over, design, &grid);
        list.append_turned(over.shapes(), centre, turn);
    }
    // The black at the end of a landing and the start of a lift-off: the
    // planet has filled the window and the world is being laid out on
    // it — or taken apart again. Over the lot, square to the window.
    if let Some(black) = blackout(game) {
        list.rect(
            0.0,
            0.0,
            half_w * 3.0,
            half_h * 3.0,
            0.0,
            Color::rgba(0.0, 0.0, 0.0, black),
        );
    }
}

/// The ground under a landed ship, by the settlement's biome: the very
/// colour the town's outdoor floor is drawn in, so the two meet without
/// a line.
fn ground_color(biome: Biome) -> Color {
    match biome {
        Biome::Desert => SAND,
        Biome::Temperate => GRASS,
        Biome::Arctic => SNOW,
    }
}

/// The darker tone of the same ground: the patches on the backdrop.
fn ground_dark(biome: Biome) -> Color {
    match biome {
        Biome::Desert => SAND_DARK,
        Biome::Temperate => GRASS_DARK,
        Biome::Arctic => SNOW_DARK,
    }
}

/// How black the window is: the last [`LANDING_BLACK`] of a landing fades
/// to black, and the first [`LIFT_BLACK`] of a lift-off fades from it.
/// `None` the rest of the time, which is to say nearly always.
fn blackout(game: &Game) -> Option<f32> {
    if let Some((_, done)) = game.world.landing() {
        let from = 1.0 - LANDING_BLACK;
        return (done > from).then(|| ((done - from) / LANDING_BLACK) as f32);
    }
    if let Some((_, done)) = game.world.lifting() {
        return (done < LIFT_BLACK).then(|| (1.0 - done / LIFT_BLACK) as f32);
    }
    None
}

/// The ground about a landed ship: patches in the biome's darker tone —
/// bare earth, drifts — scattered at fixed places in the camera's units
/// about the ship, so they zoom and pan with the view and hold still
/// between frames. The ship does not move on the ground, so the patches
/// need no world position; a hash puts each where it is.
fn ground(game: &Game, list: &mut DrawList, biome: Biome) {
    let patch = ground_dark(biome);
    let camera = &game.ship_view;
    let scale = camera.scale().max(1e-9);
    let reach = (camera.width.max(camera.height) / scale) * 1.5;
    let tile = TILE as f32;
    let mut h: u32 = 0x9E37_79B9;
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        (h & 0xFFFF) as f32 / 65535.0
    };
    for _ in 0..140 {
        let x = (next() * 2.0 - 1.0) * reach;
        let y = (next() * 2.0 - 1.0) * reach;
        let w = tile * (1.5 + next() * 6.0);
        let hgt = w * (0.4 + next() * 0.5);
        let rot = next() * core::f32::consts::PI;
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            w,
            hgt,
            rot,
            0.0,
            0.0,
            patch,
        );
    }
}

/// The landing pad: paving under the hull's whole box and a margin past
/// it, with a lighter border, in the ship's own frame.
fn pad(design: &ShipDesign, ship: &mut DrawList) {
    let mut span: Option<(u32, u32, u32, u32)> = None;
    for part in &design.parts {
        for (x, y) in part.tiles() {
            span = Some(match span {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    let Some((x0, y0, x1, y1)) = span else {
        return;
    };
    let tile = TILE as f32;
    let (x0, y0) = (
        x0 as f32 * tile - PAD_MARGIN * tile,
        y0 as f32 * tile - PAD_MARGIN * tile,
    );
    let (x1, y1) = (
        (x1 + 1) as f32 * tile + PAD_MARGIN * tile,
        (y1 + 1) as f32 * tile + PAD_MARGIN * tile,
    );
    let (w, hgt) = (x1 - x0, y1 - y0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    ship.rect(cx, cy, w, hgt, 0.0, PAD);
    ship.push(
        crate::draw::KIND_RECT,
        cx,
        cy,
        w - tile * 0.5,
        hgt - tile * 0.5,
        0.0,
        0.0,
        4.0,
        PAD_EDGE,
    );
}

/// The middle of a tile that may be off the hull, in design world units.
fn signed_tile_middle(x: i32, y: i32) -> DVec2 {
    dvec2(
        (x as f64 + 0.5) * TILE as f64,
        (y as f64 + 0.5) * TILE as f64,
    )
}

/// The mining site's rocks, a tile each, in the ship's frame: every tile
/// standing, in its kind's colour with a seam round it and a lit edge, and
/// the marked ones ringed. Nothing while the ship is not at a site.
fn rocks(game: &Game, ship: &mut DrawList) {
    let Some(site) = game.world.site_here() else {
        return;
    };
    let tile = TILE as f32;
    for rock in &site.tiles {
        let m = signed_tile_middle(rock.x, rock.y);
        let (x, y) = (m.x as f32, m.y as f32);
        let color = ROCK_COLORS[rock.kind.code() as usize % ROCK_COLORS.len()];
        list_rock(ship, x, y, tile, color);
    }
    for &(x, y) in &site.marked {
        let m = signed_tile_middle(x, y);
        let (x, y) = (m.x as f32, m.y as f32);
        ship.push(
            crate::draw::KIND_RECT,
            x,
            y,
            tile - 4.0,
            tile - 4.0,
            0.0,
            4.0,
            0.0,
            MARK.alpha(0.35),
        );
        ship.push(
            crate::draw::KIND_RECT,
            x,
            y,
            tile - 4.0,
            tile - 4.0,
            0.0,
            4.0,
            4.0,
            MARK,
        );
    }
}

/// One tile of rock: the seam is the tile, the rock is inset into it, and a
/// lighter sliver along its top-left edge is the light on it.
fn list_rock(list: &mut DrawList, x: f32, y: f32, tile: f32, color: Color) {
    list.push(
        crate::draw::KIND_RECT,
        x,
        y,
        tile,
        tile,
        0.0,
        0.0,
        0.0,
        ROCK_SEAM,
    );
    list.push(
        crate::draw::KIND_RECT,
        x,
        y,
        tile - 3.0,
        tile - 3.0,
        0.0,
        5.0,
        0.0,
        color,
    );
    list.push(
        crate::draw::KIND_RECT,
        x - tile * 0.12,
        y - tile * 0.12,
        tile * 0.5,
        tile * 0.5,
        0.0,
        6.0,
        0.0,
        ROCK_LIT,
    );
}

/// The part a site or a blueprint is for, as its own picture, faded into
/// `list`: the hull's or the fittings' picture where there is one, the
/// deck's tile for plating, and the part's colour as a block otherwise —
/// the same three askings as `hull_tiles`, so a ghost of a thing looks
/// like the thing.
fn faded_part(
    list: &mut DrawList,
    design: &ShipDesign,
    grid: &Grid,
    kind: PartKind,
    origin: (u32, u32),
    rotation: Rotation,
    alpha: f32,
) {
    let tile = TILE as f32;
    let part = shipdesign::PlacedPart {
        id: 0,
        kind,
        origin,
        rotation,
    };
    let mut picture = DrawList::default();
    let drawn = kind != PartKind::Floor
        && (hull::part(&mut picture, &part, grid, hull::Firing::NONE, None)
            || crate::fittings::part(&mut picture, &part));
    if !drawn {
        let color = match kind {
            PartKind::Floor => DECK,
            PartKind::Structure => FRAME,
            _ => PART_COLORS[kind as usize],
        };
        for (x, y) in part.tiles() {
            let m = tile_middle(x, y);
            picture.push(
                crate::draw::KIND_RECT,
                m.x as f32,
                m.y as f32,
                tile - 3.0,
                tile - 3.0,
                0.0,
                4.0,
                0.0,
                color,
            );
        }
    }
    let _ = design;
    list.append_faded(picture.shapes(), alpha);
}

/// The lamps' glass as the fight left it, over the hull's picture: every
/// light part of `design` asked of the world (`World::lamp_look`) — the
/// ship's own at `station` `None`, a station's by its id — and drawn
/// dark, cracked or veiled where it is out or flickering
/// (`fittings::lamp_face`). A lamp whole and steady draws nothing here.
fn lamp_faces(list: &mut DrawList, game: &Game, design: &ShipDesign, station: Option<u32>) {
    for part in &design.parts {
        if !shipdesign::is_light(part.kind) {
            continue;
        }
        let (share, level) = game.world.lamp_look(station, part.origin);
        if share > 0.0 && level >= 1.0 {
            continue;
        }
        crate::fittings::lamp_face(list, part, share, level);
    }
}

/// The box round a part's tiles, in design units: `(x0, y0, x1, y1)`.
fn part_box(kind: PartKind, origin: (u32, u32), rotation: Rotation) -> (f32, f32, f32, f32) {
    let t = TILE as f32;
    let (w, h) = shipdesign::parts::footprint(kind, rotation);
    let x0 = origin.0 as f32 * t;
    let y0 = origin.1 as f32 * t;
    (x0, y0, x0 + w as f32 * t, y0 + h as f32 * t)
}

/// The lights round every research desk of a design with a key on it: a
/// wash over the desk and a ring of small lights a little way out from
/// its footprint, pulsing together on the frame's clock — what
/// "highlighted" is on the deck, and how a crew ashore finds the desk in
/// a station of rooms. In the design's frame, like the hull.
fn key_lights(list: &mut DrawList, design: &ShipDesign, frame: u32) {
    let pulse = 0.55 + 0.45 * (frame as f32 / KEY_PULSE * core::f32::consts::TAU).sin();
    let out = TILE as f32 * 0.55;
    for part in design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::ResearchDesk)
    {
        let (x0, y0, x1, y1) = part_box(part.kind, part.origin, part.rotation);
        let (w, h) = (x1 - x0 + 2.0 * out, y1 - y0 + 2.0 * out);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        list.push(
            crate::draw::KIND_RECT,
            cx,
            cy,
            w,
            h,
            0.0,
            8.0,
            0.0,
            KEY_WASH.alpha(KEY_WASH.a * pulse),
        );
        // The lights along each edge, a tile apart, corners included.
        let mut spots = Vec::new();
        let along = |a: f32, b: f32| {
            let n = ((b - a) / TILE as f32).round().max(1.0) as u32;
            (0..=n).map(move |i| a + (b - a) * i as f32 / n as f32)
        };
        for x in along(x0 - out, x1 + out) {
            spots.push((x, y0 - out));
            spots.push((x, y1 + out));
        }
        for y in along(y0 - out, y1 + out) {
            spots.push((x0 - out, y));
            spots.push((x1 + out, y));
        }
        for (x, y) in spots {
            list.push(
                crate::draw::KIND_ELLIPSE,
                x,
                y,
                12.0 * pulse + 4.0,
                12.0 * pulse + 4.0,
                0.0,
                0.0,
                0.0,
                KEY_LIGHT.alpha(0.25 * pulse),
            );
            list.push(
                crate::draw::KIND_ELLIPSE,
                x,
                y,
                5.0,
                5.0,
                0.0,
                0.0,
                0.0,
                KEY_LIGHT.alpha(0.5 + 0.5 * pulse),
            );
        }
    }
}

/// Every construction site, over the deck: the part shown through in the
/// blueprint's blue, ringed, with a bar along its foot for how much of
/// what it is made of has been carried to it. In the ship's frame, since
/// a site is a tile of the ship.
fn sites(game: &Game, grid: &Grid, ship: &mut DrawList) {
    let design = &game.world.ship.design;
    for site in &game.world.builds {
        faded_part(
            ship,
            design,
            grid,
            site.kind,
            site.origin,
            site.rotation,
            SITE_FADE,
        );
        let (x0, y0, x1, y1) = part_box(site.kind, site.origin, site.rotation);
        ship.box_between(x0, y0, x1, y1, 3.0, BLUEPRINT.alpha(0.16));
        ship.stroke_between(x0 + 1.5, y0 + 1.5, x1 - 1.5, y1 - 1.5, 3.0, 2.0, BLUEPRINT);
        // What has arrived, as a share of what it is made of.
        let recipe = site.recipe(design);
        let wanted: u32 = recipe.iter().map(|&(_, u)| u).sum();
        let there: u32 = recipe
            .iter()
            .map(|&(id, u)| site.delivered[id as usize].min(u))
            .sum();
        if wanted > 0 && there > 0 {
            let share = there as f32 / wanted as f32;
            let w = (x1 - x0 - 8.0) * share;
            ship.rect(
                x0 + 4.0 + w / 2.0,
                y1 - 6.0,
                w,
                4.0,
                0.0,
                if there >= wanted { LIVE } else { BLUEPRINT },
            );
        }
    }
}

/// The blueprint in the player's hand, over the tile the pointer is on:
/// the part shown through, ringed in the live colour where it would go
/// and the warning colour where it would not — the world's own answer,
/// asked through `Game::ghost_check`. Nothing with no tool in hand, and
/// nothing in the map view, where a tile means nothing.
fn blueprint(game: &Game, grid: &Grid, ship: &mut DrawList) {
    let Some((kind, rotation)) = game.placing else {
        return;
    };
    let Some((x, y)) = game.hover else {
        return;
    };
    if x < 0 || y < 0 {
        return;
    }
    let origin = (x as u32, y as u32);
    let ok = game.ghost_ok();
    let color = if ok { LIVE } else { DEAD };
    faded_part(
        ship,
        &game.world.ship.design,
        grid,
        kind,
        origin,
        rotation,
        GHOST_FADE,
    );
    let (x0, y0, x1, y1) = part_box(kind, origin, rotation);
    ship.box_between(x0, y0, x1, y1, 4.0, color.alpha(0.18));
    ship.stroke_between(x0 + 1.0, y0 + 1.0, x1 - 1.0, y1 - 1.0, 4.0, 2.0, color);
    // Where whoever uses it will stand, as the designer marks it.
    if ok {
        let t = TILE as f32;
        let any = shipdesign::parts::any_side_will_do(kind);
        for (dx, dy) in shipdesign::parts::use_spots(kind, rotation) {
            let tile = (x + dx, y + dy);
            if any && (grid.get(Layer::Floor, tile) == 0 || grid.get(Layer::Object, tile) != 0) {
                continue;
            }
            let m = signed_tile_middle(tile.0, tile.1);
            ship.ellipse(m.x as f32, m.y as f32, t * 0.30, t * 0.30, SPOT);
        }
    }
}

/// The ground a settlement stands on, as [`hull_tiles`] draws it: the
/// biome, and which of its tiles are **out of doors** — reached from the
/// tile inside the port by a four-neighbour flood over tiles with nothing
/// standing on them but the wild (a tree, a shrub, a boulder, water, a
/// field), a lamp, sandbags or a plant. A door, a wall or any other part
/// stops the flood, so a house's floor is inside and the street outside
/// its door is not. Built afresh each frame in `stations`, which is a walk
/// of the parts and the tiles and nothing more.
struct Terrain {
    biome: Biome,
    side: u32,
    outdoors: Vec<bool>,
}

impl Terrain {
    /// The flood, from the port's inside tile — the tile one step in from
    /// the airlock's middle, the way `world::crew` finds it.
    fn of(design: &ShipDesign, biome: Biome) -> Terrain {
        let side = design.build_area;
        let n = (side * side) as usize;
        let mut open = vec![true; n];
        for part in &design.parts {
            if part.layer() != Layer::Object || passable_outdoors(part.kind) {
                continue;
            }
            for (x, y) in part.tiles() {
                if x < side && y < side {
                    open[(y * side + x) as usize] = false;
                }
            }
        }
        let mut outdoors = vec![false; n];
        let Some(port) = shipdesign::port(design) else {
            return Terrain {
                biome,
                side,
                outdoors,
            };
        };
        let t = TILE as f64;
        let start = (
            (port.centre.0 / t) as i32 - port.outward.0,
            (port.centre.1 / t) as i32 - port.outward.1,
        );
        let at = |(x, y): (i32, i32)| -> Option<usize> {
            (x >= 0 && y >= 0 && (x as u32) < side && (y as u32) < side)
                .then(|| (y as u32 * side + x as u32) as usize)
        };
        let mut stack = Vec::new();
        if let Some(i) = at(start)
            && open[i]
        {
            outdoors[i] = true;
            stack.push(start);
        }
        while let Some((x, y)) = stack.pop() {
            for next in [(x, y - 1), (x + 1, y), (x, y + 1), (x - 1, y)] {
                if let Some(i) = at(next)
                    && open[i]
                    && !outdoors[i]
                {
                    outdoors[i] = true;
                    stack.push(next);
                }
            }
        }
        Terrain {
            biome,
            side,
            outdoors,
        }
    }

    fn outdoor(&self, x: u32, y: u32) -> bool {
        x < self.side && y < self.side && self.outdoors[(y * self.side + x) as usize]
    }
}

/// What the outdoors flood passes over: the wild, and the few parts
/// that stand out of doors without closing the ground off.
fn passable_outdoors(kind: PartKind) -> bool {
    matches!(
        kind,
        PartKind::Tree
            | PartKind::Shrub
            | PartKind::Boulder
            | PartKind::Water
            | PartKind::Field
            | PartKind::StandingLight
            | PartKind::Sandbags
            | PartKind::BigPlant
            | PartKind::SmallPlant
    )
}

/// A number off a tile's place, for which tiles carry a decoration and
/// which one: fixed, so the ground holds still from frame to frame.
fn tile_hash(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// A settlement's floor: one rect a **run** of floor tiles along each
/// row — the outdoor runs in the biome's ground colour, the indoor runs
/// in boards — rather than a rect a tile, since a town is nine thousand
/// tiles and most of them are ground. Then, on roughly one outdoor tile
/// in [`DECORATED_ONE_IN`] with nothing standing on it, the ground's
/// decoration: a tuft of grass and now and then a flower, a ripple in
/// the sand or a pebble, a drift of snow.
fn ground_floor(list: &mut DrawList, grid: &Grid, terrain: &Terrain) {
    let tile = TILE as f32;
    let side = terrain.side;
    for y in 0..side {
        let mut run: Option<(u32, bool)> = None;
        for x in 0..=side {
            let here =
                (x < side && grid.has_floor((x as i32, y as i32))).then(|| terrain.outdoor(x, y));
            if let Some((x0, outdoor)) = run
                && here != Some(outdoor)
            {
                let color = if outdoor {
                    ground_color(terrain.biome)
                } else {
                    FLOORBOARD
                };
                list.box_between(
                    x0 as f32 * tile,
                    y as f32 * tile,
                    x as f32 * tile,
                    (y + 1) as f32 * tile,
                    0.0,
                    color,
                );
                run = None;
            }
            if run.is_none()
                && let Some(outdoor) = here
            {
                run = Some((x, outdoor));
            }
        }
    }
    for y in 0..side {
        for x in 0..side {
            let h = tile_hash(x, y);
            if h % DECORATED_ONE_IN != 0
                || !terrain.outdoor(x, y)
                || !grid.has_floor((x as i32, y as i32))
                || grid.get(Layer::Object, (x as i32, y as i32)) != 0
            {
                continue;
            }
            let m = tile_middle(x, y);
            decoration(list, terrain.biome, m.x as f32, m.y as f32, h);
        }
    }
}

/// One tile's decoration — see [`ground_floor`] — about `(cx, cy)`, off
/// the tile's hash `h`.
fn decoration(list: &mut DrawList, biome: Biome, cx: f32, cy: f32, h: u32) {
    let tile = TILE as f32;
    {
        {
            // Where in the tile, and which way: off the hash's upper bits.
            let roll = h / DECORATED_ONE_IN;
            let dx = ((roll & 0xF) as f32 / 15.0 - 0.5) * tile * 0.5;
            let dy = (((roll >> 4) & 0xF) as f32 / 15.0 - 0.5) * tile * 0.5;
            let rot = ((roll >> 8) & 0xF) as f32 / 15.0 * core::f32::consts::PI;
            let (x, y) = (cx + dx, cy + dy);
            match biome {
                Biome::Temperate => {
                    for i in 0..2 {
                        let a = rot + i as f32 * 0.9 - 0.6;
                        let reach = tile * 0.16;
                        list.line(
                            x,
                            y + tile * 0.06,
                            x + a.cos() * reach,
                            y - a.sin().abs() * reach,
                            1.5,
                            TUFT,
                        );
                    }
                    if (roll >> 13) % 7 == 0 {
                        list.ellipse(x + tile * 0.1, y - tile * 0.04, 4.0, 4.0, FLOWER);
                    }
                }
                Biome::Desert => {
                    if roll & 0x1000 == 0 {
                        list.push(
                            crate::draw::KIND_RECT,
                            x,
                            y,
                            tile * 0.55,
                            1.5,
                            rot * 0.25 - 0.4,
                            0.0,
                            0.0,
                            RIPPLE,
                        );
                    } else {
                        list.ellipse(x, y, 5.0, 4.0, PEBBLE);
                    }
                }
                Biome::Arctic => {
                    list.push(
                        crate::draw::KIND_ELLIPSE,
                        x,
                        y,
                        tile * 0.7,
                        tile * 0.32,
                        rot * 0.3 - 0.4,
                        0.0,
                        0.0,
                        DRIFT,
                    );
                }
            }
        }
    }
}

// --- the plain ----------------------------------------------------------------------

/// The plain's features, by biome: a cliff's rock face and the lighter
/// lip along its top; water and its wavelets; a forest's canopy, dark,
/// with lighter crowns on it.
const CLIFF_TEMPERATE: Color = Color::rgb(0.40, 0.38, 0.34);
const CLIFF_DESERT: Color = Color::rgb(0.52, 0.34, 0.22);
const CLIFF_ARCTIC: Color = Color::rgb(0.46, 0.50, 0.56);
const CLIFF_LIP: Color = Color::rgba(1.0, 1.0, 1.0, 0.16);
const CLIFF_FOOT: Color = Color::rgba(0.0, 0.0, 0.0, 0.30);
const WATER_TEMPERATE: Color = Color::rgb(0.24, 0.44, 0.64);
const WATER_DESERT: Color = Color::rgb(0.22, 0.54, 0.62);
const ICE: Color = Color::rgb(0.70, 0.82, 0.90);
const FOREST_TEMPERATE: Color = Color::rgb(0.14, 0.26, 0.12);
const FOREST_ARCTIC: Color = Color::rgb(0.10, 0.22, 0.16);
const CROWN_TEMPERATE: Color = Color::rgb(0.20, 0.36, 0.16);
const CROWN_ARCTIC: Color = Color::rgb(0.16, 0.32, 0.22);
/// The fog over the plain: never seen, and seen once and not now.
const PLAIN_BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
const PLAIN_GREY: Color = Color::rgba(0.06, 0.07, 0.08, 0.80);
/// A shift that puts every tile of the plain the camera can reach at a
/// non-negative tile, so the wild's painters — which take a placed part,
/// whose origin is unsigned — can draw it; the picture is placed back by
/// the same shift.
const PLAIN_SHIFT: i32 = 8192;
/// How far from the camera's middle the plain is drawn at most, in tiles:
/// the corners of a view held to `bims::terrain::VIEW` along its nearer
/// edge (`Camera::scale_for_reach`), with a tile over.
const PLAIN_DRAWN: i32 = bims::terrain::VIEW * 2;
/// How far a run of the plain's fog or ground reaches past its tiles, in
/// room units, so two runs meet under the feathering rather than beside it.
const RUN_LAP: f32 = 3.0;
/// The same for the fog, which is opaque and may lap by half a tile: at the
/// floor a tile is a few pixels and the feathering a pixel of it.
const VEIL_LAP: f32 = 26.0;

/// The ground beyond the deck, while the ship is on a planet: the plain
/// (`bims::terrain`) as the crew's room reads it, drawn a tile at a time
/// in the room's frame over the window the camera can see — never more
/// than [`PLAIN_DRAWN`] tiles from the camera's middle, which is as far
/// as the world is loaded — and then the fog over what the crew
/// do not see of it, black where nobody has looked and grey where
/// somebody has. The deck's own tiles are the station's and the ship's
/// to draw, and the fog over the room's box is the room's light map.
/// Water, cliff and forest are runs along a row; a tree, a shrub and a
/// rock are the biome's, as `fittings` draws the town's.
fn plain(game: &Game, list: &mut DrawList) {
    let room = &game.world.aboard.room;
    let Some(plane) = room.plane() else {
        return;
    };
    let biome = Biome::from_code(plane.biome() as u32).unwrap_or(Biome::Temperate);
    let tile = TILE as f32;
    let camera = &game.ship_view;
    let scale = camera.scale().max(1e-9);
    let offset = game.world.aboard.offset;
    // The camera's middle, in the room's units, and how far from it the
    // canvas reaches at its corners.
    let mid = game
        .design_point_at(camera.width / 2.0, camera.height / 2.0)
        .add(offset);
    let (cx, cy) =
        bims::terrain::Plane::tile_of(bims::math::vec2(mid.x as f32, mid.y as f32), tile);
    let half_diag = camera.width.hypot(camera.height) / 2.0 / scale / tile;
    let reach = (half_diag.ceil() as i32 + 1).min(PLAIN_DRAWN);
    let grid = game.world.aboard.design.grid();
    let interior = room.interior();
    let s = PLAIN_SHIFT;
    let side = 2 * reach + 1;
    let mut veils = vec![bims::terrain::VEIL_NONE; (side * side) as usize];
    let mut picture = DrawList::default();
    let (cliff, water, forest, crown) = match biome {
        Biome::Temperate => (
            CLIFF_TEMPERATE,
            WATER_TEMPERATE,
            FOREST_TEMPERATE,
            CROWN_TEMPERATE,
        ),
        Biome::Desert => (
            CLIFF_DESERT,
            WATER_DESERT,
            FOREST_TEMPERATE,
            CROWN_TEMPERATE,
        ),
        Biome::Arctic => (CLIFF_ARCTIC, ICE, FOREST_ARCTIC, CROWN_ARCTIC),
    };
    use bims::terrain::Ground;
    let deck = |rx: i32, ry: i32| grid.has_floor((rx, ry));
    // A run overlaps its neighbours by a hair, or the feathering of every
    // edge shows the ground between two rows of fog as a seam.
    let box_run = |picture: &mut DrawList, x0: i32, x1: i32, ry: i32, color: Color| {
        picture.box_between(
            (x0 + s) as f32 * tile - RUN_LAP,
            (ry + s) as f32 * tile - RUN_LAP,
            (x1 + s) as f32 * tile + RUN_LAP,
            (ry + s + 1) as f32 * tile + RUN_LAP,
            0.0,
            color,
        );
    };
    for ry in cy - reach..=cy + reach {
        // The runs first: one rect a stretch of the same ground.
        let mut run: Option<(i32, Ground)> = None;
        for rx in cx - reach..=cx + reach + 1 {
            let here = (rx <= cx + reach && !deck(rx, ry))
                .then(|| plane.at_room(rx, ry))
                .filter(|g| matches!(g, Ground::Water | Ground::Cliff | Ground::Forest));
            if let Some((x0, kind)) = run
                && here != Some(kind)
            {
                let color = match kind {
                    Ground::Water => water,
                    Ground::Cliff => cliff,
                    _ => forest,
                };
                box_run(&mut picture, x0, rx, ry, color);
                if kind == Ground::Cliff {
                    // The lip along the top where the ground above is not
                    // cliff, and the shadow at its foot.
                    for x in x0..rx {
                        if plane.at_room(x, ry - 1) != Ground::Cliff {
                            picture.box_between(
                                (x + s) as f32 * tile,
                                (ry + s) as f32 * tile,
                                (x + s + 1) as f32 * tile,
                                (ry + s) as f32 * tile + tile * 0.18,
                                0.0,
                                CLIFF_LIP,
                            );
                        }
                        if plane.at_room(x, ry + 1) != Ground::Cliff {
                            picture.box_between(
                                (x + s) as f32 * tile,
                                (ry + s + 1) as f32 * tile - tile * 0.22,
                                (x + s + 1) as f32 * tile,
                                (ry + s + 1) as f32 * tile,
                                0.0,
                                CLIFF_FOOT,
                            );
                        }
                    }
                }
                run = None;
            }
            if run.is_none()
                && let Some(kind) = here
            {
                run = Some((rx, kind));
            }
        }
        // Then what stands a tile at a time, and the decoration.
        for rx in cx - reach..=cx + reach {
            if deck(rx, ry) {
                continue;
            }
            let h = tile_hash((rx + s) as u32, (ry + s) as u32);
            let origin = ((rx + s) as u32, (ry + s) as u32);
            match plane.at_room(rx, ry) {
                Ground::Open => {
                    if h % DECORATED_ONE_IN == 0 {
                        let m = tile_middle(origin.0, origin.1);
                        decoration(&mut picture, biome, m.x as f32, m.y as f32, h);
                    }
                }
                Ground::Forest => {
                    // A crown on one tile in three, off its own hash.
                    if h % 3 == 0 {
                        let m = tile_middle(origin.0, origin.1);
                        let dx = ((h >> 8) & 0xF) as f32 / 15.0 - 0.5;
                        let dy = ((h >> 12) & 0xF) as f32 / 15.0 - 0.5;
                        picture.ellipse(
                            m.x as f32 + dx * tile * 0.6,
                            m.y as f32 + dy * tile * 0.6,
                            tile * 0.9,
                            tile * 0.8,
                            crown,
                        );
                    }
                }
                Ground::Water | Ground::Cliff => {}
                kind => {
                    let part = shipdesign::PlacedPart {
                        id: 0,
                        kind: match kind {
                            Ground::Tree => PartKind::Tree,
                            Ground::Shrub => PartKind::Shrub,
                            _ => PartKind::Boulder,
                        },
                        origin,
                        rotation: Rotation::R0,
                    };
                    crate::fittings::part_in(&mut picture, &part, Some(biome));
                }
            }
        }
        // The fog, over everything off the room's box: what each tile
        // wants, kept for the merge below.
        for rx in cx - reach..=cx + reach {
            let m = bims::math::vec2((rx as f32 + 0.5) * tile, (ry as f32 + 0.5) * tile);
            let code = if interior.contains(m) {
                bims::terrain::VEIL_NONE
            } else {
                plane.veil_at_room(rx, ry)
            };
            veils[((ry - cy + reach) * side + (rx - cx + reach)) as usize] = code;
        }
    }
    // The fog as rectangles: a run along a row, and a run the same as the
    // one under it joined to it — a stepped edge is still a rect a row,
    // but the body of it is a few big ones, and there are no seams for
    // the feathering to show at the zoom the floor allows. Opaque, both
    // of them, so that lapping them costs nothing: the grey is the fog's
    // grey over the ground's colour, worked out once.
    let ground = ground_color(biome);
    let grey = Color::rgb(
        ground.r * (1.0 - PLAIN_GREY.a) + PLAIN_GREY.r * PLAIN_GREY.a,
        ground.g * (1.0 - PLAIN_GREY.a) + PLAIN_GREY.g * PLAIN_GREY.a,
        ground.b * (1.0 - PLAIN_GREY.a) + PLAIN_GREY.b * PLAIN_GREY.a,
    );
    let mut open: Vec<(i32, i32, u8, i32)> = Vec::new();
    let emit = |picture: &mut DrawList, x0: i32, x1: i32, kind: u8, y0: i32, y1: i32| {
        let color = if kind == bims::terrain::VEIL_BLACK {
            PLAIN_BLACK
        } else {
            grey
        };
        picture.box_between(
            (x0 + s) as f32 * tile - VEIL_LAP,
            (y0 + s) as f32 * tile - VEIL_LAP,
            (x1 + s) as f32 * tile + VEIL_LAP,
            (y1 + s) as f32 * tile + VEIL_LAP,
            0.0,
            color,
        );
    };
    for ry in cy - reach..=cy + reach + 1 {
        let mut runs: Vec<(i32, i32, u8)> = Vec::new();
        if ry <= cy + reach {
            let mut run: Option<(i32, u8)> = None;
            for rx in cx - reach..=cx + reach + 1 {
                let here = (rx <= cx + reach)
                    .then(|| veils[((ry - cy + reach) * side + (rx - cx + reach)) as usize])
                    .filter(|&v| v != bims::terrain::VEIL_NONE);
                if let Some((x0, kind)) = run
                    && here != Some(kind)
                {
                    runs.push((x0, rx, kind));
                    run = None;
                }
                if run.is_none()
                    && let Some(kind) = here
                {
                    run = Some((rx, kind));
                }
            }
        }
        // What was open and is not in this row is done; what is carries on.
        let mut next: Vec<(i32, i32, u8, i32)> = Vec::new();
        for &(x0, x1, kind, y0) in &open {
            if runs.contains(&(x0, x1, kind)) {
                next.push((x0, x1, kind, y0));
            } else {
                emit(&mut picture, x0, x1, kind, y0, ry);
            }
        }
        for &(x0, x1, kind) in &runs {
            if !next.iter().any(|&(a, b, k, _)| (a, b, k) == (x0, x1, kind)) {
                next.push((x0, x1, kind, ry));
            }
        }
        open = next;
    }
    let centre = game.world.ship.dynamics.centre_of_mass;
    let shift = s as f32 * tile;
    let pivot = (
        centre.x as f32 + offset.x as f32 + shift,
        centre.y as f32 + offset.y as f32 + shift,
    );
    list.append_turned(picture.shapes(), pivot, game.ship_turn() as f32);
}

/// Frame, then deck, then what is standing on them — the same order the
/// design phase paints in, so the two views read as one ship. `skip` is
/// what somebody else draws: the parts the room has pictures for. The
/// hull's own working parts have pictures in `hull`; everything else is its
/// colour, a tile at a time.
///
/// On a planet — `terrain` given — there is no frame to draw and the deck
/// is the ground: the structure layer is skipped, the floor is
/// [`ground_floor`], and the objects are asked of `fittings::part_in`
/// with the biome, so the walls are stone and the trees are the biome's.
fn hull_tiles(
    list: &mut DrawList,
    design: &ShipDesign,
    grid: &Grid,
    firing: hull::Firing,
    skip: &[u32],
    open_airlock: Option<(u32, f32)>,
    terrain: Option<&Terrain>,
) {
    let tile = TILE as f32;
    if let Some(terrain) = terrain {
        ground_floor(list, grid, terrain);
    }
    let biome = terrain.map(|t| t.biome);
    // Not the utility layer: the conduit under the deck is the electricity
    // overlay's to draw, and only while that is up — see `electricity`.
    for layer in [Layer::Structure, Layer::Floor, Layer::Object] {
        if terrain.is_some() && layer != Layer::Object {
            continue;
        }
        for part in &design.parts {
            if part.layer() != layer || skip.contains(&part.id) {
                continue;
            }
            if layer == Layer::Object && hull::part(list, part, grid, firing, open_airlock) {
                continue;
            }
            if crate::fittings::part_in(list, part, biome) {
                continue;
            }
            let color = match layer {
                Layer::Structure => FRAME,
                Layer::Floor => DECK,
                _ => PART_COLORS[part.kind as usize],
            };
            let inset = if layer == Layer::Object { 3.0 } else { 0.0 };
            for (x, y) in part.tiles() {
                let m = tile_middle(x, y);
                // The frame under a corner piece is the same half of the
                // tile, as in the design phase: the edge of the ship is the
                // chamfer, not the square behind it.
                if layer == Layer::Structure
                    && let Some(rotation) = hull::diagonal_at(design, grid, (x, y))
                {
                    let c = hull::corner(rotation);
                    list.triangle(m.x as f32, m.y as f32, tile, tile, c.rot, color);
                    continue;
                }
                list.push(
                    crate::draw::KIND_RECT,
                    m.x as f32,
                    m.y as f32,
                    tile - inset,
                    tile - inset,
                    0.0,
                    if layer == Layer::Object { 4.0 } else { 0.0 },
                    0.0,
                    color,
                );
            }
        }
    }
}

/// The electricity overlay, over the ship's picture: every part that makes,
/// holds or draws power washed and rung — in the live colour when it is on
/// a network with a reactor, in the warning colour when it is not — and
/// the conduit under the deck drawn on top, the way the design phase draws
/// it, so what is wired to what can be followed. The networks are asked
/// once, for the whole ship, rather than a part at a time.
fn electricity(list: &mut DrawList, design: &ShipDesign, grid: &Grid) {
    let tile = TILE as f32;
    let live: Vec<u32> = shipdesign::networks(design)
        .into_iter()
        .filter(|net| net.live())
        .flat_map(|net| net.parts)
        .collect();
    for part in &design.parts {
        let def = part.kind.def();
        if !(def.supplies() || def.draws() || def.stores()) {
            continue;
        }
        let color = if live.contains(&part.id) { LIVE } else { DEAD };
        for (x, y) in part.tiles() {
            let m = tile_middle(x, y);
            list.rect(m.x as f32, m.y as f32, tile, tile, 0.0, color.alpha(0.18));
            list.stroke_rect(
                m.x as f32,
                m.y as f32,
                tile - 3.0,
                tile - 3.0,
                2.0,
                2.0,
                color,
            );
        }
    }
    for part in &design.parts {
        if part.kind != PartKind::PowerConduit {
            continue;
        }
        for at in part.tiles() {
            let links = crate::fittings::conduit_links(design, grid, at);
            crate::fittings::conduit(list, at, links);
        }
    }
}

/// Every station near enough to be in the picture, drawn where it is and
/// as big as it is, so that one **approaches** rather than appears.
///
/// Three distances, three pictures. Out to `STATION_VISIBLE` a station is a
/// plate the size of its hull with its icon on it — a shape getting nearer,
/// and nothing to simulate. Inside the local frame's radius it is its
/// hull, tile by tile, the parts inside it as their colours. And while the
/// ship is within the residents' range the room aboard it is open, so the
/// fixtures are the room's pictures and the people living there walk about
/// between them — the same way the ship's own room is drawn over its hull.
///
/// A station does not turn, so its picture is turned by the camera alone;
/// the ship's own airlock is mated to the station's while docked, and both
/// are drawn open.
///
/// Returns the residents' bodies, placed and turned like the rest but not
/// drawn: the caller paints them over the ship, since they can be on it.
fn stations(game: &Game, list: &mut DrawList) -> DrawList {
    let mut lifted = DrawList::default();
    let here = game.world.ship.position();
    let turn = game.camera_turn() as f32;
    let docked = game.world.ship.state.alongside();
    // The system's stations — and, on a planet, the settlement the ship
    // is tied up at, which is not among them and is not out there: it is
    // the ground the ship stands on, drawn only while the ship is on it.
    let settlement = docked
        .filter(|&id| world::surface_body(id).is_some())
        .and_then(|id| game.world.station(id));
    let on_the_ground = settlement.is_some();
    for station in game.world.stations.iter().chain(settlement) {
        // From the ground nothing in orbit is in the picture.
        if on_the_ground && station.plan != world::Plan::Surface {
            continue;
        }
        let clearance = station.clearance(here);
        if clearance > world::data::STATION_VISIBLE {
            continue;
        }
        // Where the station's middle lands: system offset, y flipped, then
        // turned with the camera.
        let offset = station.centre().sub(here);
        let at = crate::game::turned(offset.x as f32, -offset.y as f32, turn);
        let side = station.design.build_area as f32 * TILE as f32;
        let middle = (side / 2.0, side / 2.0);
        let residents = game
            .world
            .residents
            .as_ref()
            .filter(|r| r.station == station.id);

        // Somebody else's station is under a black fog until the crew
        // have looked into it — see `bims::sight` — and its room, which
        // draws that fog, is only open within the residents' range. Out
        // to there it stays the plate it was from further off: a shape
        // and a kind, and nothing of what is inside. The crew's own is
        // its hull from the local frame in, as before.
        let stance = game.world.stance(station.id);
        let stranger = stance != Stance::Friendly;
        if clearance > world::data::LOCAL_RADIUS_STATION || (stranger && residents.is_none()) {
            let hull = (station.design.build_area as f32 - 2.0) * TILE as f32;
            let plate = if stranger {
                hull::HULL_UNKNOWN
            } else {
                hull::HULL_FAR
            };
            list.push(
                crate::draw::KIND_RECT,
                at.0,
                at.1,
                hull,
                hull,
                turn,
                8.0,
                0.0,
                plate,
            );
            // An enemy's plate is washed in the enemy red, under its icon:
            // the fog says nothing about who is in there, and from this far
            // off the stance is the one thing worth knowing about a station.
            if stance == Stance::Hostile {
                list.push(
                    crate::draw::KIND_RECT,
                    at.0,
                    at.1,
                    hull,
                    hull,
                    turn,
                    8.0,
                    0.0,
                    ENEMY.alpha(ENEMY_TINT),
                );
            }
            paint_station(list, at.0, at.1, hull * 0.8, station.kind, 6.0);
            continue;
        }

        let grid = station.design.grid();
        // The station's door opens with the ship's: they are one passage.
        let open = docked
            .filter(|&id| id == station.id)
            .and(station.port().map(|p| (p.part_id, game.airlock_ajar)));
        let skip: Vec<u32> = match residents {
            Some(_) => bims::aboard::drawn_by_room(&station.design),
            None => Vec::new(),
        };
        // On a planet the settlement's deck is the ground: no rim round
        // it — the ground goes on past its edge — and its floor and its
        // walls drawn as the biome has them (`Terrain`).
        let terrain = (station.plan == world::Plan::Surface)
            .then(|| world::surface_body(station.id))
            .flatten()
            .and_then(|body| game.world.surface(body))
            .map(|surface| Terrain::of(&station.design, surface.biome));
        let mut picture = DrawList::default();
        if terrain.is_none() {
            hull::shadow(&mut picture, &station.design, &grid);
        }
        hull_tiles(
            &mut picture,
            &station.design,
            &grid,
            hull::Firing::NONE,
            &skip,
            open,
            terrain.as_ref(),
        );
        hull::lights(&mut picture, &station.design, &grid, game.frame);
        lamp_faces(&mut picture, game, &station.design, Some(station.id));
        // The key on its research desk, lit so the crew can find it: a
        // ring of lights round the desk while the key is there.
        if game.world.station_has_key(station.id) {
            key_lights(&mut picture, &station.design, game.frame);
        }
        list.append_turned_at(picture.shapes(), middle, turn, at);
        if let Some(residents) = residents {
            // Their room is the station's design plus the shift its deck
            // took with the ship on it, so its picture turns about the
            // station's middle where that middle is in the room.
            let shift = residents.aboard.offset;
            let pivot = (middle.0 + shift.x as f32, middle.1 + shift.y as f32);
            let (deck, bodies) = residents.aboard.room.shapes_split();
            list.append_turned_at(deck, pivot, turn, at);
            // Their people go to the caller, to be drawn over the ship:
            // the ship's hull and its room aboard are painted after the
            // stations, and one of them who has followed the crew through
            // the passage would otherwise be under the ship's deck — a
            // name over an empty tile.
            lifted.append_turned_at(bodies, pivot, turn, at);
        }
    }
    lifted
}

/// Where one of a station's residents lands in the camera's units, for the
/// host's name over their head: the same turn and shift the room's picture
/// went through.
pub fn resident_on_screen(game: &Game, who: u32) -> (f32, f32) {
    let Some(residents) = &game.world.residents else {
        return (0.0, 0.0);
    };
    let Some(station) = game.world.station(residents.station) else {
        return (0.0, 0.0);
    };
    let turn = game.camera_turn();
    let side = station.design.build_area as f64 * TILE as f64;
    let (x, y) = on_screen(
        residents.aboard.position(who),
        dvec2(side / 2.0, side / 2.0),
        turn,
    );
    let offset = station.centre().sub(game.world.ship.position());
    let at = crate::game::turned(offset.x as f32, -offset.y as f32, turn as f32);
    (x + at.0, y + at.1)
}

/// The three parallax layers.
///
/// In screen pixels, turned back into camera units on the way out — a
/// backdrop covers the window rather than the world, so it does not tile four
/// hundred times over when the view is zoomed out. How far each layer has
/// streamed is the field's own clock (`Starfield::advance`, from
/// `ship_render`); this only draws it where it has got to.
fn starfield(game: &Game, list: &mut DrawList) {
    let camera = &game.ship_view;
    let scale = camera.scale().max(1e-9);

    // The screen pixels to cover. The window, unless the field is about to be
    // turned round the ship: then a square about the ship's own pixel wide
    // enough to reach the furthest corner, or the corners would be bare
    // after the turn. Worked out from the corners rather than the diagonal
    // because the pan has the ship off the middle.
    let (x0, y0, x1, y1) = if game.head_up {
        let (ox, oy) = (camera.offset_x() as f64, camera.offset_y() as f64);
        let reach = ox
            .max(camera.width as f64 - ox)
            .hypot(oy.max(camera.height as f64 - oy));
        (ox - reach, oy - reach, ox + reach, oy + reach)
    } else {
        (0.0, 0.0, camera.width as f64, camera.height as f64)
    };
    // The tile the first repeat sits in, and how many repeats reach the far
    // side. A repeat is `FIELD` wide, so the one holding `x0` starts at the
    // multiple of `FIELD` at or below it.
    let (first_x, first_y) = ((x0 / FIELD).floor() as i32, (y0 / FIELD).floor() as i32);
    let across = ((x1 - x0) / FIELD).ceil() as i32 + 1;
    let down = ((y1 - y0) / FIELD).ceil() as i32 + 1;

    for (i, layer) in game.stars.layers.iter().enumerate() {
        let slide = game.stars.slid[i];
        for speck in layer {
            let base = dvec2(
                (speck.at.x + slide.x).rem_euclid(FIELD),
                (speck.at.y + slide.y).rem_euclid(FIELD),
            );
            for tx in first_x..first_x + across {
                for ty in first_y..first_y + down {
                    let px = base.x + tx as f64 * FIELD;
                    let py = base.y + ty as f64 * FIELD;
                    if px < x0 || px > x1 || py < y0 || py > y1 {
                        continue;
                    }
                    // Screen pixels back into the camera's own units, so the
                    // speck stays the same size on screen at any zoom.
                    let x = (px as f32 - camera.offset_x()) / scale;
                    let y = (py as f32 - camera.offset_y()) / scale;
                    let size = speck.size / scale;
                    list.ellipse(
                        x,
                        y,
                        size,
                        size,
                        Color::rgba(1.0, 1.0, 1.0, speck.brightness),
                    );
                }
            }
        }
    }
}

/// The body the ship is alongside, drawn where it actually is: the ground.
///
/// **Never turned.** A planet does not tip over because the ship holding
/// beside it has rolled. Stations used to be drawn here too, a ring the
/// hull sat inside; they are places now, with hulls of their own, and
/// [`stations`] draws every one in range where it stands.
fn local_node(game: &Game, list: &mut DrawList) {
    let Some(node) = game.world.ship.frame.node() else {
        return;
    };
    let Some(at) = game.world.system.absolute_position(node) else {
        return;
    };
    let offset = at.sub(game.world.ship.position());
    // System `+y` is north and the screen's `y` grows down.
    let (x, y) = (offset.x as f32, -offset.y as f32);
    // Sized against the hull: a station is something the ship fits inside
    // the ring of, and a planet is something the ship is a speck against.
    let hull = game.world.ship.design.build_area as f32 * TILE as f32;
    match node {
        Node::Station(_) => {}
        Node::Body(id) => {
            // A belt the ship is holding at is drawn as its rocks, tile by
            // tile in the ship's frame — see `rocks` — not as the handful
            // of stones the map's icon is.
            if game.world.site_here().is_some() {
                return;
            }
            // Coming down onto it, the planet grows under the ship until
            // it is the whole window; lifting off, it shrinks back to the
            // size it is held beside at. Geometric, so the growth reads
            // the same all the way down.
            let growth = match (game.world.landing(), game.world.lifting()) {
                (Some((_, done)), _) => LANDING_GROWTH.powf(done as f32),
                (_, Some((_, done))) => LANDING_GROWTH.powf(1.0 - done as f32),
                _ => 1.0,
            };
            if let Some(body) = game.world.system.body(id) {
                paint_body(list, x, y, hull * 4.0 * growth, body.kind, 6.0 * growth);
            }
        }
    }
}

// --- what a planet and a station look like --------------------------------------
//
// One drawing of each kind, used at two scales: a few pixels across on the
// map, and tiles across when the ship is alongside. Everything is ellipses
// and rectangles, because that is all the draw format has, and everything is
// worked out from `size` — the diameter — so the same picture reads at both.
// `thin` is a line width in the caller's units, since a hairline on the map
// is a different number from a hairline alongside.

const RIM: Color = Color::rgba(0.02, 0.03, 0.04, 0.55);
const LIGHT: Color = Color::rgba(1.0, 1.0, 1.0, 0.16);
const SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 0.18);

fn disc(list: &mut DrawList, x: f32, y: f32, d: f32, color: Color) {
    list.ellipse(x, y, d, d, color);
}

fn ring(list: &mut DrawList, x: f32, y: f32, w: f32, h: f32, rot: f32, thin: f32, color: Color) {
    list.push(crate::draw::KIND_ELLIPSE, x, y, w, h, rot, 0.0, thin, color);
}

/// A planet or a belt, `size` across, centred on `(x, y)`.
pub fn paint_body(list: &mut DrawList, x: f32, y: f32, size: f32, kind: BodyKind, thin: f32) {
    let color = body_color(Some(kind));
    let r = size / 2.0;
    match kind {
        BodyKind::RockyPlanet => {
            disc(list, x, y, size, color);
            // Continents: three darker patches, and a lit limb.
            for (dx, dy, dw, dh) in [
                (-0.25, -0.15, 0.40, 0.28),
                (0.20, 0.10, 0.34, 0.36),
                (-0.05, 0.36, 0.26, 0.18),
            ] {
                list.ellipse(x + dx * r, y + dy * r, dw * size, dh * size, SHADE);
            }
            list.ellipse(x - 0.3 * r, y - 0.3 * r, 0.42 * size, 0.42 * size, LIGHT);
            ring(list, x, y, size, size, 0.0, thin, RIM);
        }
        BodyKind::GasGiant => {
            disc(list, x, y, size, color);
            // Bands across the face, darker towards the poles, then a ring
            // seen a little from above.
            for (dy, dh, dark) in [
                (-0.55, 0.16, 0.22),
                (-0.15, 0.12, 0.10),
                (0.30, 0.18, 0.16),
                (0.65, 0.12, 0.24),
            ] {
                let half = (1.0f32 - dy * dy).max(0.0).sqrt();
                list.ellipse(
                    x,
                    y + dy * r,
                    half * size * 0.98,
                    dh * size,
                    SHADE.alpha(dark),
                );
            }
            list.ellipse(x - 0.3 * r, y - 0.35 * r, 0.4 * size, 0.3 * size, LIGHT);
            ring(list, x, y, size, size, 0.0, thin, RIM);
            ring(
                list,
                x,
                y,
                size * 1.9,
                size * 0.42,
                -0.3,
                thin * 1.5,
                color.alpha(0.55),
            );
        }
        BodyKind::IceWorld => {
            disc(list, x, y, size, color);
            // A bright cap and a bright limb: the whole thing reads as glare.
            list.ellipse(
                x,
                y - 0.62 * r,
                0.6 * size,
                0.3 * size,
                Color::rgba(1.0, 1.0, 1.0, 0.45),
            );
            list.ellipse(
                x + 0.2 * r,
                y + 0.25 * r,
                0.3 * size,
                0.2 * size,
                SHADE.alpha(0.12),
            );
            list.ellipse(x - 0.3 * r, y - 0.2 * r, 0.4 * size, 0.4 * size, LIGHT);
            ring(list, x, y, size, size, 0.0, thin, RIM);
        }
        BodyKind::AsteroidBelt => {
            // Measured as a point, drawn as a handful of rocks about it — at
            // fixed angles and sizes, so it holds still between frames.
            for (i, scale) in [0.9f32, 0.6, 1.0, 0.5, 0.75, 0.55, 0.8]
                .into_iter()
                .enumerate()
            {
                let a = i as f32 * core::f32::consts::TAU / 7.0 + 0.5;
                let (dx, dy) = (a.cos() * 0.62 * r, a.sin() * 0.55 * r);
                let d = 0.22 * size * scale;
                list.push(
                    crate::draw::KIND_ELLIPSE,
                    x + dx,
                    y + dy,
                    d,
                    d * 0.75,
                    a,
                    0.0,
                    0.0,
                    color,
                );
            }
        }
    }
}

/// The pickaxe's two parts: a wooden haft and a steel head.
const HAFT: Color = Color::rgb(0.66, 0.44, 0.24);
const HEAD: Color = Color::rgb(0.90, 0.90, 0.86);

/// Where a landable planet's pad sits, as a share of the icon size out
/// from its middle: past the stance ring's edge (`STANCE_RING / 2`, 0.65)
/// and the reticle round a ship docked at the planet's own station
/// (`HERE_RING`, 44 pixels across on a 26-pixel icon), so the glyph stands
/// clear of both rather than under one.
const PAD_SHOULDER: f32 = 0.95;
/// The slab under a landing pad's plate on the map: the void's own dark,
/// so the plate has an edge against whatever is behind it.
const PAD_SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 0.55);

/// A landing pad, `size` across its box, centred on `(x, y)`: a plate
/// across the foot of the box in `colour` — the side's, so the pad says
/// whose ground it is like the ring does — and an arrow coming straight
/// down onto it in the pickaxe's light, a shaft and the two arms of its
/// head. The mark of a planet that can be landed on, at its shoulder on
/// the map, as the pickaxe is a belt's. Rectangles alone, as that is.
pub fn paint_pad(list: &mut DrawList, x: f32, y: f32, size: f32, colour: Color) {
    use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    let r = size / 2.0;
    // The plate: wide and flat along the foot, rounded at the ends, with
    // a dark slab under it so it reads as a thing with an edge.
    let plate = 0.24 * size;
    let py = y + r - plate / 2.0;
    list.rect(x, py + 0.07 * size, size, plate, plate / 2.0, PAD_SHADE);
    list.rect(x, py, size, plate, plate / 2.0, colour);
    // The arrow: a shaft down from the top of the box to just over the
    // plate, and the head's two arms leaning up and out from its tip.
    let width = 0.16 * size;
    let (top, tip) = (y - r, py - plate / 2.0 - 0.08 * size);
    let shaft = tip - top;
    list.push(
        crate::draw::KIND_RECT,
        x,
        top + shaft / 2.0,
        width,
        shaft,
        0.0,
        width / 2.0,
        0.0,
        HEAD,
    );
    let arm = 0.5 * size;
    for side in [1.0f32, -1.0] {
        let a = -FRAC_PI_2 + side * FRAC_PI_4;
        let (ax, ay) = (a.cos(), a.sin());
        list.push(
            crate::draw::KIND_RECT,
            x + 0.5 * arm * ax,
            tip + 0.5 * arm * ay,
            arm,
            width,
            a,
            width / 2.0,
            0.0,
            HEAD,
        );
    }
}

/// A pickaxe, `size` across its box, centred on `(x, y)`: the haft up from
/// bottom left to top right, and the head across its top end, the two
/// halves of it bent back down towards the haft the way a pick's are. The
/// mark of a mining site on the map. Rectangles alone — turned, and
/// rounded at the ends — since that is what the format has.
pub fn paint_pickaxe(list: &mut DrawList, x: f32, y: f32, size: f32) {
    use core::f32::consts::FRAC_PI_4;
    let r = size / 2.0;
    // Along the haft, bottom left to top right, in the screen's y-down.
    let up = -FRAC_PI_4;
    let (ux, uy) = (up.cos(), up.sin());
    // The haft: from a little inside the bottom-left corner to the head.
    let (haft, width) = (1.7 * r, 0.14 * size);
    let (hx, hy) = (x - 0.1 * r * ux, y - 0.1 * r * uy);
    list.push(
        crate::draw::KIND_RECT,
        hx,
        hy,
        haft,
        width,
        up,
        width / 2.0,
        0.0,
        HAFT,
    );
    // The head sits on the haft's top end and reaches out either side of
    // it, each half bent a little back towards the haft.
    let (tx, ty) = (hx + 0.5 * haft * ux, hy + 0.5 * haft * uy);
    let (half, thick) = (0.44 * size, 0.15 * size);
    let bend = 0.45;
    for side in [1.0f32, -1.0] {
        let a = up + side * (core::f32::consts::FRAC_PI_2 + bend);
        let (ax, ay) = (a.cos(), a.sin());
        list.push(
            crate::draw::KIND_RECT,
            tx + 0.5 * half * ax,
            ty + 0.5 * half * ay,
            half,
            thick,
            a,
            thick / 2.0,
            0.0,
            HEAD,
        );
    }
    // The boss where the head meets the haft, so the join reads as one piece.
    disc(list, tx, ty, thick * 1.3, HEAD);
}

/// A station, `size` across, centred on `(x, y)`.
pub fn paint_station(list: &mut DrawList, x: f32, y: f32, size: f32, kind: StationKind, thin: f32) {
    let color = station_color(Some(kind));
    let r = size / 2.0;
    match kind {
        StationKind::Orbital => {
            // A wheel: a ring, a hub, and four spokes.
            ring(list, x, y, size, size, 0.0, thin * 2.0, color);
            disc(list, x, y, 0.36 * size, color);
            for i in 0..2 {
                let a = i as f32 * core::f32::consts::FRAC_PI_2;
                list.push(
                    crate::draw::KIND_RECT,
                    x,
                    y,
                    size,
                    thin * 1.2,
                    a,
                    0.0,
                    0.0,
                    color.alpha(0.8),
                );
            }
        }
        StationKind::Refinery => {
            // Two tanks side by side, a stack between them, and a flare.
            for dx in [-0.42f32, 0.42] {
                list.ellipse(x + dx * r, y + 0.2 * r, 0.52 * size, 0.56 * size, color);
                ring(
                    list,
                    x + dx * r,
                    y + 0.2 * r,
                    0.52 * size,
                    0.56 * size,
                    0.0,
                    thin,
                    RIM,
                );
            }
            list.rect(
                x,
                y - 0.2 * r,
                0.16 * size,
                0.9 * size,
                thin,
                color.alpha(0.9),
            );
            disc(
                list,
                x,
                y - 0.72 * r,
                0.24 * size,
                Color::rgb(1.0, 0.72, 0.30),
            );
        }
        StationKind::MiningOutpost => {
            // A rig set into its rock: a square turned on its corner, with
            // the rubble it is working beside it.
            list.push(
                crate::draw::KIND_RECT,
                x,
                y,
                0.6 * size,
                0.6 * size,
                core::f32::consts::FRAC_PI_4,
                thin,
                0.0,
                color,
            );
            ring(
                list,
                x,
                y,
                0.6 * size,
                0.6 * size,
                core::f32::consts::FRAC_PI_4,
                thin,
                RIM,
            );
            for (dx, dy, d) in [(0.55f32, -0.35, 0.28), (0.62, 0.3, 0.2), (-0.6, 0.4, 0.22)] {
                disc(
                    list,
                    x + dx * r,
                    y + dy * r,
                    d * size,
                    body_color(Some(BodyKind::AsteroidBelt)),
                );
            }
        }
        StationKind::Derelict => {
            // A wheel that has come apart: the ring dim and broken — drawn
            // as three arcs' worth of short straight pieces, since the
            // format has no arcs and painting a gap over it would paint
            // over whatever is underneath — a dark hub, and a scatter of
            // what came off.
            let pieces = 10;
            for i in 0..pieces {
                if i == 2 || i == 3 {
                    continue; // the bite
                }
                let a = i as f32 * core::f32::consts::TAU / pieces as f32;
                let (px, py) = (x + a.cos() * r, y + a.sin() * r);
                let len = core::f32::consts::TAU * r / pieces as f32 * 0.85;
                list.push(
                    crate::draw::KIND_RECT,
                    px,
                    py,
                    thin * 2.0,
                    len,
                    a,
                    0.0,
                    0.0,
                    color.alpha(0.7),
                );
            }
            disc(list, x, y, 0.3 * size, color.alpha(0.5));
            ring(
                list,
                x,
                y,
                0.3 * size,
                0.3 * size,
                0.0,
                thin,
                color.alpha(0.7),
            );
            for (dx, dy, d) in [
                (0.9f32, -0.95, 0.12),
                (1.15, -0.55, 0.09),
                (0.65, -1.2, 0.08),
            ] {
                disc(list, x + dx * r, y + dy * r, d * size, color);
            }
        }
        StationKind::Relay => {
            // A hub, a mast, and a dish looking out.
            disc(list, x, y + 0.2 * r, 0.34 * size, color);
            list.rect(x, y - 0.2 * r, thin * 1.5, 0.6 * size, 0.0, color);
            ring(
                list,
                x,
                y - 0.55 * r,
                0.7 * size,
                0.34 * size,
                0.0,
                thin * 1.6,
                color,
            );
            disc(list, x, y - 0.55 * r, 0.1 * size, color);
        }
    }
}

// --- the map -----------------------------------------------------------------

fn paint_map(game: &Game, list: &mut DrawList) {
    let camera = &game.map_view;
    let scale = camera.scale().max(1e-30) as f64;
    let here = game.world.ship.position();

    let half_w = camera.width as f64 / scale;
    let half_h = camera.height as f64 / scale;
    list.rect(
        0.0,
        0.0,
        (half_w * 3.0) as f32,
        (half_h * 3.0) as f32,
        0.0,
        VOID,
    );

    // The whole of the system is out there, so the whole of it is turned with
    // the camera at the end — square to the window unless the view is head
    // up. The marker for the ship is drawn after, through its own turn.
    let out_there = list.len();

    // Where a system position lands, in the camera's units about the ship.
    let place = |at: DVec2| {
        let offset = at.sub(here);
        (offset.x as f32, -offset.y as f32)
    };

    // How far the crew can see. Drawn round the ship because that is where the
    // sensors are, and it is the one thing on the map that explains why the
    // rest of it is empty.
    let range = game.world.detection_range() as f32;
    list.push(
        crate::draw::KIND_ELLIPSE,
        0.0,
        0.0,
        range * 2.0,
        range * 2.0,
        0.0,
        0.0,
        (2.0 / scale) as f32,
        MUTED,
    );

    // The star. Always discovered: it is the thing the system is named after
    // and the origin everything else is measured from.
    let (sx, sy) = place(DVec2::ZERO);
    let star = (18.0 / scale) as f32;
    list.ellipse(sx, sy, star, star, STAR);

    // Orbits under everything: a faint ring through each known body, so the
    // map reads as a system rather than as dots. A belt's is a little
    // stronger, because a belt *is* its orbit.
    let thin = (1.0 / scale) as f32;
    for &node in &game.world.discovered {
        let Node::Body(id) = node else { continue };
        let Some(body) = game.world.system.body(id) else {
            continue;
        };
        let d = (body.position.length() * 2.0) as f32;
        let strength = if body.kind == BodyKind::AsteroidBelt {
            0.30
        } else {
            0.12
        };
        ring(list, sx, sy, d, d, 0.0, thin, MUTED.alpha(strength));
    }

    // The bodies first and the stations over them, because a station sits
    // in orbit of its body and at this scale that is on top of it. Sized in
    // pixels, like the icons they are: a planet drawn to scale would be a
    // fraction of one.
    let size = (26.0 / scale) as f32;
    for &node in &game.world.discovered {
        let Node::Body(id) = node else { continue };
        let Some(at) = game.world.system.absolute_position(node) else {
            continue;
        };
        let (x, y) = place(at);
        if let Some(body) = game.world.system.body(id) {
            paint_body(list, x, y, size, body.kind, thin);
            // A belt is a mining site — hold station at it and the rocks
            // are laid out about the ship, and nothing else stands at one
            // — and the map says so with a pickaxe at its shoulder.
            if body.kind == BodyKind::AsteroidBelt {
                paint_pickaxe(list, x + 0.62 * size, y - 0.62 * size, size * 0.6);
            }
            // A planet with ground has a settlement on it the ship can
            // land at, and the map says so twice: a landing pad at its
            // shoulder, where a belt has its pickaxe — the one mark that
            // says *this one can be set down on* and a gas giant cannot —
            // and a ring by its side the way it rings a station, since
            // whose it is is the other thing worth knowing before coming
            // down. The pad is in the side's colour too, so the two agree.
            if let Some(surface) = game.world.surface(id) {
                let colour = match game.world.stance(surface.id) {
                    Stance::Hostile => ENEMY,
                    Stance::Friendly | Stance::Neutral => FRIEND,
                };
                let d = size * STANCE_RING;
                ring(list, x, y, d, d, 0.0, thin * 1.5, colour.alpha(0.9));
                paint_pad(
                    list,
                    x + PAD_SHOULDER * size,
                    y - PAD_SHOULDER * size,
                    size * 0.8,
                    colour,
                );
            }
        }
    }
    for &node in &game.world.discovered {
        let Node::Station(id) = node else { continue };
        let Some(at) = game.world.system.absolute_position(node) else {
            continue;
        };
        let (x, y) = place(at);
        let Some(station) = game.world.system.station(id) else {
            continue;
        };
        paint_station(list, x, y, size * 0.75, station.kind, thin * 1.5);
        // Ringed by stance, outside the aim ring so the two read apart when
        // the helm is pointed at an enemy's: red for a hostile station, blue
        // for any other somebody lives on — home and a stranger's alike —
        // and nothing for a derelict, which is nobody's. This is what the
        // map says about who lives where; `World::stance` is the one rule,
        // and the plates out of the window agree with it.
        if station.kind == StationKind::Derelict {
            continue;
        }
        let colour = match game.world.stance(id) {
            Stance::Hostile => ENEMY,
            Stance::Friendly | Stance::Neutral => FRIEND,
        };
        let d = size * STANCE_RING;
        ring(list, x, y, d, d, 0.0, thin * 1.5, colour.alpha(0.9));
    }

    // What the helm is aimed at, ringed, so a click has visibly landed on
    // the thing and not beside it.
    let aimed_at = match game.aimed {
        Some(flight::Target::Body(id)) => game.world.system.absolute_position(Node::Body(id)),
        Some(flight::Target::Station(id)) => game.world.system.absolute_position(Node::Station(id)),
        Some(flight::Target::Point(p)) => Some(p),
        None => None,
    };
    if let Some(at) = aimed_at {
        let (x, y) = place(at);
        let d = (26.0 / scale) as f32;
        ring(list, x, y, d, d, 0.0, (1.5 / scale) as f32, GLOW.alpha(0.9));
    }

    // The route, in the colour of whoever set it.
    if let ShipState::Travelling { plan, .. } = &game.world.ship.state {
        let colour = player_color(game.world.ship.destination_set_by.unwrap_or(0));
        let (x0, y0) = place(plan.start);
        let (x1, y1) = place(plan.arrival);
        list.line(x0, y0, x1, y1, (2.0 / scale) as f32, colour);
        let end = (7.0 / scale) as f32;
        list.push(
            crate::draw::KIND_ELLIPSE,
            x1,
            y1,
            end * 2.0,
            end * 2.0,
            0.0,
            0.0,
            (2.0 / scale) as f32,
            colour,
        );
    }

    list.turn_from(out_there, game.camera_turn() as f32);

    // Where you are, over everything: a reticle round the ship — a ring
    // wider than any icon, so it stands out from the station the ship is
    // docked on top of, with a tick at each compass point and a breathing
    // wash inside it — and the ship itself on top, pointing where it is
    // pointing: a little hull with fins, so it says which way round it is
    // and that it is the ship. Head up, that is straight up, and it is the
    // map that says where north went. The reticle is not turned with the
    // world: it is the screen's, and its ticks stay square to the window.
    here_reticle(list, scale as f32, game.frame);
    hull::marker(list, game.ship_turn() as f32, (18.0 / scale) as f32, GLOW);
}

/// How wide the reticle round the ship is on the map, in pixels: outside
/// a station's stance ring (`26 * 0.75 * STANCE_RING`, about 25), so the
/// two never sit on each other at a dock.
const HERE_RING: f32 = 44.0;
/// How long a tick of it is, and how far outside the ring it starts.
const HERE_TICK: f32 = 9.0;
/// How many frames one breath of its wash takes.
const HERE_PULSE: f32 = 120.0;

/// The mark that says *you are here* on the map, about the origin — the
/// ship — in the camera's units so it comes out the same size at any zoom.
fn here_reticle(list: &mut DrawList, scale: f32, frame: u32) {
    let px = 1.0 / scale;
    let d = HERE_RING * px;
    let breath = 0.5 + 0.5 * (frame as f32 / HERE_PULSE * core::f32::consts::TAU).sin();
    // The wash: a soft disc that breathes, so the eye is drawn to it even
    // on a busy map, and never so strong that the icon under it is lost.
    list.ellipse(0.0, 0.0, d, d, GLOW.alpha(0.06 + 0.10 * breath));
    ring(list, 0.0, 0.0, d, d, 0.0, 2.0 * px, GLOW.alpha(0.95));
    // Four ticks, outside the ring at the compass points of the window.
    let (from, to) = (d / 2.0 + 2.0 * px, d / 2.0 + 2.0 * px + HERE_TICK * px);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        list.line(
            dx * from,
            dy * from,
            dx * to,
            dy * to,
            2.0 * px,
            GLOW.alpha(0.95),
        );
    }
}

fn body_color(kind: Option<BodyKind>) -> Color {
    match kind {
        Some(BodyKind::RockyPlanet) => Color::rgb(0.72, 0.56, 0.44),
        Some(BodyKind::GasGiant) => Color::rgb(0.82, 0.70, 0.44),
        Some(BodyKind::IceWorld) => Color::rgb(0.66, 0.84, 0.92),
        Some(BodyKind::AsteroidBelt) => Color::rgb(0.55, 0.55, 0.58),
        None => Color::rgb(0.6, 0.6, 0.6),
    }
}

fn station_color(kind: Option<StationKind>) -> Color {
    match kind {
        Some(StationKind::Orbital) => Color::rgb(0.58, 0.82, 0.90),
        Some(StationKind::Refinery) => Color::rgb(0.86, 0.62, 0.30),
        Some(StationKind::MiningOutpost) => Color::rgb(0.70, 0.66, 0.46),
        Some(StationKind::Derelict) => Color::rgb(0.52, 0.48, 0.50),
        Some(StationKind::Relay) => Color::rgb(0.62, 0.74, 0.92),
        None => Color::rgb(0.6, 0.6, 0.6),
    }
}

/// Which phase to colour a readout by. Not drawn here — the host does the
/// words — but the codes have to come from somewhere and this is where the
/// rest of the view's vocabulary lives.
pub fn phase_code(game: &Game) -> u32 {
    game.world
        .trip_state()
        .map(|state| state.phase.code())
        .unwrap_or(Phase::Arrived.code())
}

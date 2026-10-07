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
//! Anything drawn because it is *out there* — a station
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
use shipdesign::parts::{Layer, PartKind, Rotation, TILE};
use shipdesign::{Grid, PlacedPart, ShipDesign};
use world::Biome;
use worldgen::math::{DVec2, dvec2};
use worldgen::{BodyKind, Node, StationKind};

use crate::draw::{Color, DrawList, Surface};
use crate::game::{Game, ViewMode};
use crate::hull;
use crate::paint::PART_COLORS;

const VOID: Color = Color::rgb(0.02, 0.03, 0.04);
const FRAME: Color = Color::rgb(0.10, 0.11, 0.13);
const DECK: Color = Color::rgb(0.13, 0.15, 0.18);
const GLOW: Color = Color::rgb(0.38, 0.86, 0.95);
const STAR: Color = Color::rgb(0.98, 0.88, 0.55);
const MUTED: Color = Color::rgba(0.55, 0.85, 0.95, 0.22);
/// A blueprint under the pointer that would go down, and one that would
/// not.
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
/// A site the machines are coming for, on the map (task 111): a warning
/// amber, between the enemy's red and the friend's blue, since it is a
/// friend's place with the enemy on the way.
const DEFEND: Color = Color::rgb(1.0, 0.70, 0.18);
/// A trader, on the map (task 114's sites, ringed since task 111): the
/// green of money, told from the defence's amber and the aim ring's cyan.
const TRADE: Color = Color::rgb(0.40, 0.90, 0.46);

/// The ring a site of this system wears on the map (task 111): what it is
/// to the crew, off `Game::map_sites` — an attack in the enemy's red, a
/// defence the machines are still coming for in amber, a trader in green
/// — and how its fight stands: an attack cleared is faded, a defence
/// fought and held is blue as a friend's, and a trader shut while its
/// system is the machines' is faded.
fn site_ring(site: &crate::game::MapSite) -> Color {
    match site.kind {
        world::SiteKind::Attack if site.cleared => ENEMY.alpha(0.4),
        world::SiteKind::Attack => ENEMY,
        world::SiteKind::Defend if site.threatened => DEFEND,
        world::SiteKind::Defend => FRIEND,
        world::SiteKind::Trader if site.closed => TRADE.alpha(0.4),
        world::SiteKind::Trader => TRADE,
    }
}
/// An **elite** (`world::elite`), on the map: a crown over its icon and
/// a second ring outside the attack's, in a magenta no other mark on
/// either map is — the galaxy chart crowns the elite's star the same
/// way (`lobby::preview`'s `ELITE`, by value). Faded once cleared.
pub const ELITE: Color = Color::rgb(1.0, 0.40, 0.90);
/// How far over the icon's middle the crown's foot sits, and how wide
/// the crown is, as shares of the icon size.
const CROWN_LIFT: f32 = 0.95;
const CROWN_WIDTH: f32 = 0.9;

/// The stance ring on the map, as a share of the icon size: outside the
/// aim ring (which is the icon size across), so the two never sit on each
/// other when an enemy's station is the one picked.
const STANCE_RING: f32 = 1.3;

/// Somewhere the crew have already been, on the map (feature 85): a
/// small tick at the node's upper-left shoulder — the upper-right one is
/// the settlement's pad, and under the icon is
/// where its name is written. The pale white the galaxy chart
/// rings a visited star in (`lobby::preview`'s `VISITED`), written out
/// here for the reason the enemy's red is: this crate imports neither
/// the lobby nor the room. A grey until the second map rework, which was lost on the
/// map; the tick sits on a dark disc now, so it reads over any icon.
const VISITED: Color = Color::rgb(0.86, 0.92, 1.0);
/// How far out the tick sits, as a share of the icon size, and how big
/// it is drawn.
const TICK_SHOULDER: f32 = 0.85;
const TICK_SIZE: f32 = 0.62;
/// The site the crew will go to next, on the map (the second map rework): the
/// hyperdrive's violet the galaxy chart rings the picked star in
/// (`lobby::preview`'s `TARGET`), by value.
const HEADING: Color = Color::rgb(0.62, 0.42, 0.86);

/// A construction site: the blueprint's blue, and how far through the
/// part's own picture shows for a site and for the blueprint in hand.
const BLUEPRINT: Color = Color::rgb(0.45, 0.72, 1.0);
const SITE_FADE: f32 = 0.55;
const GHOST_FADE: f32 = 0.50;
/// Where whoever uses a part would stand, as the designer marks it.
const SPOT: Color = Color::rgba(0.98, 0.82, 0.35, 0.85);
/// The ground, when the ship is on a planet (`World::landed`), by the
/// settlement's biome (`world::Biome`): desert sand, a warm ochre;
/// temperate grass, a muted green; arctic snow, a cold blue-grey — each
/// dark enough that the ship's deck and the bodies on the ground read on
/// it, and each **the same colour the town's outdoor floor is drawn in**
/// (`ground_floor`), so where the settlement's deck ends is invisible
/// and the wild ring is what marks the edge of the ground. Each is the
/// average of its texture ([`ground_surface`]), which makes it ground and
/// not a colour. Taken down a quarter from (0.58, 0.46,
/// 0.30), (0.36, 0.46, 0.28) and (0.64, 0.70, 0.76) when the player found
/// the Bims hard to pick out on it; the snow then brought back up
/// halfway (from 0.47, 0.51, 0.56), the player finding it too dark. The
/// pad under the ship: concrete, with a lighter border.
const SAND: Color = Color::rgb(0.44, 0.35, 0.23);
const GRASS: Color = Color::rgb(0.27, 0.35, 0.21);
const SNOW: Color = Color::rgb(0.56, 0.61, 0.67);
/// The floor inside a town's buildings: boards, warmer than a deck.
const FLOORBOARD: Color = Color::rgb(0.36, 0.28, 0.20);
/// What is scattered over the ground between the wild: a tuft of grass
/// and, rarely, a flower on it; a ripple in the sand or a pebble; a
/// drift of snow.
const TUFT: Color = Color::rgba(0.16, 0.26, 0.12, 0.7);
const FLOWER: Color = Color::rgb(0.94, 0.82, 0.70);
const RIPPLE: Color = Color::rgba(1.0, 0.94, 0.80, 0.30);
const PEBBLE: Color = Color::rgb(0.35, 0.27, 0.18);
const DRIFT: Color = Color::rgba(0.80, 0.88, 0.96, 0.55);
/// Roughly one outdoor tile in this many carries a decoration.
const DECORATED_ONE_IN: u32 = 6;
const PAD: Color = Color::rgb(0.24, 0.25, 0.27);
const PAD_EDGE: Color = Color::rgba(1.0, 1.0, 1.0, 0.22);
/// How far past the hull the pad reaches, in tiles.
const PAD_MARGIN: f32 = 1.5;

/// One colour per lobby slot, the slot's own of `character::Tint::ALL`
/// — the colour a session deals it — so another player's pointer in the
/// yard and the lobby is the colour their Bim is ringed in.
pub fn player_color(slot: u32) -> Color {
    let tints = bims::character::Tint::ALL;
    let (r, g, b) = tints[slot as usize % tints.len()].rgb();
    Color::rgb(r, g, b)
}

/// The whole frame, whichever view is up.
pub fn paint(game: &Game, list: &mut DrawList) {
    paint_with(game, list, &[]);
}

/// The same, with the stations' own pictures already to hand — the ones
/// `Session::render` keeps from frame to frame ([`KeptStation`]), each
/// checked against its station this frame: any it has is used and any it
/// lacks is built here, so the frame is the one [`paint`] draws either
/// way. [`paint`] itself keeps nothing, so no caller of it can be handed a
/// picture of a design that has changed since.
pub(crate) fn paint_with(game: &Game, list: &mut DrawList, prebuilt: &[KeptStation]) {
    list.clear();
    match game.mode {
        ViewMode::Ship => paint_ship(game, list, prebuilt),
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

/// Any design point in the camera's units — another player's pointer,
/// which arrives as a point on the ship's grid (`Session::design_point`)
/// and is drawn where that tile is on this screen. `Game::design_point_at`
/// read forwards.
pub fn design_on_screen(game: &Game, x: f32, y: f32) -> (f32, f32) {
    on_screen(
        dvec2(x as f64, y as f64),
        game.world.ship.dynamics.centre_of_mass,
        game.ship_turn(),
    )
}

/// Where one of the crew lands in the camera's units, for the host's name
/// over their head. The same arithmetic the room's picture is turned with.
pub fn crew_on_screen(game: &Game, who: u32) -> (f32, f32) {
    on_screen(
        game.world.aboard.shown_position(who),
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

/// A piece of a fog picture for the host to draw: the part of the
/// texture between `uv0` and `uv1` (nought to one across the map), at
/// `corners` in the camera's units — the piece's origin, then clockwise,
/// the way `light_map_on_screen` orders them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FogPiece {
    pub uv0: (f32, f32),
    pub uv1: (f32, f32),
    pub corners: [(f32, f32); 4],
}

/// The plain's fog: every chunk picture the room composed for this
/// frame's window (`bims::terrain::Plane::pictures`), by its chunk, with
/// the pieces of it to draw — the chunk less the room's box, which the
/// light map covers, cut into at most four rectangles so no fog is ever
/// laid twice; the picture's apron is never drawn. Empty off a planet.
pub fn plain_fog_on_screen(
    game: &Game,
) -> Vec<((i32, i32), &bims::sight::LightMap, Vec<FogPiece>)> {
    let room = &game.world.aboard.room;
    let Some(plane) = room.plane() else {
        return Vec::new();
    };
    let tile = TILE as f64;
    let offset = game.world.aboard.offset;
    let centre = game.world.ship.dynamics.centre_of_mass;
    let turn = game.ship_turn();
    let (dx0, dy0, dx1, dy1) = plane.deck();
    room.plain_pictures()
        .into_iter()
        .map(|(key, map)| {
            let (cx0, cy0, cx1, cy1) = bims::terrain::Plane::picture_tiles(key);
            // The chunk less the deck's box: the strips either side of
            // it at full height, and what is left above and below it.
            let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
            let (mx0, mx1) = (cx0.max(dx0), cx1.min(dx1));
            if mx0 >= mx1 || cy0.max(dy0) >= cy1.min(dy1) {
                rects.push((cx0, cy0, cx1, cy1));
            } else {
                if cx0 < mx0 {
                    rects.push((cx0, cy0, mx0, cy1));
                }
                if mx1 < cx1 {
                    rects.push((mx1, cy0, cx1, cy1));
                }
                if cy0 < dy0 {
                    rects.push((mx0, cy0, mx1, dy0));
                }
                if dy1 < cy1 {
                    rects.push((mx0, dy1, mx1, cy1));
                }
            }
            let origin = dvec2(map.origin.x as f64, map.origin.y as f64);
            let size = map.size();
            let (sw, sh) = (size.x as f64, size.y as f64);
            let pieces = rects
                .into_iter()
                .map(|(x0, y0, x1, y1)| {
                    let lo = dvec2(x0 as f64 * tile, y0 as f64 * tile);
                    let hi = dvec2(x1 as f64 * tile, y1 as f64 * tile);
                    let uv = |p: DVec2| {
                        let d = p.sub(origin);
                        ((d.x / sw) as f32, (d.y / sh) as f32)
                    };
                    let at = |p: DVec2| on_screen(p.sub(offset), centre, turn);
                    FogPiece {
                        uv0: uv(lo),
                        uv1: uv(hi),
                        corners: [at(lo), at(dvec2(hi.x, lo.y)), at(hi), at(dvec2(lo.x, hi.y))],
                    }
                })
                .collect();
            (key, map, pieces)
        })
        .collect()
}

/// The middle of a tile, in design world units.
fn tile_middle(x: u32, y: u32) -> DVec2 {
    dvec2(
        (x as f64 + 0.5) * TILE as f64,
        (y as f64 + 0.5) * TILE as f64,
    )
}

fn paint_ship(game: &Game, list: &mut DrawList, prebuilt: &[KeptStation]) {
    let camera = &game.ship_view;
    let scale = camera.scale().max(1e-9);

    // On a planet, the ground first, big enough to cover the canvas at
    // any pan: there is no space down there, and the planet is the whole
    // of the surroundings. At a station nothing: the window's picture
    // behind the deck is the app's (`Game::backdrop`), and the canvas is
    // left clear for it.
    let half_w = camera.width / scale;
    let half_h = camera.height / scale;
    let landed = game
        .world
        .landed()
        .and_then(|body| game.world.surface(body))
        .map(|surface| surface.biome);
    if let Some(biome) = landed {
        list.surface(
            ground_surface(biome),
            0.0,
            0.0,
            half_w * 3.0,
            half_h * 3.0,
            0.0,
            (0.0, 0.0),
            ground_color(biome),
        );
    }

    // Everything out there, drawn square to the window and then turned with
    // the camera — which is not at all unless the view is head up. The
    // planet the ship is at is the ground under it; the stations are drawn
    // where they are, already turned, by `stations`. On the ground there
    // is no planet in the sky: the ground itself, its texture's own.
    let out_there = list.len();
    if landed.is_none() {
        local_node(game, list);
    }
    list.turn_from(out_there, game.camera_turn() as f32);
    // The plain the town stands on, under it and the ship: the ground
    // beyond the deck, and the fog over what the crew have not seen of it.
    {
        let _timed = bims::timing::scope(bims::timing::Part::Plain);
        plain(game, list);
    }
    let (visitors, visitors_over, station_shade) = {
        let _timed = bims::timing::scope(bims::timing::Part::Stations);
        stations(game, list, prebuilt)
    };

    // The ship, drawn in its own frame — design units about the design's
    // origin, the grid it was laid out in — and turned with it at the end.
    // One turn for the whole picture, so a picture made of many shapes only
    // has to be right the once.
    let design = &game.world.ship.design;
    let grid = design.grid();
    // Nothing burns: a trip is resolved rather than flown (feature 104),
    // so the engines and the thrusters are drawn cold, as the designer
    // draws them.
    let firing = hull::Firing::NONE;
    let centre = game.world.ship.dynamics.centre_of_mass;
    let centre = (centre.x as f32, centre.y as f32);
    let mut ship = DrawList::default();

    // The pad the ship stands on, on a planet: paving under the whole
    // hull, in the ship's frame, and under everything.
    if landed.is_some() {
        pad(design, &mut ship);
    }
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

    // The tile under the pointer, rung. Part of the ship, so turned with
    // it.
    let tile = TILE as f32;
    if let Some((x, y)) = game.hover
        && design.holds((x, y))
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
    // The engineer's deployables (feature 74) under the room's picture,
    // in the room's units: sandbags on a station's tile are on the
    // joined deck, which is the room's grid and not the ship's.
    let mut laid = DrawList::default();
    fob(game, &mut laid);
    deployables(game, &mut laid);
    laid.textured_from(0);
    list.append_turned(laid.shapes(), room_centre, turn);
    let (under_fog, over_fog) = game.world.aboard.room.shapes_fog_split();
    list.append_turned(under_fog, room_centre, turn);
    // The station's people, over the ship's picture: one that has come
    // through the passage is standing on this deck, and one that has not
    // is on the station's, where nothing of the ship's is drawn.
    list.append(visitors.shapes());
    // The host's smooth fog goes here: over the crew and the station's
    // people, and under what both rooms draw over their fog — the shots,
    // which are always seen whatever they fly through, and the rings.
    list.mark_fog();
    // First over the fog, the shade along the walls (feature 98): under it
    // the light map's lamplight, which is added, washes a darkened deck
    // back out, so it goes on top — over whatever stands against a wall
    // too, the way a wall's shade falls — and under the shots.
    let mut walls = DrawList::default();
    wall_shade(&mut walls, design, &grid, false);
    list.append_turned(walls.shapes(), centre, turn);
    list.append(station_shade.shapes());
    list.append_turned(over_fog, room_centre, turn);
    list.append(visitors_over.shapes());
}

/// The green of the ground an Area defend holds: a wash over the ring and
/// a rim round it (the app's `theme::AREA`).
const AREA_GROUND: Color = Color::rgba(0.30, 0.88, 0.48, 0.09);
const AREA_RIM: Color = Color::rgba(0.30, 0.88, 0.48, 0.75);
/// How wide the rim is, in room units.
const AREA_RIM_LINE: f32 = 5.0;
/// The ring's mending (`World::area_healing`): a brighter green that
/// glows, a wash over the ground it heals, a rim of it inside the plain
/// one, and rings of it closing on the middle every
/// [`AREA_HEAL_PULSE_STEPS`].
const AREA_HEAL: Color = Color::rgba(0.36, 1.0, 0.52, 1.0);
const AREA_HEAL_WASH: f32 = 0.07;
const AREA_HEAL_LINE: f32 = 4.0;
const AREA_HEAL_PULSE_STEPS: u64 = 90;

/// An Area defend's FOB (October 2026), in the room's units, which the
/// caller turns with the room: the ring on the ground, the sandbags on its
/// open ground and the post in the middle.
fn fob(game: &Game, list: &mut DrawList) {
    let (Some((ring, radius)), Some((bags, post))) =
        (game.world.area_in_room(), game.world.fob_in_room())
    else {
        return;
    };
    let across = radius * 2.0;
    list.ellipse(ring.x, ring.y, across, across, AREA_GROUND);
    list.push(
        crate::draw::KIND_ELLIPSE,
        ring.x,
        ring.y,
        across,
        across,
        0.0,
        0.0,
        AREA_RIM_LINE,
        AREA_RIM,
    );
    if game.world.area_healing() {
        heal_ring(list, ring, radius, game.world.mission_steps());
    }
    let t = TILE as f32;
    for at in bags {
        let part = PlacedPart {
            id: 0,
            kind: PartKind::Sandbags,
            origin: (
                (at.x / t).floor().max(0.0) as u32,
                (at.y / t).floor().max(0.0) as u32,
            ),
            rotation: Rotation::R0,
        };
        crate::fittings::sandbags(list, &part);
    }
    crate::fittings::fob_post(list, post.x, post.y);
}

/// The green of an Area defend's ring while it mends (October 2026, the
/// player's word): a wash, a glowing rim just inside the plain one,
/// breathing, and two rings closing on the middle half a pulse apart,
/// fading as they go — off the mission clock, so a paused world stands.
fn heal_ring(list: &mut DrawList, ring: bims::math::Vec2, radius: f32, steps: u64) {
    use crate::draw::KIND_ELLIPSE;
    let across = radius * 2.0;
    list.ellipse(
        ring.x,
        ring.y,
        across,
        across,
        AREA_HEAL.alpha(AREA_HEAL_WASH),
    );
    let phase = (steps % AREA_HEAL_PULSE_STEPS) as f32 / AREA_HEAL_PULSE_STEPS as f32;
    let breath = 0.5 + 0.5 * (phase * std::f32::consts::TAU).cos();
    let rim = across - 2.0 * (AREA_RIM_LINE + AREA_HEAL_LINE);
    list.push(
        KIND_ELLIPSE,
        ring.x,
        ring.y,
        rim,
        rim,
        0.0,
        0.0,
        AREA_HEAL_LINE,
        AREA_HEAL.alpha(0.55 + 0.35 * breath).glowing(1.6),
    );
    for half in [0.0, 0.5] {
        let go = (phase + half).fract();
        let size = rim * (1.0 - 0.8 * go);
        list.push(
            KIND_ELLIPSE,
            ring.x,
            ring.y,
            size,
            size,
            0.0,
            0.0,
            AREA_HEAL_LINE * (1.0 - 0.5 * go),
            AREA_HEAL.alpha(0.6 * (1.0 - go)).glowing(1.4),
        );
    }
}

/// Every deployable in the crew's room, as a part stood on its tile: a
/// mine low on the deck, the satchels on a tile as one stack of them
/// (task 154), the engineer's ultimate as
/// the gun turret — drawn larger, its head turned the way the room last
/// swung it and its health a ring of lit segments — and a Healing Sentry
/// with no barrel, its health a dark ring closing on it (task 127).
/// In the room's units, which the caller turns with the room.
fn deployables(game: &Game, list: &mut DrawList) {
    let t = TILE as f32;
    let laid = game.world.deployables_in_room();
    for (i, (d, at)) in laid.iter().copied().enumerate() {
        let origin = (
            (at.x / t).floor().max(0.0) as u32,
            (at.y / t).floor().max(0.0) as u32,
        );
        let part = PlacedPart {
            id: 0,
            kind: PartKind::Sandbags,
            origin,
            rotation: Rotation::R0,
        };
        let health = d.health / game.world.laid_health(d.kind, d.owner_slot).max(1.0);
        match d.kind {
            world::DeployKind::Mine => crate::fittings::mine(list, &part),
            // Satchels stack (task 154): the first on a tile draws the
            // lot, the rest of them nothing.
            world::DeployKind::Satchel => {
                let here = |e: &world::Deployable| {
                    e.kind == world::DeployKind::Satchel && e.deck == d.deck && e.tile == d.tile
                };
                if laid[..i].iter().any(|(e, _)| here(e)) {
                    continue;
                }
                let count = laid.iter().filter(|(e, _)| here(e)).count() as u32;
                crate::fittings::satchels(list, &part, count);
            }
            world::DeployKind::HealingSentry => {
                crate::fittings::healing_sentry(list, &part, health)
            }
            world::DeployKind::Sentry => {
                // Which way the room last swung its head, and its flash.
                let (facing, flash) = game
                    .world
                    .aboard
                    .room
                    .sentries()
                    .iter()
                    .find(|s| s.id == d.id)
                    .map_or((0.0, 0.0), |s| (s.facing, s.flash));
                // Its flash in the engineer's colour, as its bolts are.
                let room = &game.world.aboard.room;
                let hue = room.tint(d.owner_slot as usize).map(|tint| {
                    let c = tint.shot();
                    Color::rgb(c.r, c.g, c.b)
                });
                crate::fittings::turret(list, &part, health, SENTRY_DRAWN, facing, flash, hue);
            }
        }
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

/// The texture of the same ground ([`ground_color`]'s), tied to the
/// world so the backdrop and the town's yards are one field of it.
fn ground_surface(biome: Biome) -> Surface {
    match biome {
        Biome::Desert => Surface::Sand,
        Biome::Temperate => Surface::Grass,
        Biome::Arctic => Surface::Snow,
    }
}

/// The landing pad: concrete slabs under the hull's whole box and a margin
/// past it, with a lighter border, in the ship's own frame.
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
    ship.surface(Surface::Concrete, cx, cy, w, hgt, 0.0, (x0, y0), PAD);
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

/// The crates and the fuel drums as the fight left them, over the
/// station's picture (`World::prop_look`, `fittings::prop_face`): a whole
/// one draws nothing here.
fn prop_faces(list: &mut DrawList, game: &Game, design: &ShipDesign, station: u32) {
    let t = TILE as f32;
    let pulse = (game.frame as f32 * 0.05).sin() * 0.5 + 0.5;
    for part in &design.parts {
        if !matches!(part.kind, PartKind::Crate | PartKind::FuelTank) {
            continue;
        }
        let share = game.world.prop_look(Some(station), part.origin);
        if share < 1.0 {
            crate::fittings::prop_face(list, part, share);
        }
        // A drum still whole is a thing to shoot (October 2026): a slow
        // amber hazard ring round it that reads through the dark.
        if part.kind == PartKind::FuelTank && share > 0.0 {
            let (x, y) = hull::middle(part.origin.0, part.origin.1);
            let r = t * (0.55 + 0.08 * pulse);
            list.push(
                crate::draw::KIND_ELLIPSE,
                x,
                y,
                r * 2.0,
                r * 2.0,
                0.0,
                0.0,
                3.0,
                DRUM_HAZARD.glowing(1.2 + 0.5 * pulse),
            );
        }
    }
}

/// A fuel drum's hazard ring.
const DRUM_HAZARD: Color = Color::rgb(1.0, 0.55, 0.12);

/// The amber of a way in the waves may take, the red of the one the next
/// wave takes, and a weld's hot orange (October 2026).
const WAY_IN: Color = Color::rgba(0.95, 0.70, 0.20, 0.55);
const WAY_IN_NEXT: Color = Color::rgb(1.0, 0.25, 0.18);
const WELD_SEAM: Color = Color::rgb(1.0, 0.62, 0.22);
const WELD_PLATE: Color = Color::rgb(0.32, 0.30, 0.30);

/// The site's ways in (October 2026, `World::entries`), in the station's
/// own frame: a hazard bar across each doorway, the next wave's in red
/// with chevrons pulsing inward, and a welded one plated over with a
/// glowing seam.
fn ways_in(list: &mut DrawList, game: &Game, station: u32) {
    if game.world.ship.state.alongside() != Some(station) {
        return;
    }
    let t = TILE as f32;
    let pulse = (game.frame as f32 * 0.12).sin() * 0.5 + 0.5;
    for e in game.world.entries() {
        let (x, y) = (e.site_at.x as f32, e.site_at.y as f32);
        let (ox, oy) = (e.site_out.x as f32, e.site_out.y as f32);
        // Across the doorway: the axis at right angles to the way out.
        let (ax, ay) = (-oy, ox);
        let half = e.width * t * 0.5;
        let bar = |list: &mut DrawList, inset: f32, thick: f32, c: Color| {
            let (cx, cy) = (x - ox * inset, y - oy * inset);
            list.line(
                cx - ax * half,
                cy - ay * half,
                cx + ax * half,
                cy + ay * half,
                thick,
                c,
            );
        };
        if e.welded {
            bar(list, t * 0.5, t * 0.55, WELD_PLATE);
            bar(list, t * 0.5, 3.0, WELD_SEAM.glowing(1.2 + 0.4 * pulse));
            continue;
        }
        // Glowing, so a way in reads through the fog and the dark.
        let colour = if e.next {
            WAY_IN_NEXT.glowing(1.4 + 0.6 * pulse)
        } else {
            WAY_IN.alpha(0.9).glowing(1.25)
        };
        bar(list, t * 0.9, 7.0, colour);
        // A weld under way: a seam growing across the doorway from one
        // side, glowing, as far as the work has got.
        if e.progress > 0.0 {
            let (cx, cy) = (x - ox * t * 0.5, y - oy * t * 0.5);
            let reach = half * 2.0 * e.progress.min(1.0);
            let (sx, sy) = (cx - ax * half, cy - ay * half);
            list.line(
                sx,
                sy,
                cx + ax * half,
                cy + ay * half,
                t * 0.3,
                WELD_PLATE.alpha(0.5),
            );
            list.line(
                sx,
                sy,
                sx + ax * reach,
                sy + ay * reach,
                4.0,
                WELD_SEAM.glowing(1.4 + 0.6 * pulse),
            );
        }
        if e.next {
            // Two chevrons pointing in, pulsing.
            for k in 0..2 {
                let inset = t * (1.6 + k as f32 * 0.7 + 0.3 * pulse);
                let (cx, cy) = (x - ox * inset, y - oy * inset);
                let (bx, by) = (cx + ox * t * 0.35, cy + oy * t * 0.35);
                let wing = t * 0.45;
                let c = WAY_IN_NEXT.glowing(1.2 + 0.6 * pulse);
                list.line(bx - ax * wing, by - ay * wing, cx, cy, 4.0, c);
                list.line(bx + ax * wing, by + ay * wing, cx, cy, 4.0, c);
            }
        }
    }
}

/// A nest hunt's standing nests (October 2026, `World::nests_look`), in
/// the station's own frame: a slow red pulse round each, seen through the
/// fog — the hunt's quarry is known, what guards it is not.
fn nest_marks(list: &mut DrawList, game: &Game, station: u32) {
    if game.world.ship.state.alongside() != Some(station) {
        return;
    }
    let t = TILE as f32;
    let pulse = (game.frame as f32 * 0.06).sin() * 0.5 + 0.5;
    for at in game.world.nests_look() {
        let (x, y) = (at.x as f32, at.y as f32);
        let r = t * (1.1 + 0.3 * pulse);
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            r * 2.0,
            r * 2.0,
            0.0,
            0.0,
            8.0,
            CHARGE_ARMED.glowing(1.6 + 0.8 * pulse),
        );
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            t * 0.5,
            t * 0.5,
            0.0,
            0.0,
            0.0,
            CHARGE_ARMED.glowing(2.0),
        );
    }
}

const TERMINAL_SCREEN: Color = Color::rgb(0.30, 0.85, 1.0);
const DONE_GREEN: Color = Color::rgb(0.40, 0.95, 0.55);
const DRUM_RED: Color = Color::rgb(0.85, 0.30, 0.18);
const CRATE_BROWN: Color = Color::rgb(0.62, 0.45, 0.26);
const SALVAGE_GOLD: Color = Color::rgb(1.0, 0.82, 0.30);
const OVERSEER_GOLD: Color = Color::rgb(1.0, 0.78, 0.25);

/// The second set of attacks' marks (October 2026, `World::objective_look`),
/// in the station's own frame and glowing so they read through the fog: a
/// terminal's console, cyan to hack and green once taken, its hacking a
/// bar under it; the cell, an amber ring, and its door a red bar sealed,
/// green cut; a drum, red with an amber hazard ring, and a crate, brown
/// with a gold ring — smaller and lifted while carried; the reactor's core,
/// cyan rings filling with the drums in, red and throbbing critical; the
/// Overseer, a gold ring and crown over him, his health beside it, red
/// while he flees, and his airlock a red ring and cross.
fn objective_marks(list: &mut DrawList, game: &Game, station: u32) {
    use world::MarkKind;
    if game.world.ship.state.alongside() != Some(station) {
        return;
    }
    let Some(look) = game.world.objective_look() else {
        return;
    };
    let t = TILE as f32;
    let pulse = (game.frame as f32 * 0.1).sin() * 0.5 + 0.5;
    let ring = |list: &mut DrawList, x: f32, y: f32, r: f32, w: f32, c: Color| {
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            r * 2.0,
            r * 2.0,
            0.0,
            0.0,
            w,
            c,
        );
    };
    let disc = |list: &mut DrawList, x: f32, y: f32, r: f32, c: Color| {
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            r * 2.0,
            r * 2.0,
            0.0,
            0.0,
            0.0,
            c,
        );
    };
    let bar = |list: &mut DrawList, x: f32, y: f32, share: f32, c: Color| {
        let w = 1.6 * t;
        let x0 = x - w / 2.0;
        list.line(x0, y, x0 + w, y, 6.0, Color::rgba(0.0, 0.0, 0.0, 0.6));
        list.line(x0, y, x0 + w * share.clamp(0.0, 1.0), y, 6.0, c);
    };
    for m in &look.marks {
        let (x, y) = (m.at.x as f32, m.at.y as f32);
        match m.kind {
            MarkKind::Terminal { taken } => {
                let screen = if taken { DONE_GREEN } else { TERMINAL_SCREEN };
                list.rect(x, y, t * 0.9, t * 0.65, 4.0, Color::rgb(0.10, 0.12, 0.15));
                list.rect(x, y - t * 0.05, t * 0.7, t * 0.4, 2.0, screen.glowing(1.5));
                if !taken {
                    ring(
                        list,
                        x,
                        y,
                        t * (0.85 + 0.12 * pulse),
                        5.0,
                        screen.glowing(1.2 + 0.6 * pulse),
                    );
                    if m.progress > 0.0 {
                        bar(list, x, y + t * 0.85, m.progress, screen.glowing(1.3));
                    }
                }
            }
            MarkKind::Cell => {
                ring(list, x, y, t * 1.4, 4.0, CHARGE.glowing(0.9 + 0.4 * pulse));
            }
            MarkKind::CellDoor { open } => {
                let c = if open { DONE_GREEN } else { CHARGE_ARMED };
                ring(list, x, y, t * 0.8, 6.0, c.glowing(1.3 + 0.5 * pulse));
                if !open && m.progress > 0.0 {
                    bar(list, x, y + t * 1.0, m.progress, CHARGE.glowing(1.3));
                }
            }
            MarkKind::Drum { carried } => {
                let (r, lift) = if carried {
                    (t * 0.28, t * 0.55)
                } else {
                    (t * 0.38, 0.0)
                };
                disc(list, x, y - lift, r, DRUM_RED);
                ring(
                    list,
                    x,
                    y - lift,
                    r * 0.6,
                    3.0,
                    Color::rgb(0.30, 0.10, 0.06),
                );
                ring(
                    list,
                    x,
                    y - lift,
                    r + t * 0.2,
                    4.0,
                    CHARGE.glowing(1.2 + 0.6 * pulse),
                );
            }
            MarkKind::Crate { carried } => {
                let (s, lift) = if carried {
                    (t * 0.5, t * 0.55)
                } else {
                    (t * 0.7, 0.0)
                };
                list.rect(x, y - lift, s, s, 3.0, CRATE_BROWN);
                list.line(
                    x - s / 2.0,
                    y - lift - s / 2.0,
                    x + s / 2.0,
                    y - lift + s / 2.0,
                    3.0,
                    Color::rgb(0.35, 0.24, 0.12),
                );
                ring(
                    list,
                    x,
                    y - lift,
                    s * 0.75 + t * 0.15,
                    4.0,
                    SALVAGE_GOLD.glowing(1.2 + 0.6 * pulse),
                );
            }
            MarkKind::Reactor { critical } => {
                if critical {
                    let blink = ((game.frame / 6) % 2) as f32;
                    ring(list, x, y, t * 2.4, 8.0, CHARGE_ARMED.glowing(1.5 + blink));
                    disc(list, x, y, t * 0.9, CHARGE_ARMED.glowing(1.8 + blink));
                } else {
                    disc(
                        list,
                        x,
                        y,
                        t * 0.7,
                        TERMINAL_SCREEN.glowing(1.2 + 0.5 * pulse),
                    );
                    ring(
                        list,
                        x,
                        y,
                        t * 2.2,
                        5.0,
                        TERMINAL_SCREEN.glowing(0.9 + 0.4 * pulse),
                    );
                    ring(
                        list,
                        x,
                        y,
                        t * 2.2 * m.progress.max(0.05),
                        4.0,
                        CHARGE.glowing(1.5),
                    );
                }
            }
            MarkKind::Overseer { fleeing } => {
                let c = if fleeing { CHARGE_ARMED } else { OVERSEER_GOLD };
                ring(
                    list,
                    x,
                    y,
                    t * (0.9 + 0.1 * pulse),
                    6.0,
                    c.glowing(1.4 + 0.6 * pulse),
                );
                // A crown over his head.
                let top = y - t * 1.2;
                for k in [-1.0, 0.0, 1.0] {
                    let px = x + k * t * 0.25;
                    list.line(px, top, px, top - t * 0.3, 5.0, OVERSEER_GOLD.glowing(1.6));
                }
                list.line(
                    x - t * 0.3,
                    top,
                    x + t * 0.3,
                    top,
                    6.0,
                    OVERSEER_GOLD.glowing(1.6),
                );
                bar(list, x, y + t * 1.1, m.progress, c.glowing(1.3));
            }
            MarkKind::Escape => {
                ring(
                    list,
                    x,
                    y,
                    t * 1.0,
                    6.0,
                    CHARGE_ARMED.glowing(1.0 + 0.6 * pulse),
                );
                let arm = t * 0.4;
                list.line(
                    x - arm,
                    y - arm,
                    x + arm,
                    y + arm,
                    5.0,
                    CHARGE_ARMED.glowing(1.3),
                );
                list.line(
                    x - arm,
                    y + arm,
                    x + arm,
                    y - arm,
                    5.0,
                    CHARGE_ARMED.glowing(1.3),
                );
            }
        }
    }
}

const FLAG_POLE: Color = Color::rgb(0.80, 0.82, 0.86);
const FLAG_CLOTH: Color = Color::rgb(0.30, 0.85, 1.0);

/// An Evacuation's flag (October 2026, `World::evacuation_look`), in the
/// station's own frame: a pole and a cloth that flutters, ringed where it
/// lies so the people's gathering place reads at a glance; lifted over
/// its carrier while it is carried.
fn evacuation_flag(list: &mut DrawList, game: &Game) {
    let Some(look) = game.world.evacuation_look() else {
        return;
    };
    let t = TILE as f32;
    let (x, y) = (look.flag.x as f32, look.flag.y as f32);
    let lift = if look.carried { t * 0.6 } else { 0.0 };
    if !look.carried {
        let pulse = (game.frame as f32 * 0.08).sin() * 0.5 + 0.5;
        let r = t * (1.3 + 0.15 * pulse);
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            r * 2.0,
            r * 2.0,
            0.0,
            0.0,
            7.0,
            FLAG_CLOTH.glowing(1.3 + 0.5 * pulse),
        );
    }
    let (bx, by) = (x, y - lift);
    let top = by - t * 1.7;
    list.line(bx, by, bx, top, 6.0, FLAG_POLE.glowing(1.4));
    let wave = (game.frame as f32 * 0.2).sin() * 3.0;
    let w = t * 1.0;
    list.push(
        crate::draw::KIND_RECT,
        bx + w * 0.5,
        top + t * 0.28 + wave * 0.3,
        w,
        t * 0.56,
        0.0,
        2.0,
        0.0,
        FLAG_CLOTH.glowing(1.9),
    );
}

const CHARGE: Color = Color::rgb(1.0, 0.72, 0.20);
const CHARGE_ARMED: Color = Color::rgb(1.0, 0.25, 0.18);
const WAY_OUT: Color = Color::rgb(0.45, 0.95, 0.60);

/// A Sabotage's marks (October 2026, `World::sabotage_look`), in the
/// station's own frame: the charge — an amber ring and cross to plant at,
/// the planting's arc filling round it; red once planted, with the
/// machines' disarming as a bar under it; blinking while it counts down —
/// and, once it is armed, the way out: a green ring a tile and a half
/// inside the airlock with chevrons pointing out.
fn sabotage_marks(list: &mut DrawList, game: &Game, station: u32) {
    use world::droid::SabotagePhase;
    if game.world.ship.state.alongside() != Some(station) {
        return;
    }
    let Some(look) = game.world.sabotage_look() else {
        return;
    };
    let t = TILE as f32;
    let pulse = (game.frame as f32 * 0.12).sin() * 0.5 + 0.5;
    let (cx, cy) = (look.charge.x as f32, look.charge.y as f32);
    let ring = |list: &mut DrawList, x: f32, y: f32, r: f32, w: f32, c: Color| {
        list.push(
            crate::draw::KIND_ELLIPSE,
            x,
            y,
            r * 2.0,
            r * 2.0,
            0.0,
            0.0,
            w,
            c,
        );
    };
    match look.phase {
        SabotagePhase::Plant => {
            ring(
                list,
                cx,
                cy,
                t * (0.8 + 0.15 * pulse),
                7.0,
                CHARGE.glowing(1.4 + 0.6 * pulse),
            );
            let arm = t * 0.35;
            let cross = CHARGE.glowing(1.6);
            list.line(cx - arm, cy - arm, cx + arm, cy + arm, 6.0, cross);
            list.line(cx - arm, cy + arm, cx + arm, cy - arm, 6.0, cross);
            if look.planted > 0.0 {
                let w = 1.6 * t;
                let (x0, y) = (cx - w / 2.0, cy + t * 1.1);
                list.line(x0, y, x0 + w, y, 6.0, Color::rgba(0.0, 0.0, 0.0, 0.6));
                list.line(
                    x0,
                    y,
                    x0 + w * look.planted.min(1.0),
                    y,
                    6.0,
                    CHARGE.glowing(1.3),
                );
            }
        }
        SabotagePhase::Hold | SabotagePhase::Escape => {
            let blink = if look.phase == SabotagePhase::Escape {
                ((game.frame / 8) % 2) as f32
            } else {
                pulse
            };
            ring(
                list,
                cx,
                cy,
                t * 0.7,
                5.0,
                CHARGE_ARMED.glowing(1.2 + 0.8 * blink),
            );
            list.push(
                crate::draw::KIND_ELLIPSE,
                cx,
                cy,
                t * 0.45,
                t * 0.45,
                0.0,
                0.0,
                0.0,
                CHARGE_ARMED.glowing(1.0 + 1.2 * blink),
            );
            if look.phase == SabotagePhase::Hold && look.disarmed > 0.0 {
                let w = 1.6 * t;
                let (x0, y) = (cx - w / 2.0, cy + t * 1.1);
                list.line(x0, y, x0 + w, y, 6.0, Color::rgba(0.0, 0.0, 0.0, 0.6));
                list.line(x0, y, x0 + w * look.disarmed.min(1.0), y, 6.0, CHARGE_ARMED);
            }
            // The way out.
            let (ox, oy) = (look.way_out_out.x as f32, look.way_out_out.y as f32);
            let (wx, wy) = (
                look.way_out.x as f32 - ox * 1.5 * t,
                look.way_out.y as f32 - oy * 1.5 * t,
            );
            ring(
                list,
                wx,
                wy,
                t * (1.4 + 0.2 * pulse),
                7.0,
                WAY_OUT.glowing(1.4 + 0.6 * pulse),
            );
            let (ax, ay) = (-oy, ox);
            for k in 0..2 {
                let ahead = t * (0.2 + 0.5 * k as f32 + 0.3 * pulse);
                let (px, py) = (wx + ox * ahead, wy + oy * ahead);
                let (bx, by) = (px - ox * t * 0.35, py - oy * t * 0.35);
                let wing = t * 0.45;
                let c = WAY_OUT.glowing(1.2 + 0.6 * pulse);
                list.line(bx - ax * wing, by - ay * wing, px, py, 5.0, c);
                list.line(bx + ax * wing, by + ay * wing, px, py, 5.0, c);
            }
        }
        _ => {}
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
/// row — the outdoor runs the biome's ground ([`ground_surface`], in
/// [`ground_color`]), the indoor runs floorboards — rather
/// than a rect a tile, since a town is nine thousand tiles and most of them are ground. Then, on roughly one outdoor tile
/// in [`DECORATED_ONE_IN`] with nothing standing on it, the ground's
/// decoration: a tuft of grass and now and then a flower, a ripple in
/// the sand or a pebble, a drift of snow.
fn ground_floor(list: &mut DrawList, grid: &Grid, terrain: &Terrain) {
    let tile = TILE as f32;
    // A run overlaps the next by a hair, or the feathering of their edges
    // shows the backdrop between two rows as a seam.
    let lap = crate::fittings::LAP / 2.0;
    let side = terrain.side;
    for y in 0..side {
        let mut run: Option<(u32, bool)> = None;
        for x in 0..=side {
            let here =
                (x < side && grid.has_floor((x as i32, y as i32))).then(|| terrain.outdoor(x, y));
            if let Some((x0, outdoor)) = run
                && here != Some(outdoor)
            {
                let (surface, color) = if outdoor {
                    (ground_surface(terrain.biome), ground_color(terrain.biome))
                } else {
                    (Surface::Floorboard, FLOORBOARD)
                };
                list.surface_box(
                    surface,
                    (
                        x0 as f32 * tile - lap,
                        y as f32 * tile - lap,
                        x as f32 * tile + lap,
                        (y + 1) as f32 * tile + lap,
                    ),
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
/// A shift that puts every tile of the plain the camera can reach at a
/// non-negative tile, so the wild's painters — which take a placed part,
/// whose origin is unsigned — can draw it; the picture is placed back by
/// the same shift.
const PLAIN_SHIFT: i32 = 8192;
/// How far from the camera's middle the plain is drawn at most, in tiles:
/// the corners of a view held to `bims::terrain::VIEW` along its nearer
/// edge (`Camera::scale_for_reach`), with a tile over.
const PLAIN_DRAWN: i32 = bims::terrain::VIEW * 2;
/// How far a run of the plain's ground reaches past its tiles, in room
/// units, so two runs meet under the feathering rather than beside it.
const RUN_LAP: f32 = 3.0;

/// The room tiles the plain is drawn over this frame, both ends in —
/// the window the camera can see, never more than [`PLAIN_DRAWN`] tiles
/// from its middle, which is as far as the world is loaded — or `None`
/// off a planet. What `plain` draws and what the plain's picture is
/// asked for (`bims::Game::picture_plain`), so the two agree.
pub fn plain_window(game: &Game) -> Option<(i32, i32, i32, i32)> {
    game.world.aboard.room.plane()?;
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
    Some((cx - reach, cy - reach, cx + reach, cy + reach))
}

/// The ground beyond the deck, while the ship is on a planet: the plain
/// (`bims::terrain`) as the crew's room reads it, drawn a tile at a time
/// in the room's frame over [`plain_window`]. The deck's own tiles are
/// the station's and the ship's to draw. Water, cliff and forest are
/// runs along a row; a tree, a shrub and a rock are the biome's, as
/// `fittings` draws the town's. The fog over it — black where nobody
/// has looked and grey where somebody has — is not drawn here: it is
/// the plain's picture, marched like the room's light map and drawn by
/// the host over everything, a chunk a texture (`plain_fog_on_screen`).
fn plain(game: &Game, list: &mut DrawList) {
    let room = &game.world.aboard.room;
    let Some(plane) = room.plane() else {
        return;
    };
    let Some((wx0, wy0, wx1, wy1)) = plain_window(game) else {
        return;
    };
    let biome = Biome::from_code(plane.biome() as u32).unwrap_or(Biome::Temperate);
    let tile = TILE as f32;
    let offset = game.world.aboard.offset;
    let grid = game.world.aboard.design.grid();
    let s = PLAIN_SHIFT;
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
    let box_run = |picture: &mut DrawList, x0: i32, x1: i32, ry: i32, ground: Ground| {
        let corners = (
            (x0 + s) as f32 * tile - RUN_LAP,
            (ry + s) as f32 * tile - RUN_LAP,
            (x1 + s) as f32 * tile + RUN_LAP,
            (ry + s + 1) as f32 * tile + RUN_LAP,
        );
        match ground {
            Ground::Cliff => picture.surface_box(Surface::Rock, corners, cliff),
            Ground::Forest => {
                picture.surface_box(ground_surface(Biome::Temperate), corners, forest)
            }
            _ => picture.surface_box(crate::fittings::water_look(biome).0, corners, water),
        }
    };
    for ry in wy0..=wy1 {
        // The runs first: one rect a stretch of the same ground.
        let mut run: Option<(i32, Ground)> = None;
        for rx in wx0..=wx1 + 1 {
            let here = (rx <= wx1 && !deck(rx, ry))
                .then(|| plane.at_room(rx, ry))
                .filter(|g| matches!(g, Ground::Water | Ground::Cliff | Ground::Forest));
            if let Some((x0, kind)) = run
                && here != Some(kind)
            {
                box_run(&mut picture, x0, rx, ry, kind);
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
        for rx in wx0..=wx1 {
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
                        let from = picture.len();
                        picture.ellipse(
                            m.x as f32 + dx * tile * 0.6,
                            m.y as f32 + dy * tile * 0.6,
                            tile * 0.9,
                            tile * 0.8,
                            crown,
                        );
                        picture.foliage_from(from, true);
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
    }
    let centre = game.world.ship.dynamics.centre_of_mass;
    let shift = s as f32 * tile;
    let pivot = (
        centre.x as f32 + offset.x as f32 + shift,
        centre.y as f32 + offset.y as f32 + shift,
    );
    // The boulders and the trees' trunks get the objects' texture (the
    // crowns have their leaves already).
    picture.textured_from(0);
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
    // Where the objects start: everything standing on the deck gets the
    // objects' texture over its fills (`DrawList::textured_from`).
    let mut objects = list.len();
    for layer in [Layer::Structure, Layer::Floor, Layer::Object] {
        if terrain.is_some() && layer != Layer::Object {
            continue;
        }
        if layer == Layer::Object {
            objects = list.len();
        }
        for part in &design.parts {
            if part.layer() != layer || skip.contains(&part.id) {
                continue;
            }
            if layer == Layer::Object && hull::part(list, part, grid, firing, open_airlock) {
                continue;
            }
            // A railing joins the railings beside it into one rail.
            if part.kind == PartKind::Railing {
                let (x, y) = (part.origin.0 as i32, part.origin.1 as i32);
                let railing = |dx: i32, dy: i32| {
                    design
                        .part(grid.get(Layer::Object, (x + dx, y + dy)))
                        .is_some_and(|p| p.kind == PartKind::Railing)
                };
                let joins = [railing(0, -1), railing(1, 0), railing(0, 1), railing(-1, 0)];
                crate::fittings::railing_joined(list, part, joins);
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
                // The deck is steel plate, a plate a tile, anchored to the
                // tile so its seams are the grid's.
                if layer == Layer::Floor {
                    let (x, y) = (m.x as f32, m.y as f32);
                    let size = tile + crate::fittings::LAP;
                    list.surface(Surface::Deck, x, y, size, size, 0.0, (x, y), color);
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
    list.textured_from(objects);
}

/// The shade along the inner side of the walls (feature 98): how deep
/// each band reaches onto the deck, as a share of a tile, and how dark it
/// is. Stacked, they are darkest against the wall — a quarter darker —
/// and gone four tenths of a tile out: a soft edge, not a line.
const WALL_SHADE: [(f32, f32); 3] = [(0.10, 0.16), (0.24, 0.09), (0.42, 0.05)];
const WALL_SHADE_COLOUR: Color = Color::rgb(0.0, 0.01, 0.03);

/// The shade along the inner side of every wall (feature 98): a soft dark
/// band on the deck against each wall, so a room reads as walls standing
/// round a floor rather than as tiles laid flat. A run of tiles along one
/// wall is one band — a rect a run, not a tile — and nothing is shaded
/// that is a wall, a door, an airlock or a corner piece, or that has no
/// floor (open space); on the ground (`ground`) every tile that is not
/// one of those is a floor. Worked out from the design every frame, a
/// pass over the parts and one over the tiles: a picture, and nothing any
/// rule reads.
fn wall_shade(list: &mut DrawList, design: &ShipDesign, grid: &Grid, ground: bool) {
    const WALL: u8 = 1;
    const NEITHER: u8 = 2;
    let side = grid.side() as i32;
    let mut class = vec![0u8; (side * side) as usize];
    for part in &design.parts {
        let mark = match part.kind {
            PartKind::Wall | PartKind::OutsideWall => WALL,
            PartKind::Door
            | PartKind::Airlock
            | PartKind::DiagonalWall
            | PartKind::DiagonalOutsideWall => NEITHER,
            _ => continue,
        };
        for (x, y) in part.tiles() {
            let (x, y) = (x as i32, y as i32);
            if x < side && y < side {
                class[(y * side + x) as usize] = mark;
            }
        }
    }
    let at = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= side || y >= side {
            NEITHER
        } else {
            class[(y * side + x) as usize]
        }
    };
    let shaded = |x: i32, y: i32, dx: i32, dy: i32| {
        at(x, y) == 0 && at(x + dx, y + dy) == WALL && (ground || grid.has_floor((x, y)))
    };
    let t = TILE as f32;
    // A wall above, below, to the left and to the right: a run goes along
    // the wall, so along x for one above or below.
    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let along_x = dx == 0;
        let toward = (dx + dy) as f32;
        for across in 0..side {
            let tile = |along: i32| {
                if along_x {
                    (along, across)
                } else {
                    (across, along)
                }
            };
            let mut along = 0;
            while along < side {
                let (x, y) = tile(along);
                if !shaded(x, y, dx, dy) {
                    along += 1;
                    continue;
                }
                let start = along;
                while along < side && {
                    let (x, y) = tile(along);
                    shaded(x, y, dx, dy)
                } {
                    along += 1;
                }
                let length = (along - start) as f32 * t;
                let middle = (start + along) as f32 * 0.5 * t;
                let row = (across as f32 + 0.5) * t;
                for (depth, alpha) in WALL_SHADE {
                    let d = depth * t;
                    let edge = row + toward * (t - d) * 0.5;
                    let colour = WALL_SHADE_COLOUR.alpha(alpha);
                    if along_x {
                        list.rect(middle, edge, length, d, 0.0, colour);
                    } else {
                        list.rect(edge, middle, d, length, 0.0, colour);
                    }
                }
            }
        }
    }
}

/// The stations in the picture this frame, in the order they are drawn,
/// and whether each is drawn **whole** — its hull tile by tile — rather
/// than as the plate it is from further off. The one rule [`stations`]
/// draws by and [`stations_to_build`] builds ahead by.
fn stations_in_view(game: &Game) -> Vec<(&world::Station, bool)> {
    let here = game.world.ship.position();
    let docked = game.world.ship.state.alongside();
    // The system's stations — and, on a planet, the settlement the ship
    // is tied up at, which is not among them and is not out there: it is
    // the ground the ship stands on, drawn only while the ship is on it.
    let settlement = docked
        .filter(|&id| world::surface_body(id).is_some())
        .and_then(|id| game.world.station(id));
    let on_the_ground = settlement.is_some();
    let mut out = Vec::new();
    for station in game.world.stations.iter().chain(settlement) {
        // From the ground nothing in orbit is in the picture.
        if on_the_ground && station.plan != world::Plan::Surface {
            continue;
        }
        let clearance = station.clearance(here);
        if clearance > world::data::STATION_VISIBLE {
            continue;
        }
        // Somebody else's station is drawn from its room — its
        // structure under the one fog, nobody in it, see `bims::sight` —
        // and its room is only open within the residents' range. Out to
        // there it stays the plate it was from further off: a shape and a
        // kind, and nothing of what is inside. The crew's own is its hull
        // from the local frame in, as before.
        let lived_in = game
            .world
            .residents
            .as_ref()
            .is_some_and(|r| r.station == station.id);
        let stranger = game.world.stance(station.id) != Stance::Friendly;
        let whole = !(clearance > world::data::LOCAL_RADIUS_STATION || (stranger && !lived_in));
        out.push((station, whole));
    }
    out
}

/// What a station's own picture is built from, besides its design: all of
/// it read off the world before the picture is built, so the building
/// asks the world nothing and can be done while the crew's room draws
/// itself (task 122). Two of these alike are one picture.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct StationWork {
    id: u32,
    /// The station's door, and how far it stands open: it opens with the
    /// ship's, since they are one passage — `None` unless docked there.
    open: Option<(u32, f32)>,
    /// Whether its room is open, so the room draws its fixtures and the
    /// hull leaves them out (`bims::aboard::drawn_by_room`).
    lived_in: bool,
    /// On a planet, the settlement's biome: its deck is the ground.
    biome: Option<Biome>,
}

/// A station's own picture, in the station's own frame and not yet placed:
/// its rim and its tiles, and apart, the shade along its walls — what
/// [`station_picture`] builds and [`stations`] finishes and places. The
/// grid comes with it, since the lights are drawn on it after.
pub(crate) struct StationPicture {
    work: StationWork,
    grid: Grid,
    picture: DrawList,
    walls: DrawList,
}

fn station_work(game: &Game, station: &world::Station) -> StationWork {
    let docked = game.world.ship.state.alongside();
    StationWork {
        id: station.id,
        open: docked
            .filter(|&id| id == station.id)
            .and(station.port().map(|p| (p.part_id, game.airlock_ajar))),
        lived_in: game
            .world
            .residents
            .as_ref()
            .is_some_and(|r| r.station == station.id),
        // On a planet the settlement's deck is the ground: no rim round
        // it — the ground goes on past its edge — and its floor and its
        // walls drawn as the biome has them (`Terrain`).
        biome: (station.plan == world::Plan::Surface)
            .then(|| world::surface_body(station.id))
            .flatten()
            .and_then(|body| game.world.surface(body))
            .map(|surface| surface.biome),
    }
}

/// The pictures [`stations`] will want this frame: every station drawn
/// whole, as [`stations_in_view`] says — none on the map, which draws no
/// station's hull. Read off the world before
/// `Session::render` lets the crew's room draw, so the pictures can be
/// built beside it with [`station_pictures`].
pub(crate) fn stations_to_build(game: &Game) -> Vec<StationWork> {
    if game.mode != ViewMode::Ship {
        return Vec::new();
    }
    stations_in_view(game)
        .into_iter()
        .filter(|&(_, whole)| whole)
        .map(|(station, _)| station_work(game, station))
        .collect()
}

/// A station's own picture kept from one frame to the next (task 122),
/// with what it was built from: its [`StationWork`], and its design's
/// build area and every part as it stood — id, kind, place and turn, in
/// the design's own order, which is the order the tiles are drawn in. It
/// is used again only while all of that is still so ([`KeptStation::fits`]),
/// compared outright rather than by a hash, so a design written anywhere,
/// for any reason, is a picture built again and never a stale one; the
/// cargo is the one thing of a design not in it, and nothing draws it.
pub(crate) struct KeptStation {
    build_area: u32,
    parts: Vec<PlacedPart>,
    picture: StationPicture,
}

impl KeptStation {
    /// The hull's shapes as kept, for a test to tell a picture used again
    /// from one built again.
    #[cfg(test)]
    pub(crate) fn shapes(&self) -> &[f32] {
        self.picture.picture.shapes()
    }

    /// Whether this is still the picture of `work` at a station of `design`.
    fn fits(&self, work: &StationWork, design: &ShipDesign) -> bool {
        self.picture.work == *work
            && self.build_area == design.build_area
            && self.parts == design.parts
    }
}

/// Which of the pictures `work` names want building this frame: those
/// nothing in `kept` is still the picture of — the first frame a station
/// is drawn whole, a design that changed, an airlock moving.
pub(crate) fn stale_stations(
    game: &Game,
    work: &[StationWork],
    kept: &[KeptStation],
) -> Vec<StationWork> {
    work.iter()
        .copied()
        .filter(|w| {
            game.world
                .station(w.id)
                .is_some_and(|s| !kept.iter().any(|k| k.fits(w, &s.design)))
        })
        .collect()
}

/// Build the pictures `work` names, off the world's stations and
/// settlements alone — the fields, not the `World`, so the crew's room can
/// be borrowed to draw itself at the same time.
pub(crate) fn station_pictures(
    work: &[StationWork],
    stations: &[world::Station],
    surfaces: &[world::Surface],
) -> Vec<KeptStation> {
    work.iter()
        .filter_map(|&w| {
            let station = stations.iter().find(|s| s.id == w.id).or_else(|| {
                world::surface_body(w.id)
                    .and_then(|body| surfaces.iter().find(|s| s.body == body))
                    .map(world::Surface::station)
            })?;
            Some(KeptStation {
                build_area: station.design.build_area,
                parts: station.design.parts.clone(),
                picture: station_picture(w, station),
            })
        })
        .collect()
}

/// Keep this frame's pictures and nothing else, in `work`'s order: for a
/// station `stale` names, the one just built, and for the rest the one
/// kept before, which [`stale_stations`] found still fits. What
/// `Session::render` hands the painter, and keeps for the next frame.
pub(crate) fn keep_stations(
    work: &[StationWork],
    stale: &[StationWork],
    kept: &mut Vec<KeptStation>,
    mut fresh: Vec<KeptStation>,
) {
    let mut old = std::mem::take(kept);
    for w in work {
        let from = if stale.contains(w) {
            &mut fresh
        } else {
            &mut old
        };
        if let Some(i) = from.iter().position(|k| k.picture.work == *w) {
            kept.push(from.swap_remove(i));
        }
    }
}

/// One station's own picture: its rim, its tiles and the shade along its
/// walls, from its design and `work` and nothing else.
fn station_picture(work: StationWork, station: &world::Station) -> StationPicture {
    let _timed = bims::timing::scope(bims::timing::Part::StationPicture);
    let prep_timed = bims::timing::scope(bims::timing::Part::StationPrep);
    let design = &station.design;
    let grid = design.grid();
    let skip: Vec<u32> = if work.lived_in {
        bims::aboard::drawn_by_room(design)
    } else {
        Vec::new()
    };
    let terrain = work.biome.map(|biome| Terrain::of(design, biome));
    drop(prep_timed);
    let hull_timed = bims::timing::scope(bims::timing::Part::StationHull);
    let mut picture = DrawList::default();
    if terrain.is_none() {
        hull::shadow(&mut picture, design, &grid);
    }
    hull_tiles(
        &mut picture,
        design,
        &grid,
        hull::Firing::NONE,
        &skip,
        work.open,
        terrain.as_ref(),
    );
    drop(hull_timed);
    let _shade_timed = bims::timing::scope(bims::timing::Part::StationShade);
    let mut walls = DrawList::default();
    wall_shade(&mut walls, design, &grid, terrain.is_some());
    StationPicture {
        work,
        grid,
        picture,
        walls,
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
/// drawn: the caller paints them over the ship, since they can be on it —
/// and, apart, what their room draws over its fog, which the caller
/// paints over the crew's fog — and the shade along every station's walls,
/// which the caller paints over the fog as well (feature 98).
fn stations(
    game: &Game,
    list: &mut DrawList,
    prebuilt: &[KeptStation],
) -> (DrawList, DrawList, DrawList) {
    let mut lifted = DrawList::default();
    let mut lifted_over = DrawList::default();
    let mut shade = DrawList::default();
    let here = game.world.ship.position();
    let turn = game.camera_turn() as f32;
    for (station, whole) in stations_in_view(game) {
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

        // Somebody else's station is drawn from its room — its
        // structure under the one fog, nobody in it, see `bims::sight` —
        // and its room is only open within the residents' range. Out to
        // there it stays the plate it was from further off: a shape and a
        // kind, and nothing of what is inside. The crew's own is its hull
        // from the local frame in, as before.
        let stance = game.world.stance(station.id);
        let stranger = stance != Stance::Friendly;
        if !whole {
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

        // Its own picture — the hull and the shade along its walls — kept
        // from the frame before or built beside the crew's room by
        // `Session::render`, or built here if it was not (task 122): the
        // same function from the same inputs either way, so the same
        // picture.
        let work = station_work(game, station);
        let fresh;
        let built = match prebuilt.iter().find(|k| k.picture.work == work) {
            Some(kept) => &kept.picture,
            None => {
                fresh = station_picture(work, station);
                &fresh
            }
        };
        let grid = &built.grid;
        // What changes from frame to frame goes into a list of its own,
        // placed straight after the picture: a shape is placed on its own,
        // so the two placed one after the other are the one list placed.
        let mut picture = DrawList::default();
        let lights_timed = bims::timing::scope(bims::timing::Part::StationLights);
        hull::lights(&mut picture, &station.design, grid, game.frame);
        lamp_faces(&mut picture, game, &station.design, Some(station.id));
        prop_faces(&mut picture, game, &station.design, station.id);
        ways_in(&mut picture, game, station.id);
        sabotage_marks(&mut picture, game, station.id);
        evacuation_flag(&mut picture, game);
        nest_marks(&mut picture, game, station.id);
        objective_marks(&mut picture, game, station.id);
        // The machines' ship, tied up at the far airlock, or their
        // lander down on the plain beyond a gate (feature 83). Drawn in
        // the station's own frame, so it turns with the station; it is a
        // picture and nothing else — not part of the room, not walkable,
        // and nothing a bolt can reach.
        if let Some((at, outward, lander)) = game.world.droid_ship(station.id) {
            droid_ship(
                &mut picture,
                (at.x as f32, at.y as f32),
                (outward.x as f32, outward.y as f32),
                lander,
                game.frame,
            );
        }
        drop(lights_timed);
        list.append_turned_at(built.picture.shapes(), middle, turn, at);
        list.append_turned_at(picture.shapes(), middle, turn, at);
        // The shade along its walls, apart: it goes over the fog with the
        // ship's (`wall_shade`).
        shade.append_turned_at(built.walls.shapes(), middle, turn, at);
        if let Some(residents) = residents {
            // Their room is the station's design plus the shift its deck
            // took with the ship on it, so its picture turns about the
            // station's middle where that middle is in the room.
            let shift = residents.aboard.offset;
            let pivot = (middle.0 + shift.x as f32, middle.1 + shift.y as f32);
            let (deck, bodies, over) = residents.aboard.room.shapes_in_three();
            list.append_turned_at(deck, pivot, turn, at);
            // Their people go to the caller, to be drawn over the ship:
            // the ship's hull and its room aboard are painted after the
            // stations, and one of them who has followed the crew through
            // the passage would otherwise be under the ship's deck — a
            // name over an empty tile.
            lifted.append_turned_at(bodies, pivot, turn, at);
            // And what their room draws over its fog — their shots —
            // apart, to go over the crew's fog with the crew's own.
            lifted_over.append_turned_at(over, pivot, turn, at);
        }
    }
    (lifted, lifted_over, shade)
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
        residents.aboard.shown_position(who),
        dvec2(side / 2.0, side / 2.0),
        turn,
    );
    let offset = station.centre().sub(game.world.ship.position());
    let at = crate::game::turned(offset.x as f32, -offset.y as f32, turn as f32);
    (x + at.0, y + at.1)
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
            if let Some(body) = game.world.system.body(id) {
                paint_body(list, x, y, hull * 4.0, body.kind, 6.0);
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

/// The landing pad's arrow on the map: a steel white. (It was the head of
/// the belt's pickaxe too, until the mining sites went in task 111.)
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
/// down onto it in steel white, a shaft and the two arms of its
/// head. The mark of a planet that can be landed on, at its shoulder on
/// the map. Rectangles alone, as the rest of the map's marks are.
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

/// The mark that says *the crew have been here*, `size` across, centred
/// on `(x, y)`: a tick, two strokes of the map's own line (feature 85).
/// A shape nothing else on the map draws, since every other mark there
/// is a ring, a disc or an icon — so a place the crew have been reads as
/// one at a glance, whatever else is marked on it.
pub fn paint_tick(list: &mut DrawList, x: f32, y: f32, size: f32, thin: f32) {
    let (w, h) = (size / 2.0, size / 2.0);
    // A dark disc under it, so it reads over an icon or a ring (the second map rework).
    disc(list, x, y, size * 1.5, VOID.alpha(0.85));
    // The short stroke down into the corner, and the long one back up.
    let (cx, cy) = (x - 0.15 * w, y + h);
    list.line(x - w, y + 0.25 * h, cx, cy, thin, VISITED);
    list.line(cx, cy, x + w, y - h, thin, VISITED);
}

/// A euro sign, `size` tall, centred on `(x, y)`: a C open to the right
/// in short straight pieces and two bars across it — a trader's mark on
/// the map (the second map rework), which the galaxy chart puts by a trader's star
/// too (`lobby::preview`'s `euro`, drawn again there as the crown is).
pub fn paint_euro(list: &mut DrawList, x: f32, y: f32, size: f32, thin: f32, c: Color) {
    let r = size / 2.0;
    let (from, to) = (0.8f32, core::f32::consts::TAU - 0.8);
    let pieces = 8;
    let at = |i: u32| {
        let a = from + (to - from) * i as f32 / pieces as f32;
        (x + r * a.cos(), y - r * a.sin())
    };
    for i in 0..pieces {
        let (p, q) = (at(i), at(i + 1));
        list.line(p.0, p.1, q.0, q.1, thin, c);
    }
    for dy in [-0.22 * size, 0.12 * size] {
        list.line(x - r * 1.25, y + dy, x + r * 0.35, y + dy, thin, c);
    }
}

/// An elite's crown, `width` across, its foot centred on `(x, y)`: a
/// band, three points — the middle one tallest — and a jewel on each.
pub fn paint_crown(list: &mut DrawList, x: f32, y: f32, width: f32, thin: f32, c: Color) {
    let w = width / 2.0;
    let h = width * 0.7;
    let points = [
        (x - w, y),
        (x - w, y - h),
        (x - w / 2.0, y - h * 0.45),
        (x, y - h * 1.15),
        (x + w / 2.0, y - h * 0.45),
        (x + w, y - h),
        (x + w, y),
        (x - w, y),
    ];
    for pair in points.windows(2) {
        list.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, thin, c);
    }
    for (px, py) in [points[1], points[3], points[5]] {
        disc(list, px, py, thin * 2.6, c);
    }
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
    let own = game.shows_own_system();
    let system = game.map_system();
    let turn = game.camera_turn() as f32;

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

    // Where a system position lands, in the camera's units about the map's
    // origin — the ship in its own system, the star in another (the map rework).
    let origin = if own {
        game.world.ship.position()
    } else {
        DVec2::ZERO
    };
    let place = |at: DVec2| {
        let offset = at.sub(origin);
        (offset.x as f32, -offset.y as f32)
    };
    // Where an icon is drawn: its spot, moved clear of the icons before it
    // (`Game::map_spots`), before the camera's turn, which comes at the end.
    let spots = game.map_spots();
    let spot_of = |node: Node| {
        spots
            .iter()
            .find(|(n, _)| *n == node)
            .map(|(_, (x, y))| crate::game::turned(*x, *y, -turn))
    };
    let sites = game.map_sites();
    let site_of = |node: Node| sites.iter().find(|s| s.node == node);

    // How far the crew can see. Drawn round the ship because that is where the
    // sensors are, and it is the one thing on the map that explains why the
    // rest of it is empty. Only in the ship's own system.
    if own {
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
    }

    // The star. Always discovered: it is the thing the system is named after
    // and the origin everything else is measured from.
    let (sx, sy) = place(DVec2::ZERO);
    let star = (18.0 / scale) as f32;
    list.ellipse(sx, sy, star, star, STAR);

    // Orbits under everything: a faint ring through each known body, so the
    // map reads as a system rather than as dots. A belt's is a little
    // stronger, because a belt *is* its orbit.
    let thin = (1.0 / scale) as f32;
    let nodes = game.map_nodes();
    for &node in &nodes {
        let Node::Body(id) = node else { continue };
        let Some(body) = system.body(id) else {
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
    // in orbit of its body and at this scale that is on top of it — or
    // beside it, where the two are both sites (`Game::map_spots`). Sized in
    // pixels, like the icons they are: a planet drawn to scale would be a
    // fraction of one.
    let size = (26.0 / scale) as f32;
    for &node in &nodes {
        let Node::Body(id) = node else { continue };
        let Some((x, y)) = spot_of(node) else {
            continue;
        };
        if let Some(body) = system.body(id) {
            paint_body(list, x, y, size, body.kind, thin);
            // A belt is scenery (task 111: the mining sites are gone), and
            // wears no mark.
            // A planet with ground has a settlement on it the ship can
            // land at, and the map says so twice: a landing pad at its
            // shoulder — the one mark that
            // says *this one can be set down on* and a gas giant cannot —
            // and a ring by its side the way it rings a station, since
            // what it is to the crew is the other thing worth knowing
            // before coming down (task 111, `site_ring`). The pad is in
            // the ring's colour too, so the two agree.
            if let Some(site) = site_of(node) {
                let colour = site_ring(site);
                let d = size * STANCE_RING;
                ring(list, x, y, d, d, 0.0, thin * 2.5, colour);
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
    for &node in &nodes {
        let Node::Station(id) = node else { continue };
        let Some((x, y)) = spot_of(node) else {
            continue;
        };
        // A derived jammer or the fortress of a system looked at from
        // afar is no station of the generated system: the machines' relay.
        let kind = system
            .station(id)
            .map(|s| s.kind)
            .unwrap_or(StationKind::Relay);
        paint_station(list, x, y, size * 0.75, kind, thin * 1.5);
        let Some(site) = site_of(node) else {
            continue;
        };
        // Ringed by what it is to the crew (task 111, `site_ring`), outside
        // the aim ring so the two read apart when one is picked: an attack
        // red, a defence amber, a trader green — every site, a derelict
        // too, since every site is one of the three. A little heavier than
        // the stance ring it replaced, since it is the thing the map is
        // read for now.
        let colour = site_ring(site);
        let d = size * STANCE_RING;
        ring(list, x, y, d, d, 0.0, thin * 2.5, colour);
        // A trader wears a euro sign at its upper-right shoulder (task
        // 139), in its ring's green.
        if site.kind == world::SiteKind::Trader {
            paint_euro(
                list,
                x + TICK_SHOULDER * size,
                y - TICK_SHOULDER * size,
                size * 0.55,
                thin * 1.8,
                colour,
            );
        }
        // And an elite crowned, a second ring outside the first.
        if site.elite {
            let elite = if site.cleared {
                ELITE.alpha(0.4)
            } else {
                ELITE
            };
            let d = d * 1.3;
            ring(list, x, y, d, d, 0.0, thin * 2.0, elite);
            paint_crown(
                list,
                x,
                y - CROWN_LIFT * size,
                size * CROWN_WIDTH,
                thin * 1.6,
                elite,
            );
        }
    }

    // Where the crew have already been (feature 85): a tick at the lower
    // shoulder of every node the ship has stopped at, station and body
    // alike, drawn over the icons and under the ring round the pick.
    // The chart is what remembers this — `World::visited`, filed with
    // the system when the ship jumps out — so a system met twice opens
    // with its ticks on. In another system looked at, its memory's ticks
    // (the second map rework), so a system the crew have been to says where.
    let visited = if own {
        Some(&game.world.visited)
    } else {
        let star = game.shown_star();
        game.world
            .memories
            .iter()
            .find(|m| m.star == star)
            .map(|m| &m.visited)
    };
    for &node in visited.into_iter().flatten() {
        let Some((x, y)) = spot_of(node) else {
            continue;
        };
        paint_tick(
            list,
            x - TICK_SHOULDER * size,
            y - TICK_SHOULDER * size,
            size * TICK_SIZE,
            thin * 2.2,
        );
    }

    // The site the crew will go to next — picked on the world map, or the
    // one on the table (the app's `aimed`) — ringed heavily in the
    // hyperdrive's violet over a dark edge, and in the ship's own system a
    // dashed line from the ship to it (the second map rework): where they are heading.
    let aimed_at = match game.aimed {
        Some(flight::Target::Body(id)) => spot_of(Node::Body(id)),
        Some(flight::Target::Station(id)) => spot_of(Node::Station(id)),
        Some(flight::Target::Point(p)) => Some(place(p)),
        None => None,
    };
    if let Some((x, y)) = aimed_at {
        let px = (1.0 / scale) as f32;
        if own {
            let (length, dash) = ((x * x + y * y).sqrt(), 10.0 * px);
            let reach = length - HEADING_RING * px / 2.0 - 4.0 * px;
            let from = HERE_RING * px / 2.0 + 4.0 * px;
            let mut at = from;
            while length > 0.0 && at < reach {
                let end = (at + dash).min(reach);
                let (a, b) = (at / length, end / length);
                list.line(a * x, a * y, b * x, b * y, 2.0 * px, HEADING.alpha(0.85));
                at += dash * 1.8;
            }
        }
        let d = HEADING_RING * px;
        ring(list, x, y, d, d, 0.0, 5.0 * px, VOID.alpha(0.7));
        ring(list, x, y, d, d, 0.0, 3.0 * px, HEADING);
    }

    list.turn_from(out_there, turn);

    // Where you are, over everything: a reticle round the ship — a ring
    // wider than any icon, so it stands out from the station the ship is
    // docked on top of, with a tick at each compass point and a breathing
    // wash inside it — and the ship itself on top, pointing where it is
    // pointing: a little hull with fins, so it says which way round it is
    // and that it is the ship. Head up, that is straight up, and it is the
    // map that says where north went. The reticle is not turned with the
    // world: it is the screen's, and its ticks stay square to the window.
    // Only in the ship's own system: another has no ship in it.
    if own {
        here_reticle(list, scale as f32, game.frame);
        hull::marker(list, game.ship_turn() as f32, (18.0 / scale) as f32, GLOW);
    }
}

/// How wide the reticle round the ship is on the map, in pixels: outside
/// a station's stance ring (`26 * 0.75 * STANCE_RING`, about 25), so the
/// two never sit on each other at a dock.
const HERE_RING: f32 = 44.0;
/// How long a tick of it is, and how far outside the ring it starts.
const HERE_TICK: f32 = 9.0;
/// How wide the ring round the site the crew will go to next is, in
/// pixels (the second map rework): outside a site's stance ring and an elite's second
/// one, so it rings them both.
const HEADING_RING: f32 = 48.0;
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
    list.ellipse(0.0, 0.0, d, d, GLOW.alpha(0.10 + 0.14 * breath));
    ring(list, 0.0, 0.0, d, d, 0.0, 3.0 * px, GLOW.alpha(0.95));
    // Four ticks, outside the ring at the compass points of the window.
    let (from, to) = (d / 2.0 + 2.0 * px, d / 2.0 + 2.0 * px + HERE_TICK * px);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        list.line(
            dx * from,
            dy * from,
            dx * to,
            dy * to,
            2.6 * px,
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

// --- the machines' ship (feature 83) --------------------------------------

/// What a droid hull is drawn in: the machines' own gunmetal, and their
/// sensors in the hostile bolt's red. The same palette `bims::droid`
/// draws a machine in, said again here because this crate has no
/// business reaching into that one's private colours.
const DROID_HULL: Color = Color::rgb(0.18, 0.20, 0.23);
const DROID_PLATE: Color = Color::rgb(0.28, 0.31, 0.35);
const DROID_TRIM: Color = Color::rgb(0.44, 0.47, 0.51);
const DROID_LIGHT: Color = Color::rgb(1.0, 0.28, 0.22);
/// How long a droid ship is along the way it points, and how wide, in
/// room units; and how long a collar it puts against the airlock.
const DROID_SHIP_LONG: f32 = 9.0 * TILE as f32;
const DROID_SHIP_WIDE: f32 = 5.0 * TILE as f32;
const DROID_COLLAR: f32 = TILE as f32;
/// A lander is squatter than a ship: it came down rather than across.
const DROID_LANDER_LONG: f32 = 6.5 * TILE as f32;
const DROID_LANDER_WIDE: f32 = 6.5 * TILE as f32;
/// How many frames a hull light takes to pulse.
const DROID_PULSE: u32 = 110;

/// The machines' ship as seen from above, drawn into a station's own
/// frame: `at` is where its collar meets the station's skin — or, for a
/// lander, where it sits on the ground — and `outward` the unit step
/// away from the station along which it lies.
///
/// A dark wedge of a hull with plating down it, a collar reaching back
/// to the airlock, and three sensor lights along each flank pulsing
/// together. Nothing of the crew's ship: no engine bells, no running
/// lights in white, no deck. It is a shape on the skin that says *they
/// came in that*.
fn droid_ship(list: &mut DrawList, at: (f32, f32), outward: (f32, f32), lander: bool, frame: u32) {
    use crate::draw::{KIND_ELLIPSE, KIND_RECT};
    // A filled shape, the way `DrawList::rect` says it.
    const FILLED: f32 = 0.0;
    let span = (outward.0 * outward.0 + outward.1 * outward.1)
        .sqrt()
        .max(1e-6);
    let dir = (outward.0 / span, outward.1 / span);
    // The shape's own frame: `+x` along `outward`.
    let rot = dir.1.atan2(dir.0);
    let (long, wide) = if lander {
        (DROID_LANDER_LONG, DROID_LANDER_WIDE)
    } else {
        (DROID_SHIP_LONG, DROID_SHIP_WIDE)
    };
    let out = if lander { 0.0 } else { DROID_COLLAR };
    let mid = (
        at.0 + dir.0 * (out + long / 2.0),
        at.1 + dir.1 * (out + long / 2.0),
    );
    // The collar, reaching back to the station's skin.
    if !lander {
        let collar = (at.0 + dir.0 * (out / 2.0), at.1 + dir.1 * (out / 2.0));
        list.push(
            KIND_RECT,
            collar.0,
            collar.1,
            out + 4.0,
            TILE as f32 * 1.6,
            rot,
            3.0,
            FILLED,
            DROID_PLATE,
        );
    }
    // The hull: a dark body with a lighter plate down its spine, rimmed
    // in trim so its edge holds against any fog.
    for (w, h, radius, colour) in [
        (long + 6.0, wide + 6.0, 10.0, DROID_TRIM),
        (long, wide, 9.0, DROID_HULL),
        (long * 0.62, wide * 0.44, 6.0, DROID_PLATE),
    ] {
        list.push(KIND_RECT, mid.0, mid.1, w, h, rot, radius, FILLED, colour);
    }
    // The nose plate, at the far end from the station.
    let nose = (mid.0 + dir.0 * (long * 0.34), mid.1 + dir.1 * (long * 0.34));
    list.push(
        KIND_RECT,
        nose.0,
        nose.1,
        long * 0.22,
        wide * 0.68,
        rot,
        6.0,
        FILLED,
        DROID_HULL,
    );
    // Three sensor lights a flank, pulsing together.
    let t = (frame % DROID_PULSE) as f32 / DROID_PULSE as f32;
    let lit = 0.35 + 0.45 * (t * std::f32::consts::TAU).sin().abs();
    let across = (-dir.1, dir.0);
    for side in [-1.0f32, 1.0] {
        for i in 0..3 {
            let along = (i as f32 - 1.0) * long * 0.26;
            let off = wide * 0.46 * side;
            let p = (
                mid.0 + dir.0 * along + across.0 * off,
                mid.1 + dir.1 * along + across.1 * off,
            );
            list.push(
                KIND_ELLIPSE,
                p.0,
                p.1,
                7.0,
                7.0,
                0.0,
                0.0,
                FILLED,
                DROID_LIGHT.alpha(lit),
            );
        }
    }
    // A lander stands on legs; a ship at an airlock does not.
    if lander {
        for side in [-1.0f32, 1.0] {
            for end in [-1.0f32, 1.0] {
                let p = (
                    mid.0 + dir.0 * (long * 0.34 * end) + across.0 * (wide * 0.5 * side),
                    mid.1 + dir.1 * (long * 0.34 * end) + across.1 * (wide * 0.5 * side),
                );
                list.push(
                    KIND_RECT,
                    p.0,
                    p.1,
                    TILE as f32 * 0.5,
                    TILE as f32 * 1.1,
                    rot,
                    3.0,
                    FILLED,
                    DROID_TRIM,
                );
            }
        }
    }
}

/// How much larger than a tile the engineer's ultimate sentry is drawn
/// (task 127).
const SENTRY_DRAWN: f32 = 1.35;

/// A point of the crew's room in the camera's units: what
/// [`crew_on_screen`] does for a body, for any point.
pub fn room_point_on_screen(game: &Game, p: bims::math::Vec2) -> (f32, f32) {
    on_screen(
        dvec2(p.x as f64, p.y as f64).sub(game.world.aboard.offset),
        game.world.ship.dynamics.centre_of_mass,
        game.ship_turn(),
    )
}

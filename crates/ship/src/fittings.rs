//! The inside of the ship: pictures of the parts the room has none for.
//!
//! The room aboard draws its own fixtures — the galley, the heads, the
//! table, the bunks, the bay, the locker — with the pictures in
//! `crates/game/src/room.rs`, and `hull` draws the skin and everything that
//! fires. What is left is what a body walks past between them: the helm,
//! the shelves, the shower, the bulkheads and the doors in them. On a planet the same parts
//! are asked of [`part_in`] with the ground's biome, and the wall and the
//! wild parts — tree, shrub, boulder, water, field — are drawn as that
//! biome has them. Those used to be a coloured block a tile, which is what
//! the design phase still shows and is fine at eight pixels a tile; at the
//! game's scale a block is a hole in the picture, and a station with rooms
//! in it is mostly bulkhead.
//!
//! Drawn in **design space** like `hull`, in each part's own frame through
//! [`hull::Local`] — `u` across the part, `v` along it towards whoever uses
//! it, which is grid down for a part at `R0` — so a helm turned to face
//! the stern is drawn turned with it and its seat is still on the side the
//! Bim stands. The palette is the room's, so the two halves of the picture
//! read as one deck.

use shipdesign::PlacedPart;
use shipdesign::parts::{PartKind, TILE};
use world::Biome;

use crate::draw::{Color, DrawList, KIND_ELLIPSE, KIND_RECT, Surface};
use crate::hull::{Corner, Local, corner, middle};

const T: f32 = TILE as f32;
/// How far a tile of wall is drawn past its edges, so two side by side
/// overlap and the feathering of their edges shows no seam between them;
/// the texture is one across both, so the overlap shows nothing either.
pub(crate) const LAP: f32 = 1.0;

// --- the room's palette, again ------------------------------------------------

/// Composite panelling in the room's three shades.
const PANEL: Color = Color::rgb(0.19, 0.22, 0.26);
const PANEL_LIT: Color = Color::rgb(0.28, 0.32, 0.37);
const PANEL_EDGE: Color = Color::rgb(0.40, 0.46, 0.53);
const GLOW: Color = Color::rgb(0.38, 0.86, 0.95);
const GOOD: Color = Color::rgb(0.50, 0.90, 0.60);
const WARN: Color = Color::rgb(0.98, 0.45, 0.32);
const STEEL: Color = Color::rgb(0.78, 0.83, 0.87);
const DECK: Color = Color::rgb(0.13, 0.15, 0.18);

/// A bulkhead: the room's wall shade, with a darker seam between panels.
const WALL: Color = Color::rgb(0.30, 0.34, 0.40);
const WALL_TRIM: Color = Color::rgba(0.62, 0.70, 0.78, 0.35);

/// The shower's tray and what is in it.
const TRAY: Color = Color::rgb(0.60, 0.70, 0.76);
const TRAY_LINE: Color = Color::rgba(0.20, 0.26, 0.31, 0.35);
const DRAIN: Color = Color::rgb(0.16, 0.19, 0.22);
const CURTAIN: Color = Color::rgba(0.86, 0.92, 0.96, 0.55);

/// What is on the shelves.
const CRATES: [Color; 4] = [
    Color::rgb(0.55, 0.44, 0.31),
    Color::rgb(0.44, 0.52, 0.58),
    Color::rgb(0.62, 0.58, 0.36),
    Color::rgb(0.38, 0.46, 0.40),
];

/// Life support's teal — the palette swatch, so the deck and the design
/// phase agree about what is what.
const LIFE: Color = Color::rgb(0.34, 0.62, 0.52);
const STRIPE: Color = Color::rgb(0.92, 0.72, 0.18);

/// The workshop: the smelter's melt from dull to white, the firebrick
/// round it, the bench top, the lamp over it and the board under it, the
/// suit through the locker's window, and the armoury's gunmetal and the
/// rifles' stocks. The part colours are the palette swatches again.
const BENCH: Color = Color::rgb(0.56, 0.50, 0.38);
const BENCH_EDGE: Color = Color::rgba(0.30, 0.26, 0.18, 0.6);
const LAMP: Color = Color::rgb(1.0, 0.92, 0.70);
const CIRCUIT: Color = Color::rgb(0.16, 0.36, 0.28);
const SUIT: Color = Color::rgb(0.78, 0.80, 0.84);
const VISOR: Color = Color::rgb(0.38, 0.62, 0.78);
const GUNMETAL: Color = Color::rgb(0.42, 0.38, 0.44);
const STOCK: Color = Color::rgb(0.42, 0.30, 0.22);
/// The clinical green-white the drug lab was drawn in, until it went with
/// the medicine (task 120); the research desk's screen still is.
const LAB: Color = Color::rgb(0.74, 0.82, 0.78);

/// The picture for an interior part, if it has one. `false` means the
/// caller draws its block — the same contract as [`hull::part`], which is
/// asked first. The ship's look: [`part_in`] with no biome.
pub fn part(list: &mut DrawList, part: &PlacedPart) -> bool {
    part_in(list, part, None)
}

/// The picture for a part where it stands: aboard a ship or a station
/// (`None`), or on a planet's ground in a biome (`Some`), where a wall is
/// stone, adobe or timber rather than a bulkhead and the wild parts —
/// the tree, the shrub, the boulder, the water, the field — are drawn as
/// that biome grows them. With `None` the wild parts are the temperate
/// look, which is what the designer's ghost and a station show.
pub fn part_in(list: &mut DrawList, part: &PlacedPart, biome: Option<Biome>) -> bool {
    match part.kind {
        PartKind::Wall => {
            for tile in part.tiles() {
                match biome {
                    Some(biome) => stone_wall(list, tile, biome),
                    None => wall(list, tile),
                }
            }
        }
        PartKind::Field => field(list, part),
        PartKind::Tree => tree(list, part, biome.unwrap_or(Biome::Temperate)),
        PartKind::Shrub => shrub(list, part, biome.unwrap_or(Biome::Temperate)),
        PartKind::Boulder => boulder(list, part, biome.unwrap_or(Biome::Temperate)),
        PartKind::Water => water(list, part, biome.unwrap_or(Biome::Temperate)),
        PartKind::DiagonalWall => diagonal_wall(list, part),
        PartKind::Door => door(list, part),
        PartKind::Helm => helm(list, part),
        PartKind::Shelf => shelf(list, part),
        PartKind::Shower => shower(list, part),
        PartKind::LifeSupport => life_support(list, part),
        PartKind::Workbench => workbench(list, part),
        PartKind::SuitLocker => suit_locker(list, part),
        PartKind::Armoury => armoury(list, part),
        PartKind::TradingDesk => trading_desk(list, part),
        PartKind::Sandbags => sandbags(list, part),
        PartKind::ResearchDesk => research_desk(list, part),
        PartKind::Hyperdrive => hyperdrive(list, part),
        PartKind::WallLight => wall_light(list, part),
        PartKind::StandingLight => standing_light(list, part),
        PartKind::SmallPlant => small_plant(list, part),
        PartKind::BigPlant => big_plant(list, part),
        PartKind::Picture => picture(list, part),
        _ => return false,
    }
    true
}

// --- bulkheads ---------------------------------------------------------------------

/// One tile of bulkhead: a painted steel panel in a riveted frame, the
/// [`Surface::Bulkhead`] texture anchored to the tile so a run of them
/// is a run of panels, joint to joint.
fn wall(list: &mut DrawList, tile: (u32, u32)) {
    let (cx, cy) = middle(tile.0, tile.1);
    list.surface(
        Surface::Bulkhead,
        cx,
        cy,
        T + LAP,
        T + LAP,
        0.0,
        (cx, cy),
        WALL,
    );
}

/// A bulkhead cut across its tile, with the trim along the cut.
fn diagonal_wall(list: &mut DrawList, part: &PlacedPart) {
    let (cx, cy) = middle(part.origin.0, part.origin.1);
    let c: Corner = corner(part.rotation);
    list.surface_triangle(Surface::Bulkhead, cx, cy, T - 1.0, T - 1.0, c.rot, WALL);
    let trim = 2.0;
    list.push(
        KIND_RECT,
        cx - c.normal.0 * trim,
        cy - c.normal.1 * trim,
        (T - 1.0) * core::f32::consts::SQRT_2 - 3.0 * trim,
        1.5,
        c.along,
        0.0,
        0.0,
        WALL_TRIM,
    );
}

/// A door: a threshold in the deck, and the two leaves drawn back into the
/// bulkhead either side of it. Two tiles along the bulkhead and one deep,
/// drawn in the part's own frame so it turns with the part — `along` is
/// the run and `across` the bulkhead's depth, and which is which is the
/// rotation's business, read where the room reads it
/// (`door_slides_along_x`) rather than guessed off the neighbours.
fn door(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    // The threshold: a lighter strip of deck the length of the opening.
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        across * 0.5,
        along - 2.0,
        2.0,
        0.0,
        PANEL,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        across * 0.34,
        along - 6.0,
        2.0,
        0.0,
        DECK,
    );
    // The two leaves, a tile each, drawn back so the way through is open.
    let leaf = along * 0.22;
    for side in [-1.0f32, 1.0] {
        let v = side * (along / 2.0 - leaf / 2.0 - 1.0);
        local.push(
            list,
            KIND_RECT,
            0.0,
            v,
            across * 0.28,
            leaf,
            1.5,
            0.0,
            PANEL_LIT,
        );
        local.push(
            list,
            KIND_RECT,
            0.0,
            v,
            across * 0.28,
            leaf,
            1.5,
            1.0,
            PANEL_EDGE,
        );
    }
    // And the lamp over it, lit.
    let (gx, gy) = local.at(across * 0.34, 0.0);
    list.ellipse(gx, gy, 5.0, 5.0, GOOD);
}

// --- systems -------------------------------------------------------------------------

/// The helm: a console the width of the part with a wide screen along its
/// far edge, instruments under it, and a yoke on the near side where the
/// pilot stands.
fn helm(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    // The desk, and a lighter working surface let into it.
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 5.0, 0.0, PANEL);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 5.0, 1.5, PANEL_EDGE);
    // The screen, dark with the glow of a chart on it.
    let screen_h = h * 0.42;
    let screen_v = -h / 2.0 + screen_h / 2.0 + 4.0;
    local.push(
        list,
        KIND_RECT,
        0.0,
        screen_v,
        w - 10.0,
        screen_h,
        3.0,
        0.0,
        DRAIN,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        screen_v,
        w - 14.0,
        screen_h - 4.0,
        2.0,
        0.0,
        GLOW.alpha(0.16),
    );
    // A track across the chart, and the ship on it.
    local.push(
        list,
        KIND_RECT,
        0.0,
        screen_v + 1.0,
        w - 24.0,
        1.5,
        0.0,
        0.0,
        GLOW.alpha(0.55),
    );
    local.push(
        list,
        KIND_ELLIPSE,
        -w * 0.18,
        screen_v + 1.0,
        6.0,
        6.0,
        0.0,
        0.0,
        GLOW,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        w * 0.3,
        screen_v + 1.0,
        4.0,
        4.0,
        0.0,
        1.0,
        GLOW.alpha(0.7),
    );
    // Instruments under the screen: a row of readouts in three colours.
    let row_v = screen_v + screen_h / 2.0 + 7.0;
    let lights = [
        (-0.36, GOOD),
        (-0.24, GOOD),
        (-0.12, GLOW),
        (0.12, GLOW),
        (0.24, WARN),
        (0.36, GOOD),
    ];
    for &(u, color) in &lights {
        local.push(
            list,
            KIND_RECT,
            u * w,
            row_v,
            w * 0.08,
            5.0,
            1.0,
            0.0,
            color.alpha(0.85),
        );
    }
    // The yoke, on the pilot's side, and the throttle beside it.
    let yoke_v = h / 2.0 - 9.0;
    local.push(list, KIND_RECT, 0.0, yoke_v, w * 0.3, 5.0, 2.0, 0.0, STEEL);
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        yoke_v - 1.0,
        8.0,
        8.0,
        0.0,
        0.0,
        PANEL_EDGE,
    );
    local.push(
        list,
        KIND_RECT,
        w * 0.32,
        yoke_v - 2.0,
        4.0,
        12.0,
        1.0,
        0.0,
        STEEL,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        w * 0.32,
        yoke_v - 7.0,
        6.0,
        6.0,
        0.0,
        0.0,
        WARN,
    );
}

// --- stores ----------------------------------------------------------------------------

/// A shelf: a rack seen from above, three boards deep, with crates on them
/// and the open side towards whoever takes something down.
fn shelf(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 0.0, PANEL);
    // The uprights down each side.
    for u in [-w / 2.0 + 2.5, w / 2.0 - 2.5] {
        local.push(list, KIND_RECT, u, 0.0, 4.0, h, 1.0, 0.0, PANEL_EDGE);
    }
    // Three boards, each with a couple of crates on it.
    for (row, share) in [-0.3f32, 0.0, 0.3].iter().enumerate() {
        let v = share * h;
        local.push(
            list,
            KIND_RECT,
            0.0,
            v + 5.0,
            w - 8.0,
            2.0,
            0.0,
            0.0,
            PANEL_EDGE,
        );
        for (i, u) in [-w * 0.22, w * 0.1, w * 0.3].iter().enumerate() {
            // Not every place is taken, or the rack reads as a solid block.
            if (row + i) % 3 == 2 {
                continue;
            }
            let crate_w = if i == 1 { w * 0.3 } else { w * 0.2 };
            local.push(
                list,
                KIND_RECT,
                *u,
                v,
                crate_w,
                9.0,
                1.5,
                0.0,
                CRATES[(row * 2 + i) % CRATES.len()],
            );
        }
    }
}

// --- the shower ----------------------------------------------------------------------

/// A shower: a tiled tray with the drain in the middle, the head on an arm
/// out of the far wall, and a curtain drawn back along one side.
fn shower(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 4.0, 0.0, TRAY);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 4.0, 1.5, PANEL_EDGE);
    // Tile lines across the tray, both ways.
    for share in [-0.25f32, 0.25] {
        local.push(
            list,
            KIND_RECT,
            share * w,
            0.0,
            1.0,
            h - 6.0,
            0.0,
            0.0,
            TRAY_LINE,
        );
        local.push(
            list,
            KIND_RECT,
            0.0,
            share * h,
            w - 6.0,
            1.0,
            0.0,
            0.0,
            TRAY_LINE,
        );
    }
    // The drain.
    local.push(list, KIND_ELLIPSE, 0.0, 0.0, 10.0, 10.0, 0.0, 0.0, DRAIN);
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        5.0,
        5.0,
        0.0,
        0.0,
        TRAY_LINE.alpha(0.8),
    );
    // The head, on its arm out of the far wall.
    let arm_v = -h / 2.0 + 6.0;
    local.push(
        list,
        KIND_RECT,
        0.0,
        arm_v + 2.0,
        3.0,
        12.0,
        1.0,
        0.0,
        STEEL,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        arm_v + 9.0,
        11.0,
        11.0,
        0.0,
        0.0,
        STEEL,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        arm_v + 9.0,
        6.0,
        6.0,
        0.0,
        0.0,
        PANEL_EDGE,
    );
    // The curtain rail down one side, with the curtain bunched at the far
    // end of it.
    let rail_u = w / 2.0 - 4.0;
    local.push(list, KIND_RECT, rail_u, 0.0, 2.0, h - 4.0, 0.0, 0.0, STEEL);
    for i in 0..4 {
        let v = -h / 2.0 + 6.0 + i as f32 * 6.0;
        local.push(
            list,
            KIND_ELLIPSE,
            rail_u - 1.0,
            v,
            9.0,
            7.0,
            0.0,
            0.0,
            CURTAIN,
        );
    }
}

// --- machinery ----------------------------------------------------------------------

/// Life support: a cabinet with the big fan behind its grille, the pipes
/// out of one end, and a lamp that says the air is good.
fn life_support(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 5.0, 0.0, PANEL);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 5.0, 1.5, PANEL_EDGE);
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        w - 12.0,
        h - 12.0,
        4.0,
        0.0,
        LIFE.alpha(0.35),
    );
    // The fan: a well, four blades, and the hub.
    let d = w.min(h) * 0.62;
    let (fu, fv) = (-w * 0.12, 0.0);
    local.push(list, KIND_ELLIPSE, fu, fv, d, d, 0.0, 0.0, DRAIN);
    for i in 0..4 {
        let angle = i as f32 * core::f32::consts::FRAC_PI_4;
        let (x, y) = local.at(fu, fv);
        list.push(KIND_RECT, x, y, d * 0.86, d * 0.16, angle, 3.0, 0.0, LIFE);
    }
    local.push(
        list,
        KIND_ELLIPSE,
        fu,
        fv,
        d * 0.26,
        d * 0.26,
        0.0,
        0.0,
        STEEL,
    );
    local.push(list, KIND_ELLIPSE, fu, fv, d, d, 0.0, 2.0, PANEL_EDGE);
    // The grille: three bars across the fan.
    for i in 0..3 {
        let v = fv - d * 0.3 + i as f32 * d * 0.3;
        local.push(
            list,
            KIND_RECT,
            fu,
            v,
            d,
            2.0,
            0.0,
            0.0,
            PANEL_EDGE.alpha(0.8),
        );
    }
    // Pipes down the other side, and the lamp.
    for i in 0..3 {
        let u = w * 0.3 + (i as f32 - 1.0) * 7.0;
        local.push(list, KIND_RECT, u, 0.0, 4.0, h - 16.0, 2.0, 0.0, STEEL);
    }
    local.push(
        list,
        KIND_ELLIPSE,
        w * 0.3,
        h / 2.0 - 7.0,
        6.0,
        6.0,
        0.0,
        0.0,
        GOOD,
    );
}

// --- the workshop ------------------------------------------------------------------

/// The workbench: a bench top with a tool rail along its far edge and the
/// tools hung on it, a vice at one end, a lamp over the other, and the
/// job in the middle — a circuit board with its parts on it. The drawer is
/// on the near side, where the Bim stands.
fn workbench(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    // The frame under the top, then the top, a shade lighter on the far
    // two thirds where the light falls.
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 0.0, PANEL);
    local.push(
        list,
        KIND_RECT,
        0.0,
        -2.0,
        w - 4.0,
        h - 10.0,
        2.0,
        0.0,
        BENCH,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        -2.0,
        w - 4.0,
        h - 10.0,
        2.0,
        1.0,
        BENCH_EDGE,
    );
    // The drawer face along the near edge, and its pull.
    local.push(
        list,
        KIND_RECT,
        0.0,
        h / 2.0 - 4.0,
        w - 8.0,
        6.0,
        1.5,
        0.0,
        PANEL_LIT,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        h / 2.0 - 4.0,
        w * 0.2,
        2.0,
        1.0,
        0.0,
        STEEL,
    );
    // The rail along the far edge, with a spanner, a driver and a file on it.
    let rv = -h / 2.0 + 5.0;
    local.push(
        list,
        KIND_RECT,
        0.0,
        rv,
        w - 12.0,
        2.0,
        0.0,
        0.0,
        PANEL_EDGE,
    );
    local.push(
        list,
        KIND_RECT,
        -w * 0.3,
        rv + 4.0,
        3.0,
        11.0,
        1.0,
        0.0,
        STEEL,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        -w * 0.3,
        rv + 1.5,
        6.0,
        5.0,
        0.0,
        0.0,
        STEEL,
    );
    local.push(
        list,
        KIND_RECT,
        -w * 0.2,
        rv + 5.0,
        2.5,
        8.0,
        1.0,
        0.0,
        STEEL,
    );
    local.push(
        list,
        KIND_RECT,
        -w * 0.2,
        rv + 10.0,
        4.0,
        5.0,
        1.0,
        0.0,
        WARN,
    );
    local.push(
        list,
        KIND_RECT,
        -w * 0.1,
        rv + 5.0,
        3.5,
        12.0,
        0.5,
        0.0,
        PANEL_EDGE,
    );
    // The vice at the right-hand end: its body, the two jaws, the handle.
    let vu = w * 0.36;
    local.push(list, KIND_RECT, vu, 2.0, 16.0, 14.0, 2.0, 0.0, PANEL_LIT);
    local.push(list, KIND_RECT, vu - 3.0, 2.0, 4.0, 12.0, 1.0, 0.0, STEEL);
    local.push(list, KIND_RECT, vu + 4.0, 2.0, 4.0, 12.0, 1.0, 0.0, STEEL);
    local.push(list, KIND_RECT, vu + 10.0, 2.0, 2.0, 12.0, 1.0, 0.0, STEEL);
    // The job: a board with its parts on it, and the lamp's pool of light.
    local.push(
        list,
        KIND_ELLIPSE,
        -w * 0.05,
        0.0,
        34.0,
        20.0,
        0.0,
        0.0,
        LAMP.alpha(0.16),
    );
    local.push(
        list,
        KIND_RECT,
        -w * 0.05,
        1.0,
        22.0,
        14.0,
        1.5,
        0.0,
        CIRCUIT,
    );
    for (du, dv, c) in [
        (-6.0, -3.0, GOOD),
        (0.0, -3.0, STEEL),
        (6.0, -3.0, GOOD),
        (-4.0, 3.0, GLOW),
        (5.0, 3.0, STEEL),
    ] {
        local.push(
            list,
            KIND_RECT,
            -w * 0.05 + du,
            1.0 + dv,
            4.0,
            3.0,
            0.5,
            0.0,
            c,
        );
    }
    // The lamp on its arm at the left-hand end, lit.
    local.push(list, KIND_RECT, -w * 0.38, -2.0, 3.0, 16.0, 1.0, 0.0, STEEL);
    local.push(
        list,
        KIND_ELLIPSE,
        -w * 0.38,
        -8.0,
        10.0,
        10.0,
        0.0,
        0.0,
        PANEL_LIT,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        -w * 0.38,
        -8.0,
        5.0,
        5.0,
        0.0,
        0.0,
        LAMP,
    );
}

/// The suit locker: a cabinet with a window in the door, and the suit
/// through it — helmet, visor and shoulders, white against the dark of
/// the inside — with the air line coiled beside it and the ready lamp lit.
fn suit_locker(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 0.0, PANEL);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 1.5, PANEL_EDGE);
    // The door, with its window and the dark inside showing through.
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        w - 8.0,
        h - 8.0,
        2.0,
        0.0,
        PANEL_LIT,
    );
    local.push(
        list,
        KIND_RECT,
        -3.0,
        -1.0,
        w - 20.0,
        h - 16.0,
        2.0,
        0.0,
        DRAIN,
    );
    // The suit: shoulders, then the helmet over them, then the visor.
    local.push(list, KIND_RECT, -3.0, 7.0, 18.0, 12.0, 4.0, 0.0, SUIT);
    local.push(list, KIND_ELLIPSE, -3.0, -3.0, 14.0, 14.0, 0.0, 0.0, SUIT);
    local.push(list, KIND_RECT, -3.0, -3.0, 10.0, 5.0, 2.0, 0.0, VISOR);
    // The air line hung in a coil beside it, and the lamp on the frame.
    local.push(
        list,
        KIND_ELLIPSE,
        w / 2.0 - 8.0,
        -6.0,
        8.0,
        8.0,
        0.0,
        1.5,
        STEEL,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        w / 2.0 - 8.0,
        4.0,
        8.0,
        8.0,
        0.0,
        1.5,
        STEEL,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        w / 2.0 - 8.0,
        h / 2.0 - 7.0,
        4.0,
        4.0,
        0.0,
        0.0,
        GOOD,
    );
    // The handle on the near side, where the Bim reaches for it.
    local.push(
        list,
        KIND_RECT,
        -3.0,
        h / 2.0 - 5.0,
        12.0,
        2.5,
        1.0,
        0.0,
        STEEL,
    );
}

/// The armoury: a gunmetal cabinet with a barred window down its front and
/// the rack behind it, three rifles stood in it, a strongbox at one end
/// and the lock's lamp glowing red — shut, and staying so.
fn armoury(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 0.0, GUNMETAL);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 1.5, PANEL_EDGE);
    // The rack: a dark well with the rifles stood in it, seen end on as
    // stocks, barrels running away from the Bim.
    let rw = w * 0.6;
    local.push(
        list,
        KIND_RECT,
        -w * 0.14,
        0.0,
        rw,
        h - 10.0,
        2.0,
        0.0,
        DRAIN,
    );
    for i in 0..3 {
        let u = -w * 0.14 + (i as f32 - 1.0) * rw * 0.3;
        local.push(list, KIND_RECT, u, -3.0, 4.0, h - 20.0, 1.0, 0.0, STEEL);
        local.push(
            list,
            KIND_RECT,
            u,
            h / 2.0 - 12.0,
            7.0,
            9.0,
            1.5,
            0.0,
            STOCK,
        );
        local.push(
            list,
            KIND_RECT,
            u,
            -h / 2.0 + 10.0,
            5.0,
            3.0,
            0.5,
            0.0,
            PANEL_EDGE,
        );
    }
    // The bars across the window.
    for i in 0..2 {
        let v = (i as f32 - 0.5) * (h - 10.0) * 0.4;
        local.push(list, KIND_RECT, -w * 0.14, v, rw, 2.0, 0.0, 0.0, PANEL_EDGE);
    }
    // The strongbox at the other end, with its dial, and the lock's lamp.
    let bu = w * 0.32;
    local.push(
        list,
        KIND_RECT,
        bu,
        -4.0,
        w * 0.24,
        h - 22.0,
        2.0,
        0.0,
        PANEL,
    );
    local.push(
        list,
        KIND_RECT,
        bu,
        -4.0,
        w * 0.24,
        h - 22.0,
        2.0,
        1.0,
        PANEL_EDGE,
    );
    local.push(list, KIND_ELLIPSE, bu, -4.0, 9.0, 9.0, 0.0, 0.0, STEEL);
    local.push(list, KIND_RECT, bu, -6.0, 1.5, 4.0, 0.0, 0.0, DRAIN);
    local.push(
        list,
        KIND_ELLIPSE,
        bu,
        h / 2.0 - 7.0,
        5.0,
        5.0,
        0.0,
        0.0,
        WARN,
    );
    // Hazard chevrons along the near edge, either side of the lamp.
    for i in 0..3 {
        let u = -w * 0.38 + i as f32 * 9.0;
        local.push(
            list,
            KIND_RECT,
            u,
            h / 2.0 - 5.0,
            5.0,
            4.0,
            0.0,
            0.0,
            STRIPE,
        );
    }
}

// --- the trading desk ---------------------------------------------------------------

/// A station's trading desk: a counter across the two tiles in the bench's
/// wood, a raised ledge along the far edge with a terminal standing on it,
/// a ledger open on the near side where the crew member stands, and a
/// coin tray beside it — a desk to trade across, not a bench to work at.
fn trading_desk(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 0.0, PANEL);
    local.push(list, KIND_RECT, 0.0, 0.0, w - 4.0, h - 6.0, 2.0, 0.0, BENCH);
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        w - 4.0,
        h - 6.0,
        2.0,
        1.0,
        BENCH_EDGE,
    );
    // The ledge along the far edge, and the terminal on it, lit.
    local.push(
        list,
        KIND_RECT,
        0.0,
        -h / 2.0 + 6.0,
        w - 8.0,
        6.0,
        1.0,
        0.0,
        PANEL_EDGE,
    );
    local.push(
        list,
        KIND_RECT,
        w * 0.28,
        -h / 2.0 + 9.0,
        14.0,
        10.0,
        1.5,
        0.0,
        PANEL,
    );
    local.push(
        list,
        KIND_RECT,
        w * 0.28,
        -h / 2.0 + 9.0,
        10.0,
        6.0,
        0.5,
        0.0,
        LAB,
    );
    // The ledger, open, two pages with a spine, on the near side.
    local.push(list, KIND_RECT, -w * 0.2, 4.0, 20.0, 14.0, 1.0, 0.0, STEEL);
    local.push(
        list,
        KIND_RECT,
        -w * 0.2,
        4.0,
        1.5,
        12.0,
        0.0,
        0.0,
        PANEL_EDGE,
    );
    for line in [-3.0f32, 0.0, 3.0] {
        local.push(
            list,
            KIND_RECT,
            -w * 0.2 - 5.0,
            4.0 + line,
            6.0,
            0.8,
            0.0,
            0.0,
            PANEL_EDGE,
        );
        local.push(
            list,
            KIND_RECT,
            -w * 0.2 + 5.0,
            4.0 + line,
            6.0,
            0.8,
            0.0,
            0.0,
            PANEL_EDGE,
        );
    }
    // The coin tray, a shallow dish with a couple of coins in it.
    local.push(
        list,
        KIND_ELLIPSE,
        w * 0.18,
        5.0,
        14.0,
        9.0,
        0.0,
        0.0,
        PANEL_EDGE,
    );
    for (du, dv) in [(-3.0f32, 0.0f32), (2.5, -1.5), (1.0, 2.0)] {
        local.push(
            list,
            KIND_ELLIPSE,
            w * 0.18 + du,
            5.0 + dv,
            4.0,
            4.0,
            0.0,
            0.0,
            CRATES[3],
        );
    }
}

// --- the research desk ----------------------------------------------------------

/// The research desk's own colours: the console's blue-grey, its screens
/// lit in the deck's glow, and the key's slot, a dark well with a brass
/// rim, on the far side of the console. The part colour is the palette
/// swatch again.
const CONSOLE: Color = Color::rgb(0.30, 0.52, 0.62);
const SCREEN: Color = Color::rgb(0.16, 0.30, 0.38);
const SCREEN_LIT: Color = Color::rgba(0.38, 0.86, 0.95, 0.85);
const SLOT: Color = Color::rgb(0.08, 0.09, 0.11);
const BRASS: Color = Color::rgb(0.80, 0.66, 0.30);

/// The research desk: a console on a table's footprint, worked from the
/// near side, with two screens lit along it, a keyboard's ledge in
/// front, and the key's slot let into the top on the far side — a tall
/// dark well with a brass rim, the shape of the key that goes in it. The
/// AI lives in the cabinet under it, which is the thing itself.
fn research_desk(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 3.0, 0.0, PANEL);
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        w - 4.0,
        h - 6.0,
        2.0,
        0.0,
        CONSOLE,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        w - 4.0,
        h - 6.0,
        2.0,
        1.0,
        PANEL_EDGE,
    );
    // Two screens along the far edge, each with a lit line or two on it.
    for (i, u) in [-w * 0.26, w * 0.06].into_iter().enumerate() {
        local.push(
            list,
            KIND_RECT,
            u,
            -h / 2.0 + 11.0,
            22.0,
            12.0,
            1.5,
            0.0,
            SCREEN,
        );
        for (j, dv) in [-3.0f32, 0.0, 3.0].into_iter().enumerate() {
            let len = if (i + j) % 2 == 0 { 14.0 } else { 9.0 };
            local.push(
                list,
                KIND_RECT,
                u - (22.0 - len) / 2.0 + 3.0,
                -h / 2.0 + 11.0 + dv,
                len,
                1.2,
                0.0,
                0.0,
                SCREEN_LIT,
            );
        }
    }
    // The key's slot at the far right, tall and narrow with a brass rim.
    local.push(
        list,
        KIND_RECT,
        w * 0.36,
        -h / 2.0 + 12.0,
        9.0,
        16.0,
        1.5,
        0.0,
        BRASS,
    );
    local.push(
        list,
        KIND_RECT,
        w * 0.36,
        -h / 2.0 + 12.0,
        5.0,
        12.0,
        1.0,
        0.0,
        SLOT,
    );
    // The keyboard's ledge on the near side, and a few keys on it.
    local.push(
        list,
        KIND_RECT,
        0.0,
        h / 2.0 - 8.0,
        w - 12.0,
        7.0,
        1.0,
        0.0,
        PANEL_EDGE,
    );
    for i in 0..6 {
        let u = -w * 0.3 + i as f32 * w * 0.12;
        local.push(list, KIND_RECT, u, h / 2.0 - 8.0, 4.0, 3.0, 0.5, 0.0, STEEL);
    }
}

// --- the hyperdrive ---------------------------------------------------------------

/// The hyperdrive's violet, the far end of the exhaust: the palette swatch.
const HYPER: Color = Color::rgb(0.62, 0.42, 0.86);
const HYPER_CORE: Color = Color::rgb(0.90, 0.84, 1.0);

/// The hyperdrive: a housing, six field coils round a ring, and a core
/// that is the violet of the exhaust's tail. The picture only; whether it is bolted to an engine is the
/// validator's, and the checks panel says so.
fn hyperdrive(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 6.0, along - 6.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 5.0, 0.0, PANEL);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 5.0, 1.5, PANEL_EDGE);
    let d = w.min(h) * 0.72;
    local.push(list, KIND_ELLIPSE, 0.0, 0.0, d, d, 0.0, 0.0, PANEL_LIT);
    local.push(list, KIND_ELLIPSE, 0.0, 0.0, d, d, 0.0, 2.0, PANEL_EDGE);
    for i in 0..6 {
        let a = i as f32 * core::f32::consts::FRAC_PI_3;
        let (u, v) = (a.cos() * d * 0.42, a.sin() * d * 0.42);
        local.push(list, KIND_ELLIPSE, u, v, 8.0, 8.0, 0.0, 0.0, STEEL);
        local.push(list, KIND_ELLIPSE, u, v, 4.0, 4.0, 0.0, 0.0, HYPER);
    }
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        d * 0.56,
        d * 0.56,
        0.0,
        0.0,
        DRAIN,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        d * 0.4,
        d * 0.4,
        0.0,
        0.0,
        HYPER.alpha(0.7),
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        d * 0.18,
        d * 0.18,
        0.0,
        0.0,
        HYPER_CORE,
    );
}

// --- the lights --------------------------------------------------------------------

/// Lamplight, and the fitting it comes out of.
pub(crate) const LAMPLIGHT: Color = Color::rgb(1.0, 0.92, 0.70);
const FITTING: Color = Color::rgb(0.30, 0.32, 0.36);
/// The wall lamp's own light: cooler than the rest of the lamplight, a
/// tube's blue-white rather than a bulb's. Its hot middle is nearly
/// white.
pub(crate) const WALL_LAMP: Color = Color::rgb(0.74, 0.88, 1.0);
const WALL_LAMP_CORE: Color = Color::rgb(0.92, 0.97, 1.0);
/// How far off the wall the wall lamp's casing stands, and its glass's
/// height, in points. The whole fitting is flat against the wall.
const WALL_LAMP_CASE: f32 = 9.0;
const WALL_LAMP_GLASS: f32 = 4.0;

/// A wall light: a flat strip lamp flush against the wall its rotation
/// names — the top edge of its tile unturned, `wall_light_back` — a dark
/// casing hugging the wall with a bar of glass along its face, the way a
/// tube light sits on a wall rather than hanging off it into the
/// gangway. No halo: what the light does to the deck is the room's light
/// map, which the host draws over this.
fn wall_light(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    // The casing, flat along the wall's face.
    local.push(
        list,
        KIND_RECT,
        0.0,
        -h * 0.5 + WALL_LAMP_CASE * 0.5,
        w * 0.56,
        WALL_LAMP_CASE,
        1.5,
        0.0,
        FITTING,
    );
    // The glass along it, and its hot middle.
    let (u, v, gw, gh) = wall_lamp_glass(w, h);
    local.push(list, KIND_RECT, u, v, gw, gh, 1.0, 0.0, WALL_LAMP);
    local.push(
        list,
        KIND_RECT,
        u,
        v,
        gw * 0.7,
        gh * 0.4,
        0.5,
        0.0,
        WALL_LAMP_CORE,
    );
}

/// Where a wall lamp's glass is in its tile's frame, and how big: the
/// bar set into the casing's face, `(u, v, w, h)`.
fn wall_lamp_glass(w: f32, h: f32) -> (f32, f32, f32, f32) {
    (
        0.0,
        -h * 0.5 + WALL_LAMP_CASE * 0.5 + 1.0,
        w * 0.44,
        WALL_LAMP_GLASS,
    )
}

/// A standing light: a pole on a round base with the lamp head over it.
/// No halo, for the wall light's reason.
fn standing_light(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        h * 0.28,
        w * 0.5,
        h * 0.28,
        0.0,
        0.0,
        FITTING,
    );
    local.push(list, KIND_RECT, 0.0, 0.0, 4.0, h * 0.6, 0.0, 0.0, STEEL);
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        -h * 0.3,
        w * 0.44,
        w * 0.44,
        0.0,
        0.0,
        LAMPLIGHT,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        -h * 0.3,
        w * 0.44,
        w * 0.44,
        0.0,
        1.5,
        FITTING,
    );
}

/// The glass of a lamp that is out, and the crack across it.
const GLASS_OUT: Color = Color::rgb(0.22, 0.23, 0.25);
const CRACK: Color = Color::rgba(0.06, 0.06, 0.07, 0.9);

/// A lamp's glass as the fight left it, drawn over its picture: nothing
/// while it is whole and steady; a veil the darker the dimmer it is
/// shown while it flickers (`level` nought to one); and, out (`share`
/// of its health nought), the glass gone dark with a crack across it.
/// What the light does to the deck is the room's light map, as ever;
/// this is the fitting alone. See `bims::sight::Lamp`.
pub fn lamp_face(list: &mut DrawList, part: &PlacedPart, share: f32, level: f32) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    // Where the glass is, and how big — and its shape: the wall light's
    // bar flat on its wall, the standing light's round head.
    let (kind, (u, v, gw, gh)) = match part.kind {
        PartKind::WallLight => (KIND_RECT, wall_lamp_glass(w, h)),
        PartKind::StandingLight => (KIND_ELLIPSE, (0.0, -h * 0.3, w * 0.44, w * 0.44)),
        _ => return,
    };
    if share <= 0.0 {
        local.push(list, kind, u, v, gw, gh, 0.0, 0.0, GLASS_OUT);
        let (x0, y0) = local.at(u - gw * 0.3, v - gh * 0.25);
        let (x1, y1) = local.at(u + gw * 0.1, v + gh * 0.3);
        let (x2, y2) = local.at(u + gw * 0.35, v - gh * 0.1);
        list.line(x0, y0, x1, y1, 1.5, CRACK);
        list.line(x1, y1, x2, y2, 1.5, CRACK);
        return;
    }
    let dim = 1.0 - level.clamp(0.0, 1.0);
    if dim > 0.01 {
        local.push(
            list,
            kind,
            u,
            v,
            gw,
            gh,
            0.0,
            0.0,
            GLASS_OUT.alpha(0.85 * dim),
        );
    }
}

// --- the comforts ------------------------------------------------------------------

/// A pot's terracotta and the soil in it, and the plants' two greens —
/// the palette swatches for the leaf, so the design phase's block and the
/// deck's plant read as one thing.
const POT: Color = Color::rgb(0.62, 0.40, 0.28);
const POT_RIM: Color = Color::rgb(0.74, 0.50, 0.36);
const SOIL: Color = Color::rgb(0.24, 0.17, 0.11);
const LEAF: Color = Color::rgb(0.42, 0.66, 0.36);
const LEAF_DARK: Color = Color::rgb(0.30, 0.56, 0.30);
const LEAF_LIGHT: Color = Color::rgb(0.58, 0.78, 0.44);
/// A picture's frame, and what is in it: a sky over a hill, which is
/// what anybody on a ship hangs on the wall.
pub(crate) const FRAME_BRASS: Color = Color::rgb(0.72, 0.58, 0.32);
const CANVAS_SKY: Color = Color::rgb(0.52, 0.70, 0.86);
const CANVAS_HILL: Color = Color::rgb(0.34, 0.52, 0.30);

/// Foliage seen from above: a cluster of round leaves round `(u, v)`,
/// the dark ones under, the light ones on top, `spread` out from the
/// middle. Both plants are this at a size.
fn foliage(list: &mut DrawList, local: &Local, u: f32, v: f32, spread: f32, leaf: f32) {
    const RING: [(f32, f32); 5] = [
        (0.0, -1.0),
        (0.95, -0.31),
        (0.59, 0.81),
        (-0.59, 0.81),
        (-0.95, -0.31),
    ];
    for (dx, dy) in RING {
        local.push(
            list,
            KIND_ELLIPSE,
            u + dx * spread,
            v + dy * spread,
            leaf,
            leaf,
            0.0,
            0.0,
            LEAF_DARK,
        );
    }
    for (dx, dy) in RING {
        local.push(
            list,
            KIND_ELLIPSE,
            u + dx * spread * 0.55,
            v + dy * spread * 0.55,
            leaf * 0.85,
            leaf * 0.85,
            0.0,
            0.0,
            LEAF,
        );
    }
    local.push(
        list,
        KIND_ELLIPSE,
        u - spread * 0.15,
        v - spread * 0.2,
        leaf * 0.6,
        leaf * 0.6,
        0.0,
        0.0,
        LEAF_LIGHT,
    );
}

/// A small plant: a pot half the tile across, its rim and the soil in it,
/// and a modest head of leaves over it.
fn small_plant(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let pot = w * 0.46;
    local.push(list, KIND_ELLIPSE, 0.0, 0.0, pot, pot, 0.0, 0.0, POT_RIM);
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        pot * 0.78,
        pot * 0.78,
        0.0,
        0.0,
        SOIL,
    );
    foliage(list, &local, 0.0, 0.0, w * 0.13, h * 0.18);
}

/// A big plant: a tub most of the tile across, and a head of leaves that
/// reaches its rim.
fn big_plant(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let tub = w * 0.82;
    local.push(list, KIND_ELLIPSE, 0.0, 0.0, tub, tub, 0.0, 0.0, POT);
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        tub * 0.86,
        tub * 0.86,
        0.0,
        0.0,
        POT_RIM,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        tub * 0.72,
        tub * 0.72,
        0.0,
        0.0,
        SOIL,
    );
    foliage(list, &local, 0.0, 0.0, w * 0.22, h * 0.28);
}

/// A picture: a framed canvas flush against the wall its rotation names —
/// the top edge of its tile unturned, like the wall light's bracket — the
/// frame's brass round a sky over a hill. Seen from above it is a strip
/// along the wall, which is what a picture on a wall is from above.
fn picture(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let depth = 9.0;
    let v = -h * 0.5 + depth * 0.5 + 1.0;
    local.push(
        list,
        KIND_RECT,
        0.0,
        v,
        w * 0.72,
        depth,
        1.0,
        0.0,
        FRAME_BRASS,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        v,
        w * 0.62,
        depth - 4.0,
        0.0,
        0.0,
        CANVAS_SKY,
    );
    local.push(
        list,
        KIND_RECT,
        0.0,
        v + (depth - 4.0) * 0.25,
        w * 0.62,
        (depth - 4.0) * 0.5,
        0.0,
        0.0,
        CANVAS_HILL,
    );
}

// --- sandbags ----------------------------------------------------------------------

/// Hessian, and its shadow between the bags.
const SACK: Color = Color::rgb(0.66, 0.60, 0.42);
const SACK_DARK: Color = Color::rgb(0.46, 0.41, 0.28);

/// A tile of sandbags: three courses of rounded bags, each course
/// staggered half a bag on the one below, over a dark ground so the
/// seams read, and a rope tie across the top course. The engineer's
/// laid sandbags are drawn with it too (`world_paint::deployables`), as
/// a part stood on the tile.
pub(crate) fn sandbags(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across - 8.0, along - 8.0);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 4.0, 0.0, SACK_DARK);
    let bag_h = h / 3.0;
    for course in 0..3 {
        let v = -h / 2.0 + bag_h * (course as f32 + 0.5);
        let bags = if course % 2 == 0 { 3 } else { 2 };
        let bag_w = w / bags as f32;
        for i in 0..bags {
            let u = -w / 2.0 + bag_w * (i as f32 + 0.5);
            local.push(
                list,
                KIND_RECT,
                u,
                v,
                bag_w - 2.0,
                bag_h - 2.0,
                (bag_h - 2.0) / 2.0,
                0.0,
                SACK,
            );
        }
    }
    local.push(
        list,
        KIND_RECT,
        0.0,
        -h / 2.0 + bag_h * 0.5,
        w - 6.0,
        1.5,
        0.0,
        0.0,
        SACK_DARK,
    );
}

// --- the engineer's sentries (feature 74, task 127) -----------------------------

/// The Healing Sentry's grey, and the dark of its feet and base.
const SENTRY: Color = Color::rgb(0.40, 0.44, 0.50);
const SENTRY_DARK: Color = Color::rgb(0.24, 0.26, 0.30);
/// The Healing Sentry's cross (task 127): the medic's beam's green.
const HEALING_CROSS: Color = Color::rgb(0.55, 0.95, 0.75);

/// The Healing Sentry on its tile: a squat base on three feet, the drum
/// on it and a green cross where a gun's eye would be — it has no
/// barrel. `health` is nought to one, for the dark ring that grows as it
/// is shot up.
pub(crate) fn healing_sentry(list: &mut DrawList, part: &PlacedPart, health: f32) {
    let (local, across, along) = Local::of(part);
    let side = across.min(along);
    // The feet, three round pads.
    for (u, v) in [(-0.30, 0.26), (0.30, 0.26), (0.0, -0.34)] {
        local.push(
            list,
            KIND_ELLIPSE,
            u * side,
            v * side,
            side * 0.22,
            side * 0.22,
            0.0,
            0.0,
            SENTRY_DARK,
        );
    }
    // The base, and the drum on it.
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        side * 0.62,
        side * 0.62,
        0.0,
        0.0,
        SENTRY_DARK,
    );
    local.push(
        list,
        KIND_ELLIPSE,
        0.0,
        0.0,
        side * 0.46,
        side * 0.46,
        0.0,
        0.0,
        SENTRY,
    );
    // What it has taken: a dark ring closing over the drum.
    let hurt = (1.0 - health.clamp(0.0, 1.0)) * side * 0.46;
    if hurt > 0.5 {
        local.push(
            list,
            KIND_ELLIPSE,
            0.0,
            0.0,
            hurt,
            hurt,
            0.0,
            0.0,
            SENTRY_DARK,
        );
    }
    for (w, h) in [(0.10, 0.34), (0.34, 0.10)] {
        local.push(
            list,
            KIND_RECT,
            0.0,
            0.0,
            side * w,
            side * h,
            0.0,
            0.0,
            HEALING_CROSS,
        );
    }
}

/// The gun turret's colours: its armour, the dark of its joints and
/// barrels, the plate's lit edge, the hazard stripes round its base, the
/// eye it aims with and the flash at its muzzles. The eye and the flash
/// are past white, so the bloom picks them up.
const TURRET_ARMOUR: Color = Color::rgb(0.36, 0.40, 0.46);
const TURRET_PLATE: Color = Color::rgb(0.52, 0.57, 0.64);
const TURRET_DARK: Color = Color::rgb(0.13, 0.14, 0.17);
const TURRET_BARREL: Color = Color::rgb(0.30, 0.33, 0.38);
const TURRET_SHINE: Color = Color::rgb(0.62, 0.68, 0.76);
const TURRET_SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.35);
const TURRET_HAZARD: Color = Color::rgb(0.95, 0.70, 0.12);
const TURRET_EYE: Color = Color::rgb(0.55, 1.35, 1.9);
const TURRET_LASER: Color = Color::rgba(0.40, 0.85, 1.0, 0.22);
const TURRET_FLASH: Color = Color::rgb(2.4, 1.9, 1.1);
/// Its health as a ring of segments round the base: how many, and the
/// colours of a full and an empty one.
const TURRET_HEALTH_PIECES: u32 = 12;
const TURRET_HEALTH_FULL: Color = Color::rgb(0.30, 0.85, 1.0);
const TURRET_HEALTH_LOW: Color = Color::rgb(1.0, 0.30, 0.22);
const TURRET_HEALTH_GONE: Color = Color::rgba(0.10, 0.11, 0.13, 0.8);
/// How far its aiming laser runs past the muzzles, in its own sides.
const TURRET_LASER_REACH: f32 = 1.6;

/// The engineer's ultimate on its tile (task 127): a tripod with a
/// hazard-striped base plate, its health a ring of lit segments round
/// the plate, and on it a **head that turns** — `facing` radians in the
/// room's frame, nought east — an armoured drum carrying twin minigun
/// barrels, a glowing eye, a faint aiming laser, and a flash at the
/// muzzles while `flash` is above nought. `scale` is how much bigger
/// than a tile it is drawn.
pub(crate) fn turret(
    list: &mut DrawList,
    part: &PlacedPart,
    health: f32,
    scale: f32,
    facing: f32,
    flash: f32,
) {
    let (local, across, along) = Local::of(part);
    let side = across.min(along) * scale;
    let (cx, cy) = local.at(0.0, 0.0);
    let health = health.clamp(0.0, 1.0);
    let tau = core::f32::consts::TAU;
    // The ground under it.
    list.push(
        KIND_ELLIPSE,
        cx + side * 0.04,
        cy + side * 0.06,
        side * 0.92,
        side * 0.92,
        0.0,
        0.0,
        0.0,
        TURRET_SHADOW,
    );
    // The tripod: three legs splayed from the plate to their pads, fixed
    // to the deck whichever way the head turns.
    for k in 0..3 {
        let a = -core::f32::consts::FRAC_PI_2 + k as f32 * tau / 3.0;
        let (c, s) = (a.cos(), a.sin());
        let (fx, fy) = (cx + c * side * 0.44, cy + s * side * 0.44);
        list.line(cx, cy, fx, fy, side * 0.09, TURRET_DARK);
        list.push(
            KIND_RECT,
            fx,
            fy,
            side * 0.16,
            side * 0.11,
            a,
            side * 0.03,
            0.0,
            TURRET_ARMOUR,
        );
    }
    // The base plate, its hazard ring, and the health round it.
    list.ellipse(cx, cy, side * 0.64, side * 0.64, TURRET_DARK);
    for k in 0..8 {
        let a = k as f32 * tau / 8.0 + tau / 16.0;
        let r = side * 0.27;
        list.push(
            KIND_RECT,
            cx + a.cos() * r,
            cy + a.sin() * r,
            side * 0.07,
            side * 0.035,
            a + core::f32::consts::FRAC_PI_2,
            0.0,
            0.0,
            TURRET_HAZARD,
        );
    }
    let lit = (health * TURRET_HEALTH_PIECES as f32).ceil() as u32;
    let tint = mix(TURRET_HEALTH_LOW, TURRET_HEALTH_FULL, health);
    let step = tau / TURRET_HEALTH_PIECES as f32;
    let r = side * 0.36;
    for k in 0..TURRET_HEALTH_PIECES {
        let a = -core::f32::consts::FRAC_PI_2 + (k as f32 + 0.5) * step;
        list.push(
            KIND_RECT,
            cx + a.cos() * r,
            cy + a.sin() * r,
            side * 0.13,
            side * 0.05,
            a + core::f32::consts::FRAC_PI_2,
            side * 0.02,
            0.0,
            if k < lit { tint } else { TURRET_HEALTH_GONE },
        );
    }
    // The head, in its own frame: `u` along the barrels, `v` across.
    let (c, s) = (facing.cos(), facing.sin());
    let at = |u: f32, v: f32| (cx + u * c - v * s, cy + u * s + v * c);
    let part_of = |list: &mut DrawList, u: f32, v: f32, w: f32, h: f32, round: f32, colour| {
        let (x, y) = at(u * side, v * side);
        list.push(
            KIND_RECT,
            x,
            y,
            w * side,
            h * side,
            facing,
            round * side,
            0.0,
            colour,
        );
    };
    // The aiming laser, under everything of the head.
    let (lx, ly) = at(side * 0.70, 0.0);
    let (tx, ty) = at(side * (0.70 + TURRET_LASER_REACH), 0.0);
    list.line(lx, ly, tx, ty, side * 0.02, TURRET_LASER);
    // The barrels: two, side by side, with a shroud round their roots
    // and a brake on each muzzle.
    for v in [-0.075, 0.075] {
        part_of(list, 0.38, v, 0.58, 0.085, 0.01, TURRET_DARK);
        part_of(list, 0.38, v, 0.56, 0.05, 0.0, TURRET_BARREL);
        part_of(list, 0.38, v - 0.012, 0.52, 0.012, 0.0, TURRET_SHINE);
        part_of(list, 0.66, v, 0.08, 0.10, 0.015, TURRET_DARK);
        part_of(list, 0.66, v, 0.06, 0.07, 0.01, TURRET_PLATE);
    }
    part_of(list, 0.22, 0.0, 0.18, 0.28, 0.03, TURRET_ARMOUR);
    // The ammunition drum to one side, the counterweight behind.
    part_of(list, -0.02, 0.24, 0.26, 0.14, 0.04, TURRET_DARK);
    part_of(list, -0.28, 0.0, 0.14, 0.34, 0.05, TURRET_DARK);
    // The armoured head: a plate with a lighter face and two cheeks.
    part_of(list, 0.0, 0.0, 0.50, 0.44, 0.12, TURRET_DARK);
    part_of(list, 0.0, 0.0, 0.46, 0.40, 0.10, TURRET_ARMOUR);
    part_of(list, 0.04, 0.0, 0.30, 0.26, 0.07, TURRET_PLATE);
    part_of(list, 0.02, -0.21, 0.30, 0.06, 0.02, TURRET_DARK);
    part_of(list, 0.02, 0.21, 0.30, 0.06, 0.02, TURRET_DARK);
    // The eye it aims with.
    let (ex, ey) = at(side * 0.15, 0.0);
    list.ellipse(ex, ey, side * 0.12, side * 0.12, TURRET_DARK);
    list.ellipse(ex, ey, side * 0.07, side * 0.07, TURRET_EYE);
    // The flash at both muzzles, fading as it runs out.
    if flash > 0.0 {
        let bright = (flash / bims::combat::SENTRY_FLASH).clamp(0.0, 1.0);
        for v in [-0.075, 0.075] {
            let (mx, my) = at(side * 0.78, v * side);
            list.push(
                KIND_ELLIPSE,
                mx,
                my,
                side * 0.26 * bright,
                side * 0.12 * bright,
                facing,
                0.0,
                0.0,
                TURRET_FLASH,
            );
        }
    }
}

/// Between two colours: `a` at nought, `b` at one.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgba(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

// --- the ground's walls ---------------------------------------------------------------

/// A wall on a planet is what the ground gives: dressed stone in a
/// temperate town, adobe in a desert one, timber in an arctic one — each
/// a [`Surface`] texture anchored to the tile, in its average colour.
const STONE: Color = Color::rgb(0.50, 0.47, 0.42);
const ADOBE: Color = Color::rgb(0.74, 0.58, 0.40);
const TIMBER: Color = Color::rgb(0.44, 0.30, 0.19);

/// One tile of a town's wall, by the biome: stone courses, plaster over
/// mud brick, or timber planks. The bulkhead ([`wall`]) is a ship's; this
/// is what [`part_in`] draws for a `Wall` with a biome.
fn stone_wall(list: &mut DrawList, tile: (u32, u32), biome: Biome) {
    let (cx, cy) = middle(tile.0, tile.1);
    let (surface, colour) = match biome {
        Biome::Temperate => (Surface::Stone, STONE),
        Biome::Desert => (Surface::Adobe, ADOBE),
        Biome::Arctic => (Surface::Timber, TIMBER),
    };
    list.surface(surface, cx, cy, T + LAP, T + LAP, 0.0, (cx, cy), colour);
}

// --- the wild ------------------------------------------------------------------------

/// What a tree, a shrub or a rock throws on the ground to its south-east.
const SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 0.22);
/// The palm's trunk and its fronds; the cactus and the light on it; the
/// fir's two greens and the snow on its top tier; the tussock's dry grass.
const TRUNK: Color = Color::rgb(0.46, 0.32, 0.20);
const FROND: Color = Color::rgb(0.34, 0.56, 0.30);
const FROND_LIGHT: Color = Color::rgb(0.52, 0.72, 0.38);
const CACTUS: Color = Color::rgb(0.30, 0.54, 0.34);
const CACTUS_LIT: Color = Color::rgb(0.42, 0.66, 0.42);
const FIR: Color = Color::rgb(0.13, 0.32, 0.21);
const FIR_LIT: Color = Color::rgb(0.19, 0.42, 0.27);
const SNOW: Color = Color::rgb(0.92, 0.95, 0.98);
const TUSSOCK: Color = Color::rgb(0.68, 0.62, 0.42);
const TUSSOCK_DARK: Color = Color::rgb(0.48, 0.42, 0.26);
/// Rock by biome — a temperate grey, a desert's sandstone, an arctic
/// blue-grey — with the seam round each lump and the light on its edge,
/// as `world_paint::list_rock` draws an asteroid's.
const ROCK_TEMPERATE: Color = Color::rgb(0.52, 0.50, 0.46);
const ROCK_DESERT: Color = Color::rgb(0.64, 0.44, 0.30);
const ROCK_ARCTIC: Color = Color::rgb(0.56, 0.60, 0.64);
const ROCK_EDGE: Color = Color::rgba(0.0, 0.0, 0.0, 0.35);
const ROCK_GLINT: Color = Color::rgba(1.0, 1.0, 1.0, 0.12);
/// Water by biome — a lake, an oasis pool, and ice — the average colour
/// of the [`Surface::Water`] or [`Surface::Ice`] it is drawn with.
pub(crate) const WATER_TEMPERATE: Color = Color::rgb(0.24, 0.44, 0.64);
pub(crate) const WATER_DESERT: Color = Color::rgb(0.22, 0.54, 0.62);
pub(crate) const ICE: Color = Color::rgb(0.70, 0.82, 0.90);
/// A field from afar: the soil, its edge, and the furrows along it.
const FIELD_SOIL: Color = Color::rgb(0.42, 0.30, 0.18);
const FIELD_EDGE: Color = Color::rgba(0.20, 0.13, 0.07, 0.6);
const FURROW: Color = Color::rgba(0.16, 0.10, 0.05, 0.5);

/// A number nought to one that is the part's own — off its origin, so
/// two trees in a row are not the same tree and the picture holds still
/// from frame to frame. A hash rather than the RNG, for the flame's
/// reason (`hull::flicker`).
fn salt(part: &PlacedPart) -> f32 {
    let mut h = part.origin.0.wrapping_mul(0x9E37_79B9) ^ part.origin.1.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// The angle a part's frame is turned by, read back off [`Local`] for
/// the one shape that has to be turned *within* it: the fir's tiers.
fn turn_of(local: &Local) -> f32 {
    let (ox, oy) = local.at(0.0, 0.0);
    let (rx, ry) = local.at(1.0, 0.0);
    (ry - oy).atan2(rx - ox)
}

/// A tier of a fir: an isosceles triangle `base` wide pointing up the
/// part (`-v`), its base at `v`, its apex half the base above it. The
/// format's triangle is a right-angled half box; turned three-eighths
/// round its right angle is the apex, at the top.
fn tier(list: &mut DrawList, local: &Local, u: f32, v: f32, base: f32, color: Color) {
    let side = base / core::f32::consts::SQRT_2;
    let (x, y) = local.at(u, v);
    let up = 3.0 * core::f32::consts::FRAC_PI_4;
    list.triangle(x, y, side, side, turn_of(local) + up, color);
}

/// A tree, as the biome grows it: a round canopy — two overlapping
/// ellipses in the two greens, the lighter one off-centre to the north —
/// over its shadow; a palm, a trunk disc with six fronds radiating; or a
/// fir, three tiers stacked up the tile with snow on the top one. Three
/// shapes for the canopy, since a forest ring is a thousand of them.
fn tree(list: &mut DrawList, part: &PlacedPart, biome: Biome) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let size = 0.86 + salt(part) * 0.14;
    // A canopy throws a shadow; a fir's tiers are their own shading.
    if biome != Biome::Arctic {
        local.push(
            list,
            KIND_ELLIPSE,
            w * 0.08,
            h * 0.1,
            w * 0.78 * size,
            h * 0.72 * size,
            0.0,
            0.0,
            SHADE,
        );
    }
    match biome {
        Biome::Temperate => {
            local.push(
                list,
                KIND_ELLIPSE,
                -w * 0.06,
                h * 0.04,
                w * 0.74 * size,
                h * 0.70 * size,
                0.0,
                0.0,
                LEAF_DARK,
            );
            local.push(
                list,
                KIND_ELLIPSE,
                w * 0.08,
                -h * 0.1,
                w * 0.58 * size,
                h * 0.56 * size,
                0.0,
                0.0,
                LEAF,
            );
        }
        Biome::Desert => {
            let reach = w * 0.42 * size;
            for i in 0..6 {
                let a = i as f32 * core::f32::consts::FRAC_PI_3 + salt(part) * 0.5;
                let (x0, y0) = local.at(0.0, 0.0);
                let (x1, y1) = local.at(a.cos() * reach, a.sin() * reach);
                let color = if i % 2 == 0 { FROND } else { FROND_LIGHT };
                list.line(x0, y0, x1, y1, 3.0, color);
            }
            local.push(
                list,
                KIND_ELLIPSE,
                0.0,
                0.0,
                w * 0.2,
                w * 0.2,
                0.0,
                0.0,
                TRUNK,
            );
        }
        Biome::Arctic => {
            let base = w * 0.8 * size;
            tier(list, &local, 0.0, h * 0.42, base, FIR);
            tier(list, &local, 0.0, h * 0.16, base * 0.78, FIR_LIT);
            tier(list, &local, 0.0, -h * 0.08, base * 0.56, FIR);
            tier(
                list,
                &local,
                0.0,
                -h * 0.08 - base * 0.14,
                base * 0.28,
                SNOW,
            );
        }
    }
}

/// A shrub: a small bush of two leaves and a light one; a cactus, a
/// rounded body with an arm out each side; or a tussock, dry grass
/// fanned out of one root.
fn shrub(list: &mut DrawList, part: &PlacedPart, biome: Biome) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let lean = (salt(part) - 0.5) * w * 0.2;
    match biome {
        Biome::Temperate => {
            local.push(
                list,
                KIND_ELLIPSE,
                lean + w * 0.06,
                h * 0.08,
                w * 0.5,
                h * 0.42,
                0.0,
                0.0,
                SHADE,
            );
            local.push(
                list,
                KIND_ELLIPSE,
                lean - w * 0.04,
                0.0,
                w * 0.46,
                h * 0.40,
                0.0,
                0.0,
                LEAF_DARK,
            );
            local.push(
                list,
                KIND_ELLIPSE,
                lean + w * 0.06,
                -h * 0.06,
                w * 0.32,
                h * 0.28,
                0.0,
                0.0,
                LEAF,
            );
            local.push(
                list,
                KIND_ELLIPSE,
                lean - w * 0.02,
                -h * 0.1,
                w * 0.14,
                h * 0.12,
                0.0,
                0.0,
                LEAF_LIGHT,
            );
        }
        Biome::Desert => {
            local.push(
                list,
                KIND_ELLIPSE,
                lean + w * 0.08,
                h * 0.1,
                w * 0.44,
                h * 0.3,
                0.0,
                0.0,
                SHADE,
            );
            // The body, and an arm out each side that turns up.
            local.push(
                list,
                KIND_RECT,
                lean,
                0.0,
                w * 0.22,
                h * 0.64,
                w * 0.11,
                0.0,
                CACTUS,
            );
            local.push(
                list,
                KIND_RECT,
                lean - w * 0.2,
                h * 0.02,
                w * 0.24,
                w * 0.12,
                w * 0.06,
                0.0,
                CACTUS,
            );
            local.push(
                list,
                KIND_RECT,
                lean - w * 0.28,
                -h * 0.1,
                w * 0.12,
                h * 0.28,
                w * 0.06,
                0.0,
                CACTUS,
            );
            local.push(
                list,
                KIND_RECT,
                lean + w * 0.2,
                h * 0.12,
                w * 0.24,
                w * 0.12,
                w * 0.06,
                0.0,
                CACTUS,
            );
            local.push(
                list,
                KIND_RECT,
                lean + w * 0.28,
                0.0,
                w * 0.12,
                h * 0.28,
                w * 0.06,
                0.0,
                CACTUS,
            );
            local.push(
                list,
                KIND_RECT,
                lean - w * 0.03,
                -h * 0.04,
                w * 0.06,
                h * 0.5,
                w * 0.03,
                0.0,
                CACTUS_LIT,
            );
        }
        Biome::Arctic => {
            let (x0, y0) = local.at(lean, h * 0.16);
            const BLADES: [(f32, f32); 5] = [
                (-0.30, -0.22),
                (-0.14, -0.34),
                (0.02, -0.38),
                (0.18, -0.32),
                (0.30, -0.18),
            ];
            for (i, (du, dv)) in BLADES.into_iter().enumerate() {
                let (x1, y1) = local.at(lean + du * w, h * 0.16 + dv * h);
                let color = if i % 2 == 0 { TUSSOCK } else { TUSSOCK_DARK };
                list.line(x0, y0, x1, y1, 2.0, color);
            }
            local.push(
                list,
                KIND_ELLIPSE,
                lean,
                h * 0.16,
                w * 0.2,
                h * 0.1,
                0.0,
                0.0,
                TUSSOCK_DARK,
            );
        }
    }
}

/// The rock a biome's ground is made of.
fn rock_color(biome: Biome) -> Color {
    match biome {
        Biome::Temperate => ROCK_TEMPERATE,
        Biome::Desert => ROCK_DESERT,
        Biome::Arctic => ROCK_ARCTIC,
    }
}

/// A boulder: the seam is the tile, the rock is inset into it with the
/// light along its top-left edge, and a second, smaller lump lies against
/// it to the south-east.
fn boulder(list: &mut DrawList, part: &PlacedPart, biome: Biome) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let color = rock_color(biome);
    let lean = (salt(part) - 0.5) * w * 0.16;
    // The second lump first, under the rock: what pokes out past the
    // rock's seam is the lump, and it costs one shape.
    local.push(
        list,
        KIND_RECT,
        lean + w * 0.26,
        h * 0.26,
        w * 0.36,
        h * 0.32,
        w * 0.1,
        0.0,
        color,
    );
    local.push(
        list,
        KIND_RECT,
        lean - w * 0.06,
        -h * 0.06,
        w * 0.78,
        h * 0.72,
        w * 0.2,
        0.0,
        ROCK_EDGE,
    );
    local.push(
        list,
        KIND_RECT,
        lean - w * 0.06,
        -h * 0.06,
        w * 0.72,
        h * 0.66,
        w * 0.18,
        0.0,
        color,
    );
    local.push(
        list,
        KIND_RECT,
        lean - w * 0.18,
        -h * 0.18,
        w * 0.32,
        h * 0.28,
        w * 0.1,
        0.0,
        ROCK_GLINT,
    );
}

/// A tile of water: the whole tile in the biome's water, so a lake of
/// them reads as one sheet, and on some of them two short wavelets — or,
/// frozen, a crack across the ice.
fn water(list: &mut DrawList, part: &PlacedPart, biome: Biome) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    let (surface, color) = water_look(biome);
    // Tied to the world by the app, so a lake of many tiles is one sheet.
    let (x, y) = local.at(0.0, 0.0);
    let turn = turn_of(&local);
    list.surface(surface, x, y, w + LAP, h + LAP, turn, (x, y), color);
}

/// The surface and colour water is drawn in, in `biome`: a lake, an
/// oasis pool, or ice. The plain's water is the same.
pub(crate) fn water_look(biome: Biome) -> (Surface, Color) {
    match biome {
        Biome::Temperate => (Surface::Water, WATER_TEMPERATE),
        Biome::Desert => (Surface::Water, WATER_DESERT),
        Biome::Arctic => (Surface::Ice, ICE),
    }
}

/// A field from afar: the soil the strip's whole size with a darker
/// edge, and three furrows along it. The room draws a live field over
/// this while it is open (`bims::aboard::drawn_by_room`); this is the far
/// view of a town whose room is not.
fn field(list: &mut DrawList, part: &PlacedPart) {
    let (local, across, along) = Local::of(part);
    let (w, h) = (across, along);
    local.push(list, KIND_RECT, 0.0, 0.0, w, h, 0.0, 0.0, FIELD_SOIL);
    local.push(
        list,
        KIND_RECT,
        0.0,
        0.0,
        w - 2.0,
        h - 2.0,
        0.0,
        1.5,
        FIELD_EDGE,
    );
    for v in [-h * 0.28, 0.0, h * 0.28] {
        local.push(list, KIND_RECT, 0.0, v, w - 8.0, 1.5, 0.0, 0.0, FURROW);
    }
}

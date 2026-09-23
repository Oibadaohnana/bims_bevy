//! A picture for every thing that can sit in a grid cell.
//!
//! The grids — the pack, the armoury, the shelves, the cold store — show
//! an icon a cell rather than a word, and this is where the icons are:
//! one per `ResourceId`, drawn with egui's own shapes into whatever rect
//! the cell has, small and flat with one accent colour each, so a row of
//! them reads at a glance. The lists show the same picture beside the
//! word — the trade window's rows and the Inventory tab's, through
//! [`resource_cell`] — so a thing looks the same wherever it is met. A
//! piece of armour is its resource's icon with a
//! crack across it when it is broken; the weapon in hand is its resource's.
//!
//! Nothing here is text: an icon that needed a glyph would need the font
//! to have it, and the default font has few — see the root notes.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, pos2, vec2};
use bims::combat::{ArmourKind, Item};
use physics::ResourceId;

use crate::theme;

// The accents. One a thing, and the armour's are the colours the deck
// draws the pieces in (`crates/game/src/character.rs`), so the icon and
// the cap on the Bim's head agree.
const VEG: Color32 = Color32::from_rgb(0x7c, 0xc4, 0x5a);
const TOFU: Color32 = Color32::from_rgb(0xf0, 0xe8, 0xd0);
const SUIT: Color32 = Color32::from_rgb(0xe4, 0xe4, 0xdc);
const GUN: Color32 = Color32::from_rgb(0x50, 0x58, 0x64);
const GUN_LIGHT: Color32 = Color32::from_rgb(0x66, 0xb8, 0xff);
const MEDKIT: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf2);
const CROSS: Color32 = Color32::from_rgb(0xe0, 0x40, 0x40);
const BANDAGE: Color32 = Color32::from_rgb(0xe8, 0xdc, 0xc8);
const HELM: Color32 = Color32::from_rgb(0x8c, 0x9e, 0xb8);
const KEVLAR: Color32 = Color32::from_rgb(0x38, 0x3d, 0x47);
const KEVLAR_YOKE: Color32 = Color32::from_rgb(0x5c, 0x64, 0x72);
const LEGS: Color32 = Color32::from_rgb(0x3c, 0x34, 0x2c);
const LEGS_BAND: Color32 = Color32::from_rgb(0x8c, 0x9e, 0xb8);
// The long guns' wood, the sniper's scope and the glass in it, and the
// schword's hilt and blade, in the colours the deck draws them
// (`character::draw_gun`).
const STOCK: Color32 = Color32::from_rgb(0x73, 0x4d, 0x29);
const SCOPE: Color32 = Color32::from_rgb(0x4d, 0x57, 0x66);
const LENS: Color32 = Color32::from_rgb(0x8c, 0xc7, 0xeb);
/// A shade either side of [`GUN`], so the pieces of a gun hold apart at
/// the size of a cell: the pale steel of a muzzle brake and a bipod, and
/// the dark of a magazine, a sight rail and the vents in a handguard.
const GUN_STEEL: Color32 = Color32::from_rgb(0x78, 0x84, 0x93);
const GUN_DARK: Color32 = Color32::from_rgb(0x2e, 0x34, 0x3d);
const HILT: Color32 = Color32::from_rgb(0x38, 0x38, 0x42);
const BLADE_CORE: Color32 = Color32::from_rgb(0xfa, 0xff, 0xff);
const BLADE_EDGE: Color32 = Color32::from_rgb(0x73, 0xf2, 0xff);
const BLADE_GLOW: Color32 = Color32::from_rgba_premultiplied(0x1c, 0x3d, 0x40, 0x40);
/// The research key: a slab of somebody else's circuitry, dark, with a
/// brass edge and a lit trace down it.
const KEY: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x38);
const KEY_EDGE: Color32 = Color32::from_rgb(0xcc, 0xa8, 0x4c);
const KEY_TRACE: Color32 = Color32::from_rgb(0xff, 0xdb, 0x66);
// The engineer's kits: the sack's hessian and a sentry crate's grey
// with its barrel.
const SACK: Color32 = Color32::from_rgb(0xb8, 0xa2, 0x70);
const SACK_DARK: Color32 = Color32::from_rgb(0x7e, 0x6c, 0x48);
const CRATE: Color32 = Color32::from_rgb(0x6a, 0x72, 0x7e);
// The soldier's grenade: a dark shell with a band, and the fuse's cap.
const SHELL: Color32 = Color32::from_rgb(0x2e, 0x33, 0x28);
const SHELL_BAND: Color32 = Color32::from_rgb(0x6b, 0x73, 0x4c);
const FUSE_CAP: Color32 = Color32::from_rgb(0xd8, 0xb0, 0x40);
/// The galley's own things, which are no resource: a bowl of stew and a
/// plate, for the Inventory tab's last rows.
const BOWL: Color32 = Color32::from_rgb(0x9c, 0x6a, 0x48);
const STEW: Color32 = Color32::from_rgb(0xc8, 0x78, 0x3c);
const PLATE: Color32 = Color32::from_rgb(0xe8, 0xec, 0xf0);
/// The crack across a broken piece: the same light stroke the deck draws.
const CRACK: Color32 = Color32::from_rgb(0xe8, 0xf0, 0xf4);
/// A darker edge on a light shape, so it does not vanish on a pale cell.
const SHADE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 90);
/// A highlight on a dark one.
const SHINE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 70);

/// The icon for a thing in a cell, whatever it is.
pub fn icon(painter: &egui::Painter, rect: Rect, item: Item) {
    match item {
        Item::Armour(piece) => armour(painter, rect, piece.kind, piece.broken()),
        Item::Weapon(weapon) => match weapon
            .kind
            .resource()
            .and_then(|c| ResourceId::ALL.get(c as usize))
        {
            Some(&id) => resource(painter, rect, id),
            None => unknown(painter, rect),
        },
        Item::Stack(code) => match ResourceId::ALL.get(code as usize) {
            Some(&id) => resource(painter, rect, id),
            None => unknown(painter, rect),
        },
        // A key is its tier's resource, tall: the cell it is given is two
        // cells high, and the picture is drawn to fill it.
        Item::Key(tier) => match world::armour::key_resource(tier) {
            Some(id) => resource(painter, rect, id),
            None => unknown(painter, rect),
        },
    }
}

/// A piece of armour: its resource's icon, cracked if it is broken.
pub fn armour(painter: &egui::Painter, rect: Rect, kind: ArmourKind, broken: bool) {
    let id = ResourceId::ALL
        .get(kind.resource() as usize)
        .copied()
        .unwrap_or(ResourceId::Helm);
    resource(painter, rect, id);
    if broken {
        let b = Box_::new(rect);
        painter.line_segment(
            [b.at(0.22, 0.78), b.at(0.78, 0.22)],
            Stroke::new(b.px(0.08), CRACK),
        );
    }
}

/// One resource's icon, into `rect`: square, in the middle of it.
pub fn resource(painter: &egui::Painter, rect: Rect, id: ResourceId) {
    let mut s = Sketch::default();
    draw_resource(&mut s, &Box_::new(rect), id);
    painter.extend(s.shapes);
}

/// One resource's icon, drawn in fractions of a box.
fn draw_resource(s: &mut Sketch, b: &Box_, id: ResourceId) {
    match id {
        ResourceId::Vegetable => {
            // A leaf on a stem.
            s.add(egui::Shape::convex_polygon(
                b.poly(&[
                    (0.50, 0.14),
                    (0.84, 0.40),
                    (0.62, 0.78),
                    (0.30, 0.70),
                    (0.18, 0.40),
                ]),
                VEG,
                Stroke::NONE,
            ));
            s.line_segment(
                [b.at(0.50, 0.30), b.at(0.44, 0.88)],
                Stroke::new(b.px(0.06), SHADE),
            );
        }
        ResourceId::Tofu => {
            // A block, its top face lit.
            s.rect_filled(b.rect(0.20, 0.30, 0.80, 0.82), b.px(0.04), TOFU);
            s.rect_filled(b.rect(0.20, 0.30, 0.80, 0.44), b.px(0.04), SHINE);
            s.rect_stroke(
                b.rect(0.20, 0.30, 0.80, 0.82),
                b.px(0.04),
                Stroke::new(b.px(0.03), SHADE),
                egui::StrokeKind::Inside,
            );
        }
        ResourceId::Suit => {
            // A helmet over a body.
            s.circle_filled(b.at(0.5, 0.30), b.px(0.18), SUIT);
            s.circle_filled(b.at(0.5, 0.30), b.px(0.10), SHADE);
            s.rect_filled(b.rect(0.28, 0.48, 0.72, 0.88), b.px(0.08), SUIT);
        }
        ResourceId::Handgun => {
            // A sidearm, and drawn small: a stubby slide with the emitter
            // at its nose and a grip under it, well inside the cell. On
            // the deck the pistol is a third of the rifle, and it is a
            // third of it here.
            s.rect_filled(b.rect(0.28, 0.34, 0.78, 0.50), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.28, 0.48, 0.46, 0.76), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.33, 0.37, 0.70, 0.41), b.px(0.01), SCOPE);
            s.circle_filled(b.at(0.79, 0.42), b.px(0.07), GUN_LIGHT);
        }
        ResourceId::Medkit => {
            // A case with a cross on it.
            s.rect_filled(b.rect(0.14, 0.26, 0.86, 0.82), b.px(0.06), MEDKIT);
            s.rect_filled(b.rect(0.42, 0.36, 0.58, 0.72), b.px(0.02), CROSS);
            s.rect_filled(b.rect(0.24, 0.46, 0.76, 0.62), b.px(0.02), CROSS);
        }
        ResourceId::Bandage => {
            // A box of dressings since feature 87: two rolls, the one
            // behind a little higher, each seen end on beside its tail —
            // a cell of them is five, and one roll read as one bandage.
            for (dy, ex) in [(-0.16, 0.76), (0.10, 0.84)] {
                s.rect_filled(b.rect(0.16, 0.42 + dy, ex, 0.58 + dy), b.px(0.08), BANDAGE);
                s.circle_filled(b.at(0.30, 0.50 + dy), b.px(0.14), BANDAGE);
                s.circle_stroke(
                    b.at(0.30, 0.50 + dy),
                    b.px(0.07),
                    Stroke::new(b.px(0.03), SHADE),
                );
            }
        }
        ResourceId::Helm => {
            // A cap: the dome and the brim.
            s.add(egui::Shape::convex_polygon(
                b.poly(&[
                    (0.18, 0.62),
                    (0.22, 0.40),
                    (0.36, 0.24),
                    (0.64, 0.24),
                    (0.78, 0.40),
                    (0.82, 0.62),
                ]),
                HELM,
                Stroke::NONE,
            ));
            s.rect_filled(b.rect(0.12, 0.60, 0.88, 0.70), b.px(0.03), HELM);
            s.rect_filled(b.rect(0.12, 0.60, 0.88, 0.70), b.px(0.03), SHADE);
        }
        ResourceId::Kevlar => {
            vest(s, b, KEVLAR, KEVLAR_YOKE);
        }
        ResourceId::LegGuard => {
            // Two guards, a band across each shin.
            for x in [0.22, 0.56] {
                s.rect_filled(b.rect(x, 0.16, x + 0.22, 0.86), b.px(0.06), LEGS);
                s.rect_filled(b.rect(x, 0.42, x + 0.22, 0.52), b.px(0.02), LEGS_BAND);
            }
        }
        // The three long guns all face right like the handgun, the muzzle
        // lit, and are told apart by what the deck tells them apart by
        // (`character::draw_gun`): the shotgun heavy and short, a wide
        // bore over a wooden pump with the butt braced behind; the auto
        // rifle a rifle — a vented handguard along the barrel, a magazine
        // under the receiver, a sight rail on top and a brake on the end;
        // the sniper the long one, with a scope and its glass, a thin
        // barrel and a bipod under it.
        ResourceId::Shotgun => {
            s.rect_filled(b.rect(0.04, 0.44, 0.26, 0.64), b.px(0.05), STOCK);
            s.rect_filled(b.rect(0.22, 0.38, 0.40, 0.60), b.px(0.03), GUN);
            s.rect_filled(b.rect(0.34, 0.38, 0.94, 0.54), b.px(0.03), GUN);
            s.rect_filled(b.rect(0.46, 0.40, 0.70, 0.58), b.px(0.04), STOCK);
            s.rect_filled(b.rect(0.30, 0.56, 0.40, 0.78), b.px(0.03), GUN);
            s.circle_filled(b.at(0.91, 0.46), b.px(0.08), GUN_LIGHT);
        }
        ResourceId::AutoRifle => {
            s.rect_filled(b.rect(0.04, 0.42, 0.24, 0.58), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.20, 0.36, 0.42, 0.58), b.px(0.03), GUN);
            s.rect_filled(b.rect(0.40, 0.38, 0.84, 0.52), b.px(0.03), GUN);
            for x in [0.50, 0.58, 0.66] {
                s.rect_filled(b.rect(x, 0.40, x + 0.03, 0.50), 0.0, GUN_DARK);
            }
            s.rect_filled(b.rect(0.24, 0.28, 0.62, 0.36), b.px(0.02), GUN_DARK);
            s.rect_filled(b.rect(0.30, 0.56, 0.42, 0.84), b.px(0.03), GUN_DARK);
            s.rect_filled(b.rect(0.84, 0.36, 0.92, 0.54), b.px(0.02), GUN_STEEL);
            s.circle_filled(b.at(0.93, 0.45), b.px(0.06), GUN_LIGHT);
        }
        ResourceId::SniperRifle => {
            s.rect_filled(b.rect(0.02, 0.46, 0.24, 0.62), b.px(0.05), STOCK);
            s.rect_filled(b.rect(0.10, 0.41, 0.28, 0.49), b.px(0.02), STOCK);
            s.rect_filled(b.rect(0.22, 0.44, 0.40, 0.58), b.px(0.02), GUN);
            s.rect_filled(b.rect(0.34, 0.46, 0.96, 0.53), b.px(0.02), GUN);
            s.rect_filled(b.rect(0.26, 0.26, 0.62, 0.38), b.px(0.03), SCOPE);
            s.circle_filled(b.at(0.58, 0.32), b.px(0.055), LENS);
            s.rect_filled(b.rect(0.28, 0.56, 0.38, 0.80), b.px(0.03), GUN);
            for foot in [0.68f32, 0.84] {
                s.line_segment(
                    [b.at(0.76, 0.53), b.at(foot, 0.78)],
                    Stroke::new(b.px(0.04), GUN_STEEL),
                );
            }
            s.rect_filled(b.rect(0.90, 0.43, 0.96, 0.56), b.px(0.02), GUN_STEEL);
            s.circle_filled(b.at(0.96, 0.49), b.px(0.05), GUN_LIGHT);
        }
        // The schword: a hilt at the bottom left and the blade up to the
        // right, a white core between two cyan strokes — the wide faint
        // one under the bright thin one, which is as near as a flat cell
        // gets to the deck's glow.
        ResourceId::Schword => {
            let (foot, tip) = (b.at(0.32, 0.66), b.at(0.84, 0.14));
            s.line_segment([foot, tip], Stroke::new(b.px(0.22), BLADE_GLOW));
            s.line_segment([foot, tip], Stroke::new(b.px(0.11), BLADE_EDGE));
            s.line_segment([foot, tip], Stroke::new(b.px(0.05), BLADE_CORE));
            s.line_segment(
                [b.at(0.18, 0.60), b.at(0.40, 0.82)],
                Stroke::new(b.px(0.07), HILT),
            );
            s.line_segment(
                [b.at(0.30, 0.68), b.at(0.14, 0.86)],
                Stroke::new(b.px(0.12), HILT),
            );
        }
        // The research key: a tall dark slab with a brass rim, a notch cut
        // out of its top edge, and a lit trace zig-zagging down it. Drawn
        // to the cell's height, so in a two-cell slot it is a tall key
        // and in a one-cell one a short one. The tier-two key is the same
        // slab with its rim and trace in the theme's tier-two colour.
        ResourceId::ResearchKey => key(s, b, KEY_EDGE, KEY_TRACE),
        ResourceId::ResearchKeyTwo => key(s, b, theme::TIER_TWO, theme::TIER_TWO),
        // The sandbag kit: three courses of sacks, the way the part is
        // drawn on the deck.
        ResourceId::SandbagKit => {
            s.rect_filled(b.rect(0.10, 0.22, 0.90, 0.86), b.px(0.04), SACK_DARK);
            for (row, y) in [0.26, 0.46, 0.66].into_iter().enumerate() {
                let bags = if row % 2 == 0 { 3 } else { 2 };
                let w = 0.76 / bags as f32;
                for i in 0..bags {
                    let x = 0.12 + w * i as f32;
                    s.rect_filled(
                        b.rect(x + 0.01, y, x + w - 0.01, y + 0.17),
                        b.px(0.06),
                        SACK,
                    );
                }
            }
        }
        // The sentry kit: a crate with the turret's barrel and eye showing.
        ResourceId::SentryKit => {
            s.rect_filled(b.rect(0.12, 0.30, 0.88, 0.86), b.px(0.05), CRATE);
            s.rect_filled(b.rect(0.12, 0.30, 0.88, 0.40), b.px(0.03), GUN);
            s.circle_filled(b.at(0.40, 0.62), b.px(0.16), GUN);
            s.rect_filled(b.rect(0.40, 0.57, 0.86, 0.67), b.px(0.02), GUN);
            s.circle_filled(b.at(0.40, 0.62), b.px(0.06), GUN_LIGHT);
        }
        // The grenade: a round shell with a band across it and the fuse's
        // cap and lever on top.
        ResourceId::Grenade => {
            s.circle_filled(b.at(0.50, 0.58), b.px(0.30), SHELL);
            s.rect_filled(b.rect(0.22, 0.52, 0.78, 0.64), b.px(0.02), SHELL_BAND);
            s.rect_filled(b.rect(0.42, 0.18, 0.58, 0.34), b.px(0.03), FUSE_CAP);
            s.rect_filled(b.rect(0.56, 0.20, 0.80, 0.28), b.px(0.02), FUSE_CAP);
        }
    }
}

/// A research key: the slab, its rim and notch, and the lit trace down it
/// in the two colours a tier gives it.
fn key(s: &mut Sketch, b: &Box_, edge: Color32, trace: Color32) {
    s.rect_filled(b.rect(0.28, 0.08, 0.72, 0.92), b.px(0.04), edge);
    s.rect_filled(b.rect(0.33, 0.13, 0.67, 0.87), b.px(0.03), KEY);
    s.rect_filled(b.rect(0.44, 0.08, 0.56, 0.18), 0.0, KEY);
    let path = [
        b.at(0.50, 0.24),
        b.at(0.40, 0.38),
        b.at(0.60, 0.52),
        b.at(0.40, 0.66),
        b.at(0.50, 0.80),
    ];
    for pair in path.windows(2) {
        s.line_segment([pair[0], pair[1]], Stroke::new(b.px(0.05), trace));
    }
    s.circle_filled(b.at(0.50, 0.80), b.px(0.05), trace);
}

/// A vest: the body of it with the neck cut out, and the yoke across the
/// shoulders in the second colour.
fn vest(s: &mut Sketch, b: &Box_, body: Color32, yoke: Color32) {
    s.add(egui::Shape::convex_polygon(
        b.poly(&[
            (0.20, 0.20),
            (0.36, 0.20),
            (0.50, 0.32),
            (0.64, 0.20),
            (0.80, 0.20),
            (0.80, 0.86),
            (0.20, 0.86),
        ]),
        body,
        Stroke::NONE,
    ));
    s.rect_filled(b.rect(0.20, 0.20, 0.36, 0.40), b.px(0.02), yoke);
    s.rect_filled(b.rect(0.64, 0.20, 0.80, 0.40), b.px(0.02), yoke);
}

/// The side of an icon drawn inline in a row of text, in points: a little
/// over a line of the default type, so it sits level with the word beside
/// it rather than pushing the row taller.
pub const INLINE: f32 = 16.0;

/// A resource's icon as one cell of a row: allocated inline, `INLINE`
/// square, drawn where it lands. For the lists — a row is the icon, then
/// the word — so a resource is met with the same picture in a list as in
/// a grid.
pub fn resource_cell(ui: &mut egui::Ui, id: ResourceId) -> egui::Response {
    cell(ui, |painter, rect| resource(painter, rect, id))
}

/// Any picture as one cell of a row.
pub fn cell(ui: &mut egui::Ui, paint: impl FnOnce(&egui::Painter, Rect)) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(INLINE, INLINE), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect);
    }
    response
}

/// A bowl of stew: the bowl, seen a little from above, and the stew in it
/// with a piece of each thing it is made of.
pub fn stew(painter: &egui::Painter, rect: Rect) {
    let b = Box_::new(rect);
    painter.add(egui::Shape::convex_polygon(
        b.poly(&[(0.12, 0.46), (0.88, 0.46), (0.74, 0.84), (0.26, 0.84)]),
        BOWL,
        Stroke::new(b.px(0.04), SHADE),
    ));
    painter.add(egui::Shape::ellipse_filled(
        b.at(0.50, 0.46),
        vec2(b.px(0.38), b.px(0.13)),
        STEW,
    ));
    painter.circle_filled(b.at(0.40, 0.44), b.px(0.05), VEG);
    painter.circle_filled(b.at(0.60, 0.48), b.px(0.05), TOFU);
}

/// A plate: a disc with the rim marked.
pub fn plate(painter: &egui::Painter, rect: Rect) {
    let b = Box_::new(rect);
    painter.circle(
        b.at(0.50, 0.50),
        b.px(0.38),
        PLATE,
        Stroke::new(b.px(0.04), SHADE),
    );
    painter.circle_stroke(b.at(0.50, 0.50), b.px(0.24), Stroke::new(b.px(0.03), SHADE));
}

/// A thing the app has no picture of: a resource code from a newer rules
/// crate. A plain box, so the cell is at least seen to be full.
fn unknown(painter: &egui::Painter, rect: Rect) {
    let b = Box_::new(rect);
    painter.rect_stroke(
        b.rect(0.2, 0.2, 0.8, 0.8),
        b.px(0.05),
        Stroke::new(b.px(0.06), theme::MUTED),
        egui::StrokeKind::Inside,
    );
}

// --- a thing laid on the lockers' grid ------------------------------------------

/// A thing drawn over its footprint in the armoury: `rect` is the cells
/// it covers, as laid. A gun lies along its footprint — the long guns
/// have a drawing of their own for it, [`draw_wide`], since the square
/// one stretched seven times over is a smear — and everything else sits
/// square in the middle of its footprint the way it sits in a cell.
/// `turned` draws the thing a quarter round: the picture is made for the
/// footprint the other way up and turned with it, so a rifle stood on
/// end is a rifle standing, muzzle up.
pub fn laid(painter: &egui::Painter, rect: Rect, item: Item, turned: bool) {
    let upright = if turned {
        Rect::from_center_size(rect.center(), vec2(rect.height(), rect.width()))
    } else {
        rect
    };
    let mut s = Sketch::default();
    let id = match item {
        Item::Armour(piece) => ResourceId::ALL.get(piece.kind.resource() as usize).copied(),
        Item::Weapon(weapon) => weapon
            .kind
            .resource()
            .and_then(|c| ResourceId::ALL.get(c as usize))
            .copied(),
        Item::Stack(code) => ResourceId::ALL.get(code as usize).copied(),
        Item::Key(tier) => world::armour::key_resource(tier),
    };
    match id {
        Some(id) if upright.width() >= 1.5 * upright.height() && is_long(id) => {
            draw_wide(&mut s, &Box_::stretched(upright), id);
        }
        Some(id) => draw_resource(&mut s, &Box_::new(upright), id),
        None => {
            let b = Box_::new(upright);
            s.rect_stroke(
                b.rect(0.2, 0.2, 0.8, 0.8),
                b.px(0.05),
                Stroke::new(b.px(0.06), theme::MUTED),
                egui::StrokeKind::Inside,
            );
        }
    }
    if let Item::Armour(piece) = item
        && piece.broken()
    {
        let b = Box_::new(upright);
        s.line_segment(
            [b.at(0.22, 0.78), b.at(0.78, 0.22)],
            Stroke::new(b.px(0.08), CRACK),
        );
    }
    if turned {
        s.turn(rect.center());
    }
    painter.extend(s.shapes);
}

/// The things with a drawing made for a footprint wider than it is tall.
fn is_long(id: ResourceId) -> bool {
    matches!(
        id,
        ResourceId::Handgun
            | ResourceId::Shotgun
            | ResourceId::AutoRifle
            | ResourceId::SniperRifle
            | ResourceId::Schword
    )
}

/// A long gun lying along its footprint: the muzzle to the right, lit,
/// the way the square icons face, and the parts the deck tells them
/// apart by laid out along the length rather than crowded into a square.
/// `b` is the whole footprint, so `x` runs the length and `y` the height;
/// widths of strokes are in `px`, which is the short side.
fn draw_wide(s: &mut Sketch, b: &Box_, id: ResourceId) {
    match id {
        ResourceId::Handgun => {
            // Small even laid out: the slide takes half the footprint and
            // the rest is the air round a sidearm.
            s.rect_filled(b.rect(0.24, 0.26, 0.80, 0.48), b.px(0.06), GUN);
            s.rect_filled(b.rect(0.26, 0.44, 0.44, 0.86), b.px(0.06), GUN);
            s.circle_filled(b.at(0.82, 0.37), b.px(0.09), GUN_LIGHT);
        }
        ResourceId::Shotgun => {
            s.rect_filled(b.rect(0.02, 0.38, 0.22, 0.68), b.px(0.06), STOCK);
            s.rect_filled(b.rect(0.18, 0.32, 0.34, 0.62), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.30, 0.32, 0.96, 0.52), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.42, 0.34, 0.66, 0.58), b.px(0.05), STOCK);
            s.rect_filled(b.rect(0.26, 0.56, 0.36, 0.90), b.px(0.04), GUN);
            s.circle_filled(b.at(0.94, 0.42), b.px(0.09), GUN_LIGHT);
        }
        // The one-row guns are a cell tall, so their parts take a good
        // half of the height each or they are a hair.
        ResourceId::AutoRifle => {
            s.rect_filled(b.rect(0.01, 0.30, 0.20, 0.66), b.px(0.07), GUN);
            s.rect_filled(b.rect(0.16, 0.24, 0.38, 0.66), b.px(0.05), GUN);
            s.rect_filled(b.rect(0.36, 0.28, 0.82, 0.58), b.px(0.05), GUN);
            for x in [0.46, 0.56, 0.66] {
                s.rect_filled(b.rect(x, 0.32, x + 0.025, 0.54), 0.0, GUN_DARK);
            }
            s.rect_filled(b.rect(0.22, 0.14, 0.46, 0.28), b.px(0.03), GUN_DARK);
            s.rect_filled(b.rect(0.26, 0.64, 0.38, 0.98), b.px(0.04), GUN_DARK);
            s.rect_filled(b.rect(0.82, 0.26, 0.92, 0.60), b.px(0.03), GUN_STEEL);
            s.circle_filled(b.at(0.94, 0.43), b.px(0.10), GUN_LIGHT);
        }
        ResourceId::SniperRifle => {
            s.rect_filled(b.rect(0.00, 0.34, 0.16, 0.74), b.px(0.07), STOCK);
            s.rect_filled(b.rect(0.08, 0.26, 0.24, 0.40), b.px(0.03), STOCK);
            s.rect_filled(b.rect(0.18, 0.34, 0.34, 0.64), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.30, 0.40, 0.97, 0.56), b.px(0.03), GUN);
            s.rect_filled(b.rect(0.22, 0.12, 0.54, 0.36), b.px(0.04), SCOPE);
            s.circle_filled(b.at(0.49, 0.24), b.px(0.10), LENS);
            s.rect_filled(b.rect(0.24, 0.62, 0.34, 0.96), b.px(0.04), GUN);
            for foot in [0.68f32, 0.82] {
                s.line_segment(
                    [b.at(0.75, 0.56), b.at(foot, 0.96)],
                    Stroke::new(b.px(0.05), GUN_STEEL),
                );
            }
            s.rect_filled(b.rect(0.90, 0.34, 0.97, 0.62), b.px(0.03), GUN_STEEL);
            s.circle_filled(b.at(0.96, 0.48), b.px(0.09), GUN_LIGHT);
        }
        ResourceId::Schword => {
            let (foot, tip) = (b.at(0.26, 0.50), b.at(0.96, 0.50));
            s.line_segment([foot, tip], Stroke::new(b.px(0.62), BLADE_GLOW));
            s.line_segment([foot, tip], Stroke::new(b.px(0.32), BLADE_EDGE));
            s.line_segment([foot, tip], Stroke::new(b.px(0.14), BLADE_CORE));
            s.line_segment(
                [b.at(0.24, 0.12), b.at(0.24, 0.88)],
                Stroke::new(b.px(0.16), HILT),
            );
            s.line_segment(
                [b.at(0.04, 0.50), b.at(0.22, 0.50)],
                Stroke::new(b.px(0.30), HILT),
            );
        }
        _ => draw_resource(s, b, id),
    }
}

/// The shapes of an icon, gathered before they go to the painter, so a
/// picture can be turned a quarter round on the way — the painter takes
/// shapes and cannot turn one. The methods are the painter's, so a
/// drawing reads the same whichever it is given.
#[derive(Default)]
struct Sketch {
    shapes: Vec<egui::Shape>,
}

impl Sketch {
    fn add(&mut self, shape: egui::Shape) {
        self.shapes.push(shape);
    }

    fn rect_filled(&mut self, rect: Rect, rounding: f32, fill: Color32) {
        self.add(egui::Shape::rect_filled(rect, rounding, fill));
    }

    fn rect_stroke(&mut self, rect: Rect, rounding: f32, stroke: Stroke, kind: egui::StrokeKind) {
        self.add(egui::Shape::rect_stroke(rect, rounding, stroke, kind));
    }

    fn circle_filled(&mut self, center: Pos2, radius: f32, fill: Color32) {
        self.add(egui::Shape::circle_filled(center, radius, fill));
    }

    fn circle_stroke(&mut self, center: Pos2, radius: f32, stroke: Stroke) {
        self.add(egui::Shape::circle_stroke(center, radius, stroke));
    }

    fn line_segment(&mut self, points: [Pos2; 2], stroke: Stroke) {
        self.add(egui::Shape::line_segment(points, stroke));
    }

    /// Every shape a quarter turn clockwise about `origin`. A rect turned
    /// a quarter is still a rect, so its rounding is kept; the icons use
    /// nothing else that has a direction of its own.
    fn turn(&mut self, origin: Pos2) {
        for shape in &mut self.shapes {
            turn_shape(shape, origin);
        }
    }
}

/// A point a quarter turn clockwise about `origin`.
fn turn_point(p: Pos2, origin: Pos2) -> Pos2 {
    let d = p - origin;
    pos2(origin.x - d.y, origin.y + d.x)
}

fn turn_shape(shape: &mut egui::Shape, origin: Pos2) {
    use egui::Shape;
    match shape {
        Shape::Rect(r) => {
            r.rect = Rect::from_two_pos(
                turn_point(r.rect.min, origin),
                turn_point(r.rect.max, origin),
            );
        }
        Shape::Circle(c) => c.center = turn_point(c.center, origin),
        Shape::LineSegment { points, .. } => {
            for p in points.iter_mut() {
                *p = turn_point(*p, origin);
            }
        }
        Shape::Path(path) => {
            for p in path.points.iter_mut() {
                *p = turn_point(*p, origin);
            }
        }
        Shape::Ellipse(e) => {
            e.center = turn_point(e.center, origin);
            e.radius = vec2(e.radius.y, e.radius.x);
        }
        Shape::Vec(shapes) => {
            for shape in shapes.iter_mut() {
                turn_shape(shape, origin);
            }
        }
        _ => {}
    }
}

/// The cell's rect as a unit square, so every icon is drawn in fractions
/// and comes out the size of whatever cell it is in.
struct Box_ {
    rect: Rect,
}

impl Box_ {
    fn new(rect: Rect) -> Box_ {
        // Square, centred: an icon in a wide cell sits in the middle.
        let side = rect.width().min(rect.height());
        Box_ {
            rect: Rect::from_center_size(rect.center(), vec2(side, side)),
        }
    }

    /// The whole rect, whatever its shape: `x` in fractions of its width
    /// and `y` of its height, for a drawing made for that shape.
    fn stretched(rect: Rect) -> Box_ {
        Box_ { rect }
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.rect.min.x + self.rect.width() * x,
            self.rect.min.y + self.rect.height() * y,
        )
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }

    /// A length in fractions of the short side, so a stroke is as thick
    /// in a long box as in a square one.
    fn px(&self, fraction: f32) -> f32 {
        self.rect.width().min(self.rect.height()) * fraction
    }

    fn poly(&self, points: &[(f32, f32)]) -> Vec<Pos2> {
        points.iter().map(|&(x, y)| self.at(x, y)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::{Piece, WeaponKind};

    /// Every resource and every kind of item draws without a panic, into
    /// a painter with no window behind it: the shapes need no font, which
    /// is the point of them.
    #[test]
    fn every_resource_and_every_item_has_an_icon() {
        let ctx = egui::Context::default();
        let painter = egui::Painter::new(
            ctx.clone(),
            egui::LayerId::background(),
            Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0)),
        );
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(24.0, 24.0));
        for &id in ResourceId::ALL.iter() {
            resource(&painter, rect, id);
            icon(&painter, rect, Item::Stack(id as u32));
        }
        for &kind in ArmourKind::ALL.iter() {
            let mut piece = Piece::new(1, kind, bims::combat::Tier::One);
            icon(&painter, rect, Item::Armour(piece));
            piece.health = 0.0;
            icon(&painter, rect, Item::Armour(piece));
        }
        for &weapon in WeaponKind::ALL.iter() {
            icon(&painter, rect, Item::Weapon(weapon.basic()));
        }
        icon(&painter, rect, Item::Stack(u32::MAX));
        icon(&painter, rect, Item::Key(1));
        icon(&painter, rect, Item::Key(2));
        icon(&painter, rect, Item::Key(9));
        stew(&painter, rect);
        plate(&painter, rect);
        // And laid over a footprint, either way round: the long guns on
        // their own drawings, the rest square in the middle.
        let long = Rect::from_min_size(pos2(10.0, 10.0), vec2(240.0, 24.0));
        let tall = Rect::from_min_size(pos2(10.0, 10.0), vec2(24.0, 240.0));
        for &id in ResourceId::ALL.iter() {
            laid(&painter, long, Item::Stack(id as u32), false);
            laid(&painter, tall, Item::Stack(id as u32), true);
            laid(&painter, rect, Item::Stack(id as u32), false);
        }
        for &kind in ArmourKind::ALL.iter() {
            let mut piece = Piece::new(1, kind, bims::combat::Tier::One);
            piece.health = 0.0;
            laid(&painter, long, Item::Armour(piece), true);
        }
        laid(&painter, long, Item::Key(9), true);
    }
}

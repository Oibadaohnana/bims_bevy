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
        // The minigun (task 115), as the deck draws it: a boxy receiver
        // with a drum hung off it and three barrels side by side, clamped
        // twice and lit at the end.
        ResourceId::Minigun => {
            s.circle_filled(b.at(0.30, 0.70), b.px(0.13), STOCK);
            s.rect_filled(b.rect(0.06, 0.40, 0.22, 0.62), b.px(0.04), GUN);
            s.rect_filled(b.rect(0.18, 0.34, 0.46, 0.64), b.px(0.04), GUN);
            for y in [0.39, 0.47, 0.55] {
                s.rect_filled(b.rect(0.44, y, 0.92, y + 0.06), b.px(0.02), GUN);
            }
            for x in [0.56, 0.84] {
                s.rect_filled(b.rect(x, 0.35, x + 0.05, 0.65), b.px(0.01), GUN_STEEL);
            }
            s.rect_filled(b.rect(0.22, 0.28, 0.40, 0.33), b.px(0.01), GUN_DARK);
            s.circle_filled(b.at(0.93, 0.50), b.px(0.06), GUN_LIGHT);
        }
        // The rail lance: the longest, a stock and a slim body with two
        // rails running out either side of an open channel, coils across
        // it, and the glow between the rails' ends.
        ResourceId::RailLance => {
            s.rect_filled(b.rect(0.02, 0.44, 0.20, 0.60), b.px(0.05), STOCK);
            s.rect_filled(b.rect(0.16, 0.40, 0.38, 0.60), b.px(0.03), GUN);
            for y in [0.38, 0.56] {
                s.rect_filled(b.rect(0.34, y, 0.97, y + 0.06), b.px(0.02), GUN);
            }
            for x in [0.44, 0.56, 0.68, 0.80] {
                s.rect_filled(b.rect(x, 0.35, x + 0.03, 0.65), 0.0, GUN_STEEL);
            }
            s.rect_filled(b.rect(0.20, 0.32, 0.36, 0.37), b.px(0.01), SCOPE);
            s.circle_filled(b.at(0.95, 0.50), b.px(0.06), GUN_LIGHT);
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

/// The shapes of an icon, gathered before they go to the painter. The
/// methods are the painter's, so a drawing reads the same whichever it
/// is given.
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
    }
}

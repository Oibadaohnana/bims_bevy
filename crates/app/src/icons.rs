//! A picture for every thing that can sit in a grid cell.
//!
//! The grids — the pack, the armoury, the shelves, the cold store — show
//! an icon a cell rather than a word, and this is where the icons are:
//! one per `ResourceId`, drawn with egui's own shapes into whatever rect
//! the cell has, small and flat with one accent colour each, so a row of
//! them reads at a glance. A piece of armour is its resource's icon with a
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
const ORE: Color32 = Color32::from_rgb(0xa8, 0x7c, 0x5a);
const METAL: Color32 = Color32::from_rgb(0xb4, 0xbe, 0xc8);
const FUEL: Color32 = Color32::from_rgb(0xe8, 0xa8, 0x3a);
const COMPONENTS: Color32 = Color32::from_rgb(0x5f, 0xc8, 0xb8);
const VEG: Color32 = Color32::from_rgb(0x7c, 0xc4, 0x5a);
const TOFU: Color32 = Color32::from_rgb(0xf0, 0xe8, 0xd0);
const GALVUM: Color32 = Color32::from_rgb(0xb0, 0x7c, 0xe8);
const EMITTER: Color32 = Color32::from_rgb(0x66, 0xd8, 0xf0);
const SUIT: Color32 = Color32::from_rgb(0xe4, 0xe4, 0xdc);
const GUN: Color32 = Color32::from_rgb(0x50, 0x58, 0x64);
const GUN_LIGHT: Color32 = Color32::from_rgb(0x66, 0xb8, 0xff);
const VEST: Color32 = Color32::from_rgb(0x8a, 0x92, 0x5c);
const MEDKIT: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf2);
const CROSS: Color32 = Color32::from_rgb(0xe0, 0x40, 0x40);
const ROCK: Color32 = Color32::from_rgb(0x7a, 0x7e, 0x82);
const FIBRE: Color32 = Color32::from_rgb(0xd0, 0xb8, 0x80);
const BANDAGE: Color32 = Color32::from_rgb(0xe8, 0xdc, 0xc8);
const HELM: Color32 = Color32::from_rgb(0x8c, 0x9e, 0xb8);
const KEVLAR: Color32 = Color32::from_rgb(0x38, 0x3d, 0x47);
const KEVLAR_YOKE: Color32 = Color32::from_rgb(0x5c, 0x64, 0x72);
const LEGS: Color32 = Color32::from_rgb(0x3c, 0x34, 0x2c);
const LEGS_BAND: Color32 = Color32::from_rgb(0x8c, 0x9e, 0xb8);
// The long guns' wood and the sniper's scope, and the schword's hilt and
// blade, in the colours the deck draws them (`character::draw_weapon`).
const STOCK: Color32 = Color32::from_rgb(0x73, 0x4d, 0x29);
const SCOPE: Color32 = Color32::from_rgb(0x4d, 0x57, 0x66);
const HILT: Color32 = Color32::from_rgb(0x38, 0x38, 0x42);
const BLADE_CORE: Color32 = Color32::from_rgb(0xfa, 0xff, 0xff);
const BLADE_EDGE: Color32 = Color32::from_rgb(0x73, 0xf2, 0xff);
const BLADE_GLOW: Color32 = Color32::from_rgba_premultiplied(0x1c, 0x3d, 0x40, 0x40);
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
        Item::Weapon(kind) => match ResourceId::ALL.get(kind.resource() as usize) {
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

/// One resource's icon, into `rect`.
pub fn resource(painter: &egui::Painter, rect: Rect, id: ResourceId) {
    let b = Box_::new(rect);
    match id {
        ResourceId::Ore => {
            // A lump, and a fleck of the iron in it.
            painter.add(egui::Shape::convex_polygon(
                b.poly(&[
                    (0.18, 0.62),
                    (0.30, 0.30),
                    (0.55, 0.20),
                    (0.82, 0.40),
                    (0.80, 0.72),
                    (0.50, 0.84),
                ]),
                ORE,
                Stroke::new(b.px(0.04), SHADE),
            ));
            painter.circle_filled(b.at(0.58, 0.48), b.px(0.10), METAL);
        }
        ResourceId::Metal => {
            // A bar, with the light along its top edge.
            painter.rect_filled(b.rect(0.12, 0.34, 0.88, 0.66), b.px(0.06), METAL);
            painter.rect_filled(b.rect(0.16, 0.37, 0.84, 0.44), b.px(0.03), SHINE);
        }
        ResourceId::Fuel => {
            // A can with a spout.
            painter.rect_filled(b.rect(0.22, 0.32, 0.78, 0.86), b.px(0.06), FUEL);
            painter.rect_filled(b.rect(0.32, 0.18, 0.50, 0.34), b.px(0.03), FUEL);
            painter.rect_filled(b.rect(0.32, 0.50, 0.68, 0.72), b.px(0.03), SHADE);
        }
        ResourceId::Components => {
            // Three chips.
            for (x, y) in [(0.22, 0.22), (0.54, 0.22), (0.38, 0.54)] {
                painter.rect_filled(b.rect(x, y, x + 0.26, y + 0.26), b.px(0.03), COMPONENTS);
                painter.circle_filled(b.at(x + 0.13, y + 0.13), b.px(0.04), SHADE);
            }
        }
        ResourceId::Vegetable => {
            // A leaf on a stem.
            painter.add(egui::Shape::convex_polygon(
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
            painter.line_segment(
                [b.at(0.50, 0.30), b.at(0.44, 0.88)],
                Stroke::new(b.px(0.06), SHADE),
            );
        }
        ResourceId::Tofu => {
            // A block, its top face lit.
            painter.rect_filled(b.rect(0.20, 0.30, 0.80, 0.82), b.px(0.04), TOFU);
            painter.rect_filled(b.rect(0.20, 0.30, 0.80, 0.44), b.px(0.04), SHINE);
            painter.rect_stroke(
                b.rect(0.20, 0.30, 0.80, 0.82),
                b.px(0.04),
                Stroke::new(b.px(0.03), SHADE),
                egui::StrokeKind::Inside,
            );
        }
        ResourceId::Galvum => {
            // A crystal, one facet lit.
            painter.add(egui::Shape::convex_polygon(
                b.poly(&[
                    (0.50, 0.10),
                    (0.80, 0.42),
                    (0.62, 0.90),
                    (0.38, 0.90),
                    (0.20, 0.42),
                ]),
                GALVUM,
                Stroke::NONE,
            ));
            painter.add(egui::Shape::convex_polygon(
                b.poly(&[(0.50, 0.10), (0.62, 0.42), (0.50, 0.90), (0.38, 0.42)]),
                SHINE,
                Stroke::NONE,
            ));
        }
        ResourceId::Emitter => {
            // A lens: a ring, and the point of light in it.
            painter.circle_stroke(b.at(0.5, 0.5), b.px(0.30), Stroke::new(b.px(0.08), EMITTER));
            painter.circle_filled(b.at(0.5, 0.5), b.px(0.10), EMITTER);
        }
        ResourceId::Suit => {
            // A helmet over a body.
            painter.circle_filled(b.at(0.5, 0.30), b.px(0.18), SUIT);
            painter.circle_filled(b.at(0.5, 0.30), b.px(0.10), SHADE);
            painter.rect_filled(b.rect(0.28, 0.48, 0.72, 0.88), b.px(0.08), SUIT);
        }
        ResourceId::Handgun => {
            // A pistol: the barrel, the grip, and the emitter at the
            // muzzle.
            painter.rect_filled(b.rect(0.14, 0.34, 0.86, 0.52), b.px(0.04), GUN);
            painter.rect_filled(b.rect(0.30, 0.50, 0.50, 0.84), b.px(0.04), GUN);
            painter.circle_filled(b.at(0.82, 0.43), b.px(0.07), GUN_LIGHT);
        }
        ResourceId::Vest => {
            vest(painter, &b, VEST, SHADE);
        }
        ResourceId::Medkit => {
            // A case with a cross on it.
            painter.rect_filled(b.rect(0.14, 0.26, 0.86, 0.82), b.px(0.06), MEDKIT);
            painter.rect_filled(b.rect(0.42, 0.36, 0.58, 0.72), b.px(0.02), CROSS);
            painter.rect_filled(b.rect(0.24, 0.46, 0.76, 0.62), b.px(0.02), CROSS);
        }
        ResourceId::Rock => {
            painter.add(egui::Shape::convex_polygon(
                b.poly(&[
                    (0.16, 0.66),
                    (0.26, 0.34),
                    (0.52, 0.18),
                    (0.84, 0.36),
                    (0.82, 0.74),
                    (0.48, 0.86),
                ]),
                ROCK,
                Stroke::new(b.px(0.04), SHADE),
            ));
        }
        ResourceId::Fibre => {
            // A bundle of stalks, tied.
            for x in [0.34, 0.44, 0.54, 0.64] {
                painter.line_segment(
                    [b.at(x, 0.14), b.at(x, 0.86)],
                    Stroke::new(b.px(0.06), FIBRE),
                );
            }
            painter.rect_filled(b.rect(0.26, 0.44, 0.74, 0.56), b.px(0.02), SHADE);
        }
        ResourceId::Bandage => {
            // A roll, seen end on beside its tail.
            painter.rect_filled(b.rect(0.16, 0.40, 0.84, 0.60), b.px(0.10), BANDAGE);
            painter.circle_filled(b.at(0.30, 0.50), b.px(0.16), BANDAGE);
            painter.circle_stroke(b.at(0.30, 0.50), b.px(0.08), Stroke::new(b.px(0.03), SHADE));
        }
        ResourceId::Helm => {
            // A cap: the dome and the brim.
            painter.add(egui::Shape::convex_polygon(
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
            painter.rect_filled(b.rect(0.12, 0.60, 0.88, 0.70), b.px(0.03), HELM);
            painter.rect_filled(b.rect(0.12, 0.60, 0.88, 0.70), b.px(0.03), SHADE);
        }
        ResourceId::Kevlar => {
            vest(painter, &b, KEVLAR, KEVLAR_YOKE);
        }
        ResourceId::LegGuard => {
            // Two guards, a band across each shin.
            for x in [0.22, 0.56] {
                painter.rect_filled(b.rect(x, 0.16, x + 0.22, 0.86), b.px(0.06), LEGS);
                painter.rect_filled(b.rect(x, 0.42, x + 0.22, 0.52), b.px(0.02), LEGS_BAND);
            }
        }
        // The three long guns all face right like the handgun, the muzzle
        // lit, and are told apart by what the deck tells them apart by
        // (`character::draw_weapon`): the shotgun's wide barrel and wooden
        // stock and fore-end, the rifle's magazine under a short barrel,
        // the sniper's length and the scope block on top.
        ResourceId::Shotgun => {
            painter.rect_filled(b.rect(0.06, 0.48, 0.30, 0.66), b.px(0.04), STOCK);
            painter.rect_filled(b.rect(0.24, 0.38, 0.92, 0.54), b.px(0.03), GUN);
            painter.rect_filled(b.rect(0.44, 0.52, 0.66, 0.62), b.px(0.03), STOCK);
            painter.rect_filled(b.rect(0.32, 0.52, 0.42, 0.72), b.px(0.03), GUN);
            painter.circle_filled(b.at(0.90, 0.46), b.px(0.07), GUN_LIGHT);
        }
        ResourceId::AutoRifle => {
            painter.rect_filled(b.rect(0.08, 0.46, 0.26, 0.62), b.px(0.03), GUN);
            painter.rect_filled(b.rect(0.20, 0.40, 0.88, 0.52), b.px(0.03), GUN);
            painter.rect_filled(b.rect(0.30, 0.50, 0.40, 0.70), b.px(0.03), GUN);
            painter.rect_filled(b.rect(0.50, 0.50, 0.62, 0.78), b.px(0.03), SCOPE);
            painter.circle_filled(b.at(0.88, 0.46), b.px(0.06), GUN_LIGHT);
        }
        ResourceId::SniperRifle => {
            painter.rect_filled(b.rect(0.04, 0.50, 0.24, 0.66), b.px(0.04), STOCK);
            painter.rect_filled(b.rect(0.18, 0.44, 0.96, 0.54), b.px(0.02), GUN);
            painter.rect_filled(b.rect(0.34, 0.30, 0.62, 0.42), b.px(0.03), SCOPE);
            painter.circle_filled(b.at(0.36, 0.36), b.px(0.05), GUN_LIGHT);
            painter.rect_filled(b.rect(0.28, 0.52, 0.38, 0.72), b.px(0.03), GUN);
            painter.circle_filled(b.at(0.94, 0.49), b.px(0.05), GUN_LIGHT);
        }
        // The schword: a hilt at the bottom left and the blade up to the
        // right, a white core between two cyan strokes — the wide faint
        // one under the bright thin one, which is as near as a flat cell
        // gets to the deck's glow.
        ResourceId::Schword => {
            let (foot, tip) = (b.at(0.32, 0.66), b.at(0.84, 0.14));
            painter.line_segment([foot, tip], Stroke::new(b.px(0.22), BLADE_GLOW));
            painter.line_segment([foot, tip], Stroke::new(b.px(0.11), BLADE_EDGE));
            painter.line_segment([foot, tip], Stroke::new(b.px(0.05), BLADE_CORE));
            painter.line_segment(
                [b.at(0.18, 0.60), b.at(0.40, 0.82)],
                Stroke::new(b.px(0.07), HILT),
            );
            painter.line_segment(
                [b.at(0.30, 0.68), b.at(0.14, 0.86)],
                Stroke::new(b.px(0.12), HILT),
            );
        }
    }
}

/// A vest: the body of it with the neck cut out, and the yoke across the
/// shoulders in the second colour.
fn vest(painter: &egui::Painter, b: &Box_, body: Color32, yoke: Color32) {
    painter.add(egui::Shape::convex_polygon(
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
    painter.rect_filled(b.rect(0.20, 0.20, 0.36, 0.40), b.px(0.02), yoke);
    painter.rect_filled(b.rect(0.64, 0.20, 0.80, 0.40), b.px(0.02), yoke);
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

    fn px(&self, fraction: f32) -> f32 {
        self.rect.width() * fraction
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
            let mut piece = Piece::new(1, kind);
            icon(&painter, rect, Item::Armour(piece));
            piece.health = 0.0;
            icon(&painter, rect, Item::Armour(piece));
        }
        for &weapon in WeaponKind::ALL.iter() {
            icon(&painter, rect, Item::Weapon(weapon));
        }
        icon(&painter, rect, Item::Stack(u32::MAX));
    }
}

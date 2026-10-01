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
//! A relic has a picture too (task 136, [`relic`]): a plate rimmed in its
//! family's colour, beside its name on the character sheet and on the side
//! panel of a crewmate picked.
//!
//! Nothing here is text: an icon that needed a glyph would need the font
//! to have it, and the default font has few — see the root notes.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, pos2, vec2};
use bims::combat::{ArmourKind, Item};
use physics::ResourceId;
use world::Relic;

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
// The armour (October 2026, the one piece a Bim wears): dark plate in
// the deck's colour, trimmed in steel-blue at the joints.
const ARMOUR: Color32 = Color32::from_rgb(0x38, 0x3d, 0x47);
const ARMOUR_TRIM: Color32 = Color32::from_rgb(0x8c, 0x9e, 0xb8);
// A pale arc blue and a mirror's silver, with its bar of light: once the
// arc greaves' and the Reflective plate's, now the relics' pictures'.
const GREAVES_COIL: Color32 = Color32::from_rgb(0x8c, 0xd1, 0xff);
const MIRROR: Color32 = Color32::from_rgb(0xbd, 0xc9, 0xd9);
const MIRROR_YOKE: Color32 = Color32::from_rgb(0x8a, 0x96, 0xa8);
const MIRROR_SHINE: Color32 = Color32::from_rgb(0xf5, 0xfa, 0xff);
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
const SACK_DARK: Color32 = Color32::from_rgb(0x7e, 0x6c, 0x48);
const CRATE: Color32 = Color32::from_rgb(0x6a, 0x72, 0x7e);
/// The Healing Sentry's cross (task 127), the medic's beam's green.
const HEAL: Color32 = Color32::from_rgb(0x6c, 0xe0, 0x8a);
/// An EMP's band and spark (task 127): the stun's pale blue.
const EMP_BAND: Color32 = Color32::from_rgb(0x9e, 0xd6, 0xff);
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
        Item::Weapon(weapon) => match weapon.kind.resource().and_then(ResourceId::from_code) {
            Some(id) => resource(painter, rect, id),
            None => unknown(painter, rect),
        },
        Item::Stack(code) => match ResourceId::from_code(code) {
            Some(id) => resource(painter, rect, id),
            None => unknown(painter, rect),
        },
        Item::Module(item) => module(painter, rect, item),
    }
}

// The items' pictures (October 2026): a dark plate rimmed in brass, the
// thing on it in its own colour, and its tier as pips in the corner.
const ITEM_PLATE: Color32 = Color32::from_rgb(0x1c, 0x1e, 0x26);
const ITEM_RIM: Color32 = Color32::from_rgb(0xb8, 0x92, 0x4a);
const BLINK: Color32 = Color32::from_rgb(0xb4, 0x80, 0xff);
const BLINK_PALE: Color32 = Color32::from_rgb(0xe6, 0xd8, 0xff);
const EXECUTE: Color32 = Color32::from_rgb(0xe8, 0x4c, 0x3c);
const HEART: Color32 = Color32::from_rgb(0xc8, 0x34, 0x40);
const HEART_CORE: Color32 = Color32::from_rgb(0x7c, 0xf0, 0x9a);
const CORE_GOLD: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x48);
const CORE_DEEP: Color32 = Color32::from_rgb(0x6a, 0x4c, 0x18);

/// An item's picture, into `rect` (October 2026): its plate and the
/// thing, with one pip a tier in the top right for a tiered kind.
pub fn module(painter: &egui::Painter, rect: Rect, item: bims::module::Module) {
    let mut s = Sketch::default();
    let plate = Box_::new(rect);
    let r = plate.px(0.16);
    s.rect_filled(plate.rect(0.02, 0.02, 0.98, 0.98), r, ITEM_PLATE);
    s.rect_stroke(
        plate.rect(0.02, 0.02, 0.98, 0.98),
        r,
        Stroke::new(plate.px(0.05), ITEM_RIM),
        egui::StrokeKind::Inside,
    );
    let b = Box_::new(plate.rect(0.14, 0.14, 0.86, 0.86));
    draw_module(&mut s, &b, item.kind);
    if item.kind.tiered() {
        for n in 0..item.tier.code() {
            let x = 0.84 - 0.13 * n as f32;
            s.circle_filled(plate.at(x, 0.14), plate.px(0.05), ITEM_RIM);
        }
    }
    painter.extend(s.shapes);
}

/// One item's thing, drawn in fractions of the plate's inside.
fn draw_module(s: &mut Sketch, b: &Box_, kind: bims::module::ModuleKind) {
    use bims::module::ModuleKind;
    let st = |width: f32, colour: Color32| Stroke::new(b.px(width), colour);
    match kind {
        // A shard flying up and right, the streaks it leaves behind it.
        ModuleKind::BlinkDrive => {
            for (x, y) in [(0.06, 0.70), (0.14, 0.86), (0.02, 0.54)] {
                s.line_segment([b.at(x, y), b.at(x + 0.26, y - 0.26)], st(0.05, BLINK));
            }
            s.fill(
                b.poly(&[(0.92, 0.08), (0.80, 0.46), (0.54, 0.46), (0.54, 0.20)]),
                BLINK,
            );
            s.fill(
                b.poly(&[(0.54, 0.46), (0.80, 0.46), (0.36, 0.86), (0.26, 0.76)]),
                BLINK,
            );
            s.line_segment([b.at(0.88, 0.12), b.at(0.40, 0.72)], st(0.05, BLINK_PALE));
            s.circle_filled(b.at(0.88, 0.12), b.px(0.07), BLINK_PALE);
        }
        // A reticle, its four ticks, and the red mark in it.
        ModuleKind::Executioner => {
            s.circle_stroke(b.at(0.5, 0.5), b.px(0.34), st(0.07, EXECUTE));
            for (a, c) in [
                ((0.5, 0.02), (0.5, 0.24)),
                ((0.5, 0.76), (0.5, 0.98)),
                ((0.02, 0.5), (0.24, 0.5)),
                ((0.76, 0.5), (0.98, 0.5)),
            ] {
                s.line_segment([b.at(a.0, a.1), b.at(c.0, c.1)], st(0.07, EXECUTE));
            }
            s.circle_filled(b.at(0.5, 0.5), b.px(0.10), EXECUTE);
            s.circle_filled(b.at(0.47, 0.47), b.px(0.04), SHINE);
        }
        // A heart, a reactor's green core lit in it.
        ModuleKind::ReactorHeart => {
            s.circle_filled(b.at(0.32, 0.36), b.px(0.22), HEART);
            s.circle_filled(b.at(0.68, 0.36), b.px(0.22), HEART);
            s.fill(b.poly(&[(0.12, 0.44), (0.88, 0.44), (0.50, 0.92)]), HEART);
            s.circle_filled(b.at(0.5, 0.48), b.px(0.15), HEART_CORE);
            s.circle_stroke(b.at(0.5, 0.48), b.px(0.22), st(0.03, HEART_CORE));
            s.circle_filled(b.at(0.28, 0.30), b.px(0.06), SHINE);
        }
        // A gold hexagon, a chevron up through it: one rank more.
        ModuleKind::OverrideCore => {
            let hex: Vec<(f32, f32)> = (0..6)
                .map(|i| polar_at(0.5, 0.5, 0.46, 60.0 * i as f32 + 30.0))
                .collect();
            s.fill(b.poly(&hex), CORE_DEEP);
            let inner: Vec<(f32, f32)> = (0..6)
                .map(|i| polar_at(0.5, 0.5, 0.36, 60.0 * i as f32 + 30.0))
                .collect();
            s.fill(b.poly(&inner), CORE_GOLD);
            s.path(
                b.poly(&[(0.28, 0.62), (0.5, 0.36), (0.72, 0.62)]),
                st(0.10, CORE_DEEP),
            );
            s.path(
                b.poly(&[(0.32, 0.80), (0.5, 0.58), (0.68, 0.80)]),
                st(0.08, CORE_DEEP),
            );
        }
        // The three relics in item form keep their pictures.
        ModuleKind::CoolantLoop => draw_relic(s, b, Relic::CoolantLoop),
        ModuleKind::PressureSeal => draw_relic(s, b, Relic::PressureSeal),
        ModuleKind::SteadyGrip => draw_relic(s, b, Relic::SteadyGrip),
        // A long barrel, its muzzle, and the reach beyond it.
        ModuleKind::LongBarrel => {
            s.rect_filled(b.rect(0.04, 0.44, 0.30, 0.62), b.px(0.03), STOCK);
            s.rect_filled(b.rect(0.26, 0.46, 0.80, 0.56), b.px(0.02), GUN_STEEL);
            s.rect_filled(b.rect(0.76, 0.42, 0.86, 0.60), b.px(0.02), GUN_DARK);
            for (x, w) in [(0.90, 0.05), (0.96, 0.035)] {
                s.line_segment([b.at(x, 0.36), b.at(x, 0.66)], st(w, GUN_LIGHT));
            }
            s.line_segment([b.at(0.30, 0.30), b.at(0.86, 0.30)], st(0.03, GUN_LIGHT));
        }
        // A capacitor's two plates, a drop of blood drawn through them.
        ModuleKind::LeechCapacitor => {
            s.rect_filled(b.rect(0.18, 0.20, 0.30, 0.80), b.px(0.02), GUN_STEEL);
            s.rect_filled(b.rect(0.70, 0.20, 0.82, 0.80), b.px(0.02), GUN_STEEL);
            s.fill(b.poly(&[(0.50, 0.20), (0.62, 0.48), (0.38, 0.48)]), CROSS);
            s.circle_filled(b.at(0.50, 0.58), b.px(0.14), CROSS);
            s.circle_filled(b.at(0.46, 0.54), b.px(0.04), SHINE);
            s.line_segment([b.at(0.30, 0.50), b.at(0.36, 0.50)], st(0.04, HEAL));
            s.line_segment([b.at(0.64, 0.50), b.at(0.70, 0.50)], st(0.04, HEAL));
        }
        // A coil, and the arc jumping off it.
        ModuleKind::ArcCoil => {
            for y in [0.56, 0.68, 0.80] {
                s.path(b.arc(0.34, y, 0.18, 180.0, 360.0), st(0.06, KEY_EDGE));
            }
            s.path(
                b.poly(&[
                    (0.40, 0.44),
                    (0.58, 0.30),
                    (0.52, 0.22),
                    (0.74, 0.10),
                    (0.68, 0.28),
                    (0.92, 0.20),
                ]),
                st(0.06, EMP_BAND),
            );
            s.circle_filled(b.at(0.92, 0.20), b.px(0.05), BLINK_PALE);
        }
        // A ring round a green cross: the crew round the holder mended.
        ModuleKind::FieldMender => {
            s.circle_stroke(b.at(0.5, 0.5), b.px(0.42), st(0.05, HEAL));
            s.rect_filled(b.rect(0.40, 0.18, 0.60, 0.82), b.px(0.03), HEAL);
            s.rect_filled(b.rect(0.18, 0.40, 0.82, 0.60), b.px(0.03), HEAL);
            s.rect_filled(b.rect(0.44, 0.44, 0.56, 0.56), b.px(0.02), SHINE);
        }
        // A charge's cell, and the arrow round it back to the start.
        ModuleKind::ResetCapacitor => {
            s.path(b.arc(0.5, 0.5, 0.38, -60.0, 230.0), st(0.08, EMP_BAND));
            s.fill(
                b.poly(&[(0.70, 0.04), (0.92, 0.22), (0.62, 0.30)]),
                EMP_BAND,
            );
            s.rect_filled(b.rect(0.38, 0.34, 0.62, 0.70), b.px(0.04), GUN_DARK);
            s.rect_filled(b.rect(0.42, 0.44, 0.58, 0.66), b.px(0.02), KEY_TRACE);
            s.rect_filled(b.rect(0.45, 0.28, 0.55, 0.34), b.px(0.01), GUN_STEEL);
        }
        // A shield of plates, layered.
        ModuleKind::AblativeShell => {
            s.fill(
                b.poly(&[
                    (0.50, 0.04),
                    (0.90, 0.18),
                    (0.84, 0.60),
                    (0.50, 0.96),
                    (0.16, 0.60),
                    (0.10, 0.18),
                ]),
                CORE_DEEP,
            );
            s.fill(
                b.poly(&[
                    (0.50, 0.14),
                    (0.80, 0.25),
                    (0.75, 0.58),
                    (0.50, 0.86),
                    (0.25, 0.58),
                    (0.20, 0.25),
                ]),
                CORE_GOLD,
            );
            for y in [0.38, 0.56] {
                s.line_segment([b.at(0.26, y), b.at(0.74, y)], st(0.04, CORE_DEEP));
            }
        }
    }
}

/// A piece of armour: its resource's icon, cracked if it is broken.
pub fn armour(painter: &egui::Painter, rect: Rect, kind: ArmourKind, broken: bool) {
    let id = ResourceId::from_code(kind.resource()).unwrap_or(ResourceId::Armour);
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
        ResourceId::Armour => armour_suit(s, b),
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
    }
}

// --- the relics (task 136) -------------------------------------------------------

// A relic is a little plate with a picture on it, the plate's rim in the
// colour of its family (the first twelve, then task 118's five patches),
// so a Bim's row of them says at a glance what kind of help it carries.
// The tier is never shown (`world::relic`), and the rims are no tier's
// colours.
const PLATE: Color32 = Color32::from_rgb(0x14, 0x1b, 0x18);
const RIM_CORE: Color32 = Color32::from_rgb(0x9a, 0xa8, 0xb8);
const RIM_DISMANTLER: Color32 = Color32::from_rgb(0xe8, 0x84, 0x3c);
const RIM_LIFELINE: Color32 = HEAL;
const RIM_FLANKER: Color32 = Color32::from_rgb(0xb4, 0x82, 0xf0);
const RIM_COMMAND: Color32 = Color32::from_rgb(0x6f, 0xa8, 0xe8);
const RIM_SUPPLY: Color32 = Color32::from_rgb(0xe8, 0xc8, 0x50);
/// A machine's eye, and whatever in a picture is the machines'.
const EYE: Color32 = Color32::from_rgb(0xff, 0x4a, 0x3a);
/// A machine's plating.
const MACHINE: Color32 = Color32::from_rgb(0x7a, 0x82, 0x8e);
/// Light gathered, a shot, a spark.
const BEAM: Color32 = Color32::from_rgb(0xff, 0x9a, 0x4a);
const GOLD: Color32 = Color32::from_rgb(0xf0, 0xc4, 0x4a);
const GOLD_DARK: Color32 = Color32::from_rgb(0xa8, 0x80, 0x24);
/// A sight's green.
const SIGHT: Color32 = Color32::from_rgb(0x9e, 0xf0, 0xb8);
/// The Phase Harness's glow, and the ghost of the Bim it leaves behind.
const PHASE: Color32 = Color32::from_rgb(0xc8, 0xa8, 0xff);
const PHASE_GHOST: Color32 = Color32::from_rgba_premultiplied(0x3c, 0x30, 0x58, 0x70);
/// The fan a Bim sees over and the cone a machine aims down, faint.
const FAN: Color32 = Color32::from_rgba_premultiplied(0x30, 0x24, 0x48, 0x70);
const CONE: Color32 = Color32::from_rgba_premultiplied(0x58, 0x16, 0x10, 0x70);
const LID: Color32 = Color32::from_rgb(0x96, 0x68, 0x3a);

/// Which family a relic is, as its plate's rim: the app's grouping, as
/// `names::relic_line` lists them, since the rules keep none.
fn relic_rim(relic: Relic) -> Color32 {
    use Relic::*;
    match relic {
        FocusingLens | ServoBraces | FieldPlating | CoolantLoop | SteadyGrip | TraumaKit
        | SecondWind | SalvageBeacon | OverchargeCell | LastStand | KillRelay | PhaseHarness => {
            RIM_CORE
        }
        MarksmansHabit | ServoCutter | CripplersMark | PartsBroker | TotalTeardown => {
            RIM_DISMANTLER
        }
        PressureSeal | QuickWrap | ClotBooster | TetherField | Lifeline => RIM_LIFELINE,
        BlindSpot | SprintCoil | SignalScrambler | WideAngleOptics | Crossfire => RIM_FLANKER,
        FieldRadio | Spotter | SquadMorale | CoverFormation | RallyPoint => RIM_COMMAND,
        HazardPay | TradeLicense | RestockCodes | StrongWill | WarChest => RIM_SUPPLY,
    }
}

/// A relic's picture, into `rect`: its plate, rimmed, square in the
/// middle of it.
pub fn relic(painter: &egui::Painter, rect: Rect, relic: Relic) {
    let mut s = Sketch::default();
    let plate = Box_::new(rect);
    let r = plate.px(0.18);
    s.rect_filled(plate.rect(0.02, 0.02, 0.98, 0.98), r, PLATE);
    s.rect_stroke(
        plate.rect(0.02, 0.02, 0.98, 0.98),
        r,
        Stroke::new(plate.px(0.06), relic_rim(relic)),
        egui::StrokeKind::Inside,
    );
    let b = Box_::new(plate.rect(0.13, 0.13, 0.87, 0.87));
    draw_relic(&mut s, &b, relic);
    painter.extend(s.shapes);
}

/// One relic's picture, drawn in fractions of the plate's inside.
fn draw_relic(s: &mut Sketch, b: &Box_, relic: Relic) {
    let st = |width: f32, colour: Color32| Stroke::new(b.px(width), colour);
    match relic {
        // A lens, and the light it gathers drawn to a point.
        Relic::FocusingLens => {
            for y in [0.30, 0.50, 0.70] {
                s.line_segment([b.at(0.02, y), b.at(0.34, y)], st(0.04, LENS));
            }
            for y in [0.30, 0.50, 0.70] {
                s.line_segment([b.at(0.46, y), b.at(0.92, 0.50)], st(0.05, BEAM));
            }
            s.fill(b.arc(0.40, 0.50, 0.12, 0.0, 360.0), LENS);
            s.fill(b.arc(0.40, 0.50, 0.12, 180.0, 270.0), SHINE);
            s.circle_filled(b.at(0.92, 0.50), b.px(0.07), BEAM);
        }
        // A leg in a brace, bent at a powered knee, and the speed behind it.
        Relic::ServoBraces => {
            for (y, x) in [(0.30, 0.10), (0.50, 0.02), (0.70, 0.12)] {
                s.line_segment([b.at(x, y), b.at(x + 0.24, y)], st(0.04, LENS));
            }
            s.line_segment([b.at(0.56, 0.08), b.at(0.64, 0.48)], st(0.15, GUN_STEEL));
            s.line_segment([b.at(0.64, 0.48), b.at(0.48, 0.86)], st(0.13, GUN_STEEL));
            s.line_segment([b.at(0.46, 0.88), b.at(0.70, 0.92)], st(0.10, GUN_STEEL));
            s.circle_filled(b.at(0.64, 0.48), b.px(0.11), GUN_DARK);
            s.circle_filled(b.at(0.64, 0.48), b.px(0.05), GUN_LIGHT);
        }
        // A plate shield, riveted.
        Relic::FieldPlating => {
            s.fill(
                b.poly(&[
                    (0.50, 0.04),
                    (0.90, 0.18),
                    (0.84, 0.60),
                    (0.50, 0.96),
                    (0.16, 0.60),
                    (0.10, 0.18),
                ]),
                MIRROR_YOKE,
            );
            s.fill(
                b.poly(&[
                    (0.50, 0.16),
                    (0.78, 0.26),
                    (0.74, 0.58),
                    (0.50, 0.84),
                    (0.26, 0.58),
                    (0.22, 0.26),
                ]),
                MIRROR,
            );
            s.line_segment([b.at(0.34, 0.30), b.at(0.40, 0.62)], st(0.06, MIRROR_SHINE));
            for (x, y) in [(0.50, 0.10), (0.18, 0.22), (0.82, 0.22), (0.50, 0.90)] {
                s.circle_filled(b.at(x, y), b.px(0.035), GUN_DARK);
            }
        }
        // A loop of pipe with the coolant going round it, and a drop.
        Relic::CoolantLoop => {
            s.path(b.arc(0.50, 0.52, 0.34, -60.0, 250.0), st(0.10, EMP_BAND));
            s.fill(
                b.poly(&[(0.36, 0.09), (0.56, 0.14), (0.42, 0.31)]),
                EMP_BAND,
            );
            s.fill(b.arc(0.50, 0.60, 0.12, 0.0, 180.0), LENS);
            s.fill(b.poly(&[(0.50, 0.34), (0.62, 0.60), (0.38, 0.60)]), LENS);
        }
        // A sight, dead still on its mark.
        Relic::SteadyGrip => {
            s.circle_stroke(b.at(0.5, 0.5), b.px(0.30), st(0.07, SIGHT));
            for (a, z) in [((0.5, 0.02), (0.5, 0.30)), ((0.5, 0.70), (0.5, 0.98))] {
                s.line_segment([b.at(a.0, a.1), b.at(z.0, z.1)], st(0.06, SIGHT));
                s.line_segment([b.at(a.1, a.0), b.at(z.1, z.0)], st(0.06, SIGHT));
            }
            s.circle_filled(b.at(0.5, 0.5), b.px(0.07), EYE);
        }
        // The medkit, and a clock's face: the revive done sooner.
        Relic::TraumaKit => {
            draw_resource(s, b, ResourceId::Medkit);
            s.circle_filled(b.at(0.78, 0.76), b.px(0.20), GUN_DARK);
            s.circle_stroke(b.at(0.78, 0.76), b.px(0.20), st(0.04, MEDKIT));
            s.line_segment([b.at(0.78, 0.76), b.at(0.78, 0.63)], st(0.04, MEDKIT));
            s.line_segment([b.at(0.78, 0.76), b.at(0.88, 0.76)], st(0.04, MEDKIT));
        }
        // A heart, and the arrow up: back on its feet.
        Relic::SecondWind => {
            s.circle_filled(b.at(0.34, 0.38), b.px(0.19), CROSS);
            s.circle_filled(b.at(0.66, 0.38), b.px(0.19), CROSS);
            s.fill(b.poly(&[(0.155, 0.44), (0.845, 0.44), (0.50, 0.90)]), CROSS);
            s.path(
                b.poly(&[(0.34, 0.60), (0.50, 0.42), (0.66, 0.60)]),
                st(0.08, MEDKIT),
            );
        }
        // A beacon on its mast, calling.
        Relic::SalvageBeacon => {
            s.fill(
                b.poly(&[(0.40, 0.94), (0.60, 0.94), (0.53, 0.40), (0.47, 0.40)]),
                GUN_STEEL,
            );
            s.circle_filled(b.at(0.50, 0.36), b.px(0.09), KEY_TRACE);
            for r in [0.20, 0.34] {
                s.path(b.arc(0.50, 0.36, r, -40.0, 40.0), st(0.05, KEY_TRACE));
                s.path(b.arc(0.50, 0.36, r, 140.0, 220.0), st(0.05, KEY_TRACE));
            }
        }
        // A cell with a bolt on it.
        Relic::OverchargeCell => {
            s.rect_filled(b.rect(0.40, 0.04, 0.60, 0.16), b.px(0.03), GUN_STEEL);
            s.rect_filled(b.rect(0.22, 0.14, 0.78, 0.96), b.px(0.08), GUN_DARK);
            s.rect_stroke(
                b.rect(0.22, 0.14, 0.78, 0.96),
                b.px(0.08),
                st(0.05, GUN_STEEL),
                egui::StrokeKind::Inside,
            );
            s.fill(
                b.poly(&[(0.58, 0.24), (0.34, 0.60), (0.54, 0.60)]),
                KEY_TRACE,
            );
            s.fill(
                b.poly(&[(0.46, 0.52), (0.66, 0.52), (0.42, 0.88)]),
                KEY_TRACE,
            );
        }
        // A blade held up, edged in red: fighting on with somebody down.
        Relic::LastStand => {
            s.fill(
                b.poly(&[
                    (0.44, 0.64),
                    (0.44, 0.16),
                    (0.50, 0.02),
                    (0.56, 0.16),
                    (0.56, 0.64),
                ]),
                EYE,
            );
            s.fill(
                b.poly(&[
                    (0.475, 0.62),
                    (0.475, 0.18),
                    (0.50, 0.10),
                    (0.525, 0.18),
                    (0.525, 0.62),
                ]),
                BLADE_CORE,
            );
            s.rect_filled(b.rect(0.26, 0.64, 0.74, 0.72), b.px(0.03), HILT);
            s.rect_filled(b.rect(0.45, 0.72, 0.55, 0.90), b.px(0.02), HILT);
            s.circle_filled(b.at(0.50, 0.92), b.px(0.06), FUSE_CAP);
        }
        // An hourglass, and a machine's eye struck out beside it: a kill
        // is time back.
        Relic::KillRelay => {
            s.rect_filled(b.rect(0.08, 0.06, 0.60, 0.14), b.px(0.02), STOCK);
            s.rect_filled(b.rect(0.08, 0.86, 0.60, 0.94), b.px(0.02), STOCK);
            s.fill(b.poly(&[(0.14, 0.14), (0.54, 0.14), (0.34, 0.50)]), SHINE);
            s.fill(b.poly(&[(0.34, 0.50), (0.54, 0.86), (0.14, 0.86)]), SHINE);
            s.fill(b.poly(&[(0.24, 0.26), (0.44, 0.26), (0.34, 0.44)]), GOLD);
            s.fill(b.poly(&[(0.34, 0.62), (0.50, 0.86), (0.18, 0.86)]), GOLD);
            s.circle_filled(b.at(0.80, 0.50), b.px(0.14), GUN_DARK);
            s.circle_filled(b.at(0.80, 0.50), b.px(0.06), EYE);
            s.line_segment([b.at(0.66, 0.36), b.at(0.94, 0.64)], st(0.06, MEDKIT));
            s.line_segment([b.at(0.94, 0.36), b.at(0.66, 0.64)], st(0.06, MEDKIT));
        }
        // A Bim with its ghost behind it, in a ring of light.
        Relic::PhaseHarness => {
            s.circle_filled(b.at(0.64, 0.38), b.px(0.24), PHASE_GHOST);
            s.circle_filled(b.at(0.42, 0.58), b.px(0.24), PHASE);
            s.circle_filled(b.at(0.42, 0.58), b.px(0.12), SHINE);
            s.circle_stroke(b.at(0.42, 0.58), b.px(0.36), st(0.05, PHASE));
        }
        // A machine, and the sight on its leg.
        Relic::MarksmansHabit => {
            machine(s, b, [true, true], true);
            s.circle_stroke(b.at(0.62, 0.78), b.px(0.15), st(0.05, EYE));
            s.line_segment([b.at(0.62, 0.58), b.at(0.62, 0.98)], st(0.04, EYE));
            s.line_segment([b.at(0.42, 0.78), b.at(0.82, 0.78)], st(0.04, EYE));
        }
        // A saw blade.
        Relic::ServoCutter => {
            for i in 0..10 {
                let a = i as f32 * 36.0;
                let (x0, y0) = polar(0.30, a - 14.0);
                let (x1, y1) = polar(0.30, a + 14.0);
                let (x2, y2) = polar(0.46, a + 12.0);
                s.fill(b.poly(&[(x0, y0), (x1, y1), (x2, y2)]), GUN_STEEL);
            }
            s.circle_filled(b.at(0.5, 0.5), b.px(0.32), GUN_STEEL);
            s.circle_stroke(b.at(0.5, 0.5), b.px(0.22), st(0.03, GUN_DARK));
            s.circle_filled(b.at(0.5, 0.5), b.px(0.09), GUN_DARK);
        }
        // A machine down a leg, sparking, marked.
        Relic::CripplersMark => {
            machine(s, b, [true, false], true);
            for (x, y) in [(0.62, 0.84), (0.74, 0.76), (0.54, 0.88)] {
                s.line_segment([b.at(0.60, 0.66), b.at(x, y)], st(0.035, BEAM));
            }
            s.circle_stroke(b.at(0.50, 0.46), b.px(0.10), st(0.05, EYE));
        }
        // A gauge with its needle in the green.
        Relic::PressureSeal => {
            s.circle_filled(b.at(0.5, 0.5), b.px(0.44), GUN_DARK);
            s.circle_stroke(b.at(0.5, 0.5), b.px(0.44), st(0.06, GUN_STEEL));
            s.path(b.arc(0.5, 0.5, 0.32, 150.0, 390.0), st(0.04, GUN_STEEL));
            s.path(b.arc(0.5, 0.5, 0.32, 330.0, 390.0), st(0.06, HEAL));
            s.line_segment([b.at(0.5, 0.5), b.at(0.74, 0.38)], st(0.06, HEAL));
            s.circle_filled(b.at(0.5, 0.5), b.px(0.06), MEDKIT);
            s.rect_filled(b.rect(0.46, 0.64, 0.54, 0.84), 0.0, HEAL);
            s.rect_filled(b.rect(0.40, 0.70, 0.60, 0.78), 0.0, HEAL);
        }
        // A roll of dressing and a green cross.
        Relic::QuickWrap => {
            s.rect_filled(b.rect(0.08, 0.50, 0.84, 0.74), b.px(0.08), BANDAGE);
            s.circle_filled(b.at(0.28, 0.62), b.px(0.22), BANDAGE);
            s.circle_stroke(b.at(0.28, 0.62), b.px(0.11), st(0.04, SHADE));
            s.rect_filled(b.rect(0.64, 0.04, 0.78, 0.40), 0.0, HEAL);
            s.rect_filled(b.rect(0.53, 0.15, 0.89, 0.29), 0.0, HEAL);
        }
        // A syringe.
        Relic::ClotBooster => {
            s.fill(
                b.poly(&[(0.17, 0.69), (0.63, 0.23), (0.77, 0.37), (0.31, 0.83)]),
                LENS,
            );
            s.fill(
                b.poly(&[(0.17, 0.69), (0.40, 0.46), (0.54, 0.60), (0.31, 0.83)]),
                HEAL,
            );
            s.line_segment([b.at(0.70, 0.30), b.at(0.94, 0.06)], st(0.035, GUN_STEEL));
            s.line_segment([b.at(0.24, 0.76), b.at(0.10, 0.90)], st(0.06, GUN_STEEL));
            s.line_segment([b.at(0.02, 0.82), b.at(0.18, 0.98)], st(0.06, GUN_STEEL));
            s.line_segment([b.at(0.10, 0.62), b.at(0.38, 0.90)], st(0.05, GUN_STEEL));
        }
        // Two Bims tied together, the one brought round under a dome.
        Relic::TetherField => {
            s.path(
                b.poly(&[(0.38, 0.66), (0.45, 0.60), (0.52, 0.72), (0.58, 0.66)]),
                st(0.04, HEAL),
            );
            s.circle_filled(b.at(0.24, 0.66), b.px(0.15), SUIT);
            s.circle_filled(b.at(0.74, 0.66), b.px(0.15), SUIT);
            s.path(b.arc(0.74, 0.70, 0.28, 180.0, 360.0), st(0.06, HEAL));
        }
        // A heartbeat.
        Relic::Lifeline => {
            s.path(
                b.poly(&[
                    (0.00, 0.56),
                    (0.26, 0.56),
                    (0.34, 0.40),
                    (0.44, 0.76),
                    (0.54, 0.14),
                    (0.64, 0.68),
                    (0.70, 0.56),
                    (1.00, 0.56),
                ]),
                st(0.08, HEAL),
            );
        }
        // A machine from above, its sight down the front, and the shot
        // coming into its back.
        Relic::BlindSpot => {
            s.fill(b.poly(&[(0.62, 0.50), (1.00, 0.24), (1.00, 0.76)]), CONE);
            s.circle_filled(b.at(0.58, 0.50), b.px(0.18), MACHINE);
            s.circle_filled(b.at(0.70, 0.50), b.px(0.05), EYE);
            s.line_segment([b.at(0.00, 0.50), b.at(0.28, 0.50)], st(0.06, KEY_TRACE));
            s.fill(
                b.poly(&[(0.38, 0.50), (0.24, 0.38), (0.24, 0.62)]),
                KEY_TRACE,
            );
        }
        // A spring, and the speed behind it.
        Relic::SprintCoil => {
            for (y, x) in [(0.30, 0.04), (0.50, 0.00), (0.70, 0.06)] {
                s.line_segment([b.at(x, y), b.at(x + 0.20, y)], st(0.04, LENS));
            }
            s.path(
                b.poly(&[
                    (0.34, 0.10),
                    (0.92, 0.22),
                    (0.34, 0.34),
                    (0.92, 0.46),
                    (0.34, 0.58),
                    (0.92, 0.70),
                    (0.34, 0.82),
                    (0.92, 0.94),
                ]),
                st(0.06, GREAVES_COIL),
            );
        }
        // A machine's mast, its signal struck through.
        Relic::SignalScrambler => {
            s.line_segment([b.at(0.30, 0.96), b.at(0.30, 0.40)], st(0.07, GUN_STEEL));
            s.circle_filled(b.at(0.30, 0.36), b.px(0.08), EYE);
            for r in [0.22, 0.38] {
                s.path(b.arc(0.30, 0.36, r, -60.0, 60.0), st(0.05, RIM_FLANKER));
            }
            s.line_segment([b.at(0.36, 0.86), b.at(0.94, 0.10)], st(0.07, EYE));
        }
        // A lens, and the wide fan it takes in.
        Relic::WideAngleOptics => {
            let mut fan = vec![b.at(0.50, 0.84)];
            fan.extend(b.arc(0.50, 0.84, 0.54, 205.0, 335.0));
            s.fill(fan, FAN);
            s.line_segment([b.at(0.50, 0.84), b.at(0.01, 0.61)], st(0.04, RIM_FLANKER));
            s.line_segment([b.at(0.50, 0.84), b.at(0.99, 0.61)], st(0.04, RIM_FLANKER));
            s.circle_filled(b.at(0.50, 0.84), b.px(0.13), GUN_DARK);
            s.circle_filled(b.at(0.50, 0.84), b.px(0.07), LENS);
        }
        // Two shots from either side meeting in one machine.
        Relic::Crossfire => {
            s.circle_filled(b.at(0.50, 0.50), b.px(0.16), MACHINE);
            s.circle_filled(b.at(0.50, 0.50), b.px(0.05), EYE);
            s.line_segment([b.at(0.00, 0.50), b.at(0.24, 0.50)], st(0.06, KEY_TRACE));
            s.fill(
                b.poly(&[(0.32, 0.50), (0.20, 0.40), (0.20, 0.60)]),
                KEY_TRACE,
            );
            s.line_segment([b.at(1.00, 0.50), b.at(0.76, 0.50)], st(0.06, KEY_TRACE));
            s.fill(
                b.poly(&[(0.68, 0.50), (0.80, 0.40), (0.80, 0.60)]),
                KEY_TRACE,
            );
        }
        // A handset with its aerial, talking.
        Relic::FieldRadio => {
            s.rect_filled(b.rect(0.56, 0.04, 0.64, 0.30), b.px(0.02), GUN_DARK);
            s.rect_filled(b.rect(0.28, 0.26, 0.68, 0.96), b.px(0.07), GUN);
            s.rect_filled(b.rect(0.35, 0.34, 0.61, 0.48), b.px(0.02), LENS);
            for y in [0.58, 0.66, 0.74, 0.82] {
                s.line_segment([b.at(0.36, y), b.at(0.60, y)], st(0.03, GUN_DARK));
            }
            for r in [0.14, 0.26] {
                s.path(b.arc(0.64, 0.12, r, -30.0, 50.0), st(0.045, RIM_COMMAND));
            }
        }
        // Field glasses.
        Relic::Spotter => {
            s.rect_filled(b.rect(0.08, 0.24, 0.44, 0.84), b.px(0.10), GUN);
            s.rect_filled(b.rect(0.56, 0.24, 0.92, 0.84), b.px(0.10), GUN);
            s.rect_filled(b.rect(0.40, 0.36, 0.60, 0.54), b.px(0.02), GUN_DARK);
            s.circle_filled(b.at(0.26, 0.72), b.px(0.13), LENS);
            s.circle_filled(b.at(0.74, 0.72), b.px(0.13), LENS);
            s.circle_filled(b.at(0.22, 0.68), b.px(0.04), SHINE);
            s.circle_filled(b.at(0.70, 0.68), b.px(0.04), SHINE);
        }
        // A sergeant's three stripes.
        Relic::SquadMorale => {
            for dy in [0.0, 0.24, 0.48] {
                s.path(
                    b.poly(&[(0.14, 0.34 + dy), (0.50, 0.10 + dy), (0.86, 0.34 + dy)]),
                    st(0.11, GOLD),
                );
            }
        }
        // Three bots under one shield.
        Relic::CoverFormation => {
            for (x, y) in [(0.20, 0.76), (0.50, 0.82), (0.80, 0.76)] {
                s.circle_filled(b.at(x, y), b.px(0.12), CRATE);
                s.circle_filled(b.at(x, y), b.px(0.04), GUN_LIGHT);
            }
            s.path(b.arc(0.50, 0.96, 0.62, 212.0, 328.0), st(0.11, RIM_COMMAND));
        }
        // A flag planted, and the arrow up: on your feet.
        Relic::RallyPoint => {
            s.rect_filled(b.rect(0.10, 0.88, 0.40, 0.96), b.px(0.02), GUN_STEEL);
            s.line_segment([b.at(0.25, 0.04), b.at(0.25, 0.90)], st(0.06, GUN_STEEL));
            s.fill(
                b.poly(&[(0.28, 0.06), (0.90, 0.22), (0.28, 0.40)]),
                RIM_COMMAND,
            );
            s.path(
                b.poly(&[(0.46, 0.76), (0.64, 0.56), (0.82, 0.76)]),
                st(0.08, HEAL),
            );
        }
        // A warning sign, and a coin for going in anyway.
        Relic::HazardPay => {
            s.fill(
                b.poly(&[(0.38, 0.04), (0.74, 0.66), (0.02, 0.66)]),
                FUSE_CAP,
            );
            s.rect_filled(b.rect(0.35, 0.24, 0.41, 0.48), 0.0, PLATE);
            s.circle_filled(b.at(0.38, 0.56), b.px(0.04), PLATE);
            coin(s, b, 0.70, 0.72, 0.24);
        }
        // A paper with a seal on it.
        Relic::TradeLicense => {
            s.rect_filled(b.rect(0.16, 0.04, 0.78, 0.94), b.px(0.03), TOFU);
            for y in [0.20, 0.32, 0.44, 0.56] {
                s.line_segment([b.at(0.26, y), b.at(0.68, y)], st(0.035, GUN_STEEL));
            }
            s.circle_filled(b.at(0.70, 0.78), b.px(0.18), CROSS);
            s.circle_stroke(b.at(0.70, 0.78), b.px(0.11), st(0.03, TOFU));
        }
        // A crate, and the arrow round it: again.
        Relic::RestockCodes => {
            s.rect_filled(b.rect(0.20, 0.42, 0.80, 0.92), b.px(0.04), STOCK);
            s.rect_filled(b.rect(0.20, 0.42, 0.80, 0.52), b.px(0.03), SACK_DARK);
            s.line_segment([b.at(0.50, 0.52), b.at(0.50, 0.92)], st(0.04, SACK_DARK));
            s.path(b.arc(0.50, 0.46, 0.40, 190.0, 330.0), st(0.07, KEY_TRACE));
            s.fill(
                b.poly(&[(0.99, 0.38), (0.94, 0.20), (0.76, 0.30)]),
                KEY_TRACE,
            );
        }
        // A gear, and the coin it fetches.
        Relic::PartsBroker => {
            for i in 0..8 {
                let a = i as f32 * 45.0;
                let (x0, y0) = polar_at(0.40, 0.40, 0.24, a - 13.0);
                let (x1, y1) = polar_at(0.40, 0.40, 0.24, a + 13.0);
                let (x2, y2) = polar_at(0.40, 0.40, 0.37, a + 9.0);
                let (x3, y3) = polar_at(0.40, 0.40, 0.37, a - 9.0);
                s.fill(b.poly(&[(x0, y0), (x1, y1), (x2, y2), (x3, y3)]), GUN_STEEL);
            }
            s.circle_filled(b.at(0.40, 0.40), b.px(0.26), GUN_STEEL);
            s.circle_filled(b.at(0.40, 0.40), b.px(0.09), PLATE);
            coin(s, b, 0.72, 0.74, 0.22);
        }
        // An hourglass with its sand held high: an ability that lasts.
        Relic::StrongWill => {
            s.rect_filled(b.rect(0.20, 0.06, 0.80, 0.15), b.px(0.02), GUN_STEEL);
            s.rect_filled(b.rect(0.20, 0.85, 0.80, 0.94), b.px(0.02), GUN_STEEL);
            s.fill(b.poly(&[(0.27, 0.15), (0.73, 0.15), (0.50, 0.50)]), LENS);
            s.fill(b.poly(&[(0.50, 0.50), (0.73, 0.85), (0.27, 0.85)]), LENS);
            s.fill(
                b.poly(&[(0.33, 0.20), (0.67, 0.20), (0.50, 0.46)]),
                KEY_TRACE,
            );
            s.fill(
                b.poly(&[(0.50, 0.72), (0.62, 0.85), (0.38, 0.85)]),
                KEY_TRACE,
            );
            s.line_segment([b.at(0.50, 0.50), b.at(0.50, 0.72)], st(0.02, KEY_TRACE));
        }
        // A machine down a leg with its chassis cracked through.
        Relic::TotalTeardown => {
            machine(s, b, [false, true], false);
            s.path(
                b.poly(&[
                    (0.30, 0.36),
                    (0.44, 0.44),
                    (0.38, 0.50),
                    (0.56, 0.54),
                    (0.50, 0.60),
                    (0.70, 0.62),
                ]),
                st(0.05, BEAM),
            );
            for (x, y) in [(0.30, 0.84), (0.44, 0.80), (0.24, 0.72)] {
                s.line_segment([b.at(0.38, 0.64), b.at(x, y)], st(0.035, BEAM));
            }
        }
        // A strongbox, banded and locked.
        Relic::WarChest => {
            s.rect_filled(b.rect(0.08, 0.40, 0.92, 0.88), b.px(0.04), STOCK);
            s.rect_filled(b.rect(0.08, 0.18, 0.92, 0.40), b.px(0.10), LID);
            s.line_segment([b.at(0.08, 0.40), b.at(0.92, 0.40)], st(0.03, GOLD_DARK));
            s.rect_filled(b.rect(0.20, 0.18, 0.28, 0.88), 0.0, GOLD);
            s.rect_filled(b.rect(0.72, 0.18, 0.80, 0.88), 0.0, GOLD);
            s.rect_filled(b.rect(0.42, 0.34, 0.58, 0.54), b.px(0.03), GOLD);
            s.circle_filled(b.at(0.50, 0.44), b.px(0.03), PLATE);
        }
    }
}

/// A machine, face on: head and eye, body, arms, and the legs asked for
/// — a relic of the Dismantler's is about taking them off. `eye_lit`
/// false is one gone dark.
fn machine(s: &mut Sketch, b: &Box_, legs: [bool; 2], eye_lit: bool) {
    s.rect_filled(b.rect(0.38, 0.06, 0.62, 0.28), b.px(0.03), MACHINE);
    s.circle_filled(
        b.at(0.50, 0.17),
        b.px(0.045),
        if eye_lit { EYE } else { GUN_DARK },
    );
    s.rect_filled(b.rect(0.30, 0.30, 0.70, 0.62), b.px(0.04), MACHINE);
    s.rect_filled(b.rect(0.16, 0.32, 0.27, 0.60), b.px(0.03), MACHINE);
    s.rect_filled(b.rect(0.73, 0.32, 0.84, 0.60), b.px(0.03), MACHINE);
    if legs[0] {
        s.rect_filled(b.rect(0.33, 0.64, 0.46, 0.94), b.px(0.03), MACHINE);
    }
    if legs[1] {
        s.rect_filled(b.rect(0.54, 0.64, 0.67, 0.94), b.px(0.03), MACHINE);
    }
}

/// A gold coin about `(x, y)`.
fn coin(s: &mut Sketch, b: &Box_, x: f32, y: f32, r: f32) {
    s.circle_filled(b.at(x, y), b.px(r), GOLD);
    s.circle_stroke(
        b.at(x, y),
        b.px(r * 0.66),
        Stroke::new(b.px(0.03), GOLD_DARK),
    );
}

/// A point `r` from the middle of the box at `degrees`, in fractions.
fn polar(r: f32, degrees: f32) -> (f32, f32) {
    polar_at(0.5, 0.5, r, degrees)
}

fn polar_at(x: f32, y: f32, r: f32, degrees: f32) -> (f32, f32) {
    let a = degrees.to_radians();
    (x + r * a.cos(), y + r * a.sin())
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

/// The armour, seen from the front: a suit that covers the whole body —
/// a plate over the chest with a ridge down it and a gorget at the neck,
/// a plate on each shoulder, a belt, and a guard over each thigh with a
/// band across it. Every piece convex.
fn armour_suit(s: &mut Sketch, b: &Box_) {
    // The chest plate, wider at the shoulders than the waist.
    s.add(egui::Shape::convex_polygon(
        b.poly(&[(0.26, 0.22), (0.74, 0.22), (0.70, 0.58), (0.30, 0.58)]),
        ARMOUR,
        Stroke::NONE,
    ));
    s.line_segment(
        [b.at(0.50, 0.30), b.at(0.50, 0.54)],
        Stroke::new(b.px(0.04), SHINE),
    );
    // The gorget at the neck and a plate on each shoulder.
    s.rect_filled(b.rect(0.40, 0.15, 0.60, 0.24), b.px(0.04), ARMOUR_TRIM);
    for x in [0.10, 0.68] {
        s.rect_filled(b.rect(x, 0.18, x + 0.22, 0.36), b.px(0.07), ARMOUR_TRIM);
        s.rect_filled(b.rect(x, 0.30, x + 0.22, 0.36), b.px(0.03), SHADE);
    }
    // The belt, then a guard over each thigh with a band across it.
    s.rect_filled(b.rect(0.28, 0.57, 0.72, 0.64), b.px(0.02), ARMOUR_TRIM);
    for x in [0.30, 0.52] {
        s.rect_filled(b.rect(x, 0.64, x + 0.18, 0.88), b.px(0.04), ARMOUR);
        s.rect_filled(b.rect(x, 0.73, x + 0.18, 0.78), 0.0, ARMOUR_TRIM);
    }
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

/// The side of a relic's plate in a list, in points: bigger than a
/// word's [`INLINE`], since the plate is the relic's face and sits beside
/// its name and its line.
pub const RELIC: f32 = 30.0;

/// A relic's plate as one cell of a row, [`RELIC`] square.
pub fn relic_cell(ui: &mut egui::Ui, which: Relic) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(RELIC, RELIC), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        relic(ui.painter(), rect, which);
    }
    response
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

    /// A convex shape filled, no edge.
    fn fill(&mut self, points: Vec<Pos2>, fill: Color32) {
        self.add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    /// An open line through the points.
    fn path(&mut self, points: Vec<Pos2>, stroke: Stroke) {
        self.add(egui::Shape::line(points, stroke));
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

    /// Points along a circle about `(x, y)` of radius `r`, from `from` to
    /// `to` degrees — nought is to the right and the angle turns
    /// clockwise, the way the screen's y runs down.
    fn arc(&self, x: f32, y: f32, r: f32, from: f32, to: f32) -> Vec<Pos2> {
        const STEPS: usize = 16;
        (0..=STEPS)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / STEPS as f32).to_radians();
                self.at(x + r * a.cos(), y + r * a.sin())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::{Piece, Tier, WeaponKind};

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
            let mut piece = Piece::new(1, kind, Tier::One);
            icon(&painter, rect, Item::Armour(piece));
            piece.health = 0.0;
            icon(&painter, rect, Item::Armour(piece));
        }
        for &weapon in WeaponKind::ALL.iter() {
            icon(&painter, rect, Item::Weapon(weapon.basic()));
        }
        icon(&painter, rect, Item::Stack(u32::MAX));
        for kind in bims::module::ModuleKind::ALL {
            for tier in Tier::ALL {
                icon(&painter, rect, Item::Module(kind.at(tier)));
            }
        }
    }

    /// Every relic has a picture, and they are not one another's: no two
    /// draw the same shapes.
    #[test]
    fn every_relic_has_an_icon_of_its_own() {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(30.0, 30.0));
        let drawn: Vec<String> = Relic::ALL
            .iter()
            .map(|&r| {
                let mut s = Sketch::default();
                draw_relic(&mut s, &Box_::new(rect), r);
                assert!(!s.shapes.is_empty(), "{r:?} draws nothing");
                format!("{:?}", s.shapes)
            })
            .collect();
        for (i, a) in drawn.iter().enumerate() {
            for b in &drawn[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}

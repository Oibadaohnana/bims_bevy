//! A picture for every class ability: what the boxes on the hero panel
//! and the rows of the Skills tab show.
//!
//! Each class has a **family of colours** and every one of its abilities
//! is drawn in it — the soldier's blues, the engineer's ambers, the
//! medic's greens, the tank's reds and the commander's violets — so a row
//! of boxes says whose they are before a picture is read. Every picture
//! is the same build: a plate shaded from its family's dark at the top to
//! its deepest at the foot, a soft glow behind the middle, a rim, and the
//! ability's own figure over it with a shadow under it. Only the figure
//! is the ability's own, so no two pictures draw the same shapes
//! (`every_glyph_is_its_own`).
//!
//! Drawn with egui's shapes, in fractions of the box, so a picture comes
//! out the size of whatever it is painted into; a shape that is not
//! convex is built out of convex ones, since egui fills only those. No
//! text: the default font has few glyphs (see the root notes).

use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, pos2, vec2};

/// One ability's picture. Named after the ability, never the key, since a
/// slot's key can be bound elsewhere.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    // The soldier's (task 124).
    FragGrenade,
    WeakSpot,
    StunShot,
    Rampage,
    // The engineer's (task 127).
    Mine,
    HealingSentry,
    Satchel,
    Sentry,
    // The medic's (task 153), and his carry (feature 86).
    HealDrone,
    Triage,
    HealBeam,
    HealingCircle,
    Carry,
    // The tank's (task 155).
    RiotShield,
    Plated,
    Reflect,
    Bastion,
    // The commander's (task 129).
    BattleCry,
    Medivac,
    Rally,
    Reinforcements,
}

impl Glyph {
    #[cfg(test)]
    pub const ALL: [Glyph; 21] = [
        Glyph::FragGrenade,
        Glyph::WeakSpot,
        Glyph::StunShot,
        Glyph::Rampage,
        Glyph::Mine,
        Glyph::HealingSentry,
        Glyph::Satchel,
        Glyph::Sentry,
        Glyph::HealDrone,
        Glyph::Triage,
        Glyph::HealBeam,
        Glyph::HealingCircle,
        Glyph::Carry,
        Glyph::RiotShield,
        Glyph::Plated,
        Glyph::Reflect,
        Glyph::Bastion,
        Glyph::BattleCry,
        Glyph::Medivac,
        Glyph::Rally,
        Glyph::Reinforcements,
    ];

    /// The picture of a class's ability slot, Q C E R as 0 to 3. `None`
    /// for a class with no kit and a slot past the four.
    pub fn of(class: world::Class, slot: u8) -> Option<Glyph> {
        use world::Class;
        let four = match class {
            Class::None => return None,
            Class::Soldier => [
                Glyph::FragGrenade,
                Glyph::WeakSpot,
                Glyph::StunShot,
                Glyph::Rampage,
            ],
            Class::Engineer => [
                Glyph::Mine,
                Glyph::HealingSentry,
                Glyph::Satchel,
                Glyph::Sentry,
            ],
            Class::Medic => [
                Glyph::HealDrone,
                Glyph::Triage,
                Glyph::HealBeam,
                Glyph::HealingCircle,
            ],
            Class::Tank => [
                Glyph::RiotShield,
                Glyph::Plated,
                Glyph::Reflect,
                Glyph::Bastion,
            ],
            Class::Commander => [
                Glyph::BattleCry,
                Glyph::Medivac,
                Glyph::Rally,
                Glyph::Reinforcements,
            ],
        };
        four.get(usize::from(slot)).copied()
    }

    /// Its class's colour, a figure's body's: what a mark on the deck
    /// for the ability — its reach while it is aimed — is drawn in.
    pub fn colour(self) -> Color32 {
        self.family().bright
    }

    /// Whose colours it is drawn in.
    fn family(self) -> Ink {
        use Glyph::*;
        match self {
            FragGrenade | WeakSpot | StunShot | Rampage => SOLDIER,
            Mine | HealingSentry | Satchel | Sentry => ENGINEER,
            HealDrone | Triage | HealBeam | HealingCircle | Carry => MEDIC,
            RiotShield | Plated | Reflect | Bastion => TANK,
            BattleCry | Medivac | Rally | Reinforcements => COMMANDER,
        }
    }

    /// Whether its box counts a stock of charges (task 127): the count
    /// sits on a disc of its own, with the ring of the next one coming
    /// back round it.
    pub fn is_stock(self) -> bool {
        matches!(
            self,
            Glyph::FragGrenade | Glyph::Mine | Glyph::HealingSentry | Glyph::Satchel
        )
    }
}

/// A class's family of colours, darkest to lightest.
#[derive(Clone, Copy)]
struct Ink {
    /// The plate's foot, and the shadow under a figure.
    deep: Color32,
    /// The plate's head, and a figure's shaded parts.
    dark: Color32,
    /// A figure's body.
    mid: Color32,
    /// A figure's lit parts.
    bright: Color32,
    /// Its hottest: a spark, a lens, an edge catching the light.
    glow: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The soldier's blues.
const SOLDIER: Ink = Ink {
    deep: rgb(0x07, 0x11, 0x26),
    dark: rgb(0x17, 0x33, 0x66),
    mid: rgb(0x2f, 0x70, 0xd8),
    bright: rgb(0x72, 0xb4, 0xff),
    glow: rgb(0xdc, 0xee, 0xff),
};
/// The engineer's ambers.
const ENGINEER: Ink = Ink {
    deep: rgb(0x22, 0x10, 0x03),
    dark: rgb(0x5e, 0x30, 0x08),
    mid: rgb(0xd8, 0x76, 0x18),
    bright: rgb(0xff, 0xb0, 0x44),
    glow: rgb(0xff, 0xec, 0xc4),
};
/// The medic's greens.
const MEDIC: Ink = Ink {
    deep: rgb(0x03, 0x1f, 0x15),
    dark: rgb(0x0b, 0x4f, 0x34),
    mid: rgb(0x1c, 0xa8, 0x64),
    bright: rgb(0x5c, 0xf0, 0xa0),
    glow: rgb(0xdc, 0xff, 0xec),
};
/// The tank's reds.
const TANK: Ink = Ink {
    deep: rgb(0x24, 0x06, 0x08),
    dark: rgb(0x60, 0x13, 0x18),
    mid: rgb(0xc8, 0x32, 0x3c),
    bright: rgb(0xff, 0x6e, 0x66),
    glow: rgb(0xff, 0xdf, 0xda),
};
/// The commander's violets.
const COMMANDER: Ink = Ink {
    deep: rgb(0x16, 0x09, 0x30),
    dark: rgb(0x3c, 0x1e, 0x74),
    mid: rgb(0x7c, 0x4a, 0xdc),
    bright: rgb(0xb8, 0x96, 0xff),
    glow: rgb(0xf2, 0xe8, 0xff),
};

/// An ability's picture over the whole of `rect`, plate and all. `lit`
/// is the ability running now — its glow burns brighter. `corner` is the
/// plate's rounding, the box's own, so the box's frame sits on its edge.
pub fn paint(painter: &egui::Painter, rect: Rect, glyph: Glyph, lit: bool, corner: f32) {
    let ink = glyph.family();
    let mut shapes = Vec::new();
    plate(&mut shapes, rect, corner, ink, lit);
    // The figure a little in from the plate's edge, its shadow first.
    let inside = rect.shrink(rect.width().min(rect.height()) * 0.12);
    let side = inside.width().min(inside.height());
    let shadow = Ink {
        deep: SHADOW,
        dark: SHADOW,
        mid: SHADOW,
        bright: SHADOW,
        glow: SHADOW,
    };
    let mut pen = Pen::new(
        inside.translate(vec2(side * 0.025, side * 0.04)),
        shadow,
        &mut shapes,
    );
    figure(&mut pen, glyph);
    let mut pen = Pen::new(inside, ink, &mut shapes);
    figure(&mut pen, glyph);
    painter.extend(shapes);
}

/// The shadow under a figure: its family's deepest would lose it on the
/// plate's foot, so it is a black that lets some through.
const SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 150);

/// The plate: shaded top to foot, a glow in the middle, a light along
/// its head and a rim.
fn plate(shapes: &mut Vec<egui::Shape>, rect: Rect, corner: f32, ink: Ink, lit: bool) {
    let outline = rounded(rect, corner);
    let mut mesh = egui::Mesh::default();
    let shade = |y: f32| {
        let t = ((y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        mix(ink.dark, ink.deep, t)
    };
    mesh.colored_vertex(rect.center(), shade(rect.center().y));
    for &at in &outline {
        mesh.colored_vertex(at, shade(at.y));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    shapes.push(egui::Shape::mesh(mesh));
    // The glow behind the figure, stronger while the ability runs.
    let side = rect.width().min(rect.height());
    let middle = rect.center() - vec2(0.0, side * 0.04);
    let (colour, strength) = if lit {
        (ink.bright, 0.75)
    } else {
        (ink.mid, 0.55)
    };
    glow(shapes, middle, side * 0.46, colour.gamma_multiply(strength));
    // A light along the head of the plate.
    let head = [
        pos2(rect.left() + corner, rect.top() + 1.0),
        pos2(rect.right() - corner, rect.top() + 1.0),
    ];
    shapes.push(egui::Shape::line_segment(
        head,
        Stroke::new(1.0, ink.glow.gamma_multiply(0.22)),
    ));
    let mut rim = outline.clone();
    rim.push(outline[0]);
    shapes.push(egui::Shape::line(
        rim,
        Stroke::new(
            1.0,
            if lit { ink.bright } else { ink.mid }.gamma_multiply(0.7),
        ),
    ));
}

/// A soft round light: its colour at the middle, nothing at `radius`.
fn glow(shapes: &mut Vec<egui::Shape>, at: Pos2, radius: f32, colour: Color32) {
    const STEPS: u32 = 32;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(at, colour);
    // Half-way out it is still a third of itself, so the light has a
    // body and not only a point.
    for (ring, share) in [(0.5, 0.35), (1.0, 0.0)] {
        for i in 0..STEPS {
            let a = i as f32 / STEPS as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(
                at + vec2(a.cos(), a.sin()) * radius * ring,
                colour.gamma_multiply(share),
            );
        }
    }
    for i in 0..STEPS {
        let j = (i + 1) % STEPS;
        mesh.add_triangle(0, 1 + i, 1 + j);
        let (a, b) = (1 + i, 1 + j);
        let (c, d) = (1 + STEPS + i, 1 + STEPS + j);
        mesh.add_triangle(a, c, d);
        mesh.add_triangle(a, d, b);
    }
    shapes.push(egui::Shape::mesh(mesh));
}

/// The outline of a rounded rectangle, clockwise from the top left.
fn rounded(rect: Rect, corner: f32) -> Vec<Pos2> {
    const STEPS: usize = 5;
    let r = corner.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (pos2(rect.left() + r, rect.top() + r), 180.0_f32),
        (pos2(rect.right() - r, rect.top() + r), 270.0),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
    ];
    let mut points = Vec::new();
    for (centre, from) in corners {
        for i in 0..=STEPS {
            let a = (from + 90.0 * i as f32 / STEPS as f32).to_radians();
            points.push(centre + vec2(a.cos(), a.sin()) * r);
        }
    }
    points
}

/// Two colours mixed, `t` of the way from `a` to `b`.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

/// What a figure is drawn with: its box as a unit square, its family's
/// colours, and the shapes it adds to.
struct Pen<'a> {
    rect: Rect,
    k: Ink,
    shapes: &'a mut Vec<egui::Shape>,
}

impl<'a> Pen<'a> {
    fn new(rect: Rect, k: Ink, shapes: &'a mut Vec<egui::Shape>) -> Pen<'a> {
        let side = rect.width().min(rect.height());
        Pen {
            rect: Rect::from_center_size(rect.center(), vec2(side, side)),
            k,
            shapes,
        }
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.rect.min.x + self.rect.width() * x,
            self.rect.min.y + self.rect.height() * y,
        )
    }

    fn px(&self, fraction: f32) -> f32 {
        self.rect.width() * fraction
    }

    /// A convex shape, filled.
    fn poly(&mut self, points: &[(f32, f32)], fill: Color32) {
        let points = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.shapes
            .push(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    /// A convex shape, filled and edged.
    fn poly_edged(&mut self, points: &[(f32, f32)], fill: Color32, width: f32, edge: Color32) {
        let points = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        let stroke = Stroke::new(self.px(width), edge);
        self.shapes
            .push(egui::Shape::convex_polygon(points, fill, stroke));
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, round: f32, fill: Color32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        let round = self.px(round);
        self.shapes
            .push(egui::Shape::rect_filled(rect, round, fill));
    }

    fn circle(&mut self, x: f32, y: f32, r: f32, fill: Color32) {
        let (at, r) = (self.at(x, y), self.px(r));
        self.shapes.push(egui::Shape::circle_filled(at, r, fill));
    }

    fn ring(&mut self, x: f32, y: f32, r: f32, width: f32, colour: Color32) {
        let (at, r) = (self.at(x, y), self.px(r));
        let stroke = Stroke::new(self.px(width), colour);
        self.shapes.push(egui::Shape::circle_stroke(at, r, stroke));
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, colour: Color32) {
        let points = [self.at(x0, y0), self.at(x1, y1)];
        let stroke = Stroke::new(self.px(width), colour);
        self.shapes.push(egui::Shape::line_segment(points, stroke));
    }

    /// An open line through the points.
    fn path(&mut self, points: &[(f32, f32)], width: f32, colour: Color32) {
        let points = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        let stroke = Stroke::new(self.px(width), colour);
        self.shapes.push(egui::Shape::line(points, stroke));
    }

    /// Points along a circle about `(x, y)`, from `from` to `to` degrees:
    /// nought to the right, turning clockwise as the screen's y runs down.
    fn arc_points(x: f32, y: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
        const STEPS: usize = 18;
        (0..=STEPS)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / STEPS as f32).to_radians();
                (x + r * a.cos(), y + r * a.sin())
            })
            .collect()
    }

    /// An arc of a circle, stroked.
    #[allow(clippy::too_many_arguments)]
    fn arc(&mut self, x: f32, y: f32, r: f32, from: f32, to: f32, width: f32, colour: Color32) {
        self.path(&Pen::arc_points(x, y, r, from, to), width, colour);
    }

    /// A slice of a circle, filled: convex while it is half or less.
    fn pie(&mut self, x: f32, y: f32, r: f32, from: f32, to: f32, fill: Color32) {
        let mut points = Pen::arc_points(x, y, r, from, to);
        if (to - from).abs() < 180.0 {
            points.push((x, y));
        }
        self.poly(&points, fill);
    }

    /// A four-pointed spark.
    fn spark(&mut self, x: f32, y: f32, r: f32, colour: Color32) {
        let w = r * 0.28;
        self.poly(&[(x, y - r), (x + w, y), (x, y + r), (x - w, y)], colour);
        self.poly(&[(x - r, y), (x, y - w), (x + r, y), (x, y + w)], colour);
    }

    /// A star of `points` spikes, each a triangle out of a round middle.
    #[allow(clippy::too_many_arguments)]
    fn star(&mut self, x: f32, y: f32, points: u32, outer: f32, inner: f32, turn: f32, c: Color32) {
        let step = 360.0 / points as f32;
        for i in 0..points {
            let a = (turn + step * i as f32).to_radians();
            let b = (turn + step * (i as f32 - 0.5)).to_radians();
            let d = (turn + step * (i as f32 + 0.5)).to_radians();
            self.poly(
                &[
                    (x + outer * a.cos(), y + outer * a.sin()),
                    (x + inner * d.cos(), y + inner * d.sin()),
                    (x, y),
                    (x + inner * b.cos(), y + inner * b.sin()),
                ],
                c,
            );
        }
    }

    /// A plus sign: a medic's cross.
    fn plus(&mut self, x: f32, y: f32, r: f32, colour: Color32) {
        let w = r * 0.36;
        self.rect(x - w, y - r, x + w, y + r, w * 0.4, colour);
        self.rect(x - r, y - w, x + r, y + w, w * 0.4, colour);
    }

    /// A bullet flying along `degrees`, its tip at `(x, y)`, a streak
    /// behind it.
    fn bullet(&mut self, x: f32, y: f32, degrees: f32, len: f32, w: f32) {
        let a = degrees.to_radians();
        let (dx, dy) = (a.cos(), a.sin());
        let (nx, ny) = (-dy, dx);
        let at =
            |along: f32, across: f32| (x + dx * along + nx * across, y + dy * along + ny * across);
        let body = [
            at(-len * 0.45, -w),
            at(-len * 0.45, w),
            at(-len, w),
            at(-len, -w),
        ];
        let tip = [at(-len * 0.45, -w), at(0.0, 0.0), at(-len * 0.45, w)];
        let (bright, glow, mid) = (self.k.bright, self.k.glow, self.k.mid);
        let (s0, s1) = (at(-len * 1.1, 0.0), at(-len * 2.0, 0.0));
        self.line(s0.0, s0.1, s1.0, s1.1, w * 1.2, mid.gamma_multiply(0.7));
        self.poly(&body, bright);
        self.poly(&tip, glow);
    }

    /// A Bim's head and shoulders from the front, standing on `foot`.
    fn bust(
        &mut self,
        x: f32,
        foot: f32,
        size: f32,
        body: Color32,
        helmet: Color32,
        visor: Color32,
    ) {
        self.pie(x, foot, size * 0.62, 180.0, 360.0, body);
        self.circle(x, foot - size * 0.78, size * 0.30, body);
        self.pie(x, foot - size * 0.80, size * 0.36, 180.0, 360.0, helmet);
        self.rect(
            x - size * 0.42,
            foot - size * 0.82,
            x + size * 0.42,
            foot - size * 0.74,
            0.0,
            helmet,
        );
        self.rect(
            x - size * 0.2,
            foot - size * 0.70,
            x + size * 0.2,
            foot - size * 0.63,
            size * 0.03,
            visor,
        );
    }
}

/// An ability's own figure.
fn figure(p: &mut Pen, glyph: Glyph) {
    let k = p.k;
    match glyph {
        // --- the soldier's blues ---
        // A frag grenade, its fuse fizzing: a ribbed shell, the neck,
        // the lever down its side and the pin's ring.
        Glyph::FragGrenade => {
            let (cx, cy, r) = (0.46, 0.60, 0.27);
            p.circle(cx, cy, r, k.mid);
            p.pie(cx, cy, r, 200.0, 290.0, k.bright.gamma_multiply(0.55));
            for dy in [-0.09_f32, 0.09] {
                let half = (r * r - dy * dy).sqrt();
                p.line(cx - half, cy + dy, cx + half, cy + dy, 0.03, k.dark);
            }
            for dx in [-0.09_f32, 0.09] {
                let half = (r * r - dx * dx).sqrt();
                p.line(cx + dx, cy - half, cx + dx, cy + half, 0.03, k.dark);
            }
            p.circle(cx - 0.10, cy - 0.11, 0.05, k.glow.gamma_multiply(0.8));
            p.rect(0.37, 0.25, 0.55, 0.36, 0.02, k.bright);
            p.path(&[(0.54, 0.28), (0.70, 0.31), (0.74, 0.50)], 0.06, k.bright);
            p.ring(0.31, 0.24, 0.07, 0.03, k.glow);
            p.spark(0.80, 0.16, 0.11, k.glow);
            p.circle(0.80, 0.16, 0.03, k.glow);
        }
        // Weak Spot: a cracked plate of armour in the crosshairs.
        Glyph::WeakSpot => {
            let hex: Vec<(f32, f32)> = (0..6)
                .map(|i| {
                    let a = (30.0 + 60.0 * i as f32).to_radians();
                    (0.5 + 0.25 * a.cos(), 0.5 + 0.25 * a.sin())
                })
                .collect();
            p.poly_edged(&hex, k.dark, 0.03, k.mid);
            p.path(
                &[(0.50, 0.50), (0.40, 0.40), (0.44, 0.34), (0.36, 0.27)],
                0.03,
                k.glow,
            );
            p.path(&[(0.50, 0.50), (0.62, 0.56), (0.60, 0.66)], 0.03, k.glow);
            p.path(&[(0.50, 0.50), (0.56, 0.38)], 0.025, k.bright);
            p.ring(0.5, 0.5, 0.36, 0.045, k.bright);
            for (x0, y0, x1, y1) in [
                (0.50, 0.04, 0.50, 0.26),
                (0.50, 0.74, 0.50, 0.96),
                (0.04, 0.50, 0.26, 0.50),
                (0.74, 0.50, 0.96, 0.50),
            ] {
                p.line(x0, y0, x1, y1, 0.055, k.glow);
            }
            p.circle(0.5, 0.5, 0.07, k.glow);
            p.circle(0.5, 0.5, 0.035, k.bright);
        }
        // Stun Shot (October 2026): a rifle's muzzle, the charged slug
        // leaving it, and the stun bursting where it lands.
        Glyph::StunShot => {
            p.star(0.68, 0.36, 10, 0.30, 0.17, 0.0, k.mid);
            p.ring(0.68, 0.36, 0.22, 0.04, k.bright);
            p.circle(0.68, 0.36, 0.14, k.dark);
            p.circle(0.68, 0.36, 0.10, k.glow);
            p.circle(0.68, 0.36, 0.05, k.bright);
            p.line(0.40, 0.66, 0.60, 0.44, 0.06, k.glow.gamma_multiply(0.7));
            p.poly(
                &[(0.04, 0.70), (0.18, 0.66), (0.20, 0.84), (0.06, 0.92)],
                k.mid,
            );
            p.rect(0.16, 0.66, 0.42, 0.80, 0.02, k.mid);
            p.rect(0.16, 0.66, 0.42, 0.70, 0.01, k.bright);
            p.line(0.40, 0.70, 0.50, 0.62, 0.05, k.bright);
            p.circle(0.50, 0.62, 0.04, k.glow);
        }
        // Rampage: bullets pouring out of a muzzle's blaze.
        Glyph::Rampage => {
            p.star(0.30, 0.70, 8, 0.26, 0.09, -20.0, k.mid);
            p.star(0.30, 0.70, 8, 0.17, 0.07, 2.5, k.bright);
            p.circle(0.30, 0.70, 0.08, k.glow);
            p.bullet(0.92, 0.10, -45.0, 0.22, 0.045);
            p.bullet(0.94, 0.40, -25.0, 0.20, 0.045);
            p.bullet(0.62, 0.06, -65.0, 0.20, 0.045);
        }
        // --- the engineer's ambers ---
        // Mine (task 154): a squat disc on the deck, its pressure prongs
        // up and its armed light lit, the blast's ring round it.
        Glyph::Mine => {
            p.arc(0.5, 0.62, 0.42, 200.0, 340.0, 0.035, k.mid);
            p.circle(0.5, 0.70, 0.32, k.dark);
            p.circle(0.5, 0.64, 0.30, k.mid);
            p.ring(0.5, 0.64, 0.30, 0.04, k.bright);
            p.circle(0.5, 0.62, 0.15, k.dark);
            for x in [0.38, 0.5, 0.62] {
                p.line(x, 0.58, x, 0.44, 0.045, k.bright);
                p.circle(x, 0.43, 0.035, k.bright);
            }
            p.circle(0.5, 0.66, 0.06, k.glow);
            p.spark(0.14, 0.30, 0.07, k.glow);
            p.spark(0.86, 0.26, 0.06, k.bright);
        }
        // Healing Sentry: a domed post on three legs, a cross on its
        // face and the mending rising off it.
        Glyph::HealingSentry => {
            p.path(&[(0.28, 0.90), (0.50, 0.64), (0.72, 0.90)], 0.05, k.dark);
            p.line(0.50, 0.64, 0.50, 0.92, 0.05, k.dark);
            p.pie(0.50, 0.60, 0.27, 180.0, 360.0, k.mid);
            p.pie(0.50, 0.60, 0.27, 200.0, 250.0, k.bright.gamma_multiply(0.6));
            p.rect(0.20, 0.58, 0.80, 0.68, 0.03, k.dark);
            p.rect(0.20, 0.58, 0.80, 0.61, 0.01, k.bright);
            p.plus(0.50, 0.46, 0.10, k.glow);
            p.plus(0.20, 0.24, 0.06, k.bright);
            p.plus(0.80, 0.16, 0.075, k.glow);
            p.plus(0.66, 0.06, 0.04, k.bright);
        }
        // Satchel Charge (task 154): a canvas pack with its flap and strap,
        // the detonator's light lit, and the remote's aerial calling it.
        Glyph::Satchel => {
            p.rect(0.12, 0.42, 0.72, 0.88, 0.06, k.dark);
            p.rect(0.14, 0.44, 0.70, 0.86, 0.05, k.mid);
            p.rect(0.14, 0.44, 0.70, 0.58, 0.04, k.bright.gamma_multiply(0.8));
            p.rect(0.38, 0.30, 0.46, 0.88, 0.01, k.dark);
            p.rect(0.48, 0.64, 0.64, 0.78, 0.02, k.deep);
            p.circle(0.56, 0.71, 0.04, k.glow);
            p.line(0.80, 0.86, 0.80, 0.30, 0.04, k.bright);
            p.circle(0.80, 0.28, 0.04, k.glow);
            p.arc(0.80, 0.28, 0.11, 290.0, 430.0, 0.03, k.bright);
            p.arc(0.80, 0.28, 0.19, 300.0, 420.0, 0.03, k.mid);
        }
        // Sentry: the minigun turret on its tripod, barrels hot.
        Glyph::Sentry => {
            p.path(&[(0.20, 0.92), (0.42, 0.66), (0.64, 0.92)], 0.05, k.dark);
            p.line(0.42, 0.66, 0.42, 0.94, 0.05, k.dark);
            p.pie(0.42, 0.42, 0.18, 180.0, 360.0, k.mid);
            p.rect(0.22, 0.40, 0.62, 0.66, 0.05, k.mid);
            p.rect(0.22, 0.40, 0.62, 0.45, 0.02, k.bright);
            p.circle(0.34, 0.53, 0.06, k.deep);
            p.circle(0.34, 0.53, 0.035, k.glow);
            p.rect(0.60, 0.44, 0.68, 0.62, 0.02, k.dark);
            for y in [0.47, 0.53, 0.59] {
                p.line(0.66, y, 0.86, y, 0.035, k.bright);
            }
            p.rect(0.80, 0.43, 0.84, 0.63, 0.01, k.dark);
            p.star(0.91, 0.53, 6, 0.10, 0.04, 0.0, k.glow);
        }
        // --- the medic's greens ---
        // Heal Drone (task 153): a quadcopter seen from above, a cross on
        // its back and a soft light thrown down under it.
        Glyph::HealDrone => {
            p.circle(0.5, 0.56, 0.30, k.bright.gamma_multiply(0.18));
            for (x, y) in [(0.22, 0.28), (0.78, 0.28), (0.22, 0.72), (0.78, 0.72)] {
                p.line(0.5, 0.5, x, y, 0.05, k.dark);
                p.circle(x, y, 0.14, k.bright.gamma_multiply(0.25));
                p.ring(x, y, 0.14, 0.03, k.bright);
                p.circle(x, y, 0.04, k.glow);
            }
            p.rect(0.34, 0.38, 0.66, 0.62, 0.06, k.mid);
            p.rect(0.34, 0.38, 0.66, 0.44, 0.03, k.bright.gamma_multiply(0.7));
            p.plus(0.5, 0.51, 0.10, k.glow);
        }
        // Triage (task 153): a bar nearly empty, and a big cross beside it
        // with the mending rising off it.
        Glyph::Triage => {
            p.rect(0.12, 0.14, 0.32, 0.88, 0.04, k.deep);
            p.rect(0.15, 0.70, 0.29, 0.85, 0.03, k.mid);
            p.line(0.12, 0.14, 0.32, 0.14, 0.025, k.bright);
            p.plus(0.64, 0.56, 0.24, k.mid);
            p.plus(0.64, 0.56, 0.16, k.bright);
            p.plus(0.64, 0.56, 0.08, k.glow);
            p.poly(&[(0.64, 0.06), (0.52, 0.22), (0.76, 0.22)], k.glow);
            p.plus(0.88, 0.30, 0.05, k.bright);
            p.plus(0.42, 0.24, 0.04, k.bright);
        }
        // Heal Beam: a projector at the foot, its beam coiling up into a
        // cross.
        Glyph::HealBeam => {
            let (x0, y0, x1, y1) = (0.22, 0.78, 0.74, 0.28);
            p.line(x0, y0, x1, y1, 0.13, k.bright.gamma_multiply(0.35));
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len = (dx * dx + dy * dy).sqrt();
            let (nx, ny) = (-dy / len, dx / len);
            let coil: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    let w = 0.06 * (t * std::f32::consts::TAU * 2.5).sin();
                    (x0 + dx * t + nx * w, y0 + dy * t + ny * w)
                })
                .collect();
            p.path(&coil, 0.03, k.mid);
            p.line(x0, y0, x1, y1, 0.04, k.glow);
            p.circle(0.20, 0.80, 0.13, k.dark);
            p.ring(0.20, 0.80, 0.13, 0.04, k.mid);
            p.circle(0.20, 0.80, 0.06, k.glow);
            p.circle(0.76, 0.26, 0.19, k.bright.gamma_multiply(0.3));
            p.plus(0.76, 0.26, 0.15, k.bright);
            p.plus(0.76, 0.26, 0.09, k.glow);
        }
        // Healing Circle (task 153): a ring of light on the ground round a
        // cross, the mending rising inside it and the rim burning.
        Glyph::HealingCircle => {
            p.circle(0.5, 0.5, 0.42, k.bright.gamma_multiply(0.16));
            p.ring(0.5, 0.5, 0.42, 0.045, k.bright);
            p.ring(0.5, 0.5, 0.30, 0.02, k.mid);
            for i in 0..6 {
                let a = (30.0 + 60.0 * i as f32).to_radians();
                p.spark(0.5 + 0.42 * a.cos(), 0.5 + 0.42 * a.sin(), 0.07, k.glow);
            }
            p.plus(0.5, 0.5, 0.16, k.bright);
            p.plus(0.5, 0.5, 0.09, k.glow);
            p.plus(0.30, 0.38, 0.035, k.glow);
            p.plus(0.70, 0.64, 0.035, k.glow);
            p.plus(0.66, 0.32, 0.03, k.bright);
        }
        // Carry: a stretcher, its poles out at either end and a cross on
        // its canvas.
        Glyph::Carry => {
            p.line(0.06, 0.34, 0.94, 0.34, 0.05, k.bright);
            p.line(0.06, 0.70, 0.94, 0.70, 0.05, k.bright);
            p.rect(0.22, 0.32, 0.78, 0.72, 0.03, k.mid);
            p.rect(0.22, 0.32, 0.78, 0.38, 0.02, k.bright.gamma_multiply(0.6));
            p.plus(0.5, 0.52, 0.13, k.glow);
            for x in [0.06, 0.94] {
                p.line(x, 0.30, x, 0.74, 0.05, k.dark);
            }
        }
        // --- the tank's reds ---
        // Reflect Barrier: a ring of thorns round a burning core, and
        // every arrow turned back out of it.
        Glyph::Reflect => {
            p.ring(0.5, 0.5, 0.21, 0.04, k.mid);
            p.circle(0.5, 0.5, 0.12, k.bright);
            p.circle(0.5, 0.5, 0.05, k.glow);
            for (dx, dy) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                // The shaft out from the ring, the head at the far end.
                p.line(
                    0.5 + dx * 0.24,
                    0.5 + dy * 0.24,
                    0.5 + dx * 0.36,
                    0.5 + dy * 0.36,
                    0.045,
                    k.bright,
                );
                let (nx, ny) = (-dy * 0.07, dx * 0.07);
                let (bx, by) = (0.5 + dx * 0.34, 0.5 + dy * 0.34);
                p.poly(
                    &[
                        (0.5 + dx * 0.44, 0.5 + dy * 0.44),
                        (bx + nx, by + ny),
                        (bx - nx, by - ny),
                    ],
                    k.glow,
                );
            }
            // The thorns on the ring, between the arrows.
            for (dx, dy) in [(0.0_f32, -1.0_f32), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                let (nx, ny) = (-dy * 0.035, dx * 0.035);
                p.poly(
                    &[
                        (0.5 + dx * 0.32, 0.5 + dy * 0.32),
                        (0.5 + dx * 0.21 + nx, 0.5 + dy * 0.21 + ny),
                        (0.5 + dx * 0.21 - nx, 0.5 + dy * 0.21 - ny),
                    ],
                    k.mid,
                );
            }
        }
        // Plated: a breastplate of overlapping lames, riveted.
        Glyph::Plated => {
            let body = [
                (0.20, 0.18),
                (0.80, 0.18),
                (0.88, 0.40),
                (0.74, 0.88),
                (0.26, 0.88),
                (0.12, 0.40),
            ];
            p.poly_edged(&body, k.mid, 0.035, k.bright);
            p.poly(
                &[(0.20, 0.18), (0.44, 0.18), (0.36, 0.60), (0.16, 0.44)],
                k.bright.gamma_multiply(0.45),
            );
            for y in [0.56, 0.68, 0.79] {
                let inset = (y - 0.40) * 0.3;
                p.line(0.15 + inset, y, 0.85 - inset, y, 0.035, k.dark);
                p.line(
                    0.16 + inset,
                    y + 0.025,
                    0.84 - inset,
                    y + 0.025,
                    0.015,
                    k.bright,
                );
            }
            p.line(0.5, 0.26, 0.5, 0.52, 0.03, k.glow);
            p.pie(0.5, 0.15, 0.13, 0.0, 180.0, k.deep);
            for (x, y) in [(0.24, 0.26), (0.76, 0.26), (0.20, 0.44), (0.80, 0.44)] {
                p.circle(x, y, 0.025, k.glow);
            }
        }
        // Riot Shield: a tower shield held up, shots glancing off it.
        Glyph::RiotShield => {
            p.rect(0.10, 0.88, 0.90, 0.92, 0.02, k.dark);
            let shield = [
                (0.30, 0.22),
                (0.40, 0.10),
                (0.72, 0.10),
                (0.82, 0.22),
                (0.82, 0.80),
                (0.56, 0.90),
                (0.30, 0.80),
            ];
            p.poly_edged(&shield, k.mid, 0.04, k.bright);
            p.poly(
                &[
                    (0.36, 0.22),
                    (0.43, 0.16),
                    (0.52, 0.16),
                    (0.52, 0.82),
                    (0.36, 0.76),
                ],
                k.bright.gamma_multiply(0.35),
            );
            p.line(0.56, 0.14, 0.56, 0.86, 0.03, k.dark);
            p.circle(0.56, 0.46, 0.14, k.dark);
            p.ring(0.56, 0.46, 0.14, 0.035, k.bright);
            p.circle(0.56, 0.46, 0.07, k.glow);
            for (x, y, r) in [(0.20, 0.30, 0.10), (0.16, 0.60, 0.08)] {
                p.star(x, y, 6, r, r * 0.35, 0.0, k.glow);
            }
            p.path(&[(0.02, 0.22), (0.20, 0.30), (0.06, 0.44)], 0.025, k.bright);
            p.path(&[(0.02, 0.54), (0.16, 0.60), (0.04, 0.72)], 0.025, k.bright);
        }
        // Bastion: a dome of light thrown over the crew, its rim burning.
        Glyph::Bastion => {
            p.rect(0.06, 0.86, 0.94, 0.90, 0.02, k.dark);
            p.pie(0.5, 0.88, 0.42, 180.0, 360.0, k.mid.gamma_multiply(0.35));
            p.bust(0.30, 0.88, 0.30, k.dark, k.mid, k.bright);
            p.bust(0.70, 0.88, 0.30, k.dark, k.mid, k.bright);
            p.bust(0.50, 0.88, 0.38, k.mid, k.bright, k.glow);
            p.arc(0.5, 0.88, 0.42, 180.0, 360.0, 0.05, k.glow);
            p.arc(
                0.5,
                0.88,
                0.36,
                200.0,
                250.0,
                0.03,
                k.bright.gamma_multiply(0.6),
            );
        }
        // --- the commander's violets ---
        // Battle Cry: a loud-hailer, the shout going out in rings.
        Glyph::BattleCry => {
            p.poly(
                &[(0.12, 0.44), (0.56, 0.24), (0.56, 0.78), (0.12, 0.58)],
                k.mid,
            );
            p.poly(
                &[(0.12, 0.44), (0.56, 0.24), (0.56, 0.36), (0.12, 0.48)],
                k.bright.gamma_multiply(0.6),
            );
            p.rect(0.06, 0.43, 0.14, 0.59, 0.02, k.dark);
            p.rect(0.54, 0.20, 0.62, 0.82, 0.03, k.bright);
            p.poly(
                &[(0.26, 0.60), (0.34, 0.60), (0.30, 0.78), (0.22, 0.78)],
                k.dark,
            );
            for (r, a) in [(0.16, 1.0), (0.26, 0.8), (0.36, 0.55)] {
                p.arc(0.60, 0.51, r, -42.0, 42.0, 0.05, k.glow.gamma_multiply(a));
            }
        }
        // Medivac: one helmeted Bim dropped in, a medic's cross on a
        // badge over the chest.
        Glyph::Medivac => {
            p.bust(0.50, 0.96, 0.62, k.mid, k.bright, k.glow);
            p.circle(0.50, 0.80, 0.13, k.glow);
            p.rect(0.47, 0.72, 0.53, 0.88, 0.01, k.dark);
            p.rect(0.42, 0.77, 0.58, 0.83, 0.01, k.dark);
            for y in [0.04, 0.14] {
                p.path(&[(0.40, y), (0.50, y + 0.08), (0.60, y)], 0.05, k.glow);
            }
        }
        // Rally: a swallow-tailed banner on its pole, and the crew's
        // pace quickening under it.
        Glyph::Rally => {
            p.line(0.26, 0.10, 0.26, 0.92, 0.045, k.bright);
            p.circle(0.26, 0.09, 0.045, k.glow);
            p.poly(
                &[(0.28, 0.14), (0.56, 0.18), (0.56, 0.48), (0.28, 0.46)],
                k.mid,
            );
            p.poly(
                &[(0.56, 0.18), (0.88, 0.14), (0.72, 0.32), (0.56, 0.48)],
                k.mid,
            );
            p.poly(&[(0.56, 0.48), (0.72, 0.32), (0.88, 0.50)], k.mid);
            p.poly(
                &[(0.28, 0.14), (0.56, 0.18), (0.56, 0.24), (0.28, 0.22)],
                k.bright.gamma_multiply(0.7),
            );
            p.star(0.44, 0.32, 5, 0.08, 0.035, -90.0, k.glow);
            for y in [0.62, 0.76] {
                p.path(
                    &[(0.52, y + 0.10), (0.68, y), (0.84, y + 0.10)],
                    0.055,
                    k.glow,
                );
            }
        }
        // Reinforcements: three helmeted Bims coming in, dropped from
        // above.
        Glyph::Reinforcements => {
            p.bust(0.24, 0.84, 0.36, k.dark, k.mid, k.bright);
            p.bust(0.76, 0.84, 0.36, k.dark, k.mid, k.bright);
            p.bust(0.50, 0.92, 0.46, k.mid, k.bright, k.glow);
            for y in [0.06, 0.18] {
                p.path(&[(0.40, y), (0.50, y + 0.08), (0.60, y)], 0.05, k.glow);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(glyph: Glyph) -> String {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(44.0, 44.0));
        let mut shapes = Vec::new();
        let mut pen = Pen::new(rect, glyph.family(), &mut shapes);
        figure(&mut pen, glyph);
        assert!(!shapes.is_empty(), "{glyph:?} draws nothing");
        format!("{shapes:?}")
    }

    /// Every ability has a figure, and none is another's.
    #[test]
    fn every_glyph_is_its_own() {
        let all: Vec<String> = Glyph::ALL.iter().map(|&g| drawn(g)).collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    /// Every class with a kit has a picture in each of its four slots, in
    /// its own colours, and a class's four are four different pictures.
    #[test]
    fn every_class_slot_has_a_picture_in_its_family() {
        for class in world::Class::ALL {
            let four: Vec<Option<Glyph>> = (0..4).map(|slot| Glyph::of(class, slot)).collect();
            if class == world::Class::None {
                assert!(four.iter().all(Option::is_none));
                continue;
            }
            let four: Vec<Glyph> = four.into_iter().map(Option::unwrap).collect();
            let deep = four[0].family().deep;
            assert!(four.iter().all(|g| g.family().deep == deep), "{class:?}");
            for (i, a) in four.iter().enumerate() {
                assert!(!four[i + 1..].contains(a), "{class:?}");
            }
            assert_eq!(Glyph::of(class, 4), None);
        }
    }

    /// A whole picture paints into a painter with no window behind it.
    #[test]
    fn every_picture_paints() {
        let ctx = egui::Context::default();
        let painter = egui::Painter::new(
            ctx,
            egui::LayerId::background(),
            Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0)),
        );
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(44.0, 44.0));
        for glyph in Glyph::ALL {
            paint(&painter, rect, glyph, false, 4.0);
            paint(&painter, rect, glyph, true, 4.0);
        }
    }
}

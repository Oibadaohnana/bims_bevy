//! The shape buffer, turned into triangles.
//!
//! Every painter in the workspace — the room's, the designer's, the lobby's
//! — writes the same twelve floats a shape: kind, centre, size, rotation,
//! corner radius, line width and a colour. The browser used to replay that
//! onto a canvas; this replays it into one mesh, clipped to the canvas it
//! belongs to — a Bevy mesh for the world's canvas between the panels
//! (`scene.rs`, where the bloom is) and an egui mesh for a canvas inside a
//! panel (the lobby's galaxy and diagram, the setup's portrait). The field
//! order is the one thing shared with the painters, and it is in
//! `crates/game/src/draw.rs`.
//!
//! Shapes come in **world units** under a view — a scale and an offset that
//! turn them into canvas pixels — and land in a canvas, which is a rectangle
//! of the window. Screen coordinates are the canvas's: points from the
//! window's top-left corner, y down, the way every painter and every
//! pointer reading thinks.
//!
//! Every edge is **feathered**: the way epaint draws its own shapes, and
//! the only anti-aliasing there is — egui paints into the window's
//! unsampled target, and the world's camera runs without multisampling so
//! that its picture is the one egui drew. A filled shape is drawn half a
//! pixel small with a ramp round it from its colour to nothing a pixel
//! wide; a stroke is a band with a ramp down each side; and a stroke
//! thinner than a pixel is drawn a pixel wide and that much fainter, so a
//! seam at a low zoom fades rather than flickers.
//!
//! A colour is sRGB, nought to one, as it always was — and a channel may
//! go **past one**: that is an emissive colour, brighter than white, and
//! the one thing the bloom picks up ([`Paint`]).

use bevy::prelude::*;
use bevy_egui::egui;

/// Floats per shape. The painters' `draw::STRIDE`, and asserted equal to it
/// in the tests so the two cannot drift.
pub const STRIDE: usize = 12;

const KIND_ELLIPSE: f32 = 1.0;
const KIND_TRIANGLE: f32 = 2.0;

/// How a canvas maps world units to its own pixels: `px = offset + scale *
/// world`. The room fits a fixed deck to the window, the designer pans and
/// zooms a grid, the lobby projects a galaxy itself and hands over pixels
/// with a scale of one.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub scale: f32,
    pub offset: Vec2,
}

impl View {
    pub const PIXELS: View = View {
        scale: 1.0,
        offset: Vec2::ZERO,
    };

    pub fn to_canvas(self, world: Vec2) -> Vec2 {
        self.offset + world * self.scale
    }

    pub fn to_world(self, canvas: Vec2) -> Vec2 {
        (canvas - self.offset) / self.scale
    }
}

/// A rectangle of the window, in logical points from its top-left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(min: Vec2, max: Vec2) -> Rect {
        Rect { min, max }
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x < self.max.x && p.y >= self.min.y && p.y < self.max.y
    }
}

/// A vertex's colour: **premultiplied and sRGB-encoded**, what an egui
/// vertex carries, as floats rather than bytes. A colour at or under white
/// is made exactly as egui made it — through `Color32`, rounded to the
/// byte — so the world's canvas, which Bevy draws now, is the picture egui
/// drew to the bit. A colour with a channel **past one** is emissive: it
/// is kept as the float it is, premultiplied the same way, and the
/// canvas's shader carries the sRGB curve on past white for it
/// (`canvas.wgsl`), which is what lifts it over the bloom's threshold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint(pub [f32; 4]);

impl Paint {
    /// Nothing, premultiplied: the far side of a feather ramp, so the ramp
    /// is a fade of the shape's own colour rather than a fade to black.
    pub const CLEAR: Paint = Paint([0.0; 4]);

    /// A shape's colour as the painters write it: straight sRGB and an
    /// alpha.
    pub fn of(r: f32, g: f32, b: f32, a: f32) -> Paint {
        // `as u8` saturates, which is what egui was always handed.
        let byte = |v: f32| (v * 255.0).round() as u8;
        if r > 1.0 || g > 1.0 || b > 1.0 {
            let a = byte(a) as f32 / 255.0;
            return Paint([r.max(0.0) * a, g.max(0.0) * a, b.max(0.0) * a, a]);
        }
        Paint::from(egui::Color32::from_rgba_unmultiplied(
            byte(r),
            byte(g),
            byte(b),
            byte(a),
        ))
    }

    /// Whether a channel is brighter than white — premultiplied, brighter
    /// than its own alpha.
    pub fn is_emissive(self) -> bool {
        self.0[..3].iter().any(|&c| c > self.0[3])
    }

    /// So much fainter: `Color32::gamma_multiply`, rounding and all, for a
    /// colour egui could hold, and the plain product for one it could not.
    fn faded(self, by: f32) -> Paint {
        if self.is_emissive() {
            Paint(self.0.map(|c| c * by))
        } else {
            Paint::from(self.color32().gamma_multiply(by))
        }
    }

    /// As egui's bytes: an emissive channel held at the alpha, which is as
    /// bright as a premultiplied byte can say.
    pub fn color32(self) -> egui::Color32 {
        let [r, g, b, a] = self.0;
        let byte = |v: f32| (v * 255.0).round() as u8;
        let a = byte(a);
        let channel = |v: f32| byte(v).min(a);
        egui::Color32::from_rgba_premultiplied(channel(r), channel(g), channel(b), a)
    }
}

impl From<egui::Color32> for Paint {
    fn from(c: egui::Color32) -> Paint {
        Paint(c.to_array().map(|v| v as f32 / 255.0))
    }
}

/// Triangles with a colour at every corner: a mesh in the making, for the
/// world's canvas ([`ShapeBuf::into_parts`]) or for egui
/// ([`ShapeBuf::into_shape`]). Positions are window points, z nought; the
/// canvas's origin is added on the way in.
pub struct ShapeBuf {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    origin: Vec2,
    clip: Rect,
    /// One physical pixel, in points: the width of the ramp along every
    /// edge from a shape's colour to nothing.
    feather: f32,
}

/// What a [`ShapeBuf`] comes to for Bevy: a mesh's three arrays.
pub struct Parts {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl ShapeBuf {
    /// A buffer for a canvas at `clip`, on a display with `pixels_per_point`.
    pub fn new(clip: Rect, pixels_per_point: f32) -> ShapeBuf {
        ShapeBuf {
            positions: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            origin: clip.min,
            clip,
            feather: if pixels_per_point > 0.0 {
                1.0 / pixels_per_point
            } else {
                1.0
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The triangles as one egui shape, for a canvas inside a panel.
    pub fn into_shape(self) -> egui::Shape {
        let mut mesh = egui::Mesh {
            indices: self.indices,
            ..Default::default()
        };
        mesh.vertices = self
            .positions
            .iter()
            .zip(&self.colors)
            .map(|(p, c)| egui::epaint::Vertex {
                pos: egui::pos2(p[0], p[1]),
                uv: egui::epaint::WHITE_UV,
                color: Paint(*c).color32(),
            })
            .collect();
        egui::Shape::mesh(mesh)
    }

    /// The triangles as a mesh's arrays, for the world's canvas.
    pub fn into_parts(self) -> Parts {
        Parts {
            positions: self.positions,
            colors: self.colors,
            indices: self.indices,
        }
    }

    fn vertex(&mut self, canvas: Vec2, color: Paint) -> u32 {
        let i = self.positions.len() as u32;
        let p = canvas + self.origin;
        self.positions.push([p.x, p.y, 0.0]);
        self.colors.push(color.0);
        i
    }

    /// A convex polygon, as a fan.
    fn fan(&mut self, points: &[Vec2], color: Paint) {
        if points.len() < 3 {
            return;
        }
        let first = self.vertex(points[0], color);
        let mut prev = self.vertex(points[1], color);
        for &p in &points[2..] {
            let here = self.vertex(p, color);
            self.indices.extend_from_slice(&[first, prev, here]);
            prev = here;
        }
    }

    /// The band between two rings of the same length, each ring its own
    /// colour: an outline, or one side of a feather ramp.
    fn band(&mut self, outer: &[Vec2], oc: Paint, inner: &[Vec2], ic: Paint) {
        let n = outer.len().min(inner.len());
        if n < 2 {
            return;
        }
        let base = self.positions.len() as u32;
        for i in 0..n {
            self.vertex(outer[i], oc);
            self.vertex(inner[i], ic);
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            let (o, in_) = (base + 2 * i, base + 2 * i + 1);
            let (oj, inj) = (base + 2 * j, base + 2 * j + 1);
            self.indices.extend_from_slice(&[o, oj, inj, o, inj, in_]);
        }
    }

    /// A convex polygon, filled, with its edge feathered. `extent` is the
    /// shape's narrowest width: a shape narrower than the feather gets a
    /// ramp that narrow, so a dot is never pulled inside out.
    fn fill(&mut self, points: &[Vec2], color: Paint, extent: f32) {
        if points.len() < 3 {
            return;
        }
        let f = self.feather.min(extent.max(0.0));
        if f <= 0.0 {
            self.fan(points, color);
            return;
        }
        let normals = normals(points);
        let inner = offset(points, &normals, -f / 2.0);
        let outer = offset(points, &normals, f / 2.0);
        self.fan(&inner, color);
        self.band(&outer, Paint::CLEAR, &inner, color);
    }

    /// The band between two rings, feathered along both edges: a stroke.
    /// The rings are the stroke's own edges — the caller has already put
    /// them half a line either side of the path.
    fn stroke(&mut self, outer: &[Vec2], inner: &[Vec2], color: Paint) {
        if outer.len().min(inner.len()) < 2 {
            return;
        }
        let f = self.feather / 2.0;
        let (no, ni) = (normals(outer), normals(inner));
        let outer_out = offset(outer, &no, f);
        let outer_in = offset(outer, &no, -f);
        // The inner ring's normals point out of the hole, into the stroke.
        let inner_out = offset(inner, &ni, f);
        let inner_in = offset(inner, &ni, -f);
        self.band(&outer_out, Paint::CLEAR, &outer_in, color);
        self.band(&outer_in, color, &inner_out, color);
        self.band(&inner_out, color, &inner_in, Paint::CLEAR);
    }

    /// A stroke's width and colour as drawn: one thinner than a pixel is
    /// drawn a pixel wide and fainter by the same ratio, so it fades
    /// rather than breaking up.
    fn thin(&self, line: f32, color: Paint) -> (f32, Paint) {
        if line >= self.feather {
            (line, color)
        } else {
            (self.feather, color.faded(line / self.feather))
        }
    }

    /// Replay `shapes` — twelve floats each, in world units under `view`.
    pub fn replay(&mut self, shapes: &[f32], view: View) {
        // About what a shape comes to, so the arrays grow once rather than
        // a dozen times over a docked station's fifteen thousand.
        let reserve = shapes.len() / STRIDE * 16;
        self.positions.reserve(reserve);
        self.colors.reserve(reserve);
        self.indices.reserve(reserve * 2);
        let clip = Rect::new(Vec2::ZERO, self.clip.size());
        for s in shapes.as_chunks::<STRIDE>().0 {
            let kind = s[0];
            let centre = view.to_canvas(Vec2::new(s[1], s[2]));
            let size = Vec2::new(s[3].abs(), s[4].abs()) * view.scale;
            let rot = s[5];
            let radius = s[6] * view.scale;
            let line = s[7] * view.scale;
            let a = s[11];
            if a <= 0.0 {
                continue;
            }
            // Past the canvas by more than its own size, nothing of it can
            // show. The canvas's clip takes the rest.
            let reach = size.max_element() + line + 1.0;
            if centre.x + reach < clip.min.x
                || centre.x - reach > clip.max.x
                || centre.y + reach < clip.min.y
                || centre.y - reach > clip.max.y
            {
                continue;
            }
            let color = Paint::of(s[8], s[9], s[10], a);
            if kind == KIND_ELLIPSE {
                self.ellipse(centre, size, rot, line, color);
            } else if kind == KIND_TRIANGLE {
                self.triangle(centre, size, rot, line, color);
            } else {
                self.rect(centre, size, rot, radius, line, color);
            }
        }
    }

    fn ellipse(&mut self, centre: Vec2, size: Vec2, rot: f32, line: f32, color: Paint) {
        let r = size / 2.0;
        let n = segments(r.max_element() + line);
        if line > 0.0 {
            let (line, color) = self.thin(line, color);
            if r.min_element() <= line / 2.0 {
                // No hole left in the middle: a disc, as wide as the stroke.
                let r = r + line / 2.0;
                let pts = ring(centre, r, rot, n);
                self.fill(&pts, color, r.min_element() * 2.0);
                return;
            }
            let outer = ring(centre, r + line / 2.0, rot, n);
            let inner = ring(centre, r - line / 2.0, rot, n);
            self.stroke(&outer, &inner, color);
        } else {
            let pts = ring(centre, r, rot, n);
            self.fill(&pts, color, r.min_element() * 2.0);
        }
    }

    /// The bottom-left half of the box, as the painters define it.
    fn triangle(&mut self, centre: Vec2, size: Vec2, rot: f32, line: f32, color: Paint) {
        let half = size / 2.0;
        let pts = [
            turned(Vec2::new(-half.x, -half.y), rot) + centre,
            turned(Vec2::new(-half.x, half.y), rot) + centre,
            turned(Vec2::new(half.x, half.y), rot) + centre,
        ];
        // The narrowest way across a right triangle is the height over its
        // hypotenuse.
        let extent = if size.x > 0.0 && size.y > 0.0 {
            size.x * size.y / size.length()
        } else {
            0.0
        };
        if line > 0.0 {
            let (line, color) = self.thin(line, color);
            if extent <= line {
                self.fill(&pts, color, extent);
                return;
            }
            let normals = normals(&pts);
            let outer = offset(&pts, &normals, line / 2.0);
            let inner = offset(&pts, &normals, -line / 2.0);
            self.stroke(&outer, &inner, color);
        } else {
            self.fill(&pts, color, extent);
        }
    }

    fn rect(&mut self, centre: Vec2, size: Vec2, rot: f32, radius: f32, line: f32, color: Paint) {
        let half = size / 2.0;
        if line > 0.0 {
            let (line, color) = self.thin(line, color);
            let grow = Vec2::splat(line / 2.0);
            let outer_r = if radius > 0.0 {
                radius + line / 2.0
            } else {
                0.0
            };
            let corners = if radius > 0.0 {
                segments(radius + line) / 4 + 1
            } else {
                1
            };
            if half.min_element() <= line / 2.0 {
                // No hole left in the middle: a slab, as wide as the stroke.
                let pts = rounded(centre, half + grow, rot, outer_r, corners);
                self.fill(&pts, color, (half + grow).min_element() * 2.0);
                return;
            }
            // The inner ring keeps the outer's count of corner points even
            // where its radius has gone to nought, so the two pair up.
            let inner_r = if radius > 0.0 {
                (radius - line / 2.0).max(0.0)
            } else {
                0.0
            };
            let outer = rounded(centre, half + grow, rot, outer_r, corners);
            let inner = rounded(centre, half - grow, rot, inner_r, corners);
            self.stroke(&outer, &inner, color);
        } else {
            let corners = if radius > 0.0 {
                segments(radius) / 4 + 1
            } else {
                1
            };
            let pts = rounded(centre, half, rot, radius, corners);
            self.fill(&pts, color, half.min_element() * 2.0);
        }
    }
}

fn turned(p: Vec2, rot: f32) -> Vec2 {
    if rot == 0.0 {
        return p;
    }
    let (s, c) = rot.sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

/// How many straight pieces a curve of this radius wants on screen: enough
/// that a big ring is round, few enough that a dot is three triangles.
fn segments(radius: f32) -> usize {
    ((radius * 1.2) as usize).clamp(8, 64)
}

fn ring(centre: Vec2, r: Vec2, rot: f32, n: usize) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            turned(Vec2::new(a.cos() * r.x, a.sin() * r.y), rot) + centre
        })
        .collect()
}

/// A rectangle's outline, corners rounded by `radius` with `corners` points
/// each — one point when square. Clockwise on a y-down screen, starting at
/// the top-left.
fn rounded(centre: Vec2, half: Vec2, rot: f32, radius: f32, corners: usize) -> Vec<Vec2> {
    let r = radius.min(half.x).min(half.y).max(0.0);
    let mut pts = Vec::with_capacity(corners * 4);
    // Corner centres, and the quarter of the circle each one sweeps.
    let arcs = [
        (Vec2::new(-half.x + r, -half.y + r), std::f32::consts::PI),
        (
            Vec2::new(half.x - r, -half.y + r),
            1.5 * std::f32::consts::PI,
        ),
        (Vec2::new(half.x - r, half.y - r), 0.0),
        (
            Vec2::new(-half.x + r, half.y - r),
            0.5 * std::f32::consts::PI,
        ),
    ];
    for (at, start) in arcs {
        if corners <= 1 {
            let corner = Vec2::new(
                if at.x < 0.0 { -half.x } else { half.x },
                if at.y < 0.0 { -half.y } else { half.y },
            );
            pts.push(turned(corner, rot) + centre);
            continue;
        }
        // With no radius every point of the arc is the corner itself; the
        // repeats are kept so the ring pairs up with one that has a radius.
        for i in 0..corners {
            let a = start + i as f32 / (corners - 1) as f32 * 0.5 * std::f32::consts::PI;
            let p = at + Vec2::new(a.cos(), a.sin()) * r;
            pts.push(turned(p, rot) + centre);
        }
    }
    pts
}

/// Every point moved along its normal by `by`.
fn offset(points: &[Vec2], normals: &[Vec2], by: f32) -> Vec<Vec2> {
    points
        .iter()
        .zip(normals)
        .map(|(p, n)| *p + *n * by)
        .collect()
}

/// The outward normal at every corner of a convex polygon, scaled so that
/// moving each corner along it by `d` moves every edge out by `d` — the
/// mitre, as epaint does it. Repeated corners (a square corner of a
/// "rounded" ring) all get the mitre of the corner they make together, so a
/// ring with them in it offsets the same as one without. A sharp mitre is
/// capped, so an acute corner's ramp is a little short rather than a spike.
fn normals(points: &[Vec2]) -> Vec<Vec2> {
    let n = points.len();
    let area: f32 = (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let distinct = |from: usize, step: usize| -> Option<Vec2> {
        (1..n)
            .map(|k| points[(from + k * step) % n])
            .find(|p| p.distance_squared(points[from]) > 1e-8)
    };
    (0..n)
        .map(|i| {
            let (Some(prev), Some(next)) = (distinct(i, n - 1), distinct(i, 1)) else {
                return Vec2::ZERO;
            };
            let n0 = edge_normal(points[i] - prev) * sign;
            let n1 = edge_normal(next - points[i]) * sign;
            let m = (n0 + n1) / 2.0;
            let len_sq = m.length_squared();
            if len_sq < 1e-6 {
                return n0;
            }
            let m = m / len_sq;
            if m.length_squared() > 4.0 {
                m.normalize() * 2.0
            } else {
                m
            }
        })
        .collect()
}

/// The unit normal to an edge, on the outside of a polygon whose corners
/// run anticlockwise in the axes' own sense.
fn edge_normal(edge: Vec2) -> Vec2 {
    Vec2::new(edge.y, -edge.x).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIND_RECT: f32 = 0.0;

    /// The buffer's corners as egui would have been handed them.
    fn vertices(buf: &ShapeBuf) -> Vec<egui::epaint::Vertex> {
        buf.positions
            .iter()
            .zip(&buf.colors)
            .map(|(p, c)| egui::epaint::Vertex {
                pos: egui::pos2(p[0], p[1]),
                uv: egui::epaint::WHITE_UV,
                color: Paint(*c).color32(),
            })
            .collect()
    }

    #[test]
    fn the_stride_is_the_painters() {
        assert_eq!(STRIDE, bims::draw::STRIDE);
        assert_eq!(STRIDE, ship::draw::STRIDE);
        assert_eq!(STRIDE, lobby::draw::STRIDE);
    }

    #[test]
    fn a_whole_rect_is_two_triangles_in_a_ramp_and_one_past_the_canvas_is_none() {
        let clip = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(110.0, 110.0));
        let mut buf = ShapeBuf::new(clip, 1.0);
        let inside = [
            KIND_RECT, 50.0, 50.0, 10.0, 10.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0,
        ];
        buf.replay(&inside, View::PIXELS);
        // The two triangles of the rect, and two along each of its four
        // edges for the ramp.
        assert_eq!(buf.indices.len(), 6 + 4 * 6);
        // The canvas's origin goes onto every corner, and the solid corner
        // is half a pixel in from the rect's own.
        assert_eq!(vertices(&buf)[0].pos, egui::pos2(55.5, 55.5));
        assert_eq!(vertices(&buf)[0].color.a(), 255);
        // The ramp's far edge is half a pixel out, and nothing at all.
        let out = vertices(&buf)
            .into_iter()
            .find(|v| v.pos == egui::pos2(54.5, 54.5))
            .expect("the ramp's outer corner");
        assert_eq!(out.color, egui::Color32::TRANSPARENT);

        let mut gone = ShapeBuf::new(clip, 1.0);
        let outside = [
            KIND_ELLIPSE,
            300.0,
            300.0,
            20.0,
            20.0,
            0.0,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
            1.0,
        ];
        gone.replay(&outside, View::PIXELS);
        assert!(gone.is_empty());
    }

    #[test]
    fn a_stroke_is_a_band_a_hairline_is_fainter_and_a_mitre_moves_every_edge_alike() {
        // --- a_stroke_is_a_band_that_leaves_the_middle_empty ---
        {
            let clip = Rect::new(Vec2::ZERO, Vec2::new(100.0, 100.0));
            let mut buf = ShapeBuf::new(clip, 1.0);
            let ring = [
                KIND_ELLIPSE,
                50.0,
                50.0,
                40.0,
                40.0,
                0.0,
                0.0,
                2.0,
                1.0,
                1.0,
                1.0,
                1.0,
            ];
            buf.replay(&ring, View::PIXELS);
            // Every vertex sits on the stroke's two edges or half a pixel
            // either side of them, never at the centre; the ones furthest in
            // and out are the clear ends of the ramps.
            for v in &vertices(&buf) {
                let d = ((v.pos.x - 50.0).powi(2) + (v.pos.y - 50.0).powi(2)).sqrt();
                let r = [18.5f32, 19.5, 20.5, 21.5]
                    .into_iter()
                    .find(|r| (d - r).abs() < 0.1)
                    .unwrap_or_else(|| panic!("{d} is on no ring"));
                let clear = r == 18.5 || r == 21.5;
                assert_eq!(v.color == egui::Color32::TRANSPARENT, clear, "{d}");
            }
        }

        // --- a_hairline_is_drawn_a_pixel_wide_and_fainter ---
        {
            let clip = Rect::new(Vec2::ZERO, Vec2::new(100.0, 100.0));
            let mut buf = ShapeBuf::new(clip, 1.0);
            let hair = [
                KIND_RECT, 50.0, 50.0, 40.0, 40.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0, 1.0,
            ];
            buf.replay(&hair, View::PIXELS);
            let solid = vertices(&buf).iter().map(|v| v.color.a()).max().unwrap();
            assert!(solid > 0 && solid < 128, "{solid}");
            // A pixel wide about the path at 30: the ramps meet on it, solid,
            // and reach nothing half a pixel either side.
            let at = |x: f32| {
                vertices(&buf)
                    .into_iter()
                    .find(|v| (v.pos.x - x).abs() < 0.01)
                    .map(|v| v.color.a())
            };
            assert_eq!(at(30.0), Some(solid));
            assert_eq!(at(29.0), Some(0));
            assert_eq!(at(31.0), Some(0));
        }

        // --- a_stroked_triangle_is_hollow ---
        {
            let clip = Rect::new(Vec2::ZERO, Vec2::new(100.0, 100.0));
            let mut buf = ShapeBuf::new(clip, 1.0);
            let outline = [
                KIND_TRIANGLE,
                50.0,
                50.0,
                40.0,
                40.0,
                0.0,
                0.0,
                2.0,
                1.0,
                1.0,
                1.0,
                1.0,
            ];
            buf.replay(&outline, View::PIXELS);
            // Nothing lands deep inside the triangle: its middle, the centroid
            // of (30,30) (30,70) (70,70), is well off every edge.
            let (cx, cy) = (130.0 / 3.0, 170.0 / 3.0);
            for v in &vertices(&buf) {
                let d = ((v.pos.x - cx).powi(2) + (v.pos.y - cy).powi(2)).sqrt();
                assert!(d > 6.0, "{:?}", v.pos);
            }
            assert!(!buf.is_empty());
        }

        // --- the_mitre_moves_every_edge_the_same_distance ---
        {
            let square = [
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0),
                Vec2::new(0.0, 10.0),
            ];
            for n in normals(&square) {
                // Out along the diagonal, by root two: one unit along each axis.
                assert!(
                    (n.x.abs() - 1.0).abs() < 1e-5 && (n.y.abs() - 1.0).abs() < 1e-5,
                    "{n}"
                );
            }
            // The first corner is pushed towards negative x and y, and the
            // same square the other way round comes out the same way out.
            let first = normals(&square)[0];
            assert!(first.x < 0.0 && first.y < 0.0);
            let reversed: Vec<Vec2> = square.iter().rev().copied().collect();
            let last = normals(&reversed)[3];
            assert!(last.x < 0.0 && last.y < 0.0);
            // A corner given three times over is still that corner's mitre.
            let repeated = [
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0),
                Vec2::new(0.0, 10.0),
            ];
            let ns = normals(&repeated);
            assert_eq!(ns[0], ns[1]);
            assert_eq!(ns[1], ns[2]);
            assert!(ns[0].x < 0.0 && ns[0].y < 0.0);
        }
    }

    #[test]
    fn the_view_goes_both_ways() {
        let view = View {
            scale: 2.0,
            offset: Vec2::new(10.0, 20.0),
        };
        let world = Vec2::new(3.0, 4.0);
        assert_eq!(view.to_world(view.to_canvas(world)), world);
    }

    /// A colour at or under white is what egui made of it, to the byte,
    /// and fades as egui faded one — so the world's canvas, drawn by Bevy
    /// now, is the picture egui drew. A colour with a channel past one is
    /// kept past one for the bloom, and held at white where it has to be
    /// egui's bytes (feature 97).
    #[test]
    fn a_plain_colour_is_egui_s_to_the_byte_and_an_emissive_one_is_kept() {
        let plain = Paint::of(0.4, 0.72, 1.0, 0.5);
        let eguis = egui::Color32::from_rgba_unmultiplied(102, 184, 255, 128);
        assert_eq!(plain.color32(), eguis);
        assert!(!plain.is_emissive());
        assert_eq!(plain.faded(0.3).color32(), eguis.gamma_multiply(0.3));

        let hot = Paint::of(2.0, 1.5, 0.5, 1.0);
        assert!(hot.is_emissive());
        assert_eq!(hot.0, [2.0, 1.5, 0.5, 1.0]);
        assert_eq!(hot.color32(), egui::Color32::from_rgb(255, 255, 128));
        // Half as opaque: premultiplied like any other, and still past its
        // alpha, so still emissive.
        let half = Paint::of(2.0, 1.5, 0.5, 0.5);
        assert!(half.is_emissive());
        assert!((half.0[0] - 2.0 * 128.0 / 255.0).abs() < 1e-6);

        // And through the buffer to the mesh untouched.
        let mut buf = ShapeBuf::new(Rect::new(Vec2::ZERO, Vec2::splat(100.0)), 1.0);
        let rect = [
            KIND_RECT, 50.0, 50.0, 10.0, 10.0, 0.0, 0.0, 0.0, 3.0, 2.0, 1.0, 1.0,
        ];
        buf.replay(&rect, View::PIXELS);
        let parts = buf.into_parts();
        assert!(parts.colors.contains(&[3.0, 2.0, 1.0, 1.0]));
        assert_eq!(parts.positions.len(), parts.colors.len());
    }
}

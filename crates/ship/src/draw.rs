//! The bridge to the renderer, again.
//!
//! **This is a deliberate copy** of the room's `crates/game/src/draw.rs`, not
//! a shared module. The *format* is shared — twelve floats a shape, in the
//! order below — because one replay loop in JavaScript can then paint either
//! page. The *code* is not, because the room is a crate the designer has no
//! business importing: nothing here may reach into `room.rs`, `nav.rs` or
//! `task.rs`, and an import of `draw` would be the first crack in that.
//!
//! If the format ever changes it changes in two files, and the host reads the
//! stride at runtime from whichever wasm it loaded so the two can differ
//! while that is happening.

/// Floats per shape: kind, x, y, w, h, rot, radius, line, r, g, b, a.
pub const STRIDE: usize = 12;

pub const KIND_RECT: f32 = 0.0;
pub const KIND_ELLIPSE: f32 = 1.0;
/// A right-angled triangle: the half of the `w` x `h` box **below its
/// falling diagonal** — the right angle at the box's bottom-left corner,
/// the hypotenuse from top-left to bottom-right — spun about the box's
/// centre by `rot`. The one shape the diagonal walls needed and the only
/// thing added to the format since it was rectangles and ellipses; `radius`
/// is ignored and `line` strokes as for a rectangle.
pub const KIND_TRIANGLE: f32 = 2.0;

/// A filled rect, ellipse or triangle **with the objects' texture**: its
/// kind plus this (eight, nine, ten). The app lays a neutral wear-and-grime
/// texture over the fill (`crates/app/src/surfaces.rs`, [`Surface::Object`]) — what makes a locker or a counter read as a
/// made thing rather than a swatch. Its fields are the plain shape's; a
/// canvas drawn on the CPU draws it plain. Set by
/// [`DrawList::textured_from`] over what a painter drew of an object.
pub const KIND_TEXTURED: f32 = 8.0;
/// A filled shape with **leaves** laid over it instead (a tree's crown, a
/// bush, a potted plant): its kind plus sixty-four.
pub const KIND_FOLIAGE: f32 = 64.0;
/// Added to a plain or textured kind, the shape **sways in the wind**:
/// the app moves it a little each frame, by where it stands, so a crown
/// ripples and a stand of trees bends one after another. A picture only.
pub const KIND_SWAY: f32 = 4.0;

/// A rectangle filled with a **surface** — a texture the app holds for
/// floors, walls and ground, multiplied by the shape's colour — is this
/// plus the surface's number ([`Surface`]): sixteen for the deck plate,
/// seventeen for a bulkhead, and so on. Its `radius` and `line` are not a
/// corner and a stroke but the **anchor**: where the shape's centre is in
/// the texture, in world units of the painter's own frame (before any
/// turn), so the plates of a deck line up with its tiles and two shapes
/// side by side carry one texture across the seam. A surface the app ties
/// to the world instead (the open ground) ignores it. Wherever there is
/// no texture — a canvas drawn on the CPU — it is a plain rectangle of its
/// colour, which is why the colour is the surface's average.
pub const KIND_SURFACE: f32 = 16.0;
/// The same with the shape a [`KIND_TRIANGLE`]: thirty-two plus the surface.
/// The texture turns with the triangle's own `rot`.
pub const KIND_SURFACE_TRIANGLE: f32 = 32.0;

/// The anchor is kept within this much of the origin: a whole number of
/// every surface's repeat (sixteen tiles), so taking it off moves nothing.
pub const SURFACE_REPEAT: f32 = 16.0 * 52.0;

/// The textures a [`KIND_SURFACE`] rectangle can be filled with. The
/// numbers are the app's (`crates/app/src/surfaces.rs`), the same in the
/// room's `draw.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Surface {
    /// A ship's or a station's deck: steel plates, a tile each.
    Deck = 0,
    /// A bulkhead's face: a painted steel panel a tile.
    Bulkhead = 1,
    Grass = 2,
    Sand = 3,
    Snow = 4,
    /// A temperate town's wall: dressed stone, two courses a tile.
    Stone = 5,
    /// A desert town's wall: sun-baked plaster.
    Adobe = 6,
    /// An arctic town's wall: timber planks.
    Timber = 7,
    /// A town's indoor floor: boards.
    Floorboard = 8,
    /// The landing pad: concrete slabs.
    Concrete = 9,
    /// A cliff on the plain: bare rock.
    Rock = 10,
    /// A lake or a pool, from above: ripples and the sky in them.
    Water = 11,
    /// A frozen lake: cracked, with bubbles caught in it.
    Ice = 12,
    /// The objects' texture ([`KIND_TEXTURED`]): neutral wear and grime,
    /// laid over a fill rather than painted as a surface of its own.
    Object = 13,
}

/// A `line` width of zero means fill; anything greater strokes the outline.
const FILLED: f32 = 0.0;

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub const fn alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }

    /// This colour `by` times as bright, alpha kept: past one it is
    /// emissive, and the app's bloom lights the air round it — the
    /// room's `bims::draw::Color::glowing`, for the painters here.
    pub fn glowing(self, by: f32) -> Color {
        Color {
            r: self.r * by,
            g: self.g * by,
            b: self.b * by,
            a: self.a,
        }
    }
}

#[derive(Default)]
pub struct DrawList {
    data: Vec<f32>,
    /// Where in `data` the host lays its smooth fog, in floats: the
    /// crew's light map goes over everything before it and under
    /// everything after — the shots, the rings, the overlays. `None` for a
    /// picture with no fog in it, which is all of it under whatever the
    /// host lays over.
    fog_at: Option<usize>,
}

impl DrawList {
    pub fn new() -> DrawList {
        DrawList {
            data: Vec::with_capacity(4096 * STRIDE),
            fog_at: None,
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.fog_at = None;
    }

    /// The shapes as the app will read them.
    pub fn shapes(&self) -> &[f32] {
        &self.data
    }

    /// Here is where the host's fog goes: what is pushed after this is
    /// drawn over it.
    pub fn mark_fog(&mut self) {
        self.fog_at = Some(self.data.len());
    }

    /// The shapes cut where the fog goes: what it lies over, and what is
    /// drawn over it — nothing, for a picture with no fog.
    pub fn fog_split(&self) -> (&[f32], &[f32]) {
        self.data
            .split_at(self.fog_at.unwrap_or(self.data.len()).min(self.data.len()))
    }

    /// Every shape of `shapes` — this format, any buffer — as it is. What
    /// puts a picture made in the same frame, the room's, onto the ship's.
    pub fn append(&mut self, shapes: &[f32]) {
        self.data.extend_from_slice(shapes);
    }

    /// Every shape of `shapes` — this format, any buffer — re-emitted with
    /// its centre measured from `centre`, turned about it by `angle`, and its
    /// own rotation added to. What draws a picture made in the ship's own
    /// frame turned to the ship's heading: the ship's tiles, the room aboard.
    pub fn append_turned(&mut self, shapes: &[f32], centre: (f32, f32), angle: f32) {
        let (s, c) = (angle.sin(), angle.cos());
        for shape in shapes.chunks_exact(STRIDE) {
            let (x, y) = (shape[1] - centre.0, shape[2] - centre.1);
            self.data.extend_from_slice(&[
                shape[0],
                x * c - y * s,
                x * s + y * c,
                shape[3],
                shape[4],
                shape[5] + angle,
                shape[6],
                shape[7],
                shape[8],
                shape[9],
                shape[10],
                shape[11],
            ]);
        }
    }

    /// Every shape of `shapes` as it is, with its alpha scaled by `alpha`:
    /// a picture drawn once and shown through. What a blueprint is — the
    /// part's own picture, faded — so a ghost of a hob is the hob's picture
    /// and not a second drawing of one.
    pub fn append_faded(&mut self, shapes: &[f32], alpha: f32) {
        let from = self.data.len();
        self.append(shapes);
        for shape in self.data[from..].chunks_exact_mut(STRIDE) {
            shape[11] *= alpha;
        }
    }

    /// [`DrawList::append_turned`] and then moved: every centre lands `at`
    /// away from where the turn put it. What draws a picture made in
    /// somebody else's frame — a station's, whose grid is not the ship's —
    /// where that somebody is on the screen.
    pub fn append_turned_at(
        &mut self,
        shapes: &[f32],
        centre: (f32, f32),
        angle: f32,
        at: (f32, f32),
    ) {
        let from = self.data.len();
        self.append_turned(shapes, centre, angle);
        for shape in self.data[from..].chunks_exact_mut(STRIDE) {
            shape[1] += at.0;
            shape[2] += at.1;
        }
    }

    /// Turn every shape pushed since the buffer was `from` floats long about
    /// the origin, by `angle` in the screen's sense: each centre goes through
    /// the rotation and each `rot` has it added. What the game view does to
    /// the sky and the map when the camera is head up rather than north up,
    /// and it works on the buffer after the fact so the pictures of planets
    /// and stations need know nothing about it.
    pub fn turn_from(&mut self, from: usize, angle: f32) {
        if angle == 0.0 {
            return;
        }
        let (s, c) = (angle.sin(), angle.cos());
        for shape in self.data[from..].chunks_exact_mut(STRIDE) {
            let (x, y) = (shape[1], shape[2]);
            shape[1] = x * c - y * s;
            shape[2] = x * s + y * c;
            shape[5] += angle;
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Every shape pushed since `from` made foliage: a fill's kind gets
    /// [`KIND_FOLIAGE`], and with `sway` every shape of it — a palm's thin
    /// fronds too — [`KIND_SWAY`]. Out of doors a crown sways; a plant
    /// in a pot does not.
    pub fn foliage_from(&mut self, from: usize, sway: bool) {
        let from = from.min(self.data.len());
        for shape in self.data[from..].chunks_exact_mut(STRIDE) {
            if shape[0] >= KIND_TEXTURED {
                continue;
            }
            if sway {
                shape[0] += KIND_SWAY;
            }
            if shape[7] == 0.0 {
                shape[0] += KIND_FOLIAGE;
            }
        }
    }

    /// Every shape pushed since `from` swaying ([`KIND_SWAY`]) and nothing
    /// more: a palm's fronds, a tussock's blades, a cactus's arms.
    pub fn sway_from(&mut self, from: usize) {
        let from = from.min(self.data.len());
        for shape in self.data[from..].chunks_exact_mut(STRIDE) {
            if shape[0] < KIND_SWAY {
                shape[0] += KIND_SWAY;
            }
        }
    }

    /// Every filled shape pushed since `from` given the objects' texture
    /// ([`KIND_TEXTURED`]); a stroke, a surface or a shape already
    /// textured is left as it is.
    pub fn textured_from(&mut self, from: usize) {
        let from = from.min(self.data.len());
        for shape in self.data[from..].chunks_exact_mut(STRIDE) {
            if shape[0] < KIND_TEXTURED && shape[7] == 0.0 {
                shape[0] += KIND_TEXTURED;
            }
        }
    }

    /// `x`/`y` are the centre and `w`/`h` the full size, both in world units.
    /// `rot` spins the shape about its own centre; `radius` rounds rectangle
    /// corners and is ignored by ellipses; `line` strokes instead of filling.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        kind: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rot: f32,
        radius: f32,
        line: f32,
        c: Color,
    ) {
        self.data
            .extend_from_slice(&[kind, x, y, w, h, rot, radius, line, c.r, c.g, c.b, c.a]);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, c: Color) {
        self.push(KIND_RECT, x, y, w, h, 0.0, radius, FILLED, c);
    }

    pub fn stroke_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        line: f32,
        c: Color,
    ) {
        self.push(KIND_RECT, x, y, w, h, 0.0, radius, line, c);
    }

    pub fn ellipse(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        self.push(KIND_ELLIPSE, x, y, w, h, 0.0, 0.0, FILLED, c);
    }

    /// The bottom-left half of a box, turned about its centre — see
    /// [`KIND_TRIANGLE`] for which half at `rot` nought.
    pub fn triangle(&mut self, x: f32, y: f32, w: f32, h: f32, rot: f32, c: Color) {
        self.push(KIND_TRIANGLE, x, y, w, h, rot, 0.0, FILLED, c);
    }

    /// A rectangle filled with `surface`, its centre at `anchor` in the
    /// texture — for a shape laid in the painter's own frame, that is its
    /// centre, `(x, y)`, which is what [`DrawList::surface_box`] passes.
    #[allow(clippy::too_many_arguments)]
    pub fn surface(
        &mut self,
        surface: Surface,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rot: f32,
        anchor: (f32, f32),
        c: Color,
    ) {
        self.push(
            KIND_SURFACE + surface as u8 as f32,
            x,
            y,
            w,
            h,
            rot,
            anchor.0.rem_euclid(SURFACE_REPEAT),
            anchor.1.rem_euclid(SURFACE_REPEAT),
            c,
        );
    }

    /// [`DrawList::triangle`] filled with `surface`, anchored at its box's
    /// centre `(x, y)`.
    #[allow(clippy::too_many_arguments)]
    pub fn surface_triangle(
        &mut self,
        surface: Surface,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rot: f32,
        c: Color,
    ) {
        self.push(
            KIND_SURFACE_TRIANGLE + surface as u8 as f32,
            x,
            y,
            w,
            h,
            rot,
            x.rem_euclid(SURFACE_REPEAT),
            y.rem_euclid(SURFACE_REPEAT),
            c,
        );
    }

    /// [`DrawList::surface`] between two corners, unturned, anchored where
    /// it lies.
    pub fn surface_box(
        &mut self,
        surface: Surface,
        (x0, y0, x1, y1): (f32, f32, f32, f32),
        c: Color,
    ) {
        let (x, y) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        self.surface(surface, x, y, x1 - x0, y1 - y0, 0.0, (x, y), c);
    }

    /// A rectangle given by its corners rather than its centre, which is how
    /// tile geometry comes out.
    pub fn box_between(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32, c: Color) {
        self.rect(
            (x0 + x1) / 2.0,
            (y0 + y1) / 2.0,
            x1 - x0,
            y1 - y0,
            radius,
            c,
        );
    }

    /// A line between two points, as a thin rectangle turned to lie along it.
    ///
    /// The format has rectangles and ellipses and nothing else — see the
    /// module note — so a line is a rectangle with the `rot` field doing the
    /// work. The map's route line is the only thing that wants one, and it
    /// wants one badly enough to be worth the four lines of trigonometry.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Color) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let length = (dx * dx + dy * dy).sqrt();
        if length <= 0.0 {
            return;
        }
        self.push(
            KIND_RECT,
            (x0 + x1) / 2.0,
            (y0 + y1) / 2.0,
            length,
            width,
            dy.atan2(dx),
            0.0,
            FILLED,
            c,
        );
    }

    pub fn stroke_between(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        radius: f32,
        line: f32,
        c: Color,
    ) {
        self.stroke_rect(
            (x0 + x1) / 2.0,
            (y0 + y1) / 2.0,
            x1 - x0,
            y1 - y0,
            radius,
            line,
            c,
        );
    }
}

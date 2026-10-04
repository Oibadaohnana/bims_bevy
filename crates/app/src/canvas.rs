//! Where the shapes land, and where the pointer is.
//!
//! A screen paints into **canvases**: rectangles of the window, each with
//! its own view and its own shape buffer. The game's screen has one, the
//! whole of the window between the panels; the lobby's World tab has two,
//! the galaxy and the system diagram, laid out inside an egui panel like
//! any other widget. The canvas **between** the panels is Bevy's — a mesh
//! a layer on the one camera, under egui and under the bloom
//! (`scene::WorldCanvas`, feature 97) — and a canvas **inside** a panel is
//! egui's, tessellated into an egui mesh on the panel's own layer by
//! [`paint_shapes`], since a Bevy mesh under a panel's opaque fill is a
//! mesh nobody sees. Either is clipped to its rectangle.
//!
//! The pointer is read out of egui rather than out of Bevy's input, so that
//! a click on a panel is the panel's and a click beside it is the canvas's
//! by the same rule egui uses to decide — `is_pointer_over_area` — and so a
//! text field that has the keyboard keeps it.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::shapes::{Rect, ShapeBuf, View};

/// Paint `shapes`, in world units under `view`, into `rect` on `painter`'s
/// layer — clipped to the rect, so a canvas inside a panel stays inside
/// it. For a canvas inside a panel: the one between the panels is
/// `scene::WorldCanvas::shapes`.
pub fn paint_shapes(painter: &egui::Painter, rect: Rect, view: View, shapes: &[f32]) {
    let mut buf = ShapeBuf::new(rect, painter.pixels_per_point());
    buf.replay(shapes, view);
    if buf.is_empty() {
        return;
    }
    painter
        .with_clip_rect(egui_rect(rect))
        .add(buf.into_shape());
}

/// The background layer, clipped to a canvas: where a screen whose canvas
/// is the window between the panels puts its words, over the canvas's
/// picture.
pub fn canvas_painter(ctx: &egui::Context, rect: Rect) -> egui::Painter {
    ctx.layer_painter(egui::LayerId::background())
        .with_clip_rect(egui_rect(rect))
}

// --- the pointer -------------------------------------------------------------

/// The pointer this frame, as egui saw it. Positions are window pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    pub pos: Option<Vec2>,
    /// Over a panel, a window or a menu: the canvas is not to act on it.
    pub over_ui: bool,
    pub primary_pressed: bool,
    pub primary_released: bool,
    pub primary_down: bool,
    pub secondary_pressed: bool,
    pub secondary_released: bool,
    pub secondary_down: bool,
    pub middle_down: bool,
    pub middle_pressed: bool,
    /// The middle button let go short of a drag: a ping (October 2026),
    /// where a middle drag pans.
    pub middle_clicked: bool,
    /// Wheel, in points, up positive.
    pub scroll: f32,
    /// Shift held: an order given with it waits its turn behind what the
    /// crew member is on rather than displacing it (feature 69). Read
    /// with the buttons so a click and its key are the same frame's.
    pub shift: bool,
}

impl Pointer {
    pub fn read(ctx: &egui::Context) -> Pointer {
        use egui::PointerButton::{Middle, Primary, Secondary};
        // Asked before the input lock is taken: both of these lock the
        // context themselves, and egui's lock is not reentrant.
        let over_ui = ctx.is_pointer_over_egui() || ctx.egui_wants_pointer_input();
        ctx.input(|i| Pointer {
            pos: i.pointer.latest_pos().map(|p| Vec2::new(p.x, p.y)),
            over_ui,
            primary_pressed: i.pointer.button_pressed(Primary),
            primary_released: i.pointer.button_released(Primary),
            primary_down: i.pointer.button_down(Primary),
            secondary_pressed: i.pointer.button_pressed(Secondary),
            secondary_released: i.pointer.button_released(Secondary),
            secondary_down: i.pointer.button_down(Secondary),
            middle_down: i.pointer.button_down(Middle),
            middle_pressed: i.pointer.button_pressed(Middle),
            middle_clicked: i.pointer.button_clicked(Middle),
            shift: i.modifiers.shift,
            // The raw wheel events rather than the smoothed delta: a zoom
            // wants the notch, not a scroll area's easing.
            scroll: i
                .raw
                .events
                .iter()
                .map(|e| match e {
                    egui::Event::MouseWheel { unit, delta, .. } => match unit {
                        egui::MouseWheelUnit::Point => delta.y,
                        egui::MouseWheelUnit::Line => delta.y * 40.0,
                        egui::MouseWheelUnit::Page => delta.y * 400.0,
                    },
                    _ => 0.0,
                })
                .sum(),
        })
    }

    /// Where the pointer is inside `rect`, or `None` when it is outside it
    /// or over a panel.
    pub fn on(&self, rect: Rect) -> Option<Vec2> {
        let p = self.pos?;
        (!self.over_ui && rect.contains(p)).then(|| p - rect.min)
    }
}

/// How near the window's edge, in logical points, the pointer pans the
/// camera (task 123).
pub const EDGE_SCROLL_ZONE: f32 = 8.0;

/// The pan the pointer against the window's edge asks for this frame
/// (task 123), in the screen units `Session::pan` takes and the sign the
/// WASD pan gives them: `pointer` in points from the window's top left,
/// `window` its size, `zone` how near an edge counts, `speed` in points a
/// second and `dt` the frame's seconds. Each axis on its own, so a corner
/// pans on both at full speed; the pointer at the left edge pans exactly
/// as `PanLeft` does. Nothing with no pointer, a pointer outside the
/// window, or no speed.
pub fn edge_pan(pointer: Option<Vec2>, window: Vec2, zone: f32, speed: f32, dt: f32) -> Vec2 {
    let Some(p) = pointer else {
        return Vec2::ZERO;
    };
    if speed <= 0.0 || p.x < 0.0 || p.y < 0.0 || p.x > window.x || p.y > window.y {
        return Vec2::ZERO;
    }
    let step = speed * dt;
    let axis = |at: f32, size: f32| {
        if at < zone {
            step
        } else if at > size - zone {
            -step
        } else {
            0.0
        }
    };
    Vec2::new(axis(p.x, window.x), axis(p.y, window.y))
}

/// [`edge_pan`] for a screen's frame, or `None` when there is nothing to
/// pan: the window unfocused, a middle drag under way, the setting off, a
/// `BIMS_POINTER` script driving the pointer, or the pointer nowhere near
/// an edge. Measured against the whole window, panels and all. The
/// screen says for itself whether the Esc sheet is up.
pub fn edge_pan_now(
    ctx: &egui::Context,
    pointer: &Pointer,
    keys: &crate::keys::Keys,
    focused: bool,
    dt: f32,
) -> Option<Vec2> {
    if !focused || pointer.middle_down || crate::dev::pointer_scripted() {
        return None;
    }
    let window = ctx.viewport_rect();
    let d = edge_pan(
        pointer
            .pos
            .map(|p| p - Vec2::new(window.min.x, window.min.y)),
        Vec2::new(window.width(), window.height()),
        EDGE_SCROLL_ZONE,
        crate::screens::designer::PAN_SPEED * keys.edge_scroll_speed(),
        dt,
    );
    (d != Vec2::ZERO).then_some(d)
}

/// How much one point of wheel is worth as a zoom factor.
pub const ZOOM_PER_POINT: f32 = 0.003;

pub fn zoom_factor(scroll: f32) -> f32 {
    (scroll * ZOOM_PER_POINT).exp()
}

/// A `Ui` over the whole window, on the background layer: what the panels
/// are shown into, and what is left of it afterwards is the canvas.
pub fn root_ui(ctx: &egui::Context) -> egui::Ui {
    egui::Ui::new(
        ctx.clone(),
        egui::Id::new("viewport"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    )
}

/// The egui rect as ours.
pub fn rect_of(r: egui::Rect) -> Rect {
    Rect::new(Vec2::new(r.min.x, r.min.y), Vec2::new(r.max.x, r.max.y))
}

pub fn egui_rect(r: Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(r.min.x, r.min.y), egui::pos2(r.max.x, r.max.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::designer::PAN_SPEED;

    /// The pointer against the window's edge (task 123): nothing in the
    /// middle, the left edge exactly `PAN_SPEED` to the left, a corner
    /// both axes at full speed, and nothing with no pointer or the
    /// setting off.
    #[test]
    fn the_edge_pans_as_the_keys_do_and_the_middle_does_not() {
        let window = Vec2::new(1400.0, 900.0);
        let dt = 1.0 / 60.0;
        let pan = |p: Option<Vec2>, speed: f32| {
            edge_pan(p, window, EDGE_SCROLL_ZONE, PAN_SPEED * speed, dt)
        };
        let step = PAN_SPEED * dt;
        assert_eq!(pan(Some(window / 2.0), 1.0), Vec2::ZERO);
        // The left edge is `d.x += PAN_SPEED * dt` and nothing else.
        assert_eq!(pan(Some(Vec2::new(2.0, 450.0)), 1.0), Vec2::new(step, 0.0));
        assert_eq!(
            pan(Some(Vec2::new(1398.0, 450.0)), 1.0),
            Vec2::new(-step, 0.0)
        );
        assert_eq!(pan(Some(Vec2::new(700.0, 1.0)), 1.0), Vec2::new(0.0, step));
        assert_eq!(
            pan(Some(Vec2::new(700.0, 899.0)), 1.0),
            Vec2::new(0.0, -step)
        );
        // Just past the zone is the middle.
        assert_eq!(
            pan(Some(Vec2::new(EDGE_SCROLL_ZONE + 0.5, 450.0)), 1.0),
            Vec2::ZERO
        );
        // A corner: both axes, each at the whole speed.
        assert_eq!(pan(Some(Vec2::new(0.0, 0.0)), 1.0), Vec2::new(step, step));
        assert_eq!(
            pan(Some(Vec2::new(1399.0, 899.0)), 1.0),
            Vec2::new(-step, -step)
        );
        // The setting scales it.
        let faster = pan(Some(Vec2::new(2.0, 450.0)), 2.5);
        assert!(
            (faster.x - step * 2.5).abs() < 1e-3 && faster.y == 0.0,
            "{faster}"
        );
        // No pointer, or the setting at nought: nothing.
        assert_eq!(pan(None, 1.0), Vec2::ZERO);
        assert_eq!(pan(Some(Vec2::new(2.0, 450.0)), 0.0), Vec2::ZERO);
    }
}

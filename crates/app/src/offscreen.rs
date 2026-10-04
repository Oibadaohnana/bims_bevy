//! Arrows at the edge of the game view for what is off it (October 2026,
//! the player's word: "for an overview"): a red one for every enemy still
//! standing — whether or not the crew see it — and a green one for every
//! other player's own Bim, each on the canvas's edge on the line from the
//! middle to the body and pointing at it. The view reaches no further
//! than a weapon shoots (`ship::game::VIEW_REACH`), so an arrow is
//! something nobody can shoot at from here.
//!
//! A picture, read off the rooms every frame and drawn over the deck under
//! the panels; nothing the world reads.

use bevy_egui::egui;
use ship::game::Game;
use ship::world_paint;

/// How far in from the canvas's edge an arrow's point stands, in points.
const INSET: f32 = 22.0;
/// The arrow's length, point to back, and half its width at the back.
const LENGTH: f32 = 18.0;
const HALF_WIDTH: f32 = 10.0;
/// How far onto the canvas a body has to be before it has no arrow: its
/// middle this far inside the edge, so one half on screen keeps its arrow
/// until it is plainly there.
const SHOWN_INSIDE: f32 = 8.0;

const ENEMY: egui::Color32 = egui::Color32::from_rgb(0xe0, 0x2e, 0x26);
const FRIEND: egui::Color32 = egui::Color32::from_rgb(0x3f, 0xd2, 0x4a);
const EDGE: egui::Color32 = egui::Color32::from_black_alpha(200);

/// One body an arrow may point at: where it is in the camera's units,
/// and whether it is an enemy (else a fellow player).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub at: (f32, f32),
    pub enemy: bool,
}

/// Every body worth an arrow, off screen or not — [`paint`] leaves out
/// those on it. Enemies are every body standing that is not of the
/// crew's side, in the crew's room and the station's (a machine, a
/// Manufacturer); friends every player's own Bim but `local`'s, downed
/// or not, since a player down is the one most worth finding.
pub fn marks(game: &Game, local: u32) -> Vec<Mark> {
    let mut out = Vec::new();
    let crew = &game.world.aboard;
    let room = &crew.room;
    let players = game.world.players();
    for who in 0..crew.count() {
        let w = who as usize;
        if who < players && room.is_player(w) {
            if who != local && room.is_alive(w) {
                out.push(Mark {
                    at: world_paint::crew_on_screen(game, who),
                    enemy: false,
                });
            }
        } else if !room.is_friendly_body(w) && !room.is_down(w) {
            out.push(Mark {
                at: world_paint::crew_on_screen(game, who),
                enemy: true,
            });
        }
    }
    if let Some(residents) = &game.world.residents {
        let room = &residents.aboard.room;
        for who in 0..residents.aboard.count() {
            let w = who as usize;
            if !room.is_friendly_body(w) && !room.is_down(w) {
                out.push(Mark {
                    at: world_paint::resident_on_screen(game, who),
                    enemy: true,
                });
            }
        }
    }
    out
}

/// Where the arrow for a body at `to` goes in `bounds` (the canvas, less
/// what the panels cover): its point on the edge, inset by [`INSET`],
/// along the line from `from` — the middle of the canvas, where the
/// player's own Bim stands — and the way it points. `None` while the
/// body is in `bounds`.
pub fn place(
    bounds: egui::Rect,
    from: egui::Pos2,
    to: egui::Pos2,
) -> Option<(egui::Pos2, egui::Vec2)> {
    if bounds.shrink(SHOWN_INSIDE).contains(to) {
        return None;
    }
    let inner = bounds.shrink(INSET);
    if !inner.is_positive() {
        return None;
    }
    let from = inner.clamp(from);
    let way = to - from;
    let length = way.length();
    if length.is_nan() || length <= 1.0 {
        return None;
    }
    let dir = way / length;
    // As far along `dir` as the inset box allows: the nearer of the two
    // edges it is heading for.
    let reach = |d: f32, at: f32, low: f32, high: f32| {
        if d > 1e-6 {
            (high - at) / d
        } else if d < -1e-6 {
            (low - at) / d
        } else {
            f32::INFINITY
        }
    };
    let across = reach(dir.x, from.x, inner.min.x, inner.max.x);
    let down = reach(dir.y, from.y, inner.min.y, inner.max.y);
    Some((from + dir * across.min(down), dir))
}

/// Draw an arrow in `bounds` for every mark off it, from `from` (the
/// canvas's middle), enemies under friends so a fellow player's green is
/// never hidden. `to_screen` puts a point in the camera's units on the
/// canvas.
pub fn paint(
    painter: &egui::Painter,
    bounds: egui::Rect,
    from: egui::Pos2,
    marks: &[Mark],
    to_screen: impl Fn((f32, f32)) -> egui::Pos2,
) {
    for enemy in [true, false] {
        for mark in marks.iter().filter(|m| m.enemy == enemy) {
            let Some((tip, dir)) = place(bounds, from, to_screen(mark.at)) else {
                continue;
            };
            let across = egui::vec2(-dir.y, dir.x);
            let back = tip - dir * LENGTH;
            let notch = back + dir * (LENGTH * 0.3);
            let (left, right) = (back + across * HALF_WIDTH, back - across * HALF_WIDTH);
            let fill = if enemy { ENEMY } else { FRIEND };
            // A notched arrowhead is not convex: two triangles, one each
            // side of the shaft, with a dark rim round the whole.
            for wing in [left, right] {
                painter.add(egui::Shape::convex_polygon(
                    vec![tip, wing, notch],
                    fill,
                    egui::Stroke::NONE,
                ));
            }
            painter.add(egui::Shape::closed_line(
                vec![tip, left, notch, right],
                egui::Stroke::new(1.5, EDGE),
            ));
        }
    }
}

/// An Area defend's green (`theme::AREA`), and how long and how deep its
/// glow along the edge is, in points.
const AREA: egui::Color32 = crate::theme::AREA;
const GLOW_LENGTH: f32 = 300.0;
const GLOW_DEPTH: f32 = 64.0;
/// The FOB's badge on the edge: its disc's radius.
const BADGE: f32 = 15.0;

/// An Area defend's FOB off the view (October 2026, the player's word: "a
/// green area on the edge of the screen when you don't see it"): a green
/// glow along the edge of `bounds` on the line from `from` to the ring,
/// the FOB's mark on a dark disc and an arrow to it. `at` and `radius` are
/// the ring on the canvas, in points; nothing while any of it is in
/// `bounds`. `mark` draws the FOB's mark.
pub fn paint_area(
    painter: &egui::Painter,
    bounds: egui::Rect,
    from: egui::Pos2,
    at: egui::Pos2,
    radius: f32,
    mark: impl Fn(&egui::Painter, egui::Pos2, f32, egui::Color32),
) {
    if bounds.distance_to_pos(at) <= radius {
        return;
    }
    let Some((edge, _)) = place(bounds.expand(INSET), from, at) else {
        return;
    };
    let Some((tip, dir)) = place(bounds, from, at) else {
        return;
    };
    // The side the glow lies along: the nearest to where the line leaves.
    let sides = [
        (edge.x - bounds.min.x, egui::vec2(1.0, 0.0)),
        (bounds.max.x - edge.x, egui::vec2(-1.0, 0.0)),
        (edge.y - bounds.min.y, egui::vec2(0.0, 1.0)),
        (bounds.max.y - edge.y, egui::vec2(0.0, -1.0)),
    ];
    let inward = sides
        .iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(egui::vec2(0.0, 1.0), |s| s.1);
    let along = egui::vec2(-inward.y, inward.x);
    let on_edge = bounds.clamp(edge);
    let mut glow = egui::Mesh::default();
    let strong = AREA.gamma_multiply(0.85);
    let clear = egui::Color32::TRANSPARENT;
    // Four stations along it, faded at both ends, each the edge and a
    // point inside it.
    let stations = [(-0.5, clear), (-0.2, strong), (0.2, strong), (0.5, clear)];
    for (k, (s, colour)) in stations.iter().enumerate() {
        let p = on_edge + along * (s * GLOW_LENGTH);
        glow.colored_vertex(bounds.clamp(p), *colour);
        glow.colored_vertex(bounds.clamp(p) + inward * GLOW_DEPTH, clear);
        if k > 0 {
            let i = (2 * k) as u32;
            glow.add_triangle(i - 2, i - 1, i);
            glow.add_triangle(i - 1, i + 1, i);
        }
    }
    painter.add(egui::Shape::mesh(glow));
    // The edge itself lit, a hard line in the middle of the glow.
    painter.line_segment(
        [
            bounds.clamp(on_edge - along * (GLOW_LENGTH * 0.25)),
            bounds.clamp(on_edge + along * (GLOW_LENGTH * 0.25)),
        ],
        egui::Stroke::new(5.0, AREA),
    );
    // The badge, a little in from the arrow's point, and the arrow past it.
    let badge = tip - dir * (BADGE + 4.0);
    painter.circle_filled(badge, BADGE, EDGE);
    painter.circle_stroke(badge, BADGE, egui::Stroke::new(1.8, AREA));
    mark(painter, badge, BADGE * 1.5, AREA);
    let across = egui::vec2(-dir.y, dir.x);
    let point = tip + dir * 8.0;
    let back = tip - dir * 2.0;
    painter.add(egui::Shape::convex_polygon(
        vec![point, back + across * 7.0, back - across * 7.0],
        AREA,
        egui::Stroke::new(1.2, EDGE),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIDDLE: egui::Pos2 = egui::pos2(400.0, 300.0);

    fn canvas() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn a_body_on_the_canvas_has_no_arrow() {
        assert_eq!(place(canvas(), MIDDLE, egui::pos2(400.0, 300.0)), None);
        assert_eq!(place(canvas(), MIDDLE, egui::pos2(20.0, 590.0)), None);
    }

    #[test]
    fn an_arrow_stands_on_the_edge_towards_the_body_and_points_at_it() {
        // Straight off to the right: on the right edge, at the middle's height.
        let (tip, dir) = place(canvas(), MIDDLE, egui::pos2(2000.0, 300.0)).unwrap();
        assert!((tip.x - (800.0 - INSET)).abs() < 1e-3 && (tip.y - 300.0).abs() < 1e-3);
        assert!((dir.x - 1.0).abs() < 1e-5);
        // Up and a little left: on the top edge.
        let (tip, dir) = place(canvas(), MIDDLE, egui::pos2(350.0, -1000.0)).unwrap();
        assert!((tip.y - INSET).abs() < 1e-3, "{tip:?}");
        assert!(tip.x < 400.0 && dir.y < 0.0);
        // Far off a corner: inside the inset box, never off it.
        let (tip, _) = place(canvas(), MIDDLE, egui::pos2(-5000.0, 9000.0)).unwrap();
        assert!(canvas().shrink(INSET - 0.01).contains(tip), "{tip:?}");
        // With the panel cutting the bottom off, a body under it is off
        // too, and its arrow stands over the panel, straight below the
        // player.
        let above = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(800.0, 450.0));
        let (tip, dir) = place(above, MIDDLE, egui::pos2(400.0, 520.0)).unwrap();
        assert!((tip.x - 400.0).abs() < 1e-3 && (tip.y - (450.0 - INSET)).abs() < 1e-3);
        assert!((dir.y - 1.0).abs() < 1e-5);
    }
}

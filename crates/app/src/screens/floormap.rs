//! The floor's chart (October 2026, `world::floor`): the world map in
//! place of the galaxy chart. The start at the bottom, the Machine Heart
//! at the top, a row a hop and a day, the places joined by the trips up —
//! drawn in egui over the canvas, the way Slay the Spire draws its map.
//!
//! The wheel scrolls it up and down, Ctrl and the wheel zooms it (and the
//! buttons in the top left corner), a left drag or a middle drag pans it.
//! A click on a place picks it, as a star on the galaxy chart was picked;
//! the bar at the foot of the map proposes it. A right drag draws on it
//! the way the player means to go, in their colour on everybody's chart,
//! kept across the missions ([`Sketches`]). Nothing here decides
//! anything: where a trip may go is the world's (`World::floor_next`), and
//! every mark is read off it.

use bevy_egui::egui;
use bims::combat::Tier;
use world::floor::Floor;
use world::{FloorMark, Site, SiteKind, World};

use crate::names::*;
use crate::theme;

/// Points between two rows at a zoom of one.
const ROW_GAP: f32 = 58.0;
/// Points across the floor at a zoom of one, at the most.
const SPAN: f32 = 460.0;
/// Points from the chart's foot to the start, unpanned.
const FOOT: f32 = 90.0;
/// A place's radius at a zoom of one.
const NODE: f32 = 13.0;
/// The gutter on the left the days and the tiers are written in.
const GUTTER: f32 = 74.0;
/// How far the chart zooms out and in.
const ZOOM_RANGE: (f32, f32) = (0.3, 2.5);
/// The elite's ring and crown (`ship::world_paint::ELITE`).
const ELITE: egui::Color32 = egui::Color32::from_rgb(0xff, 0x66, 0xe6);

/// The chart's own state: how it is panned and zoomed, and the place
/// under the pointer.
#[derive(Default)]
pub struct FloorChart {
    /// Points the floor is moved from where it stands unpanned.
    offset: egui::Vec2,
    /// Nought until the chart is first shown: one.
    zoom: f32,
    /// The place under the pointer, `(row, index)`.
    pub hovered: Option<(u32, usize)>,
    /// Whether the chart has been brought to the crew's place once.
    focused: bool,
    /// The Heart's row: how far up the floor goes.
    rows: u32,
}

impl FloorChart {
    pub fn zoom(&self) -> f32 {
        if self.zoom > 0.0 { self.zoom } else { 1.0 }
    }

    fn span(&self, rect: egui::Rect) -> f32 {
        (rect.width() - 2.0 * GUTTER - 40.0).clamp(160.0, SPAN) * self.zoom().max(0.6)
    }

    fn gap(&self) -> f32 {
        ROW_GAP * self.zoom()
    }

    /// Where a point of the floor is on the screen: `x` across the floor,
    /// nought to one, and `row` up it.
    pub fn to_screen(&self, rect: egui::Rect, x: f32, row: f32) -> egui::Pos2 {
        egui::pos2(
            rect.center().x + self.offset.x + (x - 0.5) * self.span(rect),
            rect.max.y - FOOT + self.offset.y - row * self.gap(),
        )
    }

    /// The point of the floor under a point of the screen: what a ping is
    /// sent as, so every player's chart puts it on the same place.
    pub fn to_floor(&self, rect: egui::Rect, p: egui::Pos2) -> (f32, f32) {
        let x = (p.x - rect.center().x - self.offset.x) / self.span(rect) + 0.5;
        let row = (rect.max.y - FOOT + self.offset.y - p.y) / self.gap();
        (x, row)
    }

    /// Moved by `d` points, kept where some of the floor shows.
    pub fn pan(&mut self, rect: egui::Rect, d: egui::Vec2) {
        self.offset += d;
        self.keep_in(rect);
    }

    /// The wheel, `scroll` points (up positive): up the floor and down it.
    pub fn scroll(&mut self, rect: egui::Rect, scroll: f32) {
        self.pan(rect, egui::vec2(0.0, scroll));
    }

    /// Zoomed by `factor` about `at`, which stays where it is.
    pub fn zoom_at(&mut self, rect: egui::Rect, at: egui::Pos2, factor: f32) {
        let (x, row) = self.to_floor(rect, at);
        self.zoom = (self.zoom() * factor).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        let now = self.to_screen(rect, x, row);
        self.offset += at - now;
        self.keep_in(rect);
    }

    /// The crew's row brought a quarter of the way up the chart, the floor
    /// across the middle.
    pub fn focus(&mut self, rect: egui::Rect, row: u32) {
        let now = self.to_screen(rect, 0.5, row as f32);
        let want = egui::pos2(rect.center().x, rect.max.y - rect.height() * 0.25);
        self.offset += want - now;
        self.keep_in(rect);
        self.focused = true;
    }

    /// Told how far up `floor` goes, every frame it is up, and brought to
    /// the crew's row `row` the first time.
    pub fn fit(&mut self, rect: egui::Rect, floor: &Floor, row: u32) {
        self.rows = floor.heart_row();
        if !self.focused {
            self.focus(rect, row);
        }
    }

    /// Never panned so far that the floor leaves the chart: the Heart
    /// comes no lower than the middle, the start no higher.
    fn keep_in(&mut self, rect: egui::Rect) {
        let total = self.rows as f32 * self.gap();
        let (low, high) = (
            -rect.height() * 0.5,
            (total - rect.height() * 0.5 + FOOT).max(-rect.height() * 0.5),
        );
        self.offset.y = self.offset.y.clamp(low, high);
        let side = self.span(rect);
        self.offset.x = self.offset.x.clamp(-side, side);
    }

    /// The place under `p`, if any is near enough.
    pub fn hit(&self, rect: egui::Rect, floor: &Floor, p: egui::Pos2) -> Option<(u32, usize)> {
        let reach = (NODE * self.zoom() * 1.5).max(12.0);
        let mut best = None;
        let mut nearest = reach;
        for (row, nodes) in floor.rows.iter().enumerate() {
            for (i, node) in nodes.iter().enumerate() {
                let at = self.place(rect, floor, row as u32, node.x);
                let d = at.distance(p);
                if d < nearest {
                    nearest = d;
                    best = Some((row as u32, i));
                }
            }
        }
        best
    }

    /// Where a place is drawn: the Heart a little above its row.
    fn place(&self, rect: egui::Rect, floor: &Floor, row: u32, x: f32) -> egui::Pos2 {
        let lift = if row == floor.heart_row() { 0.4 } else { 0.0 };
        self.to_screen(rect, x, row as f32 + lift)
    }
}

/// What the chart is drawn with, read off the world once a frame.
pub struct FloorView<'a> {
    pub world: &'a World,
    pub floor: &'a Floor,
    /// Every place's mark (`World::floor_marks`), worked out when the run
    /// moves on.
    pub marks: &'a [FloorMark],
    /// The stars the crew have been to.
    pub visited: &'a [u32],
    /// The place picked, or the one on the table.
    pub heading: Option<u32>,
    /// What the players drew on it, and each player's colour.
    pub sketches: &'a Sketches,
    pub colour_of: &'a dyn Fn(u32) -> egui::Color32,
}

/// The chart, painted into `rect`.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, chart: &FloorChart, view: &FloorView) {
    let FloorView {
        world,
        floor,
        marks,
        visited,
        heading,
        sketches,
        colour_of,
    } = *view;
    let zoom = chart.zoom();
    let gap = chart.gap();
    let heart = floor.heart_row();
    let here = world.floor_at();
    let next = world.floor_next();
    let mark_of = |row: u32, index: usize| {
        marks
            .iter()
            .find(|m| m.row == row && m.index as usize == index)
    };
    painter.rect_filled(rect, 0.0, theme::PANEL_DEEP);
    let left = chart.to_screen(rect, 0.0, 0.0).x - 30.0 * zoom.max(0.6);
    let right = chart.to_screen(rect, 1.0, 0.0).x + 30.0 * zoom.max(0.6);
    let band = |a: f32, b: f32| {
        egui::Rect::from_x_y_ranges(
            rect.min.x..=rect.max.x,
            chart.to_screen(rect, 0.5, b).y..=chart.to_screen(rect, 0.5, a).y,
        )
    };
    // The tiers, a band each in the tier's colour, and where each begins.
    let mut row = 1;
    while row < heart {
        let tier = world.floor_tier(row);
        let mut end = row;
        while end + 1 < heart && world.floor_tier(end + 1) == tier {
            end += 1;
        }
        let colour = tier_colour(tier);
        painter.rect_filled(
            band(row as f32 - 0.5, end as f32 + 0.5),
            0.0,
            colour.gamma_multiply(0.06),
        );
        let y = chart.to_screen(rect, 0.5, row as f32 - 0.5).y;
        painter.line_segment(
            [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
            egui::Stroke::new(1.0, colour.gamma_multiply(0.5)),
        );
        painter.text(
            egui::pos2(rect.max.x - 10.0, y - 3.0),
            egui::Align2::RIGHT_BOTTOM,
            floor_tier_from(tier.code(), world::floor::row_day(row)),
            egui::FontId::proportional(12.0),
            colour,
        );
        // The band's name down its right edge, kept on the screen while any
        // of the band is.
        let top = chart.to_screen(rect, 0.5, end as f32 + 0.5).y;
        let (shown_top, shown_foot) = (top.max(rect.min.y + 110.0), y.min(rect.max.y - 20.0));
        if shown_top < shown_foot {
            painter.text(
                egui::pos2(rect.max.x - 12.0, (shown_top + shown_foot) / 2.0),
                egui::Align2::RIGHT_CENTER,
                floor_tier_band(tier.code()),
                egui::FontId::proportional(18.0),
                colour.gamma_multiply(0.8),
            );
        }
        row = end + 1;
    }
    // The traders' rows.
    for &row in &floor.shop_rows {
        painter.rect_filled(
            band(row as f32 - 0.4, row as f32 + 0.4),
            0.0,
            theme::SITE_TRADER.gamma_multiply(0.07),
        );
        painter.text(
            egui::pos2(right + 8.0, chart.to_screen(rect, 0.5, row as f32).y),
            egui::Align2::LEFT_CENTER,
            FLOOR_TRADERS,
            egui::FontId::proportional(12.0),
            theme::SITE_TRADER.gamma_multiply(0.8),
        );
    }
    // The days, a row each where there is room, every fifth otherwise.
    let every = if gap >= 20.0 {
        1
    } else if gap >= 8.0 {
        5
    } else {
        10
    };
    for row in 1..=heart {
        if row % every != 0 && row != 1 && row != heart {
            continue;
        }
        let y = chart.to_screen(rect, 0.5, row as f32).y;
        if y < rect.min.y - 10.0 || y > rect.max.y + 10.0 {
            continue;
        }
        let fifth = row % 5 == 0;
        painter.text(
            egui::pos2(rect.min.x + GUTTER - 6.0, y),
            egui::Align2::RIGHT_CENTER,
            floor_day(world::floor::row_day(row)),
            egui::FontId::proportional(if fifth { 12.5 } else { 11.0 }),
            if fifth {
                theme::MUTED
            } else {
                theme::MUTED.gamma_multiply(0.6)
            },
        );
        if fifth {
            painter.line_segment(
                [egui::pos2(left, y), egui::pos2(right, y)],
                egui::Stroke::new(1.0, theme::LINE.gamma_multiply(0.5)),
            );
        }
    }
    // The trips: faint, the way taken bright, the ways on from here lit.
    let been = |star: u32| visited.contains(&star);
    for (row, nodes) in floor.rows.iter().enumerate() {
        let row = row as u32;
        for (i, node) in nodes.iter().enumerate() {
            let from = chart.place(rect, floor, row, node.x);
            for &j in &node.up {
                let Some(to_node) = floor.node(row + 1, j as usize) else {
                    continue;
                };
                let to = chart.place(rect, floor, row + 1, to_node.x);
                let at_here = here == Some((row, i));
                let taken =
                    here.is_some_and(|(r, _)| row < r) && been(node.star) && been(to_node.star);
                let stroke = if at_here {
                    let picked = heading == Some(to_node.star);
                    egui::Stroke::new(
                        if picked { 4.0 } else { 2.5 },
                        theme::HYPER.gamma_multiply(if picked { 1.0 } else { 0.8 }),
                    )
                } else if taken {
                    egui::Stroke::new(3.0, theme::YOURS.gamma_multiply(0.85))
                } else {
                    egui::Stroke::new(1.5, theme::MUTED.gamma_multiply(0.35))
                };
                dotted(painter, from, to, stroke, at_here || taken);
            }
        }
    }
    // The places.
    let radius = (NODE * zoom).clamp(6.0, 22.0);
    for (row, nodes) in floor.rows.iter().enumerate() {
        let row = row as u32;
        for (i, node) in nodes.iter().enumerate() {
            let at = chart.place(rect, floor, row, node.x);
            if !rect.expand(40.0).contains(at) {
                continue;
            }
            let hovered = chart.hovered == Some((row, i));
            let r = if hovered { radius * 1.18 } else { radius };
            let mark = mark_of(row, i);
            let site = Site {
                star: node.star,
                station: node.station,
            };
            if row == heart {
                heart_mark(painter, at, r * 1.9);
                painter.text(
                    at - egui::vec2(0.0, r * 2.3 + 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    HEART_NAME,
                    egui::FontId::proportional(15.0),
                    theme::ATTACK,
                );
            } else if row == 0 {
                painter.circle_filled(at, r, theme::PANEL_DEEP);
                painter.circle_stroke(at, r, egui::Stroke::new(2.0, theme::YOURS));
                super::game::station_icon(painter, at, r * 1.2, theme::YOURS);
                painter.text(
                    at + egui::vec2(0.0, r + 6.0),
                    egui::Align2::CENTER_TOP,
                    FLOOR_START,
                    egui::FontId::proportional(13.0),
                    theme::YOURS,
                );
            } else {
                let kind = mark.map(|m| m.kind);
                let cleared = mark.is_some_and(|m| m.cleared);
                let colour = match kind {
                    Some(kind) if !cleared => theme::site_kind_colour(kind),
                    Some(_) => theme::MUTED,
                    None => theme::MUTED.gamma_multiply(0.6),
                };
                let fill = if been(node.star) && here.is_some_and(|(r, _)| row <= r) {
                    theme::YOURS.gamma_multiply(0.25)
                } else {
                    theme::PANEL_DEEP
                };
                if mark.is_some_and(|m| m.elite) {
                    painter.circle_stroke(at, r + 4.0, egui::Stroke::new(2.0, ELITE));
                    crown(painter, at - egui::vec2(0.0, r + 6.0), r * 0.9);
                }
                painter.circle_filled(at, r, fill);
                painter.circle_stroke(at, r, egui::Stroke::new(1.8, colour));
                match kind {
                    Some(SiteKind::Attack) => {
                        super::game::blades_icon(painter, at, r * 1.05, colour)
                    }
                    Some(SiteKind::Defend) => {
                        super::game::shield_icon(painter, at, r * 1.2, colour)
                    }
                    Some(SiteKind::Trader) => {
                        painter.text(
                            at,
                            egui::Align2::CENTER_CENTER,
                            "$",
                            egui::FontId::proportional(r * 1.3),
                            colour,
                        );
                    }
                    None => {}
                }
            }
            if next.contains(&site) {
                let picked = heading == Some(node.star);
                painter.circle_stroke(
                    at,
                    r + if picked { 7.0 } else { 4.0 },
                    egui::Stroke::new(if picked { 3.5 } else { 2.0 }, theme::HYPER),
                );
            }
            if here == Some((row, i)) {
                painter.circle_stroke(at, r + 5.0, egui::Stroke::new(3.0, theme::YOURS));
                if row != 0 {
                    theme::name_over(
                        painter,
                        at + egui::vec2(r + 10.0, 0.0),
                        FLOOR_HERE,
                        theme::YOURS,
                    );
                }
            }
        }
    }
    // The ways the players mean to go, over the places.
    let from = here.map_or(0, |(row, _)| row);
    paint_sketches(painter, rect, chart, floor, sketches, colour_of, from);
    // What the place under the pointer is.
    if let Some((row, i)) = chart.hovered
        && let Some(node) = floor.node(row, i)
    {
        let at = chart.place(rect, floor, row, node.x);
        let site = Site {
            star: node.star,
            station: node.station,
        };
        let mut lines: Vec<(String, egui::Color32)> = Vec::new();
        match mark_of(row, i) {
            Some(m) if m.heart => lines.push((HEART_NAME.to_string(), theme::ATTACK)),
            Some(m) => lines.push((
                if m.kind == SiteKind::Trader {
                    site_kind_word(m.kind).to_string()
                } else {
                    format!(
                        "{} {}",
                        site_kind_word(m.kind),
                        site_place_word(m.site.station)
                    )
                },
                theme::site_kind_colour(m.kind),
            )),
            None if row == 0 => lines.push((FLOOR_START.to_string(), theme::YOURS)),
            None => {}
        }
        let tier = world.floor_tier(row);
        lines.push((
            floor_place_day(world::floor::row_day(row), tier.code()),
            tier_colour(tier),
        ));
        if let Some(m) = mark_of(row, i) {
            if m.elite {
                lines.push((ARRIVE_ELITE.to_string(), ELITE));
            }
            if m.cleared {
                lines.push((ARRIVE_CLEARED.to_string(), theme::MUTED));
            }
        }
        let (word, colour) = if here == Some((row, i)) {
            (FLOOR_HERE, theme::YOURS)
        } else if next.contains(&site) {
            (FLOOR_WAY_UP, theme::HYPER)
        } else {
            (FLOOR_OUT_OF_REACH, theme::MUTED)
        };
        lines.push((word.to_string(), colour));
        tip(
            painter,
            rect,
            at + egui::vec2(radius + 14.0, -radius),
            &lines,
        );
    }
}

/// A line from `a` to `b`, dashed where it is a way not yet taken.
fn dotted(
    painter: &egui::Painter,
    a: egui::Pos2,
    b: egui::Pos2,
    stroke: egui::Stroke,
    solid: bool,
) {
    if solid {
        painter.line_segment([a, b], stroke);
        return;
    }
    let length = a.distance(b);
    let dash = 6.0;
    let n = (length / (dash * 2.0)).floor().max(1.0) as usize;
    for k in 0..n {
        let t0 = (k as f32 * 2.0 * dash) / length;
        let t1 = ((k as f32 * 2.0 + 1.0) * dash / length).min(1.0);
        painter.line_segment([a + (b - a) * t0, a + (b - a) * t1], stroke);
    }
}

/// The Machine Heart: a heart in the enemy's red on a dark disc, `size`
/// across.
fn heart_mark(painter: &egui::Painter, at: egui::Pos2, size: f32) {
    painter.circle_filled(at, size * 0.62, theme::PANEL_DEEP);
    painter.circle_stroke(at, size * 0.62, egui::Stroke::new(2.5, theme::ATTACK));
    let r = size * 0.2;
    let up = at - egui::vec2(0.0, size * 0.08);
    painter.circle_filled(up - egui::vec2(r * 0.95, 0.0), r, theme::ATTACK);
    painter.circle_filled(up + egui::vec2(r * 0.95, 0.0), r, theme::ATTACK);
    painter.add(egui::Shape::convex_polygon(
        vec![
            up + egui::vec2(-r * 1.9, r * 0.35),
            up + egui::vec2(r * 1.9, r * 0.35),
            up + egui::vec2(0.0, r * 2.6),
        ],
        theme::ATTACK,
        egui::Stroke::NONE,
    ));
}

/// An elite's crown, `size` across, its foot at `at`.
fn crown(painter: &egui::Painter, at: egui::Pos2, size: f32) {
    let (w, h) = (size / 2.0, size * 0.55);
    let points = vec![
        at + egui::vec2(-w, 0.0),
        at + egui::vec2(-w, -h),
        at + egui::vec2(-w * 0.5, -h * 0.45),
        at + egui::vec2(0.0, -h),
        at + egui::vec2(w * 0.5, -h * 0.45),
        at + egui::vec2(w, -h),
        at + egui::vec2(w, 0.0),
    ];
    painter.add(egui::Shape::closed_line(
        points,
        egui::Stroke::new(1.8, ELITE),
    ));
}

/// Lines on a dark tag, its top left at `at`, kept inside `rect`.
fn tip(
    painter: &egui::Painter,
    rect: egui::Rect,
    at: egui::Pos2,
    lines: &[(String, egui::Color32)],
) {
    let font = egui::FontId::proportional(13.0);
    let galleys: Vec<_> = lines
        .iter()
        .map(|(text, colour)| painter.layout_no_wrap(text.clone(), font.clone(), *colour))
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y + 2.0).sum();
    let size = egui::vec2(width + 16.0, height + 10.0);
    let min = egui::pos2(
        at.x.min(rect.max.x - size.x - 4.0).max(rect.min.x + 4.0),
        at.y.min(rect.max.y - size.y - 4.0).max(rect.min.y + 4.0),
    );
    let tag = egui::Rect::from_min_size(min, size);
    painter.rect_filled(tag, 5.0, theme::PANEL_DEEP.gamma_multiply(0.95));
    painter.rect_stroke(
        tag,
        5.0,
        egui::Stroke::new(1.0, theme::LINE),
        egui::StrokeKind::Inside,
    );
    let mut y = tag.min.y + 5.0;
    for galley in galleys {
        let h = galley.size().y;
        painter.galley(egui::pos2(tag.min.x + 8.0, y), galley, theme::MUTED);
        y += h + 2.0;
    }
}

/// A tier's colour on the floor: the muted grey for one, the caution
/// colour for two, the attack red for three.
pub fn tier_colour(tier: Tier) -> egui::Color32 {
    match tier {
        Tier::One => theme::MUTED,
        Tier::Two => theme::CAUTION,
        Tier::Three => theme::ATTACK,
    }
}

/// The floor's word in the top left corner: how far up the crew are.
pub fn progress(world: &World, floor: &Floor) -> String {
    floor_progress(world.run_day(), world::floor::row_day(floor.heart_row()))
}

/// Most points in one line; a longer one stops growing.
const STROKE_POINTS: usize = 800;
/// Most lines a player keeps up; a new one past it rubs out the oldest.
const STROKES: usize = 64;
/// Points on the screen the pen moves before the line takes a new point.
const PEN_STEP: f32 = 3.0;
/// Points on the screen from a line that rub it out.
const RUB_REACH: f32 = 12.0;
/// How near, in points of the chart at a zoom of one, a line passes a
/// place's middle and still runs over it.
const OVER: f32 = 17.0;
/// Seconds between a growing line's goings out to the others.
const SKETCH_EVERY: f64 = 0.1;

/// One line drawn on the floor: whose, its number among theirs, and its
/// points of the floor (across it nought to one, up it in rows), so it
/// lies on the same places at every zoom and on every player's chart.
pub struct Stroke {
    pub slot: u32,
    pub id: u32,
    pub points: Vec<(f32, f32)>,
}

/// What the players drew on the floor's map (October 2026): the way each
/// means to go up it, planned ahead the way Slay the Spire's map is drawn
/// on. A right drag draws, Shift and a right drag rubs out this player's
/// own lines. Kept by the screen across the missions — and a resync, a
/// load or a restart — until rubbed out; everybody's in its player's
/// colour, sent over the wire as `Packet::Sketch`. A picture only: the
/// world never hears of it. The places a player's lines run over are
/// ringed in their colour, and the next of this player's is picked for
/// the bar when nothing up the floor is.
#[derive(Default)]
pub struct Sketches {
    strokes: Vec<Stroke>,
    /// This player's line under the pen.
    drawing: Option<u32>,
    /// The pen down but not yet moved off the click's slop: where it
    /// went down, on the floor. A pen lifted here was a click — it picks
    /// the place under it and leaves no line.
    pressed: Option<(f32, f32)>,
    /// The pen is rubbing out rather than drawing: where it was last.
    erasing: Option<egui::Pos2>,
    /// This player's next line's number.
    next: u32,
    /// This player's lines changed since they last went out.
    unsent: Vec<u32>,
    /// When a line last went out, in the egui clock's seconds.
    sent_at: f64,
}

impl Sketches {
    /// A line another player drew, whole as it stands — empty when they
    /// rubbed it out.
    pub fn put(&mut self, slot: u32, id: u32, points: Vec<(f32, f32)>) {
        let at = self
            .strokes
            .iter()
            .position(|s| s.slot == slot && s.id == id);
        match (at, points.is_empty()) {
            (Some(i), true) => {
                self.strokes.remove(i);
            }
            (Some(i), false) => self.strokes[i].points = points,
            (None, true) => {}
            (None, false) => {
                let mut points = points;
                points.truncate(STROKE_POINTS);
                if self.strokes.iter().filter(|s| s.slot == slot).count() >= STROKES
                    && let Some(i) = self.strokes.iter().position(|s| s.slot == slot)
                {
                    self.strokes.remove(i);
                }
                self.strokes.push(Stroke { slot, id, points });
            }
        }
    }

    /// The pen goes down at `at` on the chart, rubbing out with `erase`.
    pub fn press(
        &mut self,
        slot: u32,
        chart: &FloorChart,
        rect: egui::Rect,
        at: egui::Pos2,
        erase: bool,
    ) {
        if erase {
            self.rub(slot, chart, rect, at, at);
            return;
        }
        self.pressed = Some(chart.to_floor(rect, at));
    }

    /// A line begun at `from`, the pen having moved off its click.
    fn begin(&mut self, slot: u32, from: (f32, f32)) {
        if self.strokes.iter().filter(|s| s.slot == slot).count() >= STROKES
            && let Some(i) = self.strokes.iter().position(|s| s.slot == slot)
        {
            let gone = self.strokes.remove(i).id;
            self.unsent.push(gone);
        }
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        self.strokes.push(Stroke {
            slot,
            id,
            points: vec![from],
        });
        self.drawing = Some(id);
        self.unsent.push(id);
    }

    /// The pen moved to `at` while down.
    pub fn drag(&mut self, slot: u32, chart: &FloorChart, rect: egui::Rect, at: egui::Pos2) {
        if let Some(from) = self.erasing {
            self.rub(slot, chart, rect, from, at);
            return;
        }
        if let Some(from) = self.pressed {
            let (x, row) = from;
            if chart.to_screen(rect, x, row).distance(at) < crate::crew::CLICK_SLOP {
                return;
            }
            self.pressed = None;
            self.begin(slot, from);
        }
        let Some(id) = self.drawing else {
            return;
        };
        let Some(stroke) = self
            .strokes
            .iter_mut()
            .find(|s| s.slot == slot && s.id == id)
        else {
            return;
        };
        let last = stroke.points.last().copied();
        let far =
            last.is_none_or(|(x, row)| chart.to_screen(rect, x, row).distance(at) >= PEN_STEP);
        if far && stroke.points.len() < STROKE_POINTS {
            stroke.points.push(chart.to_floor(rect, at));
            if !self.unsent.contains(&id) {
                self.unsent.push(id);
            }
        }
    }

    /// The pen lifted: whether it was a click, never moved off where it
    /// went down, so drew nothing.
    pub fn release(&mut self) -> bool {
        self.drawing = None;
        self.erasing = None;
        self.pressed.take().is_some()
    }

    /// Whether the pen is down.
    pub fn busy(&self) -> bool {
        self.drawing.is_some() || self.erasing.is_some() || self.pressed.is_some()
    }

    /// Whether this player has any line up.
    pub fn any_of(&self, slot: u32) -> bool {
        self.strokes.iter().any(|s| s.slot == slot)
    }

    /// Every line of this player's rubbed out.
    pub fn clear(&mut self, slot: u32) {
        for stroke in self.strokes.iter().filter(|s| s.slot == slot) {
            self.unsent.push(stroke.id);
        }
        self.strokes.retain(|s| s.slot != slot);
        self.drawing = None;
    }

    /// This player's lines near the pen's way from `from` to `at` rubbed
    /// out, and the pen left at `at`.
    fn rub(
        &mut self,
        slot: u32,
        chart: &FloorChart,
        rect: egui::Rect,
        from: egui::Pos2,
        at: egui::Pos2,
    ) {
        self.erasing = Some(at);
        let steps = (from.distance(at) / 4.0).ceil().max(1.0) as usize;
        let pen: Vec<egui::Pos2> = (0..=steps)
            .map(|k| from + (at - from) * (k as f32 / steps as f32))
            .collect();
        let near = |s: &Stroke| {
            let on_screen: Vec<egui::Pos2> = s
                .points
                .iter()
                .map(|&(x, row)| chart.to_screen(rect, x, row))
                .collect();
            pen.iter()
                .any(|&p| distance_to_line(&on_screen, p) < RUB_REACH)
        };
        let gone: Vec<u32> = self
            .strokes
            .iter()
            .filter(|s| s.slot == slot && near(s))
            .map(|s| s.id)
            .collect();
        self.strokes
            .retain(|s| !(s.slot == slot && gone.contains(&s.id)));
        self.unsent.extend(gone);
    }

    /// This player's lines that changed, as they now stand (empty for one
    /// rubbed out), to go out to the others: every [`SKETCH_EVERY`] while
    /// one grows, at once otherwise.
    pub fn outgoing(&mut self, slot: u32, now: f64) -> Vec<(u32, Vec<(f32, f32)>)> {
        if self.unsent.is_empty() || (self.drawing.is_some() && now - self.sent_at < SKETCH_EVERY) {
            return Vec::new();
        }
        self.sent_at = now;
        let mut ids = std::mem::take(&mut self.unsent);
        ids.dedup();
        ids.iter()
            .map(|&id| {
                let points = self
                    .strokes
                    .iter()
                    .find(|s| s.slot == slot && s.id == id)
                    .map_or_else(Vec::new, |s| s.points.clone());
                (id, points)
            })
            .collect()
    }

    /// How near, in points at a zoom of one, `slot`'s lines come to the
    /// place `(row, x)`; `None` when they have none.
    fn nearest(&self, slot: u32, floor: &Floor, row: u32, x: f32) -> Option<f32> {
        let lift = if row == floor.heart_row() { 0.4 } else { 0.0 };
        let flat = |(x, row): (f32, f32)| egui::pos2(x * SPAN, row * ROW_GAP);
        let at = flat((x, row as f32 + lift));
        self.strokes
            .iter()
            .filter(|s| s.slot == slot)
            .map(|s| {
                let points: Vec<egui::Pos2> = s.points.iter().map(|&p| flat(p)).collect();
                distance_to_line(&points, at)
            })
            .reduce(f32::min)
    }

    /// Whether `slot`'s lines run over the place `(row, x)`.
    fn over(&self, slot: u32, floor: &Floor, row: u32, x: f32) -> bool {
        self.nearest(slot, floor, row, x).is_some_and(|d| d < OVER)
    }

    /// The place of the ways up from here that `slot`'s lines run over,
    /// the nearest to them where they run over more than one: the star
    /// the plan picks.
    pub fn planned_next(&self, slot: u32, world: &World, floor: &Floor) -> Option<u32> {
        let next = world.floor_next();
        let mut best: Option<(f32, u32)> = None;
        for (row, nodes) in floor.rows.iter().enumerate() {
            for node in nodes {
                let site = Site {
                    star: node.star,
                    station: node.station,
                };
                if !next.contains(&site) {
                    continue;
                }
                if let Some(d) = self.nearest(slot, floor, row as u32, node.x)
                    && d < OVER
                    && best.is_none_or(|(b, _)| d < b)
                {
                    best = Some((d, node.star));
                }
            }
        }
        best.map(|(_, star)| star)
    }
}

/// How near `p` comes to the line through `points`.
fn distance_to_line(points: &[egui::Pos2], p: egui::Pos2) -> f32 {
    match points {
        [] => f32::INFINITY,
        [only] => only.distance(p),
        _ => points
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                let ab = b - a;
                let t = if ab.length_sq() > 0.0 {
                    ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (a + ab * t).distance(p)
            })
            .fold(f32::INFINITY, f32::min),
    }
}

/// Everybody's lines on the chart, each in its player's colour, and the
/// places up the floor from the crew's row `here` they run over ringed.
fn paint_sketches(
    painter: &egui::Painter,
    rect: egui::Rect,
    chart: &FloorChart,
    floor: &Floor,
    sketches: &Sketches,
    colour_of: &dyn Fn(u32) -> egui::Color32,
    here: u32,
) {
    let zoom = chart.zoom();
    let width = (3.0 * zoom).clamp(2.0, 5.0);
    let radius = (NODE * zoom).clamp(6.0, 22.0);
    let mut slots: Vec<u32> = sketches.strokes.iter().map(|s| s.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    for (k, &slot) in slots.iter().enumerate() {
        let colour = colour_of(slot);
        for (row, nodes) in floor.rows.iter().enumerate() {
            let row = row as u32;
            for node in nodes {
                if row <= here || !sketches.over(slot, floor, row, node.x) {
                    continue;
                }
                let at = chart.place(rect, floor, row, node.x);
                let r = if row == floor.heart_row() {
                    radius * 1.9 * 0.62
                } else {
                    radius
                };
                painter.circle_stroke(
                    at,
                    r + 9.0 + 3.0 * k as f32,
                    egui::Stroke::new(2.0, colour.gamma_multiply(0.9)),
                );
            }
        }
    }
    for stroke in &sketches.strokes {
        let colour = colour_of(stroke.slot);
        let points: Vec<egui::Pos2> = stroke
            .points
            .iter()
            .map(|&(x, row)| chart.to_screen(rect, x, row))
            .collect();
        match points.as_slice() {
            [] => {}
            [only] => {
                painter.circle_filled(*only, width * 0.6, colour);
            }
            _ => {
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(width, colour.gamma_multiply(0.9)),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0))
    }

    #[test]
    fn a_line_drawn_goes_out_whole_and_comes_back_on_another_chart() {
        let chart = FloorChart::default();
        let mut mine = Sketches::default();
        mine.press(0, &chart, rect(), egui::pos2(500.0, 700.0), false);
        for y in [680.0, 660.0, 640.0] {
            mine.drag(0, &chart, rect(), egui::pos2(500.0, y));
        }
        mine.release();
        let sent = mine.outgoing(0, 1.0);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1.len(), 4);
        assert!(mine.outgoing(0, 2.0).is_empty(), "nothing new to say");
        let mut theirs = Sketches::default();
        for (id, points) in sent {
            theirs.put(0, id, points);
        }
        assert!(theirs.any_of(0) && !theirs.any_of(1));
    }

    #[test]
    fn a_click_draws_nothing_and_says_so() {
        let chart = FloorChart::default();
        let mut sketches = Sketches::default();
        sketches.press(0, &chart, rect(), egui::pos2(500.0, 700.0), false);
        sketches.drag(0, &chart, rect(), egui::pos2(502.0, 701.0));
        assert!(sketches.release(), "a pen that never left its click");
        assert!(!sketches.any_of(0));
        assert!(sketches.outgoing(0, 1.0).is_empty());
        sketches.press(0, &chart, rect(), egui::pos2(500.0, 700.0), false);
        sketches.drag(0, &chart, rect(), egui::pos2(500.0, 650.0));
        assert!(!sketches.release(), "a drag is a line");
        assert!(sketches.any_of(0));
    }

    #[test]
    fn a_line_rubbed_out_goes_out_empty_and_only_the_own_are_rubbed() {
        let chart = FloorChart::default();
        let mut sketches = Sketches::default();
        sketches.put(1, 0, vec![(0.5, 1.0), (0.5, 3.0)]);
        sketches.press(0, &chart, rect(), egui::pos2(500.0, 700.0), false);
        sketches.drag(0, &chart, rect(), egui::pos2(500.0, 600.0));
        sketches.release();
        sketches.outgoing(0, 1.0);
        // Shift and a right drag straight across both.
        sketches.press(0, &chart, rect(), egui::pos2(400.0, 650.0), true);
        sketches.drag(0, &chart, rect(), egui::pos2(600.0, 650.0));
        sketches.release();
        assert!(!sketches.any_of(0));
        assert!(
            sketches.any_of(1),
            "another player's line is theirs to rub out"
        );
        let sent = sketches.outgoing(0, 2.0);
        assert_eq!(sent.len(), 1);
        assert!(sent[0].1.is_empty());
        // And an empty one from the wire rubs theirs out here.
        sketches.put(1, 0, Vec::new());
        assert!(!sketches.any_of(1));
    }
}

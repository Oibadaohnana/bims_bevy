//! The look: the pages' palette, egui's visuals set to it once, and the
//! handful of widgets every screen draws the same way.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

/// The window behind everything: the void the ship sits in.
pub const VOID: Color = Color::srgb(0.047, 0.071, 0.063);

pub const INK: egui::Color32 = egui::Color32::from_rgb(0xe7, 0xef, 0xe9);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(0x8f, 0xa8, 0x9a);
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x7f, 0xd1, 0xa8);
pub const WARN: egui::Color32 = egui::Color32::from_rgb(0xfa, 0x73, 0x52);
pub const BAD: egui::Color32 = egui::Color32::from_rgb(0xff, 0x8f, 0x7a);
pub const CAUTION: egui::Color32 = egui::Color32::from_rgb(0xff, 0xd7, 0xa6);
pub const GRAVE: egui::Color32 = egui::Color32::from_rgb(0xff, 0xd1, 0x5a);
pub const PANEL: egui::Color32 = egui::Color32::from_rgb(0x18, 0x21, 0x1d);
pub const PANEL_DEEP: egui::Color32 = egui::Color32::from_rgb(0x0c, 0x12, 0x10);
pub const RAISED: egui::Color32 = egui::Color32::from_rgb(0x22, 0x33, 0x2b);
pub const RAISED_ON: egui::Color32 = egui::Color32::from_rgb(0x4a, 0x6d, 0x5c);
pub const LINE: egui::Color32 = egui::Color32::from_rgb(0x2a, 0x3b, 0x33);
pub const YOURS: egui::Color32 = ACCENT;
pub const THEIRS: egui::Color32 = INK;
/// The attack banner and the armed pointer that puts one down (feature
/// 84): the enemy's own red, since what both mean is a fight, and one
/// no other mark on the deck wears.
pub const ATTACK: egui::Color32 = egui::Color32::from_rgb(0xff, 0x5e, 0x4a);
/// The **defend sign** over the ship while the crew are falling back to
/// it (feature 84): the armour blue, since what it means is cover and
/// not a fight, and the one mark on the deck the attack banner's red
/// could never be mistaken for.
pub const DEFEND: egui::Color32 = egui::Color32::from_rgb(0x6f, 0xa8, 0xe8);
/// A medic's heal beam and the surge on a body (feature 76): a pale
/// healing green, the room's own `character::SURGE`.
pub const HEAL: egui::Color32 = egui::Color32::from_rgb(0x8c, 0xf2, 0xbf);
/// The red of the cross over a dying Bim and of the peril block on its
/// panel: a proper signal red rather than the palette's salmon [`BAD`],
/// because a cross on a lit deck has to be picked out across the room
/// rather than merely noticed once the eye is already on it.
pub const DYING: egui::Color32 = egui::Color32::from_rgb(0xe8, 0x2a, 0x24);
pub const SLEEP: egui::Color32 = egui::Color32::from_rgb(0x3f, 0x6e, 0xa8);
pub const ANY: egui::Color32 = egui::Color32::from_rgb(0x4a, 0x55, 0x60);
/// Armour: the blue on the end of a health bar, and a piece's own health
/// under its icon.
pub const ARMOUR: egui::Color32 = egui::Color32::from_rgb(0x6f, 0xa8, 0xe8);
/// Equipment tiers: a tier-two piece or weapon is tinted blue in its cell
/// and slot, a tier-three one gold; tier one is untinted.
pub const TIER_TWO: egui::Color32 = egui::Color32::from_rgb(0x5a, 0x9c, 0xf0);
pub const TIER_THREE: egui::Color32 = egui::Color32::from_rgb(0xf0, 0xc4, 0x4a);
/// The numbers the electricity view writes over the drainers: what each
/// draws, in the yellow of a meter's needle.
pub const DRAW: egui::Color32 = egui::Color32::from_rgb(0xff, 0xe0, 0x3c);
/// The hyperdrive's violet: the star picked on the galaxy chart, and the
/// charge bar.
pub const HYPER: egui::Color32 = egui::Color32::from_rgb(0x9e, 0x6b, 0xdb);
/// A landable planet's name on the map: the friendly blue the map rings
/// a station in (`ship::world_paint::FRIEND`, the same three numbers),
/// so the words and the ring say the same thing; a hostile one's is
/// `BAD`.
pub const LAND: egui::Color32 = egui::Color32::from_rgb(0x5c, 0x8c, 0xff);
pub const NAME_STROKE: egui::Color32 = egui::Color32::from_rgba_premultiplied(6, 10, 9, 217);

/// The name over a Bim's head: how big, and how far above the body it
/// sits. The lift is in room units and scaled with the view, so the name
/// stays over the head at any window size; the size is in points, because
/// type that scaled with the room would go illegible on a small window.
pub const NAME_SIZE: f32 = 13.5;
pub const NAME_LIFT: f32 = 46.0;

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, style.run_if(run_once));
    }
}

fn style(mut contexts: EguiContexts) -> Result {
    let ctx = contexts.ctx_mut()?;
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = PANEL_DEEP;
    visuals.faint_bg_color = RAISED;
    visuals.override_text_color = Some(INK);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = RAISED_ON;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE);
    visuals.widgets.inactive.bg_fill = RAISED;
    visuals.widgets.inactive.weak_bg_fill = RAISED;
    visuals.widgets.hovered.bg_fill = RAISED_ON;
    visuals.widgets.hovered.weak_bg_fill = RAISED_ON;
    visuals.widgets.active.bg_fill = RAISED_ON;
    visuals.widgets.active.weak_bg_fill = RAISED_ON;
    visuals.window_stroke = egui::Stroke::new(1.0, LINE);
    visuals.window_corner_radius = egui::CornerRadius::same(6);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        // The type, a size up from egui's own throughout: the panels are read
        // at arm's length over a deck, and the default is sized for a form.
        // The scale setting on the Esc sheet ([`ui_scale_row`]) multiplies
        // everything on top of this.
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(10.5, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(21.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(14.0, FontFamily::Monospace),
            ),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        // Tooltips are asked for, never stumbled into: long enough that
        // crossing a panel sets nothing off, short enough that asking feels
        // like no wait at all.
        style.interaction.tooltip_delay = 0.3;
        style.interaction.tooltip_grace_time = 0.1;
    });
    Ok(())
}

// --- widgets -----------------------------------------------------------------

/// A section heading, the pages' `h2`.
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .small()
            .color(MUTED)
            .strong(),
    );
}

/// A word that already names the thing, underlined, with its explanation
/// hanging off it. The word is the affordance.
pub fn asks(ui: &mut egui::Ui, word: &str, tip: &str) -> egui::Response {
    ui.add(egui::Label::new(egui::RichText::new(word).underline()).sense(egui::Sense::hover()))
        .on_hover_cursor(egui::CursorIcon::Help)
        .on_hover_text(tip)
}

/// A "?" to put beside something that has no word of its own to underline.
pub fn question_mark(ui: &mut egui::Ui, tip: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new("?").small()).min_size(egui::vec2(17.0, 17.0)))
        .on_hover_cursor(egui::CursorIcon::Help)
        .on_hover_text(tip)
}

/// A bar: a fraction of a strip, with the track behind it.
pub fn bar(ui: &mut egui::Ui, width: f32, fraction: f32, fill: egui::Color32) -> egui::Response {
    bar_of_height(ui, width, 8.0, &[(fraction, fill)])
}

/// The same bar half as tall, for a strip that is a detail of something
/// rather than a thing in its own right — a job's progress along the
/// bottom of its row.
pub fn thin_bar(
    ui: &mut egui::Ui,
    width: f32,
    fraction: f32,
    fill: egui::Color32,
) -> egui::Response {
    bar_of_height(ui, width, 4.0, &[(fraction, fill)])
}

/// A bar in two tones: `first` of the strip in `fill`, and `second` of it
/// in `fill2` set on the end of that — the body's health in green with
/// the armour's in blue after it. Both fractions are of the *whole*
/// strip, so the two together are the Bim's total.
pub fn two_tone_bar(
    ui: &mut egui::Ui,
    width: f32,
    first: f32,
    second: f32,
    fill: egui::Color32,
    fill2: egui::Color32,
) -> egui::Response {
    bar_of_height(ui, width, 8.0, &[(first, fill), (second, fill2)])
}

/// Health's own bar, half again as tall as a need's and drawn in two
/// tones like [`two_tone_bar`]. It is the one bar a player watches in a
/// fight, so it is the one bar that is not the same size as the rest.
pub fn health_bar(
    ui: &mut egui::Ui,
    width: f32,
    first: f32,
    second: f32,
    fill: egui::Color32,
    fill2: egui::Color32,
) -> egui::Response {
    bar_of_height(ui, width, 13.0, &[(first, fill), (second, fill2)])
}

/// The bar behind them all: the track, then each segment laid end to end
/// from the left, clipped at the far end of the strip.
fn bar_of_height(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    segments: &[(f32, egui::Color32)],
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    let round = height * 0.375;
    painter.rect_filled(rect, round, RAISED);
    let mut from = rect.min.x;
    for &(fraction, fill) in segments {
        let to = (from + rect.width() * fraction.clamp(0.0, 1.0)).min(rect.max.x);
        if to <= from {
            continue;
        }
        let done =
            egui::Rect::from_min_max(egui::pos2(from, rect.min.y), egui::pos2(to, rect.max.y));
        painter.rect_filled(done, round, fill);
        from = to;
    }
    response
}

/// A tiny bar with no track spacing of its own, painted into a rect the
/// caller has: a piece's health under its icon in a grid cell.
pub fn bar_in(painter: &egui::Painter, rect: egui::Rect, fraction: f32, fill: egui::Color32) {
    let round = rect.height() * 0.375;
    painter.rect_filled(rect, round, RAISED);
    let mut done = rect;
    done.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    painter.rect_filled(done, round, fill);
}

/// The tint a tier is drawn in: `None` for tier one, which is what
/// everything is unless it has been through the workbench.
pub fn tier_tint(tier: bims::combat::Tier) -> Option<egui::Color32> {
    match tier {
        bims::combat::Tier::One => None,
        bims::combat::Tier::Two => Some(TIER_TWO),
        bims::combat::Tier::Three => Some(TIER_THREE),
    }
}

/// The tint an item is drawn in, if it has a tier above one: a weapon's or
/// a piece's, and a research key's by its tier; a stack has none.
pub fn item_tint(item: bims::combat::Item) -> Option<egui::Color32> {
    match item {
        bims::combat::Item::Armour(piece) => tier_tint(piece.tier),
        bims::combat::Item::Weapon(weapon) => tier_tint(weapon.tier),
        bims::combat::Item::Key(2) => Some(TIER_TWO),
        bims::combat::Item::Stack(_) | bims::combat::Item::Key(_) => None,
    }
}

/// A tiered item's cell: the tint washed over the fill and the border
/// stroked in it, over whatever the cell was drawn as. The icon on top is
/// unchanged, so a gold pistol is the pistol in a gold cell.
pub fn tint_cell(painter: &egui::Painter, rect: egui::Rect, round: f32, tint: egui::Color32) {
    painter.rect(
        rect,
        round,
        egui::Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), 48),
        egui::Stroke::new(1.5, tint),
        egui::StrokeKind::Inside,
    );
}

// --- pop-up menus -----------------------------------------------------------

/// One row of a pop-up menu: what it does, a line under it saying why or
/// with what, and whether it can be pressed at all.
pub struct Row {
    pub label: String,
    pub hint: String,
    pub disabled: bool,
}

impl Row {
    pub fn new(label: impl Into<String>, hint: impl Into<String>, disabled: bool) -> Row {
        Row {
            label: label.into(),
            hint: hint.into(),
            disabled,
        }
    }
}

/// A small menu at a point on the window — a fixture's, a grid cell's —
/// one button a row, disabled rows dimmed with their hint still legible.
/// Answers the row pressed, if one was, and the rect the menu took, so the
/// caller can shut it on a press anywhere else.
pub fn popup(
    ctx: &egui::Context,
    id: &str,
    at: egui::Pos2,
    rows: &[Row],
) -> (Option<usize>, egui::Rect) {
    let mut chosen = None;
    let response = egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(PANEL).show(ui, |ui| {
                ui.set_min_width(200.0);
                for (i, row) in rows.iter().enumerate() {
                    let mut text = egui::text::LayoutJob::default();
                    text.append(
                        &row.label,
                        0.0,
                        egui::TextFormat {
                            color: if row.disabled { MUTED } else { INK },
                            ..Default::default()
                        },
                    );
                    if !row.hint.is_empty() {
                        text.append(
                            &format!("\n{}", row.hint),
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::proportional(11.0),
                                color: MUTED,
                                ..Default::default()
                            },
                        );
                    }
                    let button = egui::Button::new(text)
                        .frame(false)
                        .min_size(egui::vec2(200.0, 0.0));
                    if ui.add_enabled(!row.disabled, button).clicked() {
                        chosen = Some(i);
                    }
                }
            });
        });
    (chosen, response.response.rect)
}

/// A button that is marked as the one in force.
pub fn toggle(ui: &mut egui::Ui, on: bool, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(toggle_button(on, text))
}

/// The same, as a widget, for a caller that wants to `add_enabled` it.
pub fn toggle_button(on: bool, text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    let text: egui::WidgetText = text.into();
    if on {
        egui::Button::new(text).fill(RAISED_ON)
    } else {
        egui::Button::new(text)
    }
}

/// A big button, the pages' `.big`.
pub fn big(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).size(17.0)).min_size(egui::vec2(130.0, 32.0)),
    )
}

/// The sizes the whole window can be shown at, as multiples of the
/// display's own: egui's zoom factor, which scales the type, the panels and
/// the canvas between them alike, and which bevy_egui reads back so the
/// pointer lands where it looks. One is the display as it is.
pub const UI_SCALES: [(f32, &str); 5] = [
    (1.0, "100%"),
    (1.25, "125%"),
    (1.5, "150%"),
    (1.75, "175%"),
    (2.0, "200%"),
];

/// The UI scale, chosen: a row of the sizes with the one in force marked.
/// On the Esc sheet, and nowhere else — it is the one setting there is.
pub fn ui_scale_row(ui: &mut egui::Ui) {
    let now = ui.ctx().zoom_factor();
    let mut pick = None;
    ui.horizontal(|ui| {
        for (scale, label) in UI_SCALES {
            if toggle(ui, (now - scale).abs() < 0.01, label).clicked() {
                pick = Some(scale);
            }
        }
    });
    if let Some(scale) = pick {
        ui.ctx().set_zoom_factor(scale);
    }
}

/// A swatch of colour, for a palette row or a legend.
pub fn swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, color);
}

pub fn ship_color32(c: ship::draw::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    )
}

/// A name over a head, on the background layer so it sits over the deck
/// and under every panel. Stroked first in the void's own darkness, so it
/// stays legible over a pale part as readily as over the floor.
pub fn name_over(painter: &egui::Painter, at: egui::Pos2, name: &str, color: egui::Color32) {
    let font = egui::FontId::proportional(NAME_SIZE);
    for (dx, dy) in [
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.0, -1.0),
        (0.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
    ] {
        painter.text(
            at + egui::vec2(dx, dy),
            egui::Align2::CENTER_BOTTOM,
            name,
            font.clone(),
            NAME_STROKE,
        );
    }
    painter.text(at, egui::Align2::CENTER_BOTTOM, name, font, color);
}

/// The tag on a bunk (feature 61): whose it is, or that it is nobody's,
/// written small across the middle of the bed on the background layer,
/// stroked in the void's darkness like a name so it reads over the
/// mattress. `at` is the bunk's middle; a name over a sleeper's head is
/// lifted well clear of it.
pub fn bunk_tag(painter: &egui::Painter, at: egui::Pos2, words: &str, color: egui::Color32) {
    let font = egui::FontId::proportional(NAME_SIZE * 0.8);
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        painter.text(
            at + egui::vec2(dx, dy),
            egui::Align2::CENTER_CENTER,
            words,
            font.clone(),
            NAME_STROKE,
        );
    }
    painter.text(at, egui::Align2::CENTER_CENTER, words, font, color);
}

/// Another player's pointer, where it is over the ship (feature 60): an
/// arrow in their colour, see-through so the deck under it still reads,
/// with their Bim's name small beside it. Drawn in points, so it is the
/// same size at any zoom, like the names.
pub fn ghost_pointer(painter: &egui::Painter, at: egui::Pos2, color: egui::Color32, name: &str) {
    // The head is a triangle and the tail a stroke: an arrow is concave,
    // and epaint fills a polygon as if it were convex.
    let head: Vec<egui::Pos2> = [(0.0, 0.0), (0.0, 15.0), (11.0, 11.0)]
        .into_iter()
        .map(|(dx, dy)| at + egui::vec2(dx, dy))
        .collect();
    let tail = [at + egui::vec2(4.5, 10.5), at + egui::vec2(8.0, 18.0)];
    let fill = color.gamma_multiply(GHOST_POINTER_ALPHA);
    let dark = NAME_STROKE.gamma_multiply(GHOST_POINTER_ALPHA);
    painter.line_segment(tail, egui::Stroke::new(5.0, dark));
    painter.add(egui::Shape::convex_polygon(
        head.clone(),
        dark,
        egui::Stroke::new(2.0, dark),
    ));
    painter.line_segment(tail, egui::Stroke::new(3.0, fill));
    painter.add(egui::Shape::convex_polygon(head, fill, egui::Stroke::NONE));
    if !name.is_empty() {
        let font = egui::FontId::proportional(NAME_SIZE * 0.85);
        let label = at + egui::vec2(14.0, 12.0);
        painter.text(
            label + egui::vec2(1.0, 1.0),
            egui::Align2::LEFT_TOP,
            name,
            font.clone(),
            NAME_STROKE.gamma_multiply(GHOST_POINTER_ALPHA),
        );
        painter.text(label, egui::Align2::LEFT_TOP, name, font, fill);
    }
}

/// How see-through another player's pointer is.
const GHOST_POINTER_ALPHA: f32 = 0.6;

/// A mark over a name — the `?` over a mercenary for hire: a small disc in
/// the void's darkness with the glyph on it in `color`, on the background
/// layer like [`name_over`]. `at` is the bottom middle, as for a name.
pub fn badge_over(painter: &egui::Painter, at: egui::Pos2, mark: &str, color: egui::Color32) {
    let r = NAME_SIZE * 0.62;
    let centre = at - egui::vec2(0.0, r);
    painter.circle(centre, r, NAME_STROKE, egui::Stroke::new(1.0, color));
    painter.text(
        centre,
        egui::Align2::CENTER_CENTER,
        mark,
        egui::FontId::proportional(NAME_SIZE),
        color,
    );
}

/// A medic's heal beam (feature 76): a line from the medic to the
/// patient in the beam's green, a wide faint one under a bright thin
/// one, with a pip at each end — so it reads as a beam and not as a
/// queued walk's dashes.
pub fn heal_beam(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, scale: f32) {
    let wide = (5.0 * scale).clamp(2.0, 9.0);
    let thin = (1.6 * scale).clamp(1.0, 3.0);
    painter.line_segment(
        [from, to],
        egui::Stroke::new(wide, HEAL.gamma_multiply(0.22)),
    );
    painter.line_segment(
        [from, to],
        egui::Stroke::new(thin, HEAL.gamma_multiply(0.9)),
    );
    for end in [from, to] {
        painter.circle_filled(end, thin * 1.8, HEAL);
    }
}

/// The mark on a body a surge is running on (feature 76): a ring in the
/// beam's green outside the body, wider than the deck's own halo so it
/// reads at any zoom.
pub fn surge_mark(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let radius = (34.0 * scale).clamp(8.0, 40.0);
    painter.circle_filled(at, radius, HEAL.gamma_multiply(0.10));
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new((2.0 * scale).clamp(1.0, 3.0), HEAL.gamma_multiply(0.85)),
    );
}

/// The cross over a Bim in a dying state: a part of it at nothing with
/// the trauma on it untreated, which is the one condition a crewmate
/// with a medkit is the answer to. A white disc with a red cross on it,
/// the medical sign rather than another coloured ring, because every
/// other mark on this deck is a ring of some colour and this one has to
/// say *that* one instead of *one of those*. Drawn over the head — the
/// body itself stays visible — and sized off the zoom but clamped, so it
/// is still legible with the whole station in view.
pub fn dying_cross(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let radius = (14.0 * scale).clamp(6.0, 20.0);
    let at = egui::pos2(at.x, at.y - (0.6 * NAME_LIFT * scale).clamp(8.0, 34.0));
    painter.circle_filled(at, radius * 1.12, egui::Color32::from_black_alpha(90));
    painter.circle_filled(at, radius, egui::Color32::from_rgb(0xf6, 0xf8, 0xf6));
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new((1.5 * scale).clamp(1.0, 2.5), DYING),
    );
    // The cross itself: an arm of the disc across and an arm down, each
    // rounded, so it reads as the sign and not as a plus.
    let arm = radius * 0.72;
    let thick = radius * 0.30;
    let round = thick * 0.5;
    painter.rect_filled(
        egui::Rect::from_center_size(at, egui::vec2(arm * 2.0, thick * 2.0)),
        round,
        DYING,
    );
    painter.rect_filled(
        egui::Rect::from_center_size(at, egui::vec2(thick * 2.0, arm * 2.0)),
        round,
        DYING,
    );
}

/// A tank standing as a wall (feature 77): a thick arc of shield round
/// the body out to the bulwark's reach, in the caution colour, so a
/// wall up reads at a glance and reads differently from a surge's ring.
pub fn wall_mark(painter: &egui::Painter, at: egui::Pos2, radius: f32, scale: f32) {
    let radius = radius.max(6.0);
    painter.circle_filled(at, radius, CAUTION.gamma_multiply(0.10));
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new((3.0 * scale).clamp(1.5, 5.0), CAUTION.gamma_multiply(0.75)),
    );
}

/// A tank's taunt while it runs (feature 77): the radius it reaches, a
/// dashed ring in the warning colour — the enemy inside it is shooting
/// at him.
pub fn taunt_ring(painter: &egui::Painter, at: egui::Pos2, radius: f32) {
    let radius = radius.max(8.0);
    let steps = 48;
    for i in (0..steps).step_by(2) {
        let a = i as f32 / steps as f32 * std::f32::consts::TAU;
        let b = (i + 1) as f32 / steps as f32 * std::f32::consts::TAU;
        let p = |t: f32| egui::pos2(at.x + radius * t.cos(), at.y + radius * t.sin());
        painter.line_segment(
            [p(a), p(b)],
            egui::Stroke::new(2.0, WARN.gamma_multiply(0.8)),
        );
    }
}

/// A commander's aura (feature 78): the radius it lifts every friendly
/// Bim within, drawn faintly — it is always on, so it must not shout —
/// as a thin ring with a wash inside it, in the crew's own colour.
pub fn aura_ring(painter: &egui::Painter, at: egui::Pos2, radius: f32) {
    let radius = radius.max(8.0);
    painter.circle_filled(at, radius, YOURS.gamma_multiply(0.05));
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new(1.0, YOURS.gamma_multiply(0.35)),
    );
}

/// A Bim the aura lifts (feature 78): a small open ring under it, in
/// the same colour — enough to say "this one is in it" and no more.
pub fn lifted_mark(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let radius = (10.0 * scale).clamp(4.0, 16.0);
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new(1.0, YOURS.gamma_multiply(0.55)),
    );
}

/// A squad member under a commander's order (feature 78): a short
/// bracket over its head in the caution colour, and a thread to what
/// the order is about — the enemy it was sent at, or the tile it was
/// called back to. `to` is `None` for stand ground, which is about
/// nowhere but where it stands.
pub fn squad_mark(painter: &egui::Painter, at: egui::Pos2, to: Option<egui::Pos2>, scale: f32) {
    let r = (9.0 * scale).clamp(4.0, 14.0);
    let stroke = egui::Stroke::new((1.5 * scale).clamp(1.0, 2.5), CAUTION.gamma_multiply(0.8));
    painter.line_segment(
        [
            egui::pos2(at.x - r, at.y - r),
            egui::pos2(at.x, at.y - r * 1.4),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(at.x, at.y - r * 1.4),
            egui::pos2(at.x + r, at.y - r),
        ],
        stroke,
    );
    let Some(to) = to else {
        return;
    };
    // A dashed thread, like the queued walks': the order has a place.
    let step = (to - at) / 16.0;
    for i in (0..16).step_by(2) {
        let a = at + step * i as f32;
        let b = at + step * (i + 1) as f32;
        painter.line_segment([a, b], egui::Stroke::new(1.0, CAUTION.gamma_multiply(0.5)));
    }
}

/// A commander's **fall back** (feature 86, for the ability box and the
/// deck): two chevrons pointing back at a bar — the order to give
/// ground to a line. Laid out in a box `radius` from the middle, the way
/// every other box mark is.
pub fn fall_back_mark(painter: &egui::Painter, at: egui::Pos2, radius: f32) {
    let stroke = egui::Stroke::new((radius * 0.16).clamp(1.0, 3.0), CAUTION);
    // The line held, on the left.
    painter.line_segment(
        [
            egui::pos2(at.x - radius, at.y - radius * 0.8),
            egui::pos2(at.x - radius, at.y + radius * 0.8),
        ],
        stroke,
    );
    // And two chevrons walking back to it.
    for n in 0..2 {
        let x = at.x + radius * (0.1 + 0.55 * n as f32);
        painter.line_segment(
            [
                egui::pos2(x, at.y - radius * 0.6),
                egui::pos2(x - radius * 0.45, at.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(x - radius * 0.45, at.y),
                egui::pos2(x, at.y + radius * 0.6),
            ],
            stroke,
        );
    }
}

/// A commander's **stand ground** (feature 86): a body's bracket planted
/// on a line — hold exactly where you are. The bracket is the squad
/// mark's own shape, so the two read as one family.
pub fn stand_ground_mark(painter: &egui::Painter, at: egui::Pos2, radius: f32) {
    let stroke = egui::Stroke::new((radius * 0.16).clamp(1.0, 3.0), CAUTION);
    let base = at.y + radius * 0.7;
    painter.line_segment(
        [
            egui::pos2(at.x - radius, base),
            egui::pos2(at.x + radius, base),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(at.x - radius * 0.7, at.y - radius * 0.2),
            egui::pos2(at.x, at.y - radius * 0.8),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(at.x, at.y - radius * 0.8),
            egui::pos2(at.x + radius * 0.7, at.y - radius * 0.2),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(at.x, at.y - radius * 0.8),
            egui::pos2(at.x, base),
        ],
        stroke,
    );
}

/// A medic's **carry** (feature 86): a body lying across two arms — a
/// capsule with a bar under each end of it, which is a stretcher seen
/// from above and reads as one at box size.
pub fn carry_mark(painter: &egui::Painter, at: egui::Pos2, radius: f32) {
    let stroke = egui::Stroke::new((radius * 0.16).clamp(1.0, 3.0), HEAL);
    let body = egui::Rect::from_center_size(
        egui::pos2(at.x, at.y - radius * 0.2),
        egui::vec2(radius * 1.7, radius * 0.7),
    );
    painter.rect_filled(body, radius * 0.35, HEAL.gamma_multiply(0.55));
    painter.circle_filled(
        egui::pos2(body.min.x + radius * 0.1, body.center().y),
        radius * 0.3,
        HEAL,
    );
    // The two arms under it.
    for n in 0..2 {
        let y = at.y + radius * (0.45 + 0.35 * n as f32);
        painter.line_segment(
            [
                egui::pos2(at.x - radius * 0.9, y),
                egui::pos2(at.x + radius * 0.9, y),
            ],
            stroke,
        );
    }
}

/// A Bim a cast of the commander's is **working on** (feature 86): a
/// ring on the ground under it, brighter than [`lifted_mark`]'s and in
/// the caution colour the squad's bracket is in, drawn while the
/// pointer rests on the box for that cast. It is the panels' rule said
/// on the deck — resting on a row rings what it names — and it is what
/// answers "who does this reach".
pub fn affected_ring(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let radius = (13.0 * scale).clamp(6.0, 20.0);
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new((2.0 * scale).clamp(1.2, 3.0), CAUTION),
    );
    painter.circle_stroke(
        at,
        radius * 0.7,
        egui::Stroke::new(1.0, CAUTION.gamma_multiply(0.45)),
    );
}

/// A Bim a **rally** is lifting (feature 86): the aura's own ring with a
/// chevron over it, so a rally running is told from the aura standing.
/// Drawn in place of [`lifted_mark`] for as long as the rally does.
pub fn rallied_mark(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let radius = (10.0 * scale).clamp(4.0, 16.0);
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new(1.0, YOURS.gamma_multiply(0.9)),
    );
    let r = radius * 0.8;
    let stroke = egui::Stroke::new((1.4 * scale).clamp(1.0, 2.4), YOURS);
    painter.line_segment(
        [
            egui::pos2(at.x - r, at.y - radius * 0.9),
            egui::pos2(at.x, at.y - radius * 1.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(at.x, at.y - radius * 1.5),
            egui::pos2(at.x + r, at.y - radius * 0.9),
        ],
        stroke,
    );
}

/// An **attack banner** on the deck (feature 84): a staff standing on
/// the tile with a pennant flying off it, and a ring on the ground round
/// its foot — the ground the crew that follow you are fighting for. Red,
/// steady, and drawn over the deck under every body, so a banner behind
/// a Bim is still a banner.
pub fn attack_banner(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let h = (26.0 * scale).clamp(12.0, 40.0);
    let w = (14.0 * scale).clamp(7.0, 22.0);
    let stroke = egui::Stroke::new((2.0 * scale).clamp(1.2, 3.0), ATTACK);
    let foot = at;
    let head = egui::pos2(at.x, at.y - h);
    painter.circle_filled(foot, w * 0.32, ATTACK.gamma_multiply(0.35));
    painter.circle_stroke(
        foot,
        w * 0.55,
        egui::Stroke::new(stroke.width * 0.8, ATTACK.gamma_multiply(0.8)),
    );
    painter.line_segment([foot, head], stroke);
    // The pennant: a triangle off the top of the staff, filled faintly
    // and outlined, so it reads at a glance and never hides a body.
    let tip = egui::pos2(at.x + w, at.y - h * 0.78);
    let low = egui::pos2(at.x, at.y - h * 0.56);
    painter.add(egui::Shape::convex_polygon(
        vec![head, tip, low],
        ATTACK.gamma_multiply(0.45),
        egui::Stroke::new(stroke.width * 0.7, ATTACK),
    ));
}

/// The **defend sign** on the deck (feature 84): a shield standing on
/// the spot a fall back gathers on — the deck just inside the ship's own
/// airlock — with a ring on the ground round its foot, the attack
/// banner's shape said the other way about. Blue, steady, and drawn over
/// the deck under every body, so a crew member standing on the spot does
/// not hide it. It is up exactly while a retreat is called, which is how
/// the player sees at a glance that the crew are coming home and where
/// they are coming home to.
pub fn defend_banner(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let h = (34.0 * scale).clamp(18.0, 52.0);
    let w = (23.0 * scale).clamp(12.0, 35.0);
    let stroke = egui::Stroke::new((2.4 * scale).clamp(1.5, 3.6), DEFEND);
    let foot = at;
    painter.circle_filled(foot, w * 0.32, DEFEND.gamma_multiply(0.35));
    painter.circle_stroke(
        foot,
        w * 0.55,
        egui::Stroke::new(stroke.width * 0.8, DEFEND.gamma_multiply(0.8)),
    );
    // The shield: shoulders square across the top, sides falling in, a
    // point at the bottom. Drawn as one convex polygon so it reads at
    // any size, filled faintly and outlined.
    let top = at.y - h;
    let mid = at.y - h * 0.34;
    let tip = at.y - h * 0.06;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(at.x - w * 0.5, top),
            egui::pos2(at.x + w * 0.5, top),
            egui::pos2(at.x + w * 0.42, mid),
            egui::pos2(at.x, tip),
            egui::pos2(at.x - w * 0.42, mid),
        ],
        DEFEND.gamma_multiply(0.45),
        stroke,
    ));
    // And the bar across it, which is what says *shield* rather than
    // *pennant* at a glance on a small canvas.
    painter.line_segment(
        [
            egui::pos2(at.x - w * 0.34, at.y - h * 0.72),
            egui::pos2(at.x + w * 0.34, at.y - h * 0.72),
        ],
        egui::Stroke::new(stroke.width * 0.8, DEFEND),
    );
}

/// The burst a soldier is aiming a grenade at (feature 75): the radius
/// round the tile under the pointer, in canvas pixels, a faint wash and
/// a ring — the caution colour while the world would take the throw,
/// the warning one when it would refuse it.
pub fn burst_ring(painter: &egui::Painter, at: egui::Pos2, radius: f32, ok: bool) {
    let color = if ok { CAUTION } else { WARN };
    painter.circle_filled(at, radius, color.gamma_multiply(0.12));
    painter.circle_stroke(
        at,
        radius,
        egui::Stroke::new(2.0, color.gamma_multiply(0.8)),
    );
    painter.circle_filled(at, 3.0, color);
}

/// A soldier braced (feature 80): feet planted — a base line with two
/// struts down onto it and a chevron over them, in the caution colour.
/// Drawn in the ability box at the foot of the screen and nowhere on the
/// deck, where the panel's word says it.
pub fn brace_mark(painter: &egui::Painter, at: egui::Pos2, radius: f32) {
    let r = radius.max(4.0);
    let stroke = egui::Stroke::new((r * 0.18).clamp(1.0, 3.0), CAUTION.gamma_multiply(0.85));
    let foot = at.y + r * 0.7;
    painter.line_segment(
        [egui::pos2(at.x - r, foot), egui::pos2(at.x + r, foot)],
        stroke,
    );
    for side in [-1.0f32, 1.0] {
        painter.line_segment(
            [
                egui::pos2(at.x + side * r * 0.2, at.y - r * 0.2),
                egui::pos2(at.x + side * r * 0.7, foot),
            ],
            stroke,
        );
    }
    painter.line_segment(
        [
            egui::pos2(at.x - r * 0.7, at.y - r * 0.25),
            egui::pos2(at.x, at.y - r * 0.8),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(at.x, at.y - r * 0.8),
            egui::pos2(at.x + r * 0.7, at.y - r * 0.25),
        ],
        stroke,
    );
}

/// The charge bar over a tile something is being put together on
/// (feature 91): an engineer laying a kit, or anybody building a site.
/// A dark track with the progress filled in the caution colour and a
/// hairline round it, drawn just above the tile's middle so the thing
/// under it is still visible. `progress` is nought to one; `at` is the
/// tile's middle on the canvas.
///
/// It is sized off the zoom and clamped, like every other deck mark, so
/// a build across a station still reads with the whole deck in view.
pub fn work_bar(painter: &egui::Painter, at: egui::Pos2, scale: f32, progress: f32) {
    let w = (34.0 * scale).clamp(14.0, 46.0);
    let h = (5.0 * scale).clamp(3.0, 7.0);
    let at = egui::pos2(at.x, at.y - (16.0 * scale).clamp(7.0, 22.0));
    let track = egui::Rect::from_center_size(at, egui::vec2(w, h));
    let round = h * 0.5;
    painter.rect_filled(track, round, egui::Color32::from_black_alpha(170));
    let done = progress.clamp(0.0, 1.0);
    if done > 0.0 {
        let filled = egui::Rect::from_min_size(track.min, egui::vec2(w * done, h));
        painter.rect_filled(filled, round, CAUTION.gamma_multiply(0.9));
    }
    painter.rect_stroke(
        track,
        round,
        egui::Stroke::new(1.0, CAUTION.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
}

/// A heal landing on a beamed body (feature 91): the points put back,
/// floating up off the patient in the beam's own green and fading as it
/// goes. `rise` is nought the moment it appears and one as it goes out,
/// so the screen holds nothing but a birth time.
///
/// Drawn for every beamed crew member, whoever holds the beam, since the
/// point of it is that another player can see a medic working.
pub fn heal_number(painter: &egui::Painter, at: egui::Pos2, scale: f32, text: &str, rise: f32) {
    let rise = rise.clamp(0.0, 1.0);
    let lift = (NAME_LIFT * 0.7 * scale).clamp(12.0, 40.0);
    let at = egui::pos2(at.x, at.y - lift - rise * lift * 0.8);
    // Out over the last third, so it is read before it goes.
    let fade = (1.0 - (rise - 0.66) / 0.34).clamp(0.0, 1.0);
    let size = (NAME_SIZE * 1.05).max(11.0);
    // A shadow under it first: the deck is any colour, and green on
    // green is nothing at all.
    painter.text(
        at + egui::vec2(1.0, 1.0),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(size),
        egui::Color32::from_black_alpha(160).gamma_multiply(fade),
    );
    painter.text(
        at,
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(size),
        HEAL.gamma_multiply(fade),
    );
}

/// A tank standing as a wall, on the tank itself (feature 91): a small
/// shield over its head in the wall's own caution colour, inside the
/// ring [`wall_mark`] draws at the bulwark's reach — the ring says how
/// far the wall covers, and the shield says *which body is holding it*,
/// which the ring alone cannot when two tanks stand near each other.
/// The shape is [`defend_banner`]'s, small and without its ground ring:
/// a shield is a shield wherever it is drawn.
pub fn bulwark_shield(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let h = (15.0 * scale).clamp(7.0, 21.0);
    let w = h * 0.68;
    let lift = (0.55 * NAME_LIFT * scale).clamp(8.0, 30.0);
    let foot = egui::pos2(at.x, at.y - lift);
    let stroke = egui::Stroke::new((1.6 * scale).clamp(1.0, 2.4), CAUTION);
    let top = foot.y - h;
    let mid = foot.y - h * 0.34;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(foot.x - w * 0.5, top),
            egui::pos2(foot.x + w * 0.5, top),
            egui::pos2(foot.x + w * 0.42, mid),
            egui::pos2(foot.x, foot.y - h * 0.06),
            egui::pos2(foot.x - w * 0.42, mid),
        ],
        CAUTION.gamma_multiply(0.40),
        stroke,
    ));
    painter.line_segment(
        [
            egui::pos2(foot.x - w * 0.34, foot.y - h * 0.72),
            egui::pos2(foot.x + w * 0.34, foot.y - h * 0.72),
        ],
        egui::Stroke::new(stroke.width * 0.8, CAUTION),
    );
}

/// A tank taunting, on the tank itself (feature 91): rings thrown off
/// the body in the warning colour, swelling outwards and fading as they
/// go — a shout, drawn where the body is rather than at the taunt's
/// reach, which is what [`taunt_ring`]'s dashes say. The two are told
/// apart by size and by motion: this one is small and moving, that one
/// wide and still.
///
/// `phase` is seconds; the rings cycle on it, so nothing is kept between
/// frames. No facing: a shout goes every way at once, which is also why
/// a taunt pulls whoever is round him rather than whoever is in front.
pub fn taunt_shout(painter: &egui::Painter, at: egui::Pos2, scale: f32, phase: f32) {
    let near = (12.0 * scale).clamp(5.0, 16.0);
    let far = near * 2.4;
    let rings = 2;
    for i in 0..rings {
        // Each ring a half-cycle behind the last, so one is always on
        // its way out as the next leaves the body.
        let t = (phase / TAUNT_PULSE + i as f32 / rings as f32).fract();
        let r = near + (far - near) * t;
        painter.circle_stroke(
            at,
            r,
            egui::Stroke::new(
                (2.0 * scale).clamp(1.0, 3.0),
                WARN.gamma_multiply(0.8 * (1.0 - t)),
            ),
        );
    }
}

/// Seconds one of a taunt's rings takes to travel out from the body.
const TAUNT_PULSE: f32 = 0.9;

/// The commander **calling** a rally, on the commander himself (feature
/// 91): two chevrons over his head in the crew's own colour — the
/// [`rallied_mark`]'s one chevron said twice, so the caller reads
/// differently from the called.
///
/// He needs a mark of his own because `World::aura_reaching` answers
/// `None` for a commander asked about his own aura — nobody is in their
/// own — so the one Bim on the deck that is certainly rallying had
/// nothing on it to say so; and it goes over the head because a small
/// ring at the body is lost under his own rig and his selection ring,
/// which is where [`rallied_mark`] puts it.
pub fn rally_call(painter: &egui::Painter, at: egui::Pos2, scale: f32) {
    let w = (9.0 * scale).clamp(4.0, 13.0);
    let lift = (0.62 * NAME_LIFT * scale).clamp(10.0, 32.0);
    let step = w * 0.7;
    let stroke = egui::Stroke::new((1.8 * scale).clamp(1.2, 2.8), YOURS);
    // Upwards, so the pair stacks clear of the head rather than into it.
    for i in 0..2 {
        let y = at.y - lift - i as f32 * step;
        painter.line_segment(
            [egui::pos2(at.x - w, y + step * 0.7), egui::pos2(at.x, y)],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(at.x, y), egui::pos2(at.x + w, y + step * 0.7)],
            stroke,
        );
    }
}

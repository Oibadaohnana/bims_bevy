//! The heads-up display (feature 107): what stands over the deck while a
//! mission runs, cut down to what a player needs to act on in the next
//! few seconds — everything else is a key or a tray button away.
//!
//! Where each piece sits, and why none of them can land on another at
//! 1280×720 or larger:
//!
//! * **top left**, the crew's portraits, eight to a row;
//! * **top centre**, one frame — the day, the pool, the bounty waiting on
//!   the site being cleared, the most urgent warning and the pause — never
//!   left of the portraits' right edge, and the *You're out* banner under
//!   it;
//! * **bottom centre**, the hero panel: the player's own Bim, its level,
//!   health, experience and the class's keys. It stands clear of anything
//!   on the left that reaches down into its row — the tray opened, the
//!   character sheet — the way the ability bar used to stand clear of the
//!   tray;
//! * **bottom left**, the tray (`crew::CrewPanels::tray`), and over it
//!   the character sheet (`crew::CrewPanels::character_sheet`);
//! * **bottom right**, *Back to ship* (`worldmap::back_to_ship`) and the
//!   log directly above it;
//! * **right**, the inspect panel while a Bim is picked, under the top
//!   frame and over the log.
//!
//! Nothing here decides anything and nothing here is kept but the log:
//! every piece is laid out afresh from the world each frame.

use bevy_egui::egui;

use crate::format::euros;
use crate::keys::Action;
use crate::names::*;
use crate::theme;

/// The gap between a piece and the canvas's edge.
pub const MARGIN: f32 = 10.0;
/// The gap between two pieces.
pub const GAP: f32 = 8.0;

// --- the warnings -------------------------------------------------------------

/// What a warning is about, **most urgent first**: the order these are
/// declared in is the order the chip picks by, so `Ord` is the rule.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ThreatKind {
    /// A wave of machines on the deck, or the mission clock counting down
    /// to the next.
    Droids,
    /// The crew's alarm: an enemy near, or one of them hit.
    Alarm,
    /// A blade at the player's own Bim, which has stopped its gun.
    Locked,
    /// The player's own Bim recruited, and the crew following it.
    Recruited,
    /// The player's standing order to the crew: a banner down, or a
    /// retreat called.
    Standing,
}

/// One warning, in the words and colour it always had.
#[derive(Clone, PartialEq, Debug)]
pub struct Threat {
    pub kind: ThreatKind,
    pub text: String,
    pub colour: egui::Color32,
    pub tip: String,
}

/// Every warning up for the player in `local`, most urgent first.
pub fn threats(world: &world::World, local: u32) -> Vec<Threat> {
    let mut out = Vec::new();
    if let Some((text, tip)) = droid_line(world) {
        out.push(Threat {
            kind: ThreatKind::Droids,
            text,
            colour: theme::BAD,
            tip: tip.into(),
        });
    }
    let room = &world.aboard.room;
    if room.is_alarmed() {
        out.push(Threat {
            kind: ThreatKind::Alarm,
            text: ALARM_STATUS.into(),
            colour: theme::CAUTION,
            tip: ALARM_TIP.into(),
        });
    }
    if room.is_recruited(local) {
        let who = local as usize;
        let locked =
            local < room.crew_count() && room.is_alive(who) && room.is_locked(who).is_some();
        out.push(if locked {
            Threat {
                kind: ThreatKind::Locked,
                text: format!("Recruited — {LOCKED_STATUS}"),
                colour: theme::CAUTION,
                tip: locked_tip(),
            }
        } else {
            Threat {
                kind: ThreatKind::Recruited,
                text: RECRUITED_STATUS.into(),
                colour: theme::ACCENT,
                tip: ORDERS_TIP.into(),
            }
        });
    }
    if let Some(line) = orders_line(world.standing_of(local).code()) {
        out.push(Threat {
            kind: ThreatKind::Standing,
            text: line.into(),
            colour: theme::ATTACK,
            tip: ORDERS_TIP.into(),
        });
    }
    out.sort_by_key(|t| t.kind);
    out
}

/// The one warning the chip shows, and how many more are up behind it.
pub fn top_threat(threats: &[Threat]) -> Option<(&Threat, usize)> {
    let top = threats.iter().min_by_key(|t| t.kind)?;
    Some((top, threats.len() - 1))
}

/// The machines' line (features 83 and 94): which wave holds the place
/// and how many of it are standing, or — the moment the last of them is
/// down — the mission clock's countdown to the next. A town under attack
/// says the same three things. `None` where nobody holds the place.
fn droid_line(world: &world::World) -> Option<(String, &'static str)> {
    if let Some(defending) = world.defense_here() {
        let waves = defending.wave + defending.waves_left;
        let standing = world.droids_standing();
        let words = if standing > 0 {
            droids_standing(defending.wave, waves, standing)
        } else if let Some(due) = world.defense_wave_due() {
            droids_next_wave(&crate::format::countdown(due), defending.wave + 1, waves)
        } else if defending.wave > 0 && defending.waves_left == 0 {
            DROIDS_CLEARED.into()
        } else {
            droids_standing(defending.wave.max(1), waves.max(1), 0)
        };
        return Some((words, DEFENSE_TIP));
    }
    let (wave, left) = world.droid_wave_standing()?;
    let waves = wave + left;
    let standing = world.droids_standing();
    let words = if standing > 0 {
        droids_standing(wave, waves, standing)
    } else if let Some(due) = world.droid_wave_due() {
        droids_next_wave(&crate::format::countdown(due), wave + 1, waves)
    } else if left == 0 {
        DROIDS_CLEARED.into()
    } else {
        // The last machine went down this very step: the clock is set at
        // the top of the next one.
        droids_standing(wave, waves, 0)
    };
    Some((words, DROIDS_TIP))
}

/// The frame a machines' warning sits in: the panel's, filled and edged
/// in red, so it is the one red thing on the screen.
pub fn warning_frame() -> egui::Frame {
    theme::panel_frame()
        .fill(egui::Color32::from_rgba_unmultiplied(64, 14, 10, 235))
        .stroke(egui::Stroke::new(1.0, theme::WARN))
}

/// A chip: a small raised frame round a word or two.
fn chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::RAISED)
        .stroke(egui::Stroke::new(1.0, theme::LINE))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
}

// --- experience -----------------------------------------------------------------

/// How far through its level `xp` is, in whole points: what is in, and
/// what the level wants in all. `None` at the tenth, which is the top.
pub fn xp_into(xp: u32) -> Option<(u8, u32, u32)> {
    let level = world::class::level_of(xp);
    if level >= world::class::LEVELS {
        return None;
    }
    let from = world::class::LEVEL_XP[level as usize - 1];
    let to = world::class::LEVEL_XP[level as usize];
    Some((level, xp.saturating_sub(from), to - from))
}

/// The line under the hero's health: `Lv 4 · 50 / 250 XP`, `Lv 10 · Max`
/// at the top, and `No class` for a crew member without one.
pub fn xp_text(class: world::Class, xp: u32) -> String {
    if class == world::Class::None {
        return NO_CLASS.into();
    }
    match xp_into(xp) {
        Some((level, into, of)) => hero_xp_line(level, into, of),
        None => hero_xp_max(world::class::LEVELS),
    }
}

/// How full the experience bar is: full at the top.
pub fn xp_fill(xp: u32) -> f32 {
    match xp_into(xp) {
        Some((_, into, of)) if of > 0 => into as f32 / of as f32,
        Some(_) => 0.0,
        None => 1.0,
    }
}

// --- the log ----------------------------------------------------------------------

/// How many lines of what just happened stand over *Back to ship*.
pub const LOG_LINES: usize = 4;
/// How long a line stands, in real seconds, the last of them fading.
pub const LOG_SECONDS: f64 = 8.0;
const LOG_FADE: f64 = 1.0;
/// Two of the same thing inside this many seconds are one line: the same
/// words said twice, or experience from the same source.
pub const GROUP_SECONDS: f64 = 1.0;
/// How wide the log may grow before a line wraps: narrow enough to stand
/// clear of the hero panel at the smallest window.
pub const LOG_WIDTH: f32 = 260.0;

/// What just happened, a line at a time, each gone a few seconds after
/// it came. The screen's own, carried across a world replaced.
#[derive(Default)]
pub struct Log {
    lines: Vec<Line>,
    /// The frame's clock, from `tick`: what a line pushed now is stamped
    /// with. `None` before the first frame.
    now: Option<f64>,
}

struct Line {
    text: String,
    /// When it came, on the window's clock: `None` until the first frame.
    born: Option<f64>,
    /// How many times the same words came inside [`GROUP_SECONDS`].
    times: u32,
    /// Experience gathered into this line, and what it was for.
    xp: Option<(&'static str, u32)>,
}

impl Line {
    fn words(&self) -> String {
        match self.xp {
            Some((source, xp)) => xp_gain_line(source, xp),
            None if self.times > 1 => format!("{} ×{}", self.text, self.times),
            None => self.text.clone(),
        }
    }

    /// Still inside the window a second of the same is folded into.
    fn fresh(&self, now: Option<f64>) -> bool {
        match (self.born, now) {
            (Some(born), Some(now)) => now - born < GROUP_SECONDS,
            _ => true,
        }
    }
}

impl Log {
    /// A line of words; the same words again inside a second count up on
    /// the line already there.
    pub fn push(&mut self, text: String) {
        let now = self.now;
        if let Some(line) = self
            .lines
            .iter_mut()
            .rev()
            .find(|l| l.xp.is_none() && l.text == text && l.fresh(now))
        {
            line.times += 1;
            return;
        }
        self.lines.push(Line {
            text,
            born: now,
            times: 1,
            xp: None,
        });
    }

    /// Every line of `lines`, in order.
    pub fn extend(&mut self, lines: impl IntoIterator<Item = String>) {
        for line in lines {
            self.push(line);
        }
    }

    /// Experience gained: gathered onto a line of the same source born
    /// inside the last second, or a new one.
    pub fn xp(&mut self, source: &'static str, amount: u32) {
        if amount == 0 {
            return;
        }
        let now = self.now;
        if let Some(line) = self
            .lines
            .iter_mut()
            .rev()
            .find(|l| l.xp.is_some_and(|(s, _)| s == source) && l.fresh(now))
        {
            if let Some((_, xp)) = line.xp.as_mut() {
                *xp += amount;
            }
            return;
        }
        self.lines.push(Line {
            text: String::new(),
            born: now,
            times: 1,
            xp: Some((source, amount)),
        });
    }

    /// Once a frame, first: the clock the lines are stamped and aged by,
    /// and the lines past their time let go.
    pub fn tick(&mut self, now: f64) {
        self.now = Some(now);
        for line in &mut self.lines {
            line.born.get_or_insert(now);
        }
        self.lines
            .retain(|l| l.born.is_some_and(|born| now - born < LOG_SECONDS));
        let over = self.lines.len().saturating_sub(LOG_LINES);
        self.lines.drain(..over);
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The lines as they stand, words and how visible, newest last.
    fn shown(&self) -> Vec<(String, f32)> {
        let now = self.now.unwrap_or(0.0);
        let skip = self.lines.len().saturating_sub(LOG_LINES);
        self.lines[skip..]
            .iter()
            .map(|l| {
                let age = l.born.map_or(0.0, |b| now - b);
                let alpha = ((LOG_SECONDS - age) / LOG_FADE).clamp(0.0, 1.0) as f32;
                (l.words(), alpha)
            })
            .collect()
    }
}

/// The log, its right edge at `right` and its foot `lift` over the
/// canvas's bottom, a small dark plate under each line so it reads over
/// the deck. The rectangle it took, `None` with nothing to say.
pub fn log_area(
    ctx: &egui::Context,
    canvas: egui::Rect,
    right: f32,
    lift: f32,
    log: &Log,
) -> Option<egui::Rect> {
    if log.is_empty() {
        return None;
    }
    let lines = log.shown();
    let area = egui::Area::new(egui::Id::new("hud-log"))
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(egui::pos2(right, canvas.max.y - MARGIN - lift))
        .order(egui::Order::Middle)
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_max_width(LOG_WIDTH);
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (words, alpha) in lines {
                    egui::Frame::new()
                        .fill(egui::Color32::from_black_alpha((150.0 * alpha) as u8))
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(words)
                                        .small()
                                        .color(theme::INK.gamma_multiply(alpha)),
                                )
                                .wrap(),
                            );
                        });
                }
            });
        });
    Some(area.response.rect)
}

/// What is under the pointer on the deck, in a small plate beside it
/// (feature 107): the part, or the fixture and its state, and the tile.
/// It stands off the pointer so the pointer is never under it, and takes
/// no clicks.
pub fn readout(ctx: &egui::Context, at: egui::Pos2, words: &str) {
    egui::Area::new(egui::Id::new("hud-readout"))
        .fixed_pos(at + egui::vec2(16.0, 18.0))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::PANEL_DEEP.gamma_multiply(0.92))
                .stroke(egui::Stroke::new(1.0, theme::LINE))
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(words).small().color(theme::INK));
                });
        });
}

// --- the portraits ------------------------------------------------------------------

/// One crew member as the top left shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct Portrait {
    pub who: u32,
    pub name: String,
    pub class: world::Class,
    /// The level, `None` without a class.
    pub level: Option<u8>,
    /// The body's share of the bar and the armour's after it, as the side
    /// panel splits them.
    pub body: f32,
    pub armour: f32,
    pub hurt: bool,
    /// The player's own.
    pub yours: bool,
    /// A player's Bim, not a bot.
    pub player: bool,
    /// That player has pressed *Back to ship*.
    pub returning: bool,
    /// Out cold or dying on the deck.
    pub downed: bool,
    /// Dead, or a player's Bim out until it is bought back.
    pub out: bool,
    /// The one an out player's camera is on.
    pub watched: bool,
}

/// Every crew member's portrait, players first as the crew are numbered.
pub fn portraits_of(world: &world::World, local: u32, watched: Option<u32>) -> Vec<Portrait> {
    let room = &world.aboard.room;
    let players = world.players();
    (0..world.aboard.crew_count())
        .map(|who| {
            let w = who as usize;
            let alive = room.is_alive(w);
            let class = world.class_of(who);
            let points = room.health(w);
            let armour = if alive { room.armour_health(w) } else { 0.0 };
            let total = bims::health::MAX_HEALTH + armour;
            let player = who < players;
            Portrait {
                who,
                name: crew_name(who),
                class,
                level: (class != world::Class::None).then(|| world.progress_of(who).level()),
                body: (points / total).clamp(0.0, 1.0),
                armour: (armour / total).clamp(0.0, 1.0),
                hurt: crate::crew::is_hurt(room, w),
                yours: who == local,
                player,
                returning: player && world.run.is_returning(who),
                downed: alive && room.is_down(w),
                out: !alive || (player && world.run.is_out(who)),
                watched: watched == Some(who),
            }
        })
        .collect()
}

/// What a click on the portraits asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortraitPress {
    /// That crew member, picked as a click on it on the deck picks it —
    /// or, for a player who is out, watched.
    Pick(u32),
    /// The player's own, twice: the character sheet.
    Sheet,
}

pub const PORTRAIT_W: f32 = 44.0;
pub const PORTRAIT_H: f32 = 38.0;
pub const PORTRAITS_PER_ROW: usize = 8;

/// The portraits, a row of eight and then another, from `at`. The
/// rectangle they took, and what was clicked.
pub fn portraits(
    ctx: &egui::Context,
    at: egui::Pos2,
    cells: &[Portrait],
) -> (egui::Rect, Option<PortraitPress>) {
    let mut pressed = None;
    let area = egui::Area::new(egui::Id::new("hud-portraits"))
        .fixed_pos(at)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for row in cells.chunks(PORTRAITS_PER_ROW) {
                ui.horizontal(|ui| {
                    for cell in row {
                        if let Some(press) = portrait(ui, cell) {
                            pressed = Some(press);
                        }
                    }
                });
            }
        });
    (area.response.rect, pressed)
}

/// One portrait: the initial in a box, outlined in the accent for the
/// player's own; a thin health bar under it; the level under that, or
/// `out`. A check in the corner while that player is on the way home,
/// the red cross while it is down.
fn portrait(ui: &mut egui::Ui, cell: &Portrait) -> Option<PortraitPress> {
    let size = egui::vec2(PORTRAIT_W, PORTRAIT_H + 6.0 + 14.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    let face = egui::Rect::from_min_size(rect.min, egui::vec2(PORTRAIT_W, PORTRAIT_H));
    let dim = |c: egui::Color32| if cell.out { c.gamma_multiply(0.45) } else { c };
    painter.rect_filled(
        face,
        5.0,
        theme::panel_frame()
            .fill
            .gamma_multiply(if cell.out { 0.6 } else { 1.0 }),
    );
    let edge = if cell.yours {
        egui::Stroke::new(2.0, theme::ACCENT)
    } else if cell.watched || response.hovered() {
        egui::Stroke::new(1.5, theme::INK)
    } else {
        egui::Stroke::new(1.0, theme::LINE)
    };
    painter.rect_stroke(face, 5.0, edge, egui::StrokeKind::Inside);
    let words = initials(&cell.name);
    let size = if words.chars().count() > 2 {
        15.0
    } else {
        18.0
    };
    painter.text(
        face.center(),
        egui::Align2::CENTER_CENTER,
        words,
        egui::FontId::proportional(size),
        dim(if cell.player {
            theme::INK
        } else {
            theme::MUTED
        }),
    );
    // The bar: the body's share and the armour's after it.
    let bar = egui::Rect::from_min_size(
        egui::pos2(face.min.x, face.max.y + 3.0),
        egui::vec2(PORTRAIT_W, 4.0),
    );
    painter.rect_filled(bar, 2.0, theme::RAISED);
    let body = egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * cell.body, 4.0));
    painter.rect_filled(
        body,
        2.0,
        dim(if cell.hurt { theme::BAD } else { theme::ACCENT }),
    );
    let armour = egui::Rect::from_min_size(
        egui::pos2(body.max.x, bar.min.y),
        egui::vec2(bar.width() * cell.armour, 4.0),
    );
    painter.rect_filled(armour, 2.0, dim(theme::ARMOUR));
    let under = if cell.out {
        PORTRAIT_OUT.to_string()
    } else {
        cell.level
            .map_or_else(|| "–".to_string(), |l| l.to_string())
    };
    painter.text(
        egui::pos2(face.center().x, bar.max.y + 2.0),
        egui::Align2::CENTER_TOP,
        under,
        egui::FontId::proportional(11.0),
        if cell.out { theme::MUTED } else { theme::INK },
    );
    // The markers, in the corners of the face.
    if cell.returning && !cell.out {
        let at = egui::pos2(face.max.x - 7.0, face.min.y + 7.0);
        painter.circle_filled(at, 6.0, theme::ACCENT);
        painter.add(egui::Shape::line(
            vec![
                at + egui::vec2(-3.0, 0.0),
                at + egui::vec2(-1.0, 2.5),
                at + egui::vec2(3.0, -2.5),
            ],
            egui::Stroke::new(1.6, theme::PANEL_DEEP),
        ));
    }
    // Down: the deck's red cross in small, a white cross on red.
    if cell.downed && !cell.out {
        let at = egui::pos2(face.min.x + 7.0, face.min.y + 7.0);
        painter.circle_filled(at, 6.0, theme::DYING);
        let arm = egui::Stroke::new(1.8, theme::INK);
        painter.line_segment([at - egui::vec2(3.0, 0.0), at + egui::vec2(3.0, 0.0)], arm);
        painter.line_segment([at - egui::vec2(0.0, 3.0), at + egui::vec2(0.0, 3.0)], arm);
    }
    let response = response.on_hover_text(portrait_tip(
        &cell.name,
        class_name(cell.class),
        cell.level,
        cell.downed,
        cell.out,
        cell.returning,
    ));
    if cell.yours && response.double_clicked() {
        return Some(PortraitPress::Sheet);
    }
    response.clicked().then_some(PortraitPress::Pick(cell.who))
}

/// What a portrait says of a name: its first letter, and the number on
/// the end of one that has one — `Crew 12` is `C12`, since the bots the
/// table runs out of names for are numbered, and a row of `C`s tells
/// nobody apart.
pub fn initials(name: &str) -> String {
    let first: String = name.chars().take(1).collect();
    let number: String = name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{first}{number}")
}

// --- the top frame ------------------------------------------------------------------

/// The frame at the top centre: the day, the pool, the bounty waiting on
/// the site, the one warning that matters most with how many more are up
/// behind it, and the pause. Never left of `clear`, which is the
/// portraits' right edge. The rectangle it took.
pub fn top_frame(
    ctx: &egui::Context,
    canvas: egui::Rect,
    clear: f32,
    world: &world::World,
    threats: &[Threat],
    paused: bool,
) -> egui::Rect {
    let id = egui::Id::new("hud-top");
    let width = ctx
        .memory(|m| m.area_rect(id).map(|r| r.width()))
        .unwrap_or(260.0);
    let x = ((canvas.min.x + canvas.max.x - width) / 2.0).max(clear + GAP);
    egui::Area::new(id)
        .fixed_pos(egui::pos2(x, canvas.min.y + MARGIN))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(day_word(world.day())).strong());
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(euros(world.money))
                            .strong()
                            .color(theme::ACCENT),
                    );
                    if world.run.pending_bounty > 0 {
                        ui.label(
                            egui::RichText::new(on_clear_line(world.run.pending_bounty))
                                .color(theme::MUTED),
                        );
                    }
                    if let Some((top, more)) = top_threat(threats) {
                        ui.add_space(4.0);
                        let frame = if top.kind == ThreatKind::Droids {
                            warning_frame().inner_margin(egui::Margin::symmetric(8, 3))
                        } else {
                            chip_frame()
                        };
                        let chip = frame
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&top.text).strong().color(top.colour),
                                    );
                                    if more > 0 {
                                        ui.label(
                                            egui::RichText::new(format!("+{more}"))
                                                .color(theme::MUTED),
                                        );
                                    }
                                });
                            })
                            .response
                            .interact(egui::Sense::hover());
                        chip.on_hover_ui(|ui| {
                            ui.set_max_width(360.0);
                            for (i, t) in threats.iter().enumerate() {
                                if i > 0 {
                                    ui.separator();
                                }
                                ui.label(egui::RichText::new(&t.text).strong().color(t.colour));
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&t.tip).small()).wrap(),
                                );
                            }
                        });
                    }
                    if paused {
                        ui.add_space(4.0);
                        chip_frame()
                            .stroke(egui::Stroke::new(1.0, theme::CAUTION))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(PAUSED_CHIP)
                                        .strong()
                                        .color(theme::CAUTION),
                                );
                            });
                    }
                });
            });
        })
        .response
        .rect
}

/// The banner under the top frame for a player whose Bim is out: that it
/// is, what buying it back costs and what the pool holds.
pub fn out_banner(ctx: &egui::Context, centre: f32, top: f32, pool: u64) -> egui::Rect {
    let id = egui::Id::new("hud-out");
    let width = ctx
        .memory(|m| m.area_rect(id).map(|r| r.width()))
        .unwrap_or(240.0);
    egui::Area::new(id)
        .fixed_pos(egui::pos2(centre - width / 2.0, top))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            warning_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(OUT_BANNER).strong().color(theme::BAD));
                    ui.label(
                        egui::RichText::new(out_banner_line(world::data::BUYBACK_COST, pool))
                            .color(theme::INK),
                    );
                });
            });
        })
        .response
        .rect
}

// --- the hero panel -----------------------------------------------------------------

/// The player's own Bim, as the panel at the foot of the canvas shows it.
pub struct Hero {
    pub class: world::Class,
    pub xp: u32,
    /// The body's points and the armour's, as the side panel reads them.
    pub points: f32,
    pub armour: f32,
    pub hurt: bool,
    /// Out cold or dying.
    pub downed: bool,
    /// The worst of the peril block, in a line, and its colour.
    pub peril: Option<(String, egui::Color32)>,
    /// A talent waiting to be picked.
    pub pick: bool,
}

/// What the hero panel was asked, and where it stood.
pub struct HeroOut {
    pub rect: egui::Rect,
    /// The key whose box the pointer rests on.
    pub hovered: Option<Action>,
    /// The `+1` was pressed: the character sheet.
    pub sheet: bool,
}

/// The circle the level stands in.
const LEVEL_DISC: f32 = 24.0;
/// The bars' width.
const HERO_BAR_W: f32 = 180.0;

/// The hero panel, centred at the foot of the canvas but never left of
/// `clear` — the right edge of whatever on the left reaches down into its
/// row, the tray opened or the character sheet — the way the ability bar
/// stood clear of the tray; nor right of `right`, the left edge of what
/// stands at the bottom right; nor off the canvas. `boxes` lays the
/// class's keys and the medicine out, and says which the pointer rests
/// on.
pub fn hero_panel(
    ctx: &egui::Context,
    canvas: egui::Rect,
    clear: f32,
    right: f32,
    hero: &Hero,
    boxes: impl FnOnce(&mut egui::Ui) -> Option<Action>,
) -> HeroOut {
    let id = egui::Id::new("hud-hero");
    let size = ctx
        .memory(|m| m.area_rect(id).map(|r| r.size()))
        .unwrap_or(egui::vec2(520.0, 76.0));
    let x = ((canvas.min.x + canvas.max.x - size.x) / 2.0)
        .max(clear + GAP)
        .min(right.min(canvas.max.x - MARGIN) - GAP - size.x)
        .max(canvas.min.x + MARGIN);
    let y = canvas.max.y - MARGIN - size.y;
    let mut hovered = None;
    let mut sheet = false;
    let area = egui::Area::new(id)
        .fixed_pos(egui::pos2(x, y))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    level_disc(ui, hero);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        let total = bims::health::MAX_HEALTH + hero.armour;
                        theme::health_bar(
                            ui,
                            HERO_BAR_W,
                            hero.points / total,
                            hero.armour / total,
                            if hero.hurt { theme::BAD } else { theme::ACCENT },
                            theme::ARMOUR,
                        );
                        if hero.class != world::Class::None {
                            thin_bar(ui, HERO_BAR_W, xp_fill(hero.xp), theme::HYPER);
                        }
                        ui.label(
                            egui::RichText::new(xp_text(hero.class, hero.xp))
                                .small()
                                .color(theme::MUTED),
                        );
                        if let Some((line, colour)) = &hero.peril {
                            ui.label(egui::RichText::new(line).small().strong().color(*colour));
                        }
                    });
                    hovered = boxes(ui);
                    if hero.pick {
                        let plus = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("+1")
                                        .strong()
                                        .size(16.0)
                                        .color(theme::HYPER),
                                )
                                .min_size(egui::vec2(40.0, 44.0))
                                .stroke(egui::Stroke::new(1.5, theme::HYPER)),
                            )
                            .on_hover_text(TALENT_WAITING_TIP);
                        if plus.clicked() {
                            sheet = true;
                        }
                    }
                });
            });
        });
    let rect = area.response.rect;
    // Down, the panel is greyed over and says so — with how long the body
    // has, where the blood is running out, since that is the one time the
    // simulation keeps for a Bim on the deck.
    if hero.downed {
        let painter = ctx.layer_painter(area.response.layer_id);
        painter.rect_filled(rect, 6.0, theme::PANEL_DEEP.gamma_multiply(0.75));
        let lift = if hero.peril.is_some() { 10.0 } else { 0.0 };
        painter.text(
            rect.center() - egui::vec2(0.0, lift),
            egui::Align2::CENTER_CENTER,
            DOWNED_BANNER,
            egui::FontId::proportional(20.0),
            theme::BAD,
        );
        if let Some((line, colour)) = &hero.peril {
            painter.text(
                rect.center() + egui::vec2(0.0, 12.0),
                egui::Align2::CENTER_CENTER,
                line,
                egui::FontId::proportional(12.0),
                *colour,
            );
        }
    }
    HeroOut {
        rect,
        hovered,
        sheet,
    }
}

/// The level in its circle: the number, or a dash without a class, ringed
/// by how far through the level the experience is.
fn level_disc(ui: &mut egui::Ui, hero: &Hero) {
    use std::f32::consts::TAU;
    let side = LEVEL_DISC * 2.0 + 6.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter();
    let at = rect.center();
    painter.circle_filled(at, LEVEL_DISC, theme::PANEL_DEEP);
    painter.circle_stroke(at, LEVEL_DISC, egui::Stroke::new(3.0, theme::LINE));
    let classed = hero.class != world::Class::None;
    if classed {
        let share = xp_fill(hero.xp);
        let steps = ((share * 48.0).ceil() as usize).max(1);
        let arc: Vec<egui::Pos2> = (0..=steps)
            .map(|i| {
                let a = share * TAU * i as f32 / steps as f32;
                at + egui::vec2(a.sin(), -a.cos()) * LEVEL_DISC
            })
            .collect();
        if share > 0.0 {
            painter.add(egui::Shape::line(arc, egui::Stroke::new(3.0, theme::HYPER)));
        }
    }
    let level = if classed {
        world::class::level_of(hero.xp).to_string()
    } else {
        "–".to_string()
    };
    painter.text(
        at,
        egui::Align2::CENTER_CENTER,
        level,
        egui::FontId::proportional(22.0),
        theme::INK,
    );
}

/// A bar half the height of the others, for the experience.
fn thin_bar(ui: &mut egui::Ui, width: f32, fraction: f32, fill: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 4.0), egui::Sense::hover());
    theme::bar_in(ui.painter(), rect, fraction, fill);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn threat(kind: ThreatKind) -> Threat {
        Threat {
            kind,
            text: format!("{kind:?}"),
            colour: theme::INK,
            tip: String::new(),
        }
    }

    /// The chip shows the most urgent warning whatever order they come in
    /// — the machines over the alarm over a blade over the recruit over
    /// the standing order — and counts the rest behind it.
    #[test]
    fn the_chip_picks_the_most_urgent_warning_and_counts_the_rest() {
        assert!(top_threat(&[]).is_none());
        let all = [
            threat(ThreatKind::Standing),
            threat(ThreatKind::Recruited),
            threat(ThreatKind::Alarm),
            threat(ThreatKind::Droids),
            threat(ThreatKind::Locked),
        ];
        let (top, more) = top_threat(&all).unwrap();
        assert_eq!(top.kind, ThreatKind::Droids);
        assert_eq!(more, 4);
        let (top, more) = top_threat(&all[..3]).unwrap();
        assert_eq!(top.kind, ThreatKind::Alarm);
        assert_eq!(more, 2);
        let two = [threat(ThreatKind::Standing), threat(ThreatKind::Locked)];
        let (top, more) = top_threat(&two).unwrap();
        assert_eq!(top.kind, ThreatKind::Locked);
        assert_eq!(more, 1);
        let one = [threat(ThreatKind::Standing)];
        let (top, more) = top_threat(&one).unwrap();
        assert_eq!(top.kind, ThreatKind::Standing);
        assert_eq!(more, 0);
    }

    /// The experience line is whole points through the level, and `Max`
    /// with the bar full at the tenth.
    #[test]
    fn the_experience_line_is_whole_numbers_and_max_at_the_top() {
        let soldier = world::Class::Soldier;
        assert_eq!(xp_text(soldier, 0), "Lv 1 · 0 / 100 XP");
        assert_eq!(xp_text(soldier, 500), "Lv 4 · 50 / 250 XP");
        assert_eq!(xp_text(soldier, 3_199), "Lv 9 · 699 / 700 XP");
        assert!((xp_fill(500) - 0.2).abs() < 1e-6);
        for xp in [3_200, 3_201, u32::MAX] {
            assert_eq!(xp_text(soldier, xp), "Lv 10 · Max");
            assert_eq!(xp_fill(xp), 1.0);
        }
        assert_eq!(xp_text(world::Class::None, 500), "No class");
        // Nothing but digits between the words: no fraction of a point.
        assert!(!xp_text(soldier, 777).contains('.'));
    }

    /// The log keeps four lines, folds the same words and the same
    /// source's experience inside a second, and lets a line go after its
    /// eight seconds.
    #[test]
    fn the_log_folds_what_repeats_and_lets_lines_go() {
        let mut log = Log::default();
        log.tick(10.0);
        log.push("A".into());
        log.push("A".into());
        log.xp("Machine down", 10);
        log.xp("Machine down", 10);
        let shown: Vec<String> = log.shown().into_iter().map(|(w, _)| w).collect();
        assert_eq!(
            shown,
            vec!["A ×2".to_string(), xp_gain_line("Machine down", 20)]
        );
        log.tick(11.5);
        log.xp("Machine down", 5);
        assert_eq!(log.shown().len(), 3, "a second on, a new line");
        for i in 0..6 {
            log.push(format!("line {i}"));
        }
        log.tick(11.6);
        assert_eq!(log.shown().len(), LOG_LINES);
        log.tick(11.6 + LOG_SECONDS);
        assert!(log.is_empty());
    }
}

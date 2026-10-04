//! The heads-up display (feature 107): what stands over the deck while a
//! mission runs, cut down to what a player needs to act on in the next
//! few seconds — everything else is a key or a tray button away.
//!
//! Where each piece sits, and why none of them can land on another at
//! 1280×720 or larger:
//!
//! * **top left**, the crew's portraits, eight to a row;
//! * **top centre**, the count — the enemies standing in big red, and
//!   which wave of how many beside them — and the pause, never left of
//!   the portraits' right edge; the day, the money and every warning are
//!   its tooltip. The *You're out* banner under it, and under both the
//!   crew's relics in a row (`relic_bar`, over the map too);
//! * **bottom centre**, the hero panel, the main of the HUD (Dota 2's
//!   bottom bar): the player's own Bim, its level, hands, money, the
//!   class's keys over the health bar, the magazine and the items. No
//!   words on it but numbers; what a piece means is its tooltip. It
//!   stands clear of anything on the left that reaches down into its row
//!   — the tray opened, the character sheet — the way the ability bar
//!   used to stand clear of the tray;
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

/// The machines' line (features 83 and 94): which wave holds the place
/// and how many of it are standing, or — the moment the last of them is
/// down — the mission clock's countdown to the next. A town under attack
/// says the same three things. `None` where nobody holds the place.
fn droid_line(world: &world::World) -> Option<(String, &'static str)> {
    // An Area defend (October 2026): the wave, and the hold's time left.
    if let Some(defending) = world.defense_here()
        && let Some(area) = defending.area.as_ref()
    {
        let standing = world.droids_standing();
        let hold = crate::format::countdown(world.area_time_left().unwrap_or(0.0));
        let words = if defending.wave == 0
            && let Some(due) = world.defense_wave_due()
        {
            defense_prepare(&crate::format::countdown(due))
        } else if area.left == 0 {
            area_last_wave(standing)
        } else if standing > 0 {
            let next = world.defense_wave_due().map(crate::format::countdown);
            area_standing(defending.wave, standing, next.as_deref(), &hold)
        } else if let Some(due) = world.defense_wave_due() {
            area_next_wave(&crate::format::countdown(due), defending.wave + 1, &hold)
        } else {
            area_standing(defending.wave.max(1), standing, None, &hold)
        };
        return Some((words, AREA_DEFENSE_TIP));
    }
    if let Some(defending) = world.defense_here() {
        let waves = defending.wave + defending.waves_left;
        let standing = world.droids_standing();
        // Before day ten the waves are the Manufacturers' (task 131).
        let theirs = world.defense_by_manufacturers();
        let up = |wave, waves, standing| {
            if theirs {
                manufacturers_standing(wave, waves, standing)
            } else {
                droids_standing(wave, waves, standing)
            }
        };
        let words = if standing > 0 {
            up(defending.wave, waves, standing)
        } else if defending.wave == 0
            && let Some(due) = world.defense_wave_due()
        {
            // Before the first wave (task 111): the prep time, counting.
            defense_prepare(&crate::format::countdown(due))
        } else if let Some(due) = world.defense_wave_due() {
            let span = crate::format::countdown(due);
            if theirs {
                manufacturers_next_wave(&span, defending.wave + 1, waves)
            } else {
                droids_next_wave(&span, defending.wave + 1, waves)
            }
        } else if defending.wave > 0 && defending.waves_left == 0 {
            if theirs {
                MANUFACTURERS_CLEARED.into()
            } else {
                DROIDS_CLEARED.into()
            }
        } else {
            up(defending.wave.max(1), waves.max(1), 0)
        };
        let tip = if theirs {
            DEFENSE_TIP_MANUFACTURERS
        } else {
            DEFENSE_TIP
        };
        return Some((words, tip));
    }
    // At the Machine Heart's fortress (feature 108) the core and its
    // conduits come first in the same red chip, the waves after them.
    if let Some(heart) = world.heart_status() {
        let line = heart_line(
            heart.phase,
            heart.core_health,
            heart.core_max,
            heart.conduits_left,
            heart.conduits,
        );
        let waves = world
            .droid_wave_standing()
            .map(|(wave, left)| {
                let standing = world.droids_standing();
                match world.droid_wave_due() {
                    Some(due) if standing == 0 => {
                        heart_next_wave(&crate::format::countdown(due), wave + 1, wave + left)
                    }
                    _ => heart_wave(wave, wave + left, standing),
                }
            })
            .unwrap_or_default();
        let text = if waves.is_empty() {
            line
        } else {
            heart_and_waves(&line, &waves)
        };
        return Some((text, HEART_TIP));
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
/// what the level wants in all, on the one table every class climbs
/// (twenty levels since October 2026; sixteen from task 139). `None` at the top.
pub fn xp_into(xp: u32) -> Option<(u8, u32, u32)> {
    let table = world::class::LEVEL_XP;
    let level = world::class::level_of(xp);
    if level as usize >= table.len() {
        return None;
    }
    let from = table[level as usize - 1];
    let to = table[level as usize];
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
    /// A shield's share after the armour (a tank's Bastion).
    pub shield: f32,
    pub hurt: bool,
    /// The player's own.
    pub yours: bool,
    /// A player's Bim, not a bot.
    pub player: bool,
    /// That player has pressed *Back to ship*.
    pub returning: bool,
    /// Downed on the deck (task 120).
    pub downed: bool,
    /// What is left of its countdown while it is downed, nought to one:
    /// the ring in the face's corner.
    pub down_share: f32,
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
            let shield = if alive { room.shield_hp(w) } else { 0.0 };
            let total = room.max_health(w) + armour + shield;
            let player = who < players;
            Portrait {
                who,
                name: crew_name(who),
                class,
                level: (class != world::Class::None).then(|| world.level_of(who)),
                body: (points / total).clamp(0.0, 1.0),
                armour: (armour / total).clamp(0.0, 1.0),
                shield: (shield / total).clamp(0.0, 1.0),
                hurt: crate::crew::is_hurt(room, w),
                yours: who == local,
                player,
                returning: player && world.run.is_returning(who),
                downed: alive && room.is_down(w),
                down_share: room
                    .down_left(w)
                    .map_or(0.0, |left| left / bims::health::DOWNED_SECONDS),
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
/// the deck's countdown ring in small while it is down.
pub fn portrait(ui: &mut egui::Ui, cell: &Portrait) -> Option<PortraitPress> {
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
    let shield = egui::Rect::from_min_size(
        egui::pos2(armour.max.x, bar.min.y),
        egui::vec2(bar.width() * cell.shield, 4.0),
    );
    painter.rect_filled(shield, 2.0, dim(theme::SHIELD));
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
    // Down: the deck's countdown ring in small.
    if cell.downed && !cell.out {
        let at = egui::pos2(face.min.x + 8.0, face.min.y + 8.0);
        theme::countdown_ring(painter, at, 5.0, 2.0, cell.down_share);
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

/// The count at the top centre: how many enemies stand — or, between
/// waves, the countdown to the next — and which wave of how many, and at
/// the Machine Heart its core's health.
#[derive(Clone, PartialEq, Debug)]
pub struct Counter {
    /// The big figure and its colour: the enemies standing, red; the
    /// countdown, amber; nought once the last wave is down, grey.
    pub big: Option<(String, egui::Color32)>,
    /// Which wave of how many: the one on the deck, or the next.
    pub wave: Option<(u32, u32)>,
    /// The Heart's core, its share of its health left.
    pub core: Option<f32>,
    /// An Area defend's count (October 2026), in place of `wave`.
    pub area: Option<AreaCount>,
}

/// What the top count says of an Area defend: the wave (there is no
/// last but the one standing when the time runs out), the hold's time
/// left — `None` once it is out — and how far the machines are to taking
/// the FOB, and whether somebody of the crew's side is holding them.
#[derive(Clone, PartialEq, Debug)]
pub struct AreaCount {
    pub wave: u32,
    /// The next wave's countdown while enemies stand (the waves are on
    /// a clock and stack); `None` while it is the big figure, and once
    /// the hold is out.
    pub next: Option<String>,
    pub hold: Option<String>,
    pub fob: f32,
    pub contested: bool,
}

/// What the count at the top says (`droid_line`'s figures, without its
/// words). `None` where nobody holds the place.
pub fn counter(world: &world::World) -> Option<Counter> {
    let standing = world.droids_standing();
    let up = |n: u32| Some((n.to_string(), theme::BAD));
    let counting = |due: f64| Some((crate::format::countdown(due), theme::CAUTION));
    let down = || Some(("0".to_string(), theme::MUTED));
    // An Area defend (October 2026): the waves never run out while the
    // hold's time runs, so no "of how many" — the time left instead, and
    // the FOB's bar.
    if let Some(defending) = world.defense_here()
        && let Some(area) = defending.area.as_ref()
    {
        let due = world.defense_wave_due();
        let (big, wave) = if standing > 0 {
            (up(standing), defending.wave)
        } else if let Some(due) = due {
            (counting(due), defending.wave + 1)
        } else {
            (down(), defending.wave)
        };
        let (fob, contested) = world.area_taken_share().unwrap_or((0.0, false));
        return Some(Counter {
            big,
            wave: None,
            core: None,
            area: Some(AreaCount {
                wave: wave.max(1),
                next: due.filter(|_| standing > 0).map(crate::format::countdown),
                hold: (area.left > 0)
                    .then(|| crate::format::countdown(world.area_time_left().unwrap_or(0.0))),
                fob,
                contested,
            }),
        });
    }
    if let Some(defending) = world.defense_here() {
        let waves = defending.wave + defending.waves_left;
        let due = world.defense_wave_due();
        let (big, wave) = if standing > 0 {
            (up(standing), defending.wave)
        } else if let Some(due) = due {
            (counting(due), defending.wave + 1)
        } else if defending.wave > 0 && defending.waves_left == 0 {
            (down(), defending.wave)
        } else {
            (up(0), defending.wave.max(1))
        };
        return Some(Counter {
            big,
            wave: Some((wave.min(waves.max(1)), waves.max(1))),
            core: None,
            area: None,
        });
    }
    let core = world.heart_status().map(|heart| {
        if heart.core_max > 0.0 {
            (heart.core_health / heart.core_max).clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    let Some((wave, left)) = world.droid_wave_standing() else {
        return core.map(|core| Counter {
            big: None,
            wave: None,
            core: Some(core),
            area: None,
        });
    };
    let waves = wave + left;
    let (big, wave) = if standing > 0 {
        (up(standing), wave)
    } else if let Some(due) = world.droid_wave_due() {
        (counting(due), wave + 1)
    } else if left == 0 {
        (down(), wave)
    } else {
        (up(0), wave)
    };
    Some(Counter {
        big,
        wave: Some((wave, waves)),
        core,
        area: None,
    })
}

/// An Area defend's half of the top count: the wave and the hold's time
/// left (or the last wave) on a line, and under it the FOB's bar — how
/// far the machines are to taking it, red, amber while somebody of the
/// crew's side stands in the ring and holds their count.
fn area_count(ui: &mut egui::Ui, area: &AreaCount) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.label(
            egui::RichText::new(area_wave(area.wave))
                .size(COUNT_WAVE_SIZE)
                .strong()
                .color(theme::INK),
        );
        if let Some(span) = &area.next {
            ui.label(
                egui::RichText::new(area_next(span))
                    .size(COUNT_WAVE_SIZE)
                    .strong()
                    .color(theme::CAUTION),
            );
        }
        let (hold, colour) = match &area.hold {
            Some(span) => (area_hold(span), theme::AREA),
            None => (AREA_LAST_WAVE.to_string(), theme::CAUTION),
        };
        ui.label(
            egui::RichText::new(hold)
                .size(COUNT_WAVE_SIZE)
                .strong()
                .color(colour),
        );
    })
    .response
    .on_hover_text(AREA_DEFENSE_TIP);
    if area.fob > 0.0 {
        let colour = if area.contested {
            theme::CAUTION
        } else {
            theme::BAD
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(AREA_FOB).small().strong().color(colour));
            theme::bar_of_height(ui, CORE_BAR_W, 8.0, &[(area.fob, colour)]);
            if area.contested {
                ui.label(egui::RichText::new(AREA_CONTESTED).small().color(colour));
            }
        });
    }
    ui.add_space(4.0);
}

/// How big the count of enemies standing is, and the wave beside it.
const COUNT_SIZE: f32 = 40.0;
const COUNT_WAVE_SIZE: f32 = 20.0;
/// How wide the Heart's core's bar under the count is.
const CORE_BAR_W: f32 = 180.0;

/// The count at the top centre — the enemies standing in big red, and
/// which wave of how many beside them — and the pause. Everything the
/// top frame used to spell out (the day, this player's money, the bounty
/// waiting on the site, every warning up and what it means) is in the
/// tooltip of the count. Never left of `clear`, which is the portraits'
/// right edge. The rectangle it took; an empty one at the top centre
/// with nothing to show.
pub fn top_frame(
    ctx: &egui::Context,
    canvas: egui::Rect,
    clear: f32,
    world: &world::World,
    local: u32,
    threats: &[Threat],
    paused: bool,
) -> egui::Rect {
    let count = counter(world);
    let centre = (canvas.min.x + canvas.max.x) / 2.0;
    if count.is_none() && !paused {
        return egui::Rect::from_min_size(
            egui::pos2(centre, canvas.min.y + MARGIN),
            egui::Vec2::ZERO,
        );
    }
    let id = egui::Id::new("hud-top");
    let width = ctx
        .memory(|m| m.area_rect(id).map(|r| r.width()))
        .unwrap_or(160.0);
    let x = (centre - width / 2.0).max(clear + GAP);
    egui::Area::new(id)
        .fixed_pos(egui::pos2(x, canvas.min.y + MARGIN))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            let shown = egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(140))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(12, 2))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            if let Some((big, colour)) = count.as_ref().and_then(|c| c.big.as_ref())
                            {
                                ui.label(
                                    egui::RichText::new(big)
                                        .size(COUNT_SIZE)
                                        .strong()
                                        .color(*colour),
                                );
                            }
                            if let Some((wave, waves)) = count.as_ref().and_then(|c| c.wave) {
                                ui.label(
                                    egui::RichText::new(wave_counter(wave, waves))
                                        .size(COUNT_WAVE_SIZE)
                                        .strong()
                                        .color(theme::INK),
                                );
                            }
                            if paused {
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
                        if let Some(core) = count.as_ref().and_then(|c| c.core) {
                            theme::bar_of_height(ui, CORE_BAR_W, 8.0, &[(core, theme::BAD)]);
                            ui.add_space(4.0);
                        }
                        if let Some(area) = count.as_ref().and_then(|c| c.area.as_ref()) {
                            area_count(ui, area);
                        }
                    });
                })
                .response
                .interact(egui::Sense::hover());
            shown.on_hover_ui(|ui| {
                ui.set_max_width(360.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(day_word(world.day())).strong());
                    ui.label(
                        egui::RichText::new(euros(world.share_of(local)))
                            .strong()
                            .color(theme::ACCENT),
                    );
                    if world.run.pending_bounty > 0 {
                        ui.label(
                            egui::RichText::new(on_clear_line(world.run.pending_bounty))
                                .color(theme::MUTED),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(wave_size_chip(world.droid_wave_size())).color(theme::WARN),
                );
                ui.add(egui::Label::new(egui::RichText::new(WAVE_SIZE_TIP).small()).wrap());
                for t in threats {
                    ui.separator();
                    ui.label(egui::RichText::new(&t.text).strong().color(t.colour));
                    ui.add(egui::Label::new(egui::RichText::new(&t.tip).small()).wrap());
                }
            });
        })
        .response
        .rect
}

/// The banner under the top frame for a player whose Bim is out: that it
/// is, and — on a hover — what buying it back costs and what the pool
/// holds.
pub fn out_banner(ctx: &egui::Context, centre: f32, top: f32, pool: u64, cost: u64) -> egui::Rect {
    let id = egui::Id::new("hud-out");
    let width = ctx
        .memory(|m| m.area_rect(id).map(|r| r.width()))
        .unwrap_or(120.0);
    egui::Area::new(id)
        .fixed_pos(egui::pos2(centre - width / 2.0, top))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            warning_frame()
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(OUT_BANNER).strong().color(theme::BAD));
                })
                .response
                .interact(egui::Sense::hover())
                .on_hover_text(out_banner_line(cost, pool));
        })
        .response
        .rect
}

/// The crew's relics in a row under the top frame, centred on `centre`,
/// the way a run's relics stand along the top of the screen in *Slay the
/// Spire*: every one counts for the whole crew all the time, so they are
/// always in sight — on the deck and over the map alike. Each is its
/// plate, in the order taken; resting the pointer on one says its name,
/// its boons and its price. Nothing while the crew hold none.
pub fn relic_bar(
    ctx: &egui::Context,
    centre: f32,
    top: f32,
    relics: &[world::Relic],
) -> Option<egui::Rect> {
    if relics.is_empty() {
        return None;
    }
    let id = egui::Id::new("hud-relics");
    let width = ctx
        .memory(|m| m.area_rect(id).map(|r| r.width()))
        .unwrap_or(crate::icons::RELIC * relics.len() as f32);
    let rect = egui::Area::new(id)
        .fixed_pos(egui::pos2(centre - width / 2.0, top))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::panel_frame()
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for &relic in relics {
                            crate::icons::relic_cell(ui, relic).on_hover_ui(|ui| {
                                ui.set_max_width(260.0);
                                ui.label(
                                    egui::RichText::new(relic_name(relic))
                                        .strong()
                                        .color(theme::INK),
                                );
                                super::worldmap::relic_lines_ui(ui, relic);
                            });
                        }
                    });
                });
        })
        .response
        .rect;
    Some(rect)
}

// --- the hero panel -----------------------------------------------------------------

/// The player's own Bim, as the panel at the foot of the canvas shows it.
pub struct Hero {
    pub class: world::Class,
    pub xp: u32,
    /// The body's points and the armour's, as the side panel reads them.
    pub points: f32,
    /// A whole bar for it (October 2026: a Reactor Heart raises it).
    pub max: f32,
    pub armour: f32,
    /// A shield's hit points on the end of the armour (a tank's Bastion).
    pub shield: f32,
    pub hurt: bool,
    /// Downed on the deck, with the countdown running (task 120).
    pub downed: bool,
    /// The seconds that countdown has left, while it runs: what the
    /// panel greyed over says under *Downed*.
    pub down_left: Option<f32>,
    /// The worst of what is wrong with it, in a line, and its colour:
    /// the countdown while it is down, the slower walk once it was.
    pub peril: Option<(String, egui::Color32)>,
    /// Skill points not spent on a ranked kit's ranks (task 124), said
    /// beside the experience bar.
    pub points_waiting: u8,
    /// This player's money: its wallet and its share of the takings.
    pub money: u64,
    /// The magazine in its hand (October 2026): shots left, shots it
    /// holds, and the share of a reload still to run — `Game::magazine`.
    pub magazine: Option<(u32, u32, f32)>,
    /// The trigger's two clocks, as `Game::trigger_times` reads them:
    /// the seconds until the next shot and the whole wait, and a
    /// magazine's reload left and its whole.
    pub trigger: Option<TriggerClocks>,
    /// The piece of armour worn, for the health bar's tier table.
    pub armour_worn: Option<bims::combat::Item>,
}

impl Hero {
    /// Critically hit (feature 110): down, or under
    /// [`bims::health::BLEEDS_UNDER`] and bleeding on the deck (task 120)
    /// — what the panel's red frame and its tag say at a glance, since
    /// the player's own Bim is the one body a player cannot afford to
    /// lose track of.
    pub fn critical(&self) -> bool {
        self.downed || self.points < bims::health::BLEEDS_UNDER
    }
}

/// What the hero panel was asked, and where it stood.
pub struct HeroOut {
    pub rect: egui::Rect,
    /// The key whose box the pointer rests on.
    pub hovered: Option<Action>,
}

/// The trigger's two clocks, as `Game::trigger_times` hands them: the
/// shot's (seconds left, the whole wait) and a magazine's reload's.
pub type TriggerClocks = ((f32, f32), Option<(f32, f32)>);

/// The circle the level stands in.
const LEVEL_DISC: f32 = 32.0;
/// The health bar's least width: as wide as the row of ability boxes over
/// it, but never narrower than this for a class with few boxes or none.
const HERO_BAR_W: f32 = 300.0;
/// The health bar's height: the tallest bar on the screen, since it is
/// the one a player has to read without looking (feature 110). The number
/// stands inside it.
const HERO_BAR_H: f32 = 26.0;
/// The number inside the health bar, and the magazine's.
const HERO_NUMBER: f32 = 18.0;
const MAGAZINE_NUMBER: f32 = 28.0;
/// The experience bar under the health bar.
const XP_BAR_H: f32 = 6.0;

/// The hero panel — the main of the HUD, Dota 2's bottom bar — centred at
/// the foot of the canvas but never left of `clear` — the right edge of
/// whatever on the left reaches down into its row, the tray opened or the
/// character sheet; nor right of `right`, the left edge of what stands at
/// the bottom right; nor off the canvas. From the left: the level, ringed
/// by the experience; the hands (`hand`) over this player's money; the
/// class's keys (`abilities`, which says which box the pointer rests on)
/// over the health bar with its number inside and the experience bar; the
/// magazine; and the four items (`items`). No words but numbers: what a
/// piece means is its tooltip.
#[allow(clippy::too_many_arguments)]
pub fn hero_panel(
    ctx: &egui::Context,
    canvas: egui::Rect,
    clear: f32,
    right: f32,
    hero: &Hero,
    hand: impl FnOnce(&mut egui::Ui),
    abilities: impl FnOnce(&mut egui::Ui) -> Option<Action>,
    items: impl FnOnce(&mut egui::Ui),
) -> HeroOut {
    let id = egui::Id::new("hud-hero");
    let size = ctx
        .memory(|m| m.area_rect(id).map(|r| r.size()))
        .unwrap_or(egui::vec2(760.0, 130.0));
    let x = ((canvas.min.x + canvas.max.x - size.x) / 2.0)
        .max(clear + GAP)
        .min(right.min(canvas.max.x - MARGIN) - GAP - size.x)
        .max(canvas.min.x + MARGIN);
    let y = canvas.max.y - MARGIN - size.y;
    let mut hovered = None;
    // Critically hit, the frame is the red of the ring on the deck and
    // beats, so it is seen out of the corner of an eye in a fight.
    let critical = hero.critical();
    let frame = if critical {
        let t = ctx.input(|i| i.time) as f32;
        let beat = 0.6 + 0.4 * (t * std::f32::consts::TAU * 1.2).sin().abs();
        theme::panel_frame().stroke(egui::Stroke::new(2.5, theme::DYING.gamma_multiply(beat)))
    } else {
        theme::panel_frame()
    };
    let area = egui::Area::new(id)
        .fixed_pos(egui::pos2(x, y))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            frame.inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    level_disc(ui, hero);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        hand(ui);
                        ui.label(
                            egui::RichText::new(euros(hero.money))
                                .size(15.0)
                                .strong()
                                .color(theme::ACCENT),
                        )
                        .on_hover_text(MAP_MONEY_TIP);
                    });
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let row = ui.horizontal(|ui| abilities(ui));
                        hovered = row.inner;
                        let width = row.response.rect.width().max(HERO_BAR_W);
                        health_bar(ui, hero, critical, width);
                        if hero.class != world::Class::None {
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(width, XP_BAR_H),
                                egui::Sense::hover(),
                            );
                            theme::bar_in(ui.painter(), rect, xp_fill(hero.xp), theme::HYPER);
                            response.on_hover_text(xp_text(hero.class, hero.xp));
                        }
                        trigger_row(ui, hero, width);
                    });
                    magazine_box(ui, hero);
                    ui.separator();
                    items(ui);
                });
            });
        });
    let rect = area.response.rect;
    // Down, the panel is greyed over and says so — with how long the body
    // has before the countdown runs out, since that is the one clock the
    // simulation keeps for a Bim on the deck.
    if hero.downed {
        let painter = ctx.layer_painter(area.response.layer_id);
        painter.rect_filled(rect, 6.0, theme::PANEL_DEEP.gamma_multiply(0.75));
        let lift = if hero.down_left.is_some() { 12.0 } else { 0.0 };
        painter.text(
            rect.center() - egui::vec2(0.0, lift),
            egui::Align2::CENTER_CENTER,
            DOWNED_BANNER,
            egui::FontId::proportional(26.0),
            theme::BAD,
        );
        if let Some(left) = hero.down_left {
            painter.text(
                rect.center() + egui::vec2(0.0, 16.0),
                egui::Align2::CENTER_CENTER,
                downed_left(left),
                egui::FontId::proportional(16.0),
                theme::WARN,
            );
        }
    }
    HeroOut { rect, hovered }
}

/// The health bar, tall and as wide as the keys over it: the body's
/// points, the armour's after them and a shield's after that, with the
/// numbers inside — the body's in white, the armour's and the shield's in
/// their colours after it. What is wrong with the Bim (critically hit,
/// bleeding, slowed) is its tooltip.
fn health_bar(ui: &mut egui::Ui, hero: &Hero, critical: bool, width: f32) {
    let ink = if critical {
        theme::DYING
    } else if hero.hurt {
        theme::BAD
    } else {
        theme::ACCENT
    };
    let total = (hero.max + hero.armour + hero.shield).max(1.0);
    let response = theme::bar_of_height(
        ui,
        width,
        HERO_BAR_H,
        &[
            (hero.points / total, ink),
            (hero.armour / total, theme::ARMOUR),
            (hero.shield / total, theme::SHIELD),
        ],
    );
    let rect = response.rect;
    let font = egui::FontId::proportional(HERO_NUMBER);
    let mut job = egui::text::LayoutJob::default();
    let mut part = |text: String, colour: egui::Color32| {
        job.append(
            &text,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: colour,
                ..Default::default()
            },
        );
    };
    part(
        format!("{} / {}", hero.points.max(0.0).round(), hero.max.round()),
        egui::Color32::WHITE,
    );
    if hero.armour > 0.0 {
        part(format!("  +{}", hero.armour.round()), theme::ARMOUR);
    }
    if hero.shield > 0.0 {
        part(format!("  +{}", hero.shield.round()), theme::SHIELD);
    }
    let painter = ui.painter();
    let galley = painter.layout_job(job);
    let at = rect.center() - galley.size() / 2.0;
    // A shadow under the numbers, so they read over any colour of bar.
    painter.galley_with_override_text_color(
        at + egui::vec2(1.0, 1.0),
        galley.clone(),
        egui::Color32::from_black_alpha(200),
    );
    painter.galley(at, galley, egui::Color32::WHITE);
    let tip: Vec<(String, egui::Color32)> = [
        critical.then(|| (CRITICAL_TIP.to_string(), theme::DYING)),
        hero.peril.clone().filter(|_| !hero.downed),
    ]
    .into_iter()
    .flatten()
    .collect();
    // And the armour worn: its name, what it has left, its tier table.
    let armour = hero.armour_worn;
    if !tip.is_empty() || armour.is_some() {
        response.on_hover_ui(|ui| {
            ui.set_max_width(320.0);
            for (line, colour) in tip {
                ui.add(egui::Label::new(egui::RichText::new(line).color(colour)).wrap());
            }
            if let Some(piece) = armour {
                crate::crew::tip_ui(ui, piece, 1);
            }
        });
    }
}

/// The level in its circle: the number, or a dash without a class, ringed
/// by how far through the level the experience is, and — while skill
/// points wait on a ranked kit (task 124) — their count on an accent disc
/// in the corner. Resting on it says the experience in numbers.
fn level_disc(ui: &mut egui::Ui, hero: &Hero) {
    use std::f32::consts::TAU;
    let side = LEVEL_DISC * 2.0 + 6.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter();
    let at = rect.center();
    painter.circle_filled(at, LEVEL_DISC, theme::PANEL_DEEP);
    painter.circle_stroke(at, LEVEL_DISC, egui::Stroke::new(4.0, theme::LINE));
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
            painter.add(egui::Shape::line(arc, egui::Stroke::new(4.0, theme::HYPER)));
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
        egui::FontId::proportional(28.0),
        theme::INK,
    );
    if hero.points_waiting > 0 {
        // A pill as wide as its figure, its right end on the disc's edge.
        let galley = painter.layout_no_wrap(
            format!("+{}", hero.points_waiting),
            egui::FontId::proportional(12.0),
            theme::PANEL_DEEP,
        );
        let pill = egui::Rect::from_min_size(
            egui::pos2(rect.max.x - galley.size().x - 10.0, rect.min.y),
            galley.size() + egui::vec2(10.0, 2.0),
        );
        painter.rect_filled(pill, pill.height() / 2.0, theme::ACCENT);
        painter.galley(pill.min + egui::vec2(5.0, 1.0), galley, theme::PANEL_DEEP);
    }
    response.on_hover_ui(|ui| {
        ui.label(xp_text(hero.class, hero.xp));
        if let Some(points) = crate::names::points_waiting(hero.points_waiting) {
            ui.label(egui::RichText::new(points).strong().color(theme::ACCENT));
        }
    });
}

/// The magazine's column (October 2026), between the keys and the items:
/// the shots left, large — warm at a quarter and less, red empty — over
/// what it holds, and while it reloads a bar filling as the reload runs.
/// Nothing for a weapon with no magazine.
fn magazine_box(ui: &mut egui::Ui, hero: &Hero) {
    let Some((left, size, reloading)) = hero.magazine else {
        return;
    };
    let response = ui
        .vertical(|ui| {
            ui.set_width(MAGAZINE_W);
            ui.spacing_mut().item_spacing.y = 2.0;
            let ink = if reloading > 0.0 || left == 0 {
                theme::BAD
            } else if left * 4 <= size {
                theme::WARN
            } else {
                theme::INK
            };
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(left.to_string())
                        .size(MAGAZINE_NUMBER)
                        .strong()
                        .color(ink),
                );
                ui.label(
                    egui::RichText::new(format!("/ {size}"))
                        .size(13.0)
                        .color(theme::MUTED),
                );
            });
            if reloading > 0.0 {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(MAGAZINE_W, 5.0), egui::Sense::hover());
                theme::bar_in(ui.painter(), rect, 1.0 - reloading, theme::ACCENT);
            }
        })
        .response;
    response.on_hover_text(magazine_tip(left, size));
}

/// The magazine's column's width.
const MAGAZINE_W: f32 = 64.0;

/// The trigger's clocks under the experience bar: the wait between two
/// shots on the left, a magazine's reload on the right, each in seconds
/// to the hundredth — the whole wait, muted, while the weapon is ready,
/// and red, counting down, while it is not. Nothing for a blade or an
/// empty hand.
fn trigger_row(ui: &mut egui::Ui, hero: &Hero, width: f32) {
    let Some((fire, reload)) = hero.trigger else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, TRIGGER_ROW_H), egui::Sense::hover());
    let painter = ui.painter();
    let clock = |word: &str, (left, whole): (f32, f32)| {
        let running = left > 0.0;
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("{word} "),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(12.0),
                color: theme::MUTED,
                ..Default::default()
            },
        );
        job.append(
            &trigger_seconds(if running { left } else { whole }),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(14.0),
                color: if running { theme::BAD } else { theme::MUTED },
                ..Default::default()
            },
        );
        painter.layout_job(job)
    };
    let fire = clock(TRIGGER_FIRE, fire);
    painter.galley(
        egui::pos2(rect.min.x, rect.center().y - fire.size().y / 2.0),
        fire,
        theme::MUTED,
    );
    if let Some(reload) = reload {
        let reload = clock(TRIGGER_RELOAD, reload);
        painter.galley(
            egui::pos2(
                rect.max.x - reload.size().x,
                rect.center().y - reload.size().y / 2.0,
            ),
            reload,
            theme::MUTED,
        );
    }
}

/// The trigger row's height.
const TRIGGER_ROW_H: f32 = 16.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// The experience line is whole points through the level, and `Max`
    /// with the bar full at the sixteenth.
    #[test]
    fn the_experience_line_is_whole_numbers_and_max_at_the_top() {
        // Every class climbs the same twenty (task 139, twenty since
        // October 2026), the tank too.
        let tank = world::Class::Tank;
        assert_eq!(xp_text(tank, 0), "Lv 1 · 0 / 100 XP");
        assert_eq!(xp_text(tank, 500), "Lv 4 · 140 / 160 XP");
        assert!((xp_fill(500) - 140.0 / 160.0).abs() < 1e-6);
        for xp in [4_920, 4_921, u32::MAX] {
            assert_eq!(xp_text(tank, xp), "Lv 20 · Max");
            assert_eq!(xp_fill(xp), 1.0);
        }
        let soldier = world::Class::Soldier;
        assert_eq!(xp_text(soldier, 500), "Lv 4 · 140 / 160 XP");
        assert_eq!(xp_text(soldier, 3_199), "Lv 15 · 329 / 330 XP");
        assert_eq!(xp_text(soldier, 3_200), "Lv 16 · 0 / 370 XP");
        assert_eq!(xp_text(soldier, 4_919), "Lv 19 · 489 / 490 XP");
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

//! The dice (task 146): a reward's clash played out on screen. Where two
//! players or more picked the same relic the world has already thrown —
//! [`world::relic::dice`], said as `WorldEvent::RelicDice` in the order
//! thrown — and given it; this window only plays those throws back, one
//! after another, two dice each, [`THROW_SECONDS`] a throw, and then says
//! who won. Nothing here decides anything.
//!
//! While it plays, the log's lines for the throws and the relics given
//! wait for it (so the log does not tell the end first), and the reward
//! window stands aside for it (`worldmap::relic_window`'s `hidden`).

use bevy_egui::egui;
use world::{Relic, WorldEvent};

use crate::names::*;
use crate::theme;

/// How long one player's throw takes on screen.
pub const THROW_SECONDS: f64 = 2.0;
/// Of a throw's seconds, how long the dice tumble before they lie.
const ROLLING: f64 = 1.4;
/// How long the winner is said after a clash's last throw.
pub const WINNER_SECONDS: f64 = 1.5;
/// A die's side, in points.
const DIE: f32 = 46.0;

/// One throw the world made: whose, for which relic, and the two dice.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Throw {
    slot: u32,
    relic: Relic,
    a: u32,
    b: u32,
}

/// A stretch of the show: a throw (by its index), or a clash's winner
/// said (by the index of the clash's first throw).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Part {
    Throw(usize),
    Winner(usize),
}

/// The throws still to play, and what waits on them.
#[derive(Default)]
pub struct DiceShow {
    /// Every throw queued, in the order thrown.
    throws: Vec<Throw>,
    /// Who the world gave each clash's relic to, as `RelicGiven` said.
    winners: Vec<(Relic, u32)>,
    /// When the show began, on egui's clock; `None` until it is first drawn.
    since: Option<f64>,
    /// The log's lines held until the show is over.
    held: Vec<String>,
}

impl DiceShow {
    /// Whether throws are queued or playing.
    pub fn playing(&self) -> bool {
        !self.throws.is_empty()
    }

    /// Reads one of the frame's events: a throw is queued, and a relic
    /// given for one queued is noted as its clash's winner. Whether the
    /// event's log line should wait for the show.
    pub fn note(&mut self, event: &WorldEvent) -> bool {
        match *event {
            WorldEvent::RelicDice { slot, relic, a, b } => {
                if let Some(relic) = Relic::from_code(relic) {
                    self.throws.push(Throw { slot, relic, a, b });
                }
                true
            }
            WorldEvent::RelicGiven { slot, relic } if self.playing() => {
                if let Some(relic) = Relic::from_code(relic)
                    && self.throws.iter().any(|t| t.relic == relic)
                {
                    self.winners.push((relic, slot));
                }
                true
            }
            _ => false,
        }
    }

    /// A clash to look at without two players picking (`BIMS_DICE=1`):
    /// crew members 0 and 1 tie for *Focusing Lens*, throw again, and the
    /// second wins.
    pub fn staged() -> DiceShow {
        let relic = Relic::FocusingLens;
        let throws = [(0, 3, 4), (1, 5, 2), (0, 2, 2), (1, 6, 3)];
        DiceShow {
            throws: throws
                .iter()
                .map(|&(slot, a, b)| Throw { slot, relic, a, b })
                .collect(),
            winners: vec![(relic, 1)],
            ..DiceShow::default()
        }
    }

    /// A log line that waits for the show to end.
    pub fn hold(&mut self, line: String) {
        self.held.push(line);
    }

    /// The show's stretches in order, each with its length: a throw each,
    /// and a clash's winner after its last throw.
    fn parts(&self) -> Vec<(Part, f64)> {
        let mut parts = Vec::new();
        let mut first = 0;
        for (i, t) in self.throws.iter().enumerate() {
            if i > 0 && self.throws[i - 1].relic != t.relic {
                first = i;
            }
            parts.push((Part::Throw(i), THROW_SECONDS));
            if self.throws.get(i + 1).is_none_or(|n| n.relic != t.relic) {
                parts.push((Part::Winner(first), WINNER_SECONDS));
            }
        }
        parts
    }

    /// Once a frame: the window for the stretch playing, over everything.
    /// The lines held, once the show is over — for the log.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        name: &dyn Fn(u32) -> String,
        colour: &dyn Fn(u32) -> egui::Color32,
    ) -> Vec<String> {
        if !self.playing() {
            return Vec::new();
        }
        let now = ctx.input(|i| i.time);
        let since = *self.since.get_or_insert(now);
        let mut left = now - since;
        let mut playing = None;
        for (part, length) in self.parts() {
            if left < length {
                playing = Some((part, left));
                break;
            }
            left -= length;
        }
        let Some((part, t)) = playing else {
            let held = std::mem::take(&mut self.held);
            *self = DiceShow::default();
            return held;
        };
        ctx.request_repaint();
        let first = match part {
            Part::Throw(i) => (0..=i)
                .rev()
                .take_while(|&j| self.throws[j].relic == self.throws[i].relic)
                .last()
                .unwrap_or(i),
            Part::Winner(first) => first,
        };
        let relic = self.throws[first].relic;
        let clash: Vec<Throw> = self.throws[first..]
            .iter()
            .take_while(|t| t.relic == relic)
            .copied()
            .collect();
        egui::Modal::new(egui::Id::new("relic-dice"))
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .frame(theme::tray_frame().inner_margin(14.0))
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                    crate::icons::relic(ui.painter(), rect, relic);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(DICE_TITLE).strong().size(18.0));
                        ui.label(egui::RichText::new(dice_heading(relic)).color(theme::MUTED));
                    });
                });
                ui.add_space(6.0);
                // The throws so far, the one playing last.
                let upto = match part {
                    Part::Throw(i) => i - first,
                    Part::Winner(_) => clash.len(),
                };
                let round = rounds(&clash);
                for (k, throw) in clash.iter().enumerate().take(upto + 1) {
                    if k == upto && matches!(part, Part::Winner(_)) {
                        break;
                    }
                    // The first throw of a new round of the tied.
                    if k > 0 && round[k] != round[k - 1] {
                        ui.label(egui::RichText::new(DICE_TIE).small().color(theme::MUTED));
                    }
                    let landed = k < upto || t >= ROLLING;
                    let who = name(throw.slot);
                    let text = if landed {
                        dice_threw(&who, throw.a, throw.b)
                    } else {
                        dice_throwing(&who)
                    };
                    ui.label(egui::RichText::new(text).strong().color(colour(throw.slot)));
                }
                ui.add_space(6.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(360.0, 120.0), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                match part {
                    Part::Throw(i) => {
                        let throw = self.throws[i];
                        roll(&painter, rect, throw, i, t, colour(throw.slot));
                    }
                    Part::Winner(_) => {
                        let winner = self
                            .winners
                            .iter()
                            .find(|(r, _)| *r == relic)
                            .map(|&(_, slot)| slot);
                        if let Some(slot) = winner {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                dice_winner(&name(slot), relic),
                                egui::FontId::proportional(22.0),
                                colour(slot),
                            );
                        }
                    }
                }
            });
        Vec::new()
    }
}

/// Which round of a clash each throw is in: a round ends where a player
/// throws who has thrown in it already — one of the tied throwing again.
fn rounds(clash: &[Throw]) -> Vec<u32> {
    let mut round = 0;
    let mut seen: Vec<u32> = Vec::new();
    clash
        .iter()
        .map(|t| {
            if seen.contains(&t.slot) {
                round += 1;
                seen.clear();
            }
            seen.push(t.slot);
            round
        })
        .collect()
}

/// One throw's two dice at `t` seconds into it: thrown in from the left,
/// tumbling and bouncing with their faces flickering for [`ROLLING`]
/// seconds, then lying still on the faces thrown, the sum under them.
fn roll(
    painter: &egui::Painter,
    rect: egui::Rect,
    throw: Throw,
    index: usize,
    t: f64,
    ink: egui::Color32,
) {
    let k = (t / ROLLING).min(1.0) as f32;
    // Out fast, slowing as it lands.
    let ease = 1.0 - (1.0 - k).powi(3);
    let tick = (t / 0.07) as u64;
    let rest_y = rect.center().y - 10.0;
    for (die, face) in [(0usize, throw.a), (1, throw.b)] {
        let rest_x = rect.center().x + if die == 0 { -38.0 } else { 38.0 };
        let start_x = rect.left() - DIE;
        let x = start_x + (rest_x - start_x) * ease;
        // Three bounces, each lower than the last.
        let bounce = ((k * std::f32::consts::PI * 3.0).sin().abs()) * 34.0 * (1.0 - k);
        let y = rest_y - bounce;
        let spin = if die == 0 { 1.0 } else { -1.3 };
        let angle = spin * 9.0 * (1.0 - ease);
        let shown = if k < 1.0 {
            let h = (tick ^ (die as u64 * 0x9E37) ^ (index as u64 * 0x7F4A_7C15))
                .wrapping_mul(0x2545_F491_4F6C_DD1D);
            (h >> 40) as u32 % 6 + 1
        } else {
            face
        };
        draw_die(painter, egui::pos2(x, y), DIE, angle, shown);
    }
    if k >= 1.0 {
        painter.text(
            egui::pos2(rect.center().x, rest_y + DIE * 0.5 + 18.0),
            egui::Align2::CENTER_CENTER,
            format!("{} + {} = {}", throw.a, throw.b, throw.a + throw.b),
            egui::FontId::proportional(18.0),
            ink,
        );
    }
}

/// A die of side `side` round `centre`, turned by `angle` radians, showing
/// `face` (one to six) as pips: an ivory body with rounded corners.
fn draw_die(painter: &egui::Painter, centre: egui::Pos2, side: f32, angle: f32, face: u32) {
    let (s, c) = angle.sin_cos();
    let turn = |v: egui::Vec2| centre + egui::vec2(v.x * c - v.y * s, v.x * s + v.y * c);
    let half = side * 0.5;
    let r = side * 0.18;
    let mut points = Vec::with_capacity(20);
    for (cx, cy, from) in [
        (half - r, half - r, 0.0f32),
        (-(half - r), half - r, 90.0),
        (-(half - r), -(half - r), 180.0),
        (half - r, -(half - r), 270.0),
    ] {
        for step in 0..=4 {
            let a = (from + step as f32 * 22.5).to_radians();
            points.push(turn(egui::vec2(cx + r * a.cos(), cy + r * a.sin())));
        }
    }
    // A shadow under it, then the body.
    let shadow: Vec<egui::Pos2> = points.iter().map(|p| *p + egui::vec2(3.0, 4.0)).collect();
    painter.add(egui::Shape::convex_polygon(
        shadow,
        egui::Color32::from_black_alpha(90),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        points,
        egui::Color32::from_rgb(244, 238, 222),
        egui::Stroke::new(1.5, egui::Color32::from_rgb(70, 64, 56)),
    ));
    let o = side * 0.27;
    let pips: &[(f32, f32)] = match face {
        1 => &[(0.0, 0.0)],
        2 => &[(-1.0, -1.0), (1.0, 1.0)],
        3 => &[(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)],
        4 => &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)],
        5 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (0.0, 0.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        _ => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
    };
    let pip = if face == 1 {
        egui::Color32::from_rgb(190, 40, 40)
    } else {
        egui::Color32::from_rgb(30, 28, 26)
    };
    for &(px, py) in pips {
        painter.circle_filled(turn(egui::vec2(px * o, py * o)), side * 0.09, pip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dice(slot: u32, relic: Relic, a: u32, b: u32) -> WorldEvent {
        WorldEvent::RelicDice {
            slot,
            relic: relic.code(),
            a,
            b,
        }
    }

    /// **A throw is two seconds, and each clash ends on its winner**: two
    /// clashes of two and three throws are five throws and two winners,
    /// in the order thrown; the given relics' lines wait for the show, a
    /// line of anything else does not.
    #[test]
    fn every_throw_is_two_seconds_and_a_clash_ends_on_its_winner() {
        let mut show = DiceShow::default();
        assert!(!show.note(&WorldEvent::RelicGiven { slot: 0, relic: 3 }));
        for e in [
            dice(0, Relic::KillRelay, 3, 4),
            dice(1, Relic::KillRelay, 6, 2),
            dice(0, Relic::FocusingLens, 1, 1),
            dice(1, Relic::FocusingLens, 1, 1),
            dice(2, Relic::FocusingLens, 5, 5),
        ] {
            assert!(show.note(&e));
        }
        assert!(show.note(&WorldEvent::RelicGiven {
            slot: 1,
            relic: Relic::KillRelay.code()
        }));
        assert!(!show.note(&WorldEvent::RelicPicked { slot: 0, relic: 3 }));
        assert_eq!(show.winners, vec![(Relic::KillRelay, 1)]);
        let parts = show.parts();
        assert_eq!(
            parts.iter().map(|p| p.0).collect::<Vec<_>>(),
            vec![
                Part::Throw(0),
                Part::Throw(1),
                Part::Winner(0),
                Part::Throw(2),
                Part::Throw(3),
                Part::Throw(4),
                Part::Winner(2),
            ]
        );
        let throws: f64 = parts
            .iter()
            .filter(|p| matches!(p.0, Part::Throw(_)))
            .map(|p| p.1)
            .sum();
        assert_eq!(throws, 5.0 * THROW_SECONDS);
    }
}

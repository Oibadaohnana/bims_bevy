//! The end of a fight: the screen that comes up in the mission the moment
//! the site is cleared — the last wave of a held station down, a town
//! held, a site of the Manufacturers' taken — and says what the fight
//! earned before the crew go on. Nothing here is a rule: the clear is the
//! world's (`Run::cleared_here`, set by `World::settle_clear`), the bounty
//! is already in the pool by the time it shows, and what follows is the
//! game's own way on — *Back to ship*, the relic reward on the map
//! (`worldmap::relic_window`), the map and the next trip.
//!
//! The tally is the screen's alone, like the heal numbers: the world keeps
//! run totals and no per-mission ones, so the screen notes the totals and
//! every crew member's experience the first frame of a mission, and adds up
//! the bounty, the Manufacturers down and each crew member's damage off
//! the events as they come. A world loaded or restarted mid-mission starts the tally from
//! there.

use bevy_egui::egui;
use world::{World, WorldEvent};

use super::designer::Order;
use crate::format::{euros, grouped};
use crate::names::*;
use crate::theme;

/// What this mission has earned so far, and whether the player has put
/// the screen away.
#[derive(Default)]
pub struct FightTally {
    /// `Run::missions` of the mission the tally is for; `None` between
    /// missions.
    mission: Option<u32>,
    /// The run's machines destroyed when the tally began.
    machines: u32,
    /// The run's deaths when the tally began.
    deaths: u64,
    /// Every crew member's experience when the tally began, by index — a
    /// crew member joined since began at nought.
    xp: Vec<u32>,
    /// The Republic's bounty paid into the pool during the mission.
    bounty: u64,
    /// The Manufacturers' people down (they die as people, not machines).
    people: u32,
    /// A town's survivors who joined the crew when it was held.
    joined: u32,
    /// What each crew member's hits took off the enemies, by index
    /// (`WorldEvent::Hit`'s `by`), in whole points as the numbers over
    /// them say it.
    damage: Vec<u64>,
    /// The player pressed *Back to ship* on it: the one way it is put
    /// away before the mission ends.
    dismissed: bool,
}

impl FightTally {
    /// Once a frame, before the frame's events are read: a new mission
    /// starts the tally afresh, and none clears it.
    pub fn follow(&mut self, world: &World) {
        if !world.in_mission() {
            self.mission = None;
            return;
        }
        if self.mission == Some(world.run.missions) {
            return;
        }
        *self = FightTally {
            mission: Some(world.run.missions),
            machines: world.run.machines_destroyed,
            deaths: world.run.deaths,
            xp: (0..world.aboard.crew_count())
                .map(|who| world.progress_of(who).xp)
                .collect(),
            ..FightTally::default()
        };
    }

    /// One of the frame's events, counted if it is something the fight
    /// earned.
    pub fn note(&mut self, event: &WorldEvent, manufacturer: bool) {
        if self.mission.is_none() {
            return;
        }
        match *event {
            WorldEvent::Bounty { amount } => self.bounty = self.bounty.saturating_add(amount),
            WorldEvent::EnemyDown { .. } if manufacturer => self.people += 1,
            WorldEvent::TownsfolkJoined { count } => self.joined += count,
            WorldEvent::Hit {
                resident: true,
                damage,
                by: Some(by),
                ..
            } => {
                let by = by as usize;
                if self.damage.len() <= by {
                    self.damage.resize(by + 1, 0);
                }
                self.damage[by] += u64::from(damage);
            }
            _ => {}
        }
    }

    /// Whether the screen is up this frame: the site of this mission
    /// cleared of a fight there was, the run neither won nor lost (those
    /// have their own screen), nobody being asked about leaving somebody
    /// behind, and the player not having put it away.
    pub fn showing(&self, world: &World) -> bool {
        self.mission.is_some()
            && !self.dismissed
            && world.in_mission()
            && world.run.fought
            && world.run.cleared_here
            && !world.is_won()
            && !world.lost
            && world.run.departure.is_none()
    }
}

/// The window's width, in points.
const WIDTH: f32 = 440.0;

/// The screen itself: a modal over the dimmed deck — the headline, three
/// tiles (the player's own damage, the machines destroyed, the bounty),
/// a row a player with its damage and experience and the bots together,
/// what else happened, and *Back to ship* — the bottom-right button's own
/// order, and the one way to put it away: a click beside it or Esc does
/// nothing, so a stray click after a fight never loses it. `leave` is
/// the Propose key (Space) pressed this frame and its name, a press of
/// the button too. A player who is out has no button and is told the
/// others will go. `tint` is a player's colour, by slot.
#[allow(clippy::too_many_arguments)]
pub fn fight_won_window(
    ctx: &egui::Context,
    tally: &mut FightTally,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
    tint: &dyn Fn(u32) -> egui::Color32,
    leave: (bool, &str),
) {
    if !tally.showing(world) {
        return;
    }
    let held = world
        .run
        .site
        .and_then(|id| world.defense(id))
        .is_some_and(|d| d.won);
    let machines = world.run.machines_destroyed.saturating_sub(tally.machines);
    let lost = world.run.deaths.saturating_sub(tally.deaths);
    let players = world.players();
    let crew = world.aboard.crew_count();
    let damage = |who: u32| tally.damage.get(who as usize).copied().unwrap_or(0);
    // Each player's Bim on a row of its own; the bots together.
    let gained = |who: u32| {
        let now = world.progress_of(who);
        let before = tally.xp.get(who as usize).copied().unwrap_or(0);
        (
            now.xp.saturating_sub(before),
            world::class::level_of(before),
            now.level(),
        )
    };
    let bots_xp: u32 = (players..crew).map(|who| gained(who).0).sum();
    let bots_damage: u64 = (players..crew).map(damage).sum();
    let out = world.run.is_out(local);
    let mut pressed = !out && leave.0;
    let _ = egui::Modal::new(egui::Id::new("fight-won"))
        .backdrop_color(egui::Color32::from_black_alpha(160))
        .frame(
            theme::tray_frame()
                .inner_margin(egui::Margin::symmetric(20, 18))
                .corner_radius(10.0)
                .stroke(egui::Stroke::new(1.0, theme::RAISED_ON)),
        )
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 4.0;
            // The headline.
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(FIGHT_WON_KICKER)
                        .small()
                        .strong()
                        .color(theme::MUTED),
                );
                ui.label(
                    egui::RichText::new(FIGHT_WON_TITLE)
                        .strong()
                        .size(28.0)
                        .color(theme::ACCENT),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(if held {
                            FIGHT_WON_HELD
                        } else {
                            FIGHT_WON_CLEARED
                        })
                        .color(theme::MUTED),
                    )
                    .wrap(),
                );
            });
            ui.add_space(12.0);
            // The three tiles.
            let gap = 8.0;
            let tile_w = (WIDTH - 2.0 * gap) / 3.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                tile(
                    ui,
                    tile_w,
                    &grouped(damage(local)),
                    FIGHT_WON_DAMAGE,
                    theme::CAUTION,
                );
                let enemies = if tally.people > 0 {
                    FIGHT_WON_ENEMIES
                } else {
                    FIGHT_WON_MACHINES
                };
                tile(
                    ui,
                    tile_w,
                    &grouped(u64::from(machines) + u64::from(tally.people)),
                    enemies,
                    theme::INK,
                );
                tile(
                    ui,
                    tile_w,
                    &euros(tally.bounty),
                    FIGHT_WON_BOUNTY,
                    theme::ACCENT,
                );
            });
            ui.add_space(12.0);
            // The crew: a row a player, the bots together.
            theme::heading(ui, FIGHT_WON_CREW);
            ui.add_space(2.0);
            egui::Frame::new()
                .fill(theme::PANEL_DEEP)
                .stroke(egui::Stroke::new(1.0, theme::LINE))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(WIDTH - 20.0);
                    egui::Grid::new("fight-won-crew")
                        .num_columns(3)
                        .spacing([14.0, 6.0])
                        .min_col_width(0.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("").small());
                            column_head(ui, FIGHT_WON_DAMAGE_HEAD);
                            column_head(ui, FIGHT_WON_XP_HEAD);
                            ui.end_row();
                            for slot in 0..players.min(crew) {
                                let (xp, from, to) = gained(slot);
                                let mine = slot == local;
                                ui.horizontal(|ui| {
                                    dot(ui, tint(slot));
                                    let who = egui::RichText::new(name(slot));
                                    ui.label(if mine {
                                        who.strong().color(theme::INK)
                                    } else {
                                        who.color(theme::INK)
                                    });
                                });
                                number(ui, &grouped(damage(slot)), mine);
                                ui.label(egui::RichText::new(fight_won_xp(xp, from, to)).color(
                                    if to > from {
                                        theme::ACCENT
                                    } else {
                                        theme::MUTED
                                    },
                                ));
                                ui.end_row();
                            }
                            if crew > players {
                                ui.horizontal(|ui| {
                                    dot(ui, theme::LINE);
                                    ui.label(
                                        egui::RichText::new(FIGHT_WON_BOTS).color(theme::MUTED),
                                    );
                                });
                                number(ui, &grouped(bots_damage), false);
                                ui.label(
                                    egui::RichText::new(fight_won_bots_xp(bots_xp))
                                        .color(theme::MUTED),
                                );
                                ui.end_row();
                            }
                        });
                });
            ui.add_space(8.0);
            // And the rest, a line each.
            let line = |ui: &mut egui::Ui, label: &str, value: egui::RichText| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(label).color(theme::MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(value);
                    });
                });
            };
            line(
                ui,
                FIGHT_WON_POOL,
                egui::RichText::new(euros(world.share_of(local))).strong(),
            );
            if tally.joined > 0 {
                line(
                    ui,
                    FIGHT_WON_JOINED,
                    egui::RichText::new(tally.joined.to_string()).strong(),
                );
            }
            if lost > 0 {
                line(
                    ui,
                    FIGHT_WON_LOST,
                    egui::RichText::new(lost.to_string())
                        .strong()
                        .color(theme::BAD),
                );
            }
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(FIGHT_WON_NEXT)
                        .small()
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            ui.add_space(12.0);
            if out {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(FIGHT_WON_WAITING).color(theme::MUTED));
                });
            } else if leave_button(ui, leave.1).clicked() {
                pressed = true;
            }
        });
    // Only the button (or its key) puts it away: a click beside the
    // window or Esc (what egui's modal calls closing it) is ignored, so a
    // stray click after a fight never loses the way back.
    if pressed {
        orders.push(Order::ReturnToShip);
        tally.dismissed = true;
    }
}

/// One of the three tiles: a big number over what it counts.
fn tile(ui: &mut egui::Ui, width: f32, value: &str, label: &str, colour: egui::Color32) {
    egui::Frame::new()
        .fill(theme::RAISED)
        .stroke(egui::Stroke::new(1.0, theme::LINE))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(6, 10))
        .show(ui, |ui| {
            ui.set_width(width - 14.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(value).strong().size(22.0).color(colour));
                ui.label(egui::RichText::new(label).small().color(theme::MUTED));
            });
        });
}

/// A column's head over the crew's rows.
fn column_head(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(theme::MUTED),
    );
}

/// A crew member's damage, the player's own brighter.
fn number(ui: &mut egui::Ui, text: &str, mine: bool) {
    let text = egui::RichText::new(text);
    ui.label(if mine {
        text.strong().color(theme::CAUTION)
    } else {
        text.color(theme::INK)
    });
}

/// A player's colour, a dot before the name.
fn dot(ui: &mut egui::Ui, colour: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.5, colour);
}

/// *Back to ship*, the width of the window, its words in the middle and
/// its key on a chip at the right.
fn leave_button(ui: &mut egui::Ui, key: &str) -> egui::Response {
    let (rect, press) = ui.allocate_exact_size(egui::vec2(WIDTH, 42.0), egui::Sense::click());
    let lit = press.hovered();
    let painter = ui.painter();
    painter.rect(
        rect,
        8.0,
        if lit { theme::ACCENT } else { theme::RAISED_ON },
        egui::Stroke::new(1.0, theme::ACCENT),
        egui::StrokeKind::Inside,
    );
    let ink = if lit { theme::PANEL_DEEP } else { theme::INK };
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        BACK_TO_SHIP,
        egui::FontId::proportional(18.0),
        ink,
    );
    let galley = painter.layout_no_wrap(
        key.to_uppercase(),
        egui::FontId::proportional(12.0),
        theme::INK,
    );
    let size = galley.size() + egui::vec2(12.0, 6.0);
    let chip = egui::Rect::from_center_size(
        egui::pos2(rect.max.x - 12.0 - size.x / 2.0, rect.center().y),
        size,
    );
    painter.rect(
        chip,
        4.0,
        theme::PANEL_DEEP,
        egui::Stroke::new(1.0, theme::ACCENT),
        egui::StrokeKind::Inside,
    );
    painter.galley(chip.center() - galley.size() / 2.0, galley, theme::INK);
    press.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The damage is each crew member's hits on the enemies added up: a
    /// hit on one of the crew, or nobody's (a sentry's, an enemy's), is
    /// nobody's damage, and between missions nothing is counted.
    #[test]
    fn damage_is_the_crews_hits_on_the_enemies() {
        let hit = |resident, damage, by| WorldEvent::Hit {
            resident,
            who: 9,
            damage,
            crit: false,
            by,
        };
        let mut tally = FightTally::default();
        tally.note(&hit(true, 50, Some(0)), false);
        assert!(tally.damage.is_empty(), "no mission, no tally");
        tally.mission = Some(1);
        tally.note(&hit(true, 50, Some(0)), false);
        tally.note(&hit(true, 7, Some(0)), false);
        tally.note(&hit(true, 30, Some(3)), false);
        tally.note(&hit(true, 99, None), false);
        tally.note(&hit(false, 40, Some(1)), false);
        assert_eq!(tally.damage, vec![57, 0, 0, 30]);
    }
}

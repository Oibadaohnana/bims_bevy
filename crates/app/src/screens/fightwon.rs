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
//! the bounty, the relics kept and the Manufacturers down off the events as
//! they come. A world loaded or restarted mid-mission starts the tally from
//! there.

use bevy_egui::egui;
use world::{World, WorldEvent};

use super::designer::Order;
use crate::format::euros;
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
    /// crew member hired since began at nought.
    xp: Vec<u32>,
    /// The Republic's bounty paid into the pool during the mission.
    bounty: u64,
    /// Relics kept on the clear: whose, and the relic's code.
    relics: Vec<(u32, u32)>,
    /// The Manufacturers' people down (they die as people, not machines).
    people: u32,
    /// A town's survivors who joined the crew when it was held.
    joined: u32,
    /// The player said *Stay here*, or pressed *Back to ship* on it.
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
            WorldEvent::RelicGiven { slot, relic } => self.relics.push((slot, relic)),
            WorldEvent::EnemyDown { .. } if manufacturer => self.people += 1,
            WorldEvent::TownsfolkJoined { count } => self.joined += count,
            _ => {}
        }
    }

    /// Whether the screen is up this frame: the site of this mission
    /// cleared of a fight there was, the run neither won nor lost (those
    /// have their own screen), nobody being asked about leaving somebody
    /// behind, and the player not having put it away.
    fn showing(&self, world: &World) -> bool {
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

/// The screen itself: a modal over the dimmed deck, the fight's earnings
/// in rows, what comes next, and two buttons — *Back to ship*, which is
/// the bottom-right button's own order, and *Stay here*, which puts the
/// screen away for the rest of the mission (the button at the bottom
/// right is still there for when the crew are done).
pub fn fight_won_window(
    ctx: &egui::Context,
    tally: &mut FightTally,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
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
    // Each player's Bim on a row of its own; the bots together.
    let gained = |who: u32| {
        let now = world.progress_of(who);
        let class = world.class_of(who);
        let before = tally.xp.get(who as usize).copied().unwrap_or(0);
        (
            now.xp.saturating_sub(before),
            world::class::level_of(class, before),
            now.level(class),
        )
    };
    let bots_xp: u32 = (players..crew).map(|who| gained(who).0).sum();
    let out = world.run.is_out(local);
    let returning = world.run.is_returning(local);
    let mut stay = false;
    let modal =
        egui::Modal::new(egui::Id::new("fight-won"))
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .frame(theme::tray_frame().inner_margin(14.0))
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(egui::RichText::new(FIGHT_WON_TITLE).strong().size(22.0));
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
                ui.add_space(8.0);
                egui::Grid::new("fight-won-rows")
                    .num_columns(2)
                    .spacing([18.0, 4.0])
                    .show(ui, |ui| {
                        let row = |ui: &mut egui::Ui, label: &str, value: egui::RichText| {
                            ui.label(egui::RichText::new(label).color(theme::MUTED));
                            ui.label(value);
                            ui.end_row();
                        };
                        row(
                            ui,
                            FIGHT_WON_MACHINES,
                            egui::RichText::new(machines.to_string()).strong(),
                        );
                        if tally.people > 0 {
                            row(
                                ui,
                                FIGHT_WON_PEOPLE,
                                egui::RichText::new(tally.people.to_string()).strong(),
                            );
                        }
                        row(
                            ui,
                            FIGHT_WON_BOUNTY,
                            egui::RichText::new(euros(tally.bounty))
                                .strong()
                                .color(theme::ACCENT),
                        );
                        row(ui, FIGHT_WON_POOL, egui::RichText::new(euros(world.money)));
                        for slot in 0..players.min(crew) {
                            let (xp, from, to) = gained(slot);
                            // A classless Bim earns nothing, and a row of noughts
                            // is noise.
                            if xp == 0 {
                                continue;
                            }
                            row(
                                ui,
                                &name(slot),
                                egui::RichText::new(fight_won_xp(xp, from, to))
                                    .color(if to > from { theme::ACCENT } else { theme::INK }),
                            );
                        }
                        if bots_xp > 0 {
                            row(
                                ui,
                                FIGHT_WON_BOTS,
                                egui::RichText::new(fight_won_bots_xp(bots_xp)),
                            );
                        }
                        for &(slot, relic) in &tally.relics {
                            if let Some(relic) = world::Relic::from_code(relic) {
                                row(
                                    ui,
                                    FIGHT_WON_RELIC,
                                    egui::RichText::new(format!(
                                        "{} · {}",
                                        relic_name(relic),
                                        name(slot)
                                    ))
                                    .color(theme::ACCENT),
                                );
                            }
                        }
                        if tally.joined > 0 {
                            row(
                                ui,
                                FIGHT_WON_JOINED,
                                egui::RichText::new(tally.joined.to_string()),
                            );
                        }
                        if lost > 0 {
                            row(
                                ui,
                                FIGHT_WON_LOST,
                                egui::RichText::new(lost.to_string()).color(theme::BAD),
                            );
                        }
                    });
                ui.add_space(8.0);
                ui.add(egui::Label::new(egui::RichText::new(FIGHT_WON_NEXT).small()).wrap());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if !out && !returning {
                        let press = ui.add(
                            egui::Button::new(egui::RichText::new(BACK_TO_SHIP).strong())
                                .min_size(egui::vec2(150.0, 30.0)),
                        );
                        if press.clicked() {
                            orders.push(Order::ReturnToShip);
                            stay = true;
                        }
                    }
                    if ui
                        .add(egui::Button::new(FIGHT_WON_STAY).min_size(egui::vec2(110.0, 30.0)))
                        .clicked()
                    {
                        stay = true;
                    }
                });
            });
    if stay || modal.should_close() {
        tally.dismissed = true;
    }
}

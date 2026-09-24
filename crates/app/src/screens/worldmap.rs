//! The world map and the end of a mission (feature 103, `world::run`):
//! the list of places a trip can go with what each would cost and find,
//! the vote on one, the *Back to ship* button and the departure check.
//!
//! Nothing here decides anything. What a trip costs is
//! `World::travel_quotes`, whether a player may vote or press is the
//! world's to refuse, and every press is an [`Order`] through the seam
//! like any other — so two players' maps agree because the world does.

use bevy_egui::egui;
use world::run::Departure;
use world::{Refusal, Site, TravelQuote, World};
use worldgen::{Galaxy, StarSystem};

use super::designer::Order;
use crate::format::roman;
use crate::names::*;
use crate::theme;

/// One place a trip could go: its name, and the quote or why not.
pub struct Destination {
    pub site: Site,
    pub name: String,
    pub quote: Result<TravelQuote, Refusal>,
}

/// The destinations of one system, under its heading.
pub struct Group {
    pub title: String,
    pub destinations: Vec<Destination>,
}

/// The map's own state: the list, worked out once per state of the world
/// it depends on rather than every frame — a quote to another system
/// generates it — and the destination picked on it.
#[derive(Default)]
pub struct WorldMap {
    key: Option<Key>,
    pub groups: Vec<Group>,
    /// The site the player has picked, off the list or the system map:
    /// looking, not a vote. [`Order::Propose`] is the vote.
    pub picked: Option<Site>,
}

/// Everything a quote reads that changes in a run: the star, the day,
/// which mission, where the crew are, the phase, and whether the jammer
/// or the site in hand has been dealt with since.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    star: u32,
    clock: u64,
    missions: u32,
    site: Option<u32>,
    phase: u32,
    jammed: bool,
    cleared: bool,
}

impl WorldMap {
    /// The list again, if anything it depends on has moved.
    pub fn refresh(&mut self, world: &World) {
        let key = Key {
            star: world.star_id,
            clock: world.clock_minutes.to_bits(),
            missions: world.run.missions,
            site: world.run.site,
            phase: world.run.phase.code(),
            jammed: world.jammed(),
            cleared: world.mission_cleared(),
        };
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let galaxy = world.galaxy();
        let mut groups: Vec<Group> = Vec::new();
        for (site, quote) in world.travel_quotes() {
            let name = site_name(world, &galaxy, site);
            if groups
                .last()
                .is_none_or(|g| g.destinations.last().map(|d| d.site.star) != Some(site.star))
            {
                let title = if site.star == world.star_id {
                    MAP_THIS_SYSTEM.to_string()
                } else {
                    map_next_system(
                        &galaxy
                            .star(site.star)
                            .map(|s| star_name(s.name))
                            .unwrap_or_default(),
                    )
                };
                groups.push(Group {
                    title,
                    destinations: Vec::new(),
                });
            }
            if let Some(group) = groups.last_mut() {
                group.destinations.push(Destination { site, name, quote });
            }
        }
        self.groups = groups;
    }

    /// The destination on the list for a site, if it is on it.
    pub fn find(&self, site: Site) -> Option<&Destination> {
        self.groups
            .iter()
            .flat_map(|g| g.destinations.iter())
            .find(|d| d.site == site)
    }
}

/// A site's name: a station's own, a settlement by its planet, and the
/// machines' relay where they built one.
pub fn site_name(world: &World, galaxy: &Galaxy, site: Site) -> String {
    let generated;
    let system: &StarSystem = if site.star == world.star_id {
        &world.system
    } else {
        match galaxy.system(site.star) {
            Some(system) => {
                generated = system;
                &generated
            }
            None => return String::new(),
        }
    };
    let star = galaxy
        .star(site.star)
        .map(|s| star_name(s.name))
        .unwrap_or_default();
    if let Some(body) = world::surface_body(site.station) {
        let part = system
            .body(body)
            .map(|b| roman(b.name.part as u32))
            .unwrap_or_default();
        return format!("{star} {part} settlement");
    }
    match system.station(site.station) {
        Some(station) => station_name(station.name),
        None => DERIVED_JAMMER_NAME.to_string(),
    }
}

/// The few words a row of the list says about what is there.
fn tags(quote: &TravelQuote) -> String {
    let mut words: Vec<String> = Vec::new();
    if quote.infested {
        words.push(ARRIVE_MACHINES.into());
        words.push(arrive_tier(quote.tier.code()));
    }
    if quote.jammer {
        words.push(ARRIVE_JAMMER.into());
    }
    if quote.threatened {
        words.push(ARRIVE_THREATENED.into());
    }
    if quote.cleared {
        words.push(ARRIVE_CLEARED.into());
    }
    words.join(" · ")
}

/// The world map's panel, in the strip at the top: the pool and the day,
/// every destination with its quote, the one picked spelt out, and the
/// vote — which only runs between missions; during one the list is to
/// read. `name` names a player's slot.
pub fn map_panel(
    ui: &mut egui::Ui,
    map: &mut WorldMap,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    map.refresh(world);
    let between = !world.in_mission();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(MAP_TITLE).strong());
        ui.label(
            egui::RichText::new(pool_line(
                world.day(),
                world.money,
                world.run.pending_bounty,
            ))
            .color(theme::MUTED),
        );
        theme::question_mark(ui, MAP_TIP);
    });
    ui.label(
        egui::RichText::new(if between { MAP_BETWEEN } else { MAP_READ_ONLY })
            .small()
            .color(if between { theme::ACCENT } else { theme::MUTED }),
    );
    if world.run.is_out(local) {
        ui.label(
            egui::RichText::new(out_line())
                .small()
                .color(theme::CAUTION),
        );
    }
    let here = world.current_site();
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for group in &map.groups {
                ui.label(
                    egui::RichText::new(&group.title)
                        .small()
                        .color(theme::MUTED),
                );
                for d in &group.destinations {
                    let picked = map.picked == Some(d.site);
                    let at = here == Some(d.site);
                    let (text, colour) = match &d.quote {
                        Ok(q) => {
                            let tags = tags(q);
                            let mut line =
                                format!("{}  {}", d.name, trip_quote(q.minutes, q.arrival_date));
                            if !tags.is_empty() {
                                line.push_str(&format!("  · {tags}"));
                            }
                            if at {
                                line.push_str(&format!("  ({MAP_HERE})"));
                            }
                            let colour = if q.infested || q.threatened {
                                theme::BAD
                            } else {
                                theme::INK
                            };
                            (line, colour)
                        }
                        Err(why) => (format!("{}  — {}", d.name, refusal(*why)), theme::MUTED),
                    };
                    let row = ui
                        .selectable_label(picked, egui::RichText::new(text).small().color(colour));
                    if row.clicked() {
                        map.picked = Some(d.site);
                    }
                }
            }
        });
    ui.separator();
    // The one picked, spelt out, and the vote on it.
    if let Some(site) = map.picked
        && let Some(d) = map.find(site)
    {
        ui.label(egui::RichText::new(&d.name).strong());
        match &d.quote {
            Ok(q) => {
                ui.label(picked_travel(q.minutes, q.days, q.arrival_date));
                ui.label(
                    egui::RichText::new(arrive_state(
                        q.infested,
                        q.tier.code(),
                        q.jammer,
                        q.threatened,
                    ))
                    .color(if q.infested || q.threatened {
                        theme::BAD
                    } else {
                        theme::MUTED
                    }),
                );
                if ui
                    .add_enabled(between, egui::Button::new(PROPOSE))
                    .clicked()
                {
                    orders.push(Order::Propose {
                        star: site.star,
                        station: site.station,
                    });
                }
            }
            Err(why) => {
                ui.label(egui::RichText::new(refusal(*why)).color(theme::WARN));
            }
        }
    }
    // The destination on the table, and who has said yes.
    if let Some(proposal) = &world.run.proposal {
        ui.separator();
        let galaxy_name = map
            .find(proposal.site)
            .map(|d| d.name.clone())
            .unwrap_or_default();
        ui.label(
            egui::RichText::new(proposal_line(&galaxy_name, &name(proposal.by)))
                .color(theme::ACCENT),
        );
        ui.horizontal_wrapped(|ui| {
            for (slot, &yes) in proposal.accepted.iter().enumerate() {
                let slot = slot as u32;
                ui.label(
                    egui::RichText::new(accepted_line(
                        &name(slot),
                        yes,
                        !world.run.is_connected(slot),
                    ))
                    .small()
                    .color(if yes { theme::ACCENT } else { theme::MUTED }),
                );
            }
        });
        let mine = proposal
            .accepted
            .get(local as usize)
            .copied()
            .unwrap_or(false);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(between && !mine, egui::Button::new(ACCEPT_TRIP))
                .clicked()
            {
                orders.push(Order::AcceptTrip(true));
            }
            if ui
                .add_enabled(between && mine, egui::Button::new(TAKE_BACK))
                .clicked()
            {
                orders.push(Order::AcceptTrip(false));
            }
        });
    }
}

/// The *Back to ship* button, at the bottom right of the canvas, during a
/// mission: pressed, it says how many of the players the ship waits for
/// are aboard — and presses again to ask a turned-down departure anew.
/// The rectangle it took, for whatever sits above it.
pub fn back_to_ship(
    ctx: &egui::Context,
    canvas_right: f32,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
) -> Option<egui::Rect> {
    if !world.in_mission() || world.run.is_out(local) {
        return None;
    }
    let returning = world.run.is_returning(local);
    let declined = matches!(world.run.departure, Some(Departure::Declined { .. }));
    let area = egui::Area::new(egui::Id::new("game-back-to-ship"))
        .anchor(
            egui::Align2::RIGHT_BOTTOM,
            egui::vec2(-10.0 - (ctx.viewport_rect().max.x - canvas_right), -10.0),
        )
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            crate::theme::panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if returning {
                        let (home, waited) = world.returning_count();
                        ui.label(
                            egui::RichText::new(returning_line(home, waited)).color(theme::ACCENT),
                        );
                        if declined && ui.button(ASK_AGAIN).clicked() {
                            orders.push(Order::ReturnToShip);
                        }
                    } else {
                        let press = ui.add(
                            egui::Button::new(egui::RichText::new(BACK_TO_SHIP).strong())
                                .min_size(egui::vec2(120.0, 28.0)),
                        );
                        if press.clicked() {
                            orders.push(Order::ReturnToShip);
                        }
                    }
                    theme::question_mark(ui, BACK_TO_SHIP_TIP);
                });
            });
        });
    Some(area.response.rect)
}

/// The departure check, while it is asking: who would be left behind,
/// every player's answer so far, and this player's two buttons.
pub fn departure_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    let Some(Departure::Asking { behind, answers }) = &world.run.departure else {
        return;
    };
    egui::Window::new(DEPARTURE_TITLE)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(DEPARTURE_LINE);
            for &who in behind {
                let down = world.aboard.room.is_down(who as usize);
                ui.label(
                    egui::RichText::new(if down {
                        format!("{} ({})", name(who), DOWNED_WORD)
                    } else {
                        name(who)
                    })
                    .color(theme::BAD),
                );
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for (slot, answer) in answers.iter().enumerate() {
                    let slot = slot as u32;
                    ui.label(
                        egui::RichText::new(departure_answer(
                            &name(slot),
                            *answer,
                            !world.run.is_connected(slot),
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                }
            });
            let mine = answers.get(local as usize).copied().flatten();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(mine != Some(true), egui::Button::new(LEAVE_YES))
                    .clicked()
                {
                    orders.push(Order::LeaveBehind(true));
                }
                if ui.button(LEAVE_NO).clicked() {
                    orders.push(Order::LeaveBehind(false));
                }
            });
        });
}

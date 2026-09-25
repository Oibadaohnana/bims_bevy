//! The world map and the end of a mission (feature 103, `world::run`):
//! the list of places a trip can go with what each would cost and find,
//! the vote on one, the *Back to ship* button and the departure check.
//! Feature 107 laid them out anew: the map is the chart on the left of
//! the whole canvas and a column down its right — the day, the pool, the
//! buyback queue, the list and a card for the place looked at — and the
//! departure check is one modal over the dimmed canvas.
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
use crate::format::{euros, roman};
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
    /// The site the pointer rests on, a row of the list or a station on
    /// the chart: what the card shows while it does (feature 107). The
    /// screen and the column set it afresh every frame.
    pub hovered: Option<Site>,
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

/// How wide the world map's column is, on the right of the chart.
pub const COLUMN_W: f32 = 360.0;

/// What the column asked of the screen that it cannot do itself.
#[derive(Default)]
pub struct ColumnAsk {
    /// The galaxy chart put up, or taken down again.
    pub chart: bool,
    /// The map closed (during a mission only: between missions it is the
    /// one thing there is to do).
    pub close: bool,
}

/// The world map's column (feature 107), down the right of the canvas
/// while the map is up: the day and the pool, the buyback queue, every
/// destination with its quote, and the card for the one looked at — the
/// one the pointer rests on, else the one picked, else the one on the
/// table — with the vote on it. The chart is the canvas to its left.
/// `chart_up` says which chart that is, for the toggle's word; `name`
/// names a player's slot.
#[allow(clippy::too_many_arguments)]
pub fn map_column(
    ctx: &egui::Context,
    canvas: egui::Rect,
    map: &mut WorldMap,
    world: &World,
    local: u32,
    chart_up: bool,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) -> ColumnAsk {
    map.refresh(world);
    let mut ask = ColumnAsk::default();
    let between = !world.in_mission();
    let here = world.current_site();
    // Hovered is this frame's: the rows below and the chart under the
    // pointer set it afresh.
    let hovered_before = map.hovered.take();
    let height = canvas.height() - 2.0 * crate::screens::hud::MARGIN;
    egui::Area::new(egui::Id::new("hud-map-column"))
        .fixed_pos(egui::pos2(
            canvas.max.x - crate::screens::hud::MARGIN - COLUMN_W,
            canvas.min.y + crate::screens::hud::MARGIN,
        ))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::tray_frame().show(ui, |ui| {
                ui.set_width(COLUMN_W - 16.0);
                ui.set_min_height(height - 16.0);
                ui.set_max_height(height - 16.0);
                // The head: the title, the chart's toggle, and the way out.
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(MAP_TITLE).strong().size(17.0));
                    theme::question_mark(ui, MAP_TIP);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !between && ui.button(MAP_CLOSE).clicked() {
                            ask.close = true;
                        }
                        if ui
                            .button(if chart_up { SYSTEM_VIEW } else { GALAXY_VIEW })
                            .clicked()
                        {
                            ask.chart = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(if between { MAP_BETWEEN } else { MAP_READ_ONLY })
                        .small()
                        .color(if between { theme::ACCENT } else { theme::MUTED }),
                );
                ui.add_space(4.0);
                egui::Grid::new("map-facts")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(MAP_DAY).color(theme::MUTED));
                        ui.label(egui::RichText::new(world.day().to_string()).strong());
                        ui.end_row();
                        ui.label(egui::RichText::new(MAP_POOL).color(theme::MUTED));
                        ui.label(
                            egui::RichText::new(euros(world.money))
                                .strong()
                                .color(theme::ACCENT),
                        );
                        ui.end_row();
                    });
                buyback_queue(ui, world, name);
                if world.run.is_out(local) {
                    ui.label(
                        egui::RichText::new(out_line())
                            .small()
                            .color(theme::CAUTION),
                    );
                }
                ui.separator();
                // Every destination, a row each, under its system.
                egui::ScrollArea::vertical()
                    .id_salt("map-list")
                    .max_height((height * 0.38).max(120.0))
                    .min_scrolled_height((height * 0.38).max(120.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for group in &map.groups {
                            ui.label(
                                egui::RichText::new(&group.title)
                                    .small()
                                    .color(theme::MUTED),
                            );
                            for d in &group.destinations {
                                let at = here == Some(d.site);
                                let (text, colour) = row_words(d, at);
                                let row = ui.selectable_label(
                                    map.picked == Some(d.site),
                                    egui::RichText::new(text).small().color(colour),
                                );
                                if row.hovered() {
                                    map.hovered = Some(d.site);
                                }
                                if row.clicked() {
                                    map.picked = Some(d.site);
                                }
                            }
                        }
                    });
                ui.separator();
                // The card: what the pointer rests on, else the pick, else
                // the proposal on the table. The chart's hover came in
                // before this frame's rows.
                let proposed = world.run.proposal.as_ref().map(|p| p.site);
                let shown = map.hovered.or(hovered_before).or(map.picked).or(proposed);
                if let Some(site) = shown {
                    destination_card(ui, map, world, site, local, between, orders, name);
                } else {
                    ui.label(
                        egui::RichText::new(MAP_PICK_HINT)
                            .small()
                            .color(theme::MUTED),
                    );
                }
            });
        });
    ask
}

/// The words of a row of the list, and their colour: the danger colour
/// for a place the machines hold or are coming for.
fn row_words(d: &Destination, at: bool) -> (String, egui::Color32) {
    match &d.quote {
        Ok(q) => {
            let tags = tags(q);
            let mut line = format!(
                "{}  {}",
                d.name,
                trip_quote(q.minutes, q.minimum, q.arrival_date)
            );
            if !tags.is_empty() {
                line.push_str(&format!("  · {tags}"));
            }
            if at {
                line.push_str(&format!("  ({MAP_HERE})"));
            }
            let colour = if q.infested || q.threatened {
                theme::BAD
            } else if at {
                theme::YOURS
            } else {
                theme::INK
            };
            (line, colour)
        }
        // The site the crew are at is never a trip (feature 105): it is
        // listed as where they are, not as a place refused.
        Err(_) if at => (format!("{}  ({MAP_HERE})", d.name), theme::YOURS),
        Err(why) => (format!("{}  — {}", d.name, refusal(*why)), theme::MUTED),
    }
}

/// The dead players waiting to be bought back, longest dead first, what
/// each costs and whether the pool covers it by the time their turn
/// comes — the order the next mission's start pays them in.
fn buyback_queue(ui: &mut egui::Ui, world: &World, name: &dyn Fn(u32) -> String) {
    if world.run.fallen.is_empty() {
        return;
    }
    ui.add_space(4.0);
    theme::heading(ui, BUYBACK_HEADING);
    let mut left = world.money;
    egui::Grid::new("map-buyback")
        .num_columns(3)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for fallen in &world.run.fallen {
                let covered = left >= world::data::BUYBACK_COST;
                if covered {
                    left -= world::data::BUYBACK_COST;
                }
                ui.label(name(fallen.slot));
                ui.label(euros(world::data::BUYBACK_COST));
                ui.label(
                    egui::RichText::new(if covered {
                        BUYBACK_COVERED
                    } else {
                        BUYBACK_SHORT
                    })
                    .small()
                    .color(if covered { theme::ACCENT } else { theme::WARN }),
                );
                ui.end_row();
            }
        });
}

/// The card for one destination: its name and how many hops, the trip,
/// the day of arrival, what is there on it, and — for the one on the
/// table — who put it and who has said yes, with this player's answer;
/// for any other, the button that puts it.
#[allow(clippy::too_many_arguments)]
fn destination_card(
    ui: &mut egui::Ui,
    map: &WorldMap,
    world: &World,
    site: Site,
    local: u32,
    between: bool,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    let Some(d) = map.find(site) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&d.name).strong().size(16.0));
        if world.current_site() == Some(site) {
            ui.label(egui::RichText::new(MAP_HERE).small().color(theme::YOURS));
        }
    });
    let quote = match &d.quote {
        Ok(q) => q,
        Err(why) => {
            ui.label(egui::RichText::new(refusal(*why)).color(theme::WARN));
            return;
        }
    };
    egui::Grid::new("map-card")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            let row = |ui: &mut egui::Ui, label: &str, value: String| {
                ui.label(egui::RichText::new(label).color(theme::MUTED));
                ui.label(value);
                ui.end_row();
            };
            row(ui, CARD_HOPS, hops_words(quote.jump));
            row(ui, CARD_TRAVEL, days_words(quote.days, quote.minimum));
            row(ui, CARD_ARRIVAL, quote.arrival_date.to_string());
        });
    // What is there on arrival, wrapped under the rows: a grid gives a
    // wrapping label no width of its own.
    ui.add(
        egui::Label::new(
            egui::RichText::new(arrive_state(
                quote.infested,
                quote.tier.code(),
                quote.jammer,
                quote.threatened,
            ))
            .color(if quote.infested || quote.threatened {
                theme::BAD
            } else {
                theme::MUTED
            }),
        )
        .wrap(),
    );
    ui.add_space(4.0);
    match world.run.proposal.as_ref().filter(|p| p.site == site) {
        Some(proposal) => {
            ui.label(egui::RichText::new(proposed_by(&name(proposal.by))).color(theme::ACCENT));
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
                        .color(if yes {
                            theme::ACCENT
                        } else {
                            theme::MUTED
                        }),
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
        None => {
            if ui
                .add_enabled(between, egui::Button::new(PROPOSE))
                .on_disabled_hover_text(MAP_READ_ONLY)
                .clicked()
            {
                orders.push(Order::Propose {
                    star: site.star,
                    station: site.station,
                });
            }
        }
    }
}

/// The *Back to ship* button, at the bottom right of the canvas, during a
/// mission: pressed, it says how many of the players the ship waits for
/// are aboard — `Returning · 1 / 2`, off `World::returning_count`, which
/// is the departure check's own rule — and presses again to ask a
/// turned-down departure anew. The rectangle it took, for the log that
/// sits above it.
pub fn back_to_ship(
    ctx: &egui::Context,
    canvas_right: f32,
    world: &World,
    local: u32,
    out: bool,
    orders: &mut Vec<Order>,
) -> Option<egui::Rect> {
    if !world.in_mission() || out || world.run.is_out(local) {
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
                        ui.add_sized(
                            egui::vec2(150.0, 32.0),
                            egui::Label::new(
                                egui::RichText::new(returning_text(world))
                                    .strong()
                                    .color(theme::ACCENT),
                            ),
                        );
                        if declined && ui.button(ASK_AGAIN).clicked() {
                            orders.push(Order::ReturnToShip);
                        }
                    } else {
                        let press = ui.add(
                            egui::Button::new(egui::RichText::new(BACK_TO_SHIP).strong())
                                .min_size(egui::vec2(150.0, 32.0)),
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

/// What the button says once it has been pressed: `Returning · a / b`,
/// the players aboard who have pressed it of the players the ship waits
/// for — `World::returning_count`, which is the departure check's own
/// rule, so a player down or out is in neither number.
pub fn returning_text(world: &World) -> String {
    let (home, waited) = world.returning_count();
    returning_line(home, waited)
}

/// The departure check (feature 107's restyle of feature 103's): one
/// modal over a dimmed canvas while it is asking — who would be left
/// behind and what each costs the run, every player's answer so far, and
/// this player's two buttons. What the buttons do is unchanged.
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
    let players = world.players();
    egui::Modal::new(egui::Id::new("departure"))
        .backdrop_color(egui::Color32::from_black_alpha(150))
        .frame(theme::tray_frame().inner_margin(14.0))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(egui::RichText::new(DEPARTURE_TITLE).strong().size(18.0));
            ui.add(
                egui::Label::new(egui::RichText::new(DEPARTURE_LINE).color(theme::MUTED)).wrap(),
            );
            ui.add_space(6.0);
            egui::Grid::new("departure-behind")
                .num_columns(2)
                .spacing([16.0, 3.0])
                .show(ui, |ui| {
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
                        ui.label(if who < players {
                            buyback_cost(world::data::BUYBACK_COST)
                        } else {
                            euros(world::data::BOT_DEATH_PENALTY)
                        });
                        ui.end_row();
                    }
                });
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
                        .color(match answer {
                            Some(true) => theme::ACCENT,
                            Some(false) => theme::WARN,
                            None => theme::MUTED,
                        }),
                    );
                }
            });
            ui.add_space(6.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use shipdesign::fixture::flyer;
    use world::fixture::{REFERENCE_MONEY, simulation_world};
    use world::world::Command;

    /// **`Returning · a / b` counts the way the departure check does**
    /// (feature 107): of the players the ship waits for — alive, at the
    /// keyboard, not out and **not down** — how many have pressed *Back to
    /// ship* and are aboard. A player out cold is in neither number, since
    /// the ship does not wait for somebody who cannot walk in.
    #[test]
    fn returning_counts_the_players_the_departure_check_waits_for() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.returning_count(), (0, 3));
        assert_eq!(returning_text(&world), "Returning · 0 / 3");
        world.apply_now(Command::Return { slot: 0 });
        assert_eq!(returning_text(&world), "Returning · 1 / 3");
        // Player 2 out cold — the blood taken under the line, and a step
        // for the body to go down — is no longer waited for, so the count
        // is of two.
        world.aboard.room.knock_out_for_probe(2);
        world.step(&[]);
        assert!(world.aboard.room.is_down(2));
        assert_eq!(returning_text(&world), "Returning · 1 / 2");
        // And a press from the one down counts for nothing: it is not
        // waited for, pressed or not.
        world.apply_now(Command::Return { slot: 2 });
        assert_eq!(returning_text(&world), "Returning · 1 / 2");
        world.apply_now(Command::Return { slot: 1 });
        assert_eq!(returning_text(&world), "Returning · 2 / 2");
    }
}

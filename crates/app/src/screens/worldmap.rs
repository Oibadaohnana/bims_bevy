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
use world::{Refusal, Site, SiteKind, TravelQuote, World};
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
    /// Whether it is a trader (task 114): marked in the list, and greyed
    /// out with the reason while it is closed.
    pub trader: bool,
    /// What it is to the crew (task 111): the quote's, else — a site
    /// refused, the one the crew are at among them — worked out here for
    /// this system and a trader anywhere. What the row leads with.
    pub kind: Option<SiteKind>,
}

/// The destinations of one system, under its heading.
pub struct Group {
    pub title: String,
    /// The system's star, by name: what a row says beside itself when the
    /// list is sorted by distance and the headings are gone (task 135).
    pub star: String,
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
    /// The pick came from the galaxy chart ([`WorldMap::pick_star`]): the
    /// list scrolls its row into view the next frame, once.
    scroll_to_pick: bool,
    /// The list sorted by how long each trip is rather than grouped by
    /// system (task 135): the viewer's choice, kept while the map is.
    pub by_distance: bool,
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
        let traders = world.trader_sites();
        let mut groups: Vec<Group> = Vec::new();
        for (site, quote) in world.travel_quotes() {
            let trader = traders.contains(&site);
            let name = site_name(world, &galaxy, site);
            if groups
                .last()
                .is_none_or(|g| g.destinations.last().map(|d| d.site.star) != Some(site.star))
            {
                let star = galaxy
                    .star(site.star)
                    .map(|s| star_name(s.name))
                    .unwrap_or_default();
                let title = if site.star == world.star_id {
                    MAP_THIS_SYSTEM.to_string()
                } else {
                    map_next_system(&star)
                };
                groups.push(Group {
                    title,
                    star,
                    destinations: Vec::new(),
                });
            }
            if let Some(group) = groups.last_mut() {
                let kind = match &quote {
                    Ok(q) => Some(q.kind),
                    Err(_) if trader => Some(SiteKind::Trader),
                    Err(_) if site.star == world.star_id => Some(world.site_kind(site.station)),
                    Err(_) => None,
                };
                group.destinations.push(Destination {
                    site,
                    name,
                    quote,
                    trader,
                    kind,
                });
            }
        }
        self.groups = groups;
    }

    /// A star picked on the galaxy chart: its first place a trip can go
    /// picked on the list — a trader first, else the first quoted, else
    /// the first listed — so the card offers the trip. A star with no
    /// place on the list (the ship's own is on it; one more than a lane
    /// off is not) leaves nothing picked, and the chart's panel says why.
    pub fn pick_star(&mut self, star: u32) {
        let of_star = || {
            self.groups
                .iter()
                .flat_map(|g| g.destinations.iter())
                .filter(move |d| d.site.star == star)
        };
        self.picked = of_star()
            .find(|d| d.trader && d.quote.is_ok())
            .or_else(|| of_star().find(|d| d.quote.is_ok()))
            .or_else(|| of_star().next())
            .map(|d| d.site);
        self.scroll_to_pick = self.picked.is_some();
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
    // The Machine Heart's fortress (feature 108) is called what it is,
    // whatever name its seed rolled.
    if world::heart::is_heart(site.station) {
        return HEART_NAME.to_string();
    }
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

/// The few words a row of the list says about what is there, after the
/// kind's own word (task 111), which the row leads with in its colour
/// (`row_job`) and these do not repeat.
fn tags(quote: &TravelQuote) -> String {
    let mut words: Vec<String> = Vec::new();
    if quote.infested {
        words.push(ARRIVE_MACHINES.into());
        words.push(arrive_tier(quote.tier.code()));
    }
    if quote.jammer {
        words.push(ARRIVE_JAMMER.into());
    }
    // A defence to come says at what tier the machines will come.
    if quote.threatened {
        words.push(arrive_tier(quote.tier.code()));
    }
    if quote.cleared {
        words.push(ARRIVE_CLEARED.into());
    }
    if quote.heart.is_some() {
        words.push(ARRIVE_HEART.into());
    }
    if quote.manufacturers {
        words.push(ARRIVE_MANUFACTURERS.into());
        words.push(arrive_tier(quote.tier.code()));
    }
    words.join(" · ")
}

/// A row of the list as egui lays it out: the kind's word first, strong and
/// in its colour (task 111), then the rest in the row's own colour.
fn row_job(
    style: &egui::Style,
    kind: Option<SiteKind>,
    text: &str,
    colour: egui::Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let small = egui::TextStyle::Small.resolve(style);
    if let Some(kind) = kind {
        job.append(
            site_kind_word(kind),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(small.size + 1.0, egui::FontFamily::Proportional),
                color: theme::site_kind_colour(kind),
                ..Default::default()
            },
        );
        job.append("  ", 0.0, egui::TextFormat::default());
    }
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: small,
            color: colour,
            ..Default::default()
        },
    );
    job
}

/// How wide the world map's column is, on the right of the chart.
pub const COLUMN_W: f32 = 360.0;

/// What the column asked of the screen that it cannot do itself.
#[derive(Default)]
pub struct ColumnAsk {
    /// The map closed (during a mission only: between missions it is the
    /// one thing there is to do).
    pub close: bool,
}

/// The world map's column (feature 107), down the right of the canvas
/// while the map is up: the day and the pool, the buyback queue, every
/// destination with its quote, and the card for the one looked at — the
/// one the pointer rests on, else the one picked, else the one on the
/// table — with the vote on it. The charts are the canvas to its left:
/// the galaxy and the system side by side (task 135). `name` names a
/// player's slot.
#[allow(clippy::too_many_arguments)]
pub fn map_column(
    ctx: &egui::Context,
    canvas: egui::Rect,
    map: &mut WorldMap,
    world: &World,
    local: u32,
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
                // The head: the title and the way out.
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(MAP_TITLE).strong().size(17.0));
                    theme::question_mark(ui, MAP_TIP);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !between && ui.button(MAP_CLOSE).clicked() {
                            ask.close = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(if world.at_trader() {
                        MAP_AT_TRADER
                    } else if between {
                        MAP_BETWEEN
                    } else {
                        MAP_READ_ONLY
                    })
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
                // The list's order (task 135): by system, or by distance.
                ui.horizontal(|ui| {
                    for (by_distance, word) in [(false, MAP_SORT_SYSTEM), (true, MAP_SORT_DISTANCE)]
                    {
                        let chosen = map.by_distance == by_distance;
                        if ui
                            .selectable_label(chosen, egui::RichText::new(word).small())
                            .clicked()
                        {
                            map.by_distance = by_distance;
                        }
                    }
                    theme::question_mark(ui, MAP_SORT_TIP);
                });
                // The groups out of the map while the rows pick on it.
                let groups = std::mem::take(&mut map.groups);
                let sorted = map.by_distance;
                // Every destination, a row each: under its system, or all
                // together the nearest first with its system beside it.
                egui::ScrollArea::vertical()
                    .id_salt("map-list")
                    .max_height((height * 0.38).max(120.0))
                    .min_scrolled_height((height * 0.38).max(120.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let mut row = |ui: &mut egui::Ui, d: &Destination, star: Option<&str>| {
                            let at = here == Some(d.site);
                            let (mut text, colour) = row_words(d, at);
                            if let Some(star) = star {
                                text.push_str(&format!("  · {star}"));
                            }
                            let job = row_job(ui.style(), d.kind, &text, colour);
                            let row = ui.selectable_label(map.picked == Some(d.site), job);
                            if row.hovered() {
                                map.hovered = Some(d.site);
                            }
                            if row.clicked() {
                                map.picked = Some(d.site);
                            }
                            if map.scroll_to_pick && map.picked == Some(d.site) {
                                row.scroll_to_me(Some(egui::Align::Center));
                                map.scroll_to_pick = false;
                            }
                        };
                        if sorted {
                            for (group, d) in by_distance(&groups, here) {
                                let other = d.site.star != world.star_id;
                                row(ui, d, other.then_some(group.star.as_str()));
                            }
                        } else {
                            for group in &groups {
                                ui.label(
                                    egui::RichText::new(&group.title)
                                        .small()
                                        .color(theme::MUTED),
                                );
                                for d in &group.destinations {
                                    row(ui, d, None);
                                }
                            }
                        }
                    });
                map.groups = groups;
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

/// Every destination of every group, the nearest first (task 135): the
/// site the crew are at, then every trip by its length in minutes, then
/// the sites no trip goes to. Ties keep the list's own order — this
/// system first, then the stars in id order.
fn by_distance(groups: &[Group], here: Option<Site>) -> Vec<(&Group, &Destination)> {
    let mut rows: Vec<(&Group, &Destination)> = groups
        .iter()
        .flat_map(|g| g.destinations.iter().map(move |d| (g, d)))
        .collect();
    rows.sort_by_key(|(_, d)| match &d.quote {
        _ if here == Some(d.site) => (0, 0),
        Ok(q) => (1, q.minutes),
        Err(_) => (2, 0),
    });
    rows
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
        // A trader shut by the crisis (task 114): marked, greyed out,
        // and the reason in a word.
        Err(Refusal::TraderClosed) if d.trader => {
            (format!("{}  · {TRADER_CLOSED}", d.name), theme::MUTED)
        }
        Err(Refusal::ClosedOnArrival) if d.trader => (
            format!("{}  · {TRADER_CLOSED_ON_ARRIVAL}", d.name),
            theme::MUTED,
        ),
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
    // What it is to the crew (task 111), big and in its colour before
    // anything else, with what that means on a hover; a trader (task 114)
    // says what a visit is as well.
    if let Some(kind) = d.kind {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(site_kind_word(kind))
                    .strong()
                    .size(18.0)
                    .color(theme::site_kind_colour(kind)),
            );
            theme::question_mark(ui, SITE_KIND_TIP);
        });
    }
    if d.trader {
        theme::asks(ui, ARRIVE_TRADER, TRADER_TIP);
    }
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
    // The Machine Heart's strength on arrival (feature 108): the numbers
    // the fight will be built with, so waiting is seen to cost.
    if let Some(heart) = quote.heart {
        ui.label(
            egui::RichText::new(HEART_ON_ARRIVAL)
                .strong()
                .color(theme::BAD),
        );
        egui::Grid::new("map-heart")
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for (label, value) in heart_preview_rows(&heart) {
                    ui.label(egui::RichText::new(label).color(theme::MUTED));
                    ui.label(value);
                    ui.end_row();
                }
            });
    }
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
    // Not while the mission waits for the ready check: nothing has
    // started to go back from.
    if !world.in_mission() || out || world.run.is_out(local) || world.awaiting_ready() {
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
                        // A fight won says so on the button: the deck is
                        // frozen and the press is all that is left (task 133).
                        let label = if world.fight_over() {
                            egui::RichText::new(FIGHT_WON_BACK_TO_SHIP)
                                .strong()
                                .color(theme::ACCENT)
                        } else {
                            egui::RichText::new(BACK_TO_SHIP).strong()
                        };
                        let press =
                            ui.add(egui::Button::new(label).min_size(egui::vec2(150.0, 32.0)));
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

/// The ready check: while a mission with a fight in it is held
/// (`World::awaiting_ready`), a panel at the top of the canvas — the
/// site's kind, every player's answer so far and this player's button.
/// Not a modal: the loadouts and the skill points stay in reach, and the
/// world hears them while it waits.
pub fn ready_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    if !world.in_mission() || !world.awaiting_ready() {
        return;
    }
    let kind = world
        .ship
        .state
        .alongside()
        .map_or(SiteKind::Defend, |id| world.site_kind(id));
    let mine = world.run.is_ready(local);
    egui::Area::new(egui::Id::new("ready-check"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 90.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            theme::tray_frame().inner_margin(14.0).show(ui, |ui| {
                ui.set_width(360.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(ready_title(kind)).strong().size(20.0));
                    ui.add(
                        egui::Label::new(egui::RichText::new(READY_LINE).color(theme::MUTED))
                            .wrap(),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for slot in 0..world.players() {
                            let ready = world.run.is_ready(slot);
                            let gone = !world.run.is_connected(slot);
                            ui.label(
                                egui::RichText::new(ready_answer(&name(slot), ready, gone))
                                    .small()
                                    .color(if ready { theme::ACCENT } else { theme::MUTED }),
                            );
                        }
                    });
                    ui.add_space(6.0);
                    let (word, yes) = if mine {
                        (READY_NO, false)
                    } else {
                        (READY_YES, true)
                    };
                    let press = ui.add(
                        egui::Button::new(egui::RichText::new(word).strong().size(16.0))
                            .min_size(egui::vec2(160.0, 34.0)),
                    );
                    if press.clicked() {
                        orders.push(Order::Ready(yes));
                    }
                });
            });
        });
}

/// What this player has picked in the relic window and not yet proposed:
/// a relic's code or `u32::MAX` for none, and whose Bim. The window's own,
/// kept in egui's memory between frames.
#[derive(Clone, Copy, PartialEq, Debug)]
struct RelicPick {
    relic: Option<u32>,
    to: u32,
}

/// The relic choice (feature 106): the reward screen after a site cleared
/// with machines in it — a modal over a dimmed canvas, since the map waits
/// on it — or a cache's one relic in the mission, a window beside the
/// fight, which goes on. Either way the same vote the map's trips are
/// chosen by: a player picks a relic, or none, and a player's Bim for it
/// and proposes; every connected player says yes; a new proposal clears
/// every yes.
pub fn relic_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    let Some(choice) = world.relic_choice() else {
        return;
    };
    let id = egui::Id::new("relic-pick");
    let mut pick = ctx
        .data(|d| d.get_temp::<RelicPick>(id))
        .unwrap_or(RelicPick {
            relic: None,
            to: local,
        });
    let reward = choice.source == world::relic::Source::Reward;
    let body = |ui: &mut egui::Ui, pick: &mut RelicPick, orders: &mut Vec<Order>| {
        ui.set_width(420.0);
        let (title, intro) = if reward {
            (REWARD_TITLE, REWARD_INTRO)
        } else {
            (CACHE_TITLE, CACHE_INTRO)
        };
        ui.label(egui::RichText::new(title).strong().size(18.0));
        ui.add(egui::Label::new(egui::RichText::new(intro).color(theme::MUTED)).wrap());
        ui.add_space(6.0);
        for &relic in &choice.options {
            let on = pick.relic == Some(relic.code());
            let text = egui::RichText::new(relic_name(relic)).strong();
            if theme::toggle(ui, on, text).clicked() {
                pick.relic = Some(relic.code());
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(relic_line(relic))
                        .small()
                        .color(theme::INK),
                )
                .wrap(),
            );
            ui.add_space(3.0);
        }
        if theme::toggle(ui, pick.relic == Some(u32::MAX), TAKE_NONE).clicked() {
            pick.relic = Some(u32::MAX);
        }
        ui.add_space(4.0);
        // Whose Bim: a player's, never a bot's.
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(FOR_BIM).color(theme::MUTED));
            for slot in 0..world.players() {
                if theme::toggle(ui, pick.to == slot, name(slot)).clicked() {
                    pick.to = slot;
                }
            }
        });
        ui.add_space(4.0);
        if ui
            .add_enabled(pick.relic.is_some(), egui::Button::new(PROPOSE))
            .clicked()
            && let Some(code) = pick.relic
        {
            orders.push(Order::ProposeRelic {
                relic: world::Relic::from_code(code),
                to: pick.to,
            });
        }
        // What is on the table, and who has said yes to it.
        if let Some(p) = &choice.proposal {
            ui.separator();
            ui.label(egui::RichText::new(relic_proposal_line(p.relic, p.to)).strong());
            ui.horizontal_wrapped(|ui| {
                for (slot, &yes) in p.accepted.iter().enumerate() {
                    let slot = slot as u32;
                    ui.label(
                        egui::RichText::new(relic_answer(
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
            let mine = p.accepted.get(local as usize).copied().unwrap_or(false);
            ui.horizontal(|ui| {
                if ui.add_enabled(!mine, egui::Button::new(ACCEPT)).clicked() {
                    orders.push(Order::AcceptRelic(true));
                }
                if ui.add_enabled(mine, egui::Button::new(TAKE_BACK)).clicked() {
                    orders.push(Order::AcceptRelic(false));
                }
            });
        }
    };
    if reward {
        egui::Modal::new(egui::Id::new("relic-reward"))
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .frame(theme::tray_frame().inner_margin(14.0))
            .show(ctx, |ui| body(ui, &mut pick, orders));
    } else {
        egui::Window::new(CACHE_TITLE)
            .id(egui::Id::new("relic-cache"))
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 90.0))
            .frame(theme::tray_frame().inner_margin(14.0))
            .show(ctx, |ui| body(ui, &mut pick, orders));
    }
    ctx.data_mut(|d| d.insert_temp(id, pick));
}

/// The Trader panel (task 114), while the crew are at a trader: the whole
/// visit on the map. The shelf in weapons and armour — each thing's kind,
/// tier, numbers and price, and *Buy* for whoever the player has chosen
/// (its own Bim, a bot, or the armory) — then the relic with its vote,
/// then what the armory and the Bims the player may change have to
/// combine, and the pool. A window of its own that may be moved, so the
/// Armory panel (Tab) can be up beside it. Nothing here decides anything:
/// every press is an [`Order`] the world may refuse, and the refusal is
/// the log's line.
pub fn trader_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    let Some(trader) = world.trader_here() else {
        return;
    };
    // Whom a purchase is for, kept between frames: a crew index, or the
    // armory as `u32::MAX`. The player's own Bim to begin with.
    let id = egui::Id::new("trader-for");
    let mut to = ctx.data(|d| d.get_temp::<u32>(id)).unwrap_or(local);
    let crew = world.aboard.crew_count();
    if to != u32::MAX && !(to < crew && world.may_change(local, to)) {
        to = local;
    }
    let relic_id = egui::Id::new("trader-relic-for");
    let mut relic_to = ctx.data(|d| d.get_temp::<u32>(relic_id)).unwrap_or(local);
    egui::Window::new(TRADER_TITLE)
        .id(egui::Id::new("trader-window"))
        .title_bar(false)
        .default_pos(egui::pos2(24.0, 90.0))
        .collapsible(false)
        .resizable(false)
        .frame(theme::panel_frame())
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(TRADER_TITLE).strong().size(17.0));
                theme::question_mark(ui, TRADER_TIP);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(euros(world.money))
                            .strong()
                            .color(theme::ACCENT),
                    );
                    ui.label(egui::RichText::new(MAP_POOL).color(theme::MUTED));
                });
            });
            ui.add(
                egui::Label::new(
                    egui::RichText::new(TRADER_INTRO)
                        .small()
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            // A trader near the machines charges over the odds for what a
            // fight is fought with (feature 94): said outright.
            if let Some(hops) = world.run.site.and_then(|id| world.front_at(id)) {
                theme::asks(ui, &front_premium(hops), FRONT_PREMIUM_TIP);
            }
            // For whom: the player's own Bim, every bot, or the armory.
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(FOR_BIM).color(theme::MUTED));
                for who in 0..crew {
                    if world.may_change(local, who)
                        && world.aboard.room.is_alive(who as usize)
                        && theme::toggle(ui, to == who, name(who)).clicked()
                    {
                        to = who;
                    }
                }
                if theme::toggle(ui, to == u32::MAX, TRADER_INTO_ARMORY).clicked() {
                    to = u32::MAX;
                }
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("trader-body")
                .max_height(520.0)
                .show(ui, |ui| {
                    for (heading, weapons) in [(TRADER_WEAPONS, true), (TRADER_ARMOUR, false)] {
                        theme::heading(ui, heading);
                        egui::Grid::new(("trader-shelf", weapons))
                            .num_columns(3)
                            .spacing([10.0, 3.0])
                            .show(ui, |ui| {
                                for (index, slot) in trader.shelf.iter().enumerate() {
                                    shelf_row(ui, world, index, *slot, weapons, to, orders);
                                }
                            });
                        ui.add_space(4.0);
                    }
                    restock_row(ui, world, orders);
                    theme::heading(ui, TRADER_RELIC);
                    relic_at_trader(ui, world, trader, local, &mut relic_to, orders, name);
                    ui.add_space(4.0);
                    theme::heading(ui, TRADER_COMBINE);
                    combine_rows(ui, world, local, orders, name);
                });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(TRADER_ARMORY_HINT)
                    .small()
                    .color(theme::MUTED),
            );
        });
    ctx.data_mut(|d| {
        d.insert_temp(id, to);
        d.insert_temp(relic_id, relic_to);
    });
}

/// *Restock Codes* (task 118): a button that rolls the shelf again, while
/// a player holds the relic — greyed once it has been pressed this visit.
fn restock_row(ui: &mut egui::Ui, world: &World, orders: &mut Vec<Order>) {
    let can = world.can_restock();
    if can == Err(world::Refusal::NoRestock) {
        return;
    }
    let button = ui
        .add_enabled(can.is_ok(), egui::Button::new(TRADER_RESTOCK))
        .on_hover_text(TRADER_RESTOCK_TIP);
    if button.clicked() {
        orders.push(Order::Restock);
    }
    ui.add_space(4.0);
}

/// The thing a shelf slot holds, as a thing: a weapon, or a whole piece.
fn shelf_thing(item: world::trader::ShelfItem) -> bims::combat::Item {
    match (item.weapon(), item.armour()) {
        (Some(weapon), _) => bims::combat::Item::Weapon(weapon),
        (None, Some(kind)) => {
            bims::combat::Item::Armour(bims::combat::Piece::new(0, kind, item.tier))
        }
        (None, None) => bims::combat::Item::Stack(item.resource as u32),
    }
}

/// One row of the shelf, if the slot belongs under this heading: the
/// thing's kind and tier (its numbers on a hover), its price, and *Buy*
/// — or the word that it is sold.
fn shelf_row(
    ui: &mut egui::Ui,
    world: &World,
    index: usize,
    slot: Option<world::trader::ShelfItem>,
    weapons: bool,
    to: u32,
    orders: &mut Vec<Order>,
) {
    // A slot sold is shown under the heading of the half of the shelf it
    // was in: the weapons come first, [`world::data::TRADER_WEAPONS`] of
    // them.
    let in_weapons = index < world::data::TRADER_WEAPONS;
    if in_weapons != weapons {
        return;
    }
    let Some(item) = slot else {
        ui.label(egui::RichText::new("—").color(theme::MUTED));
        ui.label(egui::RichText::new(TRADER_SOLD).small().color(theme::MUTED));
        ui.label("");
        ui.end_row();
        return;
    };
    let thing = shelf_thing(item);
    let what = match thing {
        bims::combat::Item::Weapon(w) => weapon_name(Some(w.kind)),
        bims::combat::Item::Armour(p) => armour_name(Some(p.kind)),
        bims::combat::Item::Stack(_) => resource_name(item.resource),
    };
    // Its numbers under its name: the tooltip's second line, a piece's
    // without the state it is in (whole, as anything bought is).
    let tip = crate::crew::tip_of(thing, 1);
    let numbers = tip
        .lines()
        .nth(1)
        .map(|l| l.split(" · ").next().unwrap_or(l).to_string())
        .unwrap_or_default();
    ui.vertical(|ui| {
        ui.set_min_width(250.0);
        ui.label(shelf_line(what, item.tier.code()))
            .on_hover_text(&tip);
        ui.label(egui::RichText::new(numbers).small().color(theme::MUTED));
    });
    let price = world.shelf_price(item);
    ui.label(
        egui::RichText::new(euros(price)).color(if price <= world.money {
            theme::INK
        } else {
            theme::WARN
        }),
    );
    if ui
        .add_enabled(price <= world.money, egui::Button::new(TRADER_BUY))
        .clicked()
    {
        orders.push(Order::BuyShelf {
            index: index as u32,
            to: (to != u32::MAX).then_some(to),
        });
    }
    ui.end_row();
}

/// The trader's relic: what it is and costs, whose Bim to propose it for,
/// and the vote on the table — the reward's own vote, the pool paying the
/// price when it carries.
fn relic_at_trader(
    ui: &mut egui::Ui,
    world: &World,
    trader: &world::trader::Trader,
    local: u32,
    to: &mut u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    let Some(relic) = trader.relic else {
        ui.label(egui::RichText::new(TRADER_NO_RELIC).color(theme::MUTED));
        return;
    };
    // With *Trade License* off it (task 118).
    let price = world.trader_relic_price(relic);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(relic_name(relic)).strong());
        ui.label(
            egui::RichText::new(euros(price)).color(if price <= world.money {
                theme::MUTED
            } else {
                theme::WARN
            }),
        );
    });
    ui.add(egui::Label::new(egui::RichText::new(relic_line(relic)).small()).wrap());
    ui.add(
        egui::Label::new(
            egui::RichText::new(TRADER_RELIC_INTRO)
                .small()
                .color(theme::MUTED),
        )
        .wrap(),
    );
    if *to >= world.players() {
        *to = local;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(FOR_BIM).color(theme::MUTED));
        for slot in 0..world.players() {
            if theme::toggle(ui, *to == slot, name(slot)).clicked() {
                *to = slot;
            }
        }
        if ui.button(TRADER_PROPOSE).clicked() {
            orders.push(Order::ProposeRelic {
                relic: Some(relic),
                to: *to,
            });
        }
    });
    let Some(p) = world.trade_relic() else {
        return;
    };
    ui.label(egui::RichText::new(relic_proposal_line(p.relic, p.to)).strong());
    ui.horizontal_wrapped(|ui| {
        for (slot, &yes) in p.accepted.iter().enumerate() {
            let slot = slot as u32;
            ui.label(
                egui::RichText::new(relic_answer(
                    &name(slot),
                    yes,
                    !world.run.is_connected(slot),
                ))
                .small()
                .color(if yes { theme::ACCENT } else { theme::MUTED }),
            );
        }
    });
    let mine = p.accepted.get(local as usize).copied().unwrap_or(false);
    ui.horizontal(|ui| {
        if ui.add_enabled(!mine, egui::Button::new(ACCEPT)).clicked() {
            orders.push(Order::AcceptRelic(true));
        }
        if ui.add_enabled(mine, egui::Button::new(TAKE_BACK)).clicked() {
            orders.push(Order::AcceptRelic(false));
        }
        if ui.button(TRADER_WITHDRAW).clicked() {
            orders.push(Order::ProposeRelic {
                relic: None,
                to: p.to,
            });
        }
    });
}

/// Every pair the player may combine: two of one kind at one tier under
/// three, out of the armory or off its own Bim or a bot, a row a pair —
/// a worn one first, so the result is worn in its place.
fn combine_rows(
    ui: &mut egui::Ui,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(TRADER_COMBINE_INTRO)
                .small()
                .color(theme::MUTED),
        )
        .wrap(),
    );
    // Every thing the player may use, with where it is: the worn first.
    let mut things: Vec<(world::GearSource, bims::combat::Item, Option<u32>)> = Vec::new();
    for who in 0..world.aboard.crew_count() {
        if !world.may_change(local, who) {
            continue;
        }
        for part in world::GearSlot::ALL {
            if let Some(item) = world.worn_on(who, part) {
                things.push((world::GearSource::Worn { who, slot: part }, item, Some(who)));
            }
        }
    }
    for stored in &world.holdings.armory {
        things.push((
            world::GearSource::Armory { id: stored.id },
            stored.item,
            None,
        ));
    }
    let key = |item: bims::combat::Item| match item {
        bims::combat::Item::Weapon(w) => Some((0u32, w.kind.code(), w.tier.code())),
        bims::combat::Item::Armour(p) => Some((1u32, p.kind.code(), p.tier.code())),
        bims::combat::Item::Stack(_) => None,
    };
    let mut seen: Vec<(u32, u32, u32)> = Vec::new();
    let mut any = false;
    for (i, &(a, first, worn)) in things.iter().enumerate() {
        let Some(k) = key(first) else {
            continue;
        };
        if k.2 >= 3 || seen.contains(&k) {
            continue;
        }
        let Some(&(b, _, _)) = things[i + 1..].iter().find(|t| key(t.1) == Some(k)) else {
            continue;
        };
        seen.push(k);
        any = true;
        let what = match first {
            bims::combat::Item::Weapon(w) => weapon_name(Some(w.kind)),
            bims::combat::Item::Armour(p) => armour_name(Some(p.kind)),
            bims::combat::Item::Stack(_) => "",
        };
        let from = combine_from(worn.map(name).as_deref());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(combine_line(what, k.2, &from)).small());
            let fee = world::data::COMBINE_FEE;
            if ui
                .add_enabled(fee <= world.money, egui::Button::new(TRADER_COMBINE))
                .clicked()
            {
                orders.push(Order::Combine { a, b });
            }
        });
    }
    if !any {
        ui.label(
            egui::RichText::new(TRADER_COMBINE_NONE)
                .small()
                .color(theme::MUTED),
        );
    }
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
    /// ship* and are aboard. A player downed is in neither number, since
    /// the ship does not wait for somebody who cannot walk in.
    #[test]
    fn returning_counts_the_players_the_departure_check_waits_for() {
        let mut world = simulation_world(flyer(3), REFERENCE_MONEY, 3);
        assert_eq!(world.returning_count(), (0, 3));
        assert_eq!(returning_text(&world), "Returning · 0 / 3");
        world.apply_now(Command::Return { slot: 0 });
        assert_eq!(returning_text(&world), "Returning · 1 / 3");
        // Player 2 downed — the bar taken to nothing, and a step for the
        // body to go down — is no longer waited for, so the count
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

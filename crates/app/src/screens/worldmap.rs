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
use crate::net::{Choice, Spot, TradeLine};
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
    /// Every place on the floor as the chart marks it (October 2026,
    /// `World::floor_marks`), worked out with the list.
    pub marks: Vec<world::FloorMark>,
    /// The same as a star's one mission, for the word on the place picked.
    pub missions: Vec<world::run::StarMission>,
    /// The site the player has picked, off the list or the galaxy chart:
    /// looking, not a vote. [`Order::Propose`] is the vote.
    pub picked: Option<Site>,
    /// The site the pointer rests on, a row of the list or a star on the
    /// chart: what the card shows while it does (feature 107). The
    /// screen and the column set it afresh every frame.
    pub hovered: Option<Site>,
    /// The pick came from the galaxy chart ([`WorldMap::pick_star`]): the
    /// list scrolls its row into view the next frame, once.
    scroll_to_pick: bool,
    /// The list sorted by how long each trip is rather than grouped by
    /// system (task 135): the viewer's choice, kept while the map is.
    pub by_distance: bool,
    /// The column popped out from the right edge; retracted to begin
    /// with, so the chart has the width.
    pub column_open: bool,
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
    /// How much of the canvas's right the column takes: nought while it
    /// is retracted, when only its tab lies over the system view.
    pub fn column_w(&self) -> f32 {
        if self.column_open { COLUMN_W } else { 0.0 }
    }

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
                let floor_day = world
                    .floor()
                    .and_then(|f| f.find(site.star))
                    .map(|(row, _)| world::floor::row_day(row));
                let title = if site.star == world.star_id {
                    MAP_THIS_SYSTEM.to_string()
                } else if let Some(day) = floor_day {
                    // A way up the floor (October 2026): the star and its row's day.
                    map_floor_next(&star, day)
                } else {
                    // One or two lanes off (the second map rework).
                    let hops = world
                        .trip_route_in(&galaxy, site.star)
                        .map_or(1, |(route, _)| route.len() as u32 - 1);
                    map_next_system(&star, hops)
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
        self.marks = world.floor_marks();
        self.missions = self
            .marks
            .iter()
            .map(|m| world::run::StarMission {
                site: m.site,
                kind: m.kind,
                cleared: m.cleared,
                area: m.area,
            })
            .collect();
    }

    /// The site a star is gone to by, off the galaxy chart (the galaxy-only
    /// map): its one place on the list — a system offers one, the
    /// Heart's fortress at the machines' origin and a fallen trader
    /// included. `None` for a star with no place on the list (the ship's
    /// own is on it; one more than two lanes off is not).
    pub fn star_site(&self, star: u32) -> Option<Site> {
        self.groups
            .iter()
            .flat_map(|g| g.destinations.iter())
            .find(|d| d.site.star == star)
            .map(|d| d.site)
    }

    /// A star picked on the galaxy chart: the site it is gone to by
    /// ([`WorldMap::star_site`]) picked on the list, so the card and the
    /// bar offer the trip. A star with no place on the list leaves nothing
    /// picked, and the chart's panel says why.
    pub fn pick_star(&mut self, star: u32) {
        self.picked = self.star_site(star);
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
    if quote.elite {
        words.push(ARRIVE_ELITE.into());
    }
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
/// in its colour (task 111) — a mission's with whether it is on a station
/// or a planet — then the rest in the row's own colour.
fn row_job(
    style: &egui::Style,
    d: &Destination,
    text: &str,
    colour: egui::Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let small = egui::TextStyle::Small.resolve(style);
    if let Some(kind) = d.kind {
        job.append(
            site_kind_word(kind),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(small.size + 1.0, egui::FontFamily::Proportional),
                color: theme::site_kind_colour(kind),
                ..Default::default()
            },
        );
        if kind != SiteKind::Trader {
            job.append(
                &format!(" {}", site_place_word(d.site.station)),
                0.0,
                egui::TextFormat {
                    font_id: small.clone(),
                    color: theme::site_kind_colour(kind),
                    ..Default::default()
                },
            );
        }
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
    /// A place picked on the list (the map rework): its star, to be picked on
    /// the chart and shown in the system view.
    pub show: Option<u32>,
}

/// The world map's column (feature 107), down the right of the canvas
/// while the map is up: the day and the pool, the buyback queue, every
/// destination with its quote, and the card for the one looked at — the
/// one the pointer rests on, else the one picked, else the one on the
/// table — with the vote on it. The charts are the canvas to its left:
/// the galaxy and the system side by side (task 135). `name` names a
/// player's slot.
pub fn map_column(
    ctx: &egui::Context,
    canvas: egui::Rect,
    map: &mut WorldMap,
    world: &World,
    local: u32,
    name: &dyn Fn(u32) -> String,
) -> ColumnAsk {
    map.refresh(world);
    let mut ask = ColumnAsk::default();
    let between = !world.in_mission();
    let here = world.current_site();
    // Hovered is this frame's: the rows below and the chart under the
    // pointer set it afresh.
    let hovered_before = map.hovered.take();
    // Retracted: only its tab on the right edge, the way out and the
    // trader's beside it.
    if !map.column_open {
        egui::Area::new(egui::Id::new("hud-map-tab"))
            .fixed_pos(egui::pos2(
                canvas.max.x - crate::screens::hud::MARGIN,
                canvas.min.y + crate::screens::hud::MARGIN,
            ))
            .pivot(egui::Align2::RIGHT_TOP)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                theme::tray_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if !between && ui.button(MAP_CLOSE).clicked() {
                            ask.close = true;
                        }
                        trader_reopen(ui, world, local);
                        let tab = egui::Button::new(
                            egui::RichText::new(MAP_COLUMN_OPEN).strong().size(15.0),
                        );
                        if ui.add(tab).on_hover_text(MAP_COLUMN_OPEN_TIP).clicked() {
                            map.column_open = true;
                        }
                    });
                });
            });
        return ask;
    }
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
                        if ui
                            .button(egui::RichText::new(MAP_COLUMN_SHUT).strong())
                            .on_hover_text(MAP_COLUMN_SHUT_TIP)
                            .clicked()
                        {
                            map.column_open = false;
                        }
                        if !between && ui.button(MAP_CLOSE).clicked() {
                            ask.close = true;
                        }
                        trader_reopen(ui, world, local);
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
                            egui::RichText::new(euros(world.share_of(local)))
                                .strong()
                                .color(theme::ACCENT),
                        );
                        ui.end_row();
                    });
                buyback_queue(ui, world, name);
                if world.run.is_out(local) {
                    ui.label(
                        egui::RichText::new(out_line(world.rewards().buyback))
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
                            let job = row_job(ui.style(), d, &text, colour);
                            let row = ui.selectable_label(map.picked == Some(d.site), job);
                            if row.hovered() {
                                map.hovered = Some(d.site);
                            }
                            if row.clicked() {
                                map.picked = Some(d.site);
                                ask.show = Some(d.site.star);
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
                    destination_card(ui, map, world, site, name);
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
            let mut line = format!("{}  {}", d.name, trip_quote(q.minutes, q.arrival_date));
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
/// each costs and whether the crew's money between them covers it by
/// the time their turn comes — the order they are paid for in, a fallen
/// player's own money first and everybody's pooled after
/// (`World::buy_back`).
fn buyback_queue(ui: &mut egui::Ui, world: &World, name: &dyn Fn(u32) -> String) {
    if world.run.fallen.is_empty() {
        return;
    }
    ui.add_space(4.0);
    theme::heading(ui, BUYBACK_HEADING);
    let mut left = world.crew_money();
    let cost = world.rewards().buyback;
    egui::Grid::new("map-buyback")
        .num_columns(3)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for fallen in &world.run.fallen {
                let covered = left >= cost;
                if covered {
                    left -= cost;
                }
                ui.label(name(fallen.slot));
                ui.label(euros(cost));
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
/// the day of arrival, what is there on it, and who put it on the table
/// if it is there. It only reads: the vote is the bar's at the foot of
/// the map ([`propose_bar`], the second map rework).
fn destination_card(
    ui: &mut egui::Ui,
    map: &WorldMap,
    world: &World,
    site: Site,
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
            if kind != SiteKind::Trader {
                ui.label(
                    egui::RichText::new(site_place_word(site.station))
                        .size(15.0)
                        .color(theme::site_kind_colour(kind)),
                );
            }
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
            row(ui, CARD_HOPS, hops_words(quote.hops));
            row(ui, CARD_TRAVEL, days_words(quote.days));
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
    // The trip on the table says so; the vote itself is the bar's at the
    // foot of the map (the second map rework).
    if let Some(proposal) = world.run.proposal.as_ref().filter(|p| p.site == site) {
        ui.label(egui::RichText::new(proposed_by(&name(proposal.by))).color(theme::ACCENT));
    }
}

/// Whether the map's bar puts trips to the crew now: between missions,
/// with no relic being chosen. Then the Propose key is the bar's, and
/// the reload it shares Space with is not read.
pub fn proposing(world: &World) -> bool {
    !world.in_mission() && world.relic_choice().is_none()
}

/// How wide the bar at the foot of the map is, in points.
const BAR_W: f32 = 460.0;

/// The bar at the foot of the map (the second map rework), centred on `centre_x` with
/// its foot at `bottom`, between missions: the trip on the table — who
/// put it, every player's answer and this player's two buttons — and the
/// trip picked on the chart or the list with *Propose*, which is the one
/// place a trip is put to the crew; the card in the column only reads.
/// Nothing during a mission, where the map is to be looked at. The
/// rectangle it took, for the log to stand above it.
///
/// `hotkey` is the Propose key (`keys::Action::Propose`, Space; October
/// 2026) — whether it went down this frame, and its name: it presses
/// *Propose* with a place picked that is not the one on the table, and
/// *Accept* otherwise, and the button it would press says so.
#[allow(clippy::too_many_arguments)]
pub fn propose_bar(
    ctx: &egui::Context,
    centre_x: f32,
    bottom: f32,
    map: &WorldMap,
    world: &World,
    local: u32,
    hotkey: (bool, &str),
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) -> Option<egui::Rect> {
    if !proposing(world) {
        return None;
    }
    let proposal = world.run.proposal.as_ref();
    let picked = map
        .picked
        .and_then(|site| map.find(site))
        .filter(|d| proposal.is_none_or(|p| p.site != d.site));
    let (pressed, key) = hotkey;
    let mine = proposal.is_some_and(|p| p.accepted.get(local as usize).copied().unwrap_or(false));
    // What the key presses: the pick put to the crew, else a yes.
    let key_proposes = picked.is_some_and(|d| d.quote.is_ok());
    let key_accepts = !key_proposes && proposal.is_some() && !mine;
    if pressed && key_proposes {
        let d = picked.unwrap();
        orders.push(Order::Propose {
            star: d.site.star,
            station: d.site.station,
        });
    } else if pressed && key_accepts {
        orders.push(Order::AcceptTrip(true));
    }
    let area = egui::Area::new(egui::Id::new("map-propose-bar"))
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(centre_x, bottom))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::tray_frame().inner_margin(10.0).show(ui, |ui| {
                ui.set_width(BAR_W);
                ui.vertical_centered(|ui| {
                    if let Some(p) = proposal {
                        let site = map.find(p.site).map(|d| d.name.clone()).unwrap_or_default();
                        ui.label(
                            egui::RichText::new(on_the_table(&name(p.by), &site))
                                .strong()
                                .color(theme::ACCENT),
                        );
                        ui.horizontal_wrapped(|ui| {
                            for (slot, &yes) in p.accepted.iter().enumerate() {
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
                        ui.horizontal(|ui| {
                            let size = egui::vec2(140.0, 30.0);
                            let word = if key_accepts {
                                with_key(ACCEPT_TRIP, key)
                            } else {
                                ACCEPT_TRIP.to_owned()
                            };
                            let accept = egui::Button::new(egui::RichText::new(word).strong())
                                .min_size(size);
                            if ui.add_enabled(!mine, accept).clicked() {
                                orders.push(Order::AcceptTrip(true));
                            }
                            if ui
                                .add_enabled(mine, egui::Button::new(TAKE_BACK).min_size(size))
                                .clicked()
                            {
                                orders.push(Order::AcceptTrip(false));
                            }
                        });
                    }
                    if proposal.is_some() && picked.is_some() {
                        ui.separator();
                    }
                    match picked {
                        Some(d) => {
                            let at = world.current_site() == Some(d.site);
                            let (text, colour) = row_words(d, at);
                            ui.label(row_job(ui.style(), d, &text, colour));
                            let press = ui
                                .add_enabled(
                                    d.quote.is_ok(),
                                    egui::Button::new(
                                        egui::RichText::new(if key_proposes {
                                            with_key(&propose_trip(&d.name), key)
                                        } else {
                                            propose_trip(&d.name)
                                        })
                                        .strong()
                                        .size(16.0),
                                    )
                                    .min_size(egui::vec2(240.0, 34.0)),
                                )
                                .on_hover_text(PROPOSE_TIP);
                            if press.clicked() {
                                orders.push(Order::Propose {
                                    star: d.site.star,
                                    station: d.site.station,
                                });
                            }
                        }
                        None if proposal.is_none() => {
                            ui.label(
                                egui::RichText::new(PROPOSE_NOTHING)
                                    .small()
                                    .color(theme::MUTED),
                            );
                            ui.add_enabled(
                                false,
                                egui::Button::new(egui::RichText::new(PROPOSE).strong().size(16.0))
                                    .min_size(egui::vec2(240.0, 34.0)),
                            );
                        }
                        None => {}
                    }
                });
            });
        });
    Some(area.response.rect)
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
                            buyback_cost(world.rewards().buyback)
                        } else {
                            BOT_GONE_WORD.to_owned()
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
/// (`World::awaiting_ready`), Dota's ready check in the middle of the
/// screen — the site's kind over a card a player, each lit in its
/// player's colour with a tick once that player is ready, a bar of how
/// many are, and this player's button. The deck is not drawn behind it
/// (the game screen's `veiled`), only the site's backdrop. Not a modal:
/// the loadouts and the skill points stay in reach, and the world hears
/// them while it waits.
///
/// `hotkey` is the Propose key (Space; October 2026) — whether it went
/// down this frame, and its name: it presses this player's button, and
/// the button says so.
pub fn ready_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
    colour: &dyn Fn(u32) -> egui::Color32,
    hotkey: (bool, &str),
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
    let players = world.players();
    let (ready, of) = world.ready_count();
    let time = ctx.input(|i| i.time) as f32;
    // A waiting card breathes, so the eye goes to whoever is holding the
    // rest up.
    ctx.request_repaint();
    let width = (players as f32 * (READY_CARD.x + READY_GAP) + 40.0).max(440.0);
    egui::Area::new(egui::Id::new("ready-check"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -60.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            theme::tray_frame().inner_margin(18.0).show(ui, |ui| {
                ui.set_width(width);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(READY_CHECK.to_uppercase())
                            .small()
                            .strong()
                            .color(theme::MUTED),
                    );
                    ui.label(
                        egui::RichText::new(ready_title(kind))
                            .strong()
                            .size(24.0)
                            .color(theme::site_kind_colour(kind)),
                    );
                    ui.add(
                        egui::Label::new(egui::RichText::new(READY_LINE).color(theme::MUTED))
                            .wrap(),
                    );
                    ui.add_space(12.0);
                    // The cards, one a player, in a row in the middle.
                    let row = players as f32 * (READY_CARD.x + READY_GAP) - READY_GAP;
                    let (strip, _) =
                        ui.allocate_exact_size(egui::vec2(row, READY_CARD.y), egui::Sense::hover());
                    let painter = ui.painter();
                    for slot in 0..players {
                        let at =
                            strip.min + egui::vec2(slot as f32 * (READY_CARD.x + READY_GAP), 0.0);
                        ready_card(
                            painter,
                            egui::Rect::from_min_size(at, READY_CARD),
                            &name(slot),
                            colour(slot),
                            world.run.is_ready(slot),
                            !world.run.is_connected(slot),
                            slot == local,
                            time + slot as f32 * 0.7,
                        );
                    }
                    ui.add_space(10.0);
                    // How many are ready, as a bar of a segment each.
                    let (bar, _) = ui
                        .allocate_exact_size(egui::vec2(row.max(200.0), 8.0), egui::Sense::hover());
                    let segment = (bar.width() - (of.max(1) - 1) as f32 * 4.0) / of.max(1) as f32;
                    for i in 0..of.max(1) {
                        let x = bar.min.x + i as f32 * (segment + 4.0);
                        let piece = egui::Rect::from_min_size(
                            egui::pos2(x, bar.min.y),
                            egui::vec2(segment, bar.height()),
                        );
                        let lit = i < ready;
                        painter_fill(
                            ui.painter(),
                            piece,
                            if lit { theme::ACCENT } else { theme::LINE },
                        );
                    }
                    ui.label(egui::RichText::new(ready_count(ready, of)).small().color(
                        if ready == of {
                            theme::ACCENT
                        } else {
                            theme::MUTED
                        },
                    ));
                    ui.add_space(10.0);
                    // The bonus wave (October 2026): chosen here, before
                    // the fight, by any player; a change takes back every
                    // *Ready*.
                    if world.can_choose_bonus_wave(local).is_ok() {
                        let on = world.bonus_wave_here().chosen();
                        let (text, ink, fill, rim) = if on {
                            (
                                BONUS_WAVE_CHOSEN,
                                theme::CAUTION,
                                theme::CAUTION.gamma_multiply(0.18),
                                theme::CAUTION,
                            )
                        } else {
                            (BONUS_WAVE_NOT_CHOSEN, theme::INK, theme::RAISED, theme::LINE)
                        };
                        let press = ui
                            .add(
                                egui::Button::new(egui::RichText::new(text).strong().color(ink))
                                    .fill(fill)
                                    .stroke(egui::Stroke::new(1.0, rim))
                                    .min_size(egui::vec2(220.0, 30.0)),
                            )
                            .on_hover_text(BONUS_WAVE_TIP);
                        if press.clicked() {
                            orders.push(Order::BonusWave(!on));
                        }
                        ui.add_space(8.0);
                    }
                    let (word, yes, fill, ink) = if mine {
                        (READY_NO, false, theme::RAISED, theme::MUTED)
                    } else {
                        (READY_YES, true, READY_GREEN, egui::Color32::WHITE)
                    };
                    let (pressed, key) = hotkey;
                    let press = ui.add(
                        egui::Button::new(
                            egui::RichText::new(with_key(word, key).to_uppercase())
                                .strong()
                                .size(18.0)
                                .color(ink),
                        )
                        .fill(fill)
                        .min_size(egui::vec2(220.0, 42.0)),
                    );
                    if press.clicked() || pressed {
                        orders.push(Order::Ready(yes));
                    }
                });
            });
        });
}

/// A player's card in the ready check, and the gap between two.
const READY_CARD: egui::Vec2 = egui::vec2(96.0, 118.0);
const READY_GAP: f32 = 12.0;
/// The ready check's green: the button to press, and a card that is in.
const READY_GREEN: egui::Color32 = egui::Color32::from_rgb(0x2f, 0x9e, 0x5b);

fn painter_fill(painter: &egui::Painter, rect: egui::Rect, colour: egui::Color32) {
    painter.rect_filled(rect, 2.0, colour);
}

/// One player's card: a disc in its colour with its initial, ringed green
/// and ticked once it is ready, breathing grey while it is waited for,
/// dimmed and crossed once it has gone; its name and its state under it.
/// The tick and the cross are drawn, not glyphs (the default font has
/// neither).
#[allow(clippy::too_many_arguments)]
fn ready_card(
    painter: &egui::Painter,
    card: egui::Rect,
    name: &str,
    colour: egui::Color32,
    ready: bool,
    gone: bool,
    mine: bool,
    time: f32,
) {
    let breathe = 0.5 + 0.5 * (time * 3.0).sin();
    let (rim, fill) = if gone {
        (theme::LINE, theme::PANEL_DEEP)
    } else if ready {
        (READY_GREEN, READY_GREEN.gamma_multiply(0.18))
    } else {
        (
            theme::MUTED.gamma_multiply(0.45 + 0.4 * breathe),
            theme::PANEL_DEEP,
        )
    };
    painter.rect_filled(card, 6.0, fill);
    painter.rect_stroke(
        card,
        6.0,
        egui::Stroke::new(if mine { 2.5 } else { 1.5 }, rim),
        egui::StrokeKind::Inside,
    );
    let middle = egui::pos2(card.center().x, card.min.y + 42.0);
    let disc = if gone {
        colour.gamma_multiply(0.3)
    } else {
        colour
    };
    painter.circle_filled(middle, 26.0, theme::PANEL_DEEP);
    painter.circle_filled(middle, 23.0, disc);
    let initial: String = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    painter.text(
        middle,
        egui::Align2::CENTER_CENTER,
        initial,
        egui::FontId::proportional(22.0),
        theme::PANEL_DEEP,
    );
    let ring = if ready && !gone { READY_GREEN } else { rim };
    painter.circle_stroke(middle, 27.0, egui::Stroke::new(3.0, ring));
    // The badge on the disc's shoulder.
    let badge = middle + egui::vec2(20.0, 18.0);
    if gone {
        painter.circle_filled(badge, 10.0, theme::PANEL_DEEP);
        let d = 4.5;
        for (a, b) in [((-d, -d), (d, d)), ((-d, d), (d, -d))] {
            painter.line_segment(
                [badge + egui::vec2(a.0, a.1), badge + egui::vec2(b.0, b.1)],
                egui::Stroke::new(2.0, theme::MUTED),
            );
        }
    } else if ready {
        painter.circle_filled(badge, 11.0, READY_GREEN);
        painter.line_segment(
            [badge + egui::vec2(-5.0, 0.0), badge + egui::vec2(-1.5, 4.0)],
            egui::Stroke::new(2.6, egui::Color32::WHITE),
        );
        painter.line_segment(
            [badge + egui::vec2(-1.5, 4.0), badge + egui::vec2(5.5, -4.5)],
            egui::Stroke::new(2.6, egui::Color32::WHITE),
        );
    } else {
        // Three dots taking turns: still deciding.
        painter.circle_filled(badge, 10.0, theme::PANEL_DEEP);
        for i in 0..3 {
            let lit = ((time * 2.5) as i32).rem_euclid(3) == i;
            painter.circle_filled(
                badge + egui::vec2((i - 1) as f32 * 5.0, 0.0),
                1.8,
                if lit {
                    theme::INK
                } else {
                    theme::MUTED.gamma_multiply(0.6)
                },
            );
        }
    }
    let mut short: String = name.chars().take(12).collect();
    if name.chars().count() > 12 {
        short.push('.');
    }
    painter.text(
        egui::pos2(card.center().x, card.max.y - 32.0),
        egui::Align2::CENTER_CENTER,
        short,
        egui::FontId::proportional(14.0),
        if gone { theme::MUTED } else { theme::INK },
    );
    painter.text(
        egui::pos2(card.center().x, card.max.y - 14.0),
        egui::Align2::CENTER_CENTER,
        ready_state(ready, gone).to_uppercase(),
        egui::FontId::proportional(11.0),
        if gone {
            theme::MUTED
        } else if ready {
            theme::ACCENT
        } else {
            theme::MUTED
        },
    );
}

/// The other players as the windows over the map show them: where each
/// one's pointer is, with the name it is labelled by, and what each has
/// their eye on — both in their colours. Their pointer over the trader's
/// window or the relic choice is drawn over this player's own copy of
/// it, and the line or relic each has their eye on is outlined.
#[derive(Default)]
pub struct Mates {
    pub pointers: Vec<(egui::Color32, String, Spot)>,
    pub choices: Vec<(egui::Color32, Choice)>,
}

/// Where the trader's window and the relic choice stood the last frame
/// they were laid out, for the pointer to be read against before this
/// frame lays them out again: `None` for one not up then.
pub fn trader_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    shown_rect(ctx, "trader-window-rect")
}

pub fn relic_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    shown_rect(ctx, "relic-window-rect")
}

fn shown_rect(ctx: &egui::Context, key: &str) -> Option<egui::Rect> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(egui::Rect, u64)>(egui::Id::new(key)))
        .filter(|&(_, pass)| pass + 1 >= now)
        .map(|(rect, _)| rect)
}

/// A window laid out this frame at `rect`: kept for [`shown_rect`], and
/// the others' pointers over it drawn over it, above every window.
fn window_shown(ctx: &egui::Context, key: &str, rect: egui::Rect, mates: &Mates, relic: bool) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(key), (rect, pass)));
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("mates-over-windows"),
    ));
    for (colour, name, spot) in &mates.pointers {
        let (x, y) = match (*spot, relic) {
            (Spot::Trader(x, y), false) | (Spot::Relic(x, y), true) => (x, y),
            _ => continue,
        };
        theme::ghost_pointer(&painter, rect.min + egui::vec2(x, y), *colour, name);
    }
}

/// Outlines round `rect`, one a colour, each inside the last: the
/// players who have their eye on what it holds.
fn mark_chosen(painter: &egui::Painter, rect: egui::Rect, colours: &[egui::Color32]) {
    for (i, &colour) in colours.iter().enumerate() {
        painter.rect_stroke(
            rect.shrink(1.0 + 3.0 * i as f32),
            3.0,
            egui::Stroke::new(2.0, colour),
            egui::StrokeKind::Inside,
        );
    }
}

/// What this player has picked in the relic window and not yet proposed:
/// a relic's code or `u32::MAX` for none. The window's own, kept in
/// egui's memory between frames.
#[derive(Clone, Copy, PartialEq, Debug)]
struct RelicPick {
    relic: Option<u32>,
}

/// The side of a relic's picture on the choice, in points: bigger than the
/// sheet's (`icons::RELIC`), since here it is the thing chosen.
const RELIC_PLATE: f32 = 44.0;

/// The colour of a relic's boon, and of its price.
const BOON: egui::Color32 = egui::Color32::from_rgb(120, 200, 120);
const PRICE: egui::Color32 = egui::Color32::from_rgb(225, 110, 100);

/// A relic's lines under its name: each boon in green and its price in
/// red, so what it gives and what it costs read apart at a glance.
pub fn relic_lines_ui(ui: &mut egui::Ui, relic: world::Relic) {
    for (line, good) in relic_lines(relic) {
        let colour = if good { BOON } else { PRICE };
        ui.add(egui::Label::new(egui::RichText::new(line).small().color(colour)).wrap());
    }
}

/// The relic choice (feature 106, October 2026): the reward screen after
/// an elite's site cleared, a modal over a dimmed canvas, since the map
/// waits on it. The crew choose **one relic together, or none** — the
/// vote the map's trips are chosen by: a player picks and proposes, every
/// connected player says yes, and a new proposal clears every yes. Every
/// other player's pick is outlined in its colour.
pub fn relic_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
    mates: &Mates,
    mine: &mut Choice,
) {
    mine.relic = None;
    let Some(choice) = world.relic_choice() else {
        return;
    };
    // Who else has their eye on which relic, in their colours.
    let eyed = |code: u32| -> Vec<egui::Color32> {
        mates
            .choices
            .iter()
            .filter(|(_, c)| c.relic == Some(code))
            .map(|(colour, _)| *colour)
            .collect()
    };
    let id = egui::Id::new("relic-pick");
    let mut pick = ctx
        .data(|d| d.get_temp::<RelicPick>(id))
        .unwrap_or(RelicPick { relic: None });
    // A pick no longer on offer (another choice since) is forgotten.
    if pick
        .relic
        .is_some_and(|c| c != u32::MAX && !choice.options.iter().any(|r| r.code() == c))
    {
        pick.relic = None;
    }
    let shown = egui::Modal::new(egui::Id::new("relic-reward"))
        .backdrop_color(egui::Color32::from_black_alpha(150))
        .frame(theme::tray_frame().inner_margin(14.0))
        .show(ctx, |ui| {
            ui.set_width(440.0);
            // No words that explain on the choice itself: they are the
            // title's hover.
            ui.label(egui::RichText::new(REWARD_TITLE).strong().size(18.0))
                .on_hover_text(REWARD_INTRO);
            ui.add_space(6.0);
            // Each relic its picture, then its name to pick it by and its
            // boons and price; a click on the picture picks it too.
            for &relic in &choice.options {
                let on = pick.relic == Some(relic.code());
                let row = ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    let (rect, plate) = ui.allocate_exact_size(
                        egui::vec2(RELIC_PLATE, RELIC_PLATE),
                        egui::Sense::click(),
                    );
                    crate::icons::relic(ui.painter(), rect, relic);
                    if plate.clicked() {
                        pick.relic = Some(relic.code());
                    }
                    ui.vertical(|ui| {
                        let text = egui::RichText::new(relic_name(relic)).strong();
                        if theme::toggle(ui, on, text).clicked() {
                            pick.relic = Some(relic.code());
                        }
                        relic_lines_ui(ui, relic);
                    });
                });
                mark_chosen(
                    ui.painter(),
                    row.response.rect.expand(3.0),
                    &eyed(relic.code()),
                );
                ui.add_space(4.0);
            }
            let none = theme::toggle(ui, pick.relic == Some(u32::MAX), TAKE_NONE);
            if none.clicked() {
                pick.relic = Some(u32::MAX);
            }
            mark_chosen(ui.painter(), none.rect.expand(3.0), &eyed(u32::MAX));
            ui.add_space(6.0);
            if ui
                .add_enabled(pick.relic.is_some(), egui::Button::new(PROPOSE))
                .clicked()
                && let Some(code) = pick.relic
            {
                orders.push(Order::ProposeRelic {
                    relic: world::Relic::from_code(code),
                });
            }
            // What is on the table, and who has said yes to it.
            if let Some(p) = &choice.proposal {
                ui.separator();
                ui.label(egui::RichText::new(relic_proposal_line(p.relic)).strong());
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
        });
    window_shown(ctx, "relic-window-rect", shown.response.rect, mates, true);
    mine.relic = pick.relic;
    ctx.data_mut(|d| d.insert_temp(id, pick));
}

/// The Trader panel (task 114), while the crew are at a trader: the whole
/// visit on the map, drawn as a purchase order. A header band with the
/// form's number, *Deliver to* (the player's own Bim, a bot it may change,
/// or the armory), then one line item a thing — its icon in its tier's
/// cell, its name, its tier as pips, a dotted leader out to its price and
/// *Buy* — for the weapons, the armour and the items (an item the Bim
/// carries offered as its *Upgrade*), a *Sell* tab beside it of what the
/// player may sell back, and its balance as the total at the foot.
/// The words that explain are hovers: a thing's numbers on its line, a
/// section's rules on its "?". A window of its own, opened in the middle of
/// the screen, that may be moved, so the Armory panel (Tab) can be up
/// beside it. Nothing here decides anything: every press is an
/// [`Order`] the world may refuse, and the refusal is the log's line.
pub fn trader_window(
    ctx: &egui::Context,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
    mates: &Mates,
    mine: &mut Choice,
) {
    mine.line = None;
    let Some(trader) = world.trader_here(local) else {
        return;
    };
    if trader_shut(ctx, trader) {
        return;
    }
    // Whom a purchase is for, kept between frames: a crew index, or the
    // armory as `u32::MAX`. The player's own Bim to begin with.
    let id = egui::Id::new("trader-for");
    let mut to = ctx.data(|d| d.get_temp::<u32>(id)).unwrap_or(local);
    // Which tab is up, kept between frames: Buy to begin with.
    let tab = egui::Id::new("trader-selling");
    let mut selling = ctx.data(|d| d.get_temp::<bool>(tab)).unwrap_or(false);
    let crew = world.aboard.crew_count();
    if to != u32::MAX && !(to < crew && world.may_change(local, to)) {
        to = local;
    }
    // The lines the others have their eye on, for `line_item` to outline,
    // and the one this player's pointer is on, which it notes.
    let marks: Vec<(TradeLine, egui::Color32)> = mates
        .choices
        .iter()
        .filter_map(|(colour, c)| c.line.map(|line| (line, *colour)))
        .collect();
    ctx.data_mut(|d| {
        d.insert_temp(line_marks_id(), marks);
        d.insert_temp(line_eyed_id(), None::<TradeLine>);
    });
    let shown = egui::Window::new(TRADER_TITLE)
        .id(egui::Id::new("trader-window"))
        .title_bar(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .collapsible(false)
        .resizable(false)
        .frame(form_frame())
        .show(ctx, |ui| {
            ui.set_width(FORM_WIDTH);
            if form_header(ui, world, trader) {
                let at = (trader.site.star, trader.site.station);
                ui.ctx().data_mut(|d| d.insert_temp(trader_shut_id(), at));
            }
            ui.add_space(6.0);
            // The two tabs, Sell to the right of Buy (October 2026).
            ui.horizontal(|ui| {
                if theme::toggle(ui, !selling, TRADER_TAB_BUY).clicked() {
                    selling = false;
                }
                if theme::toggle(ui, selling, TRADER_TAB_SELL).clicked() {
                    selling = true;
                }
            });
            ui.add_space(4.0);
            if !selling {
                // Deliver to: the player's own Bim, every bot, or the armory.
                ui.horizontal_wrapped(|ui| {
                    theme::asks(ui, TRADER_DELIVER_TO, TRADER_INTRO);
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
                ui.add_space(2.0);
            }
            perforation(ui);
            egui::ScrollArea::vertical()
                .id_salt("trader-body")
                .max_height(500.0)
                .show(ui, |ui| {
                    // Both tabs the same height, so the window (pinned at
                    // its middle) does not move the tabs under the pointer.
                    ui.set_min_height(500.0);
                    if selling {
                        form_heading(ui, TRADER_SELL_HEADING, Some(TRADER_SELL_INTRO));
                        sell_rows(ui, world, local, orders, name);
                        return;
                    }
                    for (heading, weapons) in [(TRADER_WEAPONS, true), (TRADER_ARMOUR, false)] {
                        form_heading(ui, heading, None);
                        let mut row = 0;
                        for (index, slot) in trader.shelf.iter().enumerate() {
                            // A slot sold is shown under the heading of the
                            // half of the shelf it was in: the weapons come
                            // first, [`world::data::TRADER_WEAPONS`] of them.
                            if (index < world::data::TRADER_WEAPONS) == weapons {
                                shelf_row(ui, world, local, index, *slot, row, to, orders);
                                row += 1;
                            }
                        }
                    }
                    form_heading(ui, TRADER_ITEMS, Some(TRADER_ITEMS_INTRO));
                    item_rows(ui, world, local, orders);
                });
            form_total(ui, world, local);
        });
    if let Some(shown) = shown {
        window_shown(ctx, "trader-window-rect", shown.response.rect, mates, false);
    }
    mine.line = ctx
        .data(|d| d.get_temp::<Option<TradeLine>>(line_eyed_id()))
        .flatten();
    ctx.data_mut(|d| {
        d.insert_temp(id, to);
        d.insert_temp(tab, selling);
    });
}

/// Where the trader's window keeps, for its line items, the lines the
/// others have their eye on, and the one this player's pointer is on.
fn line_marks_id() -> egui::Id {
    egui::Id::new("trader-line-marks")
}

fn line_eyed_id() -> egui::Id {
    egui::Id::new("trader-line-eyed")
}

/// The form's width, in points, and a line item's height.
const FORM_WIDTH: f32 = 440.0;
const LINE_HEIGHT: f32 = 42.0;
/// The side of a line item's icon cell.
const LINE_ICON: f32 = 34.0;
/// The form's paper: darker and more solid than a panel, ruled in the
/// accent, so it reads as a document laid over the map.
const FORM_PAPER: egui::Color32 = egui::Color32::from_rgba_premultiplied(13, 19, 17, 246);
const FORM_RULE: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x5a, 0x4b);
const FORM_STRIPE: egui::Color32 = egui::Color32::from_rgba_premultiplied(24, 34, 30, 150);
const FORM_HOVER: egui::Color32 = egui::Color32::from_rgba_premultiplied(34, 51, 43, 200);

fn form_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FORM_PAPER)
        .stroke(egui::Stroke::new(1.0, FORM_RULE))
        .corner_radius(4.0)
        .inner_margin(12.0)
        .shadow(egui::epaint::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: egui::Color32::from_black_alpha(140),
        })
}

/// The header band: *TRADER* over *PURCHASE ORDER · No. 0042-01*, the
/// front's stamp where the machines are near, and the "?".
/// Where the trader the player put the Trader panel away at is kept:
/// shut at one trader, it comes up again at the next.
fn trader_shut_id() -> egui::Id {
    egui::Id::new("trader-shut-at")
}

/// Whether the player has put the panel away at this trader.
fn trader_shut(ctx: &egui::Context, trader: &world::trader::Trader) -> bool {
    let at = (trader.site.star, trader.site.station);
    ctx.data(|d| d.get_temp::<(u32, u32)>(trader_shut_id())) == Some(at)
}

/// The button that brings the Trader panel back, while the crew are at
/// a trader and the panel is put away; on the map column's head or
/// under its tab.
fn trader_reopen(ui: &mut egui::Ui, world: &World, local: u32) {
    let Some(trader) = world.trader_here(local) else {
        return;
    };
    if !trader_shut(ui.ctx(), trader) {
        return;
    }
    let button = egui::Button::new(
        egui::RichText::new(TRADER_REOPEN)
            .strong()
            .color(theme::SITE_TRADER),
    );
    if ui.add(button).on_hover_text(TRADER_REOPEN_TIP).clicked() {
        ui.ctx()
            .data_mut(|d| d.remove::<(u32, u32)>(trader_shut_id()));
    }
}

/// The form's header band; true when its × put the panel away.
fn form_header(ui: &mut egui::Ui, world: &World, trader: &world::trader::Trader) -> bool {
    let mut shut = false;
    let top = ui.cursor().min;
    let band = egui::Rect::from_min_size(top, egui::vec2(FORM_WIDTH, 46.0));
    ui.painter().rect_filled(band, 3.0, theme::RAISED);
    // An accent edge down the band's left, the form's spine.
    ui.painter().rect_filled(
        egui::Rect::from_min_size(band.min, egui::vec2(4.0, band.height())),
        egui::CornerRadius {
            nw: 3,
            sw: 3,
            ne: 0,
            se: 0,
        },
        theme::SITE_TRADER,
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(band.shrink2(egui::vec2(12.0, 5.0))),
        |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(
                        egui::RichText::new(TRADER_TITLE.to_uppercase())
                            .strong()
                            .size(18.0)
                            .color(theme::INK),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            TRADER_FORM.to_uppercase(),
                            trader_form_no(trader.site.star, trader.site.station)
                        ))
                        .monospace()
                        .size(10.5)
                        .color(theme::MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new(TRADER_SHUT).strong().size(16.0))
                        .on_hover_text(TRADER_SHUT_TIP)
                        .clicked()
                    {
                        shut = true;
                    }
                    theme::question_mark(ui, TRADER_TIP);
                    // A trader near the machines charges over the odds for
                    // what a fight is fought with (feature 94): stamped.
                    if let Some(hops) = world.run.site.and_then(|id| world.front_at(id)) {
                        stamp(ui, TRADER_FRONT_STAMP, theme::WARN)
                            .on_hover_text(format!("{}\n{FRONT_PREMIUM_TIP}", front_premium(hops)));
                    }
                });
            });
        },
    );
    ui.advance_cursor_after_rect(band);
    shut
}

/// A word in a stamped box, tilted a little, in `color`.
fn stamp(ui: &mut egui::Ui, word: &str, color: egui::Color32) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(word.to_uppercase(), egui::FontId::monospace(11.0), color);
    let size = galley.size() + egui::vec2(10.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    paint_stamp(ui.painter(), rect.center(), galley, color, -0.06);
    response.on_hover_cursor(egui::CursorIcon::Help)
}

/// A stamp's box and word, turned by `angle` about `at`.
fn paint_stamp(
    painter: &egui::Painter,
    at: egui::Pos2,
    galley: std::sync::Arc<egui::Galley>,
    color: egui::Color32,
    angle: f32,
) {
    let half = (galley.size() + egui::vec2(10.0, 6.0)) / 2.0;
    let rot = egui::emath::Rot2::from_angle(angle);
    let corners: Vec<egui::Pos2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|&(x, y)| at + rot * egui::vec2(x * half.x, y * half.y))
        .collect();
    painter.add(egui::Shape::convex_polygon(
        corners.clone(),
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 22),
        egui::Stroke::new(1.5, color),
    ));
    let text_at = at + rot * (-galley.size() / 2.0);
    painter.add(egui::epaint::TextShape::new(text_at, galley, color).with_angle(angle));
}

/// A row of short dashes across the form: the tear-off line between the
/// header and the order.
fn perforation(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(FORM_WIDTH, 8.0), egui::Sense::hover());
    ui.painter().add(egui::Shape::dashed_line(
        &[rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, FORM_RULE),
        5.0,
        4.0,
    ));
}

/// A section's heading: the word in small capitals, the section's rules
/// on a "?" if it has any, and a rule drawn out to the form's edge.
fn form_heading(ui: &mut egui::Ui, text: &str, tip: Option<&str>) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(text.to_uppercase())
                .small()
                .strong()
                .color(theme::ACCENT),
        );
        if let Some(tip) = tip {
            theme::question_mark(ui, tip);
        }
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().max(0.0), 10.0),
            egui::Sense::hover(),
        );
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            egui::Stroke::new(1.0, FORM_RULE),
        );
    });
    ui.add_space(2.0);
}

/// What a line item's icon cell shows.
#[derive(Clone, Copy)]
enum Face {
    Thing(bims::combat::Item),
    /// A slot sold: the cell left empty.
    Empty,
}

/// One line of the order.
struct Line<'a> {
    face: Face,
    /// The cell's tint: its tier's colour, if above one.
    tint: Option<egui::Color32>,
    name: &'a str,
    /// Pips under the name: a tier, and the tier it becomes (an upgrade).
    tier: Option<(u32, Option<u32>)>,
    /// A short word under the name, after the pips.
    note: Option<String>,
    price: u64,
    button: &'a str,
    /// Whether the button may be pressed: the pool can pay, or (the
    /// relic) pays only later.
    open: bool,
    /// The hover over the line: a thing's numbers, a relic's effect.
    tip: Option<String>,
    row: usize,
    /// Which line of the form it is, for the others' eyes on it.
    key: Option<TradeLine>,
    /// An item the player's own Bim carries: bought, it goes a tier up.
    /// The line is outlined in the next tier's colour and its button is
    /// wide and lit.
    upgrade: bool,
    /// A stamp in place of the price and the button: an item at its top
    /// tier. A slot sold is stamped *SOLD* whatever this says.
    stamp: Option<&'a str>,
    /// A thing on the Sell tab the player's own Bim wears or carries in
    /// an item slot: washed and outlined in the caution colour, its note
    /// in it too, so the weapon in hand is not sold by a slip.
    worn: bool,
}

/// Draws a line item and answers whether its button was pressed. A line
/// the pool cannot pay for shows its price in the warning colour, its
/// button greyed.
fn line_item(ui: &mut egui::Ui, wallet: economy::Money, line: Line) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(FORM_WIDTH, LINE_HEIGHT), egui::Sense::hover());
    ui.add_space(2.0);
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let painter = ui.painter().clone();
    let hovered = response.hovered();
    if hovered {
        painter.rect_filled(rect, 3.0, FORM_HOVER);
    } else if line.row % 2 == 1 {
        painter.rect_filled(rect, 3.0, FORM_STRIPE);
    }
    if let Some(key) = line.key {
        if ui.rect_contains_pointer(rect) {
            ui.ctx()
                .data_mut(|d| d.insert_temp(line_eyed_id(), Some(key)));
        }
        let marks: Vec<(TradeLine, egui::Color32)> = ui
            .ctx()
            .data(|d| d.get_temp(line_marks_id()))
            .unwrap_or_default();
        let colours: Vec<egui::Color32> = marks
            .iter()
            .filter(|(k, _)| *k == key)
            .map(|(_, c)| *c)
            .collect();
        mark_chosen(&painter, rect, &colours);
    }
    // An upgrade: washed and outlined in the tier it makes.
    let upgrade_colour = line.tint.unwrap_or(theme::ACCENT);
    if line.upgrade {
        painter.rect(
            rect,
            3.0,
            upgrade_colour.gamma_multiply(0.14),
            egui::Stroke::new(1.5, upgrade_colour),
            egui::StrokeKind::Inside,
        );
    }
    if line.worn {
        painter.rect(
            rect,
            3.0,
            theme::CAUTION.gamma_multiply(0.10),
            egui::Stroke::new(1.5, theme::CAUTION),
            egui::StrokeKind::Inside,
        );
    }
    let button_width = if line.upgrade { 96.0 } else { 70.0 };
    // The icon, in its cell.
    let cell = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 6.0 + LINE_ICON / 2.0, rect.center().y),
        egui::vec2(LINE_ICON, LINE_ICON),
    );
    painter.rect(
        cell,
        4.0,
        theme::PANEL_DEEP,
        egui::Stroke::new(1.0, theme::LINE),
        egui::StrokeKind::Inside,
    );
    if let Some(tint) = line.tint {
        theme::tint_cell(&painter, cell, 4.0, tint);
    }
    let inner = cell.shrink(4.0);
    match line.face {
        Face::Thing(item) => crate::icons::icon(&painter, inner, item),
        Face::Empty => {}
    }
    let sold = matches!(line.face, Face::Empty);
    // The button at the right edge, the price before it.
    let button_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 6.0 - button_width, rect.center().y - 12.0),
        egui::vec2(button_width, 24.0),
    );
    let affordable = line.price <= wallet;
    let text_x = cell.right() + 10.0;
    let name_y = rect.center().y - 8.0;
    let name_galley = painter.layout_no_wrap(
        line.name.to_string(),
        egui::FontId::proportional(14.5),
        if sold { theme::MUTED } else { theme::INK },
    );
    let name_end = text_x + name_galley.size().x;
    painter.galley(
        egui::pos2(text_x, name_y - name_galley.size().y / 2.0),
        name_galley,
        theme::INK,
    );
    // Under the name: the tier's pips, and the note after them.
    let mut under_x = text_x;
    let under_y = rect.center().y + 10.0;
    if let Some((tier, next)) = line.tier {
        under_x = pips(&painter, egui::pos2(under_x, under_y), tier);
        if let Some(next) = next {
            let a = egui::pos2(under_x + 3.0, under_y);
            let b = egui::pos2(under_x + 15.0, under_y);
            painter.line_segment([a, b], egui::Stroke::new(1.5, theme::MUTED));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    b + egui::vec2(3.0, 0.0),
                    b + egui::vec2(-2.0, -3.5),
                    b + egui::vec2(-2.0, 3.5),
                ],
                theme::MUTED,
                egui::Stroke::NONE,
            ));
            under_x = pips(&painter, egui::pos2(b.x + 8.0, under_y), next);
        }
        under_x += 8.0;
    }
    if let Some(note) = &line.note {
        // Cut short of the button: a long one is whole on the hover.
        let mut clip = rect;
        clip.set_right(button_rect.left() - 6.0);
        painter
            .with_clip_rect(clip.intersect(painter.clip_rect()))
            .text(
                egui::pos2(under_x, under_y),
                egui::Align2::LEFT_CENTER,
                note,
                egui::FontId::proportional(11.0),
                if line.worn {
                    theme::CAUTION
                } else {
                    theme::MUTED
                },
            );
    }
    let stamp = if sold { Some(TRADER_SOLD) } else { line.stamp };
    if let Some(stamp) = stamp {
        let galley = painter.layout_no_wrap(
            stamp.to_string(),
            egui::FontId::monospace(15.0),
            theme::WARN,
        );
        paint_stamp(
            &painter,
            egui::pos2(button_rect.center().x - 30.0, rect.center().y),
            galley,
            theme::WARN,
            -0.10,
        );
        return false;
    }
    // The price, and the dotted leader out to it along the name's line.
    let price_galley = painter.layout_no_wrap(
        euros(line.price),
        egui::FontId::monospace(14.0),
        if affordable { theme::INK } else { theme::WARN },
    );
    let price_left = button_rect.left() - 10.0 - price_galley.size().x;
    painter.galley(
        egui::pos2(price_left, name_y - price_galley.size().y / 2.0),
        price_galley,
        theme::INK,
    );
    let dot_y = name_y + 4.0;
    let mut x = name_end + 8.0;
    while x < price_left - 6.0 {
        painter.circle_filled(egui::pos2(x, dot_y), 0.9, FORM_RULE);
        x += 4.0;
    }
    if let Some(tip) = &line.tip {
        // An item's tier table under its words, the tier it would be
        // bought at lit (an upgrade's, the one above).
        let tiers = match line.face {
            Face::Thing(bims::combat::Item::Module(item)) => Some((
                item.kind,
                line.tier
                    .map_or(item.tier.code(), |(tier, up)| up.unwrap_or(tier)),
            )),
            _ => None,
        };
        // A weapon's or a piece's numbers, every tier's, the same tier lit.
        let gear = match line.face {
            Face::Thing(thing) => Some((thing, line.tier.map(|(tier, up)| up.unwrap_or(tier)))),
            _ => None,
        };
        response.on_hover_ui(|ui| {
            ui.label(tip);
            if let Some((kind, lit)) = tiers {
                crate::crew::item_tiers(ui, kind, lit);
            }
            if let Some((thing, lit)) = gear {
                crate::crew::gear_tiers(ui, thing, lit);
            }
        });
    }
    let mut button = egui::Button::new(
        egui::RichText::new(line.button.to_uppercase())
            .strong()
            .size(12.0),
    )
    .fill(match (line.open, line.upgrade) {
        (true, true) => upgrade_colour.gamma_multiply(0.55),
        (true, false) => theme::RAISED_ON,
        (false, _) => theme::RAISED,
    });
    if line.upgrade {
        button = button.stroke(egui::Stroke::new(1.0, upgrade_colour));
    }
    ui.put(button_rect, button).clicked() && line.open
}

/// Three pips from `at` (their left edge, their middle), `tier` of them
/// lit in the tier's colour. Answers where they end.
fn pips(painter: &egui::Painter, at: egui::Pos2, tier: u32) -> f32 {
    let lit = tier_colour(tier);
    let mut x = at.x;
    for k in 1..=3 {
        let r = egui::Rect::from_min_size(egui::pos2(x, at.y - 2.5), egui::vec2(9.0, 5.0));
        if k <= tier {
            painter.rect_filled(r, 1.5, lit);
        } else {
            painter.rect_stroke(
                r,
                1.5,
                egui::Stroke::new(1.0, theme::LINE),
                egui::StrokeKind::Inside,
            );
        }
        x += 11.0;
    }
    x - 2.0
}

/// A tier's colour: its tint, or the ink's for tier one.
fn tier_colour(tier: u32) -> egui::Color32 {
    match tier {
        2 => theme::TIER_TWO,
        3 => theme::TIER_THREE,
        _ => theme::MUTED,
    }
}

/// The foot of the form: a double rule, *Restock* while a player holds
/// Restock Codes (task 118), and this player's own balance as the total.
fn form_total(ui: &mut egui::Ui, world: &World, local: u32) {
    ui.add_space(4.0);
    let (rule, _) = ui.allocate_exact_size(egui::vec2(FORM_WIDTH, 5.0), egui::Sense::hover());
    for y in [rule.top() + 1.0, rule.bottom() - 1.0] {
        ui.painter()
            .hline(rule.x_range(), y, egui::Stroke::new(1.0, FORM_RULE));
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(euros(world.wallet(local)))
                    .monospace()
                    .strong()
                    .size(20.0)
                    .color(theme::ACCENT),
            );
            ui.label(
                egui::RichText::new(TRADER_BALANCE.to_uppercase())
                    .small()
                    .strong()
                    .color(theme::MUTED),
            );
        });
    });
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

/// One line of the shelf: the thing's icon, name and tier (its numbers on
/// a hover), its price, and *Buy* — or the stamp that it is sold.
#[allow(clippy::too_many_arguments)]
fn shelf_row(
    ui: &mut egui::Ui,
    world: &World,
    local: u32,
    index: usize,
    slot: Option<world::trader::ShelfItem>,
    row: usize,
    to: u32,
    orders: &mut Vec<Order>,
) {
    let wallet = world.wallet(local);
    let Some(item) = slot else {
        line_item(ui, wallet, sold_line("—", row));
        return;
    };
    let thing = shelf_thing(item);
    let what = thing_name(thing);
    let to = (to != u32::MAX).then_some(to);
    let price = world.shelf_price(item);
    // The same weapon or armour a tier under on the Bim it goes to (the
    // player's own, bought into the armory) reads as an upgrade, as an
    // item the Bim carries does: bought onto it, the old one is sold.
    let worn_tier = world::GearSlot::of_item(thing)
        .and_then(|part| world.worn_on(to.unwrap_or(local), part))
        .and_then(|worn| match (worn, thing) {
            (bims::combat::Item::Weapon(o), bims::combat::Item::Weapon(n)) => {
                (o.kind == n.kind).then_some(o.tier)
            }
            (bims::combat::Item::Armour(o), bims::combat::Item::Armour(n)) => {
                (o.kind == n.kind).then_some(o.tier)
            }
            _ => None,
        })
        .filter(|&tier| tier < item.tier);
    let upgrade = worn_tier.is_some();
    let tip = crate::crew::tip_of(thing, 1);
    let bought = line_item(
        ui,
        wallet,
        Line {
            face: Face::Thing(thing),
            tint: theme::item_tint(thing),
            name: what,
            tier: Some((
                worn_tier.map_or(item.tier.code(), |t| t.code()),
                upgrade.then_some(item.tier.code()),
            )),
            price,
            button: if upgrade && to.is_some() {
                TRADER_UPGRADE
            } else {
                TRADER_BUY
            },
            open: price <= wallet,
            tip: Some(if upgrade {
                format!("{TRADER_SHELF_UPGRADE_TIP}\n\n{tip}")
            } else {
                tip
            }),
            row,
            key: Some(TradeLine::Shelf(index as u32)),
            upgrade,
            stamp: None,
            note: upgrade.then(|| TRADER_SHELF_UPGRADE_NOTE.to_string()),
            worn: false,
        },
    );
    if bought {
        orders.push(Order::BuyShelf {
            index: index as u32,
            to,
        });
    }
}

/// A line sold: its slot's cell left empty and stamped *SOLD*.
fn sold_line(name: &str, row: usize) -> Line<'_> {
    Line {
        face: Face::Empty,
        tint: None,
        name,
        tier: None,
        note: None,
        price: 0,
        button: "",
        open: false,
        tip: None,
        row,
        key: None,
        upgrade: false,
        stamp: None,
        worn: false,
    }
}

/// What a thing is called on a line.
fn thing_name(thing: bims::combat::Item) -> &'static str {
    match thing {
        bims::combat::Item::Weapon(w) => weapon_name(Some(w.kind)),
        bims::combat::Item::Armour(p) => armour_name(Some(p.kind)),
        bims::combat::Item::Module(m) => crate::names::item_name(m.kind),
        bims::combat::Item::Stack(_) => "",
    }
}

/// The trader's items (October 2026): a line a kind, always onto the
/// player's own Bim — the day's tier off the shelf, or, where the Bim
/// carries the kind, its *Upgrade* a tier up, outlined in that tier's
/// colour; one at its top stamped *MAX*.
fn item_rows(ui: &mut egui::Ui, world: &World, local: u32, orders: &mut Vec<Order>) {
    let wallet = world.wallet(local);
    let mut row = 0;
    for kind in bims::module::ModuleKind::ALL {
        let Some(offer) = world.item_offer(local, kind.code()) else {
            continue;
        };
        // Bought this visit: SOLD, as a slot of the shelf is, until the
        // next visit.
        if world.item_sold(local, kind.code()) {
            line_item(ui, wallet, sold_line(crate::names::item_name(kind), row));
            row += 1;
            continue;
        }
        let (item, upgrade) = match offer {
            world::items::ItemOffer::Buy(item) => (item, false),
            world::items::ItemOffer::Upgrade { to, .. } => (to, true),
            world::items::ItemOffer::Top(item) => (item, false),
        };
        let top = matches!(offer, world::items::ItemOffer::Top(_));
        let thing = bims::combat::Item::Module(item);
        let tier = item.tier.code();
        let price = world.item_offer_price(offer).unwrap_or(0);
        let tip = crate::names::module_tip(item, !kind.active());
        let bought = line_item(
            ui,
            wallet,
            Line {
                face: Face::Thing(thing),
                tint: theme::item_tint(thing),
                name: crate::names::item_name(kind),
                tier: kind.tiered().then_some(if upgrade {
                    (tier - 1, Some(tier))
                } else {
                    (tier, None)
                }),
                price,
                button: if upgrade { TRADER_UPGRADE } else { TRADER_BUY },
                open: !top && price <= wallet,
                tip: Some(if upgrade {
                    format!("{TRADER_UPGRADE_TIP}\n\n{tip}")
                } else {
                    tip
                }),
                row,
                key: Some(TradeLine::Item(kind.code())),
                upgrade,
                stamp: top.then_some(TRADER_TOP),
                note: upgrade.then(|| TRADER_UPGRADE_NOTE.to_string()),
                worn: false,
            },
        );
        row += 1;
        if bought {
            orders.push(Order::BuyItem { kind: kind.code() });
        }
    }
}

/// The Sell tab (October 2026): everything the player may sell — its own
/// Bim's weapon, armour and items, then its own armory's — a
/// line each at [`World::sell_value`], *Sell* on it.
fn sell_rows(
    ui: &mut egui::Ui,
    world: &World,
    local: u32,
    orders: &mut Vec<Order>,
    name: &dyn Fn(u32) -> String,
) {
    let mut things: Vec<(world::GearSource, Option<u32>)> = Vec::new();
    for who in 0..world.aboard.crew_count() {
        if !world.may_change(local, who) {
            continue;
        }
        for part in world::GearSlot::ALL {
            if world.worn_on(who, part).is_some() {
                things.push((world::GearSource::Worn { who, slot: part }, Some(who)));
            }
        }
    }
    for stored in world.holdings.of(local) {
        things.push((world::GearSource::Armory { id: stored.id }, None));
    }
    let mut row = 0;
    for (from, worn) in things {
        let Ok((thing, value)) = world.sellable(local, from) else {
            continue;
        };
        let tier = match thing {
            bims::combat::Item::Weapon(w) => Some(w.tier.code()),
            bims::combat::Item::Armour(p) => Some(p.tier.code()),
            bims::combat::Item::Module(m) => m.kind.tiered().then_some(m.tier.code()),
            bims::combat::Item::Stack(_) => None,
        };
        // The player's own Bim's (always first: the other players' are
        // not its to sell) is marked, so a slip does not sell it.
        let own = worn == Some(local);
        let sold = line_item(
            ui,
            world.wallet(local),
            Line {
                face: Face::Thing(thing),
                tint: theme::item_tint(thing),
                name: thing_name(thing),
                tier: tier.map(|t| (t, None)),
                note: Some(if own {
                    TRADER_SELL_WORN.to_string()
                } else {
                    sell_from(worn.map(name).as_deref())
                }),
                price: value,
                button: TRADER_SELL,
                // A sale is never short of money.
                open: true,
                tip: Some(crate::crew::tip_of(thing, 1)),
                row,
                key: Some(TradeLine::Sell(row as u32)),
                upgrade: false,
                stamp: None,
                worn: own,
            },
        );
        row += 1;
        if sold {
            orders.push(Order::Sell { from });
        }
    }
    if row == 0 {
        ui.label(
            egui::RichText::new(TRADER_SELL_NONE)
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

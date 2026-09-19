//! The ship design phase: a palette, a tile grid, and everything that
//! decides whether the ship is finished.
//!
//! This is the screen the lobby's Start goes to. It has **no spawn of its
//! own**: the lobby names a star and a station and the world opens docked
//! there; a start without them, or with ones the galaxy has not got, shows
//! "Nowhere to start" and the way back. It never picks a different dock.
//!
//! Two rules the screen is built around and that are cheap to break:
//!
//! * **No strings come out of the rules.** The parts, the prices, the
//!   reasons an edit was refused and the faults in a design are all
//!   numbers, and `names.rs` is where every word lives; `format::euros` is
//!   where the euro sign and the digit grouping live.
//! * **Everything that changes the ship goes through [`Net`].** It is a
//!   local stand-in with a transport's shape: no click handler calls
//!   `Editor::place` directly, every Edit carries the design hash it was
//!   made against, and the day this grows a socket the change is to that
//!   object and to nothing else.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use flight::Target;
use physics::ResourceId;
use ship::{Preset, Session};
use shipdesign::parts::{PartKind, Rotation};
use shipdesign::validate::Severity;
use world::Speed;
use world::world::Command;

use crate::canvas::{Pointer, canvas_painter, paint_shapes, rect_of, root_ui, zoom_factor};
use crate::crew::{GearOrder, ResearchOrder};
use crate::format::euros;
use crate::names::*;
use crate::settings::{Sheet, settings_sheet};
use crate::shapes::View;
use crate::sound::Sounds;
use crate::{Screen, theme};

use super::builder::Settings;

/// How long a refusal stays on screen, in seconds.
const SAID_SECONDS: f64 = 4.0;

/// WASD, in points a second.
pub const PAN_SPEED: f32 = 900.0;

/// Longest a frame may pretend to be, so a window that was hidden does not
/// come back and pan the view across the room.
pub const MAX_FRAME_DT: f32 = 0.1;

/// What the lobby chose, handed over at Start. Consumed when the designer
/// opens; absent for the simulation, which skips the design phase.
#[derive(Resource, Clone)]
pub struct Start(pub Settings);

/// The session: the design phase, and the game the last Accept turns it
/// into. Shared with the game screen, which is the other half of its life.
#[derive(Resource)]
pub struct ShipSession(pub Session);

// --- the network that is not there yet ---------------------------------------

/// An edit to the design, or an order to the ship, as a transport would
/// carry it.
pub enum Message {
    Place {
        kind: u32,
        x: u32,
        y: u32,
        rotation: u32,
    },
    Remove(u32),
    Trade {
        resource: u32,
        units: u32,
        buying: bool,
    },
    Accept(bool),
    Order(Order),
}

/// An order to the ship, once the game has started. Stamped with the
/// **step** it applies at rather than with a design hash: an Edit has to be
/// judged against the ship it was made for, and an order against *when*.
pub enum Order {
    Fly(Target),
    Stop,
    Speed(Speed),
    Deal {
        resource: ResourceId,
        units: u32,
        buying: bool,
    },
    Keep {
        resource: ResourceId,
        units: u32,
    },
    /// Mark a rock at the mining site to be mined, or unmark it: a design
    /// tile, the grid the rocks are laid out on.
    Mark {
        x: i32,
        y: i32,
    },
    ClearMarks,
    /// Lay out a part to be built, at a design tile, turned so.
    Build {
        kind: PartKind,
        x: u32,
        y: u32,
        rotation: Rotation,
    },
    /// Call a site off.
    Cancel {
        site: u32,
    },
    /// Somebody's gear moved: into or out of the hold, on or off. The
    /// hold is the world's, so even putting a helm on goes through the
    /// seam — every player's ship has to agree about where each piece is.
    Gear(GearOrder),
    /// The AI put onto a node, taken off, or a key consumed at the desk:
    /// what the crew know is the world's, so it goes through the seam too.
    Research(ResearchOrder),
    /// The Management tab's tick box: combine matching gear at the
    /// workbench, or stop. The hold is the world's, so it is a command.
    AutoUpgrade(bool),
    /// Charge the hyperdrive for a jump to a star picked on the galaxy
    /// chart. From the helm, like a trip.
    Jump(u32),
}

/// What the other end said about a message.
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub ok: bool,
    /// An `EditError` code when it was refused, else 0.
    pub why: u32,
}

/// The seam. `send` is the client half — it stamps every message with the
/// design hash it was made against — and `receive` is the other end,
/// which applies messages **in arrival order** and reports a refusal back.
/// That is the whole protocol, and it is written out now rather than
/// discovered later inside a click handler.
pub struct Net {
    pub slot: u32,
    pub players: u32,
}

impl Net {
    pub fn place(
        &self,
        session: &mut Session,
        kind: u32,
        x: u32,
        y: u32,
        rotation: u32,
    ) -> Outcome {
        self.send(
            session,
            Message::Place {
                kind,
                x,
                y,
                rotation,
            },
        )
    }

    pub fn remove(&self, session: &mut Session, part: u32) -> Outcome {
        self.send(session, Message::Remove(part))
    }

    pub fn buy(&self, session: &mut Session, resource: u32, units: u32) -> Outcome {
        self.send(
            session,
            Message::Trade {
                resource,
                units,
                buying: true,
            },
        )
    }

    pub fn sell(&self, session: &mut Session, resource: u32, units: u32) -> Outcome {
        self.send(
            session,
            Message::Trade {
                resource,
                units,
                buying: false,
            },
        )
    }

    pub fn accept(&self, session: &mut Session, on: bool) -> Outcome {
        self.send(session, Message::Accept(on))
    }

    pub fn order(&self, session: &mut Session, order: Order) -> Outcome {
        self.send(session, Message::Order(order))
    }

    /// Stamp a message with the design it was made against and post it.
    fn send(&self, session: &mut Session, message: Message) -> Outcome {
        let at = session.editor.hash();
        Self::receive(session, self.slot, at, message)
    }

    /// The other end of the wire. Nothing is queued or reordered; one that
    /// `apply` refuses is rejected and reported to its sender.
    fn receive(session: &mut Session, from: u32, at: u64, message: Message) -> Outcome {
        let why = match message {
            Message::Place {
                kind,
                x,
                y,
                rotation,
            } => session.editor.place(kind, x, y, rotation),
            Message::Remove(part) => session.editor.remove(part),
            Message::Trade {
                resource,
                units,
                buying,
            } => {
                if buying {
                    session.editor.buy(resource, units)
                } else {
                    session.editor.sell(resource, units)
                }
            }
            Message::Accept(on) => {
                if on {
                    return Outcome {
                        ok: session.accept(from, at),
                        why: 0,
                    };
                }
                session.editor.unaccept(from);
                0
            }
            Message::Order(order) => {
                // Queued, never applied: a command lands at a step, the same
                // step for everybody.
                if let Some(game) = &mut session.game {
                    let slot = from;
                    game.send(match order {
                        Order::Fly(target) => Command::Confirm { slot, target },
                        Order::Stop => Command::Abort { slot },
                        Order::Jump(star) => Command::Jump { slot, star },
                        Order::Speed(speed) => Command::SetSpeed { slot, speed },
                        Order::Deal {
                            resource,
                            units,
                            buying,
                        } => {
                            if buying {
                                Command::Buy {
                                    slot,
                                    resource,
                                    units,
                                }
                            } else {
                                Command::Sell {
                                    slot,
                                    resource,
                                    units,
                                }
                            }
                        }
                        Order::Keep { resource, units } => Command::SetCraftTarget {
                            slot,
                            resource,
                            units,
                        },
                        Order::Mark { x, y } => Command::MarkRock { slot, x, y },
                        Order::ClearMarks => Command::ClearMarks { slot },
                        Order::Build {
                            kind,
                            x,
                            y,
                            rotation,
                        } => Command::PlaceSite {
                            slot,
                            kind,
                            origin: (x, y),
                            rotation,
                        },
                        Order::Cancel { site } => Command::CancelSite { slot, site },
                        Order::Gear(GearOrder::Stow { who, cell }) => {
                            Command::Stow { slot, who, cell }
                        }
                        Order::Gear(GearOrder::Fetch { who, kind }) => {
                            Command::Fetch { slot, who, kind }
                        }
                        Order::Gear(GearOrder::Equip { who, cell }) => {
                            Command::Equip { slot, who, cell }
                        }
                        Order::Gear(GearOrder::Unequip { who, part }) => {
                            Command::Unequip { slot, who, part }
                        }
                        Order::Gear(GearOrder::Discard { who, cell }) => {
                            Command::Discard { slot, who, cell }
                        }
                        Order::Gear(GearOrder::Loot { who, source, cell }) => Command::Loot {
                            slot,
                            who,
                            source,
                            cell,
                        },
                        Order::Gear(GearOrder::Hire { who, resident }) => Command::Hire {
                            slot,
                            who,
                            resident,
                        },
                        Order::Gear(GearOrder::Execute { who, resident }) => Command::Execute {
                            slot,
                            who,
                            resident,
                        },
                        Order::Gear(GearOrder::TakeKey { who }) => Command::TakeKey { slot, who },
                        Order::Research(ResearchOrder::Begin(node)) => {
                            Command::Research { slot, node }
                        }
                        Order::Research(ResearchOrder::Cancel) => Command::CancelResearch { slot },
                        Order::Research(ResearchOrder::Unlock(node)) => {
                            Command::Unlock { slot, node }
                        }
                        Order::AutoUpgrade(on) => Command::SetAutoUpgrade { slot, on },
                    });
                }
                0
            }
        };
        Outcome { ok: why == 0, why }
    }
}

// --- the screen ----------------------------------------------------------------

#[derive(Resource)]
pub struct DesignerScreen {
    pub net: Net,
    /// Something said for a moment: a refusal.
    said: Option<(String, f64)>,
    /// A middle-drag, panning: where the pointer was last.
    pan_from: Option<Vec2>,
    /// The Esc sheet, if it is up, and which page.
    pub sheet: Option<Sheet>,
    size: Vec2,
    /// Why there is nowhere to start, on the lost screen.
    lost: String,
}

pub struct DesignerPlugin;

impl Plugin for DesignerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Design), open)
            .add_systems(
                EguiPrimaryContextPass,
                frame.run_if(in_state(Screen::Design)),
            )
            .add_systems(EguiPrimaryContextPass, lost.run_if(in_state(Screen::Lost)));
    }
}

fn open(
    mut commands: Commands,
    start: Option<Res<Start>>,
    window: Single<&Window>,
    mut next: ResMut<NextState<Screen>>,
) {
    let Some(start) = start else {
        // Nothing handed over: nowhere to start.
        commands.insert_resource(DesignerScreen {
            net: Net {
                slot: 0,
                players: 1,
            },
            said: None,
            pan_from: None,
            sheet: None,
            size: Vec2::ZERO,
            lost: "The lobby did not say which station to start at.".into(),
        });
        next.set(Screen::Lost);
        return;
    };
    let s = start.0.clone();
    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
    let session = Session::design(
        s.ship,
        s.money_per_bim,
        s.players,
        s.slot,
        s.seed,
        s.galaxy,
        s.spawn,
        Preset::Playtest,
        size.x,
        size.y,
    );
    let ok = session.spawn_ok();
    let lost = match s.spawn {
        None => "The lobby did not say which station to start at.".into(),
        Some((star, station)) => {
            format!("There is no station {station} at star {star} in this galaxy.")
        }
    };
    commands.insert_resource(DesignerScreen {
        net: Net {
            slot: session.editor.local,
            players: session.editor.players,
        },
        said: None,
        pan_from: None,
        sheet: None,
        size: Vec2::ZERO,
        lost,
    });
    commands.insert_resource(ShipSession(session));
    commands.remove_resource::<Start>();
    if !ok {
        next.set(Screen::Lost);
    }
}

/// Nowhere to start. Shown instead of the design phase when the lobby
/// named no station, or one the galaxy has not got. The way back is the
/// whole of it: the lobby is where a start is chosen.
fn lost(
    mut contexts: EguiContexts,
    screen: Res<DesignerScreen>,
    mut next: ResMut<NextState<Screen>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = root_ui(&ctx);
    egui::CentralPanel::default().show(&mut root, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(egui::RichText::new("Nowhere to start").size(28.0).strong());
            ui.label(egui::RichText::new(&screen.lost).color(theme::MUTED));
            ui.add_space(12.0);
            if ui.link("Back to the lobby").clicked() {
                next.set(Screen::Menu);
            }
        });
    });
    Ok(())
}

fn frame(
    mut contexts: EguiContexts,
    mut screen: ResMut<DesignerScreen>,
    mut session: ResMut<ShipSession>,
    mut next: ResMut<NextState<Screen>>,
    time: Res<Time>,
    mut sounds: ResMut<Sounds>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let screen = &mut *screen;
    let session = &mut session.0;
    let now = ctx.input(|i| i.time);
    let dt = time.delta_secs().min(MAX_FRAME_DT);
    let mut root = root_ui(&ctx);
    let editable = session.designing();

    // --- the header --------------------------------------------------------
    egui::Panel::top("design-bar").show(&mut root, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Ship design");
            let area = session.editor.design.build_area;
            ui.label(egui::RichText::new(format!("{area} × {area} tiles")).color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // What is left, beside what there was. A pool the current
                // tool would overdraw is marked, which is the only warning a
                // player gets before a click is simply refused.
                let left = session.remaining();
                let pool = session.editor.budget.pool;
                let short = session.editor.tool.def().price > left;
                ui.label(egui::RichText::new(format!("/ {}", euros(pool))).color(theme::MUTED));
                ui.label(egui::RichText::new(euros(left)).strong().color(if short {
                    theme::WARN
                } else {
                    theme::INK
                }));
                ui.label(egui::RichText::new("Money").color(theme::MUTED));
            });
        });
    });

    // --- the palette -------------------------------------------------------
    egui::Panel::left("palette")
        .default_size(200.0)
        .show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Only what a crew knows how to build at the start: the yard
                // builds what the crew can ask for, and a smelter or an
                // armoury is researched in the game — `shipdesign::research`.
                let known = shipdesign::research::Research::new();
                let offered = |k: &u32| {
                    (*k as usize) < PartKind::ALL.len()
                        && !NOT_A_TOOL.contains(k)
                        && PartKind::from_code(*k).is_some_and(|kind| known.part_allowed(kind))
                };
                let mut placed: Vec<u32> = Vec::new();
                let mut groups: Vec<(&str, Vec<u32>)> = Vec::new();
                for (name, kinds) in PART_GROUPS {
                    let kinds: Vec<u32> = kinds.iter().copied().filter(offered).collect();
                    if kinds.is_empty() {
                        continue;
                    }
                    placed.extend(&kinds);
                    groups.push((name, kinds));
                }
                // Whatever the enum has that the groups above have not. Empty
                // in a healthy build; a heading nobody meant to see is the point.
                let rest: Vec<u32> = (0..PartKind::ALL.len() as u32)
                    .filter(|k| !placed.contains(k) && offered(k))
                    .collect();
                if !rest.is_empty() {
                    groups.push(("Anything else", rest));
                }
                let tool = session.editor.tool.code();
                for (name, kinds) in groups {
                    theme::heading(ui, name);
                    for code in kinds {
                        let Some(kind) = PartKind::from_code(code) else {
                            continue;
                        };
                        let (w, h) = session.part_size(kind);
                        let size = if w == 1 && h == 1 {
                            String::new()
                        } else {
                            format!("{w}×{h}")
                        };
                        let on = code == tool;
                        let response = ui.add_enabled_ui(editable, |ui| {
                            ui.horizontal(|ui| {
                                theme::swatch(ui, theme::ship_color32(Session::part_color(kind)));
                                let button = egui::Button::new(part_name(kind))
                                    .min_size(egui::vec2(130.0, 0.0));
                                let button = if on {
                                    button.fill(theme::RAISED_ON)
                                } else {
                                    button
                                };
                                let r = ui.add(button);
                                ui.label(egui::RichText::new(size).small().color(theme::MUTED));
                                r
                            })
                            .inner
                        });
                        if response.inner.clicked() {
                            session.editor.set_tool(code);
                        }
                    }
                }
            });
        });

    // --- what is wrong with it, and the station ------------------------------
    let mut accept_clicked = false;
    let mut trade: Option<(u32, u32, bool)> = None;
    let mut focus: Option<usize> = None;
    egui::Panel::right("checks")
        .default_size(300.0)
        .show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                theme::heading(ui, "Station");
                trade = trade_rows(ui, session, editable, |s, id| s.cargo(id));
                theme::heading(ui, "Checks");
                focus = issue_rows(ui, session);
                theme::heading(ui, "Crew");
                for slot in 0..screen.net.players {
                    let yes = session.editor.accepted(slot);
                    ui.horizontal(|ui| {
                        theme::swatch(ui, if yes { theme::ACCENT } else { theme::RAISED });
                        ui.label(if slot == screen.net.slot {
                            format!("Player {} (you)", slot + 1)
                        } else {
                            format!("Player {}", slot + 1)
                        });
                        ui.label(
                            egui::RichText::new(if yes { "Accepted" } else { "Designing" })
                                .small()
                                .color(theme::MUTED),
                        );
                    });
                }
                ui.add_space(10.0);
                let blocked = session.editor.has_errors();
                let mine = session.editor.accepted(screen.net.slot);
                let label = if mine { "Accepted" } else { "Accept" };
                if theme::big(ui, label, !blocked && editable).clicked() {
                    accept_clicked = true;
                }
                let agreed = (0..screen.net.players)
                    .filter(|&s| session.editor.accepted(s))
                    .count();
                let note = if blocked {
                    "Put the errors above right first.".to_string()
                } else if screen.net.players == 1 {
                    "Accepting settles the ship.".to_string()
                } else {
                    format!("{agreed} of {} have accepted.", screen.net.players)
                };
                ui.label(egui::RichText::new(note).small().color(theme::MUTED));
            });
        });
    session.editor.focus = focus;

    if let Some((resource, units, buying)) = trade {
        let done = if buying {
            screen.net.buy(session, resource, units)
        } else {
            screen.net.sell(session, resource, units)
        };
        screen.said = (!done.ok).then(|| (edit_line(done.why).to_string(), now + SAID_SECONDS));
    }
    if accept_clicked {
        let mine = session.editor.accepted(screen.net.slot);
        let done = screen.net.accept(session, !mine);
        if !done.ok {
            screen.said = Some((
                "That Accept was for a ship that has since changed.".into(),
                now + SAID_SECONDS,
            ));
        }
    }

    // --- the grid ------------------------------------------------------------
    let canvas = rect_of(root.available_rect_before_wrap());
    let size = canvas.size();
    if size != screen.size && size.x > 0.0 && size.y > 0.0 {
        // The first size the canvas is actually seen at is the one the
        // phase should have opened on: the whole build area, in view.
        if screen.size == Vec2::ZERO {
            session.fit(size.x, size.y);
        } else {
            session.resize(size.x, size.y);
        }
        screen.size = size;
    }
    let pointer = Pointer::read(&ctx);
    let on_grid = pointer.on(canvas);
    let keys = !ctx.egui_wants_keyboard_input() && screen.sheet.is_none();

    // Left drags place, right drags clear, middle drags pan.
    if let Some(p) = on_grid {
        if pointer.middle_pressed {
            screen.pan_from = Some(p);
        }
        if editable && session.editor.drag.is_none() {
            if pointer.primary_pressed {
                session.editor.drag_begin(p.x, p.y, false);
            } else if pointer.secondary_pressed {
                session.editor.drag_begin(p.x, p.y, true);
            }
        }
        if pointer.scroll != 0.0 {
            session.zoom(p.x, p.y, zoom_factor(pointer.scroll));
        }
    }
    let here = pointer.pos.map(|p| p - canvas.min);
    if let Some(from) = screen.pan_from {
        match here {
            Some(p) if pointer.middle_down => {
                session.pan(p.x - from.x, p.y - from.y);
                screen.pan_from = Some(p);
            }
            _ => screen.pan_from = None,
        }
    }
    if session.editor.drag.is_some() {
        if let Some(p) = here {
            session.editor.hover_at(p.x, p.y);
        }
        let removing = session.editor.drag.is_some_and(|d| d.removing);
        let let_go = if removing {
            pointer.secondary_released
        } else {
            pointer.primary_released
        };
        if let_go {
            commit_drag(screen, session, now);
        } else if pointer.pos.is_none() {
            session.editor.drag_cancel();
        }
    } else if let Some(p) = on_grid {
        session.editor.hover_at(p.x, p.y);
    } else if screen.pan_from.is_none() {
        session.editor.leave();
    }

    if keys {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::R) {
                session.editor.rotate_ghost();
            }
            if i.key_pressed(egui::Key::Escape) {
                session.editor.drag_cancel();
                screen.sheet = Some(Sheet::Menu);
            }
            let step = PAN_SPEED * dt;
            let mut d = Vec2::ZERO;
            if i.key_down(egui::Key::A) {
                d.x += step;
            }
            if i.key_down(egui::Key::D) {
                d.x -= step;
            }
            if i.key_down(egui::Key::W) {
                d.y += step;
            }
            if i.key_down(egui::Key::S) {
                d.y -= step;
            }
            if d != Vec2::ZERO {
                session.pan(d.x, d.y);
            }
        });
    } else if screen.sheet.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        screen.sheet = None;
    }

    // --- the readout ---------------------------------------------------------
    // What is under the pointer and what the tool would cost. Said before
    // the click rather than after it: the ghost is already red on the deck;
    // this is the same fact where the pointer is looking.
    let hovered = session.editor.hovered_part();
    let tool = session.editor.tool;
    let what = if hovered != 0 {
        session
            .part_kind(hovered)
            .map(part_name)
            .unwrap_or("Something")
    } else {
        part_name(tool)
    };
    let tile = if session.hover_inside() {
        session
            .editor
            .hover
            .map(|t| format!("{}, {}", t.0.max(0), t.1.max(0)))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let ok = session.editor.ghost_ok();
    egui::Area::new(egui::Id::new("design-readout"))
        .fixed_pos(egui::pos2(canvas.min.x + 10.0, canvas.min.y + 10.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            super::room::panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(what).strong().color(if ok {
                        theme::INK
                    } else {
                        theme::WARN
                    }));
                    ui.label(egui::RichText::new(euros(tool.def().price)).color(theme::MUTED));
                    ui.label(egui::RichText::new(tile).color(theme::MUTED));
                });
            });
        });
    egui::Area::new(egui::Id::new("design-keys"))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(canvas.min.x + 10.0, -10.0))
        .order(egui::Order::Middle)
        .show(&ctx, |ui| {
            if let Some((said, until)) = &screen.said
                && now < *until
            {
                ui.label(egui::RichText::new(said).color(theme::WARN));
            }
            ui.label(
                egui::RichText::new(
                    "R turns · drag to fill · right-click peels the top part, again for the next · middle-drag or WASD to pan · wheel to zoom · Esc for the keys",
                )
                .small()
                .color(theme::MUTED),
            );
        });
    settings_sheet(&ctx, &mut screen.sheet, &mut sounds.mix);

    // --- painting --------------------------------------------------------------
    let view = View {
        scale: session.view_scale(),
        offset: {
            let (x, y) = session.view_offset();
            Vec2::new(x, y)
        },
    };
    let painter = canvas_painter(&ctx, canvas);
    paint_shapes(&painter, canvas, view, session.render());

    // The last Accept settles the ship *and* opens the world — one event.
    if session.playing() {
        next.set(Screen::Game);
    }
    Ok(())
}

/// Turn the drag under the pointer into a run of Edits and post them.
///
/// Everything is read out **before** the first Edit is applied: the parts a
/// clearing drag names are looked up in the design it was drawn over, and
/// applying as you go would have the list shifting under itself. A failing
/// Edit is skipped and counted, never fatal: a rectangle of deck over a
/// half-floored room is *meant* to fill the gaps and pass over the rest.
fn commit_drag(screen: &mut DesignerScreen, session: &mut Session, now: f64) {
    let removing = session.editor.drag.is_some_and(|d| d.removing);
    let edits: Vec<Message> = if removing {
        session
            .editor
            .drag_parts()
            .into_iter()
            .map(Message::Remove)
            .collect()
    } else {
        let kind = session.editor.tool.code();
        let rotation = session.editor.ghost.code();
        session
            .editor
            .drag_tiles()
            .into_iter()
            .map(|(x, y)| Message::Place {
                kind,
                x,
                y,
                rotation,
            })
            .collect()
    };
    session.editor.drag_cancel();

    let total = edits.len();
    let mut skipped = 0;
    // The **first** refusal, not the last: a removing drag goes from the
    // top of the stack down, so the first thing to refuse is the thing the
    // player was pointing at.
    let mut why = 0;
    for edit in edits {
        let done = match edit {
            Message::Remove(part) => screen.net.remove(session, part),
            Message::Place {
                kind,
                x,
                y,
                rotation,
            } => screen.net.place(session, kind, x, y, rotation),
            _ => continue,
        };
        if !done.ok {
            skipped += 1;
            if why == 0 {
                why = done.why;
            }
        }
    }
    screen.said = if skipped == 0 {
        None
    } else if total == 1 {
        Some((edit_line(why).to_string(), now + SAID_SECONDS))
    } else {
        Some((
            format!("{skipped} of {total} skipped — {}", edit_line(why)),
            now + SAID_SECONDS,
        ))
    };
}

/// The station's goods and the ship's holds: one row per resource — what
/// it costs, how much is aboard, and the buttons that move it — then how
/// full each hold is. `aboard` says where the count comes from. Returns a
/// deal the player asked for, if any.
pub fn trade_rows(
    ui: &mut egui::Ui,
    session: &Session,
    enabled: bool,
    aboard: impl Fn(&Session, ResourceId) -> u32,
) -> Option<(u32, u32, bool)> {
    let mut deal = None;
    let left = session.remaining();
    egui::Grid::new(("goods", enabled))
        .num_columns(4)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            for (i, &id) in ResourceId::ALL.iter().enumerate() {
                let sold = session.sold_here(id);
                let held = aboard(session, id);
                let class = Session::storage_of(id);
                let room = session
                    .storage_capacity(class)
                    .saturating_sub(session.storage_used(class));
                let price = Session::trade_price(id);
                let color = if sold { theme::INK } else { theme::MUTED };
                ui.label(egui::RichText::new(resource_name(id)).color(color));
                ui.label(
                    egui::RichText::new(euros(price))
                        .small()
                        .color(theme::MUTED),
                );
                ui.label(egui::RichText::new(held.to_string()).color(if held > 0 {
                    theme::ACCENT
                } else {
                    theme::MUTED
                }));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for step in TRADE_STEPS {
                        let can = enabled && sold && step as u64 * price <= left && step <= room;
                        if ui
                            .add_enabled(can, egui::Button::new(format!("+{step}")).small())
                            .clicked()
                        {
                            deal = Some((i as u32, step, true));
                        }
                    }
                    for step in TRADE_STEPS {
                        let can = enabled && step <= held;
                        if ui
                            .add_enabled(can, egui::Button::new(format!("−{step}")).small())
                            .clicked()
                        {
                            deal = Some((i as u32, step, false));
                        }
                    }
                });
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    for &class in shipdesign::Storage::ALL.iter() {
        let total = session.storage_capacity(class);
        let used = session.storage_used(class);
        let full = total > 0 && used >= total;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    STORAGE_NAMES
                        .get(class as usize)
                        .copied()
                        .unwrap_or("Storage"),
                )
                .small()
                .color(theme::MUTED),
            );
            ui.label(
                egui::RichText::new(format!("{used} / {total}"))
                    .small()
                    .color(if full { theme::WARN } else { theme::INK }),
            );
        });
    }
    deal
}

/// One row per issue, worst first. Resting on a row rings the tiles it
/// names — a highlight, not a tooltip. Returns the row under the pointer.
fn issue_rows(ui: &mut egui::Ui, session: &Session) -> Option<usize> {
    let mut focus = None;
    let issues = session.editor.issues();
    let mut shown = 0;
    for (i, issue) in issues.iter().enumerate() {
        let Some(line) = issue_line(issue.code) else {
            continue;
        };
        shown += 1;
        let color = if issue.code == ISSUE_GRAVE {
            theme::GRAVE
        } else if issue.severity == Severity::Error {
            theme::BAD
        } else {
            theme::CAUTION
        };
        let response = ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(line).color(color));
            if !issue.tiles.is_empty() {
                ui.label(
                    egui::RichText::new(issue.tiles.len().to_string())
                        .small()
                        .color(theme::MUTED),
                );
            }
        });
        if response.response.contains_pointer() {
            focus = Some(i);
        }
    }
    if shown == 0 {
        ui.label(egui::RichText::new("Nothing wrong with it.").color(theme::ACCENT));
    }
    focus
}

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
use bims::order::CrewOrder;
use physics::ResourceId;
use ship::{Preset, Session};
use shipdesign::parts::{PartKind, Rotation};
use shipdesign::validate::Severity;
use world::Speed;
use world::world::Command;

use crate::canvas::{Pointer, canvas_painter, rect_of, root_ui, zoom_factor};
use crate::crew::{GearOrder, ResearchOrder};
use crate::format::euros;
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::net::{Event, Online, Packet, Wire};
use crate::scene::WorldCanvas;
use crate::settings::{Allowed, Sheet, settings_sheet};
use crate::shapes::View;
use crate::sound::Sounds;
use crate::{Screen, icons, theme};
use economy::Money;
use wire::{PeerId, To};

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

// --- the seam, and the wire behind it ------------------------------------------

/// An edit to the design, or an order to the ship, as the wire carries it.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Order {
    Speed(Speed),
    Deal {
        resource: ResourceId,
        units: u32,
        buying: bool,
        /// Which tier of a gun or a piece of armour: every tier is on
        /// sale since the money rework (feature 95). One for anything
        /// that comes at no tier.
        tier: u32,
    },
    Keep {
        resource: ResourceId,
        units: u32,
    },
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
    /// The workbench window's button: the day's work begun on the pair in
    /// its slots — `Command::Upgrade`.
    Upgrade,
    /// An order to the crew's room — a click on the deck, a walk, a row of
    /// a fixture's menu, a Management box — `Command::Crew`. The crew's
    /// positions are in the checksum, so nothing reaches the room but
    /// through here (`bims::order`).
    Crew(CrewOrder),
    /// The same, given with Shift held: it waits its turn on the crew
    /// member's queue rather than displacing what it is on —
    /// `Command::CrewLater` (feature 69).
    CrewLater(CrewOrder),
    /// Walk the player's own crew member to the station's trading desk —
    /// `Command::ToDesk`.
    ToDesk,
    /// The player's class chosen or changed while playing, until the
    /// first undock — `Command::SetClass` (feature 74).
    SetClass(world::Class),
    /// A talent picked at a level reached — `Command::PickTalent`.
    PickTalent {
        level: u32,
        side: world::Side,
    },
    /// The engineer sent to lay a kit on a room tile — `Command::Deploy`,
    /// the Q and E keys over the deck.
    Deploy {
        kit: world::Kit,
        x: i32,
        y: i32,
    },
    /// A deployable packed up into the engineer's pack — `Command::PackUp`.
    PackUp(u32),
    /// The armourer's repair begun at the workbench — `Command::Repair`.
    Repair,
    /// The soldier braced, or stood easy — `Command::Brace`, the E key
    /// (feature 75).
    Brace(bool),
    /// The soldier's grenade thrown at a room tile — `Command::Throw`,
    /// the Q key over the deck.
    Throw {
        x: i32,
        y: i32,
    },
    /// The medic's heal beam linked to a crew member, or unlinked —
    /// `Command::Beam`, the E key over one (feature 76).
    Beam(Option<u32>),
    /// The medic's surge triggered — `Command::Surge`, the Q key.
    Surge,
    /// The tank stood as a wall, or stood down — `Command::Bulwark`, the
    /// E key (feature 77).
    Bulwark(bool),
    /// The tank's taunt — `Command::Taunt`, the Q key.
    Taunt,
    /// The commander's squad sent, called back or held —
    /// `Command::Squad`, the E, X and Z keys (feature 78).
    Squad(world::SquadAsk),
    /// The commander's rally — `Command::Rally`, the Q key.
    Rally,
    /// Every player's own standing order to the bots that follow them —
    /// `Command::Orders`, the F and T keys (feature 84).
    Orders(world::Standing),
    /// A medic taking a crewmate up into its arms, or setting one
    /// down with `None` — `Command::Carry`, the G key (feature 86).
    Carry(Option<u32>),
    /// A destination put to the crew on the world map, between missions
    /// — `Command::Propose` (feature 103).
    Propose {
        star: u32,
        station: u32,
    },
    /// A yes to the destination on the table, or one taken back —
    /// `Command::Accept`.
    AcceptTrip(bool),
    /// The *Back to ship* button — `Command::Return`.
    ReturnToShip,
    /// An answer to the departure check — `Command::LeaveBehind`.
    LeaveBehind(bool),
    /// The host saying that player has left the game —
    /// `Command::PlayerGone`, carrying the gone player's slot rather than
    /// the sender's.
    PlayerGone(u32),
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
/// That is the whole protocol, and it was written out before there was a
/// wire rather than discovered later inside a click handler.
///
/// With a `wire` (feature 59, `crate::net`) the two halves are on two
/// machines: the **host** receives — its own messages at once, a guest's
/// as they arrive — and tells everybody what went (`Packet::Applied`),
/// which every guest then receives in that order; a **guest** posts an
/// ask and applies nothing itself until the host says so. Without one
/// the two halves are the two calls one after the other, as they were.
pub struct Net {
    pub slot: u32,
    pub players: u32,
    /// The wire, while the game is played with company. `None` is a game
    /// of one — or a guest whose host has gone, which is the same thing
    /// from then on.
    pub wire: Option<Wire>,
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
    /// The host applies at once and tells the room; a guest asks the host
    /// and is told later — the outcome it gets now is a provisional yes,
    /// and a refusal comes back through `Packet::Refused` onto the said
    /// line.
    fn send(&self, session: &mut Session, message: Message) -> Outcome {
        let at = session.editor.hash();
        match &self.wire {
            Some(wire) if !wire.host => {
                wire.send(To::Host, &Packet::Ask { at, message });
                Outcome { ok: true, why: 0 }
            }
            _ => {
                let outcome = Self::receive(session, self.slot, at, message);
                if outcome.ok
                    && let Some(wire) = &self.wire
                {
                    wire.send(
                        To::All,
                        &Packet::Applied {
                            from: self.slot,
                            at,
                            message,
                        },
                    );
                }
                outcome
            }
        }
    }

    /// A guest's ask, arrived at the host: applied in arrival order and
    /// told to the room if it went, refused to the asker if not. Nothing
    /// on a guest, whose asks go to the host.
    pub fn asked(&self, session: &mut Session, from: u32, at: u64, message: Message, peer: PeerId) {
        let Some(wire) = &self.wire else { return };
        if !wire.host {
            return;
        }
        let outcome = Self::receive(session, from, at, message);
        if outcome.ok {
            wire.send(To::All, &Packet::Applied { from, at, message });
        } else {
            wire.send(To::Peer(peer), &Packet::Refused { why: outcome.why });
        }
    }

    /// The host said this went: applied here the same way, in the same
    /// order. What a guest's world is made of.
    pub fn applied(&self, session: &mut Session, from: u32, at: u64, message: Message) {
        if self.wire.as_ref().is_some_and(|w| !w.host) {
            Self::receive(session, from, at, message);
        }
    }

    /// Whether this end is the clock: a game of one, or the host.
    pub fn is_clock(&self) -> bool {
        self.wire.as_ref().is_none_or(|w| w.host)
    }

    /// An order applied here and told to nobody — the one thing a guest
    /// must never do, done on purpose: `BIMS_DESYNC_AT` (`dev.rs`) parts
    /// a guest's world from the host's with it, for the resync to mend
    /// (feature 67). Nothing else calls it.
    pub fn apply_unasked(&self, session: &mut Session, order: Order) {
        let at = session.editor.hash();
        Self::receive(session, self.slot, at, Message::Order(order));
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
                        Order::Speed(speed) => Command::SetSpeed { slot, speed },
                        Order::Deal {
                            resource,
                            units,
                            buying,
                            tier,
                        } => {
                            if buying {
                                Command::Buy {
                                    slot,
                                    resource,
                                    units,
                                    tier,
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
                        Order::Gear(GearOrder::Arrange {
                            class,
                            id,
                            x,
                            y,
                            turned,
                        }) => Command::Arrange {
                            slot,
                            class: class.code(),
                            id,
                            x,
                            y,
                            turned,
                        },
                        Order::Gear(GearOrder::Repack {
                            who,
                            cell,
                            to,
                            turned,
                        }) => Command::Repack {
                            slot,
                            who,
                            cell,
                            to,
                            turned,
                        },
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
                        Order::Gear(GearOrder::TakeKey { who }) => Command::TakeKey { slot, who },
                        // The row on a box of dressings (feature 87): it
                        // binds its own wounds, which is an order to the
                        // room like any other.
                        Order::Gear(GearOrder::BandageAll { who }) => Command::Crew {
                            slot,
                            order: bims::order::CrewOrder::BandageAll { who, patient: who },
                        },
                        Order::Research(ResearchOrder::Begin(node)) => {
                            Command::Research { slot, node }
                        }
                        Order::Research(ResearchOrder::Cancel) => Command::CancelResearch { slot },
                        Order::Research(ResearchOrder::Dequeue(node)) => {
                            Command::Dequeue { slot, node }
                        }
                        Order::Research(ResearchOrder::Unlock(node)) => {
                            Command::Unlock { slot, node }
                        }
                        Order::AutoUpgrade(on) => Command::SetAutoUpgrade { slot, on },
                        Order::Upgrade => Command::Upgrade { slot },
                        Order::Crew(order) => Command::Crew { slot, order },
                        Order::CrewLater(order) => Command::CrewLater { slot, order },
                        Order::ToDesk => Command::ToDesk { slot },
                        Order::SetClass(class) => Command::SetClass { slot, class },
                        Order::PickTalent { level, side } => {
                            Command::PickTalent { slot, level, side }
                        }
                        Order::Deploy { kit, x, y } => Command::Deploy { slot, kit, x, y },
                        Order::PackUp(id) => Command::PackUp { slot, id },
                        Order::Repair => Command::Repair { slot },
                        Order::Brace(on) => Command::Brace { slot, on },
                        Order::Throw { x, y } => Command::Throw { slot, x, y },
                        Order::Beam(patient) => Command::Beam { slot, patient },
                        Order::Surge => Command::Surge { slot },
                        Order::Bulwark(on) => Command::Bulwark { slot, on },
                        Order::Taunt => Command::Taunt { slot },
                        Order::Squad(order) => Command::Squad { slot, order },
                        Order::Rally => Command::Rally { slot },
                        Order::Orders(order) => Command::Orders { slot, order },
                        Order::Carry(who) => Command::Carry { slot, who },
                        Order::Propose { star, station } => Command::Propose {
                            slot,
                            star,
                            station,
                        },
                        Order::AcceptTrip(yes) => Command::Accept { slot, yes },
                        Order::ReturnToShip => Command::Return { slot },
                        Order::LeaveBehind(yes) => Command::LeaveBehind { slot, yes },
                        Order::PlayerGone(gone) => Command::PlayerGone { slot: gone },
                        Order::Gear(GearOrder::StowOnBench { who, cell }) => {
                            Command::StowOnBench { slot, who, cell }
                        }
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
    /// The slots whose players have left the room: their seats agree to
    /// whatever the rest accept, so a game is never held up by an empty
    /// chair. The host's to keep, since the host is who accepts for them.
    gone: Vec<u32>,
    /// `BIMS_AUTO` has pressed Accept; once.
    auto_accepted: bool,
    /// Something said for a moment: a refusal.
    said: Option<(String, f64)>,
    /// What is in the station panel's cart, not yet bought or sold.
    cart: Cart,
    /// A middle-drag, panning: where the pointer was last.
    pan_from: Option<Vec2>,
    /// The Esc sheet, if it is up, and which page.
    pub sheet: Option<Sheet>,
    /// The sheet's save and load pages' state — `crate::save`. Nothing
    /// is saved from here — there is no game yet — but a game is loaded.
    saves: crate::save::Saves,
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
    online: Res<Online>,
) {
    let Some(start) = start else {
        // Nothing handed over: nowhere to start.
        commands.insert_resource(DesignerScreen {
            net: Net {
                slot: 0,
                players: 1,
                wire: None,
            },
            gone: Vec::new(),
            auto_accepted: false,
            said: None,
            cart: Cart::new(),
            pan_from: None,
            sheet: None,
            saves: crate::save::Saves::default(),
            size: Vec2::ZERO,
            lost: "The lobby did not say which station to start at.".into(),
        });
        next.set(Screen::Lost);
        return;
    };
    let s = start.0.clone();
    let size = Vec2::new(window.width().max(64.0), window.height().max(64.0));
    let mut session = Session::design(
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
    // What the players called their crew: on the session, which the
    // save carries, and in `names` for every word said about them.
    session.crew_names = s.names.clone();
    crate::names::set_crew_names(&session.crew_names);
    // And how they wear their hair: on the session, put onto the crew
    // when the world opens (`Session::dress_crew`, feature 62).
    session.crew_hair = s.hair.clone();
    // And which colour each player's own Bim is ringed in, put on with
    // the hair (feature 84).
    session.crew_tints = s.tints.clone();
    // And their classes, through the seam that works the pool out again
    // (`Session::set_class`, feature 74).
    for (slot, &class) in s.classes.iter().enumerate() {
        session.set_class(slot as u32, class);
    }
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
            wire: online.wire(),
        },
        gone: Vec::new(),
        auto_accepted: false,
        said: None,
        cart: Cart::new(),
        pan_from: None,
        sheet: None,
        saves: crate::save::Saves::default(),
        size: Vec2::ZERO,
        lost,
    });
    commands.insert_resource(ShipSession(session));
    commands.remove_resource::<Start>();
    if !ok {
        next.set(Screen::Lost);
    }
}

/// A run from the lobby's settings (feature 102): what Start does now that
/// the design phase is gone from the `game` flow. The session opens
/// straight onto the world — the default ship docked at the station the
/// lobby picked, `money_per_bim` a player's Bim in the pool
/// (`Session::run`) — with the names, the hair, the colours and the
/// classes the lobby dealt, and the screen to go to is handed back: the
/// game, or the lost screen when there is nowhere to start. Every machine
/// of a lobby calls this with the same numbers, the host at its own Start
/// and every guest at the host's, so the worlds are one world.
pub fn start_run(commands: &mut Commands, s: &Settings, size: Vec2) -> Screen {
    let mut session = Session::run(
        s.money_per_bim,
        s.players,
        s.slot,
        s.seed,
        s.galaxy,
        s.spawn,
        &s.classes,
        size.x,
        size.y,
    );
    session.crew_names = s.names.clone();
    crate::names::set_crew_names(&session.crew_names);
    session.crew_hair = s.hair.clone();
    session.crew_tints = s.tints.clone();
    session.dress_crew();
    let ok = session.spawn_ok() && session.game.is_some();
    commands.remove_resource::<Start>();
    if ok {
        commands.insert_resource(ShipSession(session));
        return Screen::Game;
    }
    let lost = match s.spawn {
        None => "The lobby did not say which station to start at.".into(),
        Some((star, station)) => {
            format!("There is no station {station} at star {star} in this galaxy.")
        }
    };
    commands.insert_resource(DesignerScreen {
        net: Net {
            slot: s.slot,
            players: s.players.max(1),
            wire: None,
        },
        gone: Vec::new(),
        auto_accepted: false,
        said: None,
        cart: Cart::new(),
        pan_from: None,
        sheet: None,
        saves: crate::save::Saves::default(),
        size: Vec2::ZERO,
        lost,
    });
    Screen::Lost
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

#[allow(clippy::too_many_arguments)]
fn frame(
    mut contexts: EguiContexts,
    mut screen: ResMut<DesignerScreen>,
    mut session: ResMut<ShipSession>,
    mut next: ResMut<NextState<Screen>>,
    time: Res<Time>,
    mut sounds: ResMut<Sounds>,
    mut bindings: ResMut<Keys>,
    mut commands: Commands,
    mut online: ResMut<Online>,
    // The canvas between the panels, which Bevy draws (feature 97).
    mut world_canvas: WorldCanvas,
) -> Result {
    let _timed = crate::perf::scope(crate::perf::Phase::Frame);
    let ctx = contexts.ctx_mut()?.clone();
    let screen = &mut *screen;
    let session = &mut session.0;
    let keys_now = *bindings;
    let now = ctx.input(|i| i.time);
    let dt = time.delta_secs().min(MAX_FRAME_DT);
    let mut root = root_ui(&ctx);
    let editable = session.designing();

    // --- the wire ------------------------------------------------------------
    // What the others said: a guest's ask lands on the host and goes round
    // as applied; the host's applied lands on every guest. A refusal is a
    // line. The room closing is the end of company — the yard is this
    // player's alone from here, and the empty seats agree to the ship.
    for event in online.drain(now) {
        match event {
            Event::Packet { from, packet } => match packet {
                Packet::Ask { at, message } => {
                    if let Some(slot) = online.slot_of(from) {
                        screen.net.asked(session, slot, at, message, from);
                        accept_for_the_gone(screen, session, message);
                    }
                }
                Packet::Applied { from, at, message } => {
                    screen.net.applied(session, from, at, message);
                }
                // The host's first steps can land here: the last Accept
                // opens the world on the host, which steps it and says so
                // in its next frame, while this end is still a frame from
                // the game screen — the `Applied` that opened the world
                // here is earlier in this very drain. Dropped, they were a
                // guest one step behind for the whole game (found by
                // feature 67's checksum, which then mended it).
                Packet::Steps { n, .. } if !screen.net.is_clock() && session.playing() => {
                    for _ in 0..n {
                        session.world_step();
                    }
                }
                Packet::Refused { why } => {
                    // An Accept refused comes back with no code: it was
                    // for a ship that has since changed.
                    let line = if why == 0 {
                        ACCEPT_STALE
                    } else {
                        edit_line(why)
                    };
                    screen.said = Some((line.to_string(), now + SAID_SECONDS));
                }
                // The host loaded a game from the yard: its world, whole,
                // is this end's now, as this player's own slot, and the
                // game screen opens round it — the way the host's own
                // load leaves the yard (feature 67).
                Packet::World { save, .. }
                    if Some(from) == online.host
                        && screen.net.wire.as_ref().is_some_and(|w| !w.host) =>
                {
                    let (w, h) = (screen.size.x.max(64.0), screen.size.y.max(64.0));
                    match Session::restore_as(&save, online.my_slot(), w, h) {
                        Ok(loaded) => {
                            commands.insert_resource(ShipSession(loaded));
                            screen.sheet = None;
                            next.set(Screen::Game);
                        }
                        Err(why) => {
                            let line = world_refused(&crate::save::load_error(why));
                            screen.said = Some((line, now + SAID_SECONDS));
                        }
                    }
                }
                _ => {}
            },
            Event::Roster => {
                // Whoever is not in the room any more has an empty seat.
                for slot in 0..screen.net.players {
                    let there = online
                        .slots
                        .get(slot as usize)
                        .is_some_and(|id| online.peers.iter().any(|p| p.id == *id));
                    if !there && !screen.gone.contains(&slot) {
                        screen.gone.push(slot);
                        screen.said = Some((
                            player_left(&crate::names::crew_name(slot)),
                            now + SAID_SECONDS,
                        ));
                    }
                }
            }
            Event::Closed(_) | Event::Lost(_) => {
                screen.net.wire = None;
                screen.gone = (0..screen.net.players)
                    .filter(|&s| s != screen.net.slot)
                    .collect();
                screen.said = Some((HOST_GONE.into(), now + SAID_SECONDS));
            }
            _ => {}
        }
    }

    // A Bim's name said late — after the host's Start, or typed since —
    // lands on the crew here, and on every word about them.
    if online.names_said(&mut session.crew_names) {
        crate::names::set_crew_names(&session.crew_names);
    }
    if online.hair_said(&mut session.crew_hair) || online.tint_said(&mut session.crew_tints) {
        session.dress_crew();
    }
    // A class said late lands the same way, through the pool.
    let mut classes = session.crew_classes.clone();
    if online.classes_said(&mut classes) {
        for (slot, &class) in classes.iter().enumerate() {
            session.set_class(slot as u32, class);
        }
    }

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
    let mut trade: Vec<(u32, u32, bool, u32)> = Vec::new();
    let mut focus: Option<usize> = None;
    egui::Panel::right("checks")
        .default_size(420.0)
        .show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                theme::heading(ui, "Station");
                trade = trade_rows(
                    ui,
                    session,
                    &mut screen.cart,
                    editable,
                    editable,
                    |s, id| s.cargo(id),
                );
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
                        // The class a slot (feature 74): everybody's is
                        // shown, this player's own is a picker, and a
                        // change works the pool out again and is said
                        // to the room like the hair.
                        let class = session.class_of(slot);
                        if slot == screen.net.slot && editable {
                            for pick in world::Class::ALL {
                                if theme::toggle(ui, pick == class, class_name(pick)).clicked()
                                    && session.set_class(slot, pick)
                                {
                                    online.say_bim_class(pick);
                                }
                            }
                            theme::question_mark(ui, class_tip(class));
                        } else {
                            ui.label(
                                egui::RichText::new(class_name(class))
                                    .small()
                                    .color(theme::MUTED),
                            );
                        }
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

    // The cart goes as one lot, sells first; the first refusal is what is
    // said, and the rest of the lot still goes — a line that fits fits.
    let mut refused = None;
    // The yard buys at tier one and offers no chooser: `Edit::Buy` takes
    // no tier, and the goods bought here are the crew's to start with.
    for (resource, units, buying, _tier) in trade {
        let done = if buying {
            screen.net.buy(session, resource, units)
        } else {
            screen.net.sell(session, resource, units)
        };
        if !done.ok && refused.is_none() {
            refused = Some(done.why);
        }
    }
    if let Some(why) = refused {
        screen.said = Some((edit_line(why).to_string(), now + SAID_SECONDS));
    }
    // `BIMS_AUTO`: Accept a second into the yard, once.
    if crate::dev::auto().is_some()
        && editable
        && !screen.auto_accepted
        && !session.editor.has_errors()
        && now > 1.0
    {
        accept_clicked = true;
        screen.auto_accepted = true;
    }
    if accept_clicked {
        let mine = session.editor.accepted(screen.net.slot);
        let done = screen.net.accept(session, !mine);
        if !done.ok {
            screen.said = Some((ACCEPT_STALE.into(), now + SAID_SECONDS));
        } else if !mine {
            accept_for_the_gone(screen, session, Message::Accept(true));
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
    // Where this pointer is over the grid, to the room, as a design point;
    // off it — over a panel, out of the window — as nothing.
    online.point(now, on_grid.map(|p| session.design_point(p.x, p.y)));

    if keys {
        ctx.input(|i| {
            if keys_now.pressed(i, Action::Turn) {
                session.editor.rotate_ghost();
            }
            if i.key_pressed(egui::Key::Escape) {
                session.editor.drag_cancel();
                screen.sheet = Some(Sheet::Menu);
            }
            let step = PAN_SPEED * dt;
            let mut d = Vec2::ZERO;
            if keys_now.down(i, Action::PanLeft) {
                d.x += step;
            }
            if keys_now.down(i, Action::PanRight) {
                d.x -= step;
            }
            if keys_now.down(i, Action::PanUp) {
                d.y += step;
            }
            if keys_now.down(i, Action::PanDown) {
                d.y -= step;
            }
            if d != Vec2::ZERO {
                session.pan(d.x, d.y);
            }
        });
    } else if screen.sheet.is_some()
        && keys_now.listening.is_none()
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        // Esc while the controls page waits on a key is that page's.
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
            crate::theme::panel_frame().show(ui, |ui| {
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
            let key = |a: Action| keys_now.key(a).symbol_or_name();
            ui.label(
                egui::RichText::new(format!(
                    "{} turns · drag to fill · right-click peels the top part, again for the next · middle-drag or {}{}{}{} to pan · wheel to zoom · Esc for the keys",
                    key(Action::Turn),
                    key(Action::PanUp),
                    key(Action::PanLeft),
                    key(Action::PanDown),
                    key(Action::PanRight),
                ))
                .small()
                .color(theme::MUTED),
            );
        });
    // Save is greyed out here — a design phase is not a game — and a load
    // is the way out of one: the loaded session takes this one's place
    // and the game screen opens round it, as it does after an Accept.
    // With company the host's load goes to everybody as its whole world
    // (`Packet::World`, feature 67), and has to fit the room first; a
    // guest's Load is greyed.
    let asked = settings_sheet(
        &ctx,
        &mut screen.sheet,
        &mut sounds.mix,
        &mut bindings,
        &mut screen.saves,
        Allowed::of(false, online.is_guest()),
    );
    if let Some(crate::save::Request::Load(path)) = asked {
        let read = crate::save::read(&path).and_then(|text| {
            if let Some(here) = online.room_size()
                && ship::save::players_of(&text) != Some(here)
            {
                let saved = ship::save::players_of(&text).unwrap_or(0);
                return Err(load_players(saved, here));
            }
            Session::restore(&text, screen.size.x.max(64.0), screen.size.y.max(64.0))
                .map(|loaded| (loaded, text))
                .map_err(crate::save::load_error)
        });
        match read {
            Ok((loaded, text)) => {
                if let Some(wire) = &screen.net.wire
                    && wire.host
                {
                    let at = loaded.game.as_ref().map_or(0, |g| g.world.steps);
                    wire.send(To::All, &Packet::World { save: text, at });
                }
                commands.insert_resource(ShipSession(loaded));
                screen.sheet = None;
                next.set(Screen::Game);
            }
            Err(why) => screen.saves.failed(why),
        }
    }

    // --- painting --------------------------------------------------------------
    let view = View {
        scale: session.view_scale(),
        offset: {
            let (x, y) = session.view_offset();
            Vec2::new(x, y)
        },
    };
    let painter = canvas_painter(&ctx, canvas);
    let shapes = {
        let _timed = crate::perf::scope(crate::perf::Phase::Render);
        session.render()
    };
    world_canvas.shapes(&ctx, canvas, view, shapes);
    // The others' pointers over the grid, each in its player's colour
    // with their Bim's name, where the tile they are over is on this
    // screen.
    for (slot, (x, y)) in online.others_pointing() {
        let (x, y) = session.design_point_on_screen(x, y);
        let at = view.to_canvas(Vec2::new(x, y)) + canvas.min;
        theme::ghost_pointer(
            &painter,
            egui::pos2(at.x, at.y),
            theme::ship_color32(ship::world_paint::player_color(slot)),
            &crew_name(slot),
        );
    }

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
        let editor = &session.editor;
        editor
            .drag_tiles()
            .into_iter()
            .map(|(x, y)| Message::Place {
                kind,
                x,
                y,
                // Each tile's own turn: a run of wall lights along a
                // bulkhead hangs from it tile by tile.
                rotation: editor.turn_at((x, y)).code(),
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

/// What a player means to trade, before any of it is: a line a resource,
/// units to buy (positive) or to sell (negative), summed and priced under
/// the rows and sent as one lot on Confirm — a deal is looked over before
/// it is done, the way a market's is. The app's own: nothing moves in the
/// hold or the money until the orders go, and the rules crates never see
/// a cart, so a line that stopped fitting (somebody spent the money,
/// something was stowed) is refused where it always was.
pub struct Cart {
    /// Indexed by `ResourceId`.
    lines: Vec<i32>,
    /// Which **tier** each line is bought at, indexed the same way, and
    /// one wherever nobody has said otherwise (feature 95). Only a gun or
    /// a piece of armour comes at a tier — `economy::tiered` — and only a
    /// buy names one: a sale gives up the lowest tiers first and the
    /// world prices each thing at its own (`World::sell`), so the line
    /// means "buy at this tier" and nothing else.
    tiers: Vec<u32>,
}

/// Why the cart cannot go as it stands, worst first.
enum Short {
    Money(Money),
    Room(shipdesign::Storage, u32),
}

impl Cart {
    pub fn new() -> Cart {
        Cart {
            lines: vec![0; ResourceId::ALL.len()],
            tiers: vec![1; ResourceId::ALL.len()],
        }
    }

    fn line(&self, id: ResourceId) -> i32 {
        self.lines.get(id as usize).copied().unwrap_or(0)
    }

    /// The tier this line buys at: one for anything that comes at no
    /// tier, and whatever the row's chooser was last set to for gear.
    fn tier(&self, id: ResourceId) -> u32 {
        if !economy::tiered(id) {
            return 1;
        }
        self.tiers.get(id as usize).copied().unwrap_or(1)
    }

    /// Deal at another tier. The line goes with it: a tier changed under
    /// a line would be that line at another price, which nobody asked
    /// for.
    fn set_tier(&mut self, id: ResourceId, to: u32) {
        if !economy::tiered(id) {
            return;
        }
        if self.tier(id) != to {
            self.set(id, 0);
        }
        if let Some(tier) = self.tiers.get_mut(id as usize) {
            *tier = to;
        }
    }

    fn add(&mut self, id: ResourceId, by: i32) {
        if let Some(line) = self.lines.get_mut(id as usize) {
            *line = line.saturating_add(by);
        }
    }

    fn set(&mut self, id: ResourceId, to: i32) {
        if let Some(line) = self.lines.get_mut(id as usize) {
            *line = to;
        }
    }

    pub fn clear(&mut self) {
        self.lines.iter_mut().for_each(|l| *l = 0);
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|&l| l == 0)
    }

    /// Units bought and sold, and what they come to: `(bought, cost,
    /// sold, earned)` — the buys at the desk's ask and the sells at its
    /// bid, the desk being the station's (`Session::quote`); a line with
    /// no desk to quote it comes to nothing. Saturated rather than
    /// refused — a sum too big to hold is one the world refuses when it
    /// is sent, and the window only has to show it.
    fn totals(&self, session: &Session) -> (u32, Money, u32, Money) {
        let mut out = (0u32, 0 as Money, 0u32, 0 as Money);
        for &id in ResourceId::ALL.iter() {
            let line = self.line(id);
            let units = line.unsigned_abs();
            if line > 0 {
                // A buy is at the tier the row's chooser names; a sale is
                // quoted at the plain bid, since which tiers actually
                // leave the hold is the world's to decide (`World::sell`
                // gives up the lowest first and pays each at its own), and
                // a summary that guessed high would be a promise.
                let ask = session.quote_at(id, self.tier(id)).map_or(0, |q| q.ask);
                out.0 = out.0.saturating_add(units);
                out.1 = out.1.saturating_add(ask.saturating_mul(units as Money));
            } else if line < 0 {
                let bid = session.quote(id).map_or(0, |q| q.bid);
                out.2 = out.2.saturating_add(units);
                out.3 = out.3.saturating_add(bid.saturating_mul(units as Money));
            }
        }
        out
    }

    /// How many units a class of storage gains (or, negative, loses).
    /// What the cart does to a class, in cells: the stacks each resource
    /// would be in after, less the stacks it is in now, a footprint each —
    /// so a buy that tops up a part-full stack costs no cell, and a sale
    /// frees one only when a stack empties.
    fn change(&self, session: &Session, class: shipdesign::Storage) -> i64 {
        ResourceId::ALL
            .iter()
            .filter(|&&id| Session::storage_of(id) == class)
            .map(|&id| {
                let now = session.cargo(id) as i64;
                let after = (now + self.line(id) as i64).max(0) as u32;
                let stacks = |units: u32| {
                    shipdesign::stacks_of(id, units) as i64 * shipdesign::cells(id) as i64
                };
                stacks(after) - stacks(now as u32)
            })
            .sum()
    }

    /// What is in the way of the cart going, if anything: the money it
    /// wants beyond what there is, or a hold it would overfill and by how
    /// much. The sells are counted first, as they are sent first, so a
    /// cart that sells the ore off a shelf to make room for metal fits.
    fn short(&self, session: &Session) -> Option<Short> {
        let (_, cost, _, earned) = self.totals(session);
        let have = session.remaining().saturating_add(earned);
        if cost > have {
            return Some(Short::Money(cost - have));
        }
        for &class in shipdesign::Storage::ALL.iter() {
            let after = session.storage_used(class) as i64 + self.change(session, class);
            let over = after - session.storage_capacity(class) as i64;
            if over > 0 {
                return Some(Short::Room(class, over as u32));
            }
        }
        None
    }

    /// The deals to send, sells first so the money and the room are there
    /// for the buys: `(resource index, units, buying, tier)`. A sale's
    /// tier is one and means nothing — the world gives up the lowest
    /// tiers first whatever is asked; a buy's is the row's chooser.
    pub fn deals(&self) -> Vec<(u32, u32, bool, u32)> {
        let mut out = Vec::new();
        for (i, &id) in ResourceId::ALL.iter().enumerate() {
            let line = self.line(id);
            if line < 0 {
                out.push((i as u32, line.unsigned_abs(), false, 1));
            }
        }
        for (i, &id) in ResourceId::ALL.iter().enumerate() {
            let line = self.line(id);
            if line > 0 {
                out.push((i as u32, line.unsigned_abs(), true, self.tier(id)));
            }
        }
        out
    }
}

/// The station's goods and the ship's holds: one row per resource — its
/// icon, what one of it costs here and what the desk pays for one (the
/// station's ask and bid, `Session::quote`: every station quotes its own
/// numbers), how much is aboard, the buttons that put it in the cart or
/// take it out, the cart's line as a number to type over (bought above
/// zero, sold below), and what that line comes to — then what the cart
/// comes to, how full each hold would be, and Confirm.
/// `aboard` says where the count comes from. Rows are edited while
/// `editable`; Confirm goes while `confirm` too. Returns the deals the
/// player confirmed, sells first, and empties the cart — or nothing.
pub fn trade_rows(
    ui: &mut egui::Ui,
    session: &Session,
    cart: &mut Cart,
    editable: bool,
    confirm: bool,
    aboard: impl Fn(&Session, ResourceId) -> u32,
) -> Vec<(u32, u32, bool, u32)> {
    // Whether the cart with `id`'s line at `to` would still go.
    let fits_at = |cart: &Cart, id: ResourceId, to: i32| -> bool {
        let mut next = Cart {
            lines: cart.lines.clone(),
            tiers: cart.tiers.clone(),
        };
        next.set(id, to);
        if to < 0 && to.unsigned_abs() > aboard(session, id) {
            return false;
        }
        next.short(session).is_none()
    };
    // Whether the cart with `by` more of `id` in it would still go: the
    // buttons are greyed a step early rather than a deal refused late.
    let fits = |cart: &Cart, id: ResourceId, by: i32| -> bool {
        fits_at(cart, id, cart.line(id).saturating_add(by))
    };
    // A number typed into a line is held to what fits, the way the
    // buttons are: the furthest from `from` (which fit) towards `to`
    // that still does. Money and room run one way along a line, so the
    // search can halve. If `from` itself no longer fits — somebody spent
    // the money since — the typed number stands and the summary says.
    let held_to = |cart: &Cart, id: ResourceId, from: i32, to: i32| -> i32 {
        if fits_at(cart, id, to) || !fits_at(cart, id, from) {
            return to;
        }
        let (mut lo, mut hi) = (from as i64, to as i64);
        while (hi - lo).abs() > 1 {
            let mid = lo + (hi - lo) / 2;
            if fits_at(cart, id, mid as i32) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo as i32
    };
    // What one of a thing costs bought here and fetches sold here, as
    // words: the desk's ask and bid, or a dash where nobody quotes.
    let priced = |session: &Session, id: ResourceId, tier: u32| -> (String, String) {
        match session.quote_at(id, tier) {
            Some(q) => (euros(q.ask), euros(q.bid)),
            None => (NO_QUOTE.into(), NO_QUOTE.into()),
        }
    };
    egui::Grid::new(("goods", editable))
        .num_columns(9)
        .min_col_width(icons::INLINE)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            for word in [
                "",
                "",
                TIER_HEAD,
                ASK_HEAD,
                BID_HEAD,
                ABOARD_HEAD,
                "",
                CART_HEAD,
                "",
            ] {
                ui.label(egui::RichText::new(word).small().color(theme::MUTED));
            }
            ui.end_row();
            for &id in ResourceId::ALL.iter() {
                let sold = session.sold_here(id);
                let held = aboard(session, id);
                // The tier this row deals at: a gun or a piece of armour
                // is on sale at every tier (feature 95), the book times
                // one, four and sixteen, so the row carries a chooser and
                // everything else a blank. It is a **buy**'s tier: the
                // world gives up the lowest tiers first on a sale.
                let tier = cart.tier(id);
                let quote = session.quote_at(id, tier);
                let line = cart.line(id);
                let color = if sold || held > 0 {
                    theme::INK
                } else {
                    theme::MUTED
                };
                icons::resource_cell(ui, id);
                ui.label(egui::RichText::new(resource_name(id)).color(color));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    // Only where there is a world to buy in: the yard's
                    // `Edit::Buy` takes no tier and buys at one.
                    if !economy::tiered(id) || session.game.is_none() {
                        return;
                    }
                    for t in 1..=economy::TIER_PRICE.len() as u32 - 1 {
                        let on = t == tier;
                        let button = egui::Button::new(t.to_string())
                            .small()
                            .selected(on)
                            .min_size(egui::vec2(16.0, 0.0));
                        if ui.add_enabled(editable, button).clicked() {
                            cart.set_tier(id, t);
                        }
                    }
                });
                // What one of it costs here, and what the desk pays for
                // one: the station's own two numbers, every line of them,
                // whether it stocks the thing or only takes it. The ask is
                // dimmed where the thing is not on the shelf, since nobody
                // can buy at it.
                let (ask, bid) = priced(session, id, tier);
                ui.label(egui::RichText::new(ask).color(if sold {
                    theme::INK
                } else {
                    theme::MUTED
                }));
                ui.label(egui::RichText::new(bid).color(color));
                ui.label(egui::RichText::new(held.to_string()).color(if held > 0 {
                    theme::ACCENT
                } else {
                    theme::MUTED
                }));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for step in TRADE_STEPS {
                        let by = step as i32;
                        // Taking a sale back is always fine; buying wants
                        // the station to sell it and the cart to fit.
                        let can = editable && (line + by <= 0 || sold) && fits(cart, id, by);
                        if ui
                            .add_enabled(can, egui::Button::new(format!("+{step}")).small())
                            .clicked()
                        {
                            cart.add(id, by);
                        }
                    }
                    for step in TRADE_STEPS {
                        let by = -(step as i32);
                        let can = editable && fits(cart, id, by);
                        if ui
                            .add_enabled(can, egui::Button::new(format!("−{step}")).small())
                            .clicked()
                        {
                            cart.add(id, by);
                        }
                    }
                });
                // The cart's line, as a number to type over or drag:
                // above zero bought, below zero sold. It can go no lower
                // than what is aboard and no higher than zero where the
                // station does not stock the thing, and a number beyond
                // the money or the room is held back to the most that
                // fits, the way the buttons stop a step early.
                let line = cart.line(id);
                let mut typed = line;
                let lowest = -(held.min(i32::MAX as u32) as i32);
                let highest = if sold { i32::MAX } else { 0 };
                let field = ui.add_enabled(
                    editable,
                    egui::DragValue::new(&mut typed)
                        .range(lowest..=highest)
                        .speed(0.2)
                        .custom_formatter(|n, _| {
                            let n = n as i64;
                            if n > 0 {
                                format!("+{n}")
                            } else if n < 0 {
                                format!("−{}", -n)
                            } else {
                                "—".into()
                            }
                        })
                        .custom_parser(|text| {
                            let text = text.trim().replace('−', "-");
                            if text.is_empty() || text == "—" {
                                Some(0.0)
                            } else {
                                text.parse::<i64>().ok().map(|n| n as f64)
                            }
                        }),
                );
                if field.changed() && typed != line {
                    cart.set(id, held_to(cart, id, line, typed));
                }
                // And what that line comes to: at the ask bought, at the
                // bid sold.
                let line = cart.line(id);
                if line == 0 {
                    ui.label("");
                } else {
                    let each = quote.map_or(0, |q| if line > 0 { q.ask } else { q.bid });
                    ui.label(
                        egui::RichText::new(euros(
                            each.saturating_mul(line.unsigned_abs() as Money),
                        ))
                        .small()
                        .color(if line > 0 {
                            theme::ACCENT
                        } else {
                            theme::CAUTION
                        }),
                    );
                }
                ui.end_row();
            }
        });
    ui.add_space(6.0);

    // --- what the cart comes to --------------------------------------------
    let (bought, cost, sold, earned) = cart.totals(session);
    let short = cart.short(session);
    if bought > 0 {
        ui.label(
            egui::RichText::new(format!("Buying {bought} for {}", euros(cost)))
                .small()
                .color(theme::MUTED),
        );
    }
    if sold > 0 {
        ui.label(
            egui::RichText::new(format!("Selling {sold} for {}", euros(earned)))
                .small()
                .color(theme::MUTED),
        );
    }
    let money = session.remaining();
    ui.horizontal(|ui| {
        if cart.is_empty() {
            ui.label(egui::RichText::new(CART_EMPTY).color(theme::MUTED));
        } else if cost >= earned {
            ui.label(egui::RichText::new(format!("{YOU_PAY} {}", euros(cost - earned))).strong());
        } else {
            ui.label(
                egui::RichText::new(format!("{YOU_EARN} {}", euros(earned - cost)))
                    .strong()
                    .color(theme::ACCENT),
            );
        }
    });
    let after = money.saturating_add(earned).saturating_sub(cost);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(MONEY_AFTER).small().color(theme::MUTED));
        ui.label(egui::RichText::new(euros(after)).small().color(
            if matches!(short, Some(Short::Money(_))) {
                theme::WARN
            } else {
                theme::INK
            },
        ));
    });
    for &class in shipdesign::Storage::ALL.iter() {
        let total = session.storage_capacity(class);
        let used = session.storage_used(class);
        let change = cart.change(session, class);
        let would = (used as i64 + change).max(0) as u32;
        let full = total > 0 && would >= total;
        let over = would > total;
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
            let words = if change == 0 {
                format!("{used} / {total}")
            } else {
                format!("{used} / {total}, {would} after")
            };
            ui.label(egui::RichText::new(words).small().color(if over {
                theme::WARN
            } else if full {
                theme::CAUTION
            } else {
                theme::INK
            }));
        });
    }
    match short {
        Some(Short::Money(by)) => {
            ui.label(
                egui::RichText::new(format!("{SHORT_BY} {}.", euros(by)))
                    .small()
                    .color(theme::WARN),
            );
        }
        Some(Short::Room(class, by)) => {
            // The lockers count cells, a thing's footprint each; the rest
            // count units.
            let what = if class == shipdesign::Storage::Locker {
                format!("{by} more cells")
            } else {
                format!("{by} more")
            };
            ui.label(
                egui::RichText::new(format!(
                    "{NO_ROOM_FOR} {what} on the {}.",
                    STORAGE_NAMES
                        .get(class as usize)
                        .copied()
                        .unwrap_or("storage")
                        .to_lowercase()
                ))
                .small()
                .color(theme::WARN),
            );
        }
        None => {}
    }
    ui.add_space(4.0);
    let mut deals = Vec::new();
    ui.horizontal(|ui| {
        let can = editable && confirm && !cart.is_empty() && short.is_none();
        if theme::big(ui, CONFIRM_TRADE, can).clicked() {
            deals = cart.deals();
            cart.clear();
        }
        if ui
            .add_enabled(editable && !cart.is_empty(), egui::Button::new(CLEAR_CART))
            .clicked()
        {
            cart.clear();
        }
    });
    deals
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

/// An Accept that went is echoed for every seat whose player has left:
/// the empty chairs agree to the ship as it stands, so the last Accept
/// of those still here is the one that opens the world. The host's to
/// do — its own Accepts and a guest's alike land here — and a player
/// alone after the host left has every other seat empty.
fn accept_for_the_gone(screen: &DesignerScreen, session: &mut Session, message: Message) {
    if !matches!(message, Message::Accept(true)) || !screen.net.is_clock() {
        return;
    }
    let at = session.editor.hash();
    for &slot in &screen.gone {
        if !session.editor.accepted(slot) {
            let done = Net::receive(session, slot, at, Message::Accept(true));
            if done.ok
                && let Some(wire) = &screen.net.wire
            {
                wire.send(
                    To::All,
                    &Packet::Applied {
                        from: slot,
                        at,
                        message: Message::Accept(true),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cart is a list of what to buy and sell, priced off the desk the
    /// world charges at — the buys at its ask, the sells at its bid — and
    /// it goes sells first: the money and the shelf room a sale frees are
    /// there for the buys behind it.
    #[test]
    fn a_cart_prices_its_lines_and_sells_before_it_buys() {
        let mut cart = Cart::new();
        assert!(cart.is_empty());
        assert!(cart.deals().is_empty());
        cart.add(ResourceId::Medkit, 10);
        cart.add(ResourceId::Bandage, -10);
        cart.add(ResourceId::Tofu, 3);
        cart.add(ResourceId::Tofu, -3);
        assert!(!cart.is_empty());
        // Docked at the default seed's first dock, so there is a desk to
        // quote; a session with no spawn quotes nothing and the cart
        // comes to nothing.
        let nowhere = Session::design(12, 10_000, 1, 0, 1, 0, None, Preset::Playtest, 800.0, 600.0);
        assert!(nowhere.quote(ResourceId::Medkit).is_none());
        assert_eq!(cart.totals(&nowhere), (10, 0, 10, 0));
        let spawn = world::spawn(&worldgen::Galaxy::new(
            1,
            worldgen::GalaxyType::SpiralTwoArm,
        ));
        let session = Session::design(
            12,
            10_000,
            1,
            0,
            1,
            0,
            spawn,
            Preset::Playtest,
            800.0,
            600.0,
        );
        let (bought, cost, sold, earned) = cart.totals(&session);
        assert_eq!((bought, sold), (10, 10));
        let kit = session.quote(ResourceId::Medkit).unwrap();
        let dressing = session.quote(ResourceId::Bandage).unwrap();
        assert!(kit.bid < kit.ask);
        assert_eq!(cost, 10 * kit.ask);
        assert_eq!(earned, 10 * dressing.bid);
        // The medkits and the dressings share a class, and the change to
        // it is in cells, a stack a footprint: on an empty hold ten
        // medkits bought are ten boxes of two by two and ten dressings
        // sold are nothing gone, so the class is forty cells up; three
        // tofu bought and three sold is nought, and three tofu on their
        // own would be a block of four by four.
        let session = nowhere;
        assert_eq!(session.cargo(ResourceId::Medkit), 0, "an empty hold");
        assert_eq!(
            cart.change(&session, Session::storage_of(ResourceId::Medkit)),
            40
        );
        assert_eq!(
            cart.change(&session, Session::storage_of(ResourceId::Tofu)),
            0
        );
        let mut block = Cart::new();
        block.add(ResourceId::Tofu, 3);
        assert_eq!(
            block.change(&session, Session::storage_of(ResourceId::Tofu)),
            16
        );
        assert_eq!(
            cart.deals(),
            vec![
                (ResourceId::Bandage as u32, 10, false, 1),
                (ResourceId::Medkit as u32, 10, true, 1),
            ]
        );
        cart.clear();
        assert!(cart.is_empty());
    }

    /// A row's tier (feature 95): gear alone has one, a line bought at it
    /// costs the book times `TIER_PRICE`, and the tier rides out on the
    /// deal. A sale is at no tier — the world gives up the lowest first.
    #[test]
    fn a_cart_buys_gear_at_the_tier_its_row_names() {
        let spawn = world::spawn(&worldgen::Galaxy::new(
            1,
            worldgen::GalaxyType::SpiralTwoArm,
        ));
        let session = Session::design(
            12,
            10_000_000,
            1,
            0,
            1,
            0,
            spawn,
            Preset::Playtest,
            800.0,
            600.0,
        );
        let mut cart = Cart::new();
        // Nothing but a gun or a piece of armour comes at a tier, and a
        // row that has none stays at one whatever is asked of it.
        cart.set_tier(ResourceId::Tofu, 3);
        assert_eq!(cart.tier(ResourceId::Tofu), 1);
        assert_eq!(cart.tier(ResourceId::Handgun), 1, "one until it is said");

        cart.add(ResourceId::Handgun, 2);
        let one = session.quote_at(ResourceId::Handgun, 1).unwrap();
        assert_eq!(cart.totals(&session).1, 2 * one.ask);
        assert_eq!(cart.deals(), vec![(ResourceId::Handgun as u32, 2, true, 1)]);

        // At tier two the line is dropped — a tier picked under a line is
        // that line at another price, which nobody asked for — and a fresh
        // one costs four times the book.
        cart.set_tier(ResourceId::Handgun, 2);
        assert_eq!(cart.totals(&session).1, 0, "the line went with the tier");
        cart.add(ResourceId::Handgun, 2);
        let two = session.quote_at(ResourceId::Handgun, 2).unwrap();
        assert_eq!(two.ask, one.ask * economy::TIER_PRICE[2]);
        assert_eq!(cart.totals(&session).1, 2 * two.ask);
        assert_eq!(cart.deals(), vec![(ResourceId::Handgun as u32, 2, true, 2)]);

        // A sale carries tier one and means nothing by it.
        let mut sale = Cart::new();
        sale.set_tier(ResourceId::Handgun, 3);
        sale.add(ResourceId::Handgun, -1);
        assert_eq!(
            sale.deals(),
            vec![(ResourceId::Handgun as u32, 1, false, 1)]
        );
    }
}

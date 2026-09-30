//! The loading screen: a mission's site being built off Bevy's thread.
//!
//! Building a site is seconds of work — a station's decks, a settlement
//! and the plain round it, their nav grids — two at the run's first
//! mission and eight or more for a planet's, in a release build. Done in
//! a frame, the window stopped answering for that long and the desktop
//! offered to kill it. So the two things that build one run here on a
//! thread of their own, with this screen up until they come back:
//!
//! - **the run opening** (`designer::build_run`), at the builder's Start
//!   or the host's `Packet::Start`: the builder stops and the game screen
//!   opens round the session when it is ready;
//! - **a trip** — the order that carries the vote (`World::would_travel`):
//!   the session is taken out of `ShipSession` (a bare one stands in),
//!   the order applied to it on the thread through the same `Net` calls
//!   as ever, and put back. The game screen draws nothing meanwhile and
//!   drains nothing off the wire, so whatever the room says in the
//!   meantime waits in the socket's channel in order; what was already
//!   drained behind the trip waits in [`Loading::stash`]. Lockstep holds:
//!   everything is applied in the order it came, only later.
//!
//! A worker that panics panics here too when it is joined, as the same
//! work did before it had a thread.

use std::thread::JoinHandle;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use ship::Preset;
use ship::Session;
use wire::PeerId;

use super::backdrop::{Backdrop, Backdrops};
use super::builder::Settings;
use super::designer::{Message, Net, Order, ShipSession};
use crate::names::{LOADING_MISSION, LOADING_RUN};
use crate::net::{Event, Online};
use crate::{Screen, theme};

/// What is being built, if anything, and what waits on it.
#[derive(Resource, Default)]
pub struct Loading {
    job: Option<Job>,
    /// An order of this end's own that carries the vote, held from the
    /// frame it was given to the top of the next, where the trip is begun
    /// before anything else happens (`Loading::begin_deferred`).
    deferred: Option<Order>,
    /// The room's events drained behind the trip, for the game screen to
    /// go on with once it is back — before anything newer.
    stash: Vec<Event>,
}

struct Job {
    worker: JoinHandle<Session>,
    kind: Kind,
}

enum Kind {
    /// The run opening, from these settings.
    Run(Box<Settings>),
    /// A trip: the session comes back into `ShipSession`.
    Trip,
}

/// A message applied on the thread, through the `Net` call it would have
/// been applied through here.
pub enum Apply {
    /// This end's own order (`Net::order`).
    Order(Order),
    /// A guest's, arrived at the host (`Net::asked`).
    Asked {
        from: u32,
        at: u64,
        message: Message,
        peer: PeerId,
    },
    /// The host's, arrived at a guest (`Net::applied`).
    Applied {
        from: u32,
        at: u64,
        message: Message,
    },
}

impl Loading {
    /// Whether something is being built: the screens underneath stand
    /// still while it is.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The run opened on a thread: `designer::build_run` there, `open_run` when it is back.
    pub fn start_run(&mut self, commands: &mut Commands, settings: &Settings, size: Vec2) {
        commands.insert_resource(settings.unlocks);
        let s = settings.clone();
        let worker = std::thread::spawn(move || super::designer::build_run(&s, size));
        self.job = Some(Job {
            worker,
            kind: Kind::Run(Box::new(settings.clone())),
        });
    }

    /// Whether `message` from player `from`, applied here now, would be
    /// the trip: only where this end applies it (a game of one, or the
    /// host, or a guest told by the host).
    pub fn travels(session: &Session, from: u32, message: &Message) -> bool {
        let Some(game) = &session.game else {
            return false;
        };
        let command = match *message {
            Message::Order(Order::Propose { star, station }) => world::Command::Propose {
                slot: from,
                star,
                station,
            },
            Message::Order(Order::AcceptTrip(yes)) => world::Command::Accept { slot: from, yes },
            Message::Order(Order::PlayerGone(slot)) => world::Command::PlayerGone { slot },
            _ => return false,
        };
        game.world.would_travel(&command)
    }

    /// This end's own order: applied now, or — when it is the trip, and
    /// this end is the one to apply it — held for the top of the next
    /// frame, which begins the trip with it.
    pub fn order(&mut self, net: &Net, session: &mut Session, order: Order) {
        if net.is_clock()
            && self.deferred.is_none()
            && Self::travels(session, net.slot, &Message::Order(order))
        {
            self.deferred = Some(order);
            return;
        }
        net.order(session, order);
    }

    /// The trip held last frame, begun: true when one was, and the frame
    /// is then the loading screen's.
    pub fn begin_deferred(&mut self, net: &Net, session: &mut Session) -> bool {
        let Some(order) = self.deferred.take() else {
            return false;
        };
        self.trip(net, session, Apply::Order(order));
        true
    }

    /// A trip applied on a thread: the session taken out of the screen's
    /// hands (a bare one left) and `apply` run on it there.
    pub fn trip(&mut self, net: &Net, session: &mut Session, apply: Apply) {
        let mut taken = std::mem::replace(session, stand_in());
        let net = Net {
            slot: net.slot,
            players: net.players,
            wire: net.wire.clone(),
        };
        let worker = std::thread::spawn(move || {
            match apply {
                Apply::Order(order) => {
                    net.order(&mut taken, order);
                }
                Apply::Asked {
                    from,
                    at,
                    message,
                    peer,
                } => net.asked(&mut taken, from, at, message, peer),
                Apply::Applied { from, at, message } => net.applied(&mut taken, from, at, message),
            }
            taken
        });
        self.job = Some(Job {
            worker,
            kind: Kind::Trip,
        });
    }

    /// The events drained behind a trip, kept for when it is back.
    pub fn stash(&mut self, rest: impl IntoIterator<Item = Event>) {
        self.stash.extend(rest);
    }

    /// What was stashed, to go on with before anything newer.
    pub fn take_stash(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.stash)
    }
}

/// What stands in `ShipSession` while the real one is on the thread:
/// an empty yard, which nothing reads, since the game screen is not
/// drawn.
fn stand_in() -> Session {
    Session::design(12, 0, 1, 0, 0, 0, None, Preset::Empty, 64.0, 64.0)
}

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loading>()
            .add_systems(EguiPrimaryContextPass, show);
    }
}

/// While something is being built: the picture, a card saying so and a
/// spinner; the room kept awake. When it is back, the session is handed
/// on — to the game screen opening, or back into `ShipSession`.
#[allow(clippy::too_many_arguments)]
fn show(
    mut contexts: EguiContexts,
    mut loading: ResMut<Loading>,
    session: Option<ResMut<ShipSession>>,
    backdrops: Res<Backdrops>,
    mut online: ResMut<Online>,
    mut commands: Commands,
    mut next: ResMut<NextState<Screen>>,
) -> Result {
    let Some(job) = &loading.job else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    if job.worker.is_finished() {
        let Some(job) = loading.job.take() else {
            return Ok(());
        };
        let built = match job.worker.join() {
            Ok(built) => built,
            Err(panic) => std::panic::resume_unwind(panic),
        };
        match job.kind {
            Kind::Run(settings) => {
                next.set(super::designer::open_run(&mut commands, &settings, built));
            }
            Kind::Trip => {
                if let Some(mut session) = session {
                    session.0 = built;
                }
            }
        }
        ctx.request_repaint();
        return Ok(());
    }
    // Nothing drains the room while this is up; a quiet socket is
    // reaped by the relay, so it is kept talking.
    online.keep_alive(ctx.input(|i| i.time));
    let line = match job.kind {
        Kind::Run(_) => LOADING_RUN,
        Kind::Trip => LOADING_MISSION,
    };
    super::backdrop::paint(&ctx, &backdrops, Backdrop::Start);
    egui::Area::new(egui::Id::new("loading"))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_premultiplied(9, 13, 12, 228))
                .stroke(egui::Stroke::new(1.0, theme::LINE))
                .corner_radius(10.0)
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(22.0));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(line).size(20.0).strong());
                    });
                });
        });
    // The spinner turns, and the worker is looked at again, next frame.
    ctx.request_repaint();
    Ok(())
}

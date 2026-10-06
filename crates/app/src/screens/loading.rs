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
//!   everything is applied in the order it came, only later. The host
//!   tells the room of the order before it builds (`Net::told_first`), so
//!   every end builds its own copy of the site from the run's seed at
//!   once rather than the guests after the host.
//!
//! A worker that panics panics here too when it is joined, as the same
//! work did before it had a thread.
//!
//! The card's **progress bar** counts the work: the worker thread watches
//! a `world::loading::Meter`, and the world ticks off each room of a site
//! into it, weighted by how long a room of that size takes (the three
//! rooms of a site are nearly all of it). Within the room being built the
//! bar moves on at the pace the rooms before it went ([`Bar`]), but never
//! past that room's end. The `end` run docks twice — at the start's
//! station, then at the Heart's fortress — so its bar is two sites long.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use ship::Preset;
use ship::Session;
use wire::PeerId;
use world::loading::{Meter, Reading};

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
    /// Seconds a unit of the meter took, the last time one was measured —
    /// the first room's pace until this load has its own.
    pace: Option<f32>,
}

struct Job {
    worker: JoinHandle<Session>,
    kind: Kind,
    meter: Arc<Meter>,
    /// How many sites the work docks at.
    sites: u32,
    bar: Bar,
}

impl Job {
    fn new(worker: JoinHandle<Session>, kind: Kind, meter: Arc<Meter>, pace: Option<f32>) -> Self {
        let sites = match &kind {
            // The `end` run ties up at the start's station first, then at
            // the fortress (`Session::end_for_probe`).
            Kind::Run(settings) if settings.end => 2,
            _ => 1,
        };
        Job {
            worker,
            kind,
            meter,
            sites,
            bar: Bar::new(pace),
        }
    }
}

/// What the bar has seen of the meter, to move on within a room.
struct Bar {
    seen: Reading,
    /// When the site under way began, and the room under way.
    site_began: Instant,
    room_began: Instant,
    /// Seconds a unit took, from this load's finished rooms, or the last
    /// load's until one is.
    pace: Option<f32>,
    /// The fill last shown: the bar never goes back.
    shown: f32,
}

/// Seconds a unit takes before anything has been measured: a unit is
/// about a tenth of a second in a release build (`world::loading`); the
/// app's own crates are unoptimised in a debug one.
const FIRST_PACE: f32 = if cfg!(debug_assertions) { 0.3 } else { 0.1 };

impl Bar {
    fn new(pace: Option<f32>) -> Self {
        let now = Instant::now();
        Bar {
            seen: Reading::default(),
            site_began: now,
            room_began: now,
            pace,
            shown: 0.0,
        }
    }

    /// The fill, 0 to 1, with the meter reading `r` at `now`, the work
    /// `sites` long: the sites behind, and of this one the units done and
    /// as much of the room under way as the pace says has gone by (at
    /// most 95% of it).
    fn fill(&mut self, r: Reading, sites: u32, now: Instant) -> f32 {
        if r.sites != self.seen.sites {
            self.site_began = now;
            self.room_began = now;
        } else if r.done != self.seen.done {
            if r.done > 0 {
                let took = now.duration_since(self.site_began).as_secs_f32();
                self.pace = Some(took / r.done as f32);
            }
            self.room_began = now;
        } else if r.doing != self.seen.doing {
            self.room_began = now;
        }
        self.seen = r;
        let pace = self.pace.unwrap_or(FIRST_PACE).max(1e-4);
        let creep = if r.doing > 0 {
            let gone = now.duration_since(self.room_began).as_secs_f32() / pace;
            gone.min(0.95 * r.doing as f32)
        } else {
            0.0
        };
        let here = if r.total > 0 {
            ((r.done as f32 + creep) / r.total as f32).min(1.0)
        } else {
            0.0
        };
        let sites = sites.max(1);
        let behind = r.sites.saturating_sub(1).min(sites - 1);
        let fill = (behind as f32 + here) / sites as f32;
        self.shown = self.shown.max(fill);
        self.shown
    }
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
        // The host's tuning, dealt with company: armed before the run is
        // built, and kept over this machine's files for the run.
        match (settings.rewards, settings.weapons) {
            (Some(rewards), Some(weapons)) => {
                weapons.arm();
                commands.insert_resource(crate::wavecfg::Dealt { rewards, weapons });
            }
            _ => commands.remove_resource::<crate::wavecfg::Dealt>(),
        }
        let s = settings.clone();
        let meter = Arc::new(Meter::default());
        let watched = meter.clone();
        let worker = std::thread::spawn(move || {
            world::loading::watch(watched);
            super::designer::build_run(&s, size)
        });
        self.job = Some(Job::new(
            worker,
            Kind::Run(Box::new(settings.clone())),
            meter,
            self.pace,
        ));
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
            ledger: None,
        };
        let meter = Arc::new(Meter::default());
        let watched = meter.clone();
        let worker = std::thread::spawn(move || {
            world::loading::watch(watched);
            match apply {
                Apply::Order(order) => {
                    net.order(&mut taken, order);
                }
                Apply::Asked {
                    from,
                    at,
                    message,
                    peer,
                } => net.asked(&mut taken, from, at, message, peer, None),
                Apply::Applied { from, at, message } => net.applied(&mut taken, from, at, message),
            }
            taken
        });
        self.job = Some(Job::new(worker, Kind::Trip, meter, self.pace));
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

/// While something is being built: the picture, a card saying so with a
/// spinner and a progress bar; the room kept awake. When it is back, the
/// session is handed on — to the game screen opening, or back into
/// `ShipSession`.
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
    if loading.job.is_none() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    if loading
        .job
        .as_ref()
        .is_some_and(|job| job.worker.is_finished())
    {
        let Some(job) = loading.job.take() else {
            return Ok(());
        };
        // The pace this load went at, for the next one's first room.
        if job.bar.pace.is_some() {
            loading.pace = job.bar.pace;
        }
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
    let Some(job) = loading.job.as_mut() else {
        return Ok(());
    };
    // Nothing drains the room while this is up; a quiet socket is
    // reaped by the relay, so it is kept talking.
    online.keep_alive(ctx.input(|i| i.time));
    let line = match job.kind {
        Kind::Run(_) => LOADING_RUN,
        Kind::Trip => LOADING_MISSION,
    };
    let progress = job.bar.fill(job.meter.read(), job.sites, Instant::now());
    super::backdrop::paint(&ctx, &backdrops, Backdrop::Start);
    egui::Area::new(egui::Id::new("loading"))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_premultiplied(10, 10, 12, 228))
                .stroke(egui::Stroke::new(1.0, theme::LINE))
                .corner_radius(10.0)
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(22.0));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(line).size(20.0).strong());
                    });
                    ui.add_space(14.0);
                    bar(ui, progress);
                });
        });
    // The spinner turns, and the worker is looked at again, next frame.
    ctx.request_repaint();
    Ok(())
}

/// The progress bar: a sunk track the card's width, filled `progress` of
/// the way in the accent colour.
fn bar(ui: &mut egui::Ui, progress: f32) {
    let width = ui.min_rect().width().max(260.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), egui::Sense::hover());
    let round = egui::CornerRadius::same(4);
    let painter = ui.painter();
    painter.rect_filled(rect, round, theme::PANEL_DEEP);
    let mut done = rect;
    done.set_width(rect.width() * progress.clamp(0.0, 1.0));
    if done.width() > 0.5 {
        painter.rect_filled(done, round, theme::ACCENT);
    }
    painter.rect_stroke(
        rect,
        round,
        egui::Stroke::new(1.0, theme::LINE),
        egui::StrokeKind::Outside,
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(t0: Instant, secs: f32) -> Instant {
        t0 + Duration::from_secs_f32(secs)
    }

    fn reading(sites: u32, total: u32, done: u32, doing: u32) -> Reading {
        Reading {
            sites,
            total,
            done,
            doing,
        }
    }

    /// The bar is the units done, moves on within a room at the pace the
    /// finished ones went but never past its end, and never goes back.
    #[test]
    fn the_bar_counts_the_rooms_and_creeps_within_one() {
        let t0 = Instant::now();
        let mut bar = Bar::new(None);
        bar.site_began = t0;
        bar.room_began = t0;
        assert_eq!(bar.fill(Reading::default(), 1, t0), 0.0);
        // A site of 20 units, its first room of 8 begun.
        bar.fill(reading(1, 20, 0, 8), 1, t0);
        // At the first pace, 0.1 s a unit in a release build.
        let first = bar.fill(reading(1, 20, 0, 8), 1, at(t0, 4.0 * FIRST_PACE));
        assert!((first - 4.0 / 20.0).abs() < 1e-3, "{first}");
        // Long past the room's size: held just short of its end.
        let held = bar.fill(reading(1, 20, 0, 8), 1, at(t0, 100.0));
        assert!((held - 0.95 * 8.0 / 20.0).abs() < 1e-3, "{held}");
        // The room done at 1.6 s: 8 units done, 0.2 s a unit from now on.
        let done = bar.fill(reading(1, 20, 8, 0), 1, at(t0, 1.6));
        assert!(done >= held);
        assert!((bar.pace.unwrap() - 0.2).abs() < 1e-3);
        bar.fill(reading(1, 20, 8, 12), 1, at(t0, 1.6));
        let mid = bar.fill(reading(1, 20, 8, 12), 1, at(t0, 1.6 + 1.0));
        assert!((mid - 13.0 / 20.0).abs() < 1e-3, "{mid}");
    }

    /// A run of two sites is half done when the first is, and goes on
    /// from there through the second.
    #[test]
    fn two_sites_share_the_bar() {
        let t0 = Instant::now();
        let mut bar = Bar::new(Some(0.1));
        let first = bar.fill(reading(1, 10, 10, 0), 2, t0);
        assert!((first - 0.5).abs() < 1e-6);
        let second = bar.fill(reading(2, 40, 0, 0), 2, at(t0, 0.1));
        assert!((second - 0.5).abs() < 1e-6);
        let later = bar.fill(reading(2, 40, 20, 0), 2, at(t0, 2.0));
        assert!((later - 0.75).abs() < 1e-6);
    }
}

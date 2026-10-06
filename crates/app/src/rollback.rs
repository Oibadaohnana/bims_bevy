//! A guest's own orders played at once, and rolled back (task 156).
//!
//! # What was wrong
//!
//! A guest's world is the host's, step for step (`net.rs`): every order
//! goes to the host, the host applies it and says so, and only then does
//! the guest's copy apply it — so a guest's own Bim answered its keys a
//! whole round trip late, plus the playout buffer's depth (`playout.rs`).
//! Fifty to two hundred milliseconds of walking into a wall is what made a
//! guest's movement feel sluggish.
//!
//! # What a guest does now
//!
//! A guest keeps **two** worlds in a mission. The *confirmed* one is the
//! host's timeline exactly as before — the host's steps and its applied
//! orders, in their order, and the checksum compared on it — kept here as
//! a [`Timeline`]. The *shown* one is the session's, the one drawn and
//! clicked on, and it runs **ahead** of the confirmed one by about a round
//! trip ([`Rollback::lead`]). Every order of the guest's own is applied to
//! the shown world the moment it is given, stamped with the step it was
//! applied at, and asked of the host with that stamp ([`Stamp`]); it is
//! remembered as pending until the host's word on it comes back.
//!
//! The client is authoritative over **when** its orders land: the host
//! holds a stamped ask until its own world reaches the step ([`Held`]) and
//! applies it there, so the host's world takes the guest's order at the
//! very step the guest's own did. An ask that arrives too late for its
//! step is applied at once, and the host tells the asker by how much
//! ([`Asked::early`]), which is what the lead is steered by.
//!
//! The shown world is right for as long as the host's timeline brings
//! nothing it did not guess: the machines are the same deterministic
//! machines on both, so the only thing a guess can get wrong is another
//! player's order, or an order of this player's own landing at another
//! step than it was played at. Whenever the confirmed timeline takes one
//! of those, the shown world is **rolled back**: thrown away, copied
//! afresh from the confirmed one ([`ship::game::Game::restore`]), and
//! stepped back up to where it was with the pending orders put back at
//! their steps.
//!
//! # Heard once
//!
//! What a world puts out for the picture and the speakers — its events,
//! its rooms' cues and particles — is kept by the step it came with
//! ([`Heard`]; an order played between two steps is the later one's).
//! The shown world's is heard as it plays; what the confirmed world puts
//! out, and what a rollback puts out playing a step again, is heard only
//! where it differs from what was heard at that step, one for one
//! ([`Output::unheard`], `Game::drop_heard`). So another player's shot is
//! heard on the guest once, whether it came with the host's word or with
//! the replay after it, and this player's own is not heard twice.
//!
//! The host's world is still the world. What ends a run is read off the
//! confirmed timeline ([`Rollback::confirmed`]); a trip, a load and the
//! host's world arriving put the guest back on the host's alone, and
//! between missions (where nothing steps) there is nothing to guess and
//! the guest plays the host's timeline as before.
//!
//! # What it costs
//!
//! A rollback is a copy of the world — a millisecond or two, the rooms'
//! sight being most of it — and a replay of the lead's steps, a fraction
//! of a millisecond each: four milliseconds in a fight of forty on a
//! desktop, nine at the most. Another player's aim moving is an order,
//! twenty a second a player, and a rollback for each stalled a guest's
//! frames and its sound with them. So another player's walk and aim are
//! played on the shown world as they come, and rolled back for at most
//! every [`DRIFT_STEPS`]; anything else they do, and this player's own
//! order landing at another step, still at once. `BIMS_ROLLBACK=0` is the
//! old way, for comparing.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bims::order::CrewOrder;
use ship::Session;
use ship::game::{Output, Timeline};
use wire::PeerId;
use world::{World, WorldEvent};

use crate::screens::designer::{Message, Net, Order};
use crate::screens::loading::Loading;

/// Steps ahead of the confirmed world aimed at before the host has said
/// anything: a hundred milliseconds at 1×.
pub const FIRST_LEAD: f64 = 6.0;
/// The least and the most the lead is let be, in steps: half a second is
/// further than a guess of the others' keys is worth.
pub const MIN_LEAD: f64 = 1.0;
pub const MAX_LEAD: f64 = 30.0;
/// How early an ask is to reach the host, in steps: a frame and a bit of
/// a line's wander, so a steady line lands every order at its step.
pub const MARGIN: f64 = 2.0;
/// How much of what it did not need the lead lets go at each answer: a
/// late answer raises it at once, an early one lowers it slowly.
pub const EASE_DOWN: f64 = 0.05;
/// How far past the confirmed world the shown one may run, in steps,
/// beyond the lead: a host that has gone quiet stops the guest's world
/// there rather than letting it run off on its guesses.
pub const RUN_ON: f64 = 30.0;
/// How long the distance ahead is followed over, in seconds, for the
/// pace: the host's steps come a frame's worth at a time, give or take.
pub const SMOOTH: f64 = 0.5;
/// The pace's give: this many seconds off the lead is twice the pace.
pub const CATCH: f64 = 0.5;
/// The slowest and the fastest the shown world is played to keep its lead.
pub const SLOWEST: f64 = 0.9;
pub const FASTEST: f64 = 1.25;
/// How far ahead of the host's world an ask may be stamped, in steps,
/// before the host stops waiting for it: a stamp from a world that
/// diverged, or from before a load.
pub const MAX_HOLD: u64 = 120;
/// How many steps what was heard is kept for: more than the shown world
/// ever runs ahead.
pub const HEARD_STEPS: usize = 240;
/// Steps of the shown world at the least between two rollbacks asked for
/// by nothing but the others' walk and aim — a fifth of a second at 1×.
/// A player's control order goes twenty times a second (task 144's
/// `CONTROL_EVERY`), and a rollback for each, a copy of the world and the
/// lead's steps played again, was most of a guest's frames with company:
/// its frames stalled and its sound with them. Those orders are played
/// on the shown world the moment they come instead, a few steps late, and
/// put right by the next rollback.
pub const DRIFT_STEPS: u64 = 12;

/// What a guest's ask carries of its own guess: the step its own world
/// applied it at, and its number among this player's guesses.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Stamp {
    pub step: u64,
    pub seq: u32,
}

/// What the host's `Applied` says of a stamped ask: its number, and how
/// many steps before the host's world reached the stamp it arrived —
/// negative when it came too late and was applied later than played.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Asked {
    pub seq: u32,
    pub early: i32,
}

/// One of this player's orders, applied to the shown world and asked of
/// the host, whose word has not come back.
#[derive(Clone, Copy, Debug)]
struct Pending {
    seq: u32,
    step: u64,
    at: u64,
    message: Message,
    /// How far the shown world was past the confirmed one when it went.
    ahead: u64,
}

/// What `Net` and the screen share: whether this end is guessing, and
/// the guesses out. `Net` stamps an order into it as it goes
/// ([`predict`]); the screen reads the answers off it.
#[derive(Default, Debug)]
pub struct Ledger {
    on: bool,
    next: u32,
    pending: Vec<Pending>,
    /// The confirmed world's step when the screen last looked.
    confirmed_at: u64,
    /// What the orders played since the screen last looked put out, by
    /// the step they came before, for [`Heard`].
    heard: Vec<(u64, Output)>,
}

pub type Shared = Arc<Mutex<Ledger>>;

/// An order of this end's own, played on the shown world at once and
/// stamped, when this end is guessing (`Net::send`, a guest's): `None`
/// when it is not — off, no world, an edit rather than an order, or the
/// trip, which is the host's to set off — and the ask goes unstamped, to
/// be applied when the host says so.
pub fn predict(
    ledger: &Shared,
    session: &mut Session,
    slot: u32,
    at: u64,
    message: Message,
) -> Option<Stamp> {
    if !matches!(message, Message::Order(_)) {
        return None;
    }
    let mut ledger = ledger.lock().ok()?;
    if !ledger.on || Loading::travels(session, slot, &message) {
        return None;
    }
    let game = session.game.as_mut()?;
    let step = game.world.steps;
    let mark = game.out_mark();
    let seq = ledger.next;
    ledger.next = ledger.next.wrapping_add(1);
    Net::receive(session, slot, at, message);
    if let Some(game) = session.game.as_mut() {
        let out = game.out_since(mark);
        ledger.heard.push((step + 1, out));
    }
    let ahead = step.saturating_sub(ledger.confirmed_at);
    ledger.pending.push(Pending {
        seq,
        step,
        at,
        message,
        ahead,
    });
    Some(Stamp { step, seq })
}

/// What the shown world has put out, by the step it came with: a step's
/// own, and an order played before it. Kept for the steps past the
/// confirmed world's, which a rollback may play again.
#[derive(Default)]
struct Heard {
    steps: VecDeque<(u64, Output)>,
}

impl Heard {
    fn at(&self, step: u64) -> Option<&Output> {
        self.steps.iter().find(|(s, _)| *s == step).map(|(_, o)| o)
    }

    /// `out` heard at `step`, added to what was.
    fn add(&mut self, step: u64, out: Output) {
        match self.steps.iter_mut().find(|(s, _)| *s == step) {
            Some((_, have)) => have.merge(out),
            None => {
                let at = self.steps.partition_point(|(s, _)| *s < step);
                self.steps.insert(at, (step, out));
                while self.steps.len() > HEARD_STEPS {
                    self.steps.pop_front();
                }
            }
        }
    }

    /// Steps the confirmed world has taken, never played again.
    fn forget_through(&mut self, step: u64) {
        while self.steps.front().is_some_and(|(s, _)| *s <= step) {
            self.steps.pop_front();
        }
    }
}

/// How the guessing went, for `BIMS_AUTO`'s printout and the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tally {
    /// Times the shown world was thrown away and stepped up again.
    pub rollbacks: u32,
    /// Steps stepped again doing it.
    pub replayed: u64,
    /// This player's orders the host applied at their stamp, and later.
    pub on_time: u32,
    pub late: u32,
}

/// The guest's side, kept by the game screen.
#[derive(Default)]
pub struct Rollback {
    pub ledger: Shared,
    /// The host's timeline while this end runs ahead of it — or, between
    /// [`Rollback::enter`] and [`Rollback::leave`], the shown world stood
    /// aside while the host's steps and orders go to the session's.
    other: Option<Timeline>,
    inside: bool,
    /// The shown world's unread events, kept aside while the confirmed
    /// one is in the session.
    events: Vec<WorldEvent>,
    /// What the confirmed world put out this frame that the shown world
    /// had not, to be heard on it.
    news: Output,
    /// The confirmed timeline took something the shown one had not
    /// played at that step.
    parted: bool,
    /// It took another player's walk or aim, which waits for a rollback
    /// until the shown world is [`DRIFT_STEPS`] past the last one.
    drifted: bool,
    /// Those orders, from whom and as said, to be played on the shown
    /// world this frame if it is not rolled back.
    drift: Vec<(u32, u64, Message)>,
    /// The shown world's step when it was last rolled back, or guessing
    /// began.
    rolled_at: u64,
    /// Steps ahead of the confirmed world aimed at; [`FIRST_LEAD`] when
    /// guessing begins.
    lead: f64,
    /// How far ahead the shown world is, followed over [`SMOOTH`].
    ahead: f64,
    /// Steps owed to the shown world, a fraction carried.
    backlog: f64,
    heard: Heard,
    pub tally: Tally,
}

impl Rollback {
    /// Whether the shown world is running ahead of the host's.
    pub fn active(&self) -> bool {
        self.other.is_some()
    }

    /// The steps ahead aimed at.
    pub fn lead(&self) -> f64 {
        self.lead
    }

    /// How far the shown world's clock is into its next step, nought to
    /// one: what the picture blends the bodies by between two steps.
    pub fn fraction(&self) -> f64 {
        self.backlog.clamp(0.0, 1.0)
    }

    /// The host's world as this end has it: the confirmed timeline while
    /// guessing, else `None` — the session's world is the host's then.
    pub fn confirmed(&self) -> Option<&World> {
        if self.inside {
            return None;
        }
        self.other.as_ref().map(|t| &t.world)
    }

    /// How many orders of this player's own are out.
    pub fn pending(&self) -> usize {
        self.ledger.lock().map_or(0, |l| l.pending.len())
    }

    /// What the orders played since last asked put out, into [`Heard`].
    fn hear_orders(&mut self) {
        let played = self
            .ledger
            .lock()
            .map(|mut l| std::mem::take(&mut l.heard))
            .unwrap_or_default();
        for (step, out) in played {
            self.heard.add(step, out);
        }
    }

    /// The confirmed world into the session, the shown one aside: what
    /// the host says this frame — its steps, its orders, its checksum —
    /// is put to the host's timeline the way it always was.
    pub fn enter(&mut self, session: &mut Session) {
        self.hear_orders();
        let (Some(other), Some(game)) = (self.other.as_mut(), session.game.as_mut()) else {
            return;
        };
        hand_dials(&game.world, &mut other.world);
        self.events = std::mem::take(&mut game.events);
        game.swap(other);
        self.inside = true;
    }

    /// Something the host said, put to the session's world by `apply` —
    /// an order applied, a step taken. Between `enter` and `leave` that
    /// is the confirmed world, and what it puts out that the shown world
    /// did not at that step is kept to be heard (the rest was). Outside
    /// them it is the only world, and heard as ever.
    pub fn confirm(&mut self, session: &mut Session, apply: impl FnOnce(&mut Session)) {
        let Some(game) = session.game.as_mut().filter(|_| self.inside) else {
            apply(session);
            return;
        };
        let step = game.world.steps + 1;
        let mark = game.out_mark();
        apply(session);
        let Some(game) = session.game.as_mut() else {
            return;
        };
        let out = game.out_since(mark);
        let news = match self.heard.at(step) {
            Some(heard) => out.unheard(heard),
            None => out,
        };
        self.news.extend(news);
    }

    /// The host applied an order: from player `from` (a slot), said at
    /// `at`, with what it said of a stamped ask, at `step` of the
    /// confirmed world. Called after it was applied to the session's,
    /// between `enter` and `leave`.
    pub fn applied(
        &mut self,
        me: u32,
        from: u32,
        at: u64,
        message: Message,
        asked: Option<Asked>,
        step: u64,
    ) {
        if !self.inside {
            return;
        }
        let Ok(mut ledger) = self.ledger.lock() else {
            return;
        };
        // Somebody else's walk and aim: played on the shown world at once
        // and rolled back for now and then ([`DRIFT_STEPS`]).
        if from != me
            && matches!(
                message,
                Message::Order(Order::Crew(CrewOrder::Control { .. }))
            )
        {
            self.drifted = true;
            self.drift.push((from, at, message));
            return;
        }
        let mine = (from == me).then_some(asked).flatten();
        let Some(asked) = mine else {
            // Somebody else's, or one of this player's never played here:
            // nothing the shown world guessed.
            self.parted = true;
            return;
        };
        let Some(i) = ledger.pending.iter().position(|p| p.seq == asked.seq) else {
            self.parted = true;
            return;
        };
        let p = ledger.pending[i];
        // Anything older is the host's to have applied already: an order
        // is never refused, so it went in another frame.
        ledger.pending.drain(..=i);
        if i != 0 || p.step != step {
            self.parted = true;
        }
        // The lead it would have taken to reach the host `MARGIN` early:
        // raised to it at once, let down to it slowly.
        let needed = (p.ahead as f64 + MARGIN - f64::from(asked.early)).clamp(MIN_LEAD, MAX_LEAD);
        if needed > self.lead {
            self.lead = needed;
        } else {
            self.lead += (needed - self.lead) * EASE_DOWN;
        }
        if asked.early < 0 {
            self.tally.late += 1;
        } else {
            self.tally.on_time += 1;
        }
    }

    /// The host's word for this frame is in. With `keep`, this end
    /// guesses on: the shown world back into the session — rolled back
    /// first if the confirmed one parted from it — or, not guessing yet,
    /// a copy of the session's taken as the confirmed timeline and the
    /// guessing begun. Without, the guessing stops and the session's
    /// world is the host's alone, as it was before task 156: a trip
    /// taken, a world arrived, the host gone, or the mission over.
    pub fn leave(&mut self, session: &mut Session, me: u32, keep: bool) {
        if !self.inside {
            if keep
                && self.other.is_none()
                && let Some(game) = session.game.as_ref()
            {
                self.other = Some(game.fork());
                self.parted = false;
                self.drifted = false;
                self.drift.clear();
                self.rolled_at = game.world.steps;
                self.lead = FIRST_LEAD;
                self.ahead = 0.0;
                self.backlog = 0.0;
                if let Ok(mut ledger) = self.ledger.lock() {
                    ledger.on = true;
                    ledger.pending.clear();
                    ledger.heard.clear();
                    ledger.confirmed_at = game.world.steps;
                }
            } else if !keep && self.other.is_some() {
                self.stop();
            }
            return;
        }
        self.inside = false;
        if !keep {
            // The session's is the confirmed world already, and what it
            // put out this frame is heard as it was before any guessing.
            self.stop();
            return;
        }
        let (Some(other), Some(game)) = (self.other.as_mut(), session.game.as_mut()) else {
            self.stop();
            return;
        };
        // What the confirmed world put out is heard by the news alone.
        game.silence();
        game.swap(other);
        let confirmed = other.world.steps;
        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.confirmed_at = confirmed;
        }
        self.hear_orders();
        self.heard.forget_through(confirmed);
        // The others' walk and aim alone wait their turn; a world not
        // running has no steps to play again, and is put right at once.
        let drift_due = self.drifted
            && (game.world.effective_speed().multiplier() == 0
                || game.world.steps >= self.rolled_at + DRIFT_STEPS);
        if self.parted || drift_due || game.world.steps < confirmed {
            let _timed = crate::perf::scope(crate::perf::Phase::Rollback);
            self.drift.clear();
            self.roll_back(session, me);
        } else {
            for (from, at, message) in std::mem::take(&mut self.drift) {
                self.again(session, |s| {
                    Net::receive(s, from, at, message);
                });
            }
        }
        if let Some(game) = session.game.as_mut() {
            let news = std::mem::take(&mut self.news);
            let mut events = std::mem::take(&mut self.events);
            events.extend(news.events.iter().copied());
            events.append(&mut game.events);
            game.events = events;
            game.put_back(news);
        }
    }

    /// Guessing over: the shown world and the guesses dropped.
    fn stop(&mut self) {
        self.other = None;
        self.inside = false;
        self.parted = false;
        self.drifted = false;
        self.drift.clear();
        self.events.clear();
        self.news = Output::default();
        self.heard = Heard::default();
        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.on = false;
            ledger.pending.clear();
            ledger.heard.clear();
        }
    }

    /// The shown world thrown away and copied afresh from the confirmed
    /// one, then stepped back up to where it stood with this player's
    /// orders still out put back at their steps. What it puts out playing
    /// them again is heard only where it was not the first time; what the
    /// world thrown away had put out and nobody had taken yet is put back
    /// in front. A world paused on the way stops there, the orders after
    /// it applied where it stopped.
    fn roll_back(&mut self, session: &mut Session, me: u32) {
        let (Some(other), Some(game)) = (self.other.as_ref(), session.game.as_mut()) else {
            return;
        };
        let target = game.world.steps.max(other.world.steps);
        let unread = game.take_unread();
        game.restore(other);
        self.parted = false;
        self.drifted = false;
        self.rolled_at = target;
        self.tally.rollbacks += 1;
        let pending: Vec<Pending> = self
            .ledger
            .lock()
            .map(|l| l.pending.clone())
            .unwrap_or_default();
        let steps = |s: &Session| s.game.as_ref().map_or(0, |g| g.world.steps);
        let running = |s: &Session| {
            s.game
                .as_ref()
                .is_some_and(|g| g.world.effective_speed().multiplier() > 0)
        };
        for p in &pending {
            while steps(session) < p.step.min(target) && running(session) {
                self.again(session, |s| s.world_step());
                self.tally.replayed += 1;
            }
            self.again(session, |s| {
                Net::receive(s, me, p.at, p.message);
            });
        }
        while steps(session) < target && running(session) {
            self.again(session, |s| s.world_step());
            self.tally.replayed += 1;
        }
        if let Some(game) = session.game.as_mut() {
            game.put_back(unread);
        }
    }

    /// A step or an order played again in a rollback: what it puts out
    /// that was heard at that step dropped, the rest kept to be heard and
    /// added to what was.
    fn again(&mut self, session: &mut Session, apply: impl FnOnce(&mut Session)) {
        let Some(game) = session.game.as_mut() else {
            return;
        };
        let step = game.world.steps + 1;
        let mark = game.out_mark();
        apply(session);
        let Some(game) = session.game.as_mut() else {
            return;
        };
        let out = game.out_since(mark);
        if let Some(heard) = self.heard.at(step) {
            game.drop_heard(mark, heard);
        }
        self.heard.add(step, out);
    }

    /// One step of the shown world played for the first time, what it
    /// puts out remembered against a rollback playing it again.
    pub fn step_shown(&mut self, session: &mut Session) {
        let Some(game) = session.game.as_mut() else {
            return;
        };
        if !self.active() {
            game.step();
            return;
        }
        let mark = game.out_mark();
        game.step();
        let step = game.world.steps;
        let out = game.out_since(mark);
        self.heard.add(step, out);
    }

    /// How many steps the shown world takes this frame: the world's pace
    /// at its own speed, a little slower or faster to hold the lead, and
    /// none past [`RUN_ON`] beyond it — a host gone quiet is waited for.
    pub fn pace(&mut self, session: &Session, dt: f64) -> u32 {
        let (Some(other), Some(game)) = (self.other.as_ref(), session.game.as_ref()) else {
            return 0;
        };
        let times = game.world.effective_speed().multiplier();
        if times == 0 {
            self.backlog = 0.0;
            return 0;
        }
        let rate = session.steps_per_second() * f64::from(times);
        let ahead = game.world.steps as f64 - other.world.steps as f64;
        self.ahead += (ahead - self.ahead) * (dt / SMOOTH).min(1.0);
        let off = self.lead - self.ahead;
        let pace = (1.0 + off / (rate * CATCH)).clamp(SLOWEST, FASTEST);
        self.backlog += dt * rate * pace;
        let mut n = self.backlog.floor().max(0.0);
        self.backlog -= n;
        let room = (self.lead + RUN_ON - ahead).floor().max(0.0);
        if n > room {
            n = room;
            self.backlog = 0.0;
        }
        n as u32
    }

    /// The confirmed world's rooms' passing lights aged with the shown
    /// one's, so a rollback does not bring back a flash long gone.
    pub fn age(&mut self, real: f32) {
        if self.inside {
            return;
        }
        if let Some(other) = self.other.as_mut() {
            other.world.aboard.room.fade(real);
            if let Some(residents) = other.world.residents.as_mut() {
                residents.aboard.room.fade(real);
            }
        }
    }
}

/// The tuning files' dials (`wavecfg`) of `from` handed to `to` where they
/// differ: they are neither saved nor hashed, and the app hands them to
/// the session's world alone. A guest's copy of the host's timeline takes
/// them from the world shown, and a world arrived over the wire from the
/// one it replaces — else a mission's world come mid-mission (a Retry, a
/// resync) lays its waves by the constants on the guest alone, and parts
/// from the host's at the first.
pub fn hand_dials(from: &World, to: &mut World) {
    if to.wave_scaling() != from.wave_scaling() {
        to.set_wave_scaling(from.wave_scaling());
    }
    if to.rewards() != from.rewards() {
        to.set_rewards(from.rewards());
    }
}

/// A guest's ask the host holds for a step its world has not reached.
#[derive(Clone, Copy, Debug)]
pub struct HeldAsk {
    pub from: u32,
    pub peer: PeerId,
    pub at: u64,
    pub message: Message,
    pub stamp: Stamp,
    /// What the host says of it when it is applied.
    pub asked: Asked,
}

/// The host's side: the guests' stamped asks, held until its world
/// reaches their steps, in the order they are to go — by step, and in
/// the order they came within one.
#[derive(Default, Debug)]
pub struct Held {
    asks: Vec<HeldAsk>,
}

impl Held {
    /// An ask arrived at step `now` of the host's world: back at once to
    /// be applied if its step is here or gone, if it is no stamped ask,
    /// if the world is not running or if it is stamped implausibly far
    /// ahead; else held.
    #[allow(clippy::too_many_arguments)]
    pub fn arrive(
        &mut self,
        from: u32,
        peer: PeerId,
        at: u64,
        message: Message,
        stamp: Option<Stamp>,
        now: u64,
        running: bool,
    ) -> Option<Option<Asked>> {
        let Some(stamp) = stamp else {
            return Some(None);
        };
        let early = stamp.step as i64 - now as i64;
        let asked = Asked {
            seq: stamp.seq,
            early: early.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        };
        if early <= 0 || !running || stamp.step > now + MAX_HOLD {
            return Some(Some(asked));
        }
        self.asks.push(HeldAsk {
            from,
            peer,
            at,
            message,
            stamp,
            asked,
        });
        None
    }

    /// The asks due at step `now` — stamped for it or before — in order.
    pub fn due(&mut self, now: u64) -> Vec<HeldAsk> {
        if self.asks.iter().all(|a| a.stamp.step > now) {
            return Vec::new();
        }
        let (mut due, rest): (Vec<HeldAsk>, Vec<HeldAsk>) =
            self.asks.drain(..).partition(|a| a.stamp.step <= now);
        self.asks = rest;
        due.sort_by_key(|a| a.stamp.step);
        due
    }

    /// Every ask held, in order: the world stopped, and nothing will
    /// reach their steps.
    pub fn all(&mut self) -> Vec<HeldAsk> {
        self.due(u64::MAX)
    }

    pub fn is_empty(&self) -> bool {
        self.asks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Packet, Wire};
    use crate::screens::designer::Order;
    use bims::order::CrewOrder;
    use ship::Preset;
    use std::collections::VecDeque;
    use std::sync::mpsc::{Receiver, channel};
    use wire::{ClientCtl, To, decode};

    const HOST: usize = 0;
    const GUEST: usize = 1;
    /// The host's peer id, and the guest's, as the host's screen keeps them.
    const GUEST_PEER: PeerId = 2;

    /// One end: its session and seam, what it posts, what is on its way
    /// to it — each packet with the frame it lands on — and its halves of
    /// the scheme, as the game screen keeps them.
    struct End {
        session: Session,
        net: Net,
        rx: Receiver<ClientCtl>,
        inbox: VecDeque<(u32, Packet)>,
        rollback: Rollback,
        held: Held,
    }

    /// A host and a guest in one world, the combat ship's, opened by both
    /// accepting the way the yard opens it.
    fn pair() -> [End; 2] {
        let spawn = ship::session::pick_dock(world::data::DEFAULT_SEED, 0, 0);
        let mut ends: Vec<End> = (0..2u32)
            .map(|slot| {
                let mut session = Session::design(
                    shipdesign::fixture::AREA,
                    100_000,
                    2,
                    slot,
                    world::data::DEFAULT_SEED,
                    0,
                    spawn,
                    Preset::Empty,
                    800.0,
                    600.0,
                );
                session.editor.give(shipdesign::fixture::combat_ship());
                let (tx, rx) = channel();
                let rollback = Rollback::default();
                End {
                    session,
                    net: Net {
                        slot,
                        players: 2,
                        wire: Some(Wire::for_test(tx, slot == 0)),
                        ledger: Some(rollback.ledger.clone()),
                    },
                    rx,
                    inbox: VecDeque::new(),
                    rollback,
                    held: Held::default(),
                }
            })
            .collect();
        for end in ends.iter_mut() {
            for slot in 0..2 {
                let at = end.session.editor.hash();
                Net::receive(&mut end.session, slot, at, Message::Accept(true));
            }
            assert!(end.session.playing());
        }
        let [host, guest]: [End; 2] = ends.try_into().ok().unwrap();
        [host, guest]
    }

    /// What each end posted, on its way to the other, landing `delay`
    /// frames on.
    fn post(ends: &mut [End; 2], frame: u32, delay: u32) {
        for from in [HOST, GUEST] {
            let mut out = Vec::new();
            while let Ok(ClientCtl::Relay { to, payload }) = ends[from].rx.try_recv() {
                assert!(from == HOST || to == To::Host);
                out.push(decode::<Packet>(&payload).expect("a packet"));
            }
            for packet in out {
                ends[1 - from].inbox.push_back((frame + delay, packet));
            }
        }
    }

    fn steps(end: &End) -> u64 {
        end.session.game.as_ref().map_or(0, |g| g.world.steps)
    }

    /// The host's steps since `said`, said — `game.rs`'s `say_steps`.
    fn say(end: &End, said: &mut u64) {
        let now = steps(end);
        if now > *said {
            let checksum = Some(world::world_checksum(
                &end.session.game.as_ref().unwrap().world,
            ));
            let n = (now - *said) as u32;
            end.net
                .wire
                .as_ref()
                .unwrap()
                .send(To::All, &Packet::Steps { n, checksum });
            *said = now;
        }
    }

    /// The host's frame as the game screen plays it: the guest's asks
    /// in, held to their steps, and `n` steps taken, said around the
    /// held asks that go between them.
    fn host_frame(end: &mut End, frame: u32, n: u32) {
        while end.inbox.front().is_some_and(|(due, _)| *due <= frame) {
            let (_, packet) = end.inbox.pop_front().unwrap();
            if let Packet::Ask { at, message, stamp } = packet {
                let now = steps(end);
                if let Some(asked) = end
                    .held
                    .arrive(1, GUEST_PEER, at, message, stamp, now, true)
                {
                    end.net
                        .asked(&mut end.session, 1, at, message, GUEST_PEER, asked);
                }
            }
        }
        let mut said = steps(end);
        for _ in 0..n {
            let due = end.held.due(steps(end));
            if !due.is_empty() {
                say(end, &mut said);
                for ask in due {
                    end.net.asked(
                        &mut end.session,
                        ask.from,
                        ask.at,
                        ask.message,
                        ask.peer,
                        Some(ask.asked),
                    );
                }
            }
            end.session.world_step();
        }
        say(end, &mut said);
    }

    /// The guest's frame: the host's word put to the confirmed world —
    /// its checksum compared there — the shown one rolled back if they
    /// parted, then stepped at its own pace.
    fn guest_frame(end: &mut End, frame: u32) {
        end.rollback.enter(&mut end.session);
        while end.inbox.front().is_some_and(|(due, _)| *due <= frame) {
            let (_, packet) = end.inbox.pop_front().unwrap();
            match packet {
                Packet::Applied {
                    from,
                    at,
                    message,
                    asked,
                } => {
                    let net = &end.net;
                    end.rollback
                        .confirm(&mut end.session, |s| net.applied(s, from, at, message));
                    let step = steps(end);
                    end.rollback.applied(1, from, at, message, asked, step);
                }
                Packet::Steps { n, checksum } => {
                    for _ in 0..n {
                        end.rollback.confirm(&mut end.session, |s| s.world_step());
                    }
                    let mine = world::world_checksum(&end.session.game.as_ref().unwrap().world);
                    assert_eq!(checksum, Some(mine), "the confirmed world is the host's");
                }
                _ => {}
            }
        }
        end.rollback.leave(&mut end.session, 1, true);
        let dt = 1.0 / end.session.steps_per_second();
        for _ in 0..end.rollback.pace(&end.session, dt) {
            end.rollback.step_shown(&mut end.session);
        }
    }

    fn walk(angle: Option<u16>) -> Order {
        Order::Crew(CrewOrder::Control {
            walk: angle,
            aim: 0,
            fire: false,
            sprint: false,
        })
    }

    fn bim(end: &End, who: usize) -> bims::math::Vec2 {
        end.session.room_ref().unwrap().bim_pos(who)
    }

    /// The guest's shown world and the host's, stepped to the same step
    /// with nothing more to come: the same world, if every guess was
    /// right or put right.
    fn same_world(ends: &mut [End; 2]) {
        let (h, g) = (steps(&ends[HOST]), steps(&ends[GUEST]));
        for _ in h..g {
            ends[HOST].session.world_step();
        }
        for _ in g..h {
            ends[GUEST].session.world_step();
        }
        let sum = |e: &End| world::world_checksum(&e.session.game.as_ref().unwrap().world);
        assert_eq!(sum(&ends[HOST]), sum(&ends[GUEST]));
        for who in 0..2 {
            assert_eq!(bim(&ends[HOST], who), bim(&ends[GUEST], who), "Bim {who}");
        }
    }

    /// The point of it: a guest's own walk shows on its deck the frame it
    /// is given, the host applies it at the step the guest played it at
    /// once the lead has found the line, and the two worlds stay one.
    #[test]
    fn a_guest_walks_at_once_and_the_host_takes_it_at_the_step_it_was_played() {
        let mut ends = pair();
        // Five frames each way: a round trip of a sixth of a second.
        let delay = 5;
        let mut frame = 0;
        let mut run = |ends: &mut [End; 2], frames: u32, order: Option<Order>| {
            for i in 0..frames {
                frame += 1;
                host_frame(&mut ends[HOST], frame, 1);
                guest_frame(&mut ends[GUEST], frame);
                if i == 0
                    && let Some(order) = order
                {
                    let e = &mut ends[GUEST];
                    e.net.order(&mut e.session, order);
                }
                post(ends, frame, delay);
            }
        };
        run(&mut ends, 60, None);
        assert!(ends[GUEST].rollback.active());
        // Walking right and stopping, again and again: every order is
        // played on the guest's deck before the host has heard of it.
        for round in 0..6 {
            let before = bim(&ends[GUEST], 1);
            let host_before = bim(&ends[HOST], 1);
            run(&mut ends, 1, Some(walk(Some(0))));
            run(&mut ends, 1, None);
            assert_ne!(
                bim(&ends[GUEST], 1),
                before,
                "round {round}: walked at once"
            );
            assert_eq!(
                bim(&ends[HOST], 1),
                host_before,
                "the host has not heard yet"
            );
            run(&mut ends, 20, None);
            run(&mut ends, 20, Some(walk(None)));
        }
        run(&mut ends, 40, None);
        let r = &ends[GUEST].rollback;
        assert_eq!(r.pending(), 0);
        assert!(r.tally.on_time >= 8, "{:?}", r.tally);
        assert!(r.lead() >= MIN_LEAD && r.lead() <= MAX_LEAD);
        // Once the lead was found, the guest's own orders landed on time
        // and asked for no rollback: the first few, before it, did.
        assert!(r.tally.rollbacks <= r.tally.late, "{:?}", r.tally);
        same_world(&mut ends);
    }

    /// Another player's orders are what the guest cannot guess: each one
    /// rolls its world back, and it comes out the host's.
    #[test]
    fn the_host_s_orders_roll_the_guest_back_onto_the_host_s_world() {
        let mut ends = pair();
        let delay = 4;
        let mut frame = 0;
        for i in 0..240u32 {
            frame += 1;
            if i % 30 == 10 {
                let e = &mut ends[HOST];
                let angle = if i % 60 == 10 { Some(16_384) } else { None };
                e.net.order(&mut e.session, walk(angle));
            }
            // And the guest's own now and then, crossing the host's.
            if i % 45 == 20 {
                let e = &mut ends[GUEST];
                let angle = if i % 90 == 20 { Some(32_768) } else { None };
                e.net.order(&mut e.session, walk(angle));
            }
            host_frame(&mut ends[HOST], frame, 1);
            guest_frame(&mut ends[GUEST], frame);
            post(&mut ends, frame, delay);
        }
        for _ in 0..30 {
            frame += 1;
            host_frame(&mut ends[HOST], frame, 1);
            guest_frame(&mut ends[GUEST], frame);
            post(&mut ends, frame, delay);
        }
        let r = &ends[GUEST].rollback;
        assert!(r.tally.rollbacks >= 8, "{:?}", r.tally);
        assert!(r.tally.replayed > 0);
        same_world(&mut ends);
    }

    /// Another player's aim swung every frame is a rollback at most every
    /// `DRIFT_STEPS` on the guest, not one a frame, and the guest's world
    /// still comes out the host's.
    #[test]
    fn the_host_s_aim_rolls_the_guest_back_only_now_and_then() {
        let mut ends = pair();
        let delay = 4;
        let mut frame = 0;
        let mut step = |ends: &mut [End; 2], aim: Option<u16>| {
            frame += 1;
            if let Some(aim) = aim {
                let e = &mut ends[HOST];
                let order = Order::Crew(CrewOrder::Control {
                    walk: None,
                    aim,
                    fire: false,
                    sprint: false,
                });
                e.net.order(&mut e.session, order);
            }
            host_frame(&mut ends[HOST], frame, 1);
            guest_frame(&mut ends[GUEST], frame);
            post(ends, frame, delay);
        };
        for _ in 0..60 {
            step(&mut ends, None);
        }
        let before = ends[GUEST].rollback.tally.rollbacks;
        let frames = 120u16;
        for i in 0..frames {
            step(&mut ends, Some(i * 500));
        }
        for _ in 0..30 {
            step(&mut ends, None);
        }
        let r = &ends[GUEST].rollback;
        let rolled = r.tally.rollbacks - before;
        assert!(rolled >= 2, "{:?}", r.tally);
        assert!(
            u64::from(rolled) <= u64::from(frames) / DRIFT_STEPS + 2,
            "{rolled} rollbacks for {frames} aims"
        );
        same_world(&mut ends);
    }

    /// The host's hold: an ask stamped ahead waits for its step, one
    /// stamped behind or for a stopped world goes at once, and those due
    /// together go by step and then in the order they came.
    #[test]
    fn the_host_holds_a_stamped_ask_to_its_step() {
        let message = Message::Accept(true);
        let stamp = |step, seq| Some(Stamp { step, seq });
        let mut held = Held::default();
        assert_eq!(held.arrive(1, 9, 0, message, None, 50, true), Some(None));
        let late = held.arrive(1, 9, 0, message, stamp(48, 0), 50, true);
        assert_eq!(late, Some(Some(Asked { seq: 0, early: -2 })));
        let stopped = held.arrive(1, 9, 0, message, stamp(60, 1), 50, false);
        assert_eq!(stopped, Some(Some(Asked { seq: 1, early: 10 })));
        let wild = held.arrive(1, 9, 0, message, stamp(50 + MAX_HOLD + 1, 2), 50, true);
        assert!(wild.is_some());
        assert_eq!(held.arrive(1, 9, 0, message, stamp(55, 3), 50, true), None);
        assert_eq!(held.arrive(2, 8, 0, message, stamp(53, 0), 50, true), None);
        assert_eq!(held.arrive(1, 9, 0, message, stamp(55, 4), 51, true), None);
        assert!(held.due(52).is_empty());
        let due: Vec<(u32, u32, i32)> = held
            .due(55)
            .iter()
            .map(|a| (a.from, a.stamp.seq, a.asked.early))
            .collect();
        assert_eq!(due, vec![(2, 0, 3), (1, 3, 5), (1, 4, 4)]);
        assert!(held.is_empty());
        assert_eq!(held.arrive(1, 9, 0, message, stamp(70, 5), 60, true), None);
        assert_eq!(held.all().len(), 1);
    }
    /// What another player does is news to the guest, rolled back onto or
    /// not: the host's shots are heard on the guest once each, none lost
    /// to the replay that brought them and none heard twice over — a
    /// guess of the trigger still held past its release the most it hears
    /// besides.
    #[test]
    fn another_player_s_shots_are_heard_on_the_guest_once_each() {
        let mut ends = pair();
        for end in ends.iter_mut() {
            end.session
                .game
                .as_mut()
                .unwrap()
                .world
                .set_quiet_sites_for_probe(true);
        }
        let delay = 4;
        let shots = |e: &mut End| -> usize {
            let room = &mut e.session.game.as_mut().unwrap().world.aboard.room;
            room.take_cues()
                .iter()
                .filter(|c| matches!(c.cue, bims::cue::Cue::Shot { .. }))
                .count()
        };
        let trigger = |fire: bool| {
            Order::Crew(CrewOrder::Control {
                walk: None,
                aim: 0,
                fire,
                sprint: false,
            })
        };
        let (mut host_heard, mut guest_heard) = (0, 0);
        for frame in 1..=360u32 {
            if frame == 40 || frame == 160 {
                let e = &mut ends[HOST];
                e.net.order(&mut e.session, trigger(true));
            }
            if frame == 100 || frame == 220 {
                let e = &mut ends[HOST];
                e.net.order(&mut e.session, trigger(false));
            }
            // The guest's own orders in flight all the while.
            if frame % 25 == 5 {
                let e = &mut ends[GUEST];
                let angle = (frame % 50 == 5).then_some(16_384);
                e.net.order(&mut e.session, walk(angle));
            }
            host_frame(&mut ends[HOST], frame, 1);
            guest_frame(&mut ends[GUEST], frame);
            post(&mut ends, frame, delay);
            host_heard += shots(&mut ends[HOST]);
            guest_heard += shots(&mut ends[GUEST]);
        }
        let r = &ends[GUEST].rollback;
        assert!(r.tally.rollbacks >= 4, "{:?}", r.tally);
        assert!(host_heard >= 6, "the host fired: {host_heard}");
        assert!(
            guest_heard >= host_heard && guest_heard <= host_heard + 4,
            "the guest heard {guest_heard} of the host's {host_heard}"
        );
    }
}

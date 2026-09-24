//! The wire's client end: the socket on a thread of its own, the room the
//! player is in, and what the players say to each other over it.
//!
//! # Who decides what
//!
//! The relay (`crates/server`) introduces the players and passes bytes
//! between them; it knows nothing of the game. The **host** — whoever
//! opened the room — is the one machine that decides: every edit to the
//! ship and every order to the world goes to the host, the host applies
//! them in the order they arrive and tells everybody what it applied, and
//! the host's frame is the clock: it says how many steps of the world went
//! by. A guest applies exactly what it is told, in the order it is told,
//! and never anything else — so every copy of the world is the host's
//! copy, and `world_checksum`, sent along every couple of seconds, is how
//! a guest finds out if it is not. That is deterministic lockstep with the
//! host as the sequencer, and it is what the two `Net` seams
//! (`screens/builder.rs`, `screens/designer.rs`) were built for before
//! there was a wire.
//!
//! The *protocol* between the players is [`Packet`], carried as opaque
//! bytes in the relay's `Relay`/`Relayed`. The order of packets on one
//! connection is the order they were sent in, which is what makes the
//! scheme work: a guest sees the host's `Applied`s and `Steps` in the
//! host's own order.
//!
//! # The host's world is the world
//!
//! Lockstep only holds while every copy agrees, and two things break it:
//! a guest whose checksum has parted from the host's — a bug, or a build
//! that is not quite the host's — and the host loading a saved game,
//! which is a new world on one machine. Both are mended the same way
//! (feature 67): **the host can ship its whole world, and a guest that
//! receives one replaces its own with it.** A guest that sees a checksum
//! it cannot match asks with `Packet::Resync`; the host answers that
//! guest with `Packet::World` — its `Session::save()` text, the same
//! text a save file holds — and sends the same to everybody after it
//! loads. A guest stands a session up round it as *its own* slot
//! (`Session::restore_as`) and carries on. It is a full replacement, not
//! a patch, and the ordering guarantee is what makes that safe: the
//! `Steps` and `Applied` that arrive after the `World` on the guest's
//! connection were sent after the host took it, so they are steps of the
//! new world, and whatever arrived before it was of the old one and is
//! thrown away with it. Nothing is queued or reconciled on either end.
//!
//! The text is megabytes, and reading it is one long frame on the guest
//! (the host's write is another): the same hitch a load has, and taken
//! as such for now. It is decoded on Bevy's thread on purpose — a world
//! stood up on the worker would have to be handed across and would land
//! between two frames' worth of `Steps`, which is the ordering problem
//! again.
//!
//! # The socket
//!
//! Bevy's schedule must never block, and a socket read does, so the
//! connection runs on a thread and reaches the screens through two
//! channels ([`Link`]). The thread polls rather than blocking on a read,
//! because it serves both directions and tungstenite's socket cannot be
//! split. A millisecond between passes keeps the latency it adds well
//! under a frame.
//!
//! `BIMS_SERVER` names the relay (`wire::DEFAULT_SERVER` otherwise) and
//! `BIMS_NAME` the name shown in the lobby (`$USER`, else "Player").

use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;
use std::time::Duration;

use bevy::prelude::*;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message as Frame, WebSocket};
use wire::{ClientCtl, Closed, PeerId, PeerInfo, Refusal, ServerCtl, To, decode, encode};

use bims::character::{Hair, Look, Shade, Tint};
use world::Class;

use crate::screens::builder::Settings;
use crate::screens::designer::Message;

/// How long the worker sleeps between passes when there was nothing to do.
const POLL: Duration = Duration::from_millis(1);

/// How often a quiet connection says something, so nothing in the middle
/// reaps an idle lobby. The relay drops a socket silent for 45 s.
const PING_EVERY: f64 = 10.0;

/// The host sends its `world_checksum` with the steps every so many
/// steps — two seconds of the clock at 1× — and a guest compares.
pub const CHECK_EVERY: u64 = 120;

/// How often a moving pointer is sent, in seconds: twenty a second is
/// smooth enough to follow across a deck and a fraction of what the
/// steps cost. A pointer that stops is sent once more, where it stopped.
pub const CURSOR_EVERY: f64 = 0.05;

/// What the game knows about the connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkState {
    /// No socket, and none wanted.
    Offline,
    /// The worker is dialling.
    Connecting,
    Connected,
    /// Dead, with something worth showing the player. Held rather than
    /// dropped straight back to `Offline` so the menu has something to
    /// print; cleared when the player next asks to connect.
    Failed(String),
}

/// The game's end of the connection.
pub struct Link {
    state: LinkState,
    /// `None` while offline. Dropping it is what tells the worker to stop.
    outgoing: Option<Sender<ClientCtl>>,
    /// Behind a `Mutex` only to satisfy `Sync`: a Bevy resource must be
    /// shareable between threads, and a channel receiver is `Send` but not
    /// `Sync`. Nothing ever contends for it — one system drains it, once a
    /// frame — so the lock is free in practice.
    incoming: Option<Mutex<Receiver<Incoming>>>,
}

/// What the worker sends back up: a message, or news about the socket.
enum Incoming {
    Msg(ServerCtl),
    Up,
    Down(String),
}

impl Default for Link {
    fn default() -> Self {
        Self {
            state: LinkState::Offline,
            outgoing: None,
            incoming: None,
        }
    }
}

impl Link {
    pub fn state(&self) -> &LinkState {
        &self.state
    }

    /// Start dialling. Any existing connection is dropped first, so this
    /// doubles as "reconnect".
    pub fn connect(&mut self, url: &str, name: &str) {
        self.disconnect();
        let (out_tx, out_rx) = channel::<ClientCtl>();
        let (in_tx, in_rx) = channel::<Incoming>();
        let url = url.to_string();
        // The hello is the first thing on the socket, sent by the worker
        // the moment the handshake is done, before anything the screens
        // queue behind it.
        let hello = ClientCtl::Hello {
            protocol: wire::PROTOCOL,
            name: name.to_string(),
        };
        // Detached deliberately. The handle would only ever be used to
        // join, and joining is exactly what a frame must not do; the thread
        // ends on its own when `outgoing` is dropped or the socket dies.
        thread::Builder::new()
            .name("bims-net".into())
            .spawn(move || worker(&url, hello, &out_rx, &in_tx))
            .expect("spawning the network thread");
        self.state = LinkState::Connecting;
        self.outgoing = Some(out_tx);
        self.incoming = Some(Mutex::new(in_rx));
    }

    pub fn disconnect(&mut self) {
        // Dropping the sender is the stop signal: the worker sees the
        // channel hang up and closes the socket on its way out.
        self.outgoing = None;
        self.incoming = None;
        self.state = LinkState::Offline;
    }

    /// Queue a message. Silently dropped while offline, which is what
    /// every caller would do with the error anyway.
    pub fn send(&self, msg: ClientCtl) {
        if let Some(tx) = &self.outgoing {
            let _ = tx.send(msg);
        }
    }

    /// A handle the seams keep, to post with: the sender half, cloned.
    fn sender(&self) -> Option<Sender<ClientCtl>> {
        self.outgoing.clone()
    }

    /// Everything that arrived since the last call, with connection news
    /// folded into [`Self::state`] on the way past.
    fn drain(&mut self) -> Vec<ServerCtl> {
        let mut out = Vec::new();
        let mut state = None;
        {
            let Some(guarded) = &self.incoming else {
                return out;
            };
            let rx = guarded.lock().expect("the network channel is poisoned");
            loop {
                match rx.try_recv() {
                    Ok(Incoming::Msg(msg)) => out.push(msg),
                    Ok(Incoming::Up) => state = Some(LinkState::Connected),
                    Ok(Incoming::Down(why)) => {
                        state = Some(LinkState::Failed(why));
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    // The worker died without saying why — a panic, which
                    // is a bug rather than a network problem, but the
                    // player still needs the connection to stop claiming
                    // it is up.
                    Err(TryRecvError::Disconnected) => {
                        if self.state != LinkState::Offline {
                            state = Some(LinkState::Failed("the connection thread stopped".into()));
                        }
                        break;
                    }
                }
            }
        }
        if let Some(state) = state {
            let dead = matches!(state, LinkState::Failed(_));
            self.state = state;
            if dead {
                self.outgoing = None;
                self.incoming = None;
            }
        }
        out
    }
}

fn worker(url: &str, hello: ClientCtl, out_rx: &Receiver<ClientCtl>, in_tx: &Sender<Incoming>) {
    let mut socket = match dial(url) {
        Ok(s) => s,
        Err(why) => {
            let _ = in_tx.send(Incoming::Down(why));
            return;
        }
    };
    if let Ok(bytes) = encode(&hello)
        && let Err(e) = socket.write(Frame::Binary(bytes))
    {
        let _ = in_tx.send(Incoming::Down(format!("send failed: {e}")));
        return;
    }
    if in_tx.send(Incoming::Up).is_err() {
        return;
    }
    loop {
        let mut idled = true;
        // Outgoing first, so a message queued this frame goes out in this
        // pass rather than waiting behind a sleep.
        loop {
            match out_rx.try_recv() {
                Ok(msg) => {
                    idled = false;
                    let Ok(bytes) = encode(&msg) else {
                        continue;
                    };
                    // A write that would block is not a failure: the
                    // frame is kept in tungstenite's write buffer and
                    // goes out on the flushes that follow. A whole world
                    // (`Packet::World`, megabytes) is the message that
                    // fills the socket's buffer and finds this out.
                    if let Err(e) = socket.write(Frame::Binary(bytes))
                        && !would_block(&e)
                    {
                        let _ = in_tx.send(Incoming::Down(format!("send failed: {e}")));
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                // The game hung up. Close politely so the relay frees the
                // berth straight away instead of waiting out the idle
                // timeout.
                Err(TryRecvError::Disconnected) => {
                    let _ = socket.close(None);
                    let _ = socket.flush();
                    return;
                }
            }
        }
        // A non-blocking flush can come back part-written; that is not an
        // error, the rest goes out on the next pass.
        if let Err(e) = socket.flush()
            && !would_block(&e)
        {
            let _ = in_tx.send(Incoming::Down(format!("send failed: {e}")));
            return;
        }
        loop {
            match socket.read() {
                Ok(Frame::Binary(bytes)) => {
                    idled = false;
                    match decode::<ServerCtl>(&bytes) {
                        Ok(msg) => {
                            if in_tx.send(Incoming::Msg(msg)).is_err() {
                                return; // game gone
                            }
                        }
                        Err(e) => {
                            let _ = in_tx.send(Incoming::Down(format!("bad frame: {e}")));
                            return;
                        }
                    }
                }
                Ok(Frame::Close(_)) => {
                    let _ = in_tx.send(Incoming::Down("the server closed the connection".into()));
                    return;
                }
                // Text, ping and pong: tungstenite answers pings itself,
                // and the relay never sends text.
                Ok(_) => idled = false,
                Err(e) if would_block(&e) => break,
                Err(e) => {
                    let _ = in_tx.send(Incoming::Down(format!("connection lost: {e}")));
                    return;
                }
            }
        }
        if idled {
            thread::sleep(POLL);
        }
    }
}

fn dial(url: &str) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, String> {
    // `connect` does its own DNS, TLS and handshake on a blocking socket,
    // which is what is wanted here — there is nothing else for this thread
    // to do until it finishes. Non-blocking is switched on afterwards, for
    // the poll loop.
    let (socket, _response) =
        tungstenite::connect(url).map_err(|e| format!("cannot reach {url}: {e}"))?;
    // The poll loop reads until it is told there is nothing left, so a
    // blocking socket here would park the thread inside `read` and never
    // send anything again. Plain or under TLS, the socket underneath is
    // the same `TcpStream`.
    let stream = match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream,
        MaybeTlsStream::Rustls(tls) => tls.get_ref(),
        _ => return Err("unsupported socket kind".into()),
    };
    stream
        .set_nonblocking(true)
        .map_err(|e| format!("cannot poll the socket: {e}"))?;
    // One small frame a tick is precisely the traffic Nagle would hold
    // back waiting for company.
    let _ = stream.set_nodelay(true);
    Ok(socket)
}

/// Whether an error is "nothing to do right now" rather than a failure.
fn would_block(e: &tungstenite::Error) -> bool {
    match e {
        tungstenite::Error::Io(io) => io.kind() == ErrorKind::WouldBlock,
        _ => false,
    }
}

// --- what the players say to each other ------------------------------------------

/// The lobby's settings, as they cross: the same plain numbers `Settings`
/// holds, without the lobby's two seats, which are the relay's to deal.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct SettingsWire {
    pub money_per_bim: u64,
    pub ship: u32,
    pub seed: u64,
    pub galaxy: u32,
    pub spawn: Option<(u32, u32)>,
}

impl SettingsWire {
    pub fn of(settings: &Settings) -> SettingsWire {
        SettingsWire {
            money_per_bim: settings.money_per_bim,
            ship: settings.ship,
            seed: settings.seed,
            galaxy: settings.galaxy,
            spawn: settings.spawn,
        }
    }

    /// Put these onto the lobby's settings. The seats are left alone.
    pub fn onto(self, settings: &mut Settings) {
        settings.money_per_bim = self.money_per_bim;
        settings.ship = self.ship;
        settings.seed = self.seed;
        settings.galaxy = self.galaxy;
        settings.spawn = self.spawn;
    }
}

/// One thing a player said to the others. The relay passes it as bytes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Packet {
    /// The host's settings, to everybody: on every change, and again
    /// whenever somebody joins.
    Settings(SettingsWire),
    /// A guest pointing at a star: a ring on everybody's map.
    Suggest { star: u32 },
    /// The host pressed Start: the settings the game opens with, and the
    /// crew in slot order — slot *i* is `slots[i]`'s Bim, called
    /// `names[i]` (empty for the app's own name).
    Start {
        settings: SettingsWire,
        slots: Vec<PeerId>,
        names: Vec<String>,
        hair: Vec<(Hair, Shade)>,
        /// Each slot's class, by `world::Class` code (feature 74).
        classes: Vec<u32>,
        /// Each slot's colour, by its place in `Tint::ALL` (feature 84).
        tints: Vec<u8>,
    },
    /// What the player called their Bim, to everybody: on every change,
    /// and again whenever somebody joins. Empty is the app's own name for
    /// whichever slot they are dealt. Tidied by `wire::tidy_name` before
    /// it goes out and again on arrival, since the other end is a
    /// stranger's build.
    BimName(String),
    /// How the player wears their Bim's hair and what colour it is
    /// (`character::Hair`, `Shade`), to everybody like the name: on every
    /// change and again whenever somebody joins (feature 62).
    BimHair(Hair, Shade),
    /// Which colour the player's own Bim is ringed in
    /// (`character::Tint`'s place in `Tint::ALL`), to everybody like the
    /// hair: on every change and again whenever somebody joins (feature
    /// 84). A code rather than the value, since a stranger's build may
    /// know more colours than this one.
    BimTint(u8),
    /// What class the player chose for their Bim (`world::Class`'s code),
    /// to everybody like the hair: on every change and again whenever
    /// somebody joins (feature 74).
    BimClass(u32),
    /// Where the player's pointer is, as a point on the ship's grid
    /// (`Session::design_point`) — the same tile on every machine
    /// whatever each has zoomed and turned — or `None` when it left the
    /// ship for a panel or the map. Everybody draws everybody else's,
    /// faint, in their colour (feature 60). Sent at most `CURSOR_EVERY`
    /// apart, and only when it moved.
    Cursor(Option<(f32, f32)>),
    /// A guest asking the host to apply an edit or an order, stamped with
    /// the design hash it was made against.
    Ask { at: u64, message: Message },
    /// The host applied this, from that slot: every guest applies it too,
    /// in this order. The host's own go out this way as well.
    Applied {
        from: u32,
        at: u64,
        message: Message,
    },
    /// The host refused the asker's message: an `EditError` code.
    Refused { why: u32 },
    /// The host's world went so many steps, and — every `CHECK_EVERY` —
    /// what its checksum was after them.
    Steps { n: u32, checksum: Option<u64> },
    /// A guest asking the host for its whole world (feature 67), sent
    /// `To::Host`. `reason`: 0 — the checksum drifted; 1 — a late or
    /// explicit request (reserved).
    Resync { reason: u32 },
    /// The host's world, whole: its `Session::save()` text and the
    /// world's `steps` when it was taken. To one peer in answer to a
    /// `Resync`, to everybody after the host loads a game. A guest
    /// **replaces** its world with it — as its own slot — and applies
    /// the `Steps` and `Applied` that come after it to the new one.
    World { save: String, at: u64 },
}

/// A handle the seams post with: the socket's sender and which end this
/// is. Cheap to clone, so a screen's `Net` keeps one.
#[derive(Clone)]
pub struct Wire {
    tx: Sender<ClientCtl>,
    pub host: bool,
}

impl Wire {
    pub fn send(&self, to: To, packet: &Packet) {
        if let Ok(payload) = encode(packet) {
            let _ = self.tx.send(ClientCtl::Relay { to, payload });
        }
    }
}

/// What a screen is told by the room, once a frame, from `Online::drain`.
#[derive(Clone, Debug)]
pub enum Event {
    /// The socket is up and the room was asked for; nothing to do yet.
    Connected,
    /// In a room: `Online::code`, `peers`, `host` are set.
    Joined,
    /// The roster changed.
    Roster,
    /// The room is gone.
    Closed(Closed),
    /// The relay said no.
    Rejected(Refusal),
    /// The connection died, with why.
    Lost(String),
    /// Somebody said something.
    Packet { from: PeerId, packet: Packet },
}

/// What was asked for when the socket was opened, sent once `Welcome`
/// lands.
#[derive(Clone, Debug)]
enum Pending {
    Create,
    Join(String),
}

/// The room the player is in, and the socket to it. One for the whole
/// game: the lobby opens it, the designer and the game play over it.
#[derive(Resource, Default)]
pub struct Online {
    pub link: Link,
    pub me: Option<PeerId>,
    pub code: Option<String>,
    pub host: Option<PeerId>,
    /// In join order, the host first — what the lobby lists.
    pub peers: Vec<PeerInfo>,
    /// The crew in slot order, once the host has pressed Start.
    pub slots: Vec<PeerId>,
    /// What each peer called their Bim, as they said it (`Packet::BimName`);
    /// nothing for a peer who has not said. The host reads it at Start to
    /// deal the names with the slots.
    pub bims: Vec<(PeerId, String)>,
    /// What each peer chose for their Bim's hair (`Packet::BimHair`);
    /// nothing for a peer who has not said. Dealt at Start like the names.
    pub hairs: Vec<(PeerId, (Hair, Shade))>,
    /// What each peer chose for their Bim's colour (`Packet::BimTint`);
    /// nothing for a peer who has not said. Dealt at Start like the
    /// hair, and what the lobby greys a swatch by, so no two players
    /// take the same one (feature 84).
    pub tints: Vec<(PeerId, Tint)>,
    /// What each peer chose for their Bim's class (`Packet::BimClass`);
    /// nothing for a peer who has not said. Dealt at Start like the hair.
    pub classes: Vec<(PeerId, Class)>,
    /// Where each peer's pointer is over the ship — a design point — for
    /// those whose pointer is over it. Folded in from `Packet::Cursor`
    /// as it arrives, never an event: it is a picture, not a decision.
    pub cursors: Vec<(PeerId, (f32, f32))>,
    pending: Option<Pending>,
    /// When the last ping went, in the egui clock's seconds.
    pinged: f64,
    /// The pointer as last sent, and when — `Self::point`'s throttle.
    sent_cursor: Option<(f32, f32)>,
    cursor_at: f64,
}

impl Online {
    /// Open a room at the relay. The lobby shows "connecting" until
    /// `Event::Joined`.
    pub fn create(&mut self) {
        self.open(Pending::Create);
    }

    /// Walk into somebody else's room.
    pub fn join(&mut self, code: &str) {
        self.open(Pending::Join(wire::normalise_code(code)));
    }

    fn open(&mut self, pending: Pending) {
        self.leave();
        self.pending = Some(pending);
        self.link.connect(&server_url(), &player_name());
    }

    /// Out of the room and off the wire. The relay tells the others.
    pub fn leave(&mut self) {
        self.link.disconnect();
        self.me = None;
        self.code = None;
        self.host = None;
        self.peers.clear();
        self.slots.clear();
        self.bims.clear();
        self.hairs.clear();
        self.tints.clear();
        self.classes.clear();
        self.cursors.clear();
        self.pending = None;
        self.sent_cursor = None;
    }

    /// What a peer called their Bim, if they said: `Some` even when they
    /// said nothing yet — empty — so the lobby can tell "not said" from
    /// "left blank" and print neither.
    pub fn bim_name(&self, peer: PeerId) -> Option<&str> {
        self.bims
            .iter()
            .find(|(p, _)| *p == peer)
            .map(|(_, n)| n.as_str())
    }

    /// Tell the room what this player calls their Bim. Every change, and
    /// again when somebody joins.
    pub fn say_bim_name(&self, name: &str) {
        self.send(To::All, &Packet::BimName(wire::tidy_name(name)));
    }

    /// The crew's names in slot order, as the host deals them at Start:
    /// what each peer said, and nothing — the app's own name — for one
    /// who said nothing. Everybody keeps the same list.
    pub fn deal_names(&self, slots: &[PeerId]) -> Vec<String> {
        slots
            .iter()
            .map(|&p| self.bim_name(p).unwrap_or("").to_string())
            .collect()
    }

    /// What the others said their Bims are called, put onto the crew's
    /// names by slot: a name that reached the host after it pressed Start
    /// — or was typed after — still lands on every machine. `true` when
    /// something changed, for the caller to put the names where the
    /// words are. Nothing for this player's own slot, which is its own
    /// field's.
    pub fn names_said(&self, names: &mut Vec<String>) -> bool {
        let mut changed = false;
        for (peer, name) in &self.bims {
            let Some(slot) = self.slot_of(*peer) else {
                continue;
            };
            let slot = slot as usize;
            if names.len() <= slot {
                names.resize(slot + 1, String::new());
            }
            if names[slot] != *name {
                names[slot] = name.clone();
                changed = true;
            }
        }
        changed
    }

    /// What a peer chose for their Bim's hair, if they said.
    pub fn bim_hair(&self, peer: PeerId) -> Option<(Hair, Shade)> {
        self.hairs.iter().find(|(p, _)| *p == peer).map(|(_, h)| *h)
    }

    /// Tell the room how this player's Bim wears its hair. Every change,
    /// and again when somebody joins.
    pub fn say_bim_hair(&self, hair: Hair, shade: Shade) {
        self.send(To::All, &Packet::BimHair(hair, shade));
    }

    /// The crew's hair in slot order, as the host deals it at Start: what
    /// each peer said, and the look the slot deals (`Look::of`) for one
    /// who said nothing. Everybody keeps the same list.
    pub fn deal_hair(&self, slots: &[PeerId]) -> Vec<(Hair, Shade)> {
        slots
            .iter()
            .enumerate()
            .map(|(slot, &p)| {
                self.bim_hair(p).unwrap_or_else(|| {
                    let look = Look::of(slot);
                    (look.hair, look.shade)
                })
            })
            .collect()
    }

    /// What the others said their Bims' hair is, put onto the crew's
    /// list by slot, the way [`Online::names_said`] puts the names: a
    /// choice that reached the host after Start still lands on every
    /// machine. `true` when something changed. Nothing for this player's
    /// own slot.
    pub fn hair_said(&self, hair: &mut Vec<(Hair, Shade)>) -> bool {
        let mut changed = false;
        for (peer, said) in &self.hairs {
            let Some(slot) = self.slot_of(*peer) else {
                continue;
            };
            let slot = slot as usize;
            if hair.len() <= slot {
                // The slots between are the dealt looks, as `deal_hair`
                // would have dealt them.
                for s in hair.len()..=slot {
                    let look = Look::of(s);
                    hair.push((look.hair, look.shade));
                }
            }
            if hair[slot] != *said {
                hair[slot] = *said;
                changed = true;
            }
        }
        changed
    }

    /// What a peer chose for their Bim's colour, if they said (feature
    /// 84).
    pub fn bim_tint(&self, peer: PeerId) -> Option<Tint> {
        self.tints.iter().find(|(p, _)| *p == peer).map(|(_, t)| *t)
    }

    /// Tell the room which colour this player's Bim is ringed in. Every
    /// change, and again when somebody joins.
    pub fn say_bim_tint(&self, tint: Tint) {
        self.send(To::All, &Packet::BimTint(tint.code()));
    }

    /// Which colours the **other** players in the lobby have taken, so
    /// the chooser can grey them: a colour is one player's, which is the
    /// whole point of it.
    pub fn tints_taken(&self) -> Vec<Tint> {
        let me = self.me;
        self.tints
            .iter()
            .filter(|(p, _)| Some(*p) != me)
            .map(|(_, t)| *t)
            .collect()
    }

    /// The crew's colours in slot order, as the host deals it at Start:
    /// what each peer said, and the slot's own of [`Tint::ALL`] for one
    /// who said nothing. Everybody keeps the same list.
    pub fn deal_tints(&self, slots: &[PeerId]) -> Vec<Tint> {
        slots
            .iter()
            .enumerate()
            .map(|(slot, &p)| {
                self.bim_tint(p)
                    .unwrap_or(Tint::ALL[slot % Tint::ALL.len()])
            })
            .collect()
    }

    /// What the others said their Bims' colours are, put onto the crew's
    /// list by slot, the way [`Online::hair_said`] puts the hair. `true`
    /// when something changed. Nothing for this player's own slot.
    pub fn tint_said(&self, tints: &mut Vec<Tint>) -> bool {
        let mut changed = false;
        for (peer, said) in &self.tints {
            let Some(slot) = self.slot_of(*peer) else {
                continue;
            };
            let slot = slot as usize;
            if tints.len() <= slot {
                // The slots between are the dealt colours, as
                // `deal_tints` would have dealt them.
                for s in tints.len()..=slot {
                    tints.push(Tint::ALL[s % Tint::ALL.len()]);
                }
            }
            if tints[slot] != *said {
                tints[slot] = *said;
                changed = true;
            }
        }
        changed
    }

    /// Where this player's pointer is over the ship, as a design point,
    /// or `None` off it: sent to the room when it moved and `CURSOR_EVERY`
    /// has gone by since the last — or at once when it left, so nobody
    /// is left drawing a pointer that is not there. Called every frame;
    /// a pointer that moved and stopped inside the interval goes out on
    /// the first frame after it.
    pub fn point(&mut self, now: f64, at: Option<(f32, f32)>) {
        if !self.is_online() || at == self.sent_cursor {
            return;
        }
        // Leaving, and arriving from off the ship, go at once; only a move
        // over it is thinned.
        if at.is_some() && self.sent_cursor.is_some() && now - self.cursor_at < CURSOR_EVERY {
            return;
        }
        self.sent_cursor = at;
        self.cursor_at = now;
        self.send(To::All, &Packet::Cursor(at));
    }

    /// Every other player's pointer over the ship: their slot, the point,
    /// for the screens to draw. Nothing for a peer not dealt a slot.
    pub fn others_pointing(&self) -> Vec<(u32, (f32, f32))> {
        self.cursors
            .iter()
            .filter(|(p, _)| Some(*p) != self.me)
            .filter_map(|(p, at)| self.slot_of(*p).map(|slot| (slot, *at)))
            .collect()
    }

    /// What a peer chose for their Bim's class, if they said.
    pub fn bim_class(&self, peer: PeerId) -> Option<Class> {
        self.classes
            .iter()
            .find(|(p, _)| *p == peer)
            .map(|(_, c)| *c)
    }

    /// Tell the room what class this player's Bim is. Every change, and
    /// again when somebody joins.
    pub fn say_bim_class(&self, class: Class) {
        self.send(To::All, &Packet::BimClass(class.code()));
    }

    /// The crew's classes in slot order, as the host deals them at Start:
    /// what each peer said, and none for one who said nothing.
    pub fn deal_classes(&self, slots: &[PeerId]) -> Vec<Class> {
        slots
            .iter()
            .map(|&p| self.bim_class(p).unwrap_or_default())
            .collect()
    }

    /// What the others said their Bims' classes are, put onto the crew's
    /// list by slot, the way [`Online::hair_said`] puts the hair. `true`
    /// when something changed. Nothing for this player's own slot.
    pub fn classes_said(&self, classes: &mut Vec<Class>) -> bool {
        let mut changed = false;
        for (peer, said) in &self.classes {
            let Some(slot) = self.slot_of(*peer) else {
                continue;
            };
            let slot = slot as usize;
            if classes.len() <= slot {
                classes.resize(slot + 1, Class::None);
            }
            if classes[slot] != *said {
                classes[slot] = *said;
                changed = true;
            }
        }
        changed
    }

    fn set_cursor(&mut self, peer: PeerId, at: Option<(f32, f32)>) {
        self.cursors.retain(|(p, _)| *p != peer);
        if let Some(at) = at {
            self.cursors.push((peer, at));
        }
    }

    /// Forget whoever is no longer in the room.
    fn prune(&mut self) {
        let keep: Vec<PeerId> = self.peers.iter().map(|p| p.id).collect();
        self.bims.retain(|(p, _)| keep.contains(p));
        self.hairs.retain(|(p, _)| keep.contains(p));
        self.tints.retain(|(p, _)| keep.contains(p));
        self.classes.retain(|(p, _)| keep.contains(p));
        self.cursors.retain(|(p, _)| keep.contains(p));
    }

    pub fn is_online(&self) -> bool {
        self.code.is_some() && self.link.state() == &LinkState::Connected
    }

    pub fn is_host(&self) -> bool {
        self.me.is_some() && self.me == self.host
    }

    /// On the wire and not the host: somebody whose world is the host's.
    /// What greys a guest's Load (feature 67).
    pub fn is_guest(&self) -> bool {
        self.link.state() == &LinkState::Connected && !self.is_host()
    }

    pub fn connecting(&self) -> bool {
        self.link.state() == &LinkState::Connecting
            || (self.link.state() == &LinkState::Connected && self.code.is_none())
    }

    /// The handle a seam posts with, while in a room.
    pub fn wire(&self) -> Option<Wire> {
        if !self.is_online() {
            return None;
        }
        self.link.sender().map(|tx| Wire {
            tx,
            host: self.is_host(),
        })
    }

    /// The slot a peer plays, once the crew were dealt.
    pub fn slot_of(&self, peer: PeerId) -> Option<u32> {
        self.slots.iter().position(|&p| p == peer).map(|i| i as u32)
    }

    /// This player's own slot.
    pub fn my_slot(&self) -> u32 {
        self.me.and_then(|me| self.slot_of(me)).unwrap_or(0)
    }

    /// How many the crew were dealt for, while on the wire with company:
    /// what a saved game has to have been saved for before the host may
    /// load it (feature 67). `None` in a game of one — off the wire, or
    /// before the crew were dealt.
    pub fn room_size(&self) -> Option<u32> {
        (self.is_online() && !self.slots.is_empty()).then_some(self.slots.len() as u32)
    }

    /// Deal the crew: the peers in join order, host first — what the host
    /// puts in its `Start`. Everybody keeps the same list.
    pub fn deal(&mut self) -> Vec<PeerId> {
        self.slots = self.peers.iter().map(|p| p.id).collect();
        self.slots.clone()
    }

    pub fn send(&self, to: To, packet: &Packet) {
        if let Some(wire) = self.wire() {
            wire.send(to, packet);
        }
    }

    /// Tell the relay the game has begun: nobody else joins this room.
    /// The host's to say, at Start.
    pub fn begin(&self) {
        self.link.send(ClientCtl::Begin);
    }

    /// Everything the relay and the others said since last frame, in
    /// order, with the room's own state kept up on the way past. `now`
    /// is the clock the ping runs on.
    pub fn drain(&mut self, now: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for msg in self.link.drain() {
            match msg {
                ServerCtl::Welcome { you, .. } => {
                    self.me = Some(you);
                    match self.pending.take() {
                        Some(Pending::Create) => self.link.send(ClientCtl::Create),
                        Some(Pending::Join(code)) => self.link.send(ClientCtl::Join { code }),
                        None => {}
                    }
                    events.push(Event::Connected);
                }
                ServerCtl::Rejected { why } => {
                    // A refusal of the room asked for is the end of the
                    // connection: there is nothing to stay for.
                    if self.code.is_none() {
                        self.leave();
                    }
                    events.push(Event::Rejected(why));
                }
                ServerCtl::RoomJoined { code, host, peers } => {
                    self.code = Some(code);
                    self.host = Some(host);
                    self.peers = peers;
                    events.push(Event::Joined);
                }
                ServerCtl::RoomUpdate { host, peers } => {
                    self.host = Some(host);
                    self.peers = peers;
                    self.prune();
                    events.push(Event::Roster);
                }
                ServerCtl::RoomClosed { why } => {
                    self.leave();
                    events.push(Event::Closed(why));
                }
                ServerCtl::Relayed { from, payload } => match decode::<Packet>(&payload) {
                    // A pointer and a Bim's name are the room's own
                    // state, kept here for the screens to read; neither
                    // is anything a screen has to act on.
                    Ok(Packet::Cursor(at)) => self.set_cursor(from, at),
                    Ok(Packet::BimName(name)) => {
                        let name = wire::tidy_name(&name);
                        self.bims.retain(|(p, _)| *p != from);
                        self.bims.push((from, name));
                    }
                    Ok(Packet::BimHair(hair, shade)) => {
                        self.hairs.retain(|(p, _)| *p != from);
                        self.hairs.push((from, (hair, shade)));
                    }
                    Ok(Packet::BimClass(code)) => {
                        self.classes.retain(|(p, _)| *p != from);
                        self.classes
                            .push((from, Class::from_code(code).unwrap_or_default()));
                    }
                    Ok(Packet::BimTint(code)) => {
                        self.tints.retain(|(p, _)| *p != from);
                        self.tints.push((from, Tint::from_code(code)));
                    }
                    Ok(packet) => events.push(Event::Packet { from, packet }),
                    Err(_) => {
                        // Somebody on another version of the game, past
                        // the protocol check: nothing to make of it.
                    }
                },
                ServerCtl::Pong { .. } => {}
            }
        }
        if let LinkState::Failed(why) = self.link.state().clone() {
            self.leave();
            events.push(Event::Lost(why));
        }
        if self.link.state() == &LinkState::Connected && now - self.pinged > PING_EVERY {
            self.pinged = now;
            self.link.send(ClientCtl::Ping { stamp: 0 });
        }
        events
    }
}

/// Where the relay is: `BIMS_SERVER`, else the one at `bims.buggly.de`.
pub fn server_url() -> String {
    std::env::var("BIMS_SERVER").unwrap_or_else(|_| wire::DEFAULT_SERVER.to_string())
}

/// What the lobby calls this player: `BIMS_NAME`, else the login name,
/// else "Player".
pub fn player_name() -> String {
    let raw = std::env::var("BIMS_NAME")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let name = wire::tidy_name(&raw);
    if name.is_empty() {
        "Player".into()
    } else {
        name
    }
}

/// The name of a peer, by id, for the roster and the log.
pub fn peer_name(online: &Online, peer: PeerId) -> String {
    online
        .peers
        .iter()
        .find(|p| p.id == peer)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| format!("Player {peer}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::designer::{Net, Order};
    use bims::order::CrewOrder;
    use server::hub::{Hub, Outbound};
    use ship::{Preset, Session};
    use shipdesign::parts::PartKind;
    use std::sync::mpsc::Receiver;
    use wire::PROTOCOL;

    const CANVAS: (f32, f32) = (800.0, 600.0);

    /// One end of the wire in the test: its session, its seam, and the
    /// far side of its socket — the relay reads what it posts off `rx`.
    struct End {
        peer: PeerId,
        session: Session,
        net: Net,
        rx: Receiver<ClientCtl>,
        refused: Vec<u32>,
    }

    fn end(hub: &mut Hub, slot: u32, host: bool) -> End {
        let peer = hub.connect();
        hub.handle(
            peer,
            ClientCtl::Hello {
                protocol: PROTOCOL,
                name: format!("P{slot}"),
            },
        );
        let spawn = ship::session::pick_dock(world::data::DEFAULT_SEED, 0, 0);
        // The combat ship, on the playtest's own area: bunks and chairs
        // for two, which the playtest ship has not.
        let mut session = Session::design(
            shipdesign::fixture::AREA,
            100_000,
            2,
            slot,
            world::data::DEFAULT_SEED,
            0,
            spawn,
            Preset::Empty,
            CANVAS.0,
            CANVAS.1,
        );
        session.editor.give(shipdesign::fixture::combat_ship());
        let (tx, rx) = channel();
        End {
            peer,
            session,
            net: Net {
                slot,
                players: 2,
                wire: Some(Wire { tx, host }),
            },
            rx,
            refused: Vec::new(),
        }
    }

    /// The relay's turn: everything both ends posted goes through the hub,
    /// and what it hands back is applied at the end it names — as the
    /// screens apply it — until nothing is in flight.
    fn pump(hub: &mut Hub, ends: &mut [End; 2]) {
        loop {
            let mut out: Vec<Outbound> = Vec::new();
            for end in ends.iter() {
                while let Ok(msg) = end.rx.try_recv() {
                    out.extend(hub.handle(end.peer, msg));
                }
            }
            if out.is_empty() {
                return;
            }
            for Outbound { to, msg } in out {
                let ServerCtl::Relayed { from, payload } = msg else {
                    continue;
                };
                let packet: Packet = decode(&payload).expect("a packet");
                let from_slot = ends.iter().position(|e| e.peer == from).unwrap() as u32;
                let end = ends.iter_mut().find(|e| e.peer == to).unwrap();
                match packet {
                    Packet::Ask { at, message } => {
                        end.net
                            .asked(&mut end.session, from_slot, at, message, from);
                    }
                    Packet::Applied { from, at, message } => {
                        end.net.applied(&mut end.session, from, at, message);
                    }
                    Packet::Refused { why } => end.refused.push(why),
                    Packet::Steps { n, checksum } => {
                        for _ in 0..n {
                            end.session.world_step();
                        }
                        if let Some(theirs) = checksum {
                            let mine = end.session.game.as_ref().unwrap().world.checksum();
                            assert_eq!(theirs, mine, "the guest's world is the host's");
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn two_ends_of_the_wire_lay_out_one_ship_and_fly_one_world() {
        let mut hub = Hub::new();
        let mut ends = [end(&mut hub, 0, true), end(&mut hub, 1, false)];
        let code = match hub
            .handle(ends[0].peer, ClientCtl::Create)
            .pop()
            .unwrap()
            .msg
        {
            ServerCtl::RoomJoined { code, .. } => code,
            other => panic!("{other:?}"),
        };
        hub.handle(ends[1].peer, ClientCtl::Join { code });
        assert_eq!(ends[0].session.editor.hash(), ends[1].session.editor.hash());
        assert!(
            !ends[0].session.editor.has_errors(),
            "{:?}",
            ends[0].session.editor.issues()
        );

        // The guest lays a plant somewhere it goes: refused where it does
        // not, by the host, to the guest alone; applied everywhere once it
        // does. The host's own edit goes round the same way.
        let plant = PartKind::SmallPlant.code();
        let mut placed = 0;
        'tiles: for y in 1..19 {
            for x in 1..19 {
                let before = ends[1].refused.len();
                ends[1].net.place(&mut ends[1].session, plant, x, y, 0);
                pump(&mut hub, &mut ends);
                if ends[1].refused.len() != before {
                    continue;
                }
                // Laid, but in somebody's way: taken up again, through
                // the seam like the laying.
                if ends[0].session.editor.has_errors() {
                    let last = ends[1].session.editor.design.parts.last().unwrap().id;
                    ends[1].net.remove(&mut ends[1].session, last);
                    pump(&mut hub, &mut ends);
                    assert!(!ends[0].session.editor.has_errors());
                    continue;
                }
                placed += 1;
                if placed == 2 {
                    break 'tiles;
                }
            }
        }
        assert_eq!(placed, 2, "the guest laid two plants");
        assert!(!ends[1].refused.is_empty(), "and was refused a few tiles");
        let hash = ends[0].session.editor.hash();
        assert_eq!(hash, ends[1].session.editor.hash(), "one ship on both ends");
        let host_placed = (1..19).any(|x| {
            let went = ends[0].net.place(&mut ends[0].session, plant, x, 10, 0).ok;
            if went && ends[0].session.editor.has_errors() {
                let last = ends[0].session.editor.design.parts.last().unwrap().id;
                ends[0].net.remove(&mut ends[0].session, last);
                return false;
            }
            went
        });
        assert!(host_placed);
        pump(&mut hub, &mut ends);
        assert_ne!(ends[0].session.editor.hash(), hash);
        assert_eq!(ends[0].session.editor.hash(), ends[1].session.editor.hash());

        // Both accept, the guest first: the last Accept opens the world on
        // both ends at once, on the same design.
        assert!(ends[1].net.accept(&mut ends[1].session, true).ok);
        pump(&mut hub, &mut ends);
        assert!(
            ends[0].session.editor.accepted(1),
            "refused {:?}, errors {}",
            ends[1].refused,
            ends[0].session.editor.has_errors()
        );
        assert!(!ends[0].session.playing());
        assert!(ends[0].net.accept(&mut ends[0].session, true).ok);
        pump(&mut hub, &mut ends);
        assert!(ends[0].session.playing() && ends[1].session.playing());
        let checksum = |e: &End| e.session.game.as_ref().unwrap().world.checksum();
        assert_eq!(checksum(&ends[0]), checksum(&ends[1]));

        // The host is the clock: it steps and says so, the guest follows.
        // A guest's order — a walk — goes to the host and comes round as
        // applied; the guest's own copy applies it then and not before.
        let steps = |ends: &mut [End; 2], n: u32| {
            for _ in 0..n {
                ends[0].session.world_step();
            }
            let checksum = Some(ends[0].session.game.as_ref().unwrap().world.checksum());
            ends[0]
                .net
                .wire
                .as_ref()
                .unwrap()
                .send(To::All, &Packet::Steps { n, checksum });
        };
        steps(&mut ends, 120);
        pump(&mut hub, &mut ends);
        let from = ends[1].session.room_ref().unwrap().bim_pos(1);
        ends[1]
            .net
            .order(&mut ends[1].session, Order::Crew(CrewOrder::SelectOwn));
        ends[1].net.order(
            &mut ends[1].session,
            Order::Crew(CrewOrder::Move {
                x: from.x + 3.0 * shipdesign::parts::TILE as f32,
                y: from.y,
            }),
        );
        assert!(
            !ends[1].session.room_ref().unwrap().is_selected(1, 1),
            "nothing applied on the guest until the host says"
        );
        pump(&mut hub, &mut ends);
        assert!(ends[0].session.room_ref().unwrap().is_selected(1, 1));
        assert!(ends[1].session.room_ref().unwrap().is_selected(1, 1));
        steps(&mut ends, 600);
        pump(&mut hub, &mut ends);
        assert_eq!(checksum(&ends[0]), checksum(&ends[1]));
        let now = ends[1].session.room_ref().unwrap().bim_pos(1);
        assert!(
            (now - from).len() > shipdesign::parts::TILE as f32,
            "the guest's crew member walked on both ends"
        );
        assert_eq!(
            ends[0].session.room_ref().unwrap().bim_pos(1),
            ends[1].session.room_ref().unwrap().bim_pos(1)
        );
        // And the host's own speed request goes round too.
        ends[0]
            .net
            .order(&mut ends[0].session, Order::Speed(world::Speed::Paused));
        pump(&mut hub, &mut ends);
        assert_eq!(
            ends[1]
                .session
                .game
                .as_ref()
                .unwrap()
                .world
                .effective_speed(),
            world::Speed::Paused
        );
    }

    /// The run's loop across the wire (feature 103): both players press
    /// Back to ship, the host puts a destination to the crew and the
    /// guest accepts it — and both ends leave, travel and arrive as one
    /// world.
    #[test]
    fn two_ends_leave_vote_and_travel_as_one_world() {
        let mut hub = Hub::new();
        let mut ends = [end(&mut hub, 0, true), end(&mut hub, 1, false)];
        let code = match hub
            .handle(ends[0].peer, ClientCtl::Create)
            .pop()
            .unwrap()
            .msg
        {
            ServerCtl::RoomJoined { code, .. } => code,
            other => panic!("{other:?}"),
        };
        hub.handle(ends[1].peer, ClientCtl::Join { code });
        assert!(ends[1].net.accept(&mut ends[1].session, true).ok);
        pump(&mut hub, &mut ends);
        assert!(ends[0].net.accept(&mut ends[0].session, true).ok);
        pump(&mut hub, &mut ends);
        assert!(ends[0].session.playing() && ends[1].session.playing());
        fn world(e: &End) -> &world::World {
            &e.session.game.as_ref().unwrap().world
        }
        let steps = |ends: &mut [End; 2], n: u32| {
            for _ in 0..n {
                ends[0].session.world_step();
            }
            let checksum = Some(ends[0].session.game.as_ref().unwrap().world.checksum());
            ends[0]
                .net
                .wire
                .as_ref()
                .unwrap()
                .send(To::All, &Packet::Steps { n, checksum });
        };
        steps(&mut ends, 30);
        pump(&mut hub, &mut ends);
        // Both press: the ship leaves on the next step, the map up on both.
        ends[0].net.order(&mut ends[0].session, Order::ReturnToShip);
        ends[1].net.order(&mut ends[1].session, Order::ReturnToShip);
        pump(&mut hub, &mut ends);
        steps(&mut ends, 2);
        pump(&mut hub, &mut ends);
        assert!(!world(&ends[0]).in_mission() && !world(&ends[1]).in_mission());
        assert_eq!(world(&ends[0]).checksum(), world(&ends[1]).checksum());
        // The host proposes; the guest accepts.
        let site = world(&ends[0])
            .travel_quotes()
            .into_iter()
            .find(|(s, q)| q.is_ok() && Some(*s) != world(&ends[0]).current_site())
            .map(|(s, _)| s)
            .unwrap();
        ends[0].net.order(
            &mut ends[0].session,
            Order::Propose {
                star: site.star,
                station: site.station,
            },
        );
        pump(&mut hub, &mut ends);
        assert!(world(&ends[1]).run.proposal.is_some(), "the guest sees it");
        assert!(!world(&ends[0]).in_mission(), "one yes of two");
        ends[1]
            .net
            .order(&mut ends[1].session, Order::AcceptTrip(true));
        pump(&mut hub, &mut ends);
        assert!(world(&ends[0]).in_mission(), "the host travelled");
        assert!(world(&ends[1]).in_mission(), "and the guest");
        assert_eq!(world(&ends[0]).checksum(), world(&ends[1]).checksum());
        steps(&mut ends, 120);
        pump(&mut hub, &mut ends);
        assert_eq!(world(&ends[0]).checksum(), world(&ends[1]).checksum());
    }

    /// The room's own state off the wire: a Bim's name and a pointer are
    /// folded into `Online` as they arrive, never events, and a pointer
    /// goes out only when it moved and the interval has gone by — or at
    /// once when it left the ship.
    #[test]
    fn a_name_and_a_pointer_are_the_room_s_to_keep_and_the_pointer_is_thinned() {
        let (tx, rx) = channel::<ClientCtl>();
        let mut online = Online {
            link: Link {
                state: LinkState::Connected,
                outgoing: Some(tx),
                incoming: None,
            },
            me: Some(1),
            code: Some("ABCDEF".into()),
            host: Some(1),
            peers: vec![
                PeerInfo {
                    id: 1,
                    name: "Host".into(),
                },
                PeerInfo {
                    id: 2,
                    name: "Guest".into(),
                },
            ],
            slots: vec![1, 2],
            ..Default::default()
        };
        let sent = |rx: &Receiver<ClientCtl>| -> Vec<Packet> {
            let mut out = Vec::new();
            while let Ok(ClientCtl::Relay { payload, .. }) = rx.try_recv() {
                out.push(decode(&payload).unwrap());
            }
            out
        };
        // A pointer: the first goes, the next inside the interval waits,
        // and the one after the interval goes where it is *now*.
        online.point(0.0, Some((10.0, 10.0)));
        online.point(0.01, Some((11.0, 10.0)));
        online.point(0.02, Some((12.0, 10.0)));
        online.point(0.02 + CURSOR_EVERY, Some((13.0, 10.0)));
        online.point(0.03 + CURSOR_EVERY, Some((13.0, 10.0)));
        online.point(0.04 + CURSOR_EVERY, None);
        let went = sent(&rx);
        assert!(
            matches!(
                went.as_slice(),
                [
                    Packet::Cursor(Some((10.0, 10.0))),
                    Packet::Cursor(Some((13.0, 10.0))),
                    Packet::Cursor(None)
                ]
            ),
            "{went:?}"
        );
        // What the guest says arrives as state, not as an event.
        let relayed = |from: PeerId, packet: &Packet| ServerCtl::Relayed {
            from,
            payload: encode(packet).unwrap(),
        };
        let (in_tx, in_rx) = channel::<Incoming>();
        online.link.incoming = Some(Mutex::new(in_rx));
        in_tx
            .send(Incoming::Msg(relayed(
                2,
                &Packet::BimName("  Ada\n ".into()),
            )))
            .unwrap();
        in_tx
            .send(Incoming::Msg(relayed(2, &Packet::Cursor(Some((3.0, 4.0))))))
            .unwrap();
        let events = online.drain(1.0);
        assert!(events.is_empty(), "{events:?}");
        assert_eq!(online.bim_name(2), Some("Ada"));
        assert_eq!(online.others_pointing(), vec![(1, (3.0, 4.0))]);
        assert_eq!(
            online.deal_names(&[1, 2]),
            vec!["".to_string(), "Ada".to_string()]
        );
        // And a name that arrived after the deal lands on the slot.
        let mut names = vec!["Zed".to_string()];
        assert!(online.names_said(&mut names));
        assert_eq!(names, vec!["Zed".to_string(), "Ada".to_string()]);
        assert!(!online.names_said(&mut names));
        // The hair the same way: said, dealt with the slot's own for
        // whoever said nothing, and landing late on the slot.
        in_tx
            .send(Incoming::Msg(relayed(
                2,
                &Packet::BimHair(Hair::Mohawk, Shade::Red),
            )))
            .unwrap();
        assert!(online.drain(1.5).is_empty());
        assert_eq!(online.bim_hair(2), Some((Hair::Mohawk, Shade::Red)));
        let dealt = Look::of(0);
        assert_eq!(
            online.deal_hair(&[1, 2]),
            vec![(dealt.hair, dealt.shade), (Hair::Mohawk, Shade::Red)]
        );
        let mut hair = vec![(Hair::Bald, Shade::Grey)];
        assert!(online.hair_said(&mut hair));
        assert_eq!(
            hair,
            vec![(Hair::Bald, Shade::Grey), (Hair::Mohawk, Shade::Red)]
        );
        assert!(!online.hair_said(&mut hair));
        // Gone from the room, gone from all three.
        in_tx
            .send(Incoming::Msg(ServerCtl::RoomUpdate {
                host: 1,
                peers: online.peers[..1].to_vec(),
            }))
            .unwrap();
        online.drain(2.0);
        assert_eq!(online.bim_name(2), None);
        assert_eq!(online.bim_hair(2), None);
        assert!(online.others_pointing().is_empty());
    }
}

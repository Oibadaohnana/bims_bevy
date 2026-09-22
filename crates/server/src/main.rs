//! The Bims relay.
//!
//! It introduces the players to each other and passes bytes between them,
//! and that is all it does. The game is simulated on every player's own
//! machine, the host's being the clock — see `crates/app/src/net.rs` — so
//! nothing here knows what a ship is, which is why a new part or a new
//! command never needs this redeployed.
//!
//! Configuration is three environment variables, so the systemd unit is the
//! whole of the configuration (`nixos/server/games/bims.nix` in the
//! server's config):
//!
//! - `HOST` — address to bind, default `127.0.0.1`. The unit leaves this
//!   on loopback and lets nginx be the public face, as the other games on
//!   the box do; `bims.buggly.de` is nginx terminating TLS in front.
//! - `PORT` — default `8792` (`wire::DEFAULT_PORT`).
//! - `BIMS_MAX_PEERS` — refuse connections past this many, default 256.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use wire::{ClientCtl, MAX_PAYLOAD, PeerId, ServerCtl, decode, encode};

use server::hub::{Hub, Outbound};

/// How long a socket may stay silent before it is dropped.
///
/// The game pings every few seconds, so anything quiet for this long is a
/// connection that died without saying so — a laptop lid shut mid-game,
/// most often. Without this they accumulate, holding berths in rooms
/// nobody can use.
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

/// Time allowed between the TCP connection and a finished WebSocket
/// handshake. Anything slower is a port scanner, not a player.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Everything shared between connection tasks.
struct Shared {
    hub: Mutex<Hub>,
    /// One outbox per live connection. Unbounded because the alternative
    /// — blocking a *sender* because a *receiver* is slow — would let one
    /// lagging guest stall the host's broadcast to everybody else. A
    /// client that cannot keep up loses its connection to the idle timeout
    /// instead.
    outboxes: Mutex<HashMap<PeerId, mpsc::UnboundedSender<ServerCtl>>>,
}

impl Shared {
    /// Put a batch of hub output on the right sockets.
    ///
    /// Send failures are ignored: the only way one happens is that the
    /// receiving task has already gone, and that task's own cleanup is
    /// what tells the hub about it.
    fn dispatch(&self, out: Vec<Outbound>) {
        if out.is_empty() {
            return;
        }
        let boxes = self.outboxes.lock().expect("outboxes poisoned");
        for Outbound { to, msg } in out {
            if let Some(tx) = boxes.get(&to) {
                let _ = tx.send(msg);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| wire::DEFAULT_PORT.to_string());
    let max_peers: usize = std::env::var("BIMS_MAX_PEERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(256);

    let addr = format!("{host}:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bims-server: cannot bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "bims-server: listening on {addr} (protocol {})",
        wire::PROTOCOL
    );

    let shared = Arc::new(Shared {
        hub: Mutex::new(Hub::new()),
        outboxes: Mutex::new(HashMap::new()),
    });

    // A line every few minutes, so `journalctl -u bims` answers "is
    // anybody playing" without attaching anything to the process.
    let mut census = tokio::time::interval(Duration::from_secs(300));
    census.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = census.tick() => {
                let hub = shared.hub.lock().expect("hub poisoned");
                println!(
                    "bims-server: {} connected, {} rooms",
                    hub.peer_count(),
                    hub.room_count()
                );
            }
            // systemd sends SIGTERM on stop and on a rebuild's restart.
            // Taking it here means the process exits instead of being
            // killed, so the unit's stop is clean rather than a
            // ninety-second wait and a SIGKILL.
            _ = tokio::signal::ctrl_c() => {
                println!("bims-server: shutting down");
                return;
            }
            accepted = listener.accept() => {
                let (stream, addr) = match accepted {
                    Ok(pair) => pair,
                    // A failed accept is nearly always one dead client,
                    // not a dead listener, so log and carry on.
                    Err(e) => { eprintln!("bims-server: accept failed: {e}"); continue; }
                };
                if shared.hub.lock().expect("hub poisoned").peer_count() >= max_peers {
                    // Dropped without a WebSocket handshake: there is
                    // nowhere to put a polite refusal before one, and
                    // finishing a handshake purely to say no is work an
                    // attacker chooses.
                    continue;
                }
                let shared = Arc::clone(&shared);
                tokio::spawn(async move {
                    if let Err(e) = serve(shared, stream, addr).await {
                        // Ordinary disconnects land here too, so this is a
                        // debug line rather than a warning.
                        eprintln!("bims-server: {addr} closed: {e}");
                    }
                });
            }
        }
    }
}

async fn serve(
    shared: Arc<Shared>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Nagle batches small writes, which is the wrong trade for a stream of
    // one message a frame: it adds tens of milliseconds to exactly the
    // messages that are most sensitive to it.
    stream.set_nodelay(true)?;

    let ws = tokio::time::timeout(HANDSHAKE_TIMEOUT, tokio_tungstenite::accept_async(stream))
        .await
        .map_err(|_| "handshake timed out")??;
    let (mut sink, mut source) = ws.split();

    let id = shared.hub.lock().expect("hub poisoned").connect();
    let (tx, mut rx) = mpsc::unbounded_channel();
    shared
        .outboxes
        .lock()
        .expect("outboxes poisoned")
        .insert(id, tx);

    // Writer task. Split off so that a broadcast to a slow client cannot
    // hold up the reader that is serving everybody else.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let Ok(bytes) = encode(&msg) else {
                // Encoding our own message failing is a bug on this side,
                // not something the client did. Nothing useful to send
                // instead.
                continue;
            };
            if sink.send(Message::Binary(bytes)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let result = read_loop(&shared, id, &mut source).await;

    // Cleanup runs on every exit path — error, clean close, or timeout —
    // because a peer left in the hub holds a berth in a room for ever.
    shared
        .outboxes
        .lock()
        .expect("outboxes poisoned")
        .remove(&id);
    let out = shared.hub.lock().expect("hub poisoned").disconnect(id);
    shared.dispatch(out);
    writer.abort();

    let _ = addr;
    result
}

async fn read_loop(
    shared: &Arc<Shared>,
    id: PeerId,
    source: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let next = tokio::time::timeout(IDLE_TIMEOUT, source.next()).await;
        let Ok(frame) = next else {
            return Err("idle too long".into());
        };
        let Some(frame) = frame else {
            return Ok(()); // stream ended: an ordinary disconnect
        };

        let bytes = match frame? {
            Message::Binary(b) => b,
            // The game only ever sends binary. Text is either a browser
            // poking at the port or a client bug; either way there is
            // nothing to do with it, and answering would make this an echo
            // service.
            Message::Text(_) => continue,
            Message::Close(_) => return Ok(()),
            // Ping/Pong are answered by tungstenite itself.
            _ => continue,
        };

        if bytes.len() > MAX_PAYLOAD {
            return Err("frame over the payload limit".into());
        }

        let Ok(msg) = decode::<ClientCtl>(&bytes) else {
            // Undecodable means the peer is not speaking this protocol.
            // Dropping beats staying connected to something that will
            // never make sense.
            return Err("undecodable frame".into());
        };

        let out = shared.hub.lock().expect("hub poisoned").handle(id, msg);
        shared.dispatch(out);
    }
}

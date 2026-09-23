//! The wire between a Bims client and the relay it finds its crew through.
//!
//! Only the *control* plane lives here: saying hello, opening a room,
//! joining one by its code, and asking the relay to pass a blob of bytes
//! to somebody else in the room. What is in the blob is the game's
//! business and not the relay's — `crates/app/src/net.rs` is where every
//! message about settings, edits, orders and steps is defined.
//!
//! That split is the whole design. The relay never learns what a ship is,
//! so a new part, a new command or a new screen is a client-side change a
//! running server plays host to without being rebuilt. The alternative — a
//! server that understood the game — would have made every commit to the
//! rules a deployment, and the rules crates are the one thing that has to
//! come out identical everywhere (`crates/app/CLAUDE.md`).
//!
//! **No words come out of the relay.** A refusal is a [`Refusal`] code and
//! a room closing is a [`Closed`] code; every sentence is the app's
//! (`names.rs`), the way every refusal of the world's is. A name is the
//! one string here, and it is the player's own, echoed back to the room.

use serde::{Deserialize, Serialize};

/// Bumped whenever anything in this file, or in the app's own message set,
/// stops meaning what it used to.
///
/// Checked at `Hello` and refused on mismatch. Two clients on different
/// versions of the game desync in ways that look like bugs in the room,
/// which is a miserable thing to debug from a bug report; being told
/// "this server speaks protocol 3, you speak 2" is not.
pub const PROTOCOL: u32 = 19;

/// Where the relay lives. `BIMS_SERVER` in the environment overrides it
/// — `ws://127.0.0.1:8792` for one on the same machine.
pub const DEFAULT_SERVER: &str = "wss://bims.buggly.de";

/// The port the relay listens on by default, and the one the server box's
/// nginx proxies `bims.buggly.de` to.
pub const DEFAULT_PORT: u16 = 8792;

/// How long a room code is, and what it is drawn from: read out loud down
/// a phone line, so the alphabet leaves out the pairs that are heard
/// wrong — O/0, I/1. The lobby's own rule, since before there was a wire.
pub const CODE_LENGTH: usize = 6;
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// How many may be in one room: the lobby's four berths, one crew member
/// a player.
pub const ROOM_SIZE: usize = 4;

/// Longest name accepted, in characters. Names are echoed to everybody
/// in the room, so this is a limit on what one person can make the other
/// three draw.
pub const MAX_NAME: usize = 24;

/// Largest payload the relay passes along, in bytes. The whole world as
/// text, for a late joiner, is the big one — a few megabytes at most —
/// and this is enough room for that while still being a number a bad
/// client cannot use to allocate the box's memory out from under it.
pub const MAX_PAYLOAD: usize = 16 * 1024 * 1024;

/// Who a client is, for as long as it stays connected. Handed out by the
/// relay and never reused within a run. There is no account and no name
/// that has to be unique; nothing outlives the socket.
pub type PeerId = u32;

/// Where a relayed payload is going.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum To {
    /// To whoever opened the room. What a guest sends its asks to.
    Host,
    /// To one peer. The host answering one guest — a refusal, or the
    /// world for somebody who has just joined.
    Peer(PeerId),
    /// To everybody in the room but the sender. The host's broadcast.
    All,
}

/// Client to relay.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientCtl {
    /// First message on the socket. Anything before it is refused.
    Hello { protocol: u32, name: String },
    /// Open a fresh room and be its host.
    Create,
    /// Walk into an existing one. Codes are matched as [`normalise_code`]
    /// leaves them, since they are read aloud and typed by hand.
    Join { code: String },
    /// Leave without dropping the connection, so the menu can go back to
    /// its buttons without reconnecting.
    Leave,
    /// The host pressed Start: the crew are dealt, and nobody else joins
    /// this room. The host's alone to say.
    Begin,
    /// Pass these bytes on. The relay does not look inside.
    Relay { to: To, payload: Vec<u8> },
    /// Answered with `Pong`. What keeps an idle lobby's connection from
    /// being reaped by something in the middle, and measures the round
    /// trip.
    Ping { stamp: u64 },
}

/// Relay to client.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerCtl {
    /// Hello accepted.
    Welcome {
        you: PeerId,
        protocol: u32,
    },
    /// ...or something was not. A code, for the app to name.
    Rejected {
        why: Refusal,
    },
    /// You are in a room now, whether you made it or joined it. Sent to
    /// the one who arrived; everybody already inside gets `RoomUpdate`.
    RoomJoined {
        code: String,
        host: PeerId,
        peers: Vec<PeerInfo>,
    },
    /// The room's roster changed. Always the whole roster rather than a
    /// difference: it is four entries at most, and a client that missed
    /// a difference would be wrong about who is playing until somebody
    /// else came or went.
    RoomUpdate {
        host: PeerId,
        peers: Vec<PeerInfo>,
    },
    /// The room is gone — the host left, or it emptied out.
    RoomClosed {
        why: Closed,
    },
    /// Somebody sent you a payload.
    Relayed {
        from: PeerId,
        payload: Vec<u8>,
    },
    Pong {
        stamp: u64,
    },
}

/// Why the relay said no. The app has a line for each (`names.rs`).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The client speaks another protocol than the server's; the
    /// server's is carried so the line can say which.
    Protocol { server: u32 },
    /// A message before `Hello`.
    HelloFirst,
    /// A second `Hello`.
    Greeted,
    /// A name that is empty once trimmed.
    NoName,
    /// A create or a join from inside a room: leave first.
    InRoom,
    /// No free codes — a relay holding a million live rooms has a bigger
    /// problem than this.
    NoCodes,
    /// A join with a code no room has.
    NoSuchRoom,
    /// A join to a room with its four berths taken.
    RoomFull,
    /// A join to a room whose game has begun: the ship is being laid out
    /// or flown, and a fifth pair of hands mid-design has no slot.
    Begun,
    /// A relay or a begin from outside any room.
    NotInRoom,
    /// A begin from a guest: the host starts the game.
    NotHost,
    /// A payload over [`MAX_PAYLOAD`].
    TooBig,
}

/// Why a room closed under its members.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Closed {
    /// The host left. The host *is* the game under this design — the
    /// clock and the one who applies every edit — and there is no state
    /// on the relay to hand over, so the room ends rather than promoting
    /// somebody. Telling the rest plainly beats leaving three people
    /// watching a world that stopped.
    HostLeft,
}

/// One member of a room, as far as the relay is concerned. Deliberately
/// thin: the relay tracks who is connected, and the *game* decides which
/// slot each of them plays. Putting a slot in here would have meant the
/// relay knowing what a crew is.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PeerInfo {
    pub id: PeerId,
    pub name: String,
}

/// Postcard both ways, so the two halves can never disagree about the
/// format.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(value)
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

/// Normalise a room code the way `Join` matches them: upper case, and the
/// spaces and dashes people put in by hand taken back out.
pub fn normalise_code(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Whether a string is a room code: six of the alphabet's letters.
pub fn is_code(text: &str) -> bool {
    text.len() == CODE_LENGTH && text.bytes().all(|b| CODE_ALPHABET.contains(&b))
}

/// Trim a name to something safe to draw in somebody else's lobby: no
/// control characters (a newline in a player list has no answer), and
/// the length capped by characters rather than bytes so a name of
/// accented letters is not cut in half mid-character.
pub fn tidy_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_NAME)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_survives_being_read_aloud_and_the_alphabet_has_no_lookalikes() {
        assert_eq!(normalise_code("q7-fkab"), "Q7FKAB");
        assert_eq!(normalise_code(" Q7FK AB "), "Q7FKAB");
        assert!(is_code("Q7FKAB"));
        assert!(!is_code("O0I1AB"));
        assert!(!is_code("ABCDE"));
        for c in b"01OI" {
            assert!(
                !CODE_ALPHABET.contains(c),
                "{} is a character somebody will mis-hear",
                *c as char
            );
        }
        assert_eq!(tidy_name("  Jam\nes  "), "James");
        assert_eq!(tidy_name(&"é".repeat(40)).chars().count(), MAX_NAME);
    }

    #[test]
    fn a_relayed_payload_and_a_refusal_round_trip() {
        let msg = ClientCtl::Relay {
            to: To::Host,
            payload: vec![9, 8, 7],
        };
        let bytes = encode(&msg).unwrap();
        let back: ClientCtl = decode(&bytes).unwrap();
        match back {
            ClientCtl::Relay { to, payload } => {
                assert_eq!(to, To::Host);
                assert_eq!(payload, vec![9, 8, 7]);
            }
            other => panic!("came back as {other:?}"),
        }
        let no = ServerCtl::Rejected {
            why: Refusal::Protocol { server: 7 },
        };
        let back: ServerCtl = decode(&encode(&no).unwrap()).unwrap();
        assert!(matches!(
            back,
            ServerCtl::Rejected {
                why: Refusal::Protocol { server: 7 }
            }
        ));
    }
}

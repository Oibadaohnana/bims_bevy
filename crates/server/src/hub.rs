//! Rooms, and who is in them.
//!
//! Written as a pure state machine: messages go in, messages to send back
//! come out, and nothing in here touches a socket or a clock. That is what
//! makes the awkward cases — the host leaving mid-game, two people racing
//! for the last berth, a client asking to relay to a room it was just put
//! out of — testable at all, and each has a test at the bottom of the file.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher, RandomState};

use wire::{
    CODE_ALPHABET, CODE_LENGTH, ClientCtl, Closed, MAX_PAYLOAD, PROTOCOL, PeerId, PeerInfo,
    ROOM_SIZE, Refusal, ServerCtl, To, normalise_code, tidy_name,
};

/// One message the caller should put on one socket.
#[derive(Debug, Clone)]
pub struct Outbound {
    pub to: PeerId,
    pub msg: ServerCtl,
}

/// A connected client, from the relay's point of view.
struct Peer {
    name: String,
    /// `None` until `Hello` lands. Everything but `Hello` is refused while
    /// it is, which is what stops an unauthenticated socket opening rooms.
    greeted: bool,
    room: Option<String>,
}

struct Room {
    host: PeerId,
    /// In join order, host first. Stable, so the lobby does not reshuffle
    /// itself under the players every time somebody arrives.
    members: Vec<PeerId>,
    /// The host has pressed Start: the crew are laying the ship out, or
    /// flying it, and a fifth pair of hands has no slot. Nobody joins.
    begun: bool,
}

#[derive(Default)]
pub struct Hub {
    peers: HashMap<PeerId, Peer>,
    rooms: HashMap<String, Room>,
    next_id: PeerId,
    /// A counter the codes are rolled off, through a hasher seeded from
    /// the operating system: two relays started a second apart deal
    /// different codes, and nobody guesses the next one from the last.
    rolls: u64,
    salt: RandomState,
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a freshly accepted socket and hand back the id it will be
    /// known by. No `Welcome` yet — that waits for `Hello`, because until
    /// then we do not know whether the client even speaks this protocol.
    pub fn connect(&mut self) -> PeerId {
        // Wrapping is not a real concern at one per connection, but an id
        // that silently collided with a live peer would cross two players'
        // traffic over, so skip any that is somehow still in use.
        loop {
            self.next_id = self.next_id.wrapping_add(1);
            if !self.peers.contains_key(&self.next_id) {
                break;
            }
        }
        let id = self.next_id;
        self.peers.insert(
            id,
            Peer {
                name: String::new(),
                greeted: false,
                room: None,
            },
        );
        id
    }

    /// The socket is gone. Takes the peer out of whatever room it was in
    /// and tells the rest.
    pub fn disconnect(&mut self, id: PeerId) -> Vec<Outbound> {
        let out = self.leave_room(id);
        self.peers.remove(&id);
        out
    }

    pub fn handle(&mut self, id: PeerId, msg: ClientCtl) -> Vec<Outbound> {
        // A peer that has already been dropped can still have a frame in
        // flight. Nothing to do, and nobody to tell.
        if !self.peers.contains_key(&id) {
            return Vec::new();
        }

        // Ping is answered whether or not the client has said hello: it is
        // how a connection proves it is still alive, and refusing it before
        // the handshake would mean a slow client could be reaped for not
        // having finished handshaking.
        if let ClientCtl::Ping { stamp } = msg {
            return vec![Outbound {
                to: id,
                msg: ServerCtl::Pong { stamp },
            }];
        }

        if !self.peers[&id].greeted {
            return match msg {
                ClientCtl::Hello { protocol, name } => self.hello(id, protocol, name),
                _ => reject(id, Refusal::HelloFirst),
            };
        }

        match msg {
            // Twice is a client bug rather than an attack, but a second
            // hello could rename a peer mid-room, so it is refused.
            ClientCtl::Hello { .. } => reject(id, Refusal::Greeted),
            ClientCtl::Create => self.create_room(id),
            ClientCtl::Join { code } => self.join_room(id, &code),
            ClientCtl::Leave => self.leave_room(id),
            ClientCtl::Begin => self.begin(id),
            ClientCtl::Relay { to, payload } => self.relay(id, to, payload),
            ClientCtl::Ping { .. } => unreachable!("handled above"),
        }
    }

    fn hello(&mut self, id: PeerId, protocol: u32, name: String) -> Vec<Outbound> {
        if protocol != PROTOCOL {
            return reject(id, Refusal::Protocol { server: PROTOCOL });
        }
        let name = tidy_name(&name);
        if name.is_empty() {
            return reject(id, Refusal::NoName);
        }
        let peer = self.peers.get_mut(&id).expect("checked in handle");
        peer.name = name;
        peer.greeted = true;
        vec![Outbound {
            to: id,
            msg: ServerCtl::Welcome {
                you: id,
                protocol: PROTOCOL,
            },
        }]
    }

    fn create_room(&mut self, id: PeerId) -> Vec<Outbound> {
        // Already somewhere: leaving first is the client's job, so that
        // "create" can never silently abandon a room full of people.
        if self.peers[&id].room.is_some() {
            return reject(id, Refusal::InRoom);
        }
        let Some(code) = self.free_code() else {
            return reject(id, Refusal::NoCodes);
        };
        self.rooms.insert(
            code.clone(),
            Room {
                host: id,
                members: vec![id],
                begun: false,
            },
        );
        self.peers.get_mut(&id).expect("checked in handle").room = Some(code.clone());
        vec![Outbound {
            to: id,
            msg: ServerCtl::RoomJoined {
                code,
                host: id,
                peers: vec![self.info(id)],
            },
        }]
    }

    fn join_room(&mut self, id: PeerId, raw: &str) -> Vec<Outbound> {
        if self.peers[&id].room.is_some() {
            return reject(id, Refusal::InRoom);
        }
        let code = normalise_code(raw);
        let Some(room) = self.rooms.get_mut(&code) else {
            return reject(id, Refusal::NoSuchRoom);
        };
        if room.begun {
            return reject(id, Refusal::Begun);
        }
        if room.members.len() >= ROOM_SIZE {
            return reject(id, Refusal::RoomFull);
        }
        room.members.push(id);
        let host = room.host;
        self.peers.get_mut(&id).expect("checked in handle").room = Some(code.clone());

        let roster = self.roster(&code);
        let members = self.rooms[&code].members.clone();

        // The joiner is told where they landed; everybody already inside
        // just needs the new roster. Two message kinds rather than one so a
        // client can tell "I am in" from "somebody else arrived" without
        // remembering whether it had asked.
        let mut out = vec![Outbound {
            to: id,
            msg: ServerCtl::RoomJoined {
                code,
                host,
                peers: roster.clone(),
            },
        }];
        out.extend(members.iter().filter(|&&m| m != id).map(|&m| Outbound {
            to: m,
            msg: ServerCtl::RoomUpdate {
                host,
                peers: roster.clone(),
            },
        }));
        out
    }

    /// Take a peer out of its room, closing the room if it was the host's.
    ///
    /// The host leaving ends the game for everybody rather than promoting
    /// somebody: the host *is* the clock under this design, and there is
    /// no state on the relay to hand over. Telling the room plainly beats
    /// leaving three people watching a world that stopped.
    fn leave_room(&mut self, id: PeerId) -> Vec<Outbound> {
        let Some(peer) = self.peers.get_mut(&id) else {
            return Vec::new();
        };
        let Some(code) = peer.room.take() else {
            return Vec::new();
        };
        let Some(room) = self.rooms.get_mut(&code) else {
            return Vec::new();
        };

        if room.host == id {
            let others: Vec<PeerId> = room.members.iter().copied().filter(|&m| m != id).collect();
            self.rooms.remove(&code);
            for other in &others {
                if let Some(p) = self.peers.get_mut(other) {
                    p.room = None;
                }
            }
            return others
                .into_iter()
                .map(|m| Outbound {
                    to: m,
                    msg: ServerCtl::RoomClosed {
                        why: Closed::HostLeft,
                    },
                })
                .collect();
        }

        room.members.retain(|&m| m != id);
        let host = room.host;
        let members = room.members.clone();
        let roster = self.roster(&code);
        members
            .into_iter()
            .map(|m| Outbound {
                to: m,
                msg: ServerCtl::RoomUpdate {
                    host,
                    peers: roster.clone(),
                },
            })
            .collect()
    }

    /// The host pressed Start: the room is closed to joiners from here.
    /// The relay knows nothing of what a Start is; it is told, so that a
    /// late joiner is refused rather than handed a lobby that has gone.
    fn begin(&mut self, id: PeerId) -> Vec<Outbound> {
        let Some(code) = self.peers[&id].room.clone() else {
            return reject(id, Refusal::NotInRoom);
        };
        let room = self.rooms.get_mut(&code).expect("a peer's room exists");
        if room.host != id {
            return reject(id, Refusal::NotHost);
        }
        room.begun = true;
        Vec::new()
    }

    /// Pass a payload along without looking inside it.
    ///
    /// Two things are checked besides the size: that the sender is in a
    /// room, and that the recipient is in the same one. Without the
    /// second, a peer id is all anybody would need to push bytes into a
    /// stranger's game.
    fn relay(&mut self, id: PeerId, to: To, payload: Vec<u8>) -> Vec<Outbound> {
        if payload.len() > MAX_PAYLOAD {
            return reject(id, Refusal::TooBig);
        }
        let Some(code) = self.peers[&id].room.clone() else {
            return reject(id, Refusal::NotInRoom);
        };
        let room = &self.rooms[&code];

        let recipients: Vec<PeerId> = match to {
            To::Host => vec![room.host].into_iter().filter(|&h| h != id).collect(),
            To::All => room.members.iter().copied().filter(|&m| m != id).collect(),
            To::Peer(target) => {
                if room.members.contains(&target) && target != id {
                    vec![target]
                } else {
                    // Silently dropped rather than refused. A peer that
                    // left half a frame ago is an ordinary race, and
                    // answering it with an error would put a line in the
                    // host's log every time somebody quit.
                    Vec::new()
                }
            }
        };

        recipients
            .into_iter()
            .map(|m| Outbound {
                to: m,
                msg: ServerCtl::Relayed {
                    from: id,
                    payload: payload.clone(),
                },
            })
            .collect()
    }

    fn info(&self, id: PeerId) -> PeerInfo {
        PeerInfo {
            id,
            name: self.peers[&id].name.clone(),
        }
    }

    fn roster(&self, code: &str) -> Vec<PeerInfo> {
        self.rooms[code]
            .members
            .iter()
            .map(|&m| self.info(m))
            .collect()
    }

    /// A code nobody is using. Gives up rather than looping for ever, on
    /// the theory that a relay holding a million live rooms has a bigger
    /// problem than this function.
    fn free_code(&mut self) -> Option<String> {
        (0..64).find_map(|_| {
            self.rolls = self.rolls.wrapping_add(1);
            let mut hasher = self.salt.build_hasher();
            hasher.write_u64(self.rolls);
            let mut roll = hasher.finish();
            let code: String = (0..CODE_LENGTH)
                .map(|_| {
                    let c = CODE_ALPHABET[(roll % CODE_ALPHABET.len() as u64) as usize] as char;
                    roll /= CODE_ALPHABET.len() as u64;
                    c
                })
                .collect();
            (!self.rooms.contains_key(&code)).then_some(code)
        })
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}

fn reject(id: PeerId, why: Refusal) -> Vec<Outbound> {
    vec![Outbound {
        to: id,
        msg: ServerCtl::Rejected { why },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Connect a peer and get it past the handshake.
    fn greeted(hub: &mut Hub, name: &str) -> PeerId {
        let id = hub.connect();
        hub.handle(
            id,
            ClientCtl::Hello {
                protocol: PROTOCOL,
                name: name.into(),
            },
        );
        id
    }

    fn code_of(out: &[Outbound]) -> String {
        for o in out {
            if let ServerCtl::RoomJoined { code, .. } = &o.msg {
                return code.clone();
            }
        }
        panic!("no room was opened");
    }

    fn refused(out: &[Outbound], why: Refusal) -> bool {
        out.iter()
            .any(|o| matches!(o.msg, ServerCtl::Rejected { why: w } if w == why))
    }

    #[test]
    fn nothing_works_before_hello_and_the_protocol_is_checked() {
        let mut hub = Hub::new();
        let id = hub.connect();
        assert!(refused(
            &hub.handle(id, ClientCtl::Create),
            Refusal::HelloFirst
        ));
        assert_eq!(hub.room_count(), 0);
        let out = hub.handle(
            id,
            ClientCtl::Hello {
                protocol: PROTOCOL + 1,
                name: "x".into(),
            },
        );
        assert!(refused(&out, Refusal::Protocol { server: PROTOCOL }));
        let out = hub.handle(
            id,
            ClientCtl::Hello {
                protocol: PROTOCOL,
                name: "  \n ".into(),
            },
        );
        assert!(refused(&out, Refusal::NoName));
        // Ping is answered regardless.
        let out = hub.handle(id, ClientCtl::Ping { stamp: 9 });
        assert!(matches!(out[0].msg, ServerCtl::Pong { stamp: 9 }));
    }

    #[test]
    fn a_room_is_joined_by_its_code_and_the_roster_goes_round() {
        let mut hub = Hub::new();
        let host = greeted(&mut hub, "James");
        let out = hub.handle(host, ClientCtl::Create);
        let code = code_of(&out);
        assert!(wire::is_code(&code), "{code}");
        assert!(refused(
            &hub.handle(host, ClientCtl::Create),
            Refusal::InRoom
        ));

        let guest = greeted(&mut hub, "Kate");
        let out = hub.handle(
            guest,
            ClientCtl::Join {
                code: code.to_lowercase(),
            },
        );
        // The joiner gets RoomJoined, the host RoomUpdate, both with two.
        assert!(out.iter().any(|o| o.to == guest
            && matches!(&o.msg, ServerCtl::RoomJoined { peers, host: h, .. } if peers.len() == 2 && *h == host)));
        assert!(out.iter().any(|o| o.to == host
            && matches!(&o.msg, ServerCtl::RoomUpdate { peers, .. } if peers.len() == 2 && peers[1].name == "Kate")));

        let nobody = greeted(&mut hub, "Nobody");
        assert!(refused(
            &hub.handle(
                nobody,
                ClientCtl::Join {
                    code: "ZZZZZZ".into()
                }
            ),
            Refusal::NoSuchRoom
        ));
        // Four berths, no more.
        for n in 0..2 {
            let p = greeted(&mut hub, &format!("P{n}"));
            let out = hub.handle(p, ClientCtl::Join { code: code.clone() });
            assert!(!refused(&out, Refusal::RoomFull));
        }
        assert!(refused(
            &hub.handle(nobody, ClientCtl::Join { code: code.clone() }),
            Refusal::RoomFull
        ));
    }

    #[test]
    fn bytes_go_to_the_host_or_to_everybody_and_never_out_of_the_room() {
        let mut hub = Hub::new();
        let host = greeted(&mut hub, "James");
        let code = code_of(&hub.handle(host, ClientCtl::Create));
        let guest = greeted(&mut hub, "Kate");
        hub.handle(guest, ClientCtl::Join { code: code.clone() });
        let stranger = greeted(&mut hub, "Nobody");
        let other = code_of(&hub.handle(stranger, ClientCtl::Create));
        assert_ne!(code, other);

        let out = hub.handle(
            guest,
            ClientCtl::Relay {
                to: To::Host,
                payload: vec![1, 2],
            },
        );
        assert_eq!(out.len(), 1);
        assert!(
            out[0].to == host
                && matches!(&out[0].msg, ServerCtl::Relayed { from, payload } if *from == guest && payload == &[1, 2])
        );
        // The host to everybody: the guest alone gets it.
        let out = hub.handle(
            host,
            ClientCtl::Relay {
                to: To::All,
                payload: vec![3],
            },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].to, guest);
        // Begun is the host's word, and shuts the door on joiners.
        assert!(refused(
            &hub.handle(guest, ClientCtl::Begin),
            Refusal::NotHost
        ));
        assert!(hub.handle(host, ClientCtl::Begin).is_empty());
        let late = greeted(&mut hub, "Late");
        assert!(refused(
            &hub.handle(late, ClientCtl::Join { code: code.clone() }),
            Refusal::Begun
        ));
        // To a peer in another room: dropped, silently.
        let out = hub.handle(
            host,
            ClientCtl::Relay {
                to: To::Peer(stranger),
                payload: vec![4],
            },
        );
        assert!(out.is_empty());
        // From outside any room: refused.
        hub.handle(stranger, ClientCtl::Leave);
        assert!(refused(
            &hub.handle(
                stranger,
                ClientCtl::Relay {
                    to: To::All,
                    payload: vec![5],
                }
            ),
            Refusal::NotInRoom
        ));
        assert!(refused(
            &hub.handle(
                host,
                ClientCtl::Relay {
                    to: To::All,
                    payload: vec![0; MAX_PAYLOAD + 1],
                }
            ),
            Refusal::TooBig
        ));
    }

    #[test]
    fn the_host_leaving_closes_the_room_and_a_guest_leaving_does_not() {
        let mut hub = Hub::new();
        let host = greeted(&mut hub, "James");
        let code = code_of(&hub.handle(host, ClientCtl::Create));
        let a = greeted(&mut hub, "A");
        let b = greeted(&mut hub, "B");
        hub.handle(a, ClientCtl::Join { code: code.clone() });
        hub.handle(b, ClientCtl::Join { code: code.clone() });
        let out = hub.disconnect(a);
        assert!(
            out.iter()
                .all(|o| matches!(&o.msg, ServerCtl::RoomUpdate { peers, .. } if peers.len() == 2))
        );
        assert_eq!(hub.room_count(), 1);
        let out = hub.handle(host, ClientCtl::Leave);
        assert_eq!(out.len(), 1);
        assert!(
            out[0].to == b
                && matches!(
                    out[0].msg,
                    ServerCtl::RoomClosed {
                        why: Closed::HostLeft
                    }
                )
        );
        assert_eq!(hub.room_count(), 0);
        // B is out of any room now, and can open one of its own.
        code_of(&hub.handle(b, ClientCtl::Create));
        assert_eq!(hub.room_count(), 1);
        hub.disconnect(b);
        assert_eq!(hub.room_count(), 0);
        assert_eq!(hub.peer_count(), 1);
    }
}

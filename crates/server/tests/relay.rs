//! The relay over a real socket: the binary started on a free port, two
//! clients speaking `wire` to it, a room opened and joined, bytes passed
//! along, and the room closing when the host hangs up.

use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use wire::{ClientCtl, Closed, PROTOCOL, PeerId, ServerCtl, To, decode, encode};

struct Relay {
    child: Child,
    port: u16,
}

impl Relay {
    /// The binary on a port nobody is using, and a wait until it answers.
    fn start() -> Relay {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = Command::new(env!("CARGO_BIN_EXE_bims-server"))
            .env("HOST", "127.0.0.1")
            .env("PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the relay starts");
        let began = Instant::now();
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(
                began.elapsed() < Duration::from_secs(10),
                "the relay never listened"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        Relay { child, port }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Client(WebSocket<MaybeTlsStream<TcpStream>>);

impl Client {
    fn connect(relay: &Relay, name: &str) -> (Client, PeerId) {
        let (socket, _) = tungstenite::connect(format!("ws://127.0.0.1:{}", relay.port)).unwrap();
        let mut client = Client(socket);
        client.send(ClientCtl::Hello {
            protocol: PROTOCOL,
            name: name.into(),
        });
        match client.read() {
            ServerCtl::Welcome { you, protocol } => {
                assert_eq!(protocol, PROTOCOL);
                (client, you)
            }
            other => panic!("no welcome: {other:?}"),
        }
    }

    fn send(&mut self, msg: ClientCtl) {
        self.0
            .send(Message::Binary(encode(&msg).unwrap().into()))
            .unwrap();
    }

    fn read(&mut self) -> ServerCtl {
        loop {
            match self.0.read().unwrap() {
                Message::Binary(bytes) => return decode(&bytes).unwrap(),
                _ => continue,
            }
        }
    }
}

#[test]
fn a_room_is_opened_joined_and_talked_through_and_closes_with_its_host() {
    let relay = Relay::start();
    let (mut host, host_id) = Client::connect(&relay, "James");
    host.send(ClientCtl::Create);
    let code = match host.read() {
        ServerCtl::RoomJoined { code, host, peers } => {
            assert_eq!(host, host_id);
            assert_eq!(peers.len(), 1);
            code
        }
        other => panic!("{other:?}"),
    };
    assert!(wire::is_code(&code));

    let (mut guest, guest_id) = Client::connect(&relay, "Kate");
    guest.send(ClientCtl::Join {
        code: code.to_lowercase(),
    });
    match guest.read() {
        ServerCtl::RoomJoined { host, peers, .. } => {
            assert_eq!(host, host_id);
            assert_eq!(peers.len(), 2);
            assert_eq!(peers[1].name, "Kate");
        }
        other => panic!("{other:?}"),
    }
    match host.read() {
        ServerCtl::RoomUpdate { peers, .. } => assert_eq!(peers.len(), 2),
        other => panic!("{other:?}"),
    }

    // Bytes go both ways, untouched.
    guest.send(ClientCtl::Relay {
        to: To::Host,
        payload: vec![1, 2, 3],
    });
    match host.read() {
        ServerCtl::Relayed { from, payload } => {
            assert_eq!(from, guest_id);
            assert_eq!(payload, vec![1, 2, 3]);
        }
        other => panic!("{other:?}"),
    }
    host.send(ClientCtl::Relay {
        to: To::All,
        payload: vec![9; 100_000],
    });
    match guest.read() {
        ServerCtl::Relayed { from, payload } => {
            assert_eq!(from, host_id);
            assert_eq!(payload.len(), 100_000);
        }
        other => panic!("{other:?}"),
    }
    // A ping is answered.
    guest.send(ClientCtl::Ping { stamp: 42 });
    assert!(matches!(guest.read(), ServerCtl::Pong { stamp: 42 }));

    // Begun, a third is refused; the host hanging up closes the room.
    host.send(ClientCtl::Begin);
    let (mut late, _) = Client::connect(&relay, "Late");
    late.send(ClientCtl::Join { code });
    assert!(matches!(
        late.read(),
        ServerCtl::Rejected {
            why: wire::Refusal::Begun
        }
    ));
    drop(host);
    match guest.read() {
        ServerCtl::RoomClosed { why } => assert_eq!(why, Closed::HostLeft),
        other => panic!("{other:?}"),
    }
}

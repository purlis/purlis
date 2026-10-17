// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Two session hosts on one machine, for the tests of the link between them (#2042): host A's
//! identity and host B's, each pinned by the other, a host B that answers the session protocol,
//! and a cable between them that a test can cut or freeze.

use std::sync::{Arc, Mutex};

use purlis_session_protocol::hostlink::{
    self, Connector, Handshake, Identity, LinkKey, Liveness, Pinned,
};
use purlis_session_protocol::session::{self, Answer, Chat, Command, Host, Peer, Refusal};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream, duplex};
use tokio::sync::{mpsc, watch};

pub const HOST_A: &str = "01JA0DESKT0P00000000000000";
pub const HOST_B: &str = "01JB0RUNNER0000000000000000";
pub const CHAT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";

/// A device's identity: its id, and the link key numbered `which`, made once per test process
/// so every call for one number is the same key.
pub fn identity(device: &str, which: u8) -> Identity {
    static KEYS: std::sync::LazyLock<Mutex<std::collections::HashMap<u8, LinkKey>>> =
        std::sync::LazyLock::new(Mutex::default);
    let key = KEYS
        .lock()
        .unwrap()
        .entry(which)
        .or_insert_with(|| LinkKey::generate().expect("a link key"))
        .clone();
    Identity {
        device: device.into(),
        key,
    }
}

pub fn host_a() -> Identity {
    identity(HOST_A, 0xA1)
}

pub fn host_b() -> Identity {
    identity(HOST_B, 0xB2)
}

/// What host A pinned for host B when it was paired.
pub fn b_as_pinned() -> Pinned {
    pinned(&host_b())
}

pub fn pinned(identity: &Identity) -> Pinned {
    Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    }
}

/// Host B's one peer: host A, the desktop that relays to it.
pub fn a_as_pinned() -> Pinned {
    pinned(&host_a())
}

/// A host that lists one chat, keeps every command it hears, and counts the links that ended.
#[derive(Clone, Default)]
pub struct Board {
    pub heard: Arc<Mutex<Vec<String>>>,
    pub links_ended: Arc<Mutex<usize>>,
}

impl Board {
    pub fn links_ended(&self) -> usize {
        *self.links_ended.lock().unwrap()
    }
}

impl Host for Board {
    async fn call(&self, command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        self.heard.lock().unwrap().push(command.word().to_owned());
        Ok(match command {
            Command::List => Answer::Chats(vec![Chat {
                chat: CHAT.into(),
                project: "01J9PRJCT00000000000000000".into(),
                state: "working".into(),
                ..Chat::default()
            }]),
            _ => Answer::Done,
        })
    }

    async fn link_ended(&self, _peer: &Peer) {
        *self.links_ended.lock().unwrap() += 1;
    }
}

/// What a cable is doing: carrying bytes, frozen (open, and carrying nothing either way, as a
/// half-open connection does), or cut (closed both ways).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cable {
    Carrying,
    Frozen,
    Cut,
}

/// One connection between the hosts through a cable the test holds: host A's end and host B's.
pub fn cable(state: watch::Receiver<Cable>) -> (DuplexStream, DuplexStream) {
    let (a, a_inside) = duplex(1 << 16);
    let (b, b_inside) = duplex(1 << 16);
    let (a_reads, a_writes) = tokio::io::split(a_inside);
    let (b_reads, b_writes) = tokio::io::split(b_inside);
    tokio::spawn(carry(a_reads, b_writes, state.clone()));
    tokio::spawn(carry(b_reads, a_writes, state));
    (a, b)
}

async fn carry(
    mut from: tokio::io::ReadHalf<DuplexStream>,
    mut to: tokio::io::WriteHalf<DuplexStream>,
    mut state: watch::Receiver<Cable>,
) {
    let mut buffer = vec![0u8; 1 << 14];
    loop {
        let now = *state.borrow_and_update();
        match now {
            Cable::Cut => return,
            Cable::Frozen => {
                if state.changed().await.is_err() {
                    // Nobody will thaw it: it stays frozen, and open, for good.
                    std::future::pending::<()>().await;
                }
                continue;
            }
            Cable::Carrying => {}
        }
        tokio::select! {
            read = from.read(&mut buffer) => match read {
                Ok(0) | Err(_) => return,
                Ok(read) => {
                    // A cable frozen mid-way holds what it read until it thaws or is cut.
                    while *state.borrow() == Cable::Frozen {
                        if state.changed().await.is_err() {
                            std::future::pending::<()>().await;
                        }
                    }
                    if *state.borrow() == Cable::Cut || to.write_all(&buffer[..read]).await.is_err() {
                        return;
                    }
                }
            },
            changed = state.changed() => if changed.is_err() { return },
        }
    }
}

/// Host B, answering every link that reaches it with the session protocol, as `remote-link`.
/// Each link it admitted, or the words it refused one with, is told on the returned channel.
pub fn serve_b(
    board: Board,
    peers: Pinned,
    liveness: Liveness,
) -> (
    mpsc::UnboundedSender<DuplexStream>,
    mpsc::UnboundedReceiver<Result<String, String>>,
) {
    let (arrive, mut arrivals) = mpsc::unbounded_channel::<DuplexStream>();
    let (tell, told) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(io) = arrivals.recv().await {
            let (board, peers, tell) = (board.clone(), peers.clone(), tell.clone());
            tokio::spawn(async move {
                match hostlink::answer(io, Handshake::THIS_BUILD, &host_b(), &peers, liveness).await
                {
                    Ok(answered) => {
                        let _ = tell.send(Ok(answered.device.clone()));
                        let _ = session::serve(answered.link, board, None).await;
                    }
                    Err(refused) => {
                        let _ = tell.send(Err(refused.to_string()));
                    }
                }
            });
        }
    });
    (arrive, told)
}

/// Host A's connector to host B: each connection a new cable, which the test's one switch cuts
/// or freezes; a cut cable stays cut, and the next connection gets a new one that carries.
#[derive(Clone)]
pub struct ToB {
    pub b: mpsc::UnboundedSender<DuplexStream>,
    pub switch: Arc<Mutex<watch::Sender<Cable>>>,
    /// The switches of the cables before the one in use: each carries on until its link ends.
    pub earlier: Arc<Mutex<Vec<watch::Sender<Cable>>>>,
    pub dials: Arc<Mutex<usize>>,
}

impl ToB {
    pub fn new(b: mpsc::UnboundedSender<DuplexStream>) -> ToB {
        ToB {
            b,
            switch: Arc::new(Mutex::new(watch::channel(Cable::Carrying).0)),
            earlier: Arc::default(),
            dials: Arc::default(),
        }
    }

    /// Cut or freeze the cable in use now.
    pub fn set(&self, state: Cable) {
        let _ = self.switch.lock().unwrap().send(state);
    }

    pub fn dials(&self) -> usize {
        *self.dials.lock().unwrap()
    }
}

impl Connector for ToB {
    type Io = DuplexStream;

    fn connect(&self) -> futures::future::BoxFuture<'static, std::io::Result<DuplexStream>> {
        let (switch, _) = watch::channel(Cable::Carrying);
        let state = switch.subscribe();
        let earlier = std::mem::replace(&mut *self.switch.lock().unwrap(), switch);
        self.earlier.lock().unwrap().push(earlier);
        *self.dials.lock().unwrap() += 1;
        let (a, b) = cable(state);
        let sent = self.b.send(b);
        Box::pin(async move {
            sent.map_err(|_| std::io::Error::other("host B is gone"))?;
            Ok(a)
        })
    }
}

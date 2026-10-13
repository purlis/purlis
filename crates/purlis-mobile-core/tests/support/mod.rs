//! A real gateway in process (iOS design §8): the protocol crate's `Gateway` over a desk the
//! test sets, pins the test fills, and a host that records what the phone sent and opens the
//! streams a desktop would — all over in-process streams, so no port is bound.

#![allow(dead_code)]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
// `Future` and `Pin` are the desk's `offered` answer; `Dialed` carries the dials'.
use std::time::{Duration, Instant};

use bytes::Bytes;
use purlis_mobile_core::Error;
use purlis_mobile_core::hosts::Host;
use purlis_mobile_core::payload::Payload;
use purlis_mobile_core::session::{Dial, Dialed, Io};
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::gateway::{Gateway, Heard, Offered, PairingDesk, Pins, Window};
use purlis_session_protocol::noise::Identity;
use purlis_session_protocol::pairing::{Code, Verdict};
use purlis_session_protocol::session::{
    self, Answer, Attached, Chat, Command, Host as SessionHost, Peer, Pushed, Refusal, Since,
    TimelineAttached,
};
use purlis_session_protocol::timeline::{self, Choice, Item, TimelineId, What};
use purlis_session_protocol::view::{self, ViewId};
use tokio::io::{DuplexStream, duplex};
use tokio::sync::{mpsc, watch};

pub const HOST: &str = "01J9DEVCE00000000000000000";
pub const KEPT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";
pub const PROJECT: &str = "01J9PROJECT000000000000000";

pub fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Default)]
pub struct Peers(pub Mutex<HashMap<[u8; 32], Scope>>);

impl Pins for Peers {
    fn scope_of(&self, key: &[u8; 32]) -> Option<Scope> {
        self.0.lock().unwrap().get(key).copied()
    }
}

/// A desk with one live window, whose Pair the test decided in advance: the person confirms,
/// or does not.
pub struct Desk {
    code: Code,
    ends: Instant,
    taken: Mutex<bool>,
    confirms: bool,
    peers: Arc<Peers>,
    pub offered: Mutex<Vec<Offered>>,
}

impl PairingDesk for Desk {
    fn take(&self) -> Option<Window> {
        let mut taken = self.taken.lock().unwrap();
        if *taken {
            return None;
        }
        *taken = true;
        Some(Window {
            code: self.code.clone(),
            ends: self.ends,
        })
    }

    fn withdraw(&self, _offer: &Offered) {}

    fn offered(&self, offer: Offered) -> Pin<Box<dyn Future<Output = Verdict> + Send>> {
        self.offered.lock().unwrap().push(offer.clone());
        let verdict = if self.confirms {
            self.peers
                .0
                .lock()
                .unwrap()
                .insert(offer.key, Scope::RemoteDevice);
            Verdict::Paired {
                device: HOST.into(),
            }
        } else {
            Verdict::Refused {
                why: "not confirmed on the desktop".into(),
            }
        };
        Box::pin(async move { verdict })
    }
}

/// What the host keeps alive so a stream it opened is written out.
enum Kept {
    View(view::Feed),
    Timeline(timeline::Feed),
}

/// A desktop's host as the phone sees it: one level-2 chat, a view with a screen and a live
/// line, a timeline with a turn and an ask, and every command recorded.
#[derive(Clone, Default)]
pub struct FakeHost {
    pub calls: Arc<Mutex<Vec<Command>>>,
    kept: Arc<Mutex<Vec<Kept>>>,
    pub events: Arc<Mutex<Option<mpsc::Sender<Pushed>>>>,
    pub since: Arc<Mutex<Option<Since>>>,
}

impl FakeHost {
    pub fn calls(&self) -> Vec<Command> {
        self.calls.lock().unwrap().clone()
    }

    /// Push an event to a subscribed phone.
    pub async fn push(&self, event: session::Event) {
        let tx = self.events.lock().unwrap().clone().expect("subscribed");
        tx.send(Pushed::Event(event)).await.unwrap();
    }

    /// The desktop ends every view and timeline it opened (a view that fell behind, a chat
    /// that ended): their feeds go, so their streams close.
    pub fn end_streams(&self) {
        self.kept.lock().unwrap().clear();
    }
}

pub fn the_chat() -> Chat {
    Chat {
        chat: KEPT.into(),
        project: PROJECT.into(),
        project_path: Some("/home/dev/plane".into()),
        title: Some("one".into()),
        state: "idle".into(),
        workspace: Some("login".into()),
        level: Some(2),
    }
}

impl SessionHost for FakeHost {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        self.calls.lock().unwrap().push(command.clone());
        let failed = |e: purlis_session_protocol::link::LinkError| Refusal {
            code: "link_failed".into(),
            why: e.to_string(),
        };
        match command {
            Command::List => Ok(Answer::Chats(vec![the_chat()])),
            Command::Attach { .. } => {
                let feed = view::Attacher::new(peer.views().clone(), view::Limits::default())
                    .attach(ViewId(1), Bytes::from("screen\r\n"))
                    .await
                    .map_err(failed)?;
                let _ = feed.push(Bytes::from("live\r\n"));
                self.kept.lock().unwrap().push(Kept::View(feed));
                Ok(Answer::Attached(Attached {
                    view: 1,
                    cols: 80,
                    rows: 24,
                }))
            }
            Command::AttachTimeline { .. } => {
                let snapshot = vec![
                    Item {
                        level: 2,
                        what: What::Turn {
                            turn: "r1".into(),
                            began: true,
                            stop_reason: None,
                        },
                    },
                    Item {
                        level: 2,
                        what: What::Ask {
                            ask: "a1".into(),
                            summary: "Allow cargo test?".into(),
                            choices: vec![
                                Choice {
                                    id: "allow".into(),
                                    label: "Allow".into(),
                                    lasts: false,
                                },
                                Choice {
                                    id: "always".into(),
                                    label: "Allow from now on".into(),
                                    lasts: true,
                                },
                            ],
                            elicits_secret: false,
                        },
                    },
                ];
                let feed =
                    timeline::Attacher::new(peer.views().clone(), timeline::Limits::default())
                        .attach(TimelineId(1), snapshot)
                        .await
                        .map_err(failed)?;
                let _ = feed.push(&Item {
                    level: 2,
                    what: What::AskClosed {
                        ask: "a1".into(),
                        by: "desktop".into(),
                    },
                });
                self.kept.lock().unwrap().push(Kept::Timeline(feed));
                Ok(Answer::TimelineAttached(TimelineAttached {
                    timeline: 1,
                    level: 2,
                }))
            }
            _ => Ok(Answer::Done),
        }
    }

    async fn elicits_a_secret(&self, _chat: &str, _ask: &str) -> bool {
        false
    }

    async fn choice_lasts(&self, _chat: &str, _ask: &str, answer: &str) -> bool {
        answer == "always"
    }

    async fn subscribe(
        &self,
        since: Since,
        _peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let (tx, rx) = mpsc::channel(16);
        *self.events.lock().unwrap() = Some(tx);
        *self.since.lock().unwrap() = Some(since);
        Ok(rx)
    }
}

pub struct Rig {
    pub gateway: Arc<Gateway<FakeHost>>,
    pub identity: Identity,
    pub peers: Arc<Peers>,
    pub host: FakeHost,
    pub heard: Arc<Mutex<Vec<Heard>>>,
    pub code: Code,
    pub revoke: watch::Sender<u64>,
    /// Every link the gateway is serving, so a test can take the desktop away.
    pub serving: Mutex<Vec<tokio::task::JoinHandle<Result<(), String>>>>,
}

/// A gateway whose one pairing window the person confirms, or does not.
pub fn rig(confirms: bool) -> Rig {
    let identity = Identity::generate();
    let peers = Arc::new(Peers::default());
    let heard = Arc::new(Mutex::new(vec![]));
    let code = Code::mint();
    let host = FakeHost::default();
    let (revoke, revoked) = watch::channel(0u64);
    let sink = Arc::clone(&heard);
    let serving = host.clone();
    let gateway = Arc::new(Gateway {
        identity: identity.clone(),
        host_device: HOST.into(),
        pins: peers.clone(),
        desk: Arc::new(Desk {
            code: code.clone(),
            ends: Instant::now() + Duration::from_secs(300),
            taken: Mutex::new(false),
            confirms,
            peers: Arc::clone(&peers),
            offered: Mutex::new(vec![]),
        }),
        revoked,
        host_for: Arc::new(move |_| serving.clone()),
        on_event: Arc::new(move |event| sink.lock().unwrap().push(event)),
    });
    Rig {
        gateway,
        identity,
        peers,
        host,
        heard,
        code,
        revoke,
        serving: Mutex::new(vec![]),
    }
}

impl Rig {
    /// The pairing code's text, as the desktop's QR shows it.
    pub fn payload(&self, addresses: Vec<String>) -> Payload {
        Payload {
            version: 1,
            host_device: HOST.into(),
            host_key: hex(&self.identity.public()),
            addresses,
            code: self.code.to_hex(),
        }
    }

    /// This desktop as a phone that paired with it holds it.
    pub fn pinned(&self) -> Host {
        Host {
            device: HOST.into(),
            name: "Mac".into(),
            key: self.identity.public(),
            addresses: vec!["192.168.9.155:7777".into()],
            paired: 1,
        }
    }

    /// Pin `key` as a paired phone, as the desktop's `peers.json` would.
    pub fn pin(&self, key: [u8; 32]) {
        self.peers
            .0
            .lock()
            .unwrap()
            .insert(key, Scope::RemoteDevice);
    }

    /// The phone's end of a new connection the gateway serves.
    pub fn phone_side(&self) -> DuplexStream {
        let (a, b) = duplex(256 * 1024);
        let gateway = Arc::clone(&self.gateway);
        let serving = tokio::spawn(async move { gateway.serve_one(b).await });
        self.serving.lock().unwrap().push(serving);
        a
    }

    /// The desktop goes away: every link it serves ends.
    pub fn desktop_goes_away(&self) {
        for serving in self.serving.lock().unwrap().drain(..) {
            serving.abort();
        }
    }
}

/// A dial that reaches the rig's gateway, over an in-process stream, whatever the addresses.
pub struct DialTheRig(pub Arc<Rig>);

impl Dial for DialTheRig {
    fn dial<'a>(&'a self, _addresses: &'a [String]) -> Dialed<'a> {
        Box::pin(async move { Ok(Box::new(self.0.phone_side()) as Box<dyn Io>) })
    }
}

/// A dial that reaches nothing.
pub struct NoDial;

impl Dial for NoDial {
    fn dial<'a>(&'a self, addresses: &'a [String]) -> Dialed<'a> {
        Box::pin(async move {
            Err(Error::Unreachable(format!(
                "{}: connection refused",
                addresses.join(", ")
            )))
        })
    }
}

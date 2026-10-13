//! The gateway (iOS design §4.2, §4.3): a pinned key is served the session protocol as
//! `remote-device`; an unpinned key inside a pairing window may send one proof and gets the
//! window's verdict; an unpinned key outside one is dropped before a frame; a key revoked
//! mid-link loses the link at once.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::gateway::{Gateway, Heard, Offered, PairingDesk, Pins, Window};
use purlis_session_protocol::link;
use purlis_session_protocol::noise::{self, Identity};
use purlis_session_protocol::pairing::{self, Code, Proof, Verdict};
use purlis_session_protocol::session::{self, Answer, Client, Command, Host, Peer, Refusal};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
use tokio::sync::{oneshot, watch};

const HOST: &str = "01J9DEVCE00000000000000000";
const PHONE: &str = "01J9PHONE00000000000000000";

#[derive(Clone, Default)]
struct Board;

impl Host for Board {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        assert_eq!(peer.scope(), Scope::RemoteDevice);
        match command {
            Command::List => Ok(Answer::Chats(vec![])),
            _ => Ok(Answer::Done),
        }
    }
}

#[derive(Default)]
struct Peers(Mutex<HashMap<[u8; 32], Scope>>);

impl Pins for Peers {
    fn scope_of(&self, key: &[u8; 32]) -> Option<Scope> {
        self.0.lock().unwrap().get(key).copied()
    }
}

/// A desk whose window and verdicts the test sets.
struct Desk {
    window: Mutex<Option<Window>>,
    spent: Mutex<u32>,
    offers: Mutex<Vec<(Offered, oneshot::Sender<Verdict>)>>,
    withdrawn: Mutex<Vec<[u8; 32]>>,
}

impl Desk {
    fn with_window(code: Code, for_secs: u64) -> Arc<Desk> {
        Arc::new(Desk {
            window: Mutex::new(Some(Window {
                code,
                ends: Instant::now() + Duration::from_secs(for_secs),
            })),
            spent: Mutex::new(0),
            offers: Mutex::new(vec![]),
            withdrawn: Mutex::new(vec![]),
        })
    }

    fn closed() -> Arc<Desk> {
        Arc::new(Desk {
            window: Mutex::new(None),
            spent: Mutex::new(0),
            offers: Mutex::new(vec![]),
            withdrawn: Mutex::new(vec![]),
        })
    }

    fn spent(&self) -> u32 {
        *self.spent.lock().unwrap()
    }

    fn withdrawn(&self) -> Vec<[u8; 32]> {
        self.withdrawn.lock().unwrap().clone()
    }

    /// The pending offer, and the sender its verdict goes through.
    fn pending(&self) -> Option<(Offered, oneshot::Sender<Verdict>)> {
        self.offers.lock().unwrap().pop()
    }

    async fn wait_pending(&self) -> (Offered, oneshot::Sender<Verdict>) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(pending) = self.pending() {
                    break pending;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the window is asked")
    }
}

impl PairingDesk for Desk {
    /// The live window, spent in the same breath: a second taker finds none.
    fn take(&self) -> Option<Window> {
        let taken = self.window.lock().unwrap().take();
        if taken.is_some() {
            *self.spent.lock().unwrap() += 1;
        }
        taken
    }

    fn withdraw(&self, offer: &Offered) {
        self.withdrawn.lock().unwrap().push(offer.key);
    }

    fn offered(
        &self,
        offer: Offered,
    ) -> Pin<Box<dyn std::future::Future<Output = Verdict> + Send>> {
        let (tx, rx) = oneshot::channel();
        self.offers.lock().unwrap().push((offer, tx));
        Box::pin(async move {
            rx.await.unwrap_or(Verdict::Refused {
                why: "the window closed".into(),
            })
        })
    }
}

struct Rig {
    gateway: Arc<Gateway<Board>>,
    host: Identity,
    peers: Arc<Peers>,
    heard: Arc<Mutex<Vec<Heard>>>,
    revoke: watch::Sender<u64>,
    /// The key each served link's host was made for, in order.
    made_for: Arc<Mutex<Vec<[u8; 32]>>>,
}

fn rig(desk: Arc<Desk>) -> Rig {
    let host = Identity::generate();
    let peers = Arc::new(Peers::default());
    let heard = Arc::new(Mutex::new(vec![]));
    let made_for = Arc::new(Mutex::new(vec![]));
    let (revoke, revoked) = watch::channel(0u64);
    let sink = Arc::clone(&heard);
    let making = Arc::clone(&made_for);
    let gateway = Arc::new(Gateway {
        identity: host.clone(),
        host_device: HOST.into(),
        pins: peers.clone(),
        desk,
        revoked,
        host_for: Arc::new(move |device| {
            making.lock().unwrap().push(device.key());
            Board
        }),
        on_event: Arc::new(move |event| sink.lock().unwrap().push(event)),
    });
    Rig {
        gateway,
        host,
        peers,
        heard,
        revoke,
        made_for,
    }
}

type Serving = tokio::task::JoinHandle<Result<(), String>>;

/// A phone's side of a link: the handshake, then the plaintext end.
async fn phone_link(rig: &Rig, phone: &Identity) -> (tokio::io::DuplexStream, Serving) {
    let (a, b) = duplex(256 * 1024);
    let gateway = Arc::clone(&rig.gateway);
    let serving = tokio::spawn(async move { gateway.serve_one(b).await });
    let secured = noise::connect(a, phone, HOST, Some(rig.host.public()))
        .await
        .unwrap();
    (noise::pump(secured), serving)
}

fn proof_for(code: &Code, phone: &Identity, rig: &Rig) -> Proof {
    Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: pairing::prove(code, &phone.public(), &rig.host.public(), HOST),
    }
}

#[tokio::test]
async fn a_pinned_key_is_served_the_session_protocol_as_remote_device() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    rig.peers
        .0
        .lock()
        .unwrap()
        .insert(phone.public(), Scope::RemoteDevice);
    let (plain, _serving) = phone_link(&rig, &phone).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client, _views, _events) = Client::new(link);
    assert_eq!(client.list().await.unwrap(), vec![]);
    assert!(matches!(
        rig.heard.lock().unwrap().first(),
        Some(Heard::Linked {
            scope: Scope::RemoteDevice,
            ..
        })
    ));
}

#[tokio::test]
async fn an_unpinned_key_outside_a_window_is_dropped_before_a_frame() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    // Nothing of the session protocol is sent: the first read is the end.
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), plain.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    let served = serving.await.unwrap();
    assert!(served.is_err(), "{served:?}");
    assert!(matches!(
        rig.heard.lock().unwrap().first(),
        Some(Heard::Refused { key }) if *key == phone.public()
    ));
}

#[tokio::test]
async fn inside_a_window_one_right_proof_then_the_windows_pair_pins_and_answers_paired() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    pairing::send_proof(&mut plain, &proof_for(&code, &phone, &rig))
        .await
        .unwrap();
    // The window is asked, with the fingerprint it shows.
    let (offered, answer) = desk.wait_pending().await;
    assert_eq!(offered.name, "Noro's iPhone");
    assert_eq!(offered.device, PHONE);
    assert_eq!(offered.fingerprint, noise::fingerprint(&phone.public()));
    // The person presses Pair: the desk pins (the test does, as the desk would) and answers.
    rig.peers
        .0
        .lock()
        .unwrap()
        .insert(phone.public(), Scope::RemoteDevice);
    answer
        .send(Verdict::Paired {
            device: HOST.into(),
        })
        .unwrap();
    assert_eq!(
        pairing::read_verdict(&mut plain).await.unwrap(),
        Verdict::Paired {
            device: HOST.into()
        }
    );
    assert_eq!(desk.spent(), 1, "the code is spent");
    assert!(serving.await.unwrap().is_ok());
    let heard = rig.heard.lock().unwrap();
    assert!(heard.iter().any(|h| matches!(h, Heard::PairOffered { .. })));
    assert!(
        heard
            .iter()
            .any(|h| matches!(h, Heard::Paired { device, .. } if device == PHONE))
    );
}

#[tokio::test]
async fn a_wrong_proof_is_refused_and_spends_the_code() {
    let desk = Desk::with_window(Code::mint(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    pairing::send_proof(&mut plain, &proof_for(&Code::mint(), &phone, &rig))
        .await
        .unwrap();
    assert!(matches!(
        pairing::read_verdict(&mut plain).await.unwrap(),
        Verdict::Refused { .. }
    ));
    assert_eq!(desk.spent(), 1);
    assert!(desk.pending().is_none(), "the window was never asked");
    assert!(serving.await.unwrap().is_err());
}

#[tokio::test]
async fn anything_but_one_proof_frame_closes_the_link_and_spends_the_code() {
    let desk = Desk::with_window(Code::mint(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    plain
        .write_all(b"\x89CSP\r\n\x1a\n\x00\x11{\"call\":{\"id\":1}}")
        .await
        .unwrap();
    plain.flush().await.unwrap();
    assert!(serving.await.unwrap().is_err());
    assert_eq!(desk.spent(), 1);
    assert!(desk.pending().is_none(), "the window was asked");
}

#[tokio::test]
async fn a_second_frame_while_the_window_decides_closes_the_link() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    pairing::send_proof(&mut plain, &proof_for(&code, &phone, &rig))
        .await
        .unwrap();
    let (_offered, _answer) = desk.wait_pending().await;
    // The phone talks again before the person has answered.
    pairing::send_proof(&mut plain, &proof_for(&code, &phone, &rig))
        .await
        .unwrap();
    let served = tokio::time::timeout(Duration::from_secs(5), serving)
        .await
        .unwrap()
        .unwrap();
    assert!(served.is_err(), "{served:?}");
    assert!(rig.peers.0.lock().unwrap().is_empty(), "nothing was pinned");
    assert_eq!(desk.spent(), 1);
    assert_eq!(
        desk.withdrawn(),
        vec![phone.public()],
        "the abandoned offer was withdrawn from the desk"
    );
}

#[tokio::test]
async fn a_second_frame_pipelined_with_the_proof_closes_the_link_and_asks_no_window() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    // Both frames in ONE write, so they reach the host inside one Noise message.
    let mut both = Vec::new();
    pairing::send_proof(&mut both, &proof_for(&code, &phone, &rig))
        .await
        .unwrap();
    pairing::send_proof(&mut both, &proof_for(&code, &phone, &rig))
        .await
        .unwrap();
    plain.write_all(&both).await.unwrap();
    plain.flush().await.unwrap();
    let served = tokio::time::timeout(Duration::from_secs(5), serving)
        .await
        .unwrap()
        .unwrap();
    assert!(served.is_err(), "{served:?}");
    assert!(desk.pending().is_none(), "the window was asked");
    assert_eq!(desk.spent(), 1);
    assert!(rig.peers.0.lock().unwrap().is_empty(), "nothing was pinned");
}

#[tokio::test]
async fn two_links_in_one_window_get_one_code() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 300);
    let rig = rig(desk.clone());
    let first = Identity::generate();
    let second = Identity::generate();
    let (mut plain_first, serving_first) = phone_link(&rig, &first).await;
    let (mut plain_second, serving_second) = phone_link(&rig, &second).await;
    // The first link took the code; the second finds no window and is dropped before a frame.
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), plain_second.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    assert!(serving_second.await.unwrap().is_err());
    assert_eq!(desk.spent(), 1, "one code, taken once");
    // The first link goes on to pair.
    pairing::send_proof(&mut plain_first, &proof_for(&code, &first, &rig))
        .await
        .unwrap();
    let (offered, answer) = desk.wait_pending().await;
    assert_eq!(offered.key, first.public());
    answer
        .send(Verdict::Refused {
            why: "the test is over".into(),
        })
        .unwrap();
    assert!(serving_first.await.unwrap().is_err());
}

#[tokio::test]
async fn a_verdict_that_never_comes_closes_the_link_when_the_window_ends() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 1);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    pairing::send_proof(&mut plain, &proof_for(&code, &phone, &rig))
        .await
        .unwrap();
    let _pending = desk.wait_pending().await;
    // Nobody presses anything; the window's second passes.
    let verdict = tokio::time::timeout(Duration::from_secs(5), pairing::read_verdict(&mut plain))
        .await
        .unwrap();
    assert!(
        matches!(verdict, Ok(Verdict::Refused { .. }) | Err(_)),
        "{verdict:?}"
    );
    assert!(serving.await.unwrap().is_err());
    assert!(rig.peers.0.lock().unwrap().is_empty(), "nothing was pinned");
}

#[tokio::test]
async fn each_link_gets_a_host_made_for_its_key() {
    let rig = rig(Desk::closed());
    let one = Identity::generate();
    let two = Identity::generate();
    for phone in [&one, &two] {
        rig.peers
            .0
            .lock()
            .unwrap()
            .insert(phone.public(), Scope::RemoteDevice);
    }
    let (plain, _serving_one) = phone_link(&rig, &one).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client_one, _v1, _e1) = Client::new(link);
    client_one.list().await.unwrap();
    let (plain, _serving_two) = phone_link(&rig, &two).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client_two, _v2, _e2) = Client::new(link);
    client_two.list().await.unwrap();
    assert_eq!(
        *rig.made_for.lock().unwrap(),
        vec![one.public(), two.public()],
        "one host per link, made for the link's own key"
    );
}

#[tokio::test]
async fn a_key_revoked_mid_link_closes_the_link_at_once() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    rig.peers
        .0
        .lock()
        .unwrap()
        .insert(phone.public(), Scope::RemoteDevice);
    let (plain, serving) = phone_link(&rig, &phone).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client, _views, _events) = Client::new(link);
    client.list().await.unwrap();
    // Unpair: the record goes, the generation bumps.
    rig.peers.0.lock().unwrap().remove(&phone.public());
    rig.revoke.send(1).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(5), serving)
        .await
        .unwrap()
        .unwrap();
    assert!(ended.is_err(), "{ended:?}");
    let after = tokio::time::timeout(Duration::from_secs(5), client.list()).await;
    assert!(matches!(after, Ok(Err(_))), "the link is closed: {after:?}");
    assert!(
        rig.heard
            .lock()
            .unwrap()
            .iter()
            .any(|h| matches!(h, Heard::Unlinked { .. }))
    );
}

#[tokio::test]
async fn a_revocation_of_another_key_leaves_the_link_alone() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    rig.peers
        .0
        .lock()
        .unwrap()
        .insert(phone.public(), Scope::RemoteDevice);
    let (plain, _serving) = phone_link(&rig, &phone).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client, _views, _events) = Client::new(link);
    rig.revoke.send(1).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    client.list().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn the_handshake_itself_has_a_deadline() {
    let rig = rig(Desk::closed());
    let (_a, b) = duplex(1024);
    // Nobody speaks on `_a`.
    let gateway = Arc::clone(&rig.gateway);
    let serving = tokio::spawn(async move { gateway.serve_one(b).await });
    // The serve registers its deadline on its first poll; only then may the clock move.
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(11)).await;
    let served = tokio::time::timeout(Duration::from_secs(1), serving).await;
    assert!(matches!(served, Ok(Ok(Err(_)))), "{served:?}");
}

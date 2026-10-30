// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Host A keeps its link to host B for each window link it relays (#2215, ADR 0078 §2): the link is
//! kept through `hostlink::keep_linked`, so a link that is cut comes back on its own and one
//! renewed ahead of its limit is replaced by the next. Over each new link host A holds the window's
//! projects again, subscribes again from the window's cursor and attaches the window's views again,
//! each with a snapshot, under the numbers the window already knows them by, and hands no event
//! twice. A project is held where the window holds it now, and a hold host B refuses is not carried
//! (#2535), and the window is told so, in host B's words, as the carry-over ends; that is host A's
//! word, never host B's (#2645). A command in flight as its link is renewed is answered on it. The window is told once
//! that the link is down and once that it is back, and a renewal is neither (#2496). While no link
//! is up, every command is refused in words and none is sent on later; a host at the far end whose
//! key is not the one pinned is never linked; and over each new link the window takes back the
//! confinements of the chats it started on host B, and only its own (#2511). Host B's programs are
//! stand-ins and its link runs over an in-memory cable the test cuts, so none of this needs
//! a terminal.

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use purlis_host::standin::{Lines, StandIn, StandInConfines, StandIns};
use purlis_host::{Compose, Hosting, Keep, Relay, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{
    self, Connector, Handshake, Identity, LinkKey, Liveness, Pinned,
};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Answer, Beside, Client, Command, Confine, ConfinePlan, Event, Events, Exit, Launch,
    Layer, Listed, ProgramKind, Pushed, ReachChange, StartProgram, Told, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Reader, Viewer};
use purlis_terminal::session::Spec;
use tokio::io::{DuplexStream, duplex};
use tokio::sync::mpsc;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const HOST_A: &str = "01JA0DESKT0P00000000000000";
const HOST_B: &str = "01JB0RUNNER0000000000000000";
const PATIENCE: Duration = Duration::from_secs(10);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes every start as `/bin/echo`.
struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// What the cable to host B does with the next connection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// It reaches host B.
    HostB,
    /// It reaches nothing.
    Nothing,
    /// It reaches a host that offers another key than the one pinned for host B.
    AnotherHost,
}

/// Host A's connector to host B: each connection is a cable the test can cut, to host B or
/// wherever [`Reach`] says.
#[derive(Clone)]
struct Cable<B> {
    b: B,
    another: Hosting,
    b_key: LinkKey,
    a_key: LinkKey,
    reach: Arc<Mutex<Reach>>,
    lines: Lines,
    dials: Arc<Mutex<usize>>,
}

impl<B> Cable<B> {
    fn reach(&self, reach: Reach) {
        *self.reach.lock().unwrap() = reach;
    }

    /// Cut every connection made so far: both ends read their end.
    fn cut(&self) {
        self.lines.cut();
    }

    fn dials(&self) -> usize {
        *self.dials.lock().unwrap()
    }
}

impl<B: session::Host + Clone> Connector for Cable<B> {
    type Io = DuplexStream;

    fn connect(&self) -> BoxFuture<'static, std::io::Result<DuplexStream>> {
        *self.dials.lock().unwrap() += 1;
        let reach = *self.reach.lock().unwrap();
        let (b, another, key) = match reach {
            Reach::Nothing => {
                return Box::pin(async {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "nothing answers",
                    ))
                });
            }
            Reach::HostB => (Some(self.b.clone()), None, self.b_key.clone()),
            Reach::AnotherHost => (
                None,
                Some(self.another.clone()),
                LinkKey::generate().unwrap(),
            ),
        };
        let desktop = Pinned {
            device: HOST_A.into(),
            key: self.a_key.public(),
        };
        let (a_end, b_end) = self.lines.line();
        tokio::spawn(async move {
            let ours = Identity {
                device: HOST_B.into(),
                key,
            };
            if let Ok(answered) = hostlink::answer(
                b_end,
                Handshake::THIS_BUILD,
                &ours,
                &desktop,
                Liveness::default(),
            )
            .await
            {
                match (b, another) {
                    (Some(b), _) => drop(session::serve(answered.link, b, None).await),
                    (_, Some(another)) => drop(session::serve(answered.link, another, None).await),
                    _ => {}
                }
            }
        });
        Box::pin(async move { Ok(a_end) })
    }
}

struct Window<B = Hosting> {
    relay: Relay,
    client: Client,
    views: Acceptor,
    events: Events,
    runs: StandIns,
    another: StandIns,
    cable: Cable<B>,
    _project: tempfile::TempDir,
}

/// A window on host A, which keeps a link to host B through `liveness` for each window link.
async fn a_window(liveness: Liveness) -> Window {
    let runs = StandIns::default();
    a_window_on(Hosting::running_with(Echo, runs.clone()), runs, liveness).await
}

/// [`a_window`], to `b`, whose programs are `runs`.
async fn a_window_on<B: session::Host + Clone>(
    b: B,
    runs: StandIns,
    liveness: Liveness,
) -> Window<B> {
    let another = StandIns::default();
    let (a_key, b_key) = (LinkKey::generate().unwrap(), LinkKey::generate().unwrap());
    let cable = Cable {
        b,
        another: Hosting::running_with(Echo, another.clone()),
        b_key: b_key.clone(),
        a_key: a_key.clone(),
        reach: Arc::new(Mutex::new(Reach::HostB)),
        lines: Lines::default(),
        dials: Arc::default(),
    };
    let keep: Keep = {
        let cable = cable.clone();
        Arc::new(move || {
            hostlink::keep_linked(
                cable.clone(),
                Handshake::THIS_BUILD,
                Identity {
                    device: HOST_A.into(),
                    key: a_key.clone(),
                },
                Pinned {
                    device: HOST_B.into(),
                    key: b_key.public(),
                },
                liveness,
            )
        })
    };
    let relay = Relay::kept(keep);
    let (client, views, events) = a_window_link(&relay).await;
    Window {
        relay,
        client,
        views,
        events,
        runs,
        another,
        cable,
        _project: tempfile::tempdir().unwrap(),
    }
}

/// A window's link to host A, which relays it.
async fn a_window_link(relay: &Relay) -> (Client, Acceptor, Events) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), relay.clone(), None));
    Client::new(client.unwrap())
}

/// Every liveness as the link has it, but renewed only as the test says.
fn never_renewed() -> Liveness {
    Liveness {
        renew_after: None,
        ..Liveness::default()
    }
}

impl<B> Window<B> {
    /// Hold the project, subscribe, start a program and attach a terminal view of it.
    async fn watching(&mut self) -> (String, StandIn, Viewer, Reader) {
        self.client
            .hold_project(PROJECT, self._project.path().to_str().unwrap())
            .await
            .expect("host B holds the project");
        self.client.subscribe(session::Since::new()).await.unwrap();
        let program = self
            .client
            .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
            .await
            .expect("host B starts it");
        let running = self.runs.last();
        running.writes(b"ready> ");
        let attached = self
            .client
            .call(session::Command::AttachView {
                program: program.clone(),
                kind: ViewKind::Terminal,
                role: None,
                rate: None,
            })
            .await
            .expect("host B opens a view");
        let mut viewer = Viewer::default();
        let reader = self.next_view(&mut viewer).await;
        match attached {
            session::Answer::Attached(attached) => assert_eq!(attached.view, reader.view().0),
            other => panic!("{other:?}"),
        }
        (program, running, viewer, reader)
    }

    async fn next_view(&mut self, viewer: &mut Viewer) -> Reader {
        let stream = timeout(PATIENCE, self.views.accept())
            .await
            .expect("a view comes")
            .unwrap();
        viewer
            .accept(stream)
            .await
            .expect("a view the window takes")
    }

    /// What the window is pushed next.
    async fn pushed(&mut self) -> Pushed {
        match timeout(PATIENCE, self.events.recv())
            .await
            .expect("the window is told")
            .expect("the window's link is up")
        {
            Told::Pushed(pushed) => pushed,
            other => panic!("{other:?}"),
        }
    }

    /// Wait until a command is answered again rather than refused for want of a link.
    async fn linked_again(&self) {
        timeout(PATIENCE, async {
            while self.client.list().await.is_err() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("host A links again");
    }
}

/// The whole snapshot `reader` opens with, acknowledged as it is drawn.
async fn snapshot(reader: &mut Reader) -> Vec<u8> {
    let mut drawn = Vec::new();
    while !reader.snapshot_complete() || drawn.is_empty() {
        match timeout(PATIENCE, reader.next())
            .await
            .expect("the view draws")
            .expect("the view is open")
            .unwrap()
        {
            Chunk::Snapshot(bytes) => {
                drawn.extend_from_slice(&bytes);
                reader.ack(bytes.len()).await.unwrap();
            }
            Chunk::Live(bytes) => panic!("live bytes {bytes:?} before the snapshot was whole"),
        }
    }
    drawn
}

/// Live bytes until `reader` has drawn `want`.
async fn live_until(reader: &mut Reader, want: &[u8]) {
    let mut drawn = Vec::new();
    while drawn != want {
        match timeout(PATIENCE, reader.next())
            .await
            .expect("the view draws")
            .expect("the view is open")
            .unwrap()
        {
            Chunk::Live(bytes) => {
                drawn.extend_from_slice(&bytes);
                reader.ack(bytes.len()).await.unwrap();
            }
            Chunk::Snapshot(bytes) => panic!("a snapshot {bytes:?} in the live bytes"),
        }
    }
}

/// Read `reader` until its stream ends.
async fn ends(reader: &mut Reader) {
    timeout(PATIENCE, async {
        while let Some(Ok(chunk)) = reader.next().await {
            let drawn = match chunk {
                Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes.len(),
            };
            let _ = reader.ack(drawn).await;
        }
    })
    .await
    .expect("the view's stream ends with its link");
}

#[tokio::test]
async fn a_link_cut_under_a_window_comes_back_and_its_view_draws_again_from_a_snapshot() {
    let mut window = a_window(never_renewed()).await;
    let (program, running, mut viewer, mut first) = window.watching().await;
    assert_eq!(snapshot(&mut first).await, b"ready> ");

    window.cable.cut();
    ends(&mut first).await;
    running.writes(b"more ");

    let mut again = window.next_view(&mut viewer).await;
    assert_eq!(
        again.view(),
        first.view(),
        "the window knows it by the same number"
    );
    assert!(again.epoch() > first.epoch());
    assert_eq!(snapshot(&mut again).await, b"ready> more ");
    running.writes(b"live");
    live_until(&mut again, b"live").await;
    assert_eq!(window.cable.dials(), 2);

    // The window's subscription is carried over: host B's word of the program's end reaches it.
    window.client.write(&program, b"typed").await.unwrap();
    assert_eq!(running.seen().typed, b"typed");
    running.ends(Exit::Code(0));
    loop {
        if let Pushed::Ended { program: ended, .. } = window.pushed().await {
            assert_eq!(ended, program);
            break;
        }
    }
}

#[tokio::test]
async fn while_no_link_is_up_every_command_is_refused_and_none_is_sent_on_later() {
    let mut window = a_window(never_renewed()).await;
    let (program, running, _viewer, _first) = window.watching().await;

    window.cable.reach(Reach::Nothing);
    window.cable.cut();
    // A stop sent as the cut is noticed is refused as one its link did not answer; once host A
    // knows no link is up, it says so.
    let refusal = timeout(PATIENCE, async {
        loop {
            match window.client.stop(&program).await {
                Err(refused) => {
                    let refusal = refused.refusal().expect("host A's own refusal").clone();
                    if refusal.why.contains("not linked") {
                        break refusal;
                    }
                }
                Ok(()) => panic!("a stop was carried out with no link up"),
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("host A says no link is up");

    assert_eq!(refusal.code, session::NOT_STARTED);
    window.cable.reach(Reach::HostB);
    window.linked_again().await;
    assert!(
        !running.seen().stopped,
        "a refused stop was sent on once linked"
    );
}

#[tokio::test]
async fn the_window_is_told_once_that_the_link_is_down_and_then_that_it_is_back() {
    let mut window = a_window(never_renewed()).await;
    let (_program, _running, mut viewer, mut first) = window.watching().await;
    assert_eq!(snapshot(&mut first).await, b"ready> ");

    window.cable.reach(Reach::Nothing);
    window.cable.cut();
    match window.pushed().await {
        Pushed::LinkDown { why, .. } => assert!(!why.is_empty()),
        other => panic!("the window was told {other:?} before the link was down"),
    }
    // Host A tries again and again while nothing answers, and the window is told nothing more.
    timeout(PATIENCE, async {
        while window.cable.dials() < 4 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("host A keeps trying");

    window.cable.reach(Reach::HostB);
    assert_eq!(window.pushed().await, Pushed::LinkBack {});
    let mut again = window.next_view(&mut viewer).await;
    assert_eq!(again.view(), first.view());
    assert_eq!(snapshot(&mut again).await, b"ready> ");
}

#[tokio::test]
async fn a_host_at_the_other_end_with_a_key_that_is_not_pinned_is_never_linked() {
    let mut window = a_window(never_renewed()).await;
    let (program, _running, _viewer, _first) = window.watching().await;

    window.cable.reach(Reach::AnotherHost);
    window.cable.cut();
    timeout(PATIENCE, async {
        while window.cable.dials() < 4 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("host A keeps trying");

    // Refused as the runner's key having changed (1.33, #2679), so the window can offer to
    // remove it.
    let refused = window.client.write(&program, b"typed").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::KEY_CHANGED)
    );
    assert!(window.another.ran().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_link_renewed_ahead_of_its_limit_carries_the_subscription_and_views_over() {
    let liveness = Liveness {
        renew_after: Some(Duration::from_secs(60 * 60)),
        ..Liveness::default()
    };
    let mut window = a_window(liveness).await;
    let (program, running, mut viewer, mut first) = window.watching().await;
    assert_eq!(snapshot(&mut first).await, b"ready> ");

    // The window types through the renewal: every keystroke is answered, none refused.
    tokio::time::sleep(Duration::from_secs(60 * 60 - 10)).await;
    let mut typed = Vec::new();
    for _ in 0..40 {
        window
            .client
            .write(&program, b"k")
            .await
            .expect("a keystroke sent during the renewal is answered");
        typed.push(b'k');
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(
        running.seen().typed,
        typed,
        "every keystroke reached it once"
    );
    let mut again = window.next_view(&mut viewer).await;

    assert_eq!(window.cable.dials(), 2, "renewed once");
    assert_eq!(again.view(), first.view());
    // The stand-in echoes what is typed, and its screen is all it wrote: the new view's
    // snapshot is that screen as the view came back, and the rest follows it live.
    let written = running.seen().written.clone();
    let drawn = snapshot(&mut again).await;
    assert!(
        written.starts_with(&drawn),
        "{drawn:?} is not what it wrote"
    );
    assert!(drawn.starts_with(b"ready> "));
    live_until(&mut again, &written[drawn.len()..]).await;
    running.writes(b"live");
    live_until(&mut again, b"live").await;
    window.client.write(&program, b"typed").await.unwrap();
    typed.extend_from_slice(b"typed");
    assert_eq!(running.seen().typed, typed);
    running.ends(Exit::Code(0));
    loop {
        match window.pushed().await {
            Pushed::Ended { program: ended, .. } => {
                assert_eq!(ended, program);
                break;
            }
            Pushed::LinkDown { .. } | Pushed::LinkBack {} => {
                panic!("a renewal was told to the window as a link that went down")
            }
            _ => {}
        }
    }
}

const CONFINEMENT: &str = "01J9CNFNMNT0000000000000000";

fn planned() -> Confine {
    Confine {
        confinement: CONFINEMENT.into(),
        project: PROJECT.into(),
        plan: ConfinePlan {
            listed: vec![Listed {
                host: "api.anthropic.com".into(),
                layer: Layer::Open,
            }],
            never: Vec::new(),
            withdrawn: Vec::new(),
            anything: false,
            tunnel_ports: vec![443],
            plain_ports: vec![80],
            any_port: false,
            beside: Beside::Wrapped,
            given: Vec::new(),
        },
        asks: true,
    }
}

/// A chat started in confinement `id`, which the window made on host B.
fn running_in(id: &str) -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/echo".into(),
            args: Vec::new(),
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: None,
            confinement: Some(id.into()),
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

/// `client`, holding the project, runs a chat in confinement `id` it made on host B.
async fn confined_chat(client: &Client, project: &std::path::Path, id: &str) {
    client
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .unwrap();
    client
        .call(Command::Confine(Box::new(Confine {
            confinement: id.into(),
            ..planned()
        })))
        .await
        .expect("confined on host B");
    client
        .start_program(running_in(id))
        .await
        .expect("it runs in the confinement on host B");
}

fn reach(id: &str) -> Command {
    Command::ReachOf {
        confinement: id.into(),
    }
}

/// A chat the window started in a confinement it made on host B. Over the next link host A
/// names the window again, and host B names that link the chat's confinement: the window
/// changes the chat's reach again, and forgets an Allow the person gave.
#[tokio::test]
async fn after_a_re_link_the_window_acts_on_its_runner_chats_confinement_again() {
    let runs = StandIns::default();
    let confines = StandInConfines::default();
    let b = Hosting::confining(Echo, confines.clone(), runs.clone());
    let window = a_window_on(b, runs, never_renewed()).await;
    confined_chat(&window.client, window._project.path(), CONFINEMENT).await;
    let allowed = Layer::Chat;
    window
        .client
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: allowed,
            },
        })
        .await
        .expect("the person allows a host");

    window.cable.cut();
    window.linked_again().await;

    window
        .client
        .call(reach(CONFINEMENT))
        .await
        .expect("the window acts on it again");
    let forget = ReachChange::Forget {
        host: "pypi.example.com".into(),
        layer: allowed,
    };
    window
        .client
        .call(Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: forget.clone(),
        })
        .await
        .expect("the person forgets the Allow");
    assert_eq!(confines.seen()[0].changes.last(), Some(&forget));
}

/// Two windows relayed by host A, each with a chat in a confinement of its own on host B: after
/// a re-link, each takes back its own and is refused the other's as one host B does not hold.
#[tokio::test]
async fn after_a_re_link_a_window_takes_back_its_own_runner_chats_and_not_another_windows() {
    const ANOTHER: &str = "01J9CNFNMNT0000000000000009";
    let runs = StandIns::default();
    let b = Hosting::confining(Echo, StandInConfines::default(), runs.clone());
    let window = a_window_on(b, runs, never_renewed()).await;
    let (another, _views, _events) = a_window_link(&window.relay).await;
    confined_chat(&window.client, window._project.path(), CONFINEMENT).await;
    confined_chat(&another, window._project.path(), ANOTHER).await;

    window.cable.cut();
    window.linked_again().await;
    timeout(PATIENCE, async {
        while another.list().await.is_err() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the other window's link comes back too");

    for (client, own, other) in [
        (&window.client, CONFINEMENT, ANOTHER),
        (&another, ANOTHER, CONFINEMENT),
    ] {
        client.call(reach(own)).await.expect("its own chat's");
        let refused = client.call(reach(other)).await.unwrap_err();
        assert_eq!(
            refused.refusal().map(|refusal| refusal.code.as_str()),
            Some(session::NO_SUCH_CHAT),
            "{refused}"
        );
    }
}

/// `take_back` is host A's own word to host B: a window's is refused, whatever host A grants.
#[tokio::test]
async fn a_window_never_sends_a_take_back_through_the_relay() {
    let window = a_window(never_renewed()).await;

    let refused = window
        .client
        .call(Command::TakeBack {
            window: "01JA0WNDW0000000000000000".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::NOT_ALLOWED),
        "{refused}"
    );
}

/// The name a window keeps for a link across its restarts (#2623).
const NAME: &str = "0123456789abcdef0123456789abcdef";

/// `client` names its window `name` to host A, as its link's first command.
async fn named(client: &Client, name: &str) -> Result<Answer, session::CallError> {
    client
        .call(Command::NameWindow {
            name: purlis_session_protocol::browsing::Secret::new(name.into()),
        })
        .await
}

/// A host B, `inner`, that keeps the word of every command it is sent and the window each
/// `take_back` names.
#[derive(Clone)]
struct Hears<B> {
    inner: B,
    words: Arc<Mutex<Vec<&'static str>>>,
    windows: Arc<Mutex<Vec<String>>>,
    ended: Arc<Mutex<usize>>,
}

impl<B> Hears<B> {
    fn new(inner: B) -> Self {
        Hears {
            inner,
            words: Arc::default(),
            windows: Arc::default(),
            ended: Arc::default(),
        }
    }
}

impl<B: session::Host + Clone + Send + Sync + 'static> session::Host for Hears<B> {
    async fn call(
        &self,
        command: Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        self.words.lock().unwrap().push(command.word());
        if let Command::TakeBack { window } = &command {
            self.windows.lock().unwrap().push(window.clone());
        }
        self.inner.call(command, peer).await
    }

    async fn link_ended(&self, peer: &session::Peer) {
        self.inner.link_ended(peer).await;
        *self.ended.lock().unwrap() += 1;
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<mpsc::Receiver<Pushed>, session::Refusal> {
        self.inner.subscribe(since, peer).await
    }
}

/// Until host B has seen `links` of its links end.
async fn ended<B>(b: &Hears<B>, links: usize) {
    timeout(PATIENCE, async {
        while *b.ended.lock().unwrap() < links {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("host A lets go of the link of the window that went");
}

/// A window that names itself runs a chat in a confinement it made on host B, and restarts: its
/// link to host A ends, and the window that comes back names itself the same way on a new link.
/// It acts on its runner chat's confinement again. A window link that names itself otherwise,
/// or not at all, gets none of it.
#[tokio::test]
async fn a_window_that_comes_back_takes_back_its_runner_chats_and_another_window_does_not() {
    let runs = StandIns::default();
    let b = Hears::new(Hosting::confining(
        Echo,
        StandInConfines::default(),
        runs.clone(),
    ));
    let mut window = a_window_on(b.clone(), runs, never_renewed()).await;
    named(&window.client, NAME)
        .await
        .expect("host A takes the name");
    confined_chat(&window.client, window._project.path(), CONFINEMENT).await;

    // The window restarts.
    let (client, views, events) = a_window_link(&window.relay).await;
    drop(std::mem::replace(&mut window.client, client));
    window.views = views;
    window.events = events;
    ended(&b, 1).await;

    // Another window names itself otherwise, and another not at all: neither has the chat.
    let (another, _views, _events) = a_window_link(&window.relay).await;
    named(&another, "fedcba9876543210fedcba9876543210")
        .await
        .unwrap();
    another
        .hold_project(PROJECT, window._project.path().to_str().unwrap())
        .await
        .unwrap();
    let (unnamed, _views, _events) = a_window_link(&window.relay).await;
    unnamed
        .hold_project(PROJECT, window._project.path().to_str().unwrap())
        .await
        .unwrap();
    for other in [&another, &unnamed] {
        let refused = other.call(reach(CONFINEMENT)).await.unwrap_err();
        assert_eq!(
            refused.refusal().map(|refusal| refusal.code.as_str()),
            Some(session::NO_SUCH_CHAT),
            "{refused}"
        );
    }

    // The window that came back names itself as before, and takes its chat back.
    named(&window.client, NAME)
        .await
        .expect("host A takes the name again");
    window
        .client
        .hold_project(PROJECT, window._project.path().to_str().unwrap())
        .await
        .unwrap();
    window
        .client
        .call(reach(CONFINEMENT))
        .await
        .expect("the window acts on its runner chat again");
}

/// While one window link holds a name, no other link takes it; and a name comes first on a
/// link, before anything is sent on it.
#[tokio::test]
async fn a_name_a_live_window_link_holds_is_refused_and_so_is_a_name_sent_late() {
    let window = a_window(never_renewed()).await;
    named(&window.client, NAME).await.unwrap();
    window.client.list().await.unwrap();

    let (another, _views, _events) = a_window_link(&window.relay).await;
    let refused = named(&another, NAME).await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::NOT_ALLOWED),
        "{refused}"
    );

    let (late, _views, _events) = a_window_link(&window.relay).await;
    late.list().await.unwrap();
    let refused = named(&late, "fedcba9876543210fedcba9876543210")
        .await
        .unwrap_err();
    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::MALFORMED),
        "{refused}"
    );
}

/// Host B never hears the name: not `name_window`, and only a digest of the name in the
/// `take_back` host A says. And no `Debug` of the command shows it.
#[tokio::test]
async fn host_b_never_hears_the_name_a_window_keeps() {
    let runs = StandIns::default();
    let b = Hears::new(Hosting::running_with(Echo, runs.clone()));
    let window = a_window_on(b.clone(), runs, never_renewed()).await;
    named(&window.client, NAME).await.unwrap();
    window.client.list().await.unwrap();

    let words = b.words.lock().unwrap().clone();
    assert!(!words.contains(&"name_window"), "{words:?}");
    let windows = b.windows.lock().unwrap().clone();
    // One name, said in each pass of the carry-over, and never the window's own.
    assert!(!windows.is_empty(), "{windows:?}");
    assert!(
        windows
            .iter()
            .all(|window| *window == windows[0] && !window.contains(NAME)),
        "{windows:?}"
    );
    let shown = format!(
        "{:?}",
        Command::NameWindow {
            name: purlis_session_protocol::browsing::Secret::new(NAME.into()),
        }
    );
    assert!(!shown.contains(NAME), "{shown}");
}

/// A host B that pushes the same events again to every link that subscribes, one more each
/// time, whatever cursor it is given: what a renewed link's first events can overlap with the
/// last of the link before it.
#[derive(Clone, Default)]
struct Repeats(Arc<Mutex<u64>>);

fn event(seq: u64) -> Event {
    Event {
        v: 1,
        device_id: HOST_B.into(),
        seq,
        ulid: format!("01J9ZZEVENT{seq:015}"),
        chat: None,
        run: None,
        parent_run: None,
        kind: "hook.stop".into(),
        body: serde_json::Value::Null,
    }
}

impl session::Host for Repeats {
    async fn call(
        &self,
        _command: Command,
        _peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        Ok(Answer::Chats(Vec::new()))
    }

    async fn subscribe(
        &self,
        _since: session::Since,
        _peer: &session::Peer,
    ) -> Result<mpsc::Receiver<Pushed>, session::Refusal> {
        let most = {
            let mut subscribed = self.0.lock().unwrap();
            *subscribed += 1;
            *subscribed + 2
        };
        let (tell, told) = mpsc::channel(16);
        for seq in 1..=most {
            tell.send(Pushed::Event(event(seq))).await.unwrap();
        }
        tokio::spawn(async move { tell.closed().await });
        Ok(told)
    }
}

#[tokio::test]
async fn an_event_handed_to_the_window_over_one_link_is_not_handed_again_over_the_next() {
    let mut window = a_window_on(Repeats::default(), StandIns::default(), never_renewed()).await;
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();
    for seq in 1..=3 {
        assert_eq!(window.pushed().await, Pushed::Event(event(seq)));
    }

    window.cable.cut();

    // Host B pushes 1 to 4 again over the next link: only 4 is new to the window, which is
    // told the link went down and is back besides (#2496).
    assert!(matches!(window.pushed().await, Pushed::LinkDown { .. }));
    let mut told = vec![window.pushed().await, window.pushed().await];
    told.retain(|pushed| *pushed != Pushed::LinkBack {});
    assert_eq!(told, [Pushed::Event(event(4))]);
    assert!(
        timeout(Duration::from_millis(300), window.events.recv())
            .await
            .is_err(),
        "the window was handed an event twice"
    );
}

/// A command in flight on a link as it is renewed is answered on that link: the link it is on
/// ends only once the relay has moved to the next and nothing sent on it waits for an answer.
/// A host B that says, as a relaying host would, that its own link is down and back, and then
/// pushes one event.
#[derive(Clone, Default)]
struct SaysItsLink;

impl session::Host for SaysItsLink {
    async fn call(
        &self,
        _command: Command,
        _peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        Ok(Answer::Chats(Vec::new()))
    }

    async fn subscribe(
        &self,
        _since: session::Since,
        _peer: &session::Peer,
    ) -> Result<mpsc::Receiver<Pushed>, session::Refusal> {
        let (tell, told) = mpsc::channel(16);
        for pushed in [
            Pushed::LinkDown {
                why: "host B's own link".into(),
                key_changed: false,
                gone: false,
            },
            Pushed::LinkBack {},
            Pushed::Event(event(1)),
        ] {
            tell.send(pushed).await.unwrap();
        }
        tokio::spawn(async move { tell.closed().await });
        Ok(told)
    }
}

/// A host B that says a project is not held, as host A says it to its client.
#[derive(Clone)]
struct SaysNotHeld;

impl session::Host for SaysNotHeld {
    async fn call(
        &self,
        _command: Command,
        _peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        Ok(Answer::Done)
    }

    async fn subscribe(
        &self,
        _since: session::Since,
        _peer: &session::Peer,
    ) -> Result<mpsc::Receiver<Pushed>, session::Refusal> {
        let (tell, told) = mpsc::channel(16);
        for pushed in [
            Pushed::NotHeld {
                project: PROJECT.into(),
                why: "host B's own word".into(),
            },
            Pushed::Event(event(1)),
        ] {
            tell.send(pushed).await.unwrap();
        }
        tokio::spawn(async move { tell.closed().await });
        Ok(told)
    }
}

#[tokio::test]
async fn that_a_project_is_not_held_is_host_as_word_and_never_host_bs() {
    let mut window = a_window_on(SaysNotHeld, StandIns::default(), never_renewed()).await;
    let path = window._project.path().to_str().unwrap().to_owned();
    window.client.hold_project(PROJECT, &path).await.unwrap();
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();

    assert_eq!(window.pushed().await, Pushed::Event(event(1)));
    assert!(
        timeout(Duration::from_millis(300), window.events.recv())
            .await
            .is_err(),
        "host B's word that a project is not held was handed to the window"
    );
}

#[tokio::test]
async fn whether_the_link_is_up_is_host_as_word_and_never_host_bs() {
    let mut window = a_window_on(SaysItsLink, StandIns::default(), never_renewed()).await;
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();

    assert_eq!(window.pushed().await, Pushed::Event(event(1)));
    assert!(
        timeout(Duration::from_millis(300), window.events.recv())
            .await
            .is_err(),
        "host B's word on its own link was handed to the window"
    );
}

#[tokio::test]
async fn a_command_in_flight_as_its_link_is_renewed_is_answered_on_it() {
    let (runs, confines) = (StandIns::default(), StandInConfines::default());
    let b = Hosting::confining(Echo, confines.clone(), runs.clone());
    let liveness = Liveness {
        renew_after: Some(Duration::from_millis(300)),
        ..Liveness::default()
    };
    let window = Arc::new(a_window_on(b, runs, liveness).await);
    window
        .client
        .hold_project(PROJECT, window._project.path().to_str().unwrap())
        .await
        .unwrap();
    confines.hold_starts();
    // Whatever happens, the start held on its blocking thread is let go, so the test ends.
    let _let_go = LetGo(confines.clone());
    let confining = tokio::spawn({
        let window = Arc::clone(&window);
        async move {
            window
                .client
                .call(Command::Confine(Box::new(planned())))
                .await
        }
    });
    timeout(PATIENCE, async {
        while confines.waiting() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the confinement's start waits on host B");

    // The link it was sent on is renewed while it waits, and the window's commands go on.
    timeout(PATIENCE, async {
        while window.cable.dials() < 2 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the link is renewed");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(window.client.list().await.is_ok());
    confines.let_starts_go();

    let answered = timeout(PATIENCE, confining).await.unwrap().unwrap();
    assert!(
        matches!(answered, Ok(Answer::Confined(_))),
        "a command in flight at a renewal was not answered: {answered:?}"
    );
    assert!(window.client.list().await.is_ok());
}

/// Lets every held start go when dropped.
struct LetGo(StandInConfines);

impl Drop for LetGo {
    fn drop(&mut self) {
        self.0.let_starts_go();
    }
}

/// A host B that keeps what each link held, and where, and how it last subscribed, and holds
/// back every attach on a link after the first until the test lets them go: so what the window
/// does while a renewed link is carried over lands between the carry-over's passes. As a host
/// does, it holds a project at one place for each link; and on a link after the first it
/// refuses a hold at a path the test names.
#[derive(Clone, Default)]
struct Gated {
    links: Arc<Mutex<Vec<u64>>>,
    held: Arc<Mutex<HashMap<u64, BTreeMap<String, String>>>>,
    /// How many holds each link sent.
    tried: Arc<Mutex<HashMap<u64, usize>>>,
    refusing: Arc<Mutex<BTreeSet<String>>>,
    subscribed: Arc<Mutex<HashMap<u64, session::Since>>>,
    waiting: Arc<Mutex<usize>>,
    open: Arc<tokio::sync::watch::Sender<bool>>,
}

impl Gated {
    fn new() -> Gated {
        Gated {
            open: Arc::new(tokio::sync::watch::channel(false).0),
            ..Gated::default()
        }
    }

    /// Which link this is, in the order they came: 0 for the first.
    fn nth(&self, peer: &session::Peer) -> usize {
        let mut links = self.links.lock().unwrap();
        if !links.contains(&peer.link()) {
            links.push(peer.link());
        }
        links.iter().position(|link| *link == peer.link()).unwrap()
    }

    fn held_on(&self, nth: usize) -> BTreeMap<String, String> {
        let link = self.links.lock().unwrap()[nth];
        self.held
            .lock()
            .unwrap()
            .get(&link)
            .cloned()
            .unwrap_or_default()
    }

    fn holds_tried_on(&self, nth: usize) -> usize {
        let link = self.links.lock().unwrap()[nth];
        self.tried.lock().unwrap().get(&link).copied().unwrap_or(0)
    }

    fn subscribed_on(&self, nth: usize) -> Option<session::Since> {
        let link = self.links.lock().unwrap()[nth];
        self.subscribed.lock().unwrap().get(&link).cloned()
    }
}

impl session::Host for Gated {
    async fn call(
        &self,
        command: Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        let nth = self.nth(peer);
        match command {
            Command::HoldProject {
                project,
                project_path,
            } => {
                let refused = |why: &str| session::Refusal {
                    code: session::NOT_ALLOWED.into(),
                    why: why.into(),
                };
                *self.tried.lock().unwrap().entry(peer.link()).or_default() += 1;
                if nth > 0 && self.refusing.lock().unwrap().contains(&project_path) {
                    return Err(refused("that folder is not there"));
                }
                let mut held = self.held.lock().unwrap();
                let held = held.entry(peer.link()).or_default();
                if held.get(&project).is_some_and(|at| *at != project_path) {
                    return Err(refused("this link holds the project at another path"));
                }
                held.insert(project, project_path);
                Ok(Answer::Done)
            }
            Command::LetGoOfProject { project } => {
                if let Some(held) = self.held.lock().unwrap().get_mut(&peer.link()) {
                    held.remove(&project);
                }
                Ok(Answer::Done)
            }
            Command::AttachView { .. } => {
                if nth > 0 {
                    *self.waiting.lock().unwrap() += 1;
                    let _ = self.open.subscribe().wait_for(|open| *open).await;
                }
                Ok(Answer::Attached(session::Attached {
                    view: 1,
                    cols: 80,
                    rows: 24,
                    kind: Some(ViewKind::Terminal),
                }))
            }
            _ => Ok(Answer::Chats(Vec::new())),
        }
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<mpsc::Receiver<Pushed>, session::Refusal> {
        self.nth(peer);
        self.subscribed.lock().unwrap().insert(peer.link(), since);
        let (tell, told) = mpsc::channel(1);
        tokio::spawn(async move { tell.closed().await });
        Ok(told)
    }
}

#[tokio::test]
async fn what_the_window_changes_while_a_renewed_link_is_carried_over_is_carried_too() {
    let b = Gated::new();
    let liveness = Liveness {
        renew_after: Some(Duration::from_millis(300)),
        ..Liveness::default()
    };
    let window = a_window_on(b.clone(), StandIns::default(), liveness).await;
    let path = window._project.path().to_str().unwrap().to_owned();
    window.client.hold_project(PROJECT, &path).await.unwrap();
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();
    window
        .client
        .call(Command::AttachView {
            program: "01J9ZQ3V7K8M2N4P6R8T0V2X4Z".into(),
            kind: ViewKind::Terminal,
            role: None,
            rate: None,
        })
        .await
        .unwrap();

    // The renewed link is part-way through its carry-over: it holds the project, and its
    // attach waits.
    timeout(PATIENCE, async {
        while *b.waiting.lock().unwrap() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the renewed link is carried over");
    assert!(b.held_on(1).contains_key(PROJECT));

    // Meanwhile the window lets go of the project and subscribes from somewhere new.
    window
        .client
        .call(Command::LetGoOfProject {
            project: PROJECT.into(),
        })
        .await
        .unwrap();
    let since: session::Since = [(HOST_B.to_owned(), 7)].into();
    window.client.subscribe(since.clone()).await.unwrap();
    b.open.send_replace(true);

    timeout(PATIENCE, async {
        while b.held_on(1).contains_key(PROJECT) || b.subscribed_on(1) != Some(since.clone()) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the renewed link lets go of the project and subscribes from where the window did");
}

/// A window on host A, through `b`, that holds the project at `path`, subscribes and attaches a
/// view: so a renewed link's carry-over waits at its attach until the test opens `b`.
async fn watching_through(b: &Gated, path: &str) -> Window<Gated> {
    let liveness = Liveness {
        renew_after: Some(Duration::from_millis(300)),
        ..Liveness::default()
    };
    let window = a_window_on(b.clone(), StandIns::default(), liveness).await;
    window.client.hold_project(PROJECT, path).await.unwrap();
    window
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();
    window
        .client
        .call(Command::AttachView {
            program: "01J9ZQ3V7K8M2N4P6R8T0V2X4Z".into(),
            kind: ViewKind::Terminal,
            role: None,
            rate: None,
        })
        .await
        .unwrap();
    window
}

/// Until the renewed link is part-way through its carry-over: its attach waits.
async fn carrying_over(b: &Gated) {
    timeout(PATIENCE, async {
        while *b.waiting.lock().unwrap() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the renewed link is carried over");
}

#[tokio::test]
async fn a_project_held_again_at_another_path_during_a_carry_over_is_held_there_on_the_new_link() {
    let b = Gated::new();
    let (first, second) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (first, second) = (
        first.path().to_str().unwrap().to_owned(),
        second.path().to_str().unwrap().to_owned(),
    );
    let window = watching_through(&b, &first).await;
    carrying_over(&b).await;
    assert_eq!(b.held_on(1).get(PROJECT), Some(&first));

    // Meanwhile the window lets go of the project and holds it again at another path.
    window
        .client
        .call(Command::LetGoOfProject {
            project: PROJECT.into(),
        })
        .await
        .unwrap();
    window.client.hold_project(PROJECT, &second).await.unwrap();
    b.open.send_replace(true);

    timeout(PATIENCE, async {
        while b.held_on(1).get(PROJECT) != Some(&second) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the renewed link holds the project where the window holds it now");
}

#[tokio::test]
async fn a_hold_host_b_refuses_during_a_carry_over_is_not_carried_and_is_held_again_once_it_is_not()
{
    let b = Gated::new();
    let path = tempfile::tempdir().unwrap();
    let path = path.path().to_str().unwrap().to_owned();
    let window = watching_through(&b, &path).await;
    // Host B refuses the renewed link's hold of the project in the carry-over's first pass.
    b.refusing.lock().unwrap().insert(path.clone());
    carrying_over(&b).await;
    assert_eq!(b.held_on(1).get(PROJECT), None);

    // It takes the hold again before the carry-over's last pass.
    b.refusing.lock().unwrap().clear();
    b.open.send_replace(true);

    timeout(PATIENCE, async {
        while b.held_on(1).get(PROJECT) != Some(&path) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the renewed link holds the project the first pass could not");
    drop(window);
}

#[tokio::test]
async fn a_hold_host_b_refuses_through_a_carry_over_is_no_longer_carried_for_the_window() {
    let b = Gated::new();
    let path = tempfile::tempdir().unwrap();
    let path = path.path().to_str().unwrap().to_owned();
    let mut window = watching_through(&b, &path).await;
    b.refusing.lock().unwrap().insert(path.clone());
    carrying_over(&b).await;
    b.open.send_replace(true);
    // Both passes of the renewed link's carry-over were refused the hold.
    timeout(PATIENCE, async {
        while b.holds_tried_on(1) < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the carry-over's last pass tries the hold again");
    assert_eq!(b.held_on(1).get(PROJECT), None);
    // The window is told, in host B's words, as the carry-over ends (#2645).
    assert_eq!(
        window.pushed().await,
        Pushed::NotHeld {
            project: PROJECT.into(),
            why: "that folder is not there".into(),
        }
    );

    // Host B would take the hold now, and the next link is not sent it: the relay no longer
    // keeps the project for the window. A hold the window sends again is answered.
    b.refusing.lock().unwrap().clear();
    timeout(PATIENCE, async {
        while b.links.lock().unwrap().len() < 3 || b.subscribed_on(2).is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the next link is carried over");
    assert_eq!(b.held_on(2).get(PROJECT), None);
    assert_eq!(b.holds_tried_on(2), 0);
    window.client.hold_project(PROJECT, &path).await.unwrap();
}

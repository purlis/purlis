// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Host A keeps its link to host B for each window link it relays (#2215, ADR 0078 §2): the
//! link is kept through `hostlink::keep_linked`, so a link that is cut comes back on its own and
//! one renewed ahead of its limit is replaced by the next. Over each new link host A holds the
//! window's projects again, subscribes again from the window's cursor and attaches the window's
//! views again, each with a snapshot, under the numbers the window already knows them by. While
//! no link is up, every command is refused in words and none is sent on later; a host at the
//! far end whose key is not the one pinned is never linked. Host B's programs are stand-ins and
//! its link runs over an in-memory cable the test cuts, so none of this needs a terminal.

#![cfg(unix)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use purlis_host::standin::{StandIn, StandIns};
use purlis_host::{Compose, Hosting, Keep, Relay, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{
    self, Connector, Handshake, Identity, LinkKey, Liveness, Pinned,
};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Client, Events, Exit, ProgramKind, Pushed, StartProgram, Told, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Reader, Viewer};
use purlis_terminal::session::Spec;
use tokio::io::{DuplexStream, duplex};
use tokio::task::AbortHandle;
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
struct Cable {
    b: Hosting,
    another: Hosting,
    b_key: LinkKey,
    a_key: LinkKey,
    reach: Arc<Mutex<Reach>>,
    lines: Arc<Mutex<Vec<AbortHandle>>>,
    dials: Arc<Mutex<usize>>,
}

impl Cable {
    fn reach(&self, reach: Reach) {
        *self.reach.lock().unwrap() = reach;
    }

    /// Cut every connection made so far: both ends read their end.
    fn cut(&self) {
        for line in self.lines.lock().unwrap().drain(..) {
            line.abort();
        }
    }

    fn dials(&self) -> usize {
        *self.dials.lock().unwrap()
    }
}

impl Connector for Cable {
    type Io = DuplexStream;

    fn connect(&self) -> BoxFuture<'static, std::io::Result<DuplexStream>> {
        *self.dials.lock().unwrap() += 1;
        let reach = *self.reach.lock().unwrap();
        let (host, key) = match reach {
            Reach::Nothing => {
                return Box::pin(async {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "nothing answers",
                    ))
                });
            }
            Reach::HostB => (self.b.clone(), self.b_key.clone()),
            Reach::AnotherHost => (self.another.clone(), LinkKey::generate().unwrap()),
        };
        let desktop = Pinned {
            device: HOST_A.into(),
            key: self.a_key.public(),
        };
        let (a_end, a_line) = duplex(256 * 1024);
        let (b_line, b_end) = duplex(256 * 1024);
        let line = tokio::spawn(async move {
            let (mut a_line, mut b_line) = (a_line, b_line);
            let _ = tokio::io::copy_bidirectional(&mut a_line, &mut b_line).await;
        });
        self.lines.lock().unwrap().push(line.abort_handle());
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
                let _ = session::serve(answered.link, host, None).await;
            }
        });
        Box::pin(async move { Ok(a_end) })
    }
}

struct Window {
    client: Client,
    views: Acceptor,
    events: Events,
    runs: StandIns,
    another: StandIns,
    cable: Cable,
    _project: tempfile::TempDir,
}

/// A window on host A, which keeps a link to host B through `liveness` for each window link.
async fn a_window(liveness: Liveness) -> Window {
    let (runs, another) = (StandIns::default(), StandIns::default());
    let (a_key, b_key) = (LinkKey::generate().unwrap(), LinkKey::generate().unwrap());
    let cable = Cable {
        b: Hosting::running_with(Echo, runs.clone()),
        another: Hosting::running_with(Echo, another.clone()),
        b_key: b_key.clone(),
        a_key: a_key.clone(),
        reach: Arc::new(Mutex::new(Reach::HostB)),
        lines: Arc::default(),
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
    tokio::spawn(session::serve(served.unwrap(), relay, None));
    let (client, views, events) = Client::new(client.unwrap());
    Window {
        client,
        views,
        events,
        runs,
        another,
        cable,
        _project: tempfile::tempdir().unwrap(),
    }
}

/// Every liveness as the link has it, but renewed only as the test says.
fn never_renewed() -> Liveness {
    Liveness {
        renew_after: None,
        ..Liveness::default()
    }
}

impl Window {
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

    let refused = window.client.write(&program, b"typed").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::NOT_STARTED)
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

    tokio::time::sleep(Duration::from_secs(60 * 60)).await;
    let mut again = window.next_view(&mut viewer).await;

    assert_eq!(window.cable.dials(), 2, "renewed once");
    assert_eq!(again.view(), first.view());
    assert_eq!(snapshot(&mut again).await, b"ready> ");
    running.writes(b"live");
    live_until(&mut again, b"live").await;
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

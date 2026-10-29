// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Host A's kill switch reaches every runner its machine keeps (#2051, ADR 0078 §7, ADR 0071 as
//! amended): a stop from any of its links is sent down a link of its own to each runner, whose
//! host throws its own switch; a runner that is not reached is named in what host A says of the
//! switch; host A's window re-arms the runners the stop reached; and a link host A relays while
//! it is stopped carries a stop first. Host B's programs are stand-ins and its link runs over an
//! in-memory cable, so none of this needs a terminal or a socket.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use purlis_host::standin::StandIns;
use purlis_host::{Compose, EveryRunner, Hosting, Keep, Routed, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{
    self, Connector, Handshake, Identity, LinkKey, Liveness, Pinned,
};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::{DuplexStream, duplex};
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const HOST_A: &str = "01JA0DESKT0P00000000000000";
const HOST_B: &str = "01JB0RUNNER0000000000000000";
const PATIENCE: Duration = Duration::from_secs(20);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes every start as `/bin/echo`.
struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A link to `host` admitted as `scope`.
async fn as_scope(scope: Scope, host: impl session::Host) -> Link {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    client.unwrap()
}

/// A window on `host`, its link routed as the host's socket routes every link.
async fn a_window(host: &Hosting) -> Client {
    Client::new(as_scope(Scope::LocalUi, Routed::new(host.clone())).await).0
}

/// Host A's connector to host B: each connection is a cable to host B, which answers it as the
/// runner answers its paired desktop.
#[derive(Clone)]
struct Cable<B = Hosting> {
    b: B,
    a_key: LinkKey,
    b_key: LinkKey,
}

impl<B: session::Host + Clone> Connector for Cable<B> {
    type Io = DuplexStream;

    fn connect(&self) -> BoxFuture<'static, std::io::Result<DuplexStream>> {
        let (a_end, b_end) = duplex(256 * 1024);
        let b = self.b.clone();
        let ours = Identity {
            device: HOST_B.into(),
            key: self.b_key.clone(),
        };
        let desktop = Pinned {
            device: HOST_A.into(),
            key: self.a_key.public(),
        };
        tokio::spawn(async move {
            if let Ok(answered) = hostlink::answer(
                b_end,
                Handshake::THIS_BUILD,
                &ours,
                &desktop,
                Liveness::default(),
            )
            .await
            {
                let _ = session::serve(answered.link, b, None).await;
            }
        });
        Box::pin(async move { Ok(a_end) })
    }
}

impl<B: session::Host + Clone> Cable<B> {
    fn to(b: &B) -> Cable<B> {
        Cable {
            b: b.clone(),
            a_key: LinkKey::generate().unwrap(),
            b_key: LinkKey::generate().unwrap(),
        }
    }

    /// How host A keeps a link to host B.
    fn keep(&self) -> Keep {
        let cable = self.clone();
        Arc::new(move || {
            hostlink::keep_linked(
                cable.clone(),
                Handshake::THIS_BUILD,
                Identity {
                    device: HOST_A.into(),
                    key: cable.a_key.clone(),
                },
                Pinned {
                    device: HOST_B.into(),
                    key: cable.b_key.public(),
                },
                Liveness::default(),
            )
        })
    }
}

/// Host A keeps host B as `pool/b`, and one more runner it cannot make a link to.
fn every_runner<B: session::Host + Clone>(cable: &Cable<B>) -> EveryRunner {
    let keep = cable.keep();
    Arc::new(move || {
        vec![
            ("pool/b".to_owned(), Ok(Arc::clone(&keep))),
            (
                "pool/gone".to_owned(),
                Err("its container is not running".to_owned()),
            ),
        ]
    })
}

#[tokio::test]
async fn a_stop_reaches_every_runner_and_names_the_one_it_did_not() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(every_runner(&Cable::to(&b)));
    let window = a_window(&a).await;
    let on_b = a_window(&b).await;

    let switch = timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();

    assert!(switch.stopped);
    let said = switch.why.expect("the runner not reached is named");
    assert!(said.contains("pool/gone"), "{said}");
    assert!(said.contains("its container is not running"), "{said}");
    assert!(!said.contains("pool/b"), "{said}");
    assert!(
        on_b.kill_switch().await.unwrap().stopped,
        "host B threw its own switch"
    );
    // Asked again, host A still names it: it is owed a stop.
    let asked = window.kill_switch().await.unwrap();
    assert!(asked.why.is_some_and(|why| why.contains("pool/gone")));
}

#[tokio::test]
async fn the_windows_re_arm_re_arms_the_runners_the_stop_reached() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(every_runner(&Cable::to(&b)));
    let window = a_window(&a).await;
    let on_b = a_window(&b).await;
    timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();
    assert!(on_b.kill_switch().await.unwrap().stopped);

    let armed = timeout(PATIENCE, window.rearm()).await.unwrap().unwrap();

    assert!(!armed.stopped);
    assert!(
        !on_b.kill_switch().await.unwrap().stopped,
        "host B is re-armed with host A"
    );
    assert!(
        armed.why.is_none(),
        "the runner it could not stop was not stopped by it, so it is not re-armed: {:?}",
        armed.why
    );
}

#[tokio::test]
async fn a_runner_stopped_from_its_own_command_line_is_not_re_armed_by_the_desktop() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(every_runner(&Cable::to(&b)));
    let window = a_window(&a).await;
    let on_b = a_window(&b).await;
    // Stopped on host B itself first: host A's stop reaches it, and it says it was already.
    on_b.stop_all().await.unwrap();
    let stopped = timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();
    assert!(stopped.runners.is_empty(), "{:?}", stopped.runners);

    timeout(PATIENCE, window.rearm()).await.unwrap().unwrap();

    assert!(on_b.kill_switch().await.unwrap().stopped);
}

#[tokio::test]
async fn the_switch_names_the_runners_its_re_arm_re_arms() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(every_runner(&Cable::to(&b)));
    let window = a_window(&a).await;

    timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();

    assert_eq!(window.kill_switch().await.unwrap().runners, ["pool/b"]);
}

/// A runner's host that will not hold its chats stopped: what a stop to it answers.
#[derive(Clone)]
struct WillNotStop;

impl session::Host for WillNotStop {
    async fn call(
        &self,
        command: session::Command,
        _peer: &session::Peer,
    ) -> Result<session::Answer, session::Refusal> {
        Ok(match command {
            session::Command::StopAll | session::Command::KillSwitch => {
                session::Answer::KillSwitch(session::KillSwitch::default())
            }
            session::Command::List => session::Answer::Chats(Vec::new()),
            _ => session::Answer::Done,
        })
    }
}

#[tokio::test]
async fn a_link_whose_runner_does_not_take_the_stop_carries_nothing_and_it_is_named() {
    let keep = Cable::to(&WillNotStop).keep();
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(Arc::new(Vec::new))
        .relaying_with(Arc::new(move |pool: &str| {
            Ok((Arc::clone(&keep), format!("{pool}/b")))
        }));
    let window = a_window(&a).await;
    timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();

    let relayed = a_window(&a).await;
    relayed.relay_to("pool").await.expect("relayed");
    let listed = timeout(PATIENCE, relayed.list()).await.unwrap();

    assert!(
        listed.is_err(),
        "the window was carried onto a runner that runs"
    );
    let said = window.kill_switch().await.unwrap().why.unwrap_or_default();
    assert!(said.contains("pool/b"), "{said}");
}

#[tokio::test]
async fn a_runner_not_reached_is_tried_again_while_the_desktop_stays_stopped() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let keep = Cable::to(&b).keep();
    let reachable = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let every: EveryRunner = {
        let reachable = Arc::clone(&reachable);
        Arc::new(move || {
            let keep = if reachable.load(std::sync::atomic::Ordering::SeqCst) {
                Ok(Arc::clone(&keep))
            } else {
                Err("its container is not running".to_owned())
            };
            vec![("pool/b".to_owned(), keep)]
        })
    };
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(every)
        .trying_runners_again_every(Duration::from_millis(100));
    let window = a_window(&a).await;
    let on_b = a_window(&b).await;
    timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();
    assert!(!on_b.kill_switch().await.unwrap().stopped);

    // The host's first look sends its stop once more, and still does not reach it.
    let looking = tokio::spawn(a.clone().heed_stops_written_elsewhere());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!on_b.kill_switch().await.unwrap().stopped);
    reachable.store(true, std::sync::atomic::Ordering::SeqCst);

    timeout(PATIENCE, async {
        while !on_b.kill_switch().await.unwrap().stopped {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("host B was stopped once it could be reached");
    looking.abort();
    assert!(window.kill_switch().await.unwrap().why.is_none());
}

#[tokio::test]
async fn a_host_that_starts_stopped_sends_its_stop_to_every_runner() {
    let folder = tempfile::tempdir().unwrap();
    let b = Hosting::running_with(Echo, StandIns::default());
    // Stopped, and let go of, as a host that ended while stopped.
    let before = Hosting::running_with_its_switch_in(folder.path(), Echo, StandIns::default());
    a_window(&before).await.stop_all().await.unwrap();
    drop(before);
    let on_b = a_window(&b).await;
    assert!(!on_b.kill_switch().await.unwrap().stopped);

    let a = Hosting::running_with_its_switch_in(folder.path(), Echo, StandIns::default())
        .stopping_runners_with(every_runner(&Cable::to(&b)));
    let looking = tokio::spawn(a.clone().heed_stops_written_elsewhere());

    timeout(PATIENCE, async {
        while !on_b.kill_switch().await.unwrap().stopped {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("host B was stopped as host A started stopped");
    looking.abort();
}

#[tokio::test]
async fn a_link_relayed_while_the_desktop_is_stopped_carries_a_stop_first() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let cable = Cable::to(&b);
    let keep = cable.keep();
    // Host A's stop reaches no runner: only the relayed link can stop host B.
    let a = Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(Arc::new(Vec::new))
        .relaying_with(Arc::new(move |pool: &str| {
            Ok((Arc::clone(&keep), format!("{pool}/b")))
        }));
    let window = a_window(&a).await;
    let on_b = a_window(&b).await;
    timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();
    assert!(!on_b.kill_switch().await.unwrap().stopped);

    let relayed = a_window(&a).await;
    relayed.relay_to("pool").await.expect("relayed");
    timeout(PATIENCE, relayed.list()).await.unwrap().unwrap();

    assert!(
        on_b.kill_switch().await.unwrap().stopped,
        "the new link stopped host B before anything else"
    );
}

#[tokio::test]
async fn a_link_relayed_while_the_desktop_runs_stops_nothing() {
    let b = Hosting::running_with(Echo, StandIns::default());
    let keep = Cable::to(&b).keep();
    let a = Hosting::running_with(Echo, StandIns::default()).relaying_with(Arc::new(
        move |pool: &str| Ok((Arc::clone(&keep), format!("{pool}/b"))),
    ));
    let on_b = a_window(&b).await;

    let relayed = a_window(&a).await;
    relayed.relay_to("pool").await.expect("relayed");
    timeout(PATIENCE, relayed.list()).await.unwrap().unwrap();

    assert!(!on_b.kill_switch().await.unwrap().stopped);
}

/// A runner's host that keeps the words of every command it is sent, and takes each.
#[derive(Clone, Default)]
struct Hears(Arc<std::sync::Mutex<Vec<String>>>, Duration);

impl Hears {
    fn heard(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }

    /// One that takes `taking` to answer a stop.
    fn slow(taking: Duration) -> Hears {
        Hears(Arc::default(), taking)
    }
}

impl session::Host for Hears {
    async fn call(
        &self,
        command: session::Command,
        _peer: &session::Peer,
    ) -> Result<session::Answer, session::Refusal> {
        self.0.lock().unwrap().push(command.word().to_owned());
        Ok(match command {
            session::Command::StopAll => {
                tokio::time::sleep(self.1).await;
                session::Answer::KillSwitch(session::KillSwitch {
                    stopped: true,
                    ..Default::default()
                })
            }
            session::Command::Rearm | session::Command::KillSwitch => {
                session::Answer::KillSwitch(session::KillSwitch::default())
            }
            session::Command::List => session::Answer::Chats(Vec::new()),
            _ => session::Answer::Done,
        })
    }
}

/// Host A relays pool `pool` to `b`, and keeps one runner, `a`, for its stop to reach.
fn relaying_to<B: session::Host + Clone>(b: &B, a: &Hears) -> Hosting {
    let to_b = Cable::to(b).keep();
    let to_a = Cable::to(a).keep();
    Hosting::running_with(Echo, StandIns::default())
        .stopping_runners_with(Arc::new(move || {
            vec![("pool/a".to_owned(), Ok(Arc::clone(&to_a)))]
        }))
        .relaying_with(Arc::new(move |pool: &str| {
            Ok((Arc::clone(&to_b), format!("{pool}/b")))
        }))
}

// Seen red: with the relay's own refusal taken out, host B hears the start.
#[tokio::test]
async fn a_start_relayed_while_the_desktop_is_stopped_is_refused_before_the_runner() {
    let b = Hears::default();
    let a = relaying_to(&b, &Hears::default());
    let window = a_window(&a).await;
    timeout(PATIENCE, window.stop_all()).await.unwrap().unwrap();
    let relayed = a_window(&a).await;
    relayed.relay_to("pool").await.expect("relayed");

    let started = timeout(
        PATIENCE,
        relayed.start_program(StartProgram::of(PROJECT, ProgramKind::Shell)),
    )
    .await
    .unwrap();

    let refused = started.expect_err("nothing starts while stopped");
    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::STOPPED)
    );
    assert!(
        !b.heard().contains(&"start_program".to_owned()),
        "{:?}",
        b.heard()
    );
}

// Seen red: with the look at the switch after the hold taken out, host B is sent a stop.
#[tokio::test]
async fn a_link_that_waited_its_turn_behind_a_re_arm_carries_no_stop() {
    let b = Hears::default();
    // Host A's stop takes a second to reach runner `a`, and holds every other send meanwhile.
    let a = relaying_to(&b, &Hears::slow(Duration::from_secs(1)));
    let window = a_window(&a).await;
    let stopping = tokio::spawn({
        let a = a.clone();
        async move { a_window(&a).await.stop_all().await }
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(window.kill_switch().await.unwrap().stopped);
    // A link to host B comes up while host A is stopped, and waits its turn to carry the stop.
    let relayed = a_window(&a).await;
    relayed.relay_to("pool").await.expect("relayed");
    let listing = tokio::spawn(async move { relayed.list().await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // The window re-arms before that turn comes.
    timeout(PATIENCE, window.rearm()).await.unwrap().unwrap();
    timeout(PATIENCE, stopping).await.unwrap().unwrap().unwrap();
    timeout(PATIENCE, listing).await.unwrap().unwrap().unwrap();

    assert!(
        !b.heard().contains(&"stop_all".to_owned()),
        "{:?}",
        b.heard()
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use futures::future::BoxFuture;
use purlis_session_protocol::auth::Credentials;
use purlis_session_protocol::hostlink::{
    self, Connector, Handshake, Identity, LinkKey, Liveness, Pinned, Zeroizing, keep_linked,
};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session::{CallError, Client};
use purlis_terminal::session::Spec;
use tokio::io::{DuplexStream, duplex};

use super::*;
use crate::standin::StandIns;
use crate::{Compose, ToCompose};

const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

const DESKTOP: &str = "01JA0DESKT0P00000000000000";
const RUNNER: &str = "01JB0RUNNER0000000000000000";

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A key's private half, kept so an identity can be made again for each link.
struct Kept {
    device: &'static str,
    private: Zeroizing<[u8; 32]>,
}

impl Kept {
    fn new(device: &'static str) -> Kept {
        Kept {
            device,
            private: LinkKey::generate().unwrap().private_half().unwrap(),
        }
    }

    fn identity(&self) -> Identity {
        Identity {
            device: self.device.into(),
            key: LinkKey::from_private(self.private.clone()).unwrap(),
        }
    }

    fn pinned(&self) -> Pinned {
        let identity = self.identity();
        Pinned {
            device: identity.device,
            key: identity.key.public(),
        }
    }
}

/// The runner's connector, in memory: each connection is answered by the runner's host `b`,
/// as the runner `runner`, admitting the desktop `desktop`.
#[derive(Clone)]
struct ToTheRunner {
    b: Hosting,
    runner: Arc<Kept>,
    desktop: Pinned,
}

impl Connector for ToTheRunner {
    type Io = DuplexStream;

    fn connect(&self) -> BoxFuture<'static, std::io::Result<DuplexStream>> {
        let this = self.clone();
        Box::pin(async move {
            let (a, b) = duplex(256 * 1024);
            tokio::spawn(async move {
                if let Ok(answered) = hostlink::answer(
                    b,
                    Handshake::THIS_BUILD,
                    &this.runner.identity(),
                    &this.desktop,
                    Liveness::default(),
                )
                .await
                {
                    let _ = session::serve(answered.link, this.b.clone(), None).await;
                }
            });
            Ok(a)
        })
    }
}

/// The desktop's host, which relays to the runner kept as `builds` alone, pinning `pinned` for
/// it; and the runner's host.
fn a_desktop(pinned: impl Fn(&Kept) -> Pinned) -> (Hosting, Hosting) {
    a_desktop_waiting(pinned, Arc::new(|| {}))
}

/// [`a_desktop`], each runner looked up only once `waits` returns: as a lookup that takes a while.
fn a_desktop_waiting(
    pinned: impl Fn(&Kept) -> Pinned,
    waits: Arc<dyn Fn() + Send + Sync>,
) -> (Hosting, Hosting) {
    let b = Hosting::running_with(Echo, StandIns::default());
    let (desktop, runner) = (Arc::new(Kept::new(DESKTOP)), Arc::new(Kept::new(RUNNER)));
    let pinned = pinned(&runner);
    let connector = ToTheRunner {
        b: b.clone(),
        runner,
        desktop: desktop.pinned(),
    };
    let relays: RelaysTo = Arc::new(move |name: &str| {
        waits();
        // `builds/old` is a runner of the pool `builds` that drains (#2865).
        if name != "builds" && name != "builds/old" {
            return Err(format!("no runner named {name} is kept on this machine"));
        }
        let (connector, desktop, pinned) =
            (connector.clone(), Arc::clone(&desktop), pinned.clone());
        Ok((
            Arc::new(move || {
                keep_linked(
                    connector.clone(),
                    Handshake::THIS_BUILD,
                    desktop.identity(),
                    pinned.clone(),
                    Liveness::default(),
                )
            }) as Keep,
            if name.contains('/') {
                name.to_owned()
            } else {
                format!("{name}/{name}")
            },
        ))
    });
    let a = Hosting::running_with(Echo, StandIns::default()).relaying_with(relays);
    (a, b)
}

/// A link to `host`, admitted as `scope` and routed as the desktop's host routes it.
async fn routed(scope: Scope, host: Hosting) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), Routed::new(host), None));
    Client::new(client.unwrap()).0
}

/// A `local-ui` link straight to `host`.
async fn straight(host: Hosting) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served): (Result<Link, _>, _) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    Client::new(client.unwrap()).0
}

fn code(refused: &CallError) -> Option<&str> {
    refused.refusal().map(|refusal| refusal.code.as_str())
}

async fn stopped(host: &Hosting) -> bool {
    straight(host.clone())
        .await
        .kill_switch()
        .await
        .unwrap()
        .stopped
}

#[tokio::test]
async fn a_window_whose_first_command_is_relay_to_reaches_the_runner_its_host_keeps() {
    let (a, b) = a_desktop(Kept::pinned);
    let window = routed(Scope::LocalUi, a.clone()).await;
    tokio::time::timeout(PATIENCE, window.relay_to("builds"))
        .await
        .unwrap()
        .unwrap();
    let switch = tokio::time::timeout(PATIENCE, window.stop_all())
        .await
        .unwrap()
        .unwrap();
    assert!(switch.stopped);
    // The runner's host was stopped, and the desktop's was not.
    assert!(stopped(&b).await);
    assert!(!stopped(&a).await);
}

#[tokio::test]
async fn a_link_that_sends_anything_first_is_the_desktops_own_and_never_relayed() {
    let (a, b) = a_desktop(Kept::pinned);
    let window = routed(Scope::LocalUi, a.clone()).await;
    window.list().await.unwrap();
    let refused = window.relay_to("builds").await.unwrap_err();
    assert_eq!(code(&refused), Some(session::NOT_ALLOWED), "{refused}");
    window.stop_all().await.unwrap();
    assert!(stopped(&a).await);
    assert!(!stopped(&b).await);
}

#[tokio::test]
async fn relay_to_is_refused_to_every_scope_but_the_windows() {
    let (a, b) = a_desktop(Kept::pinned);
    // `remote-link` is admitted only by a link between hosts' handshake, and is refused it by
    // the grants table (`grants::tests`).
    for scope in Scope::ALL
        .into_iter()
        .filter(|scope| !matches!(scope, Scope::LocalUi | Scope::RemoteLink))
    {
        let client = routed(scope, a.clone()).await;
        let refused = client.relay_to("builds").await.unwrap_err();
        assert_eq!(code(&refused), Some(session::NOT_ALLOWED), "{scope}");
    }
    assert!(!stopped(&b).await);
}

#[tokio::test]
async fn a_runner_this_machine_does_not_keep_is_refused_and_the_link_stays_the_desktops() {
    let (a, b) = a_desktop(Kept::pinned);
    let window = routed(Scope::LocalUi, a.clone()).await;
    let refused = window.relay_to("nobody").await.unwrap_err();
    assert_eq!(code(&refused), Some(session::NOT_STARTED), "{refused}");
    assert!(
        refused.to_string().contains("no runner named nobody"),
        "{refused}"
    );
    window.stop_all().await.unwrap();
    assert!(stopped(&a).await);
    assert!(!stopped(&b).await);
}

#[tokio::test]
async fn a_host_that_relays_to_no_runner_refuses_relay_to() {
    let a = Hosting::running_with(Echo, StandIns::default());
    let window = routed(Scope::LocalUi, a).await;
    let refused = window.relay_to("builds").await.unwrap_err();
    assert_eq!(code(&refused), Some(session::NOT_STARTED), "{refused}");
}

#[tokio::test]
async fn a_runner_that_answers_with_another_key_is_refused_as_its_key_changed_and_sent_nothing() {
    let (a, b) = a_desktop(|_| Kept::new(RUNNER).pinned());
    let window = routed(Scope::LocalUi, a).await;
    window.relay_to("builds").await.unwrap();
    let refused = tokio::time::timeout(PATIENCE, window.stop_all())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(code(&refused), Some(session::KEY_CHANGED), "{refused}");
    assert!(!stopped(&b).await);
}

#[tokio::test]
async fn a_window_that_names_itself_first_is_still_relayed_and_its_name_goes_with_it() {
    let (a, b) = a_desktop(Kept::pinned);
    let window = routed(Scope::LocalUi, a.clone()).await;
    window
        .call(Command::NameWindow {
            name: purlis_session_protocol::browsing::Secret::new(
                "0123456789abcdef0123456789abcdef".into(),
            ),
        })
        .await
        .unwrap();
    tokio::time::timeout(PATIENCE, window.relay_to("builds"))
        .await
        .unwrap()
        .unwrap();
    // The relay holds the name now: a second one is refused as the relay refuses it.
    let again = window
        .call(Command::NameWindow {
            name: purlis_session_protocol::browsing::Secret::new(
                "fedcba9876543210fedcba9876543210".into(),
            ),
        })
        .await
        .unwrap_err();
    assert_eq!(code(&again), Some(session::MALFORMED), "{again}");
    tokio::time::timeout(PATIENCE, window.stop_all())
        .await
        .unwrap()
        .unwrap();
    assert!(stopped(&b).await);
    assert!(!stopped(&a).await);
}

#[tokio::test]
async fn a_second_name_while_the_link_is_undecided_is_refused_and_the_first_stays() {
    let (a, _b) = a_desktop(Kept::pinned);
    let window = routed(Scope::LocalUi, a).await;
    let name = |text: &str| Command::NameWindow {
        name: purlis_session_protocol::browsing::Secret::new(text.into()),
    };
    window
        .call(name("0123456789abcdef0123456789abcdef"))
        .await
        .unwrap();
    let refused = window
        .call(name("fedcba9876543210fedcba9876543210"))
        .await
        .unwrap_err();
    assert_eq!(code(&refused), Some(session::MALFORMED), "{refused}");
    // A relay_to refused keeps the name for the next one.
    assert!(window.relay_to("nobody").await.is_err());
    window.relay_to("builds").await.unwrap();
    let again = window
        .call(name("0123456789abcdef0123456789abcdef"))
        .await
        .unwrap_err();
    assert_eq!(code(&again), Some(session::MALFORMED), "{again}");
}

/// **One link of the desktop's host holds a window's name at a time** (#2623, #2744), across the
/// relays its links make: a second link that names itself so is refused `relay_to`
/// `not_allowed` while the first holds the name, and keeps the name, so the same `relay_to`
/// sent again once the first link has ended is relayed.
#[tokio::test]
async fn a_name_another_relayed_link_holds_is_refused_until_that_link_ends() {
    let (a, _b) = a_desktop(Kept::pinned);
    let name = purlis_session_protocol::browsing::Secret::new("ab".repeat(16));
    let first = routed(Scope::LocalUi, a.clone()).await;
    first
        .call(Command::NameWindow { name: name.clone() })
        .await
        .unwrap();
    tokio::time::timeout(PATIENCE, first.relay_to("builds"))
        .await
        .unwrap()
        .expect("the first link is relayed");

    let second = routed(Scope::LocalUi, a.clone()).await;
    second
        .call(Command::NameWindow { name })
        .await
        .expect("kept while the link is undecided");
    let refused = tokio::time::timeout(PATIENCE, second.relay_to("builds"))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(code(&refused), Some(session::NOT_ALLOWED), "{refused}");

    drop(first);
    let deadline = tokio::time::Instant::now() + PATIENCE;
    while let Err(refused) = second.relay_to("builds").await {
        assert_eq!(code(&refused), Some(session::NOT_ALLOWED), "{refused}");
        assert!(
            tokio::time::Instant::now() < deadline,
            "the name was never let go of"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    second.list().await.expect("relayed");
}

/// **A link that ends while its `relay_to` is taken holds no name** (#2744): the name it gave is
/// let go of with it, so the window's next link, named the same, is relayed. Here the runner's
/// lookup waits until the first link has ended.
#[tokio::test]
async fn a_link_that_ends_while_it_is_relayed_lets_go_of_its_name() {
    let gate = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let waits: Arc<dyn Fn() + Send + Sync> = {
        let gate = Arc::clone(&gate);
        Arc::new(move || {
            let (open, changed) = &*gate;
            let mut open = open.lock().unwrap();
            while !*open {
                open = changed.wait(open).unwrap();
            }
        })
    };
    let (a, _b) = a_desktop_waiting(Kept::pinned, waits);
    let name = purlis_session_protocol::browsing::Secret::new("cd".repeat(16));

    let first = routed(Scope::LocalUi, a.clone()).await;
    first
        .call(Command::NameWindow { name: name.clone() })
        .await
        .unwrap();
    let relaying = tokio::spawn(async move {
        let _ = first.relay_to("builds").await;
    });
    // The link ends while its runner is being looked up, then the lookup goes on.
    tokio::time::sleep(Duration::from_millis(100)).await;
    relaying.abort();
    let _ = relaying.await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    {
        let (open, changed) = &*gate;
        *open.lock().unwrap() = true;
        changed.notify_all();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let second = routed(Scope::LocalUi, a).await;
    second
        .call(Command::NameWindow { name })
        .await
        .expect("kept while the link is undecided");
    tokio::time::timeout(PATIENCE, second.relay_to("builds"))
        .await
        .unwrap()
        .expect("the name the ended link gave is free");
}

/// **A link relayed to a draining runner keeps its chats and starts no new one** (#2865): the
/// window answers and watches what runs there, and a start is refused in words that say where
/// new chats start. Asked at each start: a link relayed by its pool's name before the runner was
/// replaced is refused too, once it drains.
#[tokio::test]
async fn a_link_to_a_draining_runner_reaches_its_chats_and_starts_none() {
    let (a, _b) = a_desktop(Kept::pinned);
    let draining = Arc::new(std::sync::Mutex::new(vec!["builds/old".to_owned()]));
    let a = a.draining_with({
        let draining = Arc::clone(&draining);
        Arc::new(move |name: &str| draining.lock().unwrap().iter().any(|one| one == name))
    });
    let start =
        || session::StartProgram::of("01J9PRJCT00000000000000000", session::ProgramKind::Shell);
    let refused_as_draining = |started: &Result<String, CallError>| {
        started.as_ref().err().is_some_and(|why| {
            code(why) == Some(session::NOT_ALLOWED)
                && why
                    .to_string()
                    .contains("new chats start on its pool's runner")
        })
    };

    let old = routed(Scope::LocalUi, a.clone()).await;
    assert_eq!(
        tokio::time::timeout(PATIENCE, old.relay_to("builds/old"))
            .await
            .unwrap()
            .expect("a draining runner is reached")
            .as_deref(),
        Some("builds/old"),
        "the window is told the runner its link reached (1.48)"
    );
    old.list().await.expect("its chats are listed");
    assert!(refused_as_draining(&old.start_program(start()).await));

    // Relayed by the pool's name while its runner is the pool's: it starts chats.
    let now = routed(Scope::LocalUi, a).await;
    assert_eq!(
        tokio::time::timeout(PATIENCE, now.relay_to("builds"))
            .await
            .unwrap()
            .unwrap()
            .as_deref(),
        Some("builds/builds"),
        "a pool's name is answered with the runner it resolved to"
    );
    assert!(!refused_as_draining(&now.start_program(start()).await));
    // The pool moves to a new runner: that same link starts no new chat there.
    draining.lock().unwrap().push("builds/builds".to_owned());
    assert!(refused_as_draining(&now.start_program(start()).await));
}

#[test]
fn a_runner_is_named_without_the_key_its_link_reports() {
    assert_eq!(super::runner_named("team/team@abcd"), "team/team");
    assert_eq!(super::runner_named("builds/builds-1"), "builds/builds-1");
}

/// A runner's connector whose check finds the runner removed outside purlis (#4149), counting
/// each time it is asked.
#[derive(Clone)]
struct ToAGoneRunner {
    checks: Arc<std::sync::atomic::AtomicUsize>,
}

impl Connector for ToAGoneRunner {
    type Io = DuplexStream;

    fn connect(&self) -> BoxFuture<'static, std::io::Result<DuplexStream>> {
        self.checks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async {
            Err(std::io::Error::other(hostlink::FarEndGone(
                "pool builds's runner builds was removed outside purlis".into(),
            )))
        })
    }
}

/// **A runner removed outside purlis is refused, in plain words, and never linked to in a loop**
/// (#4149): each command tries the check at most twice, and is refused saying so.
#[tokio::test]
async fn a_runner_removed_outside_purlis_is_refused_in_plain_words_and_not_tried_in_a_loop() {
    let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let connector = ToAGoneRunner {
        checks: Arc::clone(&checks),
    };
    let desktop = Arc::new(Kept::new(DESKTOP));
    let pinned = Kept::new(RUNNER).pinned();
    let relays: RelaysTo = Arc::new(move |_: &str| {
        let (connector, desktop, pinned) =
            (connector.clone(), Arc::clone(&desktop), pinned.clone());
        Ok((
            Arc::new(move || {
                keep_linked(
                    connector.clone(),
                    Handshake::THIS_BUILD,
                    desktop.identity(),
                    pinned.clone(),
                    Liveness::default(),
                )
            }) as Keep,
            "builds/builds".to_owned(),
        ))
    });
    let a = Hosting::running_with(Echo, StandIns::default()).relaying_with(relays);
    let window = routed(Scope::LocalUi, a).await;
    window.relay_to("builds").await.unwrap();

    for _ in 0..3 {
        let refused = tokio::time::timeout(PATIENCE, window.list())
            .await
            .expect("refused, not left waiting")
            .unwrap_err();
        assert_eq!(code(&refused), Some(session::NOT_STARTED), "{refused}");
        assert!(
            refused
                .to_string()
                .contains("Runner removed outside purlis"),
            "{refused}"
        );
    }
    // Seen red: with the Down that ended the links not kept, the commands looped on the check.
    let asked = checks.load(std::sync::atomic::Ordering::SeqCst);
    assert!(asked <= 7, "the check was asked {asked} times");
}

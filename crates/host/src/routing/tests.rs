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
    let b = Hosting::running_with(Echo, StandIns::default());
    let (desktop, runner) = (Arc::new(Kept::new(DESKTOP)), Arc::new(Kept::new(RUNNER)));
    let pinned = pinned(&runner);
    let connector = ToTheRunner {
        b: b.clone(),
        runner,
        desktop: desktop.pinned(),
    };
    let relays: RelaysTo = Arc::new(move |name: &str| {
        if name != "builds" {
            return Err(format!("no runner named {name} is kept on this machine"));
        }
        let (connector, desktop, pinned) =
            (connector.clone(), Arc::clone(&desktop), pinned.clone());
        Ok(Arc::new(move || {
            keep_linked(
                connector.clone(),
                Handshake::THIS_BUILD,
                desktop.identity(),
                pinned.clone(),
                Liveness::default(),
            )
        }) as Keep)
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

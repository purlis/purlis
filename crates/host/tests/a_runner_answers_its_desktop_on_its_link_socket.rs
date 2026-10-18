// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner's host on its real link socket (#2044): what the bridge a connector runs opens
//! (ADR 0078 §2). The desktop it is paired with is linked and served the session protocol as
//! `remote-link`; another key, a runner that is not paired, and what the runner keeps that
//! cannot be read are refused; the link socket is this user's alone and goes when the host ends.
//!
//! These bind a socket, which a sandboxed agent cannot: they first run on CI.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_host::{
    Answering, ComposesNothing, HostError, Hosting, Pairing, Place, Unpairing, serve_answering,
};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::Link;
use tokio::net::UnixStream;
use tokio::sync::oneshot;

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const DESKTOP: &str = "01JA0DESKT0P00000000000000";

fn place_in(dir: &Path) -> Place {
    Place::at(
        dir.join("purlis-test").join("purlisd.sock"),
        dir.join("config").join("purlisd"),
    )
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

fn pinned_of(identity: &Identity) -> Pinned {
    Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    }
}

/// What the runner answers each link with, changed as a test goes.
type Paired = Arc<Mutex<Result<Option<(Identity, Pinned)>, String>>>;

fn answering(paired: &Paired) -> Answering {
    let paired = Arc::clone(paired);
    Arc::new(move || {
        paired
            .lock()
            .unwrap()
            .as_ref()
            .map(|found| {
                found.as_ref().map(|(ours, desktop)| {
                    (
                        Identity {
                            device: ours.device.clone(),
                            key: ours.key.clone(),
                        },
                        desktop.clone(),
                    )
                })
            })
            .map_err(Clone::clone)
    })
}

/// Takes the desktop out of the runner's peers, as `runnerlink::unpair` does: only the one
/// it is paired with.
fn unpairing(paired: &Paired) -> Unpairing {
    let paired = Arc::clone(paired);
    Arc::new(move |desktop: &Pinned| {
        let mut paired = paired.lock().unwrap();
        match &*paired {
            Ok(Some((_, kept))) if kept == desktop => {
                *paired = Ok(None);
                Ok(())
            }
            _ => Err("not this runner's peer".into()),
        }
    })
}

async fn hosting(
    place: &Place,
    paired: &Paired,
) -> (
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), HostError>>,
) {
    let (stop, stopped) = oneshot::channel::<()>();
    let (ready, is_ready) = oneshot::channel::<()>();
    let (place, pairing) = (
        place.clone(),
        Pairing {
            answering: answering(paired),
            unpairing: unpairing(paired),
        },
    );
    let host = tokio::spawn(async move {
        serve_answering(
            &place,
            None,
            Hosting::new(ComposesNothing),
            Some(pairing),
            move || {
                let _ = ready.send(());
            },
            async move {
                let _ = stopped.await;
            },
        )
        .await
    });
    if is_ready.await.is_err() {
        panic!("the host did not become ready: {:?}", host.await);
    }
    (stop, host)
}

async fn link(place: &Place, desktop: &Identity, runner: &Pinned) -> Result<Link, String> {
    let stream = UnixStream::connect(place.link())
        .await
        .map_err(|why| why.to_string())?;
    hostlink::connect(
        stream,
        Handshake::THIS_BUILD,
        desktop,
        runner,
        Liveness::default(),
    )
    .await
    .map_err(|why| why.to_string())
}

async fn list(link: &mut Link) -> serde_json::Value {
    called(link, "list").await
}

async fn called(link: &mut Link, command: &str) -> serde_json::Value {
    let call = serde_json::json!({ "call": { "id": 1, "command": command } });
    link.control()
        .send(bytes::Bytes::from(serde_json::to_vec(&call).unwrap()))
        .await
        .expect("the call was sent");
    let reply = link
        .control()
        .next()
        .await
        .expect("a reply came")
        .expect("the reply was read");
    serde_json::from_slice(&reply).unwrap()
}

#[tokio::test]
async fn the_desktop_a_runner_is_paired_with_is_linked_and_served() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let (_stop, _host) = hosting(&place, &paired).await;

    let mut linked = link(&place, &desktop, &runner_pinned)
        .await
        .expect("the paired desktop is linked");
    assert_eq!(
        list(&mut linked).await,
        serde_json::json!({ "reply": { "re": 1, "ok": { "chats": [] } } })
    );
}

#[tokio::test]
async fn a_desktop_the_runner_is_not_paired_with_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let runner = identity(RUNNER);
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((
        runner,
        pinned_of(&identity(DESKTOP)),
    )))));
    let (_stop, _host) = hosting(&place, &paired).await;

    let refused = link(&place, &identity(DESKTOP), &runner_pinned)
        .await
        .err()
        .expect("another desktop's key is refused");
    assert!(refused.contains("not the key of the desktop"), "{refused}");
}

#[tokio::test]
async fn a_runner_that_is_not_paired_or_cannot_read_its_pairing_links_nobody() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(None)));
    let (_stop, _host) = hosting(&place, &paired).await;
    assert!(link(&place, &desktop, &runner_pinned).await.is_err());

    *paired.lock().unwrap() = Err("the runner's link-key could not be read safely".into());
    assert!(link(&place, &desktop, &runner_pinned).await.is_err());

    // Paired while the host runs: in force at the next link.
    *paired.lock().unwrap() = Ok(Some((runner, pinned_of(&desktop))));
    assert!(link(&place, &desktop, &runner_pinned).await.is_ok());
}

#[tokio::test]
async fn the_link_socket_is_this_users_alone_and_goes_when_the_host_ends() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let paired: Paired = Arc::new(Mutex::new(Ok(None)));
    let (stop, host) = hosting(&place, &paired).await;

    let mode = std::fs::metadata(place.link())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(
        place.link(),
        place.socket.with_file_name("purlisd.link.sock")
    );

    stop.send(()).unwrap();
    host.await.unwrap().unwrap();
    assert!(!place.link().exists());
}

#[tokio::test]
async fn the_paired_desktop_unpairs_the_runner_and_is_linked_no_more() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let (_stop, _host) = hosting(&place, &paired).await;

    let mut linked = link(&place, &desktop, &runner_pinned)
        .await
        .expect("the paired desktop is linked");
    assert_eq!(
        called(&mut linked, "unpair").await,
        serde_json::json!({ "reply": { "re": 1, "ok": "done" } })
    );
    assert!(matches!(*paired.lock().unwrap(), Ok(None)));
    assert!(link(&place, &desktop, &runner_pinned).await.is_err());
}

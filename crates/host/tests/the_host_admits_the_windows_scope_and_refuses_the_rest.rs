// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host on its real socket (#1997): it admits a client that proves a scope's
//! credential, refuses a wrong one and one below the window that holds the chats, keeps its
//! socket and its folder this user's alone, leaves a host already serving alone, and removes
//! its socket when it ends. Another uid is the protocol crate's `local` check, which the host
//! cannot skip: the only connection it serves is one that check handed on.
//!
//! These bind a socket, which a sandboxed agent cannot: they first run on CI.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use purlis_host::{ComposesNothing, HostError, Hosting, Place, Window, serve};
use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session;
use tokio::net::UnixStream;
use tokio::sync::oneshot;

/// A host's place in a folder of the test's own.
fn place_in(dir: &Path) -> Place {
    Place::at(
        dir.join("purlis-test").join("purlisd.sock"),
        dir.join("config").join("purlisd"),
    )
}

/// A host serving at `place` on a task, ready once this returns, until the sender is used or
/// dropped.
async fn hosting(
    place: &Place,
    window: Option<Window>,
) -> (
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), HostError>>,
) {
    let (stop, stopped) = oneshot::channel::<()>();
    let (ready, is_ready) = oneshot::channel::<()>();
    let place = place.clone();
    let host = tokio::spawn(async move {
        serve(
            &place,
            window,
            Hosting::new(ComposesNothing),
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

async fn connect(place: &Place, scope: Scope, credential: &Credential) -> Result<Link, String> {
    let stream = UnixStream::connect(&place.socket)
        .await
        .map_err(|why| why.to_string())?;
    link::connect(stream, session::speaks(), scope, credential)
        .await
        .map_err(|why| why.to_string())
}

/// What the host answers `list`, as the frame's JSON.
async fn list(link: &mut Link) -> serde_json::Value {
    let call = serde_json::json!({ "call": { "id": 1, "command": "list" } });
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

fn this_process() -> Window {
    Window(std::process::id())
}

#[tokio::test]
async fn the_window_is_admitted_with_its_scopes_credential_and_told_no_program_runs() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    // This test is the window: the host's chats would run below it, and it is not inside one.
    let (_stop, _host) = hosting(&place, Some(this_process())).await;

    let credential = Credential::read(&place.credentials, Scope::LocalUi).expect("minted");
    let mut window = connect(&place, Scope::LocalUi, &credential)
        .await
        .expect("the window is admitted");

    assert_eq!(
        list(&mut window).await,
        serde_json::json!({ "reply": { "re": 1, "ok": { "chats": [] } } })
    );
}

#[tokio::test]
async fn a_wrong_credential_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (_stop, _host) = hosting(&place, None).await;

    let wrong = Credential::mint().unwrap();
    assert!(connect(&place, Scope::LocalUi, &wrong).await.is_err());
    // Another scope's credential is a wrong one too.
    let terminals = Credential::read(&place.credentials, Scope::Terminal).unwrap();
    assert!(connect(&place, Scope::LocalUi, &terminals).await.is_err());
}

#[tokio::test]
async fn a_process_below_the_window_that_holds_the_chats_is_refused_whatever_it_proves() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    // The window is this test's parent, so this test is below it, as a chat's process is.
    let parent = Window(std::os::unix::process::parent_id());
    let (_stop, _host) = hosting(&place, Some(parent)).await;

    for scope in [Scope::LocalUi, Scope::Terminal, Scope::Approval] {
        let credential = Credential::read(&place.credentials, scope).unwrap();
        assert!(
            connect(&place, scope, &credential).await.is_err(),
            "{scope} was admitted from below the window"
        );
    }
}

#[tokio::test]
async fn the_socket_and_its_folder_are_this_users_alone_and_the_socket_goes_with_the_host() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (stop, host) = hosting(&place, None).await;

    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(place.socket.parent().unwrap()), 0o700);
    assert_eq!(mode(&place.socket), 0o600);
    assert_eq!(mode(&place.credentials), 0o700);
    assert_eq!(mode(&place.credentials.join("local-ui")), 0o600);

    stop.send(()).unwrap();
    host.await.unwrap().expect("the host ended cleanly");
    assert!(!place.socket.exists(), "the socket outlived its host");
}

#[tokio::test]
async fn a_second_host_leaves_the_one_serving_alone() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (_stop, _host) = hosting(&place, None).await;
    let credential = Credential::read(&place.credentials, Scope::LocalUi).unwrap();

    let second = serve(
        &place,
        None,
        Hosting::new(ComposesNothing),
        || panic!("a second host became ready"),
        async {},
    )
    .await;
    assert!(
        matches!(second, Err(HostError::AlreadyServing { .. })),
        "{second:?}"
    );

    // The first host's socket and credentials are as they were.
    assert_eq!(
        Credential::read(&place.credentials, Scope::LocalUi).unwrap(),
        credential
    );
    assert!(connect(&place, Scope::LocalUi, &credential).await.is_ok());
}

#[tokio::test]
async fn a_socket_a_host_left_behind_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    {
        let (stop, host) = hosting(&place, None).await;
        drop(stop);
        host.await.unwrap().unwrap();
    }
    // A host that was killed outright leaves its socket file, and nobody listening on it.
    purlis_same_user::private_directory(place.socket.parent().unwrap()).unwrap();
    drop(std::os::unix::net::UnixListener::bind(&place.socket).unwrap());

    let (_stop, _host) = hosting(&place, None).await;
    let credential = Credential::read(&place.credentials, Scope::LocalUi).unwrap();
    assert!(connect(&place, Scope::LocalUi, &credential).await.is_ok());
}

#[tokio::test]
async fn a_socket_folder_that_is_a_link_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, place.socket.parent().unwrap()).unwrap();

    let refused = serve(
        &place,
        None,
        Hosting::new(ComposesNothing),
        || panic!("it became ready"),
        async {},
    )
    .await;
    assert!(
        matches!(refused, Err(HostError::Unsafe { .. })),
        "{refused:?}"
    );
    assert!(!place.credentials.exists(), "credentials were minted");
}

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

/// #2114: the host of another config home, its socket named apart in the same folder, serves
/// beside the one serving, each with its own lock and credentials.
#[tokio::test]
async fn the_host_of_another_config_home_serves_beside_the_one_serving() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let other = Place::at(
        place.socket.with_file_name("purlisd-0123456789abcdef.sock"),
        dir.path().join("another-config").join("purlisd"),
    );
    let (_stop, _host) = hosting(&place, None).await;
    let (_other_stop, _other_host) = hosting(&other, None).await;

    for place in [&place, &other] {
        let credential = Credential::read(&place.credentials, Scope::LocalUi).unwrap();
        assert!(connect(place, Scope::LocalUi, &credential).await.is_ok());
    }
    assert_ne!(place.lock, other.lock);
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

/// Runs every start as `/bin/true`, for a host whose programs are stand-ins.
struct True;

impl purlis_host::Compose for True {
    fn compose(
        &self,
        starting: &purlis_host::ToCompose<'_>,
    ) -> Result<purlis_terminal::session::Spec, String> {
        Ok(purlis_terminal::session::Spec::new(
            "/bin/true",
            starting.size,
        ))
    }
}

#[tokio::test]
async fn a_host_that_stops_ends_every_program_it_runs() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let runs = purlis_host::standin::StandIns::default();
    let (stop, stopped) = oneshot::channel::<()>();
    let (ready, is_ready) = oneshot::channel::<()>();
    let host = tokio::spawn({
        let place = place.clone();
        let hosting = Hosting::running_with(True, runs.clone());
        async move {
            serve(
                &place,
                Some(this_process()),
                hosting,
                move || {
                    let _ = ready.send(());
                },
                async move {
                    let _ = stopped.await;
                },
            )
            .await
        }
    });
    is_ready.await.expect("the host became ready");
    let credential = Credential::read(&place.credentials, Scope::LocalUi).unwrap();
    let link = connect(&place, Scope::LocalUi, &credential).await.unwrap();
    let (client, _views, _events) = session::Client::new(link);
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    client
        .hold_project("01J9PRJCT00000000000000000", project.to_str().unwrap())
        .await
        .unwrap();
    for _ in 0..2 {
        client
            .start_program(session::StartProgram::of(
                "01J9PRJCT00000000000000000",
                session::ProgramKind::Shell,
            ))
            .await
            .unwrap();
    }

    stop.send(()).unwrap();
    host.await.unwrap().expect("the host ended cleanly");

    assert_eq!(runs.ran().len(), 2);
    assert!(runs.ran().iter().all(|(_, run)| run.seen().stopped));
}

/// The window quits (#2006): the host ends every program as a quit, answers, stops serving and
/// removes its socket, with nothing else to tell it to stop.
#[tokio::test]
async fn a_quit_from_the_window_ends_every_program_and_the_host() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let runs = purlis_host::standin::StandIns::default();
    let (ready, is_ready) = oneshot::channel::<()>();
    let host = tokio::spawn({
        let place = place.clone();
        let hosting = Hosting::running_with(True, runs.clone());
        async move {
            serve(
                &place,
                Some(this_process()),
                hosting,
                move || {
                    let _ = ready.send(());
                },
                std::future::pending(),
            )
            .await
        }
    });
    is_ready.await.expect("the host became ready");
    let credential = Credential::read(&place.credentials, Scope::LocalUi).unwrap();
    let link = connect(&place, Scope::LocalUi, &credential).await.unwrap();
    let (client, _views, _events) = session::Client::new(link);
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    client
        .hold_project("01J9PRJCT00000000000000000", project.to_str().unwrap())
        .await
        .unwrap();
    client
        .start_program(session::StartProgram::of(
            "01J9PRJCT00000000000000000",
            session::ProgramKind::Shell,
        ))
        .await
        .unwrap();

    client.quit().await.expect("the quit is answered");

    tokio::time::timeout(std::time::Duration::from_secs(10), host)
        .await
        .expect("the host stops")
        .unwrap()
        .expect("the host ended cleanly");
    assert!(runs.ran().iter().all(|(_, run)| run.seen().stopped));
    assert!(!place.socket.exists(), "the socket outlived its host");
}

/// A peer that is not the window may not quit the host: the grants refuse it, and the host goes
/// on (#2006).
#[tokio::test]
async fn only_the_window_quits_the_host() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (_stop, host) = hosting(&place, None).await;
    let credential = Credential::read(&place.credentials, Scope::Terminal).unwrap();
    let link = connect(&place, Scope::Terminal, &credential).await.unwrap();
    let (client, _views, _events) = session::Client::new(link);

    let refused = client.quit().await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(!host.is_finished(), "the host goes on");
}

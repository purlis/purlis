// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner's host left without a link from its desktop suspends itself (#2051, spec #2041
//! story 39): `purlis serve` on a runner serves until it has gone the limit with no link, while
//! a desktop is paired with it. A link that ends leaves its programs running until then, so a
//! laptop closed overnight stops nothing. The programs are stand-ins and the links in memory.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, Place, ToCompose, serve};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const LIMIT: Duration = Duration::from_millis(300);
const PROJECT: &str = "01J9PRJCT00000000000000000";

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A link from the desktop, as the link between hosts is admitted: by its device.
async fn the_desktops_link(hosting: &Hosting) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks()),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    Client::new(client.unwrap()).0
}

#[tokio::test]
async fn a_paired_runner_never_linked_suspends_once_the_limit_has_passed() {
    let hosting = Hosting::running_with(Echo, StandIns::default());
    timeout(LIMIT * 10, hosting.until_unlinked_for(LIMIT, || true))
        .await
        .expect("it suspended itself");
}

#[tokio::test]
async fn a_runner_with_its_desktop_linked_does_not_suspend() {
    let hosting = Hosting::running_with(Echo, StandIns::default());
    let desktop = the_desktops_link(&hosting).await;
    desktop.list().await.unwrap();

    let waited = timeout(LIMIT * 4, hosting.until_unlinked_for(LIMIT, || true)).await;

    assert!(waited.is_err(), "it suspended with its desktop linked");
}

#[tokio::test]
async fn the_limit_runs_again_from_when_the_last_link_ended() {
    let hosting = Hosting::running_with(Echo, StandIns::default());
    let desktop = the_desktops_link(&hosting).await;
    desktop.list().await.unwrap();
    let suspended = tokio::spawn({
        let hosting = hosting.clone();
        async move { hosting.until_unlinked_for(LIMIT, || true).await }
    });
    tokio::time::sleep(LIMIT * 2).await;
    assert!(!suspended.is_finished(), "linked, it waits");

    drop(desktop);

    timeout(LIMIT * 10, suspended)
        .await
        .expect("it suspended once the limit passed with no link")
        .unwrap();
}

#[tokio::test]
async fn a_host_no_desktop_is_paired_with_never_suspends() {
    let hosting = Hosting::running_with(Echo, StandIns::default());
    let waited = timeout(LIMIT * 4, hosting.until_unlinked_for(LIMIT, || false)).await;
    assert!(waited.is_err());
}

// Binds a socket, so it runs on CI only.
#[tokio::test]
async fn a_runner_that_suspends_itself_ends_every_chat_it_runs_and_stops_serving() {
    let dir = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let place = Place::at(
        dir.path().join("purlis-test").join("purlisd.sock"),
        dir.path().join("config").join("purlisd"),
    );
    let runs = StandIns::default();
    let hosting = Hosting::running_with(Echo, runs.clone());
    let desktop = the_desktops_link(&hosting).await;
    desktop
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .unwrap();
    desktop
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .unwrap();
    // The laptop closed: the link ends, and the chat keeps running until the limit.
    drop(desktop);
    assert!(!runs.last().seen().stopped);

    let waiting = hosting.clone();
    let suspended = async move { waiting.until_unlinked_for(LIMIT, || true).await };
    timeout(LIMIT * 20, serve(&place, None, hosting, || {}, suspended))
        .await
        .expect("it stopped serving once the limit passed")
        .unwrap();

    assert!(runs.last().seen().stopped, "every chat it ran ended");
}

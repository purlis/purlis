// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's host kept by its machine stops itself once a newer version is installed beside
//! it and it has nothing left to run** (#6704; ADR 0068 §3 as amended for #6704). Its service
//! manager does not start a host that exited 0 again, so pruning finds it drained. A live chat or
//! an open link from its desktop keeps it, however long. The programs are stand-ins and the
//! links in memory; `newer` stands in for the disk.

#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, ToCompose};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const AFTER: Duration = Duration::from_millis(300);
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
async fn an_old_versions_host_with_nothing_to_run_stops_once_its_time_has_passed() {
    let hosting = Hosting::new(Echo);
    timeout(
        AFTER * 10,
        hosting.until_superseded_and_idle(AFTER, || true),
    )
    .await
    .expect("it stopped itself");
}

#[tokio::test]
async fn the_newest_versions_host_never_stops_for_want_of_work() {
    let hosting = Hosting::new(Echo);
    let waited = timeout(
        AFTER * 4,
        hosting.until_superseded_and_idle(AFTER, || false),
    )
    .await;
    assert!(waited.is_err(), "nothing newer is installed: it serves on");
}

#[tokio::test]
async fn an_old_versions_host_running_a_chat_with_nobody_linked_never_stops() {
    let project = tempfile::tempdir().unwrap();
    let hosting = Hosting::running_with(Echo, StandIns::default());
    let desktop = the_desktops_link(&hosting).await;
    desktop
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .unwrap();
    desktop
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .unwrap();
    drop(desktop);
    let waited = timeout(AFTER * 4, hosting.until_superseded_and_idle(AFTER, || true)).await;
    assert!(waited.is_err(), "a live chat drains where it runs");
}

#[tokio::test]
async fn an_open_link_from_its_desktop_keeps_an_old_versions_host_until_it_ends() {
    let hosting = Hosting::new(Echo);
    let desktop = the_desktops_link(&hosting).await;
    desktop.list().await.unwrap();
    let stopped = tokio::spawn({
        let hosting = hosting.clone();
        async move { hosting.until_superseded_and_idle(AFTER, || true).await }
    });
    tokio::time::sleep(AFTER * 3).await;
    assert!(!stopped.is_finished(), "linked, it waits");

    drop(desktop);

    timeout(AFTER * 10, stopped)
        .await
        .expect("it stopped once its time passed with no link")
        .unwrap();
}

#[tokio::test]
async fn its_time_starts_as_a_newer_version_is_installed() {
    let hosting = Hosting::new(Echo);
    let newer = Arc::new(AtomicBool::new(false));
    let says = {
        let newer = newer.clone();
        move || newer.load(Ordering::SeqCst)
    };
    let stopped = tokio::spawn({
        let hosting = hosting.clone();
        async move { hosting.until_superseded_and_idle(AFTER, says).await }
    });
    tokio::time::sleep(AFTER * 2).await;
    newer.store(true, Ordering::SeqCst);
    tokio::time::sleep(AFTER / 2).await;
    assert!(
        !stopped.is_finished(),
        "idle before the newer version came does not count"
    );
    timeout(AFTER * 4, stopped)
        .await
        .expect("it stopped once its time passed since")
        .unwrap();
}

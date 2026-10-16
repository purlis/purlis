// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host hears `purlis stop --all` itself, with no window there (#2190, ADR 0071):
//! a stop the command line writes is read through the seam the binary gives the host
//! ([`purlis_host::StopsElsewhere`], which reads ADR 0071's files in the window's binary and in
//! `purlis serve`), as the host starts, before every start, and on a look every few seconds.
//! The host keeps it in its own switch and acts on it as `stop_all`; only the window re-arms.
//! The programs are stand-ins and the links in memory, so none of this needs a terminal.

#![cfg(unix)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, StopsElsewhere, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";

/// Well past the host's look every few seconds: a stop not heard by then was missed.
const HEARD_WITHIN: Duration = Duration::from_secs(15);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// ADR 0071's files as a test writes them: the stops written so far, whether they still say
/// stopped, and how many times the host has looked at them.
#[derive(Clone, Default)]
struct Files(Arc<Written>);

#[derive(Default)]
struct Written {
    stops: AtomicUsize,
    stopped: AtomicBool,
    looks: AtomicUsize,
}

impl Files {
    /// `purlis stop --all`: one more stop, and the files say stopped.
    fn throw(&self) {
        self.0.stops.fetch_add(1, Ordering::SeqCst);
        self.0.stopped.store(true, Ordering::SeqCst);
    }
    /// The window's re-arm of the files.
    fn clear(&self) {
        self.0.stopped.store(false, Ordering::SeqCst);
    }
    /// Waits until the host has looked at the files again, twice over, so a look began after
    /// this was asked: bounded, as every wait here is.
    async fn looked_at_again(&self) {
        let from = self.0.looks.load(Ordering::SeqCst);
        let began = Instant::now();
        while self.0.looks.load(Ordering::SeqCst) < from + 2 {
            assert!(began.elapsed() < HEARD_WITHIN, "the host stopped looking");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl StopsElsewhere for Files {
    fn last_stop(&self) -> Option<String> {
        self.0.looks.fetch_add(1, Ordering::SeqCst);
        self.0
            .stopped
            .load(Ordering::SeqCst)
            .then(|| format!("stop {}", self.0.stops.load(Ordering::SeqCst)))
    }
}

/// A host whose switch is kept in `folder`, reading `files` as the binary's seam reads ADR
/// 0071's, and looking at them as it serves.
fn a_host(folder: &Path, runs: &StandIns, files: &Files) -> Hosting {
    let hosting =
        Hosting::running_with_its_switch_in(folder, Echo, runs.clone()).heeding(files.clone());
    tokio::spawn(hosting.clone().heed_stops_written_elsewhere());
    hosting
}

async fn the_window(hosting: &Hosting, project: &Path) -> Client {
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
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    client
}

fn a_harness() -> StartProgram {
    StartProgram::of(PROJECT, ProgramKind::Harness)
}

async fn refused_start(client: &Client) -> Option<String> {
    client
        .start_program(a_harness())
        .await
        .err()
        .and_then(|refused| refused.refusal().map(|refusal| refusal.code.clone()))
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let began = Instant::now();
    while !done() {
        assert!(began.elapsed() < HEARD_WITHIN, "still waiting: {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn with_no_window_a_stop_written_elsewhere_ends_every_program_and_holds() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, files) = (StandIns::default(), Files::default());
    let hosting = a_host(folder.path(), &runs, &files);
    {
        let window = the_window(&hosting, project.path()).await;
        for _ in 0..2 {
            window.start_program(a_harness()).await.expect("it starts");
        }
    }
    // The window is gone, as after a crash; `purlis stop --all` writes its files.
    files.throw();

    until("every program to be stopped", || {
        runs.ran().iter().all(|(_, run)| run.seen().stopped)
    })
    .await;
    let window = the_window(&hosting, project.path()).await;
    assert!(window.kill_switch().await.unwrap().stopped);
    assert_eq!(
        refused_start(&window).await.as_deref(),
        Some(session::STOPPED)
    );
}

#[tokio::test]
async fn a_stop_written_while_no_host_ran_holds_from_its_first_start() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, files) = (StandIns::default(), Files::default());
    files.throw();
    let hosting = Hosting::running_with_its_switch_in(folder.path(), Echo, runs.clone())
        .heeding(files.clone());
    let window = the_window(&hosting, project.path()).await;

    // No look has run yet: the start itself reads the files first.
    assert_eq!(
        refused_start(&window).await.as_deref(),
        Some(session::STOPPED)
    );
    assert!(runs.ran().is_empty(), "nothing ran");
}

#[tokio::test]
async fn files_cleared_while_stopped_re_arm_nothing_in_the_host() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, files) = (StandIns::default(), Files::default());
    let hosting = a_host(folder.path(), &runs, &files);
    let window = the_window(&hosting, project.path()).await;
    files.throw();
    until_the_host_is_stopped(&window).await;

    files.clear();
    files.looked_at_again().await;

    assert!(window.kill_switch().await.unwrap().stopped);
    assert_eq!(
        refused_start(&window).await.as_deref(),
        Some(session::STOPPED)
    );
}

#[tokio::test]
async fn the_windows_re_arm_holds_while_its_files_are_cleared_and_a_later_stop_is_heard() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, files) = (StandIns::default(), Files::default());
    let hosting = a_host(folder.path(), &runs, &files);
    let window = the_window(&hosting, project.path()).await;
    files.throw();
    until_the_host_is_stopped(&window).await;

    // The window re-arms the host first and its files after: a look between the two does not
    // stop the host again.
    window.rearm().await.expect("re-armed");
    files.looked_at_again().await;
    window.start_program(a_harness()).await.expect("it starts");
    files.clear();
    files.looked_at_again().await;

    // And the next stop the command line writes is heard.
    files.throw();
    until("the next stop to be heard", || runs.last().seen().stopped).await;
}

/// A window gone between re-arming the host and clearing its files leaves them saying stopped.
/// A stop written after that, with no window there, is a stop of its own, and is heard.
#[tokio::test]
async fn a_stop_written_while_the_files_still_hold_an_older_one_is_heard() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, files) = (StandIns::default(), Files::default());
    let hosting = a_host(folder.path(), &runs, &files);
    {
        let window = the_window(&hosting, project.path()).await;
        files.throw();
        until_the_host_is_stopped(&window).await;
        window.rearm().await.expect("re-armed");
        files.looked_at_again().await;
        window.start_program(a_harness()).await.expect("it starts");
        // The window goes before it clears its files.
    }

    files.throw();

    until("the stop to be heard", || runs.last().seen().stopped).await;
}

/// Waits until the host says every agent is stopped.
async fn until_the_host_is_stopped(window: &Client) {
    let began = Instant::now();
    while !window.kill_switch().await.unwrap().stopped {
        assert!(began.elapsed() < HEARD_WITHIN, "the stop was not heard");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

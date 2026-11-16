// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A host told to stop by `SIGTERM`, as systemd and launchd stop a runner's host, hears it as a
//! quit (#6700, ADR 0068 §3 as amended for #6700): the process is not ended by the signal, the
//! host's life resolves, and what it does then is what it does at every quit, each program ended
//! as one (`Stopped::Quit`) and its host record emptied, so the next host settles nothing as a
//! crash. `purlis serve`'s own exit, socket and journal are
//! `purlis-cli`'s `purlis_serve_hears_sigterm_as_a_quit`, which binds a socket and so runs on CI.
//!
//! The test raises `SIGTERM` in its own process once the host's handler is in: under `cargo
//! test`, where the tests share a process, that handler stays for the rest of the run, and only
//! changes that `SIGTERM` no longer ends it.

#![cfg(unix)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::StandIns;
use purlis_host::{
    Compose, ConfinesNothing, Ended, HearsNothing, Hosting, Record, Started, Stopped, ToCompose,
};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A program's number, and the host's own act that ended it.
type EndedBy = (u32, Option<Stopped>);

/// How each program the host ran ended, and what it was told the last host left.
#[derive(Clone, Default)]
struct Kept {
    ended: Arc<Mutex<Vec<EndedBy>>>,
    left: Arc<Mutex<Vec<Started>>>,
}

impl Record for Kept {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, ended: &Ended<'_>) {
        self.ended
            .lock()
            .unwrap()
            .push((ended.number, ended.stopped));
    }

    fn left_by_its_last_host(&self, left: &Started) {
        self.left.lock().unwrap().push(left.clone());
    }
}

fn a_host_in(folder: &Path, runs: &StandIns, kept: &Kept) -> Hosting {
    Hosting::kept(folder, Echo, runs.clone(), HearsNothing, ConfinesNothing)
        .recording_with(kept.clone())
}

async fn the_desktop(hosting: &Hosting, path: &Path) -> Client {
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
        .hold_project(PROJECT, path.to_str().unwrap())
        .await
        .expect("the project is held");
    client
}

#[tokio::test]
async fn sigterm_ends_each_program_as_a_quit_and_leaves_the_next_host_nothing_to_settle() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host_in(folder.path(), &runs, &kept);
    let desktop = the_desktop(&hosting, project.path()).await;
    for run in ["run-1", "run-2"] {
        desktop
            .start_program(StartProgram {
                run: Some(run.to_owned()),
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("it starts");
    }
    // What `purlis serve` lives until, its handler in before it says it is ready.
    let told = purlis_host::told_to_stop().expect("SIGTERM is handled");

    nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).unwrap();
    tokio::time::timeout(Duration::from_secs(5), told)
        .await
        .expect("the host heard SIGTERM, and is still here to end its programs");
    let ending = hosting.clone();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();

    let mut ended = kept.ended.lock().unwrap().clone();
    ended.sort_by_key(|(number, _)| *number);
    assert_eq!(
        ended,
        [(1, Some(Stopped::Quit)), (2, Some(Stopped::Quit))],
        "each program ended as a quit ends it"
    );
    let next = Kept::default();
    assert_eq!(
        a_host_in(folder.path(), &StandIns::default(), &next).take_over_from_the_last_host(),
        0,
        "nothing is left to settle as a crash"
    );
    assert!(next.left.lock().unwrap().is_empty());
}

/// Set in the copy of this test binary that [`a_second_ctrl_c_ends_the_host_at_once`] runs.
const THE_HOST: &str = "PURLIS_HOST_TEST_HEARS_A_SECOND_CTRL_C";

/// A person at a terminal presses Ctrl-C again while the host waits for each chat's end: the
/// host exits at once, with 130, rather than swallowing it (#6700, ADR 0068 §3 as amended for
/// #6700). It runs this test binary again, as the host, because the exit ends its process.
#[test]
fn a_second_ctrl_c_ends_the_host_at_once() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    if std::env::var_os(THE_HOST).is_some() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let told = purlis_host::told_to_stop().expect("SIGINT is handled");
            println!("ready");
            told.await;
            println!("told");
            // Longer than any wait for the chats' ends: only the second Ctrl-C ends it sooner.
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        return;
    }
    let mut host = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "a_host_told_to_stop_by_sigterm_hears_it_as_a_quit::a_second_ctrl_c_ends_the_host_at_once",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(THE_HOST, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the host starts");
    let pid = nix::unistd::Pid::from_raw(i32::try_from(host.id()).unwrap());
    let mut said = BufReader::new(host.stdout.take().unwrap()).lines();
    // The test harness says the test's name on the line before the host's first word.
    let mut until = |line: &str| {
        for heard in said.by_ref() {
            if heard.unwrap().trim_end().ends_with(line) {
                return;
            }
        }
        panic!("the host never said {line:?}");
    };
    until("ready");
    nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGINT).unwrap();
    until("told");
    let began = Instant::now();
    nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGINT).unwrap();
    let ended = host.wait().unwrap();

    assert_eq!(
        ended.code(),
        Some(purlis_host::FORCED_EXIT),
        "the second Ctrl-C ended the host as forced: {ended:?}"
    );
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "at once, not after its wait"
    );
}

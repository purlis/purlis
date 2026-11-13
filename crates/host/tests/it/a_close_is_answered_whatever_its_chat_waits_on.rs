// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A close is answered at once, whatever a chat's program is waiting on** (#6437). A host took
//! about 30 s to answer a window's Close on a loaded macOS runner. Two things a host does to a
//! chat's program can wait that long on a loaded machine: a note of its strays runs `ps`, and a
//! hold takes the lock a program's end holds while it hangs up and reaps. Neither may hold up a
//! Close: not one of the same chat, which keeps its order behind them, and not one of another
//! chat, which shares the host's one thread with them.
//!
//! The host here serves on a thread and a runtime of its own, a current-thread one as the
//! shipped host's is (`purlis_host::run`), so a host that blocks its thread is seen as a
//! timeout on the window's side and does not hold up the test's own clock.

#![cfg(unix)]

use std::time::{Duration, Instant};

use purlis_host::standin::{Latch, StandIns};
use purlis_host::{Compose, Hosting, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
/// Half the second a window waits on a close on its own thread (#6385), and far above what an
/// answer from a host that is not waiting on anything takes.
const PROMPTLY: Duration = Duration::from_millis(500);
/// How long the gate stays shut at the most, so a host that waits on it is not waited on for
/// good: well past [`PROMPTLY`].
const SHUT_AT_MOST: Duration = Duration::from_secs(10);
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A window's link to a host serving on a thread and a current-thread runtime of its own, with
/// a project held, and the stand-ins its programs are.
async fn a_window() -> (Client, StandIns, tempfile::TempDir) {
    let runs = StandIns::default();
    let (a, b) = duplex(256 * 1024);
    let hosts_runs = runs.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let hosting = Hosting::running_with(Echo, hosts_runs);
            let served = link::serve_any(b, session::speaks(), &HELD).await.unwrap();
            let _ = session::serve(served, hosting, None).await;
        });
    });
    let connected = link::connect(
        a,
        session::speaks(),
        Scope::LocalUi,
        HELD.of(Scope::LocalUi),
    )
    .await
    .unwrap();
    let (client, _views, _events) = Client::new(connected);
    let project = tempfile::tempdir().unwrap();
    client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("the project is held");
    (client, runs, project)
}

async fn a_chat(client: &Client) -> String {
    client
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .expect("it starts")
}

/// A gate that opens by itself after [`SHUT_AT_MOST`].
fn a_gate() -> Latch {
    let gate = Latch::default();
    let opens = gate.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SHUT_AT_MOST);
        opens.open();
    });
    gate
}

/// Until the stand-in `run` of `runs` is waiting at its gate.
async fn until_it_waits(runs: &StandIns, run: usize) {
    let began = Instant::now();
    while runs.ran()[run].1.seen().waiting == 0 {
        assert!(began.elapsed() < PATIENCE, "it never reached its gate");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn a_close_is_answered_while_a_note_of_the_chats_strays_still_lists_them() {
    let (client, runs, _project) = a_window().await;
    let client = std::sync::Arc::new(client);
    let chat = a_chat(&client).await;
    let gate = a_gate();
    runs.last().waits(&gate);
    let noting = {
        let (client, chat) = (std::sync::Arc::clone(&client), chat.clone());
        tokio::spawn(async move { client.note_strays(&chat).await })
    };
    until_it_waits(&runs, 0).await;

    let asked = Instant::now();
    let closed = timeout(SHUT_AT_MOST, client.close(&chat)).await;
    let took = asked.elapsed();

    gate.open();
    assert!(
        matches!(closed, Ok(Ok(()))),
        "the close was answered: {closed:?}"
    );
    assert!(
        took < PROMPTLY,
        "a close waited {took:?} behind a note of its chat's strays that was still listing them"
    );
    let _ = noting.await;
}

#[tokio::test]
async fn a_hold_waiting_on_one_chats_program_holds_up_no_close_of_another() {
    let (client, runs, _project) = a_window().await;
    let client = std::sync::Arc::new(client);
    let held = a_chat(&client).await;
    let closing = a_chat(&client).await;
    let gate = a_gate();
    runs.ran()[0].1.waits(&gate);
    let holding = {
        let (client, held) = (std::sync::Arc::clone(&client), held.clone());
        tokio::spawn(async move { client.hold(&held, true).await })
    };
    until_it_waits(&runs, 0).await;

    let asked = Instant::now();
    let closed = timeout(SHUT_AT_MOST, client.close(&closing)).await;
    let took = asked.elapsed();

    gate.open();
    assert!(
        matches!(closed, Ok(Ok(()))),
        "the close was answered: {closed:?}"
    );
    assert!(
        took < PROMPTLY,
        "a close of one chat waited {took:?} on a hold of another whose program was ending"
    );
    let _ = holding.await;
}

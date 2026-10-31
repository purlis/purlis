// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The kill switch's authority is the session host's (#2001, ADR 0071), spoken over the session
//! protocol on in-memory links: `stop_all` ends every program the host runs and starts none
//! until the window re-arms; the stop is kept in the host's own folder, so a host started again
//! while stopped starts nothing; and only the window's `local-ui` link re-arms. The programs are
//! stand-ins, so none of this needs a terminal.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, KILL_SWITCH, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A host whose switch is kept in `folder`, as `purlis serve` keeps it in its own.
fn a_host_keeping_its_switch_in(folder: &Path, runs: &StandIns) -> Hosting {
    Hosting::running_with_its_switch_in(folder, Echo, runs.clone())
}

/// A link to `hosting` admitted as `scope`: proved by its credential, or, for `remote-link`,
/// as another host's link is, by its device.
async fn a_link(hosting: &Hosting, scope: Scope) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = if scope == Scope::RemoteLink {
        tokio::join!(
            link::connect_as_a_device(a, session::speaks()),
            link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
        )
    } else {
        tokio::join!(
            link::connect(a, session::speaks(), scope, HELD.of(scope)),
            link::serve_any(b, session::speaks(), &HELD)
        )
    };
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

/// The window's link, with `project` held.
async fn the_window(hosting: &Hosting, project: &Path) -> Client {
    let window = a_link(hosting, Scope::LocalUi).await;
    window
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    window
}

fn a_harness() -> StartProgram {
    StartProgram::of(PROJECT, ProgramKind::Harness)
}

/// The refusal code a start got, if it was refused.
async fn refused_start(client: &Client, start: StartProgram) -> Option<String> {
    client
        .start_program(start)
        .await
        .err()
        .and_then(|refused| refused.refusal().map(|refusal| refusal.code.clone()))
}

#[tokio::test]
async fn stop_all_ends_every_program_the_host_runs_and_keeps_each_listed() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&hosting, project.path()).await;
    for _ in 0..3 {
        window.start_program(a_harness()).await.expect("it starts");
    }

    let switch = window.stop_all().await.expect("stopped");

    assert!(switch.stopped);
    assert_eq!(switch.why, None, "the stop was kept");
    assert!(runs.ran().iter().all(|(_, run)| run.seen().stopped));
    let listed = window.list().await.unwrap();
    assert_eq!(listed.len(), 3, "each is still there, with its last screen");
    assert!(listed.iter().all(|chat| chat.state == "ended"));
}

#[tokio::test]
async fn while_stopped_nothing_starts_and_says_so_until_the_window_re_arms() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&hosting, project.path()).await;
    window.stop_all().await.expect("stopped");

    let refused = window.start_program(a_harness()).await.unwrap_err();

    let refusal = refused.refusal().expect("refused by the host");
    assert_eq!(refusal.code, session::STOPPED);
    assert!(refusal.why.contains("re-armed"), "{}", refusal.why);
    assert!(runs.ran().is_empty(), "nothing ran");

    let armed = window.rearm().await.expect("re-armed");

    assert!(!armed.stopped);
    window.start_program(a_harness()).await.expect("it starts");
    assert_eq!(runs.ran().len(), 1);
}

#[tokio::test]
async fn a_re_arm_starts_again_nothing_that_was_stopped() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&hosting, project.path()).await;
    window.start_program(a_harness()).await.expect("it starts");
    window.stop_all().await.expect("stopped");

    window.rearm().await.expect("re-armed");

    assert_eq!(runs.ran().len(), 1, "nothing was started again");
    assert_eq!(window.list().await.unwrap()[0].state, "ended");
}

#[tokio::test]
async fn a_host_started_again_while_stopped_starts_nothing_until_the_window_re_arms() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let before = a_host_keeping_its_switch_in(folder.path(), &StandIns::default());
    the_window(&before, project.path())
        .await
        .stop_all()
        .await
        .expect("stopped");
    before.end_all();
    drop(before);

    let runs = StandIns::default();
    let after = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&after, project.path()).await;

    assert!(window.kill_switch().await.unwrap().stopped);
    assert_eq!(
        refused_start(&window, a_harness()).await.as_deref(),
        Some(session::STOPPED)
    );
    window.rearm().await.expect("re-armed");
    window.start_program(a_harness()).await.expect("it starts");

    // And a host started after the re-arm is armed.
    let later = a_host_keeping_its_switch_in(folder.path(), &StandIns::default());
    let window = the_window(&later, project.path()).await;
    assert!(!window.kill_switch().await.unwrap().stopped);
}

#[tokio::test]
async fn a_link_that_is_not_the_window_stops_every_program_and_cannot_re_arm() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&hosting, project.path()).await;
    window.start_program(a_harness()).await.expect("it starts");

    for scope in [Scope::Terminal, Scope::FleetMcp, Scope::Approval] {
        let other = a_link(&hosting, scope).await;

        assert!(
            other
                .stop_all()
                .await
                .expect("a stop needs no human")
                .stopped
        );
        let refused = other.rearm().await.unwrap_err();

        let refusal = refused.refusal().expect("refused by the host");
        assert_eq!(refusal.code, session::NOT_ALLOWED, "{scope}");
        assert!(refusal.why.contains("window"), "{scope}: {}", refusal.why);
        assert!(other.kill_switch().await.unwrap().stopped, "{scope}");
    }
    assert!(runs.last().seen().stopped);
    assert!(window.kill_switch().await.unwrap().stopped);
}

#[tokio::test]
async fn a_runner_is_stopped_and_re_armed_over_its_desktops_link() {
    // ADR 0078 §4 and ADR 0071 as amended (V29b, #2051): a runner has no window, so its desktop's
    // host re-arms it over the link, as its own window re-arms the desktop.
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&hosting, project.path()).await;
    window.start_program(a_harness()).await.expect("it starts");
    let desktop = a_link(&hosting, Scope::RemoteLink).await;

    assert!(desktop.stop_all().await.expect("stopped").stopped);
    assert!(runs.last().seen().stopped);
    let armed = desktop.rearm().await.expect("re-armed by its desktop");
    assert!(!armed.stopped);
    assert!(!window.kill_switch().await.unwrap().stopped);
}

#[tokio::test]
async fn while_stopped_the_window_may_open_a_shell_and_no_other_link_may() {
    // ADR 0071, ruled 3: the operator may look at what the agents did. The window decides which
    // shell is the operator's, as it did before chats ran in the host.
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
    let window = the_window(&hosting, project.path()).await;
    window.stop_all().await.expect("stopped");
    let a_shell = || StartProgram::of(PROJECT, ProgramKind::Shell);

    for scope in [Scope::Terminal, Scope::RemoteLink] {
        let other = a_link(&hosting, scope).await;
        assert_eq!(
            refused_start(&other, a_shell()).await.as_deref(),
            Some(session::STOPPED),
            "{scope}"
        );
    }
    assert!(
        runs.ran().is_empty(),
        "a shell opened for a link not the window's"
    );
    window
        .start_program(a_shell())
        .await
        .expect("the operator's shell opens");
    assert_eq!(runs.ran().len(), 1);
}

#[tokio::test]
async fn the_kept_switch_is_this_users_alone() {
    let folder = tempfile::tempdir().unwrap();
    let hosting = a_host_keeping_its_switch_in(folder.path(), &StandIns::default());
    let window = a_link(&hosting, Scope::LocalUi).await;

    window.stop_all().await.expect("stopped");

    let mode = std::fs::metadata(folder.path().join(KILL_SWITCH))
        .expect("the stop is kept")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[tokio::test]
async fn a_kept_switch_that_cannot_be_read_holds_every_agent_stopped_and_says_why() {
    let project = tempfile::tempdir().unwrap();
    let garbled = tempfile::tempdir().unwrap();
    write_private(garbled.path(), b"perhaps\n");
    let readable_by_others = tempfile::tempdir().unwrap();
    write_private(readable_by_others.path(), b"armed\n");
    std::fs::set_permissions(
        readable_by_others.path().join(KILL_SWITCH),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let a_link_in_its_place = tempfile::tempdir().unwrap();
    let elsewhere = a_link_in_its_place.path().join("armed");
    std::fs::write(&elsewhere, "armed\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, a_link_in_its_place.path().join(KILL_SWITCH)).unwrap();

    for folder in [&garbled, &readable_by_others, &a_link_in_its_place] {
        let runs = StandIns::default();
        let hosting = a_host_keeping_its_switch_in(folder.path(), &runs);
        let window = the_window(&hosting, project.path()).await;

        let switch = window.kill_switch().await.unwrap();

        assert!(switch.stopped, "{}", folder.path().display());
        let why = switch.why.expect("it says why");
        assert!(why.contains("could not be read"), "{why}");
        let refused = window.start_program(a_harness()).await.unwrap_err();
        assert!(
            refused.refusal().unwrap().why.contains("could not be read"),
            "a start says why too: {refused}"
        );
        assert!(runs.ran().is_empty());
    }
}

#[tokio::test]
async fn a_host_never_stopped_is_armed() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosting = a_host_keeping_its_switch_in(folder.path(), &StandIns::default());
    let window = the_window(&hosting, project.path()).await;

    let switch = window.kill_switch().await.unwrap();

    assert_eq!(
        switch,
        session::KillSwitch {
            stopped: false,
            ..Default::default()
        }
    );
    tokio::time::timeout(PATIENCE, window.start_program(a_harness()))
        .await
        .unwrap()
        .expect("it starts");
}

fn write_private(folder: &Path, bytes: &[u8]) {
    let path = folder.join(KILL_SWITCH);
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

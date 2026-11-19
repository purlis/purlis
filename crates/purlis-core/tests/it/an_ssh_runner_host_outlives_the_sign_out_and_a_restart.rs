// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's session host kept by systemd outlives an SSH drop and a sign-out, and comes back
//! after a restart, for real** (RR-15, #724; ADR 0068 §3 as amended for #724). Against the remote
//! parity job's systemd runner fixture (`tests/ssh-runner/up-systemd.sh`): an Ubuntu machine that
//! boots systemd, with an sshd whose sign-ins are logind sessions and no purlis.
//!
//! Through the pinned route, as the desktop will:
//!
//! 1. this build's purlis is installed as a version (RR-14) and paired, and
//!    `purlis serve --install` hands its host to the account's systemd user manager: the unit is
//!    enabled and started, and linger is turned on by the account itself;
//! 2. doctor finds the unit installed, linger on and the host answering;
//! 3. two chats run on the host; then every SSH connection of the account is cut (its `sshd`
//!    processes killed, as a dropped link leaves them) and the account is signed out: logind has
//!    no session of it and says it lingers. The unit is still active, both chats' processes still
//!    run, and the host, reached again through its bridge, still lists both;
//! 4. the machine restarts (`docker restart`: systemd shuts down and boots again). The shutdown
//!    stops the unit by `SIGTERM` to the host alone (`KillMode=mixed`), which hears it as a quit
//!    (#6700): it says so in the journal of the boot that ended, ends both chats as a quit and
//!    empties its host record. Nothing is run over SSH to start the host, and it serves again,
//!    with nothing to settle as a crash. Doctor is green again.
//! 5. a newer version is installed beside it, with its own service, and runs a chat. The old
//!    version's host, with nothing to run and no link, stops itself (#6704): it says so in its
//!    journal and exits 0, and systemd does not start it again. The next install removes it,
//!    its unit by name, then its folders. Two more versions are installed, so the one with the
//!    live chat may go too: it has a newer version beside it, but its chat keeps it serving, and
//!    pruning keeps it. Its `serve --remove` takes back the last runner service, and linger with
//!    it.
//!
//! The chats are shells (the chat's harness is not what this checks). A harness chat's run ended
//! as a quit is `purlis-cli`'s `purlis_serve_hears_sigterm_as_a_quit`, which also checks that a
//! host killed outright still leaves its chats for the next to settle as a crash, as
//! `a_host_started_after_one_that_crashed_settles_what_it_left` does in the host's own tests.
//!
//! It needs the fixture: `SSH_RUNNER_SYSTEMD_TEST_ROUTE` (`ssh://keeper@127.0.0.1:2225`),
//! `SSH_RUNNER_SYSTEMD_TEST_MACHINE` (the container, which the test restarts and looks into as
//! its root) and `SSH_RUNNER_TEST_PURLIS` (this build's `purlis`). Without them it runs nothing
//! and passes: a sandboxed agent can't run it.

#![cfg(unix)]

use std::time::{Duration, Instant};

use crate::support::outage::{
    Kept, QUIET_LIMIT, START_LIMIT, first_down_after, first_up_after, told_within,
};
use crate::support::remote_rig::{
    PATIENCE, PROJECT, as_root, chat, linked, running, runtime, there, until_within, verified,
};
use purlis_core::runnerinstall::{self, Delivery};
use purlis_core::sshroute::{self, Reach, Route, connect};
use purlis_session_protocol::hostlink::{Identity, LinkKey, Pinned};

const VERSION: &str = "0.0.1-rr15.1";
/// The versions installed beside it, newest last (#6704).
const NEWER: [&str; 3] = ["0.0.1-rr15.2", "0.0.1-rr15.3", "0.0.1-rr15.4"];
/// How long an old version's host may take to stop itself: its minute idle, its looks at it, and
/// a slow machine.
const STOPS_ITSELF_WITHIN: Duration = Duration::from_secs(180);
const CHATS: usize = 2;
/// The version's host record, wherever its machine store is (`purlisd`, or `charterd` before
/// rename-local), printed and measured from outside.
const RECORD_CAT: &str = "find /home/keeper/.config -path '*/server/*/hosts.jsonl' -exec cat {} +";
const RECORD_SIZE: &str =
    "find /home/keeper/.config -path '*/server/*/hosts.jsonl' -exec wc -c {} + | awk '{print $1}'";

struct Fixture {
    reach: Reach,
    machine: String,
    binary: Vec<u8>,
    _settings: tempfile::TempDir,
}

/// The fixture, or a SKIP line on a laptop and a failure where `PURLIS_PARITY` is set (#7188).
fn fixture() -> Option<Fixture> {
    crate::support::real_infra::fixture_or_skip(
        "a machine that boots systemd, behind an sshd",
        "tests/ssh-runner/up-systemd.sh",
        fixture_set_by_up(),
    )
}

fn fixture_set_by_up() -> Option<Fixture> {
    let route =
        Route::parse(&std::env::var("SSH_RUNNER_SYSTEMD_TEST_ROUTE").ok()?).expect("a route");
    let machine = std::env::var("SSH_RUNNER_SYSTEMD_TEST_MACHINE").ok()?;
    let binary = std::fs::read(std::env::var("SSH_RUNNER_TEST_PURLIS").ok()?).expect("purlis");
    let settings = tempfile::Builder::new()
        .prefix("rr")
        .tempdir_in("/tmp")
        .expect("a folder");
    let ssh = sshroute::ssh_here().expect("the system's ssh");
    let key = connect::meet(
        &ssh,
        &route,
        &connect::resolve(&ssh, &route).expect("resolved"),
        settings.path(),
    )
    .expect("the machine is met");
    Some(Fixture {
        reach: Reach {
            ssh,
            route,
            pool: "rr15".to_owned(),
            key,
            settings_in: settings.path().to_path_buf(),
        },
        machine,
        binary,
        _settings: settings,
    })
}

/// Waits until `check` holds, up to the test's patience, saying `what` where it never does.
fn until(what: &str, check: impl FnMut() -> bool) {
    until_within(PATIENCE, what, check);
}

/// Doctor's rows on the runner, by name: each `(status, detail)`.
fn doctor(reach: &Reach, binary: &str) -> std::collections::HashMap<String, (String, String)> {
    let ran = there(reach, &format!("{binary} doctor --json"), None);
    let rows: Vec<serde_json::Value> = serde_json::from_str(&ran.printed)
        .unwrap_or_else(|why| panic!("doctor's rows read ({why}): {}\n{}", ran.printed, ran.said));
    rows.into_iter()
        .map(|row| {
            let field = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
            (field("name"), (field("status"), field("detail")))
        })
        .collect()
}

fn assert_doctor_green(reach: &Reach, binary: &str) {
    let rows = doctor(reach, binary);
    for name in [
        purlis_core::runnerhost::SERVICE_ROW,
        purlis_core::runnerhost::LINGER_ROW,
        purlis_core::runnerhost::REACHABLE_ROW,
    ] {
        let (status, detail) = rows
            .get(name)
            .unwrap_or_else(|| panic!("doctor has no {name} row: {rows:?}"));
        assert_eq!(status, "ok", "{name}: {detail}");
    }
}

/// The account's chats' processes on the machine, counted from outside it.
fn chat_processes(machine: &str) -> usize {
    let (_, out) = as_root(machine, "pgrep -u keeper -fc '^sleep 6000$' || true");
    out.trim().parse().unwrap_or(0)
}

#[test]
fn a_runners_host_outlives_the_sign_out_and_comes_back_after_a_restart() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else {
        // No systemd runner fixture is named: nothing is run (the module's docs say where).
        return;
    };
    let reach = &fixture.reach;
    let machine = fixture.machine.as_str();

    // 1. Installed as a version, paired, and its host handed to systemd.
    let probed = runnerinstall::probe(reach).expect("the runner is probed");
    let binary = runnerinstall::install(
        reach,
        &probed,
        &verified(&fixture.binary, VERSION, probed.target),
        Delivery::Upload(&fixture.binary),
    )
    .expect("installed")
    .binary()
    .to_owned();
    let desktop_key = LinkKey::generate().unwrap();
    let ours = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: desktop_key.public(),
    };
    let line = format!("{}\n", purlis_core::runnerlink::to_line(&ours));
    let ran = there(reach, &format!("{binary} bridge --pair"), Some(&line));
    assert_eq!(ran.code, Some(0), "paired: {}", ran.said);
    let runner = purlis_core::runnerlink::from_line(&ran.printed).expect("the runner's key");
    let identity = || Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::from_private(desktop_key.private_half().unwrap()).unwrap(),
    };

    let installed = there(reach, &format!("{binary} serve --install"), None);
    assert_eq!(installed.code, Some(0), "installed: {}", installed.said);
    assert!(
        installed.printed.contains("systemd starts it again"),
        "{}",
        installed.printed
    );
    assert!(
        installed
            .printed
            .contains("purlis turned on linger for keeper, so your chats keep running after you sign out (undo: loginctl disable-linger keeper)."),
        "linger turned on by the account itself: {}",
        installed.printed
    );
    let unit = purlis_core::runnerhost::Service {
        binary: binary.clone().into(),
        version: Some(VERSION.to_owned()),
        config_home: None,
    }
    .unit_name();
    let (lingers, said) = as_root(machine, "ls /var/lib/systemd/linger");
    assert!(lingers && said.contains("keeper"), "linger is on: {said}");

    // 2. Doctor: the unit installed, linger on, the host answering.
    until("the host answers", || {
        there(
            reach,
            &format!("systemctl --user is-active --quiet {unit}"),
            None,
        )
        .code
            == Some(0)
    });
    assert_doctor_green(reach, &binary);

    // 3. Two chats; then every connection cut and the account signed out.
    let rt = runtime();
    let chats = rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &binary, identity(), runner.clone()).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the host holds the project");
        let mut chats = Vec::new();
        for n in 0..CHATS {
            chats.push(chat(&client, &format!("chat {n}")).await);
        }
        chats.sort();
        assert_eq!(running(&client).await, chats);
        kept.abort();
        chats
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    // A dropped link: each of the account's SSH connections ends without a word.
    let _ = as_root(machine, "pkill -KILL -u keeper '^sshd' || true");
    // logind's `lingering`: no session of the account's own is left, and its manager runs on.
    // (`list-sessions` also lists the manager's own session, of class `manager`, so it is not
    // what says the account signed out.)
    let deadline = Instant::now() + PATIENCE;
    loop {
        let (_, state) = as_root(
            machine,
            "loginctl show-user keeper --property=State --value",
        );
        if state.trim() == "lingering" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never signed out with its manager lingering: logind says {:?}",
            state.trim()
        );
        std::thread::sleep(Duration::from_secs(1));
    }
    let (_, sessions) = as_root(
        machine,
        "loginctl list-sessions --no-legend | grep keeper | grep -v manager || true",
    );
    assert_eq!(
        sessions.trim(),
        "",
        "no session of the account's own is left"
    );
    let (active, said) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-active {unit}"),
    );
    assert!(
        active,
        "the host is still running after the sign-out: {said}"
    );
    assert_eq!(
        chat_processes(machine),
        CHATS,
        "both chats still run after the sign-out"
    );
    let rt = runtime();
    rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &binary, identity(), runner.clone()).await;
        assert_eq!(
            running(&client).await,
            chats,
            "the host still lists both chats, reached again"
        );
        kept.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    let (_, record) = as_root(machine, RECORD_CAT);
    assert_eq!(
        record.matches("\"started\"").count(),
        CHATS,
        "the host noted both chats: {record}"
    );

    // 4. The machine restarts: nothing is run over SSH to start the host. A link is kept through
    // it, and the outage rules of the fault suite's ruling (#7188, case 5) are checked on it: the
    // person is told within 30 s of the restart, with a reason, and the link is back inside the
    // 3-minute limit once the machine is.
    let watching_rt = runtime();
    let began = Instant::now();
    let watching = watching_rt.block_on(Kept::start(
        reach,
        &binary,
        identity(),
        runner.clone(),
        began,
        PATIENCE,
    ));
    let fault_at = watching.now();
    let restarted = purlis_core::forklock::output(
        std::process::Command::new("docker").args(["restart", "--time", "60", machine]),
    )
    .expect("docker runs");
    assert!(restarted.status.success(), "the machine restarted");
    let returned_at = watching.now();
    until("the machine is up again", || {
        as_root(
            machine,
            "systemctl is-system-running --wait >/dev/null 2>&1; true",
        )
        .0 && as_root(
            machine,
            &format!("systemctl --user --machine=keeper@ is-active --quiet {unit}"),
        )
        .0
    });
    watching_rt.block_on(async {
        let told = watching.told();
        // Nothing silent: said why within 30 s of the restart (it is measured from when the
        // restart began, and `docker restart` itself takes up to its 60 s grace to return).
        told_within(&told, fault_at, QUIET_LIMIT);
        // The link is "back" only after the down it told, not a redial while the machine was
        // still shutting down.
        let (after_fault, _) = first_down_after(&told, fault_at).expect("told");
        let down_at = fault_at + after_fault;
        // And back inside the 3-minute limit of the machine being back.
        watching
            .until_working(START_LIMIT, "the link after the reboot")
            .await;
        let told = watching.told();
        let up = first_up_after(&told, down_at).expect("the link came back");
        assert!(
            up <= returned_at.saturating_sub(down_at) + START_LIMIT,
            "the link was back after {up:?}, past the limit from the machine's return: {told:?}"
        );
        watching.task.abort();
    });
    watching_rt.shutdown_timeout(Duration::from_secs(5));
    // The boot that ended: the shutdown's stop was heard as a quit (#6700). Every journal on
    // the disk, merged (`-m`): a container's machine id may be made new at each boot, which
    // files the boot that ended under another machine's folder, where `-b -1` does not look.
    // The host was stopped only once before now, by the restart.
    let (_, stopped) = as_root(
        machine,
        &format!("journalctl -m --no-pager _SYSTEMD_USER_UNIT={unit}"),
    );
    if !stopped.contains(purlis_host::TOLD_TO_STOP) {
        let (_, kept) = as_root(
            machine,
            "cat /etc/machine-id; ls -R /var/log/journal; journalctl -m --list-boots --no-pager",
        );
        panic!("the shutdown's SIGTERM was heard as a quit: {stopped}\n{kept}");
    }
    // This boot: the host serves again, and finds nothing left to settle as a crash.
    let (_, journal) = as_root(
        machine,
        &format!("journalctl -b --no-pager _SYSTEMD_USER_UNIT={unit}"),
    );
    assert!(
        !journal.contains("each is settled as ended by its crash"),
        "a planned restart is not kept as a crash: {journal}"
    );
    let (_, record) = as_root(machine, RECORD_SIZE);
    assert_eq!(
        record.trim(),
        "0",
        "the record was emptied at the quit: {journal}"
    );
    assert_eq!(
        chat_processes(machine),
        0,
        "the chats ended with the machine"
    );
    let rt = runtime();
    rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &binary, identity(), runner.clone()).await;
        assert!(
            running(&client).await.is_empty(),
            "nothing of the last boot runs, and the host serves"
        );
        kept.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    assert_doctor_green(reach, &binary);

    // 5. A newer version beside it, kept by a service of its own, runs a chat (#6704).
    let unit_of = |version: &str| {
        purlis_core::runnerhost::Service {
            binary: std::path::PathBuf::new(),
            version: Some(version.to_owned()),
            config_home: None,
        }
        .unit_name()
    };
    let install = |version: &str| {
        let probed = runnerinstall::probe(reach).expect("the runner is probed");
        runnerinstall::install(
            reach,
            &probed,
            &verified(&fixture.binary, version, probed.target),
            Delivery::Upload(&fixture.binary),
        )
        .expect("installed")
    };
    let [second, third, fourth] = NEWER;
    let installed = install(second);
    assert!(installed.pruned().is_empty(), "{installed:?}");
    let newer = installed.binary().to_owned();
    let ran = there(reach, &format!("{newer} serve --install"), None);
    assert_eq!(ran.code, Some(0), "the newer one installed: {}", ran.said);
    let newer_unit = unit_of(second);
    until("the newer version's host answers", || {
        as_root(
            machine,
            &format!("systemctl --user --machine=keeper@ is-active --quiet {newer_unit}"),
        )
        .0
    });
    let rt = runtime();
    let on_newer = rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &newer, identity(), runner.clone()).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the newer host holds the project");
        let started = chat(&client, "on the newer version").await;
        assert_eq!(running(&client).await, std::slice::from_ref(&started));
        kept.abort();
        started
    });
    rt.shutdown_timeout(Duration::from_secs(5));

    // The old version's host, with nothing to run and nobody linked, stops itself and exits 0,
    // which systemd does not restart.
    until_within(
        STOPS_ITSELF_WITHIN,
        "the old version's host stops itself",
        || {
            !as_root(
                machine,
                &format!("systemctl --user --machine=keeper@ is-active --quiet {unit}"),
            )
            .0
        },
    );
    let (_, journal) = as_root(
        machine,
        &format!("journalctl -b --no-pager _SYSTEMD_USER_UNIT={unit}"),
    );
    assert!(
        journal.contains(purlis_host::SUPERSEDED),
        "it says why it stopped: {journal}"
    );
    // Longer than the unit's `RestartSec`: a clean exit is not restarted.
    std::thread::sleep(Duration::from_secs(12));
    let (_, state) = as_root(
        machine,
        &format!(
            "systemctl --user --machine=keeper@ show --property=ActiveState,Result,NRestarts {unit}"
        ),
    );
    for line in ["ActiveState=inactive", "Result=success", "NRestarts=0"] {
        assert!(
            state.lines().any(|said| said.trim() == line),
            "{line}: {state}"
        );
    }
    assert_eq!(chat_processes(machine), 1, "the newer version's chat runs");

    // The next install removes it: its unit by name, then its folders.
    let installed = install(third);
    assert_eq!(installed.pruned(), [VERSION], "{installed:?}");
    let (file_there, _) = as_root(
        machine,
        &format!("test -e /home/keeper/.config/systemd/user/{unit}"),
    );
    assert!(!file_there, "the old version's unit file is gone");
    let (enabled, said) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-enabled {unit}"),
    );
    assert!(!enabled, "the old version's unit is not enabled: {said}");
    assert_eq!(
        runnerinstall::probe(reach).unwrap().versions,
        [second, third],
        "the old version's folder is gone"
    );

    // One more, so the version running a chat may go too: it has newer ones beside it, but its
    // chat keeps its host serving, past the time an idle one takes to stop, and prune keeps it.
    let installed = install(fourth);
    assert!(
        installed.pruned().is_empty(),
        "a version with a live chat stays: {installed:?}"
    );
    std::thread::sleep(purlis_host::SUPERSEDED_IDLE_FOR + Duration::from_secs(30));
    let (active, said) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-active {newer_unit}"),
    );
    assert!(
        active,
        "the host with a live chat serves on beside newer versions: {said}"
    );
    let installed = install(fourth);
    assert!(
        installed.pruned().is_empty(),
        "still a live chat, still kept: {installed:?}"
    );
    assert_eq!(
        runnerinstall::probe(reach).unwrap().versions,
        [second, third, fourth]
    );
    let rt = runtime();
    rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &newer, identity(), runner.clone()).await;
        assert_eq!(
            running(&client).await,
            std::slice::from_ref(&on_newer),
            "its chat still runs"
        );
        kept.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));

    // Taken back: it stops and its unit goes, the last runner service, and linger with it.
    let binary = newer;
    let unit = newer_unit;
    let removed = there(reach, &format!("{binary} serve --remove"), None);
    assert_eq!(removed.code, Some(0), "removed: {}", removed.said);
    let (still, _) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-active --quiet {unit}"),
    );
    assert!(!still, "the host stopped with its unit");
    assert!(
        removed
            .printed
            .contains("purlis turned linger off again for keeper"),
        "{}",
        removed.printed
    );
    let (file_there, _) = as_root(
        machine,
        &format!("test -e /home/keeper/.config/systemd/user/{unit}"),
    );
    assert!(!file_there, "the unit's file is gone");
    let (enabled, said) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-enabled {unit}"),
    );
    assert!(!enabled, "the unit is not enabled: {said}");
    let (lingers, _) = as_root(machine, "test -e /var/lib/systemd/linger/keeper");
    assert!(
        !lingers,
        "linger purlis turned on is off again with the last service"
    );
}

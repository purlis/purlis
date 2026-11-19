// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The faults a live remote chat and a live browser must survive before remote is trusted**
//! (#7188, the ruling of 2026-11-18, part of #7180): fifteen cases, one test each, against the
//! remote parity job's rigs with the fake harness and nothing else faked (ruling 5). Fourteen are
//! here; case 5 is the systemd runner's own test, as the ruling says.
//!
//! **The cases.** Pool and chat: 1 link drop of 5 s and of 90 s; 2 sshd restart; 3 host key
//! changed under a live chat; 4 runner killed; 5 machine reboot (in
//! `an_ssh_runner_host_outlives_the_sign_out_and_a_restart`); 6 app restart; 7 pool forgotten;
//! 8 a 15 s slow link; 9 registry down; 10 half-open link. Browser: B1 link drop; B2 takeover
//! during a drop; B3 pool forgotten; B4 app quit; B5 two runners at once. Each test's name says
//! what holds.
//!
//! **The outage rules, checked in every case** ("behaviour during an outage and the end
//! states"): nothing silent for more than 30 s (the link says why it is down within
//! [`QUIET_LIMIT`] of the fault, [`told_within`]); nothing started waits past three minutes
//! ([`START_LIMIT`]); and where a state has a name, it is the name. **Each of the five is asserted
//! by one case, and each is train A's, so each of those cases is `#[ignore]`d** and reads the
//! state through [`seam::state_named`]: Unreachable by the 90 s drop
//! (`a_90_second_drop_is_named_unreachable_while_it_lasts`), Refused by the registry down
//! (`a_registry_that_is_down_names_the_pool_refused`), Host key changed by case 3, Runner gone by
//! case 4, Pool forgotten by case 7.
//!
//! **Faults** are `tests/remote-faults/faults.sh`'s, put on the rig's containers and taken off by
//! `restore`, which every test runs before it starts and again as it ends, however it ends: a
//! test never starts on a fault the one before left. **No retries**: the job runs them with
//! `--retries 0` and one at a time, and a flake is a lifecycle bug (ruling 6; a quarantine needs
//! a linked issue and expires in 7 days).
//!
//! **What is not on `main` yet.** The named states and what the window does with them (`Refused`,
//! `Pool forgotten`, `Runner gone`, `Try now`) are train A's (#7187), the browsers' are train D's
//! (#7190). A case that needs them is `#[ignore]`d with that reason and reads the state through
//! [`seam`], whose functions say what to put there when the train merges; the rest of the case,
//! the fault, the clock and the rules, is real now. **Those ignores are lifted when the trains
//! merge** (`cargo test -p purlis-core --test it -- --ignored a_live_remote_chat`).
//!
//! It needs the systemd runner fixture, `SSH_RUNNER_SYSTEMD_TEST_ROUTE`,
//! `SSH_RUNNER_SYSTEMD_TEST_MACHINE` and `SSH_RUNNER_TEST_PURLIS` (`tests/ssh-runner/up-systemd.sh`),
//! and, for the cases on a Docker pool, `tests/remote-docker/up.sh`'s. Without them it prints a
//! SKIP line and passes on a laptop; where `PURLIS_PARITY` is set, it fails (ruling 4).

#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::runnerinstall::{self, Delivery};
use purlis_core::sshroute::{self, Reach, Route, connect};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::session::Client;

use crate::support::outage::{
    Kept, QUIET_LIMIT, START_LIMIT, Told, first_down_after, first_up_after, told_within,
};
use crate::support::real_infra::fixture_or_skip;
use crate::support::remote_rig::{
    PATIENCE, PROJECT, as_root, bridge_args, running, runtime, shell_launch, there, until_within,
    verified,
};

const VERSION: &str = "0.0.1-faults.1";
const DESKTOP: &str = "01JA0DESKT0P00000000000000";
/// The script that puts and takes the faults.
const FAULTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/remote-faults/faults.sh"
);

// ---------------------------------------------------------------------------------------------
// The rig.
// ---------------------------------------------------------------------------------------------

/// The systemd runner, installed, paired and its host handed to systemd, with every fault taken
/// off: what each case starts from, and, dropped, what it leaves behind it.
struct Rig {
    reach: Reach,
    machine: String,
    binary: String,
    desktop_key: LinkKey,
    runner: Pinned,
    unit: String,
    began: Instant,
    _settings: tempfile::TempDir,
}

/// Runs `faults.sh --machine <machine> <args>`; a verb that fails fails the test.
fn fault(machine: &str, args: &[&str]) {
    let out = purlis_core::forklock::output(
        std::process::Command::new("bash")
            .arg(FAULTS)
            .args(["--machine", machine])
            .args(args),
    )
    .expect("bash runs");
    assert!(
        out.status.success(),
        "faults.sh {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

impl Rig {
    /// The rig, or `None` after a SKIP line (and a failure where `PURLIS_PARITY` is set).
    fn up() -> Option<Rig> {
        let found = (|| {
            let route = Route::parse(&std::env::var("SSH_RUNNER_SYSTEMD_TEST_ROUTE").ok()?)
                .expect("a route");
            let machine = std::env::var("SSH_RUNNER_SYSTEMD_TEST_MACHINE").ok()?;
            let binary =
                std::fs::read(std::env::var("SSH_RUNNER_TEST_PURLIS").ok()?).expect("purlis");
            Some((route, machine, binary))
        })();
        let (route, machine, bytes) = fixture_or_skip(
            "a live chat on a systemd runner behind an sshd",
            "tests/ssh-runner/up-systemd.sh",
            found,
        )?;
        // Whatever the case before left, off: the link, the key and the rule are the rig's own.
        fault(&machine, &["restore"]);
        // In /tmp, as the sibling tests do: a short path, since ssh's control sockets live under
        // this folder and a unix socket's path is limited to about 104 bytes.
        let settings = tempfile::Builder::new()
            .prefix("rf")
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
        let reach = Reach {
            ssh,
            route,
            pool: "faults".to_owned(),
            key,
            settings_in: settings.path().to_path_buf(),
        };
        let probed = runnerinstall::probe(&reach).expect("the runner is probed");
        let binary = runnerinstall::install(
            &reach,
            &probed,
            &verified(&bytes, VERSION, probed.target),
            Delivery::Upload(&bytes),
        )
        .expect("installed")
        .binary()
        .to_owned();
        let desktop_key = LinkKey::generate().unwrap();
        let ours = Pinned {
            device: DESKTOP.into(),
            key: desktop_key.public(),
        };
        let line = format!("{}\n", purlis_core::runnerlink::to_line(&ours));
        let ran = there(&reach, &format!("{binary} bridge --pair"), Some(&line));
        assert_eq!(ran.code, Some(0), "paired: {}", ran.said);
        let runner = purlis_core::runnerlink::from_line(&ran.printed).expect("the runner's key");
        let installed = there(&reach, &format!("{binary} serve --install"), None);
        assert_eq!(installed.code, Some(0), "installed: {}", installed.said);
        let unit = purlis_core::runnerhost::Service {
            binary: binary.clone().into(),
            version: Some(VERSION.to_owned()),
            config_home: None,
        }
        .unit_name();
        let rig = Rig {
            reach,
            machine,
            binary,
            desktop_key,
            runner,
            unit,
            began: Instant::now(),
            _settings: settings,
        };
        // The chats an earlier case left are not this one's.
        let _ = as_root(&rig.machine, "pkill -KILL -u keeper -f 'typed-' || true");
        rig.until_the_host_answers();
        Some(rig)
    }

    fn identity(&self) -> Identity {
        Identity {
            device: DESKTOP.into(),
            key: LinkKey::from_private(self.desktop_key.private_half().unwrap()).unwrap(),
        }
    }

    fn until_the_host_answers(&self) {
        until_within(PATIENCE, "the host answers", || {
            there(
                &self.reach,
                &format!("systemctl --user is-active --quiet {}", self.unit),
                None,
            )
            .code
                == Some(0)
        });
    }

    /// Starts keeping a link to this runner's host, as the window does.
    async fn keep(&self) -> Kept {
        self.keep_within(PATIENCE).await
    }

    /// As [`Rig::keep`], waiting up to `within` for the first link: over a slow link it takes longer.
    async fn keep_within(&self, within: Duration) -> Kept {
        Kept::start(
            &self.reach,
            &self.binary,
            self.identity(),
            self.runner.clone(),
            self.began,
            within,
        )
        .await
    }

    /// Puts fault `args` on the machine; the clock of the outage starts here.
    fn fault(&self, args: &[&str]) -> Duration {
        fault(&self.machine, args);
        self.began.elapsed()
    }

    /// Takes every fault off; the clock of the restore starts here.
    fn restore(&self) -> Duration {
        fault(&self.machine, &["restore"]);
        self.began.elapsed()
    }

    /// The chat's typed input, as the machine holds it, read from outside.
    fn typed(&self, tag: &str) -> String {
        as_root(
            &self.machine,
            &format!("cat /home/keeper/typed-{tag} 2>/dev/null || true"),
        )
        .1
    }
}

impl Drop for Rig {
    /// Every fault off, however the case ended, so the next starts clean.
    fn drop(&mut self) {
        let _ =
            purlis_core::forklock::output(std::process::Command::new("bash").arg(FAULTS).args([
                "--machine",
                &self.machine,
                "restore",
            ]));
    }
}

/// A chat that keeps what it is typed in a file of the account's, so the machine, not the link,
/// says whether input arrived.
async fn typing_chat(client: &Client, tag: &str) -> String {
    use purlis_session_protocol::session::{ProgramKind, StartProgram};
    client
        .start_program(StartProgram {
            launch: Some(shell_launch(format!(
                "echo 'said {tag}'; exec cat >> /home/keeper/typed-{tag}"
            ))),
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        })
        .await
        .expect("the chat starts on the runner")
}

/// Input the chat takes: written on `client`, and on the machine within `PATIENCE`.
async fn types_into(rig: &Rig, client: &Client, chat: &str, tag: &str, word: &str) {
    client
        .write(chat, format!("{word}\n").as_bytes())
        .await
        .expect("the input is written");
    let deadline = Instant::now() + PATIENCE;
    while !rig.typed(tag).contains(word) {
        assert!(
            Instant::now() < deadline,
            "the chat never took its input {word:?}: it has {:?}",
            rig.typed(tag)
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Starts the host's side: the project held and one typing chat, returning its id.
async fn a_chat(rig: &Rig, client: &Client, tag: &str) -> String {
    client
        .hold_project(PROJECT, "")
        .await
        .expect("the host holds the project");
    let chat = typing_chat(client, tag).await;
    types_into(rig, client, &chat, tag, "before").await;
    chat
}

// ---------------------------------------------------------------------------------------------
// What is train A's and train D's: read through here, so the rest of a case is real now.
// ---------------------------------------------------------------------------------------------

/// **Reads of what is not on `main`.** Each function says what to put there when its train
/// merges; until then a case that calls one is `#[ignore]`d with the train's name.
mod seam {
    use super::Rig;

    /// The name of the state the pool or runner is in, as the window words it: `Unreachable`,
    /// `Refused`, `Host key changed`, `Runner gone` or `Pool forgotten` (#7187, CONTEXT.md).
    ///
    /// Train A: read `purlis_core::sshroute::connect::RemoteState` of the link's `Down` (its
    /// `host_key_changed`, `gone` and `refused` fields) and word it as `RunnerLinkDownNotice` does.
    pub fn state_named(_rig: &Rig, _chat: &str) -> String {
        todo!("needs #7187 train A: the pool or runner's RemoteState, as the window names it")
    }

    /// The name of the state a pool is in, as the window words it, for a pool with no chat of its
    /// own: `Refused` where its image cannot be fetched or its sign-in is turned away.
    ///
    /// Train A: as [`state_named`], read from the pool's `RemoteState`.
    pub fn pool_state_named(_pool: &str) -> String {
        todo!("needs #7187 train A: the pool's RemoteState, as the window names it")
    }

    /// A second runner on another machine of the rig, with its own host, for the cases that need
    /// two. The rig has one (`tests/ssh-runner/up-systemd.sh`'s); a second machine there is the
    /// rig's follow-up.
    pub fn a_second_runner(_rig: &Rig) -> Rig {
        todo!("needs a second runner machine in the rig (up-systemd.sh) and #7190 train D")
    }

    /// The pool forgotten on this machine, as Forget does, and what it lists as left there.
    ///
    /// Train A: `Forget works in every state and lists what may be left`; call it on the pool the
    /// chat runs on and return the listed leftovers.
    pub fn forget_the_pool(_rig: &Rig) -> Vec<String> {
        todo!(
            "needs #7187 train A: Forget on the pool, with the list of what is left on the machine"
        )
    }

    /// A browser opened on the runner, by the chat that owns it, and its state as the view says it.
    ///
    /// Train D: the runner browser of `purlis_core::browser` on a pool (#7190).
    pub fn open_a_browser(_rig: &Rig, _chat: &str) -> String {
        todo!("needs #7190 train D: a browser on a pool's runner")
    }

    /// The browser's state: `Open`, a reason it is down, or `Pool forgotten`.
    pub fn browser_state(_rig: &Rig, _browser: &str) -> String {
        todo!("needs #7190 train D: the browser's view state")
    }

    /// The person takes over the browser; whether control was kept, or the reason it was not.
    pub fn take_over(_rig: &Rig, _browser: &str) -> Result<(), String> {
        todo!("needs #7190 train D: takeover of a runner's browser")
    }

    /// Whether the browser's page is the page it was (its URL and title).
    pub fn page_of(_rig: &Rig, _browser: &str) -> String {
        todo!("needs #7190 train D: the page a runner browser shows")
    }
}

// ---------------------------------------------------------------------------------------------
// The pool and chat cases.
// ---------------------------------------------------------------------------------------------

/// Case 1, for a link down `for_`: the chat keeps what it said and takes input after the link is
/// back, and the link said why within 30 s.
fn a_link_drop_keeps_the_chat(for_: Duration, tag: &str) {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, tag).await;
        let fault_at = rig.fault(&["drop"]);
        tokio::time::sleep(for_).await;
        let back_at = rig.restore();
        let after = kept
            .until_working(START_LIMIT, "the link after the drop")
            .await;
        let told = kept.told();
        // The rules: said why within 30 s (a 5 s drop is found by the kept link's next try or the
        // ssh keepalive; the reason is in what it told), and back inside the limit.
        if for_ >= QUIET_LIMIT {
            told_within(&told, fault_at, QUIET_LIMIT);
        } else {
            // A drop shorter than the link's own deadline may heal before it is noticed; if it
            // was noticed, it said why.
            if let Some((_, why)) = first_down_after(&told, fault_at) {
                assert!(!why.trim().is_empty(), "down with no reason");
            }
        }
        // A link that was told down came back, inside the limit; one that healed unnoticed never
        // went.
        if first_down_after(&told, fault_at).is_some() {
            let back = first_up_after(&told, back_at).expect("the link came back");
            assert!(back <= START_LIMIT, "back after {back:?}");
        }
        // The chat is the same chat, still running, and takes input.
        assert_eq!(running(&after).await, vec![chat.clone()]);
        types_into(&rig, &after, &chat, tag, "after").await;
        let typed = rig.typed(tag);
        assert!(
            typed.contains("before") && typed.contains("after"),
            "the chat kept what it was typed: {typed:?}"
        );
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
fn a_link_drop_of_5_seconds_keeps_the_chat_and_it_takes_input_after_the_restore() {
    a_link_drop_keeps_the_chat(Duration::from_secs(5), "drop5");
}

#[test]
fn a_link_drop_of_90_seconds_says_why_within_30_seconds_and_the_chat_takes_input_after_the_restore()
{
    a_link_drop_keeps_the_chat(Duration::from_secs(90), "drop90");
}

/// Case 1's 90 s drop, the state it is named while it lasts: Unreachable (train A, #7187).
#[test]
#[ignore = "needs #7187 train A: the pool's state named Unreachable"]
fn a_90_second_drop_is_named_unreachable_while_it_lasts() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "unreachable").await;
        let fault_at = rig.fault(&["drop"]);
        // Past the 30 s rule and well inside the drop: the window says what the state is.
        tokio::time::sleep(QUIET_LIMIT + Duration::from_secs(5)).await;
        told_within(&kept.told(), fault_at, QUIET_LIMIT);
        assert_eq!(seam::state_named(&rig, &chat), "Unreachable");
        rig.restore();
        let after = kept
            .until_working(START_LIMIT, "the link after the drop")
            .await;
        assert_eq!(running(&after).await, vec![chat]);
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
fn an_sshd_restart_reopens_the_link_with_no_new_key_prompt_and_the_chat_survives() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "sshd").await;
        let key_before = rig.reach.key.clone();
        let fault_at = rig.fault(&["restart-sshd"]);
        kept.until_down(fault_at, QUIET_LIMIT).await;
        told_within(&kept.told(), fault_at, QUIET_LIMIT);
        let after = kept
            .until_working(START_LIMIT, "the link after sshd restarted")
            .await;
        // No new key prompt: the same pinned key still meets, and nothing says the key changed.
        let told = kept.told();
        assert!(
            told.iter().all(|(_, what)| !matches!(
                what,
                Told::Down {
                    key_changed: true,
                    ..
                }
            )),
            "a restart is not a changed key: {told:?}"
        );
        let key_now = connect::meet(
            &rig.reach.ssh,
            &rig.reach.route,
            &connect::resolve(&rig.reach.ssh, &rig.reach.route).expect("resolved"),
            &rig.reach.settings_in,
        )
        .expect("met again");
        assert_eq!(key_now, key_before, "the machine's key is the one pinned");
        assert_eq!(running(&after).await, vec![chat.clone()]);
        types_into(&rig, &after, &chat, "sshd", "after").await;
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7187 train A: Host key changed is its own state, never retried, and re-trust ends it"]
fn a_host_key_changed_under_a_live_chat_is_not_retried_and_the_chat_comes_back_after_re_trust() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "hostkey").await;
        let fault_at = rig.fault(&["rotate-hostkey"]);
        kept.until_down(fault_at, QUIET_LIMIT).await;
        told_within(&kept.told(), fault_at, QUIET_LIMIT);
        // No automatic retry: for as long again as the longest pause between tries, no link.
        let settled = kept.now();
        tokio::time::sleep(Duration::from_secs(45)).await;
        assert_eq!(
            first_up_after(&kept.told(), settled),
            None,
            "a changed key is never retried on its own"
        );
        assert_eq!(seam::state_named(&rig, &chat), "Host key changed");
        // Re-trust: the new key is met and pinned, and a link on it finds the chat.
        let key = connect::meet(
            &rig.reach.ssh,
            &rig.reach.route,
            &connect::resolve(&rig.reach.ssh, &rig.reach.route).expect("resolved"),
            &rig.reach.settings_in,
        )
        .expect("the new key is met");
        let trusted = Rig {
            reach: Reach {
                key,
                ssh: rig.reach.ssh.clone(),
                route: rig.reach.route.clone(),
                pool: rig.reach.pool.clone(),
                settings_in: rig.reach.settings_in.clone(),
            },
            machine: rig.machine.clone(),
            binary: rig.binary.clone(),
            desktop_key: LinkKey::from_private(rig.desktop_key.private_half().unwrap()).unwrap(),
            runner: rig.runner.clone(),
            unit: rig.unit.clone(),
            began: rig.began,
            _settings: tempfile::tempdir().unwrap(),
        };
        let again = trusted.keep().await;
        let client = again.client().expect("linked after re-trust");
        assert_eq!(running(&client).await, vec![chat]);
        again.task.abort();
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7187 train A: Runner gone, and a start on a dead host held to three minutes"]
fn a_killed_runner_ends_its_chat_as_runner_gone_never_reconnecting_for_ever_and_the_next_chat_starts_a_new_host()
 {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "killed").await;
        let fault_at = rig.fault(&["kill-runner"]);
        kept.until_down(fault_at, QUIET_LIMIT).await;
        told_within(&kept.told(), fault_at, QUIET_LIMIT);
        // Not "Reconnecting…" for ever: inside the limit the chat is ended, by name.
        let deadline = Instant::now() + START_LIMIT;
        let after = kept
            .until_working(START_LIMIT, "the host started again")
            .await;
        loop {
            if !running(&after).await.contains(&chat) {
                break;
            }
            assert!(Instant::now() < deadline, "the chat was never ended");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        assert_eq!(seam::state_named(&rig, &chat), "Runner gone");
        // The next chat starts a new host's chat, inside the limit.
        let began = Instant::now();
        let next = typing_chat(&after, "killed-next").await;
        assert!(
            began.elapsed() <= START_LIMIT,
            "the next start took {:?}",
            began.elapsed()
        );
        assert_eq!(running(&after).await, vec![next]);
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

// Case 5, the machine reboot, is not here: it is the systemd runner's own test
// (`an_ssh_runner_host_outlives_the_sign_out_and_a_restart`, step 4), which now keeps a link
// through the restart and checks the outage rules (told within 30 s, back inside the 3-minute
// limit), as the ruling says: "the existing systemd test, with the outage-rule checks added".

#[test]
fn an_app_restart_reattaches_each_chat_to_its_own_pool() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    // The app is the process that keeps the link: its end is the link kept going away, with
    // the chats running on the runner. A second app, as the first came back, links anew.
    let chats = rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the host holds the project");
        let one = typing_chat(&client, "app-one").await;
        let two = typing_chat(&client, "app-two").await;
        let mut chats = vec![one, two];
        chats.sort();
        assert_eq!(running(&client).await, chats);
        // The app goes away outright: no goodbye on the link.
        kept.task.abort();
        drop(client);
        chats
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked again");
        assert_eq!(
            running(&client).await,
            chats,
            "each chat is still on its pool"
        );
        types_into(&rig, &client, &chats[0], "app-one", "back").await;
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7187 train A: Pool forgotten ends its chats, and Forget lists what is left on the machine"]
fn a_pool_forgotten_during_a_chat_ends_it_as_pool_forgotten_and_lists_what_is_left_on_the_machine()
{
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "forgotten").await;
        let began = Instant::now();
        let left = seam::forget_the_pool(&rig);
        assert!(
            began.elapsed() <= QUIET_LIMIT,
            "Forget took {:?}",
            began.elapsed()
        );
        assert_eq!(seam::state_named(&rig, &chat), "Pool forgotten");
        // What is left on the machine is listed: the runner's host is still there, and says so.
        assert!(
            left.iter()
                .any(|line| line.contains(&rig.binary) || line.contains("runner")),
            "what is left was not listed: {left:?}"
        );
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
fn a_15_second_slow_link_still_links_inside_the_budgets_and_the_chat_works() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let began = Instant::now();
        rig.fault(&["slow", "15"]);
        // Every round trip costs 15 s each way, so the first link may take a minute or two: the
        // budget is the 3-minute limit.
        let kept = rig.keep_within(START_LIMIT).await;
        assert!(
            began.elapsed() <= START_LIMIT,
            "the link took {:?}",
            began.elapsed()
        );
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "slow15").await;
        assert_eq!(running(&client).await, vec![chat]);
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7187 train A: a start held to three minutes that names its stuck step"]
fn a_link_slower_than_the_budgets_fails_naming_the_step_inside_the_limit_with_no_hang() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        // 90 s a packet is past every budget (`CONNECT` 10 s, `RUN` 30 s).
        let fault_at = rig.fault(&["slow", "90"]);
        let (_folder, args) = bridge_args(&rig.reach, &rig.binary);
        let mut links = hostlink::keep_linked(
            CommandConnector::new(rig.reach.ssh.clone(), args, Arc::new(|| Ok(()))),
            Handshake::THIS_BUILD,
            rig.identity(),
            rig.runner.clone(),
            Liveness::default(),
        );
        let told = tokio::time::timeout(START_LIMIT, links.next())
            .await
            .expect("no hang: it said something inside the limit");
        let Some(Linked::Down(down)) = told else {
            panic!("a link over a 90 s delay must not come up");
        };
        let said_at = rig.began.elapsed() - fault_at;
        assert!(said_at <= START_LIMIT, "it said so after {said_at:?}");
        // It names the step it was stuck on, as the start's own words do.
        let why = down.why.to_lowercase();
        assert!(
            ["connect", "sign in", "forward", "handshake", "check"]
                .iter()
                .any(|step| why.contains(step)),
            "the failure does not name the step: {}",
            down.why
        );
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
fn a_half_open_link_is_found_within_the_stated_bound_and_the_chat_survives_it() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "blackhole").await;
        // Every packet lost, no reset: the link stays open and says nothing, as 2026-11-17's did.
        let fault_at = rig.fault(&["blackhole"]);
        kept.until_down(fault_at, QUIET_LIMIT).await;
        // **The person is told within 30 s**, the "nothing silent" rule itself, not a longer
        // detection bound. The link pings every 10 s and ends as dead after 30 s of hearing
        // nothing (`Liveness::default`), counted from the last frame heard, which was before the
        // fault: today's code meets 30 s, with no margin to spare. Were a detection ever to take
        // longer, a line saying the link is being checked must show within 30 s (train A, #7187),
        // and this bound stays 30 s. 2026-11-17's incident was five minutes.
        told_within(&kept.told(), fault_at, QUIET_LIMIT);
        let back_at = rig.restore();
        let after = kept
            .until_working(START_LIMIT, "the link after the blackhole")
            .await;
        assert!(
            first_up_after(&kept.told(), back_at).is_some_and(|back| back <= START_LIMIT),
            "back inside the limit"
        );
        assert_eq!(running(&after).await, vec![chat.clone()]);
        types_into(&rig, &after, &chat, "blackhole", "after").await;
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

// ---------------------------------------------------------------------------------------------
// Case 9, on the Docker pool: the registry down.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_registry_that_is_down_fails_the_check_within_its_budget_as_image_unreachable_and_leaves_no_container()
 {
    a_registry_that_is_down(false);
}

/// Case 9's other half, the state the window names for it: Refused (train A, #7187).
#[test]
#[ignore = "needs #7187 train A: the pool's state named Refused"]
fn a_registry_that_is_down_names_the_pool_refused() {
    a_registry_that_is_down(true);
}

/// The registry inside dind goes down under a Docker pool; the check fails inside its budget as
/// image unreachable and leaves no runner container; with `names_the_state`, the pool is also
/// named `Refused`, as the window words it.
fn a_registry_that_is_down(names_the_state: bool) {
    use purlis_core::dockerpool::{Docker, Image};
    use purlis_core::poolsetup::doctoring;
    use purlis_core::runners::{Pool, Remote, Set};
    use purlis_core::sshroute::{EnginePin, Pin, forward};

    purlis_core::unsteered!();
    let found = (|| {
        Some((
            std::env::var("REMOTE_DOCKER_TEST_ROUTE").ok()?,
            std::env::var("REMOTE_DOCKER_TEST_REGISTRY_DOWN_IMAGE").ok()?,
            std::env::var("REMOTE_DOCKER_TEST_SSHD").ok()?,
            std::env::var("REMOTE_DOCKER_TEST_DIND").ok()?,
        ))
    })();
    let Some((route, image, sshd, dind)) = fixture_or_skip(
        "Docker on another machine, with a registry inside it",
        "tests/remote-docker/up.sh",
        found,
    ) else {
        return;
    };
    struct Restore(String);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = purlis_core::forklock::output(
                std::process::Command::new("bash").arg(FAULTS).args([
                    "--machine",
                    &self.0,
                    "restore",
                ]),
            );
        }
    }
    let _restore = Restore(sshd.clone());
    fault(&sshd, &["restore"]);
    // In /tmp for a short path: ssh's control sockets live under it (a unix socket's path is
    // limited to about 104 bytes).
    let folders = tempfile::Builder::new()
        .prefix("rf")
        .tempdir_in("/tmp")
        .expect("a folder");
    let settings = folders.path().join("s");
    let ssh = sshroute::ssh_here().expect("the system's ssh");
    let program = std::path::PathBuf::from(
        std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()),
    );
    let docker = Docker::at(&program).with_settings_in(settings.clone());
    let route = Route::parse(&route).expect("a route");
    let key = connect::meet(
        &ssh,
        &route,
        &connect::resolve(&ssh, &route).expect("resolved"),
        &settings,
    )
    .expect("met");
    let found = connect::probe(&ssh, &route, "regdown", &key, &settings, "pool")
        .expect("its socket is found");
    let engine_links = folders.path().join(sshroute::DIR);
    let opening = forward::Opening {
        ssh: &ssh,
        route: &found,
        pool: "regdown",
        key: &key,
        engine_links: &engine_links,
        settings_in: &settings,
    };
    let engine: EnginePin = connect::engine_pin(&docker, &opening).expect("its engine answers");
    forward::let_go("regdown");
    let pool = Pool::new(
        "regdown",
        Image::pinned(&image).expect("an image"),
        Set::default(),
    )
    .expect("a pool")
    .on(Some(Remote {
        route: found,
        pin: Pin {
            host_key: key,
            engine,
        },
    }));
    // The registry inside dind goes down.
    fault(&sshd, &["break-registry"]);
    sshroute::pool::look_again();
    let began = Instant::now();
    let found = doctoring::check_with(&docker, &pool, &|| Vec::new());
    let took = began.elapsed();
    forward::let_go("regdown");
    let finding = found.expect_err("doctor finds the registry down");
    if names_the_state {
        assert_eq!(seam::pool_state_named("regdown"), "Refused");
    }
    assert!(
        matches!(finding, connect::Finding::ImageUnreachable { .. }),
        "image unreachable, not another finding: {finding:?}"
    );
    assert!(
        took <= Duration::from_secs(60),
        "the check took {took:?}, past its budget (CHECK_WITHIN, 60 s)"
    );
    // No container behind: no runner of any pool is on the engine.
    let left = purlis_core::forklock::output(std::process::Command::new("docker").args([
        "exec",
        &dind,
        "docker",
        "ps",
        "--all",
        "--quiet",
        "--filter",
        "label=purlis.runner",
    ]))
    .expect("docker runs");
    assert_eq!(
        String::from_utf8_lossy(&left.stdout).trim(),
        "",
        "a failed check leaves no runner container"
    );
}

// ---------------------------------------------------------------------------------------------
// The browser cases (train D, #7190).
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs #7190 train D: browsers on a pool's runner"]
fn b1_a_browsers_link_drops_and_returns_the_view_says_why_and_the_page_comes_back() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "b1").await;
        let browser = seam::open_a_browser(&rig, &chat);
        let page = seam::page_of(&rig, &browser);
        let fault_at = rig.fault(&["drop"]);
        kept.until_down(fault_at, QUIET_LIMIT).await;
        told_within(&kept.told(), fault_at, QUIET_LIMIT);
        let said = seam::browser_state(&rig, &browser);
        assert!(
            !said.trim().is_empty() && said != "Open",
            "the view says why: {said:?}"
        );
        let back_at = rig.restore();
        kept.until_working(START_LIMIT, "the link after the drop")
            .await;
        assert!(first_up_after(&kept.told(), back_at).is_some());
        assert_eq!(seam::browser_state(&rig, &browser), "Open");
        assert_eq!(seam::page_of(&rig, &browser), page, "the page returns");
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7190 train D: takeover of a runner's browser"]
fn b2_a_takeover_during_a_drop_keeps_control_or_the_view_says_why() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "b2").await;
        let browser = seam::open_a_browser(&rig, &chat);
        let fault_at = rig.fault(&["drop"]);
        kept.until_down(fault_at, QUIET_LIMIT).await;
        match seam::take_over(&rig, &browser) {
            Ok(()) => {}
            Err(why) => assert!(!why.trim().is_empty(), "control was lost without a reason"),
        }
        rig.restore();
        kept.until_working(START_LIMIT, "the link after the drop")
            .await;
        let state = seam::browser_state(&rig, &browser);
        assert!(
            !state.trim().is_empty(),
            "the view says where control is: {state:?}"
        );
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7190 train D and #7187 train A: a browser ends as Pool forgotten"]
fn b3_a_pool_forgotten_with_a_browser_open_ends_it_as_pool_forgotten() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "b3").await;
        let browser = seam::open_a_browser(&rig, &chat);
        seam::forget_the_pool(&rig);
        assert_eq!(seam::browser_state(&rig, &browser), "Pool forgotten");
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7190 train D: a quit leaves a runner's browser running and purlis reattaches"]
fn b4_an_app_quit_with_a_runner_browser_open_leaves_it_running_and_purlis_reattaches() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    let rt = runtime();
    let (chat, browser, page) = rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let chat = a_chat(&rig, &client, "b4").await;
        let browser = seam::open_a_browser(&rig, &chat);
        let page = seam::page_of(&rig, &browser);
        kept.task.abort();
        (chat, browser, page)
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked again");
        assert_eq!(running(&client).await, vec![chat]);
        assert_eq!(
            seam::browser_state(&rig, &browser),
            "Open",
            "it survived the quit"
        );
        assert_eq!(
            seam::page_of(&rig, &browser),
            page,
            "and shows what it showed"
        );
        kept.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

#[test]
#[ignore = "needs #7190 train D: browsers on a pool's runner, and a second runner machine in the rig (follow-up on up-systemd.sh)"]
fn b5_two_runners_that_start_browsers_at_once_run_them_in_parallel() {
    purlis_core::unsteered!();
    let Some(rig) = Rig::up() else { return };
    // Two runners, as ruled: each on a machine of its own, a chat on each.
    let other = seam::a_second_runner(&rig);
    let rt = runtime();
    rt.block_on(async {
        let kept = rig.keep().await;
        let client = kept.client().expect("linked");
        let one = a_chat(&rig, &client, "b5-one").await;
        let kept_other = other.keep().await;
        let client_other = kept_other.client().expect("linked");
        let two = a_chat(&other, &client_other, "b5-two").await;
        // Together, not one after the other: two starts take about as long as one.
        let alone = Instant::now();
        let first = seam::open_a_browser(&rig, &one);
        let one_took = alone.elapsed();
        let together = Instant::now();
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| seam::open_a_browser(&rig, &one));
            let b = scope.spawn(|| seam::open_a_browser(&other, &two));
            (a.join().unwrap(), b.join().unwrap())
        });
        assert!(
            together.elapsed() < one_took * 2,
            "two starts took {:?} where one took {one_took:?}: they ran one after the other",
            together.elapsed()
        );
        for (rig, browser) in [(&rig, first), (&rig, a), (&other, b)] {
            assert_eq!(seam::browser_state(rig, &browser), "Open");
        }
        kept.task.abort();
        kept_other.task.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
}

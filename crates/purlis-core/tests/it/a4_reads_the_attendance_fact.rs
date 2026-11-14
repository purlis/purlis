// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A4 reads the attendance fact** (T1-18, #6409; R-109, R-145; Q54, Q202, Q271): the release
//! floor refuses a chat the app started by the fact the app holds for it
//! ([`purlis_core::attendance::Fact`]), never by the `permission_mode` its harness reports. Where
//! the merge hold stands too, a merge is the hold's: the person is asked, and A4 does not refuse
//! it first.
//!
//! Tested at the core seam, [`purlis_core::toolgate::verdict`], which `purlis hook pretooluse`
//! asks with the fact it asked the app for ([`Launched::attendance`]).

use purlis_core::attendance::{Begun, ClaudeCodeMode, Fact};
use purlis_core::dispatchunattended::Attendance;
use purlis_core::handoffguard::Caller;
use purlis_core::harness::Harness;
use purlis_core::mergehold::{self, Held, Request};
use purlis_core::toolgate::{Arm, Attending, Call, Launched, Plane, verdict};

/// A publish A4 refuses an unattended chat.
const RELEASE: &str = "gh release create v1.0.0";

/// A merge of one request the hold can name.
const MERGE: &str = "gh pr merge 12 --squash";

/// Every `permission_mode` a harness may report: Claude Code's measured strings (Q270), Codex's
/// constant bypass report, a string nobody measured, and none.
fn every_mode() -> Vec<Option<&'static str>> {
    ClaudeCodeMode::MEASURED
        .iter()
        .map(|(said, _)| Some(*said))
        .chain([Some("never"), Some("dontAsk"), None])
        .collect()
}

/// Every harness word a hook is run under, and none.
const HARNESSES: [Option<&str>; 4] = [Some("claude"), Some("codex"), Some("opencode"), None];

/// Which of the gate's arms refuses `cmd` in a chat the app started whose fact says
/// `attendance`, its harness `harness` reporting `mode`.
fn refused_by(
    cmd: &str,
    attendance: Option<Attendance>,
    harness: Option<&str>,
    mode: Option<&str>,
) -> Option<Arm> {
    let asked = || attendance.expect("asked only of a chat the app started");
    let state = std::env::temp_dir();
    let forges = Vec::new();
    let plane = Plane {
        root: "/nowhere/plane",
        forges: &forges,
        session_dir: "",
        launched: Launched {
            sandboxed: false,
            chat_dir: None,
            attendance: attendance.map(|_| Attending(&asked)),
        },
    };
    let call = Call {
        command: cmd,
        cwd: "/nowhere/plane",
        state_dir: &state,
        caller: Caller {
            agent_id: None,
            harness,
            permission_mode: mode,
        },
    };
    verdict(&call, Some(&plane)).and_then(|said| said.arm)
}

#[test]
fn a4_denies_an_unattended_chat_whatever_the_harness_mode() {
    purlis_core::unsteered!();
    // Unattended by how purlis started it, on each harness: a task reports whatever mode its
    // harness runs in, and A4 refuses it in every one.
    for (harness, word) in [
        (Harness::ClaudeCode, Some("claude")),
        (Harness::Codex, Some("codex")),
        (Harness::Opencode, Some("opencode")),
    ] {
        let fact = Fact::at_start(Begun::Task, Some(harness), &[]);
        assert_eq!(fact.attendance(), Attendance::Unattended);
        for mode in every_mode() {
            assert_eq!(
                refused_by(RELEASE, Some(fact.attendance()), word, mode),
                Some(Arm::ReleaseFloor),
                "an unattended {word:?} chat reporting {mode:?} published"
            );
        }
    }
    // Attended, a mode that skips the person's prompts is A4's attended rule: the floor does
    // not refuse what the fact says a person watches. Codex reports its bypass on every line
    // (R-289), and A4 keys on nothing but the fact.
    let attended = Fact::at_start(Begun::Person, Some(Harness::Codex), &[]);
    assert_eq!(attended.attendance(), Attendance::Attended);
    for harness in HARNESSES {
        for mode in [Some("bypassPermissions"), Some("auto"), Some("never")] {
            assert_eq!(
                refused_by(RELEASE, Some(attended.attendance()), harness, mode),
                None,
                "an attended {harness:?} chat reporting {mode:?} was refused"
            );
        }
    }
    // A harness the app did not start has no fact: its own report is all A4 has, as the
    // recorded charter answered.
    assert_eq!(
        refused_by(RELEASE, None, Some("claude"), Some("bypassPermissions")),
        Some(Arm::ReleaseFloor)
    );
    assert_eq!(
        refused_by(RELEASE, None, Some("claude"), Some("default")),
        None
    );
}

#[test]
fn the_merge_hold_takes_precedence_over_a4() {
    purlis_core::unsteered!();
    // The hold names the request, so the person is asked to merge it.
    assert_eq!(
        mergehold::held(MERGE),
        Some(Held::Request(Request {
            repo: None,
            number: 12,
        }))
    );
    // With A4 reading attendance, neither an unattended nor an attended chat the app started is
    // refused the merge by A4: the hook goes on to the hold, which refuses it and raises the ask.
    for attendance in [Attendance::Unattended, Attendance::Attended] {
        for harness in HARNESSES {
            for mode in every_mode() {
                assert_ne!(
                    refused_by(MERGE, Some(attendance), harness, mode),
                    Some(Arm::ReleaseFloor),
                    "A4 refused a held merge of a {attendance:?} {harness:?} chat in {mode:?}"
                );
            }
        }
    }
    // A publish beside no merge is still A4's in the same unattended chat.
    assert_eq!(
        refused_by(RELEASE, Some(Attendance::Unattended), Some("claude"), None),
        Some(Arm::ReleaseFloor)
    );
    // Where no hold stands, a harness the app did not start, A4 refuses the merge as before.
    assert_eq!(
        refused_by(MERGE, None, Some("claude"), Some("bypassPermissions")),
        Some(Arm::ReleaseFloor)
    );
}

#[test]
fn a_tool_lines_own_report_only_lowers_the_fact_a4_reads() {
    purlis_core::unsteered!();
    // The hook asks the app with what its own line says (`purlis hook pretooluse`): the fact the
    // app holds, as it would be once it heard that line, which it may not have yet.
    let set = vec!["--permission-mode".to_owned(), "default".to_owned()];
    let attended = Fact::at_start(Begun::Person, Some(Harness::ClaudeCode), &set);
    assert_eq!(attended.hearing(false, None), Attendance::Attended);
    assert_eq!(
        attended.hearing(false, Some(ClaudeCodeMode::Default)),
        Attendance::Attended
    );
    // A mode purlis did not set, or the bypass bit, lowers it on Claude Code (R-107, Q271).
    assert_eq!(
        attended.hearing(false, Some(ClaudeCodeMode::Auto)),
        Attendance::Unattended
    );
    assert_eq!(attended.hearing(true, None), Attendance::Unattended);
    // Hearing it changes nothing the app holds: the line is the hook's, asked, not reported.
    assert_eq!(attended.attendance(), Attendance::Attended);
    // On Codex neither is read (R-289), and nothing a line says raises an unattended fact.
    let codex = Fact::at_start(Begun::Person, Some(Harness::Codex), &[]);
    assert_eq!(codex.hearing(true, None), Attendance::Attended);
    let task = Fact::at_start(Begun::Task, Some(Harness::ClaudeCode), &set);
    assert_eq!(
        task.hearing(false, Some(ClaudeCodeMode::Default)),
        Attendance::Unattended
    );
}

#[test]
fn the_fact_a4_reads_is_kept_across_a_relaunch() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path();
    let chat = |number: u32, unattended: bool| purlis_core::reopen::Chat {
        program: "claude".to_owned(),
        name: format!("chat {number}"),
        number: Some(number),
        unattended,
        ..Default::default()
    };
    let record = purlis_core::reopen::Record {
        chats: vec![chat(1, true), chat(2, false)],
        ..Default::default()
    };
    purlis_core::reopen::write(root, &record).expect("written");
    let read = purlis_core::reopen::read_or_refusal(root).expect("read");
    let kept: Vec<bool> = read.chats.iter().map(|chat| chat.unattended).collect();
    assert_eq!(kept, vec![true, false], "the record keeps the fact");
    // A relaunch of the chat the record kept unattended starts attended by its kind, and stays
    // unattended: the prior fact only keeps or lowers (R-318, Q271).
    let prior = Fact::kept(Attendance::Unattended);
    let relaunched = Fact::resumed(prior, Begun::Person, Some(Harness::ClaudeCode), &[]);
    assert_eq!(relaunched.attendance(), Attendance::Unattended);
    let prior = Fact::kept(Attendance::Attended);
    let relaunched = Fact::resumed(prior, Begun::Person, Some(Harness::ClaudeCode), &[]);
    assert_eq!(relaunched.attendance(), Attendance::Attended);
}

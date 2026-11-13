// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The block-loop rule (T1-12): three blocks on the same target pause a chat, and what counts,
//! what resets the count and what leaves it alone.

use super::*;
use crate::eventlog::Attributes;
use crate::eventlog::stops::{Hold, Qualifier, reported_from_outside_its_reach};
use crate::harness::Harness;
use crate::hookwire::{Decision, ToolCall};

const CHAT: u32 = 4;

/// Stuck? at 3 failures in a row, as by default: the shipped loops are at 6.
fn limits() -> Limits {
    Limits::of(&crate::stuck::Limits {
        silent_minutes: 15,
        failing_times: 3,
        long_minutes: 60,
    })
}

/// A block from `source`, reported as `qualifier` says, of a `Bash` call whose arguments hash
/// to `args`.
fn reported(source: Source, qualifier: Option<Qualifier>, args: &str) -> Stop {
    Stop::Block(Block {
        tool: "Bash".to_owned(),
        source,
        qualifier,
        target: None,
        args: Some(args.to_owned()),
    })
}

/// A block from `source`, reported plainly.
fn block(source: Source, args: &str) -> Stop {
    reported(source, None, args)
}

/// A hook line of chat [`CHAT`]: its hook's `word`, a `Bash` call whose arguments hash to
/// `args`, and what purlis's hook answered.
fn line(word: &str, args: &str, decision: Decision) -> ToolCall {
    ToolCall {
        chat: CHAT,
        tool_hook: word.to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some(format!("call-{args}")),
        args: Some(args.to_owned()),
        decision,
        rule: Some("bash-guard".to_owned()),
        arm: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

/// A guard's deny of a call whose arguments hash to `args`.
fn denied(args: &str) -> ToolCall {
    line("pretooluse", args, Decision::Deny)
}

/// A call whose arguments hash to `args`, come back from running.
fn succeeded(args: &str) -> ToolCall {
    ToolCall {
        rule: None,
        ..line("posttooluse", args, Decision::None)
    }
}

/// R-206: pins shipped behaviour (Q208, Q253); mutation: `BLOCKED_TIMES = 4`, or
/// `Loops::stopped` returning `None`.
#[test]
fn three_blocks_on_the_same_target_pause_the_chat() {
    let mut loops = Loops::default();
    assert_eq!(loops.stopped(CHAT, &block(Source::Guard, "aa")), None);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        None,
        "two do not"
    );
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
    // Told once: the chat is paused on it, and the person answers.
    assert_eq!(loops.stopped(CHAT, &block(Source::Guard, "aa")), None);

    let mut loops = Loops::default();
    for args in ["aa", "bb", "cc"] {
        assert_eq!(
            loops.stopped(CHAT, &block(Source::Guard, args)),
            None,
            "three on three digests do not"
        );
    }
}

/// R-207. Guard and sandbox are the sources phase 1 ships: a guard's deny heard on its hook
/// line, and a held connection the chat stopped waiting on, written by the app. The project
/// setting (phase 2) and the policy (phase 4) have no writer yet; their blocks count as values
/// already, and each phase adds its writer's own case.
#[test]
fn guard_and_sandbox_blocks_count_toward_the_block_loop() {
    // Guard: three denies of the same call, on its hook line, reported from outside its reach.
    let mut loops = Loops::default();
    assert_eq!(loops.called(CHAT, &denied("aa"), true), None);
    assert_eq!(loops.called(CHAT, &denied("aa"), true), None);
    assert_eq!(
        loops.called(CHAT, &denied("aa"), true),
        Some(Why::Blocked { times: 3 }),
        "a guard's denies count"
    );

    // Sandbox: three held connections to the same host under the same call, each its block.
    let held = || {
        Stop::Block(Block {
            tool: "Bash".to_owned(),
            source: Source::Sandbox,
            qualifier: None,
            target: Target::host("api.example.com").ok(),
            args: Some("aa".to_owned()),
        })
    };
    let mut loops = Loops::default();
    assert_eq!(loops.stopped(CHAT, &held()), None);
    // The call it was held under comes back: the same call, so no reset.
    assert_eq!(loops.called(CHAT, &succeeded("aa"), true), None);
    assert_eq!(loops.stopped(CHAT, &held()), None);
    assert_eq!(
        loops.stopped(CHAT, &held()),
        Some(Why::Blocked { times: 3 }),
        "the sandbox's blocks count"
    );

    // Phase 2 (project setting) and phase 4 (policy): counted as values, no writer yet.
    for source in [Source::ProjectSetting, Source::Policy] {
        let mut loops = Loops::default();
        loops.stopped(CHAT, &block(source, "aa"));
        loops.stopped(CHAT, &block(source, "aa"));
        assert_eq!(
            loops.stopped(CHAT, &block(source, "aa")),
            Some(Why::Blocked { times: 3 }),
            "{source:?}"
        );
    }
}

/// The run of a chat on `harness` under Seatbelt: the most a report can be worth.
fn on(harness: Harness) -> Attributes {
    Attributes {
        harness: Some(harness.name().to_owned()),
        sandbox: Some("seatbelt".to_owned()),
        ..Attributes::default()
    }
}

/// R-208.
#[test]
fn the_harness_own_check_never_counts_toward_the_block_loop() {
    for harness in Harness::ALL {
        let verified = reported_from_outside_its_reach(&on(harness));
        let mut loops = Loops::default();
        for _ in 0..3 {
            let refused = ToolCall {
                rule: None,
                ..line(crate::hookreg::HARNESS_DENIED, "aa", Decision::None)
            };
            assert_eq!(loops.called(CHAT, &refused, verified), None, "{harness:?}");
        }
        // Nor does its block, written plainly, nor once it is followed by a guard's.
        for _ in 0..3 {
            assert_eq!(
                loops.stopped(CHAT, &block(Source::HarnessCheck, "aa")),
                None,
                "{harness:?}"
            );
        }
        assert_eq!(loops.called(CHAT, &denied("aa"), true), None, "{harness:?}");
    }
}

/// R-209.
#[test]
fn unverified_reports_never_count_toward_the_block_loop() {
    let mut loops = Loops::default();
    for _ in 0..3 {
        assert_eq!(loops.called(CHAT, &denied("aa"), false), None, "a line");
    }
    for qualifier in [Qualifier::Unverified, Qualifier::CantTell] {
        let mut loops = Loops::default();
        for _ in 0..3 {
            assert_eq!(
                loops.stopped(CHAT, &reported(Source::Guard, Some(qualifier), "aa")),
                None,
                "{qualifier:?}"
            );
        }
    }
}

/// R-103.
#[test]
fn an_unverified_report_neither_resets_nor_suppresses_the_count() {
    let mut loops = Loops::default();
    let unverified = reported(Source::Guard, Some(Qualifier::Unverified), "aa");
    assert_eq!(loops.stopped(CHAT, &block(Source::Guard, "aa")), None);
    assert_eq!(loops.stopped(CHAT, &unverified), None);
    assert_eq!(loops.stopped(CHAT, &block(Source::Guard, "aa")), None);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 }),
        "the pause comes at the third verified block"
    );
    // On another target too: it does not reset the count it stands between.
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(
        CHAT,
        &reported(Source::Guard, Some(Qualifier::Unverified), "bb"),
    );
    loops.called(CHAT, &denied("bb"), false);
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
}

/// R-210.
#[test]
fn a_different_successful_call_resets_the_block_count() {
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    assert_eq!(loops.called(CHAT, &succeeded("bb"), true), None);
    assert_eq!(loops.stopped(CHAT, &block(Source::Guard, "aa")), None);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        None,
        "2 blocks, a success on another target, 2 blocks: no pause"
    );
    // A call allowed but not yet come back is no success.
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.called(CHAT, &line("pretooluse", "bb", Decision::Allow), true);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
}

/// R-211.
#[test]
fn an_allow_resets_the_block_count() {
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Sandbox, "aa"));
    loops.stopped(CHAT, &block(Source::Sandbox, "aa"));
    assert_eq!(loops.answered(CHAT, false, &limits()), None, "an Allow");
    assert_eq!(loops.stopped(CHAT, &block(Source::Sandbox, "aa")), None);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Sandbox, "aa")),
        None,
        "2 blocks, an Allow, 2 blocks: no pause"
    );
}

/// R-212: pins shipped behaviour (Q208, Q253); mutation: dropping the reset on an Allow in
/// `Loops::answered`, or pausing at `refused_times + 1`.
#[test]
fn six_dont_allows_still_pause_and_an_allow_resets_them() {
    let mut loops = Loops::default();
    for _ in 0..5 {
        assert_eq!(loops.answered(CHAT, true, &limits()), None);
    }
    loops.answered(CHAT, false, &limits());
    for _ in 0..5 {
        assert_eq!(
            loops.answered(CHAT, true, &limits()),
            None,
            "reset by the Allow"
        );
    }
    assert_eq!(
        loops.answered(CHAT, true, &limits()),
        Some(Why::Refused { times: 6 })
    );
}

/// R-213: pins shipped behaviour (Q208, Q253); mutation: no reset on another command or on a
/// success in `stuck::Failing::ran`, or the threshold off by one.
#[test]
fn six_identical_failures_still_pause_and_another_command_resets_them() {
    let mut loops = Loops::default();
    for _ in 0..5 {
        assert_eq!(loops.ran(CHAT, "cargo test", true, &limits()), None);
    }
    assert_eq!(
        loops.ran(CHAT, "cargo test", true, &limits()),
        Some(Why::Failing { times: 6 })
    );
    let mut loops = Loops::default();
    for _ in 0..5 {
        loops.ran(CHAT, "cargo test", true, &limits());
    }
    loops.ran(CHAT, "cargo build", true, &limits());
    for _ in 0..5 {
        assert_eq!(
            loops.ran(CHAT, "cargo test", true, &limits()),
            None,
            "another"
        );
    }
    loops.ran(CHAT, "cargo test", false, &limits());
    for _ in 0..5 {
        assert_eq!(
            loops.ran(CHAT, "cargo test", true, &limits()),
            None,
            "success"
        );
    }
}

/// A merge the hook held: a refusal with its ask open (T1-11).
fn held() -> Stop {
    Stop::Hold(Hold {
        tool: "Bash".to_owned(),
        target: Target::guard_arm(crate::mergehold::REASON).ok(),
        args: Some("aa".to_owned()),
    })
}

/// R-215: pins shipped behaviour (Q208, Q253); mutation: `Loops::stopped` counting a
/// `Stop::Hold`.
#[test]
fn a_block_with_an_open_ask_never_counts() {
    let mut loops = Loops::default();
    for _ in 0..3 {
        assert_eq!(
            loops.stopped(CHAT, &held()),
            None,
            "3 merge holds do not pause"
        );
    }
    // Nor does one reset the count it stands between.
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &held());
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
    // And its hook line, a deny the hook held as a merge, is no block.
    let mut loops = Loops::default();
    for _ in 0..3 {
        let line = ToolCall {
            rule: Some(crate::mergehold::REASON.to_owned()),
            ..denied("aa")
        };
        assert_eq!(loops.called(CHAT, &line, true), None);
    }
}

/// R-218: Let it keep trying is the pause's answer that lets the chat go on (T1-36 words it;
/// today's "It's fine, continue"). Pins shipped behaviour (Q208, Q253); mutation: `Loops::fine`
/// keeping the block count.
#[test]
fn let_it_keep_trying_resets_the_block_count() {
    let mut loops = Loops::default();
    for _ in 0..3 {
        loops.stopped(CHAT, &block(Source::Guard, "aa"));
    }
    loops.fine(CHAT);
    assert_eq!(loops.stopped(CHAT, &block(Source::Guard, "aa")), None);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        None,
        "2 more blocks do not pause"
    );
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
}

/// R-350. The cool-off (`crate::askrules::cooling`) is read from the person's answers alone,
/// which no block writes; the person's own refusals are this module's count of them. Pins
/// shipped behaviour (Q208, Q253); mutation: the refused branch of `Loops::answered` clearing
/// the block count, or `Loops::stopped` bumping `refused`.
#[test]
fn the_block_loop_and_the_cool_off_are_counted_apart() {
    // Don't allows leave the block count unchanged.
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    for _ in 0..3 {
        assert_eq!(loops.answered(CHAT, true, &limits()), None);
    }
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
    // Blocks leave the refusals' count unchanged.
    let mut loops = Loops::default();
    for _ in 0..5 {
        loops.answered(CHAT, true, &limits());
    }
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.called(CHAT, &denied("aa"), true);
    assert_eq!(
        loops.answered(CHAT, true, &limits()),
        Some(Why::Refused { times: 6 })
    );
}

/// R-211 (Q253): the person's Allow on a sandbox ask, on any of its paths (a held connection,
/// the chooser, a late answer), starts the block count again, as an Allow of a hook's ask does.
#[test]
fn an_allow_on_a_held_connection_resets_the_block_count() {
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Sandbox, "aa"));
    loops.stopped(CHAT, &block(Source::Sandbox, "aa"));
    loops.allowed(CHAT);
    assert_eq!(loops.stopped(CHAT, &block(Source::Sandbox, "aa")), None);
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Sandbox, "aa")),
        None,
        "2 blocks, an Allow on the held connection, 2 blocks: no pause"
    );
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Sandbox, "aa")),
        Some(Why::Blocked { times: 3 })
    );
}

/// The block one held connection of `call` makes: the same host every time, as a command that
/// retries one host inside it does.
fn held_under(loops: &mut Loops, call: &str) -> Option<Why> {
    let stop = Stop::Block(Block {
        tool: "Bash".to_owned(),
        source: Source::Sandbox,
        qualifier: None,
        target: Target::host("api.example.com").ok(),
        args: Some("aa".to_owned()),
    });
    loops.stopped_under(CHAT, &stop, Some(call))
}

/// R-206 (Q253): one call counts once, however many connections it held.
#[test]
fn one_call_holding_three_connections_does_not_pause() {
    let mut loops = Loops::default();
    for _ in 0..3 {
        assert_eq!(held_under(&mut loops, "toolu_1"), None, "one call");
    }
    assert_eq!(held_under(&mut loops, "toolu_2"), None);
    assert_eq!(
        held_under(&mut loops, "toolu_3"),
        Some(Why::Blocked { times: 3 }),
        "the third call held on it pauses"
    );
}

/// Q253: the count is kept per aim, so blocked calls that alternate still pause.
#[test]
fn alternating_blocked_calls_still_pause() {
    let mut loops = Loops::default();
    for args in ["aa", "bb", "aa", "bb"] {
        assert_eq!(loops.stopped(CHAT, &block(Source::Guard, args)), None);
    }
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 }),
        "A, B, A, B, A pauses at A's third"
    );

    // The same resets clear every aim: a different success, an Allow, Let it keep trying.
    let resets: [fn(&mut Loops); 4] = [
        |loops| {
            loops.called(CHAT, &succeeded("cc"), true);
        },
        |loops| loops.allowed(CHAT),
        |loops| {
            loops.answered(CHAT, false, &limits());
        },
        |loops| loops.fine(CHAT),
    ];
    for reset in resets {
        let mut loops = Loops::default();
        for args in ["aa", "bb", "aa", "bb"] {
            loops.stopped(CHAT, &block(Source::Guard, args));
        }
        reset(&mut loops);
        for args in ["aa", "bb", "aa", "bb"] {
            assert_eq!(
                loops.stopped(CHAT, &block(Source::Guard, args)),
                None,
                "counted again from nothing"
            );
        }
    }
}

/// Q253: the per-aim count is bounded; past [`AIMS_KEPT`] aims, the oldest is dropped.
#[test]
fn the_block_count_keeps_a_bounded_number_of_aims() {
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    for other in 0..AIMS_KEPT {
        loops.stopped(CHAT, &block(Source::Guard, &format!("other-{other}")));
    }
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        None,
        "its count was dropped as the oldest"
    );

    // Within the bound, the oldest is kept.
    let mut loops = Loops::default();
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    loops.stopped(CHAT, &block(Source::Guard, "aa"));
    for other in 1..AIMS_KEPT {
        loops.stopped(CHAT, &block(Source::Guard, &format!("other-{other}")));
    }
    assert_eq!(
        loops.stopped(CHAT, &block(Source::Guard, "aa")),
        Some(Why::Blocked { times: 3 })
    );
}

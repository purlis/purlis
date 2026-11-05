// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The asks to allow a program and the grants they give, with time passed in.

use super::*;
use crate::harness::asks::{Admitted, Answerer};

fn window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).unwrap()
}

fn bound(chat: u32, args: &[&str]) -> Bound {
    Bound {
        chat,
        program: "/usr/bin/curl".into(),
        vault: "team".into(),
        env: vec!["T=TOKEN".into()],
        file: Vec::new(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    }
}

fn need(bound: Bound) -> RunNeed {
    RunNeed {
        command: format!("curl {}", bound.args.join(" ")),
        bound,
        persona: "devops".into(),
        hosts: Hosts::Listed(vec!["api.example.com".into()]),
        caution: None,
    }
}

fn hold(asks: &Asks, bound: Bound, within: Duration) -> RunHeld {
    asks.allowed_or_held(need(bound), within, &mut |_| {})
}

/// The chat runs `bound` with no time to wait: an ask stays open, if none covers it.
fn stopped(asks: &Asks, bound: Bound) -> RunHeld {
    hold(asks, bound, Duration::ZERO)
}

#[test]
fn a_run_with_no_grant_raises_one_ask_and_runs_after_again_join_it() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    assert!(matches!(
        stopped(&asks, bound(7, &["a"])),
        RunHeld::StillOpen { .. }
    ));
    assert!(matches!(
        stopped(&asks, bound(7, &["a"])),
        RunHeld::StillOpen { .. }
    ));
    assert_eq!(asks.open_runs().len(), 1);
    stopped(&asks, bound(8, &["a"]));
    assert_eq!(asks.open_runs().len(), 2, "one chat's ask isn't another's");
}

#[test]
fn only_the_window_answers() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    let id = asks.open_runs()[0].id;
    let refused = asks
        .answer_run(
            id,
            RunAnswer::Once,
            Answerer::admitted(Admitted::Approval).unwrap(),
        )
        .unwrap_err();
    assert!(refused.contains("window"), "{refused}");
    assert_eq!(asks.open_runs().len(), 1, "left open");
    assert!(
        matches!(stopped(&asks, bound(7, &["a"])), RunHeld::StillOpen { .. }),
        "nothing granted"
    );
}

#[test]
fn allow_once_outlives_the_wait_is_good_for_one_run_of_that_command_and_nudges() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    let id = asks.open_runs()[0].id;
    let answered = asks.answer_run(id, RunAnswer::Once, window()).unwrap();
    assert_eq!(answered.nudge, [7]);
    assert!(
        answered.told.line().contains("curl"),
        "{}",
        answered.told.line()
    );
    assert!(matches!(
        stopped(&asks, bound(7, &["b"])),
        RunHeld::StillOpen { .. }
    ));
    assert_eq!(stopped(&asks, bound(7, &["a"])), RunHeld::Allowed);
    assert!(
        matches!(stopped(&asks, bound(7, &["a"])), RunHeld::StillOpen { .. }),
        "used"
    );
    let other = asks.answer_run(99, RunAnswer::Once, window()).unwrap_err();
    assert!(other.contains("no longer open"), "{other}");
}

#[test]
fn allow_once_ends_with_the_chat() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    asks.answer_run(asks.open_runs()[0].id, RunAnswer::Once, window())
        .unwrap();
    asks.forget(7);
    assert!(matches!(
        stopped(&asks, bound(7, &["a"])),
        RunHeld::StillOpen { .. }
    ));
}

#[test]
fn allow_once_reaches_one_of_two_runs_holding_on_it_and_the_other_hears_so() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    let (first, second) = std::thread::scope(|scope| {
        let one = scope.spawn(|| hold(&asks, bound(7, &["a"]), Duration::from_secs(60)));
        let two = scope.spawn(|| hold(&asks, bound(7, &["a"]), Duration::from_secs(60)));
        let id = loop {
            let open = asks.open_runs();
            let waiting = asks
                .inner()
                .runs
                .open
                .first()
                .map_or(0, |o| o.waiters.len());
            if waiting == 2 {
                break open[0].id;
            }
            std::thread::yield_now();
        };
        let answered = asks.answer_run(id, RunAnswer::Once, window()).unwrap();
        assert!(answered.nudge.is_empty(), "both were holding");
        (one.join().unwrap(), two.join().unwrap())
    });
    let allowed = [&first, &second]
        .into_iter()
        .filter(|held| **held == RunHeld::Allowed)
        .count();
    assert_eq!(allowed, 1, "{first:?} {second:?}");
    assert!(
        matches!(stopped(&asks, bound(7, &["a"])), RunHeld::StillOpen { .. }),
        "nothing left over"
    );
}

#[test]
fn dont_allow_is_remembered_for_that_chat_program_and_vault() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    asks.answer_run(asks.open_runs()[0].id, RunAnswer::Refuse, window())
        .unwrap();
    let refused = stopped(&asks, bound(7, &["b"]));
    assert!(matches!(refused, RunHeld::Refused { .. }), "{refused:?}");
    assert!(asks.open_runs().is_empty());
    let mut other_vault = bound(7, &["a"]);
    other_vault.vault = "other".into();
    assert!(matches!(
        stopped(&asks, other_vault),
        RunHeld::StillOpen { .. }
    ));
    let mut other_program = bound(7, &["a"]);
    other_program.program = "/usr/bin/wget".into();
    assert!(matches!(
        stopped(&asks, other_program),
        RunHeld::StillOpen { .. }
    ));
    asks.forget(7);
    assert!(
        matches!(stopped(&asks, bound(7, &["b"])), RunHeld::StillOpen { .. }),
        "ends with the chat"
    );
}

#[test]
fn an_open_ask_outlives_a_restart_a_grant_does_not_and_a_chat_gone_takes_its_ask() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(crate::names::state(tmp.path()).join("app")).unwrap();
    let asks = Asks::kept(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    stopped(&asks, bound(8, &["a"]));
    stopped(&asks, bound(7, &["given"]));
    let given = asks
        .open_runs()
        .into_iter()
        .find(|open| open.bound.args == ["given"])
        .unwrap();
    asks.answer_run(given.id, RunAnswer::Once, window())
        .unwrap();
    drop(asks);

    let back = Asks::kept(tmp.path());
    assert_eq!(back.open_runs().len(), 2, "kept across the restart");
    back.only_these_came_back(&[7]);
    let open = back.open_runs();
    assert_eq!(open.len(), 1, "chat 8 did not come back");
    assert_eq!(open[0].bound.chat, 7);
    let answered = back
        .answer_run(open[0].id, RunAnswer::Once, window())
        .unwrap();
    assert_eq!(answered.nudge, [7], "its command stopped waiting");
    assert!(
        matches!(
            stopped(&back, bound(7, &["given"])),
            RunHeld::StillOpen { .. }
        ),
        "a grant is held in memory alone"
    );
}

#[test]
fn numbers_are_never_given_twice_across_the_two_kinds_of_ask() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    let run_id = asks.open_runs()[0].id;
    asks.unconfirmed(crate::secrets::ask::Need {
        chat: Some(7),
        persona: "devops".into(),
        vault: "team".into(),
        key: "K".into(),
        tool: None,
        purpose: None,
        declared: None,
        names_no_vault: false,
        reason: None,
    })
    .unwrap();
    assert_ne!(asks.open()[0].id, run_id);
}

#[test]
fn a_chat_has_at_most_so_many_asks_open() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    for n in 0..MOST_OPEN_PER_CHAT + 2 {
        stopped(&asks, bound(7, &[&n.to_string()]));
    }
    assert_eq!(asks.open_runs().len(), MOST_OPEN_PER_CHAT);
    stopped(&asks, bound(8, &["a"]));
    assert_eq!(asks.open_runs().len(), MOST_OPEN_PER_CHAT + 1, "per chat");
}

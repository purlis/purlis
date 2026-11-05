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
        not_its_vault: false,
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
        no_allow: None,
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

/// The one ask open for `bound`'s chat, answered from the window.
fn answer_open(asks: &Asks, answer: RunAnswer) {
    let open = asks.open_runs();
    asks.answer_run(open[0].id, answer, window()).unwrap();
}

#[test]
fn allow_for_this_chat_runs_any_arguments_of_that_program_and_keys() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &["get"]));
    answer_open(&asks, RunAnswer::ForChat);
    assert_eq!(stopped(&asks, bound(1, &["get"])), RunHeld::Allowed);
    assert_eq!(stopped(&asks, bound(1, &["delete", "x"])), RunHeld::Allowed);
    assert!(asks.open_runs().is_empty(), "nothing more was asked");
}

#[test]
fn allow_for_this_chat_answers_every_open_ask_it_covers_and_leaves_the_rest() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &["get"]));
    stopped(&asks, bound(1, &["delete"]));
    stopped(&asks, bound(2, &["get"]));
    let first = asks.open_runs()[0].id;
    let answered = asks
        .answer_run(first, RunAnswer::ForChat, window())
        .unwrap();
    assert_eq!(answered.nudge, [1], "one chat, nudged once");
    let open = asks.open_runs();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].bound.chat, 2, "another chat's stays open");
}

#[test]
fn a_key_added_to_what_a_chat_was_allowed_asks_again() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &[]));
    answer_open(&asks, RunAnswer::ForChat);
    let mut more = bound(1, &[]);
    more.env.push("U=OTHER".into());
    assert!(matches!(stopped(&asks, more), RunHeld::StillOpen { .. }));
    assert_eq!(asks.open_runs().len(), 1, "a key added asks again");
}

#[test]
fn allow_for_this_chat_ends_with_the_chat_and_reaches_no_other() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &[]));
    answer_open(&asks, RunAnswer::ForChat);
    assert!(matches!(
        stopped(&asks, bound(2, &[])),
        RunHeld::StillOpen { .. }
    ));
    asks.forget(1);
    assert!(asks.given_runs().is_empty());
    assert!(matches!(
        stopped(&asks, bound(1, &[])),
        RunHeld::StillOpen { .. }
    ));
}

#[test]
fn a_grant_is_listed_and_revoked_and_the_next_run_asks_again() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &[]));
    answer_open(&asks, RunAnswer::ForChat);
    let given = asks.given_runs();
    assert_eq!(given.len(), 1);
    assert!(!given[0].once);
    let gone = asks.revoke_run(given[0].id, window()).unwrap();
    assert_eq!(gone.bound, bound(1, &[]));
    assert!(asks.given_runs().is_empty());
    assert!(matches!(
        stopped(&asks, bound(1, &[])),
        RunHeld::StillOpen { .. }
    ));
    assert!(
        asks.revoke_run(given[0].id, window()).is_err(),
        "it is no longer there"
    );
}

#[test]
fn an_allow_once_not_yet_used_is_listed_and_goes_when_it_is_used() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &["get"]));
    answer_open(&asks, RunAnswer::Once);
    assert!(asks.given_runs()[0].once);
    assert_eq!(stopped(&asks, bound(1, &["get"])), RunHeld::Allowed);
    assert!(asks.given_runs().is_empty());
}

#[test]
fn only_the_window_revokes_a_grant() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &[]));
    answer_open(&asks, RunAnswer::ForChat);
    let id = asks.given_runs()[0].id;
    assert!(
        asks.revoke_run(id, Answerer::admitted(Admitted::Approval).unwrap())
            .is_err()
    );
    assert_eq!(asks.given_runs().len(), 1);
}

#[test]
fn dont_allow_takes_back_what_that_chat_was_given_of_that_program_and_vault() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &[]));
    answer_open(&asks, RunAnswer::ForChat);
    let mut more = bound(1, &[]);
    more.env.push("U=OTHER".into());
    stopped(&asks, more.clone());
    answer_open(&asks, RunAnswer::Refuse);
    assert!(asks.given_runs().is_empty());
    assert!(matches!(
        stopped(&asks, bound(1, &[])),
        RunHeld::Refused { .. }
    ));
}

#[test]
fn a_kept_ask_comes_back_but_never_a_chat_grant() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(crate::names::state(tmp.path()).join("app")).unwrap();
    let asks = Asks::kept(tmp.path());
    stopped(&asks, bound(1, &[]));
    answer_open(&asks, RunAnswer::ForChat);
    assert!(Asks::kept(tmp.path()).given_runs().is_empty());
}

#[test]
fn what_the_audit_says_names_the_program_keys_vault_scope_and_chat_and_nothing_of_the_command() {
    let mut one = bound(4, &["--token", "s3cret-in-an-argument"]);
    one.file.push("KUBECONFIG=config".into());
    let audited = Audited::granted(&one, RunAnswer::ForChat);
    assert_eq!(audited.kind(), GRANT_AUDITED);
    let body = audited.body();
    assert_eq!(body["program"], "/usr/bin/curl");
    assert_eq!(body["keys"], serde_json::json!(["TOKEN", "config"]));
    assert_eq!(body["vault"], "team");
    assert_eq!(body["level"], "chat");
    assert_eq!(body["chat"], 4);
    assert!(!body.to_string().contains("s3cret"), "{body}");
    assert_eq!(Audited::revoked(&one, true).kind(), REVOKE_AUDITED);
    assert_eq!(Audited::revoked(&one, true).body()["level"], "once");
    assert_eq!(
        Audited::granted(&one, RunAnswer::Refuse).kind(),
        REFUSE_AUDITED
    );
}

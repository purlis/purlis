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
        name: "curl".into(),
        program: "/usr/bin/curl".into(),
        vault: "team".into(),
        env: vec!["T=TOKEN".into()],
        file: Vec::new(),
        cwd: "/work".into(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    }
}

fn need(bound: Bound) -> RunNeed {
    RunNeed {
        ahead: false,
        fills: Vec::new(),
        offer: Offer::Program,
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

/// [`hold`], and, where it may go on, the run starting, which uses the grant (#5242).
fn started(asks: &Asks, bound: Bound, within: Duration) -> RunHeld {
    match hold(asks, bound.clone(), within) {
        RunHeld::Allowed if !asks.granted(&bound) => RunHeld::Refused {
            line: "taken".into(),
        },
        held => held,
    }
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
    assert_eq!(
        started(&asks, bound(7, &["a"]), Duration::ZERO),
        RunHeld::Allowed
    );
    assert!(
        matches!(stopped(&asks, bound(7, &["a"])), RunHeld::StillOpen { .. }),
        "used"
    );
    let other = asks.answer_run(99, RunAnswer::Once, window()).unwrap_err();
    assert!(other.contains("already been answered"), "{other}");
}

#[test]
fn allow_once_is_used_as_its_run_starts_so_one_that_stops_short_keeps_it_for_its_retry() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    stopped(&asks, bound(7, &["a"]));
    asks.answer_run(asks.open_runs()[0].id, RunAnswer::Once, window())
        .unwrap();
    // The run told it may go stops short: its slots are full, or it waits on a value.
    assert_eq!(stopped(&asks, bound(7, &["a"])), RunHeld::Allowed);
    assert_eq!(
        started(&asks, bound(7, &["a"]), Duration::ZERO),
        RunHeld::Allowed,
        "its retry runs with no new ask"
    );
    assert!(
        matches!(stopped(&asks, bound(7, &["a"])), RunHeld::StillOpen { .. }),
        "and then it is used"
    );
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
        let one = scope.spawn(|| started(&asks, bound(7, &["a"]), Duration::from_secs(60)));
        let two = scope.spawn(|| started(&asks, bound(7, &["a"]), Duration::from_secs(60)));
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
            if one.is_finished() || two.is_finished() {
                let ended = if one.is_finished() {
                    one.join()
                } else {
                    two.join()
                };
                panic!("a run ended before both were holding: {ended:?}");
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
    assert!(
        asks.given_runs()[0].once,
        "a run told it may go is yet to start: it is not used until then (#5242)"
    );
    assert!(asks.granted(&bound(1, &["get"])), "the run starts");
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
    let audited = Audited::granted(&one, RunAnswer::ForChat).expect("a Run grant's");
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
        Audited::granted(&one, RunAnswer::Refuse)
            .expect("a Run grant's")
            .kind(),
        REFUSE_AUDITED
    );
}

#[test]
fn only_a_program_reviewed_by_both_its_names_may_have_a_scope_wider_than_one_command() {
    assert!(bound(1, &[]).reviewed_for_any_arguments());
    let mut other = bound(1, &[]);
    other.program = "/usr/bin/wget".into();
    other.name = "wget".into();
    assert!(
        !other.reviewed_for_any_arguments(),
        "wget's --use-askpass runs a command"
    );
    let mut linked = bound(1, &[]);
    linked.program = "/opt/somewhere/sh".into();
    assert!(
        !linked.reviewed_for_any_arguments(),
        "the name alone is not enough"
    );
    let mut spelled = bound(1, &[]);
    spelled.name = "CURL".into();
    assert!(!spelled.reviewed_for_any_arguments());
}

#[test]
fn a_wider_answer_on_a_program_not_reviewed_is_refused_from_the_window_too() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    let mut one = bound(1, &[]);
    one.program = "/usr/bin/kubectl".into();
    one.name = "kubectl".into();
    stopped(&asks, one);
    let id = asks.open_runs()[0].id;
    assert!(asks.answer_run(id, RunAnswer::ForChat, window()).is_err());
    assert!(asks.answer_run(id, RunAnswer::Always, window()).is_err());
    assert_eq!(asks.open_runs().len(), 1);
    assert!(asks.answer_run(id, RunAnswer::Refuse, window()).is_ok());
}

/// [`need`] for a vault its persona may not use: only the vault's own Allow would open it.
fn on_a_vault_not_its(bound: Bound) -> RunNeed {
    RunNeed {
        not_its_vault: true,
        offer: Offer::Vault { no_allow: None },
        ..need(bound)
    }
}

#[test]
fn keep_blocked_after_the_gate_read_it_refuses_an_ask_opened_after_it() {
    // The run read "not kept blocked" at the gate; the person's Keep blocked lands before its ask
    // opens (#5424). It opens nothing, and is refused, rather than raise a notice after the sweep.
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    let swept = asks.keep_vault_blocked(7, "team");
    assert!(swept.is_empty(), "nothing open to sweep yet");
    let held = asks.allowed_or_held(
        on_a_vault_not_its(bound(7, &["a"])),
        Duration::ZERO,
        &mut |_| {},
    );
    assert!(matches!(held, RunHeld::Refused { .. }), "{held:?}");
    assert!(asks.open_runs().is_empty());
    // Another chat, and a vault its persona uses, are another matter.
    asks.allowed_or_held(
        on_a_vault_not_its(bound(8, &["a"])),
        Duration::ZERO,
        &mut |_| {},
    );
    stopped(&asks, bound(7, &["a"]));
    assert_eq!(asks.open_runs().len(), 2);
}

#[test]
fn keep_blocked_names_the_open_asks_that_would_open_the_vault_and_ends_with_the_chat() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    asks.allowed_or_held(
        on_a_vault_not_its(bound(7, &["a"])),
        Duration::ZERO,
        &mut |_| {},
    );
    stopped(&asks, bound(7, &["b"]));
    let vaults = asks.open_runs();
    let not_its = vaults.iter().find(|open| open.not_its_vault).unwrap().id;
    assert_eq!(asks.keep_vault_blocked(7, "team"), vec![not_its]);
    asks.forget(7);
    let held = asks.allowed_or_held(
        on_a_vault_not_its(bound(7, &["a"])),
        Duration::ZERO,
        &mut |_| {},
    );
    assert!(
        matches!(held, RunHeld::StillOpen { .. }),
        "a chat number used again starts clean: {held:?}"
    );
}

/// What chat `chat`'s trace says of its Run grant asks, `(what, ask)`, in the order it was written.
fn runs_traced(root: &std::path::Path, chat: u32) -> Vec<(String, u64)> {
    let file = crate::trace::file(root, &chat.to_string());
    std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["event"] == super::super::TRACED_AS && line["program"].is_string())
        .map(|line| {
            (
                line["what"].as_str().unwrap_or_default().to_owned(),
                line["ask"].as_u64().unwrap_or_default(),
            )
        })
        .collect()
}

/// #5509, as #5220 holds for a Secret ask: a Run grant's ask is traced raised, and joined, before
/// any answer to it, however soon the person answers. One thread raises and another joins while
/// a third answers the moment the ask is listed, many times over.
#[test]
fn a_run_grants_raise_is_traced_before_its_answer_however_soon_it_is_answered() {
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    for round in 0..100 {
        let args = [format!("round-{round}")];
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        std::thread::scope(|scope| {
            let runs = [0, 1].map(|_| {
                let (asks, bound) = (&asks, bound(7, &args));
                scope.spawn(move || hold(asks, bound, Duration::from_secs(60)))
            });
            while !runs.iter().all(std::thread::ScopedJoinHandle::is_finished) {
                if let Some(open) = asks
                    .open_runs()
                    .into_iter()
                    .find(|open| open.bound.args == args)
                {
                    let _ = asks.answer_run(open.id, RunAnswer::Once, window());
                }
                std::thread::yield_now();
            }
        });
    }
    let traced = runs_traced(tmp.path(), 7);
    let mut answered = std::collections::HashSet::new();
    let mut raised = std::collections::HashSet::new();
    for (what, ask) in &traced {
        match what.as_str() {
            "raised" => assert!(raised.insert(*ask), "raised twice: {traced:?}"),
            "joined" => assert!(
                raised.contains(ask) && !answered.contains(ask),
                "joined {ask} out of turn: {traced:?}"
            ),
            _ => {
                assert!(
                    raised.contains(ask),
                    "{what} for {ask} before its raise: {traced:?}"
                );
                answered.insert(*ask);
            }
        }
    }
    assert!(!answered.is_empty(), "{traced:?}");
}

#[test]
fn keep_blocked_takes_back_a_grant_given_on_that_vault_for_that_chat() {
    // A run past the gate before the press is covered by a grant, and never reaches an ask: the
    // grant goes with the press (#5424), and another chat's stays.
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    for chat in [7, 8] {
        asks.allowed_or_held(
            RunNeed {
                not_its_vault: true,
                ..need(bound(chat, &["a"]))
            },
            Duration::ZERO,
            &mut |_| {},
        );
        answer_open(&asks, RunAnswer::ForChat);
    }
    assert!(asks.covers(&bound(7, &["b"])));
    asks.keep_vault_blocked(7, "team");
    assert!(!asks.covers(&bound(7, &["b"])), "taken back");
    assert!(!asks.granted(&bound(7, &["b"])));
    assert!(asks.covers(&bound(8, &["b"])), "another chat's stays");
}

#[test]
fn an_answer_that_would_open_a_vault_kept_blocked_for_its_chat_is_refused_and_gives_nothing() {
    // A press already past the window's own checks when Keep blocked lands (#5617): refused under
    // the lock the mark is set under, so it gives nothing. Refuse still answers, and another
    // chat's ask, or one on its persona's own vault, is another matter.
    let tmp = tempfile::tempdir().unwrap();
    let asks = Asks::new(tmp.path());
    asks.allowed_or_held(
        RunNeed {
            not_its_vault: true,
            ..need(bound(7, &["a"]))
        },
        Duration::ZERO,
        &mut |_| {},
    );
    let id = asks.open_runs()[0].id;
    asks.keep_vault_blocked(7, "team");
    for answer in [RunAnswer::Once, RunAnswer::ForChat, RunAnswer::Always] {
        let given = asks.answer_run(id, answer, window());
        assert!(given.is_err(), "{answer:?}: {given:?}");
    }
    assert!(asks.given_runs().is_empty());
    assert_eq!(asks.open_runs().len(), 1, "left for Keep blocked to answer");
    asks.answer_run(id, RunAnswer::Refuse, window())
        .expect("Don't allow still answers");

    asks.allowed_or_held(
        RunNeed {
            not_its_vault: true,
            ..need(bound(8, &["a"]))
        },
        Duration::ZERO,
        &mut |_| {},
    );
    answer_open(&asks, RunAnswer::ForChat);
    assert!(asks.covers(&bound(8, &["b"])), "another chat's is its own");
}

#[test]
fn keep_blocked_answers_the_grants_it_took_back_for_their_audit() {
    // Each grant Keep blocked takes back is audited as taken back, as one removed from the
    // Allowed list is (#5617): it names them, and only that chat's on that vault.
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    for chat in [7, 8] {
        asks.allowed_or_held(
            RunNeed {
                not_its_vault: true,
                ..need(bound(chat, &["a"]))
            },
            Duration::ZERO,
            &mut |_| {},
        );
        answer_open(&asks, RunAnswer::ForChat);
    }
    let kept = asks.keep_vault_blocked_taking(7, "team");
    assert!(kept.open.is_empty());
    assert_eq!(kept.taken_back.len(), 1, "{kept:?}");
    assert_eq!(kept.taken_back[0].bound.chat, 7);
    assert!(!kept.taken_back[0].once);
    assert!(
        asks.keep_vault_blocked_taking(7, "team")
            .taken_back
            .is_empty(),
        "taken back once"
    );
}

// #5800: the Allowed list lists Allow once grants, so it is told when one goes some way other
// than an answer or a Keep blocked, which the window says itself.

/// `asks`, counting each time it tells that its Run grants moved.
fn counting(asks: &Asks) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let told = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    asks.telling_run_grants_moved({
        let told = std::sync::Arc::clone(&told);
        std::sync::Arc::new(move || {
            told.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    });
    told
}

fn times(told: &std::sync::atomic::AtomicUsize) -> usize {
    told.load(std::sync::atomic::Ordering::SeqCst)
}

#[test]
fn an_allow_once_taken_by_its_run_tells_that_the_run_grants_moved() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &["get"]));
    answer_open(&asks, RunAnswer::Once);
    let told = counting(&asks);
    assert_eq!(stopped(&asks, bound(1, &["get"])), RunHeld::Allowed);
    assert_eq!(times(&told), 0, "a run told it may go has not used it yet");
    assert!(asks.granted(&bound(1, &["get"])));
    assert_eq!(times(&told), 1);
}

#[test]
fn a_grant_for_the_chat_used_by_a_run_stays_and_tells_nothing() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &["get"]));
    answer_open(&asks, RunAnswer::ForChat);
    let told = counting(&asks);
    assert!(asks.granted(&bound(1, &["other"])));
    assert_eq!(times(&told), 0);
}

#[test]
fn an_allow_once_dropped_unused_tells_that_the_run_grants_moved() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(1, &["get"]));
    answer_open(&asks, RunAnswer::Once);
    let told = counting(&asks);
    asks.drop_once(&bound(1, &["other"]));
    assert_eq!(times(&told), 0, "nothing was dropped");
    asks.drop_once(&bound(1, &["get"]));
    assert!(asks.given_runs().is_empty());
    assert_eq!(times(&told), 1);
}

#[test]
fn a_chat_that_closes_holding_a_grant_tells_that_the_run_grants_moved() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(7, &["get"]));
    answer_open(&asks, RunAnswer::Once);
    let told = counting(&asks);
    asks.forget(8);
    assert_eq!(times(&told), 0, "chat 8 held no grant");
    asks.forget(7);
    assert!(asks.given_runs().is_empty());
    assert_eq!(times(&told), 1);
}

#[test]
fn what_is_told_of_moved_grants_may_read_them_again_at_once() {
    let asks = std::sync::Arc::new(Asks::new(std::path::Path::new("/nowhere")));
    stopped(&asks, bound(1, &["get"]));
    answer_open(&asks, RunAnswer::Once);
    let read = std::sync::Arc::new(std::sync::Mutex::new(None));
    asks.telling_run_grants_moved({
        let asks = std::sync::Arc::downgrade(&asks);
        let read = std::sync::Arc::clone(&read);
        std::sync::Arc::new(move || {
            if let Some(asks) = asks.upgrade() {
                *read.lock().unwrap() = Some(asks.given_runs().len());
            }
        })
    });
    assert!(asks.granted(&bound(1, &["get"])));
    assert_eq!(*read.lock().unwrap(), Some(0), "told off the asks' lock");
}

#[test]
fn grants_of_chats_a_launch_did_not_bring_back_tell_that_the_run_grants_moved() {
    let asks = Asks::new(std::path::Path::new("/nowhere"));
    stopped(&asks, bound(7, &["get"]));
    answer_open(&asks, RunAnswer::Once);
    let told = counting(&asks);
    asks.only_these_came_back(&[7]);
    assert_eq!(times(&told), 0, "chat 7 came back with its grant");
    asks.only_these_came_back(&[8]);
    assert!(asks.given_runs().is_empty());
    assert_eq!(times(&told), 1);
}

#[test]
fn a_run_grant_that_waits_says_where_the_person_answers_and_that_it_stays_answerable() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("smart-ide");
    std::fs::create_dir(&project).unwrap();
    let asks = Asks::new(&project);
    let mut said = Vec::new();
    let held = asks.allowed_or_held(need(bound(7, &["a"])), Duration::ZERO, &mut |line: &str| {
        said.push(line.to_owned())
    });
    assert_eq!(
        said,
        [
            "purlis asked the person to allow curl with vault team; waiting for the person: Asks \
          (✋) in the purlis window, project smart-ide, for up to 0 seconds"
        ]
    );
    assert_eq!(
        held,
        RunHeld::StillOpen {
            line: "waiting on the person to allow curl with vault team; the ask is still \
                   answerable in Asks (✋) in the purlis window, project smart-ide; you will be \
                   told their answer, so run this command again only then"
                .into()
        }
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Ask less** on the ✋'s list (ADR 0121 decision 7, #6010): which asks offer a rule, which
//! grants it suggests first, and which a cool-off holds apart.

use super::*;
use crate::sandboxing::GrantWhat;
use crate::taskblocks::HeldBlock;
use purlis_core::askrules::{Answered, Record};

const NOW: u64 = 1_800_000_000;
const PROJECT: &str = "project-key";

fn host_ask(session: u32) -> Shown {
    let block = HeldBlock {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: "api.example.com".to_owned(),
    };
    sandbox_block(
        session,
        &block,
        &purlis_core::sandbox::policy::Locks::none(),
        &purlis_core::sandbox::services::Declared::default(),
    )
    .expect("a host's ask")
}

fn listing(asks: Vec<Shown>) -> Asking {
    Asking {
        plane: PlaneId::for_tests(std::path::Path::new("/p")),
        asks,
        with_asking_chat: Vec::new(),
        snoozed: Vec::new(),
        snoozed_until: None,
        cooling: Vec::new(),
        cooling_until: None,
    }
}

fn answered(ask: &Shown, session: u32, option: &str, allows: bool, at: u64) -> Answered {
    Answered {
        project: PROJECT.to_owned(),
        session,
        request: request_of(ask).expect("named"),
        option: option.to_owned(),
        allows,
        at,
        raised: None,
    }
}

#[test]
fn a_hosts_ask_is_named_by_its_source_and_host() {
    assert_eq!(
        request_of(&host_ask(1)).as_deref(),
        Some(r#"["sandbox-host","api.example.com"]"#)
    );
    assert_eq!(request_of(&host_ask(1)), request_of(&host_ask(2)));
}

#[test]
fn a_hosts_ask_after_the_same_allow_from_two_chats_suggests_the_project_on_this_machine() {
    let ask = host_ask(3);
    let record = Record {
        answered: vec![
            answered(&ask, 1, "chat", true, NOW - 600),
            answered(&ask, 2, "chat", true, NOW - 60),
        ],
        ..Record::default()
    };
    let listed = with_rules(listing(vec![ask]), &record, PROJECT, NOW);
    let offer = listed.asks[0]
        .rule
        .as_ref()
        .and_then(|rule| rule.offer.clone())
        .expect("a rule offered");
    assert_eq!(offer.times, 2);
    // Every chat here on this machine is the narrowest that would have prevented both; for
    // everyone in the project, committed, is the other.
    assert_eq!(offer.options, ["you", "project"]);
}

#[test]
fn a_hosts_ask_answered_once_offers_no_rule_but_is_named() {
    let ask = host_ask(3);
    let record = Record {
        answered: vec![answered(&ask, 1, "chat", true, NOW - 60)],
        ..Record::default()
    };
    let listed = with_rules(listing(vec![ask]), &record, PROJECT, NOW);
    let rule = listed.asks[0].rule.as_ref().expect("named");
    assert_eq!(rule.offer, None);
}

#[test]
fn an_allow_given_at_the_widest_scope_offers_nothing_wider() {
    let ask = host_ask(3);
    let record = Record {
        answered: vec![
            answered(&ask, 1, "project", true, NOW - 600),
            answered(&ask, 2, "project", true, NOW - 60),
        ],
        ..Record::default()
    };
    let listed = with_rules(listing(vec![ask]), &record, PROJECT, NOW);
    assert_eq!(
        listed.asks[0]
            .rule
            .as_ref()
            .and_then(|one| one.offer.clone()),
        None
    );
}

#[test]
fn a_question_is_never_named_nor_offered_a_rule() {
    let ask = question(1, None, Vec::new());
    assert_eq!(request_of(&ask), None);
    let listed = with_rules(listing(vec![ask]), &Record::default(), PROJECT, NOW);
    assert_eq!(listed.asks[0].rule, None);
}

#[test]
fn a_second_dont_allow_holds_that_chats_ask_apart_until_the_cool_off_ends() {
    let held = host_ask(1);
    let other = host_ask(2);
    let record = Record {
        answered: vec![
            answered(&held, 1, "keep", false, NOW - 600),
            answered(&held, 1, "keep", false, NOW - 60),
        ],
        ..Record::default()
    };
    let listed = with_rules(listing(vec![held, other]), &record, PROJECT, NOW);
    assert_eq!(
        listed
            .asks
            .iter()
            .map(|one| one.session)
            .collect::<Vec<_>>(),
        [2],
        "another chat's same ask is counted"
    );
    assert_eq!(
        listed
            .cooling
            .iter()
            .map(|one| one.session)
            .collect::<Vec<_>>(),
        [1]
    );
    let until = u32::try_from(NOW - 60 + purlis_core::askrules::COOL_FOR_SECS).expect("a time");
    assert_eq!(listed.cooling_until, Some(until));
}

#[test]
fn a_budget_or_loop_pause_is_never_named_offered_a_rule_nor_cooled_off() {
    use crate::pausing::{Paused, Why};
    let budget = Paused {
        why: Why::Budget(purlis_core::dispatchlimits::Reached::Time {
            limit: 30,
            worked: 30,
        }),
        since: 1_000,
        held: vec![1],
    };
    let looping = Paused {
        why: Why::Looping(purlis_core::looping::Why::Failing { times: 6 }),
        since: 1_000,
        held: vec![2],
    };
    let asks = vec![
        pauses::paused(1, &budget, Vec::new(), None),
        pauses::paused(2, &looping, Vec::new(), None),
    ];
    for ask in &asks {
        assert_eq!(request_of(ask), None, "{:?}", ask.source);
    }
    // Stop twice, and Continue twice, as a host's answers would be counted.
    let said = |session: u32, option: &str, allows: bool, at: u64| Answered {
        project: PROJECT.to_owned(),
        session,
        request: format!(r#"["budget","paused:{session}"]"#),
        option: option.to_owned(),
        allows,
        at,
        raised: None,
    };
    let record = Record {
        answered: vec![
            said(1, pauses::STOP, false, NOW - 600),
            said(1, pauses::STOP, false, NOW - 120),
            said(2, pauses::FINE, true, NOW - 600),
            said(2, pauses::FINE, true, NOW - 120),
        ],
        ..Record::default()
    };
    let listed = with_rules(listing(asks), &record, PROJECT, NOW);
    assert_eq!(
        listed
            .asks
            .iter()
            .map(|one| one.session)
            .collect::<Vec<_>>(),
        [1, 2],
        "both still counted on the ✋"
    );
    assert!(listed.cooling.is_empty());
    assert_eq!(listed.cooling_until, None);
    assert!(listed.asks.iter().all(|one| one.rule.is_none()));
    assert!(
        listed
            .asks
            .iter()
            .flat_map(|one| &one.options)
            .all(|one| one.scope.is_none())
    );
}

/// Chat `session`'s ask to run `curl` with `TOKEN` from vault `team`, as persona `devops`: a
/// reviewed program, so it offers Allow for this chat and Always allow (ADR 0112).
fn run_ask(session: u32) -> Shown {
    use purlis_core::secrets::ask::run::{Bound, Hosts, Offer, OpenRun};
    let open = OpenRun {
        ahead: false,
        fills: Vec::new(),
        offer: Offer::Program,
        id: session,
        persona: "devops".to_owned(),
        bound: Bound {
            chat: session,
            name: "curl".to_owned(),
            program: "/usr/bin/curl".into(),
            vault: "team".to_owned(),
            env: vec!["A=TOKEN".to_owned()],
            file: Vec::new(),
            cwd: "/work".into(),
            args: vec!["https://api.example.test".to_owned()],
        },
        hosts: Hosts::Listed(vec!["api.example.test".to_owned()]),
        caution: None,
        not_its_vault: false,
        command: "curl https://api.example.test".to_owned(),
        since: 0,
    };
    run_grant(
        &open,
        &purlis_core::sandbox::policy::Locks::none(),
        false,
        &|_| false,
    )
}

fn offered_options(ask: Shown, record: &Record) -> Option<Vec<String>> {
    let listed = with_rules(listing(vec![ask]), record, PROJECT, NOW);
    listed.asks[0]
        .rule
        .as_ref()
        .and_then(|rule| rule.offer.clone())
        .map(|offer| offer.options)
}

#[test]
fn a_runs_always_allow_is_a_rule_for_this_project() {
    let ask = run_ask(1);
    let scopes: Vec<(&str, Option<RuleScope>)> = ask
        .options
        .iter()
        .map(|one| (one.id.as_str(), one.scope))
        .collect();
    assert_eq!(
        scopes,
        [
            (RUN_ALLOW_ONCE, None),
            (RUN_ALLOW_CHAT, Some(RuleScope::Chat)),
            (RUN_ALLOW_ALWAYS, Some(RuleScope::Project)),
            (KEEP, None),
        ]
    );
}

/// **A run grant that allows a program is a vault use** (#6178): it hands the vault's values to
/// the chat's run, so it is risky and, as every risky ask, is never offered a rule (#6177), what
/// the same Allow was given before. Its own Always allow stays among its answers.
#[test]
fn a_run_grant_that_hands_a_program_vault_values_is_never_offered_a_rule() {
    let ask = run_ask(3);
    assert_eq!(
        ask.risk.as_ref().map(|risk| risk.class),
        Some(RiskClass::VaultUse)
    );
    assert!(ask.options.iter().any(|one| one.id == RUN_ALLOW_ALWAYS));
    let from_two_chats = Record {
        answered: vec![
            answered(&ask, 1, RUN_ALLOW_CHAT, true, NOW - 600),
            answered(&ask, 2, RUN_ALLOW_CHAT, true, NOW - 60),
        ],
        ..Record::default()
    };
    let twice_in_one = Record {
        answered: vec![
            answered(&ask, 3, RUN_ALLOW_ONCE, true, NOW - 600),
            answered(&ask, 3, RUN_ALLOW_ONCE, true, NOW - 60),
        ],
        ..Record::default()
    };
    assert_eq!(offered_options(ask.clone(), &from_two_chats), None);
    assert_eq!(offered_options(ask, &twice_in_one), None);
}

/// The same ask offered a rule unmarked offers none once it carries a risk, of every class.
#[test]
fn an_ask_marked_risky_is_never_offered_a_rule() {
    let ask = host_ask(3);
    let record = Record {
        answered: vec![
            answered(&ask, 1, "chat", true, NOW - 600),
            answered(&ask, 2, "chat", true, NOW - 60),
        ],
        ..Record::default()
    };
    assert!(
        offered_options(ask.clone(), &record).is_some(),
        "unmarked, it is offered a rule"
    );
    for class in [
        RiskClass::ProtectedPush,
        RiskClass::ProductionHost,
        RiskClass::OutsideProject,
        RiskClass::Delete,
        RiskClass::VaultUse,
    ] {
        let mut risky = ask.clone();
        risky.risk = Some(RiskShown {
            class,
            why: "Risky".to_owned(),
            confirm: "Allow it?".to_owned(),
        });
        assert_eq!(offered_options(risky, &record), None, "{class:?}");
    }
}

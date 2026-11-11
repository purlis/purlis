// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
const NOW: u64 = 1_800_000_000;

fn asked(project: &str, session: u32) -> Asked {
    Asked {
        project: project.to_owned(),
        session,
        request: "sandbox-host:api.example.com".to_owned(),
    }
}

fn answered(asked: &Asked, option: &str, allows: bool, at: u64) -> Answered {
    Answered {
        project: asked.project.clone(),
        session: asked.session,
        request: asked.request.clone(),
        option: option.to_owned(),
        allows,
        at,
        raised: None,
    }
}

fn record(answers: Vec<Answered>) -> Record {
    Record {
        answered: answers,
        ..Record::default()
    }
}

#[test]
fn one_allow_offers_no_rule() {
    let first = asked("p", 1);
    let kept = record(vec![answered(&first, "chat", true, NOW - HOUR)]);
    assert_eq!(offer(&kept, &asked("p", 2), NOW), None);
}

#[test]
fn the_same_allow_twice_from_one_chat_offers_a_rule_for_that_chat() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "once", true, NOW - 2 * HOUR),
        answered(&here, "once", true, NOW - HOUR),
    ]);
    assert_eq!(
        offer(&kept, &here, NOW),
        Some(Offer {
            answered: "once".to_owned(),
            times: 2,
            needs: Scope::Chat,
        })
    );
}

#[test]
fn the_same_allow_from_two_chats_of_a_project_needs_the_project() {
    let kept = record(vec![
        answered(&asked("p", 1), "chat", true, NOW - 2 * HOUR),
        answered(&asked("p", 2), "chat", true, NOW - HOUR),
    ]);
    assert_eq!(
        offer(&kept, &asked("p", 3), NOW).map(|one| one.needs),
        Some(Scope::Project)
    );
}

#[test]
fn the_same_allow_from_two_projects_needs_the_machine() {
    let kept = record(vec![
        answered(&asked("p", 1), "chat", true, NOW - 2 * HOUR),
        answered(&asked("q", 1), "chat", true, NOW - HOUR),
    ]);
    assert_eq!(
        offer(&kept, &asked("p", 1), NOW).map(|one| one.needs),
        Some(Scope::Machine)
    );
}

#[test]
fn answers_older_than_the_window_offer_nothing() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "chat", true, NOW - OFFER_WINDOW_SECS - 1),
        answered(&here, "chat", true, NOW - HOUR),
    ]);
    assert_eq!(offer(&kept, &here, NOW), None);
}

#[test]
fn two_different_allows_offer_nothing() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "once", true, NOW - 2 * HOUR),
        answered(&here, "chat", true, NOW - HOUR),
    ]);
    assert_eq!(offer(&kept, &here, NOW), None);
}

#[test]
fn refusals_never_offer_a_rule() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "keep", false, NOW - 2 * HOUR),
        answered(&here, "keep", false, NOW - HOUR),
    ]);
    assert_eq!(offer(&kept, &here, NOW), None);
}

#[test]
fn another_request_counts_for_nothing() {
    let here = asked("p", 1);
    let other = Asked {
        request: "sandbox-host:other.example.com".to_owned(),
        ..here.clone()
    };
    let kept = record(vec![
        answered(&other, "chat", true, NOW - 2 * HOUR),
        answered(&other, "chat", true, NOW - HOUR),
    ]);
    assert_eq!(offer(&kept, &here, NOW), None);
}

#[test]
fn a_rule_made_for_the_request_starts_the_count_again() {
    let here = asked("p", 1);
    let mut kept = record(vec![
        answered(&here, "once", true, NOW - 3 * HOUR),
        answered(&here, "once", true, NOW - 2 * HOUR),
    ]);
    kept.rules.push(rule(&here, NOW - HOUR));
    assert_eq!(offer(&kept, &here, NOW), None);
}

fn rule(asked: &Asked, at: u64) -> Rule {
    Rule {
        id: format!("rule-{at}"),
        at,
        project: asked.project.clone(),
        root: std::path::PathBuf::from("/p"),
        session: asked.session,
        request: asked.request.clone(),
        source: "sandbox-host".to_owned(),
        says: "The sandbox refused api.example.com".to_owned(),
        label: "Allow for me on this machine".to_owned(),
        option: "you".to_owned(),
        scope: Scope::Project,
        grant: None,
        declared: None,
    }
}

#[test]
fn one_dont_allow_starts_no_cool_off() {
    let here = asked("p", 1);
    let kept = record(vec![answered(&here, "keep", false, NOW - 60)]);
    assert_eq!(cooling(&kept, &here, NOW), None);
}

#[test]
fn a_second_dont_allow_from_the_same_chat_starts_a_cool_off() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "keep", false, NOW - 600),
        answered(&here, "keep", false, NOW - 60),
    ]);
    assert_eq!(cooling(&kept, &here, NOW), Some(NOW - 60 + COOL_FOR_SECS));
}

#[test]
fn a_cool_off_holds_only_the_chat_that_heard_dont_allow() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "keep", false, NOW - 600),
        answered(&here, "keep", false, NOW - 60),
    ]);
    assert_eq!(cooling(&kept, &asked("p", 2), NOW), None);
    assert_eq!(cooling(&kept, &asked("q", 1), NOW), None);
}

#[test]
fn a_cool_off_ends_by_itself() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "keep", false, NOW - 600),
        answered(&here, "keep", false, NOW - 60),
    ]);
    assert_eq!(cooling(&kept, &here, NOW - 60 + COOL_FOR_SECS), None);
}

#[test]
fn dont_allows_far_apart_start_no_cool_off() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "keep", false, NOW - COOL_WITHIN_SECS - 120),
        answered(&here, "keep", false, NOW - 60),
    ]);
    assert_eq!(cooling(&kept, &here, NOW), None);
}

#[test]
fn an_allow_after_dont_allow_ends_the_count() {
    let here = asked("p", 1);
    let kept = record(vec![
        answered(&here, "keep", false, NOW - 900),
        answered(&here, "chat", true, NOW - 600),
        answered(&here, "keep", false, NOW - 60),
    ]);
    assert_eq!(cooling(&kept, &here, NOW), None);
}

#[test]
fn the_person_ends_a_cool_off() {
    let here = asked("p", 1);
    let mut kept = record(vec![
        answered(&here, "keep", false, NOW - 600),
        answered(&here, "keep", false, NOW - 60),
    ]);
    kept.ended.push(Ended {
        project: here.project.clone(),
        session: here.session,
        request: here.request.clone(),
        at: NOW - 30,
    });
    assert_eq!(cooling(&kept, &here, NOW), None);
}

#[test]
fn a_store_keeps_answers_rules_and_endings_and_undoes_a_rule() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    let here = asked("p", 1);
    store
        .answered(&here, "once", true, None, NOW - 2 * HOUR)
        .expect("noted");
    store
        .answered(&here, "once", true, None, NOW - HOUR)
        .expect("noted");
    assert_eq!(
        offer(&store.read(), &here, NOW).map(|one| one.times),
        Some(2)
    );

    let made = store.make(rule(&here, NOW)).expect("made");
    assert_eq!(store.read().rules, vec![made.clone()]);
    assert_eq!(store.undo(&made.id).expect("undone"), Some(made));
    assert!(store.read().rules.is_empty());

    store.ended(&here, NOW).expect("ended");
    assert_eq!(store.read().ended.len(), 1);
}

#[test]
fn a_store_lets_go_of_answers_past_the_window() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    let here = asked("p", 1);
    store
        .answered(&here, "once", true, None, NOW - OFFER_WINDOW_SECS - DAY)
        .expect("noted");
    store
        .answered(&here, "once", true, None, NOW)
        .expect("noted");
    assert_eq!(store.read().answered.len(), 1);
}

#[test]
fn an_answer_without_a_request_offers_no_rule_and_starts_no_cool_off() {
    let unnamed = Asked {
        request: String::new(),
        ..asked("p", 1)
    };
    let kept = record(vec![
        answered(&unnamed, "once", true, NOW - 2 * HOUR),
        answered(&unnamed, "once", true, NOW - HOUR),
        answered(&unnamed, "keep", false, NOW - 120),
        answered(&unnamed, "keep", false, NOW - 60),
    ]);
    assert_eq!(offer(&kept, &unnamed, NOW), None);
    assert_eq!(cooling(&kept, &unnamed, NOW), None);
}

#[test]
fn a_store_keeps_when_an_ask_was_raised_but_never_after_its_answer() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    let here = asked("p", 1);
    store
        .answered(&here, "once", true, Some(NOW - 90), NOW)
        .expect("noted");
    store
        .answered(&here, "once", true, Some(NOW + 90), NOW)
        .expect("noted");
    let raised: Vec<_> = store.read().answered.iter().map(|one| one.raised).collect();
    assert_eq!(raised, vec![Some(NOW - 90), None]);
}

#[test]
fn a_file_this_build_cannot_read_reads_as_nothing() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    std::fs::create_dir_all(data.path().join(crate::inboxupdates::DIR)).expect("dir");
    std::fs::write(store.file(), b"not json").expect("written");
    assert_eq!(store.read(), Record::default());
}

#[test]
fn a_rule_made_of_an_always_allow_keeps_its_tool_line_whole_and_never_a_cut_one() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    let here = asked("p", 1);
    let line = r#"curl = { vault = "team", as = "V file", key = "TOKEN" }"#.to_owned();
    let made = store
        .make(Rule {
            declared: Some(Declared {
                persona: "devops".to_owned(),
                line: line.clone(),
            }),
            ..rule(&here, NOW)
        })
        .expect("made");
    assert_eq!(made.declared.map(|one| one.line), Some(line));
    let long = store
        .make(Rule {
            id: "long".to_owned(),
            declared: Some(Declared {
                persona: "devops".to_owned(),
                line: "x".repeat(MOST_LINE + 1),
            }),
            ..rule(&here, NOW)
        })
        .expect("made");
    assert_eq!(long.declared, None);
}

#[test]
fn a_rule_kept_before_tool_lines_were_reads_as_one_with_none() {
    let kept: Rule = serde_json::from_str(
        r#"{"id":"a","at":1,"project":"p","root":"/p","session":1,"request":"r","source":"run-grant","says":"s","label":"l","option":"allow-chat","scope":"chat","grant":null,"saved":0}"#,
    )
    .expect("read");
    assert_eq!(kept.declared, None);
    assert!(!serde_json::to_string(&kept).unwrap().contains("declared"));
}

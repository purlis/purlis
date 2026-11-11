// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::askrules::Answered;
use crate::sandboxblock::record::Chat;

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
/// Noon of a day, so a few hours either way stay on it.
const NOW: u64 = 1_800_000_000 / DAY * DAY + 12 * HOUR;
const HOST: &str = r#"["sandbox-host","api.example.com"]"#;

fn answer(project: &str, at: u64, raised: Option<u64>) -> Answered {
    Answered {
        project: project.to_owned(),
        session: 1,
        request: String::new(),
        option: "once".to_owned(),
        allows: true,
        at,
        raised,
    }
}

fn rule(request: &str, scope: Scope, at: u64) -> Rule {
    Rule {
        id: format!("rule-{at}"),
        at,
        project: "p".to_owned(),
        root: std::path::PathBuf::from("/p"),
        session: 1,
        request: request.to_owned(),
        source: "sandbox-host".to_owned(),
        says: "The sandbox refused api.example.com".to_owned(),
        label: "Allow for me on this machine".to_owned(),
        option: "you".to_owned(),
        scope,
        grant: None,
        declared: None,
    }
}

fn reached(session: u32, target: &str, by: &str, at: u64) -> Entry {
    Entry::connected(
        Some(target),
        by,
        3,
        Chat {
            id: None,
            name: None,
            session: Some(session),
        },
        None,
        at,
    )
}

#[test]
fn the_asks_of_the_week_are_the_projects_answers_of_the_last_seven_days() {
    let record = Record {
        answered: vec![
            answer("p", NOW - HOUR, None),
            answer("p", NOW - 6 * DAY, None),
            answer("p", NOW - 8 * DAY, None),
            answer("q", NOW - HOUR, None),
        ],
        ..Record::default()
    };
    assert_eq!(measures(&record, "p", &[], NOW).asks, 2);
    assert_eq!(measures(&record, "q", &[], NOW).asks, 1);
    assert_eq!(measures(&record, "r", &[], NOW), Measures::default());
}

#[test]
fn the_time_to_answer_is_counted_from_when_the_ask_was_raised() {
    let record = Record {
        answered: vec![
            answer("p", NOW - HOUR, Some(NOW - HOUR - 30)),
            // A timed-out ask answered late counts from when it was raised (#6007).
            answer("p", NOW - 2 * HOUR, Some(NOW - 3 * HOUR)),
            answer("p", NOW - 3 * HOUR, Some(NOW - 3 * HOUR - 90)),
            // Raised at a time its source did not know: counted as an ask, not as a wait.
            answer("p", NOW - 4 * HOUR, None),
        ],
        ..Record::default()
    };
    let week = measures(&record, "p", &[], NOW);
    assert_eq!(week.asks, 4);
    assert_eq!(week.median_secs, Some(90));
}

#[test]
fn the_median_of_an_even_count_is_the_mean_of_the_middle_two() {
    assert_eq!(median(vec![]), None);
    assert_eq!(median(vec![7]), Some(7));
    assert_eq!(median(vec![40, 10, 20, 30]), Some(25));
}

#[test]
fn a_host_rule_saves_one_ask_a_day_for_each_other_chat_it_let_through() {
    let made = NOW - 2 * DAY;
    let rule = rule(HOST, Scope::Project, made);
    let lines = vec![
        // Chat 2 reached it twice on one day and once on the next: two asks it would have had.
        reached(2, "api.example.com:443", "you", made + HOUR),
        reached(2, "api.example.com:443", "you", made + 2 * HOUR),
        reached(2, "api.example.com:443", "you", made + DAY),
        // Chat 3, once.
        reached(3, "api.example.com:443", "project", made + HOUR),
    ];
    assert_eq!(saved_by(&rule, &lines, 0), 3);
}

#[test]
fn a_rule_saves_nothing_before_it_was_made_nor_for_its_own_chat_that_day() {
    let made = NOW - 2 * DAY;
    let rule = rule(HOST, Scope::Project, made);
    let lines = vec![
        reached(2, "api.example.com:443", "you", made - HOUR),
        reached(1, "api.example.com:443", "you", made + 60),
        // Its own chat, a day later: an ask its own answer no longer covered.
        reached(1, "api.example.com:443", "you", made + DAY),
    ];
    assert_eq!(saved_by(&rule, &lines, 0), 1);
}

#[test]
fn a_rule_saves_only_what_its_own_scope_let_through_to_its_own_host() {
    let made = NOW - 2 * DAY;
    let lines = vec![
        reached(2, "api.example.com:443", "chat", made + HOUR),
        reached(3, "api.example.com:443", "open", made + HOUR),
        reached(4, "api.example.com:443", "refused", made + HOUR),
        reached(5, "other.example.com:443", "you", made + HOUR),
    ];
    assert_eq!(saved_by(&rule(HOST, Scope::Project, made), &lines, 0), 0);
    assert_eq!(saved_by(&rule(HOST, Scope::Chat, made), &lines, 0), 1);
}

#[test]
fn a_rule_of_anything_but_a_host_saves_nothing_known() {
    let made = NOW - 2 * DAY;
    let folder = rule(r#"["sandbox-write","/tmp/x"]"#, Scope::Project, made);
    let lines = vec![reached(2, "api.example.com:443", "you", made + HOUR)];
    assert_eq!(saved_by(&folder, &lines, 0), 0);
    assert_eq!(
        saved_by(&rule("not json", Scope::Project, made), &lines, 0),
        0
    );
}

#[test]
fn the_week_counts_only_the_asks_its_rules_saved_in_the_last_seven_days() {
    let made = NOW - 20 * DAY;
    let record = Record {
        rules: vec![rule(HOST, Scope::Project, made)],
        ..Record::default()
    };
    let lines = vec![
        reached(2, "api.example.com:443", "you", NOW - 10 * DAY),
        reached(2, "api.example.com:443", "you", NOW - DAY),
        reached(3, "api.example.com:443", "you", NOW - HOUR),
    ];
    assert_eq!(measures(&record, "p", &lines, NOW).saved, 2);
    assert_eq!(measures(&record, "q", &lines, NOW).saved, 0);
}

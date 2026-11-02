// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::skills::grade::{Arm, Check, OutcomeRun, Stat};
use purlis_session_protocol::session::GradeEndTold;

fn plan() -> GradePlanTold {
    GradePlanTold {
        profile: Some("work".into()),
        model: None,
        sandbox: Some("Full".into()),
        refused: None,
        chats: 6,
        token_cap: 200_000,
        limit_micro_usd: 5_000_000,
        running: None,
        graded: Some("a".into()),
        consent: Some("b".into()),
    }
}

#[test]
fn a_command_in_a_chat_is_known_by_its_number_its_socket_or_its_sandbox() {
    let with = |name: &'static str, value: &'static str| {
        move |asked: &str| (asked == name).then(|| value.to_owned())
    };
    assert!(in_a_chat(&with(purlis_core::hookwire::CHAT_ENV, "7")));
    assert!(in_a_chat(&with(SOCKET_ENV, "/tmp/hooks.sock")));
    assert!(in_a_chat(&with(purlis_core::hookwire::SANDBOXED_ENV, "1")));
    assert!(!in_a_chat(&|_: &str| None));
    assert!(!in_a_chat(&with(SOCKET_ENV, "")), "an empty one names none");
}

#[test]
fn only_a_yes_is_a_yes() {
    for yes in ["y\n", "Y\n", "yes\n", " YES \n"] {
        assert!(is_yes(yes), "{yes}");
    }
    for no in ["", "\n", "n\n", "yeah\n", "y y\n"] {
        assert!(!is_yes(no), "{no}");
    }
}

#[test]
fn the_terms_a_yes_covers_are_printed_with_their_limits() {
    let said = terms_said(&plan());
    for word in [
        "6 chats",
        "work",
        "the profile's own",
        "Full",
        "200000",
        "$5.00",
    ] {
        assert!(said.contains(word), "{said}");
    }
}

#[test]
fn a_plan_that_cannot_start_says_why_before_a_yes_is_asked() {
    assert_eq!(can_start(&plan()), Ok(()));
    let no_sandbox = GradePlanTold {
        refused: Some("No profile.".into()),
        ..plan()
    };
    assert!(can_start(&no_sandbox).unwrap_err().contains("No profile."));
    let running = GradePlanTold {
        running: Some("g".into()),
        ..plan()
    };
    assert!(can_start(&running).unwrap_err().contains("running"));
}

#[test]
fn a_named_eval_takes_its_yes_only_from_a_person_at_a_terminal() {
    assert!(yes_can_be_taken(true, false, true));
    assert!(!yes_can_be_taken(true, true, true), "--yes beside --eval");
    assert!(!yes_can_be_taken(true, false, false), "a piped y");
    assert!(!yes_can_be_taken(true, true, false));
}

#[test]
fn a_grade_of_trigger_tests_alone_keeps_its_yes_for_a_script() {
    assert!(yes_can_be_taken(false, true, false));
    assert!(yes_can_be_taken(false, false, false));
}

#[test]
fn the_scorecard_says_each_outcome_eval_in_each_arm_and_why_a_run_failed() {
    let run = |passed: &[bool], failed: Option<&str>| OutcomeRun {
        chat: None,
        judge: None,
        passed: None,
        checks: passed
            .iter()
            .map(|passed| Check {
                passed: *passed,
                evidence: String::new(),
            })
            .collect(),
        failed: failed.map(str::to_owned),
        asked: false,
    };
    let said = outcomes_said(&[
        OutcomeScore {
            eval: 4,
            arm: Arm::With,
            expectations: vec!["It ships".into(), "It is dated".into()],
            runs: 3,
            pass_rate: Some(Stat {
                mean: 2.0 / 3.0,
                stddev: 0.0,
                min: 0.0,
                max: 1.0,
            }),
            results: vec![
                run(&[true, false], None),
                run(&[true, false], None),
                run(&[], Some("The judge's answer was not whole.")),
            ],
        },
        OutcomeScore {
            eval: 4,
            arm: Arm::Without,
            expectations: vec!["It ships".into()],
            runs: 0,
            pass_rate: None,
            results: vec![run(&[], None)],
        },
    ]);
    assert_eq!(
        said,
        "  eval 4 with     67% passed over 3 runs\n\
         \x20   failed in 2 runs: It is dated\n\
         \x20   The judge's answer was not whole.\n\
         \x20 eval 4 without  not judged over 0 runs\n"
    );
}

#[test]
fn a_grade_stopped_short_says_what_stopped_it() {
    let ended = |ended, said: Option<&str>| GradeStepTold {
        grade: "g".into(),
        ran: 4,
        of: 18,
        tokens: None,
        cost_micro_usd: None,
        asked: 0,
        ended: Some(ended),
        said: said.map(str::to_owned),
    };
    let said = |step| end_said(&step, 200_000).unwrap();
    assert_eq!(
        said(ended(GradeEndTold::OverCap, None)),
        "Stopped at its cap of 200000 tokens after 4 chats. What ran is scored."
    );
    assert_eq!(
        said(ended(GradeEndTold::OverLimit, None)),
        "Stopped at its dollar limit after 4 chats. What ran is scored."
    );
    assert!(said(ended(GradeEndTold::SpendNotReported, None)).contains("did not say"));
    assert_eq!(
        end_said(&ended(GradeEndTold::Whole, None), 200_000),
        None,
        "its scorecard follows"
    );
    assert_eq!(
        said(ended(GradeEndTold::Failed, Some("A fault."))),
        "A fault."
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A grade runs each of a built-in's trigger tests three times among the crowd and three times
//! bare, holds the crowded score to the skill's trigger bar, and says both scores with the
//! harness, its version and the model in the run's summary.

use crate::fixture::fixture;
use skill_grader::{Graded, grade};

#[test]
fn a_skill_that_starts_where_it_should_meets_its_bar_and_the_summary_says_so() {
    let fixture = fixture(true, true);
    let Graded::Done {
        passed, summary, ..
    } = grade(&fixture.ask(Some("sk-test")))
    else {
        panic!("it graded nothing");
    };
    assert!(passed, "{summary}");
    for said in [
        "purlis-secrets",
        "strict",
        "Claude Code",
        // What the runs said they ran as, not the pin.
        "2.3.4",
        "claude-sonnet-5-5",
        "| crowded | 100% (2 of 2) | 0% (0 of 2) |",
        "| bare | 100% (2 of 2) | 0% (0 of 2) |",
        "meets its strict trigger bar",
    ] {
        assert!(summary.contains(said), "no {said:?} in:\n{summary}");
    }
    let argv = std::fs::read_to_string(&fixture.argv).unwrap();
    let runs = argv.lines().filter(|word| *word == "-p").count();
    // Four tests, three runs each, crowded and bare.
    assert_eq!(runs, 4 * 3 * 2);
    assert!(!argv.contains("sk-test"), "the key reached a command line");
}

#[test]
fn a_skill_the_crowd_hides_is_below_its_bar_and_the_bare_score_shows_it_is_crowding() {
    let fixture = fixture(false, true);
    let Graded::Done {
        passed, summary, ..
    } = grade(&fixture.ask(Some("sk-test")))
    else {
        panic!("it graded nothing");
    };
    assert!(!passed, "{summary}");
    for said in [
        "| crowded | 0% (0 of 2) | 0% (0 of 2) |",
        "| bare | 100% (2 of 2) | 0% (0 of 2) |",
        "is below its strict trigger bar",
        "\"run the migration with the database password from the vault\" (missed)",
    ] {
        assert!(summary.contains(said), "no {said:?} in:\n{summary}");
    }
}

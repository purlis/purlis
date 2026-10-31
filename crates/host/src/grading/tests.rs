// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn started() -> GradeAnswer {
    GradeAnswer::Started {
        grade: "g".to_owned(),
    }
}

#[test]
fn a_grade_started_for_a_terminal_that_went_is_stopped_as_its_answer_comes() {
    let mut asked = GradeAsks::default();
    let _answer = asked.asked(1, 10, 20, true);
    assert_eq!(asked.link_gone(10), Gone::default());
    assert_eq!(asked.answered(1, 20, started()), Some("g".to_owned()));
}

#[test]
fn a_terminal_going_stops_the_grade_it_follows_and_only_that() {
    let mut asked = GradeAsks::default();
    let _one = asked.asked(1, 10, 20, true);
    let _two = asked.asked(2, 11, 20, true);
    assert_eq!(asked.answered(1, 20, started()), None);
    assert_eq!(asked.answered(2, 20, started()), None);
    assert_eq!(
        asked.link_gone(10).to_stop,
        vec![(20, "g".to_owned())],
        "the other terminal's stays"
    );
    assert_eq!(asked.step(2, 20, false), Some(11));
}

#[test]
fn a_window_going_tells_the_terminal_following_its_grade() {
    let mut asked = GradeAsks::default();
    let _answer = asked.asked(1, 10, 20, true);
    asked.answered(1, 20, started());
    assert_eq!(asked.link_gone(20).to_tell, vec![(10, "g".to_owned())]);
}

#[tokio::test]
async fn only_the_link_asked_answers_or_tells_a_step() {
    let mut asked = GradeAsks::default();
    let answer = asked.asked(1, 10, 20, true);
    assert_eq!(asked.step(1, 21, false), None);
    asked.answered(
        1,
        21,
        GradeAnswer::Refused {
            why: "another window".to_owned(),
        },
    );
    asked.answered(1, 20, started());
    assert_eq!(answer.await.unwrap(), started());
}

#[tokio::test]
async fn a_grade_that_ends_before_its_answer_comes_is_still_answered() {
    let mut asked = GradeAsks::default();
    let answer = asked.asked(1, 10, 20, true);
    assert_eq!(asked.step(1, 20, true), Some(10));
    assert_eq!(asked.answered(1, 20, started()), None, "nothing to stop");
    assert_eq!(answer.await.unwrap(), started());
    assert_eq!(asked.link_gone(10), Gone::default());
}

#[test]
fn a_grades_end_lets_it_go() {
    let mut asked = GradeAsks::default();
    let _answer = asked.asked(1, 10, 20, true);
    asked.answered(1, 20, started());
    assert_eq!(asked.step(1, 20, true), Some(10));
    assert_eq!(asked.step(1, 20, false), None);
    assert_eq!(asked.link_gone(10), Gone::default());
}

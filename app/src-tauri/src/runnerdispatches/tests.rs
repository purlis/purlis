// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner chat's open or dispatch, held for the person: what its row says and offers.

use super::*;

fn a_task(chat: u32, name: &str, brief: &str) -> Wanted {
    Wanted {
        chat,
        to: None,
        name: name.to_owned(),
        brief: brief.to_owned(),
        profile: None,
        place: None,
        by: purlis_core::dispatchdecision::By::Chat,
        moved: None,
        long: false,
    }
}

#[test]
fn its_row_names_the_pool_the_persona_and_the_brief_and_offers_open_or_refuse() {
    let held = RunnerDispatches::default();
    let id = held
        .note(
            "builds",
            Some("devops".to_owned()),
            a_task(4, "check prod", "look at the cluster"),
            100,
        )
        .expect("held");
    let one = &held.every()[0];
    assert_eq!(
        one.to(),
        Some("devops"),
        "the chat's own persona, named none"
    );

    let shown = crate::asking::runner_dispatch(one);
    assert_eq!(
        shown.says,
        "Asks, from pool builds, to start the task 'check prod' as devops on this machine: look \
         at the cluster"
    );
    let offered: Vec<(&str, bool)> = shown
        .options
        .iter()
        .map(|one| (one.id.as_str(), one.allows))
        .collect();
    assert_eq!(
        offered,
        vec![(crate::asking::OPEN, false), (crate::asking::REFUSE, false)],
        "never Approve on a row that cuts the brief"
    );
    assert_eq!(
        shown.answer,
        crate::asking::AnswerPath::RunnerDispatch { id }
    );
    assert_eq!(shown.session, 4);
}

#[test]
fn its_row_draws_a_brief_that_hides_or_reverses_characters_inertly() {
    let held = RunnerDispatches::default();
    held.note("builds", None, a_task(4, "t", "run\u{202e}gol\nnext"), 100)
        .expect("held");
    let shown = crate::asking::runner_dispatch(&held.every()[0]);
    assert!(!shown.says.contains('\u{202e}'), "{}", shown.says);
    assert!(!shown.says.contains('\n'), "{}", shown.says);
}

#[test]
fn a_chat_holds_at_most_eight_and_its_close_takes_them() {
    let held = RunnerDispatches::default();
    for _ in 0..AT_MOST_PER_CHAT {
        assert!(
            held.note("builds", None, a_task(4, "t", "b"), 100)
                .is_some()
        );
    }
    assert!(
        held.note("builds", None, a_task(4, "t", "b"), 100)
            .is_none()
    );
    assert!(
        held.note("builds", None, a_task(5, "t", "b"), 100)
            .is_some()
    );
    held.forget(4);
    assert_eq!(held.every().len(), 1);
}

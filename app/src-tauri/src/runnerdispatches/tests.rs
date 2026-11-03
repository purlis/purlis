// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner chat's open or dispatch, held for the person: what its row says and offers.

use super::*;
use purlis_core::machine::StartChoice;

fn a_task(chat: u32, name: &str, brief: &str) -> Wanted {
    Wanted {
        runs_on: None,
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
            Some("builds"),
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
        "Asks, from pool builds, to start the task 'check prod' as devops: look \
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
    held.note(
        Some("builds"),
        None,
        a_task(4, "t", "run\u{202e}gol\nnext"),
        100,
    )
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
            held.note(Some("builds"), None, a_task(4, "t", "b"), 100)
                .is_some()
        );
    }
    assert!(
        held.note(Some("builds"), None, a_task(4, "t", "b"), 100)
            .is_none()
    );
    assert!(
        held.note(Some("builds"), None, a_task(5, "t", "b"), 100)
            .is_some()
    );
    held.forget(4);
    assert_eq!(held.every().len(), 1);
}

/// A task of chat `chat` that names pool `pool` to run on: held because it moves work (#4536).
fn a_move(chat: u32, pool: &str) -> Wanted {
    Wanted {
        runs_on: Some(StartChoice::Pool(pool.to_owned())),
        ..a_task(chat, "t", "b")
    }
}

/// **A refused move isn't asked again by the same chat** (#4862): once the person refuses a
/// chat's dispatch to a place, that chat's later dispatches to that place are refused without
/// asking, while it runs. Other places and other chats still ask.
#[test]
fn a_refused_move_is_refused_again_for_its_chat_and_place_alone_while_the_chat_runs() {
    let held = RunnerDispatches::default();
    let builds = StartChoice::Pool("builds".to_owned());
    let id = held
        .note(None, None, a_move(4, "builds"), 100)
        .expect("held");
    assert!(
        !held.refused_before(4, &builds),
        "not before the person refuses"
    );

    held.refuse(id).expect("it was held");

    assert!(held.refused_before(4, &builds));
    assert!(!held.refused_before(4, &StartChoice::Pool("tests".to_owned())));
    assert!(!held.refused_before(4, &StartChoice::ThisMachine));
    assert!(!held.refused_before(5, &builds), "another chat still asks");
    held.forget(4);
    assert!(!held.refused_before(4, &builds), "it ends with the chat");
}

/// Only the person's Refuse of a move is kept: an Approve keeps nothing, and a refused ask of a
/// chat on a pool's runner is held for another reason, so it is no refusal of a place.
#[test]
fn only_a_refused_move_is_kept_never_an_approved_one_or_a_pool_chats() {
    let held = RunnerDispatches::default();
    let builds = StartChoice::Pool("builds".to_owned());
    let approved = held
        .note(None, None, a_move(4, "builds"), 100)
        .expect("held");
    held.take(approved).expect("it was held");
    assert!(!held.refused_before(4, &builds));

    let on_a_pool = held
        .note(Some("builds"), None, a_move(4, "builds"), 100)
        .expect("held");
    held.refuse(on_a_pool).expect("it was held");
    assert!(!held.refused_before(4, &builds));
}

/// What a chat is told when the person refuses its move says, in words, that it won't be asked
/// again for that place.
#[test]
fn a_refused_moves_chat_is_told_it_is_not_asked_again_for_that_place() {
    let held = RunnerDispatches::default();
    let id = held
        .note(None, None, a_move(4, "builds"), 100)
        .expect("held");
    let said = refused_said(&held.refuse(id).expect("it was held"));
    assert!(said.starts_with("the person refused it"), "{said}");
    assert!(said.contains("pool builds"), "{said}");
    assert!(
        said.contains("refused from now on without asking the person"),
        "{said}"
    );
}

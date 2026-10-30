// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for grading after each edit (SK-58, #3238): the opt-in kept on this machine, and what
//! a saved edit does under it.

use super::*;

fn accepted() -> Accepted {
    Accepted {
        profile: "work".into(),
        model: Some("sonnet".into()),
        chats: 12,
        token_cap: 200_000,
        limit_usd: 3.0,
        tests: TESTS.into(),
        previous: None,
        runs_on: Some("sonnet".into()),
        at: "2026-10-28T10:00:00+00:00".into(),
        graded: None,
        paused: None,
    }
}

const CONSENT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const TESTS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn now(chats: u32, token_cap: u64, limit_usd: f64) -> Now {
    Now {
        profile: "work".into(),
        runs_on: Some("sonnet".into()),
        chats,
        token_cap,
        limit_usd,
        tests: TESTS.into(),
        previous: None,
        consent: CONSENT.into(),
    }
}

/// A project folder whose state is its own: `PURLIS_HOME` is never read in a test that names
/// the folder it writes to, so the state is `<root>/.purlis` or `<root>/.charter`.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    tmp
}

#[test]
fn it_is_off_for_every_skill_until_it_is_turned_on() {
    let project = project();
    let root = project.path();
    assert_eq!(of(root, &Level::project(), "deploy"), None);

    turn_on(root, &Level::project(), "deploy", accepted()).unwrap();

    assert_eq!(of(root, &Level::project(), "deploy"), Some(accepted()));
    assert_eq!(of(root, &Level::project(), "ship"), None, "per skill");
    turn_off(root, &Level::project(), "deploy").unwrap();
    assert_eq!(of(root, &Level::project(), "deploy"), None);
}

#[test]
fn it_is_kept_in_this_machines_state_never_in_the_project() {
    let project = project();
    let root = project.path();
    turn_on(root, &Level::project(), "deploy", accepted()).unwrap();
    let file = crate::appstate::dir(root).unwrap().join(FILE);
    assert!(file.is_file(), "{}", file.display());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn a_record_that_does_not_hold_together_reads_as_off() {
    let project = project();
    let root = project.path();
    turn_on(root, &Level::project(), "deploy", accepted()).unwrap();
    let file = crate::appstate::dir(root).unwrap().join(FILE);
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("\"chats\": 12", "\"chats\": 0")).unwrap();
    assert_eq!(of(root, &Level::project(), "deploy"), None);
    std::fs::write(&file, "not json").unwrap();
    assert_eq!(of(root, &Level::project(), "deploy"), None);
    let refused = Accepted {
        limit_usd: f64::NAN,
        ..accepted()
    };
    assert!(turn_on(root, &Level::project(), "deploy", refused).is_err());
}

#[test]
fn an_edit_grades_within_what_was_accepted() {
    assert_eq!(next(&accepted(), &now(12, 200_000, 3.0)), Next::Grade);
    assert_eq!(
        next(&accepted(), &now(6, 100_000, 1.5)),
        Next::Grade,
        "less is within it"
    );
}

#[test]
fn a_grade_that_would_run_more_chats_waits_for_the_person_to_see_the_cost_again() {
    let Next::Waits(why) = next(&accepted(), &now(18, 200_000, 3.0)) else {
        panic!("it graded past what was accepted");
    };
    assert!(
        why.contains("18 chats, more than the 12 you accepted"),
        "{why}"
    );
}

#[test]
fn a_grade_that_may_spend_more_waits_for_the_person_too() {
    let Next::Waits(why) = next(&accepted(), &now(12, 400_000, 6.0)) else {
        panic!("it graded past the limit accepted");
    };
    assert!(why.contains("up to $6.00"), "{why}");
    assert!(why.contains("more than the $3.00 you accepted"), "{why}");
    assert!(matches!(
        next(&accepted(), &now(12, 200_000, 3.5)),
        Next::Waits(_)
    ));
}

#[test]
fn the_same_copy_is_never_graded_twice_and_a_waiting_opt_in_stays_waiting() {
    let graded = Accepted {
        graded: Some(CONSENT.into()),
        ..accepted()
    };
    assert_eq!(next(&graded, &now(12, 200_000, 3.0)), Next::Same);
    let waits = Accepted {
        paused: Some("waits".into()),
        ..accepted()
    };
    assert_eq!(
        next(&waits, &now(1, 200_000, 1.0)),
        Next::Waits("waits".into())
    );
}

#[test]
fn what_an_edit_graded_and_why_it_waits_are_kept() {
    let project = project();
    let root = project.path();
    turn_on(root, &Level::project(), "deploy", accepted()).unwrap();
    update(root, &Level::project(), "deploy", |one| {
        one.graded = Some(CONSENT.into());
        one.paused = Some("waits".into());
    })
    .unwrap();
    let kept = of(root, &Level::project(), "deploy").unwrap();
    assert_eq!(kept.graded.as_deref(), Some(CONSENT));
    assert_eq!(kept.paused.as_deref(), Some("waits"));
    // An update of a copy that is off turns nothing on.
    update(root, &Level::project(), "ship", |one| one.paused = None).unwrap();
    assert_eq!(of(root, &Level::project(), "ship"), None);
}

#[test]
fn other_trigger_tests_or_another_version_compared_with_wait_for_the_person() {
    // The review of #3238: a chat can write a fixture or make a commit; neither runs unseen.
    let tests = Now {
        tests: CONSENT.into(),
        ..now(12, 200_000, 3.0)
    };
    assert!(matches!(next(&accepted(), &tests), Next::Waits(why) if why.contains("trigger tests")));
    let version = Now {
        previous: Some("a".repeat(40)),
        ..now(12, 200_000, 3.0)
    };
    assert!(
        matches!(next(&accepted(), &version), Next::Waits(why) if why.contains("compares with"))
    );
}

#[test]
fn a_limit_past_any_grades_is_not_kept() {
    let project = project();
    let wild = Accepted {
        limit_usd: 1.0e9,
        ..accepted()
    };
    assert!(turn_on(project.path(), &Level::project(), "deploy", wild).is_err());
}

#[test]
fn a_changed_profile_model_cap_or_limit_pauses_the_opt_in() {
    // #3622: an edit grades only on the terms the plan showed when it was turned on.
    let changed = [
        Now {
            profile: "home".into(),
            ..now(12, 200_000, 3.0)
        },
        Now {
            runs_on: Some("opus".into()),
            ..now(12, 200_000, 3.0)
        },
        Now {
            runs_on: None,
            ..now(12, 200_000, 3.0)
        },
        now(12, 400_000, 3.0),
        now(12, 200_000, 3.5),
    ];
    for one in changed {
        assert!(
            matches!(next(&accepted(), &one), Next::Waits(_)),
            "{one:?} graded past what was accepted"
        );
    }
    assert_eq!(next(&accepted(), &now(12, 200_000, 3.0)), Next::Grade);
}

#[test]
fn an_opt_in_in_the_plane_s_state_folder_is_never_read() {
    // #3718: the opt-in is read from this machine's app state alone.
    let project = project();
    let root = project.path();
    turn_on(root, &Level::project(), "deploy", accepted()).unwrap();
    crate::appstate::put_in_the_plane(root, FILE);
    assert_eq!(of(root, &Level::project(), "deploy"), None);
}

#[test]
fn a_rename_or_a_move_carries_the_opt_in_and_a_delete_drops_it() {
    let project = project();
    let root = project.path();
    let ops = Level::workspace("ops");
    turn_on(root, &Level::project(), "deploy", accepted()).unwrap();

    moved(root, &Level::project(), "deploy", &Level::project(), "ship").unwrap();
    assert_eq!(of(root, &Level::project(), "deploy"), None);
    assert_eq!(of(root, &Level::project(), "ship"), Some(accepted()));

    moved(root, &Level::project(), "ship", &ops, "ship").unwrap();
    assert_eq!(of(root, &Level::project(), "ship"), None);
    assert_eq!(
        of(root, &ops, "ship"),
        Some(accepted()),
        "what was accepted goes along unchanged, and `next` holds a grade to it"
    );

    dropped(root, &ops, "ship").unwrap();
    assert_eq!(of(root, &ops, "ship"), None);
}

#[test]
fn a_change_of_a_skill_never_graded_after_each_edit_keeps_no_state() {
    let project = project();
    let root = project.path();

    moved(root, &Level::project(), "deploy", &Level::project(), "ship").unwrap();
    dropped(root, &Level::project(), "deploy").unwrap();

    assert!(
        crate::appstate::dir(root).is_none_or(|dir| !dir.join(FILE).exists()),
        "nothing written"
    );
    assert_eq!(of(root, &Level::project(), "ship"), None);
}

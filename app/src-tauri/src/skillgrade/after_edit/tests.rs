// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What turning it on from `planned`, its chats on `on(None)`, consents to (#3622).
fn shown(planned: &grades::Planned) -> String {
    super::super::consent_shown(planned, &on(None)).unwrap()
}

/// Where a plan's chats run: on `work`, at the dearest price, or refused with `why`.
fn on(why: Option<&str>) -> On {
    On {
        profile: Some("work".into()),
        checked: why.map_or(Ok("full sandbox".into()), |why| Err(why.to_owned())),
        priced: grades::price::of_model(None),
        models: vec!["sonnet".into()],
        model: Some("sonnet".into()),
    }
}

#[test]
fn turning_it_on_accepts_what_the_plan_showed_and_nothing_else() {
    let planned = super::super::tests::planned("ship it");
    let accepted = accepted_of(&planned, on(None), &shown(&planned), "deploy").unwrap();
    assert_eq!(
        accepted.runs_on.as_deref(),
        Some("sonnet"),
        "the model its chats run on"
    );
    assert_eq!(accepted.profile, "work");
    assert_eq!(accepted.model.as_deref(), Some("sonnet"));
    assert_eq!(accepted.chats, planned.plan.unwrap().chats);
    assert_eq!(accepted.token_cap, 50_000);
    assert_eq!(
        accepted.limit_usd,
        planned.cap.cost_usd(&grades::price::of_model(None)),
        "the dollar limit the plan showed"
    );
    assert_eq!((accepted.graded, accepted.paused), (None, None));
}

#[test]
fn it_is_turned_on_only_from_a_plan_that_still_holds_and_can_run() {
    let planned = super::super::tests::planned("ship it");
    let stale = accepted_of(&planned, on(None), "another", "deploy").unwrap_err();
    assert!(stale.contains("changed since it was shown"), "{stale}");
    let refused = accepted_of(
        &planned,
        on(Some("the sandbox is off")),
        &shown(&planned),
        "deploy",
    );
    assert_eq!(refused.unwrap_err(), "the sandbox is off");
    let nothing = grades::Planned {
        plan: None,
        ..planned.clone()
    };
    assert!(accepted_of(&nothing, on(None), &shown(&nothing), "deploy").is_err());
    // A plan that shows another profile, model or limit is another consent (#3622).
    for other in [
        On {
            profile: Some("home".into()),
            ..on(None)
        },
        On {
            model: Some("opus".into()),
            ..on(None)
        },
    ] {
        let refused = accepted_of(&planned, other, &shown(&planned), "deploy").unwrap_err();
        assert!(refused.contains("changed since it was shown"), "{refused}");
    }
    let mut capped = planned.clone();
    capped.cap.tokens = 400_000;
    assert!(accepted_of(&capped, on(None), &shown(&planned), "deploy").is_err());
}

#[test]
fn a_burst_of_saves_grades_once_quiet_follows_its_last_save() {
    let first = Instant::now();
    let mut burst = Burst::at(first);
    assert_eq!(burst.left(first), QUIET);
    burst.saved(first + Duration::from_secs(4));
    assert_eq!(
        burst.left(first + Duration::from_secs(5)),
        QUIET - Duration::from_secs(1),
        "each save puts the grade off until the saves go quiet"
    );
    // A save heard out of order never brings the grade forward.
    burst.saved(first);
    assert_eq!(burst.left(first + Duration::from_secs(14)), Duration::ZERO);
}

#[test]
fn turning_it_on_and_off_are_the_windows_alone() {
    use purlis_session_protocol::ui;
    assert!(ui::window_only("skill_grade_after_edit_on"));
    assert!(ui::window_only("skill_grade_after_edit_off"));
    assert!(!ui::window_only("skill_grade_after_edit"));
}

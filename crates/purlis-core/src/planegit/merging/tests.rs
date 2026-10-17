// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn lint() -> FailedCheck {
    FailedCheck::of(Some("lint"), Some("https://github.com/acme/api/runs/2"))
}

#[test]
fn each_answer_of_the_forge_is_one_state_and_no_answer_waits() {
    let state = |looked: Result<Ready, ForgeError>| Merging::of(&looked).state;
    assert_eq!(state(Ok(Ready::Merged)), MergeState::Merged);
    assert_eq!(state(Ok(Ready::Waiting("x".into()))), MergeState::Waiting);
    assert_eq!(state(Ok(Ready::Red(vec![lint()]))), MergeState::Red);
    assert_eq!(state(Ok(Ready::Queued("x".into()))), MergeState::Queued);
    assert_eq!(
        state(Ok(Ready::Refused("x".into()))),
        MergeState::CannotMerge
    );
    assert_eq!(
        state(Err(ForgeError::new("HTTP 502".to_string()))),
        MergeState::Waiting
    );
}

#[test]
fn a_look_the_forge_would_not_answer_for_want_of_a_sign_in_says_so() {
    // The app's own look asks as a trigger, which takes no sign-in of the person's
    // (ADR 0070 §5): with no forge CLI signed in, it must say why nothing happens.
    let refused = ForgeError::of(
        crate::forge::Failure::Auth,
        "gh: To get started with GitHub CLI, please run: gh auth login",
    );
    let waits = Merging::of(&Err(refused));
    assert_eq!(waits.state, MergeState::Waiting);
    assert!(waits.why.contains("has no sign-in"), "{}", waits.why);
    assert!(waits.why.contains("save again"), "{}", waits.why);
    assert_eq!(waits.needs, None, "what `of` alone cannot know");
    let no_cli = Merging::of(&Err(ForgeError::transport("gh is not installed")));
    assert!(
        no_cli.why.contains("could not reach the forge"),
        "{}",
        no_cli.why
    );
}

#[test]
fn a_wait_nothing_purlis_reads_will_end_says_it_is_the_persons_to_end() {
    let stalled = Merging::of(&Ok(Ready::Stalled("no check has started on it".into())));
    assert_eq!(
        (stalled.state, stalled.needs),
        (MergeState::Waiting, Some(Needs::Person))
    );
    // A wait that ends by itself, and every other state, names nothing.
    for looked in [
        Ok(Ready::Waiting("its checks are still running".into())),
        Ok(Ready::Red(vec![lint()])),
        Ok(Ready::Refused("it conflicts".into())),
        Ok(Ready::Queued("in the queue".into())),
        Ok(Ready::Merged),
    ] {
        assert_eq!(Merging::of(&looked).needs, None, "{looked:?}");
    }
    assert_eq!(Merging::from_json(&stalled.to_json()), Some(stalled));
    // The word is a wait's alone, whatever a file says.
    let forged = json!({"state": "red", "why": "x", "checks": [], "needs": "person"});
    assert_eq!(Merging::from_json(&forged).unwrap().needs, None);
}

#[test]
fn a_look_with_no_sign_in_says_a_sign_in_is_needed_and_one_that_could_ask_does_not() {
    let forge = crate::forge::Forge::default_of(crate::forge::Kind::GitHub);
    let no_login = || Err(ForgeError::new("gh is not authenticated".to_string()));
    let refused = Err(ForgeError::new("gh: HTTP 401".to_string()));
    let needs = Merging::of_a_look(&refused, &forge, no_login);
    assert_eq!(
        (needs.state, needs.needs),
        (MergeState::Waiting, Some(Needs::SignIn))
    );
    assert!(
        needs.why.contains("has no sign-in to GitHub"),
        "{}",
        needs.why
    );
    assert!(needs.why.contains("Sign in"), "{}", needs.why);
    assert_eq!(Merging::from_json(&needs.to_json()), Some(needs));

    // The forge said the sign-in was refused: nothing more is asked.
    let auth = Err(ForgeError::of(crate::forge::Failure::Auth, "HTTP 401"));
    let asked = std::cell::Cell::new(false);
    let said = Merging::of_a_look(&auth, &forge, || {
        asked.set(true);
        Ok(())
    });
    assert_eq!(said.needs, Some(Needs::SignIn));
    assert!(!asked.get());

    // It could ask, and the forge failed otherwise: a wait, with no sign-in asked for.
    let failed = Err(ForgeError::new("HTTP 502".to_string()));
    assert_eq!(Merging::of_a_look(&failed, &forge, || Ok(())).needs, None);
    // Not reaching the forge is not a missing sign-in, and neither is a CLI that did not
    // answer in time.
    let offline = Err(ForgeError::transport("no network"));
    assert_eq!(Merging::of_a_look(&offline, &forge, no_login).needs, None);
    let slow = || Err(ForgeError::transport("gh did not answer"));
    assert_eq!(Merging::of_a_look(&failed, &forge, slow).needs, None);
    // An answer is never rewritten.
    let red = Ok(Ready::Red(vec![lint()]));
    assert_eq!(
        Merging::of_a_look(&red, &forge, no_login),
        Merging::of(&red)
    );
}

#[test]
fn a_merge_is_made_only_of_the_commit_and_the_standing_the_person_was_shown() {
    let head = "6dcb09b5b57875f334f61aebed695e2e4193db5e";
    let waits = Merging::of(&Ok(Ready::Stalled("no check has started on it".into())));
    let seen = Seen {
        head: head.to_string(),
        shown: waits.shown(),
    };
    // The same commit, standing as it was shown: it is the one confirmed.
    assert_eq!(changed_since(&seen, head, Some(&waits)), None);
    // The words of a wait may differ between two looks; what it is does not.
    let reworded = Merging::of(&Ok(Ready::Stalled("no check has started on it yet".into())));
    assert_eq!(changed_since(&seen, head, Some(&reworded)), None);

    // A check failed since the person looked: not merged past.
    let red = Merging::of(&Ok(Ready::Red(vec![lint()])));
    let why = changed_since(&seen, head, Some(&red)).expect("it changed");
    assert!(why.contains("changed since you looked"), "{why}");
    assert!(why.contains("1 check failed: lint"), "{why}");
    // Another check failed beside the one that was shown.
    let shown_red = Seen {
        head: head.to_string(),
        shown: red.shown(),
    };
    let more = Merging::of(&Ok(Ready::Red(vec![
        lint(),
        FailedCheck::of(Some("build"), None),
    ])));
    assert!(changed_since(&shown_red, head, Some(&more)).is_some());
    assert_eq!(changed_since(&shown_red, head, Some(&red)), None);
    // A wait that ends by itself is not the wait that was confirmed.
    let running = Merging::of(&Ok(Ready::Waiting("its checks are still running".into())));
    assert!(changed_since(&seen, head, Some(&running)).is_some());

    // A commit pushed while the question was open: not merged.
    let pushed = "1111111111111111111111111111111111111111";
    let why = changed_since(&seen, pushed, Some(&waits)).expect("it moved");
    assert!(why.contains("a new commit was pushed"), "{why}");
    // Nothing kept of it any more.
    assert!(changed_since(&seen, head, None).is_some());
}

#[test]
fn what_the_forge_reads_a_check_by_is_never_kept() {
    let red = Merging::of(&Ok(Ready::Red(vec![lint().read_by(Some(7))])));
    assert_eq!(red.checks[0].run, None);
}

#[test]
fn a_red_request_names_its_failed_checks_and_says_it_stays_open() {
    let red = Merging::of(&Ok(Ready::Red(vec![
        lint(),
        FailedCheck::of(Some("build"), None),
    ])));
    assert_eq!(red.why, "2 checks failed: lint, build");
    let said = red.said("pull request #12", "https://github.com/acme/api/pull/12");
    assert!(said.contains("stays open, unmerged"), "{said}");
    assert!(said.contains("lint, build"), "{said}");
    let many: Vec<FailedCheck> = (0..8)
        .map(|n| FailedCheck::of(Some(&format!("job {n}")), None))
        .collect();
    let why = Merging::of(&Ok(Ready::Red(many))).why;
    assert!(why.starts_with("8 checks failed: job 0, "), "{why}");
    assert!(why.ends_with("and 3 more"), "{why}");
}

#[test]
fn a_state_survives_its_file_and_a_file_nobody_wrote_is_read_as_nothing() {
    let red = Merging::of(&Ok(Ready::Red(vec![lint()])));
    assert_eq!(Merging::from_json(&red.to_json()), Some(red));
    for not_ours in [
        json!({}),
        json!({"state": "green"}),
        json!({"state": 4}),
        json!("red"),
    ] {
        assert_eq!(Merging::from_json(&not_ours), None, "{not_ours}");
    }
}

#[test]
fn what_a_rewritten_file_names_is_bounded_again_on_the_way_in() {
    let planted = json!({
        "state": "red",
        "why": "x".repeat(5000),
        "checks": (0..200).map(|n| json!({
            "name": format!("job {n}\u{1b}[31m"),
            "url": "javascript:alert(1)",
        })).collect::<Vec<_>>(),
    });
    let read = Merging::from_json(&planted).expect("a state");
    assert!(
        read.why.len() <= WHY_LIMIT + '…'.len_utf8(),
        "{}",
        read.why.len()
    );
    assert_eq!(read.checks.len(), FAILED_NAMED);
    assert!(read.checks.iter().all(|check| check.url.is_none()));
    assert!(!read.checks[0].name.contains('\u{1b}'));
}

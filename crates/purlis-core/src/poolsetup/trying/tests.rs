// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn candidate(name: &str, reports_its_start: bool) -> Candidate {
    Candidate {
        name: name.to_owned(),
        reports_its_start,
    }
}

fn read(profiles: &[(&str, Standing)]) -> SignIns {
    SignIns::Read(
        profiles
            .iter()
            .map(|(name, standing)| ((*name).to_owned(), Profile::Standing(*standing)))
            .collect(),
    )
}

#[test]
fn the_default_is_tried_where_it_is_signed_in_and_says_when_it_starts() {
    let profiles = [candidate("work", true), candidate("home", true)];
    let sign_ins = read(&[("work", Standing::SignedIn), ("home", Standing::SignedIn)]);
    assert_eq!(
        profile_to_try("dev-box", &sign_ins, Some("home"), &profiles),
        Ok("home".to_owned())
    );
}

#[test]
fn a_profile_whose_harness_reports_its_start_only_with_a_message_is_never_tried() {
    let profiles = [candidate("codex", false), candidate("work", true)];
    let sign_ins = read(&[("codex", Standing::SignedIn), ("work", Standing::SignedIn)]);
    assert_eq!(
        profile_to_try("dev-box", &sign_ins, Some("codex"), &profiles),
        Ok("work".to_owned())
    );
}

#[test]
fn a_signed_out_default_gives_way_to_a_signed_in_profile() {
    let profiles = [candidate("work", true), candidate("home", true)];
    let sign_ins = read(&[("work", Standing::SignedOut), ("home", Standing::SignedIn)]);
    assert_eq!(
        profile_to_try("dev-box", &sign_ins, Some("work"), &profiles),
        Ok("home".to_owned())
    );
}

#[test]
fn a_profile_its_runner_cannot_say_about_is_tried_when_none_is_signed_in() {
    let profiles = [candidate("work", true), candidate("home", true)];
    let sign_ins = read(&[
        ("work", Standing::SignedOut),
        ("home", Standing::CannotTell),
    ]);
    assert_eq!(
        profile_to_try("dev-box", &sign_ins, None, &profiles),
        Ok("home".to_owned())
    );
}

#[test]
fn nothing_signed_in_sends_the_person_to_sign_ins() {
    let profiles = [candidate("work", true)];
    let sign_ins = read(&[("work", Standing::SignedOut)]);
    let refused = profile_to_try("dev-box", &sign_ins, None, &profiles).unwrap_err();
    assert_eq!(refused.step, Some(StepId::Signins));
    assert!(refused.why.contains("No profile is signed in on dev-box"));
}

#[test]
fn only_harnesses_that_cannot_be_seen_starting_signed_in_says_so_and_sends_to_sign_ins() {
    let profiles = [candidate("codex", false), candidate("work", true)];
    let sign_ins = read(&[("codex", Standing::SignedIn), ("work", Standing::SignedOut)]);
    let refused = profile_to_try("dev-box", &sign_ins, None, &profiles).unwrap_err();
    assert_eq!(refused.step, Some(StepId::Signins));
    assert!(
        refused
            .why
            .contains("don't say when their chat has started")
    );
}

#[test]
fn a_runner_not_asked_tries_the_default_and_lets_the_start_answer() {
    let profiles = [candidate("work", true), candidate("home", true)];
    assert_eq!(
        profile_to_try("dev-box", &SignIns::NotAsked, Some("home"), &profiles),
        Ok("home".to_owned())
    );
}

#[test]
fn a_project_with_no_profile_that_says_when_it_starts_is_told_what_to_add() {
    let profiles = [candidate("codex", false)];
    let refused = profile_to_try(
        "dev-box",
        &read(&[("codex", Standing::SignedIn)]),
        None,
        &profiles,
    )
    .unwrap_err();
    assert_eq!(refused.step, None);
    assert!(refused.why.contains("Add a Claude Code profile"));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::recorded::Recorded;

fn github() -> Forge {
    Forge::default_of(Kind::GitHub)
}

#[test]
fn an_account_with_no_signing_keys_lists_none_and_not_this_devices() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("none_listed", &signer);
    let listed = listed_over(&recorded, &github()).unwrap();
    assert_eq!(listed.this_device(&signer), None);
    assert!(listed.other_devices(&signer).is_empty());
    assert!(recorded.unspent().is_empty());
}

#[test]
fn a_key_added_is_listed_as_this_devices_beside_the_other_devices_purlis_keys() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("added_then_listed", &signer);
    let added = add_over(&recorded, &github(), &signer, "purlis · studio").unwrap();
    assert_eq!(added.id, 4242);
    assert!(added.is(&signer));

    let listed = listed_over(&recorded, &github()).unwrap();
    assert_eq!(listed.this_device(&signer).map(|key| key.id), Some(4242));
    // The person's own key, titled by them, is no other device's purlis key.
    let others: Vec<_> = listed
        .other_devices(&signer)
        .iter()
        .map(|key| key.title.as_str())
        .collect();
    assert_eq!(others, ["purlis · laptop"]);
    assert!(recorded.unspent().is_empty());
}

/// Each other device's key reads as its device, the day the forge says it was added, and
/// whether an older setup titled it (`purlis on …`), the likely leftover (#5918).
#[test]
fn other_devices_keys_name_their_device_the_day_added_and_an_older_setup() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("other_devices_listed", &signer);
    let listed = listed_over(&recorded, &github()).unwrap();
    let others: Vec<_> = listed
        .other_devices(&signer)
        .iter()
        .map(|key| (key.device(), key.added.as_deref(), key.older_setup()))
        .collect();
    assert_eq!(
        others,
        [
            ("laptop", Some("2026-10-01"), false),
            ("old-mini", Some("2026-08-14"), true),
            ("desk", None, false),
        ]
    );
}

#[test]
fn an_account_whose_sign_in_may_not_add_signing_keys_is_told_apart_from_other_refusals() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("add_needs_permission", &signer);
    let refused = add_over(&recorded, &github(), &signer, "purlis · studio").unwrap_err();
    assert_eq!(refused, Refused::NeedsPermission);
    assert!(refused.said(Kind::GitHub).contains("may not add"));
}

#[test]
fn a_key_is_removed_by_its_id() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("removed", &signer);
    remove_over(&recorded, &github(), 17).unwrap();
    assert!(recorded.unspent().is_empty());
}

#[test]
fn a_commit_the_forge_shows_as_verified_is_verified() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("commit_verified", &signer);
    let checked = check_over(&recorded, &github(), "octo/hello", "0123abc").unwrap();
    assert!(checked.verified);
    assert_eq!(checked.said(), "GitHub shows it as Verified");
}

#[test]
fn a_commit_whose_email_is_not_verified_says_why_in_plain_words() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("commit_email_unverified", &signer);
    let checked = check_over(&recorded, &github(), "octo/hello", "0123abc").unwrap();
    assert!(!checked.verified);
    assert_eq!(checked.reason, "unverified_email");
    assert!(checked.said().contains("not a verified address"));
}

#[test]
fn a_commit_the_forge_does_not_have_asks_whether_it_is_pushed() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("commit_not_found", &signer);
    let why = check_over(&recorded, &github(), "octo/hello", "0123abc").unwrap_err();
    assert!(why.contains("is it pushed?"), "{why}");
}

#[test]
fn a_repository_or_commit_that_does_not_read_is_refused_before_anything_is_sent() {
    let recorded = Recorded::parse(r#"{"source": "nothing", "exchanges": []}"#).unwrap();
    for (repo, commit) in [
        ("octo", "0123abc"),
        ("octo/../x", "0123abc"),
        ("octo/hello/x", "0123abc"),
        ("octo/hello", "main"),
        ("octo/hello", "0123abc?x=1"),
    ] {
        assert!(check_over(&recorded, &github(), repo, commit).is_err());
    }
}

#[test]
fn a_purlis_title_names_this_device() {
    assert!(title().starts_with("purlis · "));
    assert!(title().len() > TITLED.len());
}

#[test]
fn two_lines_are_one_key_by_type_and_data_whatever_their_comments() {
    assert!(same_key("ssh-ed25519 AAAA x", "ssh-ed25519 AAAA y z"));
    assert!(!same_key("ssh-ed25519 AAAA", "ssh-ed25519 BBBB"));
    assert!(!same_key("", ""));
}

#[test]
fn a_key_setup_had_the_person_add_before_is_still_purlis_made() {
    let key = |title: &str| Key {
        id: 1,
        title: title.to_owned(),
        key: "ssh-ed25519 AAAA".to_owned(),
        added: None,
    };
    assert!(key("purlis · studio").purlis_made());
    assert!(key("purlis on studio").purlis_made());
    assert!(!key("my laptop").purlis_made());
}

#[test]
fn a_key_added_by_hand_is_noticed_in_the_public_list_with_no_scope() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("publicly_listed", &signer);
    let listed = listed_publicly_over(&recorded, &github(), "octocat").unwrap();
    assert_eq!(listed.this_device(&signer).map(|key| key.id), Some(4242));
    let nothing = Recorded::parse(r#"{"source": "nothing", "exchanges": []}"#).unwrap();
    assert!(listed_publicly_over(&nothing, &github(), "../x").is_err());
}

// GitLab (#5629): user keys with the usage type Signing, the commit signature API, and the
// server's version.

fn gitlab() -> Forge {
    Forge::default_of(Kind::GitLab)
}

fn gitlab_recording(case: &str, signer: &Signer) -> Recorded {
    recording_of("gitlab", case, &[("@KEY@", &signer.public_line())])
}

#[test]
fn gitlab_adds_the_key_as_signing_only_and_lists_only_the_keys_that_may_sign() {
    let signer = Signer::generate().unwrap();
    // The recording holds the call purlis must send: `usage_type=signing`, never GitLab's
    // default `auth_and_signing`.
    let recorded = gitlab_recording("added_then_listed", &signer);
    let added = add_over(&recorded, &gitlab(), &signer, "purlis · studio").unwrap();
    assert_eq!(added.id, 77);
    assert!(added.is(&signer));

    let listed = listed_over(&recorded, &gitlab()).unwrap();
    assert_eq!(listed.this_device(&signer).map(|key| key.id), Some(77));
    let titles: Vec<_> = listed.keys.iter().map(|key| key.title.as_str()).collect();
    // The login-only key is no signing key.
    assert_eq!(titles, ["purlis · laptop", "purlis · studio"]);
    assert!(recorded.unspent().is_empty());
}

#[test]
fn gitlab_answers_a_key_it_has_already_with_the_one_it_has() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("already_taken", &signer);
    let added = add_over(&recorded, &gitlab(), &signer, "purlis · studio").unwrap();
    assert_eq!(added.id, 77);
    assert!(recorded.unspent().is_empty());
}

#[test]
fn gitlab_removes_a_key_by_its_id() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("removed", &signer);
    remove_over(&recorded, &gitlab(), 12).unwrap();
    assert!(recorded.unspent().is_empty());
}

#[test]
fn gitlabs_public_list_counts_a_key_only_where_it_may_sign() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("publicly_listed", &signer);
    let listed = listed_publicly_over(&recorded, &gitlab(), "sam").unwrap();
    assert_eq!(listed.this_device(&signer).map(|key| key.id), Some(77));

    let recorded = gitlab_recording("publicly_auth_only", &signer);
    let listed = listed_publicly_over(&recorded, &gitlab(), "sam").unwrap();
    assert_eq!(listed.this_device(&signer), None);
}

#[test]
fn gitlab_shows_a_pushed_signed_commit_as_verified() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("commit_verified", &signer);
    let checked = check_over(&recorded, &gitlab(), "platform/billing", "0123abc").unwrap();
    assert!(checked.verified);
    assert_eq!(checked.reason, "valid");
}

#[test]
fn gitlabs_other_email_reads_as_githubs_unverified_email() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("commit_email_unverified", &signer);
    let checked = check_over(&recorded, &gitlab(), "platform/billing", "0123abc").unwrap();
    assert!(!checked.verified);
    assert_eq!(checked.reason, "unverified_email");
}

#[test]
fn gitlab_has_no_signature_for_an_unsigned_commit_and_no_tick_comes_of_it() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("commit_unsigned", &signer);
    let why = check_over(&recorded, &gitlab(), "platform/billing", "0123abc").unwrap_err();
    assert!(why.contains("unsigned"), "{why}");
}

#[test]
fn a_gitlab_project_may_sit_in_nested_groups_and_nothing_else_reads_as_one() {
    assert!(project_named("acme/platform/billing").is_ok());
    assert!(project_named("billing").is_err());
    assert!(project_named("acme/../x").is_err());
    assert!(project_named("acme//x").is_err());
}

#[test]
fn a_gitlab_older_than_the_first_that_verifies_ssh_signatures_cannot_verify() {
    let signer = Signer::generate().unwrap();
    let old = gitlab_recording("version_old", &signer);
    assert!(!verifies_over(&old, &gitlab()).unwrap());
    let current = gitlab_recording("version_current", &signer);
    assert!(verifies_over(&current, &gitlab()).unwrap());
    // GitHub verifies them, and is not asked.
    let nothing = Recorded::parse(r#"{"source": "nothing", "exchanges": []}"#).unwrap();
    assert!(verifies_over(&nothing, &github()).unwrap());
}

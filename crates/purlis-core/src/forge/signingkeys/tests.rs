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

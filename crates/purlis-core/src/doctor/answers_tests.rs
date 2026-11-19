// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `answers` rows: each declared gap and each state in words.

use super::*;
use crate::doctor::Status;
use crate::doctor::tests::{doctor, plane};
use crate::sandbox::local::OwnConfigHome;
use crate::secrets::answers::Lost;

#[test]
fn doctor_declares_the_seal_gap_on_linux_and_windows() {
    let linux = seal_gap_row(Os::Linux).expect("a row");
    assert_eq!(linux.status, Status::Ok, "a declared gap, not a fault");
    assert!(
        linux.detail.contains("declared gap")
            && linux.detail.contains("hand edit and a sandboxed chat only"),
        "{}",
        linux.detail
    );
    let windows = seal_gap_row(Os::Windows).expect("a row");
    assert_eq!(windows.status, Status::Ok);
    assert!(
        windows.detail.contains("no Windows sandbox yet")
            && windows.detail.contains("hand edits only"),
        "{}",
        windows.detail
    );
    // The Keychain holds the key to the app on a Mac: no gap to declare.
    assert_eq!(seal_gap_row(Os::MacOs), None);
}

#[test]
fn doctor_says_the_device_id_is_unset_where_the_registry_says_none() {
    let none = device_row(&DeviceView::Seen(Device::None));
    assert_eq!(none.status, Status::Warn);
    assert_eq!(none.detail, answers::DEVICE_UNSET);
    let unconfirmed = device_row(&DeviceView::Seen(Device::Unconfirmed("id".to_owned())));
    assert_eq!(unconfirmed.status, Status::Warn);
    assert!(unconfirmed.hint.contains("Confirm"), "{}", unconfirmed.hint);
    let confirmed = device_row(&DeviceView::Seen(Device::Confirmed("id".to_owned())));
    assert_eq!(confirmed.status, Status::Ok);
}

#[test]
fn a_doctor_that_is_not_the_app_does_not_call_the_device_id_unset() {
    let row = device_row(&DeviceView::NotChecked);
    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert!(row.detail.starts_with("not checked here"), "{}", row.detail);
    assert!(!row.detail.contains("unset"), "{}", row.detail);
}

#[test]
fn every_deny_cause_has_words_of_its_own() {
    let lost = Lost::default();
    let causes = [
        Deny::ManifestMissing(lost),
        Deny::ManifestOlder(lost),
        Deny::AnchorOlder,
        Deny::CloneKeySwapped,
        Deny::ManifestFailsSeal,
        Deny::EntryMissing {
            kind: "never".into(),
            digest: "d".into(),
        },
        Deny::EntryFailsSeal {
            kind: "never".into(),
            digest: "d".into(),
        },
        Deny::CountsDisagree,
        Deny::PlaneMoved {
            id: "i".into(),
            anchored_at: "/old".into(),
        },
        Deny::StoreWithoutAnchor,
        Deny::AnchorGone,
        Deny::RegistryUnreadable,
        Deny::EverywhereOlder(lost),
        Deny::PastTheFloorCap,
        Deny::SealKeyLocked,
        Deny::KeyringDidNotStart,
        Deny::NeversNotImported,
    ];
    let said: Vec<String> = causes.iter().map(cause_said).collect();
    let mut unique = said.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), causes.len(), "{said:?}");
    let row = store_row(&State::Deny(causes.to_vec()));
    assert_eq!(row.status, Status::Fail);
    for words in &said {
        assert!(row.detail.contains(words.as_str()), "{}", row.detail);
    }
}

#[test]
fn the_store_row_says_each_state() {
    assert_eq!(store_row(&State::Fresh).status, Status::Ok);
    assert_eq!(store_row(&State::Holds).status, Status::Ok);
    assert_eq!(store_row(&State::NoKeyring).status, Status::Warn);
    let elsewhere = store_row(&State::Unverifiable);
    assert_eq!(elsewhere.status, Status::Ok);
    assert!(elsewhere.detail.starts_with("not checked here"));
}

#[test]
fn doctor_names_the_old_answer_files_as_safe_to_delete() {
    let files = vec![
        "app/sandbox.json".to_owned(),
        "app/dispatch-never.json".to_owned(),
        "sandbox-grants/abc.json (in purlis's config folder)".to_owned(),
        "inbox/rules.json (in purlis's data folder)".to_owned(),
    ];
    let row = old_files_row(&files, Imported::Yes).expect("a row");
    assert_eq!(row.status, Status::Ok);
    let (safe, kept) = row
        .detail
        .split_once("Keep app/dispatch-never.json")
        .expect("the nevers file is kept");
    assert!(safe.contains("safe to delete"), "{}", row.detail);
    for name in ["app/sandbox.json", "sandbox-grants/", "inbox/rules.json"] {
        assert!(safe.contains(name), "{name} in {}", row.detail);
    }
    // Its reader still exists, so it is never listed among the files to delete.
    assert!(!safe.contains("dispatch-never"), "{}", row.detail);
    assert!(kept.contains("still reads it"), "{}", row.detail);
    // Where it is the only file there is nothing to delete.
    let only = old_files_row(&files[1..2], Imported::Yes).expect("a row");
    assert!(!only.detail.contains("safe to delete"), "{}", only.detail);
    // Nothing to say where an earlier build left nothing.
    assert_eq!(old_files_row(&[], Imported::Yes), None);
}

#[test]
fn doctor_never_calls_old_files_safe_to_delete_before_their_nevers_are_imported() {
    let files = vec!["app/dispatch-never.json".to_owned()];
    let row = old_files_row(&files, Imported::No).expect("a row");
    assert_eq!(row.status, Status::Warn);
    assert!(!row.detail.contains("safe to delete"), "{}", row.detail);
    assert!(row.detail.contains("not imported yet"), "{}", row.detail);
    let more = vec![files[0].clone(), "app/sandbox.json".to_owned()];
    let unknown = old_files_row(&more, Imported::Unknown).expect("a row");
    assert!(
        unknown.detail.contains("once purlis's app has imported"),
        "{}",
        unknown.detail
    );
    assert!(
        !unknown.detail.starts_with("app/dispatch-never.json"),
        "{}",
        unknown.detail
    );
}

#[test]
fn the_cross_filesystem_move_is_a_declared_gap_that_slice_11_closes() {
    let row = moved_row();
    assert_eq!(row.status, Status::Ok);
    assert!(
        row.detail.contains("another filesystem") && row.detail.contains("slice 11"),
        "{}",
        row.detail
    );
}

#[test]
fn a_doctor_a_test_names_prints_no_answers_rows_and_one_on_a_machine_prints_them() {
    let (_d, root) = plane("schema = 1\n");
    let _home = OwnConfigHome::new();
    let mut d = doctor(&root);
    assert!(
        answers(&d).is_empty(),
        "a test's doctor never reads a keyring"
    );
    d.answers_here = true;
    let rows = answers(&d);
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    assert!(names.contains(&"answers device id"), "{names:?}");
    assert!(names.contains(&"answers store"), "{names:?}");
    assert!(names.contains(&"answers moved plane"), "{names:?}");
}

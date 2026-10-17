// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const DESKTOP: &str = "01JA0DESKT0P00000000000000";

fn desktop() -> Pinned {
    Pinned {
        device: DESKTOP.into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

#[test]
fn a_runners_pairing_makes_its_key_and_keeps_the_desktop_as_its_one_peer() {
    let folder = tempfile::tempdir().unwrap();
    let desktop = desktop();
    let runner = pair(folder.path(), RUNNER, &desktop).unwrap();
    assert_eq!(runner.device, RUNNER);
    let (ours, peer) = answering(folder.path(), RUNNER).unwrap().expect("paired");
    assert_eq!(ours.device, RUNNER);
    assert_eq!(ours.key.public(), runner.key);
    assert_eq!(peer, desktop);
}

#[test]
fn a_runner_is_paired_once_whichever_desktop_asks_again() {
    let folder = tempfile::tempdir().unwrap();
    let first = desktop();
    let runner = pair(folder.path(), RUNNER, &first).unwrap();
    for again in [first.clone(), desktop()] {
        let refused = pair(folder.path(), RUNNER, &again).unwrap_err();
        assert!(refused.contains("already holds"), "{refused}");
    }
    let (ours, peer) = answering(folder.path(), RUNNER).unwrap().unwrap();
    assert_eq!((ours.key.public(), peer), (runner.key, first));
}

#[test]
fn a_home_that_already_holds_a_link_key_or_a_peers_file_is_not_paired() {
    type Folder = fn(&std::path::Path) -> std::path::PathBuf;
    for (folder_of, name) in [
        (crate::machine::charterd as Folder, KEY_FILE),
        (crate::machine::dir as Folder, PEERS_FILE),
    ] {
        let folder = tempfile::tempdir().unwrap();
        purlis_same_user::PrivateDirectory::open(&folder_of(folder.path()))
            .unwrap()
            .replace_file(name, b"brought by the image")
            .unwrap();
        let refused = pair(folder.path(), RUNNER, &desktop()).unwrap_err();
        assert!(refused.contains("already holds"), "{name}: {refused}");
    }
}

#[test]
fn a_runner_that_was_never_paired_answers_no_link() {
    let folder = tempfile::tempdir().unwrap();
    assert!(answering(folder.path(), RUNNER).unwrap().is_none());
}

#[test]
fn the_runners_key_and_peer_are_kept_for_its_user_alone() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), RUNNER, &desktop()).unwrap();
    for file in [
        crate::machine::charterd(folder.path()).join(KEY_FILE),
        crate::machine::dir(folder.path()).join(PEERS_FILE),
    ] {
        let name = file.display();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{name}");
    }
}

#[test]
fn a_kept_key_others_may_read_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), RUNNER, &desktop()).unwrap();
    std::fs::set_permissions(
        crate::machine::charterd(folder.path()).join(KEY_FILE),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(answering(folder.path(), RUNNER).is_err());
}

#[test]
fn a_kept_key_that_is_not_one_key_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), RUNNER, &desktop()).unwrap();
    purlis_same_user::PrivateDirectory::open(&crate::machine::charterd(folder.path()))
        .unwrap()
        .replace_file(KEY_FILE, b"not a key")
        .unwrap();
    assert!(answering(folder.path(), RUNNER).is_err());
}

#[test]
fn a_device_id_and_key_are_written_and_read_as_one_line() {
    let pinned = desktop();
    let line = to_line(&pinned);
    assert_eq!(from_line(&line).unwrap(), pinned);
    assert!(!line.contains('\n'));
    for refused in [
        "",
        "{}",
        r#"{"device":"01JA","key":"00"}"#,
        r#"{"device":"","key":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
        r#"{"device":"01JA","key":"0000000000000000000000000000000000000000000000000000000000000000","more":1}"#,
    ] {
        assert!(from_line(refused).is_err(), "{refused} was taken");
    }
}

fn write_peers(folder: &std::path::Path, text: &str) {
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(folder))
        .unwrap()
        .replace_file(PEERS_FILE, text.as_bytes())
        .unwrap();
}

#[test]
fn the_runners_peers_file_names_its_one_desktop_with_when_it_was_paired() {
    let folder = tempfile::tempdir().unwrap();
    let desktop = desktop();
    pair(folder.path(), RUNNER, &desktop).unwrap();
    let text =
        std::fs::read_to_string(crate::machine::dir(folder.path()).join(PEERS_FILE)).unwrap();
    let peers: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(peers["version"], 1);
    assert_eq!(peers["peers"].as_array().unwrap().len(), 1);
    assert_eq!(peers["peers"][0]["device"], DESKTOP);
    let hex: String = desktop
        .key
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(peers["peers"][0]["key"], hex);
    assert!(peers["peers"][0]["paired"].as_u64().unwrap() > 1_700_000_000);
}

#[test]
fn a_peers_file_with_more_than_one_desktop_or_of_another_shape_links_nobody() {
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), RUNNER, &desktop()).unwrap();
    let one = |device: &str| serde_json::json!({ "device": device, "key": "00".repeat(32), "paired": 1_800_000_000u64 });
    for text in [
        serde_json::json!({ "version": 1, "peers": [one(DESKTOP), one("01JC0OTHER")] }).to_string(),
        serde_json::json!({ "version": 2, "peers": [one(DESKTOP)] }).to_string(),
        serde_json::json!({ "version": 1, "peers": [] }).to_string(),
        serde_json::json!({ "version": 1, "peers": [{ "device": DESKTOP, "key": "00".repeat(32) }] }).to_string(),
        "not json".to_owned(),
    ] {
        write_peers(folder.path(), &text);
        assert!(answering(folder.path(), RUNNER).is_err(), "{text} was taken");
        assert!(pair(folder.path(), RUNNER, &desktop()).is_err(), "{text} let a pairing in");
    }
}

#[test]
fn of_two_pairings_at_once_one_is_made_and_the_other_refused() {
    for _ in 0..20 {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().to_path_buf();
        let (a, b) = (desktop(), desktop());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let racing = |desktop: Pinned| {
            let (root, barrier) = (root.clone(), std::sync::Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                pair(&root, RUNNER, &desktop).map(|runner| (runner, desktop))
            })
        };
        let (first, second) = (racing(a), racing(b));
        let results = [first.join().unwrap(), second.join().unwrap()];
        let made: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(made.len(), 1, "{results:?}");
        let (runner, desktop) = made[0];
        let (ours, peer) = answering(&root, RUNNER).unwrap().unwrap();
        assert_eq!((ours.key.public(), &peer), (runner.key, desktop));
    }
}

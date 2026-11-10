// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The first-open baseline (#5648, ADR 0119): the record, kept per project beside the updates,
//! that the past was counted as seen the first time the project opened on this machine. Driven
//! through [`Store`] alone, on a data home of the test's own.

use super::*;

const NOW: u64 = 1_800_000_000;

/// A project whose committed settings carry a teammate's host and a preset off the defaults.
const TEAMMATES: &str = "[sandbox]\nmode = \"on\"\nhosts = [\"a.example\"]\negress = [\"forge\"]\n";

fn project(charter_toml: &str) -> (tempfile::TempDir, PathBuf, Store) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::write(root.join(crate::plane::MANIFEST), charter_toml).expect("charter.toml");
    let store = Store::in_data(&dir.path().join("data"));
    (dir, root, store)
}

fn update(key: &str, kind: Kind, at: u64) -> Update {
    Update {
        key: key.to_owned(),
        kind,
        at,
        session: Some(3),
        chain: vec!["steward 3".to_owned()],
        says: "drill finished".to_owned(),
        read: false,
        dismissed: false,
        baselined: false,
    }
}

/// Every file under `root`, by its path inside it.
fn files_in(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("a folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                out.push(path.strip_prefix(root).expect("inside").to_path_buf());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_first_open_with_no_seen_record_is_baselined_once_and_never_again() {
    let (_dir, root, store) = project("");
    assert_eq!(
        store.baseline(&root),
        None,
        "no record before the first open"
    );

    let first = store.first_open(&root, NOW).expect("opened");
    assert_eq!(
        first,
        Baseline {
            at: NOW,
            fresh: true,
            findings: None
        }
    );

    let again = store.first_open(&root, NOW + 3_600).expect("opened again");
    assert_eq!(
        again, first,
        "a plane with a record is never baselined again"
    );
    assert_eq!(store.baseline(&root), Some(first));
}

#[test]
fn the_record_survives_a_new_store_on_the_same_data_home_as_a_relaunch_does() {
    let (dir, root, store) = project("");
    store.first_open(&root, NOW).expect("opened");

    let relaunched = Store::in_data(&dir.path().join("data"));
    let kept = relaunched.first_open(&root, NOW + 60).expect("opened");
    assert_eq!((kept.at, kept.fresh), (NOW, true));
}

#[test]
fn a_fresh_first_open_writes_the_teammate_notices_seen_records() {
    let (_dir, root, store) = project(TEAMMATES);
    assert!(crate::sandbox::local::hosts_changed(&root).is_some());
    assert!(crate::sandbox::local::presets_changed(&root).is_some());

    store.first_open(&root, NOW).expect("opened");

    assert_eq!(crate::sandbox::local::hosts_changed(&root), None);
    assert_eq!(crate::sandbox::local::presets_changed(&root), None);
}

#[test]
fn a_teammate_change_after_the_first_open_is_told() {
    let (_dir, root, store) = project(TEAMMATES);
    store.first_open(&root, NOW).expect("opened");

    std::fs::write(
        root.join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\nhosts = [\"a.example\", \"b.example\"]\negress = [\"forge\"]\n",
    )
    .expect("a teammate's push");
    store.first_open(&root, NOW + 60).expect("opened again");

    let told = crate::sandbox::local::hosts_changed(&root).expect("a change");
    assert_eq!(told.added, ["b.example"]);
}

#[test]
fn a_machine_with_a_seen_record_is_not_baselined_and_keeps_its_unread_lines() {
    let (_dir, root, store) = project(TEAMMATES);
    store
        .note(&root, vec![update("task:a", Kind::TaskDone, NOW - 60)], NOW)
        .expect("noted");

    let record = store.first_open(&root, NOW).expect("opened");

    assert!(
        !record.fresh,
        "an existing user's updates file is a seen record"
    );
    let kept = store.read(&root, NOW);
    assert!(!kept[0].read && !kept[0].baselined, "{kept:?}");
    assert!(
        crate::sandbox::local::hosts_changed(&root).is_some(),
        "a teammate change this person has not been told is still told"
    );
}

/// **An existing user whose updates aged out and who was never told of the sandbox** (story 4):
/// the window's layout file still holds what they kept in the project, so this machine opened it
/// before, and nothing standing is set aside unseen.
#[test]
fn an_existing_user_with_aged_out_updates_and_no_told_record_is_not_baselined() {
    let (dir, root, store) = project(TEAMMATES);
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).expect("a config home");
    let project = root.to_string_lossy().into_owned();
    crate::windowprefs::set_dismissed(&config, &project, &["doctor-finding:x".to_owned()])
        .expect("a dismissal kept in the project");
    let store = store.with_layout(&config);
    assert!(!store.file(&root).exists(), "their updates aged out");

    let record = store.first_open(&root, NOW).expect("opened");

    assert!(!record.fresh, "this machine opened the project before");
    assert!(
        crate::sandbox::local::hosts_changed(&root).is_some(),
        "a teammate's host this person was never told of is still told"
    );
}

/// A project the layout file keeps an arrangement or a view for was opened here before too.
#[test]
fn a_project_the_layout_file_keeps_views_for_is_not_baselined() {
    let (dir, root, store) = project("");
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).expect("a config home");
    let project = root.to_string_lossy().into_owned();
    let layout = serde_json::json!({
        "version": 2,
        "regions": [],
        "projects": { project: { "chats": { "scope": "project" } } },
    });
    crate::windowprefs::write_layout(&config, &layout.to_string()).expect("a layout");

    assert!(
        !store
            .with_layout(&config)
            .first_open(&root, NOW)
            .expect("opened")
            .fresh
    );
}

/// Another project's entry in the layout file says nothing of this one: its first open is one.
#[test]
fn another_project_s_layout_entry_leaves_a_first_open_fresh() {
    let (dir, root, store) = project("");
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).expect("a config home");
    crate::windowprefs::set_dismissed(&config, "/somewhere/else", &["x".to_owned()])
        .expect("another project's dismissal");

    assert!(
        store
            .with_layout(&config)
            .first_open(&root, NOW)
            .expect("opened")
            .fresh
    );
}

/// A chat snoozed in the project is a record this machine kept of it.
#[test]
fn a_project_with_snoozed_chats_is_not_baselined() {
    let (_dir, root, store) = project("");
    store
        .snooze(
            &root,
            [(3, vec!["ask-1".to_owned()])].into_iter().collect(),
            None,
        )
        .expect("snoozed");

    assert!(!store.first_open(&root, NOW).expect("opened").fresh);
}

#[test]
fn a_machine_that_was_told_the_project_s_hosts_is_not_baselined() {
    let (_dir, root, store) = project(TEAMMATES);
    crate::sandbox::local::acknowledge_hosts(&root, &[]).expect("told");

    assert!(!store.first_open(&root, NOW).expect("opened").fresh);
    assert!(crate::sandbox::local::presets_changed(&root).is_some());
}

#[test]
fn what_happened_before_a_fresh_first_open_is_noted_read_and_baselined() {
    let (_dir, root, store) = project("");
    store.first_open(&root, NOW).expect("opened");

    let kept = store
        .note(
            &root,
            vec![
                update("task:before", Kind::TaskDone, NOW - 600),
                update("task:after", Kind::TaskDone, NOW + 60),
            ],
            NOW + 60,
        )
        .expect("noted");

    let of = |key: &str| kept.iter().find(|one| one.key == key).expect(key);
    assert!(of("task:before").read && of("task:before").baselined);
    assert!(!of("task:after").read && !of("task:after").baselined);
}

#[test]
fn the_findings_of_the_first_report_are_set_aside_once() {
    let (_dir, root, store) = project("");
    store.first_open(&root, NOW).expect("opened");

    store
        .set_aside_findings(
            &root,
            &["doctor-finding:a".to_owned(), "doctor-finding:b".to_owned()],
            NOW + 6,
        )
        .expect("set aside");
    assert_eq!(
        store.baseline(&root).and_then(|one| one.findings),
        Some(vec![
            "doctor-finding:a".to_owned(),
            "doctor-finding:b".to_owned()
        ])
    );

    // Taken already: a later report sets nothing aside, so its new finding is new.
    store
        .set_aside_findings(&root, &["doctor-finding:c".to_owned()], NOW + 60)
        .expect("asked again");
    assert_eq!(
        store
            .baseline(&root)
            .and_then(|one| one.findings)
            .map(|one| one.len()),
        Some(2)
    );
}

#[test]
fn a_machine_that_was_not_baselined_sets_no_finding_aside() {
    let (_dir, root, store) = project("");
    store
        .note(
            &root,
            vec![update("doctor:a:fail", Kind::Doctor, NOW - 5)],
            NOW,
        )
        .expect("noted");
    store.first_open(&root, NOW).expect("opened");

    let kept = store
        .set_aside_findings(&root, &["doctor-finding:a".to_owned()], NOW + 1)
        .expect("asked");

    assert!(!kept[0].read && !kept[0].baselined, "{kept:?}");
    assert_eq!(store.baseline(&root).and_then(|one| one.findings), None);
}

#[test]
fn nothing_of_the_baseline_is_written_into_the_plane() {
    let (_dir, root, store) = project(TEAMMATES);
    let before = files_in(&root);

    store.first_open(&root, NOW).expect("opened");
    store
        .note(
            &root,
            vec![update("doctor:a:fail", Kind::Doctor, NOW + 5)],
            NOW + 5,
        )
        .expect("noted");
    store
        .set_aside_findings(&root, &["doctor:a:fail".to_owned()], NOW + 6)
        .expect("set aside");

    assert_eq!(files_in(&root), before);
    assert!(store.baseline_file(&root).is_file());
    assert!(!store.baseline_file(&root).starts_with(&root));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

const ID: &str = "01K6CHATS0000000000000000C";
const OTHER: &str = "01K6CHATS0000000000000000D";

fn chat(number: u32, id: &str, ended: &str) -> ChatRecord {
    ChatRecord {
        number,
        id: id.to_owned(),
        title: "Tidy the changelog".to_owned(),
        started: "2026-11-12T09:00:00Z".to_owned(),
        ended: ended.to_owned(),
        started_by: StartedBy::Person,
        place: "smart-ide".to_owned(),
    }
}

fn at(time: &str) -> SystemTime {
    chrono::DateTime::parse_from_rfc3339(time).unwrap().into()
}

#[test]
fn a_chat_kept_as_it_closed_reads_back_by_its_number() {
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).unwrap();
    assert_eq!(
        of_number(&every(&root), 702),
        Some(&chat(702, ID, "2026-11-12T09:40:00Z"))
    );
    assert_eq!(of_number(&every(&root), 703), None);
}

#[test]
fn a_chat_handed_off_says_the_chat_that_started_it() {
    let (_dir, root) = project();
    let handed = ChatRecord {
        started_by: StartedBy::Chat {
            name: "steward 3".to_owned(),
        },
        ..chat(702, ID, "2026-11-12T09:40:00Z")
    };
    keep(&root, &handed).unwrap();
    assert_eq!(every(&root), vec![handed]);
}

#[test]
fn a_chat_closed_again_is_its_later_close() {
    // Closed, reopened under a new number, and closed again: one chat, its newest close.
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).unwrap();
    keep(&root, &chat(731, ID, "2026-11-12T11:00:00Z")).unwrap();
    assert_eq!(every(&root), vec![chat(731, ID, "2026-11-12T11:00:00Z")]);
}

#[test]
fn newest_first_and_held_to_the_most_kept() {
    let (_dir, root) = project();
    for n in 0..(MOST_KEPT as u32 + 3) {
        let id = ulid::Ulid::from_parts(1_700_000_000_000 + u64::from(n), 1).to_string();
        keep(&root, &chat(n + 1, &id, "2026-11-12T09:40:00Z")).unwrap();
    }
    let kept = every(&root);
    assert_eq!(kept.len(), MOST_KEPT);
    assert_eq!(kept[0].number, MOST_KEPT as u32 + 3, "the newest first");
    assert!(of_number(&kept, 1).is_none(), "the oldest went first");
}

#[test]
fn a_record_is_refused_unless_its_id_is_a_ulid_and_its_times_are_times() {
    let (_dir, root) = project();
    assert!(keep(&root, &chat(702, "not-a-ulid", "2026-11-12T09:40:00Z")).is_err());
    assert!(keep(&root, &chat(702, ID, "yesterday")).is_err());
    assert!(every(&root).is_empty());
}

#[test]
fn the_sweep_takes_a_chat_ended_more_than_thirty_days_ago() {
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-10-01T09:40:00Z")).unwrap();
    keep(&root, &chat(703, OTHER, "2026-11-01T09:40:00Z")).unwrap();
    assert_eq!(sweep(&root, at("2026-11-13T00:00:00Z")), 1);
    assert_eq!(
        every(&root)
            .iter()
            .map(|one| one.number)
            .collect::<Vec<_>>(),
        vec![703]
    );
    assert_eq!(sweep(&root, at("2026-11-13T00:00:00Z")), 0);
}

#[test]
fn the_store_is_in_the_projects_state_folder_beside_the_dispatch_records() {
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).unwrap();
    assert_eq!(
        path(&root),
        crate::names::state(&root)
            .join("app")
            .join(DIR_NAME)
            .join(FILE_NAME)
    );
    assert!(path(&root).is_file());
}

#[cfg(unix)]
#[test]
fn the_file_is_the_persons_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).unwrap();
    let mode = std::fs::metadata(path(&root)).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn a_link_in_its_place_is_never_read_through() {
    let (_dir, root) = project();
    let elsewhere = root.join("elsewhere.json");
    std::fs::write(
        &elsewhere,
        format!(
            r#"{{"v":1,"chats":[{}]}}"#,
            serde_json::to_string(&OnDisk::of(&chat(702, ID, "2026-11-12T09:40:00Z"))).unwrap()
        ),
    )
    .unwrap();
    std::fs::create_dir_all(path(&root).parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, path(&root)).unwrap();
    assert!(every(&root).is_empty());
}

#[test]
fn a_file_that_is_not_one_purlis_wrote_reads_as_none() {
    let (_dir, root) = project();
    std::fs::create_dir_all(path(&root).parent().unwrap()).unwrap();
    for text in [
        "not json",
        r#"{"v":2,"chats":[]}"#,
        r#"{"v":1,"chats":[{"number":702}]}"#,
    ] {
        std::fs::write(path(&root), text).unwrap();
        assert!(every(&root).is_empty(), "{text}");
    }
}

#[test]
fn the_sweep_takes_a_chat_ended_exactly_thirty_days_ago_as_a_dispatch_record_is() {
    // `retention` collects a dispatch record "30 days or more" after its end.
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-10-14T00:00:00Z")).unwrap();
    keep(&root, &chat(703, OTHER, "2026-10-14T00:00:01Z")).unwrap();
    assert_eq!(sweep(&root, at("2026-11-13T00:00:00Z")), 1);
    assert_eq!(
        every(&root)
            .iter()
            .map(|one| one.number)
            .collect::<Vec<_>>(),
        vec![703]
    );
}

#[test]
fn a_chat_whose_end_stands_in_the_future_is_swept_though_other_chats_keep_closing() {
    // Each keep rewrites the file, so its last write never ages: an end in the future is due
    // at once, as a dispatch record's words are (`dispatchrecord::talk_is_due`).
    let (_dir, root) = project();
    let now = SystemTime::now();
    let stamp = |time: SystemTime| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339();
    let a_year = std::time::Duration::from_secs(365 * 24 * 3600);
    let ahead = serde_json::to_value(OnDisk::of(&chat(702, ID, &stamp(now + a_year)))).unwrap();
    std::fs::create_dir_all(dir(&root)).unwrap();
    std::fs::write(
        path(&root),
        serde_json::json!({"v": 1, "chats": [ahead]}).to_string(),
    )
    .unwrap();
    keep(&root, &chat(703, OTHER, &stamp(now))).unwrap();
    assert_eq!(sweep(&root, now), 1);
    assert_eq!(
        every(&root)
            .iter()
            .map(|one| one.number)
            .collect::<Vec<_>>(),
        vec![703]
    );
}

#[test]
fn a_chat_kept_with_an_end_ahead_of_this_clock_is_kept_as_ending_now() {
    let (_dir, root) = project();
    let before = chrono::Utc::now();
    let ahead = (before + chrono::Duration::days(365)).to_rfc3339();
    keep(&root, &chat(702, ID, &ahead)).unwrap();
    let ended = chrono::DateTime::parse_from_rfc3339(&every(&root)[0].ended).unwrap();
    assert!(ended >= before && ended <= chrono::Utc::now(), "{ended}");
    assert_eq!(
        sweep(&root, SystemTime::now()),
        0,
        "an end of now is not due"
    );
}

#[test]
fn the_sweep_takes_what_a_killed_write_left_behind_and_nothing_else() {
    // A write killed between its temp file and the rename leaves the temp file, holding titles.
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let left = dir(&root).join(".purlis-generated.closed.json.4242.k1.tmp");
    let older = dir(&root).join(".charter-generated.closed.json.4242.k2.tmp");
    let other = dir(&root).join("notes.txt");
    for file in [&left, &older, &other] {
        std::fs::write(file, "{}").unwrap();
    }
    sweep(&root, at("2026-11-13T00:00:00Z"));
    assert!(!left.exists());
    assert!(!older.exists());
    assert!(
        other.exists(),
        "a file purlis never names is not its to take"
    );
}

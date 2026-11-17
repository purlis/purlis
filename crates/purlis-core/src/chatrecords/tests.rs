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
        earlier: Vec::new(),
        chat: None,
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

#[cfg(unix)]
#[test]
fn a_link_on_the_way_from_the_project_is_never_read_through() {
    let (_dir, root) = project();
    let elsewhere = root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(
        elsewhere.join(FILE_NAME),
        format!(
            r#"{{"v":1,"chats":[{}]}}"#,
            serde_json::to_string(&OnDisk::of(&chat(702, ID, "2026-11-12T09:40:00Z"))).unwrap()
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir(&root).parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir(&root)).unwrap();
    assert!(every(&root).is_empty());
}

#[cfg(unix)]
#[test]
fn a_fifo_in_its_place_reads_as_none_without_blocking() {
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let made =
        crate::forklock::status(std::process::Command::new("mkfifo").arg(path(&root))).unwrap();
    assert!(made.success());
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
    let long_ago = SystemTime::now() - std::time::Duration::from_secs(3600);
    for file in [&left, &older, &other] {
        std::fs::write(file, "{}").unwrap();
        std::fs::File::options()
            .write(true)
            .open(file)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
    }
    sweep(&root, SystemTime::now());
    assert!(!left.exists());
    assert!(!older.exists());
    assert!(
        other.exists(),
        "a file purlis never names is not its to take"
    );
}

#[test]
fn the_sweep_leaves_a_write_still_under_way() {
    // Where the folder cannot be locked, the sweep runs beside a write: a temp file written
    // within the lock's longest wait may be one, and stays.
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let live = dir(&root).join(".purlis-generated.closed.json.4242.k1.tmp");
    std::fs::write(&live, "{}").unwrap();
    sweep(&root, SystemTime::now());
    assert!(live.exists());
}

#[test]
fn chats_that_close_together_are_kept_together_each_replacing_its_older_close() {
    // #6487: a fresh start drops several chats at once.
    const THIRD: &str = "01K6CHATS0000000000000000E";
    let (_dir, root) = project();
    keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).unwrap();
    keep(&root, &chat(650, THIRD, "2026-11-12T09:00:00Z")).unwrap();

    keep_all(
        &root,
        &[
            chat(731, ID, "2026-11-12T11:00:00Z"),
            chat(732, OTHER, "2026-11-12T11:00:00Z"),
        ],
    )
    .unwrap();

    assert_eq!(
        every(&root)
            .iter()
            .map(|one| (one.number, one.id.as_str()))
            .collect::<Vec<_>>(),
        [(731, ID), (732, OTHER), (650, THIRD)]
    );
}

#[test]
fn chats_kept_together_are_refused_whole_where_one_does_not_hold_together() {
    let (_dir, root) = project();
    let refused = keep_all(
        &root,
        &[
            chat(731, ID, "2026-11-12T11:00:00Z"),
            chat(732, "732", "2026-11-12T11:00:00Z"),
        ],
    );
    assert!(refused.is_err());
    assert!(every(&root).is_empty());
}

#[test]
fn a_closed_chat_keeps_the_numbers_it_had_before_a_restart() {
    // #6486.
    let (_dir, root) = project();
    let restarted = ChatRecord {
        earlier: vec![650, 702],
        ..chat(731, ID, "2026-11-12T11:00:00Z")
    };
    keep(&root, &restarted).unwrap();
    assert_eq!(every(&root), vec![restarted.clone()]);
    assert_eq!(of_number(&every(&root), 650), Some(&restarted));
}

// #6485: a closed chat keeps what a reopen needs, and Reopen starts it again on its
// conversation.

const CONVERSATION: &str = "11111111-2222-4333-8444-555555555555";

fn as_it_ran(resume: Option<&str>) -> crate::reopen::Chat {
    crate::reopen::Chat {
        program: "claude".to_owned(),
        args: vec!["--model".to_owned(), "opus".to_owned()],
        name: "steward 702".to_owned(),
        label: Some("Tidy the changelog".to_owned()),
        persona: Some("steward".to_owned()),
        resume: resume.map(|id| crate::harness::SessionId::new(id).unwrap()),
        number: Some(702),
        earlier: vec![650],
        pid: Some(4242),
        active: true,
        identity: crate::reopen::Identity {
            id: Some(ID.to_owned()),
            run: Some(OTHER.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn closed_with(ran: crate::reopen::Chat) -> ChatRecord {
    ChatRecord {
        chat: Some(ran),
        earlier: vec![650],
        ..chat(702, ID, "2026-11-12T09:40:00Z")
    }
}

#[test]
fn a_closed_chat_reopens_on_its_conversation_as_the_same_chat_under_a_new_number() {
    let (_dir, root) = project();
    keep(&root, &closed_with(as_it_ran(Some(CONVERSATION)))).unwrap();
    let read = every(&root).remove(0);

    let again = read.reopened().expect("it reopens");

    assert_eq!(
        again.resume.map(|id| id.to_string()).as_deref(),
        Some(CONVERSATION)
    );
    assert_eq!(again.identity.id.as_deref(), Some(ID), "the same chat");
    assert_eq!(again.identity.run, None, "in a new run");
    assert_eq!(again.number, None, "a new number is dealt as it starts");
    assert_eq!(again.earlier, [650, 702], "its numbers still find it");
    assert_eq!(again.program, "claude");
    assert_eq!(again.args, ["--model", "opus"]);
    assert_eq!(again.label.as_deref(), Some("Tidy the changelog"));
    assert_eq!(again.persona.as_deref(), Some("steward"));
    assert_eq!(again.pid, None);
}

#[test]
fn a_closed_handoff_reopens_as_an_ordinary_chat_that_owes_nobody_anything() {
    let (_dir, root) = project();
    let handed = crate::reopen::Chat {
        from: Some(crate::reopen::HandedFrom {
            id: None,
            chat: 3,
            name: "steward 3".into(),
            workspace: crate::active::Place::Workspace("smart-ide".into()),
            report: crate::reopen::Owed::Due,
            mode: crate::reopen::Mode::Task,
            depth: 1,
            root: None,
            above: None,
            by_person: false,
            long: false,
        }),
        ..as_it_ran(Some(CONVERSATION))
    };
    keep(&root, &closed_with(handed)).unwrap();

    let again = every(&root).remove(0).reopened().expect("it reopens");

    assert_eq!(again.from, None);
    assert_eq!(
        again.reopened,
        Some(crate::reopen::ReopenedTask { by_person: false }),
        "its profile is asked again as it starts, as its dispatch's was"
    );
}

#[test]
fn a_closed_chat_the_person_started_reopens_as_theirs_and_a_reopened_task_stays_one() {
    let mine = closed_with(as_it_ran(Some(CONVERSATION)));
    assert_eq!(mine.reopened().unwrap().reopened, None);
    let task = closed_with(crate::reopen::Chat {
        reopened: Some(crate::reopen::ReopenedTask { by_person: true }),
        ..as_it_ran(Some(CONVERSATION))
    });
    assert_eq!(
        task.reopened().unwrap().reopened,
        Some(crate::reopen::ReopenedTask { by_person: true })
    );
}

#[test]
fn a_closed_chat_whose_harness_named_no_conversation_does_not_reopen() {
    let record = closed_with(as_it_ran(None));
    assert_eq!(record.reopened().unwrap_err(), CannotReopen::NoConversation);
}

#[test]
fn a_chat_closed_before_its_reopen_was_kept_does_not_reopen() {
    let record = chat(702, ID, "2026-11-12T09:40:00Z");
    assert_eq!(record.reopened().unwrap_err(), CannotReopen::NotKept);
}

/// A chat whose words are `bytes` long: what fills the file in a few closes.
fn ran_long(number: u32, id: &str, bytes: usize) -> ChatRecord {
    ChatRecord {
        chat: Some(crate::reopen::Chat {
            args: vec!["x".repeat(bytes)],
            ..as_it_ran(Some(CONVERSATION))
        }),
        ..chat(number, id, "2026-11-12T09:40:00Z")
    }
}

fn an_id(n: u32) -> String {
    ulid::Ulid::from_parts(1_700_000_000_000 + u64::from(n), u128::from(n)).to_string()
}

#[test]
fn the_file_is_never_written_larger_than_it_is_read_the_oldest_close_going_first() {
    let (_dir, root) = project();
    let each = usize::try_from(MOST_FILE_BYTES / 4).unwrap();
    for n in 1..=6 {
        keep(&root, &ran_long(700 + n, &an_id(n), each)).unwrap();
    }

    let written = std::fs::metadata(path(&root)).unwrap().len();
    assert!(written <= MOST_FILE_BYTES, "{written} bytes");
    let kept: Vec<u32> = every(&root).iter().map(|one| one.number).collect();
    assert_eq!(kept, [706, 705, 704], "the newest that fit, newest first");
}

#[test]
fn a_file_too_large_to_read_is_never_written_over() {
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let mut too_large = format!(
        r#"{{"v":1,"chats":[{}]}}"#,
        serde_json::to_string(&OnDisk::of(&chat(650, OTHER, "2026-11-12T09:00:00Z"))).unwrap()
    );
    too_large.push_str(&" ".repeat(usize::try_from(MOST_FILE_BYTES).unwrap()));
    std::fs::write(path(&root), &too_large).unwrap();

    assert!(keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).is_err());
    assert_eq!(sweep(&root, at("2027-06-01T00:00:00Z")), 0);

    assert_eq!(
        std::fs::read_to_string(path(&root)).unwrap(),
        too_large,
        "left as it was"
    );
}

#[test]
fn a_file_of_another_version_is_never_written_over() {
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let later = r#"{"v":2,"chats":[]}"#;
    std::fs::write(path(&root), later).unwrap();

    assert!(keep(&root, &chat(702, ID, "2026-11-12T09:40:00Z")).is_err());

    assert_eq!(std::fs::read_to_string(path(&root)).unwrap(), later);
}

#[test]
fn words_longer_than_a_record_keeps_are_cut_as_they_are_read() {
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let long = "x".repeat(10_000);
    let one = ChatRecord {
        title: long.clone(),
        place: long.clone(),
        started_by: StartedBy::Chat { name: long.clone() },
        ..chat(702, ID, "2026-11-12T09:40:00Z")
    };
    std::fs::write(
        path(&root),
        format!(
            r#"{{"v":1,"chats":[{}]}}"#,
            serde_json::to_string(&OnDisk::of(&one)).unwrap()
        ),
    )
    .unwrap();

    let read = every(&root).remove(0);

    assert_eq!(read.title.chars().count(), 200);
    assert_eq!(read.place.chars().count(), 200);
    let StartedBy::Chat { name } = read.started_by else {
        panic!("started by a chat");
    };
    assert_eq!(name.chars().count(), 200);
}

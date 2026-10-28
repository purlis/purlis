// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #3353 (L2): a grade a crash, a fault or a quit stopped is recorded as interrupted at the
//! project's next start, never left running and never lost.

use super::*;

/// This machine, as a test names it.
fn laptop() -> String {
    "laptop".to_owned()
}

const SKILL: &str = "---\nname: deploy\ndescription: Ships it.\n---\n";

/// A project with a `skills.local/` copy of `deploy`, so its grades stay on this machine.
fn project() -> (tempfile::TempDir, Level) {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("skills.local/deploy")).unwrap();
    std::fs::write(tmp.path().join("skills.local/deploy/SKILL.md"), SKILL).unwrap();
    (tmp, Level::project().local(true))
}

fn starting(id: &str, level: &Level) -> Graded {
    Graded {
        v: super::super::VERSION,
        grade: id.to_owned(),
        skill: "deploy".into(),
        level: level.rel(),
        tree: "uncommitted".into(),
        commit: None,
        harness: None,
        profile: Some("work".into()),
        ts: "2026-10-27T01:00:00+00:00".into(),
        runs: 3,
        report: interrupted_report(50_000, Some(1.5), None),
        spent: Spent::default(),
        plugins_off: None,
        model: None,
    }
}

#[test]
fn a_grade_purlis_stopped_under_is_recorded_interrupted_at_the_next_start() {
    let (tmp, level) = project();
    let root = tmp.path();
    let held = began(root, &level, &starting("g-crashed", &level))
        .expect("held")
        .into_path();
    assert!(held.exists());
    assert_eq!(interrupted(root, laptop, |_| false), ["g-crashed"]);
    assert!(!held.exists(), "held under way no more");
    let read = super::super::grades_of(root, "deploy", Some(&level));
    assert_eq!(read.len(), 1, "{read:?}");
    assert_eq!(read[0].grade, "g-crashed");
    assert_eq!(read[0].report.ended, GradeEnd::Interrupted);
    assert_eq!(read[0].report.cap, Some(50_000));
    assert!(
        interrupted(root, laptop, |_| false).is_empty(),
        "recorded once"
    );
}

#[test]
fn a_grade_that_ended_is_not_recorded_again() {
    let (tmp, level) = project();
    let root = tmp.path();
    began(root, &level, &starting("g-done", &level)).expect("held");
    ended(root, "g-done");
    assert!(interrupted(root, laptop, |_| false).is_empty());
    assert!(super::super::grades_of(root, "deploy", Some(&level)).is_empty());
}

#[test]
fn a_grade_whose_marker_is_locked_is_left_to_whoever_holds_it_3466() {
    // The grade holds its marker's lock while it runs: that is what says it runs, on every
    // system, and not its process's number. Here `runs_here` says no grade runs, and the held
    // lock still wins.
    let (tmp, level) = project();
    let root = tmp.path();
    let marker = began(root, &level, &starting("g-theirs", &level)).expect("held");
    assert!(
        interrupted(root, laptop, |_| false).is_empty(),
        "a held lock reads as running"
    );
    assert!(marker.exists(), "left as it is");
    // Its process ends, and the lock with it: the next start records it.
    drop(marker);
    assert_eq!(interrupted(root, laptop, |_| false), ["g-theirs"]);
}

#[test]
fn a_held_file_that_is_not_a_sound_row_of_its_own_name_is_taken_away_unrecorded() {
    let (tmp, level) = project();
    let root = tmp.path();
    let held = began(root, &level, &starting("g-one", &level))
        .expect("held")
        .into_path();
    std::fs::rename(&held, held.with_file_name("g-other.json")).unwrap();
    std::fs::write(held.with_file_name("g-junk.json"), "{ not json").unwrap();
    assert!(interrupted(root, laptop, |_| false).is_empty());
    assert!(
        std::fs::read_dir(held.parent().unwrap())
            .unwrap()
            .next()
            .is_none()
    );
    assert!(began(root, &level, &starting("../escape", &level)).is_err());
}

#[test]
fn a_grade_this_purlis_runs_now_is_left_to_it() {
    let (tmp, level) = project();
    let root = tmp.path();
    let held = began(root, &level, &starting("g-mine", &level)).expect("held");
    assert!(interrupted(root, laptop, |grade| grade == "g-mine").is_empty());
    assert!(held.exists(), "left to the grade that runs it");
}

#[test]
fn a_grade_whose_row_cannot_be_written_is_kept_for_the_next_start() {
    let (tmp, level) = project();
    let root = tmp.path();
    let held = began(root, &level, &starting("g-kept", &level))
        .expect("held")
        .into_path();
    // This machine's record folder is a file: no row can go there.
    let record = crate::plane::state_dir(root).join(super::super::LOCAL_DIR);
    std::fs::write(&record, "not a folder").unwrap();
    assert!(interrupted(root, laptop, |_| false).is_empty());
    assert!(held.exists(), "kept, never lost");
    std::fs::remove_file(&record).unwrap();
    assert_eq!(interrupted(root, laptop, |_| false), ["g-kept"]);
}

/// A project's start with no grade under way, or with one of a copy that is this machine's
/// alone, never asks for this machine's name, which reads its store (a fenced test's start
/// would be refused there).
#[test]
fn a_start_asks_this_machine_s_store_nothing_but_for_a_committed_row() {
    let never = || -> String { panic!("this machine's store was read") };
    let (tmp, level) = project();
    let root = tmp.path();
    assert!(
        interrupted(root, never, |_| false).is_empty(),
        "nothing under way"
    );
    began(root, &level, &starting("g-local", &level)).expect("held");
    assert_eq!(interrupted(root, never, |_| false), ["g-local"]);
}

/// `held`'s file with its process set to `pid`.
fn held_by(held: &Path, pid: u32) {
    let mut row: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(held).unwrap()).unwrap();
    row["pid"] = serde_json::json!(pid);
    std::fs::write(held, row.to_string()).unwrap();
}

#[test]
fn a_crashed_grade_is_recorded_though_another_process_now_has_its_pid_3466() {
    let (tmp, level) = project();
    let root = tmp.path();
    let held = began(root, &level, &starting("g-reused", &level))
        .expect("held")
        .into_path();
    // Process 1 runs on every unix: the pid a crashed purlis left is someone else's now.
    held_by(&held, 1);
    assert_eq!(interrupted(root, laptop, |_| false), ["g-reused"]);
    assert!(!held.exists());
}

#[test]
fn a_grade_that_could_not_lock_its_marker_is_read_by_its_process_3466() {
    // A marker its grade never locked: a free lock says nothing, and the process number is
    // read, as before #3466. This process runs it.
    let (tmp, level) = project();
    let root = tmp.path();
    let held = began(root, &level, &starting("g-unlocked", &level))
        .expect("held")
        .into_path();
    let mut row: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&held).unwrap()).unwrap();
    assert_eq!(row["locked"], serde_json::json!(true), "began locked it");
    row["locked"] = serde_json::json!(false);
    std::fs::write(&held, row.to_string()).unwrap();
    assert!(interrupted(root, laptop, |grade| grade == "g-unlocked").is_empty());
    assert!(held.exists(), "left to the grade this process runs");
    assert_eq!(interrupted(root, laptop, |_| false), ["g-unlocked"]);
}

#[test]
fn a_start_while_another_records_the_folder_waits_its_turn_and_records_once_3443() {
    // Two purlis processes starting one project at once: the second waits while the first holds
    // the folder's claim, then finds the stale grade recorded and records it never again.
    let (tmp, level) = project();
    let root = tmp.path().to_path_buf();
    // Its grade's process is gone, and its marker's lock with it (#3466).
    let held = began(&root, &level, &starting("g-both", &level))
        .expect("held")
        .into_path();
    let theirs = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(crate::plane::state_dir(&root).join(CLAIM))
        .unwrap();
    theirs.lock().unwrap();

    let waiting = std::thread::spawn({
        let root = root.clone();
        move || interrupted(&root, laptop, |_| false)
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(!waiting.is_finished(), "it waits for the claim");
    assert!(
        held.exists(),
        "nothing recorded while another holds the claim"
    );

    drop(theirs);
    assert_eq!(waiting.join().unwrap(), ["g-both"]);
    assert!(interrupted(&root, laptop, |_| false).is_empty());
    assert_eq!(
        super::super::grades_of(&root, "deploy", Some(&level)).len(),
        1,
        "recorded once"
    );
}

/// #3582: a grade's marker says what its chats spent so far, so a grade purlis stops under is
/// recorded with that spend, and not with nothing.
#[test]
fn a_grade_stopped_after_its_chats_spent_is_recorded_with_what_they_spent_3582() {
    let (tmp, level) = project();
    let root = tmp.path();
    let mut marker = began(root, &level, &starting("g-spent", &level)).expect("held");
    let spent = Spent {
        chats: 2,
        tokens: Some(12_000),
        cost_usd: Some(0.12),
    };
    marker.spent(spent).expect("rewritten");
    assert!(
        interrupted(root, laptop, |_| false).is_empty(),
        "the rewritten marker is locked as the first was"
    );
    let folder = marker.parent().unwrap().to_path_buf();
    let names: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        names,
        ["g-spent.json"],
        "nothing half-written is left beside it"
    );
    drop(marker);
    assert_eq!(interrupted(root, laptop, |_| false), ["g-spent"]);
    let read = super::super::grades_of(root, "deploy", Some(&level));
    assert_eq!(read.len(), 1, "{read:?}");
    assert_eq!(read[0].report.ended, GradeEnd::Interrupted);
    assert_eq!(read[0].spent, spent);
    assert_eq!(read[0].report.spent_tokens, Some(12_000));
    assert_eq!(read[0].report.spent_usd, Some(0.12));
    assert!(read[0].report.arms.is_empty(), "no chat is scored");
}

/// #3582: a spend no sound row could hold is never written, so the marker keeps the last one it
/// could and the grade is still recorded, never taken away as unsound.
#[test]
fn a_spend_no_row_could_hold_leaves_the_marker_as_it_was_3582() {
    let (tmp, level) = project();
    let root = tmp.path();
    let mut marker = began(root, &level, &starting("g-past", &level)).expect("held");
    let first = Spent {
        chats: 1,
        tokens: Some(5_000),
        cost_usd: None,
    };
    marker.spent(first).expect("rewritten");
    let past = Spent {
        chats: 2,
        tokens: Some(u64::MAX),
        cost_usd: None,
    };
    assert!(marker.spent(past).is_err(), "refused");
    drop(marker);
    assert_eq!(interrupted(root, laptop, |_| false), ["g-past"]);
    let read = super::super::grades_of(root, "deploy", Some(&level));
    assert_eq!(read[0].spent, first, "the last spend it could hold");
}

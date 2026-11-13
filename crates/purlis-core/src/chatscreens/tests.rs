// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::time::{Duration, SystemTime};

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

/// A project whose local settings git ignores, as `purlis init` leaves one.
fn settable() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = project();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(root.join(".gitignore"), "charter.local.toml\n").unwrap();
    crate::testgit::run(&root, &["init", "-q", "-b", "main", "."]);
    (dir, root)
}

const CHAT: &str = "01K6W0RKER000000000000000B";
const OTHER: &str = "01K6W0RKER000000000000000C";

#[test]
fn a_kept_screen_reads_back_as_it_was_kept() {
    let (_dir, root) = project();
    keep(&root, CHAT, "\x1b[1mSigned all 12.\x1b[0m\r\n").unwrap();
    assert_eq!(
        kept(&root, CHAT).as_deref(),
        Some("\x1b[1mSigned all 12.\x1b[0m\r\n")
    );
    assert!(is_kept(&root, CHAT));
}

#[test]
fn nothing_kept_reads_as_none() {
    let (_dir, root) = project();
    assert_eq!(kept(&root, CHAT), None);
    assert!(!is_kept(&root, CHAT));
}

#[test]
fn a_name_that_is_not_a_ulid_is_refused() {
    let (_dir, root) = project();
    assert!(keep(&root, "../escape", "x").is_err());
    assert_eq!(kept(&root, "../escape"), None);
}

#[test]
fn a_screen_past_the_cap_is_not_kept() {
    let (_dir, root) = project();
    let big = "x".repeat(MOST_SCREEN_BYTES + 1);
    assert!(keep(&root, CHAT, &big).is_err());
    assert!(!is_kept(&root, CHAT));
}

#[cfg(unix)]
#[test]
fn a_kept_screen_is_private() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = project();
    keep(&root, CHAT, "x").unwrap();
    let file = std::fs::metadata(dir(&root).join(format!("{CHAT}.screen"))).unwrap();
    assert_eq!(file.permissions().mode() & 0o777, 0o600);
    let folder = std::fs::metadata(dir(&root)).unwrap();
    assert_eq!(folder.permissions().mode() & 0o777, 0o700);
}

#[cfg(unix)]
#[test]
fn a_link_in_the_store_is_never_read() {
    let (_dir, root) = project();
    keep(&root, OTHER, "x").unwrap();
    let elsewhere = root.join("secret.txt");
    std::fs::write(&elsewhere, "not a screen").unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir(&root).join(format!("{CHAT}.screen"))).unwrap();
    assert_eq!(kept(&root, CHAT), None);
}

#[test]
fn the_sweep_takes_the_screens_no_record_names() {
    let (_dir, root) = project();
    keep(&root, CHAT, "kept").unwrap();
    keep(&root, OTHER, "gone").unwrap();
    assert_eq!(sweep(&root, |id| id == CHAT, SystemTime::now()), 1);
    assert!(is_kept(&root, CHAT));
    assert!(!is_kept(&root, OTHER));
}

#[test]
fn forgetting_all_takes_every_screen() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    keep(&root, OTHER, "b").unwrap();
    assert_eq!(forget_all(&root), Ok(2));
    assert!(!is_kept(&root, CHAT) && !is_kept(&root, OTHER));
}

#[test]
fn a_project_s_open_keeps_the_screens_its_records_name_and_takes_the_rest() {
    // With no record of open chats at all, which skips the rest of the open's sweep: a screen
    // is swept whatever that record says.
    use crate::dispatchrecord::{self, ChatRef, Mode, Opening, Worker};
    let (_dir, root) = project();
    let opening = Opening {
        mode: Mode::Task,
        asker: Default::default(),
        persona: None,
        worker: Worker {
            chat: ChatRef {
                chat: 641,
                id: Some(CHAT.to_owned()),
                ..ChatRef::default()
            },
            ..Worker::default()
        },
        task: Some("task".to_owned()),
        place: Default::default(),
        brief: "brief".to_owned(),
        report_owed: true,
    };
    dispatchrecord::open(&root, opening, chrono::Utc::now()).unwrap();
    keep(&root, CHAT, "named").unwrap();
    keep(&root, OTHER, "named by nothing").unwrap();
    crate::retention::on_open(&root, std::time::SystemTime::now());
    assert!(is_kept(&root, CHAT));
    assert!(!is_kept(&root, OTHER));
}

#[test]
fn screens_are_kept_where_nothing_is_set() {
    let (_dir, root) = project();
    assert!(keeps(&root));
}

#[test]
fn turning_keeping_off_takes_every_kept_screen_and_keeps_no_more() {
    let (_dir, root) = settable();
    keep(&root, CHAT, "a").unwrap();
    set_keep(&root, false).unwrap();
    assert!(!keeps(&root));
    assert!(!is_kept(&root, CHAT));
    assert!(keep(&root, OTHER, "b").is_err());
    assert!(!is_kept(&root, OTHER));
    set_keep(&root, true).unwrap();
    keep(&root, OTHER, "b").unwrap();
    assert!(is_kept(&root, OTHER));
}

#[test]
fn a_hand_edit_that_turns_it_off_is_caught_by_the_sweep() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    std::fs::write(
        root.join(crate::settings::Which::Local.file_at(&root)),
        "[task_screens]\nkeep = false\n",
    )
    .unwrap();
    assert_eq!(sweep(&root, |_| true, SystemTime::now()), 1);
    assert!(!is_kept(&root, CHAT));
}

#[test]
fn the_setting_in_the_committed_file_is_refused() {
    let (_dir, root) = project();
    let said = keyed_at(
        &root,
        "[task_screens]\nkeep = false\n",
        crate::settings::Which::Shared,
    );
    assert_eq!(said.len(), 1);
    assert!(
        keyed_at(
            &root,
            "[task_screens]\nkeep = false\n",
            crate::settings::Which::Local
        )
        .is_empty()
    );
    assert_eq!(
        keyed_at(
            &root,
            "[task_screens]\nkeep = 1\n",
            crate::settings::Which::Local
        )
        .len(),
        1
    );
}

#[test]
fn off_by_a_hand_edit_a_read_shows_nothing_and_deletes_every_screen() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    std::fs::write(
        root.join(crate::settings::Which::Local.file_at(&root)),
        "[task_screens]\nkeep = false\n",
    )
    .unwrap();
    assert!(!is_kept(&root, CHAT));
    assert_eq!(kept(&root, CHAT), None);
    assert!(!dir(&root).join(format!("{CHAT}.screen")).exists());
}

#[test]
fn off_by_a_hand_edit_the_project_s_open_deletes_every_screen() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    std::fs::write(
        root.join(crate::settings::Which::Local.file_at(&root)),
        "[task_screens]\nkeep = false\n",
    )
    .unwrap();
    crate::retention::on_open(&root, SystemTime::now());
    assert!(!dir(&root).join(format!("{CHAT}.screen")).exists());
}

/// A ULID for the `n`th screen, each its own.
fn nth(n: usize) -> String {
    let mut id = format!("01K6W0RKER{n:016}");
    id.truncate(26);
    id
}

#[test]
fn past_the_total_count_the_oldest_screens_go_first() {
    let (_dir, root) = project();
    for n in 0..MOST_KEPT {
        keep(&root, &nth(n), "x").unwrap();
        let old = SystemTime::now() - Duration::from_secs(10_000 - n as u64);
        std::fs::File::options()
            .write(true)
            .open(dir(&root).join(format!("{}.screen", nth(n))))
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    keep(&root, CHAT, "newest").unwrap();
    assert!(!is_kept(&root, &nth(0)), "the oldest went");
    assert!(is_kept(&root, &nth(1)));
    assert!(is_kept(&root, CHAT));
}

#[test]
fn a_screen_not_written_for_its_record_s_age_goes_whatever_names_it() {
    let (_dir, root) = project();
    keep(&root, CHAT, "old").unwrap();
    keep(&root, OTHER, "new").unwrap();
    let old = SystemTime::now() - crate::retention::KEEP_FOR - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(dir(&root).join(format!("{CHAT}.screen")))
        .unwrap()
        .set_modified(old)
        .unwrap();
    assert_eq!(sweep(&root, |_| true, SystemTime::now()), 1);
    assert!(!is_kept(&root, CHAT));
    assert!(is_kept(&root, OTHER));
}

#[test]
fn a_screen_last_written_exactly_thirty_days_ago_goes() {
    // plane-format.md: "last written 30 days or more before", as a dispatch record is.
    let (_dir, root) = project();
    keep(&root, CHAT, "old").unwrap();
    let written = SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(dir(&root).join(format!("{CHAT}.screen")))
        .unwrap()
        .set_modified(written)
        .unwrap();
    let now = written + crate::retention::KEEP_FOR;
    assert_eq!(sweep(&root, |_| true, now), 1);
    assert!(!is_kept(&root, CHAT));
}

#[test]
fn a_screen_is_replaced_whole_and_leaves_no_temp_file() {
    let (_dir, root) = project();
    keep(&root, CHAT, "first").unwrap();
    keep(&root, CHAT, "second").unwrap();
    assert_eq!(kept(&root, CHAT).as_deref(), Some("second"));
    let names: Vec<String> = std::fs::read_dir(dir(&root))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec![format!("{CHAT}.screen")]);
}

#[test]
fn what_the_store_holds_is_counted_and_nothing_where_it_is_off() {
    let (_dir, root) = settable();
    assert_eq!(held(&root), (0, 0));
    keep(&root, CHAT, "abc").unwrap();
    keep(&root, OTHER, "de").unwrap();
    assert_eq!(held(&root), (2, 5));
    std::fs::write(
        root.join(crate::settings::Which::Local.file_at(&root)),
        "[task_screens]\nkeep = false\n",
    )
    .unwrap();
    // Off by a hand edit: the count is read as nothing, and what was kept goes.
    assert_eq!(held(&root), (0, 0));
    assert!(every_kept(&root).is_empty());
}

/// What a write of chat `id`'s screen that was killed before its rename leaves in the store, as
/// [`crate::rewrite::replace`] names it, last written `age` ago.
fn a_killed_write_s_temp(root: &Path, id: &str, age: Duration) -> PathBuf {
    std::fs::create_dir_all(dir(root)).unwrap();
    let temp = dir(root).join(format!(
        "{}{id}{SUFFIX}.4242.k1.tmp",
        crate::rewrite::TEMP_PREFIX
    ));
    std::fs::write(&temp, "a whole screen, never renamed").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&temp)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
    temp
}

#[test]
fn turning_keeping_off_takes_a_killed_write_s_temp_and_leaves_a_write_under_way() {
    // #6451's review: a temp left by a write killed before its rename holds a whole screen, and
    // nothing named `<ULID>.screen` points at it.
    let (_dir, root) = settable();
    let killed = a_killed_write_s_temp(&root, CHAT, LOCK_WAIT + Duration::from_secs(1));
    let under_way = a_killed_write_s_temp(&root, OTHER, Duration::ZERO);

    set_keep(&root, false).unwrap();

    assert!(!killed.exists(), "the killed write's screen goes");
    assert!(
        under_way.exists(),
        "a write younger than the lock wait may be live"
    );
}

#[test]
fn the_sweep_at_open_takes_a_killed_write_s_temp_and_leaves_a_write_under_way() {
    let (_dir, root) = project();
    let killed = a_killed_write_s_temp(&root, CHAT, LOCK_WAIT + Duration::from_secs(1));
    let under_way = a_killed_write_s_temp(&root, OTHER, Duration::ZERO);

    sweep(&root, |_| true, SystemTime::now());

    assert!(!killed.exists());
    assert!(under_way.exists());
}

#[test]
fn a_temp_of_another_store_s_name_is_never_taken() {
    let (_dir, root) = project();
    std::fs::create_dir_all(dir(&root)).unwrap();
    let not_ours = dir(&root).join(format!(
        "{}notes.txt.4242.k1.tmp",
        crate::rewrite::TEMP_PREFIX
    ));
    std::fs::write(&not_ours, "x").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&not_ours)
        .unwrap()
        .set_modified(SystemTime::now() - LOCK_WAIT - Duration::from_secs(1))
        .unwrap();

    assert_eq!(forget_all(&root), Ok(0));

    assert!(not_ours.exists());
}

#[cfg(unix)]
#[test]
fn a_screen_that_could_not_be_deleted_is_said_and_still_counted() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = settable();
    keep(&root, CHAT, "abc").unwrap();
    // The folder lets nothing in it be deleted.
    std::fs::set_permissions(dir(&root), std::fs::Permissions::from_mode(0o500)).unwrap();

    let turned_off = set_keep(&root, false);
    let counted = held(&root);
    std::fs::set_permissions(dir(&root), std::fs::Permissions::from_mode(0o700)).unwrap();

    let said = turned_off.expect_err("a screen stayed");
    assert!(
        said.starts_with("1 kept screen could not be deleted"),
        "{said}"
    );
    assert!(!keeps(&root), "the setting is off all the same");
    assert_eq!(counted, (1, 3), "the row counts what is really there");
}

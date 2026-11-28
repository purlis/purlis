// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

const MINUTE: Duration = Duration::from_secs(60);

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

fn local(root: &Path, text: &str) {
    std::fs::write(root.join(Which::Local.file_at(root)), text).unwrap();
}

#[test]
fn thirty_minutes_where_nothing_is_set() {
    let (_dir, root) = project();
    assert_eq!(of(&root), Some(30 * MINUTE));
    local(&root, "[plane]\nmode = \"commit\"\n");
    assert_eq!(of(&root), Some(30 * MINUTE));
}

#[test]
fn forty_five_minutes_and_two_hours_are_read() {
    assert_eq!(read("45m"), Ok(Some(45 * MINUTE)));
    assert_eq!(read("2h"), Ok(Some(120 * MINUTE)));
    assert_eq!(
        read("10m"),
        Ok(Some(10 * MINUTE)),
        "the floor itself is allowed"
    );
    assert_eq!(read("600s"), Ok(Some(10 * MINUTE)));
    let (_dir, root) = project();
    local(&root, "[chats]\nhibernate_after = \"2h\"\n");
    assert_eq!(of(&root), Some(120 * MINUTE));
}

#[test]
fn off_is_read_as_nothing_hibernated() {
    assert_eq!(read("off"), Ok(None));
    let (_dir, root) = project();
    local(&root, "[chats]\nhibernate_after = \"off\"\n");
    assert_eq!(of(&root), None);
}

#[test]
fn a_value_under_ten_minutes_is_refused_with_the_sentence() {
    for under in ["9m", "599s", "1s"] {
        assert_eq!(
            read(under),
            Err(format!(
                "{under} is under 10 minutes, the least it may be. Set 10m or more, or off."
            )),
        );
    }
}

#[test]
fn a_value_of_another_shape_is_refused_and_says_what_to_write() {
    for word in ["soon", "", "30", "1.5h", "-30m", "Off", "30 m"] {
        let why = read(word).unwrap_err();
        assert!(
            why.ends_with("such as 45m or 2h, or off."),
            "{word:?}: {why}"
        );
    }
}

#[test]
fn a_value_that_does_not_read_leaves_the_default_and_says_why() {
    let (_dir, root) = project();
    for value in ["\"5m\"", "\"soon\"", "30"] {
        let text = format!("[chats]\nhibernate_after = {value}\n");
        local(&root, &text);
        assert_eq!(of(&root), Some(DEFAULT), "{value}");
        let said = keyed_at(&root, &text, Which::Local);
        assert_eq!(said.len(), 1, "{value}: {said:?}");
        assert!(
            said[0]
                .why
                .starts_with("[chats] hibernate_after in charter.local.toml is not read, so chats hibernate after 30m."),
            "{}",
            said[0].why
        );
    }
    let fine = "[chats]\nhibernate_after = \"45m\"\n";
    assert!(keyed_at(&root, fine, Which::Local).is_empty());
    let off = "[chats]\nhibernate_after = \"off\"\n";
    assert!(keyed_at(&root, off, Which::Local).is_empty());
}

#[test]
fn the_setting_in_the_committed_file_or_another_key_is_refused() {
    let (_dir, root) = project();
    let text = "[chats]\nhibernate_after = \"45m\"\n";
    assert_eq!(keyed_at(&root, text, Which::Shared).len(), 1);
    let other = keyed_at(&root, "[chats]\nwake_after = \"1h\"\n", Which::Local);
    assert_eq!(other.len(), 1, "{other:?}");
    assert_eq!(keyed_at(&root, "chats = 3\n", Which::Local).len(), 1);
}

#[test]
fn setting_it_writes_the_local_file_and_keeps_every_other_line() {
    let (_dir, root) = settable();
    local(&root, "# mine\n[plane]\nmode = \"commit\"\n");
    assert_eq!(set(&root, "45m"), Ok(Some(45 * MINUTE)));
    assert_eq!(of(&root), Some(45 * MINUTE));
    assert_eq!(set(&root, "off"), Ok(None));
    assert_eq!(of(&root), None);
    let text = std::fs::read_to_string(root.join(Which::Local.file_at(&root))).unwrap();
    assert!(
        text.starts_with("# mine\n[plane]\nmode = \"commit\"\n"),
        "{text}"
    );
    assert!(
        text.contains("[chats]\nhibernate_after = \"off\"\n"),
        "{text}"
    );
}

#[test]
fn setting_a_value_under_ten_minutes_writes_nothing() {
    let (_dir, root) = settable();
    local(&root, "[chats]\nhibernate_after = \"45m\"\n");
    assert!(set(&root, "5m").is_err());
    assert_eq!(of(&root), Some(45 * MINUTE));
}

#[test]
fn it_is_spelled_back_as_the_files_spell_it() {
    assert_eq!(spelled_as(Some(DEFAULT)), "30m");
    assert_eq!(spelled_as(Some(120 * MINUTE)), "2h");
    assert_eq!(spelled_as(None), "off");
}

#[test]
fn a_kept_read_reads_a_hand_edit_once_the_file_changed() {
    let (_dir, root) = project();
    let kept = Kept::default();
    assert_eq!(kept.at(&root), Some(DEFAULT));
    local(&root, "[chats]\nhibernate_after = \"off\"\n");
    assert_eq!(
        kept.at(&root),
        None,
        "a file that was not there is there now"
    );
    let file = root.join(Which::Local.file_at(&root));
    let stamp = std::fs::metadata(&file).unwrap().modified().unwrap();
    local(&root, "[chats]\nhibernate_after = \"2h\"\n");
    // The same modified time: what was read stands until it is forgotten.
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(stamp)
        .unwrap();
    assert_eq!(kept.at(&root), None);
    kept.forget();
    assert_eq!(kept.at(&root), Some(120 * MINUTE));
}

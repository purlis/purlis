// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::time::{Duration, Instant};

const KEPT: &str = r#"{"permissions": {"deny": ["Bash(rm -rf *)"]}}"#;
const NOT_KEPT: &str = r#"{"permissions": {"allow": ["Bash"]}}"#;

fn write(dir: &Path, text: &str) {
    std::fs::create_dir_all(dir.join(".claude")).expect(".claude");
    std::fs::write(dir.join(".claude/settings.json"), text).expect("written");
}

/// A watch that keeps each chat it tells.
fn telling() -> (Watch, Arc<Mutex<Vec<u32>>>) {
    let watch = Watch::new();
    let told = Arc::new(Mutex::new(Vec::new()));
    watch.telling({
        let told = Arc::clone(&told);
        Arc::new(move |session| told.lock().expect("told").push(session))
    });
    (watch, told)
}

fn told_within(told: &Arc<Mutex<Vec<u32>>>, session: u32) -> bool {
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        if told.lock().expect("told").contains(&session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn a_change_the_screen_no_longer_passes_is_told_and_the_chat_watched_no_more() {
    let dir = tempfile::tempdir().expect("a project");
    write(dir.path(), KEPT);
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    watch.chat(7, dir.path());
    assert!(
        told.lock().expect("told").is_empty(),
        "it passes at its start"
    );

    write(dir.path(), NOT_KEPT);
    assert!(told_within(&told, 7), "the change is heard and told");
    assert_eq!(watch.watched(), Vec::<u32>::new());
    assert_eq!(*told.lock().expect("told"), [7], "told once");
}

#[test]
fn a_change_that_still_passes_changes_nothing() {
    let dir = tempfile::tempdir().expect("a project");
    write(dir.path(), KEPT);
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    watch.chat(7, dir.path());
    write(
        dir.path(),
        r#"{"permissions": {"ask": ["Bash(git push *)"]}, "model": "sonnet"}"#,
    );
    watch.screen_all();
    std::thread::sleep(Duration::from_millis(1000));
    assert!(told.lock().expect("told").is_empty());
    assert_eq!(watch.watched(), [7]);
}

#[test]
fn a_settings_folder_made_after_the_start_is_watched_too() {
    let dir = tempfile::tempdir().expect("a project");
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    watch.chat(3, dir.path());
    std::fs::create_dir_all(dir.path().join(".claude")).expect(".claude");
    // Heard as the folder is made, and listened to from then on.
    std::thread::sleep(Duration::from_millis(1000));
    std::fs::write(dir.path().join(".claude/settings.local.json"), NOT_KEPT).expect("written");
    assert!(told_within(&told, 3));
}

#[test]
fn a_change_between_the_prepare_and_the_start_is_not_missed() {
    let dir = tempfile::tempdir().expect("a project");
    write(dir.path(), KEPT);
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    write(dir.path(), NOT_KEPT);
    watch.chat(5, dir.path());
    assert!(told_within(&told, 5));
}

#[test]
fn where_nothing_can_be_watched_the_prepare_is_refused() {
    let dir = tempfile::tempdir().expect("a project");
    assert!(Watch::unable().prepare(dir.path()).is_err());
}

#[test]
fn a_chat_forgotten_is_told_nothing() {
    let dir = tempfile::tempdir().expect("a project");
    write(dir.path(), KEPT);
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    watch.chat(9, dir.path());
    watch.forget(9);
    write(dir.path(), NOT_KEPT);
    watch.screen_all();
    std::thread::sleep(Duration::from_millis(1000));
    assert!(told.lock().expect("told").is_empty());
}

#[test]
fn a_file_renamed_over_the_settings_is_heard() {
    // How an editor saves, and how a checkout writes a file.
    let dir = tempfile::tempdir().expect("a project");
    write(dir.path(), KEPT);
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    watch.chat(4, dir.path());
    let saved = dir.path().join(".claude/.settings.json.tmp");
    std::fs::write(&saved, NOT_KEPT).expect("written beside it");
    std::fs::rename(&saved, dir.path().join(".claude/settings.json")).expect("renamed over it");
    assert!(told_within(&told, 4));
}

#[test]
fn a_settings_folder_taken_away_and_made_again_is_still_watched() {
    // As a checkout of a branch without it, and back, does.
    let dir = tempfile::tempdir().expect("a project");
    write(dir.path(), KEPT);
    let (watch, told) = telling();
    watch.prepare(dir.path()).expect("watched");
    watch.chat(6, dir.path());
    std::fs::remove_dir_all(dir.path().join(".claude")).expect("taken away");
    std::thread::sleep(Duration::from_millis(1000));
    write(dir.path(), KEPT);
    std::thread::sleep(Duration::from_millis(1000));
    assert!(told.lock().expect("told").is_empty(), "it passed each time");
    // A later change inside the folder made again is heard.
    std::fs::write(dir.path().join(".claude/settings.json"), NOT_KEPT).expect("written");
    assert!(told_within(&told, 6));
}

#[test]
fn the_local_file_at_the_top_of_the_repository_is_watched() {
    let dir = tempfile::tempdir().expect("a folder");
    let repo = dir.path().join("repo");
    let chat = repo.join("src");
    std::fs::create_dir_all(repo.join(".git")).expect("a repository");
    std::fs::create_dir_all(repo.join(".claude")).expect(".claude");
    std::fs::create_dir_all(&chat).expect("the chat's folder");
    let (watch, told) = telling();
    watch.prepare(&chat).expect("watched");
    watch.chat(8, &chat);
    std::fs::write(repo.join(".claude/settings.local.json"), NOT_KEPT).expect("written");
    assert!(told_within(&told, 8));
}

#[test]
fn reads_and_other_files_are_not_screened_for() {
    use notify::event::{AccessKind, CreateKind, EventKind};
    let event = |kind: EventKind, path: &str| {
        Ok(notify::Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: notify::event::EventAttributes::new(),
        })
    };
    assert!(!screened_for(&event(
        EventKind::Access(AccessKind::Any),
        "/p/.claude/settings.json"
    )));
    assert!(!screened_for(&event(
        EventKind::Create(CreateKind::File),
        "/p/README.md"
    )));
    for path in [
        "/p/.claude",
        "/p/.CLAUDE",
        "/p/.claude/settings.json",
        "/p/.claude/Settings.Local.json",
    ] {
        assert!(
            screened_for(&event(EventKind::Create(CreateKind::Any), path)),
            "{path}"
        );
    }
    assert!(screened_for(&Err(notify::Error::generic("lost"))));
}

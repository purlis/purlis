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
    std::thread::sleep(Duration::from_millis(500));
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
    std::thread::sleep(Duration::from_millis(500));
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
    std::thread::sleep(Duration::from_millis(500));
    assert!(told.lock().expect("told").is_empty());
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn the_app_is_the_topmost_ancestor_running_its_program_and_those_directly_below_it() {
    // Parent first: a harness (10), a session host (20) and its window (30), then launchd's child.
    assert_eq!(
        under_of(&[10, 20, 30, 40], &[false, true, true, false]),
        Under::App(vec![30, 20])
    );
    // A `purlis serve` the chat started (5), below the chat's shell (6): never the app's.
    assert_eq!(
        under_of(&[5, 6, 10, 20, 30], &[true, false, false, true, true]),
        Under::App(vec![30, 20])
    );
    assert_eq!(under_of(&[5, 6], &[false, false]), Under::NoApp);
    assert_eq!(under_of(&[], &[]), Under::NoApp);
}

#[test]
fn no_app_above_and_no_chat_named_is_the_person_s_terminal() {
    let tmp = tempfile::tempdir().unwrap();
    let none = |_: &str| None;
    assert_eq!(
        reach_from(Under::NoApp, tmp.path(), &none, Duration::from_millis(50)).unwrap_err(),
        Unreached::NotAChat
    );
}

#[test]
fn a_chat_named_with_no_app_above_is_answered_strictly_and_never_run_here() {
    let tmp = tempfile::tempdir().unwrap();
    let chat = |name: &str| (name == CHAT_ENV).then(|| "3".to_owned());
    let unreached =
        reach_from(Under::NoApp, tmp.path(), &chat, Duration::from_millis(50)).unwrap_err();
    assert!(matches!(unreached, Unreached::AppGone(_)), "{unreached:?}");
    assert!(unreached.said().unwrap().starts_with(NO_APP_ANSWERED));
}

#[test]
fn a_boxed_process_with_no_app_is_a_chat_s_even_with_its_environment_emptied() {
    let tmp = tempfile::tempdir().unwrap();
    let none = |_: &str| None;
    let unreached =
        reach_from(Under::Boxed, tmp.path(), &none, Duration::from_millis(50)).unwrap_err();
    assert!(matches!(unreached, Unreached::AppGone(_)), "{unreached:?}");
}

#[test]
fn a_folder_this_process_may_write_is_told_from_one_it_may_not() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!may_not_write(tmp.path()));
    assert_eq!(
        std::fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "the try is removed"
    );
}

#[test]
fn this_purlis_s_own_program_is_the_app_s_and_another_is_not() {
    let me = std::env::current_exe().unwrap();
    assert!(trusted_program(&me));
    assert!(!trusted_program(Path::new("/bin/sh")));
}

#[test]
fn whoami_and_its_answer_are_the_lines_the_app_and_the_asker_read() {
    assert_eq!(
        serde_json::to_string(&WhoAmI { whoami: true }).unwrap(),
        r#"{"whoami":true}"#
    );
    let you: YouAre = serde_json::from_str(r#"{"you_are":3,"token":"ab"}"#).unwrap();
    assert_eq!(you.you_are, 3);
    assert!(
        !format!("{you:?}").contains("ab"),
        "its Debug never shows the token"
    );
}

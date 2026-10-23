// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's settings too long for one word of its command line go through a file (SK-9, #2772).

use super::*;

const CHAT: &str = "01J0000000000000000000000A";

fn words(settings: &str) -> Vec<String> {
    [
        "--plugin-dir",
        "/p",
        "--settings",
        settings,
        "--resume",
        "x",
    ]
    .iter()
    .map(|one| (*one).to_owned())
    .collect()
}

#[test]
fn settings_longer_than_one_word_go_through_a_file_and_shorter_ones_stay() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let short = r#"{"sandbox":{"enabled":true}}"#;
    assert_eq!(
        settings_in_a_file(root, CHAT, words(short)).unwrap(),
        words(short)
    );

    let long = format!(r#"{{"deny":["{}"]}}"#, "x".repeat(SETTINGS_IN_A_WORD * 3));
    let args = settings_in_a_file(root, CHAT, words(&long)).unwrap();
    let file = crate::plane::state_dir(root)
        .join(SETTINGS_DIR)
        .join(format!("{CHAT}.json"));
    assert_eq!(args[3], file.display().to_string());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), long);
    // The rest of the line is as it was, and every word fits one word of a command line.
    assert_eq!(&args[..3], &words(&long)[..3]);
    assert_eq!(&args[4..], &words(&long)[4..]);
    assert!(args.iter().all(|word| word.len() < SETTINGS_IN_A_WORD));
    // A restart writes it again, over the last.
    let again = format!(r#"{{"deny":["{}"]}}"#, "y".repeat(SETTINGS_IN_A_WORD * 2));
    settings_in_a_file(root, CHAT, words(&again)).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), again);

    assert!(settings_in_a_file(root, "../x", words(&long)).is_err());

    // Swept once its chat is off the app's record, and only then.
    let old = std::time::SystemTime::now() - crate::skills::verbs::drafts::SWEEP_GRACE * 2;
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(old)
        .unwrap();
    sweep_settings(root, &[CHAT.to_owned()]);
    assert!(file.is_file());
    sweep_settings(root, &[]);
    assert!(!file.exists());
}

/// A start whose settings went through a file is still read as Claude Code in its own sandbox.
#[test]
fn settings_in_a_file_are_read_back_for_the_sandbox_and_no_other_file_is() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let long = format!(
        r#"{{"sandbox":{{"enabled":true}},"deny":["{}"]}}"#,
        "x".repeat(SETTINGS_IN_A_WORD * 2)
    );
    let args = settings_in_a_file(root, CHAT, words(&long)).unwrap();
    let read = settings_from_its_file(std::path::Path::new(&args[3])).expect("read back");
    assert_eq!(read.pointer("/sandbox/enabled"), Some(&true.into()));
    let elsewhere = root.join("other.json");
    std::fs::write(&elsewhere, r#"{"sandbox":{"enabled":true}}"#).unwrap();
    assert_eq!(settings_from_its_file(&elsewhere), None);
}

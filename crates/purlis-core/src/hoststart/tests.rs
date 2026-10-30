// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's settings too long for one word of its command line go through a file (SK-9, #2772).

use super::*;

const CHAT: &str = "01J0000000000000000000000A";

/// A test's plane, `plane` in `tmp`, beside its app data folder.
fn plane_in(tmp: &tempfile::TempDir) -> std::path::PathBuf {
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// The app data folder's chat settings of a test whose plane is `root`: beside it, outside it.
fn data(root: &std::path::Path) -> std::path::PathBuf {
    root.parent().unwrap().join("data").join(CHAT_SETTINGS)
}

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
    let plane = plane_in(&tmp);
    let root = plane.as_path();
    let short = r#"{"sandbox":{"enabled":true}}"#;
    assert_eq!(
        settings_in_a_file(Some(&data(root)), root, CHAT, words(short)).unwrap(),
        words(short)
    );

    let long = format!(r#"{{"deny":["{}"]}}"#, "x".repeat(SETTINGS_IN_A_WORD * 3));
    let args = settings_in_a_file(Some(&data(root)), root, CHAT, words(&long)).unwrap();
    let file = std::path::PathBuf::from(&args[3]);
    // In the app data folder, under the plane's clone key: never in the plane.
    assert_eq!(
        file.parent(),
        Some(
            data(root)
                .join(crate::plane::CloneKey::of(root).as_str())
                .as_path()
        )
    );
    assert!(!file.starts_with(root));
    assert!(names_a_settings_file(&file), "{file:?}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), long);
    // The rest of the line is as it was, and every word fits one word of a command line.
    assert_eq!(&args[..3], &words(&long)[..3]);
    assert_eq!(&args[4..], &words(&long)[4..]);
    assert!(args.iter().all(|word| word.len() < SETTINGS_IN_A_WORD));
    // A restart writes it again, over the last.
    let again = format!(r#"{{"deny":["{}"]}}"#, "y".repeat(SETTINGS_IN_A_WORD * 2));
    let restarted = settings_in_a_file(Some(&data(root)), root, CHAT, words(&again)).unwrap();
    assert_eq!(std::fs::read_to_string(&restarted[3]).unwrap(), again);

    assert!(settings_in_a_file(Some(&data(root)), root, "../x", words(&long)).is_err());

    // Swept once its chat is off the app's record, and only then.
    let old = std::time::SystemTime::now() - crate::skills::verbs::drafts::SWEEP_GRACE * 2;
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(old)
        .unwrap();
    sweep_settings(Some(&data(root)), root, &[CHAT.to_owned()]);
    assert!(file.is_file());
    sweep_settings(Some(&data(root)), root, &[]);
    assert!(!file.exists());
    // A file an earlier build kept in the plane's state is swept, whatever its chat, and never
    // read back.
    let before = crate::plane::state_dir(root)
        .join(SETTINGS_DIR)
        .join(format!("{CHAT}.json"));
    std::fs::create_dir_all(before.parent().unwrap()).unwrap();
    std::fs::write(&before, r#"{"sandbox":{"enabled":true}}"#).unwrap();
    assert_eq!(settings_from_its_file(&before), None);
    std::fs::File::options()
        .write(true)
        .open(&before)
        .unwrap()
        .set_modified(old)
        .unwrap();
    sweep_settings(Some(&data(root)), root, &[CHAT.to_owned()]);
    assert!(!before.exists());
}

/// A chat's settings file is read back only while it holds what purlis wrote in it: one that
/// holds anything else, or is named so by anything but purlis, is not.
#[test]
fn a_settings_file_that_is_not_what_purlis_wrote_is_not_read_back() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = plane_in(&tmp);
    let root = plane.as_path();
    let long = format!(
        r#"{{"sandbox":{{"enabled":true}},"deny":["{}"]}}"#,
        "x".repeat(SETTINGS_IN_A_WORD * 2)
    );
    let args = settings_in_a_file(Some(&data(root)), root, CHAT, words(&long)).unwrap();
    let file = std::path::Path::new(&args[3]);
    assert!(settings_from_its_file(file).is_some());

    std::fs::write(file, long.replacen("true", "false", 1)).unwrap();
    assert_eq!(settings_from_its_file(file), None);
    // Its name still says purlis wrote it, so what it was is read as unknown, never as off.
    assert!(names_a_settings_file(file));

    // The same file in the plane's state, where earlier builds kept them, is not read.
    let in_the_plane = crate::plane::state_dir(root)
        .join(SETTINGS_DIR)
        .join(file.file_name().unwrap());
    std::fs::create_dir_all(in_the_plane.parent().unwrap()).unwrap();
    std::fs::write(&in_the_plane, &long).unwrap();
    assert!(!names_a_settings_file(&in_the_plane));
    assert_eq!(settings_from_its_file(&in_the_plane), None);
    // Nor is one whose folder is named by no clone key.
    let keyless = data(root).join("not-a-key").join(file.file_name().unwrap());
    std::fs::create_dir_all(keyless.parent().unwrap()).unwrap();
    std::fs::write(&keyless, &long).unwrap();
    assert_eq!(settings_from_its_file(&keyless), None);

    // A name with no digest, or a digest that is not one, is no settings file of purlis's.
    let dir = file.parent().unwrap();
    for name in [
        format!("{CHAT}.json"),
        format!("{CHAT}.not-a-digest.json"),
        "x.0123456789abcdef0123456789abcdef.json".to_owned(),
    ] {
        let other = dir.join(&name);
        std::fs::write(&other, r#"{"sandbox":{"enabled":true}}"#).unwrap();
        assert!(!names_a_settings_file(&other), "{name}");
        assert_eq!(settings_from_its_file(&other), None, "{name}");
    }
}

/// A start whose settings went through a file is still read as Claude Code in its own sandbox.
#[test]
fn settings_in_a_file_are_read_back_for_the_sandbox_and_no_other_file_is() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = plane_in(&tmp);
    let root = plane.as_path();
    let long = format!(
        r#"{{"sandbox":{{"enabled":true}},"deny":["{}"]}}"#,
        "x".repeat(SETTINGS_IN_A_WORD * 2)
    );
    let args = settings_in_a_file(Some(&data(root)), root, CHAT, words(&long)).unwrap();
    let read = settings_from_its_file(std::path::Path::new(&args[3])).expect("read back");
    assert_eq!(read.pointer("/sandbox/enabled"), Some(&true.into()));
    let elsewhere = root.join("other.json");
    std::fs::write(&elsewhere, r#"{"sandbox":{"enabled":true}}"#).unwrap();
    assert_eq!(settings_from_its_file(&elsewhere), None);
}

#[test]
fn a_start_with_no_data_folder_for_its_long_settings_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let long = format!(r#"{{"deny":["{}"]}}"#, "x".repeat(SETTINGS_IN_A_WORD * 2));
    assert!(settings_in_a_file(None, tmp.path(), CHAT, words(&long)).is_err());
    // Short settings need no file, so no folder either.
    let short = r#"{"sandbox":{"enabled":true}}"#;
    assert_eq!(
        settings_in_a_file(None, tmp.path(), CHAT, words(short)).unwrap(),
        words(short)
    );
}

#[test]
fn only_a_links_start_says_what_its_desktop_allowed_and_each_replaces_the_last() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    let said = |by: &str, allowed: Option<&str>| {
        let start = purlis_session_protocol::session::StartProgram {
            persona: Some("devops".into()),
            tools_allowed: allowed.map(str::to_owned),
            ..purlis_session_protocol::session::StartProgram::of(
                "01J9PRJCT00000000000000000",
                purlis_session_protocol::session::ProgramKind::Harness,
            )
        };
        keep_what_its_desktop_allowed(&Starting {
            root: &root,
            number: 1,
            start: &start,
            reporting: None,
            started_by: by,
        })
        .unwrap();
        crate::sandbox::local::tools_allowed_by_desktop(&root)
            .into_iter()
            .map(|one| one.digest)
            .collect::<Vec<_>>()
    };
    for own in ["local-ui", "terminal", "fleet-mcp", "approval", "editor"] {
        assert!(said(own, Some("d1")).is_empty(), "{own}");
    }
    assert_eq!(said("remote-link", Some("d1")), ["d1"]);
    assert_eq!(said("remote-link", Some("d2")), ["d2"]);
    assert_eq!(
        said("local-ui", Some("d3")),
        ["d2"],
        "and it changes nothing"
    );
    assert!(
        said("remote-link", None).is_empty(),
        "an Allow taken back there holds from the next start"
    );
}

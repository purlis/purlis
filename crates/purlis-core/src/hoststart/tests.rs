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

/// #4344: a project that has not moved keeps its desktop's word, since its chats read
/// `.claude/skills` themselves, and a start that says nothing takes this machine's own reading,
/// which is what the flag's removal (#2786) turns on.
#[test]
fn an_unmoved_project_keeps_its_desktops_word_and_a_silent_start_reads_this_machine() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    let reach = |said: Option<bool>| {
        let start = purlis_session_protocol::session::StartProgram {
            plane_skills: said,
            ..purlis_session_protocol::session::StartProgram::of(
                "01J9PRJCT00000000000000000",
                purlis_session_protocol::session::ProgramKind::Harness,
            )
        };
        plane_skills_of(&Starting {
            root: &root,
            number: 1,
            start: &start,
            reporting: None,
            prompt_key: None,
            started_by: "remote-link",
        })
    };
    assert!(!reach(Some(false)));
    assert!(reach(Some(true)));
    assert_eq!(reach(None), crate::skills::reach_chats_here(&root));
}

/// #4344: what a chat runs on the runner reads the word its arming took, so a moved project's chat
/// is not told the studio is off while it holds the project's skills.
#[test]
fn a_moved_projects_chat_on_a_runner_is_started_with_the_word_its_arming_took() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    std::fs::write(
        root.join(crate::plane::MANIFEST),
        "requires = [{ feature = \"project-skills\" }]\n",
    )
    .unwrap();
    let start = purlis_session_protocol::session::StartProgram {
        plane_skills: Some(false),
        ..purlis_session_protocol::session::StartProgram::of(
            "01J9PRJCT00000000000000000",
            purlis_session_protocol::session::ProgramKind::Harness,
        )
    };
    let said = studio_env_of(&Starting {
        root: &root,
        number: 1,
        start: &start,
        reporting: None,
        prompt_key: None,
        started_by: "remote-link",
    });
    assert_eq!(
        said,
        Some((crate::skills::STUDIO_ENV.to_owned(), "1".to_owned()))
    );
}

/// SK-20b (#3084): a runner arms a chat with the plane's skills by its desktop's reading of the
/// Skill studio's flag, where the start says it, never by its own machine's, which has nobody to
/// turn it on, and a project that requires its skills gets them whatever it said (#4344). Only a
/// link's start says so. Seen red: the runner read its own machine's alone.
#[test]
fn only_a_links_start_says_whether_its_desktop_reaches_the_chat_with_the_planes_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    // The project requires its skills, so this machine's own reading is on, whatever its flag.
    std::fs::write(
        root.join(crate::plane::MANIFEST),
        "requires = [{ feature = \"project-skills\" }]\n",
    )
    .unwrap();
    let reach = |by: &str, said: Option<bool>| {
        let start = purlis_session_protocol::session::StartProgram {
            plane_skills: said,
            ..purlis_session_protocol::session::StartProgram::of(
                "01J9PRJCT00000000000000000",
                purlis_session_protocol::session::ProgramKind::Harness,
            )
        };
        plane_skills_of(&Starting {
            root: &root,
            number: 1,
            start: &start,
            reporting: None,
            prompt_key: None,
            started_by: by,
        })
    };
    assert!(
        reach("remote-link", Some(false)),
        "the project requires them and its clones hold no copy: its desktop's word cannot take \
         them away (#4344)"
    );
    assert!(reach("remote-link", Some(true)));
    assert!(
        reach("remote-link", None),
        "a desktop before 1.62: its own reading"
    );
    for own in ["local-ui", "terminal", "fleet-mcp", "approval", "editor"] {
        assert!(reach(own, Some(false)), "{own}: its own reading");
    }
}

/// #3084: what the chat runs on the runner (purlis's MCP server among them) reads the flag its
/// arming took, its desktop's, from the variable that decides it there.
#[test]
fn a_chat_its_desktop_said_the_flag_for_is_started_with_that_word() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    let env = |by: &str, said: Option<bool>| {
        let start = purlis_session_protocol::session::StartProgram {
            plane_skills: said,
            ..purlis_session_protocol::session::StartProgram::of(
                "01J9PRJCT00000000000000000",
                purlis_session_protocol::session::ProgramKind::Harness,
            )
        };
        studio_env_of(&Starting {
            root: &root,
            number: 1,
            start: &start,
            reporting: None,
            prompt_key: None,
            started_by: by,
        })
    };
    let word = |on: &str| Some((crate::skills::STUDIO_ENV.to_owned(), on.to_owned()));
    assert_eq!(env("remote-link", Some(true)), word("1"));
    assert_eq!(env("remote-link", Some(false)), word("0"));
    assert_eq!(env("remote-link", None), None);
    assert_eq!(env("local-ui", Some(true)), None);
}

#[test]
fn only_a_links_start_says_what_its_desktop_allowed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    let start = purlis_session_protocol::session::StartProgram {
        persona: Some("devops".into()),
        tools_allowed: Some("d1".into()),
        ..purlis_session_protocol::session::StartProgram::of(
            "01J9PRJCT00000000000000000",
            purlis_session_protocol::session::ProgramKind::Harness,
        )
    };
    let said = |by: &str| {
        what_its_desktop_allowed(&Starting {
            root: &root,
            number: 1,
            start: &start,
            reporting: None,
            started_by: by,
            prompt_key: None,
        })
    };
    for own in ["local-ui", "terminal", "fleet-mcp", "approval", "editor"] {
        assert_eq!(said(own), None, "{own}");
    }
    assert_eq!(said("remote-link").as_deref(), Some("d1"));
}

/// The person's ruling of 2026-10-31 (#4207): what the port's nearness refuses, and what it leaves.
#[test]
fn a_runner_told_of_a_mint_port_starts_nothing_outside_the_sandbox_but_a_shell() {
    assert_eq!(
        no_sandbox_refused(ProgramKind::Harness, Ok(true)),
        Err(NO_SANDBOX_WHERE_A_TOKEN_IS_NEAR.to_owned())
    );
    assert!(no_sandbox_refused(ProgramKind::Harness, Err("unreadable".into())).is_err());
    assert_eq!(no_sandbox_refused(ProgramKind::Harness, Ok(false)), Ok(()));
    assert_eq!(no_sandbox_refused(ProgramKind::Shell, Ok(true)), Ok(()));
}

#[test]
fn a_sandboxed_chat_of_a_link_alone_has_its_own_settings_for_the_forge_programs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    let start = purlis_session_protocol::session::StartProgram::of(
        "01J9PRJCT00000000000000000",
        purlis_session_protocol::session::ProgramKind::Harness,
    );
    let settings = |by: &str, sandboxed: bool| {
        forge_programs_settings_of(
            &Starting {
                root: &root,
                number: 1,
                start: &start,
                reporting: None,
                started_by: by,
                prompt_key: None,
            },
            sandboxed,
        )
    };
    assert!(settings("local-ui", true).is_empty());
    assert!(settings("remote-link", false).is_empty());
    let (first, second) = (settings("remote-link", true), settings("remote-link", true));
    assert_eq!(first[0].0, "GH_CONFIG_DIR");
    assert_eq!(first[1].0, "GLAB_CONFIG_DIR");
    assert_ne!(first, second, "each chat's its own");
    for (_, at) in first.iter().chain(&second) {
        let folder = std::path::Path::new(at).parent().expect("its folder");
        let _ = std::fs::remove_dir(folder);
    }
    assert!(!crate::sandbox::tools::a_run_starts_from("GH_CONFIG_DIR"));
    assert!(!crate::sandbox::tools::a_run_starts_from("GLAB_CONFIG_DIR"));
}

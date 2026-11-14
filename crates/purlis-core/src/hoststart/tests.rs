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
/// which the flag's removal (#2786) turned on.
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
    assert!(
        reach(None),
        "a start that says nothing: this machine's reading, which since #2786 reaches it"
    );
}

/// #4344: what a chat runs on the runner reads the word its arming took, so a moved project's chat
/// is not told the plane's skills do not reach it while it holds them.
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
    let said = plane_skills_env_of(&Starting {
        root: &root,
        number: 1,
        start: &start,
        reporting: None,
        prompt_key: None,
        started_by: "remote-link",
    });
    assert_eq!(
        said,
        Some((crate::skills::PLANE_SKILLS_ENV.to_owned(), "1".to_owned()))
    );
}

/// SK-20b (#3084): a runner arms a chat with the plane's skills by its desktop's word, where the
/// start says it, never by its own machine's, and a project that requires its skills gets them
/// whatever it said (#4344). Only a link's start says so. Seen red: the runner read its own
/// machine's alone.
#[test]
fn only_a_links_start_says_whether_its_desktop_reaches_the_chat_with_the_planes_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let root = plane_in(&tmp);
    // The project requires its skills, so this machine's own reading is that they reach it.
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

/// #3084: what the chat runs on the runner (purlis's MCP server among them) reads the word its
/// arming took, its desktop's, from the variable that carries it there.
#[test]
fn a_chat_its_desktop_said_the_word_for_is_started_with_that_word() {
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
        plane_skills_env_of(&Starting {
            root: &root,
            number: 1,
            start: &start,
            reporting: None,
            prompt_key: None,
            started_by: by,
        })
    };
    let word = |on: &str| Some((crate::skills::PLANE_SKILLS_ENV.to_owned(), on.to_owned()));
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

/// Ruled 2026-11-08: a runner starts no harness from a launch by its words, which no sandbox of
/// this host's making holds; a shell, and anything on a person's own machine, starts as before.
#[test]
fn a_runner_starts_no_harness_from_a_launch_by_its_words() {
    let program = |harness: Option<&str>, confinement: Option<&str>| Launch {
        program: "/usr/local/bin/codex".to_owned(),
        args: Vec::new(),
        cwd: None,
        set: Vec::new(),
        strip: Vec::new(),
        pass: Vec::new(),
        declared: Vec::new(),
        harness: harness.map(str::to_owned),
        git_hooks: None,
        reporting: None,
        confinement: confinement.map(str::to_owned),
        prompt_key: None,
    };
    let refused =
        launch_by_words_refused(&program(Some("codex"), None), true).expect_err("refused");
    assert!(refused.contains("on a runner"), "{refused}");
    assert!(refused.contains("nothing was started"), "{refused}");
    // A program that names no harness, one in a confinement, and anything on a person's own
    // machine start as before.
    assert_eq!(launch_by_words_refused(&program(None, None), true), Ok(()));
    assert_eq!(
        launch_by_words_refused(&program(Some("codex"), Some("c1")), true),
        Ok(())
    );
    assert_eq!(
        launch_by_words_refused(&program(Some("codex"), None), false),
        Ok(())
    );
}

#[test]
fn a_chats_forge_programs_keep_their_settings_in_its_own_temporary_folder() {
    let wrapped = [("TMPDIR".to_owned(), "/chat/tmp".to_owned())];
    assert_eq!(
        forge_programs_settings_in(&wrapped),
        [
            ("GH_CONFIG_DIR".to_owned(), "/chat/tmp/forge/gh".to_owned()),
            (
                "GLAB_CONFIG_DIR".to_owned(),
                "/chat/tmp/forge/glab".to_owned()
            ),
        ]
    );
    assert!(!crate::sandbox::tools::a_run_starts_from("GH_CONFIG_DIR"));
    assert!(!crate::sandbox::tools::a_run_starts_from("GLAB_CONFIG_DIR"));
}

/// Chats started together, as a record put back at launch starts them, each write their
/// settings to a file at once, in a project none has written for yet: every one starts, those
/// with ids of their own and two that share one (the same settings, the same file) alike. On
/// macOS a Claude Code chat's settings pass one word's length, so this is every launch there.
#[test]
fn chats_that_write_their_settings_at_once_all_start() {
    const IDS: [&str; 4] = [
        "01J0000000000000000000000A",
        "01J0000000000000000000000B",
        "01J0000000000000000000000C",
        "01J0000000000000000000000C",
    ];
    let long = format!(r#"{{"deny":["{}"]}}"#, "x".repeat(SETTINGS_IN_A_WORD * 2));
    for _ in 0..20 {
        let tmp = tempfile::tempdir().unwrap();
        let plane = plane_in(&tmp);
        let root = plane.as_path();
        let ready = std::sync::Barrier::new(IDS.len());
        let started: Vec<Result<Vec<String>, String>> = std::thread::scope(|scope| {
            let starts: Vec<_> = IDS
                .iter()
                .map(|id| {
                    let (ready, long) = (&ready, &long);
                    scope.spawn(move || {
                        ready.wait();
                        settings_in_a_file(Some(&data(root)), root, id, words(long))
                    })
                })
                .collect();
            starts.into_iter().map(|one| one.join().unwrap()).collect()
        });
        for (id, start) in IDS.iter().zip(started) {
            let args = start.unwrap_or_else(|why| panic!("{id}: {why}"));
            assert_eq!(std::fs::read_to_string(&args[3]).unwrap(), long, "{id}");
        }
    }
}

fn pairs(of: &[(&str, &str)]) -> Vec<(String, String)> {
    of.iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn an_outcome_eval_shells_own_temp_folder_stands_over_the_one_its_claude_code_line_names() {
    let temp = crate::sandbox::claude_wrap::TMP_ENV;
    let mut env = pairs(&[("HOME", "/home")]);
    wrapped_over(
        &mut env,
        pairs(&[(temp, "/eval/temp"), ("TMPDIR", "/eval/temp")]),
        pairs(&[(temp, "/chat/temp"), ("HTTPS_PROXY", "http://proxy")]),
    );
    env.sort();
    assert_eq!(
        env,
        pairs(&[
            (temp, "/eval/temp"),
            ("HOME", "/home"),
            ("HTTPS_PROXY", "http://proxy"),
            ("TMPDIR", "/eval/temp"),
        ])
    );
}

#[test]
fn a_chats_claude_code_line_names_its_own_temp_folder_where_nothing_armed_names_one() {
    let temp = crate::sandbox::claude_wrap::TMP_ENV;
    let mut env = pairs(&[(temp, "/profile/temp")]);
    wrapped_over(&mut env, Vec::new(), pairs(&[(temp, "/chat/temp")]));
    assert_eq!(env, pairs(&[(temp, "/chat/temp")]));
}

#[test]
fn a_wrap_that_sets_its_own_tmpdir_names_claude_codes_temp_folder_over_the_armings() {
    let temp = crate::sandbox::claude_wrap::TMP_ENV;
    let mut env = Vec::new();
    wrapped_over(
        &mut env,
        pairs(&[(temp, "/eval/temp"), ("TMPDIR", "/eval/temp")]),
        pairs(&[("TMPDIR", "/wrap/temp"), (temp, "/wrap/temp")]),
    );
    env.sort();
    assert_eq!(
        env,
        pairs(&[(temp, "/wrap/temp"), ("TMPDIR", "/wrap/temp")])
    );
}

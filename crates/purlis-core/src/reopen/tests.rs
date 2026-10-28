// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::fs;

#[cfg(target_os = "macos")]
#[test]
fn a_record_folder_the_filesystem_refuses_to_make_is_named() {
    // #1421: an EPERM here printed only "Operation not permitted (os error 1)".
    let plane = tempfile::tempdir().unwrap();
    let dir = path(plane.path()).parent().unwrap().to_path_buf();
    let state = dir.parent().unwrap();
    fs::create_dir_all(state).unwrap();
    let _frozen = crate::rewrite::frozen::Frozen::at(state);

    let refused = write(plane.path(), &Record::default()).unwrap_err();

    crate::rewrite::frozen::names(&refused, &dir);
}

/// The ULIDs the core's tests spell a chat's ids with, one set for every module.
pub(crate) const CHAT: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7F";
pub(crate) const DEVICE: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7G";
pub(crate) const RUN: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7H";
pub(crate) const RESUMED: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7J";

#[test]
fn a_minted_id_is_a_ulid_and_a_ulid_reads_back_as_itself() {
    let id = mint();
    assert_eq!(a_ulid(&id), Some(id.clone()));
    assert_ne!(mint(), id, "a new one each time");
    assert_eq!(a_ulid("-rf"), None);
}

fn claude(name: &str, resume: Option<&str>) -> Chat {
    Chat {
        program: "claude".to_owned(),
        args: vec!["--model".to_owned(), "opus".to_owned()],
        cwd: Some(PathBuf::from("/Users/aharon/IdeaProjects/charter")),
        name: name.to_owned(),
        resume: resume.map(|id| SessionId::new(id).expect("a valid id in a test")),
        active: false,
        profile: None,
        persona: None,
        show_footer: false,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    }
}

const ID: &str = "11111111-2222-4333-8444-555555555555";

#[test]
fn what_was_written_is_what_the_next_launch_reads() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![claude("ide.7", Some(ID)), claude("ide.8", None)],
        ..Default::default()
    };

    write(plane.path(), &record).expect("the record is written");

    assert_eq!(read(plane.path()), record);
}

#[test]
fn the_record_is_written_where_the_plane_format_says() {
    let plane = tempfile::tempdir().unwrap();

    write(plane.path(), &Record::default()).expect("the record is written");

    assert!(plane.path().join(".charter/app/reopen.json").is_file());
}

#[test]
fn a_plane_with_no_record_has_nothing_to_reopen() {
    let plane = tempfile::tempdir().unwrap();

    assert_eq!(read(plane.path()), Record::default());
}

#[test]
fn a_record_of_another_version_is_ignored_whole() {
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let file = path(plane.path());
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("\"version\": 1", "\"version\": 2")).unwrap();

    assert_eq!(read(plane.path()), Record::default());
}

#[test]
fn a_file_that_is_not_the_record_at_all_is_nothing_to_reopen() {
    let plane = tempfile::tempdir().unwrap();
    fs::create_dir_all(path(plane.path()).parent().unwrap()).unwrap();
    fs::write(path(plane.path()), "{ this is not json").unwrap();

    assert_eq!(read(plane.path()), Record::default());
}

#[test]
fn a_chat_whose_recorded_conversation_could_be_read_as_a_flag_comes_back_without_one() {
    // The file is on disk and every value in it reaches a command line. A chat with a
    // hostile id is still reopened — it is a chat the operator had — but as a new one.
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let file = path(plane.path());
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace(ID, "--dangerously-skip-permissions")).unwrap();

    let back = read(plane.path());

    assert_eq!(back.chats.len(), 1, "the chat itself was dropped: {back:?}");
    assert_eq!(back.chats[0].resume, None);
}

#[test]
fn a_chat_with_a_recorded_conversation_is_resumed() {
    let chat = claude("ide.7", Some(ID));

    let launch = chat.launch();

    assert_eq!(launch.program, "claude");
    assert_eq!(
        launch.args,
        vec!["--resume", ID, "--name", "ide.7", "--model", "opus"]
    );
    assert_eq!(launch.how, Reopened::Resumed(SessionId::new(ID).unwrap()));
}

#[test]
fn a_chat_with_no_recorded_conversation_comes_back_as_a_new_one() {
    // A Codex chat is always this: Codex reports its id through a hook inside its first
    // turn, so until hooks land the app has none to record.
    let chat = Chat {
        program: "codex".to_owned(),
        ..claude("ide.7", None)
    };

    let launch = chat.launch();

    assert_eq!(launch.program, "codex");
    assert_eq!(launch.args, vec!["--model", "opus"]);
    assert_eq!(launch.how, Reopened::Fresh(Fresh::NoConversationRecorded));
}

#[test]
fn a_new_claude_chat_is_started_under_an_id_the_app_chose_so_it_can_be_resumed_next_time() {
    let chat = claude("ide.7", None);

    let launch = chat.launch();

    assert_eq!(launch.how, Reopened::Fresh(Fresh::NoConversationRecorded));
    let chosen = launch
        .args
        .iter()
        .position(|word| word == "--session-id")
        .map(|at| launch.args[at + 1].clone())
        .expect("a new Claude Code chat is given an id");
    assert!(SessionId::new(&chosen).is_ok(), "{chosen:?} is not an id");
    assert_eq!(
        launch.args[launch.args.len() - 2..],
        ["--model", "opus"],
        "the chat's own arguments no longer come last: {:?}",
        launch.args
    );
}

#[test]
fn a_chat_a_workspace_rename_left_without_its_conversation_starts_fresh_and_says_why() {
    // charter#367, D10: Claude Code cannot find the conversation under the new directory,
    // so the rename dropped it. The chat starts a new one under an id charter chose, which
    // is what the next quit records, and says why rather than "nothing recorded".
    let chat = Chat {
        renamed_from: Some("alpha".to_owned()),
        ..claude("ide.7", None)
    };

    let launch = chat.launch();

    assert_eq!(launch.how, Reopened::Fresh(Fresh::WorkspaceRenamed));
    assert!(
        launch.args.contains(&"--session-id".to_owned()),
        "{:?}",
        launch.args
    );
    assert!(launch.session.is_some());
}

#[test]
fn a_profile_start_that_knew_no_chat_is_told_the_rename_and_nothing_else_is_changed() {
    let renamed = Chat {
        renamed_from: Some("alpha".to_owned()),
        ..claude("ide.7", None)
    };
    let nothing = Reopened::Fresh(Fresh::NoConversationRecorded);

    assert_eq!(
        renamed.told(nothing.clone()),
        Reopened::Fresh(Fresh::WorkspaceRenamed)
    );
    assert_eq!(claude("ide.7", None).told(nothing.clone()), nothing);
    let resumed = Reopened::Resumed(SessionId::new(ID).unwrap());
    assert_eq!(renamed.told(resumed.clone()), resumed);
}

#[test]
fn the_workspace_a_rename_moved_a_chat_from_survives_a_quit_and_a_bad_one_reads_as_none() {
    let plane = tempfile::tempdir().unwrap();
    let renamed = Chat {
        renamed_from: Some("alpha".to_owned()),
        ..claude("ide.7", None)
    };
    write(
        plane.path(),
        &Record {
            chats: vec![renamed.clone(), claude("ide.8", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("renamed_from").count(), 1, "{text}");

    assert_eq!(read(plane.path()).chats[0], renamed);

    fs::write(path(plane.path()), text.replace("\"alpha\"", "\"../x\"")).unwrap();
    assert_eq!(read(plane.path()).chats[0].renamed_from, None);
}

/// ADR 0067 §7: the record says a chat's last run was unsandboxed, so the next run can say
/// the sandbox came back on — and it is never read as an opt-out: the launch it builds is
/// the same either way.
#[test]
fn a_chat_that_ran_unsandboxed_is_recorded_so_and_its_relaunch_is_built_the_same() {
    let plane = tempfile::tempdir().unwrap();
    let unsandboxed = Chat {
        unsandboxed: true,
        ..claude("ide.7", Some(ID))
    };
    write(
        plane.path(),
        &Record {
            chats: vec![unsandboxed.clone(), claude("ide.8", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"sandbox\": \"off\"").count(), 1, "{text}");

    let back = read(plane.path());
    assert_eq!(back.chats[0], unsandboxed);
    assert!(!back.chats[1].unsandboxed);
    assert_eq!(
        back.chats[0].launch(),
        claude("ide.7", Some(ID)).launch(),
        "nothing about the next start reads it"
    );

    fs::write(path(plane.path()), text.replace("\"off\"", "\"none\"")).unwrap();
    assert!(
        !read(plane.path()).chats[0].unsandboxed,
        "only the one word"
    );
}

/// #1543: a reopened task names no asking chat, so the record says it was a task, for each
/// later start of it to be held to what its dispatch was held to.
#[test]
fn a_reopened_task_is_recorded_so_and_a_word_it_does_not_know_still_reads_as_one() {
    let plane = tempfile::tempdir().unwrap();
    let task = Chat {
        reopened: Some(ReopenedTask { by_person: false }),
        ..claude("ide.7", Some(ID))
    };
    let asked = Chat {
        reopened: Some(ReopenedTask { by_person: true }),
        ..claude("ide.8", Some(ID))
    };
    write(
        plane.path(),
        &Record {
            chats: vec![task.clone(), asked.clone(), claude("ide.9", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"reopened\"").count(), 2, "{text}");
    assert_eq!(text.matches("\"reopened\": \"task\"").count(), 1, "{text}");
    assert_eq!(
        text.matches("\"reopened\": \"person\"").count(),
        1,
        "{text}"
    );

    let back = read(plane.path());
    assert_eq!(back.chats[0], task);
    assert_eq!(back.chats[1], asked);
    assert_eq!(back.chats[2].reopened, None);

    // A word somebody else wrote never lets a chat out of the check: it is a task.
    fs::write(
        path(plane.path()),
        text.replace("\"reopened\": \"task\"", "\"reopened\": \"no\""),
    )
    .unwrap();
    assert_eq!(
        read(plane.path()).chats[0].reopened,
        Some(ReopenedTask { by_person: false })
    );
}

#[test]
fn the_process_a_chat_runs_as_is_recorded_and_a_chat_with_none_writes_no_key() {
    let plane = tempfile::tempdir().unwrap();
    let running = Chat {
        pid: Some(4242),
        ..claude("ide.7", None)
    };
    write(
        plane.path(),
        &Record {
            chats: vec![running.clone(), claude("ide.8", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"pid\"").count(), 1, "{text}");

    assert_eq!(read(plane.path()).chats[0], running);
    assert_eq!(read(plane.path()).chats[1].pid, None);

    fs::write(path(plane.path()), text.replace("4242", "0")).unwrap();
    assert_eq!(read(plane.path()).chats[0].pid, None, "zero is no process");
}

#[test]
fn the_model_a_harness_reported_is_recorded_and_a_chat_with_none_writes_no_key() {
    // #1021: what `Assisted-by: <harness>:<model>` names.
    let plane = tempfile::tempdir().unwrap();
    let reported = Chat {
        model: Some("claude-opus-4-1".to_owned()),
        ..claude("ide.7", None)
    };
    write(
        plane.path(),
        &Record {
            chats: vec![reported.clone(), claude("ide.8", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"model\"").count(), 1, "{text}");
    assert_eq!(read(plane.path()).chats[0], reported);
    assert_eq!(read(plane.path()).chats[1].model, None);

    // A value somebody else wrote that cannot stand on one trailer line reads as none.
    fs::write(
        path(plane.path()),
        text.replace("claude-opus-4-1", "two words"),
    )
    .unwrap();
    assert_eq!(read(plane.path()).chats[0].model, None);
}

#[test]
fn the_id_a_new_chat_was_given_is_the_one_the_next_quit_records() {
    // Without this a chat could never be resumed a second time: the app would choose an
    // id, hand it to the harness, and forget it before the quit that has to write it down.
    let chat = claude("ide.7", None);

    let launch = chat.launch();

    let chosen = launch.session.as_ref().expect("the chat is under an id");
    assert!(
        launch.args.contains(&chosen.to_string()),
        "the recorded id {chosen} is not the one the harness was given: {:?}",
        launch.args
    );
}

#[test]
fn codex_resumes_through_its_subcommand_before_the_chat_s_own_arguments() {
    // `codex resume <id>` is a subcommand. `codex --model opus resume <id>` is not the
    // same command line, and with a prompt among the arguments it is not one at all.
    let chat = Chat {
        program: "codex".to_owned(),
        ..claude("ide.7", Some(ID))
    };

    let launch = chat.launch();

    assert_eq!(launch.args, vec!["resume", ID, "--model", "opus"]);
}

#[test]
fn opencode_resumes_by_naming_its_session() {
    let chat = Chat {
        program: "opencode".to_owned(),
        args: Vec::new(),
        ..claude("ide.7", Some("ses_3a1b2c3d4e5f"))
    };

    let launch = chat.launch();

    assert_eq!(launch.args, vec!["-s", "ses_3a1b2c3d4e5f"]);
    assert_eq!(
        launch.how,
        Reopened::Resumed(SessionId::new("ses_3a1b2c3d4e5f").unwrap())
    );
}

#[test]
fn a_chat_s_current_conversation_is_read_by_the_number_it_answers_to() {
    // The chat asks by `$CHARTER_SESSION_ID`, which is the number the record keeps.
    let plane = tempfile::tempdir().unwrap();
    let other = "99999999-2222-4333-8444-555555555555";
    write(
        plane.path(),
        &Record {
            chats: vec![
                Chat {
                    number: Some(3),
                    ..claude("ide.3", Some(other))
                },
                Chat {
                    number: Some(7),
                    ..claude("ide.7", Some(ID))
                },
                Chat {
                    number: Some(8),
                    ..claude("ide.8", None)
                },
            ],
            ..Default::default()
        },
    )
    .expect("the record is written");

    let conversation = |chat| conversation_of(plane.path(), chat).expect("the record reads");

    assert_eq!(conversation(7), Some(SessionId::new(ID).unwrap()));
    assert_eq!(
        conversation(8),
        None,
        "a chat whose harness has named none yet"
    );
    assert_eq!(conversation(9), None, "a chat the record does not hold");
}

#[test]
fn a_chat_s_id_and_current_run_survive_a_quit_and_are_read_by_its_number() {
    let plane = tempfile::tempdir().unwrap();
    let identity = Identity {
        id: Some(CHAT.to_owned()),
        device: Some(DEVICE.to_owned()),
        run: Some(RUN.to_owned()),
        resumed_from: Some(RESUMED.to_owned()),
    };
    write(
        plane.path(),
        &Record {
            chats: vec![
                Chat {
                    number: Some(7),
                    identity: identity.clone(),
                    ..claude("ide.7", Some(ID))
                },
                Chat {
                    number: Some(8),
                    ..claude("ide.8", None)
                },
            ],
            ..Default::default()
        },
    )
    .expect("the record is written");

    assert_eq!(read(plane.path()).chats[0].identity, identity);
    let of = |chat| identity_of(plane.path(), chat).expect("the record reads");
    assert_eq!(of(7), Some(identity));
    assert_eq!(
        of(8),
        Some(Identity::default()),
        "a chat recorded before ids: one is minted at this launch"
    );
    assert_eq!(of(9), None, "a chat the record does not hold");
}

#[test]
fn a_record_written_before_chats_had_ids_writes_none_and_a_bad_id_reads_as_none() {
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", None)],
            ..Default::default()
        },
    )
    .expect("the record is written");
    let text = fs::read_to_string(path(plane.path())).unwrap();
    for key in ["\"id\"", "\"device\"", "\"run\"", "\"resumed_from\""] {
        assert!(!text.contains(key), "{key} is written only when known");
    }

    let written = text.replace(
        "\"program\": \"claude\"",
        &format!("\"program\": \"claude\", \"id\": \"-rf\", \"device\": \"\", \"run\": \"{RUN}\""),
    );
    fs::write(path(plane.path()), written).unwrap();
    assert_eq!(
        read(plane.path()).chats[0].identity,
        Identity {
            run: Some(RUN.to_owned()),
            ..Identity::default()
        },
        "a value that is not a ULID is not one"
    );
}

#[test]
fn a_chat_in_a_plane_with_no_record_is_in_no_conversation_charter_knows() {
    let plane = tempfile::tempdir().unwrap();

    assert_eq!(
        conversation_of(plane.path(), 7).expect("no record reads"),
        None
    );
}

#[test]
fn a_resumed_chat_stays_under_the_id_it_was_resumed_by() {
    let launch = claude("ide.7", Some(ID)).launch();

    assert_eq!(launch.session, Some(SessionId::new(ID).unwrap()));
}

#[test]
fn a_codex_chat_is_under_no_id_the_app_knows_because_codex_chose_it() {
    let launch = Chat {
        program: "codex".to_owned(),
        ..claude("ide.7", None)
    }
    .launch();

    assert_eq!(launch.session, None);
}

#[test]
fn a_chat_whose_own_arguments_name_a_session_is_not_given_a_second_one() {
    // `charter/harness/base.py:308` — the operator's flag wins. Charter adding
    // `--resume` beside one the operator typed gives a command line no harness has been
    // measured against.
    let chat = Chat {
        args: vec!["--continue".to_owned()],
        ..claude("ide.7", Some(ID))
    };

    let launch = chat.launch();

    assert_eq!(launch.args, vec!["--continue"]);
    assert_eq!(launch.session, None);
    assert_eq!(
        launch.how,
        Reopened::Fresh(Fresh::SessionNamedByTheOperator)
    );
}

#[test]
fn a_pane_running_a_shell_comes_back_as_a_shell_and_says_why_it_is_not_resumed() {
    let chat = Chat {
        program: "/bin/zsh".to_owned(),
        args: vec![],
        resume: None,
        ..claude("ide.9", None)
    };

    let launch = chat.launch();

    assert_eq!(launch.program, "/bin/zsh");
    assert_eq!(launch.args, Vec::<String>::new());
    assert_eq!(launch.how, Reopened::Fresh(Fresh::NoResumeForThisProgram));
    assert_eq!(launch.session, None);
}

#[test]
fn writing_the_record_leaves_nothing_beside_it() {
    // The record is written beside itself and renamed over, so a launch never reads
    // half of one. This is the guard on that: a write that stopped at the file beside
    // it would leave two here, and the record itself would never appear.
    let plane = tempfile::tempdir().unwrap();

    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();

    let left: Vec<String> = fs::read_dir(plane.path().join(".charter/app"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec!["reopen.json"]);
}

#[test]
fn a_record_written_over_an_older_one_replaces_it() {
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();

    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.8", None)],
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(read(plane.path()).chats.len(), 1);
    assert_eq!(read(plane.path()).chats[0].name, "ide.8");
}

#[test]
fn the_chat_that_was_in_front_is_the_one_marked_active() {
    let plane = tempfile::tempdir().unwrap();
    let front = Chat {
        active: true,
        profile: None,
        persona: None,
        ..claude("ide.8", None)
    };
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", None), front],
            ..Default::default()
        },
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(
        back.chats
            .iter()
            .filter(|chat| chat.active)
            .map(|chat| chat.name.as_str())
            .collect::<Vec<_>>(),
        vec!["ide.8"]
    );
}

/// A plane whose `.charter/app` is a symlink pointing out of it, and the outside
/// directory it points at.
fn a_plane_whose_record_directory_is_a_link() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().join("plane");
    let outside = held.path().join("outside");
    std::fs::create_dir_all(plane.join(".charter")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, plane.join(".charter/app")).unwrap();
    (held, plane, outside)
}

fn one_chat() -> Record {
    Record {
        views: Vec::new(),
        dealt: 0,
        relaunch_after_update: false,
        clone_seat: None,
        focus: None,
        chats: vec![Chat {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "touch /tmp/pwned".into()],
            cwd: Some(PathBuf::from("/tmp")),
            name: "planted".into(),
            resume: None,
            active: true,
            profile: None,
            persona: None,
            show_footer: false,
            pinned: false,
            number: None,
            label: None,
            from: None,
            renamed_from: None,
            ..Default::default()
        }],
    }
}

#[test]
fn a_record_directory_that_is_a_link_out_of_the_plane_is_not_written_through() {
    let (_held, plane, outside) = a_plane_whose_record_directory_is_a_link();

    let refused = write(&plane, &one_chat());

    assert!(refused.is_err(), "writing through the link was allowed");
    assert!(
        !outside.join("reopen.json").exists(),
        "the record was written outside the plane"
    );
}

#[test]
fn a_record_reached_through_a_link_is_nothing_to_put_back() {
    let (_held, plane, outside) = a_plane_whose_record_directory_is_a_link();
    // Whoever planted the link also planted what the app would launch.
    std::fs::write(
        outside.join("reopen.json"),
        r#"{"version":1,"at":1789000000,"chats":[{"program":"/bin/sh","args":["-c","touch /tmp/pwned"],"cwd":"/tmp","name":"planted","resume":"","active":true}]}"#,
    )
    .unwrap();

    let read_back = read(&plane);

    assert_eq!(
        read_back,
        Record::default(),
        "the app took its launch from outside the plane"
    );
}

#[test]
fn the_record_file_itself_being_a_link_is_refused_too() {
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().join("plane");
    let outside = held.path().join("outside");
    std::fs::create_dir_all(plane.join(".charter/app")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(outside.join("planted.json"), path(&plane)).unwrap();

    assert!(write(&plane, &one_chat()).is_err());
    assert!(!outside.join("planted.json").exists());
}

#[test]
fn an_ordinary_plane_still_writes_and_reads_its_record() {
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();

    write(&plane, &one_chat()).expect("an ordinary plane writes its record");

    assert_eq!(read(&plane), one_chat());
}

#[test]
fn a_record_that_is_a_link_is_refused_and_what_it_points_at_is_untouched() {
    // #434. The temp file the bytes land on is gated by `rewrite`'s walk, and its own
    // tests plant links there; this is the record's own name.
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().join("plane");
    let outside = held.path().join("outside");
    std::fs::create_dir_all(plane.join(".charter/app")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let theirs = outside.join("theirs.json");
    std::fs::write(&theirs, "THEIRS\n").unwrap();
    std::os::unix::fs::symlink(&theirs, path(&plane)).unwrap();

    assert!(
        write(&plane, &one_chat()).is_err(),
        "a linked record was written"
    );
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "THEIRS\n");
    assert!(path(&plane).is_symlink());
}

#[test]
fn a_record_write_that_dies_before_its_rename_leaves_the_old_record_whole() {
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();
    write(&plane, &one_chat()).unwrap();
    let before = std::fs::read(path(&plane)).unwrap();
    let _killed = crate::rewrite::hook::set(|_, _| Err(std::io::Error::other("killed")));

    assert!(write(&plane, &Record::default()).is_err());

    assert_eq!(std::fs::read(path(&plane)).unwrap(), before);
    assert_eq!(read(&plane), one_chat());
}

#[test]
fn the_record_is_private() {
    // #434: it names programs to run, so it is charter's own state at 0600.
    use std::os::unix::fs::PermissionsExt;
    let held = tempfile::tempdir().unwrap();
    write(held.path(), &one_chat()).unwrap();
    let mode = std::fs::metadata(path(held.path()))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn a_record_that_is_not_a_plain_file_is_refused_instead_of_read_for_ever() {
    // A FIFO is not a link, so a link check waves it through, and read_to_string on one
    // never returns — at launch, before there is a window to close.
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();
    std::fs::create_dir_all(plane.join(".charter/app")).unwrap();
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(path(&plane)))
        .expect("mkfifo runs");
    assert!(made.success(), "the test needs a fifo to plant");

    // In a thread, because the whole point is that the unguarded version never returns.
    let (say, heard) = std::sync::mpsc::channel();
    let asked = plane.clone();
    std::thread::spawn(move || say.send(read_or_refusal(&asked).is_err()));
    let answered = heard
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("reading a fifo record must not block");

    assert!(answered, "a fifo record was accepted");
}

#[test]
fn a_refusal_is_returned_rather_than_read_as_an_empty_record() {
    let (_held, plane, _outside) = a_plane_whose_record_directory_is_a_link();

    let refusal = read_or_refusal(&plane).expect_err("a linked record is a refusal");

    assert!(
        refusal.to_string().contains(".charter/app"),
        "the refusal must name the path an operator has to repair: {refusal}"
    );
    // And the forgiving door still answers, for callers that only want what to reopen.
    assert_eq!(read(&plane), Record::default());
}

#[test]
fn a_record_too_large_to_be_one_is_refused_rather_than_read_whole() {
    // git cannot carry a fifo, but it carries a sparse giant that arrives full size,
    // and reading it at launch is the same hang by another road.
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();
    std::fs::create_dir_all(plane.join(".charter/app")).unwrap();
    let record = std::fs::File::create(path(&plane)).unwrap();
    record.set_len(MAX_BYTES + 1).unwrap();

    let refusal = read_or_refusal(&plane).expect_err("an oversized record is a refusal");

    assert!(
        refusal.to_string().contains("never larger than"),
        "{refusal}"
    );
}

#[test]
fn a_record_of_an_honest_size_is_still_read() {
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();
    write(&plane, &one_chat()).unwrap();

    assert_eq!(read_or_refusal(&plane).unwrap(), one_chat());
}

#[test]
fn a_record_that_exists_but_cannot_be_read_is_a_refusal_not_an_empty_plane() {
    use std::os::unix::fs::PermissionsExt;
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();
    write(&plane, &one_chat()).unwrap();
    std::fs::set_permissions(path(&plane), std::fs::Permissions::from_mode(0o000)).unwrap();

    let read_back = read_or_refusal(&plane);

    // Running as root reads it anyway; the point is only that it is never a silent empty.
    if let Ok(record) = read_back {
        assert_eq!(
            record,
            one_chat(),
            "an unreadable record read as an empty one"
        );
    }
}

#[test]
fn a_plane_with_no_record_at_all_is_simply_nothing_to_reopen() {
    let held = tempfile::tempdir().unwrap();

    assert_eq!(read_or_refusal(held.path()).unwrap(), Record::default());
}

// ----- a pinned chat (ADR 0039, stored per ADR 0040) -----

#[test]
fn a_pinned_chat_comes_back_pinned() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        views: Vec::new(),
        dealt: 0,
        relaunch_after_update: false,
        clone_seat: None,
        focus: None,
        chats: vec![
            Chat {
                pinned: true,
                ..claude("ide.7", Some(ID))
            },
            claude("ide.8", None),
        ],
    };

    write(plane.path(), &record).expect("the record is written");

    assert_eq!(read(plane.path()), record);
}

#[test]
fn a_record_written_before_pins_existed_reads_as_nothing_pinned() {
    // **This is why there is no version bump**, although ADR 0039 expected one:
    // a bump would read every such record as "nothing to put back" and take the
    // operator's open chats with it at the first launch after the upgrade. A field is
    // owed a bump when its absence cannot be read honestly, and this one's reads as
    // `false` — which is what was true of every record written before pins existed.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[{"program":"claude","name":"ide.7"}]}"#,
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(back.chats.len(), 1);
    assert!(!back.chats[0].pinned);
}

#[test]
fn a_pin_that_is_not_a_boolean_is_read_as_no_record_at_all() {
    // Every other field off this file is held to what it has to be, and a value of the
    // wrong type fails the parse — which is this file's oldest rule for a record it
    // cannot understand, and the one direction that never invents an arrangement.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[{"program":"claude","name":"i","pinned":"yes"}]}"#,
    )
    .unwrap();

    assert_eq!(read(plane.path()), Record::default());
}

// ----- the number a chat keeps, and the numbers the plane has spent (charter-app#90) --

#[test]
fn a_chat_comes_back_under_the_number_it_was_recorded_with() {
    // Without this the number is the chat's POSITION in the record, which changes the
    // moment another chat is closed — and the number is the key
    // `.charter/sessions/<n>.workspace` and `<n>.lock` are written at.
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        views: Vec::new(),
        chats: vec![
            Chat {
                number: Some(2),
                ..claude("ide.7", Some(ID))
            },
            Chat {
                number: Some(5),
                ..claude("ide.8", None)
            },
        ],
        dealt: 5,
        relaunch_after_update: false,
        clone_seat: None,
        focus: None,
    };

    write(plane.path(), &record).expect("the record is written");

    assert_eq!(read(plane.path()), record);
}

#[test]
fn a_record_written_before_chats_kept_their_numbers_says_nothing_about_them() {
    // Every operator has one of these at the first launch after this change. Reading a
    // missing key as "no number" is what lets the launch deal them in order, which is
    // what it always did; inventing one would file a chat under a stranger's pointer on
    // purpose.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[{"program":"claude","name":"ide.7"}]}"#,
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(back.chats[0].number, None);
    assert_eq!(back.dealt, 0);
}

#[test]
fn a_number_of_zero_is_read_as_no_number_at_all() {
    // Zero is what serde defaults the key to and is no chat's number: `Sessions` counts
    // from one. A record that says zero is a record that says nothing.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[{"program":"claude","name":"i","number":0}]}"#,
    )
    .unwrap();

    assert_eq!(read(plane.path()).chats[0].number, None);
}

#[test]
fn the_counter_written_down_is_never_below_a_number_the_record_names() {
    // A caller that built a record by hand, or one field updated and not the other,
    // would otherwise write a counter that hands the next launch a number two chats
    // answer to.
    let plane = tempfile::tempdir().unwrap();

    write(
        plane.path(),
        &Record {
            views: Vec::new(),
            chats: vec![Chat {
                number: Some(6),
                ..claude("ide.7", None)
            }],
            dealt: 1,
            relaunch_after_update: false,
            clone_seat: None,
            focus: None,
        },
    )
    .expect("the record is written");

    assert_eq!(read(plane.path()).dealt, 6);
}

#[test]
fn a_record_whose_counter_was_edited_below_its_chats_is_read_with_it_raised() {
    // The same invariant on the way in. This file is one anybody who can write the
    // plane's state directory can write, and the one thing a counter must never do is
    // come back smaller than a number still in the file.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"dealt":1,"chats":[{"program":"claude","name":"i","number":4}]}"#,
    )
    .unwrap();

    assert_eq!(read(plane.path()).dealt, 4);
}

// ----- view tabs (ADR 0043, as amended: a pane holds a session or a view) -----

fn persona_view(name: &str) -> View {
    View {
        from: None,
        view: "persona".into(),
        key: name.into(),
        title: name.into(),
        workspace: Some("ide".into()),
        at: 1,
        active: true,
        pinned: false,
        split: None,
    }
}

#[test]
fn a_vault_s_tab_comes_back_under_any_name_charter_gives_a_vault() {
    // charter-app#235: a vault's tab is `{ view: "vault", key: <vault> }`, and every name
    // `registry::name_ok` accepts has to survive the record, or the tab silently stays shut.
    let plane = tempfile::tempdir().unwrap();
    let vault = |key: &str| View {
        view: "vault".into(),
        title: key.into(),
        active: false,
        ..persona_view(key)
    };
    let record = Record {
        chats: Vec::new(),
        views: vec![vault("ops"), vault("e2e-vault"), vault("team.prod_2")],
        dealt: 0,
        relaunch_after_update: false,
        clone_seat: None,
        focus: None,
    };

    write(plane.path(), &record).expect("the record is written");

    assert_eq!(read(plane.path()), record);
}

#[test]
fn a_view_tab_comes_back_as_it_was_recorded() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![claude("ide.7", Some(ID))],
        views: vec![
            persona_view("steward"),
            View {
                from: Some("persona-statistics".into()),
                view: "statistics".into(),
                key: String::new(),
                title: "Statistics".into(),
                workspace: None,
                at: 2,
                active: false,
                pinned: true,
                split: None,
            },
        ],
        dealt: 0,
        relaunch_after_update: false,
        clone_seat: None,
        focus: None,
    };

    write(plane.path(), &record).expect("the record is written");

    assert_eq!(read(plane.path()), record);
}

#[test]
fn a_record_written_before_tabs_held_views_reads_as_no_view_tabs() {
    // Every operator's record at the first launch after this change. The chats in it must
    // still come back: a version bump here would have dropped them all.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[{"program":"claude","name":"ide.7"}]}"#,
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(back.chats.len(), 1);
    assert!(back.views.is_empty());
}

#[test]
fn a_plane_with_no_view_tabs_writes_no_views_key() {
    // So the record of a plane that never opened a view is byte for byte the record an
    // older charter wrote and reads.
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", None)],
            ..Default::default()
        },
    )
    .unwrap();

    let text = fs::read_to_string(path(plane.path())).unwrap();

    assert!(!text.contains("\"views\""), "{text}");
}

#[test]
fn a_view_line_that_is_not_one_charter_would_write_is_dropped_and_the_rest_kept() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[],"views":[
            {"view":"persona","key":"../../etc"},
            {"from":"a/b","view":"statistics"},
            {"view":""},
            {"view":"persona","key":"steward","workspace":"no/such"}
        ]}"#,
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(back.views.len(), 1, "{:?}", back.views);
    assert_eq!(back.views[0].key, "steward");
    assert_eq!(
        back.views[0].workspace, None,
        "a name that is not a workspace files the tab outside every workspace"
    );
}

#[test]
fn a_branch_s_file_tab_comes_back_with_its_split() {
    // FM-2 (#1105): the file tab is keyed by its branch, `workspace/repo/piece`, or
    // `workspace/repo/` for the repo's own folder, and remembers where its divider was.
    let plane = tempfile::tempdir().unwrap();
    let files = |key: &str, split: Option<u8>| View {
        view: "piece-files".into(),
        title: "Files · fix-login".into(),
        active: false,
        split,
        ..persona_view(key)
    };
    let record = Record {
        views: vec![
            files("alpha/svc/fix-login", Some(30)),
            files("alpha/svc/", None),
        ],
        ..Default::default()
    };

    write(plane.path(), &record).expect("the record is written");

    assert_eq!(read(plane.path()), record);
}

#[test]
fn the_branch_a_window_focused_comes_back_with_its_views() {
    // FM-5 (#1108): the cockpit's branch is remembered with the window's saved views, so
    // one window can stay on one branch across a relaunch.
    let plane = tempfile::tempdir().unwrap();
    for piece in [Some("fix-login".to_string()), None] {
        let record = Record {
            views: vec![persona_view("steward")],
            focus: Some(Focus {
                workspace: "alpha".into(),
                repo: "svc".into(),
                piece: piece.clone(),
            }),
            ..Default::default()
        };

        write(plane.path(), &record).expect("the record is written");

        assert_eq!(read(plane.path()), record, "{piece:?}");
    }
}

#[test]
fn a_focus_that_names_no_branch_is_forgotten_and_the_rest_comes_back() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    for focus in [
        r#"{"workspace":"alpha","repo":"../etc","piece":"fix"}"#,
        r#"{"workspace":"","repo":"svc","piece":"fix"}"#,
        r#"{"workspace":"alpha","repo":"svc","piece":"a/b"}"#,
        format!(
            r#"{{"workspace":"{}","repo":"svc","piece":"fix"}}"#,
            "w".repeat(65)
        )
        .as_str(),
        r#"{"workspace":"alpha"}"#,
        r#""alpha/svc/fix""#,
    ] {
        std::fs::write(
            path(plane.path()),
            format!(
                r#"{{"version":1,"at":0,"chats":[],"views":[{{"view":"persona","key":"steward"}}],"focus":{focus}}}"#
            ),
        )
        .unwrap();

        let back = read(plane.path());

        assert_eq!(back.focus, None, "{focus}");
        assert_eq!(back.views.len(), 1, "{focus}");
    }
}

#[test]
fn a_focus_alone_is_nothing_a_launch_asks_about() {
    let record = Record {
        focus: Some(Focus {
            workspace: "alpha".into(),
            repo: "svc".into(),
            piece: None,
        }),
        ..Default::default()
    };
    assert!(!record.holds_anything());
}

#[test]
fn a_file_tab_line_that_names_no_branch_is_dropped_and_a_wild_split_is_forgotten() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[],"views":[
            {"view":"piece-files","key":"alpha/../etc"},
            {"view":"piece-files","key":"alpha/svc/fix/extra"},
            {"view":"piece-files","key":"/svc/fix"},
            {"view":"persona","key":"alpha/svc/fix"},
            {"view":"piece-files","key":"alpha/svc/fix","split":250},
            {"view":"piece-files","key":"alpha/svc/","split":0},
            {"view":"piece-files","key":"alpha/svc/","split":-3.5}
        ]}"#,
    )
    .unwrap();

    let back = read(plane.path());

    let kept: Vec<_> = back
        .views
        .iter()
        .map(|v| (v.key.as_str(), v.split))
        .collect();
    assert_eq!(
        kept,
        [
            ("alpha/svc/fix", None),
            ("alpha/svc/", None),
            ("alpha/svc/", None)
        ]
    );
}

/// A view tab of charter's own, keyed `key`, as the window would have recorded it.
fn own_view(view: &str, key: &str) -> View {
    View {
        view: view.into(),
        title: "A tab".into(),
        active: false,
        ..persona_view(key)
    }
}

/// Every key shape the window writes for a view keyed by a path of names (#1297), one per
/// line, each as its writer spells it.
const HELD_SHAPES: &[(&str, &str)] = &[
    ("todo", "alpha/20261006-091500-ship-the-fix"),
    ("todo", "alpha/ship-the-fix.md"),
    ("memory", "workspace/alpha/20261006-091500-a-lesson"),
    ("memory", "persona/steward/a-lesson"),
    ("memory", "persona/_shared/a-lesson"),
    ("memory", "shared/a-lesson"),
    ("memory", "workspace/alpha/\\"),
    ("memory", "persona/steward/\\"),
    ("memory", "shared/\\"),
    ("memory-archive", "workspace/alpha"),
    ("memory-archive", "persona/steward"),
    ("memory-archive", "shared"),
    ("piece-files", "alpha/svc/fix-login"),
    ("piece-files", "alpha/svc/"),
    ("piece-file", "alpha/svc/fix-login/src/main.rs"),
    ("piece-file", "alpha/svc//README.md"),
    ("piece-file", "alpha/svc/fix-login/.github/workflows/ci.yml"),
    ("piece-diff", "alpha/svc/fix-login/src/main.rs"),
    ("piece-diff", "alpha/svc//Cargo.toml"),
    (
        "session",
        "workspaces/alpha/sessions/20260928-140312-ship-it.md",
    ),
    ("session", "sessions/20260928-140312-ship-it.md"),
];

#[test]
fn todo_memory_session_and_file_tabs_come_back_after_a_relaunch() {
    // #1297: each of these was dropped from the record at every launch.
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        views: HELD_SHAPES
            .iter()
            .map(|(view, key)| own_view(view, key))
            .collect(),
        ..Default::default()
    };

    write(plane.path(), &record).expect("the record is written");

    let back: Vec<_> = read(plane.path())
        .views
        .iter()
        .map(|v| (v.view.clone(), v.key.clone()))
        .collect();
    let wrote: Vec<_> = HELD_SHAPES
        .iter()
        .map(|(view, key)| (view.to_string(), key.to_string()))
        .collect();
    assert_eq!(back, wrote);
}

#[test]
fn a_path_shaped_key_that_is_not_its_view_s_own_shape_is_dropped() {
    // #1297: `reopen.json` is writable by whoever can write the state folder, so each view
    // holds its key to the one shape its writer spells, and nothing else comes back.
    let refused: &[(&str, &str)] = &[
        // todo: `<ws>/<slug>`
        ("todo", ""),
        ("todo", "alpha"),
        ("todo", "alpha/"),
        ("todo", "/ship"),
        ("todo", "alpha/../ship"),
        ("todo", "alpha/.."),
        ("todo", "../alpha/ship"),
        ("todo", "/etc/passwd"),
        ("todo", "alpha/ship/more"),
        ("todo", "alpha//ship"),
        ("todo", ".hidden/ship"),
        ("todo", "alpha/a\\b"),
        ("todo", "alpha/\\"),
        ("todo", "alpha/MEMORY"),
        ("todo", "alpha/sh\nip"),
        ("todo", "alpha/sh\0ip"),
        ("todo", "C:/ship"),
        // memory: `workspace|persona/<name>/<slug>`, `shared/<slug>`
        ("memory", ""),
        ("memory", "steward"),
        ("memory", "workspace/alpha"),
        ("memory", "workspace/alpha/"),
        ("memory", "workspace//lesson"),
        ("memory", "workspace/../lesson"),
        ("memory", "workspace/alpha/.."),
        ("memory", "workspace/alpha/lesson/more"),
        ("memory", "persona/Steward/lesson"),
        ("memory", "persona/../lesson"),
        ("memory", "persona/steward/a\\b"),
        ("memory", "persona/steward/\\\\"),
        ("memory", "shared/.."),
        ("memory", "shared/"),
        ("memory", "shared/a/b"),
        ("memory", "workspaces/alpha/lesson"),
        ("memory", "/workspace/alpha/lesson"),
        ("memory", "workspace/alpha/MEMORY.md"),
        // memory-archive: `workspace|persona/<name>`, `shared`
        ("memory-archive", ""),
        ("memory-archive", "alpha"),
        ("memory-archive", "workspace/"),
        ("memory-archive", "workspace/.."),
        ("memory-archive", "workspace/alpha/more"),
        ("memory-archive", "persona/Steward"),
        ("memory-archive", "shared/x"),
        ("memory-archive", "/shared"),
        // piece-files: `<ws>/<repo>/<piece>` or `<ws>/<repo>/`
        ("piece-files", "alpha"),
        ("piece-files", "alpha/../etc"),
        ("piece-files", "alpha/svc/fix/extra"),
        ("piece-files", "/svc/fix"),
        // piece-file and piece-diff: a branch, then a plain relative path
        ("piece-file", "alpha/svc/fix"),
        ("piece-file", "alpha/svc/fix/"),
        ("piece-file", "alpha/svc/fix/src//main.rs"),
        ("piece-file", "alpha/svc/fix/../../../etc/passwd"),
        ("piece-file", "alpha/svc/fix/src/../../x"),
        ("piece-file", "alpha/svc/fix/./src"),
        ("piece-file", "alpha/svc/fix//etc/passwd"),
        ("piece-file", "alpha/svc/fix/src\\..\\x"),
        ("piece-file", "alpha/svc/fix/C:x"),
        ("piece-file", "alpha/svc/fix/a\nb"),
        ("piece-file", "../svc/fix/src/main.rs"),
        ("piece-file", "alpha/../fix/src/main.rs"),
        ("piece-file", "alpha/svc/../src/main.rs"),
        ("piece-file", "alpha"),
        ("piece-diff", "alpha/svc/fix"),
        ("piece-diff", "alpha/svc/fix/../../x"),
        ("piece-diff", "/alpha/svc/fix/src/main.rs"),
        ("piece-diff", "alpha/svc/fix/src/"),
        // session: `workspaces/<ws>/sessions/<file>` or `sessions/<file>`
        ("session", ""),
        ("session", "20260928-140312-ship-it.md"),
        ("session", "/etc/passwd"),
        ("session", "sessions/index.md"),
        ("session", "sessions/../charter.toml"),
        (
            "session",
            "workspaces/../sessions/20260928-140312-ship-it.md",
        ),
        (
            "session",
            "workspaces/alpha/sessions/../../../20260928-140312-ship-it.md",
        ),
        (
            "session",
            "workspaces/alpha/sessions/sub/20260928-140312-ship-it.md",
        ),
        (
            "session",
            "workspaces/alpha/memory/20260928-140312-ship-it.md",
        ),
        (
            "session",
            "workspaces/.hidden/sessions/20260928-140312-ship-it.md",
        ),
        ("session", "sessions\\..\\20260928-140312-ship-it.md"),
        ("session", "/sessions/20260928-140312-ship-it.md"),
        ("session", "./sessions/20260928-140312-ship-it.md"),
        ("session", "alpha"),
    ];
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    for (view, key) in refused {
        let line = serde_json::json!({
            "version": 1, "at": 0, "chats": [],
            "views": [{"view": view, "key": key}, {"view": "persona", "key": "steward"}],
        });
        std::fs::write(path(plane.path()), line.to_string()).unwrap();

        let back = read(plane.path());

        let kept: Vec<_> = back.views.iter().map(|v| v.view.as_str()).collect();
        assert_eq!(kept, ["persona"], "{view} {key:?} was taken");
    }
}

#[test]
fn a_key_longer_than_any_charter_writes_is_dropped() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    let deep = format!("alpha/svc/fix/{}", "d/".repeat(MOST_KEY / 2) + "f");
    let line = serde_json::json!({
        "version": 1, "at": 0, "chats": [],
        "views": [{"view": "piece-file", "key": deep}],
    });
    std::fs::write(path(plane.path()), line.to_string()).unwrap();

    assert!(read(plane.path()).views.is_empty());
}

#[test]
fn an_extension_s_view_is_not_held_to_charter_s_shape_for_a_view_of_the_same_name() {
    // An extension's `todo` is its own view: its key is one word, as every extension's is,
    // and a path is still not one.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    let line = serde_json::json!({
        "version": 1, "at": 0, "chats": [],
        "views": [
            {"from": "acme", "view": "todo", "key": "steward"},
            {"from": "acme", "view": "todo", "key": "alpha/ship"},
        ],
    });
    std::fs::write(path(plane.path()), line.to_string()).unwrap();

    let back = read(plane.path());

    let kept: Vec<_> = back.views.iter().map(|v| v.key.as_str()).collect();
    assert_eq!(kept, ["steward"]);
}

#[test]
fn a_view_title_that_is_not_one_line_of_words_is_the_view_s_own_id() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    let long = "x".repeat(MOST_TITLE + 1);
    std::fs::write(
        path(plane.path()),
        format!(
            r#"{{"version":1,"at":0,"chats":[],"views":[
                {{"view":"one","title":"two\nlines"}},
                {{"view":"two","title":"{long}"}},
                {{"view":"three","title":"  "}},
                {{"view":"four","title":"Statistics"}}
            ]}}"#
        ),
    )
    .unwrap();

    let titles: Vec<String> = read(plane.path())
        .views
        .into_iter()
        .map(|view| view.title)
        .collect();

    assert_eq!(titles, vec!["one", "two", "three", "Statistics"]);
}

#[test]
fn a_record_puts_back_no_more_view_tabs_than_the_bound() {
    let plane = tempfile::tempdir().unwrap();
    let many: Vec<View> = (0..MOST_VIEWS + 5)
        .map(|at| View {
            key: format!("p{at}"),
            active: false,
            ..persona_view("x")
        })
        .collect();
    write(
        plane.path(),
        &Record {
            views: many,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(read(plane.path()).views.len(), MOST_VIEWS);
}

#[test]
fn a_record_of_exactly_the_largest_size_is_still_read() {
    // The bound is "never LARGER than", as `charter/contain.py`'s `st.st_size > MAX_BYTES`
    // draws it: a record of exactly the cap is an honest one and is read.
    let held = tempfile::tempdir().unwrap();
    let plane = held.path().to_path_buf();
    write(&plane, &one_chat()).unwrap();
    let mut text = std::fs::read(path(&plane)).unwrap();
    text.resize(usize::try_from(MAX_BYTES).unwrap(), b' ');
    std::fs::write(path(&plane), &text).unwrap();

    assert_eq!(read_or_refusal(&plane).unwrap(), one_chat());
}

// ----- the choice at a relaunch (charter-app#250) -----

fn a_record_with_a_chat_and_a_view() -> Record {
    Record {
        chats: vec![Chat {
            number: Some(3),
            ..claude("ide.7", Some(ID))
        }],
        views: vec![persona_view("steward")],
        dealt: 5,
        ..Default::default()
    }
}

#[test]
fn a_record_with_no_chat_and_no_view_tab_has_nothing_to_ask_about() {
    // A first launch, and a plane whose last quit had nothing open: no question.
    assert!(!Record::default().holds_anything());
    assert!(
        !Record {
            dealt: 9,
            ..Default::default()
        }
        .holds_anything(),
        "a counter is bookkeeping, not something that was open"
    );
}

#[test]
fn a_chat_or_a_view_tab_alone_is_something_to_ask_about() {
    let chat = Record {
        chats: vec![claude("ide.7", None)],
        ..Default::default()
    };
    let view = Record {
        views: vec![persona_view("steward")],
        ..Default::default()
    };

    assert!(chat.holds_anything());
    assert!(view.holds_anything());
}

#[test]
fn reopen_all_puts_back_the_record_exactly_as_it_was() {
    let record = a_record_with_a_chat_and_a_view();

    assert_eq!(record.clone().chosen(Choice::ReopenAll), record);
}

#[test]
fn start_fresh_puts_back_no_chat_and_no_view_tab() {
    let fresh = a_record_with_a_chat_and_a_view().chosen(Choice::StartFresh);

    assert!(fresh.chats.is_empty());
    assert!(fresh.views.is_empty());
    assert!(!fresh.holds_anything());
}

#[test]
fn start_fresh_keeps_every_number_already_dealt_so_none_is_dealt_twice() {
    // charter-app#90: a number is never dealt twice. Chat 3's workspace pointer and lock
    // are still on disk, so the first chat after a fresh start must not be 3 again —
    // nor 4 or 5, which the counter says were dealt to chats closed before the quit.
    let fresh = a_record_with_a_chat_and_a_view().chosen(Choice::StartFresh);

    assert_eq!(fresh.dealt, 5);

    let only_the_chat_knew = Record {
        chats: vec![Chat {
            number: Some(7),
            ..claude("ide.7", None)
        }],
        dealt: 2,
        ..Default::default()
    }
    .chosen(Choice::StartFresh);
    assert_eq!(only_the_chat_knew.dealt, 7);
}

#[test]
fn a_fresh_start_written_and_read_back_is_still_nothing_to_reopen() {
    let plane = tempfile::tempdir().unwrap();

    write(
        plane.path(),
        &a_record_with_a_chat_and_a_view().chosen(Choice::StartFresh),
    )
    .unwrap();
    let back = read(plane.path());

    assert!(!back.holds_anything());
    assert_eq!(back.dealt, 5);
}

#[test]
fn a_relaunch_after_an_update_is_said_in_the_record_and_read_back() {
    // charter-app#251's way in: the quit that installs an update writes this, and the
    // launch after it knows why it is being asked.
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        relaunch_after_update: true,
        ..a_record_with_a_chat_and_a_view()
    };

    write(plane.path(), &record).unwrap();

    assert_eq!(read(plane.path()), record);
}

#[test]
fn an_ordinary_quit_writes_no_update_key_and_an_absent_one_reads_as_an_ordinary_quit() {
    let plane = tempfile::tempdir().unwrap();
    write(plane.path(), &a_record_with_a_chat_and_a_view()).unwrap();

    let text = fs::read_to_string(path(plane.path())).unwrap();

    assert!(!text.contains("relaunch_after_update"), "{text}");
    assert!(!read(plane.path()).relaunch_after_update);
}

#[test]
fn a_fresh_start_is_not_a_relaunch_after_an_update_any_more() {
    // The flag is about the launch that reads it, and no later one.
    let fresh = Record {
        relaunch_after_update: true,
        ..a_record_with_a_chat_and_a_view()
    }
    .chosen(Choice::StartFresh);

    assert!(!fresh.relaunch_after_update);
}

// ----- the name the operator gave a chat (charter-app#254) ----------------------------

#[test]
fn a_chat_comes_back_under_the_name_the_operator_gave_it() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![Chat {
            label: Some("billing bug".into()),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    assert_eq!(
        read(plane.path()).chats[0].label.as_deref(),
        Some("billing bug")
    );
}

#[test]
fn a_record_written_before_chats_had_names_reads_as_the_default_one() {
    // Not a format change, for `Chat::pinned`'s reason: a missing key reads as "no name
    // given", which is what was true of every record written before a chat could have one.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        br#"{"version":1,"at":0,"chats":[{"program":"claude","name":"3"}]}"#,
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(back.chats.len(), 1);
    assert_eq!(back.chats[0].label, None);
}

#[test]
fn a_chat_with_no_name_given_writes_no_label_key() {
    // So the record a plane that never renamed a chat writes is the one it always wrote.
    let plane = tempfile::tempdir().unwrap();
    write(plane.path(), &one_chat()).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();

    assert!(!text.contains("\"label\""), "{text}");
}

#[test]
fn a_label_off_disk_charter_would_refuse_reads_as_no_name_given() {
    // The file is writable by whoever can write the plane's state directory, and the label
    // is drawn on a tab: a line that would draw as something else keeps its chat and loses
    // only the name.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    std::fs::write(
        path(plane.path()),
        "{\"version\":1,\"at\":0,\"chats\":[{\"program\":\"claude\",\"name\":\"3\",\
         \"label\":\"pay\u{202e}lanigiro\"}]}",
    )
    .unwrap();

    let back = read(plane.path());

    assert_eq!(back.chats.len(), 1);
    assert_eq!(back.chats[0].label, None);
}

#[test]
fn a_label_is_trimmed() {
    assert_eq!(label("  billing bug \t"), Ok(Some("billing bug".into())));
}

#[test]
fn a_blank_label_is_no_name_given() {
    assert_eq!(label(""), Ok(None));
    assert_eq!(label("   "), Ok(None));
}

#[test]
fn a_label_of_the_longest_length_is_kept_and_one_longer_is_refused() {
    let longest = "é".repeat(MOST_LABEL);
    assert_eq!(label(&longest), Ok(Some(longest.clone())));

    let refused = label(&format!("{longest}x")).unwrap_err();
    assert!(refused.contains(&MOST_LABEL.to_string()), "{refused}");
}

#[test]
fn a_label_with_a_control_character_is_refused() {
    assert!(label("two\nlines").is_err());
    assert!(label("bell\u{7}").is_err());
}

#[test]
fn a_label_with_an_invisible_character_is_refused() {
    // Each makes two different names look like one on the strip, or turns the words
    // around it backwards.
    for sneaky in ["pay\u{202e}lanigiro", "a\u{200b}b", "\u{feff}steward"] {
        assert!(label(sneaky).is_err(), "{sneaky:?} was let through");
    }
}

#[test]
fn a_space_inside_a_label_is_content() {
    assert_eq!(label("steward 1"), Ok(Some("steward 1".into())));
}

// ----- where a handed-off chat came from (charter-app#258, #259) -----------------------

fn handed() -> HandedFrom {
    HandedFrom {
        id: None,
        chat: 16,
        name: "steward 3".into(),
        workspace: crate::active::Place::Workspace("platform-next".into()),
        report: Owed::Due,
        mode: Mode::Handoff,
        depth: 0,
        root: None,
        above: None,
        by_person: false,
    }
}

/// A handoff as this build writes one: it moved the work, and asked for no report. One that
/// asked for a report is an older build's, and is read as a task (#1519).
fn a_handoff() -> HandedFrom {
    HandedFrom {
        id: None,
        report: Owed::Nothing,
        ..handed()
    }
}

// ----- a task chat is listed without a tab until the person opens it (#1447) ----------

#[test]
fn a_task_chat_comes_back_a_task_with_no_tab_until_the_person_opened_it() {
    let plane = tempfile::tempdir().unwrap();
    let task = HandedFrom {
        id: None,
        mode: Mode::Task,
        ..handed()
    };
    let listed = Chat {
        from: Some(task.clone()),
        ..claude("3", None)
    };
    let opened = Chat {
        tab_opened: true,
        ..listed.clone()
    };
    let record = Record {
        chats: vec![listed, opened],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let back = read(plane.path()).chats;
    assert_eq!(back[0].from, Some(task));
    assert!(!back[0].has_tab(), "listed, and on no strip");
    assert!(back[1].has_tab(), "the tab the person opened comes back");
}

#[test]
fn what_a_sessions_tab_shows_comes_back_and_a_tab_on_its_own_chat_writes_nothing_of_it() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![
            Chat {
                number: Some(4),
                shows: Some(9),
                ..claude("4", None)
            },
            Chat {
                number: Some(5),
                ..claude("5", None)
            },
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches(r#""shows""#).count(), 1, "{text}");
    let back = read(plane.path()).chats;
    assert_eq!(back[0].shows, Some(9));
    assert_eq!(back[1].shows, None);
}

#[test]
fn the_tab_a_chat_has_a_pane_in_comes_back_and_a_chat_in_no_split_writes_nothing_of_it() {
    // #1489: a task opened beside its session is put back beside it, and the core reads
    // the same number for which chats are on screen with the one in front.
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![
            Chat {
                number: Some(4),
                ..claude("4", None)
            },
            Chat {
                number: Some(9),
                beside: Some(4),
                ..claude("9", None)
            },
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches(r#""beside""#).count(), 1, "{text}");
    let back = read(plane.path()).chats;
    assert_eq!(back[0].beside, None);
    assert_eq!(back[1].beside, Some(4));
}

#[test]
fn a_handoff_and_a_chat_the_person_opened_have_a_tab_and_write_the_record_they_always_wrote() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![
            Chat {
                from: Some(a_handoff()),
                ..claude("3", None)
            },
            claude("4", None),
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert!(
        !text.contains(r#""mode""#) && !text.contains("tab_opened"),
        "{text}"
    );
    let back = read(plane.path()).chats;
    assert_eq!(
        back[0].from.as_ref().map(|from| from.mode),
        Some(Mode::Handoff)
    );
    assert!(back[0].has_tab() && back[1].has_tab());
}

#[test]
fn a_mode_nobody_wrote_reads_as_a_handoff() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![Chat {
            from: Some(HandedFrom {
                id: None,
                mode: Mode::Task,
                ..handed()
            }),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert!(text.contains(r#""mode": "task""#), "{text}");
    std::fs::write(
        path(plane.path()),
        text.replace(r#""mode": "task""#, r#""mode": "hidden""#),
    )
    .unwrap();

    // Fails open to a tab: a chat is never hidden by a word purlis did not write.
    assert!(read(plane.path()).chats[0].has_tab());
}

#[test]
fn a_task_comes_back_a_task_at_its_depth_and_a_handoff_writes_neither_key() {
    // #1436: the lineage a dispatch recorded outlives a relaunch, which is what keeps a
    // task listed under the chat that asked and its depth counted.
    let plane = tempfile::tempdir().unwrap();
    let task = HandedFrom {
        id: None,
        mode: crate::dispatchdecision::Mode::Task,
        depth: 2,
        above: None,
        by_person: false,
        ..handed()
    };
    let record = Record {
        chats: vec![
            Chat {
                from: Some(task.clone()),
                ..claude("3", None)
            },
            Chat {
                from: Some(a_handoff()),
                ..claude("4", None)
            },
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let back = read(plane.path());
    assert_eq!(back.chats[0].from, Some(task));
    assert_eq!(back.chats[1].from, Some(a_handoff()));
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"mode\"").count(), 1, "{text}");
    assert_eq!(text.matches("\"depth\"").count(), 1, "{text}");
}

#[test]
fn a_lineage_s_root_comes_back_with_the_chat_and_a_record_without_one_writes_no_key() {
    // The root is what keeps a lineage one lineage when a chat in the middle closes: it
    // has to outlive a relaunch, as the depth does.
    let plane = tempfile::tempdir().unwrap();
    let root = "01J9ZQ3V5N8X4T2K7M6P0R1S2A";
    let below = HandedFrom {
        id: None,
        mode: Mode::Task,
        depth: 1,
        root: Some(root.to_owned()),
        ..handed()
    };
    let record = Record {
        chats: vec![
            Chat {
                from: Some(below.clone()),
                ..claude("3", None)
            },
            Chat {
                from: Some(a_handoff()),
                ..claude("4", None)
            },
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let back = read(plane.path());
    assert_eq!(back.chats[0].from, Some(below));
    assert_eq!(back.chats[1].from, Some(a_handoff()));
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"root\"").count(), 1, "{text}");
}

#[test]
fn a_root_that_is_not_an_id_purlis_mints_names_no_lineage() {
    // The file is writable by anything running as the person. A root is compared, never
    // drawn or run, and one in no shape an id has is dropped: the chat is then counted by
    // who dispatched whom, as a record written before roots were kept is.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(path(plane.path()).parent().unwrap()).unwrap();
    for (written, read_as) in [
        ("../../etc", None),
        ("", None),
        (
            "01j9zq3v5n8x4t2k7m6p0r1s2a",
            Some("01J9ZQ3V5N8X4T2K7M6P0R1S2A"),
        ),
    ] {
        let from = format!(
            r#"{{"chat":16,"name":"steward 3","workspace":"ops","mode":"task","root":"{written}"}}"#
        );
        std::fs::write(
            path(plane.path()),
            format!(
                r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","from":{from}}}]}}"#
            ),
        )
        .unwrap();
        let back = read(plane.path()).chats[0].from.clone().expect("it reads");
        assert_eq!(back.root.as_deref(), read_as, "{written}");
    }
}

#[test]
fn the_personas_above_a_task_come_back_with_it_and_an_older_record_writes_no_key() {
    // #1521: the chain the loop rule reads outlives a relaunch, as the depth does, and a
    // chat on no persona above it comes back as one.
    let plane = tempfile::tempdir().unwrap();
    let below = HandedFrom {
        id: None,
        mode: Mode::Task,
        depth: 2,
        above: Some(vec![Some("devops".to_owned()), None]),
        ..handed()
    };
    let record = Record {
        chats: vec![
            Chat {
                from: Some(below.clone()),
                ..claude("3", None)
            },
            // A handoff that asked for nothing (#1519: one an older build opened owing a
            // report reads as a task).
            Chat {
                from: Some(a_handoff()),
                ..claude("4", None)
            },
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let back = read(plane.path());
    assert_eq!(back.chats[0].from, Some(below));
    assert_eq!(back.chats[1].from, Some(a_handoff()));
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"above\"").count(), 1, "{text}");
    let squeezed: String = text.split_whitespace().collect();
    assert!(squeezed.contains(r#""above":["devops",""]"#), "{text}");
}

#[test]
fn personas_above_that_the_app_would_not_have_written_read_as_a_chain_not_kept() {
    // The record is the app's. One whose chain is not one per dispatch above, or names
    // what cannot be a persona, is read as kept by no one: the loop rule then reads it as a
    // chain it cannot read whole, never as a shorter one.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(path(plane.path()).parent().unwrap()).unwrap();
    let steward = || Some(vec![Some("steward".to_owned())]);
    for (depth, above, read_as) in [
        (1, r#"["steward"]"#, steward()),
        (1, r#"[""]"#, Some(vec![None])),
        (0, "[]", Some(Vec::new())),
        // Shorter or longer than the depth says.
        (2, r#"["steward"]"#, None),
        (1, "[]", None),
        (1, r#"["steward","devops"]"#, None),
        // Not a persona's name.
        (1, r#"["../steward"]"#, None),
        (1, r#"["Steward"]"#, None),
    ] {
        let from = format!(
            r#"{{"chat":16,"name":"steward 3","workspace":"ops","mode":"task","depth":{depth},"above":{above}}}"#
        );
        std::fs::write(
            path(plane.path()),
            format!(
                r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","from":{from}}}]}}"#
            ),
        )
        .unwrap();
        let back = read(plane.path()).chats[0].from.clone().expect("it reads");
        assert_eq!(back.above, read_as, "{depth} {above}");
    }
    // A record written before the key: no chain kept.
    std::fs::write(
        path(plane.path()),
        r#"{"version":1,"at":0,"chats":[{"program":"claude","name":"3","from":{"chat":16,"name":"steward 3","workspace":"ops","mode":"task","depth":1}}]}"#,
    )
    .unwrap();
    let back = read(plane.path()).chats[0].from.clone().expect("it reads");
    assert_eq!(back.above, None);
}

#[test]
fn a_chat_the_person_started_from_a_tab_comes_back_saying_so_and_no_other_writes_the_key() {
    // #1438: who a report is marked as started by outlives a relaunch.
    let plane = tempfile::tempdir().unwrap();
    let asked = HandedFrom {
        id: None,
        mode: crate::dispatchdecision::Mode::Task,
        depth: 1,
        above: None,
        by_person: true,
        ..handed()
    };
    let record = Record {
        chats: vec![
            Chat {
                from: Some(asked.clone()),
                ..claude("3", None)
            },
            Chat {
                from: Some(a_handoff()),
                ..claude("4", None)
            },
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let back = read(plane.path());
    assert_eq!(back.chats[0].from, Some(asked));
    assert_eq!(back.chats[1].from, Some(a_handoff()));
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"by\"").count(), 1, "{text}");
}

#[test]
fn a_handed_off_chat_keeps_the_handing_chat_s_id_across_a_relaunch_and_no_other_has_one() {
    // #872 (ADR 0066's `handed_from`): the id of the chat that handed it off, beside the
    // number its reports are left under, so the link still resolves once that number is
    // dealt again or the chat is read on another machine.
    let plane = tempfile::tempdir().unwrap();
    let handing = mint();
    let handed_off = HandedFrom {
        id: Some(handing.clone()),
        ..a_handoff()
    };
    let record = Record {
        chats: vec![
            Chat {
                from: Some(handed_off.clone()),
                ..claude("3", None)
            },
            // Started any other way: no chat handed it off, so it names none.
            claude("4", None),
        ],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let back = read(plane.path());
    assert_eq!(back.chats[0].from, Some(handed_off));
    assert_eq!(
        back.chats[0].from.as_ref().and_then(|from| from.id.clone()),
        Some(handing.clone())
    );
    assert_eq!(back.chats[1].from, None);
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(
        text.matches(&format!("\"id\": \"{handing}\"")).count(),
        1,
        "{text}"
    );
}

#[test]
fn a_handing_chat_s_id_that_is_not_one_purlis_mints_reads_as_unknown() {
    // The file is writable by anything running as the person: an id is held to the one
    // shape it is minted in, and a record written before the key reads as unknown.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(path(plane.path()).parent().unwrap()).unwrap();
    for id in [r#","id":"chat 16""#, r#","id":"""#, ""] {
        std::fs::write(
            path(plane.path()),
            format!(
                r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","from":{{"chat":16,"name":"steward 3","workspace":"ops"{id}}}}}]}}"#
            ),
        )
        .unwrap();
        let back = read(plane.path()).chats[0].from.clone();
        assert_eq!(back.map(|from| from.id), Some(None), "{id}");
    }
}

#[test]
fn a_depth_no_chain_can_have_reads_as_the_deepest_one_can() {
    // The file is writable by anything running as the person: a depth past the ceiling
    // is held to it, so a record never says a chain is shallower than it could be.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    let from =
        r#"{"chat":16,"name":"steward 3","workspace":"ops","mode":"task","depth":4000000000}"#;
    std::fs::write(
        path(plane.path()),
        format!(
            r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","from":{from}}}]}}"#
        ),
    )
    .unwrap();

    let back = read(plane.path()).chats[0].from.clone().expect("it reads");
    assert_eq!(back.depth, crate::dispatchdecision::DEEPEST);
    assert_eq!(back.mode, crate::dispatchdecision::Mode::Task);
}

#[test]
fn a_handed_off_chat_comes_back_knowing_where_it_came_from_and_what_it_owes() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![Chat {
            from: Some(a_handoff()),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    assert_eq!(read(plane.path()).chats[0].from, Some(a_handoff()));
}

#[test]
fn a_task_the_app_reported_failed_for_comes_back_so_and_is_never_owed_again() {
    // #1443: a relaunch does not make a chat that ended without a report owe one again.
    let plane = tempfile::tempdir().unwrap();
    let failed = HandedFrom {
        id: None,
        report: Owed::Failed,
        mode: crate::dispatchdecision::Mode::Task,
        depth: 1,
        ..handed()
    };
    let record = Record {
        chats: vec![Chat {
            from: Some(failed.clone()),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    assert_eq!(read(plane.path()).chats[0].from, Some(failed));
}

#[test]
fn a_task_that_may_report_once_more_comes_back_so() {
    // #1768: a blocked task put back at a launch can still send the report it has left.
    let plane = tempfile::tempdir().unwrap();
    let again = HandedFrom {
        id: None,
        report: Owed::Again,
        mode: crate::dispatchdecision::Mode::Task,
        depth: 1,
        ..handed()
    };
    let record = Record {
        chats: vec![Chat {
            from: Some(again.clone()),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert!(text.contains(r#""report": "again""#), "{text}");
    assert_eq!(read(plane.path()).chats[0].from, Some(again));
}

#[test]
fn a_report_already_sent_comes_back_sent() {
    let plane = tempfile::tempdir().unwrap();
    let sent = HandedFrom {
        id: None,
        report: Owed::Sent,
        ..handed()
    };
    let record = Record {
        chats: vec![Chat {
            from: Some(sent.clone()),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    assert_eq!(read(plane.path()).chats[0].from, Some(sent));
}

/// #1362, D-1362-5/6: a chat holding another persona's grants keeps holding them across a
/// relaunch, and the hold is read failing closed: a value that is not a persona's name holds
/// no persona's grants, and a handoff note that does not read never drops it.
#[test]
fn a_chat_holding_another_persona_s_grants_comes_back_holding_them() {
    let plane = tempfile::tempdir().unwrap();
    let holding = |persona: Option<&str>| Chat {
        from: Some(handed()),
        held: Some(HeldGrants {
            persona: persona.map(str::to_owned),
        }),
        ..claude("3", None)
    };
    for persona in [Some("qa"), None] {
        let record = Record {
            chats: vec![holding(persona)],
            ..Default::default()
        };
        write(plane.path(), &record).unwrap();
        assert_eq!(
            read(plane.path()).chats[0].held,
            holding(persona).held,
            "{persona:?}"
        );
    }
    let held_none = Some(HeldGrants { persona: None });
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    for (from, to) in [
        (r#""held": """#, r#""held": "../Not A Persona""#),
        (r#""held": """#, r#""held": 7"#),
        // The note around it unsound: the hold still reads.
        (r#""name": "steward 3""#, r#""name": """#),
    ] {
        std::fs::write(path(plane.path()), text.replace(from, to)).unwrap();
        assert_eq!(read(plane.path()).chats[0].held, held_none, "{to}");
    }
}

#[test]
fn a_chat_handed_off_from_the_plane_root_comes_back_knowing_it() {
    // SI-1b: the root is no workspace, and reads back as the root rather than as nothing.
    let plane = tempfile::tempdir().unwrap();
    let from_root = HandedFrom {
        id: None,
        workspace: crate::active::Place::PlaneRoot,
        ..a_handoff()
    };
    let record = Record {
        chats: vec![Chat {
            from: Some(from_root.clone()),
            ..claude("3", None)
        }],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert!(text.contains(r#""workspace": "plane root""#), "{text}");
    assert_eq!(read(plane.path()).chats[0].from, Some(from_root));
}

/// #1519: a chat an older build opened by a handoff that asked for a report still owes it
/// after an update, and is read as the task of the chat that asked: its report wakes that
/// chat, it is listed and waited on, and an end without a report is reported for. It keeps
/// the tab it always had, and it is never read as wider than it was.
#[test]
fn a_handoff_an_older_build_opened_owing_a_report_reads_as_the_task_of_the_chat_that_asked() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(path(plane.path()).parent().unwrap()).unwrap();
    // What the older build wrote: no `mode`, a report owed, and no depth for a record
    // written before depths were kept.
    let older = r#"{"chat":16,"name":"steward 3","workspace":"ops","report":"owed"}"#;
    std::fs::write(
        path(plane.path()),
        format!(
            r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","number":17,"from":{older}}}]}}"#
        ),
    )
    .unwrap();

    let back = read(plane.path()).chats.remove(0);

    let from = back.from.clone().expect("it reads");
    assert_eq!(
        (from.chat, from.mode, from.report, from.by_person),
        (16, Mode::Task, Owed::Due, false)
    );
    // A chat a dispatch started is one dispatch deep at least: never shallower than it is.
    assert_eq!(from.depth, 1);
    assert!(back.has_tab(), "it keeps the tab the older build gave it");
    // The chat that asked owns it as it owns any task; no other chat does.
    assert!(crate::dispatched::owned(16, 17, Some(&from)).is_ok());
    assert!(crate::dispatched::owned(15, 17, Some(&from)).is_err());

    // Written again, it is the newer shape, and reads back the same.
    let record = Record {
        chats: vec![back.clone()],
        ..Default::default()
    };
    write(plane.path(), &record).unwrap();
    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert!(text.contains(r#""mode": "task""#), "{text}");
    assert_eq!(read(plane.path()).chats[0], back);
}

#[test]
fn a_handoff_an_older_build_opened_that_owes_nothing_now_is_the_handoff_it_was() {
    // Fire-and-forget, or its one report already sent: nothing is owed, so nothing is a
    // task, and a chat that reported is never read as owing again.
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(path(plane.path()).parent().unwrap()).unwrap();
    for (report, owed) in [("", Owed::Nothing), ("sent", Owed::Sent)] {
        let older =
            format!(r#"{{"chat":16,"name":"steward 3","workspace":"ops","report":"{report}"}}"#);
        std::fs::write(
            path(plane.path()),
            format!(
                r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","from":{older}}}]}}"#
            ),
        )
        .unwrap();

        let back = read(plane.path()).chats.remove(0);

        let from = back.from.clone().expect("it reads");
        assert_eq!(
            (from.mode, from.report, from.depth),
            (Mode::Handoff, owed, 0)
        );
        assert!(back.has_tab(), "{report}");
        assert!(
            crate::dispatched::owned(16, 3, Some(&from)).is_err(),
            "{report}"
        );
    }
}

#[test]
fn a_chat_the_operator_opened_writes_no_from_key() {
    let plane = tempfile::tempdir().unwrap();
    write(plane.path(), &one_chat()).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();

    assert!(!text.contains("\"from\""), "{text}");
}

#[test]
fn a_from_off_disk_charter_would_not_have_written_reads_as_no_handoff() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join(".charter/app")).unwrap();
    for from in [
        r#"{"chat":0,"name":"steward 3","workspace":"ops","report":"owed"}"#,
        "{\"chat\":16,\"name\":\"pay\\u202elanigiro\",\"workspace\":\"ops\",\"report\":\"owed\"}",
        r#"{"chat":16,"name":"steward 3","workspace":"../up","report":"owed"}"#,
        r#"{"chat":16,"name":"steward 3","workspace":"","report":"owed"}"#,
        r#"{"chat":16,"name":"steward 3","workspace":"Plane Root","report":"owed"}"#,
    ] {
        std::fs::write(
            path(plane.path()),
            format!(
                r#"{{"version":1,"at":0,"chats":[{{"program":"claude","name":"3","from":{from}}}]}}"#
            ),
        )
        .unwrap();

        let back = read(plane.path());

        assert_eq!(back.chats.len(), 1, "{from}");
        assert_eq!(back.chats[0].from, None, "{from}");
        // Owing a report in a note that does not read is no task: no tab is marked opened.
        assert!(!back.chats[0].tab_opened, "{from}");
    }
}

#[test]
fn a_chat_is_shown_under_its_given_name_else_its_persona_or_harness_and_its_own() {
    let steward = Chat {
        persona: Some("steward".into()),
        ..claude("3", None)
    };
    assert_eq!(shown_name(&steward, Some("claude")), "steward 3");
    assert_eq!(shown_name(&claude("4", None), Some("claude")), "claude 4");
    assert_eq!(shown_name(&claude("5", None), None), "5");
    let named = Chat {
        label: Some("billing bug".into()),
        ..steward
    };
    assert_eq!(shown_name(&named, Some("claude")), "billing bug");
}

#[test]
fn a_restart_to_update_is_taken_by_the_next_launch_and_by_no_launch_after_it() {
    // charter-app#251's scope: a plane the launch after the restart did not open keeps its
    // flag on disk, and a launch a week later must not read it as "charter restarted".
    let config = tempfile::tempdir().unwrap();

    mark_restart_to_update(config.path()).unwrap();

    assert!(take_restart_to_update(config.path()), "the next launch");
    assert!(
        !take_restart_to_update(config.path()),
        "the launch after it"
    );
}

#[test]
fn a_restart_that_was_told_what_to_put_back_says_so_once() {
    // #1078: the answer given before the restart is the next launch's, so it asks nothing.
    for choice in [Choice::ReopenAll, Choice::StartFresh] {
        let config = tempfile::tempdir().unwrap();
        mark_restart_to_update_choosing(config.path(), Some(choice)).unwrap();

        assert_eq!(
            take_restart(config.path()),
            Some(Restarted {
                chosen: Some(choice)
            })
        );
        assert_eq!(take_restart(config.path()), None, "the launch after it");
    }
}

#[test]
fn a_restart_marker_with_no_answer_or_one_this_build_cannot_read_still_asks() {
    let config = tempfile::tempdir().unwrap();
    // What a build before #1078 leaves: the file, and nothing in it.
    mark_restart_to_update(config.path()).unwrap();
    assert_eq!(
        take_restart(config.path()),
        Some(Restarted { chosen: None })
    );
    // A word a later build might write is no answer here: the question is asked.
    mark_restart_to_update(config.path()).unwrap();
    fs::write(
        crate::machine::dir(config.path()).join(RESTARTED_TO_UPDATE),
        "reopen-some\n",
    )
    .unwrap();
    assert_eq!(
        take_restart(config.path()),
        Some(Restarted { chosen: None })
    );
}

#[test]
fn a_launch_no_restart_came_before_takes_nothing() {
    let config = tempfile::tempdir().unwrap();

    assert!(!take_restart_to_update(config.path()));
}

#[cfg(unix)]
#[test]
fn the_restart_marker_is_not_written_through_a_link() {
    let config = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = crate::machine::dir(config.path());
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.path().join("planted"),
        dir.join(RESTARTED_TO_UPDATE),
    )
    .unwrap();

    assert!(mark_restart_to_update(config.path()).is_err());
    assert!(!elsewhere.path().join("planted").exists());
}

// ----- a grade's chat is drawn nowhere (SK-16, #2779) -------------------------------------

#[test]
fn a_grades_chat_has_no_tab_and_only_its_record_says_so() {
    let plane = tempfile::tempdir().unwrap();
    let record = Record {
        chats: vec![
            Chat {
                graded: true,
                ..claude("3", None)
            },
            claude("4", None),
        ],
        ..Default::default()
    };
    assert!(!record.chats[0].has_tab());
    assert!(record.chats[1].has_tab());
    write(plane.path(), &record).unwrap();

    let text = std::fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches(r#""graded""#).count(), 1, "{text}");
    let back = read(plane.path()).chats;
    assert!(back[0].graded);
    assert!(!back[1].graded);
}

#[test]
fn the_studio_flag_a_chat_started_with_is_recorded_and_read_back_3466() {
    // A chat reattached from the session host keeps the Skill studio flag its start read, so
    // the record carries it; a chat recorded without one writes no key.
    let plane = tempfile::tempdir().unwrap();
    let on = Chat {
        plane_skills: Some(true),
        ..claude("ide.7", None)
    };
    let off = Chat {
        plane_skills: Some(false),
        ..claude("ide.8", Some(ID))
    };
    let unknown = claude("ide.9", None);
    write(
        plane.path(),
        &Record {
            chats: vec![on.clone(), off.clone(), unknown],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"plane_skills\"").count(), 2, "{text}");
    let back = read(plane.path());
    assert_eq!(back.chats[0], on);
    assert_eq!(back.chats[1], off);
    assert_eq!(back.chats[2].plane_skills, None);
}

#[test]
fn a_record_written_before_the_studio_flag_reads_as_not_known_3466() {
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![claude("ide.7", Some(ID))],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert!(!text.contains("plane_skills"), "{text}");
    let back = read(plane.path());
    assert_eq!(back.chats.len(), 1, "the record still reads whole");
    assert_eq!(back.chats[0].plane_skills, None);

    // A value that is no flag reads as not known, never as a record that does not read.
    fs::write(
        path(plane.path()),
        text.replacen("\"name\"", "\"plane_skills\":\"yes\",\"name\"", 1),
    )
    .unwrap();
    let back = read(plane.path());
    assert_eq!(back.chats.len(), 1, "{back:?}");
    assert_eq!(back.chats[0].plane_skills, None);
}

/// A routing kept from a chat's start that asked the router `relevance`, which hid `skill-02`.
fn kept_routing() -> crate::skills::router::Kept {
    use crate::skills::router::{Kept, Record, RecordHidden, Routed};
    Kept::Asked(Routed {
        record: Record {
            routing: "01HZY3M6S4K9Q2W7E8R5T1Y6V0".to_owned(),
            router: "relevance".to_owned(),
            ms: 34,
            fell_back: None,
            error: None,
            hidden: vec![RecordHidden {
                skill: "skill-02".to_owned(),
                why: "not relevant here".to_owned(),
            }],
        },
        answer: Some(vec![(
            "skill-02".to_owned(),
            purlis_skills::Line::new("not relevant here").unwrap(),
        )]),
    })
}

#[test]
fn the_routing_a_chat_started_with_is_recorded_and_read_back_3578() {
    let plane = tempfile::tempdir().unwrap();
    let asked = Chat {
        routing: Some(kept_routing()),
        ..claude("ide.7", None)
    };
    let built_in = Chat {
        routing: Some(crate::skills::router::Kept::BuiltIn),
        ..claude("ide.8", Some(ID))
    };
    let unknown = claude("ide.9", None);
    write(
        plane.path(),
        &Record {
            chats: vec![asked.clone(), built_in.clone(), unknown],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert_eq!(text.matches("\"routing\": {").count(), 2, "{text}");
    let back = read(plane.path());
    assert_eq!(back.chats[0], asked);
    assert_eq!(back.chats[1], built_in);
    assert_eq!(back.chats[2].routing, None);
}

#[test]
fn a_routing_that_does_not_read_whole_reads_as_not_known_3578() {
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &Record {
            chats: vec![Chat {
                routing: Some(kept_routing()),
                ..claude("ide.7", Some(ID))
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(path(plane.path())).unwrap();
    // A record written before the routing was kept still reattaches, and asks again.
    let before = text.replacen("\"routing\":", "\"not_routing\":", 1);
    for spoiled in [
        before,
        text.replacen("not relevant here", "two\\nlines", 1),
        // The answer's own line alone, the last one the record holds.
        {
            let at = text.rfind("not relevant here").unwrap();
            format!(
                "{}two\\nlines{}",
                &text[..at],
                &text[at + "not relevant here".len()..]
            )
        },
        text.replacen("01HZY3M6S4K9Q2W7E8R5T1Y6V0", "not-a-ulid", 1),
        text.replacen("\"relevance\"", "\"../relevance\"", 1),
        text.replacen("\"by\": \"extension\"", "\"by\": \"someone\"", 1),
        // An answer beside a fallback, a router named by nothing, a skill no reader sees whole.
        text.replacen("\"fell_back\": null", "\"fell_back\": \"slow\"", 1),
        text.replacen("\"router\": \"relevance\"", "\"router\": \"\"", 1),
        text.replacen("\"skill\": \"skill-02\"", "\"skill\": \"skill\\u0007\"", 1),
    ] {
        fs::write(path(plane.path()), &spoiled).unwrap();
        let back = read(plane.path());
        assert_eq!(
            back.chats.len(),
            1,
            "the record still reads whole: {spoiled}"
        );
        assert_eq!(back.chats[0].routing, None, "{spoiled}");
    }
}

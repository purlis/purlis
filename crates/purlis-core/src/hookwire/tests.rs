// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
// Named here rather than at the top of the module: the module's own code needs neither
// of these off unix, and an import that is only right on one platform belongs with the
// code that is only compiled there.
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::mpsc;

#[test]
fn an_empty_harness_variable_reads_as_unset_to_every_reader_of_it() {
    // SK-36: each reader goes through `chat_harness`, so an empty `PURLIS_HARNESS` is a chat
    // that names none, to the hooks, the permission hook, the MCP server and the sandbox.
    let unset = |_: &str| None::<String>;
    let empty = |name: &str| (name == HARNESS_ENV).then(String::new);
    let sandboxed = |name: &str| match name {
        HARNESS_ENV => Some(String::new()),
        crate::hookwire::SANDBOXED_ENV => Some("1".to_owned()),
        _ => None,
    };
    let sandboxed_unset =
        |name: &str| (name == crate::hookwire::SANDBOXED_ENV).then(|| "1".to_owned());
    let read = |env: &dyn Fn(&str) -> Option<String>| {
        let harness = chat_harness(env);
        (
            harness.clone(),
            crate::harness::Harness::of_chat(harness.as_deref()),
            crate::harness::hooked::Source::of_harness(harness.as_deref()),
            crate::skills::route_of(harness.as_deref(), &Default::default()),
            crate::sandboxblock::the_proxy_tells_hosts(harness.as_deref()),
            sub_agent(Some("a1"), env),
        )
    };
    assert_eq!(read(&empty), read(&unset));
    assert_eq!(read(&empty).0, None);
    assert_eq!(
        read(&empty).1,
        Some(crate::harness::Harness::ClaudeCode),
        "a chat that names none is Claude Code's"
    );
    let by_the_harness = crate::sandbox::Started::ByTheHarness;
    assert_eq!(
        crate::sandbox::writes_are_sandboxed_in(&sandboxed, by_the_harness),
        crate::sandbox::writes_are_sandboxed_in(&sandboxed_unset, by_the_harness)
    );
    let codex = |name: &str| (name == HARNESS_ENV).then(|| "codex".to_owned());
    assert_eq!(chat_harness(&codex).as_deref(), Some("codex"));
}

#[test]
fn an_opened_answer_says_no_row_and_reads_an_older_app_s_row_as_nothing() {
    // #1457: a handoff writes no row in the dispatch log now, so the answer carries none.
    // #1445: and what the app has to say about the profile it started on.
    let said = Answer::Opened {
        chat: 9,
        note: Some("runs on the asking chat's profile".to_owned()),
    };
    let line = serde_json::to_string(&said).expect("written");
    assert_eq!(
        line,
        r#"{"opened":{"chat":9,"note":"runs on the asking chat's profile"}}"#
    );
    assert_eq!(serde_json::from_str::<Answer>(&line).expect("read"), said);
    // An older app wrote the row itself and said what became of it (#1421), or said
    // nothing of it: either still reads, as the chat it opened, and nothing more.
    for older in [
        r#"{"opened":{"chat":9}}"#,
        r#"{"opened":{"chat":9,"row":"written"}}"#,
        r#"{"opened":{"chat":9,"row":{"unwritten":{"why":"No space left on device"}}}}"#,
    ] {
        assert_eq!(
            serde_json::from_str::<Answer>(older).expect("parsed"),
            Answer::Opened {
                chat: 9,
                note: None,
            },
            "{older}"
        );
    }
}

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
    let pairs: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |want| {
        pairs
            .iter()
            .find(|(k, _)| k == want)
            .map(|(_, v)| v.clone())
    }
}

const CLAUDE_STOP: &str = r#"{"session_id":"11111111-2222-4333-8444-555555555555",
        "transcript_path":"/tmp/t.jsonl","cwd":"/tmp","hook_event_name":"Stop",
        "stop_hook_active":false,"last_assistant_message":"pong"}"#;

/// A Claude Code `UserPromptSubmit` payload whose prompt is `prompt`.
fn prompt_payload(prompt: &str) -> String {
    serde_json::json!({
        "session_id": "11111111-2222-4333-8444-555555555555",
        "hook_event_name": "UserPromptSubmit",
        "prompt": prompt,
    })
    .to_string()
}

#[test]
fn a_line_s_speaker_is_read_as_a_report_s_pid_and_conversation_are() {
    // #1601: what a tool line says of the run it came from is judged by the board as a
    // report is, so it is read the same way, including a contradiction and an unread pipe.
    const ID: &str = "11111111-2222-4333-8444-555555555555";
    const OTHER: &str = "22222222-3333-4444-8555-666666666666";
    let with_id = |id: &str| serde_json::json!({ "session_id": id }).to_string();
    let cases = [
        (
            vec![(CLAUDE_PID_ENV, "4242"), (CLAUDE_CONVERSATION_ENV, ID)],
            with_id(ID),
        ),
        (
            vec![(CLAUDE_PID_ENV, "4242"), (CLAUDE_CONVERSATION_ENV, ID)],
            with_id(OTHER),
        ),
        (
            vec![(CLAUDE_PID_ENV, "4242"), (CLAUDE_CONVERSATION_ENV, ID)],
            String::new(),
        ),
        (vec![(CLAUDE_PID_ENV, "0")], "{}".to_owned()),
        (vec![], with_id(ID)),
    ];
    let expected = [
        Speaker {
            pid: Some(4242),
            conversation: Conversation::Named(ID.to_owned()),
        },
        Speaker {
            pid: Some(4242),
            conversation: Conversation::Contradicted,
        },
        Speaker {
            pid: Some(4242),
            conversation: Conversation::Unknown,
        },
        Speaker {
            pid: None,
            conversation: Conversation::Foreign,
        },
        Speaker {
            pid: None,
            conversation: Conversation::Named(ID.to_owned()),
        },
    ];
    for ((pairs, payload), expected) in cases.into_iter().zip(expected) {
        let mut pairs = pairs;
        pairs.push((CHAT_ENV, "7"));
        let env = env_of(&pairs);
        let speaker = Speaker::read(&payload, &env);
        assert_eq!(speaker, expected, "{payload}");
        let report = Report::read(Event::Stop, &payload, &env).expect("one");
        assert_eq!(
            (report.pid, report.conversation),
            (speaker.pid, speaker.conversation)
        );
    }
}

#[test]
fn a_harness_running_with_its_prompts_off_is_one_bit_of_every_report() {
    // #1446: the app keeps it as the chat's mark. Only the mode the harness names for
    // "nobody answers" sets it, on whatever event carries it.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);
    let payload = |mode: Option<&str>| {
        let mut said = serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
        });
        if let Some(mode) = mode {
            said["permission_mode"] = mode.into();
        }
        said.to_string()
    };
    for event in [Event::SessionStart, Event::Stop, Event::UserPromptSubmit] {
        let off = Report::read(event, &payload(Some("bypassPermissions")), &env).expect("one");
        assert!(off.detail.unattended, "{event:?}");
        let written = serde_json::to_string(&off).expect("json");
        assert!(written.contains(r#""unattended":true"#), "{written}");
    }
    for mode in [None, Some("default"), Some("acceptEdits"), Some("plan")] {
        let asks = Report::read(Event::Stop, &payload(mode), &env).expect("one");
        assert!(!asks.detail.unattended, "{mode:?}");
        // And a report that does not say it writes the line it always wrote.
        assert!(!serde_json::to_string(&asks).unwrap().contains("unattended"));
    }
}

#[test]
fn the_mode_a_chat_s_own_harness_names_rides_every_report() {
    // #3787: what a task the chat dispatches starts from. Read by the chat's harness's
    // adapter alone: Codex names bypassPermissions for every chat, and is not read.
    use crate::dispatchmode::Reported;
    let payload = |mode: Option<&str>| {
        let mut said = serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
        });
        if let Some(mode) = mode {
            said["permission_mode"] = mode.into();
        }
        said.to_string()
    };
    let on = |harness: &str| {
        env_of(&[
            (SOCKET_ENV, "/tmp/s.sock"),
            (CHAT_ENV, "7"),
            (HARNESS_ENV, harness),
        ])
    };
    let claude = on("claude-code");
    for (mode, reported) in [
        (Some("auto"), Some(Reported::Classifier)),
        (Some("bypassPermissions"), Some(Reported::PromptsOff)),
        (Some("default"), Some(Reported::Asks)),
        (Some("dontAsk"), Some(Reported::Asks)),
        (None, None),
    ] {
        let report = Report::read(Event::UserPromptSubmit, &payload(mode), &claude).expect("one");
        assert_eq!(report.detail.mode, reported, "{mode:?}");
        let written = serde_json::to_string(&report).expect("json");
        let back: Report = serde_json::from_str(&written).expect("reads");
        assert_eq!(back.detail.mode, reported, "{written}");
    }
    assert!(
        !serde_json::to_string(&Report::read(Event::Stop, &payload(None), &claude).unwrap())
            .unwrap()
            .contains("\"mode\""),
        "a report that names no mode writes the line it always wrote"
    );
    for other in ["codex", "opencode"] {
        let report = Report::read(Event::Stop, &payload(Some("auto")), &on(other)).expect("one");
        assert_eq!(report.detail.mode, None, "{other}");
    }
    // And a chat whose harness is not named says nothing.
    let unnamed = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);
    let report = Report::read(Event::Stop, &payload(Some("auto")), &unnamed).expect("one");
    assert_eq!(report.detail.mode, None);
}

#[test]
fn a_session_start_names_its_model_and_nothing_else_does() {
    // #1021: what a commit's `Assisted-by` names. Claude Code 2.1.295's `SessionStart`
    // payload carries `model`; no other event's is read for it.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);
    let payload = |model: serde_json::Value| {
        serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "source": "startup",
            "model": model,
        })
        .to_string()
    };
    let named = Report::read(
        Event::SessionStart,
        &payload("claude-opus-4-1".into()),
        &env,
    )
    .expect("one");
    assert_eq!(
        named.detail.model.as_ref().map(crate::state::Model::as_str),
        Some("claude-opus-4-1")
    );
    let line = serde_json::to_string(&named).expect("json");
    assert!(line.contains(r#""model":"claude-opus-4-1""#), "{line}");
    assert_eq!(serde_json::from_str::<Report>(&line).expect("read"), named);

    let stop = Report::read(Event::Stop, &payload("claude-opus-4-1".into()), &env).expect("one");
    assert_eq!(stop.detail.model, None, "only a SessionStart names it");
    assert!(!serde_json::to_string(&stop).unwrap().contains("model"));

    // A name that cannot stand on one trailer line is no name, never one cut to fit.
    for odd in [
        serde_json::json!("two words"),
        serde_json::json!(""),
        serde_json::json!("x".repeat(crate::state::Model::MOST + 1)),
        serde_json::json!("mod\u{e8}le"),
        serde_json::json!(4),
    ] {
        let read = Report::read(Event::SessionStart, &payload(odd.clone()), &env).expect("one");
        assert_eq!(read.detail.model, None, "{odd}");
    }
    // And a line another build wrote with an odd value still reads, without it.
    let odd = line.replace("claude-opus-4-1", "two words");
    let read: Report = serde_json::from_str(&odd).expect("the rest of the line reads");
    assert_eq!((read.chat, read.detail.model), (7, None));
}

#[test]
fn opencode_s_own_name_for_smart_close_is_heard_in_an_opencode_chat_alone() {
    // opencode knows purlis's built-in skills as `purlis-<name>`; nowhere else is that name
    // purlis's, so a skill named so in another chat is its own.
    let heard = |harness: Option<&str>| {
        let mut pairs = vec![(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")];
        pairs.extend(harness.map(|harness| (HARNESS_ENV, harness)));
        let env = env_of(&pairs);
        Report::read(
            Event::UserPromptSubmit,
            &prompt_payload("/purlis-smart-close"),
            &env,
        )
        .expect("a report")
        .detail
        .smart_close
    };
    assert!(heard(Some("opencode")));
    assert!(!heard(Some("claude")));
    assert!(!heard(Some("codex")));
    assert!(!heard(None));
}

#[test]
fn a_typed_smart_close_is_one_bit_of_the_prompt_and_the_prompt_goes_nowhere() {
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);
    for typed in [
        "/smart-close",
        "/purlis:smart-close",
        "  /smart-close now please",
    ] {
        let report =
            Report::read(Event::UserPromptSubmit, &prompt_payload(typed), &env).expect("a report");
        assert!(
            report.detail.smart_close,
            "{typed:?} was not heard as smart close"
        );
        let line = serde_json::to_string(&report).expect("a line");
        assert!(!line.contains("please"), "the prompt left the hook: {line}");
    }
    for typed in [
        "please /smart-close",
        "/smart-closer",
        "/other:smart-close",
        "smart-close",
        "",
    ] {
        let report =
            Report::read(Event::UserPromptSubmit, &prompt_payload(typed), &env).expect("a report");
        assert!(
            !report.detail.smart_close,
            "{typed:?} was heard as smart close"
        );
    }
    // Only the event that carries a person's prompt says it.
    let stop = Report::read(Event::Stop, &prompt_payload("/smart-close"), &env).expect("one");
    assert!(!stop.detail.smart_close);
    // And a report that is not one says nothing new on the wire.
    let plain =
        Report::read(Event::UserPromptSubmit, &prompt_payload("hi"), &env).expect("a report");
    let line = serde_json::to_string(&plain).expect("a line");
    assert!(!line.contains("smart_close"), "{line}");
}

#[test]
fn a_brokered_write_reads_back_off_the_socket_as_the_write_it_is_from_the_chat_it_names() {
    // #1333: every write a command hands the app arrives whole, and is checked against the
    // token of the chat it names.
    for write in [
        crate::brokered::Write::PersonaRemember {
            text: "fact".to_owned(),
            title: Some("T".to_owned()),
            shared: true,
        },
        crate::brokered::Write::WorkspaceRemember {
            text: "memo".to_owned(),
            title: None,
        },
        crate::brokered::Write::Todo {
            text: "todo".to_owned(),
        },
        crate::brokered::Write::WorkspaceVision {
            text: "vision".to_owned(),
        },
        crate::brokered::Write::WorkspaceSection {
            section: crate::brokered::Section::Glossary,
            text: "term".to_owned(),
        },
    ] {
        let ask = Ask::Write(Box::new(WriteAsk { chat: 3, write }));
        let token = ChatToken("t".repeat(64));
        let line = line_with(Some(&token), &ask).expect("a line");
        let (read, carried) = read_line(std::str::from_utf8(&line).unwrap()).expect("a line");
        assert_eq!(read.chat(), 3);
        assert_eq!(carried.as_deref(), Some(token.0.as_str()));
        let Line::Ask(read) = read else {
            panic!("not read as an ask: {read:?}");
        };
        assert_eq!(read, ask);
    }
}

#[test]
fn a_removal_ask_is_read_as_one_from_the_chat_it_names() {
    // qw356: a name and the chat, and nothing else on the line.
    let ask = Ask::Removal(RemovalAsk {
        chat: 3,
        removing: Removing::Workspace {
            name: "alpha".to_owned(),
        },
    });
    let token = ChatToken("t".repeat(64));
    let line = line_with(Some(&token), &ask).expect("a line");
    let text = std::str::from_utf8(&line).unwrap();
    let (read, carried) = read_line(text).expect("a line");
    assert_eq!(read.chat(), 3);
    assert_eq!(carried.as_deref(), Some(token.0.as_str()));
    let Line::Ask(read) = read else {
        panic!("not read as an ask: {read:?}");
    };
    assert_eq!(read, ask);
    assert!(!text.contains("force"), "{text}");
}

#[test]
fn the_largest_write_a_chat_may_ask_for_fits_on_one_line_of_the_socket() {
    // Every byte a control character, which JSON spells in six: what `Write::check` lets
    // through at its worst is still read whole and answered, never cut (#1333).
    let most = crate::brokered::MOST_TEXT_BYTES;
    let write = crate::brokered::Write::PersonaRemember {
        text: "\u{1}".repeat(most),
        title: None,
        shared: true,
    };
    assert_eq!(write.check(), Ok(()));
    let ask = Ask::Write(Box::new(WriteAsk {
        chat: u32::MAX,
        write,
    }));
    let line = line_with(Some(&ChatToken("t".repeat(64))), &ask).expect("a line");
    assert!(
        (line.len() as u64) < A_LINE_IS_AT_MOST,
        "{} bytes does not fit in {A_LINE_IS_AT_MOST}",
        line.len()
    );
}

#[test]
fn the_largest_proposed_change_fits_on_one_line_of_the_socket() {
    // #1975: a proposal at its largest, every byte a control character JSON spells in six,
    // with the longest path and a base, is read whole: never cut. A whole content carries
    // as many bytes as edits do, and less of the change's own words.
    let ask = Ask::Propose(Box::new(ProposeAsk {
        chat: u32::MAX,
        path: "\u{1}".repeat(crate::proposed::MOST_PATH_BYTES),
        base: Some("f".repeat(64)),
        change: crate::proposed::Change::Edits {
            edits: vec![crate::proposed::Edit {
                old: "\u{1}".repeat(crate::proposed::MOST_BYTES / 2),
                new: "\u{1}".repeat(crate::proposed::MOST_BYTES / 2),
                every: true,
            }],
        },
    }));
    let line = line_with(Some(&ChatToken("t".repeat(64))), &ask).expect("a line");
    assert!(
        (line.len() as u64) < A_LINE_IS_AT_MOST,
        "{} bytes does not fit in {A_LINE_IS_AT_MOST}",
        line.len()
    );
}

#[test]
fn a_written_answer_reads_back_as_written() {
    let answer = Answer::Written {
        to: "alpha".to_owned(),
        path: "workspaces/alpha/todos/x.md".to_owned(),
    };
    let text = serde_json::to_string(&answer).expect("json");
    assert_eq!(
        serde_json::from_str::<Answer>(&text).expect("an answer"),
        answer
    );
}

#[test]
fn the_largest_record_a_chat_may_write_fits_on_one_line_of_the_socket() {
    // Every byte of the body one JSON doubles, and the longest title: what `check` lets
    // through at its worst still reaches the app whole (#1332).
    let ask = Ask::SessionRecord(Box::new(RecordAsk {
        chat: u32::MAX,
        title: "\"".repeat(crate::sessionrecord::MOST_TITLE_CHARS),
        body: "\"".repeat(crate::sessionrecord::MOST_BODY_BYTES),
        pieces: vec!["repo/piece".to_owned(); 8],
        cwd: Some(std::path::PathBuf::from("/a/long/enough/path/to/a/piece")),
    }));
    let token = ChatToken("t".repeat(64));
    let line = line_with(Some(&token), &ask).expect("a line");
    assert!(
        (line.len() as u64) < A_LINE_IS_AT_MOST,
        "{} bytes does not fit in {A_LINE_IS_AT_MOST}",
        line.len()
    );
}

#[test]
fn a_git_ask_is_read_back_whole_and_names_the_chat_its_token_must_be() {
    // #1335: an ask for a clone and one for a worktree, through the reading every line gets.
    for work in [
        GitWork::Clone {
            repos: vec!["widget".to_owned(), "gadget".to_owned()],
        },
        GitWork::WorktreeAdd {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            branch: Some("feature/p1".to_owned()),
        },
        GitWork::Create {
            vision: Some("Ship the importer".to_owned()),
            repos: vec!["widget".to_owned()],
        },
        GitWork::WorktreeCut {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            branch: None,
            base: Some("main".to_owned()),
        },
        GitWork::WorktreePush {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            lease: true,
        },
        GitWork::WorktreeUpdate {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            step: UpdateStep::Start {
                base: Some("main".to_owned()),
            },
        },
        GitWork::WorktreeUpdate {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            step: UpdateStep::Continue,
        },
        GitWork::CloneUpdate {
            repo: "widget".to_owned(),
            step: UpdateStep::Abort,
        },
        GitWork::RootUpdate {},
        GitWork::WorktreeRemove {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            delete_branch: true,
        },
        GitWork::WorktreeClaim {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            act: ClaimAct::Push,
        },
    ] {
        let ask = Ask::Git(Box::new(GitAsk {
            chat: 7,
            workspace: "alpha".to_owned(),
            alive: false,
            work,
        }));
        let token = ChatToken("t".repeat(64));
        let line = line_with(Some(&token), &ask).expect("a line");
        let (read, carried) =
            read_line(std::str::from_utf8(&line).expect("text")).expect("it reads");
        assert_eq!(carried.as_deref(), Some(token.expose()));
        assert_eq!(read.chat(), 7);
        let Line::Ask(back) = read else {
            panic!("a git ask read as another kind of line");
        };
        assert_eq!(back, ask);
    }
}

#[test]
fn a_commit_ask_names_the_message_and_what_to_stage_and_nothing_else() {
    // #1055: the whole ask. Where it commits, and on which branch, is the app's record.
    let ask = Ask::Commit(Box::new(CommitAsk {
        chat: 7,
        message: "one\n\ntwo".to_owned(),
        stage: Stage::Paths(vec!["src/a.rs".to_owned(), "-n".to_owned()]),
        folder: None,
    }));
    let line = line_with(None, &ask).expect("a line");
    let text = std::str::from_utf8(&line).expect("text");
    assert_eq!(
        text,
        "{\"commit\":{\"chat\":7,\"message\":\"one\\n\\ntwo\",\
             \"stage\":{\"paths\":[\"src/a.rs\",\"-n\"]}}}\n"
    );
    let (read, _) = read_line(text).expect("it reads");
    assert_eq!(read.chat(), 7);
    let Line::Ask(back) = read else {
        panic!("the ask read as another kind of line");
    };
    assert_eq!(back, ask);
    let tracked = "{\"commit\":{\"chat\":7,\"message\":\"m\",\"stage\":\"tracked\"}}";
    let (read, _) = read_line(tracked).expect("it reads");
    assert!(matches!(
        read,
        Line::Ask(Ask::Commit(commit)) if commit.stage == Stage::Tracked
    ));
    // A line that names anything else is not a commit ask: no folder, repository, branch,
    // git directory, author, date, or option such as amend.
    for extra in [
        "cwd",
        "folder",
        "repo",
        "workspace",
        "branch",
        "git_dir",
        "author",
        "date",
        "amend",
        "options",
    ] {
        let line = format!(
            "{{\"commit\":{{\"chat\":7,\"message\":\"m\",\"stage\":\"tracked\",\"{extra}\":\"x\"}}}}"
        );
        assert!(
            !matches!(read_line(&line), Some((Line::Ask(Ask::Commit(_)), _))),
            "{extra} was read"
        );
    }
    // And no other word for what to stage.
    for stage in [
        "\"all\"",
        "{\"amend\":true}",
        "{\"reset\":[]}",
        "{\"push\":[]}",
    ] {
        let line = format!("{{\"commit\":{{\"chat\":7,\"message\":\"m\",\"stage\":{stage}}}}}");
        assert!(
            !matches!(read_line(&line), Some((Line::Ask(Ask::Commit(_)), _))),
            "{stage} was read"
        );
    }
}

#[test]
fn asking_where_a_chat_works_names_only_the_chat_its_token_must_be() {
    // #1450: the whole ask is the chat's number and why it asks.
    let ask = Ask::WhereWorking(WhereWorking {
        chat: 7,
        tell: crate::awareness::Tell::Turn,
    });
    let line = line_with(None, &ask).expect("a line");
    let text = std::str::from_utf8(&line).expect("text");
    assert_eq!(text, "{\"where_working\":{\"chat\":7,\"tell\":\"turn\"}}\n");
    let (read, _) = read_line(text).expect("it reads");
    assert_eq!(read.chat(), 7);
    let Line::Ask(back) = read else {
        panic!("the ask read as another kind of line");
    };
    assert_eq!(back, ask);
    // And one that says no more than its chat is the command's.
    let (bare, _) = read_line("{\"where_working\":{\"chat\":7}}").expect("it reads");
    let Line::Ask(Ask::WhereWorking(bare)) = bare else {
        panic!("the ask read as another kind of line");
    };
    assert_eq!(bare.tell, crate::awareness::Tell::Asked);
}

#[cfg(unix)]
#[test]
fn asking_where_a_chat_works_under_a_forged_sender_is_refused_and_never_reaches_the_app() {
    // #1450, D-1407-6/10: chat 3's picture is told only to a line carrying chat 3's token.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let three = listener.tokens().issue_to_this_process(3).expect("a token");
    let four = listener.tokens().issue_to_this_process(4).expect("a token");
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Answer::No {
                why: "the app was asked".to_owned(),
            }
        }),
    );
    let within = std::time::Duration::from_secs(5);
    let ask = Ask::WhereWorking(WhereWorking {
        chat: 3,
        tell: crate::awareness::Tell::Asked,
    });

    for token in [None, Some(ChatToken::from("nope")), Some(four)] {
        let answered = Asking::on(&path, token)
            .expect("connected")
            .ask(&ask, within);
        assert!(
            matches!(&answered, Ok(Answer::No { why }) if why.contains("token")),
            "answered {answered:?}"
        );
    }
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);

    let answered = Asking::on(&path, Some(three))
        .expect("connected")
        .ask(&ask, within);
    assert!(
        matches!(&answered, Ok(Answer::No { why }) if why == "the app was asked"),
        "{answered:?}"
    );
}

// ----- a dispatch (#1436) ----------------------------------------------------------------

fn a_dispatch(chat: u32) -> Ask {
    Ask::Dispatch(Box::new(DispatchAsk {
        chat,
        to: Some("devops".to_owned()),
        name: "check the queue".to_owned(),
        brief: "# Check the queue\nbody\n".to_owned(),
        profile: None,
        place: None,
        ticket: "t".repeat(64),
        long: false,
    }))
}

#[test]
fn a_dispatch_is_read_back_whole_and_names_the_chat_its_token_must_be() {
    let ask = a_dispatch(7);
    let token = ChatToken("t".repeat(64));
    let line = line_with(Some(&token), &ask).expect("a line");
    let (read, carried) = read_line(std::str::from_utf8(&line).expect("text")).expect("it reads");
    assert_eq!(carried.as_deref(), Some(token.expose()));
    assert_eq!(read.chat(), 7, "sender binding checks this chat's token");
    let Line::Ask(back) = read else {
        panic!("a dispatch read as another kind of line");
    };
    assert_eq!(back, ask);
}

#[test]
fn a_dispatch_has_no_field_that_says_who_asks_but_the_chats_number() {
    // The asker's persona, folder, lineage and grants are the app's record of chat 7. A
    // line that claims them anyway is read without them: there is nowhere to keep them.
    // The one thing more it may say is a profile's name, which the app only ever looks up
    // among the profiles the project offers here.
    let written = serde_json::to_value(a_dispatch(7)).expect("json");
    let mut keys: Vec<&str> = written["dispatch"]
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["brief", "chat", "name", "ticket", "to"]);

    let forged = concat!(
        r#"{"dispatch":{"chat":7,"name":"x","brief":"a b","ticket":"t","#,
        r#""from":"devops","persona":"devops","asker":3,"profile":"prod","cwd":"/","#,
        r#""grant":"in_force","mode":"handoff","depth":0,"without_sandbox":true,"#,
        r#""root":"01J9ZQ3V5N8X4T2K7M6P0R1S2A","by":"person","attended":true,"#,
        r#""permission_mode":"bypassPermissions","grants":{"hosts":["evil.example"]}}}"#
    );
    let (read, _) = read_line(forged).expect("it reads");
    assert_eq!(read.chat(), 7);
    let Line::Ask(Ask::Dispatch(read)) = read else {
        panic!("a dispatch");
    };
    assert_eq!(
        *read,
        DispatchAsk {
            chat: 7,
            to: None,
            name: "x".to_owned(),
            brief: "a b".to_owned(),
            profile: Some("prod".to_owned()),
            place: None,
            ticket: "t".to_owned(),
            long: false,
        }
    );
}

#[test]
fn a_dispatch_says_where_in_one_word_and_has_no_field_for_a_folder_or_a_branch() {
    // #1453: `in` is the only thing a line says about where the new chat works. A line
    // that names a folder, a repo, a piece or a branch anyway is read without them.
    let forged = concat!(
        r#"{"dispatch":{"chat":7,"name":"x","brief":"a b","ticket":"t","in":"worktree","#,
        r#""branch":"main","piece":"../../main","folder":"/etc","repo":"other","#,
        r#""worktree":{"branch":"main","path":"/"},"workspace":"beta","base":"main"}}"#
    );
    let (read, _) = read_line(forged).expect("it reads");
    let Line::Ask(Ask::Dispatch(read)) = read else {
        panic!("a dispatch");
    };
    assert_eq!(
        *read,
        DispatchAsk {
            chat: 7,
            to: None,
            name: "x".to_owned(),
            brief: "a b".to_owned(),
            profile: None,
            place: Some("worktree".to_owned()),
            ticket: "t".to_owned(),
            long: false,
        }
    );
    // And it is written back as that one word.
    let written = serde_json::to_value(Ask::Dispatch(read)).expect("json");
    assert_eq!(written["dispatch"]["in"], "worktree");
    assert_eq!(written["dispatch"].get("place"), None);
}

#[cfg(unix)]
#[test]
fn a_dispatch_that_names_another_chat_as_its_asker_is_refused_and_never_reaches_the_app() {
    // A forged line: chat 4's own token, naming chat 3 as the asker. And no token at all.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let three = listener.tokens().issue_to_this_process(3).expect("a token");
    let four = listener.tokens().issue_to_this_process(4).expect("a token");
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let heard = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            heard.lock().unwrap().push(ask);
            Answer::Dispatched {
                chat: 9,
                name: "check the queue".to_owned(),
                persona: None,
                note: None,
                works: None,
            }
        }),
    );
    let within = std::time::Duration::from_secs(5);

    for token in [None, Some(four)] {
        let answered = Asking::on(&path, token)
            .expect("connected")
            .ask(&a_dispatch(3), within);
        assert!(
            matches!(&answered, Ok(Answer::No { why }) if why.contains("token")),
            "answered {answered:?}"
        );
    }
    assert!(asked.lock().unwrap().is_empty(), "the app was never asked");

    // The chat's own line is the one the app hears.
    let answered = Asking::on(&path, Some(three))
        .expect("connected")
        .ask(&a_dispatch(3), within);
    assert!(
        matches!(answered, Ok(Answer::Dispatched { chat: 9, .. })),
        "{answered:?}"
    );
    assert_eq!(asked.lock().unwrap().clone(), vec![a_dispatch(3)]);
}

#[cfg(unix)]
#[test]
fn a_dispatch_with_its_chats_token_from_outside_the_chat_is_refused_saying_why() {
    // D-1407-6/10: the token alone is not enough. A process that is not inside the chat's
    // own program is told so, and the app is never asked.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let (token, mut elsewhere) = a_chat_elsewhere(&listener.tokens(), 5);
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Answer::Dispatched {
                chat: 9,
                name: "x".to_owned(),
                persona: None,
                note: None,
                works: None,
            }
        }),
    );

    let answered = Asking::on(&path, Some(token))
        .expect("connected")
        .ask(&a_dispatch(5), std::time::Duration::from_secs(5));

    let _ = elsewhere.kill();
    let _ = elsewhere.wait();
    assert_eq!(
        answered.ok(),
        Some(Answer::No {
            why: OUTSIDE_THE_CHAT.to_owned()
        })
    );
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn a_report_back_is_written_as_it_always_was_and_a_task_s_adds_its_outcome() {
    let plain = Ask::Report(Box::new(ReportBack {
        chat: 7,
        summary: "done".to_owned(),
        ticket: "t".to_owned(),
        task: None,
    }));
    let as_before = r#"{"report":{"chat":7,"summary":"done","ticket":"t"}}"#;
    assert_eq!(serde_json::to_string(&plain).unwrap(), as_before);
    // And a line from a purlis older than tasks reads as a report with no task part.
    let (read, _) = read_line(as_before).expect("reads");
    assert!(matches!(read, Line::Ask(back) if back == plain));

    let task = Ask::Report(Box::new(ReportBack {
        chat: 7,
        summary: "done".to_owned(),
        ticket: "t".to_owned(),
        task: Some(TaskReport {
            outcome: crate::handback::Outcome::Blocked,
            changed: Some("svc: 2 files".to_owned()),
        }),
    }));
    let line = line_with(None, &task).expect("a line");
    let (read, _) = read_line(std::str::from_utf8(&line).unwrap()).expect("reads");
    assert!(matches!(read, Line::Ask(back) if back == task));
}

#[cfg(unix)]
#[test]
fn an_answer_that_takes_longer_than_a_report_may_still_reaches_the_asker() {
    // A brokered clone runs for as long as git's network limit (#1335): the listener's own
    // deadline on the connection is for reading a line, and must not cut the answer off.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let slow = A_REPORT_TAKES_AT_MOST + std::time::Duration::from_secs(1);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            std::thread::sleep(slow);
            Answer::Said {
                lines: vec![crate::repocmd::Say::Done("cloned".to_owned().into())],
                code: 0,
            }
        }),
    );
    let answer = Asking::on(&path, Some(token))
        .expect("connected")
        .ask(
            &Ask::Git(Box::new(GitAsk {
                chat: 3,
                workspace: "alpha".to_owned(),
                work: GitWork::Clone {
                    repos: vec!["widget".to_owned()],
                },
                alive: false,
            })),
            slow * 4,
        )
        .expect("answered");
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{answer:?}");
}

/// A git ask whose asker reads [`Answer::Alive`], answered after `takes`, beating the pulse its
/// thread holds every 20 ms where `beats`.
#[cfg(unix)]
fn asked_alive(
    alive: bool,
    takes: std::time::Duration,
    beats: bool,
    within: std::time::Duration,
) -> std::io::Result<Answer> {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            let begun = std::time::Instant::now();
            while begun.elapsed() < takes {
                std::thread::sleep(std::time::Duration::from_millis(20));
                if beats {
                    alive::beat();
                }
            }
            Answer::Said {
                lines: vec![crate::repocmd::Say::Done("cloned".to_owned().into())],
                code: 0,
            }
        }),
    );
    Asking::on(&path, Some(token)).expect("connected").ask(
        &Ask::Git(Box::new(GitAsk {
            chat: 3,
            workspace: "alpha".to_owned(),
            work: GitWork::Clone {
                repos: vec!["widget".to_owned()],
            },
            alive,
        })),
        within,
    )
}

#[cfg(unix)]
#[test]
fn a_clone_that_keeps_showing_life_is_waited_for_past_the_askers_bound() {
    // #4080: the bound is on a silence, not on the whole clone.
    let within = std::time::Duration::from_millis(400);
    let answer = asked_alive(true, within * 4, true, within).expect("answered");
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{answer:?}");
}

#[cfg(unix)]
#[test]
fn a_clone_that_shows_no_life_is_given_up_on_at_the_askers_bound() {
    let within = std::time::Duration::from_millis(400);
    let gave_up = asked_alive(true, within * 4, false, within).expect_err("gave up");
    assert!(
        matches!(
            gave_up.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ),
        "{gave_up:?}"
    );
}

#[cfg(unix)]
#[test]
fn an_asker_that_does_not_read_alive_lines_is_sent_only_its_answer() {
    // An older command reads its first line as its answer.
    use std::io::{BufRead, Write};
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(20));
                alive::beat();
            }
            Answer::Said {
                lines: vec![],
                code: 0,
            }
        }),
    );
    let ask = GitAsk {
        chat: 3,
        workspace: "alpha".to_owned(),
        work: GitWork::Clone {
            repos: vec!["widget".to_owned()],
        },
        alive: false,
    };
    assert!(!serde_json::to_string(&ask).unwrap().contains("alive"));
    let mut stream = std::os::unix::net::UnixStream::connect(&path).expect("connected");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let line = line_with(Some(&token), &Ask::Git(Box::new(ask))).expect("a line");
    stream.write_all(&line).unwrap();
    let mut first = String::new();
    std::io::BufReader::new(stream)
        .read_line(&mut first)
        .unwrap();
    assert!(
        matches!(
            serde_json::from_str(&first),
            Ok(Answer::Said { code: 0, .. })
        ),
        "{first}"
    );
}

#[test]
fn the_largest_git_ask_a_command_line_makes_fits_on_one_line_of_the_socket() {
    // A clone of 256 repos with long names, every byte one JSON doubles: what a person
    // would type at its worst still reaches the app whole (#1335).
    let ask = Ask::Git(Box::new(GitAsk {
        chat: u32::MAX,
        workspace: "w".repeat(100),
        alive: false,
        work: GitWork::Clone {
            repos: vec!["\"".repeat(100); 256],
        },
    }));
    let line = line_with(Some(&ChatToken("t".repeat(64))), &ask).expect("a line");
    assert!(
        (line.len() as u64) < A_LINE_IS_AT_MOST,
        "{} bytes does not fit in {A_LINE_IS_AT_MOST}",
        line.len()
    );
}

#[test]
fn what_a_brokered_git_action_said_is_read_back_line_for_line() {
    use crate::repocmd::Say;
    let said = Answer::Said {
        lines: vec![
            Say::Info("workspace: alpha  (via --workspace)".to_owned().into()),
            Say::Done("widget → workspaces/alpha/widget".to_owned().into()),
            Say::Warn("w".to_owned().into()),
            Say::Fail("f".to_owned().into()),
            Say::Plain("p".to_owned().into()),
            Say::Out("o".to_owned()),
        ],
        code: 2,
    };
    let line = serde_json::to_string(&said).expect("a line");
    assert_eq!(
        serde_json::from_str::<Answer>(&line).expect("it reads"),
        said
    );
}

#[test]
fn only_a_stop_naming_an_agent_in_flight_says_its_helpers_are_at_work() {
    // #1626: Claude Code's `background_tasks`, as its 2.1.296 hook input schema shapes it.
    let env = env_of(&[
        (SOCKET_ENV, "/tmp/s.sock"),
        (CHAT_ENV, "7"),
        (HARNESS_ENV, "claude-code"),
    ]);
    let with = |tasks: serde_json::Value| {
        serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "background_tasks": tasks,
        })
        .to_string()
    };
    let task = |kind: &str, status: &str| {
        let mut task = serde_json::json!({"id": "t1", "description": "d"});
        task["type"] = kind.into();
        task["status"] = status.into();
        task
    };
    let at_work = |event: Event, payload: &str| {
        Report::read(event, payload, &env)
            .expect("a report")
            .detail
            .helpers_at_work
    };
    assert!(at_work(
        Event::Stop,
        &with(serde_json::json!([task("subagent", "running")]))
    ));
    assert!(at_work(
        Event::Stop,
        &with(serde_json::json!([task("workflow", "pending")]))
    ));
    // On no other event: only a `Stop` of the chat's own ends its turn.
    let one_agent = with(serde_json::json!([task("subagent", "running")]));
    assert!(!at_work(Event::Notification, &one_agent));
    assert!(!at_work(Event::SubagentStop, &one_agent));
    for not_one in [
        with(serde_json::json!([])),
        with(serde_json::json!([
            task("shell", "running"),
            task("monitor", "running")
        ])),
        with(serde_json::json!([task("subagent", "completed")])),
        with(serde_json::json!([task("subagent", "a word nobody wrote")])),
        with(serde_json::json!([{"type": 7}])),
        with(serde_json::json!("subagent")),
        CLAUDE_STOP.to_owned(),
        "not json".to_owned(),
    ] {
        assert!(
            !at_work(Event::Stop, &not_one),
            "{not_one} said helpers at work"
        );
    }
    // Read on Claude Code only, where it was read from: another harness's is not its.
    for harness in [None, Some("codex"), Some("opencode")] {
        let mut pairs = vec![(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")];
        pairs.extend(harness.map(|harness| (HARNESS_ENV, harness)));
        let report = Report::read(Event::Stop, &one_agent, &env_of(&pairs)).expect("a report");
        assert!(!report.detail.helpers_at_work, "read on {harness:?}");
    }
}

#[test]
fn only_the_nudge_a_harness_says_is_one_is_read_as_a_chat_sitting_idle() {
    // #1626: Claude Code's `notification_type`; a permission or a question is an ask.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);
    let idle = |event: Event, kind: Option<&str>| {
        let mut payload = serde_json::json!({"session_id": "11111111-2222-4333-8444-555555555555"});
        if let Some(kind) = kind {
            payload["notification_type"] = kind.into();
        }
        Report::read(event, &payload.to_string(), &env)
            .expect("a report")
            .detail
            .idle
    };
    assert!(idle(Event::Notification, Some("idle_prompt")));
    assert!(!idle(Event::Stop, Some("idle_prompt")));
    for kind in [Some("permission_prompt"), Some("elicitation_dialog"), None] {
        assert!(!idle(Event::Notification, kind), "{kind:?} read as idle");
    }
}

#[test]
fn a_notification_says_which_prompt_its_terminal_shows_or_that_it_only_informs() {
    // #1691: Claude Code's `notification_type`, as its hooks reference lists them for
    // 2.1.296. A word this does not know reads as a prompt of no kind: shown, not hidden.
    use crate::harness::model::Prompt;
    use crate::state::Notified;
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);
    let notified = |event: Event, kind: Option<&str>| {
        let mut payload = serde_json::json!({"session_id": "11111111-2222-4333-8444-555555555555"});
        if let Some(kind) = kind {
            payload["notification_type"] = kind.into();
        }
        Report::read(event, &payload.to_string(), &env)
            .expect("a report")
            .detail
            .notified
    };
    let n = Event::Notification;
    for (kind, prompt) in [
        ("permission_prompt", Prompt::Permission),
        ("elicitation_dialog", Prompt::Form),
        ("elicitation_url_dialog", Prompt::Link),
        ("agent_needs_input", Prompt::Question),
        ("question", Prompt::Question),
        ("quota_auto_resume_stale", Prompt::Continue),
    ] {
        assert_eq!(notified(n, Some(kind)), Notified::Asks(prompt), "{kind}");
    }
    for kind in ["elicitation_complete", "elicitation_response"] {
        assert_eq!(notified(n, Some(kind)), Notified::Answered, "{kind}");
    }
    for kind in [
        "auth_success",
        "agent_completed",
        "quota_auto_resume_fired",
        "quota_auto_resume_disabled",
    ] {
        assert_eq!(notified(n, Some(kind)), Notified::Informs, "{kind}");
    }
    assert_eq!(notified(n, None), Notified::Unsaid);
    assert_eq!(notified(n, Some("a_later_kind")), Notified::Unsaid);
    assert_eq!(notified(n, Some("idle_prompt")), Notified::Unsaid);
    assert_eq!(
        notified(Event::Stop, Some("permission_prompt")),
        Notified::Unsaid
    );
}

#[test]
fn a_hook_reports_the_chat_its_environment_names() {
    // Measured on claude 2.1.276: the payload carries `session_id`, and it is the same
    // value as `$CLAUDE_CODE_SESSION_ID` (ADR 0024, C7).
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);

    let report = Report::read(Event::Stop, CLAUDE_STOP, &env).expect("a report");

    assert_eq!(report.chat, 7);
    assert_eq!(report.event, Event::Stop);
    assert_eq!(
        report.conversation,
        Conversation::Named("11111111-2222-4333-8444-555555555555".to_owned())
    );
}

#[test]
fn a_hook_outside_a_chat_the_app_started_reports_nothing() {
    // No `CHARTER_CHAT` means this harness was not started by the app — the operator's
    // own `claude` in a terminal, with the plugin's hooks pointed here. There is no chat
    // to move, and inventing one would move somebody else's.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock")]);

    assert_eq!(Report::read(Event::Stop, CLAUDE_STOP, &env), None);
}

#[test]
fn a_chat_number_that_is_not_one_reports_nothing() {
    for bad in ["", "seven", "-1", "4294967296", " 7"] {
        let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, bad)]);
        assert_eq!(
            Report::read(Event::Stop, CLAUDE_STOP, &env),
            None,
            "{bad:?} was taken as a chat"
        );
    }
}

#[test]
fn a_payload_that_will_not_parse_still_reports_the_event() {
    // The event came from argv, which charter wrote into the harness's own settings. A
    // truncated or empty payload costs the conversation and nothing else — losing the
    // EVENT would leave a chat marked as working forever.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);

    for payload in ["", "{", "not json at all"] {
        let report = Report::read(Event::Stop, payload, &env)
            .unwrap_or_else(|| panic!("{payload:?} reported nothing"));
        assert_eq!(report.chat, 7);
        assert_eq!(report.event, Event::Stop);
        assert_eq!(report.conversation, Conversation::Unknown);
    }
}

#[test]
fn a_payload_that_parses_but_names_no_conversation_is_a_dialect_charter_cannot_read() {
    // Different from a payload charter could not read at ALL, and the difference is what
    // keeps a nested harness out: opencode names its conversation under `sessionID`
    // (ADR 0024, O1), inherits `CLAUDE_PID` and sets neither variable of its own. A
    // review reproduced one marking the outer Claude chat as waiting, mid-turn.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);

    for payload in [
        "null",
        "[]",
        r#"{"session_id":42}"#,
        r#"{"sessionID":"11111111-2222-4333-8444-555555555555"}"#,
    ] {
        let report = Report::read(Event::Stop, payload, &env)
            .unwrap_or_else(|| panic!("{payload:?} reported nothing"));
        assert_eq!(
            report.conversation,
            Conversation::Foreign,
            "{payload:?} was read as purlis's own dialect"
        );
    }
}

#[test]
fn a_conversation_counts_only_when_the_payload_and_the_environment_agree() {
    // ADR 0024, C7: every Claude Code hook's `CLAUDE_CODE_SESSION_ID` equals its
    // payload's `session_id`. The Python charter requires the two to be equal and so
    // does this.
    let env = env_of(&[
        (SOCKET_ENV, "/tmp/s.sock"),
        (CHAT_ENV, "7"),
        (
            CLAUDE_CONVERSATION_ENV,
            "11111111-2222-4333-8444-555555555555",
        ),
        (CLAUDE_PID_ENV, "4242"),
    ]);

    let report = Report::read(Event::Stop, CLAUDE_STOP, &env).expect("a report");

    assert_eq!(
        report.conversation,
        Conversation::Named("11111111-2222-4333-8444-555555555555".to_owned())
    );
    assert_eq!(report.pid, Some(4242));
}

#[test]
fn a_lost_payload_names_no_conversation_rather_than_the_one_in_the_environment() {
    // **This is a defect an independent review found, kept as a test so it cannot come
    // back.** The environment holds the OUTER chat's id, because a harness nested in the
    // chat's shell inherits it. Falling back to it when a payload is slow does not lose
    // the id — it substitutes the wrong one, and the nested harness's report is then
    // taken as the outer chat's. Exactly what ADR 0024's C5 exists to refuse.
    let env = env_of(&[
        (SOCKET_ENV, "/tmp/s.sock"),
        (CHAT_ENV, "7"),
        (CLAUDE_CONVERSATION_ENV, "the-outer-chat"),
    ]);

    let report = Report::read(Event::Stop, "", &env).expect("a report");

    assert_eq!(
        report.conversation,
        Conversation::Unknown,
        "the outer chat's id was borrowed"
    );
}

#[test]
fn a_payload_that_disagrees_with_the_environment_names_no_conversation() {
    // A nested harness whose payload charter can read: it says its own id, and the
    // environment says the outer one. Neither is this chat's report.
    let env = env_of(&[
        (SOCKET_ENV, "/tmp/s.sock"),
        (CHAT_ENV, "7"),
        (CLAUDE_CONVERSATION_ENV, "the-outer-chat"),
    ]);

    let report = Report::read(Event::Stop, CLAUDE_STOP, &env).expect("a report");

    // Contradicted, not merely unknown: charter read both and they disagree. That is
    // different evidence from "I could not tell", and the board treats them differently.
    assert_eq!(report.conversation, Conversation::Contradicted);
}

#[test]
fn a_conversation_that_merely_starts_the_same_way_is_a_different_conversation() {
    // A surviving mutant: `said == here` weakened to `said.starts_with(&here)` passed
    // every test, because nothing compared two ids that were nearly the same. This one
    // line is what the whole check against a nested harness rests on.
    let env = env_of(&[
        (SOCKET_ENV, "/tmp/s.sock"),
        (CHAT_ENV, "7"),
        (
            CLAUDE_CONVERSATION_ENV,
            "11111111-2222-4333-8444-555555555555",
        ),
    ]);
    let longer = r#"{"session_id":"11111111-2222-4333-8444-555555555555-nested"}"#;

    assert_eq!(
        Report::read(Event::Stop, longer, &env)
            .expect("a report")
            .conversation,
        Conversation::Contradicted
    );
}

#[test]
fn a_harness_that_names_no_conversation_in_its_environment_is_taken_at_its_payload() {
    // Codex sets no `CLAUDE_CODE_SESSION_ID`, so its payload is contradicted by nothing.
    let env = env_of(&[(SOCKET_ENV, "/tmp/s.sock"), (CHAT_ENV, "7")]);

    let report = Report::read(Event::Stop, CLAUDE_STOP, &env).expect("a report");

    assert_eq!(
        report.conversation,
        Conversation::Named("11111111-2222-4333-8444-555555555555".to_owned())
    );
    assert_eq!(report.pid, None);
}

#[test]
fn the_variables_a_session_must_not_inherit_are_the_ones_that_name_a_harness() {
    // Whatever the app inherited, a chat must start without these: they say which
    // conversation and which process a hook belongs to, and only the harness the app
    // starts may answer that.
    assert!(NOT_INHERITED.contains(&CLAUDE_CONVERSATION_ENV));
    assert!(NOT_INHERITED.contains(&CLAUDE_PID_ENV));
    // Nor where the chat that launched the app was pinned (SI-1).
    assert!(NOT_INHERITED.contains(&crate::active::WORKSPACE_ENV));
    assert!(NOT_INHERITED.contains(&crate::active::PLANE_ROOT_ENV));
    // Nor the launcher's own chat token: a chat is given its own at the `exec`.
    assert!(NOT_INHERITED.contains(&TOKEN_ENV));
    assert!(NOT_INHERITED.contains(&SANDBOXED_ENV));
    assert!(NOT_INHERITED.contains(&crate::sandboxblock::CHAT_DIR_ENV));
    assert!(NOT_INHERITED.contains(&crate::noterminal::RELAUNCHED_ENV));
    // Nor the launcher's chat's id, which its harness's figure is kept under.
    assert!(NOT_INHERITED.contains(&CHAT_ID_ENV));
    // charter's own two are set per session, after these are removed, so they are not
    // here — removing them would remove what the app just put in.
    assert!(!NOT_INHERITED.contains(&SOCKET_ENV));
    assert!(!NOT_INHERITED.contains(&CHAT_ENV));
}

#[test]
fn a_pid_that_is_not_one_is_no_pid() {
    // `charter/hooks.py` treats a missing or non-numeric `CLAUDE_PID` as "no report" for
    // adoption. A zero is not a pid either.
    for bad in ["", "0", "-1", "nine", "12x"] {
        let env = env_of(&[
            (SOCKET_ENV, "/tmp/s.sock"),
            (CHAT_ENV, "7"),
            (CLAUDE_PID_ENV, bad),
        ]);
        assert_eq!(
            Report::read(Event::Stop, CLAUDE_STOP, &env)
                .expect("a report")
                .pid,
            None,
            "{bad:?} was taken as a pid"
        );
    }
}

#[test]
fn a_listener_another_was_bound_over_stops_at_once_and_leaves_the_others_file() {
    // Two listeners at one path in one process: the second takes the path. The first is
    // woken through its own pair, not the path, so it stops at once; and the file at the
    // path is the second's, so the first leaves it. Once, a join here held a test run for
    // an hour, and an ordinary stop took the newer listener's file.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let first = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .each(Box::new(|_| {}));
    let listener = Listener::bind(dir.path(), &path).expect("the same path, bound again");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let second = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));

    let began = std::time::Instant::now();
    drop(first);
    assert!(
        began.elapsed() < A_READING_STOPS_WITHIN,
        "the first listener's thread was not woken: it waited out its bound"
    );

    // The second is untouched by it: its file is still there, and it still hears.
    assert!(path.exists(), "the first listener took the second's file");
    let sent = Report {
        chat: 7,
        event: Event::Notification,
        conversation: Conversation::Named("abc".to_owned()),
        pid: Some(99),
        agent: None,
        detail: Detail::default(),
    };
    send(&path, Some(&token), &sent).expect("the listener at the path took it");
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(sent));

    // And one dropped the ordinary way still stops at once and takes its file.
    let began = std::time::Instant::now();
    drop(second);
    assert!(began.elapsed() < A_READING_STOPS_WITHIN);
    assert!(!path.exists());
}

#[test]
fn a_listener_whose_file_was_removed_still_stops_at_once() {
    // `rm -rf .purlis`, or the project folder moved, under a live app: nothing is at the
    // path to connect to, and the listener must stop all the same.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let reading = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .each(Box::new(|_| {}));
    std::fs::remove_file(&path).expect("the file removed under it");

    let began = std::time::Instant::now();
    drop(reading);

    assert!(began.elapsed() < A_READING_STOPS_WITHIN);
    assert!(!path.exists());
}

#[test]
fn a_listeners_file_is_its_own_only_while_it_is_the_one_it_made() {
    // The decision a stopping listener makes before it removes its file, read on plain
    // files: the device and inode say which file it is, whatever its name.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    std::fs::write(&path, b"").expect("a file");
    let bound = BoundFile::of(&std::fs::symlink_metadata(&path).expect("its metadata"));

    assert_eq!(what_is_at(&path, bound), AtItsPath::Its);

    // Another file put at the path, as `bind` does: removed, then made again. The new
    // one is made before the old is gone, so it cannot be dealt the old one's inode.
    let newer = dir.path().join("newer");
    std::fs::write(&newer, b"").expect("another file");
    std::fs::rename(&newer, &path).expect("put at the path");
    assert_eq!(what_is_at(&path, bound), AtItsPath::Another);

    std::fs::remove_file(&path).expect("removed");
    assert_eq!(what_is_at(&path, bound), AtItsPath::Removed);

    // A link at the path is another file too, wherever it points.
    let target = dir.path().join("target");
    std::fs::write(&target, b"").expect("a target");
    let target_is = BoundFile::of(&std::fs::symlink_metadata(&target).expect("metadata"));
    std::os::unix::fs::symlink(&target, &path).expect("a link");
    assert_eq!(what_is_at(&path, target_is), AtItsPath::Another);
}

#[test]
fn a_report_reaches_the_app_that_is_listening() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));

    let sent = Report {
        chat: 7,
        event: Event::Notification,
        conversation: Conversation::Named("abc".to_owned()),
        pid: Some(99),
        agent: None,
        detail: Detail::default(),
    };
    send(&path, Some(&token), &sent).expect("the app took it");

    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(sent));
}

#[test]
fn every_report_arrives_even_when_many_hooks_fire_at_once() {
    // Fifty live sessions is the product's scale, and a turn ending in each of them is
    // fifty hook processes at the same moment. Here they are fifty threads of one
    // process, so the share one process is served is lifted to what fifty would have.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .serving_at_most(64, THE_CHANNEL_SERVES_AT_MOST);
    let tokens = listener.tokens();
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.chat);
    }));

    let path = Arc::new(path);
    let senders: Vec<_> = (0..50u32)
        .map(|chat| {
            let path = Arc::clone(&path);
            let token = tokens.issue_to_this_process(chat).expect("a token");
            std::thread::spawn(move || {
                send(
                    &path,
                    Some(&token),
                    &Report {
                        chat,
                        event: Event::Stop,
                        conversation: Conversation::Unknown,
                        pid: None,
                        agent: None,
                        detail: Detail::default(),
                    },
                )
            })
        })
        .collect();
    for sender in senders {
        sender.join().expect("the thread").expect("it was taken");
    }

    let mut seen: Vec<u32> = (0..50)
        .map(|_| {
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .expect("a report")
        })
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..50).collect::<Vec<_>>());
}

#[test]
fn one_connection_that_says_nothing_useful_does_not_silence_the_channel() {
    // An app that stopped listening because something wrote nonsense at it would leave
    // every chat frozen at the state it last had — and never say so. Anything at all can
    // reach this socket: a stale hook from an older charter, a port scanner, a person
    // with `nc`.
    use std::io::Write;

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));

    // Connected and closed without a word; a line that is not JSON; JSON that is not a
    // report; and a report missing the one field that names a chat.
    drop(std::os::unix::net::UnixStream::connect(&path).expect("a connection"));
    for rubbish in [
        "not json at all\n",
        "{}\n",
        r#"{"event":"stop"}"#,
        r#"{"chat":"seven","event":"stop"}"#,
    ] {
        let mut socket = std::os::unix::net::UnixStream::connect(&path).expect("a connection");
        socket.write_all(rubbish.as_bytes()).expect("it is written");
    }

    let good = Report {
        chat: 7,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    };
    send(&path, Some(&token), &good).expect("the app took it");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(good),
        "the channel went quiet after something wrote rubbish at it"
    );
}

#[test]
fn a_client_that_connects_and_says_nothing_does_not_freeze_the_channel() {
    // **A defect an independent review reproduced, kept as a test.** The listener used
    // to read every connection on one thread, so anything that connected and did not
    // write — `nc -U` on the socket, a hook stopped in a debugger, a child that
    // inherited the descriptor — held it inside `read_line` forever. Every other
    // session's report queued behind it, and the sidebar went quiet with nothing to say
    // why.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (tx, rx) = mpsc::channel();
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));

    // Connected, never written to, and held open for the rest of the test.
    let silent = std::os::unix::net::UnixStream::connect(&path).expect("a connection");
    silent
        .set_read_timeout(Some(A_REPORT_TAKES_AT_MOST * 4))
        .expect("a deadline of our own");

    let good = Report {
        chat: 7,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    };
    send(&path, Some(&token), &good).expect("the app took it");

    // Well inside the deadline the silent connection will eventually hit: the point is
    // that the good report does not WAIT for it. One reader for every connection would
    // still get there, two seconds later, with fifty sessions' reports behind it.
    assert_eq!(
        rx.recv_timeout(A_REPORT_TAKES_AT_MOST / 4),
        Ok(good),
        "a report queued behind a client that never spoke"
    );

    // And the silent one is let go rather than held forever: the app closes its end when
    // its deadline passes. Without that, every such connection costs a thread for as long
    // as the app runs.
    //
    // End of file OR an error, and the distinction that matters is WHICH error: a clean
    // close reads `Ok(0)` on macOS and can read `ECONNRESET` on Linux, and CI found that
    // difference. `WouldBlock` is the one answer that means the app did NOT let go — it
    // is this client's own deadline firing, four times longer than the app's.
    let mut nothing = [0u8; 1];
    let let_go = (&silent).read(&mut nothing);
    assert!(
        !matches!(&let_go, Err(err) if err.kind() == io::ErrorKind::WouldBlock),
        "the app never let go of a client that said nothing"
    );
}

#[test]
fn a_client_that_connects_and_says_nothing_does_not_hold_up_shutting_down() {
    // The same defect's other half: `Reading::drop` joins the reading thread, and a
    // thread stuck in `read_line` never returns — so quitting the app hung. Quitting is
    // the one thing that must always finish.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let reading = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .each(Box::new(|_| {}));
    let _silent = std::os::unix::net::UnixStream::connect(&path).expect("a connection");

    let (done, waited) = mpsc::channel();
    std::thread::spawn(move || {
        drop(reading);
        let _ = done.send(());
    });

    assert_eq!(
        waited.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(()),
        "shutting down waited on a client that never spoke"
    );
}

#[test]
fn a_client_that_writes_without_ever_ending_a_line_is_cut_off() {
    // Nothing stops a client writing bytes and never sending a newline. Without a cap
    // the app grows a `String` for it until there is no memory left.
    use std::io::Write;

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (tx, rx) = mpsc::channel();
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));

    let mut flood = std::os::unix::net::UnixStream::connect(&path).expect("a connection");
    // Set before anything is written, while the connection is certainly healthy.
    flood
        .set_read_timeout(Some(A_REPORT_TAKES_AT_MOST * 4))
        .expect("a deadline of our own");
    flood
        .set_write_timeout(Some(A_REPORT_TAKES_AT_MOST * 4))
        .expect("a deadline of our own");
    // Comfortably past the cap, with not one newline in it. The app stops reading at the
    // cap and closes, so this either errors or the read below sees end of file — what it
    // must never do is keep taking bytes.
    for _ in 0..40 {
        if flood.write_all(&vec![b'x'; 8 * 1024]).is_err() {
            break;
        }
    }
    // **Inside the deadline, not merely eventually.** Without the cap the app keeps
    // reading until the read deadline passes, so an assertion with no clock in it passes
    // either way — a review pointed that out. The cap is observable by WHEN the app lets
    // go, not by memory.
    let began = std::time::Instant::now();
    let mut nothing = [0u8; 1];
    let let_go = (&flood).read(&mut nothing);
    assert!(
        !matches!(&let_go, Err(err) if err.kind() == io::ErrorKind::WouldBlock),
        "the app is still reading a line that will never end"
    );
    assert!(
        began.elapsed() < A_REPORT_TAKES_AT_MOST / 4,
        "the app read for {:?}, so it was the deadline that stopped it and not the cap",
        began.elapsed()
    );

    let good = Report {
        chat: 7,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    };
    send(&path, Some(&token), &good).expect("the app took it");
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(good));
}

#[test]
fn the_socket_sits_in_a_directory_nobody_else_may_enter() {
    // The socket's own mode is set after `bind`, and between the two it is at whatever
    // the umask allowed. A directory nobody else may enter closes that window, and is
    // what actually holds under a permissive umask.
    let dir = tempfile::tempdir().expect("a directory");
    let inside = dir.path().join("app");
    let path = inside.join("hooks.sock");

    let _listener = Listener::bind(dir.path(), &path).expect("a socket");

    let mode = std::fs::metadata(&inside)
        .expect("the directory is there")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700, "the socket's directory is {mode:o}");
}

#[test]
fn a_socket_will_not_be_bound_inside_a_directory_somebody_else_could_have_made() {
    // The fallback path is predictable and, on Linux, lives in `/tmp`. Somebody who gets
    // there first must not end up owning the channel — they could read every report and
    // inject their own. A directory that is not a directory is the shape of that attempt
    // charter can always detect; one owned by another account fails the chmod below it.
    let dir = tempfile::tempdir().expect("a directory");
    let squatted = dir.path().join("app");
    std::fs::write(&squatted, b"not a directory").expect("something else is there first");

    let refused = Listener::bind(dir.path(), &squatted.join("hooks.sock"));

    assert!(
        refused.is_err(),
        "purlis bound a socket under something it does not own"
    );
}

#[test]
fn a_socket_reached_through_a_link_higher_up_is_refused() {
    // **The gate sat one level shallower than the write.** `private_directory` checked
    // the directory it was about to make and nothing above it, so `.charter` being a link
    // redirected where charter created a directory and bound a socket — outside the
    // plane, somewhere chosen by whoever wrote the link. `app` itself being a link was
    // caught; the component above it was not.
    //
    // charter-app#28 ruled this exact directory may not be reached through a link, for
    // the record it already keeps there. The socket is charter's own file in charter's
    // own directory: a link anywhere on the way to it has no honest use.
    let plane = tempfile::tempdir().expect("a plane");
    let elsewhere = tempfile::tempdir().expect("somewhere outside it");
    std::os::unix::fs::symlink(elsewhere.path(), plane.path().join(".charter"))
        .expect("a link in the way");

    let refused = Listener::bind(
        plane.path(),
        &plane.path().join(".charter").join("app").join("hooks.sock"),
    );

    assert!(refused.is_err(), "purlis bound a socket through a link");
    assert!(
        !elsewhere.path().join("app").exists(),
        "it made a directory outside the plane on the way"
    );
}

#[test]
fn a_component_charter_cannot_even_look_at_is_refused_rather_than_walked_past() {
    // A surviving mutant: taking ANY error as "not there yet" passed everything. It is
    // only `NotFound` that means charter is about to make it — a component it cannot
    // stat is one it knows nothing about, and walking past it would be assuming the rest
    // of the path is what it looks like.
    let plane = tempfile::tempdir().expect("a plane");
    let shut = plane.path().join(".charter");
    std::fs::create_dir(&shut).expect("a directory");
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000))
        .expect("shut to everyone");

    let refused = Listener::bind(plane.path(), &shut.join("app").join("hooks.sock"));

    // Put it back before any assertion, so a failure does not leave an unreadable
    // directory behind for the tempdir to trip over.
    let _ = std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o700));
    assert!(
        refused.is_err(),
        "purlis walked past a component it could not look at"
    );
}

#[test]
fn a_socket_under_a_root_with_nothing_linked_on_the_way_is_bound() {
    // The containment starts at a root the caller names and walks DOWN. It cannot start
    // at `/`: on macOS `/tmp` and `/var` are themselves links, so a walk from the root of
    // the filesystem refuses every path on the machine. That is the shape of resolver
    // this repo has got subtly wrong five times, so there is no resolver here — only a
    // walk of the components below a root the caller already trusts.
    let plane = tempfile::tempdir().expect("a plane");

    let listener = Listener::bind(
        plane.path(),
        &plane.path().join(".charter").join("app").join("hooks.sock"),
    )
    .expect("an ordinary plane binds");

    assert!(listener.path().exists());
}

#[test]
fn a_symlink_standing_in_for_the_socket_directory_is_refused() {
    // A surviving mutant: `symlink_metadata` weakened to `metadata` passed everything,
    // because nothing put a symlink there. It is the precise attack the guard exists to
    // stop — on the predictable path under a shared temp directory, a link left by
    // somebody else would be followed, and `set_permissions` would tighten whatever it
    // aims at while charter listened inside it.
    let dir = tempfile::tempdir().expect("a directory");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("somewhere to point at");
    let link = dir.path().join("app");
    std::os::unix::fs::symlink(&elsewhere, &link).expect("a link in the way");

    let refused = Listener::bind(dir.path(), &link.join("hooks.sock"));

    assert!(
        refused.is_err(),
        "purlis listened inside a symlink it did not make"
    );
    // And it did not tighten what the link pointed at on the way past.
    let mode = std::fs::metadata(&elsewhere)
        .expect("still there")
        .permissions()
        .mode();
    assert_ne!(
        mode & 0o777,
        0o700,
        "it followed the link and chmod'd the target"
    );
}

#[test]
fn a_socket_directory_left_at_the_umask_by_an_older_charter_is_tightened() {
    use std::os::unix::fs::DirBuilderExt;

    let dir = tempfile::tempdir().expect("a directory");
    let loose = dir.path().join("app");
    std::fs::DirBuilder::new()
        .mode(0o755)
        .create(&loose)
        .expect("a directory an older purlis might have left");

    let listener = Listener::bind(dir.path(), &loose.join("hooks.sock")).expect("a socket");

    let mode = std::fs::metadata(loose)
        .expect("it is there")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700, "the directory is still {mode:o}");
    assert!(listener.path().exists());
}

#[test]
fn a_socket_whose_directory_does_not_exist_yet_is_still_bound() {
    // The app's own path is `<plane>/.charter/app/hooks.sock`, and `.charter/app` may
    // not exist on a plane the app has never run in. A surviving mutant is why this is
    // here: every other test binds inside a directory that already exists.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("never").join("made").join("hooks.sock");

    let listener = Listener::bind(dir.path(), &path).expect("the directory is made");

    assert!(listener.path().exists());
}

#[test]
fn a_socket_directory_that_cannot_be_made_is_refused_for_that_reason() {
    // Not "already exists", which the checks after it judge: any other failure to make the
    // directory is the answer, and the reason it gives is the one the operator reads.
    let dir = tempfile::tempdir().expect("a directory");
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();

    let refused = Listener::bind(dir.path(), &locked.join("app").join("hooks.sock"));

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let Err(err) = refused else {
        panic!("bound inside a directory it could not make")
    };
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{err}");
}

#[test]
fn something_that_is_not_a_socket_where_the_socket_goes_is_refused_as_it_is() {
    // A stale socket is removed; a directory there is not, and the refusal is the removal's
    // own — not the "address in use" that binding over it would say instead.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("app").join("hooks.sock");
    std::fs::create_dir_all(path.join("inside")).unwrap();

    let Err(err) = Listener::bind(dir.path(), &path) else {
        panic!("bound over a directory")
    };
    assert_ne!(err.kind(), io::ErrorKind::AddrInUse, "{err}");
    assert!(
        path.join("inside").is_dir(),
        "the directory was left as it was"
    );
}

#[test]
fn a_socket_nothing_is_listening_on_refuses_rather_than_hangs() {
    // The app is not running, or has quit. The hook must find out at once and get out of
    // the harness's way.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("nobody.sock");

    let began = std::time::Instant::now();
    let sent = send(
        &path,
        None,
        &Report {
            chat: 1,
            event: Event::Stop,
            conversation: Conversation::Unknown,
            pid: None,
            agent: None,
            detail: Detail::default(),
        },
    );

    assert!(sent.is_err(), "a socket with no app behind it was taken");
    assert!(
        began.elapsed() < std::time::Duration::from_millis(50),
        "it took {:?} to find out nothing was listening",
        began.elapsed()
    );
}

#[test]
fn a_report_is_written_as_one_line_because_that_is_what_the_reader_reads() {
    // The reader takes a line at a time. A surviving mutant is why this is pinned: with
    // the newline dropped, every test still passed — the reader's `read_line` returns at
    // end of file too, so nothing noticed until two reports shared a connection.
    let report = Report {
        chat: 7,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    };
    let wire = line_with(Some(&ChatToken::from("t")), &report).expect("it serialises");

    assert_eq!(wire.last(), Some(&b'\n'));
    assert_eq!(wire.iter().filter(|byte| **byte == b'\n').count(), 1);
}

#[test]
fn the_socket_is_taken_away_when_the_app_stops_listening() {
    // A surviving mutant: nothing asserted the socket file was cleaned up. One left
    // behind makes the next `bind` unlink a path it does not own.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let reading = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .each(Box::new(|_| {}));
    assert!(path.exists());

    drop(reading);

    assert!(
        !path.exists(),
        "the socket outlived the app listening on it"
    );
}

#[test]
fn nobody_else_on_the_machine_can_write_to_the_socket() {
    // Anything that reaches this socket moves a chat's state. Nothing secret goes over
    // it and nothing it says is executed, so the worst another local process could do is
    // make the sidebar lie and raise a notification — but a socket left at whatever the
    // umask happened to be is not something to leave to the umask.
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");

    let listener = Listener::bind(dir.path(), &path).expect("a socket");

    let mode = std::fs::metadata(listener.path())
        .expect("the socket is there")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o600,
        "the socket is {mode:o}, so another account on this machine can move a chat"
    );
}

#[test]
fn a_socket_left_behind_by_a_process_that_is_gone_is_replaced() {
    // The app was killed, or crashed. A stale socket file must not stop the next launch
    // from listening — and `bind` refuses an address already in use.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    drop(Listener::bind(dir.path(), &path).expect("the first socket"));
    assert!(
        path.exists(),
        "the file is what makes this the interesting case"
    );

    let listener = Listener::bind(dir.path(), &path).expect("the socket is taken over");

    assert_eq!(listener.path(), path);
}

#[test]
fn an_app_is_listening_only_while_it_accepts_on_the_socket() {
    // #95: the one place that says whether an app is there, which the status line asks.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    assert!(!listening(&path), "nothing at the path");

    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    assert!(listening(&path), "an app accepts there");

    drop(listener);
    assert!(path.exists(), "its file is left behind");
    assert!(!listening(&path), "a file whose app has gone");
}

// ---- the ask, and the ticket (charter-app#204) --------------------------------------

fn an_open(chat: u32, ticket: &str) -> Ask {
    Ask::Open(Box::new(OpenChat {
        chat,
        workspace: "alpha".to_owned(),
        create_vision: None,
        persona: None,
        message: "⟨handoff from chat 3 · workspace default · 2026-05-04 11:32⟩\n\nbrief".to_owned(),
        ticket: ticket.to_owned(),
        name: None,
        older_report: false,
    }))
}

#[test]
fn an_open_from_a_charter_that_predates_names_and_reports_reads_as_neither() {
    let line = r#"{"open":{"chat":3,"workspace":"alpha","create_vision":null,"persona":null,"message":"m","ticket":"t"}}"#;

    let Ask::Open(open) = serde_json::from_str::<Ask>(line).expect("it parses") else {
        panic!("an open")
    };

    assert_eq!(open.name, None);
    assert!(!open.older_report);
}

/// #1471: an older command line's `report` is still read, so the app can refuse it, and
/// this purlis never writes it, whatever the field holds.
#[test]
fn an_older_lines_report_is_read_and_never_written() {
    let line = r#"{"open":{"chat":3,"workspace":"alpha","create_vision":null,"persona":null,"message":"m","ticket":"t","report":true}}"#;

    let Ask::Open(mut open) = serde_json::from_str::<Ask>(line).expect("it parses") else {
        panic!("an open")
    };
    assert!(open.older_report);

    open.older_report = true;
    let written = serde_json::to_string(&Ask::Open(open)).expect("json");
    assert!(!written.contains("report"), "{written}");
}

#[test]
fn a_report_back_is_an_ask_and_never_a_report() {
    let ask = Ask::Report(Box::new(ReportBack {
        chat: 7,
        summary: "done".to_owned(),
        ticket: "t".to_owned(),
        task: None,
    }));
    let line = serde_json::to_string(&ask).unwrap();

    assert!(matches!(
        serde_json::from_str::<Line>(&line),
        Ok(Line::Ask(_))
    ));
}

#[test]
fn an_asker_that_hung_up_while_it_was_answered_is_seen_to_have_gone() {
    // #1441, M3: a waiting command that was killed must not hold a thread for minutes.
    let tokens = ChatTokens::default();
    let (ours, theirs) = std::os::unix::net::UnixStream::pair().expect("a pair");
    assert!(!tokens.asker_gone(7), "nobody is being answered on 7");

    tokens.answering(7, ours);
    assert!(!tokens.asker_gone(7), "still there, and saying nothing");
    assert!(!tokens.asker_gone(7), "and a look changes nothing");

    drop(theirs);
    assert!(tokens.asker_gone(7));
    tokens.answered(7);
    assert!(
        !tokens.asker_gone(7),
        "answered: no longer anyone's to ask about"
    );
}

#[test]
fn a_ticket_opens_once_and_never_again() {
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let ticket = tickets.mint(3, 1, now).expect("a ticket");

    assert_eq!(tickets.spend(3, 1, &ticket, now), Ok(()));
    assert_eq!(
        tickets.spend(3, 1, &ticket, now),
        Err(NO_TICKET.to_owned()),
        "a replay of the same line opens nothing"
    );
}

#[test]
fn a_ticket_spends_only_on_the_connection_that_minted_it() {
    // A line copied onto a connection of its own is a replay, whatever it carries.
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let ticket = tickets.mint(3, 1, now).expect("a ticket");

    assert_eq!(tickets.spend(3, 2, &ticket, now), Err(NO_TICKET.to_owned()));
}

#[test]
fn a_ticket_spends_only_for_the_chat_it_was_minted_for() {
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let ticket = tickets.mint(3, 1, now).expect("a ticket");

    assert_eq!(tickets.spend(4, 1, &ticket, now), Err(NO_TICKET.to_owned()));
    assert_eq!(
        tickets.spend(3, 1, &ticket, now),
        Ok(()),
        "3's is untouched"
    );
}

#[test]
fn a_wrong_guess_spends_the_real_ticket_too() {
    // One try per mint. Without this a guesser could keep trying against a live ticket
    // for as long as it lived.
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let ticket = tickets.mint(3, 1, now).expect("a ticket");

    assert!(tickets.spend(3, 1, "not-it", now).is_err());
    assert_eq!(tickets.spend(3, 1, &ticket, now), Err(NO_TICKET.to_owned()));
}

#[test]
fn a_ticket_left_unspent_expires() {
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let ticket = tickets.mint(3, 1, now).expect("a ticket");

    assert_eq!(
        tickets.spend(3, 1, &ticket, now + A_TICKET_LIVES),
        Err(NO_TICKET.to_owned())
    );
}

#[test]
fn a_second_mint_for_a_chat_stands_beside_the_first_and_never_replaces_it() {
    // Replacing would let a second process cancel the handoff the chat is making,
    // silently. Each run of a command has its own ticket, on its own connection.
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let first = tickets.mint(3, 1, now).expect("a ticket");
    let second = tickets.mint(3, 2, now).expect("a ticket of its own");

    assert_ne!(first, second);
    // Neither spends on the other's connection, and a try there costs the ticket that
    // connection holds, never the other's.
    assert_eq!(tickets.spend(3, 2, &first, now), Err(NO_TICKET.to_owned()));
    assert_eq!(tickets.spend(3, 2, &second, now), Err(NO_TICKET.to_owned()));
    assert_eq!(tickets.spend(3, 1, &first, now), Ok(()));
}

#[test]
fn six_dispatches_at_once_from_one_chat_each_spend_a_ticket_of_their_own_once() {
    // Fan-out: one chat, six runs of the command, six connections, all minted before any
    // is spent. Every one spends, in any order, and none spends twice.
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let minted: Vec<(u64, String)> = (1..=6)
        .map(|connection| {
            (
                connection,
                tickets.mint(3, connection, now).expect("a ticket"),
            )
        })
        .collect();
    let distinct: std::collections::HashSet<&String> =
        minted.iter().map(|(_, ticket)| ticket).collect();
    assert_eq!(distinct.len(), 6);

    for (connection, ticket) in minted.iter().rev() {
        assert_eq!(tickets.spend(3, *connection, ticket, now), Ok(()));
    }
    for (connection, ticket) in &minted {
        assert_eq!(
            tickets.spend(3, *connection, ticket, now),
            Err(NO_TICKET.to_owned()),
            "a replay opens nothing"
        );
    }
}

#[test]
fn a_connection_holds_one_ticket_so_one_run_of_a_command_starts_one_chat() {
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let first = tickets.mint(3, 1, now).expect("a ticket");

    let again = tickets
        .mint(3, 1, now)
        .expect_err("one ticket a connection");
    assert!(again.contains("already holds a ticket"), "{again}");
    assert_eq!(tickets.spend(3, 1, &first, now), Ok(()), "the first stands");
    assert!(
        tickets.mint(3, 1, now).is_ok(),
        "and once it is spent, the connection can mint again"
    );
}

#[test]
fn a_chat_s_live_tickets_are_bounded_and_the_refusal_names_no_handoff() {
    // Abandoned mints cannot pile up without end, and the sentence is every ask's.
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    for connection in 0..MOST_LIVE_TICKETS_A_CHAT as u64 {
        tickets.mint(3, connection, now).expect("room");
    }

    let refused = tickets.mint(3, 999, now).expect_err("full");
    assert_eq!(
        refused,
        format!(
            "chat 3 has {MOST_LIVE_TICKETS_A_CHAT} requests to the app under way at once; \
                 try again in a few seconds"
        )
    );
    assert!(
        tickets.mint(4, 999, now).is_ok(),
        "another chat's are its own"
    );
    assert!(
        tickets.mint(3, 999, now + A_TICKET_LIVES).is_ok(),
        "and the abandoned ones expire"
    );
}

#[test]
fn an_abandoned_ticket_never_holds_up_the_chat_s_next_ask() {
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let _abandoned = tickets.mint(3, 1, now).expect("a ticket");

    assert!(tickets.mint(3, 2, now).is_ok());
}

#[test]
fn two_tickets_are_never_the_same() {
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    let one = tickets.mint(3, 1, now).expect("a ticket");
    let two = tickets.mint(4, 1, now).expect("a ticket");

    assert_ne!(one, two);
    assert_eq!(one.len(), 64);
    assert!(one.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn a_guess_as_long_as_the_ticket_or_a_prefix_of_it_opens_nothing() {
    // The comparison needs the lengths equal AND every byte equal. A guess of the right
    // length that differs in one byte, and the ticket cut short — whose every byte the
    // zip does compare equal — are both refused.
    let now = std::time::Instant::now();
    for guess in [
        |t: &str| {
            let mut g = t.as_bytes().to_vec();
            g[63] = if g[63] == b'0' { b'1' } else { b'0' };
            String::from_utf8(g).unwrap()
        },
        |t: &str| t[..32].to_owned(),
        |t: &str| format!("{t}0"),
        // Two bytes wrong by the same bit: differences are OR-ed together, so a second
        // one cannot cancel the first.
        |t: &str| {
            let mut g = t.as_bytes().to_vec();
            g[0] ^= 1;
            g[1] ^= 1;
            String::from_utf8(g).unwrap()
        },
    ] {
        let tickets = Tickets::default();
        let ticket = tickets.mint(3, 1, now).expect("a ticket");
        let wrong = guess(&ticket);
        assert_ne!(wrong, ticket);
        assert_eq!(
            tickets.spend(3, 1, &wrong, now),
            Err(NO_TICKET.to_owned()),
            "{wrong}"
        );
    }
}

/// A listener whose every ask is answered with `answer`.
fn an_app_answering(
    path: &std::path::Path,
    within: &std::path::Path,
    answer: Answer,
) -> (Reading, ChatToken) {
    let listener = Listener::bind(within, path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let reading = listener.each_answering(Box::new(|_| {}), Box::new(move |_, _| answer.clone()));
    (reading, token)
}

#[test]
fn an_answer_up_to_the_report_cap_is_read_whole_and_one_past_it_is_not() {
    let dir = tempfile::tempdir().expect("a directory");
    let within = std::time::Duration::from_secs(5);
    // `{"no":{"why":"…"}}` and its newline, sized to land exactly on the cap and one past.
    let frame = serde_json::to_vec(&Answer::No { why: String::new() })
        .unwrap()
        .len()
        + 1;
    let cap = usize::try_from(A_REPORT_IS_AT_MOST).unwrap();
    assert_eq!(cap, 65_536);

    let whole = Answer::No {
        why: "w".repeat(cap - frame),
    };
    let path = dir.path().join("whole.sock");
    let (_reading, token) = an_app_answering(&path, dir.path(), whole.clone());
    let got = Asking::on(&path, Some(token))
        .expect("connected")
        .ask(&Ask::Ticket { chat: 3 }, within)
        .expect("an answer exactly at the cap");
    assert_eq!(got, whole);

    let over = Answer::No {
        why: "w".repeat(cap - frame + 2),
    };
    let path = dir.path().join("over.sock");
    let (_reading, token) = an_app_answering(&path, dir.path(), over);
    assert!(
        Asking::on(&path, Some(token))
            .expect("connected")
            .ask(&Ask::Ticket { chat: 3 }, within)
            .is_err(),
        "an answer past the cap was read"
    );
}

/// A framed conversation for chat 3 with the app at `path`, which must speak frames.
fn framed_on(path: &std::path::Path, token: &ChatToken) -> Framed {
    match Framed::on(path, Some(token), 3, std::time::Duration::from_secs(5)).expect("connected") {
        framed::Greeted::Framed(framed) => framed,
        framed::Greeted::Older => panic!("the app did not speak frames"),
        framed::Greeted::Refused(why) => panic!("refused: {why}"),
    }
}

#[test]
fn an_answer_past_a_line_comes_back_whole_on_the_framed_stream_and_lines_still_work() {
    // #1996: a client that says hello gets frames; one that never does is unaffected.
    let dir = tempfile::tempdir().expect("a directory");
    let within = std::time::Duration::from_secs(5);
    let cap = usize::try_from(A_REPORT_IS_AT_MOST).unwrap();
    let large = Answer::No {
        why: "w".repeat(3 * cap),
    };
    let path = dir.path().join("hooks.sock");
    let (_reading, token) = an_app_answering(&path, dir.path(), large.clone());

    let mut framed = framed_on(&path, &token);
    let reply = framed
        .ask(&Ask::Ticket { chat: 3 }, within)
        .expect("a reply past a line");
    assert_eq!(reply.answer, large);
    // A second ask on the same stream.
    let again = framed.ask(&Ask::Ticket { chat: 3 }, within).expect("again");
    assert_eq!(again.answer, large);

    // A line client of the same app is still answered in lines, and its reader stops at
    // its cap: what it read is the answer's first line cut short, never a frame.
    let in_lines = Asking::on(&path, Some(token))
        .expect("connected")
        .ask(&Ask::Ticket { chat: 3 }, within)
        .expect_err("an answer past the line cap is not read");
    assert!(
        in_lines.to_string().contains("EOF while parsing"),
        "{in_lines}"
    );
}

#[test]
fn a_hello_without_its_chats_token_is_refused_and_never_accepted() {
    let dir = tempfile::tempdir().expect("a directory");
    let within = std::time::Duration::from_secs(5);
    let path = dir.path().join("hooks.sock");
    let (_reading, _token) = an_app_answering(
        &path,
        dir.path(),
        Answer::Ticket {
            ticket: "t".to_owned(),
        },
    );
    for token in [None, Some(ChatToken::from("nope"))] {
        match Framed::on(&path, token.as_ref(), 3, within).expect("connected") {
            framed::Greeted::Refused(why) => assert!(why.contains("token"), "{why}"),
            framed::Greeted::Framed(_) => panic!("accepted"),
            framed::Greeted::Older => panic!("read as an older app"),
        }
    }
}

#[test]
fn a_hello_at_another_version_is_answered_no_and_never_accepted() {
    use std::io::{BufRead, Write};
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, token) = an_app_answering(
        &path,
        dir.path(),
        Answer::Ticket {
            ticket: "t".to_owned(),
        },
    );
    let hello = framed::Hello {
        frames: framed::VERSION + 1,
        chat: 3,
    };
    let mut socket = std::os::unix::net::UnixStream::connect(&path).expect("connected");
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    socket
        .write_all(&line_with(Some(&token), &hello).unwrap())
        .unwrap();
    let mut said = String::new();
    std::io::BufReader::new(&socket)
        .read_line(&mut said)
        .unwrap();
    assert!(
        matches!(
            serde_json::from_str::<Answer>(&said),
            Ok(Answer::No { why }) if why.contains("version")
        ),
        "{said}"
    );
}

#[test]
fn a_framed_ask_for_another_chat_is_answered_no_and_never_reaches_the_app() {
    let dir = tempfile::tempdir().expect("a directory");
    let within = std::time::Duration::from_secs(5);
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let _four = listener.tokens().issue_to_this_process(4).expect("a token");
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Answer::Ticket {
                ticket: "t".to_owned(),
            }
        }),
    );
    let reply = framed_on(&path, &token)
        .ask(&Ask::Ticket { chat: 4 }, within)
        .expect("a refusal");
    assert!(
        matches!(&reply.answer, Answer::No { why } if why.contains("chat 4")),
        "{reply:?}"
    );
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn every_framed_ask_is_checked_against_the_chat_s_token_as_it_stands_then() {
    // The hello's token is checked again at each ask: one the chat no longer holds (it was
    // started again, and given a new one) speaks for nothing on a stream opened with it.
    let dir = tempfile::tempdir().expect("a directory");
    let within = std::time::Duration::from_secs(5);
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let tokens = listener.tokens();
    let token = tokens.issue_to_this_process(3).expect("a token");
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Answer::Ticket {
                ticket: "t".to_owned(),
            }
        }),
    );
    let mut framed = framed_on(&path, &token);
    assert!(matches!(
        framed.ask(&Ask::Ticket { chat: 3 }, within).unwrap().answer,
        Answer::Ticket { .. }
    ));

    let _anew = tokens.issue_to_this_process(3).expect("a new token");
    let reply = framed
        .ask(&Ask::Ticket { chat: 3 }, within)
        .expect("a refusal");
    assert!(
        matches!(&reply.answer, Answer::No { why } if why.contains("token")),
        "{reply:?}"
    );
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn an_open_carrying_the_largest_first_message_arrives_and_a_longer_line_does_not() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, token) = an_app_at(&path, dir.path());
    let within = std::time::Duration::from_secs(5);

    // The worst case the cap was sized for: a whole first message of a character JSON
    // writes as six bytes.
    let mut asking = Asking::on(&path, Some(token.clone())).expect("connected");
    let ticket = ticket_from(&mut asking);
    let Ask::Open(mut open) = an_open(3, &ticket) else {
        unreachable!()
    };
    open.message = "\u{1b}".repeat(crate::handoff::FIRST_MESSAGE_MAX_BYTES);
    assert_eq!(
        asking
            .ask(&Ask::Open(open), within)
            .expect("the largest first message is read"),
        Answer::Opened {
            chat: 9,
            note: None,
        }
    );

    // A line longer than the cap is cut, and a cut line is no ask: the connection ends,
    // with a refusal said first where the asker is still there to read it (#1333).
    let mut asking = Asking::on(&path, Some(token.clone())).expect("connected");
    let ticket = ticket_from(&mut asking);
    let Ask::Open(mut open) = an_open(3, &ticket) else {
        unreachable!()
    };
    open.message = "m".repeat(usize::try_from(A_LINE_IS_AT_MOST).unwrap());
    let answered = asking.ask(&Ask::Open(open), within);
    assert!(
        matches!(&answered, Err(_) | Ok(Answer::No { .. })),
        "a line past the cap was read: {answered:?}"
    );
}

/// A listener at `path` whose answers are the ticket half and an open that always
/// succeeds as chat 9.
fn an_app_at(path: &std::path::Path, within: &std::path::Path) -> (Reading, ChatToken) {
    let listener = Listener::bind(within, path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let tickets = Tickets::default();
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |connection, ask| {
            let now = std::time::Instant::now();
            // A dispatch and a report spend a ticket as an open does.
            match &ask {
                Ask::Dispatch(dispatch) => {
                    let spent = tickets.spend(dispatch.chat, connection, &dispatch.ticket, now);
                    return match spent {
                        Ok(()) => Answer::Dispatched {
                            chat: 9,
                            name: dispatch.name.clone(),
                            persona: None,
                            note: None,
                            works: None,
                        },
                        Err(why) => Answer::No { why },
                    };
                }
                Ask::Report(back) => {
                    return match tickets.spend(back.chat, connection, &back.ticket, now) {
                        Ok(()) => Answer::Reported {
                            to: "steward 3".to_owned(),
                            kept_for: None,
                        },
                        Err(why) => Answer::No { why },
                    };
                }
                _ => {}
            }
            match ask {
                Ask::Ticket { chat } => match tickets.mint(chat, connection, now) {
                    Ok(ticket) => Answer::Ticket { ticket },
                    Err(why) => Answer::No { why },
                },
                Ask::Open(open) => match tickets.spend(open.chat, connection, &open.ticket, now) {
                    Ok(()) => Answer::Opened {
                        chat: 9,
                        note: None,
                    },
                    Err(why) => Answer::No { why },
                },
                Ask::Report(_)
                | Ask::SessionRecord(_)
                | Ask::Write(_)
                | Ask::Git(_)
                | Ask::Commit(_)
                | Ask::Vaults { .. }
                | Ask::Keys { .. }
                | Ask::WhereWorking(_)
                | Ask::Dispatch(_)
                | Ask::Task(_)
                | Ask::Removal(_)
                | Ask::Propose(_)
                | Ask::Browse(_)
                | Ask::ThemePreview(_)
                | Ask::ThemeDraft(_)
                | Ask::Prompt(_) => Answer::No {
                    why: "not here".to_owned(),
                },
            }
        }),
    );
    (reading, token)
}

fn ticket_from(asking: &mut Asking) -> String {
    match asking
        .ask(&Ask::Ticket { chat: 3 }, std::time::Duration::from_secs(2))
        .expect("an answer")
    {
        Answer::Ticket { ticket } => ticket,
        other => panic!("a ticket, not {other:?}"),
    }
}

#[test]
fn six_asks_at_once_from_one_chat_are_each_answered_on_a_ticket_of_their_own() {
    // #1441: a chat that fans out runs six commands in one step. Every one has minted
    // before any spends, which is the case one live ticket per chat used to refuse.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, token) = an_app_at(&path, dir.path());
    let all_minted = Arc::new(std::sync::Barrier::new(6));

    let asked: Vec<_> = (0..6)
        .map(|_| {
            let path = path.clone();
            let token = token.clone();
            let all_minted = Arc::clone(&all_minted);
            std::thread::spawn(move || {
                let mut asking = Asking::on(&path, Some(token)).expect("connected");
                let ticket = ticket_from(&mut asking);
                all_minted.wait();
                asking.ask(&an_open(3, &ticket), std::time::Duration::from_secs(5))
            })
        })
        .collect();

    for one in asked {
        let answered = one.join().expect("it ran").expect("an answer");
        assert!(
            matches!(answered, Answer::Opened { chat: 9, .. }),
            "{answered:?}"
        );
    }
}

#[test]
fn a_mint_and_a_spend_on_one_connection_open_a_chat() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, token) = an_app_at(&path, dir.path());

    let mut asking = Asking::on(&path, Some(token.clone())).expect("connected");
    let ticket = ticket_from(&mut asking);

    assert_eq!(
        asking
            .ask(&an_open(3, &ticket), std::time::Duration::from_secs(2))
            .expect("an answer"),
        Answer::Opened {
            chat: 9,
            note: None,
        }
    );
}

/// **The ticket's rule, over the socket, for an ask that spends one** (#1441): it is taken
/// only on the connection that minted its ticket, and once; the same ask copied onto a
/// connection that minted none is refused and costs the real one nothing; and a wrong
/// guess burns the ticket of the connection it was made on, and no other's.
///
/// A ticket used to be one a chat, and a copy elsewhere spent it. It is one a connection
/// now, so a copy elsewhere has nothing to spend: it can neither use the ticket nor take
/// it away from the run of the command that asked for it.
fn the_ticket_is_its_connection_s_alone(ask: impl Fn(&str) -> Ask) {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let (_reading, token) = an_app_at(&path, dir.path());
    let within = std::time::Duration::from_secs(2);
    let on = || Asking::on(&path, Some(token.clone())).expect("connected");
    let refused = Answer::No {
        why: NO_TICKET.to_owned(),
    };

    // A copy, on a connection that minted no ticket: refused.
    let mut asking = on();
    let ticket = ticket_from(&mut asking);
    let mut elsewhere = on();
    assert_eq!(
        elsewhere.ask(&ask(&ticket), within).expect("an answer"),
        refused,
        "a copy on another connection was taken"
    );
    // Even where that connection holds a live ticket of its own for the chat: it spends
    // its own on the wrong guess, and the copied one is not what it was given.
    let mut holding = on();
    let its_own = ticket_from(&mut holding);
    assert_eq!(
        holding.ask(&ask(&ticket), within).expect("an answer"),
        refused,
        "another connection's ticket was taken"
    );
    assert_eq!(
        holding.ask(&ask(&its_own), within).expect("an answer"),
        refused,
        "the wrong guess did not burn that connection's own ticket"
    );
    // Neither cost the minting connection anything: its ask is taken there, once.
    let taken = asking.ask(&ask(&ticket), within).expect("an answer");
    assert!(!matches!(taken, Answer::No { .. }), "{taken:?}");
    assert_eq!(
        asking.ask(&ask(&ticket), within).expect("an answer"),
        refused,
        "taken a second time"
    );
}

#[test]
fn an_open_is_taken_only_on_the_connection_that_minted_its_ticket_and_only_once() {
    the_ticket_is_its_connection_s_alone(|ticket| an_open(3, ticket));
}

#[test]
fn a_dispatch_is_taken_only_on_the_connection_that_minted_its_ticket_and_only_once() {
    the_ticket_is_its_connection_s_alone(|ticket| {
        Ask::Dispatch(Box::new(DispatchAsk {
            chat: 3,
            to: None,
            name: "check the queue".to_owned(),
            brief: "# Check the queue\nbody\n".to_owned(),
            profile: None,
            place: None,
            ticket: ticket.to_owned(),
            long: false,
        }))
    });
}

#[test]
fn a_report_is_taken_only_on_the_connection_that_minted_its_ticket_and_only_once() {
    the_ticket_is_its_connection_s_alone(|ticket| {
        Ask::Report(Box::new(ReportBack {
            chat: 3,
            summary: "Forty are stuck.".to_owned(),
            ticket: ticket.to_owned(),
            task: None,
        }))
    });
}

#[test]
fn a_listener_nobody_answers_for_refuses_an_ask_rather_than_going_quiet() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let _reading = listener.each(Box::new(|_| {}));

    let answer = Asking::on(&path, Some(token))
        .expect("connected")
        .ask(&Ask::Ticket { chat: 3 }, std::time::Duration::from_secs(2))
        .expect("an answer");

    assert_eq!(
        answer,
        Answer::No {
            why: NOTHING_ANSWERS.to_owned()
        }
    );
}

#[test]
fn a_report_written_by_hand_is_still_a_report_on_a_listener_that_answers() {
    // The bytes a hook writes, with its chat's token beside them. The ask must not have
    // taken the report's place.
    use std::io::Write;

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering(
        Box::new(move |report| tx.lock().unwrap().send(report).unwrap()),
        Box::new(|_, _| panic!("a report is not an ask")),
    );

    let mut socket = std::os::unix::net::UnixStream::connect(&path).expect("connected");
    socket
        .write_all(
            format!(
                "{{\"chat\":7,\"event\":\"stop\",\"token\":\"{}\"}}\n",
                token.expose()
            )
            .as_bytes(),
        )
        .expect("written");
    // And stays until the app lets go, as a hook does (D-1407-10): on macOS a sender that
    // closed at once may be gone before the app can name it (#2458).
    let _ = socket.shutdown(std::net::Shutdown::Write);
    socket
        .set_read_timeout(Some(A_REPORT_TAKES_AT_MOST))
        .expect("a deadline");
    let _ = std::io::Read::read_to_end(&mut socket, &mut Vec::new());

    let report = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("the report arrives");
    assert_eq!(report.chat, 7);
    assert_eq!(report.event, Event::Stop);
}

#[test]
fn asking_where_no_app_is_listening_fails_at_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let started = std::time::Instant::now();

    assert!(Asking::on(&dir.path().join("gone.sock"), None).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

// ------------------------------------------------------------------------------------
// a harness started by hand in a shell tab (SI-5)
// ------------------------------------------------------------------------------------

fn by_hand() -> StartedByHand {
    StartedByHand {
        chat: 4,
        started_by_hand: "claude".to_owned(),
        cwd: Some(std::path::PathBuf::from("/work/alpha")),
    }
}

#[test]
fn a_harness_started_by_hand_reaches_the_app_as_a_notice_and_never_as_a_report() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering_and_noticing(
        Box::new(|_| panic!("a notice is not a report")),
        Box::new(|_, _| panic!("a notice is not an ask")),
        Box::new(move |notice| tx.lock().unwrap().send(notice).unwrap()),
    );

    tell(&path, Some(&token), &by_hand()).expect("the app took it");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(by_hand())
    );
}

#[test]
fn a_report_is_never_read_as_a_harness_started_by_hand() {
    let line = serde_json::to_string(&Report {
        chat: 4,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    })
    .unwrap();

    assert!(matches!(
        serde_json::from_str::<Line>(&line),
        Ok(Line::Report(_))
    ));
    let line = serde_json::to_string(&by_hand()).unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&line),
        Ok(Line::ByHand(_))
    ));
}

#[test]
fn telling_an_app_that_is_not_listening_fails_at_once_rather_than_waiting() {
    let dir = tempfile::tempdir().expect("a directory");
    let started = std::time::Instant::now();

    assert!(tell(&dir.path().join("gone.sock"), None, &by_hand()).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}
// ------------------------------------------------------------------------------------
// a session record saved: a Smart close's end (SI-8, ADR 0064)
// ------------------------------------------------------------------------------------

fn saved() -> SessionSaved {
    SessionSaved {
        chat: 4,
        session_saved: std::path::PathBuf::from(
            "/plane/workspaces/alpha/sessions/20260928-140312-ship-it.md",
        ),
    }
}

#[test]
fn a_session_saved_reaches_the_app_as_its_own_line_and_never_as_a_report_an_ask_or_a_notice() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering_noticing_and_saving(
        Box::new(|_| panic!("a session saved is not a report")),
        Box::new(|_, _| panic!("a session saved is not an ask")),
        Box::new(|_| panic!("a session saved is not a harness started by hand")),
        Box::new(move |saved| tx.lock().unwrap().send(saved).unwrap()),
    );

    tell_saved(&path, Some(&token), &saved()).expect("the app took it");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(saved())
    );
}

#[test]
fn no_other_line_is_ever_read_as_a_session_saved() {
    let report = serde_json::to_string(&Report {
        chat: 4,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    })
    .unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&report),
        Ok(Line::Report(_))
    ));
    let notice = serde_json::to_string(&by_hand()).unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&notice),
        Ok(Line::ByHand(_))
    ));
    let ask = serde_json::to_string(&Ask::Ticket { chat: 4 }).unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&ask),
        Ok(Line::Ask(_))
    ));
    let line = serde_json::to_string(&saved()).unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&line),
        Ok(Line::Saved(_))
    ));
}

#[test]
#[cfg(unix)]
fn a_line_is_handed_on_after_every_line_whose_connection_arrived_before_it() {
    // A bound no scheduler reaches, so line 2 goes when line 1 is done and never because
    // its wait ran out: with the channel's 50 ms, a runner that held this thread longer
    // let line 2 go first (#2465).
    let turns = std::sync::Arc::new(InTurn::waiting_at_most(std::time::Duration::from_secs(60)));
    let (tx, rx) = mpsc::channel();
    let second = {
        let turns = std::sync::Arc::clone(&turns);
        let tx = tx.clone();
        std::thread::spawn(move || {
            turns.wait(2);
            tx.send(2).unwrap();
            turns.done(2);
        })
    };
    // Held up past the channel's bound, as a loaded runner can hold this thread (#2465).
    std::thread::sleep(A_TURN_IS_WAITED_AT_MOST * 2);
    tx.send(1).unwrap();
    turns.done(1);
    second.join().unwrap();

    assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
#[cfg(unix)]
fn a_connection_that_never_says_anything_holds_the_next_one_only_so_long() {
    let turns = InTurn::new();
    let began = std::time::Instant::now();

    turns.wait(2);

    assert!(
        began.elapsed() < A_TURN_IS_WAITED_AT_MOST * 4,
        "{:?}",
        began.elapsed()
    );
}

#[test]
#[cfg(unix)]
fn a_notice_that_cannot_be_delivered_in_time_is_given_up_on_and_does_not_hold_the_hook() {
    let began = std::time::Instant::now();

    let gave_up = within(std::time::Duration::from_millis(50), || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        Ok(())
    });

    assert!(gave_up.is_err());
    assert!(
        began.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        began.elapsed()
    );
    assert!(within(std::time::Duration::from_secs(1), || Ok(())).is_ok());
}

#[test]
fn a_vault_listing_is_an_ask_and_no_line_can_tag_a_vault_or_name_a_persona() {
    let read = read_line(r#"{"vaults":{"chat":7},"token":"t"}"#).expect("an ask");
    assert!(
        matches!(&read.0, Line::Ask(Ask::Vaults { chat: 7 })),
        "{read:?}"
    );
    assert_eq!(
        read.0.chat(),
        7,
        "its token is checked for the chat it names"
    );
    // The ask names a chat and nothing else: a persona beside it is not read.
    assert_eq!(
        serde_json::to_value(Ask::Vaults { chat: 7 }).unwrap(),
        serde_json::json!({"vaults": {"chat": 7}})
    );
    // No kind of line grants a vault or sets a persona: each of these is no line at all,
    // so the listener drops it unanswered.
    for forged in [
        r#"{"chat":7,"vault_grant":{"vault":"ops","persona":"steward"}}"#,
        r#"{"chat":7,"allow_vault":"ops","persona":"steward"}"#,
        r#"{"vault_grant":{"chat":7,"vault":"ops","persona":"steward"}}"#,
        r#"{"grant_vault":{"chat":7,"vault":"ops"}}"#,
        r#"{"persona":{"chat":7,"name":"ops"}}"#,
        r#"{"chat":7,"persona":"ops"}"#,
    ] {
        assert!(read_line(forged).is_none(), "{forged}");
    }
}

#[test]
fn no_line_a_chat_sends_makes_widens_or_revokes_a_dispatch_grant() {
    // #1437: a dispatch grant is the person's, made by a press in the window. The socket
    // has no line for one, so a forged line that spells a grant, an Allow or a revoke is
    // either no line at all, or one of the kinds below, none of which carries a grant.
    let forged = [
        r#"{"dispatch_grant":{"asking":"steward","target":"devops","level":"project"}}"#,
        r#"{"chat":4,"dispatch_grant":{"asking":"steward","target":"devops","level":"you"}}"#,
        r#"{"allow_dispatch":{"chat":4,"id":1,"level":"project"}}"#,
        r#"{"chat":4,"allow_dispatch":{"id":1,"level":"chat"},"token":"t"}"#,
        r#"{"revoke_dispatch_grant":{"chat":4,"id":"you\u001fsteward\u001fdevops"}}"#,
        r#"{"keep_dispatch_blocked":{"chat":4,"id":1}}"#,
        // #1502: an Allow that ticks boxes and says what it was shown.
        r#"{"allow_dispatch":{"chat":4,"id":1,"level":"you","also":["qa","docs"],"shown":"0f"}}"#,
        r#"{"chat":4,"allow_dispatch":{"id":1,"level":"project","also":["*"],"shown":""},"token":"t"}"#,
        r#"{"wants":{"persona":"steward","names":["devops","qa"]}}"#,
        // #1503: never, its lifting, and "any persona", by each window command's name.
        r#"{"never_dispatch":{"chat":4,"id":1}}"#,
        r#"{"chat":4,"never_dispatch":{"id":1},"token":"t"}"#,
        r#"{"lift_dispatch_never":{"chat":4,"asking":"steward","target":"devops"}}"#,
        r#"{"allow_dispatch_to_any":{"chat":4,"asking":"steward","level":"project"}}"#,
        r#"{"chat":4,"allow_dispatch_to_any":{"asking":"steward","level":"you"},"token":"t"}"#,
        r#"{"revoke_dispatch_to_any":{"chat":4,"asking":"steward","level":"you"}}"#,
        // #1465: a standing grant made in Settings, by the window command's name.
        r#"{"add_dispatch_grant":{"chat":4,"asking":"steward","target":"devops","level":"project","workspace":null}}"#,
        r#"{"chat":4,"add_dispatch_grant":{"asking":"steward","target":"devops","level":"you"},"token":"t"}"#,
        r#"{"dispatch_standing":{"chat":4}}"#,
        r#"{"dispatch_never":[{"asking":"steward","target":"devops"}]}"#,
        r#"{"dispatch_any":["steward"]}"#,
        // #1504: Settings' table. Accept, Not on my machine, and a grant set aside given
        // back or removed, by each window command's name; and the keys it keeps.
        r#"{"accept_project_dispatch":{"chat":4,"asking":"steward","target":"*"}}"#,
        r#"{"chat":4,"accept_project_dispatch":{"asking":"steward","target":"devops"},"token":"t"}"#,
        r#"{"decline_project_dispatch":{"chat":4,"asking":"steward","target":"devops"}}"#,
        r#"{"give_back_dispatch":{"chat":4,"name":"devops"}}"#,
        r#"{"chat":4,"give_back_dispatch":{"name":"devops"},"token":"t"}"#,
        r#"{"remove_dormant_dispatch":{"chat":4,"asking":"steward","target":"devops","any":false}}"#,
        r#"{"dispatch_dormant":[{"asking":"steward","target":"*","any":true,"was":"steward"}]}"#,
        r#"{"dispatch_known":[{"name":"devops","hash":"","away":false}]}"#,
        r#"{"dispatch_accepted_aside":[{"said":"steward -> devops","was":"devops"}]}"#,
        r#"{"dispatch_declined":["steward -> devops"]}"#,
        // #1506: the Notice of a teammate's grant arriving. Its read, its two answers and
        // "told", by each window command's name; and the two keys an acceptance is bound by.
        r#"{"dispatch_arrival":{"chat":4}}"#,
        r#"{"answer_dispatch_arrival":{"chat":4,"accepted":true,"shown":["steward -> devops"]}}"#,
        r#"{"chat":4,"answer_dispatch_arrival":{"accepted":true,"shown":["steward -> *"]},"token":"t"}"#,
        r#"{"answer_dispatch_arrival":{"chat":4,"accepted":false,"shown":["steward -> devops"]}}"#,
        r#"{"dispatch_gone_told":{"chat":4,"shown":["steward -> devops"]}}"#,
        r#"{"dispatch_seen":["steward -> devops"]}"#,
        r#"{"dispatch_seen_at":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        r#"{"dispatch_gone":[{"said":"steward -> devops","why":"removed"}]}"#,
        r#"{"answer_dispatch_arrival":{"chat":4,"accepted":true,"shown":["steward -> devops"],"listed":["steward -> devops"]}}"#,
        // #1507: the refusals kept while nobody was there, and their two answers.
        r#"{"allow_dispatch_away":{"chat":4,"asking":"steward","target":"devops"}}"#,
        r#"{"chat":4,"allow_dispatch_away":{"asking":"steward","target":"devops"},"token":"t"}"#,
        r#"{"dismiss_dispatch_away":{"chat":4,"asking":"steward","target":"devops"}}"#,
        r#"{"never_dispatch_away":{"chat":4,"asking":"steward","target":"devops"}}"#,
        r#"{"dispatch_away":{"chat":4}}"#,
        r#"{"refused":[{"asking":"steward","target":"devops","first":1,"latest":1,"times":1}]}"#,
        r#"{"grant":{"chat":4,"what":"dispatch","target":"steward -> devops","level":"project"}}"#,
        // #1505: a grant limited to one workspace. The wider Allow, the change of a
        // grant's workspace, Accept and Not on my machine for a limited grant, by each
        // window command's name; and the keys it keeps.
        r#"{"allow_dispatch_anywhere":{"chat":4,"id":1,"level":"project"}}"#,
        r#"{"chat":4,"allow_dispatch_anywhere":{"id":1,"level":"you"},"token":"t"}"#,
        r#"{"set_dispatch_workspace":{"chat":4,"asking":"steward","target":"devops","level":"you","from":"runners","to":null}}"#,
        r#"{"chat":4,"set_dispatch_workspace":{"asking":"steward","target":"*","level":"project","from":null,"to":"runners"},"token":"t"}"#,
        r#"{"accept_project_dispatch_in":{"chat":4,"asking":"steward","target":"devops","workspace":"runners"}}"#,
        r#"{"decline_project_dispatch_in":{"chat":4,"asking":"steward","target":"devops","workspace":"runners"}}"#,
        r#"{"dispatch_mine_in":[{"asking":"steward","target":"devops","workspace":"runners","seen":0}]}"#,
        r#"{"dispatch_seen_in":[{"asking":"steward","target":"*","any":true,"workspace":"runners","seen":0}]}"#,
        r#"{"dispatch_workspaces":[{"name":"runners","gone":0}]}"#,
        // Train 64, where the tickets meet: the wider Allow with boxes ticked, an Allow on
        // a refusal kept while nobody was there with the workspace it is limited to, and
        // the window-only list's own name.
        r#"{"allow_dispatch_anywhere":{"chat":4,"id":1,"level":"you","also":["qa"],"shown":"0f"}}"#,
        r#"{"chat":4,"allow_dispatch_away":{"asking":"steward","target":"devops","workspace":"runners"},"token":"t"}"#,
        r#"{"allow_dispatch_away":{"chat":4,"asking":"steward","target":"devops","workspace":null}}"#,
        r#"{"never_dispatch_away":{"chat":4,"asking":"steward","target":"devops","workspace":"runners"}}"#,
        r#"{"window_only":["allow_dispatch"]}"#,
    ];
    for line in forged {
        assert!(read_line(line).is_none(), "read as a line: {line}");
    }
    // Riding on a line the listener does read, the forged fields are dropped with every
    // other field that line does not have: what is read is that line and nothing more.
    let riding = r#"{"chat":4,"event":"stop","dispatch_grant":{"asking":"steward","target":"devops","level":"project"},"allow_dispatch":{"id":1,"level":"project"}}"#;
    let (read, _) = read_line(riding).expect("a report");
    let Line::Report(report) = read else {
        panic!("not a report: {read:?}");
    };
    let kept = serde_json::to_string(&report).expect("written");
    assert!(!kept.contains("dispatch"), "{kept}");
    assert!(!kept.contains("devops"), "{kept}");
    // And an ask names a chat to open or a report to hand back, never a grant: no variant
    // of it reads from a line that only spells one.
    for line in forged {
        assert!(serde_json::from_str::<Ask>(line).is_err(), "an ask: {line}");
    }
}

#[test]
fn a_brokered_secret_exec_is_its_own_line_and_names_the_chat_its_token_is_checked_for() {
    let ask = crate::secrets::brokered::Ask {
        chat: 9,
        secret_exec: crate::secrets::brokered::Wanted {
            vault: "devops".to_owned(),
            command: vec!["kubectl".to_owned(), "get".to_owned(), "pods".to_owned()],
            ..Default::default()
        },
        ..Default::default()
    };
    let line = line_with(Some(&ChatToken::from("t")), &ask).unwrap();
    let (read, token) = read_line(std::str::from_utf8(&line).unwrap()).expect("a line");
    assert!(
        matches!(&read, Line::SecretExec(got) if *got == ask),
        "{read:?}"
    );
    assert_eq!(read.chat(), 9);
    assert_eq!(token.as_deref(), Some("t"));
    for other in [
        serde_json::to_string(&saved()).unwrap(),
        r#"{"chat":4,"event":"stop"}"#.to_owned(),
        r#"{"session_record":{"chat":4,"title":"t","body":"b"}}"#.to_owned(),
    ] {
        assert!(
            !matches!(
                serde_json::from_str::<Line>(&other),
                Ok(Line::SecretExec(_))
            ),
            "no other line reads as a brokered exec: {other}"
        );
    }
}

#[test]
fn a_tool_call_is_its_own_line_and_is_handed_to_the_app_with_its_chats_token() {
    let call = ToolCall {
        chat: 4,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some("toolu_01".to_owned()),
        args: Some("ab".repeat(32)),
        decision: Decision::Deny,
        rule: Some("no-force-push".to_owned()),
        hook_ms: 2,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    let line = serde_json::to_string(&call).unwrap();
    assert!(
        matches!(serde_json::from_str::<Line>(&line), Ok(Line::Tool(_))),
        "{line}"
    );
    for other in [
        serde_json::to_string(&saved()).unwrap(),
        r#"{"chat":4,"event":"stop"}"#.to_owned(),
    ] {
        assert!(
            !matches!(serde_json::from_str::<Line>(&other), Ok(Line::Tool(_))),
            "no older line reads as a tool call: {other}"
        );
    }

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.hear(Hearing {
        serve: crate::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        each: Box::new(|_| panic!("no report was sent")),
        answer: Box::new(|_, _| panic!("no ask was sent")),
        noticed: Box::new(|_| panic!("no harness was started by hand")),
        saved: Box::new(|_| panic!("no record was saved")),
        refused: Box::new(|_| panic!("no commit was refused")),
        tool: Box::new(move |call| {
            tx.lock().unwrap().send(call).unwrap();
            Ok(())
        }),
        permission: Box::new(|_| None),
    });
    one_line_with_a_deadline(&path, None, &call).expect("the line is written");
    one_line_with_a_deadline(&path, Some(&token), &call).expect("the line is written");
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(call));
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the line without the chat's token was dropped"
    );
}

#[test]
fn a_touched_path_is_its_own_line_handed_to_the_app_with_its_chats_token() {
    let touching = Touching {
        chat: 4,
        touching: "/w/branch/src/a.rs".to_owned(),
        wrote: false,
    };
    let line = serde_json::to_string(&touching).unwrap();
    assert!(
        matches!(serde_json::from_str::<Line>(&line), Ok(Line::Touching(_))),
        "{line}"
    );
    for other in [
        serde_json::to_string(&saved()).unwrap(),
        r#"{"chat":4,"event":"stop"}"#.to_owned(),
        r#"{"chat":4,"tool_hook":"pretooluse-read"}"#.to_owned(),
    ] {
        assert!(
            !matches!(serde_json::from_str::<Line>(&other), Ok(Line::Touching(_))),
            "no other line reads as a touch: {other}"
        );
    }

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.hear(Hearing {
        serve: crate::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        each: Box::new(|_| panic!("no report was sent")),
        answer: Box::new(|_, _| panic!("no ask was sent")),
        noticed: Box::new(|_| panic!("no harness was started by hand")),
        saved: Box::new(|_| panic!("no record was saved")),
        refused: Box::new(|_| panic!("no commit was refused")),
        tool: Box::new(|_| panic!("no tool call was sent")),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(move |touching| tx.lock().unwrap().send(touching).unwrap()),
        permission: Box::new(|_| None),
    });
    touch(&path, None, &touching).expect("the line is written");
    touch(&path, Some(&token), &touching).expect("the line is written");
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(touching.clone())
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the line without the chat's token was dropped"
    );
}

#[test]
fn what_a_turn_ended_saying_is_its_own_line_and_no_other_line_reads_as_it() {
    // #1981: the last message a task's turn ended on, for a report purlis makes from it.
    let output = TurnOutput {
        chat: 4,
        turn_output: Output::last_message("  Done: 3 files.\n").expect("a message"),
        speaker: Speaker::default(),
    };
    let line = serde_json::to_string(&output).unwrap();
    assert_eq!(
        line,
        r#"{"chat":4,"turn_output":{"is":"final","text":"Done: 3 files."}}"#
    );
    assert!(
        matches!(serde_json::from_str::<Line>(&line), Ok(Line::Output(read)) if read == output),
        "{line}"
    );
    for other in [
        r#"{"chat":4,"event":"stop"}"#.to_owned(),
        r#"{"chat":4,"doing":{"is":"began","kind":"command","name":"cargo"}}"#.to_owned(),
        // A kind this build does not know is no line at all.
        r#"{"chat":4,"turn_output":{"is":"screen","text":"x"}}"#.to_owned(),
        r#"{"chat":4,"turn_output":"x"}"#.to_owned(),
    ] {
        assert!(
            !matches!(serde_json::from_str::<Line>(&other), Ok(Line::Output(_))),
            "no other line reads as this one: {other}"
        );
    }
}

#[test]
fn a_stop_s_last_message_is_read_from_the_payload_the_harness_hands_its_hook() {
    assert_eq!(
        Output::of_stop(CLAUDE_STOP),
        Some(Output::Final {
            text: "pong".to_owned()
        })
    );
    for none in [
        r#"{"hook_event_name":"Stop"}"#,
        r#"{"last_assistant_message":null}"#,
        r#"{"last_assistant_message":"   "}"#,
        "not json",
    ] {
        assert_eq!(Output::of_stop(none), None, "{none}");
    }
}

#[test]
fn a_command_s_end_is_read_with_the_status_each_harness_says() {
    let claude = |word: &str, more: serde_json::Value| {
        let mut payload = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test -p core"},
        });
        for (key, value) in more.as_object().unwrap() {
            payload[key] = value.clone();
        }
        Output::of_ran(word, &payload, Some("claude-code"))
    };
    let ran = |exit: Option<i32>, failed: bool| {
        Some(Output::Ran {
            command: "cargo test -p core".to_owned(),
            exit,
            failed,
        })
    };
    // Claude Code: its post hook fires for a command that succeeded.
    assert_eq!(
        claude("posttooluse-blocked", serde_json::json!({})),
        ran(Some(0), false)
    );
    // And its failure hook for one that did not, with the status in its error.
    assert_eq!(
        claude(
            "posttoolusefailure-blocked",
            serde_json::json!({"error": "Exit code 101\nerror: test failed"})
        ),
        ran(Some(101), true)
    );
    assert_eq!(
        claude(
            "posttoolusefailure-blocked",
            serde_json::json!({"error": "Interrupted"})
        ),
        ran(None, true)
    );
    // A command left running in the background has no status yet.
    let background = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "sleep 60", "run_in_background": true},
    });
    assert_eq!(
        Output::of_ran("posttooluse-blocked", &background, Some("claude-code")),
        Some(Output::Ran {
            command: "sleep 60".to_owned(),
            exit: None,
            failed: false
        })
    );
    // opencode: purlis's plugin sends the status.
    let opencode = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "ls"},
        "exit_code": 2,
    });
    assert_eq!(
        Output::of_ran("posttooluse-blocked", &opencode, Some("opencode")),
        Some(Output::Ran {
            command: "ls".to_owned(),
            exit: Some(2),
            failed: true
        })
    );
    // Codex: its shell's words, and no status it says.
    let codex = serde_json::json!({
        "tool_name": "shell",
        "tool_input": {"command": ["bash", "-lc", "make"]},
    });
    assert_eq!(
        Output::of_ran("posttooluse-blocked", &codex, Some("codex")),
        Some(Output::Ran {
            command: "bash -lc make".to_owned(),
            exit: None,
            failed: false
        })
    );
    // Not a command's end: nothing.
    assert_eq!(claude("pretooluse", serde_json::json!({})), None);
    let edit = serde_json::json!({"tool_name": "Edit", "tool_input": {"command": "x"}});
    assert_eq!(
        Output::of_ran("posttooluse", &edit, Some("claude-code")),
        None
    );
}

#[test]
fn a_turn_s_last_message_is_cut_on_a_character_and_an_empty_one_is_none() {
    assert_eq!(Output::last_message(" \n\t"), None);
    let long = "é".repeat(Output::MOST_BYTES);
    let Some(Output::Final { text }) = Output::last_message(&long) else {
        panic!("a message");
    };
    assert!(text.len() <= Output::MOST_BYTES);
    assert!(text.chars().all(|c| c == 'é'));
}

#[test]
fn what_a_chat_is_doing_is_its_own_line_handed_to_the_app_with_its_chats_token() {
    use crate::doing::{Kind, Said};
    let doing = Doing {
        chat: 4,
        doing: Said::Began {
            kind: Kind::Command,
            name: Some("cargo".to_owned()),
        },
        agent: None,
        speaker: Speaker::default(),
    };
    let line = serde_json::to_string(&doing).unwrap();
    assert_eq!(
        line,
        r#"{"chat":4,"doing":{"is":"began","kind":"command","name":"cargo"}}"#
    );
    assert!(
        matches!(serde_json::from_str::<Line>(&line), Ok(Line::Doing(_))),
        "{line}"
    );
    // #1601: the run it came from rides along where the hook read one.
    let spoken = Doing {
        speaker: Speaker {
            pid: Some(4242),
            conversation: Conversation::Named("c".to_owned()),
        },
        ..doing.clone()
    };
    let line = serde_json::to_string(&spoken).unwrap();
    assert!(
        matches!(serde_json::from_str::<Line>(&line), Ok(Line::Doing(read)) if read == spoken),
        "{line}"
    );
    for other in [
        serde_json::to_string(&saved()).unwrap(),
        r#"{"chat":4,"event":"stop"}"#.to_owned(),
        r#"{"chat":4,"tool_hook":"pretooluse"}"#.to_owned(),
        r#"{"chat":4,"touching":"/w/a"}"#.to_owned(),
        // A kind this build does not know is no line at all.
        r#"{"chat":4,"doing":{"is":"began","kind":"notice"}}"#.to_owned(),
        r#"{"chat":4,"doing":"needs you"}"#.to_owned(),
    ] {
        assert!(
            !matches!(serde_json::from_str::<Line>(&other), Ok(Line::Doing(_))),
            "no other line reads as this one: {other}"
        );
    }

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let other = listener.tokens().issue_to_this_process(5).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.hear(Hearing {
        serve: crate::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        each: Box::new(|_| panic!("no report was sent")),
        answer: Box::new(|_, _| panic!("no ask was sent")),
        noticed: Box::new(|_| panic!("no harness was started by hand")),
        saved: Box::new(|_| panic!("no record was saved")),
        refused: Box::new(|_| panic!("no commit was refused")),
        tool: Box::new(|_| panic!("no tool call was sent")),
        touching: Box::new(|_| panic!("no file was touched")),
        blocked: Box::new(|_| panic!("nothing was blocked")),
        doing: Box::new(move |doing| tx.lock().unwrap().send(doing).unwrap()),
        output: Box::new(|_| panic!("no turn's output was sent")),
        permission: Box::new(|_| None),
    });
    // Forged, with every field set: no token, a token nobody was issued, another chat's
    // token, and this chat's own token on a line that names another chat.
    let of_another = Doing {
        chat: 5,
        agent: Some("agent-1".to_owned()),
        ..doing.clone()
    };
    tell_doing(&path, None, &doing).expect("the line is written");
    tell_doing(&path, Some(&ChatToken::from("not-a-token")), &doing).expect("the line is written");
    tell_doing(&path, Some(&other), &doing).expect("the line is written");
    tell_doing(&path, Some(&token), &of_another).expect("the line is written");
    assert!(
        rx.recv_timeout(std::time::Duration::from_secs(1)).is_err(),
        "a line without its own chat's token was dropped"
    );
    tell_doing(&path, Some(&token), &doing).expect("the line is written");
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(doing.clone())
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "and it was heard once"
    );
}

#[test]
fn what_a_chat_is_doing_is_lost_when_no_app_takes_it_and_never_spooled() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let doing = Doing {
        chat: 4,
        doing: crate::doing::Said::Began {
            kind: crate::doing::Kind::Editing,
            name: Some("CANARY-doing.rs".to_owned()),
        },
        agent: None,
        speaker: Speaker::default(),
    };
    assert!(tell_doing(&path, Some(&ChatToken::from("t")), &doing).is_err());
    let written = walkdir_all(dir.path());
    assert!(written.is_empty(), "nothing was written: {written:?}");
}

#[test]
fn a_sandbox_block_is_its_own_line_handed_to_the_app_with_its_chats_token() {
    use crate::sandboxblock::{Block, Kind, Operation};
    let blocked = SandboxBlocked {
        chat: 4,
        sandbox_blocked: Block {
            operation: Operation::Write,
            kind: Kind::ProjectFiles,
            ours: true,
        },
        harness: Some("claude".to_owned()),
        target: None,
        read_back: false,
        suggested: None,
    };
    let line = serde_json::to_string(&blocked).unwrap();
    assert_eq!(
        line,
        r#"{"chat":4,"sandbox_blocked":{"operation":"write","kind":"project-files","ours":true},"harness":"claude"}"#
    );
    assert!(
        matches!(serde_json::from_str::<Line>(&line), Ok(Line::Blocked(_))),
        "{line}"
    );
    for other in [
        serde_json::to_string(&saved()).unwrap(),
        r#"{"chat":4,"event":"stop"}"#.to_owned(),
        r#"{"chat":4,"tool_hook":"posttooluse-blocked"}"#.to_owned(),
        r#"{"chat":4,"touching":"/w/a"}"#.to_owned(),
    ] {
        assert!(
            !matches!(serde_json::from_str::<Line>(&other), Ok(Line::Blocked(_))),
            "no other line reads as a block: {other}"
        );
    }

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.hear(Hearing {
        serve: crate::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        each: Box::new(|_| panic!("no report was sent")),
        answer: Box::new(|_, _| panic!("no ask was sent")),
        noticed: Box::new(|_| panic!("no harness was started by hand")),
        saved: Box::new(|_| panic!("no record was saved")),
        refused: Box::new(|_| panic!("no commit was refused")),
        tool: Box::new(|_| panic!("no tool call was sent")),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| panic!("no file was touched")),
        blocked: Box::new(move |blocked| tx.lock().unwrap().send(blocked).unwrap()),
        permission: Box::new(|_| None),
    });
    tell_blocked(&path, None, &blocked).expect("the line is written");
    tell_blocked(&path, Some(&token), &blocked).expect("the line is written");
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(blocked.clone())
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the line without the chat's token was dropped"
    );
}

#[test]
fn a_touch_no_app_takes_is_lost_and_never_spooled() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let touching = Touching {
        chat: 4,
        touching: "/w/branch/CANARY-touched.rs".to_owned(),
        wrote: false,
    };
    assert!(touch(&path, Some(&ChatToken::from("t")), &touching).is_err());
    let mut written = Vec::new();
    for entry in walkdir_all(dir.path()) {
        written.push(entry.display().to_string());
        let text = std::fs::read(&entry).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&text).contains("CANARY-touched"),
            "{} holds the touched path",
            entry.display()
        );
    }
    assert!(written.is_empty(), "nothing was written: {written:?}");
}

fn walkdir_all(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir_all(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn a_refused_commit_is_its_own_line_and_is_handed_to_the_app_with_its_chats_token() {
    let refused = CommitRefused {
        chat: 4,
        commit_refused: "app: app.py:2  an email address  ad****".to_owned(),
    };
    let line = serde_json::to_string(&refused).unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&line),
        Ok(Line::Refused(_))
    ));
    let line = serde_json::to_string(&saved()).unwrap();
    assert!(matches!(
        serde_json::from_str::<Line>(&line),
        Ok(Line::Saved(_))
    ));

    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(4).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.hear(Hearing {
        serve: crate::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        each: Box::new(|_| panic!("no report was sent")),
        answer: Box::new(|_, _| panic!("no ask was sent")),
        noticed: Box::new(|_| panic!("no harness was started by hand")),
        saved: Box::new(|_| panic!("no record was saved")),
        refused: Box::new(move |refused| {
            tx.lock().unwrap().send(refused).unwrap();
            Ok(())
        }),
        tool: Box::new(|_| panic!("no tool hook ran")),
        permission: Box::new(|_| None),
    });
    // Without the token it is dropped, as every line is.
    one_line_with_a_deadline(&path, None, &refused).expect("the line is written");
    one_line_with_a_deadline(&path, Some(&token), &refused).expect("the line is written");
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(refused)
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err()
    );
}

#[test]
fn an_app_that_hears_no_session_saved_still_takes_every_other_line() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let saving = listener.tokens().issue_to_this_process(4).expect("a token");
    let reporting = listener.tokens().issue_to_this_process(9).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    // What the app calls today: no listener for a saved record at all.
    let _reading = listener.each_answering_and_noticing(
        Box::new(move |report| tx.lock().unwrap().send(report.chat).unwrap()),
        Box::new(|_, _| panic!("no ask was sent")),
        Box::new(|_| panic!("no harness was started by hand")),
    );

    tell_saved(&path, Some(&saving), &saved()).expect("the app took it");
    send(
        &path,
        Some(&reporting),
        &Report {
            chat: 9,
            event: Event::Stop,
            conversation: Conversation::Unknown,
            pid: None,
            agent: None,
            detail: Detail::default(),
        },
    )
    .expect("the app took it");

    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(9));
}

#[test]
fn telling_a_session_saved_where_no_app_is_listening_fails_at_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let started = std::time::Instant::now();

    assert!(tell_saved(&dir.path().join("gone.sock"), None, &saved()).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

// ------------------------------------------------------------------------------------
// each chat's token: the channel checks it on every line
// ------------------------------------------------------------------------------------

fn a_stop(chat: u32) -> Report {
    Report {
        chat,
        event: Event::Stop,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    }
}

#[test]
fn a_line_carrying_its_chats_token_is_read_and_the_token_goes_no_further() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));

    send(&path, Some(&token), &a_stop(7)).expect("the app took it");

    let heard = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the report arrives");
    assert_eq!(heard, a_stop(7));
    // What the app is handed holds no token, however it is written out.
    let written = serde_json::to_string(&heard).unwrap();
    assert!(!written.contains(token.expose()), "{written}");
    assert!(!format!("{heard:?}").contains(token.expose()));
}

#[test]
fn a_report_with_no_token_a_wrong_one_or_another_chats_is_dropped() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let seven = listener.tokens().issue_to_this_process(7).expect("a token");
    let eight = listener.tokens().issue_to_this_process(8).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.chat);
    }));

    let wrong = ChatToken::from("0".repeat(seven.expose().len()).as_str());
    for token in [None, Some(&wrong), Some(&eight)] {
        send(&path, token, &a_stop(7)).expect("written");
    }
    // And one line with the token of a chat that has none of its own.
    send(&path, Some(&seven), &a_stop(9)).expect("written");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(2)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "a line without its chat's token was read"
    );
    // The channel is still open to the chat's own.
    send(&path, Some(&eight), &a_stop(8)).expect("written");
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(8));
}

#[test]
fn an_ask_without_its_chats_token_is_refused_unanswered_and_ends_at_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let three = listener.tokens().issue_to_this_process(3).expect("a token");
    let four = listener.tokens().issue_to_this_process(4).expect("a token");
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Answer::Ticket {
                ticket: "t".to_owned(),
            }
        }),
    );
    let within = std::time::Duration::from_secs(5);

    for token in [None, Some(ChatToken::from("nope")), Some(four)] {
        let began = std::time::Instant::now();
        let answered = Asking::on(&path, token)
            .expect("connected")
            .ask(&Ask::Ticket { chat: 3 }, within);
        // Refused, and said so (#1333): never answered as the chat it names.
        assert!(
            matches!(&answered, Ok(Answer::No { why }) if why.contains("token")),
            "answered {answered:?}"
        );
        assert!(began.elapsed() < within / 2, "it waited for a deadline");
    }
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);

    let answered = Asking::on(&path, Some(three))
        .expect("connected")
        .ask(&Ask::Ticket { chat: 3 }, within);
    assert!(
        matches!(answered, Ok(Answer::Ticket { .. })),
        "{answered:?}"
    );
}

#[test]
fn a_notice_or_a_saved_record_without_its_chats_token_is_dropped() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let four = listener.tokens().issue_to_this_process(4).expect("a token");
    let other = listener.tokens().issue_to_this_process(5).expect("a token");
    let (tx, rx) = mpsc::channel();
    let noticed = std::sync::Mutex::new(tx.clone());
    let saving = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering_noticing_and_saving(
        Box::new(|_| {}),
        Box::new(|_, _| Answer::No { why: String::new() }),
        Box::new(move |_| noticed.lock().unwrap().send("notice").unwrap()),
        Box::new(move |_| saving.lock().unwrap().send("saved").unwrap()),
    );

    for token in [None, Some(&other)] {
        tell(&path, token, &by_hand()).expect("written");
        tell_saved(&path, token, &saved()).expect("written");
    }
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(2)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    tell(&path, Some(&four), &by_hand()).expect("written");
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok("notice")
    );
}

#[test]
fn a_connection_from_another_uid_is_refused_before_it_says_anything_even_with_its_token() {
    // FD-6: the hook channel checks the peer's uid as well as the chat's token. The
    // listener is told it belongs to a uid this test is not, so this test's own
    // connection is the other user's: it carries the right token, and is still dropped.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let ours = purlis_same_user::Uid::effective();
    let listener = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .owned_by(purlis_same_user::Uid::from_raw(
            ours.as_raw().wrapping_add(1),
        ));
    let seven = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.chat);
    }));

    let _ = send(&path, Some(&seven), &a_stop(7));

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(2)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "a line from another uid was read"
    );
}

#[test]
fn refusals_are_said_once_per_interval_and_the_rest_are_counted() {
    let refusals = RateLimited::new();
    assert_eq!(refusals.say().as_deref(), Some(""));
    for _ in 0..1000 {
        assert_eq!(refusals.say(), None);
    }
    // Past the interval the next one is said, with the count of the ones that were not.
    refusals.state.lock().unwrap().0 =
        std::time::Instant::now().checked_sub(A_REFUSAL_IS_SAID_EVERY);
    assert_eq!(
        refusals.say().as_deref(),
        Some(" (1000 more refused since the last line)")
    );
}

#[test]
fn a_connection_from_the_listeners_own_uid_with_its_token_is_read() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .owned_by(purlis_same_user::Uid::effective());
    let seven = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.chat);
    }));

    send(&path, Some(&seven), &a_stop(7)).expect("the app took it");

    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(7));
}

fn program(pid: u32) -> Program {
    Program {
        pid,
        started: "then".to_owned(),
    }
}

#[test]
fn a_line_with_a_chats_token_is_read_only_from_inside_that_chat() {
    let tokens = ChatTokens::default();
    let token = tokens.issue(5).expect("a token");
    let inside = |peer: &u32, program: &Program| *peer == program.pid || *peer == 101;
    let of = |peer| tokens.admission_by(5, Some(token.expose()), peer, &inside);
    assert_eq!(
        of(Some(100)),
        Admission::Unbound,
        "no program is recorded for the chat"
    );
    tokens.bind(5, program(100));
    assert_eq!(of(Some(100)), Admission::Admitted);
    assert_eq!(
        of(Some(101)),
        Admission::Admitted,
        "a process inside the chat"
    );
    assert_eq!(
        of(Some(200)),
        Admission::Outside,
        "the chat's token, read by a process outside it"
    );
    assert_eq!(of(None), Admission::Outside);
    assert_eq!(
        tokens.admission_by(5, Some("not-it"), Some(100), &inside),
        Admission::NoToken,
        "a wrong token is dropped, not answered"
    );
    tokens.issue(5).expect("a new token");
    assert_eq!(
        tokens.admission_by(5, Some(token.expose()), Some(100), &inside),
        Admission::NoToken,
        "a new start's token is its own"
    );
}

#[test]
fn a_refusal_names_its_own_cause() {
    assert_eq!(Admission::Outside.refusal(), Some(OUTSIDE_THE_CHAT));
    assert_eq!(
        Admission::Unbound.refusal(),
        Some(UNCONFIRMED_PROGRAM),
        "an unconfirmed program is the chat's start, not tmux or nohup"
    );
    assert!(!UNCONFIRMED_PROGRAM.contains("tmux"));
    assert_eq!(
        Admission::NoToken.refusal(),
        None,
        "a wrong token gets silence"
    );
    assert_eq!(Admission::Admitted.refusal(), None);
}

fn seen(pid: u32, started: &str, around: &[(u32, Option<&str>)]) -> Seen {
    Seen {
        pid,
        started: started.to_owned(),
        around: around
            .iter()
            .map(|(pid, started)| (*pid, started.map(str::to_owned)))
            .collect(),
    }
}

#[test]
fn a_sender_is_judged_by_what_it_was_when_it_connected() {
    let program = program(100);
    assert!(
        seen(100, "then", &[]).inside(&program),
        "the program itself"
    );
    assert!(
        seen(205, "now", &[(100, Some("then"))]).inside(&program),
        "below it, or in its session"
    );
    assert!(
        !seen(205, "now", &[(100, Some("before"))]).inside(&program),
        "below a process that had the program's number at another time"
    );
    assert!(
        !seen(205, "now", &[(100, None)]).inside(&program),
        "below a process whose start could not be read"
    );
    assert!(
        !seen(100, "another time", &[]).inside(&program),
        "the program's number, started at another time"
    );
}

/// Run by [`a_sender_that_writes_and_exits_at_once_is_still_heard`] in a process of its
/// own, which tells the app and then ends, as `purlis session save` and a shell's guard do.
#[test]
#[ignore = "run in a child by a_sender_that_writes_and_exits_at_once_is_still_heard"]
fn a_sender_that_writes_and_exits() {
    let (Some(path), Some(token)) = (
        crate::envvar::var_os("PURLIS_TEST_HOOK_SOCKET"),
        crate::envvar::var("PURLIS_TEST_HOOK_TOKEN"),
    ) else {
        return;
    };
    let notice = StartedByHand {
        chat: 12,
        started_by_hand: "claude".to_owned(),
        cwd: None,
    };
    let token = ChatToken::from(token.as_str());
    if crate::envvar::var("PURLIS_TEST_HOOK_RAW").is_some() {
        // A sender that does not wait for the app: its line written, its connection
        // closed, and it stays a moment longer only as any process would on its way out.
        use std::io::Write;
        let closed = crate::envvar::var_os("PURLIS_TEST_HOOK_CLOSED");
        if let Ok(mut socket) = std::os::unix::net::UnixStream::connect(&path)
            && let Ok(line) = line_with(Some(&token), &notice)
        {
            let written = socket.write_all(&line);
            // Asked to say it has closed, it must have sent its line first: else a refusal
            // would be a line never sent.
            if closed.is_some() {
                written.expect("the line, written");
            }
        } else {
            assert!(closed.is_none(), "the line, sent");
        }
        // Asked to, it says so once its connection is closed, and stays until it is ended.
        if let Some(closed) = closed {
            std::fs::write(closed, b"").expect("said");
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        return;
    }
    let _ = tell(std::path::Path::new(&path), Some(&token), &notice);
}

#[test]
fn a_sender_that_writes_and_exits_at_once_is_still_heard() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener
        .tokens()
        .issue_to_this_process(12)
        .expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering_and_noticing(
        Box::new(|_| panic!("a notice is not a report")),
        Box::new(|_, _| panic!("a notice is not an ask")),
        Box::new(move |notice| tx.lock().unwrap().send(notice).unwrap()),
    );
    let ran = crate::forklock::output(
        std::process::Command::new(std::env::current_exe().expect("this test binary"))
            .args([
                "--exact",
                "hookwire::tests::a_sender_that_writes_and_exits",
                "--ignored",
                "--quiet",
            ])
            .env("PURLIS_TEST_HOOK_SOCKET", &path)
            .env("PURLIS_TEST_HOOK_TOKEN", token.expose()),
    )
    .expect("the sender runs");
    assert!(ran.status.success(), "{ran:?}");
    let heard = rx.recv_timeout(std::time::Duration::from_secs(5));
    assert_eq!(
        heard.map(|notice| notice.chat),
        Ok(12),
        "the line of a sender that has already exited"
    );
}

/// Linux names a peer (`SO_PEERCRED`) as it connected, even after it has closed, so a
/// sender that does not wait is still heard there. macOS names it only while it is
/// connected, and a sender it cannot name is refused, failing closed (operator ruling
/// 2026-10-17 on #2465; every purlis sender waits, D-1407-10): see
/// `a_sender_that_does_not_wait_is_refused_on_macos_when_it_closed_before_it_was_named`.
#[test]
#[cfg(not(target_os = "macos"))]
fn a_sender_that_does_not_wait_is_heard_when_it_is_still_running_as_its_line_is_read() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener
        .tokens()
        .issue_to_this_process(12)
        .expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering_and_noticing(
        Box::new(|_| panic!("a notice is not a report")),
        Box::new(|_, _| panic!("a notice is not an ask")),
        Box::new(move |notice| tx.lock().unwrap().send(notice).unwrap()),
    );
    let ran = crate::forklock::output(
        std::process::Command::new(std::env::current_exe().expect("this test binary"))
            .args([
                "--exact",
                "hookwire::tests::a_sender_that_writes_and_exits",
                "--ignored",
                "--quiet",
            ])
            .env("PURLIS_TEST_HOOK_SOCKET", &path)
            .env("PURLIS_TEST_HOOK_TOKEN", token.expose())
            .env("PURLIS_TEST_HOOK_RAW", "1"),
    )
    .expect("the sender runs");
    assert!(ran.status.success(), "{ran:?}");
    let heard = rx.recv_timeout(std::time::Duration::from_secs(5));
    assert_eq!(
        heard.map(|notice| notice.chat),
        Ok(12),
        "the line of a sender that did not wait, read while it still ran"
    );
}

/// **A sender the listener cannot name is refused, failing closed** (operator ruling
/// 2026-10-17 on #2465). On macOS the kernel names a peer (`LOCAL_PEERPID`) only while its
/// connection is open, so a sender that closed before the app took its connection is
/// refused unread, though it is still running. Every purlis sender holds its connection
/// until the app lets it go (D-1407-10), so this refuses only a sender written by hand.
#[test]
#[cfg(target_os = "macos")]
fn a_sender_that_does_not_wait_is_refused_on_macos_when_it_closed_before_it_was_named() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let closed = dir.path().join("closed");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener
        .tokens()
        .issue_to_this_process(12)
        .expect("a token");
    // The sender connects, writes and closes before anything takes its connection: the
    // channel is read only once it says it has closed.
    let mut sender = crate::forklock::spawn(
        std::process::Command::new(std::env::current_exe().expect("this test binary"))
            .args([
                "--exact",
                "hookwire::tests::a_sender_that_writes_and_exits",
                "--ignored",
                "--quiet",
            ])
            .env("PURLIS_TEST_HOOK_SOCKET", &path)
            .env("PURLIS_TEST_HOOK_TOKEN", token.expose())
            .env("PURLIS_TEST_HOOK_RAW", "1")
            .env("PURLIS_TEST_HOOK_CLOSED", &closed)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()),
    )
    .expect("the sender runs");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !closed.exists() {
        assert!(
            std::time::Instant::now() < until,
            "the sender says it has closed"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let _reading = listener.each_answering_and_noticing(
        Box::new(|_| panic!("a notice is not a report")),
        Box::new(|_, _| panic!("a notice is not an ask")),
        Box::new(move |notice| tx.lock().unwrap().send(notice.started_by_hand).unwrap()),
    );
    // A sender that waits, after it: the channel is reading, and hears that one alone.
    tell(
        &path,
        Some(&token),
        &StartedByHand {
            chat: 12,
            started_by_hand: "a sender that waits".to_owned(),
            cwd: None,
        },
    )
    .expect("the app took it");
    let heard = rx.recv_timeout(std::time::Duration::from_secs(5));
    let after = rx.recv_timeout(std::time::Duration::from_secs(1));
    let running = sender.try_wait().expect("the sender, read").is_none();
    let _ = sender.kill();
    let _ = sender.wait();

    assert!(running, "the sender still ran as its connection was taken");
    assert_eq!(heard.as_deref(), Ok("a sender that waits"));
    assert_eq!(
        after,
        Err(mpsc::RecvTimeoutError::Timeout),
        "the line of a sender the app could not name"
    );
}

#[test]
fn a_chat_whose_program_ended_speaks_for_nothing_and_a_late_end_changes_nothing() {
    let tokens = ChatTokens::default();
    let inside = |peer: &u32, program: &Program| *peer == program.pid;
    let first = tokens.issue(5).expect("a token");
    tokens.bind(5, program(100));
    tokens.program_ended(5, 100);
    assert_eq!(
        tokens.admission_by(5, Some(first.expose()), Some(100), &inside),
        Admission::NoToken,
        "a process later given the program's number"
    );
    // Started again under the same number: the first program's end, heard late, is not
    // the second's.
    let second = tokens.issue(5).expect("a token");
    tokens.bind(5, program(300));
    tokens.program_ended(5, 100);
    assert_eq!(
        tokens.admission_by(5, Some(second.expose()), Some(300), &inside),
        Admission::Admitted
    );
}

#[test]
fn a_process_is_inside_the_chat_it_descends_from_and_not_one_beside_it() {
    let me = Program::of(std::process::id()).expect("this process, read");
    let mut child = crate::forklock::spawn(
        std::process::Command::new("/bin/sleep")
            .arg("5")
            .stdin(std::process::Stdio::null()),
    )
    .expect("a child");
    let them = Program::of(child.id()).expect("the child, read");
    assert!(descends(me.pid, &me));
    assert!(
        descends(child.id(), &me),
        "a child of this process is inside it"
    );
    assert!(
        !descends(me.pid, &them),
        "this process is not inside its own child"
    );
    let reused = Program {
        pid: me.pid,
        started: "a process that had this number before".to_owned(),
    };
    assert!(
        !descends(me.pid, &reused),
        "the same number, started at another time, is another process"
    );
    let _ = child.kill();
    let _ = child.wait();
}

/// A chat whose program is a process this one is not inside: a sleeping child.
fn a_chat_elsewhere(tokens: &ChatTokens, chat: u32) -> (ChatToken, std::process::Child) {
    let child = crate::forklock::spawn(
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .stdin(std::process::Stdio::null()),
    )
    .expect("a child");
    let token = tokens.issue(chat).expect("a token");
    tokens.bind(chat, Program::of(child.id()).expect("the child, read"));
    (token, child)
}

#[test]
fn a_report_with_its_chats_token_from_outside_the_chat_is_refused_with_why_and_not_spooled() {
    let dir = tempfile::tempdir().expect("a directory");
    // Where a project's socket is, so a line the host did not take would be spooled.
    let app = dir.path().join(".purlis/app");
    std::fs::create_dir_all(&app).expect("the state folder");
    let path = app.join("hooks.sock");
    assert!(
        spool::covered(&spool::dir_for(&path)),
        "a spool here is one hooks write"
    );
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let (token, mut child) = a_chat_elsewhere(&listener.tokens(), 7);
    let _reading = listener.each(Box::new(|_| panic!("no report is read")));
    let report = Report {
        chat: 7,
        event: Event::Notification,
        conversation: Conversation::Named("abc".to_owned()),
        pid: Some(99),
        agent: None,
        detail: Detail::default(),
    };
    let refused = deliver_report(&path, Some(&token), &report).expect_err("refused");
    assert_eq!(refused.to_string(), OUTSIDE_THE_CHAT);
    assert!(
        !spool::dir_for(&path).join("7").exists(),
        "nothing spooled as if the app were away"
    );
    let mut asking = Asking::on(&path, Some(token.clone())).expect("connects");
    assert_eq!(
        asking
            .ask(&Ask::Ticket { chat: 7 }, std::time::Duration::from_secs(5))
            .expect("an answer"),
        Answer::No {
            why: OUTSIDE_THE_CHAT.to_owned()
        }
    );
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn a_line_dripped_past_its_deadline_is_never_read() {
    use std::io::Write;
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report);
    }));
    let report = Report {
        chat: 7,
        event: Event::Notification,
        conversation: Conversation::Named("abc".to_owned()),
        pid: Some(99),
        agent: None,
        detail: Detail::default(),
    };
    let line = line_with(Some(&token), &report).expect("a line");
    let mut socket = std::os::unix::net::UnixStream::connect(&path).expect("connects");
    // A byte every half second for four seconds: each read is quick, the line is not.
    let (dripped, rest) = line.split_at(8);
    for byte in dripped {
        let _ = socket.write_all(&[*byte]);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = socket.write_all(rest);
    assert!(
        rx.recv_timeout(std::time::Duration::from_secs(1)).is_err(),
        "the line was read after its deadline"
    );
}

#[test]
fn each_chat_is_issued_a_token_of_its_own_and_only_its_latest_counts() {
    let tokens = ChatTokens::default();
    // The token alone: every process counts as inside here, which the binding's own tests
    // take apart.
    let admitted = |tokens: &ChatTokens, chat: u32, token: Option<&str>, peer: Option<u32>| {
        tokens.bind(chat, program(1));
        tokens.admission_by(chat, token, peer, &|_, _| true) == Admission::Admitted
    };
    let one = tokens.issue(1).expect("a token");
    let two = tokens.issue(2).expect("a token");

    assert_ne!(one, two);
    for token in [&one, &two] {
        // 32 bytes from the generator, as hex.
        assert_eq!(token.expose().len(), 64);
        assert!(token.expose().bytes().all(|b| b.is_ascii_hexdigit()));
    }
    assert!(admitted(&tokens, 1, Some(one.expose()), Some(1)));
    assert!(!admitted(&tokens, 1, Some(two.expose()), Some(1)));
    assert!(!admitted(&tokens, 1, None, Some(1)));
    assert!(
        !admitted(&tokens, 3, Some(one.expose()), Some(1)),
        "a chat with no token"
    );
    // A prefix, and the right length with one byte wrong.
    assert!(!admitted(&tokens, 1, Some(&one.expose()[..63]), Some(1)));
    let mut near = one.expose().to_owned().into_bytes();
    near[63] = if near[63] == b'0' { b'1' } else { b'0' };
    assert!(!admitted(
        &tokens,
        1,
        Some(std::str::from_utf8(&near).unwrap()),
        Some(1)
    ));

    let again = tokens.issue(1).expect("a token");
    assert!(
        !admitted(&tokens, 1, Some(one.expose()), Some(1)),
        "the old one still counts"
    );
    assert!(admitted(&tokens, 1, Some(again.expose()), Some(1)));

    tokens.forget(1);
    assert!(!admitted(&tokens, 1, Some(again.expose()), Some(1)));
}

#[test]
fn a_token_is_never_written_out_by_debug() {
    let tokens = ChatTokens::default();
    let token = tokens.issue_to_this_process(1).expect("a token");

    assert!(!format!("{token:?}").contains(token.expose()));
    assert!(!format!("{tokens:?}").contains(token.expose()));
}

#[test]
fn a_token_is_read_from_its_own_variable_and_an_empty_one_is_none() {
    assert_eq!(
        ChatToken::read(&env_of(&[(TOKEN_ENV, "abc")])),
        Some(ChatToken::from("abc"))
    );
    assert_eq!(ChatToken::read(&env_of(&[(TOKEN_ENV, "")])), None);
    assert_eq!(ChatToken::read(&env_of(&[(CHAT_ENV, "7")])), None);
}

#[test]
fn a_token_travels_beside_the_line_and_is_taken_off_before_it_is_read() {
    let token = ChatToken::from("abc");
    let wire = line_with(Some(&token), &Ask::Ticket { chat: 3 }).unwrap();
    let text = std::str::from_utf8(&wire).unwrap();

    let (line, carried) = read_line(text).expect("a line");
    assert!(matches!(line, Line::Ask(Ask::Ticket { chat: 3 })));
    assert_eq!(carried.as_deref(), Some("abc"));

    let bare = line_with(None, &a_stop(7)).unwrap();
    let (line, carried) = read_line(std::str::from_utf8(&bare).unwrap()).expect("a line");
    assert_eq!(line.chat(), 7);
    assert_eq!(carried, None);
}

/// #952: a delivered line's id travels beside it and is taken off it as its token is; only
/// a line a hook delivers may carry one, and only one a hook makes.
#[test]
fn a_delivery_is_read_off_a_delivered_line_and_on_no_other_kind() {
    let token = ChatToken::from("abc");
    let delivery = Delivery::new().expect("an id");
    let delivered = with_delivery(&a_stop(7), Some(&delivery)).unwrap();
    let wire = line_with(Some(&token), &delivered).unwrap();

    let ReadLine {
        line,
        token: carried,
        delivery: read,
        ..
    } = read_delivered(std::str::from_utf8(&wire).unwrap()).expect("a line");
    assert_eq!(line.chat(), 7);
    assert_eq!(carried.as_deref(), Some("abc"));
    assert_eq!(read, Some(delivery.clone()));

    let ask = with_delivery(&Ask::Ticket { chat: 3 }, Some(&delivery)).unwrap();
    let ask = line_with(Some(&token), &ask).unwrap();
    assert!(
        read_delivered(std::str::from_utf8(&ask).unwrap()).is_none(),
        "an ask never carries a delivery"
    );

    let mut forged = serde_json::to_value(a_stop(7)).unwrap();
    forged[DELIVERY] = serde_json::Value::String("../not an id".to_owned());
    assert!(read_delivered(&forged.to_string()).is_none());

    let bare = line_with(None, &a_stop(7)).unwrap();
    let none = read_delivered(std::str::from_utf8(&bare).unwrap())
        .expect("a line")
        .delivery;
    assert_eq!(none, None, "a hook of a build before #952 sends none");
}

/// What the app wrote back on `socket` before closing it, read within `within`.
fn what_the_app_said(
    socket: &std::os::unix::net::UnixStream,
    within: std::time::Duration,
) -> io::Result<String> {
    use std::io::BufRead;
    socket.set_read_timeout(Some(within))?;
    let mut said = String::new();
    std::io::BufReader::new(socket).read_line(&mut said)?;
    Ok(said)
}

#[test]
fn a_process_that_floods_the_hook_socket_is_turned_away_at_once_saying_why() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let reading = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .serving_at_most(4, 16)
        .each(Box::new(|_| {}));

    // Its share, connected and silent, held inside the app's two-second deadline.
    let held: Vec<_> = (0..4)
        .map(|_| std::os::unix::net::UnixStream::connect(&path).expect("a connection"))
        .collect();
    let began = std::time::Instant::now();
    let past = std::os::unix::net::UnixStream::connect(&path).expect("a connection");
    let said = what_the_app_said(&past, A_REPORT_TAKES_AT_MOST / 4)
        .expect("the app answered rather than holding it");
    assert!(
        began.elapsed() < A_REPORT_TAKES_AT_MOST / 2,
        "{:?}",
        began.elapsed()
    );
    assert_eq!(
        serde_json::from_str::<Answer>(&said).ok(),
        Some(Answer::No {
            why: Busy::Peer.to_string()
        }),
        "{said}"
    );
    // And closed after it.
    let mut rest = String::new();
    assert!(matches!(
        std::io::BufReader::new(&past).read_to_string(&mut rest),
        Ok(0) | Err(_)
    ));
    drop(held);
    drop(reading);
}

#[test]
fn a_hook_turned_away_by_a_busy_channel_spools_its_line_and_is_not_told_it_was_refused() {
    // Turned away is not refused: the line was never read, so a hook keeps it for the
    // next drain, as it does when no app is there. The socket is where a project's is, so
    // its spool is one a hook may write.
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter").join("app").join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .serving_at_most(1, 16);
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let _reading = listener.each(Box::new(|_| {}));
    let _held = std::os::unix::net::UnixStream::connect(&path).expect("a connection");

    let delivered = deliver_report(&path, Some(&token), &a_stop(7));

    assert!(
        !matches!(&delivered, Err(why) if why.kind() == io::ErrorKind::PermissionDenied),
        "{delivered:?}"
    );
    assert!(
        matches!(delivered, Ok(Delivered::Spooled(_))),
        "{delivered:?}"
    );
}

#[test]
fn a_burst_of_hooks_from_one_process_within_its_share_is_all_taken() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.chat);
    }));

    let path = Arc::new(path);
    let token = Arc::new(token);
    let senders: Vec<_> = (0..A_PEER_IS_SERVED_AT_MOST / 2)
        .map(|_| {
            let (path, token) = (Arc::clone(&path), Arc::clone(&token));
            std::thread::spawn(move || send(&path, Some(&token), &a_stop(7)))
        })
        .collect();
    for sender in senders {
        sender.join().expect("the thread").expect("it was taken");
    }
    for _ in 0..A_PEER_IS_SERVED_AT_MOST / 2 {
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(10)), Ok(7));
    }
}

/// Set to the hook socket's path, the flood below runs as a process of its own.
const FLOOD_AT: &str = "PURLIS_TEST_FLOOD_THE_HOOK_SOCKET_AT";

/// Not a test of its own: the other process of the one below. It opens `FLOOD` connections
/// to the socket, says so on stdout, and holds them until its stdin closes.
#[test]
#[ignore = "run by another_process_is_heard_while_one_floods_the_hook_socket"]
fn a_process_flooding_the_hook_socket() {
    use std::io::{Read, Write};
    let Some(path) = std::env::var_os(FLOOD_AT) else {
        return;
    };
    let held: Vec<_> = (0..8)
        .filter_map(|_| std::os::unix::net::UnixStream::connect(&path).ok())
        .collect();
    let mut out = std::io::stdout();
    let _ = writeln!(out, "flooding with {}", held.len());
    let _ = out.flush();
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    drop(held);
}

#[test]
fn another_process_is_heard_while_one_floods_the_hook_socket() {
    use std::io::BufRead;
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .serving_at_most(4, 16);
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.chat);
    }));

    let mut flood = crate::forklock::spawn(
        std::process::Command::new(std::env::current_exe().expect("this test"))
            .args([
                "--exact",
                "hookwire::tests::a_process_flooding_the_hook_socket",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(FLOOD_AT, &path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
    )
    .expect("the flooding process");
    // Every wait here is bounded, and the flood is ended whatever happens: a test that waits
    // on another process must never outlive it.
    let out = std::io::BufReader::new(flood.stdout.take().expect("its stdout"));
    let (lines_tx, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in out.lines().map_while(Result::ok) {
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut seen = Vec::new();
    let flooding = loop {
        match lines.recv_timeout(std::time::Duration::from_secs(20)) {
            // libtest writes the test's name before what the test prints, on one line.
            Ok(line) if line.contains("flooding with") => break Some(line),
            Ok(line) => seen.push(line),
            Err(_) => break None,
        }
    };
    let end = |flood: &mut std::process::Child| {
        drop(flood.stdin.take());
        let began = std::time::Instant::now();
        while began.elapsed() < std::time::Duration::from_secs(5) {
            if flood.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = flood.kill();
        let _ = flood.wait();
    };
    let Some(flooding) = flooding.filter(|line| line.contains("flooding with 8")) else {
        end(&mut flood);
        panic!("the flooding process never held its connections; it said {seen:?}");
    };
    let _ = flooding;

    // Its share is taken and the rest turned away; this process's line still lands, well
    // inside the deadline the flood's held connections run out at.
    let sent = send(&path, Some(&token), &a_stop(7));
    let heard = rx.recv_timeout(A_REPORT_TAKES_AT_MOST / 2);

    end(&mut flood);
    sent.expect("the app took it");
    assert_eq!(
        heard,
        Ok(7),
        "a flood from one process kept another's line out"
    );
}

/// A connection served by `serving` for `peer` that is held until `gate` lets it go, and
/// says on `served` that it is being served.
fn a_held_connection(
    serving: &Arc<Serving>,
    peer: u32,
    gate: &Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    served: &mpsc::Sender<u32>,
) -> Result<(), Busy> {
    let slot = serving.admit(Some(peer))?;
    let gate = Arc::clone(gate);
    let served = served.clone();
    serving
        .run(
            slot,
            Box::new(move || {
                let _ = served.send(peer);
                let (open, opened) = &*gate;
                let mut open = open.lock().unwrap();
                while !*open {
                    open = opened.wait(open).unwrap();
                }
            }),
        )
        .expect("a thread to serve it");
    Ok(())
}

fn a_gate() -> Arc<(std::sync::Mutex<bool>, std::sync::Condvar)> {
    Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()))
}

fn open(gate: &Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>) {
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
}

#[test]
fn a_process_that_floods_the_channel_is_served_only_its_share_and_refused_at_once_past_it() {
    let serving = Serving::new(4, 16);
    let gate = a_gate();
    let (served, serving_now) = mpsc::channel();
    for _ in 0..4 {
        a_held_connection(&serving, 7, &gate, &served).expect("within its share");
    }
    for _ in 0..4 {
        serving_now
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("each of its share is served");
    }

    let began = std::time::Instant::now();
    let refused = a_held_connection(&serving, 7, &gate, &served);
    assert_eq!(refused, Err(Busy::Peer));
    assert!(
        began.elapsed() < std::time::Duration::from_millis(500),
        "a refusal waited: {:?}",
        began.elapsed()
    );
    open(&gate);
}

#[test]
fn another_process_is_still_served_while_one_floods_the_channel() {
    let serving = Serving::new(4, 16);
    let gate = a_gate();
    let (served, serving_now) = mpsc::channel();
    while a_held_connection(&serving, 7, &gate, &served).is_ok() {}

    let other = a_gate();
    open(&other);
    a_held_connection(&serving, 8, &other, &served).expect("the other process is served");

    let heard: Vec<u32> = (0..5)
        .map(|_| {
            serving_now
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("served")
        })
        .collect();
    assert_eq!(heard.iter().filter(|peer| **peer == 8).count(), 1);
    open(&gate);
}

#[test]
fn the_channel_serves_at_most_its_bound_from_every_process_together() {
    let serving = Serving::new(4, 6);
    let gate = a_gate();
    let (served, serving_now) = mpsc::channel();
    for peer in 0..6u32 {
        a_held_connection(&serving, peer, &gate, &served).expect("within the bound");
    }
    assert_eq!(
        a_held_connection(&serving, 100, &gate, &served),
        Err(Busy::Channel)
    );

    // Once they end, the channel serves again, and so does the process that was turned away.
    open(&gate);
    for _ in 0..6 {
        serving_now
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("served");
    }
    let began = std::time::Instant::now();
    loop {
        match a_held_connection(&serving, 100, &gate, &served) {
            Ok(()) => break,
            Err(_) if began.elapsed() < std::time::Duration::from_secs(5) => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(busy) => panic!("the channel never served again: {busy}"),
        }
    }
    assert_eq!(
        serving_now.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(100)
    );
}

#[test]
fn connections_are_served_by_a_bounded_pool_of_threads_and_never_one_each() {
    let serving = Serving::new(A_PEER_IS_SERVED_AT_MOST, 3);
    let (tx, rx) = mpsc::channel();
    for n in 0..200u32 {
        // Each waits for a place, as a hook that was refused tries again later.
        let slot = loop {
            match serving.admit(Some(n)) {
                Ok(slot) => break slot,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        };
        let tx = tx.clone();
        serving
            .run(
                slot,
                Box::new(move || {
                    let _ = tx.send(std::thread::current().id());
                }),
            )
            .expect("a thread");
    }
    let threads: std::collections::HashSet<_> = (0..200)
        .map(|_| {
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .expect("served")
        })
        .collect();
    assert!(threads.len() <= 3, "{} threads served 200", threads.len());
}

#[test]
fn a_burst_of_hooks_from_fifty_chats_at_once_is_served_whole() {
    let serving = Serving::new(A_PEER_IS_SERVED_AT_MOST, THE_CHANNEL_SERVES_AT_MOST);
    let gate = a_gate();
    let (served, serving_now) = mpsc::channel();
    // Fifty chats, each with a hook mid-write and one ask held open on the operator.
    for peer in 0..100u32 {
        a_held_connection(&serving, peer, &gate, &served).expect("a normal burst");
    }
    // And one long-lived process with several asks in flight at once.
    for _ in 0..8 {
        a_held_connection(&serving, 1000, &gate, &served).expect("a normal burst");
    }
    for _ in 0..108 {
        serving_now
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("served");
    }
    open(&gate);
}

#[test]
fn a_job_that_panics_gives_its_place_back() {
    let serving = Serving::new(1, 1);
    let slot = serving.admit(Some(7)).expect("a place");
    serving
        .run(slot, Box::new(|| panic!("a hearer that panicked")))
        .expect("a thread");
    let began = std::time::Instant::now();
    let (tx, rx) = mpsc::channel();
    loop {
        if let Ok(slot) = serving.admit(Some(7)) {
            let tx = tx.clone();
            serving
                .run(slot, Box::new(move || tx.send(()).unwrap()))
                .expect("a thread");
            break;
        }
        assert!(
            began.elapsed() < std::time::Duration::from_secs(5),
            "never given back"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(()));
}

/// A report of chat `chat` whose harness names `mode`.
fn a_report_naming(chat: u32, mode: crate::dispatchmode::Reported) -> Report {
    let mut report = a_stop(chat);
    report.detail.mode = Some(mode);
    report
}

/// #3822: a report's mode is heard as its hook said it only where the line carries the key the
/// chat's hook was armed with; a line with the chat's token alone, or another key, is heard
/// asking, and a report that names no mode names none either way.
#[test]
fn a_reported_mode_is_believed_only_from_the_chats_armed_hook() {
    use crate::dispatchmode::Reported;
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path)
        .expect("a socket")
        .owned_by(purlis_same_user::Uid::effective());
    let tokens = listener.tokens();
    let seven = tokens.issue_to_this_process(7).expect("a token");
    let armed = tokens.armed_key(7).expect("chat 7 is armed");
    let eight = tokens.issue_to_this_process(8).expect("a token");
    let (tx, rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = tx.send(report.detail.mode);
    }));
    let heard = || {
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("the report was heard")
    };

    deliver_armed(
        &path,
        Some(&seven),
        Some(&armed),
        &a_report_naming(7, Reported::Classifier),
    )
    .expect("taken");
    assert_eq!(
        heard(),
        Some(Reported::Classifier),
        "the armed hook's own word"
    );

    deliver_armed(
        &path,
        Some(&seven),
        None,
        &a_report_naming(7, Reported::PromptsOff),
    )
    .expect("taken");
    assert_eq!(
        heard(),
        Some(Reported::Asks),
        "the chat's token alone proves no mode"
    );

    let other = tokens.armed_key(8).expect("chat 8 is armed");
    deliver_armed(
        &path,
        Some(&seven),
        Some(&other),
        &a_report_naming(7, Reported::Classifier),
    )
    .expect("taken");
    assert_eq!(
        heard(),
        Some(Reported::Asks),
        "another chat's key proves nothing of this one"
    );

    deliver_armed(
        &path,
        Some(&eight),
        Some(&armed),
        &a_report_naming(8, Reported::Classifier),
    )
    .expect("taken");
    assert_eq!(
        heard(),
        Some(Reported::Asks),
        "nor this chat's key of another"
    );

    deliver_armed(&path, Some(&seven), None, &a_stop(7)).expect("taken");
    assert_eq!(
        heard(),
        None,
        "a report naming no mode is not made to name one"
    );
}

/// #3822: a chat started again is armed afresh, and one that has ended holds no key, so the
/// key of a chat's earlier start proves nothing.
#[test]
fn a_chats_armed_key_is_its_current_starts_alone() {
    let tokens = ChatTokens::default();
    tokens.issue(5).expect("a token");
    let first = tokens.armed_key(5).expect("armed");
    assert!(tokens.proves(5, Some(first.expose())));

    tokens.issue(5).expect("started again");
    assert!(!tokens.proves(5, Some(first.expose())));
    let second = tokens.armed_key(5).expect("armed again");

    tokens.forget(5);
    assert!(!tokens.proves(5, Some(second.expose())));
    assert!(!tokens.proves(5, None));
}

/// #3822: the key is written where the chat's hook reads it as the token is issued, beside a
/// project's socket, and removed as the chat ends; the spool's lines never carry it.
#[test]
fn a_chats_armed_key_is_on_disk_for_its_hook_while_the_chat_runs() {
    let root = tempfile::tempdir().expect("a directory");
    let app = root.path().join(".purlis").join("app");
    std::fs::create_dir_all(&app).unwrap();
    let socket = app.join("hooks.sock");
    let spool = spool::dir_for(&socket);
    private_directory(&spool).unwrap();
    let tokens = ChatTokens::spooling_into(spool.clone());

    tokens.issue_to_this_process(3).expect("a token");
    let key = tokens.armed_key(3).expect("armed");
    assert_eq!(armed::ArmedKey::read(&socket, 3), Some(key));

    tokens.program_ended(3, std::process::id());
    assert_eq!(armed::ArmedKey::read(&socket, 3), None);
}

/// #3822: only a report carries an armed key, as a string; any other kind that carries the
/// field is not read.
#[test]
fn an_armed_key_is_read_off_a_report_and_on_no_other_kind() {
    let token = ChatToken::from("abc");
    let wire = line_armed(Some(&token), Some("k"), &a_stop(7)).unwrap();
    let read = read_delivered(std::str::from_utf8(&wire).unwrap()).expect("a line");
    assert_eq!(read.armed.as_deref(), Some("k"));
    assert_eq!(read.token.as_deref(), Some("abc"));

    let ask = line_armed(Some(&token), Some("k"), &Ask::Ticket { chat: 3 }).unwrap();
    assert!(read_delivered(std::str::from_utf8(&ask).unwrap()).is_none());

    let mut forged = serde_json::to_value(a_stop(7)).unwrap();
    forged[ARMED] = serde_json::json!(7);
    assert!(read_delivered(&forged.to_string()).is_none());
}

#[test]
fn a_prompt_for_a_router_names_its_chat_and_says_nothing_of_the_prompt_in_a_dump() {
    // SK-15c: the prompt crosses to the app on the chat's own line, and no further than the
    // router; a debug dump of the ask says nothing of it, as a prompt's marks do not (SK-38).
    let ask = Ask::Prompt(Box::new(PromptAsk {
        chat: 7,
        prompt: "deploy the secret plan".to_owned(),
    }));
    assert_eq!(ask.chat(), 7);
    let dumped = format!("{ask:?}");
    assert!(!dumped.contains("secret"), "{dumped}");
    let line = serde_json::to_value(&ask).unwrap();
    let back: Ask = serde_json::from_value(line).unwrap();
    assert_eq!(back, ask);
    let answer: Answer = serde_json::from_value(serde_json::json!({
        "pointed": { "pointer": "- `deploy`: Deploys." }
    }))
    .unwrap();
    assert_eq!(
        answer,
        Answer::Pointed {
            pointer: Some("- `deploy`: Deploys.".to_owned())
        }
    );
}

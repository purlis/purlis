// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::eventlog::{ArgsKey, Entry, Log, Recorder, read};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap())
}

fn block(source: Source, qualifier: Option<Qualifier>) -> Block {
    Block {
        tool: "Bash".to_owned(),
        source,
        qualifier,
        target: Some(Target::guard_arm("A4").unwrap()),
        args: Some(crate::eventlog::args_hash(
            &serde_json::json!({"command": "git push --force"}),
        )),
    }
}

/// R-169: a block is a refusal with one of five sources, each bare, unverified or Can't tell,
/// and it reads back with the source and qualifier it was written with.
#[test]
fn a_block_reads_back_with_the_source_and_qualifier_it_was_written_with() {
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let sources = [
        (Source::Guard, "guard"),
        (Source::HarnessCheck, "harness_check"),
        (Source::Sandbox, "sandbox"),
        (Source::Policy, "policy"),
        (Source::ProjectSetting, "project_setting"),
    ];
    let qualifiers = [
        (None, None),
        (Some(Qualifier::Unverified), Some("unverified")),
        (Some(Qualifier::CantTell), Some("cant_tell")),
    ];
    let mut wrote = Vec::new();
    for (source, source_word) in sources {
        for (qualifier, qualifier_word) in qualifiers {
            host.stop(plane, Some(4), &Stop::Block(block(source, qualifier)))
                .unwrap();
            wrote.push((source_word, qualifier_word));
        }
    }

    let read_back: Vec<(String, Option<String>)> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == BLOCK)
        .map(|event| {
            (
                event.body["source"].as_str().unwrap().to_owned(),
                event.body["qualifier"].as_str().map(str::to_owned),
            )
        })
        .collect();
    let wrote: Vec<(String, Option<String>)> = wrote
        .into_iter()
        .map(|(s, q)| (s.to_owned(), q.map(str::to_owned)))
        .collect();
    assert_eq!(read_back, wrote, "fifteen blocks, each as written");

    // The window's entry with no source is refused before anything is appended.
    let before = read(dir.path()).unwrap().len();
    let sourceless = serde_json::json!({
        "entry": "stop",
        "plane": "/planes/one",
        "number": 4,
        "stop": {"stop": "block", "tool": "Bash", "args": "00"},
    });
    assert!(
        serde_json::from_value::<Entry>(sourceless).is_err(),
        "a block with no source is no entry"
    );
    let unknown_source = serde_json::json!({
        "entry": "stop",
        "plane": "/planes/one",
        "number": 4,
        "stop": {"stop": "block", "tool": "Bash", "source": "the chat said so"},
    });
    assert!(
        serde_json::from_value::<Entry>(unknown_source).is_err(),
        "a source outside the five is no entry"
    );
    assert_eq!(read(dir.path()).unwrap().len(), before, "nothing written");
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = walk(dir)
        .into_iter()
        .map(|path| path.strip_prefix(dir).unwrap().display().to_string())
        .collect();
    names.sort();
    names
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                let mut inner = walk(&path);
                inner.push(path);
                inner
            } else {
                vec![path]
            }
        })
        .collect()
}

/// R-219: blocks, holds, loop pauses and answers to asks are kinds of the one event log, sent
/// as the window's entries and read back through the log's own subscription; the device's
/// directory gains no file for them.
#[test]
fn stop_kinds_are_written_to_the_existing_event_log_and_no_new_store_appears() {
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    // A kind the log already writes, so every file the log keeps exists before the stops.
    let last = host.refused(plane, 2).unwrap().seq;
    let before = files_in(dir.path());
    let stops = [
        Stop::Block(block(Source::Sandbox, None)),
        Stop::Hold(Hold {
            tool: "Bash".to_owned(),
            target: Some(Target::service("github.com").unwrap()),
            args: Some(crate::eventlog::args_hash(&serde_json::json!({"pr": 1}))),
        }),
        Stop::LoopPause(LoopPause {
            rule: LoopRule::Failing,
            times: 6,
        }),
        Stop::AskAnswered(AskAnswered {
            ask: AskGroup::LoopPause,
            answer: Answer::Allowed,
        }),
    ];
    for stop in &stops {
        // As the window sends it, over the wire.
        let entry = serde_json::to_value(Entry::Stop {
            plane: plane.to_path_buf(),
            number: Some(2),
            stop: stop.clone(),
        })
        .unwrap();
        let entry: Entry = serde_json::from_value(entry).unwrap();
        host.apply(entry).unwrap();
    }

    let mut subscription = crate::eventlog::subscribe(dir.path(), last);
    let kinds: Vec<String> = subscription
        .poll()
        .unwrap()
        .into_iter()
        .filter_map(|delivery| match delivery {
            crate::eventlog::Delivery::Event(event) => Some(event.kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [BLOCK, HOLD, LOOP_PAUSE, ASK_ANSWERED],
        "each stop is the next event of the one log"
    );
    let events = read(dir.path()).unwrap();
    let chat = events[0].chat.clone();
    assert!(chat.is_some());
    assert!(
        events.iter().all(|event| event.chat == chat),
        "under the chat they are of"
    );
    assert_eq!(files_in(dir.path()), before, "no new store or file");
}

/// R-220: a block event keeps the tool, the source, a short target summary and the keyed
/// digest of the arguments, and nothing of the command, its arguments, its output or the
/// harness's `reason`, each of which holds a secret here.
#[test]
fn a_serialised_block_event_keeps_the_tool_source_target_summary_and_digest_and_never_the_command_or_reason()
 {
    const SECRET: &str = "the-fixtures-secret-4f9d0c6e";
    let command = format!("curl -H 'Authorization: Bearer {SECRET}' https://api.example.com/v1");
    let args = serde_json::json!({"command": command, "env": {"TOKEN": SECRET}});
    let output = format!("401 for token {SECRET}");
    let reason = format!("Bash(curl … {SECRET}) is not allowed by the sandbox");
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let hash = crate::eventlog::args_hash(&args);

    let event = host
        .stop(
            plane,
            Some(5),
            &Stop::Block(Block {
                tool: "Bash".to_owned(),
                source: Source::Sandbox,
                qualifier: None,
                target: Some(Target::host("api.example.com").unwrap()),
                args: Some(hash.clone()),
            }),
        )
        .unwrap();

    let digest = ArgsKey::open(dir.path()).unwrap().digest(&hash);
    assert_eq!(
        event.body,
        serde_json::json!({
            "tool": "Bash",
            "source": "sandbox",
            "target": {"host": "api.example.com"},
            "args": digest,
        })
    );
    assert_ne!(digest, hash, "keyed by this device, not the hook's hash");
    let on_disk = std::fs::read_to_string(dir.path().join(crate::eventlog::FILE)).unwrap();
    assert!(on_disk.contains(&digest));
    for never in [
        SECRET,
        command.as_str(),
        output.as_str(),
        reason.as_str(),
        hash.as_str(),
        "curl",
        "Authorization",
    ] {
        assert!(!on_disk.contains(never), "{never:?} is in {on_disk}");
    }
}

/// R-220: a target summary is short, however long what it names.
#[test]
fn a_block_events_target_summary_is_clipped_short() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let long = format!("src/{}", "deep/".repeat(100));

    let event = host
        .stop(
            Path::new("/planes/one"),
            None,
            &Stop::Block(Block {
                tool: "Write".to_owned(),
                source: Source::Sandbox,
                qualifier: None,
                target: Some(Target::folder(&long).unwrap()),
                args: None,
            }),
        )
        .unwrap();

    let folder = event.body["target"]["folder"].as_str().unwrap();
    assert!(folder.len() <= TARGET_CHARS + 3, "{folder}");
}

/// The window's `stop` entry for a block, as JSON, with `target` as given and `extra` fields
/// merged into the stop.
fn wire_block(target: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    let mut stop = serde_json::json!({
        "stop": "block",
        "tool": "Bash",
        "source": "sandbox",
        "target": target,
    });
    for (key, value) in extra.as_object().unwrap() {
        stop[key] = value.clone();
    }
    serde_json::json!({"entry": "stop", "plane": "/planes/one", "number": 6, "stop": stop})
}

/// R-220, over the wire: a block whose target text is shaped like a command or an address
/// with a query is refused before anything is written, and the `reason` and `command` the
/// window had no field for are dropped; the secret each carries is nowhere on disk.
#[test]
fn a_stop_sent_with_a_command_shaped_target_or_extra_reason_and_command_never_puts_the_secret_on_disk()
 {
    const SECRET: &str = "the-fixtures-secret-8b1e57a2";
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let extra = serde_json::json!({
        "reason": format!("Bash(curl -H 'Authorization: Bearer {SECRET}') is not allowed"),
        "command": format!("curl -H 'Authorization: Bearer {SECRET}' https://api.example.com"),
    });
    let command = format!("curl -H 'Authorization: Bearer {SECRET}'");
    let address = format!("api.example.com/v1?key={SECRET}");
    let shaped = [
        serde_json::json!({"service": command}),
        serde_json::json!({"guard_arm": command}),
        serde_json::json!({"folder": command}),
        serde_json::json!({"host": command}),
        serde_json::json!({"host": address}),
        serde_json::json!({"host": format!("{SECRET}@api.example.com")}),
        serde_json::json!({"folder": format!("/Users/someone/{SECRET}")}),
    ];
    for target in shaped {
        let sent = wire_block(target.clone(), extra.clone());
        if let Ok(entry) = serde_json::from_value::<Entry>(sent) {
            host.apply(entry).unwrap();
        }
    }
    // The same extra fields beside a target that passes are dropped, and the block written.
    let entry: Entry = serde_json::from_value(wire_block(
        serde_json::json!({"host": "api.example.com"}),
        extra,
    ))
    .unwrap();
    host.apply(entry).unwrap();

    let blocks: Vec<serde_json::Value> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == BLOCK)
        .map(|event| event.body)
        .collect();
    assert_eq!(
        blocks,
        [
            serde_json::json!({"tool": "Bash", "source": "sandbox", "target": {"host": "api.example.com"}})
        ],
        "only the block with a real host was written, without its reason or command"
    );
    let on_disk = std::fs::read_to_string(dir.path().join(crate::eventlog::FILE)).unwrap();
    for never in [
        SECRET,
        "curl",
        "Authorization",
        "reason",
        "command",
        "?key=",
    ] {
        assert!(!on_disk.contains(never), "{never:?} is in {on_disk}");
    }
}

/// R-220: each kind of target takes what it names and refuses anything else, built in code as
/// read off the wire, and the refusal never repeats the value.
#[test]
fn a_target_takes_only_what_its_kind_names() {
    for host in [
        "api.example.com",
        "localhost:8080",
        "10.0.0.1",
        "[::1]:443",
        "xn--bcher-kva.example",
    ] {
        assert!(Target::host(host).is_ok(), "{host}");
    }
    for host in [
        "",
        "api.example.com/v1",
        "api.example.com?key=x",
        "user@api.example.com",
        "api.example.com#x",
        "api example.com",
        "api.example.com:",
        "api.example.com:99999",
        "https://api.example.com",
        "[::1",
    ] {
        assert!(Target::host(host).is_err(), "{host:?}");
    }
    for token in ["A4", "github", "github.com", "gh:pr", "guard_arm-2"] {
        assert!(Target::service(token).is_ok(), "{token}");
        assert!(Target::guard_arm(token).is_ok(), "{token}");
    }
    let long = "a".repeat(TOKEN_CHARS + 1);
    for token in ["", "two words", "a/b", "x=y", "Bearer\tx", long.as_str()] {
        assert!(Target::service(token).is_err(), "{token:?}");
        assert!(Target::guard_arm(token).is_err(), "{token:?}");
    }
    for folder in [".", "src", "src/deep/er", "My Documents/notes", "café"] {
        assert!(Target::folder(folder).is_ok(), "{folder}");
    }
    for folder in [
        "",
        "/Users/someone/project",
        "\\\\server\\share",
        "~/project",
        "C:\\Users",
        "src/../../etc",
        "..",
        "src\nfoo",
        "$(id)",
        "a;b",
        "it's",
        "a|b",
        "a?b",
        "a#b",
        "*",
    ] {
        assert!(Target::folder(folder).is_err(), "{folder:?}");
    }

    let refused = Target::host("api.example.com?key=the-secret").unwrap_err();
    assert!(!refused.to_string().contains("the-secret"), "{refused}");
    for (key, value) in [
        ("host", "api.example.com/v1?key=x"),
        ("folder", "/Users/someone"),
        ("service", "curl -H x"),
        ("guard_arm", "a b"),
    ] {
        let wire = serde_json::json!({ key: value });
        assert!(
            serde_json::from_value::<Target>(wire).is_err(),
            "{key}: {value}"
        );
    }
    let read_back: Target = serde_json::from_value(serde_json::json!({"host": "a.b:1"})).unwrap();
    assert_eq!(read_back, Target::host("a.b:1").unwrap());
}

/// R-220: a tool's name is written as one short, escaped line, however long what the hook sent.
#[test]
fn a_stops_tool_name_is_written_as_one_short_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let tool = format!("mcp__x__{}\nsecond line", "y".repeat(300));

    let event = host
        .stop(
            Path::new("/planes/one"),
            None,
            &Stop::Hold(Hold {
                tool,
                target: None,
                args: None,
            }),
        )
        .unwrap();

    let written = event.body["tool"].as_str().unwrap();
    assert_eq!(written.len(), TARGET_CHARS + 3, "{written}");
    assert!(written.ends_with("..."), "{written}");
    assert!(!written.contains('\n'), "{written}");
}

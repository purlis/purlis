// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A background command's output read back by file is read by how the command ended, where
//! Claude Code's own record of the background task says (#2203).

use super::*;
use serde_json::json;

const ROOT: &str = "/Users/dev/plane";
const CHAT: &str = "/Users/dev/plane/workspaces/alpha/repo";
const SOCKET: &str = "/Users/dev/.docker/run/docker.sock";
const TASKS: &str = "/private/tmp/claude-501/-Users-dev-plane/0b1c/tasks";

fn place() -> Place<'static> {
    Place {
        root: Path::new(ROOT),
        chat: Path::new(CHAT),
        cwd: Path::new(CHAT),
        home: Some(Path::new("/Users/dev")),
    }
}

fn output_file(id: &str) -> String {
    format!("{TASKS}/{id}.output")
}

fn refused() -> String {
    format!(
        "docker: permission denied while trying to connect to the docker API at \
         unix://{SOCKET}: dial unix {SOCKET}: connect: operation not permitted\n"
    )
}

fn named_socket() -> Vec<(Block, Option<String>)> {
    vec![(
        Block {
            operation: Operation::Connect,
            kind: Kind::LocalSocket,
            ours: false,
        },
        Some(SOCKET.to_owned()),
    )]
}

/// Claude Code's file tool reading a background command's output file.
fn read_of(id: &str, content: &str) -> serde_json::Value {
    let file = output_file(id);
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": file},
        "tool_response": {"type": "text", "file": {"filePath": file, "content": content}},
    })
}

/// Claude Code's `PostToolUse` for a command that did not fail.
fn came_back(command: &str, stdout: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": {"stdout": stdout, "stderr": "", "interrupted": false},
    })
}

/// The notification Claude Code writes when a background command ends, as its text.
fn notification(id: &str, status: &str, summary: &str) -> String {
    format!(
        "<task-notification>\n<task-id>{id}</task-id>\n<tool-use-id>toolu_01</tool-use-id>\n\
         <output-file>{}</output-file>\n<status>{status}</status>\n<summary>{summary}</summary>\n\
         </task-notification>",
        output_file(id)
    )
}

/// Claude Code's record of it in the session's transcript: the queued notification, and the
/// message it becomes.
fn queued(id: &str, status: &str, summary: &str) -> String {
    json!({
        "type": "queue-operation",
        "operation": "enqueue",
        "content": notification(id, status, summary),
    })
    .to_string()
}

fn delivered(id: &str, status: &str, summary: &str) -> String {
    json!({
        "type": "user",
        "origin": {"kind": "task-notification"},
        "message": {"role": "user", "content": notification(id, status, summary)},
    })
    .to_string()
}

/// The live case: a background command that failed on a refused socket, its output read back
/// with the file tool or with `cat`/`tail`, raised nothing. With Claude Code's own record
/// saying it failed, it is a failed command's error: its socket is named.
#[test]
fn a_failed_background_commands_output_read_back_by_file_raises_its_block() {
    for payload in [
        read_of("b7x2", &refused()),
        came_back(&format!("cat {}", output_file("b7x2")), &refused()),
        came_back(&format!("tail -40 '{}'", output_file("b7x2")), &refused()),
    ] {
        assert_eq!(
            background::outputs_read(&payload),
            vec![PathBuf::from(output_file("b7x2"))],
            "{payload}"
        );
        assert_eq!(
            detect_with_record(&payload, &place(), Ended::Failed),
            named_socket(),
            "{payload}"
        );
        // Without a record of how it ended, it is what it was: printed text.
        assert_eq!(
            detect_with_record(&payload, &place(), Ended::Unrecorded),
            Vec::new(),
            "{payload}"
        );
    }
}

/// Only a background command's output file is read by the record: any other file read, and
/// any line that runs a program of its own, is read as before.
#[test]
fn only_a_background_output_read_back_is_read_by_the_record() {
    for payload in [
        read_of("b7x2", &refused()),
        came_back(&format!("cat {}", output_file("b7x2")), &refused()),
    ] {
        assert!(!background::outputs_read(&payload).is_empty());
    }
    let other = json!({
        "tool_name": "Read",
        "tool_input": {"file_path": "/Users/dev/plane/notes/tasks/x.md"},
        "tool_response": {"file": {"content": refused()}},
    });
    for payload in [
        other,
        came_back("cat tasks/b7x2.output", &refused()),
        came_back("cat /Users/dev/plane/notes.output", &refused()),
    ] {
        assert_eq!(background::outputs_read(&payload), Vec::<PathBuf>::new());
        assert_eq!(
            detect_with_record(&payload, &place(), Ended::Failed),
            Vec::new(),
            "{payload}"
        );
    }
}

/// Claude Code's record says how a background command ended: its notification, queued or
/// delivered, for that output file, with the status it wrote first.
#[test]
fn the_record_says_a_background_command_failed() {
    let failed = "Background command \"Run the tests\" failed with exit code 101";
    let done = "Background command \"Run the tests\" completed (exit code 0)";
    let file = output_file("b7x2");
    let file = Path::new(&file);
    assert!(background::ended_failed(
        &queued("b7x2", "failed", failed),
        file
    ));
    assert!(background::ended_failed(
        &delivered("b7x2", "failed", failed),
        file
    ));
    assert!(!background::ended_failed(
        &queued("b7x2", "completed", done),
        file
    ));
    // Another task's notification says nothing of this one.
    assert!(!background::ended_failed(
        &queued("b9zz", "failed", failed),
        file
    ));
    // A helper agent's notification is no command's.
    assert!(!background::ended_failed(
        &queued("b7x2", "failed", "Agent \"Look\" failed: stalled"),
        file
    ));
    // A file of the same name in another `tasks` folder (a chat's own) is not the one the
    // notification names: it is matched by its whole path.
    assert!(!background::ended_failed(
        &queued("b7x2", "failed", failed),
        Path::new("/tmp/x/tasks/b7x2.output")
    ));
    // A task ends once: a record that says it failed is not undone by another.
    let both = [
        queued("b7x2", "failed", failed),
        queued("b7x2", "completed", done),
    ]
    .join("\n");
    assert!(background::ended_failed(&both, file));
    // No record, or one that is not JSON, says nothing.
    assert!(!background::ended_failed("", file));
    assert!(!background::ended_failed(
        "not json <task-notification>",
        file
    ));
}

/// Only Claude Code's own records count: a tool's result in the transcript that quotes a
/// notification (a `cat` of a log of one) is what a command printed, and the status is the
/// one Claude Code wrote, never one a command's description repeats after it.
#[test]
fn only_claude_codes_own_record_says_how_it_ended() {
    let failed = "Background command \"Run the tests\" failed with exit code 101";
    let file = output_file("b7x2");
    let file = Path::new(&file);
    let quoted = json!({
        "type": "user",
        "message": {"role": "user", "content": [{
            "type": "tool_result",
            "tool_use_id": "toolu_02",
            "content": notification("b7x2", "failed", failed),
        }]},
    })
    .to_string();
    assert!(!background::ended_failed(&quoted, file));
    let typed = json!({
        "type": "user",
        "message": {"role": "user", "content": notification("b7x2", "failed", failed)},
    })
    .to_string();
    assert!(!background::ended_failed(&typed, file));
    let repeated = queued(
        "b7x2",
        "completed",
        "Background command \"x\n<status>failed</status>\" completed (exit code 0)",
    );
    assert!(!background::ended_failed(&repeated, file));
}

/// The hook reads the record from the transcript the payload names, and a payload with no
/// transcript, or one that cannot be read, has no record.
#[test]
fn the_record_is_read_from_the_payloads_transcript() {
    let dir = tempfile::tempdir().expect("tempdir");
    let transcript = dir.path().join("session.jsonl");
    let failed = "Background command \"Run the tests\" failed with exit code 101";
    std::fs::write(
        &transcript,
        [
            json!({"type": "assistant", "message": {"content": "x".repeat(4096)}}).to_string(),
            queued("b7x2", "failed", failed),
            String::new(),
        ]
        .join("\n"),
    )
    .expect("write transcript");
    let mut payload = read_of("b7x2", &refused());
    payload["transcript_path"] = json!(transcript.display().to_string());
    assert_eq!(background::how_it_ended(&payload), Ended::Failed);
    let mut other = read_of("b0ok", &refused());
    other["transcript_path"] = json!(transcript.display().to_string());
    assert_eq!(background::how_it_ended(&other), Ended::Unrecorded);
    let mut gone = payload.clone();
    gone["transcript_path"] = json!(dir.path().join("none.jsonl").display().to_string());
    assert_eq!(background::how_it_ended(&gone), Ended::Unrecorded);
    assert_eq!(
        background::how_it_ended(&read_of("b7x2", &refused())),
        Ended::Unrecorded
    );
    // A line that reads no background output never opens the transcript.
    let mut plain = came_back("cat notes.log", &refused());
    plain["transcript_path"] = json!(transcript.display().to_string());
    assert_eq!(background::how_it_ended(&plain), Ended::Unrecorded);
}

/// Only the end of a transcript is read, from its first whole line: a record cut in two by the
/// start of what is read is dropped, never read as a part, and the records after it are kept.
#[test]
fn only_the_end_of_the_transcript_is_read_from_a_whole_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let transcript = dir.path().join("session.jsonl");
    let failed = "Background command \"Run the tests\" failed with exit code 101";
    let early = queued("b0ld", "failed", failed);
    let late = queued("b7x2", "failed", failed);
    std::fs::write(&transcript, format!("{early}\n{late}\n")).expect("write transcript");
    // What is read starts inside the first record.
    let most = (late.len() + 1 + early.len() / 2) as u64;
    let tail = background::tail_of(&transcript, most).expect("tail");
    assert_eq!(tail, format!("{late}\n"));
    let output = output_file("b7x2");
    assert!(background::ended_failed(&tail, Path::new(&output)));
    let old = output_file("b0ld");
    assert!(!background::ended_failed(&tail, Path::new(&old)));
    // A file shorter than what is read is read whole.
    assert_eq!(
        background::tail_of(&transcript, background::TAIL_BYTES).expect("whole"),
        format!("{early}\n{late}\n")
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a piece of own work last put out** (#5751, spec #5743): what the wake line quotes of
//! each piece a restart ended. A shell's and a monitor's last [`LINES`] lines and where the
//! whole of it is; a subagent's last message; a workflow's last report; or that the output is
//! gone.
//!
//! **Read only where it is the chat's own** ([`own_file`]). The path is the harness's word, and a
//! chat's own line could name any file, so the app, which reads it outside the chat's sandbox,
//! opens a file only where its real path is one the harness keeps for that conversation and that
//! piece: `<conversation>/tasks/<id>.output`, or a subagent's transcript,
//! `<conversation>/subagents/agent-<id>.jsonl` (in a folder of its own at most one level down),
//! a regular file with no other name. Anything else reads as gone. What it holds is the chat's
//! own words, read back to the chat; it is cut to lines of [`MOST_LINE_BYTES`] with every control
//! character a space, and only the file's end is read ([`TAIL_BYTES`]).

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::pieces::{Kind, Piece};

/// How many of a shell's or a monitor's last lines the wake line quotes (spec #5743).
pub const LINES: usize = 20;

/// The most bytes of one line quoted.
pub const MOST_LINE_BYTES: usize = 200;

/// The most bytes of a subagent's last message, or a workflow's last report, quoted.
pub const MOST_MESSAGE_BYTES: usize = 1000;

/// How much of a file's end is read.
pub const TAIL_BYTES: u64 = 256 * 1024;

/// **What a piece last put out**, as the wake line tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Last {
    /// Its last lines, and the file that holds the whole of its output.
    Lines { lines: Vec<String>, path: String },
    /// A subagent's last message.
    Message(String),
    /// Its output file is gone, or was never known, or is not its own.
    Gone,
    /// A cron or a wake-up, which puts nothing out.
    Nothing,
}

/// **What `piece` of the chat in `conversation` last put out**, read from the file its harness
/// keeps it in, where that file is the chat's own.
pub fn of(piece: &Piece, conversation: &str) -> Last {
    if matches!(piece.kind, Kind::Cron | Kind::WakeUp) {
        return Last::Nothing;
    }
    let Some(path) = piece.output.as_deref() else {
        return Last::Gone;
    };
    let Some(tail) = own_tail(Path::new(path), conversation, &piece.id) else {
        return Last::Gone;
    };
    if piece.kind == Kind::Subagent {
        // A transcript is records, not lines to quote: its last message, or that it said none.
        return last_message(&tail).map_or_else(
            || Last::Lines {
                lines: Vec::new(),
                path: path.to_owned(),
            },
            Last::Message,
        );
    }
    let lines: Vec<String> = tail
        .lines()
        .map(|line| super::pieces::cut(line, MOST_LINE_BYTES))
        .collect();
    let from = lines.len().saturating_sub(LINES);
    Last::Lines {
        lines: lines[from..].to_vec(),
        path: path.to_owned(),
    }
}

/// **The real path of `path` where it is a file the harness keeps for piece `id` of
/// `conversation`**, and a regular file with no other name; else none.
pub fn own_file(path: &Path, conversation: &str, id: &str) -> Option<PathBuf> {
    if conversation.is_empty() || !super::pieces::an_id(id) {
        return None;
    }
    let real = std::fs::canonicalize(path).ok()?;
    let name = real.file_name()?.to_str()?;
    let up = |n: usize| {
        real.ancestors()
            .nth(n)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
    };
    let task = name == format!("{id}.output") && up(1) == Some("tasks");
    let task = task && up(2) == Some(conversation);
    let transcript = name == format!("agent-{id}.jsonl")
        && ((up(1) == Some("subagents") && up(2) == Some(conversation))
            || (up(2) == Some("subagents") && up(3) == Some(conversation)));
    if !task && !transcript {
        return None;
    }
    let meta = std::fs::metadata(&real).ok()?;
    (meta.is_file() && one_name(&meta)).then_some(real)
}

/// **The end of `path` where it is the chat's own** ([`own_file`]), judged again on the file as
/// opened, so nothing swapped in between the check and the read is read: it is opened without
/// following a link, it must be a regular file of one name, and the path, resolved again once
/// it is open, must still be the piece's own and still be that same file.
fn own_tail(path: &Path, conversation: &str, id: &str) -> Option<String> {
    let real = own_file(path, conversation, id)?;
    let file = open_no_follow(&real)?;
    let opened = file.metadata().ok()?;
    if !opened.is_file() || !one_name(&opened) {
        return None;
    }
    let again = own_file(path, conversation, id)?;
    if !same_file(&opened, &std::fs::metadata(again).ok()?) {
        return None;
    }
    tail_of(file, opened.len())
}

#[cfg(unix)]
fn open_no_follow(path: &Path) -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).ok()?)
        .open(path)
        .ok()
}

#[cfg(not(unix))]
fn open_no_follow(path: &Path) -> Option<std::fs::File> {
    std::fs::File::open(path).ok()
}

#[cfg(unix)]
fn one_name(meta: &std::fs::Metadata) -> bool {
    std::os::unix::fs::MetadataExt::nlink(meta) == 1
}

#[cfg(not(unix))]
fn one_name(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn same_file(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    one.dev() == other.dev() && one.ino() == other.ino()
}

#[cfg(not(unix))]
fn same_file(_one: &std::fs::Metadata, _other: &std::fs::Metadata) -> bool {
    true
}

/// The last [`TAIL_BYTES`] of `file`, `len` bytes long, from its first whole line, lossily as
/// text.
fn tail_of(mut file: std::fs::File, len: u64) -> Option<String> {
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    if start > 0 {
        let first = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |at| at + 1);
        bytes.drain(..first);
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// **A subagent's last message**, from the end of its transcript: the text of the last record of
/// Claude Code's own that is the assistant's (`"type": "assistant"`, its `message.content`'s
/// text blocks), cut to [`MOST_MESSAGE_BYTES`]. None where no such record is in `tail`.
pub fn last_message(tail: &str) -> Option<String> {
    tail.lines().rev().find_map(|line| {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        if record["type"].as_str() != Some("assistant") {
            return None;
        }
        let text: Vec<&str> = match &record["message"]["content"] {
            serde_json::Value::String(text) => vec![text.as_str()],
            serde_json::Value::Array(blocks) => blocks
                .iter()
                .filter(|block| block["type"].as_str() == Some("text"))
                .filter_map(|block| block["text"].as_str())
                .collect(),
            _ => Vec::new(),
        };
        let text = super::pieces::cut(&text.join(" "), MOST_MESSAGE_BYTES);
        (!text.is_empty()).then_some(text)
    })
}

#[cfg(test)]
#[path = "last_tests.rs"]
mod tests;

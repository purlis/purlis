// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a background command ended, by Claude Code's own record of it** (#2203).
//!
//! A chat that runs a command in the background reads its output back later, with the file
//! tool or with `cat`/`tail` of the file Claude Code keeps it in (`<tasks>/<id>.output`).
//! Nothing beside that read says how the command ended (#1972), so what it handed back is
//! printed text. Claude Code does record it: when a background command ends it writes a
//! notification into the session's transcript (the payload's `transcript_path`), naming the
//! output file and a status (`completed`, `failed`, `killed`). Where that record says the
//! command failed, the output read back is that command's own error
//! ([`super::detect_with_record`]).
//!
//! Only Claude Code's own records are read: a queued notification
//! (`"type": "queue-operation"`, `"operation": "enqueue"`) and the message it becomes
//! (`"type": "user"` with `"origin": {"kind": "task-notification"}`). A tool's result that quotes
//! one is neither. Of a notification, the status is the first one it gives, the line Claude
//! Code writes before the command's description; it must name the output file read by its
//! whole path; and only a notification of a background command counts, not a helper agent's.
//! A task ends once, so any such record that says `failed` says how it ended. Only the end of
//! the transcript is read ([`TAIL_BYTES`]): a record is written when the command ends, and a
//! chat reads the output back after it is told.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::Ended;

/// How much of a transcript's end is read for a background command's record.
pub const TAIL_BYTES: u64 = 16 * 1024 * 1024;

/// The opening of the notification Claude Code writes when a background task ends.
const NOTIFICATION: &str = "<task-notification>";

/// How a notification of a background command's summary begins, in Claude Code's own words.
const A_COMMAND: &str = "Background command ";

/// **The background commands' output files `payload` reads back** (#2203): the file Claude
/// Code's file tool read, or each word of a `Bash` line that names one. A background output file
/// is an absolute `<id>.output` in a `tasks` folder. None for any other result.
pub fn outputs_read(payload: &serde_json::Value) -> Vec<PathBuf> {
    match payload["tool_name"].as_str() {
        Some(super::READ_TOOL) => payload["tool_input"]["file_path"]
            .as_str()
            .map(Path::new)
            .filter(|path| is_an_output_file(path))
            .map(Path::to_path_buf)
            .into_iter()
            .collect(),
        Some("Bash") => {
            let Some(line) = payload["tool_input"]["command"].as_str() else {
                return Vec::new();
            };
            if line.len() > crate::guardcaps::MAX_COMMAND_BYTES {
                return Vec::new();
            }
            let mut out: Vec<PathBuf> = Vec::new();
            for segment in crate::shellseg::joined_argv(line).unwrap_or_default() {
                for word in segment.argv {
                    let path = PathBuf::from(word);
                    if is_an_output_file(&path) && !out.contains(&path) {
                        out.push(path);
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Whether `path` is one Claude Code keeps a background task's output in.
fn is_an_output_file(path: &Path) -> bool {
    path.is_absolute()
        && path.extension().is_some_and(|ext| ext == "output")
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|folder| folder == "tasks")
}

/// **Whether `transcript` records that the background command whose output is `output` failed**:
/// one of Claude Code's own notifications for it says `failed`. A task ends once, so a record
/// that says it failed is not undone by another.
pub fn ended_failed(transcript: &str, output: &Path) -> bool {
    let Some(id) = output.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    transcript
        .lines()
        .filter(|line| line.contains(NOTIFICATION))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|record| {
            own_notification(&record).and_then(|text| status_of(text, id, output)) == Some(true)
        })
}

/// The notification text of one of Claude Code's own records, or None for any other record.
fn own_notification(record: &serde_json::Value) -> Option<&str> {
    let text = match record["type"].as_str()? {
        "queue-operation" if record["operation"].as_str() == Some("enqueue") => {
            record["content"].as_str()
        }
        "user" if record["origin"]["kind"].as_str() == Some("task-notification") => {
            record["message"]["content"].as_str()
        }
        _ => None,
    }?;
    text.trim_start().starts_with(NOTIFICATION).then_some(text)
}

/// Whether the notification `text` says the background command `id` (output file `output`)
/// failed; None when it is of another task, or of no background command.
fn status_of(text: &str, id: &str, output: &Path) -> Option<bool> {
    let field = |name: &str| {
        let open = format!("<{name}>");
        let close = format!("</{name}>");
        text.lines()
            .find_map(|line| line.strip_prefix(&open)?.strip_suffix(&close))
    };
    if field("task-id")? != id {
        return None;
    }
    if Path::new(field("output-file")?) != output {
        return None;
    }
    let summary = text
        .lines()
        .find_map(|line| line.strip_prefix("<summary>"))?;
    if !summary.starts_with(A_COMMAND) {
        return None;
    }
    Some(field("status")? == "failed")
}

/// **How the background command whose output `payload` reads back ended** (#2203), by Claude
/// Code's record in the transcript the payload names: [`Ended::Failed`] where it says one of
/// them failed, else [`Ended::Unrecorded`]. The transcript is opened only for a result that
/// reads a background output file back, and only its end is read. An output file is matched
/// as it was named and as its real path (`/tmp` is `/private/tmp` on macOS).
pub fn how_it_ended(payload: &serde_json::Value) -> Ended {
    let outputs = outputs_read(payload);
    if outputs.is_empty() {
        return Ended::Unrecorded;
    }
    let Some(transcript) = payload["transcript_path"]
        .as_str()
        .map(Path::new)
        .filter(|path| path.is_absolute())
    else {
        return Ended::Unrecorded;
    };
    let Some(tail) = tail_of(transcript, TAIL_BYTES) else {
        return Ended::Unrecorded;
    };
    let named_or_real = |output: &PathBuf| {
        let real = std::fs::canonicalize(output).ok();
        std::iter::once(output.clone()).chain(real)
    };
    if outputs
        .iter()
        .flat_map(named_or_real)
        .any(|output| ended_failed(&tail, &output))
    {
        Ended::Failed
    } else {
        Ended::Unrecorded
    }
}

/// The last `most` bytes of the file at `path`, from its first whole line, lossily as text.
pub(super) fn tail_of(path: &Path, most: u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(most);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(most).read_to_end(&mut bytes).ok()?;
    if start > 0 {
        let first = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |at| at + 1);
        bytes.drain(..first);
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

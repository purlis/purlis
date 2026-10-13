// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a Claude Code edit would write, read whole from its own input** (#1146).
//!
//! Claude Code asks for `Write`, `Edit` and `MultiEdit` on its `PermissionRequest` hook with
//! the change in the tool's input: a `Write` the file's whole new content, an `Edit` the text
//! it replaces and the text it puts there. That is exactly what the tool writes, so the window
//! can show it and offer Allow beside it, as the pane does.
//!
//! **Shown whole, or not at all.** A change is shown only where every part of it can be drawn
//! as it is: no NUL ([`Unshown::Binary`]), no character a window draws as nothing or elsewhere
//! beyond a tab and a newline, a carriage return included ([`Unshown::DrawnOtherwise`]), no
//! credential shape ([`Unshown::Credential`]), at most [`MOST_LINES`] lines in all
//! ([`Unshown::TooBig`]), and an input holding only the fields this reads, each of its type
//! ([`Unshown::Unread`]). Anything else is said, with no Allow, and answered in the pane.
//!
//! **Nothing is read from disk.** The file as it is now is not shown: the tool writes its input
//! whatever the file holds when it runs, and a path the chat names is never opened to draw it.

use serde_json::{Map, Value};

use super::super::hooked::drawn_otherwise;
use super::super::model::{Unshown, Writes, Written};

/// The most lines an ask's change draws, every side of every change counted. Past it the
/// change is said too big and answered in the pane, whose own view scrolls.
pub const MOST_LINES: usize = 1000;

/// What a `PermissionRequest` `payload` for one of Claude Code's edit tools would write, or
/// none for any other tool.
pub(super) fn claude(payload: &Value) -> Option<Writes> {
    let tool = payload["tool_name"].as_str()?;
    let input = payload["tool_input"].as_object();
    let read = match tool {
        "Write" => input.and_then(write),
        "Edit" => input.and_then(edit),
        "MultiEdit" => input.and_then(multi_edit),
        // A notebook's cell is JSON in a file, which a diff of its source would not show whole.
        "NotebookEdit" => None,
        _ => return None,
    };
    Some(match read {
        None => Writes::NotShown {
            why: Unshown::Unread,
        },
        Some(changes) => match unshown(&changes) {
            Some(why) => Writes::NotShown { why },
            None => Writes::Whole { changes },
        },
    })
}

/// `Write`: the file's whole content.
fn write(input: &Map<String, Value>) -> Option<Vec<Written>> {
    only(input, &["file_path", "content"])?;
    input.get("file_path")?.as_str()?;
    let content = input.get("content")?.as_str()?.to_owned();
    Some(vec![Written::File { content }])
}

/// `Edit`: one replacement.
fn edit(input: &Map<String, Value>) -> Option<Vec<Written>> {
    only(
        input,
        &["file_path", "old_string", "new_string", "replace_all"],
    )?;
    input.get("file_path")?.as_str()?;
    Some(vec![replacement(input)?])
}

/// `MultiEdit`: its replacements, in the order the tool makes them.
fn multi_edit(input: &Map<String, Value>) -> Option<Vec<Written>> {
    only(input, &["file_path", "edits"])?;
    input.get("file_path")?.as_str()?;
    let edits = input.get("edits")?.as_array()?;
    if edits.is_empty() {
        return None;
    }
    edits
        .iter()
        .map(|one| {
            let one = one.as_object()?;
            only(one, &["old_string", "new_string", "replace_all"])?;
            replacement(one)
        })
        .collect()
}

/// One replacement: a text that is not empty, the text put in its place, and whether every
/// place it appears is replaced. An empty `old_string` asks for something else than a
/// replacement (a new file, on some versions), so it is not read as one.
fn replacement(input: &Map<String, Value>) -> Option<Written> {
    let before = input.get("old_string")?.as_str()?;
    let after = input.get("new_string")?.as_str()?;
    let every = match input.get("replace_all") {
        None => false,
        Some(Value::Bool(every)) => *every,
        Some(_) => return None,
    };
    (!before.is_empty()).then(|| Written::Replace {
        before: before.to_owned(),
        after: after.to_owned(),
        every,
    })
}

/// `Some` where `input` holds only fields named in `known`: one more runs differently from what
/// the change shows.
fn only(input: &Map<String, Value>, known: &[&str]) -> Option<()> {
    input
        .keys()
        .all(|key| known.contains(&key.as_str()))
        .then_some(())
}

/// Why `changes` cannot be drawn whole, or none where they can.
fn unshown(changes: &[Written]) -> Option<Unshown> {
    let texts: Vec<&str> = changes
        .iter()
        .flat_map(|change| match change {
            Written::File { content } => vec![content.as_str()],
            Written::Replace { before, after, .. } => vec![before.as_str(), after.as_str()],
        })
        .collect();
    if texts.iter().any(|text| text.contains('\0')) {
        return Some(Unshown::Binary);
    }
    if texts
        .iter()
        .any(|text| text.chars().any(drawn_otherwise_in_a_file))
    {
        return Some(Unshown::DrawnOtherwise);
    }
    if texts.iter().any(|text| holds_a_credential(text)) {
        return Some(Unshown::Credential);
    }
    let lines: usize = texts.iter().map(|text| text.lines().count()).sum();
    (lines > MOST_LINES).then_some(Unshown::TooBig)
}

/// Whether a file's text draws `c` as nothing or elsewhere: what [`drawn_otherwise`] says, but
/// a tab and a newline, which a file's view draws as they are; and the line and paragraph
/// separators, which an editor may break a line on.
fn drawn_otherwise_in_a_file(c: char) -> bool {
    match c {
        '\t' | '\n' => false,
        '\u{2028}' | '\u{2029}' => true,
        c => drawn_otherwise(c),
    }
}

/// Whether `text` holds a credential shape, read whole and a line at a time.
fn holds_a_credential(text: &str) -> bool {
    use crate::secretshape::leak_spans;

    !leak_spans(text).is_empty() || text.lines().any(|line| !leak_spans(line).is_empty())
}

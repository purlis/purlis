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
//! beyond a tab, a newline and the carriage return of a line that ends in CRLF, and no stack of
//! marks drawn over the lines around it ([`Unshown::DrawnOtherwise`]), no
//! credential shape and no vault value this process knows ([`Unshown::Credential`]), at most [`MOST_LINES`] lines in all
//! ([`Unshown::TooBig`]), and an input holding only the fields this reads, each of its type
//! ([`Unshown::Unread`]). Anything else is said, with no Allow, and answered in the pane.
//!
//! **A line that ends in CRLF is drawn with its carriage return marked** ([`drawn`]). An editor
//! breaks a line at CRLF as it does at a line feed and draws nothing for the carriage return,
//! so a change from one line ending to the other would look like no change. The window draws
//! each such carriage return as [`CARRIAGE_RETURN_DRAWN`] before the line break, and says so
//! beside the change. A carriage return that ends no line is still not drawn as it is: a view
//! breaks the line there or draws over it. Nor is a change with a line that ends in CRLF and,
//! anywhere in it, the mark written as a character of its own, since the two would read alike.
//!
//! **Nothing is read from disk.** The file as it is now is not shown: the tool writes its input
//! whatever the file holds when it runs, and a path the chat names is never opened to draw it.

use serde_json::{Map, Value};

use super::super::hooked::stacks_marks;
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
pub fn unshown(changes: &[Written]) -> Option<Unshown> {
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
        .any(|text| holds_what_a_file_s_view_draws_otherwise(text) || stacks_marks(text))
    {
        return Some(Unshown::DrawnOtherwise);
    }
    // The mark a CRLF's carriage return is drawn as, written in the change itself beside a
    // line that ends in CRLF: the window could not tell the two apart.
    if texts.iter().any(|text| ends_a_line_in_crlf(text))
        && texts
            .iter()
            .any(|text| text.contains(CARRIAGE_RETURN_DRAWN))
    {
        return Some(Unshown::DrawnOtherwise);
    }
    if texts.iter().any(|text| holds_a_credential(text)) {
        return Some(Unshown::Credential);
    }
    let lines: usize = texts.iter().map(|text| text.lines().count()).sum();
    (lines > MOST_LINES).then_some(Unshown::TooBig)
}

/// What the window draws the carriage return of a line that ends in CRLF as: U+240D, the
/// symbol for a carriage return, before the line break.
pub const CARRIAGE_RETURN_DRAWN: char = '\u{240D}';

/// Whether a line of `text` ends in CRLF.
pub fn ends_a_line_in_crlf(text: &str) -> bool {
    text.contains("\r\n")
}

/// **`text` as the window draws it**: the carriage return of each line that ends in CRLF as
/// [`CARRIAGE_RETURN_DRAWN`], and everything else as it is. Only for a text shown whole
/// ([`Writes::Whole`]), which holds no other carriage return and, where it holds a CRLF, none
/// of the mark itself. What the tool writes is its own input, never this.
pub fn drawn(text: &str) -> String {
    text.replace("\r\n", &format!("{CARRIAGE_RETURN_DRAWN}\n"))
}

/// Whether `text` holds a character a file's view draws as nothing or elsewhere
/// ([`drawn_otherwise_in_a_file`]), but the carriage return of a line that ends in CRLF, which
/// the window draws as a mark ([`drawn`]).
fn holds_what_a_file_s_view_draws_otherwise(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' && chars.peek() == Some(&'\n') {
            continue;
        }
        if drawn_otherwise_in_a_file(c) {
            return true;
        }
    }
    false
}

/// Whether a file's text draws `c` as nothing or elsewhere: what [`purlis_unicode::invisible`]
/// says, but
/// a tab and a newline, which a file's view draws as they are; and the line and paragraph
/// separators, which an editor may break a line on.
fn drawn_otherwise_in_a_file(c: char) -> bool {
    match c {
        '\t' | '\n' => false,
        c => purlis_unicode::invisible(c),
    }
}

/// Whether `text` holds a credential shape, read whole and a line at a time, or a value this
/// process read from a vault or stored in one ([`crate::secrets::seen`], #1047), which need
/// look like nothing. Where that store cannot say, every text is said to hold one.
fn holds_a_credential(text: &str) -> bool {
    use crate::secretshape::leak_spans;

    !leak_spans(text).is_empty()
        || text.lines().any(|line| !leak_spans(line).is_empty())
        || crate::secrets::seen::holds(text)
}

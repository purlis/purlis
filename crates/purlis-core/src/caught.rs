// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **purlis catches its own failures** (#5697, spec #5683 layer 1, Q19, Q23): what a tool
//! hook's payload makes a failure of purlis's own of, for a Report draft the agent never had to
//! think of writing.
//!
//! **Only what purlis marked as internal counts.** A `purlis` command exits
//! [`INTERNAL_EXIT`] for an internal failure (a panic, a state it has no arm for) and for nothing
//! else; a purlis tool's error says [`TOOL_FAILED_INSIDE`]. A refusal on purpose (a guard, a
//! policy, a missing sign-in) and a usage error exit 1 and are never read here, so the person's
//! Inbox is not filled with purlis doing its job.
//!
//! The hook reads the call that came back, as Claude Code hands it (`PostToolUseFailure`): a
//! shell command's line and its "Exit code N", or the purlis tool's name and its error. It hands
//! the app a [`Caught`]: the call by its command words or tool name alone (never its arguments),
//! the exit, and the error cut to [`MOST_ERROR_BYTES`]. The app scrubs and holds it
//! (`report::PurlisFailure::Internal`); nothing here is sent anywhere.

/// The one exit status a `purlis` command ends with when it failed inside purlis: sysexits'
/// `EX_SOFTWARE`, "an internal software error". Never a refusal's or a usage error's.
pub const INTERNAL_EXIT: u8 = 70;

/// The hook word armed on purlis's own tools, on `PostToolUseFailure` (Claude Code).
pub const TOOL_WORD: &str = "posttoolusefailure-purlis";

/// The hook word armed on a shell command that failed; the sandbox block's word (#1338).
pub const COMMAND_WORD: &str = "posttoolusefailure-blocked";

/// The prefix of every purlis tool's name, as a harness calls it.
pub const TOOL_PREFIX: &str = "mcp__purlis__";

/// What a purlis tool's error starts with when it failed inside purlis, and only then.
pub const TOOL_FAILED_INSIDE: &str = "the tool failed inside purlis";

/// The most bytes of an error a draft carries.
pub const MOST_ERROR_BYTES: usize = 2048;

/// The most subcommand words a call is named by: `purlis worktree commit`.
const MOST_COMMAND_WORDS: usize = 3;

/// The commands that end with another program's own status, which is never purlis's.
const ENDS_WITH_ITS_COMMAND_S_STATUS: [&[&str]; 1] = [&["secret", "exec"]];

/// A purlis tool's error for a failure inside purlis, `why` after the mark.
pub fn tool_failed_inside(why: impl std::fmt::Display) -> String {
    format!("{TOOL_FAILED_INSIDE}: {why}")
}

/// **A failure of purlis's own**, as its hook caught it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Caught {
    /// The command by its program and subcommand words (`purlis worktree commit`), or the
    /// tool by its name (`mcp__purlis__session_record`). Never an argument.
    pub call: String,
    /// The command's exit status; none for a tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// What it said of the failure, cut to [`MOST_ERROR_BYTES`]. Scrubbed by the app.
    pub error: String,
}

/// **The failure of purlis's own the call `payload` came back from shows**, for the hook word
/// `word`; none for any other word, tool or failure.
pub fn of_hook(word: &str, payload: &serde_json::Value) -> Option<Caught> {
    let error = payload["error"].as_str()?;
    match word {
        COMMAND_WORD => of_command(payload["tool_input"]["command"].as_str()?, error),
        TOOL_WORD => {
            let tool = payload["tool_name"].as_str()?;
            (tool.starts_with(TOOL_PREFIX) && error.trim_start().starts_with(TOOL_FAILED_INSIDE))
                .then(|| Caught {
                    call: tool.to_owned(),
                    exit: None,
                    error: cut(error.trim()),
                })
        }
        _ => None,
    }
}

/// A shell command `line` that failed with Claude Code's `error`: purlis's own only where the
/// last program of the line, whose status the line ends with, is purlis, and it exited
/// [`INTERNAL_EXIT`].
fn of_command(line: &str, error: &str) -> Option<Caught> {
    let rest = error.trim_start().strip_prefix("Exit code ")?;
    let (code, said) = rest.split_once('\n').unwrap_or((rest, ""));
    let exit: i32 = code.trim().parse().ok()?;
    if exit != i32::from(INTERNAL_EXIT) {
        return None;
    }
    let last = crate::shellseg::segment_argv(line).pop()?;
    // Assignments in front of the program (`RUST_LOG=debug purlis …`) are not it.
    let mut words = last
        .iter()
        .skip_while(|word| word.contains('=') && !word.starts_with('='));
    let program = words.next()?;
    let name = program.rsplit('/').next().unwrap_or(program);
    if !crate::cliname::is_recognised(name) {
        return None;
    }
    let subcommand: Vec<&str> = words
        .map(String::as_str)
        .take_while(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c == '_')
                && !word.starts_with('-')
        })
        .take(MOST_COMMAND_WORDS)
        .collect();
    if ENDS_WITH_ITS_COMMAND_S_STATUS
        .iter()
        .any(|command| subcommand.starts_with(command))
    {
        return None;
    }
    let call = std::iter::once(name)
        .chain(subcommand)
        .collect::<Vec<_>>()
        .join(" ");
    Some(Caught {
        call,
        exit: Some(exit),
        error: cut(said.trim()),
    })
}

/// `text` cut on a character to [`MOST_ERROR_BYTES`].
fn cut(text: &str) -> String {
    let mut end = text.len().min(MOST_ERROR_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
#[path = "caught_tests.rs"]
mod tests;

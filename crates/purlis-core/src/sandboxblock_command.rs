// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which `purlis` command a block of purlis's own was met by** (#7178), so a Report of it names
//! the operation and the next one can be located.
//!
//! A block carries no path, so a report of purlis's own block said only "a write to purlis's state
//! for this project". The hook that found the block has the command line the chat ran, and names
//! from it the `purlis` command the line starts with: **fixed words only**. Each word is checked
//! against the closed lists below ([`TOP`], [`VERB`]), and a word that is on neither is not
//! kept, so nothing a chat typed (a path, an argument, a name) reaches a Report. The app checks
//! the words again as they arrive ([`known`]): a chat can send the hook's line itself.

use std::fmt::Write as _;

/// The words that follow `purlis`: the commands `purlis --help` lists, and `hook`.
const TOP: &[&str] = &[
    "root",
    "listen",
    "serve",
    "bridge",
    "init",
    "reinit",
    "workspace",
    "ws",
    "worktree",
    "check",
    "change",
    "curation",
    "session",
    "artifact",
    "harness",
    "theme",
    "profile",
    "plugin",
    "guard",
    "discover",
    "clone",
    "status",
    "docs",
    "save",
    "git-policy",
    "sync",
    "recall",
    "persona",
    "skill",
    "secret",
    "vault",
    "doctor",
    "migrate",
    "gl-refresh",
    "propose",
    "pool",
    "signing",
    "statusline",
    "stop",
    "news",
    "queries",
    "update",
    "version",
    "report",
    "scan",
    "dispatch",
    "handoff",
    "hook",
    "git-hook",
];

/// The words that may follow one of [`TOP`] and say what it does there.
const VERB: &[&str] = &[
    "commit",
    "update",
    "push",
    "cut",
    "done",
    "abandon",
    "remove",
    "list",
    "note",
    "report",
    "ask",
    "answer",
    "tell",
    "read",
    "wait",
    "remember",
    "todo",
    "memory",
    "vision",
    "create",
    "add",
    "show",
    "exec",
    "get",
    "set",
    "verify",
    "record",
    "write",
    "start",
    "end",
    "close",
    "runs",
    "pretooluse",
    "posttooluse",
    "userpromptsubmit",
    "stop",
    "sessionstart",
    "subagentstop",
    "precompact",
];

/// The `purlis` command a command line a chat ran **starts with**, as fixed words (`purlis
/// worktree commit`); `None` where the line does not start with one. Only the start: a chat
/// controls the rest of its line, and a `purlis` word later in it (`; echo purlis vault remove`)
/// is text it typed, not the command that was run. Words only from the closed lists.
pub fn in_line(line: &str) -> Option<String> {
    // The line's first command only: what follows a `;`, `&`, `|` or the like is another command.
    let first_command = line
        .split([';', '|', '&', '(', ')', '`', '\n'])
        .next()
        .unwrap_or_default();
    let mut words = first_command.split_whitespace().skip_while(|word| {
        word.split_once('=')
            .is_some_and(|(name, _)| is_variable(name))
    });
    let first = words.next()?.trim_matches(|c| matches!(c, '"' | '\''));
    if first.rsplit('/').next() != Some("purlis") {
        return None;
    }
    let top = listed(TOP, words.next()?)?;
    let mut said = format!("purlis {top}");
    if let Some(verb) = words.next().and_then(|w| listed(VERB, w)) {
        let _ = write!(said, " {verb}");
    }
    Some(said)
}

/// Whether `name` is the name of a shell variable, as in `HOME=/x purlis doctor`.
fn is_variable(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `word` as the list holds it, or `None`.
fn listed(list: &'static [&'static str], word: &str) -> Option<&'static str> {
    list.iter().copied().find(|one| *one == word)
}

/// What a hook's `said` is, checked again: the words of [`in_line`] and nothing else, so a line
/// a chat sent by hand names no more than the hook could have. `None` for anything else.
pub fn known(said: &str) -> Option<String> {
    let mut words = said.split(' ');
    if words.next() != Some("purlis") {
        return None;
    }
    let top = listed(TOP, words.next()?)?;
    let mut checked = format!("purlis {top}");
    if let Some(verb) = words.next() {
        checked.push(' ');
        checked.push_str(listed(VERB, verb)?);
    }
    words.next().is_none().then_some(checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_named_by_its_fixed_words() {
        assert_eq!(
            in_line("purlis worktree commit -m 'my secret message' --all").as_deref(),
            Some("purlis worktree commit")
        );
        assert_eq!(
            in_line("/Applications/purlis.app/Contents/MacOS/purlis doctor").as_deref(),
            Some("purlis doctor")
        );
    }

    #[test]
    fn what_a_chat_typed_is_never_named() {
        assert_eq!(
            in_line("purlis worktree /Users/x/secret").as_deref(),
            Some("purlis worktree")
        );
        assert_eq!(in_line("purlis my-private-thing"), None);
        assert_eq!(in_line("cargo build; purlis doctor"), None);
    }

    #[test]
    fn only_the_command_a_line_starts_with_is_named() {
        assert_eq!(
            in_line("purlis doctor; echo purlis vault remove").as_deref(),
            Some("purlis doctor")
        );
        assert_eq!(in_line("echo purlis vault remove"), None);
        assert_eq!(in_line("ls && purlis vault remove"), None);
        assert_eq!(
            in_line("HOME=/x purlis doctor && purlis status").as_deref(),
            Some("purlis doctor")
        );
    }

    #[test]
    fn the_app_takes_back_only_what_the_hook_could_say() {
        assert_eq!(
            known("purlis worktree commit").as_deref(),
            Some("purlis worktree commit")
        );
        for refused in [
            "rm -rf /",
            "purlis worktree /etc/passwd",
            "purlis doctor extra words",
            "purlis nothing",
            "",
            "purlis doctor or purlis status",
        ] {
            assert_eq!(known(refused), None, "{refused}");
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which `purlis` command a block of purlis's own was met by** (#7178), so a Report of it names
//! the operation and the next one can be located.
//!
//! A block carries no path, so a report of purlis's own block said only "a write to purlis's state
//! for this project". The hook that found the block has the command line the chat ran, and names
//! from it the `purlis` command or commands in it: **fixed words only**. Each word is checked
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

/// How many distinct commands one name holds: more than this in one line, and the rest are not
/// said.
const MOST: usize = 3;

/// The `purlis` commands in `line`, a command line a chat ran, as fixed words (`purlis worktree
/// commit`), joined with ` or `; `None` where it runs none. Words only from the closed lists.
pub fn in_line(line: &str) -> Option<String> {
    let words: Vec<&str> = line
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '`'))
        .filter(|word| !word.is_empty())
        .collect();
    let mut found: Vec<String> = Vec::new();
    for (at, word) in words.iter().enumerate() {
        let named = word.trim_matches(|c| matches!(c, '"' | '\'' | '$' | '{' | '}'));
        if named.rsplit('/').next() != Some("purlis") {
            continue;
        }
        let Some(top) = words.get(at + 1).and_then(|w| listed(TOP, w)) else {
            continue;
        };
        let mut said = format!("purlis {top}");
        if let Some(verb) = words.get(at + 2).and_then(|w| listed(VERB, w)) {
            let _ = write!(said, " {verb}");
        }
        if !found.contains(&said) {
            found.push(said);
        }
    }
    found.truncate(MOST);
    (!found.is_empty()).then(|| found.join(" or "))
}

/// `word` as the list holds it, or `None`.
fn listed(list: &'static [&'static str], word: &str) -> Option<&'static str> {
    list.iter().copied().find(|one| *one == word)
}

/// What a hook's `said` is, checked again: the words of [`in_line`] and nothing else, so a line
/// a chat sent by hand names no more than the hook could have. `None` for anything else.
pub fn known(said: &str) -> Option<String> {
    let parts: Vec<&str> = said.split(" or ").collect();
    if parts.len() > MOST {
        return None;
    }
    let mut out = Vec::new();
    for part in parts {
        let mut words = part.split(' ');
        if words.next() != Some("purlis") {
            return None;
        }
        let top = listed(TOP, words.next()?)?;
        let mut checked = format!("purlis {top}");
        if let Some(verb) = words.next() {
            checked.push(' ');
            checked.push_str(listed(VERB, verb)?);
        }
        if words.next().is_some() {
            return None;
        }
        out.push(checked);
    }
    Some(out.join(" or "))
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
        assert_eq!(in_line("cargo build; echo purlis"), None);
    }

    #[test]
    fn several_commands_are_each_named_once() {
        assert_eq!(
            in_line("purlis doctor && purlis doctor | purlis dispatch note x").as_deref(),
            Some("purlis doctor or purlis dispatch note")
        );
    }

    #[test]
    fn the_app_takes_back_only_what_the_hook_could_say() {
        assert_eq!(
            known("purlis worktree commit").as_deref(),
            Some("purlis worktree commit")
        );
        assert_eq!(
            known("purlis doctor or purlis status").as_deref(),
            Some("purlis doctor or purlis status")
        );
        for refused in [
            "rm -rf /",
            "purlis worktree /etc/passwd",
            "purlis doctor extra words",
            "purlis nothing",
            "",
            "purlis doctor or purlis status or purlis stop or purlis scan",
        ] {
            assert_eq!(known(refused), None, "{refused}");
        }
    }
}

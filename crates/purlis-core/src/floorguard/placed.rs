// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An allowlist for the words of a forge call up to its verb** (#7423, round 11).
//!
//! Ten rounds held what looked dangerous (a quoted flag, `''$V''`, `${V%/}`, a value flag before
//! the noun, a `-R` value the shell fills in), and each round's confirm found what the rule did
//! not recognise: an empty word, `-R"$UNSET"`, a program that appends arguments. Round 11 turns
//! the rule round. **A `gh` or `glab` call passes only if it is plainly a command purlis knows**,
//! and every other one is held, so a missing entry costs a false hold and never a bypass:
//!
//! * every word up to and including the verb is a non-empty [`Fill::Literal`]: no expansion
//!   outside single quotes, not empty, no glob, brace, tilde or extended-glob character;
//! * every flag before the verb takes no value ([`NO_VALUE_FLAGS`]), or is the repository's
//!   (`-R x`, `-Rx`, `--repo=x`) with a plain value;
//! * the noun and the verb are a command [`KNOWN`] lists (reads and ordinary writes), or one the
//!   floor holds on its own ([`merge::held_commands`]), which the rest of the floor decides;
//! * `api` is read word by word by [`super::merge::shell_filled_api_reason`].
//!
//! Words after the verb are judged as before. Every path that reads a call uses this one walk:
//! the direct call, the call without a digit word that stands apart from a redirection, a call
//! behind a program ([`super::exact`]), and a `for` over a forge answer.

use super::merge;

/// What the shell may make of one word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Fill {
    /// No expansion outside single quotes: exactly the characters the lexer kept.
    Literal,
    /// Expansions only inside double quotes, each a plain `$name` or `$(…)`: one word, of a
    /// text the line does not show.
    OneWord,
    /// Any other expansion, glob, brace or tilde: no word, several, or one that begins with `-`.
    Open,
}

/// What the shell may make of the word `text`, which the source spells `spelled`. Where nothing
/// measured the word (`spelled` is empty) its quotes are lost, and any expansion character in the
/// text is read as open.
pub(super) fn fill_of(text: &str, spelled: &str) -> Fill {
    if spelled.is_empty() {
        return if merge::opaque(text) || text.contains(['#', '^', '(']) || text.starts_with('=') {
            Fill::Open
        } else {
            Fill::Literal
        };
    }
    let spelled = merge::without_line_continuations(spelled);
    let chars: Vec<char> = spelled.chars().collect();
    let mut unquoted = String::new();
    let mut one_word = false;
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            '\\' => {
                // An escaped character is that character.
                i += 2;
            }
            '\'' => match chars
                .get(i + 1..)
                .and_then(|rest| rest.iter().position(|&q| q == '\''))
            {
                Some(at) => i += at + 2,
                None => return Fill::Open,
            },
            '"' => {
                let Some(end) = merge::closes_at(&chars, i + 1, '"') else {
                    return Fill::Open;
                };
                let body = &chars[i + 1..end - 1];
                if body.iter().any(|&q| q == '$' || q == '`') {
                    if !merge::plain_expansions_only(body) {
                        return Fill::Open;
                    }
                    one_word = true;
                }
                i = end;
            }
            // zsh's extended glob makes `#`, `~` and `^` operators anywhere in a word
            // (`merge#`, `merge~x`), and `=word` expands to a path.
            '$' | '`' | '*' | '?' | '[' | '(' | '#' | '~' | '^' => return Fill::Open,
            '=' if i == 0 => return Fill::Open,
            _ => {
                unquoted.push(c);
                i += 1;
            }
        }
    }
    if merge::brace_expansion(&unquoted) {
        Fill::Open
    } else if one_word {
        Fill::OneWord
    } else {
        Fill::Literal
    }
}


/// **The commands the walk knows, by CLI: `(noun, verbs)`.** A call whose noun and verb are not
/// here (and not a command the floor already holds, [`merge::held_commands`]) is held, whatever
/// it is: a gh alias or extension can stand for a merge, and a table that listed what is
/// dangerous would leak whatever it forgot. A command missing here costs a false hold, never a
/// bypass. The first rows are reads; the rest are ordinary writes that cannot land code.
const KNOWN: &[(&str, &[(&str, &[&str])])] = &[
    (
        "gh",
        &[
            ("pr", &["view", "create", "list", "diff", "checks", "status", "checkout", "comment", "edit", "close", "reopen", "ready", "review", "update-branch", "lock", "unlock", "revert"]),
            ("issue", &["view", "list", "status", "create", "edit", "close", "reopen", "comment", "delete", "lock", "unlock", "pin", "unpin", "transfer", "develop"]),
            ("repo", &["view", "list", "clone", "create", "fork", "edit", "archive", "unarchive", "delete", "rename", "set-default", "deploy-key", "gitignore", "license", "autolink"]),
            ("run", &["view", "list", "watch", "rerun", "cancel", "delete", "download"]),
            ("workflow", &["view", "list", "run", "enable", "disable"]),
            ("release", &["view", "list", "download", "upload", "delete", "delete-asset", "edit", "verify", "verify-asset"]),
            ("gist", &["view", "list", "create", "edit", "delete", "clone", "rename"]),
            ("label", &["list", "create", "edit", "delete", "clone"]),
            ("secret", &["list", "set", "delete"]),
            ("variable", &["list", "get", "set", "delete"]),
            ("ssh-key", &["list", "add", "delete"]),
            ("gpg-key", &["list", "add", "delete"]),
            ("cache", &["list", "delete"]),
            ("ruleset", &["list", "view", "check"]),
            ("org", &["list"]),
            ("search", &["repos", "issues", "prs", "commits", "code"]),
            ("config", &["list", "set", "clear-cache"]),
            ("extension", &["list", "search", "browse", "install", "remove", "upgrade", "create"]),
            ("project", &["list", "view", "create", "edit", "close", "delete", "copy", "link", "unlink", "field-list", "field-create", "field-delete", "item-list", "item-add", "item-create", "item-edit", "item-archive", "item-delete", "mark-template"]),
        ],
    ),
    (
        "glab",
        &[
            ("mr", &["view", "list", "diff", "approve", "unapprove", "note", "subscribe", "unsubscribe", "update", "rebase", "close", "reopen", "todo", "issues", "checkout", "delete", "approvers"]),
            ("issue", &["view", "list", "create", "update", "close", "reopen", "note", "subscribe", "unsubscribe", "delete", "board"]),
            ("repo", &["view", "list", "clone", "fork", "create", "archive", "delete", "search", "contributors", "members", "transfer", "publish", "mirror"]),
            ("ci", &["list", "status", "view", "trace", "run", "retry", "cancel", "lint", "get", "delete"]),
            ("pipeline", &["list", "status", "view", "trace", "run", "retry", "cancel", "get", "delete"]),
            ("release", &["view", "list", "download", "upload", "delete"]),
            ("label", &["list", "create", "edit", "delete"]),
            ("variable", &["list", "get", "set", "update", "delete", "export"]),
            ("schedule", &["list", "create", "delete", "run", "update"]),
            ("snippet", &["create"]),
            ("ssh-key", &["list", "add", "delete", "get"]),
            ("deploy-key", &["list", "add", "delete", "get"]),
            ("user", &["events"]),
            ("config", &["set", "edit"]),
            ("milestone", &["list", "view", "create", "edit", "delete"]),
            ("incident", &["list", "view", "close", "reopen", "note", "subscribe", "unsubscribe"]),
        ],
    ),
];

/// Commands of one word that run nothing and say what they are about: no verb follows.
const NOUN_ONLY: &[(&str, &[&str])] = &[
    ("gh", &["status", "browse", "completion", "help", "version"]),
    ("glab", &["check-update", "completion", "help", "version"]),
];

/// The flags that take no value, and may stand before the verb.
const NO_VALUE_FLAGS: [&str; 3] = ["-h", "--help", "--version"];

/// Whether `noun` begins some known command of `name`, or a held one.
fn known_noun(name: &str, noun: &str) -> bool {
    KNOWN
        .iter()
        .filter(|(cli, _)| *cli == name)
        .flat_map(|(_, rows)| rows.iter())
        .any(|(n, _)| *n == noun)
        || merge::held_commands(name)
            .iter()
            .any(|c| c.first() == Some(&noun))
}

/// Whether `noun verb` is a command the walk knows: a read or an ordinary write of [`KNOWN`], or
/// one the floor already holds (read by the rest of the floor, which decides it).
pub(super) fn known_command(name: &str, noun: &str, verb: &str) -> bool {
    KNOWN
        .iter()
        .filter(|(cli, _)| *cli == name)
        .flat_map(|(_, rows)| rows.iter())
        .any(|(n, verbs)| *n == noun && verbs.contains(&verb))
        || merge::held_commands(name)
            .iter()
            .any(|c| c.len() > 1 && c[0] == noun && c[1] == verb)
        || super::PUBLISH_SINCE
            .iter()
            .any(|(cli, n, v)| *cli == name && *n == noun && *v == verb)
}

/// Whether `word` is a command of one word the walk knows, or a held one that is a noun alone
/// (`auth`, `alias`).
fn noun_alone(name: &str, word: &str) -> bool {
    NOUN_ONLY
        .iter()
        .any(|(cli, words)| *cli == name && words.contains(&word))
        || merge::held_commands(name)
            .iter()
            .any(|c| c.len() == 1 && c[0] == word)
}

/// Where the walk of a forge call's words ended.
enum Ended {
    /// A word is not plainly one known word where the call cannot be read: the refusal.
    Held(String),
    /// A known noun and verb, every word before them a plain literal.
    Known,
    /// `api`, whose every word [`super::merge::shell_filled_api_reason`] reads.
    Api,
    /// The words ran out before the verb: more may be added after them.
    Exhausted,
}

/// The refusal of a word that is not plainly the one known word its place needs.
fn refusal(name: &str, word: &str, how: &str) -> String {
    format!(
        "{} {how} `{word}` is empty, filled in by the shell, or not a command purlis knows, so \
         purlis can't tell which `{name}` command this is. Write the noun and the verb out as \
         plain words (`{name} pr view 12`), and put every flag after them.",
        merge::UNREADABLE
    )
}

/// Walks the words of one forge call (`call[0]` is the program) up to its verb: **the allowlist**.
/// Every word up to and including the verb must be a non-empty [`Fill::Literal`]; a flag before
/// the verb must take no value, or be the repository's (`-R`, `--repo`) with a plain value; the
/// noun and the verb must be a command [`KNOWN`] names. `spelled` is each word as the source
/// spells it, or empty where nothing measured them.
fn walk(name: &str, call: &[String], spelled: &[String]) -> Ended {
    let args = call.get(1..).unwrap_or(&[]);
    let fill = |k: usize| fill_of(&args[k], spelled.get(k + 1).map_or("", String::as_str));
    let mut noun: Option<&str> = None;
    let mut k = 0;
    while k < args.len() {
        let text = args[k].as_str();
        let f = fill(k);
        if f != Fill::Literal || text.is_empty() {
            return Ended::Held(refusal(name, text, "The word"));
        }
        if text.starts_with('-') {
            if text == "-R" || text == "--repo" {
                // A value is one word whatever it holds (`-R "$REPO"`), but not an empty literal
                // and not one the shell may split or drop.
                if let Some(value) = args.get(k + 1)
                    && (fill(k + 1) == Fill::Open || value.is_empty() && fill(k + 1) == Fill::Literal)
                {
                    return Ended::Held(refusal(name, value, "The value of the repository flag"));
                }
                k += 2;
                continue;
            }
            let tail = text
                .strip_prefix("--repo=")
                .or_else(|| text.strip_prefix("-R").filter(|_| !text.starts_with("--")));
            let plain_glued = tail.is_some_and(|t| !t.is_empty());
            if !plain_glued && !NO_VALUE_FLAGS.contains(&text) {
                return Ended::Held(refusal(name, text, "The flag"));
            }
            k += 1;
            continue;
        }
        let Some(n) = noun else {
            if text == "api" {
                return Ended::Api;
            }
            if noun_alone(name, text) {
                return Ended::Known;
            }
            if !known_noun(name, text) {
                return Ended::Held(refusal(name, text, "The command"));
            }
            noun = Some(text);
            k += 1;
            continue;
        };
        return if known_command(name, n, text) {
            Ended::Known
        } else {
            Ended::Held(refusal(name, text, "The verb"))
        };
    }
    Ended::Exhausted
}

/// The refusal of a forge call (`call[0]` is `gh` or `glab`) whose words, at or before its verb,
/// are not plain known ones.
pub(super) fn reason(name: &str, call: &[String], spelled: &[String]) -> Option<String> {
    match walk(name, call, spelled) {
        Ended::Held(why) => Some(why),
        _ => None,
    }
}

/// Whether words added to this call, as stdin or arguments a program in front of it hands it
/// (`echo pr merge 5 | gxargs gh`, `fd -x gh api`), could still give it a noun and a verb, or
/// a method: it has not reached its verb, or it is `api`, whose method is a flag.
pub(super) fn may_be_completed(name: &str, call: &[String], spelled: &[String]) -> bool {
    matches!(walk(name, call, spelled), Ended::Exhausted | Ended::Api)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_owned).collect()
    }

    /// **The default is hold** (#7423, round 11): over every noun and verb the tables name and
    /// some they do not, a literal call passes the walk exactly when [`known_command`] says so.
    #[test]
    fn a_noun_and_verb_no_table_names_is_held() {
        let mut nouns: Vec<&str> = vec!["frob", "pr", "mr", "alias-of-merge", "extension"];
        let mut verbs: Vec<&str> = vec!["merge", "frob", "m", "land", "view", "5", "MERGE", "sync"];
        for (_, rows) in KNOWN {
            for (n, vs) in rows.iter() {
                nouns.push(n);
                verbs.extend(vs.iter());
            }
        }
        for name in ["gh", "glab"] {
            for noun in &nouns {
                for verb in &verbs {
                    let call = words(&format!("{name} {noun} {verb}"));
                    let ended = walk(name, &call, &[]);
                    let known = known_command(name, noun, verb)
                        || noun_alone(name, noun)
                        || *noun == "api";
                    assert_eq!(
                        matches!(ended, Ended::Known | Ended::Api),
                        known,
                        "{name} {noun} {verb}"
                    );
                }
            }
        }
    }
}

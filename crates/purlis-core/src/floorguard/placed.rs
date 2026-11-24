// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One rule for the words of a forge call up to its verb** (#7423, round 10).
//!
//! Nine rounds closed spellings one at a time (a quoted flag, `''$V''`, `${V%/}`, a value flag
//! before the noun, a `-R` value the shell fills in), and each round's confirm found the next. This
//! is the rule they were all instances of, read in one place and used by every path that reads a
//! `gh` or `glab` call: the direct call, the call behind a program no table knows, and the `for`
//! over a forge answer.
//!
//! **A forge CLI reads its noun and its verb from the first words that are no flag.** A word the
//! shell may make into more words, or into a flag, standing where the noun or the verb may stand
//! (or as a flag's value, or as a flag) makes the call unreadable, so it is held. What is read is
//! the lexer's word, with its quotes: a word is [`Fill::Literal`] only when it has no expansion
//! outside single quotes, and [`Fill::OneWord`] when its only expansions are plain ones inside
//! double quotes, which make exactly one word whose text is unknown.
//!
//! * Where a noun or a verb stands, only a [`Fill::Literal`] word is read. A flag's value
//!   (`-R "$REPO"`) may be one word.
//! * A flag before the noun that is not known to take no value may take the word after it, so
//!   the noun and the verb are each one word further on for every such flag, and the walk
//!   reaches that far.
//! * The walk ends once the literal noun and verb are established (a known read stays a read
//!   whatever follows it), or at `api`, whose every word is read by
//!   [`super::merge::shell_filled_api_reason`].

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
        return if merge::opaque(text) {
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
            '$' | '`' | '*' | '?' | '[' | '(' => return Fill::Open,
            '~' | '^' if i == 0 => return Fill::Open,
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

/// Where the walk of a forge call's words ended.
enum Ended {
    /// A word the shell fills in stands where the call cannot be read: the refusal.
    Held(String),
    /// A literal noun and verb, with no flag before them that may take a value.
    Established,
    /// `api`, whose every word [`super::merge::shell_filled_api_reason`] reads.
    Api,
    /// The words ran out before the verb: more may be added after them.
    Exhausted,
    /// Past the last word that could be the verb, a flag before the noun having shifted it.
    Past,
}

/// Whether a flag is known to take no value, or to take it glued (`-Ro/r`, `--repo=o/r`).
fn takes_nothing_apart(flag: &str) -> bool {
    flag.starts_with("-R")
        || flag.starts_with("--repo=")
        || matches!(flag, "-h" | "--help" | "--version")
}

/// The refusal of a word the shell fills in where `name` reads a command, a flag or its value.
fn refusal(name: &str, word: &str, how: &str) -> String {
    format!(
        "{} {how} `{word}` is filled in by the shell, so purlis can't tell which `{name}` \
         command this is. Write the noun and the verb out (`{name} pr view 12`), or put `{name}` \
         first and a filled-in value after them, or double-quote a flag's value.",
        merge::UNREADABLE
    )
}

/// Walks the words of one forge call (`call[0]` is the program) up to its verb. `spelled` is
/// each word as the source spells it, or empty where nothing measured them.
fn walk(name: &str, call: &[String], spelled: &[String]) -> Ended {
    let args = call.get(1..).unwrap_or(&[]);
    let fill = |k: usize| fill_of(&args[k], spelled.get(k + 1).map_or("", String::as_str));
    let mut positionals: Vec<&str> = Vec::new();
    // Flags that may take the word after them: each shifts the noun and the verb one word on.
    let mut unknown = 0usize;
    let mut k = 0;
    while k < args.len() {
        let text = args[k].as_str();
        let f = fill(k);
        if text.starts_with('-') {
            if f == Fill::Open {
                return Ended::Held(refusal(name, text, "The flag"));
            }
            if text == "-R" || text == "--repo" {
                if k + 1 < args.len() && fill(k + 1) == Fill::Open {
                    return Ended::Held(refusal(
                        name,
                        &args[k + 1],
                        "The value of the repository flag",
                    ));
                }
                k += 2;
                continue;
            }
            if !takes_nothing_apart(text) {
                unknown += 1;
            }
            k += 1;
            continue;
        }
        let p = positionals.len();
        if p > unknown + 1 {
            return Ended::Past;
        }
        if f == Fill::Literal && text == "api" && p == 0 && unknown == 0 {
            return Ended::Api;
        }
        if f != Fill::Literal {
            let may_be_noun = p <= unknown;
            let nouns = &positionals[..p.min(unknown + 1)];
            let may_be_verb = p >= 1 && nouns.iter().any(|n| super::noun_has_a_held_verb(name, n));
            if may_be_noun || may_be_verb {
                return Ended::Held(refusal(name, text, "The word"));
            }
        }
        positionals.push(text);
        if positionals.len() == 2 && unknown == 0 {
            return Ended::Established;
        }
        k += 1;
    }
    Ended::Exhausted
}

/// The refusal of a forge call (`call[0]` is `gh` or `glab`) one of whose words, at or before its
/// verb, the shell fills in where the call cannot then be read.
pub(super) fn reason(name: &str, call: &[String], spelled: &[String]) -> Option<String> {
    match walk(name, call, spelled) {
        Ended::Held(why) => Some(why),
        _ => None,
    }
}

/// Whether more words added to this call, as stdin or arguments a program in front of it hands
/// it (`echo pr merge 5 | gxargs gh`), could still give it a noun and a verb: it has not reached
/// its verb, or it is `api`, whose method is a flag.
pub(super) fn may_be_completed(name: &str, call: &[String], spelled: &[String]) -> bool {
    matches!(walk(name, call, spelled), Ended::Exhausted | Ended::Api)
}

/// Whether the first word of the call's arguments that is no flag (the value of `-R` taken) is a
/// literal one, and begins a command the floor holds: `pr`, `mr`, `api`, `alias`, `release`.
pub(super) fn names_a_held_noun(name: &str, after: &[String], spelled: &[String]) -> bool {
    let mut k = 0;
    while let Some(word) = after.get(k) {
        if word == "-R" || word == "--repo" {
            k += 2;
            continue;
        }
        if word.starts_with('-') {
            k += 1;
            continue;
        }
        let literal = fill_of(word, spelled.get(k).map_or("", String::as_str)) == Fill::Literal;
        return literal
            && merge::held_commands(name)
                .iter()
                .any(|command| command.first() == Some(&word.as_str()));
    }
    false
}

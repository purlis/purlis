// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The keys purlis writes in a persona's definition, read and written as the YAML they are**
//! (#3244): its `skills:` (SK-5) and `pinned-skills:` (SK-15) lists and its `profile:`. Read by
//! the one strict frontmatter reader ([`purlis_skills::fenced`]), failing closed, so a `---`
//! inside a value, a quoted list or a list written as YAML reads as a YAML reader takes it.
//!
//! **A write changes the one entry and keeps every other line byte for byte**: the other keys,
//! their comments and order, blank lines and the body. The entry keeps its own style where it can:
//! a comma list stays one (`skills: a, b`), a flow list stays one (`skills: [a, b]`), a block
//! list keeps its items' indent (`  - a`), and a comment at the end of a one-line entry stays at
//! its end. What it cannot keep: a comment inside a block list, or on an item's line, goes with
//! the items it was among; and a value written over several lines in any other way (a folded or
//! quoted scalar running on) is written again on one line. A new key goes last, as a comma
//! list. Before anything is written the new text is read again: it must say the new value and
//! every other key exactly as before, or nothing is written ([`NotWritten`]).

use purlis_skills::Refusal;
use purlis_skills::fenced::{Entry, Fence, Value, entries, fence};

/// The persona keys purlis writes a list in: each one a list of names.
const LISTS: [&str; 2] = [
    crate::skills::persona::KEY,
    crate::skills::persona::PINS_KEY,
];

/// The persona keys purlis writes one word in.
const WORDS: [&str; 1] = [crate::personaprofile::KEY];

/// Why a persona's definition was not changed. Its [`std::fmt::Display`] follows the file's
/// name in a sentence: "`<file>` has no frontmatter to hold a `skills:` line".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotWritten {
    /// The text does not start with a frontmatter fenced by `---` lines.
    NoFrontmatter(&'static str),
    /// Its frontmatter is not YAML purlis reads: what the reader said.
    Unread(Refusal),
    /// The key's value is no list of names, or no word.
    NotAList(&'static str),
    /// The new value would not read back as itself, or another key would read differently.
    WouldChange(&'static str),
}

impl std::fmt::Display for NotWritten {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFrontmatter(key) => {
                write!(f, "has no frontmatter to hold a `{key}:` line")
            }
            Self::Unread(why) => write!(f, "cannot be changed: {why}"),
            Self::NotAList(key) => write!(
                f,
                "cannot be changed: its `{key}:` is not something purlis writes over"
            ),
            Self::WouldChange(key) => write!(
                f,
                "cannot be changed: its `{key}:` cannot be written so it reads back the same"
            ),
        }
    }
}

/// The names `key`'s value lists: a comma list's, or a YAML list's, each stripped, blanks
/// dropped. `None` where it is neither.
fn names(value: &Value) -> Option<Vec<String>> {
    let strip = |one: &str| crate::memstore::py_strip(one).to_owned();
    let all: Vec<String> = match (value.text(), value.items()) {
        (Some(text), _) => text.split(',').map(strip).collect(),
        (None, Some(items)) => items.into_iter().map(strip).collect(),
        (None, None) => return None,
    };
    Some(all.into_iter().filter(|one| !one.is_empty()).collect())
}

/// The allow-listed key `key` is, as its `'static` name.
fn allowed(key: &str, of: &[&'static str]) -> &'static str {
    of.iter()
        .copied()
        .find(|one| *one == key)
        .unwrap_or_else(|| panic!("`{key}:` is not a persona key purlis writes"))
}

/// **The names the persona definition `text` lists under `key`** (`skills:`,
/// `pinned-skills:`), in order: none where it has no frontmatter or no such key, and refused
/// where its frontmatter is not YAML purlis reads or the value is no list.
pub(crate) fn list(text: &str, key: &str) -> Result<Vec<String>, NotWritten> {
    let key = allowed(key, &LISTS);
    let Some(fence) = fence(text) else {
        return Ok(Vec::new());
    };
    let found = entries(&text[fence.yaml]).map_err(NotWritten::Unread)?;
    match found.iter().find(|entry| entry.key == key) {
        None => Ok(Vec::new()),
        Some(entry) => names(&entry.value).ok_or(NotWritten::NotAList(key)),
    }
}

/// **`text` with `key`'s list saying `items`**, or without the key for none.
pub(crate) fn with_list(text: &str, key: &str, items: &[String]) -> Result<String, NotWritten> {
    let key = allowed(key, &LISTS);
    let said = (!items.is_empty()).then_some(Said::List(items));
    rewrite(text, key, said)
}

/// **`text` with `key` saying `word`**, or without the key for `None`.
pub(crate) fn with_word(text: &str, key: &str, word: Option<&str>) -> Result<String, NotWritten> {
    let key = allowed(key, &WORDS);
    rewrite(text, key, word.map(Said::Word))
}

/// What a key is to say.
#[derive(Clone, Copy)]
enum Said<'a> {
    List(&'a [String]),
    Word(&'a str),
}

/// How a list is written.
#[derive(Clone, PartialEq, Eq)]
enum Style {
    /// `key: a, b`
    Comma,
    /// `key: [a, b]`
    Flow,
    /// `key:` and a line per item, each starting with this (`  - `).
    Block(String),
}

/// `text` with `key` saying `said`, or without it, read back before it is given.
fn rewrite(text: &str, key: &'static str, said: Option<Said<'_>>) -> Result<String, NotWritten> {
    let Some(Fence { yaml, body }) = fence(text) else {
        return Err(NotWritten::NoFrontmatter(key));
    };
    let block = &text[yaml.clone()];
    let found = entries(block).map_err(NotWritten::Unread)?;
    let lines: Vec<&str> = block.split_inclusive('\n').collect();
    let at = found.iter().position(|entry| entry.key == key);
    let (span, old) = match at {
        Some(at) => (Some(span(&found, at, &lines)), Some(&found[at])),
        None => (None, None),
    };
    if let (Some(old), Some(Said::List(_))) = (old, said)
        && names(&old.value).is_none()
    {
        return Err(NotWritten::NotAList(key));
    }
    let attempt = |quote: bool| -> Option<String> {
        let entry = match said {
            None => String::new(),
            Some(said) => written(key, said, old, span.clone(), &lines, quote)?,
        };
        let mut out = String::with_capacity(text.len() + entry.len());
        out.push_str(&text[..yaml.start]);
        match span.clone() {
            Some(span) => {
                lines[..span.start].iter().for_each(|line| out.push_str(line));
                out.push_str(&entry);
                lines[span.end..].iter().for_each(|line| out.push_str(line));
            }
            None => {
                out.push_str(block);
                if !block.is_empty() && !block.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&entry);
            }
        }
        out.push_str(&text[yaml.end..]);
        reads_as(&out, &text[body..], &found, key, said).then_some(out)
    };
    attempt(false)
        .or_else(|| attempt(true))
        .ok_or(NotWritten::WouldChange(key))
}

/// The lines entry `at` of `found` takes, from its key's line to the last line before the next
/// entry that is not blank and no comment: a comment or a blank line between two entries is
/// kept.
fn span(found: &[Entry], at: usize, lines: &[&str]) -> std::ops::Range<usize> {
    let start = found[at].line;
    let mut end = found.get(at + 1).map_or(lines.len(), |next| next.line);
    while end > start + 1 {
        let line = lines[end - 1].trim();
        if line.is_empty() || line.starts_with('#') {
            end -= 1;
        } else {
            break;
        }
    }
    start..end
}

/// The lines `key` is written in to say `said`, in the style of `old` where there was one;
/// `quote` writes each item quoted. `None` where the style cannot hold it.
fn written(
    key: &str,
    said: Said<'_>,
    old: Option<&Entry>,
    span: Option<std::ops::Range<usize>>,
    lines: &[&str],
    quote: bool,
) -> Option<String> {
    let first = span.as_ref().map(|span| lines[span.start]);
    let one_line = span.as_ref().is_some_and(|span| span.len() == 1);
    // The key as written, up to its colon, and what follows the colon on its line.
    let (prefix, rest) = match first {
        Some(line) => {
            let colon = line.find(':')?;
            (&line[..=colon], line[colon + 1..].trim_end_matches(['\n', '\r']))
        }
        None => ("", ""),
    };
    let prefix = if prefix.is_empty() {
        format!("{key}:")
    } else {
        prefix.to_owned()
    };
    let style = match (old, said) {
        (Some(old), Said::List(_)) if old.value.items().is_some() => {
            if rest.trim_start().starts_with('[') {
                Style::Flow
            } else {
                let item = span
                    .as_ref()
                    .and_then(|span| lines.get(span.start + 1..span.end))
                    .and_then(|below| below.iter().find_map(|line| item_prefix(line)))
                    .unwrap_or_else(|| "  - ".to_owned());
                Style::Block(item)
            }
        }
        _ => Style::Comma,
    };
    let comment = match (&style, one_line, old) {
        (Style::Block(_), _, Some(_)) if rest.trim_start().starts_with('#') => rest.to_owned(),
        (_, true, Some(old)) => trailing_comment(rest, &old.value),
        _ => String::new(),
    };
    let one = |item: &str| if quote { quoted(item) } else { item.to_owned() };
    Some(match (said, style) {
        (Said::Word(word), _) => format!("{prefix} {}{comment}\n", one(word)),
        (Said::List(items), Style::Comma) => {
            if quote {
                return None;
            }
            format!("{prefix} {}{comment}\n", items.join(", "))
        }
        (Said::List(items), Style::Flow) => {
            let items: Vec<String> = items.iter().map(|item| one(item)).collect();
            format!("{prefix} [{}]{comment}\n", items.join(", "))
        }
        (Said::List(items), Style::Block(item)) => {
            let mut out = format!("{prefix}{comment}\n");
            for one_item in items {
                out.push_str(&format!("{item}{}\n", one(one_item)));
            }
            out
        }
    })
}

/// What a block list's item line starts with, up to its first word (`  - `), where `line` is one.
fn item_prefix(line: &str) -> Option<String> {
    let indent = line.len() - line.trim_start().len();
    let after = line[indent..].strip_prefix('-')?;
    let gap = after.len() - after.trim_start_matches([' ', '\t']).len();
    (gap > 0).then(|| line[..indent + 1 + gap].to_owned())
}

/// The comment at the end of a one-line entry whose text after its colon is `rest` and whose
/// value is `value`, with the blanks before it: what is left once the value is read to its end.
/// Empty where it has none.
fn trailing_comment(rest: &str, value: &Value) -> String {
    for (at, _) in rest.match_indices('#') {
        let before = &rest[..at];
        if !before.ends_with([' ', '\t']) {
            continue;
        }
        let kept = before.trim_end_matches([' ', '\t']);
        let again = entries(&format!("k:{kept}\n"));
        if again.is_ok_and(|found| found.len() == 1 && found[0].value == *value) {
            return rest[kept.len()..].to_owned();
        }
    }
    String::new()
}

/// `text` as a YAML double-quoted scalar.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Whether `out` reads as it should: its body still `body`, `key` saying `said` (or gone), and
/// every other entry of `before` as it was, in its order.
fn reads_as(
    out: &str,
    body: &str,
    before: &[Entry],
    key: &str,
    said: Option<Said<'_>>,
) -> bool {
    let Some(fence) = fence(out) else {
        return false;
    };
    let Ok(after) = entries(&out[fence.yaml]) else {
        return false;
    };
    if &out[fence.body..] != body {
        return false;
    }
    let others = |all: &[Entry]| -> Vec<(String, Value)> {
        all.iter()
            .filter(|entry| entry.key != key)
            .map(|entry| (entry.key.clone(), entry.value.clone()))
            .collect()
    };
    if others(before) != others(&after) {
        return false;
    }
    let now = after.iter().find(|entry| entry.key == key);
    match (said, now) {
        (None, None) => true,
        (Some(Said::Word(word)), Some(now)) => now.value.text() == Some(word),
        (Some(Said::List(items)), Some(now)) => names(&now.value).as_deref() == Some(items),
        _ => false,
    }
}

#[cfg(test)]
mod tests;

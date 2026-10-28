// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The line format's one parser** (#3244, #3460): where a frontmatter is, the lines it holds,
//! the `key: value` pairs they say, the body below it, and the one writer that changes a key's
//! line and leaves every other byte as written.
//!
//! The format is line-based, NOT YAML (`docs/plane-format.md`, ADR 0061): no quote stripping, no
//! nesting, no comments. Its fences are LINES:
//!
//! - a frontmatter opens only where the text's first line is `---`, and
//! - it ends at the first later line that is `---`,
//!
//! each a fence when it is exactly three hyphens at the start of the line, with nothing after
//! them but spaces or tabs. A `---` anywhere else, inside a value (`role: a --- b`) or indented,
//! is text. A text whose first line is no fence, or whose fence no later line closes, has no
//! frontmatter: it is all body. A line is what Python's `str.splitlines` calls one, `\r\n` as one
//! break, so every reader and writer here splits a text the same way.
//!
//! Every persona key, a `SKILL.md`'s `name:` and `description:`, a curation action, a session
//! record and the rewrites of them read their frontmatter through [`Fence`].

/// One line of a text, as byte offsets into it: `start..end` is the line without its break,
/// `end..next` the break (empty on a last line that has none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub next: usize,
}

/// A key's line in a frontmatter: the key and the value as a reader reads them (each stripped as
/// Python's `str.strip` strips), and where the line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    pub key: &'a str,
    pub value: &'a str,
    pub line: Line,
}

/// **A text's frontmatter**: the lines between its two fences, and the body below them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence<'a> {
    text: &'a str,
    /// The first byte of the first line under the opening fence.
    open: usize,
    /// The first byte of the closing fence's line.
    close: usize,
    /// The first byte after the closing fence's line and its break.
    body: usize,
    /// Where the opening fence's line ends: its break runs from here to [`Self::open`].
    open_end: usize,
}

impl<'a> Fence<'a> {
    /// The frontmatter of `text`, or `None` where it has none: its first line is no fence, or no
    /// later line closes it.
    pub fn find(text: &'a str) -> Option<Self> {
        let all = lines(text);
        let (first, rest) = all.split_first()?;
        if !is_fence(&text[first.start..first.end]) {
            return None;
        }
        let close = rest
            .iter()
            .find(|line| is_fence(&text[line.start..line.end]))?;
        Some(Self {
            text,
            open: first.next,
            close: close.start,
            body: close.next,
            open_end: first.end,
        })
    }

    /// Where the lines between the fences are, as byte offsets into the text: from the first
    /// byte under the opening fence to the first of the closing one.
    pub fn span(&self) -> (usize, usize) {
        (self.open, self.close)
    }

    /// The lines between the fences, as written, breaks and all.
    pub fn block(&self) -> &'a str {
        &self.text[self.open..self.close]
    }

    /// Each line between the fences, as offsets into the whole text.
    pub fn lines(&self) -> Vec<Line> {
        lines(self.block())
            .into_iter()
            .map(|line| Line {
                start: line.start + self.open,
                end: line.end + self.open,
                next: line.next + self.open,
            })
            .collect()
    }

    /// The text of each line between the fences, without its break.
    pub fn line_texts(&self) -> Vec<&'a str> {
        self.lines()
            .into_iter()
            .map(|line| &self.text[line.start..line.end])
            .collect()
    }

    /// Every `key: value` line, in the order written: a line holding a `:`, split at the first
    /// one, both halves stripped, kept when its key is not empty. Any other line (a blank, a
    /// `# note`, a `- item`) says no key.
    pub fn entries(&self) -> Vec<Entry<'a>> {
        self.lines()
            .into_iter()
            .filter_map(|line| {
                let (key, value) = entry_of(&self.text[line.start..line.end])?;
                Some(Entry { key, value, line })
            })
            .collect()
    }

    /// [`Self::entries`] as owned pairs.
    pub fn pairs(&self) -> Vec<(String, String)> {
        self.entries()
            .into_iter()
            .map(|entry| (entry.key.to_owned(), entry.value.to_owned()))
            .collect()
    }

    /// Everything below the closing fence's line, as written.
    pub fn body(&self) -> &'a str {
        &self.text[self.body..]
    }

    /// The break the opening fence's line ends with, `\n` or `\r\n`: what a line the writer
    /// adds ends with, so a file keeps the breaks it was written with.
    fn newline(&self) -> &'a str {
        &self.text[self.open_end..self.open]
    }
}

/// Whether `text` opens a frontmatter that no later line closes: the case where a reader finds
/// none and the person meant one, which `persona lint` names.
pub fn unclosed(text: &str) -> bool {
    lines(text)
        .first()
        .is_some_and(|first| is_fence(&text[first.start..first.end]))
        && Fence::find(text).is_none()
}

/// A line that is a fence: three hyphens at its start, nothing after them but spaces or tabs.
fn is_fence(line: &str) -> bool {
    line.trim_end_matches([' ', '\t']) == "---"
}

/// The key and value a line says, or `None` where it says none.
fn entry_of(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = py_strip(key);
    (!key.is_empty()).then(|| (key, py_strip(value)))
}

/// Why [`with_keys`] wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unwritten {
    /// The text has no frontmatter to hold the line.
    NoFrontmatter(String),
    /// The text opens a frontmatter that no `---` line closes ([`unclosed`]): it reads as all
    /// body, and a frontmatter added above it would make the person's keys body for good.
    Unclosed(String),
    /// The value cannot be one line that reads back as itself.
    Value { key: String, value: String },
    /// What would be written does not read back as asked. A defect in this writer, said rather
    /// than written.
    ReadsBack(String),
}

impl std::fmt::Display for Unwritten {
    /// The end of a sentence that follows the file's name.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFrontmatter(key) => write!(f, "has no frontmatter to hold a `{key}:` line"),
            Self::Unclosed(key) => write!(
                f,
                "opens a frontmatter that no `---` line closes, so it has none to hold a \
                 `{key}:` line (end it with a line that is only `---`)"
            ),
            Self::Value { key, value } => write!(
                f,
                "cannot hold `{key}: {}` as one line that reads back the same: a frontmatter \
                 value may hold no line break and start or end with no blank",
                value.escape_debug()
            ),
            Self::ReadsBack(key) => write!(
                f,
                "would not read back its `{key}:` line as written, so purlis wrote nothing"
            ),
        }
    }
}

impl std::error::Error for Unwritten {}

/// Whether `value` can be written on one line and read back as itself: no line break, and
/// nothing a reader would strip off its ends.
pub fn holds(value: &str) -> bool {
    !value.chars().any(is_line_break) && py_strip(value) == value
}

/// **`text` with each of `keys` saying its value**, or with no line of it for `None`: the one
/// writer of the line format.
///
/// A key's first line is rewritten where it stands, keeping its break; any later line of it goes,
/// since a later line would answer instead. A key line with an empty value takes the `- item`
/// lines right under it along, as they are its value (a block list), so none is left to read
/// as another key's. A key with no line gains one, last in the
/// frontmatter. Every other byte, the person's other lines, notes and order and the body, is left
/// as written. What is written is read back before it is answered, and a value that would not
/// read back as itself is refused, so what purlis writes is what it reads.
pub fn with_keys(text: &str, keys: &[(&str, Option<&str>)]) -> Result<String, Unwritten> {
    let Some(fence) = Fence::find(text) else {
        let key = keys.first().map_or("", |(key, _)| key).to_owned();
        return Err(if unclosed(text) {
            Unwritten::Unclosed(key)
        } else {
            Unwritten::NoFrontmatter(key)
        });
    };
    for (key, value) in keys {
        debug_assert!(entry_of(&format!("{key}:")).map(|(k, _)| k) == Some(*key));
        if let Some(value) = value
            && !holds(value)
        {
            return Err(Unwritten::Value {
                key: (*key).to_owned(),
                value: (*value).to_owned(),
            });
        }
    }
    let said = |key: &str, value: &str| {
        if value.is_empty() {
            format!("{key}:")
        } else {
            format!("{key}: {value}")
        }
    };
    let (open, close) = fence.span();
    let mut out = String::with_capacity(text.len() + 64);
    out.push_str(&text[..open]);
    let mut placed = vec![false; keys.len()];
    // Set while the lines are the `- item` lines under a key being rewritten: they are its
    // value, so they go with the line they are under.
    let mut items_of_ours = false;
    for line in fence.lines() {
        let whole = &text[line.start..line.next];
        let written = &text[line.start..line.end];
        if items_of_ours && written.trim_start().starts_with('-') {
            continue;
        }
        items_of_ours = false;
        let Some((at, empty)) = entry_of(written).and_then(|(key, value)| {
            keys.iter()
                .position(|(k, _)| *k == key)
                .map(|at| (at, value.is_empty()))
        }) else {
            out.push_str(whole);
            continue;
        };
        items_of_ours = empty;
        if let (Some(value), false) = (keys[at].1, placed[at]) {
            out.push_str(&said(keys[at].0, value));
            out.push_str(&text[line.end..line.next]);
        }
        placed[at] = true;
    }
    for (at, (key, value)) in keys.iter().enumerate() {
        let Some(value) = value.filter(|_| !placed[at]) else {
            continue;
        };
        out.push_str(&said(key, value));
        out.push_str(fence.newline());
    }
    out.push_str(&text[close..]);
    reads_back(text, &out, keys)?;
    Ok(out)
}

/// Whether `written`, `text` with `keys` set, reads as asked: the same body, each key the value
/// asked or no line of it, and every other key as it was.
fn reads_back(text: &str, written: &str, keys: &[(&str, Option<&str>)]) -> Result<(), Unwritten> {
    let first_key = || keys.first().map_or("", |(key, _)| key).to_owned();
    let (Some(before), Some(after)) = (Fence::find(text), Fence::find(written)) else {
        return Err(Unwritten::ReadsBack(first_key()));
    };
    if before.body() != after.body() {
        return Err(Unwritten::ReadsBack(first_key()));
    }
    let others = |fence: &Fence| -> Vec<(String, String)> {
        fence
            .pairs()
            .into_iter()
            .filter(|(key, _)| !keys.iter().any(|(k, _)| k == key))
            .collect()
    };
    if others(&before) != others(&after) {
        return Err(Unwritten::ReadsBack(first_key()));
    }
    let pairs = after.pairs();
    for (key, value) in keys {
        let read: Vec<&str> = pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect();
        let asked: Vec<&str> = value.iter().copied().collect();
        if read != asked {
            return Err(Unwritten::ReadsBack((*key).to_owned()));
        }
    }
    Ok(())
}

/// `str.isspace()` for one character: Python's whitespace is Rust's and the four separator
/// controls U+001C-U+001F. The one answer every `str.strip` of the line format asks.
pub fn is_python_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{1f}')
}

/// `str.strip()`.
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(is_python_space)
}

/// `str.rstrip()`.
pub fn py_rstrip(text: &str) -> &str {
    text.trim_end_matches(is_python_space)
}

/// A character `str.splitlines()` breaks a line at: the one set every splitter of the line
/// format, a `workspace.md` and a scaffolded file asks (#3482).
pub fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\u{b}'
            | '\u{c}'
            | '\u{1c}'
            | '\u{1d}'
            | '\u{1e}'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// `str.splitlines()`, as spans: every line break Python knows, `\r\n` as one, and no empty last
/// line.
pub fn lines(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if !is_line_break(c) {
            continue;
        }
        let next = if c == '\r' && it.peek().is_some_and(|&(_, next)| next == '\n') {
            it.next();
            i + 2
        } else {
            i + c.len_utf8()
        };
        out.push(Line {
            start,
            end: i,
            next,
        });
        start = next;
    }
    if start < text.len() {
        out.push(Line {
            start,
            end: text.len(),
            next: text.len(),
        });
    }
    out
}

/// `str.splitlines()`: each line of `text` without its break ([`lines`]).
pub fn split_lines(text: &str) -> Vec<&str> {
    lines(text)
        .into_iter()
        .map(|line| &text[line.start..line.end])
        .collect()
}

#[cfg(test)]
mod tests;

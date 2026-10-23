// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Reading skill folders, and the briefing listing a harness without a skills route is given
//! (ADR 0063).

use std::path::{Path, PathBuf};

use crate::{Missing, Skill};

/// Where the skills sit inside the bundled plugin directory. Claude Code reads a plugin's
/// `skills/` itself, which is why it is this name.
pub const DIR_IN_BUNDLE: &str = "skills";

/// A skill's file inside its folder.
pub const FILE: &str = "SKILL.md";

/// The skills directory inside the bundled plugin at `plugin`, when it is there.
pub fn in_bundle(plugin: &Path) -> Option<PathBuf> {
    Some(plugin.join(DIR_IN_BUNDLE)).filter(|dir| dir.is_dir())
}

/// Every folder in `dir`: the skill it holds, or the folder and what it lacks to be one. A file
/// beside the folders is neither. In the order the directory lists them.
pub fn scan(dir: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|folder| folder.is_dir())
        .map(|folder| whole(&folder).map_err(|missing| (folder, missing)))
        .collect()
}

/// The skill in `folder`, or what it lacks.
fn whole(folder: &Path) -> Result<Skill, Missing> {
    let path = folder.join(FILE);
    let text = std::fs::read_to_string(&path).map_err(|_| Missing::File)?;
    parsed(&text, path)
}

/// **The skill a `SKILL.md` whose text is `text` holds**, at `path`, or what it lacks: by the
/// same rule every skill folder is read by. For a caller that read the file itself, through a
/// handle that follows no link (purlis-core's harness and repo skills, SK-6).
pub fn parsed(text: &str, path: PathBuf) -> Result<Skill, Missing> {
    let pairs = frontmatter(text);
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    Ok(Skill {
        name: value("name").ok_or(Missing::Name)?,
        description: value("description").ok_or(Missing::Description)?,
        path,
    })
}

/// Every skill in `dir`, by name. A folder with no `SKILL.md`, or one whose frontmatter names no
/// `name` or no `description`, is not a skill any harness would load, and is left out.
pub fn read(dir: &Path) -> Vec<Skill> {
    let mut skills: Vec<Skill> = scan(dir).into_iter().filter_map(Result::ok).collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// A `SKILL.md`'s frontmatter, as `key: value` pairs in the order written: one line per key,
/// which is how every built-in skill is written.
///
/// The one rule purlis reads a frontmatter by, a persona's too
/// (`purlis_core::personas::frontmatter`): the block between the first two `---`, split into
/// lines as Python's `str.splitlines` splits them, each line at its first `:`, key and value
/// stripped as Python's `str.strip` strips, and a line with an empty key dropped.
pub fn frontmatter(text: &str) -> Vec<(String, String)> {
    frontmatter_lines(text)
        .into_iter()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = py_strip(key);
            (!key.is_empty()).then(|| (key.to_owned(), py_strip(value).to_owned()))
        })
        .collect()
}

/// **Where a frontmatter is**, as byte offsets into `text`: from just after the opening `---`
/// to the start of the next `---`, or `None` where it has none. The one rule every reader and
/// writer of a frontmatter keeps ([`frontmatter`], and the rename of a skill's text in
/// [`crate::change`]).
pub(crate) fn frontmatter_block(text: &str) -> Option<(usize, usize)> {
    let rest = text.strip_prefix("---")?;
    let end = rest.find("---")?;
    Some((3, 3 + end))
}

/// The lines of a `SKILL.md`'s frontmatter, by the rule [`frontmatter`] reads it by: the block
/// [`frontmatter_block`] finds, stripped and split as Python splits lines. Empty where there is
/// no frontmatter. What a reader of a key that spans lines (a block list) reads.
pub(crate) fn frontmatter_lines(text: &str) -> Vec<&str> {
    frontmatter_block(text)
        .map(|(start, end)| lines(py_strip(&text[start..end])))
        .unwrap_or_default()
}

/// `str.strip()`: Python's whitespace is Rust's and the four separator controls U+001C-U+001F.
fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{1f}')
    })
}

/// `str.splitlines()`: every line break Python knows, `\r\n` as one, and no empty last line.
fn lines(text: &str) -> Vec<&str> {
    fn is_break(c: char) -> bool {
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
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if !is_break(c) {
            continue;
        }
        out.push(&text[start..i]);
        if c == '\r' && it.peek().is_some_and(|&(_, next)| next == '\n') {
            it.next();
            start = i + 2;
        } else {
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// The briefing section that lists `skills`, or none when there are none.
///
/// It is what a harness with skills of its own shows its model: each skill's name, when to use
/// it, and the file to read when it applies. The model reads that file with its own tools.
///
/// **A manual-only skill** ([`crate::declared`]) is purlis's equivalent of the harness's own
/// switch ([`crate::compat::ManualOnly::Listing`], SK-8): its description stays out, so the
/// model never picks it by itself, and it is named with its file on one line of its own, to read
/// only when the person names it. Its description costs nothing.
pub fn listing(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }
    let mut text = String::from(
        "⬢ **purlis's built-in skills** — this harness does not load them itself, so they are listed \
         here. When a request matches one, read its `SKILL.md` and follow it before acting; a \
         prompt that names a skill by its name means this list.",
    );
    let (manual, chosen): (Vec<&Skill>, Vec<&Skill>) = skills
        .iter()
        .partition(|skill| crate::declared(skill).manual);
    for skill in chosen {
        text.push_str(&format!(
            "\n- `{}` — {} (`{}`)",
            skill.name,
            skill.description,
            skill.path.display()
        ));
    }
    if !manual.is_empty() {
        let named: Vec<String> = manual
            .iter()
            .map(|skill| format!("`{}` (`{}`)", skill.name, skill.path.display()))
            .collect();
        text.push_str(&format!(
            "\n- Only when the person names one, never by your own choice: {}",
            named.join(", ")
        ));
    }
    Some(text)
}

#[cfg(test)]
mod tests;

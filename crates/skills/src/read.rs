// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Reading skill folders, and the briefing listing a harness without a skills route is given
//! (ADR 0063).

use std::path::{Path, PathBuf};

use crate::{Missing, Skill};

/// Where the skills sit inside the bundled plugin directory. Claude Code reads a plugin's
/// `skills/` itself, which is why it is this name.
pub const DIR_IN_BUNDLE: &str = "skills";

/// The variable a chat whose harness cannot load its skills is started with: the skills
/// directory, which its `SessionStart` briefing then lists ([`listed_from`]). Set by the app for
/// such a chat alone, so a harness that loads the skills itself is never briefed on them twice.
pub const LISTED_ENV: &str = "PURLIS_SKILLS_DIR";

/// A skill's file inside its folder.
const FILE: &str = "SKILL.md";

/// The skills directory inside the bundled plugin at `plugin`, when it is there.
pub fn in_bundle(plugin: &Path) -> Option<PathBuf> {
    Some(plugin.join(DIR_IN_BUNDLE)).filter(|dir| dir.is_dir())
}

/// Every folder in `dir`: the skill it holds, or the folder and what it lacks to be one. A file
/// beside the folders is neither. In the order the directory lists them.
pub(crate) fn scan(dir: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
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
    let pairs = frontmatter(&text);
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
/// The same rule a persona's frontmatter is read by (`purlis_core::personas::frontmatter`), and
/// purlis-core holds the two to one answer by test: the block between the first two `---`,
/// split into lines as Python's `str.splitlines` splits them, each line at its first `:`, key and
/// value stripped as Python's `str.strip` strips, and a line with an empty key dropped.
pub fn frontmatter(text: &str) -> Vec<(String, String)> {
    let Some(rest) = text.strip_prefix("---") else {
        return Vec::new();
    };
    let Some(end) = rest.find("---") else {
        return Vec::new();
    };
    lines(py_strip(&rest[..end]))
        .into_iter()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = py_strip(key);
            (!key.is_empty()).then(|| (key.to_owned(), py_strip(value).to_owned()))
        })
        .collect()
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
pub fn listing(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }
    let mut text = String::from(
        "⬢ **purlis's skills** — this harness does not load them itself, so they are listed \
         here. When a request matches one, read its `SKILL.md` and follow it before acting; a \
         prompt that names a skill by its name means this list.",
    );
    for skill in skills {
        text.push_str(&format!(
            "\n- `{}` — {} (`{}`)",
            skill.name,
            skill.description,
            skill.path.display()
        ));
    }
    Some(text)
}

/// The listing of the skills in the directory [`LISTED_ENV`] names, or none where it is unset.
pub fn listed_from(env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let dir = env(LISTED_ENV).filter(|dir| !dir.is_empty())?;
    listing(&read(Path::new(&dir)))
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;

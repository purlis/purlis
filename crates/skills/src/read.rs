// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Reading skill folders, and the briefing listing a harness without a skills route is given
//! (ADR 0063).

use std::path::{Path, PathBuf};

use crate::fence::Fence;
use crate::{Missing, Skill};

/// Where the skills sit inside the bundled plugin directory. Claude Code reads a plugin's
/// `skills/` itself, which is why it is this name.
pub const DIR_IN_BUNDLE: &str = "skills";

/// A skill's file inside its folder.
pub const FILE: &str = "SKILL.md";

/// The skills directory inside the bundled plugin at `plugin`, when it is there as a folder of
/// its own: a link there is never read through ([`scan`]).
pub fn in_bundle(plugin: &Path) -> Option<PathBuf> {
    Some(plugin.join(DIR_IN_BUNDLE)).filter(|dir| crate::held::is_plain_dir(dir))
}

/// **Every folder in the level `rel` under `root`**: the skill it holds, or the folder and what
/// it lacks to be one. A file beside the folders is neither. By folder name.
///
/// **Nothing is read through a link** (SK-36): `root` is a folder purlis trusts (the plane's
/// root, the bundle's plugin folder), and every part of `rel` below it, each skill's folder and
/// its `SKILL.md` are opened through the handle of the folder above without following a link
/// ([`crate::held::Folder`]). A folder that is a link to a folder is left out as a link
/// ([`Missing::Link`]), and so is the level where any part of `rel` is one, whatever it leads to:
/// what is listed of a skill, and every file a load names of it, is inside it.
///
/// **A level of more names than purlis reads of one folder** ([`crate::held::MOST_NAMES`],
/// SK-52) is read for none of them, and left out once as [`Missing::Crowded`]; one whose names
/// cannot be read whole lists nothing.
pub fn scan(root: &Path, rel: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
    use crate::held::{Folder, Follow, Kind, NotListed, Opened};
    let level = match Folder::walk(root, rel) {
        Opened::Folder(level) => level,
        Opened::Link => return vec![Err((root.join(rel), Missing::Link))],
        Opened::Absent => return Vec::new(),
    };
    let names = match level.names() {
        Ok(names) => names,
        Err(NotListed::Crowded) => return vec![Err((root.join(rel), Missing::Crowded))],
        Err(NotListed::Unreadable) => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        let path = level.shown().join(&name);
        match level.kind(&name) {
            Some(Kind::Folder) => {}
            // A link is told as one only where it leads to a folder: one to a file is a file
            // beside the folders, which is nothing.
            Some(Kind::Link { to_folder: true }) => {
                out.push(Err((path, Missing::Link)));
                continue;
            }
            _ => continue,
        }
        match level.child(&name, Follow::NoLink) {
            Opened::Folder(folder) => {
                out.push(whole(&folder).map_err(|missing| (path, missing)));
            }
            // It turned into a link since it was looked at.
            Opened::Link => out.push(Err((path, Missing::Link))),
            Opened::Absent => {}
        }
    }
    out
}

/// [`scan`] of the folder `dir`, its last part opened without following a link and the folders
/// above it trusted as they are: for a folder that is no level of a plane (the bundle's own).
pub fn scan_dir(dir: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
    match (dir.parent(), dir.file_name()) {
        (Some(parent), Some(name)) => scan(parent, Path::new(name)),
        _ => Vec::new(),
    }
}

/// The skill in the folder held at `folder`, or what it lacks: its `SKILL.md` read as every
/// skill's own file is ([`crate::handed::read_skill_md`]).
fn whole(folder: &crate::held::Folder) -> Result<Skill, Missing> {
    use crate::Refusal;
    let text = crate::handed::read_held(folder, FILE).map_err(|why| match why {
        Refusal::TooLarge => Missing::TooLarge,
        // A link, or a plain file with a second name (a hard link): either reaches past it.
        Refusal::FileLink
            if matches!(
                folder.kind(FILE),
                Some(crate::held::Kind::Link { .. } | crate::held::Kind::File)
            ) =>
        {
            Missing::Link
        }
        _ => Missing::File,
    })?;
    parsed(&text, folder.shown().join(FILE))
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
    let mut skills: Vec<Skill> = scan_dir(dir).into_iter().filter_map(Result::ok).collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// A `SKILL.md`'s frontmatter, as `key: value` pairs in the order written: one line per key,
/// which is how every built-in skill is written.
///
/// The one rule purlis reads a frontmatter by, a persona's too
/// (`purlis_core::personas::frontmatter`): [`crate::fence::Fence`]'s, the lines between the
/// opening `---` line and the next `---` line, each split at its first `:`, key and value
/// stripped as Python's `str.strip` strips, and a line with an empty key dropped.
pub fn frontmatter(text: &str) -> Vec<(String, String)> {
    Fence::find(text).map(|f| f.pairs()).unwrap_or_default()
}

/// **Where a frontmatter is**, as byte offsets into `text`: from the first byte under the
/// opening fence to the first of the closing one, or `None` where it has none
/// ([`crate::fence::Fence`]). What every rewrite of a frontmatter cuts by ([`crate::change`]).
pub(crate) fn frontmatter_block(text: &str) -> Option<(usize, usize)> {
    Fence::find(text).map(|f| f.span())
}

/// The lines of a `SKILL.md`'s frontmatter, by the rule [`frontmatter`] reads it by, each
/// without its break. Empty where there is no frontmatter. What a reader of a key that spans
/// lines (a block list) reads.
pub(crate) fn frontmatter_lines(text: &str) -> Vec<&str> {
    Fence::find(text).map(|f| f.line_texts()).unwrap_or_default()
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

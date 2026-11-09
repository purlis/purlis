// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the index is built from, and who may see each part of it.
//!
//! Each source is one folder `memstore::read_files` lists: a workspace's `memory/`, `sessions/`
//! and `todos/`, the root's `sessions/`, each persona's `memory/` and each folder of its `refs/`,
//! and the shared store's `memory/` and `refs/`; and each workspace's `artifacts/`, whose pinned
//! artifacts [`crate::artifact::search`] lists through the pin's gate. A row keeps the source it
//! was listed from, so a
//! query names the sources its caller may read and nothing else answers.

use std::path::{Path, PathBuf};

use crate::contain::SHARED_PERSONA;

/// What a source holds, and whose it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A workspace's journal, `workspaces/<ws>/memory/`.
    Workspace,
    /// A persona's own memory, `personas/<name>/memory/`.
    Persona,
    /// The store every persona shares, `personas/_shared/memory/`.
    Shared,
    /// A folder of a persona's `refs/`, or of the shared store's (owner `_shared`).
    Refs,
    /// Session records: a workspace's `sessions/`, or the root's (owner `""`).
    Sessions,
    /// A workspace's open todos, `workspaces/<ws>/todos/`.
    Todos,
    /// A workspace's pinned artifacts, `workspaces/<ws>/artifacts/` (ADR 0120, #5968): one row
    /// per pin, read by [`crate::artifact::search`], never by `memstore::read_files`.
    Artifacts,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Persona => "persona",
            Self::Shared => "shared",
            Self::Refs => "refs",
            Self::Sessions => "sessions",
            Self::Todos => "todos",
            Self::Artifacts => "artifacts",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "workspace" => Self::Workspace,
            "persona" => Self::Persona,
            "shared" => Self::Shared,
            "refs" => Self::Refs,
            "sessions" => Self::Sessions,
            "todos" => Self::Todos,
            "artifacts" => Self::Artifacts,
            _ => return None,
        })
    }

    /// The letter its reach words start with.
    fn letter(self) -> char {
        match self {
            Self::Workspace => 'w',
            Self::Persona => 'p',
            Self::Shared => 's',
            Self::Refs => 'r',
            Self::Sessions => 'e',
            Self::Todos => 't',
            Self::Artifacts => 'a',
        }
    }
}

/// Where a source of `kind` owned by `owner` comes in recall's order of bases: workspace,
/// persona, (ephemeral, never indexed), shared, the persona's refs, the shared refs
/// (`recall::sources`). Rows list newest first, and in this order within a day.
pub(crate) fn rank(kind: Kind, owner: &str) -> i64 {
    match kind {
        Kind::Workspace => 0,
        Kind::Persona => 1,
        Kind::Shared => 3,
        Kind::Refs if owner == SHARED_PERSONA => 5,
        Kind::Refs => 4,
        Kind::Sessions => 6,
        Kind::Todos => 7,
        Kind::Artifacts => 8,
    }
}

/// The words a row's reach column holds: one for its kind, and one for its kind and owner.
/// The owner is written in hex, so any name is one word of letters and digits.
pub(crate) fn reach_words(kind: Kind, owner: &str) -> String {
    format!("q{} {}", kind.letter(), owner_word(kind, owner))
}

fn owner_word(kind: Kind, owner: &str) -> String {
    format!(
        "q{}{}",
        kind.letter(),
        crate::extension::hex(owner.as_bytes())
    )
}

/// Whose rows a query may answer with: each pair is a kind and its owner, or any owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach(Vec<(Kind, Option<String>)>);

impl Reach {
    /// Reaches nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// Also `kind`, of `owner`, or of anyone with `None`.
    pub fn with(mut self, kind: Kind, owner: Option<&str>) -> Self {
        self.0.push((kind, owner.map(str::to_string)));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn pairs(&self) -> &[(Kind, Option<String>)] {
        &self.0
    }

    /// The FTS5 query naming the reach: one of its words, in the reach column.
    pub(crate) fn fts(&self) -> String {
        let words: Vec<String> = self
            .0
            .iter()
            .map(|(kind, owner)| match owner {
                None => format!("q{}", kind.letter()),
                Some(owner) => owner_word(*kind, owner),
            })
            .collect();
        format!("reach : ({})", words.join(" OR "))
    }
}

/// One folder the index is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    pub kind: Kind,
    pub owner: String,
    pub dir: PathBuf,
}

/// The names of the folders in `dir` that purlis counts: not dotted, and a directory once any
/// link is followed. Sorted.
fn folders(dir: &Path) -> Vec<String> {
    let Ok(reader) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = reader
        .flatten()
        .filter(|e| std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

/// Every source of the project at `root`, in recall's order.
pub(crate) fn every(root: &Path) -> Vec<Source> {
    let mut out = Vec::new();
    let mut push = |kind: Kind, owner: &str, dir: PathBuf| {
        out.push(Source {
            kind,
            owner: owner.to_string(),
            dir,
        });
    };
    let workspaces = crate::recall::read_workspaces(root)
        .map(|(names, _)| names)
        .unwrap_or_default();
    for name in &workspaces {
        let dir = root.join("workspaces").join(name);
        push(Kind::Workspace, name, dir.join("memory"));
        push(Kind::Sessions, name, dir.join("sessions"));
        push(Kind::Todos, name, dir.join("todos"));
        push(Kind::Artifacts, name, dir.join(crate::artifact::DIR));
    }
    push(Kind::Sessions, "", root.join("sessions"));
    let personas = root.join("personas");
    let names: Vec<String> = folders(&personas)
        .into_iter()
        .filter(|n| n != SHARED_PERSONA)
        .collect();
    for name in &names {
        push(Kind::Persona, name, personas.join(name).join("memory"));
    }
    push(
        Kind::Shared,
        SHARED_PERSONA,
        personas.join(SHARED_PERSONA).join("memory"),
    );
    let mut unread = Vec::new();
    for name in names.iter().map(String::as_str).chain([SHARED_PERSONA]) {
        for dir in crate::recall::ref_dirs(root, &personas.join(name).join("refs"), &mut unread) {
            push(Kind::Refs, name, dir);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_owner_of_any_spelling_is_one_word_of_its_own() {
        assert_eq!(reach_words(Kind::Workspace, "a-b c"), "qw qw612d622063");
        assert_ne!(
            owner_word(Kind::Persona, "ab"),
            owner_word(Kind::Workspace, "ab"),
            "a persona and a workspace of one name are two reaches"
        );
        assert_eq!(
            Reach::none()
                .with(Kind::Workspace, None)
                .with(Kind::Persona, Some("ab"))
                .fts(),
            "reach : (qw OR qp6162)"
        );
    }
}

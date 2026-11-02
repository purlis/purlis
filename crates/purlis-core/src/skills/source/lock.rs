// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`skills.lock` and `skills.local.lock`** (SK-10, #2773; ADR 0100 §2): what each installed
//! skill was copied from, at which commit, and the digest of every file it was copied as. The
//! shared one is committed at the plane root; the local one, for skills installed at a local
//! level, sits beside it and is never committed.
//!
//! **Only an accepted install or update writes an entry.** Nothing a source does moves a pin: a
//! check for a newer commit reads the source's refs and writes nothing.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Commit, Remote};
use crate::skills::verbs::Level;

/// The shared lockfile's name, at the plane root.
pub const FILE: &str = "skills.lock";

/// The local lockfile's name, beside it.
pub const LOCAL_FILE: &str = "skills.local.lock";

/// The version of the lockfile's shape this purlis writes.
pub const VERSION: u32 = 1;

/// Which of the two lockfiles: the committed one the team shares, or this machine's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    Shared,
    Local,
}

impl Kept {
    /// The lockfile `level`'s installs are recorded in.
    pub fn of(level: &Level) -> Kept {
        if level.local {
            Kept::Local
        } else {
            Kept::Shared
        }
    }

    /// Its name, at the plane root.
    pub fn file(self) -> &'static str {
        match self {
            Kept::Shared => FILE,
            Kept::Local => LOCAL_FILE,
        }
    }
}

/// A level, written as its folder in the plane (`skills`, `workspaces/ops/skills.local`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtLevel(pub Level);

impl Serialize for AtLevel {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let rel = self
            .0
            .rel()
            .ok_or_else(|| serde::ser::Error::custom("a level with no folder"))?;
        s.serialize_str(&rel)
    }
}

impl<'de> Deserialize<'de> for AtLevel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let rel = String::deserialize(d)?;
        Level::of_rel(&rel)
            .map(AtLevel)
            .ok_or_else(|| serde::de::Error::custom(format!("{rel:?} is no level's folder")))
    }
}

/// One file an install wrote, and the digest of what it wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wrote {
    /// Its path inside the skill's folder, `/` between the parts.
    pub path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub executable: bool,
    /// `sha256:` and the hex digest of its bytes.
    pub sha256: String,
}

/// **One installed skill**, as its lockfile records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locked {
    pub name: String,
    /// Its name at the source, where it was installed under a new one (ADR 0100, amended
    /// 2026-11-01): what an update finds it by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    pub level: AtLevel,
    /// The repository and branch it was copied from.
    pub source: Remote,
    /// Its folder in the source, `/` between the parts; empty for a source that is one skill.
    pub folder: String,
    /// The commit it was copied from.
    pub commit: Commit,
    /// The digest of the copy as a whole ([`super::digest_of`]).
    pub digest: String,
    pub files: Vec<Wrote>,
    /// The skill whose install brought it along, as a skill it refers to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brought_by: Option<String>,
}

impl Locked {
    /// Whether it is the skill `name` at `level`.
    pub fn is(&self, level: &Level, name: &str) -> bool {
        self.name == name && self.level.0 == *level
    }

    /// Its name at its source.
    pub fn at_source(&self) -> &str {
        self.source_name.as_deref().unwrap_or(&self.name)
    }
}

/// A lockfile's whole document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    #[serde(default)]
    pub skills: Vec<Locked>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: VERSION,
            skills: Vec::new(),
        }
    }
}

/// A lockfile as it stands on disk: its document, and its bytes for the write that follows.
pub struct Read {
    pub lock: Lockfile,
    pub was: Option<String>,
}

/// The sentence a lockfile purlis cannot read whole is refused with.
const NOT_READ: &str = "the project's record of installed skills could not be read, so no \
                        installed skill was changed";

/// **The lockfile `kept` at `plane`'s root**, read without following a link, every entry checked:
/// a name that is a skill's, plain file paths, a plain folder and a source purlis reads. Which of
/// them this machine contacts is [`super::update::updates`]' to say. One that is not there is empty; anything else is refused whole in one
/// sentence (the detail is logged).
pub fn read(plane: &Path, kept: Kept) -> Result<Read, String> {
    let name = kept.file();
    let refused = |why: String| {
        tracing::info!(target: "purlis::skills", "{name} was not read: {why}");
        NOT_READ.to_owned()
    };
    let bytes = crate::proposed::read_pinned(plane, Path::new(name))
        .map_err(|_| refused("it is not a plain file".to_owned()))?;
    let Some(bytes) = bytes else {
        return Ok(Read {
            lock: Lockfile::default(),
            was: None,
        });
    };
    let text = String::from_utf8(bytes).map_err(|_| refused("it is not text".to_owned()))?;
    let lock: Lockfile = serde_json::from_str(&text).map_err(|why| refused(why.to_string()))?;
    // A name is a folder purlis opens and writes in: one that is not a skill's name (a `..`, a
    // separator) is refused, as is a recorded file whose path is not plain, whoever wrote it.
    for one in &lock.skills {
        let plain_files = one.files.iter().all(|file| {
            purlis_skills::tree::folder_parts(&file.path).is_some_and(|parts| !parts.is_empty())
        });
        let bad_source_name = one
            .source_name
            .as_deref()
            .is_some_and(|name| purlis_skills::change::name_problem(name).is_some());
        if purlis_skills::change::name_problem(&one.name).is_some()
            || bad_source_name
            || !plain_files
            || purlis_skills::tree::folder_parts(&one.folder).is_none()
            || one.level.0.local != (kept == Kept::Local)
        {
            return Err(refused(format!(
                "it names {:?} in a way purlis never writes",
                one.name
            )));
        }
        if let Err(why) = one.source.check(false) {
            return Err(refused(why));
        }
    }
    if lock.version > VERSION {
        return Err(refused(format!("it is version {}", lock.version)));
    }
    Ok(Read {
        lock,
        was: Some(text),
    })
}

/// **Every installed skill** both lockfiles hold. One that cannot be read holds none, and the
/// sentence that says why is answered beside the rest.
pub fn every(plane: &Path) -> (Vec<(Kept, Locked)>, Vec<String>) {
    let mut skills = Vec::new();
    let mut said = Vec::new();
    for kept in [Kept::Shared, Kept::Local] {
        match read(plane, kept) {
            Ok(read) => skills.extend(read.lock.skills.into_iter().map(|one| (kept, one))),
            Err(why) => said.push(why),
        }
    }
    (skills, said)
}

/// **Writes `lock` as the lockfile `kept`**, only while it is still the `was` it was read as.
pub fn write(plane: &Path, kept: Kept, was: Option<&str>, lock: &Lockfile) -> Result<(), String> {
    let mut lock = lock.clone();
    lock.version = VERSION;
    lock.skills
        .sort_by(|a, b| (a.level.0.rel(), &a.name).cmp(&(b.level.0.rel(), &b.name)));
    let text = serde_json::to_string_pretty(&lock).map_err(|why| why.to_string())? + "\n";
    let base = was.and_then(|text| crate::proposed::base_of(Some(text.as_bytes())));
    crate::proposed::write_pinned(plane, Path::new(kept.file()), base.as_deref(), &text).map_err(
        |why| match why {
            crate::proposed::NotMade::Stale => "the project's record of installed skills \
                                                 changed meanwhile, so nothing was installed: \
                                                 try again"
                .to_owned(),
            crate::proposed::NotMade::Unprepared => "the project's record of installed skills \
                                                      is not a plain file, so nothing was \
                                                      installed"
                .to_owned(),
        },
    )
}

#[cfg(test)]
mod tests;

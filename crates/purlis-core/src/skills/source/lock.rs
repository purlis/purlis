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

use crate::skills::verbs::Level;

/// The shared lockfile's name, at the plane root.
pub const FILE: &str = "skills.lock";

/// The local lockfile's name, beside it.
pub const LOCAL_FILE: &str = "skills.local.lock";

/// The version of the lockfile's shape this purlis writes.
pub const VERSION: u32 = 1;

/// The lockfile `level`'s installs are recorded in.
pub fn file_for(level: &Level) -> &'static str {
    if level.local { LOCAL_FILE } else { FILE }
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

/// Where an installed skill came from: the source's URL and branch, and the skill's folder in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct From {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The skill's folder in the source, `/` between the parts; empty for a source that is one
    /// skill.
    pub folder: String,
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
    pub level: AtLevel,
    pub source: From,
    /// The commit it was copied from.
    pub commit: String,
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

/// **The lockfile `name` at `plane`'s root**, read without following a link. One that is not
/// there is empty; one that cannot be read, or is of a newer version, is refused in a sentence.
pub fn read(plane: &Path, name: &str) -> Result<Read, String> {
    let bytes = crate::proposed::read_pinned(plane, Path::new(name))
        .map_err(|_| format!("{name} is not a plain file, so purlis does not read it"))?;
    let Some(bytes) = bytes else {
        return Ok(Read {
            lock: Lockfile::default(),
            was: None,
        });
    };
    let text = String::from_utf8(bytes).map_err(|_| format!("{name} is not text"))?;
    let lock: Lockfile = serde_json::from_str(&text).map_err(|why| {
        format!(
            "{name} cannot be read ({}), so no installed skill was changed",
            crate::shown::short(&why.to_string())
        )
    })?;
    if lock.version > VERSION {
        return Err(format!(
            "{name} was written by a newer purlis (version {}), so this one changes nothing in it",
            lock.version
        ));
    }
    Ok(Read {
        lock,
        was: Some(text),
    })
}

/// **Every installed skill** both lockfiles hold. One that cannot be read holds none, and the
/// sentence that says why is answered beside the rest.
pub fn every(plane: &Path) -> (Vec<Locked>, Vec<String>) {
    let mut skills = Vec::new();
    let mut said = Vec::new();
    for name in [FILE, LOCAL_FILE] {
        match read(plane, name) {
            Ok(read) => skills.extend(read.lock.skills),
            Err(why) => said.push(why),
        }
    }
    (skills, said)
}

/// **Writes `lock` as the lockfile `name`**, only while it is still the `was` it was read as.
pub fn write(plane: &Path, name: &str, was: Option<&str>, lock: &Lockfile) -> Result<(), String> {
    let mut lock = lock.clone();
    lock.version = VERSION;
    lock.skills
        .sort_by(|a, b| (a.level.0.rel(), &a.name).cmp(&(b.level.0.rel(), &b.name)));
    let text = serde_json::to_string_pretty(&lock).map_err(|why| why.to_string())? + "\n";
    let base = was.and_then(|text| crate::proposed::base_of(Some(text.as_bytes())));
    crate::proposed::write_pinned(plane, Path::new(name), base.as_deref(), &text).map_err(|why| {
        match why {
            crate::proposed::NotMade::Stale => format!(
                "{name} changed while the skill was being installed, so it was not written: try \
                 again"
            ),
            crate::proposed::NotMade::Unprepared => {
                format!("{name} could not be written: it is not a plain file")
            }
        }
    })
}

#[cfg(test)]
mod tests;

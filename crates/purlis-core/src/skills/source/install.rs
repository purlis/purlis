// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Installing** (ADR 0100 §2): copies of the reviewed skills into a level, pinned in its
//! lockfile, as one commit, **all or nothing**: a skill, a grant or the lockfile that cannot be
//! written takes back every skill and grant this install wrote, by exactly the names it made.

use std::path::PathBuf;

use purlis_skills::tree;

use super::fetch::fetch;
use super::lock::{self, Kept, Locked};
use super::review::{Accepted, digest_of, wrote};
use super::{Source, listed, write};
use crate::skills::Grant;
use crate::skills::verbs::{self, Ctx, Level};

/// The parts of `level`'s folder in the plane.
pub(super) fn level_parts(level: &Level) -> Result<Vec<String>, String> {
    level
        .rel()
        .map(|rel| rel.split('/').map(str::to_owned).collect())
        .ok_or_else(|| format!("{level} is no level of this project"))
}

/// Refused where a path of `files` under the skill folder `name` at the level folder `level`
/// would pass [`tree::MOST_PATH_BYTES`].
pub(super) fn paths_fit(
    level: &[String],
    name: &str,
    files: &[super::fetch::SourceFile],
) -> Result<(), String> {
    let base = level.join("/").len() + 1 + name.len() + 1;
    match files
        .iter()
        .find(|file| base + file.path().len() > tree::MOST_PATH_BYTES)
    {
        Some(file) => Err(format!(
            "{name} holds {}, whose path is longer than purlis writes ({} bytes in all)",
            file.path(),
            tree::MOST_PATH_BYTES
        )),
        None => Ok(()),
    }
}

/// What one install wrote so far, to take back where a later step fails.
struct Written<'a> {
    plane: &'a std::path::Path,
    level: Vec<String>,
    skills: Vec<(String, write::Made)>,
    grants: Vec<String>,
}

impl Written<'_> {
    /// Takes back every skill and grant file this install made.
    fn take_back(&self) {
        for name in &self.grants {
            write::take_back_file(self.plane, &self.level, &format!("{name}.toml"));
        }
        for (name, made) in self.skills.iter().rev() {
            write::take_back(self.plane, &self.level, name, made);
        }
    }
}

/// **Installs what `accepted` says from `source` at `level`** (ADR 0100 §2): fetches the
/// reviewed commit again, checks each skill's files are the reviewed ones, copies each into
/// `level` under its own name, records it in the level's lockfile, and commits the copies and the
/// lockfile as one change. A project skill lands granted to the root unless `grant` says
/// otherwise. All or nothing. The source is confirmed on this machine. Answers each skill's
/// folder.
pub fn install(
    cx: &mut Ctx<'_>,
    source: &Source,
    level: &Level,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<Vec<PathBuf>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    verbs::level_is_there(plane, level)?;
    let kept = Kept::of(level);
    let names: Vec<String> = accepted.skills.iter().map(|one| one.name.clone()).collect();
    if names.is_empty() {
        return Err("no skill was named to install".to_owned());
    }
    let fetched = fetch(plane, source, Some(&accepted.commit))?;
    let review = fetched.review(&names)?;
    if !accepted.is(&review.accepted()) {
        return Err(
            "what the source holds is not what was reviewed, so nothing was installed: review it \
             again"
                .to_owned(),
        );
    }
    let held = lock::read(plane, kept)?;
    let parts = level_parts(level)?;
    let (found, _) = fetched.skills()?;
    let mut copies = Vec::new();
    for skill in &review.skills {
        if cx.reserved(&skill.name) {
            return Err(format!(
                "{} cannot be installed: its name belongs to a built-in skill",
                skill.name
            ));
        }
        verbs::free_at(plane, level, &skill.name, &skill.name)?;
        if held
            .lock
            .skills
            .iter()
            .any(|one| one.is(level, &skill.name))
        {
            return Err(format!(
                "the {level} skills already have {} installed: update it instead",
                skill.name
            ));
        }
        let one = found
            .iter()
            .find(|f| f.name == skill.name)
            .ok_or_else(|| format!("{} is no longer in the source", skill.name))?;
        let files = fetched.files_of(&one.dir)?;
        paths_fit(&parts, &skill.name, &files)?;
        copies.push((skill, one, files));
    }
    let mut written = Written {
        plane,
        level: parts.clone(),
        skills: Vec::new(),
        grants: Vec::new(),
    };
    let mut lock = held.lock.clone();
    let mut made: Vec<PathBuf> = Vec::new();
    for (skill, one, files) in &copies {
        let writes: Vec<write::File<'_>> = files
            .iter()
            .map(|file| write::File {
                parts: &file.parts,
                executable: file.executable,
                bytes: &file.bytes,
            })
            .collect();
        match write::new_skill(plane, &parts, level.depth(), &skill.name, &writes) {
            Ok(record) => written.skills.push((skill.name.clone(), record)),
            Err(why) => {
                written.take_back();
                return Err(format!(
                    "{} could not be installed ({why}), so nothing was installed",
                    skill.name
                ));
            }
        }
        let files_wrote = wrote(files);
        lock.skills.push(Locked {
            name: skill.name.clone(),
            level: lock::AtLevel(level.clone()),
            source: source.remote.clone(),
            folder: one.folder.clone(),
            commit: review.commit.clone(),
            digest: digest_of(&files_wrote),
            files: files_wrote,
            brought_by: skill.brought_by.clone(),
        });
        made.push(plane.join(parts.join("/")).join(&skill.name));
    }
    let mut paths: Vec<String> = made
        .iter()
        .map(|folder| verbs::rel(plane, folder))
        .collect();
    if level.at == verbs::Whose::Project
        && !level.local
        && let Some(grant) = grant.filter(|g| **g != Grant::Root)
    {
        for skill in &review.skills {
            match crate::skills::grant(plane, &skill.name, grant) {
                Ok(folder) => {
                    written.grants.push(skill.name.clone());
                    paths.push(verbs::rel(plane, &crate::skills::beside(&folder)));
                }
                Err(why) => {
                    written.take_back();
                    return Err(format!("{why}, so nothing was installed"));
                }
            }
        }
    }
    if let Err(why) = lock::write(plane, kept, held.was.as_deref(), &lock) {
        written.take_back();
        return Err(why);
    }
    if kept == Kept::Shared
        && let Err(why) = listed::confirm_here(cx, &source.remote)
    {
        cx.warn(format!(
            "The source was not confirmed on this machine: {why}"
        ));
    }
    let shown: Vec<&str> = review.skills.iter().map(|s| s.name.as_str()).collect();
    cx.done(format!(
        "{} installed at {level}, pinned to {}",
        shown.join(", "),
        review.commit.short()
    ));
    if kept == Kept::Shared {
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill install {}", shown.join(" ")));
    }
    Ok(made)
}

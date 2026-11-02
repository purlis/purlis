// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Installing** (ADR 0100 §2): copies of the reviewed skills into a level, pinned in its
//! lockfile, as one commit, **all or nothing**: a skill, a grant or the lockfile that cannot be
//! written takes back every skill and grant this install wrote, by exactly the names it made.

use std::path::PathBuf;

use purlis_skills::tree;

use super::fetch::fetch;
use super::lock::{self, Kept, Locked};
use super::review::{self, Accepted, AcceptedSkill, Reviewed, Unread, digest_of, wrote};
use super::{Source, listed, widen, write};
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

/// What one install or update wrote so far, to take back where a later step fails.
pub(super) struct Written<'a> {
    pub(super) plane: &'a std::path::Path,
    pub(super) level: Vec<String>,
    pub(super) skills: Vec<(String, write::Made)>,
    /// Each skill this change granted, by name: its `<name>.toml` is new.
    pub(super) grants: Vec<String>,
    /// Each project skill whose grant this change widened, and the grant it had (#4172).
    pub(super) widened: Vec<(PathBuf, Grant)>,
}

impl<'a> Written<'a> {
    /// Nothing written yet into the level folder `level` of `plane`.
    pub(super) fn none(plane: &'a std::path::Path, level: Vec<String>) -> Self {
        Written {
            plane,
            level,
            skills: Vec::new(),
            grants: Vec::new(),
            widened: Vec::new(),
        }
    }

    /// Widens the grant of each of `widenings` that asks it (#4172, #4186), each file written
    /// added to `paths`; where one cannot be, why, and the rest are left to [`Self::take_back`].
    /// Written straight, past `skills::grant`'s checks: the grant widened to is the change's own,
    /// whose workspaces were checked when it was granted, and the plane's being writable before
    /// anything.
    pub(super) fn widen(
        &mut self,
        widenings: &[widen::Widening],
        paths: &mut Vec<String>,
    ) -> Result<(), String> {
        for one in widenings {
            let Some(to) = &one.widened else {
                continue;
            };
            crate::skills::set_grant(&one.folder, to).map_err(|why| {
                format!(
                    "the grant of {} could not be widened to {to} ({why})",
                    one.name
                )
            })?;
            self.widened.push((one.folder.clone(), one.granted.clone()));
            paths.push(verbs::rel(self.plane, &crate::skills::beside(&one.folder)));
        }
        Ok(())
    }

    /// Takes back every skill and grant file this change made, and every grant it widened.
    pub(super) fn take_back(&self) {
        for (folder, had) in &self.widened {
            let _ = crate::skills::set_grant(folder, had);
        }
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
    // The files each review read are the ones copied: read once, so a set that fit one read for
    // its review fits it for its install too (#4024).
    // A skill they refer to that is met at `level` already is left as it is, as the review left
    // it (#4032, #4155).
    let reviewed =
        review::reviewed(&fetched, &names, &review::met_at(plane, level)).map_err(Unread::said)?;
    let now = Accepted {
        commit: fetched.commit.clone(),
        skills: reviewed
            .iter()
            .map(|one| AcceptedSkill {
                name: one.review.name.clone(),
                digest: one.review.digest.clone(),
            })
            .collect(),
    };
    if !accepted.is(&now) {
        return Err(format!(
            "what the source holds, or the {level} skills, changed since the review, so nothing was \
             installed: review it again"
        ));
    }
    let held = lock::read(plane, kept)?;
    let parts = level_parts(level)?;
    let mut copies = Vec::new();
    for Reviewed {
        review: skill,
        files,
    } in &reviewed
    {
        if cx.reserved(&skill.name) {
            return Err(format!(
                "{} cannot be installed: a name starting with purlis- belongs to purlis",
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
        paths_fit(&parts, &skill.name, files)?;
        copies.push((skill, files));
    }
    let mut written = Written::none(plane, parts.clone());
    let mut lock = held.lock.clone();
    let mut made: Vec<PathBuf> = Vec::new();
    for (skill, files) in &copies {
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
            folder: skill.folder.clone(),
            commit: fetched.commit.clone(),
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
    if let Some(grant) = widen::granted_beyond_the_root(level, grant) {
        for Reviewed { review: skill, .. } in &reviewed {
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
    // A project skill they rely on reaches the workspaces they are granted to as well (#4172).
    let widenings = widen::widenings(
        plane,
        level,
        grant,
        reviewed
            .iter()
            .flat_map(|one| one.review.already.iter().map(String::as_str)),
    );
    if let Err(why) = written.widen(&widenings, &mut paths) {
        written.take_back();
        return Err(format!("{why}, so nothing was installed"));
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
    // Each is a new skill: the grades of one that had its name before are none of its own
    // (#4104).
    for Reviewed { review: skill, .. } in &reviewed {
        verbs::earlier_dropped(cx, level, &skill.name);
    }
    let shown: Vec<&str> = reviewed.iter().map(|s| s.review.name.as_str()).collect();
    cx.done(format!(
        "{} installed at {level}, pinned to {}",
        shown.join(", "),
        fetched.commit.short()
    ));
    widen::said_done(cx, &widenings, "what was installed");
    if kept == Kept::Shared {
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill install {}", shown.join(" ")));
    }
    Ok(made)
}

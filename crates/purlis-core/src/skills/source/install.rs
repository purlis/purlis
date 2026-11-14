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
use super::{Origin, Pin, SkillSource, Source, listed, widen, write};
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
pub(super) struct Written {
    pub(super) plane: PathBuf,
    pub(super) level: Vec<String>,
    pub(super) skills: Vec<(String, write::Made)>,
    /// Each skill this change granted, by name: its `<name>.toml` is new.
    pub(super) grants: Vec<String>,
    /// Each project skill whose grant this change widened, and the grant it had (#4172).
    pub(super) widened: Vec<(PathBuf, Grant)>,
    /// The same, with the grant this change left it with.
    pub(super) widened_to: Vec<(PathBuf, Grant)>,
}

impl Written {
    /// Nothing written yet into the level folder `level` of `plane`.
    pub(super) fn none(plane: &std::path::Path, level: Vec<String>) -> Self {
        Written {
            plane: plane.to_path_buf(),
            level,
            skills: Vec::new(),
            grants: Vec::new(),
            widened: Vec::new(),
            widened_to: Vec::new(),
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
            self.widened_to.push((one.folder.clone(), to.clone()));
            paths.push(verbs::rel(&self.plane, &crate::skills::beside(&one.folder)));
        }
        Ok(())
    }

    /// Takes back every skill and grant file this change made, and every grant it widened.
    pub(super) fn take_back(&self) {
        for (folder, had) in &self.widened {
            let _ = crate::skills::set_grant(folder, had);
        }
        for name in &self.grants {
            write::take_back_file(&self.plane, &self.level, &format!("{name}.toml"));
        }
        for (name, made) in self.skills.iter().rev() {
            write::take_back(&self.plane, &self.level, name, made);
        }
    }
}

/// **The new names a clash was settled with**: each skill's name at the source, and the name it
/// lands under (ADR 0100, amended 2026-11-01).
pub type Renames = std::collections::BTreeMap<String, String>;

/// **`files` of a skill, its `SKILL.md`'s `name:` line saying `name`**: all a new name changes of
/// them, with the folder it lands in. Its text is left as the source wrote it.
pub(super) fn under_new_name(
    files: &[super::fetch::SourceFile],
    name: &str,
) -> Result<Vec<super::fetch::SourceFile>, String> {
    files
        .iter()
        .map(|file| {
            let mut file = file.clone();
            if file.parts == [purlis_skills::FILE] {
                let text = std::str::from_utf8(&file.bytes)
                    .ok()
                    .and_then(|text| purlis_skills::change::with_name(text, name))
                    .ok_or_else(|| format!("{name}'s instructions have no name: line to change"))?;
                file.bytes = text.into_bytes();
            }
            Ok(file)
        })
        .collect()
}

/// [`install_named`], each skill under its own name.
pub fn install(
    cx: &mut Ctx<'_>,
    source: &Source,
    level: &Level,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<Vec<PathBuf>, String> {
    install_named(cx, source, level, accepted, grant, &Renames::new())
}

/// **Installs what `accepted` says from `source` at `level`** (ADR 0100 §2): fetches the
/// reviewed commit again, checks each skill's files are the reviewed ones, copies each into
/// `level` under its own name or the one `renames` gives it, records it in the level's lockfile, and commits the copies and the
/// lockfile as one change. A project skill lands granted to the root unless `grant` says
/// otherwise. All or nothing. The source is confirmed on this machine. Answers each skill's
/// folder.
pub fn install_named(
    cx: &mut Ctx<'_>,
    source: &Source,
    level: &Level,
    accepted: &Accepted,
    grant: Option<&Grant>,
    renames: &Renames,
) -> Result<Vec<PathBuf>, String> {
    crate::settings::writable(cx.plane)?;
    verbs::level_is_there(cx.plane, level)?;
    let fetched = fetch(cx.plane, source, Some(accepted.commit()?))?;
    let origin = Origin::Git(source.remote.clone());
    let made = install_from(
        cx,
        &fetched,
        &origin,
        &Pin::Commit(fetched.commit.clone()),
        level,
        accepted,
        grant,
        renames,
    )?;
    if Kept::of(level) == Kept::Shared
        && let Err(why) = listed::confirm_here(cx, &source.remote)
    {
        cx.warn(format!(
            "The source was not confirmed on this machine: {why}"
        ));
    }
    Ok(made)
}

/// **Installs what `accepted` says from `source`**, which answers `origin` at `pin`, at `level`
/// (ADR 0100 §2, §8): the same review read again from what `source` holds now, refused where any
/// digest differs from the one accepted, then each skill copied under its own name or the one
/// `renames` gives it, pinned in the level's lockfile, granted, and committed with the lockfile
/// as one change. All or nothing. The copy and the pin are the same for every source, the git one
/// and each extension's.
#[allow(clippy::too_many_arguments)]
pub(super) fn install_from(
    cx: &mut Ctx<'_>,
    source: &dyn SkillSource,
    origin: &Origin,
    pin: &Pin,
    level: &Level,
    accepted: &Accepted,
    grant: Option<&Grant>,
    renames: &Renames,
) -> Result<Vec<PathBuf>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    verbs::level_is_there(plane, level)?;
    let kept = Kept::of(level);
    let names: Vec<String> = accepted.skills.iter().map(|one| one.name.clone()).collect();
    if names.is_empty() {
        return Err("no skill was named to install".to_owned());
    }
    // The files each review read are the ones copied: read once, so a set that fit one read for
    // its review fits it for its install too (#4024).
    // A skill they refer to that is met at `level` already is left as it is, as the review left
    // it (#4032, #4155), and one only a hint brings comes along only where it was accepted
    // (#5874).
    let reviewed = review::reviewed(
        source,
        &names,
        &review::met_at(plane, level),
        review::Hints::Only(&names),
    )
    .map_err(Unread::said)?;
    let now = Accepted {
        pin: pin.clone(),
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
    // The org's controls, asked again of what is installed now (SK-11).
    super::policy::hold(plane, origin, names.iter().map(String::as_str), pin, || {
        source.made()
    })
    .map_err(|why| format!("{why}, so nothing was installed"))?;
    let held = lock::read(plane, kept)?;
    let parts = level_parts(level)?;
    let mut copies = Vec::new();
    let mut landing: Vec<&str> = Vec::new();
    for Reviewed {
        review: skill,
        files,
    } in &reviewed
    {
        let renamed = renames.get(&skill.name);
        let name = renamed.unwrap_or(&skill.name);
        if let Some(why) = purlis_skills::change::name_problem(name) {
            return Err(format!("{name:?} cannot be a skill's name: {why}"));
        }
        if cx.reserved(name) {
            return Err(format!(
                "{name} cannot be installed: a name starting with purlis- belongs to purlis"
            ));
        }
        if landing.contains(&name.as_str()) {
            return Err(format!("two skills cannot both be named {name}"));
        }
        landing.push(name);
        verbs::free_at(plane, level, name, name)?;
        paths_fit(&parts, name, files)?;
        let (files, recorded) = match renamed {
            Some(_) => {
                let files = under_new_name(files, name)?;
                let recorded = review::wrote_renamed(&files);
                (files, recorded)
            }
            None => (files.clone(), wrote(files)),
        };
        copies.push((skill, name.clone(), files, recorded, renamed.is_some()));
    }
    cx.audited(&super::policy::Audited::new(
        super::policy::Act::Install,
        level,
        copies.iter().map(|(_, name, ..)| name.clone()).collect(),
        origin,
        pin,
    ))
    .map_err(|why| format!("{why}, so nothing was installed"))?;
    let mut written = Written::none(plane, parts.clone());
    let mut lock = held.lock.clone();
    // An entry whose skill is gone was a deleted skill's (#5331), and the new copy takes its
    // place: `free_at` above refused any name that is really there.
    lock.skills
        .retain(|one| !(one.level.0 == *level && landing.contains(&one.name.as_str())));
    let mut made: Vec<PathBuf> = Vec::new();
    for (skill, name, files, recorded, renamed) in &copies {
        let writes: Vec<write::File<'_>> = files
            .iter()
            .map(|file| write::File {
                parts: &file.parts,
                executable: file.executable,
                bytes: &file.bytes,
            })
            .collect();
        match write::new_skill(plane, &parts, level.depth(), name, &writes) {
            Ok(record) => written.skills.push((name.clone(), record)),
            Err(why) => {
                written.take_back();
                return Err(format!(
                    "{name} could not be installed ({why}), so nothing was installed"
                ));
            }
        }
        lock.skills.push(Locked {
            name: name.clone(),
            source_name: renamed.then(|| skill.name.clone()),
            level: lock::AtLevel(level.clone()),
            source: origin.clone(),
            folder: skill.folder.clone(),
            pin: pin.clone(),
            digest: digest_of(recorded),
            files: recorded.clone(),
            brought_by: skill.brought_by.clone(),
        });
        made.push(plane.join(parts.join("/")).join(name));
    }
    let shown: Vec<&str> = copies.iter().map(|one| one.1.as_str()).collect();
    let mut paths: Vec<String> = made
        .iter()
        .map(|folder| verbs::rel(plane, folder))
        .collect();
    if let Some(grant) = widen::granted_beyond_the_root(level, grant) {
        for name in &shown {
            match crate::skills::grant(plane, name, grant) {
                Ok(folder) => {
                    written.grants.push((*name).to_owned());
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
    // Each is a new skill: the grades of one that had its name before are none of its own
    // (#4104).
    for name in &shown {
        verbs::earlier_dropped(cx, level, name);
    }
    cx.done(format!(
        "{} installed at {level}, pinned to {}",
        shown.join(", "),
        pin.short()
    ));
    widen::said_done(cx, &widenings, "what was installed");
    if kept == Kept::Shared {
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill install {}", shown.join(" ")));
    }
    Ok(made)
}

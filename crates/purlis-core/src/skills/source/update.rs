// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Updates** (ADR 0100 §4): a newer commit is offered, never applied by itself.
//!
//! **What is contacted.** Asking for a newer commit reads the source's refs alone. A source this
//! machine installed from, or the person confirmed here, is asked; one only the committed
//! lockfile names is listed as not checked on this machine, with a Confirm, and is not contacted
//! until it is confirmed ([`super::listed::confirm`]); and a folder on one machine named by the
//! committed lockfile is never contacted from another.
//!
//! **What an update shows.** The summary's changes first, then each file's, from the pinned
//! commit to the newer one; the edits made here since the install, as their own diff against the
//! pin, which accepting replaces; the skills the newer commit refers to that this level does not
//! have, which come along in the same review; and the newer commit's own review.

use std::path::{Path, PathBuf};

use purlis_skills::review::{self as summary, Changes};
use purlis_skills::tree;
use serde::Serialize;

use super::fetch::{SourceFile, fetch};
use super::install::{Written, level_parts, paths_fit};
use super::lock::{self, Kept, Locked};
use super::review::{
    Accepted, SkillReview, digest_of, met_at, review_of, sha256, summary_of, wrote,
};
use super::widen::{self, Widening};
use super::{Commit, Remote, Source, listed, write};
use crate::skills::Grant;
use crate::skills::verbs::{self, Ctx, Level};

/// **A newer commit for an installed skill**, offered, never applied by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offer {
    pub name: String,
    pub level: Level,
    pub remote: Remote,
    pub pinned: Commit,
    pub newer: Commit,
    /// Whether the copy was edited here since it was installed.
    pub edited: bool,
}

/// What an installed skill could not be asked about, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unasked {
    pub name: String,
    pub level: Level,
    pub why: String,
}

/// **A source the committed lockfile names that this machine has not confirmed**: not checked
/// on this machine until the person confirms it, and the skills installed from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unconfirmed {
    pub remote: Remote,
    pub skills: Vec<String>,
}

/// What asking every installed skill's source answered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Updates {
    pub offers: Vec<Offer>,
    pub unasked: Vec<Unasked>,
    pub unconfirmed: Vec<Unconfirmed>,
}

/// The source `locked` was installed from, with the sign-in this machine gives it.
fn source_of(plane: &Path, locked: &Locked) -> Result<Source, String> {
    let listed = listed::listed_as(plane, &locked.source)?;
    Ok(Source {
        remote: locked.source.clone(),
        folder: None,
        sign_in: listed.and_then(|one| one.sign_in),
    })
}

/// Whether this machine may contact the source of `locked`, kept in `kept`: one this machine
/// installed from or confirmed; or why not, as the kind of answer the studio gives.
enum May {
    Ask,
    Confirm,
    Never(String),
}

fn may(plane: &Path, kept: Kept, locked: &Locked) -> May {
    if kept == Kept::Local {
        return May::Ask;
    }
    match listed::confirmed(plane, &locked.source) {
        Ok(true) => May::Ask,
        Ok(false) => match locked.source.check(true) {
            Ok(()) => May::Confirm,
            Err(why) => May::Never(why),
        },
        Err(why) => May::Never(why),
    }
}

/// The files of the copy `locked` installed that differ from what it installed, or `None` where
/// the copy cannot be read.
fn edited(plane: &Path, locked: &Locked) -> Option<Vec<String>> {
    let mut parts = level_parts(&locked.level.0).ok()?;
    parts.push(locked.name.clone());
    let now = write::read_all(plane, &parts, tree::MOST_FILE_BYTES, tree::MOST_FILES)?;
    let mut changed = Vec::new();
    for file in &now {
        let shown = file.parts.join("/");
        match locked.files.iter().find(|f| f.path == shown) {
            Some(was)
                if was.sha256 == pinned_sha(locked, &shown, &file.bytes)
                    && was.executable == file.executable => {}
            _ => changed.push(shown),
        }
    }
    for was in &locked.files {
        if !now.iter().any(|file| file.parts.join("/") == was.path) {
            changed.push(was.path.clone());
        }
    }
    changed.sort();
    Some(changed)
}

/// **What the pinned-copy check digests of the file at `path` of `locked`**: its bytes, with the
/// name line of a `SKILL.md` left out where it was installed under a new name, so that name is
/// never read as an edit (ADR 0100, amended 2026-11-01).
fn pinned_sha(locked: &Locked, path: &str, bytes: &[u8]) -> String {
    if locked.source_name.is_some() {
        super::review::sha_renamed(path, bytes)
    } else {
        sha256(bytes)
    }
}

/// **Asks each source of an installed skill this machine may contact for its newest commit** —
/// its refs alone, nothing fetched or written — and answers each skill whose pin is older, each
/// that could not be asked, and each source that waits to be confirmed here.
pub fn updates(plane: &Path) -> Updates {
    let (locked, unread) = lock::every(plane);
    let mut out = Updates::default();
    for why in unread {
        out.unasked.push(Unasked {
            name: String::new(),
            level: Level::project(),
            why,
        });
    }
    let mut asked: Vec<(Remote, Result<Commit, String>)> = Vec::new();
    for (kept, one) in &locked {
        match may(plane, *kept, one) {
            May::Ask => {}
            May::Confirm => {
                match out.unconfirmed.iter_mut().find(|u| u.remote == one.source) {
                    Some(waiting) => waiting.skills.push(one.name.clone()),
                    None => out.unconfirmed.push(Unconfirmed {
                        remote: one.source.clone(),
                        skills: vec![one.name.clone()],
                    }),
                }
                continue;
            }
            May::Never(why) => {
                out.unasked.push(Unasked {
                    name: one.name.clone(),
                    level: one.level.0.clone(),
                    why,
                });
                continue;
            }
        }
        let newest = match asked.iter().find(|(remote, _)| *remote == one.source) {
            Some((_, answer)) => answer.clone(),
            None => {
                let answer = source_of(plane, one).and_then(|source| super::newest(plane, &source));
                asked.push((one.source.clone(), answer.clone()));
                answer
            }
        };
        match newest {
            Ok(commit) if commit != one.commit => out.offers.push(Offer {
                name: one.name.clone(),
                level: one.level.0.clone(),
                remote: one.source.clone(),
                pinned: one.commit.clone(),
                newer: commit,
                edited: edited(plane, one).is_none_or(|changed| !changed.is_empty()),
            }),
            Ok(_) => {}
            Err(why) => out.unasked.push(Unasked {
                name: one.name.clone(),
                level: one.level.0.clone(),
                why,
            }),
        }
    }
    let answered: Vec<Remote> = asked
        .into_iter()
        .filter_map(|(remote, answer)| answer.is_ok().then_some(remote))
        .collect();
    super::checked::asked(plane, &answered);
    out
}

/// How a file changes in an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Changed {
    Added,
    Removed,
    Changed,
}

/// One file's change: its lines' diff, empty for a file that is not text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChange {
    pub path: String,
    pub change: Changed,
    pub diff: String,
}

/// **The lines that changed from `was` to `now`**, as the one diff engine finds them
/// ([`crate::files::line_hunks`]): each hunk's numbers, then its lines taken out (`-`) and put
/// in (`+`), every invisible character escaped. Empty where nothing changed.
pub fn line_diff(was: &str, now: &str) -> String {
    let old: Vec<&str> = was.lines().collect();
    let new: Vec<&str> = now.lines().collect();
    let mut out = String::new();
    for hunk in crate::files::line_hunks(was, now) {
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
        ));
        let lines = |all: &[&str], start: u32, count: u32, mark: char, out: &mut String| {
            let from = start.saturating_sub(1) as usize;
            for line in all.iter().skip(from).take(count as usize) {
                out.push(mark);
                out.push_str(&summary::escaped(line));
                out.push('\n');
            }
        };
        lines(&old, hunk.old_start, hunk.old_lines, '-', &mut out);
        lines(&new, hunk.new_start, hunk.new_lines, '+', &mut out);
    }
    out
}

/// The text of `file`, `None` where the summary reads it as not text.
fn text(file: &SourceFile) -> Option<&str> {
    summary::FileIn {
        path: "",
        executable: file.executable,
        bytes: &file.bytes,
    }
    .text()
}

/// Each file's change from `was` to `now`.
fn file_changes(was: &[SourceFile], now: &[SourceFile]) -> Vec<FileChange> {
    let mut out = Vec::new();
    for file in now {
        match was.iter().find(|old| old.parts == file.parts) {
            None => out.push(FileChange {
                path: file.path(),
                change: Changed::Added,
                diff: text(file).map(|t| line_diff("", t)).unwrap_or_default(),
            }),
            Some(old) if old.bytes != file.bytes || old.executable != file.executable => {
                out.push(FileChange {
                    path: file.path(),
                    change: Changed::Changed,
                    diff: match (text(old), text(file)) {
                        (Some(a), Some(b)) => line_diff(a, b),
                        _ => String::new(),
                    },
                });
            }
            Some(_) => {}
        }
    }
    for old in was {
        if !now.iter().any(|file| file.parts == old.parts) {
            out.push(FileChange {
                path: old.path(),
                change: Changed::Removed,
                diff: text(old).map(|t| line_diff(t, "")).unwrap_or_default(),
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// **An update's review** (ADR 0100 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateReview {
    pub name: String,
    pub level: Level,
    pub pinned: Commit,
    pub newer: Commit,
    /// Whether the pinned commit could be read from the source; where not, the diff is from the
    /// copy installed here.
    pub from_pin: bool,
    pub changes: Changes,
    pub files: Vec<FileChange>,
    /// The files edited here since the install, which accepting replaces.
    pub edited: Vec<String>,
    /// Those edits, as their diff from the pin: beside the source's change, never lost silently.
    pub local_edits: Vec<FileChange>,
    pub review: SkillReview,
    /// The skills the newer commit refers to that are not met at this level already: they come
    /// along. Never one under a built-in skill's name, which a reference meets (#4155).
    pub brought: Vec<SkillReview>,
    /// The updated skill's own grant, where it reaches past the root (#4186): each skill it
    /// brings along lands with it, as an install's do with the install's grant.
    pub grant: Option<Grant>,
    /// What that grant asks of the project skills it and those it brings along refer to and
    /// leave as they are (#4172, #4186): each grant the update widens, and each workspace that
    /// turned one off.
    pub widenings: Vec<Widening>,
}

impl UpdateReview {
    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        let mut all = vec![self.review.clone()];
        all.extend(self.brought.iter().cloned());
        Accepted::of(&self.newer, &all)
    }
}

/// **The grant an update of `name` at `level` writes**, where it reaches past the root (#4186):
/// the updated skill's own, as [`super::install`] grants what it installs. One whose grant
/// cannot be read reaches the root alone, as a chat reads it.
fn grant_here(plane: &Path, level: &Level, name: &str) -> Option<Grant> {
    let mut parts = level_parts(level).ok()?;
    parts.push(name.to_owned());
    let grant = crate::skills::grant_of(&plane.join(parts.join("/"))).ok()?;
    widen::granted_beyond_the_root(level, Some(&grant)).cloned()
}

/// What `grant` asks of the skills `skills` refer to and leave as they are.
fn widenings_of<'a>(
    plane: &Path,
    level: &Level,
    grant: Option<&Grant>,
    skills: impl IntoIterator<Item = &'a SkillReview>,
) -> Vec<Widening> {
    widen::widenings(
        plane,
        level,
        grant,
        skills
            .into_iter()
            .flat_map(|skill| skill.already.iter().map(String::as_str)),
    )
}

/// The installed skill `name` at `level`, as its lockfile records it, and whether this machine
/// may contact its source.
fn locked_at(plane: &Path, level: &Level, name: &str) -> Result<(lock::Read, usize), String> {
    let kept = Kept::of(level);
    let held = lock::read(plane, kept)?;
    let at = held
        .lock
        .skills
        .iter()
        .position(|one| one.is(level, name))
        .ok_or_else(|| format!("no {level} skill named {name} was installed from a source"))?;
    match may(plane, kept, &held.lock.skills[at]) {
        May::Ask => Ok((held, at)),
        May::Confirm => Err(format!(
            "{} is not checked on this machine until it is confirmed here",
            held.lock.skills[at].source
        )),
        May::Never(why) => Err(why),
    }
}

/// The bytes of a file of the copy of `locked` here, its `SKILL.md` under its name at the source
/// where it has a new one, so a diff from the source never shows the new name as a change.
fn named_at_source(parts: &[String], bytes: Vec<u8>, locked: &Locked) -> Vec<u8> {
    let Some(name) = locked
        .source_name
        .as_deref()
        .filter(|_| parts == [purlis_skills::FILE])
    else {
        return bytes;
    };
    std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| purlis_skills::change::with_name(text, name))
        .map_or(bytes.clone(), String::into_bytes)
}

/// **The review of updating the installed skill `name` at `level`** to its source's newest
/// commit, with the built-in skills in `built_in`, whose names a reference meets already (#4155).
/// Refused where the pin is the newest, or where this machine may not contact the source.
pub fn update_review(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
) -> Result<UpdateReview, String> {
    let (held, at) = locked_at(plane, level, name)?;
    let locked = &held.lock.skills[at];
    let at_source = locked.at_source().to_owned();
    let source = source_of(plane, locked)?;
    let fetched = fetch(plane, &source, None)?;
    if fetched.commit == locked.commit {
        return Err(format!(
            "{name} is pinned to its source's newest commit already"
        ));
    }
    let dir = tree::folder_parts(&locked.folder)
        .ok_or_else(|| format!("{:?} is no folder", locked.folder))?;
    let (found, _) = fetched.skills()?;
    let one = found
        .iter()
        .find(|f| f.dir == dir)
        .ok_or_else(|| format!("the source no longer holds {name} at {}", locked.folder))?;
    if one.name != at_source {
        return Err(format!(
            "the source renamed {at_source} to {}: install it under its new name",
            one.name
        ));
    }
    let now = fetched.files_of(&one.dir)?;
    let pinned = fetched.files_at(plane, &locked.commit, &locked.folder)?;
    let from_pin = pinned.is_some();
    let mut parts = level_parts(level)?;
    parts.push(name.to_owned());
    let here: Vec<SourceFile> =
        write::read_all(plane, &parts, tree::MOST_FILE_BYTES, tree::MOST_FILES)
            .ok_or_else(|| format!("{name}'s copy here could not be read"))?
            .into_iter()
            .map(|file| SourceFile {
                bytes: named_at_source(&file.parts, file.bytes, locked),
                parts: file.parts,
                executable: file.executable,
            })
            .collect();
    let edited = edited(plane, locked).unwrap_or_default();
    let local_edits = match &pinned {
        Some(pin) if !edited.is_empty() => file_changes(pin, &here),
        _ => Vec::new(),
    };
    let was = pinned.unwrap_or_else(|| here.clone());
    let met_here = met_at(plane, built_in, level);
    let reviewed = fetched.review_with(std::slice::from_ref(&at_source), &met_here)?;
    let mut skills = reviewed.skills.into_iter();
    let review = skills.next().unwrap_or_else(|| {
        let held_names: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
        review_of(
            one,
            &now,
            tree::left_in(&fetched.left, &one.dir),
            None,
            &held_names,
            &|other| other != at_source && met_here(other),
        )
    });
    let brought: Vec<SkillReview> = skills.collect();
    let grant = grant_here(plane, level, name);
    let widenings = widenings_of(
        plane,
        level,
        grant.as_ref(),
        std::iter::once(&review).chain(&brought),
    );
    Ok(UpdateReview {
        name: name.to_owned(),
        level: level.clone(),
        pinned: locked.commit.clone(),
        newer: fetched.commit.clone(),
        from_pin,
        changes: summary::changes(&summary_of(&was), &review.summary),
        files: file_changes(&was, &now),
        edited,
        local_edits,
        review,
        brought,
        grant,
        widenings,
    })
}

/// **Accepts an update** of the installed skill `name` at `level` to what `accepted` says: the
/// newer commit fetched again and checked against the reviewed digests; the skills it brings
/// along installed beside it, granted as the updated skill is (#4186); each project skill it relies
/// on widened to reach there too; each of its files written over the copy, each file the old
/// commit had and the newer one does not removed; and the pins moved, as one commit. `grant` is
/// the grant the review showed ([`UpdateReview::grant`]): where the skill's grant changed since,
/// nothing is written. Where a skill it
/// brings along or the lockfile cannot be written, what this update made is taken back.
pub fn update(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let (held, at) = locked_at(plane, level, name)?;
    let locked = held.lock.skills[at].clone();
    let at_source = locked.at_source().to_owned();
    let source = source_of(plane, &locked)?;
    let fetched = fetch(plane, &source, Some(&accepted.commit))?;
    let reviewed = fetched.review_with(
        std::slice::from_ref(&at_source),
        &met_at(plane, cx.built_in, level),
    )?;
    if !accepted.is(&reviewed.accepted())
        || reviewed.skills.first().is_none_or(|s| s.name != at_source)
    {
        return Err(
            "what the source holds is not what was reviewed, so nothing was updated: review it \
             again"
                .to_owned(),
        );
    }
    let dir = tree::folder_parts(&locked.folder)
        .ok_or_else(|| format!("{:?} is no folder", locked.folder))?;
    let (found, _) = fetched.skills()?;
    let find = |wanted: &str| {
        found
            .iter()
            .find(|f| f.name == wanted)
            .ok_or_else(|| format!("the source no longer holds {wanted}"))
    };
    let one = find(&at_source)?;
    if one.dir != dir {
        return Err(format!(
            "the source moved {name}: install it again from where it is now"
        ));
    }
    // A skill it brings along takes no built-in skill's name, as an install's does not. A
    // reference to one is met already (#4155), so none is brought: this is the guard (S12).
    if let Some(skill) = reviewed
        .skills
        .iter()
        .skip(1)
        .find(|s| cx.reserved(&s.name))
    {
        return Err(format!(
            "this update brings along {}, whose name belongs to a built-in skill, so nothing \
             was updated",
            skill.name
        ));
    }
    // What it brings along lands granted as the review said, and what it relies on is widened
    // by that grant: one that changed since was never shown.
    let granted = grant_here(plane, level, name);
    if granted.as_ref() != grant {
        return Err(format!(
            "the grant of {name} changed since the review, so nothing was updated: review it again"
        ));
    }
    let level_dir = level_parts(level)?;
    let files = fetched.files_of(&one.dir)?;
    paths_fit(&level_dir, name, &files)?;
    let renamed = locked
        .source_name
        .is_some()
        .then(|| super::install::under_new_name(&files, name))
        .transpose()?;
    let files = renamed.as_ref().map_or(&files, |renamed| renamed);
    let mut lock = held.lock.clone();
    // The skills it brings along, first: each a new copy this update can take back.
    let mut written = Written::none(plane, level_dir.clone());
    let mut paths: Vec<String> = Vec::new();
    for skill in reviewed.skills.iter().skip(1) {
        let other = find(&skill.name)?;
        let their = fetched.files_of(&other.dir)?;
        let fits = verbs::free_at(plane, level, &skill.name, &skill.name)
            .and_then(|_| paths_fit(&level_dir, &skill.name, &their));
        let writes: Vec<write::File<'_>> = their
            .iter()
            .map(|file| write::File {
                parts: &file.parts,
                executable: file.executable,
                bytes: &file.bytes,
            })
            .collect();
        let made = fits.and_then(|()| {
            write::new_skill(plane, &level_dir, level.depth(), &skill.name, &writes)
                .map_err(|why| format!("{} could not be installed ({why})", skill.name))
        });
        match made {
            Ok(made) => written.skills.push((skill.name.clone(), made)),
            Err(why) => {
                written.take_back();
                return Err(format!("{why}, so nothing was updated"));
            }
        }
        let their_wrote = wrote(&their);
        lock.skills.push(Locked {
            name: skill.name.clone(),
            source_name: None,
            level: lock::AtLevel(level.clone()),
            source: locked.source.clone(),
            folder: other.folder.clone(),
            commit: fetched.commit.clone(),
            digest: digest_of(&their_wrote),
            files: their_wrote,
            brought_by: Some(name.to_owned()),
        });
        paths.push(format!("{}/{}", level_dir.join("/"), skill.name));
    }
    // Each lands where the skill that brings it reaches, and a project skill they rely on
    // reaches there too (#4186), as an install with that grant does (#4172).
    let brought: Vec<String> = written.skills.iter().map(|(n, _)| n.clone()).collect();
    if let Some(grant) = grant {
        for skill in &brought {
            let folder = plane.join(level_dir.join("/")).join(skill);
            if let Err(why) = crate::skills::set_grant(&folder, grant) {
                written.take_back();
                return Err(format!(
                    "{skill} could not be granted to {grant} ({why}), so nothing was updated"
                ));
            }
            written.grants.push(skill.clone());
            paths.push(verbs::rel(plane, &crate::skills::beside(&folder)));
        }
    }
    let widenings = widenings_of(plane, level, grant, &reviewed.skills);
    if let Err(why) = written.widen(&widenings, &mut paths) {
        written.take_back();
        return Err(format!("{why}, so nothing was updated"));
    }
    let files_wrote = if locked.source_name.is_some() {
        super::review::wrote_renamed(files)
    } else {
        wrote(files)
    };
    let mut parts = level_dir.clone();
    parts.push(name.to_owned());
    let gone: Vec<Vec<String>> = locked
        .files
        .iter()
        .filter(|was| !files_wrote.iter().any(|now| now.path == was.path))
        .filter_map(|was| tree::folder_parts(&was.path))
        .collect();
    let writes: Vec<write::File<'_>> = files
        .iter()
        .map(|file| write::File {
            parts: &file.parts,
            executable: file.executable,
            bytes: &file.bytes,
        })
        .collect();
    if let Err(why) = write::replace(plane, &parts, &writes, &gone) {
        written.take_back();
        return Err(format!(
            "{name} could not be updated ({why}): what was written of it is in its folder, and \
             the pin is where it was"
        ));
    }
    let entry = &mut lock.skills[at];
    entry.commit = fetched.commit.clone();
    entry.digest = digest_of(&files_wrote);
    entry.files = files_wrote;
    let kept = Kept::of(level);
    if let Err(why) = lock::write(plane, kept, held.was.as_deref(), &lock) {
        written.take_back();
        return Err(why);
    }
    // Each skill it brought is a new one: the grades of one that had its name before are none of
    // its own (#4104).
    for skill in &brought {
        verbs::earlier_dropped(cx, level, skill);
    }
    let folder = plane.join(parts.join("/"));
    cx.done(format!(
        "{name} is updated at {level}, pinned to {}",
        fetched.commit.short()
    ));
    if let Some(grant) = grant.filter(|_| !brought.is_empty()) {
        let are = if brought.len() == 1 { "is" } else { "are" };
        cx.done(format!(
            "{} {are} granted to {grant}, as {name} is",
            brought.join(", ")
        ));
    }
    widen::said_done(cx, &widenings, "what was updated");
    if kept == Kept::Shared {
        paths.push(verbs::rel(plane, &folder));
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill update {name}"));
    }
    Ok(folder)
}

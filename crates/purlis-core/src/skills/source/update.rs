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
    Accepted, Hints, SkillReview, digest_of, met_at, review_of, reviews_of, sha256, summary_of,
    wrote,
};
use super::widen::{self, Widening};
use super::{Origin, Pin, Remote, SkillSource, Source, listed, write};
use crate::skills::Grant;
use crate::skills::studio::Studio;
use crate::skills::verbs::{self, Ctx, Level};

mod gained;
pub mod set;

pub use gained::Gained;

/// **A newer commit or revision for an installed skill**, offered, never applied by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub name: String,
    pub level: Level,
    pub source: Origin,
    pub pinned: Pin,
    pub newer: Pin,
    /// Whether the copy was edited here since it was installed.
    pub edited: bool,
    /// What the project's controls would refuse of it, said before its review (#6031): its
    /// source or skill not allowed, or the newer commit too young, with when it can be taken.
    pub refused: Option<super::policy::Refused>,
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Updates {
    pub offers: Vec<Offer>,
    pub unasked: Vec<Unasked>,
    pub unconfirmed: Vec<Unconfirmed>,
}

/// The git source `locked` was installed from, with the sign-in this machine gives it; refused
/// for one an extension's source answered, which is asked through the studio
/// ([`super::extension`]).
fn source_of(plane: &Path, locked: &Locked) -> Result<Source, String> {
    let Origin::Git(remote) = &locked.source else {
        return Err(format!(
            "{} came from {}, which purlis asks through its extensions: update it on its own",
            locked.name, locked.source
        ));
    };
    let listed = listed::listed_as(plane, remote)?;
    Ok(Source {
        remote: remote.clone(),
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

/// An extension's source is asked where this machine approved the extension and the project has
/// it on, as every studio question is (ADR 0101): the approval is this machine's consent, and the
/// executor's gate holds it.
fn may(plane: &Path, kept: Kept, locked: &Locked) -> May {
    let Origin::Git(remote) = &locked.source else {
        return May::Ask;
    };
    if kept == Kept::Local {
        return May::Ask;
    }
    match listed::confirmed(plane, remote) {
        Ok(true) => May::Ask,
        Ok(false) => match remote.check(true) {
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

/// **Whether the copy `locked` installed was edited here since**: one that cannot be read counts,
/// so nothing is taken for untouched that was not read.
pub fn edited_since_install(plane: &Path, locked: &Locked) -> bool {
    edited(plane, locked).is_none_or(|changed| !changed.is_empty())
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
/// its refs alone, nothing fetched or written — or, for an extension's source, its newest
/// revision, through `studio` (its listing alone), and answers each skill whose pin is older,
/// each that could not be asked, and each source that waits to be confirmed here. With no
/// `studio`, an extension's source is not asked and says so.
pub fn updates(plane: &Path, studio: Option<Studio<'_>>) -> Updates {
    let (locked, unread) = lock::every(plane);
    let mut out = Updates::default();
    for why in unread {
        out.unasked.push(Unasked {
            name: String::new(),
            level: Level::project(),
            why,
        });
    }
    let mut asked: Vec<(Origin, Result<Pin, String>)> = Vec::new();
    // The org's controls, read once, and when each newer commit was made, asked once per source
    // and only while they hold a minimum age (#6031).
    let policy = super::policy::of(plane);
    let mut made: Vec<(Origin, Option<u64>)> = Vec::new();
    for (kept, one) in &locked {
        match may(plane, *kept, one) {
            May::Ask => {}
            May::Confirm => {
                let Origin::Git(remote) = &one.source else {
                    continue;
                };
                match out.unconfirmed.iter_mut().find(|u| u.remote == *remote) {
                    Some(waiting) => waiting.skills.push(one.name.clone()),
                    None => out.unconfirmed.push(Unconfirmed {
                        remote: remote.clone(),
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
        let newest = match asked.iter().find(|(origin, _)| *origin == one.source) {
            Some((_, answer)) => answer.clone(),
            None => {
                let answer = match &one.source {
                    Origin::Git(_) => source_of(plane, one)
                        .and_then(|source| super::newest(plane, &source))
                        .map(Pin::Commit),
                    Origin::Extension(id) => match studio {
                        Some(studio) => super::extension::newest(studio, id).map(Pin::Revision),
                        None => Err(format!(
                            "{} is asked only where purlis can start its extensions",
                            one.source
                        )),
                    },
                };
                asked.push((one.source.clone(), answer.clone()));
                answer
            }
        };
        match newest {
            Ok(pin) if pin != one.pin => out.offers.push(Offer {
                refused: refused_before(plane, &policy, &mut made, one, &pin),
                name: one.name.clone(),
                level: one.level.0.clone(),
                source: one.source.clone(),
                pinned: one.pin.clone(),
                newer: pin,
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
        .filter(|(_, answer)| answer.is_ok())
        .filter_map(|(origin, _)| origin.remote().cloned())
        .collect();
    super::checked::asked(plane, &answered);
    out
}

/// **What the controls `policy` would refuse of updating `one` to `pin`**, before its review
/// (#6031). A git source's newer commit is asked when it was made, once per source in `made`,
/// while the controls hold a minimum age; where that cannot be read, only the lists are held to,
/// and the review says the rest. An extension's revision cannot say, so the controls say so.
fn refused_before(
    plane: &Path,
    policy: &Result<Option<super::policy::Policy>, String>,
    made: &mut Vec<(Origin, Option<u64>)>,
    one: &Locked,
    pin: &Pin,
) -> Option<super::policy::Refused> {
    let name = one.at_source();
    let Pin::Commit(commit) = pin else {
        return super::policy::refused_before(policy, &one.source, name, pin, None);
    };
    if !super::policy::asks_age(policy) {
        return super::policy::listed_before(policy, &one.source, name);
    }
    let at = match made.iter().find(|(origin, _)| *origin == one.source) {
        Some((_, at)) => *at,
        None => {
            let at = source_of(plane, one)
                .ok()
                .and_then(|source| super::fetch::newest_made(plane, &source, commit));
            made.push((one.source.clone(), at));
            at
        }
    };
    match at {
        Some(at) => super::policy::refused_before(policy, &one.source, name, pin, Some(at)),
        None => super::policy::listed_before(policy, &one.source, name),
    }
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
    pub pinned: Pin,
    pub newer: Pin,
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
    /// along. Never one under a name of purlis's own, which a reference meets (#4155).
    pub brought: Vec<SkillReview>,
    /// The updated skill's own grant, where it reaches past the root (#4186): each skill it
    /// brings along lands with it, as an install's do with the install's grant.
    pub grant: Option<Grant>,
    /// What that grant asks of the project skills it and those it brings along refer to and
    /// leave as they are (#4172, #4186): each grant the update widens, and each workspace that
    /// turned one off.
    pub widenings: Vec<Widening>,
    /// What the update newly lets the skill do, beyond an edit of its text (#5354): a review of
    /// several updates leaves one that gains any of it unticked.
    pub gained: Vec<Gained>,
}

impl UpdateReview {
    /// **Every skill it reviews**: the one updated, then each it brings along.
    pub fn skills_mut(&mut self) -> impl Iterator<Item = &mut SkillReview> {
        std::iter::once(&mut self.review).chain(&mut self.brought)
    }

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
/// commit. Refused where the pin is the newest, or where this machine may not contact the source.
pub fn update_review(plane: &Path, level: &Level, name: &str) -> Result<UpdateReview, String> {
    let (held, at) = locked_at(plane, level, name)?;
    let locked = &held.lock.skills[at];
    let source = source_of(plane, locked)?;
    let fetched = fetch(plane, &source, None)?;
    let newer = Pin::Commit(fetched.commit.clone());
    if newer == locked.pin {
        return Err(format!(
            "{name} is pinned to its source's newest commit already"
        ));
    }
    let pinned = match &locked.pin {
        Pin::Commit(commit) => fetched.files_at(plane, commit, &locked.folder)?,
        Pin::Revision(_) => None,
    };
    let review = reviewed_update(plane, level, name, locked, &fetched, newer.clone(), pinned)?;
    // The org's controls, said in the review before it is accepted (SK-11).
    super::policy::hold(
        plane,
        &locked.source,
        std::iter::once(locked.at_source())
            .chain(review.brought.iter().map(|one| one.name.as_str())),
        &newer,
        || fetched.made(),
    )?;
    Ok(review)
}

/// **The review of updating `locked`, the skill `name` at `level`, to what `source` holds at
/// `newer`**, its files at the pin `pinned` where the source still holds them: the diff is from
/// those, else from the copy here. The same review for every source.
pub(super) fn reviewed_update(
    plane: &Path,
    level: &Level,
    name: &str,
    locked: &Locked,
    source: &dyn SkillSource,
    newer: Pin,
    pinned: Option<Vec<SourceFile>>,
) -> Result<UpdateReview, String> {
    let at_source = locked.at_source().to_owned();
    let dir = tree::folder_parts(&locked.folder)
        .ok_or_else(|| format!("{:?} is no folder", locked.folder))?;
    let (found, _) = source.skills()?;
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
    let (now, left) = source.files(one)?;
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
    let met_here = met_at(plane, level);
    let reviewed = reviews_of(
        source,
        std::slice::from_ref(&at_source),
        &met_here,
        Hints::Every,
    )?;
    let mut skills = reviewed.into_iter();
    let review = skills.next().unwrap_or_else(|| {
        let held_names: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
        review_of(one, &now, left, None, false, &held_names, &|other| {
            other != at_source && met_here(other)
        })
    });
    let brought: Vec<SkillReview> = skills.collect();
    let grant = grant_here(plane, level, name);
    let widenings = widenings_of(
        plane,
        level,
        grant.as_ref(),
        std::iter::once(&review).chain(&brought),
    );
    let gained = gained::gained(&was, &now, &brought);
    Ok(UpdateReview {
        gained,
        name: name.to_owned(),
        level: level.clone(),
        pinned: locked.pin.clone(),
        newer,
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

/// A skill an update brings along: its name, its folder in the source, and its files.
struct Bringing {
    name: String,
    folder: String,
    files: Vec<SourceFile>,
}

/// **An update checked and ready to write** ([`plan`]): everything accepting it reads, nothing
/// written yet.
pub(super) struct Planned {
    pub(super) level: Level,
    pub(super) name: String,
    /// Its lockfile entry as it is now.
    pub(super) locked: Locked,
    /// The commit or revision it moves to.
    pub(super) newer: Pin,
    /// What it newly lets the skill do (#5354), read against the pinned commit as its review is.
    pub(super) gained: Vec<Gained>,
    files: Vec<SourceFile>,
    brought: Vec<Bringing>,
    grant: Option<Grant>,
    widenings: Vec<Widening>,
}

impl Planned {
    /// The level's folder parts and the copy's.
    fn parts(&self) -> Result<(Vec<String>, Vec<String>), String> {
        let level_dir = level_parts(&self.level)?;
        let mut parts = level_dir.clone();
        parts.push(self.name.clone());
        Ok((level_dir, parts))
    }

    /// Its lockfile entry once it is written.
    pub(super) fn entry(&self) -> Locked {
        let mut entry = self.locked.clone();
        let files_wrote = if self.locked.source_name.is_some() {
            super::review::wrote_renamed(&self.files)
        } else {
            wrote(&self.files)
        };
        entry.pin = self.newer.clone();
        entry.digest = digest_of(&files_wrote);
        entry.files = files_wrote;
        entry
    }

    /// The lockfile entries of the skills it brings along.
    pub(super) fn brought_entries(&self) -> Vec<Locked> {
        self.brought
            .iter()
            .map(|skill| {
                let their_wrote = wrote(&skill.files);
                Locked {
                    name: skill.name.clone(),
                    source_name: None,
                    level: lock::AtLevel(self.level.clone()),
                    source: self.locked.source.clone(),
                    folder: skill.folder.clone(),
                    pin: self.newer.clone(),
                    digest: digest_of(&their_wrote),
                    files: their_wrote,
                    brought_by: Some(self.name.clone()),
                }
            })
            .collect()
    }

    /// The names of the skills it brings along.
    pub(super) fn brought_names(&self) -> Vec<String> {
        self.brought.iter().map(|one| one.name.clone()).collect()
    }

    /// What an org's project audits of it ([`super::policy`]): the skill and each it brings.
    pub(super) fn audited(&self) -> super::policy::Audited {
        super::policy::Audited::new(
            super::policy::Act::Update,
            &self.level,
            std::iter::once(self.name.clone())
                .chain(self.brought_names())
                .collect(),
            &self.locked.source,
            &self.newer,
        )
    }

    /// What its grant widens.
    pub(super) fn widenings(&self) -> &[Widening] {
        &self.widenings
    }
}

/// **What one update wrote**, to take back whole: the skills it brought along, their grants and
/// what it widened, and the copy's files as they were before.
pub(super) struct Wrote {
    written: Written,
    copy: Vec<String>,
    before: Vec<write::HeldFile>,
    /// What it wrote into the copy that the copy did not hold before.
    added: Vec<Vec<String>>,
    /// Every path the change wrote, for its commit.
    pub(super) paths: Vec<String>,
}

impl Wrote {
    /// Whether every grant it widened still reads as it left it: an Undo that put back the grant
    /// it had before would lose a change made since.
    pub(super) fn grants_as_left(&self) -> bool {
        self.written
            .widened_to
            .iter()
            .all(|(folder, to)| crate::skills::grant_of(folder).is_ok_and(|now| now == *to))
    }

    /// Takes back everything it wrote: the copy's files as they were, and what it brought along.
    /// What could not be, in words.
    pub(super) fn take_back(&self) -> Option<String> {
        let files: Vec<write::File<'_>> = self
            .before
            .iter()
            .map(|file| write::File {
                parts: &file.parts,
                executable: file.executable,
                bytes: &file.bytes,
            })
            .collect();
        let back = write::replace(&self.written.plane, &self.copy, &files, &self.added);
        self.written.take_back();
        back.err().map(|why| {
            format!(
                "{} could not be put back as it was ({why})",
                self.copy.last().map_or("", String::as_str)
            )
        })
    }
}

/// **Checks an update** of the installed skill `name` at `level` to what `accepted` says,
/// writing nothing: the newer commit fetched again and checked against the reviewed digests
/// ([`plan_from`]). Refused for a skill an extension's source answered, which is updated on its
/// own ([`super::extension::update`]).
pub(super) fn plan(
    cx: &Ctx<'_>,
    level: &Level,
    name: &str,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<Planned, String> {
    let plane = cx.plane;
    let (held, at) = locked_at(plane, level, name)?;
    let locked = &held.lock.skills[at];
    let source = source_of(plane, locked)?;
    let fetched = fetch(plane, &source, Some(accepted.commit()?))?;
    let pinned = match &locked.pin {
        Pin::Commit(commit) => fetched.files_at(plane, commit, &locked.folder)?,
        Pin::Revision(_) => None,
    };
    plan_from(
        cx,
        level,
        name,
        accepted,
        grant,
        &fetched,
        Pin::Commit(fetched.commit.clone()),
        pinned,
    )
}

/// **Checks an update** of the installed skill `name` at `level` to what `accepted` says, from
/// what `source` holds at `newer`, writing nothing: the review read again and checked against the
/// reviewed digests, every skill it brings along free where it lands, and `grant` the grant the
/// review showed ([`UpdateReview::grant`]). `pinned` is the files at the pin, where the source
/// still holds them, which what it gains is read against as its review read it.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_from(
    cx: &Ctx<'_>,
    level: &Level,
    name: &str,
    accepted: &Accepted,
    grant: Option<&Grant>,
    source: &dyn SkillSource,
    newer: Pin,
    pinned: Option<Vec<SourceFile>>,
) -> Result<Planned, String> {
    let plane = cx.plane;
    let (held, at) = locked_at(plane, level, name)?;
    let locked = held.lock.skills[at].clone();
    let at_source = locked.at_source().to_owned();
    let reviewed = reviews_of(
        source,
        std::slice::from_ref(&at_source),
        &met_at(plane, level),
        Hints::Every,
    )?;
    if !accepted.is(&Accepted::of(&newer, &reviewed))
        || reviewed.first().is_none_or(|s| s.name != at_source)
    {
        return Err(
            "what the source holds is not what was reviewed, so nothing was updated: review it \
             again"
                .to_owned(),
        );
    }
    // The org's controls, asked of the newer commit and of every skill it brings (SK-11).
    super::policy::hold(
        plane,
        &locked.source,
        reviewed.iter().map(|one| one.name.as_str()),
        &newer,
        || source.made(),
    )
    .map_err(|why| format!("{why}, so nothing was updated"))?;
    let dir = tree::folder_parts(&locked.folder)
        .ok_or_else(|| format!("{:?} is no folder", locked.folder))?;
    let (found, _) = source.skills()?;
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
    if let Some(skill) = reviewed.iter().skip(1).find(|s| cx.reserved(&s.name)) {
        return Err(format!(
            "this update brings along {}, whose name belongs to purlis, so nothing \
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
    let (now, _) = source.files(one)?;
    paths_fit(&level_dir, name, &now)?;
    let files = match locked.source_name.is_some() {
        true => super::install::under_new_name(&now, name)?,
        false => now.clone(),
    };
    let mut brought = Vec::new();
    for skill in reviewed.iter().skip(1) {
        let other = find(&skill.name)?;
        let (their, _) = source.files(other)?;
        verbs::free_at(plane, level, &skill.name, &skill.name)
            .and_then(|_| paths_fit(&level_dir, &skill.name, &their))
            .map_err(|why| format!("{why}, so nothing was updated"))?;
        brought.push(Bringing {
            name: skill.name.clone(),
            folder: other.folder.clone(),
            files: their,
        });
    }
    let widenings = widenings_of(plane, level, grant, &reviewed);
    // What it gains is read as its review read it: from the pinned commit, or the copy here where
    // the source no longer holds that.
    let was = match pinned {
        Some(pinned) => pinned,
        None => {
            let mut parts = level_dir.clone();
            parts.push(name.to_owned());
            write::read_all(plane, &parts, tree::MOST_FILE_BYTES, tree::MOST_FILES)
                .ok_or_else(|| format!("{name}'s copy here could not be read"))?
                .into_iter()
                .map(|file| SourceFile {
                    bytes: named_at_source(&file.parts, file.bytes, &locked),
                    parts: file.parts,
                    executable: file.executable,
                })
                .collect()
        }
    };
    let gained = gained::gained(&was, &now, &reviewed[1..]);
    Ok(Planned {
        level: level.clone(),
        name: name.to_owned(),
        newer,
        locked,
        gained,
        files,
        brought,
        grant: grant.cloned(),
        widenings,
    })
}

/// **Writes what `planned` checked**: the skills it brings along, granted as the updated skill is
/// (#4186); each project skill it relies on widened to reach there too; and each of its files
/// written over the copy, each the lockfile says the install wrote and the newer commit lacks
/// removed. The lockfile is the caller's. Where any of it fails, what it wrote is taken back.
pub(super) fn write_planned(plane: &Path, planned: &Planned) -> Result<Wrote, String> {
    let name = &planned.name;
    let level = &planned.level;
    let (level_dir, parts) = planned.parts()?;
    let before = write::read_all(plane, &parts, tree::MOST_FILE_BYTES, tree::MOST_FILES)
        .ok_or_else(|| format!("{name}'s copy here could not be read, so nothing was updated"))?;
    let mut written = Written::none(plane, level_dir.clone());
    let mut paths: Vec<String> = Vec::new();
    for skill in &planned.brought {
        let writes: Vec<write::File<'_>> = skill
            .files
            .iter()
            .map(|file| write::File {
                parts: &file.parts,
                executable: file.executable,
                bytes: &file.bytes,
            })
            .collect();
        match write::new_skill(plane, &level_dir, level.depth(), &skill.name, &writes) {
            Ok(made) => written.skills.push((skill.name.clone(), made)),
            Err(why) => {
                written.take_back();
                return Err(format!(
                    "{} could not be installed ({why}), so nothing was updated",
                    skill.name
                ));
            }
        }
        paths.push(format!("{}/{}", level_dir.join("/"), skill.name));
    }
    if let Some(grant) = &planned.grant {
        for skill in &planned.brought {
            let folder = plane.join(level_dir.join("/")).join(&skill.name);
            if let Err(why) = crate::skills::set_grant(&folder, grant) {
                written.take_back();
                return Err(format!(
                    "{} could not be granted to {grant} ({why}), so nothing was updated",
                    skill.name
                ));
            }
            written.grants.push(skill.name.clone());
            paths.push(verbs::rel(plane, &crate::skills::beside(&folder)));
        }
    }
    if let Err(why) = written.widen(&planned.widenings, &mut paths) {
        written.take_back();
        return Err(format!("{why}, so nothing was updated"));
    }
    let entry = planned.entry();
    let gone: Vec<Vec<String>> = planned
        .locked
        .files
        .iter()
        .filter(|was| !entry.files.iter().any(|now| now.path == was.path))
        .filter_map(|was| tree::folder_parts(&was.path))
        .collect();
    let added: Vec<Vec<String>> = planned
        .files
        .iter()
        .filter(|file| !before.iter().any(|old| old.parts == file.parts))
        .map(|file| file.parts.clone())
        .collect();
    let writes: Vec<write::File<'_>> = planned
        .files
        .iter()
        .map(|file| write::File {
            parts: &file.parts,
            executable: file.executable,
            bytes: &file.bytes,
        })
        .collect();
    paths.push(verbs::rel(plane, &plane.join(parts.join("/"))));
    let wrote = Wrote {
        written,
        copy: parts,
        before,
        added,
        paths,
    };
    if let Err(why) = write::replace(plane, &wrote.copy, &writes, &gone) {
        let left = wrote.take_back();
        return Err(match left {
            None => format!("{name} could not be updated ({why}), so nothing was updated"),
            Some(left) => format!("{name} could not be updated ({why}), and {left}"),
        });
    }
    Ok(wrote)
}

/// **Accepts an update** of the installed skill `name` at `level` to what `accepted` says
/// ([`plan`], then [`write_planned`]), and moves the pins, as one commit. `grant` is the grant
/// the review showed ([`UpdateReview::grant`]): where the skill's grant changed since, nothing
/// is written. Where the lockfile cannot be written, what this update made is taken back.
pub fn update(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<PathBuf, String> {
    crate::settings::writable(cx.plane)?;
    let planned = plan(cx, level, name, accepted, grant)?;
    write_update(cx, level, name, grant, &planned)
}

/// **Writes the update `planned` checked** ([`write_planned`]) and moves the pins, as one commit.
/// Where the lockfile cannot be written, what this update made is taken back.
pub(super) fn write_update(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    grant: Option<&Grant>,
    planned: &Planned,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    let (held, at) = locked_at(plane, level, name)?;
    cx.audited(&planned.audited())
        .map_err(|why| format!("{why}, so nothing was updated"))?;
    let wrote = write_planned(plane, planned)?;
    let mut lock = held.lock.clone();
    lock.skills[at] = planned.entry();
    lock.skills.extend(planned.brought_entries());
    let kept = Kept::of(level);
    if let Err(why) = lock::write(plane, kept, held.was.as_deref(), &lock) {
        wrote.take_back();
        return Err(why);
    }
    let brought = planned.brought_names();
    // Each skill it brought is a new one: the grades of one that had its name before are none of
    // its own (#4104).
    for skill in &brought {
        verbs::earlier_dropped(cx, level, skill);
    }
    let (_, parts) = planned.parts()?;
    let folder = plane.join(parts.join("/"));
    cx.done(format!(
        "{name} is updated at {level}, pinned to {}",
        planned.newer.short()
    ));
    if let Some(grant) = grant.filter(|_| !brought.is_empty()) {
        let are = if brought.len() == 1 { "is" } else { "are" };
        cx.done(format!(
            "{} {are} granted to {grant}, as {name} is",
            brought.join(", ")
        ));
    }
    widen::said_done(cx, &planned.widenings, "what was updated");
    if kept == Kept::Shared {
        let mut paths = wrote.paths.clone();
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill update {name}"));
    }
    Ok(folder)
}

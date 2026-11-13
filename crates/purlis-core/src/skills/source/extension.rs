// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill source an extension adds** (the SkillSource capability, ADR 0101 §1; #2777, #3089):
//! one that is not git, such as a registry or an artifact store, answered by an extension that
//! declares `skill-source`.
//!
//! purlis asks it what skills it has at one revision ([`browse`]), and for one skill's files at
//! that revision; the review is purlis's own, the one the git source's install shows
//! ([`super::review::reviewed`]): every file, the summary first, the skills it refers to brought
//! along, and the same clash and notes. The extension answers inside its sandbox, and its answer
//! is read failing closed within one skill's limits ([`crate::extension::studio::files_of`]). It
//! never writes the project: it hands purlis files, and purlis alone copies what the person
//! accepted.
//!
//! **What is installed is what was reviewed, and the extension is not asked again** (#3089). A
//! review answers what it read as [`Held`], beside the review the person reads; the install or
//! update the person accepts is handed that back, reviews it again from it, and refuses where any
//! digest differs from the one accepted. The copy, the pin (the revision, in the commit's place,
//! ADR 0100 §8), the grant and the commit are the git source's own ([`super::install`],
//! [`super::update`]).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use purlis_skills::tree::LeftOut;

use super::fetch::{Found, NotASkill, SourceFile};
use super::install::{Renames, install_from};
use super::lock::{self, Locked};
use super::review::{self, Accepted, Set, SkillReview, TooMuch, installed_at, met_at};
use super::update::{Planned, UpdateReview, plan_from, reviewed_update, write_update};
use super::{Origin, Pin, Revision, SkillSource};
use crate::extension::studio::{self as wire, Ask};
use crate::skills::Grant;
use crate::skills::studio::Studio;
use crate::skills::verbs::{Ctx, Level};

/// One skill an extension's source offers, the levels it is installed at from it here, and what
/// its files do beyond its instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub skill: Found,
    pub installed: Vec<Level>,
    /// **What its files do beyond its instructions**, a few words each, as a git source's page
    /// marks them (#5760): `None` where they were not read to browse, past what one browse reads
    /// or where the extension's answer for it was refused; its review reads them.
    pub marks: Option<Vec<String>>,
}

/// **The most skills one browse of an extension's source reads the files of** for their marks
/// (#5760): each is one question to the extension in its sandbox, so a long list marks its
/// first ones and leaves the rest to their reviews, as a git source's past what one read may take.
pub const MOST_MARKED: usize = 32;

/// What an extension's source offers, at one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Browse {
    /// The extension, by id.
    pub extension: String,
    /// What it names what it holds by, now.
    pub revision: Revision,
    pub skills: Vec<Offered>,
}

/// **The review of installing skills from an extension's source**, at the revision it answered:
/// the same review the git source's install shows ([`super::Review`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub extension: String,
    pub revision: Revision,
    pub skills: Vec<SkillReview>,
}

impl Review {
    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        Accepted::of(&Pin::Revision(self.revision.clone()), &self.skills)
    }
}

/// **The review of a set from an extension's source** (#4024): those named that are installed
/// from it where they land already are left out and said to be there (#4153).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetReview {
    pub review: Review,
    pub there: Vec<String>,
}

/// **What a review of an extension's source read**: the revision, what it offered then, and the
/// files of each skill the review read, as the extension answered them. Handed back to the
/// install or update the person accepts, which copies these and asks the extension nothing
/// (#3089). Never written anywhere: the window holds it in memory until the person answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    extension: String,
    revision: Revision,
    found: Vec<Found>,
    files: BTreeMap<String, Vec<SourceFile>>,
    /// The grade each skill's files answer handed beside them, by the skill's name.
    grades: BTreeMap<String, Vec<u8>>,
}

impl Held {
    /// The extension it was read from, by id.
    pub fn extension(&self) -> &str {
        &self.extension
    }

    /// The revision it was read at.
    pub fn revision(&self) -> &Revision {
        &self.revision
    }

    fn origin(&self) -> Origin {
        Origin::Extension(self.extension.clone())
    }

    fn pin(&self) -> Pin {
        Pin::Revision(self.revision.clone())
    }

    /// Refused where it is not what `accepted` was accepted at.
    fn is_at(&self, accepted: &Accepted) -> Result<(), String> {
        if accepted.pin == self.pin() {
            Ok(())
        } else {
            Err(
                "what was accepted is not what the review read, so nothing was changed: review it \
                 again"
                    .to_owned(),
            )
        }
    }
}

impl SkillSource for Held {
    fn shown(&self) -> String {
        format!("the skill source '{}'", self.extension)
    }

    fn skills(&self) -> Result<(Vec<Found>, Vec<NotASkill>), String> {
        Ok((self.found.clone(), Vec::new()))
    }

    fn files(&self, skill: &Found) -> Result<(Vec<SourceFile>, Vec<LeftOut>), String> {
        self.files
            .get(&skill.name)
            .map(|files| (files.clone(), Vec::new()))
            .ok_or_else(|| {
                format!(
                    "the review did not read {}, so nothing was changed: review it again",
                    skill.name
                )
            })
    }

    fn grade(&self, skill: &Found) -> Option<Result<Vec<u8>, String>> {
        self.grades.get(&skill.name).cloned().map(Ok)
    }

    fn whose(&self) -> String {
        self.origin().label()
    }
}

/// **An extension's source, asked at one revision**: what it offers, and each skill's files the
/// first time they are wanted, kept, so one review asks for each skill once and what it read is
/// what is [`Held`].
struct Asked<'a> {
    studio: Studio<'a>,
    id: &'a str,
    revision: Revision,
    found: Vec<Found>,
    read: RefCell<BTreeMap<String, Vec<SourceFile>>>,
    grades: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl Asked<'_> {
    /// `id`'s source, asked what it offers now.
    fn of<'a>(studio: Studio<'a>, id: &'a str) -> Result<Asked<'a>, String> {
        let doc = studio.ask(id, &Ask::Skills)?;
        let offered = wire::offered_of(id, &doc)?;
        let revision = Revision::parse(&offered.revision)
            .ok_or_else(|| format!("'{id}' answered a revision purlis does not read"))?;
        Ok(Asked {
            studio,
            id,
            revision,
            found: offered
                .skills
                .iter()
                .map(|skill| Found {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    folder: skill.name.clone(),
                    dir: vec![skill.name.clone()],
                })
                .collect(),
            read: RefCell::new(BTreeMap::new()),
            grades: RefCell::new(BTreeMap::new()),
        })
    }

    /// What it read, held for the change the person accepts.
    fn held(self) -> Held {
        Held {
            extension: self.id.to_owned(),
            revision: self.revision,
            found: self.found,
            files: self.read.into_inner(),
            grades: self.grades.into_inner(),
        }
    }
}

impl SkillSource for Asked<'_> {
    fn shown(&self) -> String {
        format!("the skill source '{}'", self.id)
    }

    fn skills(&self) -> Result<(Vec<Found>, Vec<NotASkill>), String> {
        Ok((self.found.clone(), Vec::new()))
    }

    fn files(&self, skill: &Found) -> Result<(Vec<SourceFile>, Vec<LeftOut>), String> {
        if let Some(files) = self.read.borrow().get(&skill.name) {
            return Ok((files.clone(), Vec::new()));
        }
        let doc = self.studio.ask(
            self.id,
            &Ask::Files {
                skill: &skill.name,
                revision: self.revision.as_str(),
            },
        )?;
        let files = wire::files_of(self.id, self.revision.as_str(), &doc)?;
        if let Some(grade) = wire::grade_of(&doc) {
            self.grades.borrow_mut().insert(skill.name.clone(), grade);
        }
        let total: usize = files.iter().map(|file| file.text.len()).sum();
        if total as u64 > super::MOST_SKILL_BYTES {
            return Err(format!(
                "{} holds more than {} MiB, the most a skill purlis installs may",
                skill.name,
                super::MOST_SKILL_BYTES >> 20
            ));
        }
        let files: Vec<SourceFile> = files
            .into_iter()
            .map(|file| SourceFile {
                parts: file.path.split('/').map(str::to_owned).collect(),
                executable: file.executable,
                bytes: file.text.into_bytes(),
            })
            .collect();
        self.read
            .borrow_mut()
            .insert(skill.name.clone(), files.clone());
        Ok((files, Vec::new()))
    }

    fn grade(&self, skill: &Found) -> Option<Result<Vec<u8>, String>> {
        self.grades.borrow().get(&skill.name).cloned().map(Ok)
    }

    fn whose(&self) -> String {
        Origin::Extension(self.id.to_owned()).label()
    }
}

/// **The revision the extension `id`'s source names what it holds by now**: its listing alone,
/// no skill's files asked.
pub fn newest(studio: Studio<'_>, id: &str) -> Result<Revision, String> {
    Ok(Asked::of(studio, id)?.revision)
}

/// **What the extension `id`'s source offers now**, each skill with the levels it is installed at
/// from it in the project at `plane` and what its files do (#5760). The files read to mark it
/// inform the person alone: they are let go of here, and an install copies only what its own
/// review read ([`Held`]).
pub fn browse(studio: Studio<'_>, plane: &Path, id: &str) -> Result<Browse, String> {
    let asked = Asked::of(studio, id)?;
    let revision = asked.revision.clone();
    browse_of(&asked, id, revision, plane)
}

/// [`browse`], of what `source` answers as the extension `id` at `revision`: each skill's files
/// read for its marks, the first [`MOST_MARKED`] skills, until one answer would take what was read
/// past [`super::fetch::MOST_READ_BYTES`], as a git source's browse stops where one read would
/// take its fetch past what it may read. That skill and the rest are left to their reviews.
fn browse_of(
    source: &dyn SkillSource,
    id: &str,
    revision: Revision,
    plane: &Path,
) -> Result<Browse, String> {
    let (found, _) = source.skills()?;
    let (locked, _) = lock::every(plane);
    let origin = Origin::Extension(id.to_owned());
    let mut read: u64 = 0;
    let mut reading = true;
    let skills = found
        .into_iter()
        .enumerate()
        .map(|(at, skill)| {
            let marks = if reading && at < MOST_MARKED {
                // One skill's answer refused marks that one alone: the others are still read.
                source.files(&skill).ok().and_then(|(files, _)| {
                    read += files.iter().map(|f| f.bytes.len() as u64).sum::<u64>();
                    if read > super::fetch::MOST_READ_BYTES {
                        reading = false;
                        return None;
                    }
                    Some(review::summary_of(&files).marks())
                })
            } else {
                None
            };
            Offered {
                installed: installed_at(&locked, &origin, &skill.name),
                marks,
                skill,
            }
        })
        .collect();
    Ok(Browse {
        extension: id.to_owned(),
        skills,
        revision,
    })
}

/// **The review of installing `names` from the extension `id`'s source at `level`**, and every
/// skill they refer to that it holds and that is not met at `level` already: the same review the
/// git source's install shows, marked the same way. Answered with what it read, [`Held`] for the
/// install. Each skill of `without` that only a hint would bring along is left out (#5874).
pub fn review(
    studio: Studio<'_>,
    plane: &Path,
    id: &str,
    names: &[String],
    level: &Level,
    without: &[String],
) -> Result<(Review, Held), String> {
    let asked = Asked::of(studio, id)?;
    let review = review_in(&asked, id, &asked.revision, plane, names, level, without)?;
    Ok((review, asked.held()))
}

/// [`review`], of what `source` answers as the extension `id` at `revision`.
fn review_in(
    source: &dyn SkillSource,
    id: &str,
    revision: &Revision,
    plane: &Path,
    names: &[String],
    level: &Level,
    without: &[String],
) -> Result<Review, String> {
    let mut skills = review::reviews_of(
        source,
        names,
        &met_at(plane, level),
        review::Hints::Without(without),
    )?;
    let origin = Origin::Extension(id.to_owned());
    held_to_policy(plane, &origin, &skills, revision)?;
    review::mark(&mut skills, plane, level, &origin);
    Ok(Review {
        extension: id.to_owned(),
        revision: revision.clone(),
        skills,
    })
}

/// **The review of installing `names` from the extension `id`'s source together at `level`**
/// (#4024): one review and one install for the set, those named that are installed from it at
/// `level` already left out and said to be there (#4153), and each of `without` that only a hint
/// would bring along (#5874).
pub fn review_set(
    studio: Studio<'_>,
    plane: &Path,
    id: &str,
    names: &[String],
    level: &Level,
    without: &[String],
) -> Result<(SetReview, Held), String> {
    set_review_of(Asked::of(studio, id)?, id, plane, names, level, without)
}

/// **The review of installing every skill the extension `id`'s source offers but `skipping`**
/// at `level` (`purlis skill install --extension <id> --all`, #5760): [`review_set`] of what one
/// listing names, so the names and the review are of one revision and the extension is asked for
/// no file it was not reviewed for.
pub fn review_every(
    studio: Studio<'_>,
    plane: &Path,
    id: &str,
    skipping: &[String],
    level: &Level,
) -> Result<(SetReview, Held), String> {
    let asked = Asked::of(studio, id)?;
    let names: Vec<String> = asked
        .found
        .iter()
        .map(|one| one.name.clone())
        .filter(|name| !skipping.contains(name))
        .collect();
    set_review_of(asked, id, plane, &names, level, &[])
}

/// [`review_set`], of `asked`.
fn set_review_of(
    asked: Asked<'_>,
    id: &str,
    plane: &Path,
    names: &[String],
    level: &Level,
    without: &[String],
) -> Result<(SetReview, Held), String> {
    let origin = Origin::Extension(id.to_owned());
    let (locked, _) = lock::every(plane);
    let set = review::set_reviewed(
        &asked,
        &asked.found,
        names,
        &|name| {
            installed_at(&locked, &origin, name)
                .iter()
                .any(|at| at == level)
        },
        &met_at(plane, level),
        review::Hints::Without(without),
    )?;
    let (mut skills, there) = match set {
        Set::Reviewed { skills, there } => (skills, there),
        // An extension's source has no read limit past one skill's own, so a set always fits.
        Set::TooMuch(TooMuch { why, .. }) => return Err(why),
    };
    held_to_policy(plane, &origin, &skills, &asked.revision)?;
    review::mark(&mut skills, plane, level, &origin);
    let held = asked.held();
    Ok((
        SetReview {
            review: Review {
                extension: id.to_owned(),
                revision: held.revision.clone(),
                skills,
            },
            there,
        },
        held,
    ))
}

/// **Installs what `accepted` says from what a review of an extension's source read**, `held`, at
/// `level` (ADR 0100 §2, §8): the review read again from `held`, refused where any digest differs
/// from the one accepted, then the same copy, pin, grant and commit as a git source's install,
/// pinned to the revision. The extension is not asked again (#3089). Answers each skill's folder.
pub fn install(
    cx: &mut Ctx<'_>,
    held: &Held,
    level: &Level,
    accepted: &Accepted,
    grant: Option<&Grant>,
    renames: &Renames,
) -> Result<Vec<PathBuf>, String> {
    held.is_at(accepted)?;
    install_from(
        cx,
        held,
        &held.origin(),
        &held.pin(),
        level,
        accepted,
        grant,
        renames,
    )
}

/// The installed skill `name` at `level`, where an extension's source answered it: its entry
/// and that extension's id.
fn locked_from_an_extension(
    plane: &Path,
    level: &Level,
    name: &str,
) -> Result<(Locked, String), String> {
    let held = lock::read(plane, lock::Kept::of(level))?;
    let locked = held
        .lock
        .skills
        .into_iter()
        .find(|one| one.is(level, name))
        .ok_or_else(|| format!("no {level} skill named {name} was installed from a source"))?;
    match &locked.source {
        Origin::Extension(id) => {
            let id = id.clone();
            Ok((locked, id))
        }
        Origin::Git(_) => Err(format!("{name} was installed from a git source")),
    }
}

/// **The review of updating the installed skill `name` at `level`** to the revision its
/// extension's source names now: the same review a git source's update shows, its diff from the
/// copy here, as the extension is not asked for the pinned revision. Refused where the pin is the
/// newest. Answered with what it read, [`Held`] for the update.
pub fn update_review(
    studio: Studio<'_>,
    plane: &Path,
    level: &Level,
    name: &str,
) -> Result<(UpdateReview, Held), String> {
    let (locked, id) = locked_from_an_extension(plane, level, name)?;
    let asked = Asked::of(studio, &id)?;
    let newer = Pin::Revision(asked.revision.clone());
    if newer == locked.pin {
        return Err(format!(
            "{name} is pinned to its source's newest revision already"
        ));
    }
    let update = reviewed_update(plane, level, name, &locked, &asked, newer.clone(), None)?;
    // The org's controls, said in the review before it is accepted (SK-11).
    super::policy::hold(
        plane,
        &locked.source,
        std::iter::once(locked.at_source())
            .chain(update.brought.iter().map(|one| one.name.as_str())),
        &newer,
        || None,
    )?;
    Ok((update, asked.held()))
}

/// **Refused where the org's controls refuse any of `skills`** of the extension's source at
/// `revision` ([`super::policy`]), so its review says why before anything is accepted. A
/// revision cannot say when it was made.
fn held_to_policy(
    plane: &Path,
    origin: &Origin,
    skills: &[SkillReview],
    revision: &Revision,
) -> Result<(), String> {
    super::policy::hold(
        plane,
        origin,
        skills.iter().map(|one| one.name.as_str()),
        &Pin::Revision(revision.clone()),
        || None,
    )
}

/// **Accepts an update** of the installed skill `name` at `level` to what `accepted` says, from
/// what its review read, `held`: the same checks, writes and single commit as a git source's
/// update ([`super::update()`]), the pin moved to the revision. The extension is not asked again
/// (#3089).
pub fn update(
    cx: &mut Ctx<'_>,
    held: &Held,
    level: &Level,
    name: &str,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<PathBuf, String> {
    crate::settings::writable(cx.plane)?;
    let planned = planned(cx, held, level, name, accepted, grant)?;
    write_update(cx, level, name, grant, &planned)
}

/// **Checks an update** of the installed skill `name` at `level` from what its review read,
/// `held`, writing nothing: refused where `held` is not at what was accepted or is another
/// extension's, then the git source's own checks ([`plan_from`]). The extension is not asked.
/// Several are written together by [`super::update::set::update_set`] (#5760).
pub(super) fn planned(
    cx: &Ctx<'_>,
    held: &Held,
    level: &Level,
    name: &str,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<Planned, String> {
    held.is_at(accepted)?;
    let (_, id) = locked_from_an_extension(cx.plane, level, name)?;
    if id != held.extension {
        return Err(format!(
            "{name} came from the extension '{id}', not the one reviewed, so nothing was updated"
        ));
    }
    plan_from(cx, level, name, accepted, grant, held, held.pin(), None)
}

/// A skill a test's [`Held`] holds: its name, its description, and each file's path and text.
#[cfg(any(test, feature = "test-support"))]
pub type HeldSkill<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

#[cfg(any(test, feature = "test-support"))]
impl Held {
    /// The review a review of the extension would answer, read from what it holds.
    pub fn review(&self, plane: &Path, names: &[String], level: &Level) -> Review {
        review_in(
            self,
            &self.extension,
            &self.revision,
            plane,
            names,
            level,
            &[],
        )
        .expect("reviewed")
    }

    /// What browsing the extension would answer, read from what it holds.
    pub fn browse(&self, plane: &Path) -> Browse {
        browse_of(self, &self.extension, self.revision.clone(), plane).expect("browsed")
    }

    /// The review of updating `name` at `level` to what it holds.
    pub fn update_review(
        &self,
        plane: &Path,
        level: &Level,
        name: &str,
    ) -> Result<UpdateReview, String> {
        let (locked, _) = locked_from_an_extension(plane, level, name)?;
        reviewed_update(plane, level, name, &locked, self, self.pin(), None)
    }

    /// What a review of the extension `id` would hold, its files given here.
    pub fn of(id: &str, revision: &str, skills: &[HeldSkill<'_>]) -> Held {
        Held {
            extension: id.to_owned(),
            revision: Revision::parse(revision).expect("a revision"),
            found: skills
                .iter()
                .map(|(name, description, _)| Found {
                    name: (*name).to_owned(),
                    description: (*description).to_owned(),
                    folder: (*name).to_owned(),
                    dir: vec![(*name).to_owned()],
                })
                .collect(),
            // A skill given no files is one the review did not read.
            files: skills
                .iter()
                .filter(|(_, _, files)| !files.is_empty())
                .map(|(name, _, files)| {
                    (
                        (*name).to_owned(),
                        files
                            .iter()
                            .map(|(path, text)| SourceFile {
                                parts: path.split('/').map(str::to_owned).collect(),
                                executable: false,
                                bytes: text.as_bytes().to_vec(),
                            })
                            .collect(),
                    )
                })
                .collect(),
            grades: BTreeMap::new(),
        }
    }

    /// It, its files answer for `name` having handed `grade` beside them.
    pub fn with_grade(mut self, name: &str, grade: Vec<u8>) -> Held {
        self.grades.insert(name.to_owned(), grade);
        self
    }
}

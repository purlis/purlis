// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The install review** (ADR 0100 §3, decision 39), and Browse.
//!
//! The review leads with each skill's summary ([`purlis_skills::review`]): the scripts it carries,
//! the network hosts its files name, its `!` lines, the writes it would make outside the
//! workspace, every character no reader sees and every file that is not text, the risky ones
//! marked. Then every file, one click away, its text shown with every invisible character
//! escaped. The skills a skill refers to come along in the same review, each after the one that
//! brought it.

use std::path::Path;

use purlis_skills::review as summary;
use purlis_skills::tree::LeftOut;
use serde::{Deserialize, Serialize};

use super::fetch::{Fetched, Found, NotASkill, SourceFile, fetch};
use super::{Commit, Origin, Pin, Source, lock};
use crate::skills::verbs::{Level, Whose};
use crate::skills::{Reference, name_reserved};

/// One file of a skill, as the review shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewFile {
    pub path: String,
    pub size: u64,
    pub executable: bool,
    /// Its text with every invisible character escaped ([`summary::escaped`]), `None` for a file
    /// that is not text, which the review lists as not shown.
    pub text: Option<String>,
    /// Whether the summary marks something in it.
    pub risky: bool,
    /// **Its text as the source holds it**, unescaped, for the studio's lints alone ([`linted`]);
    /// never shown, so never serialized.
    #[serde(skip)]
    pub(super) held_text: Option<String>,
}

/// **A name that is taken where a skill would land** (ADR 0100, amended 2026-11-01): the review
/// stops until the person skips it or gives it a new name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clash {
    /// What holds the name, as the end of a sentence.
    pub held_by: String,
    /// The new name offered: `<source>-<name>`.
    pub offered: String,
    /// The names a new name cannot take: the skills at that level, and the built-in skills'.
    pub taken: Vec<String>,
}

/// **One skill in an install review**: what it is, what it does beyond its instructions, every
/// file it installs, and every entry of its folder that is not installed and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillReview {
    pub name: String,
    pub description: String,
    /// Its folder in the source.
    pub folder: String,
    /// The skill whose install brings it along, as one it refers to.
    pub brought_by: Option<String>,
    /// Whether [`Self::brought_by`] only names it bare, as a hint (#4510), not as `skills:<name>`.
    pub brought_by_hint: bool,
    /// The skills it refers to that the level it lands in holds already (#4032): they are not
    /// brought along, and the install leaves them as they are.
    pub already: Vec<String>,
    /// Every skill it refers to, each once, wherever it is met.
    pub refers: Vec<String>,
    /// Every skill it names only bare, as a hint (#4510), each once: one the source holds or the
    /// level has ([`hints_of`]), so never one that is not there. Such a skill comes along, or is
    /// left as it is, as one it refers to does.
    pub hints: Vec<String>,
    /// Where its name is taken at the level it lands in.
    pub clash: Option<Clash>,
    /// A line each for what the review warns of and does not stop on: a copy at another level, and
    /// a skill it refers to that is another source's here.
    pub notes: Vec<String>,
    pub summary: summary::Summary,
    pub files: Vec<ReviewFile>,
    pub left_out: Vec<LeftOut>,
    /// What the lint says of its instructions, and each skill it refers to that the source does
    /// not hold.
    pub lint: Vec<String>,
    /// The digest of what it installs, which the install is handed back.
    pub digest: String,
    /// **The grade the source carries beside its folder** (ADR 0100, amended 2026-11-08): the
    /// publisher's, never purlis's own. `None` where it carries none.
    pub grade: Option<super::SourceGrade>,
}

/// **An install review**: the source, the commit it was read at, and each skill, those it refers
/// to after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Review {
    pub source: Source,
    pub commit: Commit,
    pub skills: Vec<SkillReview>,
}

/// One skill of what was accepted: its name and the digest the review showed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AcceptedSkill {
    pub name: String,
    pub digest: String,
}

/// **What an install or an update was accepted as**: the commit or revision and each skill's
/// digest, as the review showed them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub pin: Pin,
    pub skills: Vec<AcceptedSkill>,
}

impl Accepted {
    /// What accepting `skills` at `pin` says.
    pub fn of(pin: &Pin, skills: &[SkillReview]) -> Accepted {
        Accepted {
            pin: pin.clone(),
            skills: skills
                .iter()
                .map(|skill| AcceptedSkill {
                    name: skill.name.clone(),
                    digest: skill.digest.clone(),
                })
                .collect(),
        }
    }

    /// Whether `now` is what was accepted, whatever the order.
    pub(super) fn is(&self, now: &Accepted) -> bool {
        let mut a = self.skills.clone();
        let mut b = now.skills.clone();
        a.sort();
        b.sort();
        self.pin == now.pin && a == b
    }

    /// The commit accepted, where it was a git source's; refused where it was an extension's
    /// revision.
    pub(super) fn commit(&self) -> Result<&Commit, String> {
        self.pin
            .commit()
            .ok_or_else(|| "what was reviewed was not a git source's commit".to_owned())
    }
}

/// **Marks each of `skills`, from `origin`, with where it lands at `level`**: its clash, and its
/// one-line notes. The same marks for every source, the git one and each extension's.
pub(super) fn mark(skills: &mut [SkillReview], plane: &Path, level: &Level, origin: &Origin) {
    let (locked, _) = lock::every(plane);
    let label = origin.label();
    let levels = Level::every(plane);
    let held = names_at(plane, level);
    for skill in skills {
        let name = skill.name.clone();
        skill.clash = held_by(plane, &locked, level, &name).map(|held_by| Clash {
            held_by,
            offered: format!("{label}-{name}"),
            taken: held.clone(),
        });
        let others: Vec<String> = levels
            .iter()
            .filter(|other| *other != level && has(plane, other, &name))
            .map(|other| other.to_string())
            .collect();
        if !others.is_empty() {
            skill.notes.push(format!(
                "The {} skills have a {name} too: where a chat gets both, the narrowest copy \
                 wins.",
                others.join(", ")
            ));
        }
        for other in &skill.refers {
            let theirs = locked.iter().find(|(_, one)| {
                one.name == *other
                    && one.source != *origin
                    && (one.level.0.at == level.at || one.level.0.at == Whose::Project)
            });
            if let Some((_, one)) = theirs {
                skill.notes.push(format!(
                    "It refers to {other}, which is {}'s here.",
                    one.source.label()
                ));
            }
        }
    }
}

impl Review {
    /// **Marks each skill with where it lands at `level`** ([`mark`]).
    fn marked(mut self, plane: &Path, level: &Level) -> Review {
        mark(
            &mut self.skills,
            plane,
            level,
            &Origin::Git(self.source.remote.clone()),
        );
        self
    }

    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        Accepted::of(&Pin::Commit(self.commit.clone()), &self.skills)
    }
}

/// `sha256:` and the hex digest of `bytes`.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!(
        "sha256:{}",
        crate::extension::hex(&sha2::Sha256::digest(bytes))
    )
}

/// **The digest of a copy** whose files are `files`: over each file's path, mode and digest, in
/// path order, so the same bytes at the same paths are the same digest wherever they lie.
pub(crate) fn digest_of(files: &[lock::Wrote]) -> String {
    let mut sorted: Vec<&lock::Wrote> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut all = String::new();
    for file in sorted {
        all.push_str(&format!(
            "{}\0{}\0{}\n",
            file.path,
            if file.executable { "755" } else { "644" },
            file.sha256
        ));
    }
    sha256(all.as_bytes())
}

/// What a lockfile records of `files`.
pub(super) fn wrote(files: &[SourceFile]) -> Vec<lock::Wrote> {
    files
        .iter()
        .map(|file| lock::Wrote {
            path: file.path(),
            executable: file.executable,
            sha256: sha256(&file.bytes),
        })
        .collect()
}

/// **The digest the record keeps of the file at `path` of a copy under a new name**: a `SKILL.md`'s
/// without its name line ([`purlis_skills::change::without_name`]).
pub(super) fn sha_renamed(path: &str, bytes: &[u8]) -> String {
    let unnamed = (path == purlis_skills::FILE)
        .then(|| std::str::from_utf8(bytes).ok())
        .flatten()
        .and_then(purlis_skills::change::without_name);
    sha256(unnamed.as_deref().map_or(bytes, str::as_bytes))
}

/// What a lockfile records of `files`, written under a new name.
pub(super) fn wrote_renamed(files: &[SourceFile]) -> Vec<lock::Wrote> {
    files
        .iter()
        .map(|file| lock::Wrote {
            path: file.path(),
            executable: file.executable,
            sha256: sha_renamed(&file.path(), &file.bytes),
        })
        .collect()
}

/// The summary of `files`.
pub(super) fn summary_of(files: &[SourceFile]) -> summary::Summary {
    let paths: Vec<String> = files.iter().map(SourceFile::path).collect();
    let read: Vec<summary::FileIn<'_>> = files
        .iter()
        .zip(&paths)
        .map(|(file, path)| summary::FileIn {
            path,
            executable: file.executable,
            bytes: &file.bytes,
        })
        .collect();
    summary::summarize(&read)
}

/// The text of `file`, `None` where the summary reads it as not text.
fn text_of(file: &SourceFile) -> Option<&str> {
    summary::FileIn {
        path: "",
        executable: file.executable,
        bytes: &file.bytes,
    }
    .text()
}

/// The skills `files` refer to (`skills:<name>`, ADR 0099), by name, each once.
pub(super) fn referenced(files: &[SourceFile]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for text in files.iter().filter_map(text_of) {
        for name in purlis_skills::change::referenced(text, crate::skills::LINKED_PLUGIN) {
            if purlis_skills::change::name_problem(&name).is_none() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// **Every skill `own`'s files name only bare, as hints** (#4510,
/// [`purlis_skills::uses::hints_in`]): each a skill `known` says is there, once each, and not one
/// of `refers`, those they refer to.
fn hints_of(
    files: &[SourceFile],
    own: &str,
    refers: &[String],
    known: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for text in files.iter().filter_map(text_of) {
        for hint in purlis_skills::uses::hints_in(text, own, known) {
            if !refers.contains(&hint.name) && !out.contains(&hint.name) {
                out.push(hint.name);
            }
        }
    }
    out
}

/// The review of the skill `found`, whose files are `files` and whose left-out entries are
/// `left`, brought along by `brought_by` (only by a hint where `by_hint`), in a source that holds
/// the skills `held`, landing in a level that holds those `have` says.
pub(super) fn review_of(
    found: &Found,
    files: &[SourceFile],
    left: Vec<LeftOut>,
    brought_by: Option<String>,
    by_hint: bool,
    held: &[String],
    have: &dyn Fn(&str) -> bool,
) -> SkillReview {
    let summary = summary_of(files);
    let marked = |path: &str| {
        summary
            .scripts
            .iter()
            .any(|s| s.path == path && !s.risks.is_empty())
            || summary.commands.iter().any(|c| c.path == path)
            || summary.outside.iter().any(|o| o.path == path)
            || summary.unseen.iter().any(|u| u.path == path)
            || summary
                .hosts
                .iter()
                .any(|h| h.risky && h.files.iter().any(|f| f == path))
    };
    let mut lint: Vec<String> = files
        .iter()
        .find(|file| file.parts == [purlis_skills::FILE])
        .and_then(text_of)
        .map(|text| {
            purlis_skills::change::check(text, &found.name)
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut already = Vec::new();
    let refers: Vec<String> = referenced(files)
        .into_iter()
        .filter(|name| *name != found.name)
        .collect();
    for name in refers.iter().cloned() {
        if have(&name) {
            already.push(name);
        } else if !held.contains(&name) {
            lint.push(format!(
                "it refers to the skill {name}, which this source does not hold"
            ));
        }
    }
    let hints = hints_of(files, &found.name, &refers, &|name| {
        held.iter().any(|one| one == name) || have(name)
    });
    already.extend(hints.iter().filter(|name| have(name)).cloned());
    SkillReview {
        name: found.name.clone(),
        description: found.description.clone(),
        folder: found.folder.clone(),
        brought_by,
        brought_by_hint: by_hint,
        already,
        refers,
        hints,
        clash: None,
        notes: Vec::new(),
        files: files
            .iter()
            .map(|file| ReviewFile {
                path: file.path(),
                size: file.bytes.len() as u64,
                executable: file.executable,
                risky: marked(&file.path()) || file.executable || text_of(file).is_none(),
                text: text_of(file).map(summary::escaped),
                held_text: text_of(file).map(str::to_owned),
            })
            .collect(),
        left_out: left,
        lint,
        digest: digest_of(&wrote(files)),
        summary,
        grade: None,
    }
}

/// **The studio's lints of each of `skills`, said in its lint lines** (#3090, ADR 0101 §1): each
/// lint is handed the skill's text files as the source holds them, landing at `level`, and each
/// finding is a line after purlis's own, under the lint's name. An extension's lint only warns,
/// as in the live lint, so a review never stops on it; the digest the install is handed back is
/// not touched. purlis's own lints are already said ([`review_of`]), so `lints` are only the
/// extensions' ([`crate::skills::studio::Studio::lints`]).
pub fn linted<'a>(
    skills: impl IntoIterator<Item = &'a mut SkillReview>,
    level: &Level,
    lints: &[&dyn purlis_skills::SkillLint],
) {
    if lints.is_empty() {
        return;
    }
    for skill in skills {
        let Some(text) = skill
            .files
            .iter()
            .find(|file| file.path == purlis_skills::FILE)
            .and_then(|file| file.held_text.clone())
        else {
            continue;
        };
        let mut others: Vec<(String, String)> = Vec::new();
        let mut listed: Vec<String> = Vec::new();
        for file in skill
            .files
            .iter()
            .filter(|file| file.path != purlis_skills::FILE)
        {
            listed.push(file.path.clone());
            if let Some(text) = &file.held_text {
                others.push((file.path.clone(), text.clone()));
            }
        }
        let files = purlis_skills::SkillFiles {
            name: &skill.name,
            text: &text,
            others: &others,
            origin: level.origin(),
            plugin: None,
            listed: Some(&listed),
        };
        for lint in lints {
            skill
                .lint
                .extend(lint.findings(&files).iter().map(ToString::to_string));
        }
    }
}

/// **More than one read of a source may take** (#4024): a review of several skills stops before
/// the skill whose files would take it past the limit, and says which of those named fit, so the
/// person installs them in parts. Nothing is ever cut from a review silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TooMuch {
    /// Those named first whose reviews, with every skill each brings along, fit one read: the
    /// first part to install. Empty where the first named alone does not.
    pub fits: Vec<String>,
    /// The rest of those named, for a part after it.
    pub rest: Vec<String>,
    /// What the read's limit says.
    pub why: String,
    /// Those named that the level holds already, left out of every part (#4153), as
    /// [`SetReview::Review`]'s `there`.
    pub there: Vec<String>,
}

/// **The review of a set of skills** (#4024): the review, or, where reading it is more than one
/// read may take, which of those named fit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SetReview {
    Review {
        review: Review,
        /// Those named that are installed from this source at the level they land in already,
        /// under either name (#4153): left out of the review and the install, and left as they
        /// are. A name held any other way is a [`Clash`].
        there: Vec<String>,
    },
    TooMuch(TooMuch),
}

/// Why a review was not read.
pub(super) enum Unread {
    /// More than one read may take.
    TooMuch(TooMuch),
    /// Anything else, in a sentence.
    Refused(String),
}

impl From<String> for Unread {
    fn from(why: String) -> Unread {
        Unread::Refused(why)
    }
}

impl Unread {
    /// What it says in a sentence.
    pub(super) fn said(self) -> String {
        match self {
            Unread::TooMuch(too_much) => too_much.why,
            Unread::Refused(why) => why,
        }
    }
}

/// **One skill reviewed**, and the files its review read: an install copies those, and never
/// reads them a second time, which would count twice against the read's limit.
pub(super) struct Reviewed {
    pub review: SkillReview,
    pub files: Vec<SourceFile>,
}

/// **Which skills a hint brings along** (#5874): a skill named only bare comes along as one it
/// refers to does, unless the person left it out of the review, and an install brings along by a
/// hint only what was accepted, so a hint left out of the review is left out of the install too.
/// A skill it refers to always comes along.
#[derive(Debug, Clone, Copy)]
pub enum Hints<'a> {
    /// Every skill a hint names comes along.
    Every,
    /// Every one but these, which the person left out.
    Without(&'a [String]),
    /// Only these, which were accepted.
    Only(&'a [String]),
}

impl Hints<'_> {
    /// Whether a hint at `name` brings it along.
    fn bring(self, name: &str) -> bool {
        match self {
            Hints::Every => true,
            Hints::Without(left) => !left.iter().any(|one| one == name),
            Hints::Only(kept) => kept.iter().any(|one| one == name),
        }
    }
}

/// **The review of `names` from `source`**, and every skill they refer to that the source holds
/// and `have` does not, and every one they hint at that `hints` brings, each right after the one that brought it, so one that was also named is
/// still marked as brought along and an install's second review reads the same. One review for
/// every source, the git one and each extension's (ADR 0101 §1): what is reviewed is what the
/// source answered, and the review is purlis's.
///
/// Before each skill's files are read it asks the source whether they fit what one read may take
/// ([`super::SkillSource::fits_one_read`]); where they do not, it answers [`TooMuch`] with the
/// names whose reviews were whole before it.
pub(super) fn reviewed(
    source: &dyn super::SkillSource,
    names: &[String],
    have: &dyn Fn(&str) -> bool,
    hints: Hints<'_>,
) -> Result<Vec<Reviewed>, Unread> {
    let (found, _) = source.skills()?;
    reviewed_from(source, &found, names, have, hints)
}

/// [`reviewed`], with what the source offers already listed as `found`: listing it again would
/// read every skill's instructions a second time against the read's limit.
pub(super) fn reviewed_from(
    source: &dyn super::SkillSource,
    found: &[Found],
    names: &[String],
    have: &dyn Fn(&str) -> bool,
    hints: Hints<'_>,
) -> Result<Vec<Reviewed>, Unread> {
    let held: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
    // Each to review, the skill that brings it along, and whether only by a hint.
    let mut queue: Vec<(String, Option<String>, bool)> = names
        .iter()
        .map(|name| (name.clone(), None, false))
        .collect();
    let mut skills: Vec<Reviewed> = Vec::new();
    // The named whose reviews are whole, the named one being reviewed now, and how many skills
    // the whole ones reviewed with what they brought along: a skill brought along is reviewed
    // before the next named one is taken.
    let mut whole: Vec<String> = Vec::new();
    let mut now: Option<String> = None;
    let mut wholly = 0;
    while !queue.is_empty() {
        let (name, by, by_hint) = queue.remove(0);
        if by.is_none() {
            if let Some(done) = now.replace(name.clone())
                && !whole.contains(&done)
            {
                whole.push(done);
            }
            wholly = skills.len();
        }
        if skills.iter().any(|s| s.review.name == name) {
            continue;
        }
        let Some(one) = found.iter().find(|f| f.name == name) else {
            return Err(format!("{} holds no skill named {name:?}", source.shown()).into());
        };
        if let Err(why) = source.fits_one_read(one) {
            // A named one the first part brings along is in it, and not left for the rest.
            let first: Vec<&str> = skills[..wholly]
                .iter()
                .map(|one| one.review.name.as_str())
                .collect();
            return Err(Unread::TooMuch(TooMuch {
                rest: names
                    .iter()
                    .filter(|named| !whole.contains(named) && !first.contains(&named.as_str()))
                    .cloned()
                    .collect(),
                fits: whole,
                why,
                there: Vec::new(),
            }));
        }
        let (files, left) = source.files(one)?;
        // One that is named is reviewed itself, so it is never said to be left as it is.
        let left_as_is = |other: &str| have(other) && !names.iter().any(|named| named == other);
        let mut review = review_of(one, &files, left, by, by_hint, &held, &left_as_is);
        // What it refers to, then what it only hints at and the person did not leave out, each
        // the source holds and the level does not have, comes along right after it.
        let comes = |other: &&String, hint: bool| {
            held.contains(*other) && !have(other) && (!hint || hints.bring(other))
        };
        let mut next: Vec<(String, Option<String>, bool)> = review
            .refers
            .iter()
            .map(|other| (other, false))
            .chain(review.hints.iter().map(|other| (other, true)))
            .filter(|(other, hint)| comes(other, *hint))
            .map(|(other, hint)| (other.clone(), Some(name.clone()), hint))
            .collect();
        next.append(&mut queue);
        queue = next;
        review.grade = graded(source, one, &files);
        skills.push(Reviewed { review, files });
    }
    Ok(skills)
}

/// **The grade `source` carries for `skill`**, whose files are `files`, as the review shows it.
fn graded(
    source: &dyn super::SkillSource,
    skill: &Found,
    files: &[SourceFile],
) -> Option<super::SourceGrade> {
    let whose = source.whose();
    Some(match source.grade(skill)? {
        Ok(bytes) => {
            let tree = source.folder_tree(skill, files);
            super::carried::shown(&bytes, &skill.name, &whose, &tree)
        }
        Err(why) => super::SourceGrade::LeftOut { whose, why },
    })
}

/// **The review of `names` from `source`** ([`reviewed`]), what more than one read may take said
/// in its sentence.
pub(super) fn reviews_of(
    source: &dyn super::SkillSource,
    names: &[String],
    have: &dyn Fn(&str) -> bool,
    hints: Hints<'_>,
) -> Result<Vec<SkillReview>, String> {
    reviewed(source, names, have, hints)
        .map(|skills| skills.into_iter().map(|one| one.review).collect())
        .map_err(Unread::said)
}

impl Fetched {
    /// **The review of `names`** from this commit ([`reviews_of`]).
    pub(super) fn review_with(
        &self,
        names: &[String],
        have: &dyn Fn(&str) -> bool,
        hints: Hints<'_>,
    ) -> Result<Review, String> {
        Ok(Review {
            source: self.source.clone(),
            commit: self.commit.clone(),
            skills: reviews_of(self, names, have, hints)?,
        })
    }

    /// **The review of installing `names` together** from this commit, or which of them fit one
    /// read where all of them do not; `found` is what it offers, listed once, `holds` says which
    /// names are installed from this source where they land already, and `have` which references
    /// are met there already. One installed already is left out and said to be there (#4153).
    fn set_review(
        &self,
        found: &[Found],
        names: &[String],
        holds: &dyn Fn(&str) -> bool,
        have: &dyn Fn(&str) -> bool,
        hints: Hints<'_>,
    ) -> Result<SetReview, String> {
        Ok(
            match set_reviewed(self, found, names, holds, have, hints)? {
                Set::Reviewed { skills, there } => SetReview::Review {
                    review: Review {
                        source: self.source.clone(),
                        commit: self.commit.clone(),
                        skills,
                    },
                    there,
                },
                Set::TooMuch(too_much) => SetReview::TooMuch(too_much),
            },
        )
    }
}

/// A set reviewed from any source ([`set_reviewed`]): its skills' reviews and those named that
/// are there already, or which of them fit one read.
pub(super) enum Set {
    Reviewed {
        skills: Vec<SkillReview>,
        there: Vec<String>,
    },
    TooMuch(TooMuch),
}

/// **The review of installing `names` together from `source`** (#4024), `found` being what it
/// offers: those `holds` says are installed from it where they land already are left out and said
/// to be there (#4153), and `have` says which references are met there already.
pub(super) fn set_reviewed(
    source: &dyn super::SkillSource,
    found: &[Found],
    names: &[String],
    holds: &dyn Fn(&str) -> bool,
    have: &dyn Fn(&str) -> bool,
    hints: Hints<'_>,
) -> Result<Set, String> {
    let (there, names): (Vec<String>, Vec<String>) =
        names.iter().cloned().partition(|name| holds(name));
    match reviewed_from(source, found, &names, have, hints) {
        Ok(skills) => Ok(Set::Reviewed {
            skills: skills.into_iter().map(|one| one.review).collect(),
            there,
        }),
        Err(Unread::TooMuch(too_much)) => Ok(Set::TooMuch(TooMuch { there, ..too_much })),
        Err(Unread::Refused(why)) => Err(why),
    }
}

/// **What holds `name` where a skill would land at `level`**, as the end of a sentence: purlis
/// ([`name_reserved`]), or a skill at `level` from another source or made there.
fn held_by(
    plane: &Path,
    locked: &[(lock::Kept, lock::Locked)],
    level: &Level,
    name: &str,
) -> Option<String> {
    if name_reserved(name) {
        return Some(format!("the name {name} belongs to purlis"));
    }
    if !has(plane, level, name) {
        return None;
    }
    Some(match locked.iter().find(|(_, one)| one.is(level, name)) {
        Some((_, one)) => format!(
            "the {level} skills have a {name} from {}",
            one.source.label()
        ),
        None => format!("the {level} skills have a {name} made here"),
    })
}

/// **The name a deleted skill's Bring back waits for** (#5457): where its name belongs to purlis
/// now, what holds it, the new name offered as an install offers one (`<source>-<name>`, the
/// source being what its group was deleted from, spelled as a name; `my-<name>` where nothing is
/// said of it), and the names at its level a new name cannot take. `None` where its name is free.
pub fn deleted_clash(plane: &Path, deleted: &crate::skills::verbs::Deleted) -> Option<Clash> {
    let name = &deleted.name;
    if !name_reserved(name) {
        return None;
    }
    let from = deleted.set.as_ref().and_then(|set| set.from.as_deref());
    let offered = match from {
        Some(from) => format!("{}-{name}", super::spelled(from)),
        None => format!("my-{}", name.strip_prefix("purlis-").unwrap_or(name)),
    };
    Some(Clash {
        held_by: format!("the name {name} belongs to purlis"),
        offered,
        taken: names_at(plane, &deleted.level),
    })
}

/// The names of the folders at `level`: what a new name there cannot be.
fn names_at(plane: &Path, level: &Level) -> Vec<String> {
    let mut names: Vec<String> = level
        .rel()
        .and_then(|rel| std::fs::read_dir(plane.join(rel)).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

/// Whether `level` holds a skill named `name`, from a source or made there.
pub(super) fn has(plane: &Path, level: &Level, name: &str) -> bool {
    level.named(plane, name).is_some()
}

/// **Whether a skill installed at `level` that refers to `name` has it met already**, so the
/// source's `name` is not brought along, as the review and the install agree (ADR 0100 as amended
/// 2026-10-31): where every chat that gets the installed skill gets a skill named `name` without
/// it, by the rules [`purlis_skills::skill_set`] resolves a chat's skills by
/// ([`purlis_skills::reaches_workspace`], [`Reference::names_plane`]).
///
/// - `name` is purlis's own, which no plane skill may take (#4155);
/// - `level` holds a skill of that name, from any source or made there (#4032);
/// - at a workspace's level, a project skill of that name is granted to the workspace and not
///   turned off there (#4154);
/// - at a persona's level, the persona's `skills:` list names a project skill of that name, which
///   its chats get in any workspace (#4154).
///
/// A shared install is met by shared skills alone, as its copy reaches every clone and another
/// person's local skills are not there; a local install, which runs on this machine alone, by its
/// local skills and the shared ones. `wider` is what the wider levels say ([`Wider`]).
pub(super) fn met(plane: &Path, wider: &Wider, level: &Level, name: &str) -> bool {
    let sides: &[bool] = if level.local {
        &[true, false]
    } else {
        &[false]
    };
    let project_skill = |reaches: &dyn Fn(&Path) -> bool| {
        sides.iter().any(|&local| {
            crate::skills::named_at(plane, &Whose::Project, local, name)
                .is_some_and(|folder| reaches(&folder))
        })
    };
    name_reserved(name)
        || sides
            .iter()
            .any(|&local| has(plane, &level.clone().local(local), name))
        || match &level.at {
            Whose::Project => false,
            Whose::Workspace(ws) => project_skill(&|folder| {
                purlis_skills::reaches_workspace(folder, ws, &wider.turned_off, name)
            }),
            Whose::Persona(_) => {
                wider.persona_names.iter().any(|one| one.names_plane(name))
                    && project_skill(&|_| true)
            }
        }
}

/// What the levels wider than one a skill is installed at say of what its chats get, read once
/// for a review: the project skills a workspace turned off, and what a persona's `skills:` list
/// names.
pub(super) struct Wider {
    turned_off: Vec<String>,
    persona_names: Vec<Reference>,
}

impl Wider {
    /// What they say for `level` in the plane at `plane`.
    fn of(plane: &Path, level: &Level) -> Wider {
        Wider {
            turned_off: match &level.at {
                Whose::Workspace(ws) => crate::skills::turned_off(plane, ws),
                _ => Vec::new(),
            },
            persona_names: match &level.at {
                Whose::Persona(persona) => crate::skills::names_of(plane, persona),
                _ => Vec::new(),
            },
        }
    }
}

/// [`met`] for one review: the wider levels read once, and each name looked up once, however many skills refer to it.
pub(super) fn met_at<'a>(plane: &'a Path, level: &'a Level) -> impl Fn(&str) -> bool + 'a {
    let wider = Wider::of(plane, level);
    let seen = std::cell::RefCell::new(std::collections::HashMap::<String, bool>::new());
    move |name| {
        if let Some(known) = seen.borrow().get(name) {
            return *known;
        }
        let held = met(plane, &wider, level, name);
        seen.borrow_mut().insert(name.to_owned(), held);
        held
    }
}

/// **The review of installing `names` from `source` at `level`**, at its newest commit: every skill they refer to that the source holds and is not met
/// at `level` already ([`met`]) comes along.
pub fn review(
    plane: &Path,
    source: &Source,
    names: &[String],
    level: &Level,
) -> Result<Review, String> {
    review_without(plane, source, names, level, &[])
}

/// [`review`], with each skill of `without` that only a hint would bring along left out
/// (#5874): the person unticked it, and a skill one of them refers to still comes along.
pub fn review_without(
    plane: &Path,
    source: &Source,
    names: &[String],
    level: &Level,
    without: &[String],
) -> Result<Review, String> {
    let fetched = fetch(plane, source, None)?;
    let review = fetched
        .review_with(names, &met_at(plane, level), Hints::Without(without))?
        .marked(plane, level);
    held_to_policy(plane, &fetched, &review)?;
    Ok(review)
}

/// **Refused where the org's controls refuse any skill of `review`** of what `fetched` holds
/// ([`super::policy`]): so the review says why before anything is accepted.
fn held_to_policy(plane: &Path, fetched: &Fetched, review: &Review) -> Result<(), String> {
    super::policy::hold(
        plane,
        &Origin::Git(fetched.source.remote.clone()),
        review.skills.iter().map(|one| one.name.as_str()),
        &Pin::Commit(fetched.commit.clone()),
        || super::SkillSource::made(fetched),
    )
}

/// [`held_to_policy`] of a set's review, where it is one.
fn set_held_to_policy(plane: &Path, fetched: &Fetched, set: &SetReview) -> Result<(), String> {
    match set {
        SetReview::Review { review, .. } => held_to_policy(plane, fetched, review),
        _ => Ok(()),
    }
}

/// **The review of installing `names` from `source` together at `level`** (#4024), at its newest
/// commit: one review and one install for the set, or, past what one read may take, the names
/// that fit. One installed from this source at `level` already is left out, and said to be there
/// (#4153): the window's list is ticked before the level is chosen.
pub fn review_set(
    plane: &Path,
    source: &Source,
    names: &[String],
    level: &Level,
) -> Result<SetReview, String> {
    review_set_without(plane, source, names, level, &[])
}

/// [`review_set`], with each skill of `without` that only a hint would bring along left out
/// (#5874), as [`review_without`] leaves it.
pub fn review_set_without(
    plane: &Path,
    source: &Source,
    names: &[String],
    level: &Level,
    without: &[String],
) -> Result<SetReview, String> {
    let fetched = fetch(plane, source, None)?;
    let (found, _) = fetched.skills()?;
    let installed = lock::every(plane).0;
    let set = fetched
        .set_review(
            &found,
            names,
            &|name| {
                installed_at(&installed, &Origin::Git(source.remote.clone()), name)
                    .iter()
                    .any(|at| at == level)
            },
            &met_at(plane, level),
            Hints::Without(without),
        )?
        .marked(plane, level);
    set_held_to_policy(plane, &fetched, &set)?;
    Ok(set)
}

/// **The review of installing every skill of `source` not installed from it yet** (#4024,
/// `purlis skill install <source> --all`), at its newest commit, landing at `level`. A review of no
/// skill where every one is.
pub fn review_all(plane: &Path, source: &Source, level: &Level) -> Result<SetReview, String> {
    let origin = Origin::Git(source.remote.clone());
    let fetched = fetch(plane, source, None)?;
    let (found, _) = fetched.skills()?;
    let (locked, _) = lock::every(plane);
    let names: Vec<String> = found
        .iter()
        .filter(|skill| installed_at(&locked, &origin, &skill.name).is_empty())
        .map(|skill| skill.name.clone())
        .collect();
    // Those it names are installed nowhere from this source, so none is there already.
    let set = fetched
        .set_review(
            &found,
            &names,
            &|_| false,
            &met_at(plane, level),
            Hints::Every,
        )?
        .marked(plane, level);
    set_held_to_policy(plane, &fetched, &set)?;
    Ok(set)
}

impl SetReview {
    /// [`Review::marked`], where it is a review.
    fn marked(self, plane: &Path, level: &Level) -> SetReview {
        match self {
            SetReview::Review { review, there } => SetReview::Review {
                review: review.marked(plane, level),
                there,
            },
            other => other,
        }
    }
}

/// The levels `name` is installed at from `origin`, by the lockfiles' entries `locked`.
pub(super) fn installed_at(
    locked: &[(lock::Kept, lock::Locked)],
    origin: &Origin,
    name: &str,
) -> Vec<Level> {
    locked
        .iter()
        .filter(|(_, one)| one.at_source() == name && one.source == *origin)
        .map(|(_, one)| one.level.0.clone())
        .collect()
}

/// One skill a source offers, as the studio lists it to browse, and where it is installed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offered {
    #[serde(flatten)]
    pub skill: Found,
    /// The levels it is installed at from this source.
    pub installed: Vec<Level>,
    /// **What its files do beyond its instructions**, a few words each
    /// ([`summary::Summary::marks`]): what a source's page marks it with before any review.
    /// `None` where its files were not read to browse, as the source holds more than one read
    /// may take: its review reads them.
    pub marks: Option<Vec<String>>,
    /// What the project's controls would refuse of installing it, said before its review
    /// (#6031): its source or itself not allowed, or the commit too young, with when.
    pub refused: Option<super::policy::Refused>,
}

/// **What a source offers**: its newest commit, its skills, and each folder that is not a skill
/// purlis offers, with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Browse {
    pub commit: Commit,
    pub skills: Vec<Offered>,
    pub not_skills: Vec<NotASkill>,
    /// When it was read, in seconds since the epoch, as [`super::checked`] recorded it.
    pub checked: u64,
}

/// **Browses `source`** at its newest commit.
pub fn browse(plane: &Path, source: &Source) -> Result<Browse, String> {
    let fetched = fetch(plane, source, None)?;
    let (found, not_skills) = fetched.skills()?;
    let checked = super::checked::browsed(plane, &source.remote, found.len());
    let (locked, _) = lock::every(plane);
    let origin = Origin::Git(source.remote.clone());
    // The org's controls, and when the commit was made where they hold a minimum age (#6031).
    let policy = super::policy::of(plane);
    let pin = Pin::Commit(fetched.commit.clone());
    let made = if super::policy::asks_age(&policy) {
        super::SkillSource::made(&fetched)
    } else {
        None
    };
    // Each skill's files are read for its marks until one read would take the fetch past what it
    // may read: the rest are left for their reviews, and the browse is not refused for them.
    let mut reading = true;
    Ok(Browse {
        commit: fetched.commit.clone(),
        skills: found
            .into_iter()
            .map(|skill| {
                let marks = if reading {
                    match super::SkillSource::files(&fetched, &skill) {
                        Ok((files, _)) => Some(summary_of(&files).marks()),
                        Err(_) => {
                            reading = false;
                            None
                        }
                    }
                } else {
                    None
                };
                Offered {
                    installed: installed_at(&locked, &origin, &skill.name),
                    marks,
                    // A commit whose time could not be read here is held to the lists alone,
                    // as the Updates list holds it: its review reads the time, and says.
                    refused: if made.is_none() && super::policy::asks_age(&policy) {
                        super::policy::listed_before(&policy, &origin, &skill.name)
                    } else {
                        super::policy::refused_before(&policy, &origin, &skill.name, &pin, made)
                    },
                    skill,
                }
            })
            .collect(),
        not_skills,
        checked,
    })
}

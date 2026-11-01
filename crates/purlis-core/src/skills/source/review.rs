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
use super::{Commit, Source, lock};
use crate::skills::verbs::{Level, Whose};
use crate::skills::{Reference, Reserved};

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
}

/// **A name that is taken where a skill would land** (ADR 0100, amended 2026-11-01): the review
/// stops until the person skips it or gives it a new name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clash {
    /// What holds the name, as the end of a sentence.
    pub held_by: String,
    /// The new name offered: `<source>-<name>`.
    pub offered: String,
    /// The names of the skills at that level, which a new name cannot take.
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
    /// The skills it refers to that the level it lands in holds already (#4032): they are not
    /// brought along, and the install leaves them as they are.
    pub already: Vec<String>,
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

/// **What an install or an update was accepted as**: the commit and each skill's digest, as the
/// review showed them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    pub commit: Commit,
    pub skills: Vec<AcceptedSkill>,
}

impl Accepted {
    /// What accepting `skills` at `commit` says.
    pub fn of(commit: &Commit, skills: &[SkillReview]) -> Accepted {
        Accepted {
            commit: commit.clone(),
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
        self.commit == now.commit && a == b
    }
}

impl Review {
    /// **Marks each skill with where it lands at `level`**: its clash, and its one-line notes.
    fn marked(mut self, plane: &Path, reserved: &Reserved, level: &Level) -> Review {
        let (locked, _) = lock::every(plane);
        let label = self.source.remote.label();
        for skill in &mut self.skills {
            let name = skill.name.clone();
            skill.clash = held_by(plane, &locked, reserved, level, &name).map(|held_by| Clash {
                held_by,
                offered: format!("{label}-{name}"),
                taken: names_at(plane, level),
            });
            let others: Vec<String> = Level::every(plane)
                .into_iter()
                .filter(|other| other != level && has(plane, other, &name))
                .map(|other| other.to_string())
                .collect();
            if !others.is_empty() {
                skill.notes.push(format!(
                    "The {} skills have a {name} too: where a chat gets both, the narrowest copy \
                     wins.",
                    others.join(", ")
                ));
            }
            for other in &skill.already {
                let theirs = locked.iter().find(|(_, one)| {
                    one.name == *other
                        && one.source != self.source.remote
                        && (one.level.0 == *level || one.level.0 == Level::project())
                });
                if let Some((_, one)) = theirs {
                    skill.notes.push(format!(
                        "It refers to {other}, which is {}'s here.",
                        one.source.label()
                    ));
                }
            }
        }
        self
    }

    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        Accepted::of(&self.commit, &self.skills)
    }
}

/// `sha256:` and the hex digest of `bytes`.
pub(super) fn sha256(bytes: &[u8]) -> String {
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

/// The review of the skill `found`, whose files are `files` and whose left-out entries are
/// `left`, brought along by `brought_by`, in a source that holds the skills `held`, landing in a
/// level that holds those `have` says.
pub(super) fn review_of(
    found: &Found,
    files: &[SourceFile],
    left: Vec<LeftOut>,
    brought_by: Option<String>,
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
    for name in referenced(files) {
        if name == found.name {
            continue;
        }
        if have(&name) {
            already.push(name);
        } else if !held.contains(&name) {
            lint.push(format!(
                "it refers to the skill {name}, which this source does not hold"
            ));
        }
    }
    SkillReview {
        name: found.name.clone(),
        description: found.description.clone(),
        folder: found.folder.clone(),
        brought_by,
        already,
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
            })
            .collect(),
        left_out: left,
        lint,
        digest: digest_of(&wrote(files)),
        summary,
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

/// **The review of `names` from `source`**, and every skill they refer to that the source holds
/// and `have` does not, each right after the one that brought it, so one that was also named is
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
) -> Result<Vec<Reviewed>, Unread> {
    let (found, _) = source.skills()?;
    reviewed_from(source, &found, names, have)
}

/// [`reviewed`], with what the source offers already listed as `found`: listing it again would
/// read every skill's instructions a second time against the read's limit.
fn reviewed_from(
    source: &dyn super::SkillSource,
    found: &[Found],
    names: &[String],
    have: &dyn Fn(&str) -> bool,
) -> Result<Vec<Reviewed>, Unread> {
    let held: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
    let mut queue: Vec<(String, Option<String>)> =
        names.iter().map(|name| (name.clone(), None)).collect();
    let mut skills: Vec<Reviewed> = Vec::new();
    // The named whose reviews are whole, the named one being reviewed now, and how many skills
    // the whole ones reviewed with what they brought along: a skill brought along is reviewed
    // before the next named one is taken.
    let mut whole: Vec<String> = Vec::new();
    let mut now: Option<String> = None;
    let mut wholly = 0;
    while !queue.is_empty() {
        let (name, by) = queue.remove(0);
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
        let mut next: Vec<(String, Option<String>)> = referenced(&files)
            .into_iter()
            .filter(|other| *other != name && held.contains(other) && !have(other))
            .map(|other| (other, Some(name.clone())))
            .collect();
        next.append(&mut queue);
        queue = next;
        // One that is named is reviewed itself, so it is never said to be left as it is.
        let left_as_is = |other: &str| have(other) && !names.iter().any(|named| named == other);
        skills.push(Reviewed {
            review: review_of(one, &files, left, by, &held, &left_as_is),
            files,
        });
    }
    Ok(skills)
}

/// **The review of `names` from `source`** ([`reviewed`]), what more than one read may take said
/// in its sentence.
pub(super) fn reviews_of(
    source: &dyn super::SkillSource,
    names: &[String],
    have: &dyn Fn(&str) -> bool,
) -> Result<Vec<SkillReview>, String> {
    reviewed(source, names, have)
        .map(|skills| skills.into_iter().map(|one| one.review).collect())
        .map_err(Unread::said)
}

impl Fetched {
    /// **The review of `names`** from this commit ([`reviews_of`]).
    pub(super) fn review_with(
        &self,
        names: &[String],
        have: &dyn Fn(&str) -> bool,
    ) -> Result<Review, String> {
        Ok(Review {
            source: self.source.clone(),
            commit: self.commit.clone(),
            skills: reviews_of(self, names, have)?,
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
    ) -> Result<SetReview, String> {
        let (there, names): (Vec<String>, Vec<String>) =
            names.iter().cloned().partition(|name| holds(name));
        match reviewed_from(self, found, &names, have) {
            Ok(skills) => Ok(SetReview::Review {
                review: Review {
                    source: self.source.clone(),
                    commit: self.commit.clone(),
                    skills: skills.into_iter().map(|one| one.review).collect(),
                },
                there,
            }),
            Err(Unread::TooMuch(too_much)) => Ok(SetReview::TooMuch(TooMuch { there, ..too_much })),
            Err(Unread::Refused(why)) => Err(why),
        }
    }
}

/// **What holds `name` where a skill would land at `level`**, as the end of a sentence: a built-in
/// skill, or a skill at `level` from another source or made there.
fn held_by(
    plane: &Path,
    locked: &[(lock::Kept, lock::Locked)],
    reserved: &Reserved,
    level: &Level,
    name: &str,
) -> Option<String> {
    if reserved.holds(name) {
        return Some(format!("{name} belongs to a built-in skill"));
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
/// - `name` belongs to a built-in skill, which every chat has and no plane skill may take (#4155);
/// - `level` holds a skill of that name, from any source or made there (#4032);
/// - at a workspace's level, a project skill of that name is granted to the workspace and not
///   turned off there (#4154);
/// - at a persona's level, the persona's `skills:` list names a project skill of that name, which
///   its chats get in any workspace (#4154).
///
/// A shared install is met by shared skills alone, as its copy reaches every clone and another
/// person's local skills are not there; a local install, which runs on this machine alone, by its
/// local skills and the shared ones. `reserved` is the built-in skills' names, and `wider` what
/// the wider levels say ([`Wider`]).
pub(super) fn met(
    plane: &Path,
    reserved: &Reserved,
    wider: &Wider,
    level: &Level,
    name: &str,
) -> bool {
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
    reserved.holds(name)
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

/// [`met`] for one review, with the built-in skills in `built_in`: their names and the wider
/// levels read once, and each name looked up once, however many skills refer to it.
pub(super) fn met_at<'a>(
    plane: &'a Path,
    built_in: Option<&Path>,
    level: &'a Level,
) -> impl Fn(&str) -> bool + 'a {
    let reserved = Reserved::of(built_in);
    let wider = Wider::of(plane, level);
    let seen = std::cell::RefCell::new(std::collections::HashMap::<String, bool>::new());
    move |name| {
        if let Some(known) = seen.borrow().get(name) {
            return *known;
        }
        let held = met(plane, &reserved, &wider, level, name);
        seen.borrow_mut().insert(name.to_owned(), held);
        held
    }
}

/// **The review of installing `names` from `source` at `level`**, at its newest commit, with the
/// built-in skills in `built_in`: every skill they refer to that the source holds and is not met
/// at `level` already ([`met`]) comes along.
pub fn review(
    plane: &Path,
    built_in: Option<&Path>,
    source: &Source,
    names: &[String],
    level: &Level,
) -> Result<Review, String> {
    Ok(fetch(plane, source, None)?
        .review_with(names, &met_at(plane, built_in, level))?
        .marked(plane, &Reserved::of(built_in), level))
}

/// **The review of installing `names` from `source` together at `level`** (#4024), at its newest
/// commit: one review and one install for the set, or, past what one read may take, the names
/// that fit. One installed from this source at `level` already is left out, and said to be there
/// (#4153): the window's list is ticked before the level is chosen.
pub fn review_set(
    plane: &Path,
    built_in: Option<&Path>,
    source: &Source,
    names: &[String],
    level: &Level,
) -> Result<SetReview, String> {
    let fetched = fetch(plane, source, None)?;
    let (found, _) = fetched.skills()?;
    let installed = lock::every(plane).0;
    let reserved = Reserved::of(built_in);
    Ok(fetched
        .set_review(
            &found,
            names,
            &|name| {
                installed_at(&installed, source, name)
                    .iter()
                    .any(|at| at == level)
            },
            &met_at(plane, built_in, level),
        )?
        .marked(plane, &reserved, level))
}

/// **The review of installing every skill of `source` not installed from it yet** (#4024,
/// `purlis skill install <source> --all`), at its newest commit, landing at `level`. A review of no
/// skill where every one is.
pub fn review_all(
    plane: &Path,
    built_in: Option<&Path>,
    source: &Source,
    level: &Level,
) -> Result<SetReview, String> {
    let fetched = fetch(plane, source, None)?;
    let (found, _) = fetched.skills()?;
    let (locked, _) = lock::every(plane);
    let names: Vec<String> = found
        .iter()
        .filter(|skill| installed_at(&locked, source, &skill.name).is_empty())
        .map(|skill| skill.name.clone())
        .collect();
    let installed = lock::every(plane).0;
    let reserved = Reserved::of(built_in);
    Ok(fetched
        .set_review(
            &found,
            &names,
            &|name| {
                installed_at(&installed, source, name)
                    .iter()
                    .any(|at| at == level)
            },
            &met_at(plane, built_in, level),
        )?
        .marked(plane, &reserved, level))
}

impl SetReview {
    /// [`Review::marked`], where it is a review.
    fn marked(self, plane: &Path, reserved: &Reserved, level: &Level) -> SetReview {
        match self {
            SetReview::Review { review, there } => SetReview::Review {
                review: review.marked(plane, reserved, level),
                there,
            },
            other => other,
        }
    }
}

/// The levels `name` is installed at from `source`, by the lockfiles' entries `locked`.
fn installed_at(locked: &[(lock::Kept, lock::Locked)], source: &Source, name: &str) -> Vec<Level> {
    locked
        .iter()
        .filter(|(_, one)| one.at_source() == name && one.source == source.remote)
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
                    installed: installed_at(&locked, source, &skill.name),
                    marks,
                    skill,
                }
            })
            .collect(),
        not_skills,
        checked,
    })
}

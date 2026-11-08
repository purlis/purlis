// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Publishing a skill to a skill source** (SK-18, #2781; spec #2762 decision 44): a draft pull
//! or merge request on the source, carrying the skill, the skills it refers to, and a summary of
//! their grades.
//!
//! [`review`] answers everything that would go out, and nothing has gone out then: the source is
//! read, never written. [`publish`] is handed that review back, reads everything again, and
//! refuses where any of it moved or anything stops it, so what goes is what was reviewed.
//!
//! **The commit is built from git's objects, never a checkout** of the source's own files: its
//! tree at the reviewed commit, with each skill's folder replaced. It goes to a new branch, named
//! by the digest of what it holds, by the one refspec, never forced.
//!
//! **Where the branch goes** ([`Route`], #4342): the review asks the forge whether the person's
//! account may push to the source. Where it says yes, or does not say, the branch goes on the
//! source itself and the push answers for itself. Where it says no, the branch goes on the
//! person's own fork of the source, under their own account and never an organisation's: the
//! one they have, or one made at the forge's default visibility once they say yes to a review
//! that named it and said it would be made. A repo of theirs at that name that the forge does
//! not call a fork of the source stops the publish, and nothing is renamed. A fork is never
//! deleted, so one made before a later step failed stays, and is said.
//!
//! **Grades go as a summary** in the request's description, and never in the skill's folder
//! (decision 21): when, which harness, which grader, how it scored and how it ended. Never a
//! chat's text, a path, a cost, a profile or a device. Every word of a row that is the record's
//! own is held to plain characters, as the row may be a teammate's.

use std::path::Path;

use purlis_skills::tree;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use super::carried;
use super::fetch::{Handing, REMOTE, Scratch, handing};
use super::outgoing::{self, OutSkill, Outgoing, Purpose, Stop};
use super::{Commit, Source};
use crate::forge::backend::Visibility;
use crate::forge::pr::Repo;
use crate::forge::{Caller, Fork, ForkPlace};
use crate::secretshape;
use crate::skills::grades::{self, Graded};
use crate::skills::verbs::Level;
use crate::worktree::git;

/// The most grades a skill's summary lists, newest first.
const MOST_GRADES: usize = 10;

/// **The forge a publish opens its request on.** A test's stands in for it.
pub trait PublishForge {
    /// The credential helper the push gets, or `None` for a folder on this machine.
    fn helper(&self) -> Option<String>;
    /// Whether the person's account may push to the source, as the forge says it: `None` where
    /// its answer does not say. Read only, for the review alone.
    fn may_push(&self) -> Result<Option<bool>, String>;
    /// Where the account's own fork of the source is, or would be. Read only, for the review.
    fn fork_for(&self) -> Result<ForkPlace, String>;
    /// Makes the account's fork of the source: only after a yes to a review that said so.
    fn make_fork(&self) -> Result<Fork, String>;
    /// The page of the open request already on `branch`, of `fork` where it is given. A closed
    /// or merged one does not count.
    fn already(&self, fork: Option<&Fork>, branch: &str) -> Result<Option<String>, String>;
    /// Opens the draft request from `head`, a branch of `fork` where it is given, and answers
    /// its page.
    fn open_draft(
        &self,
        fork: Option<&Fork>,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String>;
}

/// The person's own forge, through their purlis sign-in.
pub struct OnTheForge {
    repo: Repo,
    caller: Caller,
    /// The window's own credential helper, where the window runs this.
    helper: Option<String>,
}

impl OnTheForge {
    /// The forge `source` is on, refused with a sentence where it is on none this plane knows.
    pub fn of(plane: &Path, source: &Source) -> Result<OnTheForge, String> {
        let url = &source.remote.url;
        let not_a_forge = || {
            format!(
                "{url} is not on a GitHub or GitLab host this project knows, so no request can \
                 be opened there. Export the skill instead, or declare the host as a forge of this \
                 project"
            )
        };
        if !url.starts_with("https://") {
            return Err(not_a_forge());
        }
        let forge = crate::forge::resolve_host(url, plane).ok_or_else(not_a_forge)?;
        let path = crate::forge::namespace_of(url).ok_or_else(not_a_forge)?;
        Ok(OnTheForge {
            repo: Repo { forge, path },
            caller: Caller::command(),
            helper: None,
        })
    }

    /// [`Self::of`], as the person in the window: their purlis account for the host and no other
    /// helper. Without a sign-in nothing is pushed.
    pub fn for_the_window(plane: &Path, source: &Source) -> Result<OnTheForge, String> {
        let mut forge = Self::of(plane, source)?;
        let host = forge.repo.forge.host.clone();
        forge.helper = Some(
            crate::forge::gitcredential::for_the_window(&host).ok_or_else(|| {
                format!(
                    "purlis has no sign-in to {host} to push with. Sign in in {}, then try again",
                    crate::forge::signin::SIGN_IN_AT
                )
            })?,
        );
        forge.caller = Caller::window();
        Ok(forge)
    }

    /// What this forge calls a request: "pull request" or "merge request".
    pub fn request_noun(&self) -> &'static str {
        self.repo.forge.kind.request_noun()
    }

    /// The forge's name, as a button says it.
    pub fn display(&self) -> &'static str {
        self.repo.forge.kind.display()
    }
}

impl PublishForge for OnTheForge {
    fn helper(&self) -> Option<String> {
        self.helper
            .clone()
            .or_else(|| Some(crate::forge::helper_for(&self.repo.forge)))
    }

    fn may_push(&self) -> Result<Option<bool>, String> {
        let record =
            crate::forge::backend::RepoRecord::addressed_by(&self.repo.path, self.repo.forge.kind);
        let about = self
            .repo
            .backend()
            .about(&self.caller, &record)
            .map_err(|why| why.to_string())?;
        Ok(about.can_push)
    }

    fn fork_for(&self) -> Result<ForkPlace, String> {
        self.repo
            .backend()
            .fork_for(&self.caller, &self.repo.path)
            .map_err(|why| why.to_string())
    }

    fn make_fork(&self) -> Result<Fork, String> {
        self.repo
            .backend()
            .make_fork(&self.caller, &self.repo.path)
            .map_err(|why| why.to_string())
    }

    fn already(&self, fork: Option<&Fork>, branch: &str) -> Result<Option<String>, String> {
        let backend = self.repo.backend();
        let found = match fork {
            None => backend.by_head(&self.caller, &self.repo.path, branch),
            Some(fork) => backend.by_head_from_fork(&self.caller, &self.repo.path, fork, branch),
        }
        .map_err(|why| why.to_string())?;
        Ok(found
            .filter(|request| request.state == crate::forge::pr::State::Open)
            .map(|request| request.url))
    }

    fn open_draft(
        &self,
        fork: Option<&Fork>,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        let backend = self.repo.backend();
        let path = &self.repo.path;
        let pr = match fork {
            None => backend.open_draft(&self.caller, path, head, base, title, body),
            Some(fork) => {
                backend.open_draft_from_fork(&self.caller, path, fork, head, base, title, body)
            }
        }
        .map_err(|why| why.to_string())?;
        Ok(pr.url)
    }
}

/// **Where a publish's branch goes** (#4342).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// A new branch on the source itself.
    Source,
    /// A new branch on the person's own fork of the source, which they have already.
    Fork(Fork),
    /// A new branch on the person's own fork of the source, which is made at `path` once they
    /// say yes, readable as `visibility`, the forge's default for it.
    NewFork {
        path: String,
        visibility: Visibility,
    },
}

impl Route {
    /// The route, as a yes holds it.
    fn shown(&self) -> String {
        match self {
            Route::Source => "source".to_owned(),
            Route::Fork(fork) => format!("fork {} {}", fork.path(), fork.visibility().word()),
            Route::NewFork { path, visibility } => {
                format!("new fork {path} {}", visibility.word())
            }
        }
    }
}

/// Where the branch goes, as the forge says now: the source unless the forge says the person's
/// account may not push there. A repo of theirs that is not the source's fork stops it.
fn route_of(forge: &dyn PublishForge, review: &mut PublishReview) -> Result<(), String> {
    if review.stopped() {
        return Ok(());
    }
    review.may_push = forge.may_push().ok().flatten();
    if review.may_push != Some(false) {
        return Ok(());
    }
    match forge
        .fork_for()
        .map_err(|why| format!("Where your fork of the source is could not be read: {why}"))?
    {
        ForkPlace::Yours(fork) => review.route = Route::Fork(fork),
        ForkPlace::ToMake { path, visibility } => {
            review.route = Route::NewFork { path, visibility };
        }
        ForkPlace::Taken { path } => review.outgoing.stops.push(Stop::ForkTaken { path }),
    }
    Ok(())
}

/// **Everything a person reads before a publish.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishReview {
    pub outgoing: Outgoing,
    /// The source's branch the request goes into, and its commit it was reviewed against.
    pub base: String,
    pub commit: Commit,
    /// The new branch.
    pub branch: String,
    /// Each skill's folder in the source, and whether the source holds it already.
    pub lands: Vec<(String, bool)>,
    pub title: String,
    /// Whom the commit names, as the project's git config says: it goes out with the commit.
    pub author: (String, String),
    /// The grades the description holds.
    pub grades: Vec<GradeSent>,
    /// **Each grade file the commit carries beside a skill's folder** (ADR 0100, amended
    /// 2026-11-08): its path in the source, and its bytes. None for a skill with no grade.
    pub carried: Vec<(String, Vec<u8>)>,
    /// Whether the person's account may push to the source, as its forge says (#4342). A no
    /// sends the branch to their fork ([`Route`]); nothing else refuses on it. `None` where the
    /// forge does not say, cannot be asked, or the review is stopped already.
    pub may_push: Option<bool>,
    /// Where the branch goes: the person's fork where the forge says they may not push.
    pub route: Route,
    /// The request's description, the grades among it, whole.
    pub body: String,
}

impl PublishReview {
    /// Whether anything stops it.
    pub fn stopped(&self) -> bool {
        !self.outgoing.stops.is_empty()
    }

    /// **What a yes to this review is a yes to**: everything it showed, by digest.
    pub fn accepted(&self) -> Accepted {
        let lands: Vec<&str> = self.lands.iter().map(|land| land.0.as_str()).collect();
        let route = self.route.shown();
        let shown = [
            self.outgoing.digest().as_str(),
            self.base.as_str(),
            self.commit.as_str(),
            self.branch.as_str(),
            &lands.join("\n"),
            self.title.as_str(),
            self.author.0.as_str(),
            self.author.1.as_str(),
            self.body.as_str(),
            route.as_str(),
            &carried_digest(&self.carried),
        ]
        .map(|field| format!("{}:{field}", field.len()))
        .join("\n");
        Accepted {
            base: self.base.clone(),
            commit: self.commit.clone(),
            digest: crate::runnerlink::hex_of(&sha2::Sha256::digest(shown.as_bytes())),
        }
    }
}

/// **A person's yes to one review**: where it was reviewed against, and a digest of all it showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    pub base: String,
    pub commit: Commit,
    pub digest: String,
}

/// **What a publish of `name` at `level` to `source` would send.** Writes nothing anywhere.
pub fn review(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    source: &Source,
    forge: &dyn PublishForge,
) -> Result<PublishReview, String> {
    let scratch = Scratch::new()?;
    let mut review = reviewed(plane, built_in, level, name, source, &scratch, None)?;
    route_of(forge, &mut review)?;
    Ok(review)
}

/// The review, with the source fetched into `scratch`, at the commit `at` where one is held to.
fn reviewed(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    source: &Source,
    scratch: &Scratch,
    at: Option<&Accepted>,
) -> Result<PublishReview, String> {
    source.check()?;
    let author = author_of(plane)?;
    let mut outgoing = outgoing::gather(plane, built_in, level, name, Purpose::Publish)?;
    let base = match at {
        Some(reviewed) => reviewed.base.clone(),
        None => base_of(plane, source, scratch)?,
    };
    let commit = match at {
        Some(reviewed) => reviewed.commit.clone(),
        None => super::newest(plane, source)?,
    };
    let handed = handing(plane, source)?;
    let run = scratch.fetch(&source.remote, &handed, commit.as_str(), "refs/purlis/base")?;
    if !run.ok() {
        return Err(format!(
            "{source} could not be fetched: {}",
            super::fetch::complaint(&run)
        ));
    }
    let mut lands = Vec::new();
    for skill in &outgoing.skills {
        let folder = folder_in(source, &skill.name)?;
        let held = scratch.git(&["ls-tree", "--name-only", "refs/purlis/base", "--", &folder])?;
        lands.push((folder, held.ok() && !held.out.trim().is_empty()));
    }
    let carried = carried_of(plane, scratch, &outgoing.skills, &lands)?;
    // Named by what it holds and where it goes: the same files elsewhere are another request.
    let digest = {
        let folders: Vec<&str> = lands.iter().map(|land| land.0.as_str()).collect();
        let place = format!(
            "{}\n{base}\n{}\n{}",
            outgoing.digest(),
            folders.join("\n"),
            carried_digest(&carried)
        );
        crate::runnerlink::hex_of(&sha2::Sha256::digest(place.as_bytes()))
    };
    let title = title_of(&outgoing.skills, lands.first().is_some_and(|land| land.1));
    let grades = grades_sent(plane, &outgoing.skills);
    let body = body_of(&outgoing.skills, &lands, &grades);
    // The description goes out too: anything in it stops the publish as a file would.
    for (index, line) in body.lines().enumerate() {
        for leak in secretshape::leaks(line) {
            outgoing.stops.push(Stop::Found {
                skill: name.to_owned(),
                path: "the request's description".to_owned(),
                line: index + 1,
                kind: leak.kind.to_owned(),
                masked: secretshape::masked(&line[leak.span.clone()]),
            });
        }
    }
    Ok(PublishReview {
        branch: format!("purlis/publish/{name}-{}", &digest[..12]),
        outgoing,
        base,
        commit,
        lands,
        title,
        author,
        grades,
        carried,
        body,
        may_push: None,
        route: Route::Source,
    })
}

/// The name and address the commit carries, from the project's git config.
fn author_of(plane: &Path) -> Result<(String, String), String> {
    let identity = |key: &str| {
        git::run(plane, &["config", key], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    identity("user.name")
        .zip(identity("user.email"))
        .ok_or_else(|| {
            "git has no user.name and user.email for this project, so no commit can be made. Set \
             them, then publish again"
                .to_owned()
        })
}

/// The branch a request goes into: the one the source names, or its repository's default.
fn base_of(plane: &Path, source: &Source, scratch: &Scratch) -> Result<String, String> {
    if let Some(branch) = &source.remote.branch {
        return Ok(branch.clone());
    }
    let handed = handing(plane, source)?;
    let run = scratch.network(
        &source.remote,
        &handed,
        &["ls-remote", "--symref", "--", REMOTE, "HEAD"],
    )?;
    run.out
        .lines()
        .find_map(|line| line.strip_prefix("ref: refs/heads/")?.split_once('\t'))
        .map(|(branch, _)| branch.to_owned())
        .ok_or_else(|| format!("{source} names no default branch. Add the source with a branch"))
}

/// The folder `name` lands in, in the source.
fn folder_in(source: &Source, name: &str) -> Result<String, String> {
    let at = source.folder.as_deref().unwrap_or("");
    let parts = tree::folder_parts(at)
        .ok_or_else(|| format!("{at:?} cannot be a folder of a skill source"))?;
    Ok(parts
        .iter()
        .map(String::as_str)
        .chain([name])
        .collect::<Vec<_>>()
        .join("/"))
}

fn title_of(skills: &[OutSkill], replaces: bool) -> String {
    let name = skills.first().map_or("", |skill| skill.name.as_str());
    let verb = if replaces { "Update" } else { "Add" };
    match skills.len() {
        0 | 1 => format!("{verb} the skill {name}"),
        n => format!("{verb} the skill {name} and {} it refers to", n - 1),
    }
}

/// `text` held to characters that mean nothing in a table or to a terminal.
fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
        .take(64)
        .collect()
}

fn percent(value: Option<f64>) -> String {
    value.map_or("not run".to_owned(), |rate| format!("{:.0}%", rate * 100.0))
}

/// **One grade, as it goes out**: nothing the record holds but these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeSent {
    pub skill: String,
    pub when: String,
    pub version: &'static str,
    pub harness: String,
    pub grader: String,
    pub with_it: String,
    pub without_it: String,
    pub runs: u8,
    pub ended: &'static str,
}

fn grade_sent(skill: &str, graded: &Graded, now: Option<&str>) -> GradeSent {
    let arm = |arm| {
        graded
            .report
            .arms
            .iter()
            .find(|score| score.arm == arm)
            .and_then(|score| score.pass_rate.as_ref())
            .map(|stat| stat.mean)
    };
    let date: String = graded.ts.chars().take(10).collect();
    GradeSent {
        skill: plain(skill),
        when: plain(&date),
        version: if now == Some(graded.tree.as_str()) {
            "this one"
        } else {
            "earlier"
        },
        harness: plain(graded.harness.as_deref().unwrap_or("unknown")),
        grader: plain(&graded.report.grader),
        with_it: percent(arm(purlis_skills::grade::Arm::With)),
        without_it: percent(arm(purlis_skills::grade::Arm::Without)),
        runs: graded.runs,
        ended: grades::ended_said(graded.report.ended),
    }
}

/// One row of a skill's grades, in the request's description.
fn grade_row(sent: &GradeSent) -> String {
    format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} |",
        sent.when,
        sent.version,
        sent.harness,
        sent.grader,
        sent.with_it,
        sent.without_it,
        sent.runs,
        sent.ended,
    )
}

/// The tree git holds of a skill's folder at the plane's newest commit, while its files are that.
fn tree_now(plane: &Path, folder: &Path) -> Option<String> {
    let rel = folder.strip_prefix(plane).ok()?.to_str()?;
    let changed = git::run(plane, &["status", "--porcelain", "--", rel], git::READ).ok()?;
    if !changed.ok() || !changed.out.trim().is_empty() {
        return None;
    }
    let spec = format!("HEAD:{rel}");
    let run = git::run(plane, &["rev-parse", &spec], git::READ).ok()?;
    run.ok().then(|| run.line().trim().to_owned())
}

/// **The grade file each of `skills` carries beside its folder**, `lands` their folders: the rows
/// the description holds, each with the tree it measured. A grade of the version that goes is
/// tied to the tree git gives the folder as it goes (`scratch`, holding the source's commit), so
/// the files that stay behind do not make it an earlier one. None is written where the place
/// beside the folder is inside a skill the source holds (decision 21).
fn carried_of(
    plane: &Path,
    scratch: &Scratch,
    skills: &[OutSkill],
    lands: &[(String, bool)],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for (skill, (folder, _)) in skills.iter().zip(lands) {
        let Some(path) = carried::beside(folder) else {
            continue;
        };
        if inside_a_skill(scratch, &path)? {
            continue;
        }
        let graded = grades::read_followed(plane, &skill.level, &skill.name).grades;
        if graded.is_empty() {
            continue;
        }
        let now = skill
            .level
            .named(plane, &skill.name)
            .and_then(|folder| tree_now(plane, &folder));
        let going = tree_in(scratch, &skill.files)?;
        // The same grades the description lists, so the file never holds one it does not.
        let rows: Vec<carried::Row> = graded
            .iter()
            .take(MOST_GRADES)
            .filter_map(|one| {
                let tree = if now.as_deref() == Some(one.tree.as_str()) {
                    going.clone()
                } else {
                    one.tree.clone()
                };
                carried::row_of(one, tree)
            })
            .collect();
        if let Some(bytes) = carried::file_of(&skill.name, rows) {
            out.push((path, bytes));
        }
    }
    Ok(out)
}

/// Whether `path` in the source's commit is inside a skill's folder: a folder above it holds a
/// `SKILL.md`.
fn inside_a_skill(scratch: &Scratch, path: &str) -> Result<bool, String> {
    let parts: Vec<&str> = path.split('/').collect();
    for depth in 0..parts.len() {
        let above = parts[..depth].join("/");
        let instructions = if above.is_empty() {
            purlis_skills::FILE.to_owned()
        } else {
            format!("{above}/{}", purlis_skills::FILE)
        };
        let held = scratch.git(&[
            "ls-tree",
            "--name-only",
            "refs/purlis/base",
            "--",
            &instructions,
        ])?;
        if held.ok() && !held.out.trim().is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// **The tree git gives a folder holding `files`**, written into `scratch` in the source's own
/// object format, as the commit will hold it.
fn tree_in(scratch: &Scratch, files: &[outgoing::OutFile]) -> Result<String, String> {
    let mut listed: Vec<(Vec<String>, &'static str, String)> = Vec::new();
    for file in files {
        let blob = scratch.git_raw(&["hash-object", "-w", "--stdin"], file.bytes.clone())?;
        if blob.code != Some(0) {
            return Err("a file of the skill could not be written".to_owned());
        }
        let mode = if file.executable { "100755" } else { "100644" };
        let oid = String::from_utf8_lossy(&blob.out).trim().to_owned();
        listed.push((file.parts.clone(), mode, oid));
    }
    folder_in_scratch(scratch, &listed)
}

/// The tree of `listed` (each its path's parts, mode and blob), its folders made first.
fn folder_in_scratch(
    scratch: &Scratch,
    listed: &[(Vec<String>, &'static str, String)],
) -> Result<String, String> {
    let mut lines = String::new();
    let mut folders: Vec<&str> = Vec::new();
    for (parts, mode, oid) in listed {
        match parts.as_slice() {
            [name] => lines.push_str(&format!("{mode} blob {oid}\t{name}\n")),
            [first, ..] if !folders.contains(&first.as_str()) => folders.push(first),
            _ => {}
        }
    }
    for folder in folders {
        let inner: Vec<(Vec<String>, &'static str, String)> = listed
            .iter()
            .filter(|(parts, _, _)| parts.len() > 1 && parts[0] == folder)
            .map(|(parts, mode, oid)| (parts[1..].to_vec(), *mode, oid.clone()))
            .collect();
        let tree = folder_in_scratch(scratch, &inner)?;
        lines.push_str(&format!("040000 tree {tree}\t{folder}\n"));
    }
    let made = scratch.git_raw(&["mktree"], lines.into_bytes())?;
    if made.code != Some(0) {
        return Err("the skill's folder could not be written".to_owned());
    }
    Ok(String::from_utf8_lossy(&made.out).trim().to_owned())
}

/// What the grade files hold, as one line of what a yes is a yes to.
fn carried_digest(carried: &[(String, Vec<u8>)]) -> String {
    let mut hash = sha2::Sha256::new();
    for (path, bytes) in carried {
        hash.update(format!("{}:{path}\n{}:", path.len(), bytes.len()).as_bytes());
        hash.update(bytes);
    }
    crate::runnerlink::hex_of(&hash.finalize())
}

/// The grades that go with `skills`, each skill's newest first.
fn grades_sent(plane: &Path, skills: &[OutSkill]) -> Vec<GradeSent> {
    let mut out = Vec::new();
    for skill in skills {
        let graded = grades::read_followed(plane, &skill.level, &skill.name).grades;
        let now = skill
            .level
            .named(plane, &skill.name)
            .and_then(|folder| tree_now(plane, &folder));
        out.extend(
            graded
                .iter()
                .take(MOST_GRADES)
                .map(|one| grade_sent(&skill.name, one, now.as_deref())),
        );
    }
    out
}

/// **The request's description**: what goes, then each skill's grades.
fn body_of(skills: &[OutSkill], lands: &[(String, bool)], sent: &[GradeSent]) -> String {
    let mut out = String::from("Published with purlis. Review it as you would any change.\n\n");
    out.push_str("## Files\n\n");
    for (skill, (folder, _)) in skills.iter().zip(lands) {
        out.push_str(&format!("- `{folder}/`: {} files\n", skill.files.len()));
    }
    out.push_str("\n## Grades\n");
    for skill in skills {
        let name = plain(&skill.name);
        out.push_str(&format!("\n### {name}\n\n"));
        let rows: Vec<&GradeSent> = sent.iter().filter(|one| one.skill == name).collect();
        if rows.is_empty() {
            out.push_str("Not graded.\n");
            continue;
        }
        out.push_str(
            "| When | Version | Harness | Grader | With the skill | Without | Runs | Ended |\n\
             | --- | --- | --- | --- | --- | --- | --- | --- |\n",
        );
        for one in rows {
            out.push_str(&grade_row(one));
            out.push('\n');
        }
    }
    out
}

/// What a publish did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    Opened {
        url: String,
        branch: String,
    },
    /// The same files already have a request.
    Already {
        url: String,
    },
    /// git could not push to the source, which takes write access, or to the person's fork.
    /// Nothing was opened.
    PushRefused {
        why: String,
    },
}

/// **Sends what `accepted` showed**, refused whole where any of it moved since or anything stops
/// it. Nothing is written on the source before every check has passed.
pub fn publish(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    source: &Source,
    accepted: &Accepted,
    forge: &dyn PublishForge,
) -> Result<Published, String> {
    let scratch = Scratch::new()?;
    let mut now = reviewed(
        plane,
        built_in,
        level,
        name,
        source,
        &scratch,
        Some(accepted),
    )?;
    route_of(forge, &mut now)?;
    if now.accepted() != *accepted {
        return Err(
            "What would go out is not what you reviewed, so nothing was sent. Review it again"
                .to_owned(),
        );
    }
    if let Some(first) = now.outgoing.stops.first() {
        return Err(format!("Nothing was sent: {first}"));
    }
    let fork = match &now.route {
        Route::Source | Route::NewFork { .. } => None,
        Route::Fork(fork) => Some(fork.clone()),
    };
    // A fork not made yet has no request on it.
    if !matches!(now.route, Route::NewFork { .. })
        && let Some(url) = forge.already(fork.as_ref(), &now.branch)?
    {
        return Ok(Published::Already { url });
    }
    let (commit, tree) = commit_of(&scratch, &now)?;
    let fork = match &now.route {
        Route::NewFork { path, .. } => {
            // The one place a fork is made: after a yes to a review that named it and said so.
            // A forge that made the fork and then failed to copy it says so, and the fork stays.
            let made = forge
                .make_fork()
                .map_err(|why| format!("Nothing was sent. Making your fork stopped: {why}"))?;
            if !made.path().eq_ignore_ascii_case(path) {
                return Err(format!(
                    "The forge answered your fork as {}, not {path} as reviewed, so nothing was \
                     pushed. That fork stays; review again and the publish uses it",
                    made.web_url()
                ));
            }
            Some(made)
        }
        _ => fork,
    };
    let made_now = matches!(now.route, Route::NewFork { .. });
    let kept = |why: String| match (&fork, made_now) {
        (Some(fork), true) => format!(
            "{why}. Your fork {} was made and stays; a publish again uses it",
            fork.web_url()
        ),
        _ => why,
    };
    let helper = forge.helper();
    let reach = match &fork {
        None => Reach::Source {
            remote: &source.remote,
            handed: handing(plane, source)?,
            named: source.to_string(),
        },
        Some(fork) => Reach::Fork {
            url: fork.clone_url(),
            helper: helper.as_deref(),
            named: format!("your fork {}", fork.web_url()),
        },
    };
    if made_now {
        // A forge copies a new fork in the background: git is answered once the copy is there.
        reach.answers(&scratch).map_err(kept)?;
    }
    // A branch an earlier publish pushed, whose request never opened or has since closed, is used
    // again when it holds these files, and nothing else is ever pushed over.
    if !branch_holds(&reach, &scratch, &now.branch, &tree).map_err(kept)? {
        let url = match &fork {
            None => source.remote.url.as_str(),
            Some(fork) => fork.clone_url(),
        };
        let pushed = scratch.push_new_branch(url, helper.as_deref(), &commit, &now.branch)?;
        if !pushed.ok() {
            return Ok(Published::PushRefused {
                why: kept(super::fetch::complaint(&pushed)),
            });
        }
    }
    let url = forge
        .open_draft(fork.as_ref(), &now.branch, &now.base, &now.title, &now.body)
        .map_err(|why| {
            kept(format!(
                "The branch {} was pushed, but its request could not be opened: {why}",
                now.branch
            ))
        })?;
    Ok(Published::Opened {
        url,
        branch: now.branch,
    })
}

/// **Where the branch is pushed, as git reaches it**: the source, signed in as its fetch is, or
/// the person's fork, signed in as the push is.
enum Reach<'a> {
    Source {
        remote: &'a super::Remote,
        handed: Handing,
        named: String,
    },
    Fork {
        url: &'a str,
        helper: Option<&'a str>,
        named: String,
    },
}

/// The longest a fork made just now is waited for, until git is answered there.
const FORK_ANSWERS_WITHIN: std::time::Duration = std::time::Duration::from_secs(60);

impl Reach<'_> {
    fn named(&self) -> &str {
        match self {
            Reach::Source { named, .. } | Reach::Fork { named, .. } => named,
        }
    }

    /// git's `ls-remote` of `wanted` there.
    fn list(&self, scratch: &Scratch, wanted: &str) -> Result<git::Run, String> {
        match self {
            Reach::Source { remote, handed, .. } => {
                scratch.network(remote, handed, &["ls-remote", "--", REMOTE, wanted])
            }
            Reach::Fork { url, helper, .. } => {
                scratch.network_as(*helper, &["ls-remote", "--", url, wanted])
            }
        }
    }

    /// Fetches `wanted` there to `to`.
    fn fetch(&self, scratch: &Scratch, wanted: &str, to: &str) -> Result<git::Run, String> {
        match self {
            Reach::Source { remote, handed, .. } => scratch.fetch(remote, handed, wanted, to),
            Reach::Fork { url, helper, .. } => {
                let refspec = format!("+{wanted}:{to}");
                scratch.network_as(
                    *helper,
                    &[
                        "fetch",
                        "--quiet",
                        "--no-tags",
                        "--no-recurse-submodules",
                        "--no-write-fetch-head",
                        "--depth=1",
                        "--",
                        url,
                        &refspec,
                    ],
                )
            }
        }
    }

    /// Waits until git is answered there, for at most [`FORK_ANSWERS_WITHIN`].
    fn answers(&self, scratch: &Scratch) -> Result<(), String> {
        let started = std::time::Instant::now();
        let mut pause = std::time::Duration::from_millis(500);
        loop {
            let listed = self.list(scratch, "HEAD")?;
            if listed.ok() {
                return Ok(());
            }
            if started.elapsed() > FORK_ANSWERS_WITHIN {
                return Err(format!(
                    "{} did not answer git within {} seconds, so nothing was pushed: {}",
                    self.named(),
                    FORK_ANSWERS_WITHIN.as_secs(),
                    super::fetch::complaint(&listed)
                ));
            }
            std::thread::sleep(pause);
            pause = (pause * 2).min(std::time::Duration::from_secs(8));
        }
    }
}

/// Whether `branch` is there already and holds `tree`. Holding anything else is refused, as the
/// branch is someone's work.
fn branch_holds(
    reach: &Reach<'_>,
    scratch: &Scratch,
    branch: &str,
    tree: &str,
) -> Result<bool, String> {
    let place = reach.named();
    let wanted = format!("refs/heads/{branch}");
    let listed = reach.list(scratch, &wanted)?;
    if !listed.ok() {
        return Err(format!(
            "{place} could not be asked: {}",
            super::fetch::complaint(&listed)
        ));
    }
    if listed.out.trim().is_empty() {
        return Ok(false);
    }
    let run = reach.fetch(scratch, &wanted, "refs/purlis/earlier")?;
    let held = scratch.git(&["rev-parse", "refs/purlis/earlier^{tree}"])?;
    if run.ok() && held.ok() && held.line().trim() == tree {
        return Ok(true);
    }
    Err(format!(
        "{place} already has a branch {branch} that holds something else, so nothing was sent. \
         Delete that branch, then publish again"
    ))
}

/// The commit and its tree: the source's tree at the reviewed commit, each skill's folder
/// replaced. Nothing here reaches the source.
fn commit_of(scratch: &Scratch, review: &PublishReview) -> Result<(String, String), String> {
    let failed = |doing: &str, run: &git::Run| format!("{doing}: {}", run.err.trim());
    let read = scratch.git(&["read-tree", "refs/purlis/base"])?;
    if !read.ok() {
        return Err(failed("the source could not be read", &read));
    }
    for (skill, (folder, _)) in review.outgoing.skills.iter().zip(&review.lands) {
        let held = scratch.git_raw(&["ls-files", "-z", "--", folder], Vec::new())?;
        // Mode 0 takes an entry out, and asks for no work tree where `--force-remove` does.
        let mut gone = Vec::new();
        for path in held
            .out
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            gone.extend_from_slice(format!("0 {}\t", "0".repeat(40)).as_bytes());
            gone.extend_from_slice(path);
            gone.push(0);
        }
        let removed = scratch.git_raw(&["update-index", "-z", "--index-info"], gone)?;
        if removed.code != Some(0) {
            return Err(format!(
                "a file of the source could not be replaced: {}",
                String::from_utf8_lossy(&removed.err).trim()
            ));
        }
        for file in &skill.files {
            let blob = scratch.git_raw(&["hash-object", "-w", "--stdin"], file.bytes.clone())?;
            let blob = String::from_utf8_lossy(&blob.out).trim().to_owned();
            let mode = if file.executable { "100755" } else { "100644" };
            let entry = format!("{mode},{blob},{folder}/{}", file.path());
            let added =
                scratch.git(&["update-index", "--add", "--replace", "--cacheinfo", &entry])?;
            if !added.ok() {
                return Err(failed("a file could not be added", &added));
            }
        }
    }
    for (path, bytes) in &review.carried {
        let blob = scratch.git_raw(&["hash-object", "-w", "--stdin"], bytes.clone())?;
        let blob = String::from_utf8_lossy(&blob.out).trim().to_owned();
        let entry = format!("100644,{blob},{path}");
        let added = scratch.git(&["update-index", "--add", "--replace", "--cacheinfo", &entry])?;
        if !added.ok() {
            return Err(failed("a grade file could not be added", &added));
        }
    }
    let tree = scratch.git(&["write-tree", "--missing-ok"])?;
    if !tree.ok() {
        return Err(failed("the commit's files could not be written", &tree));
    }
    let tree = tree.line().trim().to_owned();
    let base_tree = scratch.git(&["rev-parse", "refs/purlis/base^{tree}"])?;
    if base_tree.line().trim() == tree {
        return Err("The source already holds exactly these files, so nothing was sent".to_owned());
    }
    let name = format!("user.name={}", review.author.0);
    let mail = format!("user.email={}", review.author.1);
    let made = scratch.git(&[
        "-c",
        &name,
        "-c",
        &mail,
        "commit-tree",
        &tree,
        "-p",
        "refs/purlis/base",
        "-m",
        &review.title,
    ])?;
    if !made.ok() {
        return Err(failed("the commit could not be made", &made));
    }
    Ok((made.line().trim().to_owned(), tree))
}

#[cfg(test)]
mod tests;

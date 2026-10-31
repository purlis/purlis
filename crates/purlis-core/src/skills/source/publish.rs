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
//! tree at the reviewed commit, with each skill's folder replaced. It goes to a new branch of the
//! source itself, named by the digest of what it holds, by the one refspec, never forced. A person
//! who may not push to the source is told so; nothing is forked for them (#4342).
//!
//! **Grades go as a summary** in the request's description, and never in the skill's folder
//! (decision 21): when, which harness, which grader, how it scored and how it ended. Never a
//! chat's text, a path, a cost, a profile or a device. Every word of a row that is the record's
//! own is held to plain characters, as the row may be a teammate's.

use std::path::Path;

use purlis_skills::tree;

use super::fetch::{REMOTE, Scratch, handing};
use super::outgoing::{self, OutSkill, Outgoing, Purpose, Stop};
use super::{Commit, Source};
use crate::forge::Caller;
use crate::forge::pr::Repo;
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
    /// The page of the request already on `branch`, in any state.
    fn already(&self, branch: &str) -> Result<Option<String>, String>;
    /// Opens the draft request, and answers its page.
    fn open_draft(&self, head: &str, base: &str, title: &str, body: &str)
    -> Result<String, String>;
}

/// The person's own forge, through their purlis sign-in.
pub struct OnTheForge {
    repo: Repo,
    caller: Caller,
}

impl OnTheForge {
    /// The forge `source` is on, refused with a sentence where it is on none this plane knows.
    pub fn of(plane: &Path, source: &Source) -> Result<OnTheForge, String> {
        let url = &source.remote.url;
        let not_a_forge = || {
            format!(
                "{url} is not on a GitHub or GitLab host this project knows, so no request can \
                 be opened there. Export the skill instead, or add the host as a [[forge]] in \
                 charter.toml"
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
        })
    }
}

impl PublishForge for OnTheForge {
    fn helper(&self) -> Option<String> {
        Some(crate::forge::helper_for(&self.repo.forge))
    }

    fn already(&self, branch: &str) -> Result<Option<String>, String> {
        let found = self
            .repo
            .backend()
            .by_head(&self.caller, &self.repo.path, branch)
            .map_err(|why| why.to_string())?;
        Ok(found.map(|request| request.url))
    }

    fn open_draft(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        let pr = self
            .repo
            .backend()
            .open_draft(&self.caller, &self.repo.path, head, base, title, body)
            .map_err(|why| why.to_string())?;
        Ok(pr.url)
    }
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
    /// The request's description, the grades among it, whole.
    pub body: String,
}

impl PublishReview {
    /// Whether anything stops it.
    pub fn stopped(&self) -> bool {
        !self.outgoing.stops.is_empty()
    }
}

/// **What a publish of `name` at `level` to `source` would send.** Writes nothing anywhere.
pub fn review(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    source: &Source,
) -> Result<PublishReview, String> {
    let scratch = Scratch::new()?;
    reviewed(plane, built_in, level, name, source, &scratch, None)
}

/// The review, with the source fetched into `scratch`, at the commit `at` where one is held to.
fn reviewed(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    source: &Source,
    scratch: &Scratch,
    at: Option<&PublishReview>,
) -> Result<PublishReview, String> {
    source.check()?;
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
    let digest = outgoing.digest();
    let title = title_of(&outgoing.skills, lands.first().is_some_and(|land| land.1));
    let body = body_of(plane, &outgoing.skills, &lands);
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
        body,
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

/// One row of a skill's grades.
fn grade_row(graded: &Graded, now: Option<&str>) -> String {
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
    format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} |",
        plain(&date),
        if now == Some(graded.tree.as_str()) {
            "this one"
        } else {
            "earlier"
        },
        plain(graded.harness.as_deref().unwrap_or("unknown")),
        plain(&graded.report.grader),
        percent(arm(purlis_skills::grade::Arm::With)),
        percent(arm(purlis_skills::grade::Arm::Without)),
        graded.runs,
        grades::ended_said(graded.report.ended),
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

/// **The request's description**: what goes, then each skill's grades.
fn body_of(plane: &Path, skills: &[OutSkill], lands: &[(String, bool)]) -> String {
    let mut out = String::from("Published with purlis. Review it as you would any change.\n\n");
    out.push_str("## Files\n\n");
    for (skill, (folder, _)) in skills.iter().zip(lands) {
        out.push_str(&format!("- `{folder}/`: {} files\n", skill.files.len()));
    }
    out.push_str("\n## Grades\n");
    for skill in skills {
        let graded = grades::read_followed(plane, &skill.level, &skill.name).grades;
        out.push_str(&format!("\n### {}\n\n", plain(&skill.name)));
        if graded.is_empty() {
            out.push_str("Not graded.\n");
            continue;
        }
        let now = skill
            .level
            .named(plane, &skill.name)
            .and_then(|folder| tree_now(plane, &folder));
        out.push_str(
            "| When | Version | Harness | Grader | With the skill | Without | Runs | Ended |\n\
             | --- | --- | --- | --- | --- | --- | --- | --- |\n",
        );
        for one in graded.iter().take(MOST_GRADES) {
            out.push_str(&grade_row(one, now.as_deref()));
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
}

/// **Sends what `accepted` showed**, refused whole where any of it moved since or anything stops
/// it. Nothing is written on the source before every check has passed.
pub fn publish(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    source: &Source,
    accepted: &PublishReview,
    forge: &dyn PublishForge,
) -> Result<Published, String> {
    let scratch = Scratch::new()?;
    let now = reviewed(
        plane,
        built_in,
        level,
        name,
        source,
        &scratch,
        Some(accepted),
    )?;
    if now != *accepted {
        return Err(
            "What would go out is not what you reviewed, so nothing was sent. Review it again"
                .to_owned(),
        );
    }
    if let Some(first) = now.outgoing.stops.first() {
        return Err(format!("Nothing was sent: {first}"));
    }
    if let Some(url) = forge.already(&now.branch)? {
        return Ok(Published::Already { url });
    }
    let commit = commit_of(plane, &scratch, &now)?;
    let helper = forge.helper();
    let pushed =
        scratch.push_new_branch(&source.remote.url, helper.as_deref(), &commit, &now.branch)?;
    if !pushed.ok() {
        return Err(format!(
            "Nothing was opened: git could not push to {source}, which takes write access to it: \
             {}",
            super::fetch::complaint(&pushed)
        ));
    }
    let url = forge
        .open_draft(&now.branch, &now.base, &now.title, &now.body)
        .map_err(|why| {
            format!(
                "The branch {} was pushed, but its request could not be opened: {why}",
                now.branch
            )
        })?;
    Ok(Published::Opened {
        url,
        branch: now.branch,
    })
}

/// The commit: the source's tree at the reviewed commit, each skill's folder replaced.
fn commit_of(plane: &Path, scratch: &Scratch, review: &PublishReview) -> Result<String, String> {
    let failed = |doing: &str, run: &git::Run| format!("{doing}: {}", run.err.trim());
    let identity = |key: &str| {
        git::run(plane, &["config", key], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let (Some(author), Some(email)) = (identity("user.name"), identity("user.email")) else {
        return Err(
            "git has no user.name and user.email for this project, so no commit can be made. Set \
             them, then publish again"
                .to_owned(),
        );
    };
    let read = scratch.git(&["read-tree", "refs/purlis/base"])?;
    if !read.ok() {
        return Err(failed("the source could not be read", &read));
    }
    for (skill, (folder, _)) in review.outgoing.skills.iter().zip(&review.lands) {
        let held = scratch.git(&["ls-files", "--", folder])?;
        for path in held.out.lines() {
            let gone = scratch.git(&["update-index", "--force-remove", "--", path])?;
            if !gone.ok() {
                return Err(failed("a file of the source could not be replaced", &gone));
            }
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
    let tree = scratch.git(&["write-tree", "--missing-ok"])?;
    if !tree.ok() {
        return Err(failed("the commit's files could not be written", &tree));
    }
    let tree = tree.line().trim().to_owned();
    let base_tree = scratch.git(&["rev-parse", "refs/purlis/base^{tree}"])?;
    if base_tree.line().trim() == tree {
        return Err("The source already holds exactly these files, so nothing was sent".to_owned());
    }
    let name = format!("user.name={author}");
    let mail = format!("user.email={email}");
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
    Ok(made.line().trim().to_owned())
}

#[cfg(test)]
mod tests;

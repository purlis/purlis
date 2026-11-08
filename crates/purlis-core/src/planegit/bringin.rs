// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Bring a moved target into a plane's saves** (#5338): a request mode's plane whose target
//! moved while its request was open. Without this nothing moves the plane until the request
//! merges, and a request that conflicts never merges.
//!
//! # A merge, made off to the side
//!
//! The plane takes the target as one merge commit, for three reasons:
//!
//! - **It stops once.** A rebase replays commit by commit, and the `## Sessions` line of
//!   `workspace.md` conflicts again on every replay of a line of commits that each wrote a
//!   record: one stop per commit.
//! - **It does not rewrite what was pushed.** The commit the request carries stays in the
//!   plane's history, so [`super::prsave::settle`] still finds the request its own, and the
//!   save branch's next push is a fast-forward.
//! - **It merges on the forge.** A merge commit is mergeable once the target is in it. GitHub's
//!   REBASE method may refuse it, and asks `canBeRebased` for that, which the forge's merge
//!   reads to take the next method the repository allows.
//!
//! The merge is computed with `git merge-tree --write-tree`, which touches neither the working
//! tree, the index nor any ref, so **a conflict in what a person wrote leaves everything as it
//! was**: no markers staged, no merge left stopped. Only a merge with nothing left to settle
//! becomes a commit, and the plane moves onto it with `reset --keep`, as a merged request's move
//! does: that keeps uncommitted work and refuses rather than overwrite a file holding some.
//!
//! # What purlis generates is rebuilt, not merged
//!
//! Three kinds of file hold nothing a person wrote, whole or in part:
//!
//! - each place's `sessions/index.md`, from its records;
//! - a workspace's `todos/MEMORY.md`, from its todos;
//! - the `## Sessions` line of a workspace's `workspace.md`, from its records.
//!
//! Where both sides changed a place's records or todos, these are written from the merged
//! tree's own sources, conflicted or not, because git's line merge would stitch them out of
//! order. The rest of `workspace.md` is a person's: it is merged with the pointer set aside, and
//! a conflict left in it is theirs. The managed block of `.gitattributes` is set aside the same
//! way, and the block this build writes is put back.
//!
//! # Under a chat's sandbox
//!
//! A move that cannot finish does not start ([`super::sandbox_would_stop`]). The plane waits,
//! and the window's own look makes it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{WRITE, count, git, gitstate, is_object_name, sandbox_would_stop, tail};
use crate::active::Place;
use crate::sessionrecord;

/// What the commit that brings a target in says, past the target's name.
const SUBJECT: &str = "into this machine's saves";

/// What stands in for the pointer while a `workspace.md` is merged.
const SET_ASIDE: &str = "purlis sets this line aside while it merges";

/// What stands in for the managed block while a `.gitattributes` is merged, so it keeps its place.
const RULES_SET_ASIDE: &str = "# purlis sets its merge rules aside while it merges";

/// When [`bring_in`] makes a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum When<'a> {
    /// Only where the forge's own merge would stop or come out wrong: a conflict, or a file
    /// purlis generates that the line merge stitches. A request that merges as it is is left, so
    /// a moving target does not add a merge commit, and a new head, to every save (a repository
    /// that keeps its history linear refuses those).
    Needed,
    /// Whenever the target moved: the forge says the request cannot merge as it is.
    Moved,
    /// The request at this commit merged, and what was committed since is carried onto the
    /// target as one commit, with that commit as the merge base and the target its one parent.
    /// A request merged by rebase or squash is in the target as copies, and a merge with the
    /// plane's history would bring the originals back beside them.
    MergedAt(&'a str),
}

/// What [`bring_in`] came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BroughtIn {
    /// The remote's target has nothing this plane lacks.
    Nothing,
    /// The target is in the plane now: this many of its commits.
    Moved(u32),
    /// Not brought in yet, and why as a sentence. The next look or save tries again.
    Waiting(String),
    /// A conflict in what a person wrote, in these files. Nothing was changed.
    Conflict(Vec<String>),
    /// git could not do it, in its words. Nothing was changed.
    Failed(String),
}

/// **Bring the remote-tracking target into the plane**, as far as it can go by itself. `target`
/// is the branch, already fetched; the plane is on it.
pub(super) fn bring_in(root: &Path, target: &str, sign: bool, when: When) -> BroughtIn {
    let theirs = format!("refs/remotes/origin/{target}");
    let behind = count(root, &format!("HEAD..{theirs}")).unwrap_or(0);
    if behind == 0 {
        return BroughtIn::Nothing;
    }
    if let Some(stopped) = gitstate::stopped(root) {
        return BroughtIn::Waiting(stopped.why());
    }
    let onto = if count(root, &format!("{theirs}..HEAD")) == Some(0) {
        // Nothing of its own: there is nothing to merge, and no commit to make.
        theirs.clone()
    } else {
        let base = match when {
            When::MergedAt(head) => Some(head),
            When::Needed | When::Moved => None,
        };
        let plan = match plan(root, &theirs, base) {
            Ok(plan) => plan,
            Err(why) => return BroughtIn::Failed(why),
        };
        if when == When::Needed && !plan.needed(root) {
            return BroughtIn::Nothing;
        }
        if !plan.person.is_empty() {
            return BroughtIn::Conflict(plan.person);
        }
        let tree = match tree_of(root, &plan) {
            Ok(tree) => tree,
            Err(why) => return BroughtIn::Failed(why),
        };
        let same = git::run(
            root,
            &["rev-parse", &format!("{theirs}^{{tree}}")],
            git::READ,
        )
        .ok()
        .filter(git::Run::ok)
        .is_some_and(|run| run.line().trim() == tree);
        if base.is_some() && same {
            // Everything carried is in the target already: nothing to commit.
            theirs.clone()
        } else {
            match commit(root, &tree, &theirs, target, sign, base) {
                Ok(commit) => commit,
                Err(why) => return BroughtIn::Failed(why),
            }
        }
    };
    let stops = sandbox_would_stop(root, &onto);
    if !stops.is_empty() {
        return BroughtIn::Waiting(format!(
            "{target} comes in from the window, not from this chat: {}",
            super::said_sandbox_stops(&stops)
        ));
    }
    // Untimed: a reset checks out a tree, and a killed one leaves it half-written.
    match git::run_untimed(root, &["reset", "-q", "--keep", &onto]) {
        Ok(run) if run.ok() => BroughtIn::Moved(behind),
        Ok(run) => BroughtIn::Waiting(format!(
            "{target} comes in once nothing here is in the way: {}",
            tail(&run)
        )),
        Err(unavailable) => BroughtIn::Waiting(format!(
            "bringing {target} in did not finish: {unavailable}"
        )),
    }
}

/// A merge of HEAD and a target, worked out and nothing changed yet.
pub(super) struct Plan {
    /// The side merged into: HEAD, or the commit [`plan_between`] was given.
    ours: String,
    /// The tree git's merge wrote, conflict markers included in any file it could not settle.
    tree: String,
    /// Whether git's merge left any file in conflict.
    conflicted: bool,
    /// The files to write over it, as plane-relative paths.
    pub(super) settled: BTreeMap<String, String>,
    /// The files left in conflict that a person wrote, sorted.
    pub(super) person: Vec<String>,
    /// Of those, the ones purlis generates a part of: each with that part settled and markers
    /// only around what the person's sides wrote. What a stopped merge shows them.
    pub(super) marked: BTreeMap<String, String>,
}

/// A file purlis generates, whole or in part.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Generated {
    Sessions(Option<String>),
    Todos(String),
    Charter(String),
    Rules,
}

/// Which generated file `path` is, if it is one.
fn generated(path: &str) -> Option<Generated> {
    if path == ".gitattributes" {
        return Some(Generated::Rules);
    }
    if path == format!("{}/{}", sessionrecord::DIR, sessionrecord::INDEX) {
        return Some(Generated::Sessions(None));
    }
    let rest = path.strip_prefix("workspaces/")?;
    let (ws, file) = rest.split_once('/')?;
    if !crate::contain::workspace_name_ok(ws) {
        return None;
    }
    let ws = ws.to_owned();
    if file == format!("{}/{}", sessionrecord::DIR, sessionrecord::INDEX) {
        Some(Generated::Sessions(Some(ws)))
    } else if file == format!("todos/{}", crate::memstore::INDEX) {
        Some(Generated::Todos(ws))
    } else if file == "workspace.md" {
        Some(Generated::Charter(ws))
    } else {
        None
    }
}

/// The places `paths` say records or todos changed in, as what is generated from them.
fn sources_in(paths: &BTreeSet<String>) -> BTreeSet<Generated> {
    let mut out = BTreeSet::new();
    let sessions = format!("{}/", sessionrecord::DIR);
    for path in paths {
        if path.starts_with(&sessions) {
            out.insert(Generated::Sessions(None));
        } else if let Some((ws, file)) = path
            .strip_prefix("workspaces/")
            .and_then(|rest| rest.split_once('/'))
            && crate::contain::workspace_name_ok(ws)
        {
            if file.starts_with(&sessions) {
                out.insert(Generated::Sessions(Some(ws.to_owned())));
            } else if file.starts_with("todos/") {
                out.insert(Generated::Todos(ws.to_owned()));
            }
        }
    }
    out
}

impl Plan {
    /// Whether the forge's own merge would stop or come out wrong: git's merge left a conflict,
    /// or what purlis generates, written from the merged sources, is not what it wrote.
    fn needed(&self, root: &Path) -> bool {
        self.conflicted
            || self
                .settled
                .iter()
                .any(|(path, text)| blob(root, &self.tree, path).as_deref() != Some(text.as_str()))
    }
}

/// Work out the merge of HEAD and `theirs`, from `base` where it is given and from where the two
/// histories meet otherwise: nothing in the plane changes (objects aside).
pub(super) fn plan(root: &Path, theirs: &str, base: Option<&str>) -> Result<Plan, String> {
    plan_between(root, "HEAD", theirs, base)
}

/// [`plan`], of `ours` in place of HEAD: a merge of two commits, neither of them the root's.
pub(super) fn plan_between(
    root: &Path,
    ours: &str,
    theirs: &str,
    base: Option<&str>,
) -> Result<Plan, String> {
    let given = base.map(|base| format!("--merge-base={base}"));
    let mut args = vec![
        "merge-tree",
        "--write-tree",
        "-z",
        "--name-only",
        "--no-messages",
    ];
    args.extend(given.as_deref());
    args.extend([ours, theirs]);
    let merged = git::run(root, &args, WRITE).map_err(|unavailable| unavailable.to_string())?;
    // 0 is clean and 1 is conflicts: anything else is git refusing, with no tree.
    if !matches!(merged.code, Some(0 | 1)) {
        return Err(tail(&merged));
    }
    let mut fields = merged.out.split('\0').filter(|field| !field.is_empty());
    let tree = fields
        .next()
        .filter(|tree| is_object_name(tree))
        .ok_or_else(|| tail(&merged))?
        .to_owned();
    let conflicted: BTreeSet<String> = fields.map(str::to_owned).collect();

    let base = match base {
        Some(base) => base.to_owned(),
        None => git::run(root, &["merge-base", ours, theirs], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_owned())
            .ok_or("the two histories have no commit in common")?,
    };
    let ours_touched = sources_in(&changed(root, &base, ours)?);
    let theirs_touched = sources_in(&changed(root, &base, theirs)?);
    let mut rebuild: BTreeSet<Generated> = ours_touched
        .intersection(&theirs_touched)
        .cloned()
        .collect();

    let mut plan = Plan {
        ours: ours.to_owned(),
        tree,
        conflicted: !conflicted.is_empty(),
        settled: BTreeMap::new(),
        person: Vec::new(),
        marked: BTreeMap::new(),
    };
    for path in &conflicted {
        match generated(path) {
            Some(Generated::Charter(ws)) => {
                match merged_apart(root, ours, path, &base, theirs, |text| {
                    crate::mdsection::replace(text, "Sessions", SET_ASIDE)
                })? {
                    Apart::Clean(text) => {
                        plan.settled.insert(path.clone(), text);
                    }
                    Apart::Marked(text) => {
                        plan.marked.insert(path.clone(), text);
                        plan.person.push(path.clone());
                    }
                    Apart::NoFile => plan.person.push(path.clone()),
                }
                rebuild.insert(Generated::Sessions(Some(ws)));
            }
            Some(Generated::Rules) => {
                match merged_apart(root, ours, path, &base, theirs, |text| {
                    crate::scaffold::rules_set_aside(text, RULES_SET_ASIDE)
                })? {
                    Apart::Clean(text) => {
                        // A rule the remote's block holds that this build does not write stays,
                        // as a newer build's would: the remote's is the later word. The block
                        // keeps the place it had.
                        let kept = blob(root, theirs, path)
                            .map(|text| crate::scaffold::rules_in(&text))
                            .unwrap_or_default();
                        let with =
                            crate::scaffold::rules_put_back(&text, RULES_SET_ASIDE, root, &kept);
                        plan.settled.insert(path.clone(), with);
                    }
                    Apart::Marked(_) | Apart::NoFile => plan.person.push(path.clone()),
                }
            }
            Some(from_sources) => {
                rebuild.insert(from_sources);
            }
            None => plan.person.push(path.clone()),
        }
    }
    // With a person's conflict too: Resolve settles the rest around it.
    for what in rebuild {
        rebuilt(root, &mut plan, &what, theirs)?;
    }
    Ok(plan)
}

/// The paths that differ between `from` and `to`.
fn changed(root: &Path, from: &str, to: &str) -> Result<BTreeSet<String>, String> {
    let run = git::run(
        root,
        &[
            "diff-tree",
            "-r",
            "--name-only",
            "--no-renames",
            "-z",
            from,
            to,
        ],
        git::READ,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    if !run.ok() {
        return Err(tail(&run));
    }
    Ok(run
        .out
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

/// A blob's text at `rev:path`, or `None` where there is none, or it is not text.
fn blob(root: &Path, rev: &str, path: &str) -> Option<String> {
    let run = git::run(
        root,
        &["cat-file", "blob", &format!("{rev}:{path}")],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)?;
    // `Run` reads its output lossily: a file that was not text is not one to rewrite.
    (!run.out.contains('\u{fffd}')).then_some(run.out)
}

/// What merging a file with purlis's part set aside left.
enum Apart {
    /// Nothing in conflict.
    Clean(String),
    /// A conflict in what a person wrote, marked.
    Marked(String),
    /// A side has no such file, or it is not text: there is nothing to merge line by line.
    NoFile,
}

/// The three sides of `path` merged with `apart` taken to each first, so what purlis generates
/// in it cannot conflict.
fn merged_apart(
    root: &Path,
    ours: &str,
    path: &str,
    base: &str,
    theirs: &str,
    apart: impl Fn(&str) -> String,
) -> Result<Apart, String> {
    let (Some(ours), Some(old), Some(new)) = (
        blob(root, ours, path),
        blob(root, base, path),
        blob(root, theirs, path),
    ) else {
        return Ok(Apart::NoFile);
    };
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let write = |name: &str, text: &str| -> Result<String, String> {
        let file = dir.path().join(name);
        std::fs::write(&file, apart(text)).map_err(|e| e.to_string())?;
        Ok(file.to_string_lossy().into_owned())
    };
    let (ours, old, new) = (
        write("ours", &ours)?,
        write("base", &old)?,
        write("theirs", &new)?,
    );
    // Labelled as a person reads them, not by the temporary files' paths.
    let target = theirs.strip_prefix("refs/remotes/").unwrap_or(theirs);
    let run = git::run(
        root,
        &[
            "merge-file",
            "-p",
            "-L",
            "HEAD",
            "-L",
            "merge base",
            "-L",
            target,
            &ours,
            &old,
            &new,
        ],
        WRITE,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    // Its exit code is the number of conflicts left; past 127 it is git failing.
    Ok(match run.code {
        Some(0) => Apart::Clean(run.out),
        Some(1..=127) => Apart::Marked(run.out),
        _ => return Err(tail(&run)),
    })
}

/// A file's text as the merge has it: what was settled, else the merged tree's.
fn current(root: &Path, plan: &Plan, path: &str) -> Option<String> {
    plan.settled
        .get(path)
        .cloned()
        .or_else(|| blob(root, &plan.tree, path))
}

/// The regular files directly in `dir` of `tree`, with their text: what a directory listing of
/// the merged tree gives, for a file that is text.
fn files_in(root: &Path, tree: &str, dir: &str) -> Vec<(String, String)> {
    let Some(listing) = git::run(
        root,
        &["ls-tree", "-z", tree, &format!("{dir}/")],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok) else {
        return Vec::new();
    };
    listing
        .out
        .split('\0')
        .filter_map(|row| {
            let (meta, path) = row.split_once('\t')?;
            let (mode, rest) = meta.split_once(' ')?;
            let (kind, _) = rest.split_once(' ')?;
            if kind != "blob" || mode == "120000" {
                return None;
            }
            let name = path.rsplit('/').next()?.to_owned();
            Some((name, path.to_owned()))
        })
        .filter_map(|(name, path)| Some((name, blob(root, tree, &path)?)))
        .collect()
}

/// Write what `what` generates into `plan`, from the merged tree's own sources.
fn rebuilt(root: &Path, plan: &mut Plan, what: &Generated, theirs: &str) -> Result<(), String> {
    match what {
        Generated::Sessions(ws) => {
            let place = ws.clone().map_or(Place::PlaneRoot, Place::Workspace);
            let dir = match &place {
                Place::PlaneRoot => sessionrecord::DIR.to_owned(),
                Place::Workspace(ws) => format!("workspaces/{ws}/{}", sessionrecord::DIR),
            };
            let records = sessionrecord::listed_from(&place, files_in(root, &plan.tree, &dir));
            plan.settled.insert(
                format!("{dir}/{}", sessionrecord::INDEX),
                sessionrecord::index_text(&place, &records),
            );
            if let Place::Workspace(ws) = &place {
                let path = format!("workspaces/{ws}/workspace.md");
                let body = sessionrecord::pointer(root, &records);
                if let Some(text) = plan.marked.get_mut(&path) {
                    *text = crate::mdsection::replace(text, "Sessions", &body);
                } else if !plan.person.contains(&path)
                    && let Some(text) = current(root, plan, &path)
                {
                    plan.settled
                        .insert(path, crate::mdsection::replace(&text, "Sessions", &body));
                }
            }
        }
        Generated::Todos(ws) => {
            let dir = format!("workspaces/{ws}/todos");
            let index = format!("{dir}/{}", crate::memstore::INDEX);
            let ours = blob(root, &plan.ours, &index).unwrap_or_default();
            let new = blob(root, theirs, &index).unwrap_or_default();
            let todos: Vec<(String, String)> = files_in(root, &plan.tree, &dir)
                .into_iter()
                .filter(|(name, _)| name != crate::memstore::INDEX && name.ends_with(".md"))
                .collect();
            let header = crate::workspaces::TODOS_HEADER.replace("{name}", ws);
            plan.settled.insert(
                index,
                crate::memstore::index_rebuilt(&header, &[&ours, &new], &todos),
            );
        }
        Generated::Charter(_) | Generated::Rules => {}
    }
    Ok(())
}

/// The tree `plan` comes to: git's, with what was settled written over it. Built in an index of
/// its own, so what the plane has staged is never touched.
pub(super) fn tree_of(root: &Path, plan: &Plan) -> Result<String, String> {
    if plan.settled.is_empty() {
        return Ok(plan.tree.clone());
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let index = dir.path().join("index");
    let in_index = |args: &[&str]| -> Result<git::Run, String> {
        let run = git::run_with_index(root, &index, args)
            .map_err(|unavailable| unavailable.to_string())?;
        if run.ok() { Ok(run) } else { Err(tail(&run)) }
    };
    in_index(&["read-tree", &plan.tree])?;
    let held = dir.path().join("blob");
    for (path, text) in &plan.settled {
        std::fs::write(&held, text).map_err(|e| e.to_string())?;
        let held = held.to_string_lossy();
        let hashed = git::run(
            root,
            &["hash-object", "-w", "--no-filters", "--", &held],
            WRITE,
        )
        .map_err(|unavailable| unavailable.to_string())?;
        if !hashed.ok() {
            return Err(tail(&hashed));
        }
        let info = format!("100644,{},{path}", hashed.line().trim());
        in_index(&["update-index", "--add", "--cacheinfo", &info])?;
    }
    Ok(in_index(&["write-tree"])?.line().trim().to_owned())
}

/// The commit of `tree`, which a plan came to: a merge, with HEAD and `theirs` as its parents, or, carried from
/// `carried` ([`When::MergedAt`]), what HEAD added since that commit on top of `theirs` alone.
/// One commit carried keeps its own message, so the next request reads as it did; several are
/// said as one carry.
fn commit(
    root: &Path,
    tree: &str,
    theirs: &str,
    target: &str,
    sign: bool,
    carried: Option<&str>,
) -> Result<String, String> {
    let message = match carried {
        None => format!("Bring {target} {SUBJECT}"),
        Some(base) => carried_message(root, base, theirs, target),
    };
    let parents: Vec<&str> = if carried.is_some() {
        vec![theirs]
    } else {
        vec!["HEAD", theirs]
    };
    commit_tree(root, tree, &parents, &message, sign)
}

/// What a carry says: the one commit of `base..HEAD` whose work is not already in `theirs` in
/// its own words, so the next request reads as it did, or one line for several. A commit the
/// target already holds every file of as it wrote them (a request that merged it by squash) is
/// not said again, as a rebase would have dropped it.
fn carried_message(root: &Path, base: &str, theirs: &str, target: &str) -> String {
    let commits: Vec<String> = git::run(
        root,
        &["rev-list", "--reverse", &format!("{base}..HEAD")],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    .map(|run| run.out.lines().map(str::to_owned).collect())
    .unwrap_or_default();
    let new: Vec<&String> = commits
        .iter()
        .filter(|commit| !already_in(root, commit, theirs))
        .collect();
    match new.as_slice() {
        [one] => git::run(root, &["log", "-1", "--format=%B", one], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.out.trim_end().to_owned())
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| format!("Carry this machine's newer save onto {target}")),
        _ => format!("Carry this machine's newer saves onto {target}"),
    }
}

/// Whether `theirs` holds every file `commit` changed as `commit` left it.
fn already_in(root: &Path, commit: &str, theirs: &str) -> bool {
    let Some(touched) = git::run(
        root,
        &[
            "diff-tree",
            "-r",
            "--root",
            "--no-commit-id",
            "--no-renames",
            "--name-only",
            "-z",
            commit,
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok) else {
        return false;
    };
    let mut args = vec![
        "--literal-pathspecs",
        "diff",
        "--quiet",
        commit,
        theirs,
        "--",
    ];
    args.extend(touched.out.split('\0').filter(|path| !path.is_empty()));
    git::run(root, &args, git::READ).is_ok_and(|run| run.code == Some(0))
}

/// `git commit-tree`: a commit of `tree` with `parents` and `message`, made off to the side, no
/// ref moved and nothing in the work tree touched.
pub(super) fn commit_tree(
    root: &Path,
    tree: &str,
    parents: &[&str],
    message: &str,
    sign: bool,
) -> Result<String, String> {
    let mut args = vec!["commit-tree", tree];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    args.extend(["-m", message]);
    if sign {
        args.push("-S");
    }
    let made = git::run(root, &args, WRITE).map_err(|unavailable| unavailable.to_string())?;
    if !made.ok() {
        return Err(tail(&made));
    }
    Ok(made.line().trim().to_owned())
}

/// The sentence a person's conflict is said in, for a save and the record.
pub(super) fn said_conflict(target: &str, files: &[String]) -> String {
    format!(
        "{target} on the remote changed the same lines as this machine in {}. Nothing was \
         changed here: press Resolve in the Saving view to settle {}",
        files.join(", "),
        if files.len() == 1 { "it" } else { "them" }
    )
}

#[cfg(test)]
mod tests;

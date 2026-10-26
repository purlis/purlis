// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Resolve a save's conflict from the window** (qw382): the person's own press runs the
//! rebase a save's push retry could not finish, and git's part of settling it, while a chat
//! may edit the files it stopped in.
//!
//! A chat may not run git that rewrites the project root's tree (the A3c guard,
//! [`crate::rootupdate`]), and that stays so. Here the window runs every git command, under no
//! chat's sandbox, each one the person's press:
//!
//! - [`start`] fetches the target branch from the remote recorded for the project and replays
//!   the project's own commits onto it. Where git stops on a conflict, the rebase is left
//!   stopped, and the answer names the files.
//! - [`carry_on`] is *Continue*, [`skip`] is *Skip* (a commit the settling left empty), and
//!   [`abort`] is *Abort*, which puts the project back where it was.
//! - [`push_after`] is what follows a replay that finished: a push of what is committed, and
//!   nothing else.
//!
//! **What git stopped with is kept, in this process, for each stop** ([`Stop`]): the commit it
//! stopped on, the files in conflict, which of them git could not mark ([`Conflicts::by_hand`]),
//! and every other change git had staged. Continue carries on only from that: the files in
//! conflict, settled, and everything else exactly as git staged it. The commands are fixed
//! here, and file names reach git after `--` with literal pathspecs.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::{
    Claim, PushResult, Rebased, WRITE, changed_paths, count, fetch_branch, forget_replayed_push,
    gpgsign, ondisk, rebase_in_progress, rebase_onto, tail,
};
use crate::counts::{counted, plural};
use crate::gitstate;
use crate::repocmd::{Say, Sink};
use crate::worktree::git;

/// How a press of Resolve, Continue, Skip or Abort ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// The project's own commits are on the remote's now. What is left is the push
    /// ([`push_after`]).
    Replayed,
    /// git stopped on a conflict in these files. The rebase is left stopped for the person.
    Stopped(Vec<String>),
    /// The commit being replayed has nothing left to commit once settled: Skip drops it.
    Empty,
    /// The rebase was undone: the project is where it was before Resolve.
    Aborted,
    /// Nothing was changed, and why, as a sentence for the person.
    Refused(String),
}

/// The files of a stop, by who may settle them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conflicts {
    /// Files git marked the conflict in, still unsettled: a person's or a chat's to edit.
    pub marked: Vec<String>,
    /// Files git could not mark the conflict in, still unsettled: a binary file, a file one
    /// side deleted, a link, a submodule, or a file that is gone. The person settles each in a
    /// terminal, by choosing a side and staging it.
    pub by_hand: Vec<String>,
}

/// What git stopped with, as the window saw it at the stop.
#[derive(Debug, Clone)]
struct Stop {
    /// The commit being replayed.
    at: String,
    /// The files git left in conflict.
    conflicted: BTreeSet<String>,
    /// The files of those that git could not mark.
    by_hand: BTreeSet<String>,
    /// Every other change git had staged, against HEAD: each path's mode and object.
    staged: BTreeMap<String, (String, String)>,
}

/// The stops the window is holding, by the project's root.
static STOPS: Mutex<Option<HashMap<PathBuf, Stop>>> = Mutex::new(None);

/// The most of a file read for conflict markers.
const MOST_READ: u64 = 16 * 1024 * 1024;

/// What a press says while a save or a fetch holds the project.
const BUSY: &str = "A save of this project is running. Wait for it to finish, then try again.";

/// What a press says where this window holds no stop for the rebase git is stopped in.
const NOT_HELD: &str = "This window did not see where git stopped (it was restarted, or the \
     rebase was started elsewhere), so it carries nothing on. Carry on in a terminal here, which \
     keeps what is settled: `git add -- <file>` for each settled file, then `git rebase \
     --continue`. Abort instead throws away the edits already made and puts the project back \
     where it was, and Resolve then starts again.";

/// **Resolve**: fetch the target branch and replay the project's own commits onto it, stopping
/// where git stops. For the person's own press in the window, and nothing else.
pub fn start(root: &Path, say: Sink) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    let plane = crate::planesave::Settings::read(root).plane;
    if let Some(stopped) = gitstate::stopped(root) {
        return Settled::Refused(format!(
            "Nothing was started: {}. Continue or Abort it first.",
            stopped.why()
        ));
    }
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    if here.is_empty() || here == "HEAD" {
        return Settled::Refused("Nothing was started: the project is on no branch.".into());
    }
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    if here != branch {
        return Settled::Refused(format!(
            "Nothing was started: the project is on `{here}`, and it saves into `{branch}`."
        ));
    }
    let changed = changed_paths(root);
    if !changed.is_empty() {
        return Settled::Refused(format!(
            "Save first: {} here {} not saved, and a rebase never moves \
             unsaved work.",
            counted(changed.len(), "changed file", "changed files"),
            plural(changed.len(), "is", "are")
        ));
    }
    say(Say::Info(
        format!("Fetching `{branch}` from the remote …").into(),
    ));
    if let Err(why) = fetch_branch(root, &branch) {
        return Settled::Refused(format!(
            "Nothing was started: the remote could not be fetched: {}.",
            why.trim_end_matches('.')
        ));
    }
    let theirs = format!("refs/remotes/origin/{branch}");
    let Some(before) = rev(root, "HEAD") else {
        return Settled::Refused("Nothing was started: the project has no commit.".into());
    };
    if count(root, &format!("HEAD..{theirs}")) == Some(0) {
        forget_replayed_push(root);
        return Settled::Replayed;
    }
    say(Say::Info("Replaying this project's own commits …".into()));
    match rebase_onto(root, &theirs, plane.sign.value, WRITE) {
        Rebased::Replayed => {
            forget_replayed_push(root);
            Settled::Replayed
        }
        Rebased::Conflict(files) => {
            hold_stop(root);
            Settled::Stopped(files)
        }
        Rebased::Held(files) => Settled::Refused(format!(
            "Nothing was started, since these files cannot be written here: {}.",
            files.join(", ")
        )),
        other => {
            let said = match other {
                Rebased::Unsigned(signer) => {
                    format!("a replayed commit could not be signed: {signer}")
                }
                Rebased::OutOfTime => "the replay did not finish in time".into(),
                Rebased::Failed(said) => said,
                _ => String::new(),
            };
            match super::update::put_back(root, &before, &theirs) {
                Ok(()) => Settled::Refused(format!(
                    "The replay did not go through: {}. The project is exactly as it was.",
                    one_line(&said)
                )),
                Err(still) => Settled::Refused(format!(
                    "The replay stopped part-way: {}. {}",
                    one_line(&said),
                    super::update::said_not_put_back(&still)
                )),
            }
        }
    }
}

/// **Continue**: carry the stopped replay on once every file in conflict is settled.
///
/// `shown` are the files the window showed the person; each must be one git left in conflict.
/// Refused, with nothing staged, while:
/// - a file git could not mark is unsettled ([`Conflicts::by_hand`]);
/// - a marked file still has a conflict marker, is not a plain file, or is gone;
/// - anything outside the files in conflict is staged differently from how git left it.
///
/// The marked files are then staged from the bytes that were checked: what git staged is read
/// back, and where it differs the files are put back in conflict and nothing goes on.
pub fn carry_on(root: &Path, shown: &[String], say: Sink) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    let stop = match held_stop(root) {
        Ok(stop) => stop,
        Err(refused) => return refused,
    };
    let outside: Vec<&String> = shown
        .iter()
        .filter(|path| !stop.conflicted.contains(*path))
        .collect();
    if !outside.is_empty() {
        return Settled::Refused(format!(
            "purlis stages only the files git stopped in, and {} {} not one. Nothing was \
             staged.",
            join(&outside),
            if outside.len() == 1 { "is" } else { "are" },
        ));
    }
    let left = sorted(root, Some(&stop.by_hand));
    if !left.by_hand.is_empty() {
        return Settled::Refused(said_by_hand(&left.by_hand));
    }
    if let Some(why) = staged_elsewhere(root, &stop) {
        return Settled::Refused(why);
    }
    // Settled already, and staged: by the person in a terminal, or by a chat.
    for path in stop.conflicted.difference(&stop.by_hand) {
        if left.marked.contains(path) {
            continue;
        }
        if staged_text(root, path).is_some_and(|text| markers_in(text.as_bytes())) {
            return Settled::Refused(said_markers(path, "is staged with"));
        }
    }
    let mut checked: Vec<(&String, Vec<u8>)> = Vec::new();
    for path in &left.marked {
        match read_plain(root, path) {
            Ok(bytes) if markers_in(&bytes) => {
                return Settled::Refused(said_markers(path, "still has"));
            }
            Ok(bytes) => checked.push((path, bytes)),
            Err(why) => return Settled::Refused(why),
        }
    }
    if !checked.is_empty() {
        let paths: Vec<&str> = checked.iter().map(|(path, _)| path.as_str()).collect();
        let named: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
        // As a save: a settled file with a second name is refused before it is staged, so its
        // bytes never reach the project's object store ([`ondisk`]).
        if let Some(refusal) = ondisk::refused_ahead(&ondisk::second_names_of(root, &named)) {
            return Settled::Refused(not_the_file(&refusal));
        }
        #[cfg(test)]
        super::tests::before_the_add(root);
        let mut add: Vec<&str> = vec!["--literal-pathspecs", "add", "-A", "--"];
        add.extend(&paths);
        match git::run(root, &add, WRITE) {
            Ok(run) if run.ok() => {}
            Ok(run) => {
                return Settled::Refused(format!(
                    "git would not stage them: {}",
                    one_line(&tail(&run))
                ));
            }
            Err(unavailable) => return Settled::Refused(unavailable.to_string()),
        }
        // And once staged: a settled file that is not the file on disk now, with one name, is
        // marked as conflicted again, and its blob taken out of the object store where nothing
        // else reaches it. Only the files staged here are asked about: the rest of the index
        // is the paused commit as git left it.
        let ours: HashSet<&str> = paths.iter().copied().collect();
        let found = ondisk::not_as_staged(root).map(|held| {
            held.into_iter()
                .filter(|one| ours.contains(one.path.as_str()))
                .collect()
        });
        if let Some(refusal) =
            ondisk::refused_into(root, &found, ondisk::PutBack::Conflicted(&named))
        {
            return Settled::Refused(not_the_file(&refusal));
        }
        // What was staged is what was checked, as git stores it for that path (its clean
        // filter and line endings applied), or nothing goes on.
        let moved: Vec<&String> = checked
            .iter()
            .filter(|(path, bytes)| !staged_as_checked(root, path, bytes))
            .map(|(path, _)| *path)
            .collect();
        if !moved.is_empty() {
            // Every file staged here goes back in conflict, and the blobs staged for them out
            // of the object store where nothing else reaches them.
            let all: Vec<ondisk::Held> = named
                .iter()
                .map(|path| ondisk::Held {
                    path: path.clone(),
                    why: ondisk::Why::Changed,
                })
                .collect();
            let _ = ondisk::refused_into(root, &Ok(all), ondisk::PutBack::Conflicted(&named));
            return Settled::Refused(format!(
                "{} changed while purlis was checking {}, so nothing was carried on. Continue \
                 again once it is settled.",
                join(&moved),
                if moved.len() == 1 { "it" } else { "them" },
            ));
        }
    }
    say(Say::Info("Continuing the replay …".into()));
    let plane = crate::planesave::Settings::read(root).plane;
    let continued = git::run(
        root,
        &[
            "-c",
            "core.editor=true",
            "-c",
            gpgsign(plane.sign.value),
            "rebase",
            "--continue",
        ],
        WRITE,
    );
    after_step(root, continued)
}

/// **Skip**: drop the commit being replayed, where settling it left nothing to commit.
pub fn skip(root: &Path) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if let Err(refused) = held_stop(root) {
        return refused;
    }
    // Only a commit with nothing left to commit is dropped, whatever the window last drew.
    let settled = unmerged(root).is_empty();
    let empty = git::run(
        root,
        &["diff", "--cached", "--quiet", "HEAD", "--"],
        git::READ,
    )
    .is_ok_and(|run| run.ok());
    if !(settled && empty) {
        return Settled::Refused(
            "Skip drops only a commit that has nothing left to commit, and this one still \
             changes files. Continue it, or Abort."
                .into(),
        );
    }
    let skipped = git::run_untimed(root, &["-c", "core.editor=true", "rebase", "--skip"]);
    after_step(root, skipped)
}

/// **Abort**: put the project back where it was before the rebase started.
pub fn abort(root: &Path) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if !rebase_in_progress(root) {
        return Settled::Refused(
            "No rebase is stopped in this project, so there is nothing to abort.".into(),
        );
    }
    let aborted = git::run_untimed(root, &["rebase", "--abort"]);
    if rebase_in_progress(root) {
        return Settled::Refused(format!(
            "git did not abort the rebase: {}",
            match aborted {
                Ok(run) => one_line(&tail(&run)),
                Err(unavailable) => unavailable.to_string(),
            }
        ));
    }
    let_go(root);
    super::touch(root);
    Settled::Aborted
}

/// **What follows a replay that finished**: a push of what is committed, as the project's
/// mode says, and no new commit. Answers the push and the files changed meanwhile, which are
/// left as they are for the person to save or remove.
pub fn push_after(root: &Path, say: Sink) -> (Option<PushResult>, Vec<String>) {
    let Some(_claim) = Claim::of(root) else {
        return (None, Vec::new());
    };
    let left = changed_paths(root);
    let plane = crate::planesave::Settings::read(root).plane;
    if matches!(
        plane.mode.value,
        Some(crate::planesave::Mode::Commit | crate::planesave::Mode::Off)
    ) {
        return (None, left);
    }
    let pushed = super::push_saved(root, plane.sign.value, say);
    super::touch(root);
    (Some(pushed), left)
}

/// The files of the rebase stopped in the project, by who may settle them, as this window saw
/// them at the stop. Where it holds no stop, a file with no marker in it is the person's.
/// Empty where none is.
pub fn conflicts(root: &Path) -> Conflicts {
    let held = held_stop(root).ok().map(|stop| stop.by_hand);
    sorted(root, held.as_ref())
}

/// The unmerged files, sorted: by `by_hand` as held at the stop, or, with none, by whether git
/// could mark them and whether a marker is in the file now. A file sorted as markable with no
/// marker in it is the person's: a `.gitattributes` `binary`, `-merge` or merge driver leaves
/// one side's copy and no marker.
fn sorted(root: &Path, by_hand: Option<&BTreeSet<String>>) -> Conflicts {
    let mut out = Conflicts::default();
    for (path, stages) in unmerged(root) {
        let hand = marks_nothing(root, &path, &stages)
            || match by_hand {
                Some(held) => held.contains(&path),
                None => !read_plain(root, &path).is_ok_and(|bytes| markers_in(&bytes)),
            };
        if hand {
            out.by_hand.push(path);
        } else {
            out.marked.push(path);
        }
    }
    out
}

/// The two sides of a stopped rebase, by their commit subjects: the remote's (`HEAD` while a
/// rebase is stopped) and the project's own being replayed (`REBASE_HEAD`).
pub fn sides(root: &Path) -> Option<(String, String)> {
    let subject = |name: &str| {
        git::run(root, &["log", "-1", "--format=%s", name, "--"], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_string())
    };
    Some((subject("HEAD")?, subject("REBASE_HEAD")?))
}

/// Of `files`, those a chat's sandbox may not write: settled by the person, never a chat.
pub fn held_from_chats(root: &Path, files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|path| crate::rootupdate::path_is_denied(path, root))
        .cloned()
        .collect()
}

/// The sentence for files git could not mark a conflict in.
pub fn said_by_hand(files: &[String]) -> String {
    let named: Vec<&String> = files.iter().collect();
    format!(
        "git could not mark the conflict in {} (a binary file, a file one side deleted, a link, \
         a submodule, or a file that is gone), so {} settled in a terminal here: keep one \
         side with `git checkout --ours -- <file>` or `git checkout --theirs -- <file>` (during \
         a rebase, ours is the remote's and theirs is this project's own), or delete it, then \
         `git add -- <file>` or `git rm -- <file>`. Then Continue.",
        join(&named),
        if files.len() == 1 {
            "it is"
        } else {
            "they are"
        },
    )
}

/// Whether `text` has a line that starts with one of git's conflict markers: the opener, the
/// closer, or the common ancestor's (`diff3`, `zdiff3`). The separator alone is not one, since
/// a Markdown heading is underlined the same way.
pub fn markers_in(text: &[u8]) -> bool {
    text.split(|b| *b == b'\n').any(|line| {
        line.starts_with(b"<<<<<<<") || line.starts_with(b">>>>>>>") || line.starts_with(b"|||||||")
    })
}

/// How a step that ran git left the replay: finished, stopped again, empty, or refused.
fn after_step(root: &Path, ran: Result<git::Run, git::GitUnavailable>) -> Settled {
    if !rebase_in_progress(root) {
        // A finished rebase can leave its `REBASE_HEAD` behind, which would read as one still
        // stopped: it names a commit now replayed, and is dropped.
        if gitstate::stopped(root).is_some_and(|left| left.unmerged.is_empty()) {
            let _ = git::run(root, &["update-ref", "-d", "REBASE_HEAD"], WRITE);
        }
        let_go(root);
        forget_replayed_push(root);
        return Settled::Replayed;
    }
    let unmerged: Vec<String> = unmerged(root).into_keys().collect();
    if !unmerged.is_empty() {
        hold_stop(root);
        return Settled::Stopped(unmerged);
    }
    let nothing_staged = git::run(
        root,
        &["diff", "--cached", "--quiet", "HEAD", "--"],
        git::READ,
    )
    .is_ok_and(|run| run.ok());
    if nothing_staged {
        return Settled::Empty;
    }
    let unstaged = names(root, &["diff", "--name-only", "-z", "--"]);
    if !unstaged.is_empty() {
        let named: Vec<&String> = unstaged.iter().collect();
        return Settled::Refused(format!(
            "git will not carry on while {} {} changes it has not staged. Undo them, or settle \
             them in a terminal, then Continue.",
            join(&named),
            if unstaged.len() == 1 { "has" } else { "have" },
        ));
    }
    Settled::Refused(format!(
        "git did not carry on: {}. The rebase is still stopped: Continue again, or Abort.",
        match ran {
            Ok(run) => one_line(&tail(&run)),
            Err(unavailable) => unavailable.to_string(),
        }
    ))
}

/// The stop this window holds for the rebase stopped now, or the refusal to carry on.
fn held_stop(root: &Path) -> Result<Stop, Settled> {
    if !rebase_in_progress(root) {
        return Err(Settled::Refused(
            "No rebase is stopped in this project, so there is nothing to carry on.".into(),
        ));
    }
    let at = rev(root, "REBASE_HEAD");
    let stops = STOPS.lock().unwrap_or_else(PoisonError::into_inner);
    stops
        .as_ref()
        .and_then(|held| held.get(root))
        .filter(|stop| Some(&stop.at) == at.as_ref())
        .cloned()
        .ok_or_else(|| Settled::Refused(NOT_HELD.into()))
}

/// Keep what git stopped with now, for the next Continue.
fn hold_stop(root: &Path) {
    let Some(at) = rev(root, "REBASE_HEAD") else {
        return;
    };
    let found = sorted(root, None);
    let conflicted: BTreeSet<String> = found.marked.iter().chain(&found.by_hand).cloned().collect();
    let stop = Stop {
        at,
        conflicted,
        by_hand: found.by_hand.into_iter().collect(),
        staged: staged_against_head(root),
    };
    STOPS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(root.to_path_buf(), stop);
}

/// Forget the stop held for `root`.
fn let_go(root: &Path) {
    if let Some(held) = STOPS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        held.remove(root);
    }
}

/// Why anything outside the files in conflict is not as git staged it at the stop, or `None`.
fn staged_elsewhere(root: &Path, stop: &Stop) -> Option<String> {
    let now = staged_against_head(root);
    let paths: BTreeSet<&String> = now.keys().chain(stop.staged.keys()).collect();
    let differ: Vec<&String> = paths
        .into_iter()
        .filter(|path| !stop.conflicted.contains(*path) && now.get(*path) != stop.staged.get(*path))
        .collect();
    (!differ.is_empty()).then(|| {
        format!(
            "{} {} not as git left {} when the replay stopped, and purlis carries on only what \
             git staged and the files in conflict. Abort and Resolve again, or carry on in a \
             terminal.",
            join(&differ),
            if differ.len() == 1 { "is" } else { "are" },
            if differ.len() == 1 { "it" } else { "them" },
        )
    })
}

/// Every change staged against HEAD that is not in conflict: each path's mode and object.
fn staged_against_head(root: &Path) -> BTreeMap<String, (String, String)> {
    let Some(run) = git::run(
        root,
        &["diff-index", "--cached", "--no-renames", "-z", "HEAD", "--"],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok) else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    let mut fields = run.out.split('\0');
    // `:<old mode> <new mode> <old object> <new object> <status>` then the path.
    while let (Some(meta), Some(path)) = (fields.next(), fields.next()) {
        let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        if let [_, mode, _, object, status] = parts[..]
            && status != "U"
            && !path.is_empty()
        {
            out.insert(path.to_string(), (mode.to_string(), object.to_string()));
        }
    }
    out
}

/// The files git calls unmerged, each with its index entries: stage, mode and object.
fn unmerged(root: &Path) -> BTreeMap<String, Vec<(u8, String, String)>> {
    let Some(run) = git::run(root, &["ls-files", "-u", "-z"], git::READ)
        .ok()
        .filter(git::Run::ok)
    else {
        return BTreeMap::new();
    };
    let mut out: BTreeMap<String, Vec<(u8, String, String)>> = BTreeMap::new();
    // `<mode> <object> <stage>\t<path>`
    for entry in run.out.split('\0').filter(|e| !e.is_empty()) {
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let parts: Vec<&str> = meta.split(' ').collect();
        if let [mode, object, stage] = parts[..]
            && let Ok(stage) = stage.parse::<u8>()
        {
            out.entry(path.to_string()).or_default().push((
                stage,
                mode.to_string(),
                object.to_string(),
            ));
        }
    }
    out
}

/// Whether git could not mark the conflict in `path`: a side is missing (one deleted it), an
/// entry is a link or a submodule, a side is binary as git judges it (a NUL in its first 8000
/// bytes), or the file is gone from the work tree.
fn marks_nothing(root: &Path, path: &str, stages: &[(u8, String, String)]) -> bool {
    let side = |n: u8| stages.iter().find(|(stage, _, _)| *stage == n);
    let (Some(ours), Some(theirs)) = (side(2), side(3)) else {
        return true;
    };
    if stages
        .iter()
        .any(|(_, mode, _)| mode == "120000" || mode == "160000")
    {
        return true;
    }
    let binary = |object: &str| {
        git::run(root, &["cat-file", "blob", object], git::READ)
            .ok()
            .filter(git::Run::ok)
            .is_none_or(|run| run.out.bytes().take(8000).any(|b| b == 0))
    };
    if binary(&ours.2) || binary(&theirs.2) {
        return true;
    }
    std::fs::symlink_metadata(root.join(path)).is_err()
}

/// Whether what is staged at `path` is `bytes`, as git would store them for that path: its
/// clean filter and line endings applied (`hash-object --path`).
pub(super) fn staged_as_checked(root: &Path, path: &str, bytes: &[u8]) -> bool {
    let staged = git::run(
        root,
        &["rev-parse", "--verify", "-q", &format!(":0:{path}")],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    .map(|run| run.line().trim().to_string());
    let checked = git::run_with_input(
        root,
        &["hash-object", "--path", path, "--stdin"],
        bytes.to_vec(),
        git::READ,
    )
    .ok()
    .filter(|run| run.code == Some(0))
    .map(|run| String::from_utf8_lossy(&run.out).trim().to_string());
    staged.is_some() && staged == checked
}

/// What is staged at `path` (stage 0), as text; `None` where nothing is.
fn staged_text(root: &Path, path: &str) -> Option<String> {
    git::run(
        root,
        &[
            "--literal-pathspecs",
            "cat-file",
            "blob",
            &format!(":0:{path}"),
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    .map(|run| run.out)
}

/// The bytes of a plain file, or why it is not one purlis stages: opened without following a
/// link, and asked of that descriptor that it is a regular file with one name, so what is
/// checked is the file the save's rule allows ([`ondisk`]).
fn read_plain(root: &Path, path: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = opened_plain(root, path)?;
    let too_large =
        || format!("`{path}` is too large for purlis to check. Settle it in a terminal, or Abort.");
    let size = file
        .metadata()
        .map_err(|why| format!("`{path}` cannot be read: {why}."))?
        .len();
    if size > MOST_READ {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(MOST_READ + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| format!("`{path}` cannot be read: {why}."))?;
    if bytes.len() as u64 > MOST_READ {
        return Err(too_large());
    }
    Ok(bytes)
}

/// `path` opened as a regular file with one name, one folder at a time and never through a
/// link ([`ondisk::opened`]), or why it is not one purlis stages.
#[cfg(unix)]
fn opened_plain(root: &Path, path: &str) -> Result<std::fs::File, String> {
    match ondisk::opened(root, path) {
        ondisk::OnDisk::File(file, _) => Ok(file),
        ondisk::OnDisk::SecondName => Err(second_name(path)),
        ondisk::OnDisk::Other => Err(not_plain(root, path)),
    }
}

/// Where a folder cannot be held open: the name looked at, never through a link, then opened.
#[cfg(not(unix))]
fn opened_plain(root: &Path, path: &str) -> Result<std::fs::File, String> {
    match std::fs::symlink_metadata(root.join(path)) {
        Ok(meta) if meta.is_file() => std::fs::File::open(root.join(path))
            .map_err(|why| format!("`{path}` cannot be read: {why}.")),
        _ => Err(not_plain(root, path)),
    }
}

/// Why `path` is not a plain file purlis stages: gone, or something else.
fn not_plain(root: &Path, path: &str) -> String {
    if std::fs::symlink_metadata(root.join(path)).is_err() {
        format!(
            "`{path}` is gone from the project, so purlis will not stage its conflict. Settle \
             it in a terminal (`git add` or `git rm` it), then Continue."
        )
    } else {
        format!(
            "`{path}` is not a plain file, so purlis will not stage it. Settle it in a \
             terminal, or Abort."
        )
    }
}

/// What Continue says of a settled file with a second name, found before anything is staged.
#[cfg(unix)]
fn second_name(path: &str) -> String {
    let held = Ok(vec![ondisk::Held {
        path: path.to_owned(),
        why: ondisk::Why::SecondName,
    }]);
    ondisk::refused_ahead(&held)
        .map(|refusal| not_the_file(&refusal))
        .unwrap_or_default()
}

/// What Continue refused by [`ondisk`] says: a settled file is not the one file on disk.
fn not_the_file(refusal: &ondisk::Refusal) -> String {
    format!(
        "A settled file is not the one file on disk: {} Settle it again as one file, then \
         Continue.",
        refusal.sentence()
    )
}

fn said_markers(path: &str, how: &str) -> String {
    format!(
        "`{path}` {how} conflict markers (`<<<<<<<`, `|||||||`, `>>>>>>>`). Settle them, then \
         Continue."
    )
}

fn names(root: &Path, args: &[&str]) -> Vec<String> {
    git::run(root, args, git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| {
            run.out
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn rev(root: &Path, name: &str) -> Option<String> {
    git::run(root, &["rev-parse", "--verify", "-q", name], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .filter(|sha| !sha.is_empty())
}

fn join(paths: &[&String]) -> String {
    paths
        .iter()
        .map(|p| format!("`{p}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn one_line(said: &str) -> String {
    said.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;

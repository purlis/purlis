// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis worktree update` and `purlis worktree push` (qw387): keep a branch folder current
//! and publish its branch, beside `purlis worktree commit`.
//!
//! A linked worktree keeps its index, HEAD and rebase state in its clone's `.git`, which a
//! sandboxed chat may not write, so its own `git fetch`, `git rebase` and `git reset` fail
//! there. These verbs are git-shaped and small, and in a chat the app runs them
//! ([`crate::gitbroker`]); from a terminal they run here.
//!
//! - **update**: fetch the base's remote, bring the base up to it where that is a fast-forward
//!   nothing has checked out ([`crate::worktree::fresh`]), and rebase the piece onto the base:
//!   the one named, or the one recorded when the piece was cut. A rebase stops on a conflict and
//!   names the files; `--continue` stages exactly the files git named, once resolved, each as
//!   the one file on disk with one name as a save does (`planegit::ondisk`), and goes on;
//!   `--abort` puts the branch back. **A move that cannot finish does not start**: for a
//!   sandboxed chat, the rebase is judged by the clone guard's own rules first
//!   ([`crate::cloneupdate::judged`]) and refused where it would write a file the chat's
//!   sandbox denies, naming it. A folder with uncommitted changes is refused.
//! - **push**: the folder's own branch, under its own name, to its remote, after purlis's scan
//!   of what it would publish. `--force-with-lease` only after an update rewrote the branch,
//!   leased to what the remote had when it was fetched just before.
//!
//! Every git call goes through the hardened runner. Nothing here amends, resets, merges or
//! pushes another branch.

use std::path::{Path, PathBuf};

use crate::gitbroker::commit as broker;
use crate::gitstate::{self, Operation};
use crate::hookwire::UpdateStep;
use crate::planegit::ondisk;
use crate::repocmd::Say;
use crate::worktree::{self, git, name};

/// The branch folder a verb acts on.
pub struct Piece<'a> {
    pub root: &'a Path,
    pub ws: &'a str,
    pub repo: &'a str,
    pub piece: &'a str,
}

/// What the verb may reach, where a chat asks.
#[derive(Default)]
pub struct Reach<'a> {
    /// The hosts a fetch or a push may reach: a sandboxed chat's internet access. `None`, no
    /// limit, from a terminal or a chat that is not sandboxed.
    pub hosts: Option<&'a [String]>,
    /// Where a sandboxed chat that asks was started: an update is judged for the files its
    /// sandbox denies. `None` from a terminal or a chat that is not sandboxed.
    pub sandboxed_at: Option<&'a Path>,
}

/// The settings every rebase here runs with, whatever the repository's config says: no editor,
/// no signer, no upkeep after it, and no other branch moved with it.
const REBASE: [&str; 14] = [
    "-c",
    "core.editor=:",
    "-c",
    "sequence.editor=:",
    "-c",
    "commit.gpgSign=false",
    "-c",
    "rebase.updateRefs=false",
    "-c",
    "rebase.autoSquash=false",
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
];

/// `purlis worktree update`: one step of it in `piece`, its exit status, its lines said.
pub fn update(piece: &Piece, step: &UpdateStep, reach: &Reach, say: &mut dyn FnMut(Say)) -> u8 {
    // A stopped rebase leaves the folder on no branch: going on or back needs none.
    let on_a_branch = matches!(step, UpdateStep::Start { .. });
    let ran = Here::of(piece, on_a_branch).and_then(|here| {
        let lock = broker::lock_of(&here.git_dir);
        let _held = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match step {
            UpdateStep::Start { base } => start(piece, &here, base.as_deref(), reach, say),
            UpdateStep::Continue => go_on(piece, &here, say),
            UpdateStep::Abort => abort(piece, &here, say),
        }
    });
    ran.unwrap_or_else(|why| {
        say(Say::Fail(why));
        1
    })
}

/// `purlis worktree push [--force-with-lease]`: the branch of `piece` pushed to its remote.
pub fn push(piece: &Piece, lease: bool, reach: &Reach, say: &mut dyn FnMut(Say)) -> u8 {
    let ran = Here::of(piece, true).and_then(|here| {
        let lock = broker::lock_of(&here.git_dir);
        let _held = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pushed(piece, &here, lease, reach, say)
    });
    ran.unwrap_or_else(|why| {
        say(Say::Fail(why));
        1
    })
}

/// The folder, its clone, its git directory and its branch.
struct Here {
    folder: PathBuf,
    clone: PathBuf,
    git_dir: PathBuf,
    branch: String,
}

impl Here {
    /// The folder of `piece`, refused where it is on no branch and `on_a_branch` is asked.
    fn of(piece: &Piece, on_a_branch: bool) -> Result<Self, String> {
        let Piece {
            root,
            ws,
            repo,
            piece,
        } = *piece;
        let folder = worktree::path_for(root, ws, repo, piece).map_err(|e| e.to_string())?;
        let folder =
            worktree::confine::within_workspace(root, ws, &folder).map_err(|e| e.to_string())?;
        if folder.symlink_metadata().is_err() {
            return Err(format!(
                "workspace '{ws}' has no branch folder {repo} · {piece}: `purlis worktree list` \
                 shows those it has"
            ));
        }
        let clone = worktree::clone_dir(root, ws, repo).map_err(|e| e.to_string())?;
        let git_dir = ran(&folder, &["rev-parse", "--absolute-git-dir"])?;
        let branch = ran(&folder, &["branch", "--show-current"])?;
        if branch.is_empty() && on_a_branch {
            return Err(format!(
                "{repo} · {piece} is on no branch (a detached HEAD), so there is no branch to \
                 update or push"
            ));
        }
        Ok(Self {
            folder,
            clone,
            git_dir: PathBuf::from(git_dir),
            branch,
        })
    }
}

/// What git printed for `args` in `dir`, trimmed, or why not.
fn ran(dir: &Path, args: &[&str]) -> Result<String, String> {
    let run = git::run(dir, args, git::READ).map_err(|e| e.to_string())?;
    if run.ok() {
        Ok(run.line().trim().to_owned())
    } else {
        Err(format!("git could not answer: {}", first_line(&run.err)))
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").trim().to_owned()
}

/// The remote branch `branch` of `clone` is fetched from: its own, else `origin` where the
/// clone has one. `None` for a clone with no remote.
pub(crate) fn remote_of(clone: &Path, branch: &str) -> Option<String> {
    let key = format!("branch.{branch}.remote");
    let said = git::run(clone, &["config", "--get", &key], git::READ).ok()?;
    let named = said.line().trim();
    if said.ok() && !named.is_empty() && named != "." {
        return Some(named.to_owned());
    }
    let origin = git::run(clone, &["remote", "get-url", "origin"], git::READ).ok()?;
    origin.ok().then(|| "origin".to_owned())
}

/// What git has stopped in the folder, as [`gitstate::stopped`] reads it, less a rebase git
/// no longer has under way: one is in progress only while its own folder is there
/// (`rebase-merge` or `rebase-apply`), whatever other marker it left.
fn stopped_in(here: &Here) -> Option<gitstate::Stopped> {
    let stopped = gitstate::stopped(&here.folder)?;
    let rebasing = ["rebase-merge", "rebase-apply"]
        .iter()
        .any(|dir| here.git_dir.join(dir).is_dir());
    let gone =
        stopped.operation == Some(Operation::Rebase) && !rebasing && stopped.unmerged.is_empty();
    (!gone).then_some(stopped)
}

/// What a rebase that git stopped says, and its status.
fn stopped_said(piece: &Piece, here: &Here, git_said: &str, say: &mut dyn FnMut(Say)) -> u8 {
    let (repo, name) = (piece.repo, piece.piece);
    match stopped_in(here) {
        Some(stopped) if stopped.operation == Some(Operation::Rebase) => {
            let files = if stopped.unmerged.is_empty() {
                String::new()
            } else {
                format!(" on a conflict in {}", stopped.unmerged.join(", "))
            };
            say(Say::Fail(format!(
                "the rebase of {repo} · {name} stopped{files}. Resolve them in the folder, then \
                 `purlis worktree update --continue`; or `purlis worktree update --abort` puts \
                 the branch back where it was"
            )));
        }
        _ => say(Say::Fail(format!(
            "git did not rebase {repo} · {name}: {}",
            first_line(git_said)
        ))),
    }
    1
}

fn start(
    piece: &Piece,
    here: &Here,
    base: Option<&str>,
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> Result<u8, String> {
    let (repo, name_) = (piece.repo, piece.piece);
    if let Some(stopped) = stopped_in(here) {
        let what = stopped.operation.map_or("merge", Operation::word);
        return Err(format!(
            "a {what} is stopped in {repo} · {name_}: `purlis worktree update --continue` once \
             its files are resolved, or `purlis worktree update --abort`"
        ));
    }
    let changed = ran(
        &here.folder,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !changed.is_empty() {
        return Err(format!(
            "{repo} · {name_} has uncommitted changes, so nothing was moved: commit them \
             (`purlis worktree commit`), then update"
        ));
    }
    let base = match base {
        Some(base) => base.to_owned(),
        None => worktree::recorded_base(&here.clone, &here.branch).ok_or_else(|| {
            format!(
                "purlis has no record of the branch {} was cut from: name it, `purlis worktree \
                 update --base <branch>`",
                here.branch
            )
        })?,
    };
    name::branch_name_ok(&base).map_err(|e| e.to_string())?;
    if base == here.branch {
        return Err(format!(
            "{base} is the folder's own branch: name the branch it is to be brought up to"
        ));
    }
    if let Some(remote) = remote_of(&here.clone, &base) {
        crate::repocmd::sync::fetch_remote(piece.root, &here.clone, &remote, reach.hosts)
            .map_err(|why| format!("{why}. Nothing was moved"))?;
    }
    let fresh = worktree::fresh::bring_up(&here.clone, &base).map_err(|e| e.to_string())?;
    for note in fresh.notes {
        say(Say::Warn(note));
    }
    // The base, or its remote-tracking branch where the base is behind it and checked out.
    let onto = fresh.instead.clone().unwrap_or_else(|| name::as_ref(&base));
    // Each read once as a commit id, and git run on the ids: what is judged is what moves,
    // whatever a ref names by the time git runs.
    let onto_id = commit_of(&here.folder, &onto)
        .ok_or_else(|| format!("{onto} names no commit here, so nothing was moved"))?;
    let head_id = commit_of(&here.folder, "HEAD")
        .ok_or_else(|| format!("{repo} · {name_} has no commit, so nothing was moved"))?;
    let there = git::run(
        &here.folder,
        &["merge-base", "--is-ancestor", &onto_id, &head_id],
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if there.ok() {
        say(Say::Done(format!("{repo} · {name_} is already on {base}")));
        return Ok(0);
    }
    if let Some(chat_dir) = reach.sandboxed_at {
        let root = piece.root.to_string_lossy();
        let place = crate::cloneupdate::Place {
            root: &root,
            chat_dir: Some(chat_dir),
            alone: true,
            // The app runs exactly this rebase, with nothing beside it.
            sparse: true,
        };
        let folder = here.folder.to_string_lossy();
        // Judged for the two ids the rebase is handed, never for what HEAD names by then.
        if let Some(why) = crate::cloneupdate::judged_rebase(
            &folder,
            &onto_id,
            &head_id,
            &place,
            &crate::cloneupdate::Asked,
        ) {
            return Err(why);
        }
    }
    // The commits judged are those of the head read above: a branch moved since is not
    // rebased.
    if commit_of(&here.folder, "HEAD").as_deref() != Some(head_id.as_str()) {
        return Err(MOVED.to_owned());
    }
    let record = Record {
        branch: here.branch.clone(),
        head: head_id.clone(),
    };
    record.write(&here.git_dir)?;
    let mut args: Vec<&str> = REBASE.to_vec();
    args.extend([
        "rebase",
        "--no-autostash",
        "--onto",
        &onto_id,
        &onto_id,
        &head_id,
    ]);
    // Untimed: a rebase checks trees out, and one killed halfway is left half done.
    let rebased = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    if !rebased.ok() {
        return Ok(stopped_said(piece, here, &rebased.err, say));
    }
    finish(here, &record)?;
    let moved = match (&fresh.moved_to, onto.strip_prefix("refs/remotes/")) {
        (Some(up), _) => format!(" ({base} brought up to {up})"),
        (None, Some(up)) => format!(" (as {up} has it)"),
        (None, None) => String::new(),
    };
    say(Say::Done(format!(
        "{repo} · {name_} is rebased onto {base}{moved}"
    )));
    Ok(0)
}

/// A line of a file that git left a conflict in, as git writes the first marker.
const MARKER: &[u8] = b"<<<<<<< ";

fn go_on(piece: &Piece, here: &Here, say: &mut dyn FnMut(Say)) -> Result<u8, String> {
    let (repo, name_) = (piece.repo, piece.piece);
    let Some(stopped) =
        stopped_in(here).filter(|stopped| stopped.operation == Some(Operation::Rebase))
    else {
        return Err(format!(
            "no rebase is stopped in {repo} · {name_}: `purlis worktree update` starts one"
        ));
    };
    let record = Record::read(&here.git_dir).ok_or_else(|| {
        format!(
            "the rebase stopped in {repo} · {name_} is not one purlis started: finish it from \
             your own terminal"
        )
    })?;
    let files = stopped.unmerged;
    if !files.is_empty() {
        resolved(&here.folder, &files)?;
        // As a save: a resolved file with a second name is refused before it is staged, so its
        // bytes never reach the clone's object store (`planegit::ondisk`).
        if let Some(refusal) = ondisk::refused_ahead(&ondisk::second_names_of(&here.folder, &files))
        {
            return Err(not_the_file(&refusal));
        }
        let staged = git::run_with_input(
            &here.folder,
            &[
                "--literal-pathspecs",
                "add",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            files.join("\0").into_bytes(),
            broker::STAGING,
        )
        .map_err(|e| e.to_string())?;
        if staged.code != Some(0) {
            return Err(format!(
                "git did not stage the resolved files: {}",
                first_line(&String::from_utf8_lossy(&staged.err))
            ));
        }
        // And once staged: a resolved file that is not the file on disk now, with one name, is
        // marked as conflicted again, and its blob taken out of the object store where nothing
        // else reaches it. Only the files staged here are asked about: the rest of the index
        // is the paused commit as git left it.
        let ours: std::collections::HashSet<&str> = files.iter().map(String::as_str).collect();
        let found = ondisk::not_as_staged(&here.folder).map(|held| {
            held.into_iter()
                .filter(|one| ours.contains(one.path.as_str()))
                .collect()
        });
        if let Some(refusal) =
            ondisk::refused_into(&here.folder, &found, ondisk::PutBack::Conflicted(&files))
        {
            return Err(not_the_file(&refusal));
        }
    }
    let mut args: Vec<&str> = REBASE.to_vec();
    args.extend(["rebase", "--continue"]);
    let went = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    if !went.ok() && went.err.contains("no rebase in progress") {
        return Err(format!(
            "no rebase is stopped in {repo} · {name_}: `purlis worktree update` starts one"
        ));
    }
    if !went.ok() {
        return Ok(stopped_said(piece, here, &went.err, say));
    }
    finish(here, &record)?;
    say(Say::Done(format!("{repo} · {name_} is rebased")));
    Ok(0)
}

/// What `--continue` refused by `planegit::ondisk` says: a resolved file is not the one file
/// on disk.
fn not_the_file(refusal: &ondisk::Refusal) -> String {
    format!(
        "a resolved file is not the one file on disk: {} Resolve it again as one file, then \
         `purlis worktree update --continue`",
        refusal.sentence()
    )
}

/// What a branch moved while purlis judged or rebased it is told.
const MOVED: &str = "the folder's branch moved while purlis worked on it, so it was left where \
     it is. Run the update again";

/// What an update purlis started records in the folder's own git directory, which no chat
/// writes: the branch it rebases, and the commit that branch was at when it was judged.
struct Record {
    branch: String,
    head: String,
}

/// The file [`Record`] is kept in while an update is under way.
const UNDER_WAY: &str = "purlis-update";

/// The file that says which commit purlis's last update left the branch at: what a
/// `--force-with-lease` push asks for.
const UPDATED: &str = "purlis-updated";

impl Record {
    fn write(&self, git_dir: &Path) -> Result<(), String> {
        std::fs::write(
            git_dir.join(UNDER_WAY),
            format!("{}\n{}\n", self.branch, self.head),
        )
        .map_err(|why| format!("purlis could not note the update it starts: {why}"))
    }

    fn read(git_dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(git_dir.join(UNDER_WAY)).ok()?;
        let mut lines = text.lines();
        let (branch, head) = (lines.next()?.to_owned(), lines.next()?.to_owned());
        (name::branch_name_ok(&branch).is_ok() && full_id(&head)).then_some(Self { branch, head })
    }
}

/// Whether `id` is a full commit id as git prints one.
fn full_id(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The rebase is done, on a detached head: the branch is moved to it only from the commit it
/// was judged at, the folder is put back on the branch, and the commit is noted as the one
/// this update left it at.
fn finish(here: &Here, record: &Record) -> Result<(), String> {
    let _ = std::fs::remove_file(here.git_dir.join(UNDER_WAY));
    let new = commit_of(&here.folder, "HEAD").ok_or("git did not say where the rebase ended")?;
    let local_ref = name::as_ref(&record.branch);
    let moved = git::run(
        &here.folder,
        &[
            "update-ref",
            "-m",
            "purlis: worktree update",
            &local_ref,
            &new,
            &record.head,
        ],
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if !moved.ok() {
        return Err(format!(
            "{MOVED}. The rebased commits are at {new}, on no branch, in the folder"
        ));
    }
    let back = git::run(
        &here.folder,
        &["symbolic-ref", "HEAD", &local_ref],
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if !back.ok() {
        return Err(format!(
            "{} is rebased, and git did not put the folder back on it: {}",
            record.branch,
            first_line(&back.err)
        ));
    }
    std::fs::write(here.git_dir.join(UPDATED), format!("{new}\n"))
        .map_err(|why| format!("purlis could not note the update it made: {why}"))
}

/// Why the files git named as conflicted may not be staged for a chat, or nothing: each a
/// name inside the folder reached through no link, not another repository nor the file that
/// declares them, not marked for a content filter the person's own git would run, and with no
/// conflict marker left in it.
fn resolved(folder: &Path, files: &[String]) -> Result<(), String> {
    broker::paths_ok(files)?;
    for file in files {
        broker::through_no_link(folder, file)?;
        let name = file.rsplit('/').next().unwrap_or(file);
        if name.eq_ignore_ascii_case(".gitmodules") {
            return Err(format!(
                "{file} declares submodules, and purlis does not resolve it for a chat: finish \
                 the rebase from your own terminal"
            ));
        }
        let at = folder.join(file);
        if let Ok(meta) = std::fs::symlink_metadata(&at) {
            if meta.is_dir() {
                return Err(format!(
                    "{file} is a folder where git left a conflict (another repository, most \
                     often): finish the rebase from your own terminal"
                ));
            }
            if meta.is_file()
                && let Ok(text) = std::fs::read(&at)
                && (text.starts_with(MARKER)
                    || text
                        .windows(MARKER.len() + 1)
                        .any(|w| w[0] == b'\n' && &w[1..] == MARKER))
            {
                return Err(format!(
                    "{file} still holds a conflict marker (`<<<<<<<`): resolve it, then \
                     `purlis worktree update --continue`"
                ));
            }
        }
    }
    let attrs = git::run_with_input(
        folder,
        &["check-attr", "-z", "--stdin", "filter"],
        files.join("\0").into_bytes(),
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if attrs.code != Some(0) {
        return Err("git could not read the attributes of the conflicted files".to_owned());
    }
    if let Some((file, filter)) = broker::filter_named(&attrs.out) {
        return Err(format!(
            "{file} is marked for a content filter (`filter={filter}`), which git run for a chat \
             does not apply: finish the rebase from your own terminal"
        ));
    }
    Ok(())
}

fn abort(piece: &Piece, here: &Here, say: &mut dyn FnMut(Say)) -> Result<u8, String> {
    let (repo, name_) = (piece.repo, piece.piece);
    if !stopped_in(here).is_some_and(|stopped| stopped.operation == Some(Operation::Rebase)) {
        return Err(format!("no rebase is stopped in {repo} · {name_}"));
    }
    let mut args: Vec<&str> = REBASE.to_vec();
    args.extend(["rebase", "--abort"]);
    let back = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    // git's own answer settles whether one was stopped, whatever the markers said.
    if back.ok()
        && let Some(record) = Record::read(&here.git_dir)
    {
        let _ = std::fs::remove_file(here.git_dir.join(UNDER_WAY));
        let local_ref = name::as_ref(&record.branch);
        if commit_of(&here.folder, &local_ref).as_deref() == Some(record.head.as_str()) {
            let _ = git::run(
                &here.folder,
                &["symbolic-ref", "HEAD", &local_ref],
                git::READ,
            );
        }
    }
    if !back.ok() && back.err.contains("no rebase in progress") {
        return Err(format!("no rebase is stopped in {repo} · {name_}"));
    }
    if !back.ok() {
        return Err(format!(
            "git did not put {repo} · {name_} back: {}",
            first_line(&back.err)
        ));
    }
    say(Say::Done(format!(
        "{repo} · {name_} is back where it was before the update"
    )));
    Ok(0)
}

/// The commit a ref names in `dir`, or `None`.
fn commit_of(dir: &Path, name: &str) -> Option<String> {
    let spec = format!("{name}^{{commit}}");
    let found = git::run(dir, &["rev-parse", "--verify", "--quiet", &spec], git::READ).ok()?;
    (found.ok() && !found.line().is_empty()).then(|| found.line().trim().to_owned())
}

/// The id git uses for "nothing there" in a push's ref lines.
const NOTHING: &str = "0000000000000000000000000000000000000000";

fn pushed(
    piece: &Piece,
    here: &Here,
    lease: bool,
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> Result<u8, String> {
    let (repo, name_) = (piece.repo, piece.piece);
    let branch = &here.branch;
    if stopped_in(here).is_some() {
        return Err(format!(
            "an operation is stopped in {repo} · {name_}: finish it (`purlis worktree update \
             --continue`) or abort it before pushing"
        ));
    }
    let remote = remote_of(&here.clone, branch)
        .ok_or_else(|| format!("{repo} has no remote to push {branch} to"))?;
    // Asked of the remote itself, never read from the clone.
    let default = worktree::fresh::remote_default(piece.root, &here.clone, &remote, reach.hosts)
        .ok_or_else(|| {
            format!("{remote} did not say which branch is its default, so nothing was pushed")
        })?;
    if default == *branch {
        return Err(format!(
            "{branch} is the branch the remote names its default, and purlis pushes a branch \
             folder's own branch only. Nothing was pushed"
        ));
    }
    // Fetched first, so the lease and the scan read what the remote has now.
    crate::repocmd::sync::fetch_remote(piece.root, &here.clone, &remote, reach.hosts)
        .map_err(|why| format!("{why}. Nothing was pushed"))?;
    let (url, helper) =
        crate::repocmd::sync::reach(piece.root, &here.clone, &remote, reach.hosts, true)?;
    let local_ref = name::as_ref(branch);
    let ours = commit_of(&here.folder, "HEAD")
        .ok_or_else(|| format!("{repo} · {name_} has no commit to push"))?;
    let theirs = commit_of(&here.clone, &format!("refs/remotes/{remote}/{branch}"));
    if lease {
        // purlis's own note of where its last update left the branch, which no chat writes.
        let updated = std::fs::read_to_string(here.git_dir.join(UPDATED)).unwrap_or_default();
        if updated.trim() != ours {
            return Err(format!(
                "--force-with-lease pushes over {remote}'s {branch}, which purlis does only \
                 right after `purlis worktree update` rewrote it. Nothing was pushed"
            ));
        }
    }
    // purlis's scan of what the push would publish, as a chat's own push runs it (ADR 0074).
    let updates = format!(
        "{local_ref} {ours} {local_ref} {}\n",
        theirs.as_deref().unwrap_or(NOTHING)
    );
    let scan = crate::diffscan::pushed(&here.folder, &remote, &url, &updates)?;
    if scan.changes_the_allowlist {
        return Err(crate::diffscan::PUSH_ALLOWLIST_REFUSAL.trim().to_owned());
    }
    if !scan.refused.is_empty() {
        return Err(
            crate::diffscan::refusal(crate::diffscan::Stopped::Push, &scan.refused)
                .trim()
                .to_owned(),
        );
    }
    // The commit scanned, by its id: a branch moved since sends nothing unscanned.
    let refspec = format!("{ours}:{local_ref}");
    let leased = format!(
        "--force-with-lease={local_ref}:{}",
        theirs.as_deref().unwrap_or("")
    );
    let mut args = vec!["push"];
    if lease {
        args.push(&leased);
    }
    args.extend([remote.as_str(), refspec.as_str()]);
    let sent =
        git::run_network(&here.folder, helper.as_deref(), &args).map_err(|e| e.to_string())?;
    if !sent.ok() {
        return Err(format!(
            "git did not push {branch} to {remote}: {}",
            first_line(&sent.err)
        ));
    }
    say(Say::Done(format!(
        "{repo} · {name_}: {branch} pushed to {remote}"
    )));
    Ok(0)
}

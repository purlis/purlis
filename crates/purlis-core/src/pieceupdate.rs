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
//!   the one named, a remote-tracking branch among them (`origin/main`), or the one recorded
//!   when the piece was cut. **Only the piece's own commits move** (#3836): those after where
//!   it left the base it was recorded as cut from, so a piece cut from a base that had moved
//!   apart from its remote leaves that base's commits behind, as `git rebase --onto <new>
//!   <old>` would, and a base named in place of the recorded one is recorded. A rebase stops on
//!   a conflict and names the files; `--continue` stages exactly the files git named, once
//!   resolved, each as the one file on disk with one name as a save does (`planegit::ondisk`),
//!   and goes on;
//!   `--abort` puts the branch back. **A move that cannot finish does not start**: for a
//!   sandboxed chat, the rebase is judged by the clone guard's own rules first
//!   ([`crate::cloneupdate::judged`]) and refused where it would write a file the chat's
//!   sandbox denies, naming it. A folder with uncommitted changes is refused.
//! - **update, standing in a workspace's clone** (#3831): fetch the remote its branch follows,
//!   and fast-forward the branch to it or rebase the branch's own commits onto it, with the
//!   same `--continue` and `--abort`, the same judging for a sandboxed chat, and the same
//!   refusal of uncommitted changes. Never the project's root folder, which is the person's.
//!   Either kind is refused where it would write over a file on disk that the current commit
//!   does not track (an ignored `.env`, an untracked file): someone's work, which git would
//!   overwrite or stop on halfway.
//! - **push**: the folder's own branch, under its own name, to its remote, after purlis's scan
//!   of what it would publish. Over the remote's copy only where purlis's own update rewrote
//!   the branch (#3895): the update notes the commit the remote's copy was at, where the folder
//!   had all of it, and the push is leased to exactly that commit, with any commits added since
//!   on top. A remote someone else moved, a branch rewritten any other way, or a note purlis
//!   cannot read refuses it, naming what to do. A plain push does this; `--force-with-lease`
//!   asks for nothing more, so no chat ever needs a force push approved.
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
            UpdateStep::Continue => go_on(&here, say),
            UpdateStep::Abort => abort(&here, say),
        }
    });
    ran.unwrap_or_else(|why| {
        say(Say::Fail(why.into()));
        1
    })
}

/// A workspace's clone, its own checkout, that `purlis worktree update` brings up to its
/// remote (#3831).
pub struct CloneAt<'a> {
    pub root: &'a Path,
    pub ws: &'a str,
    pub repo: &'a str,
}

/// `purlis worktree update` standing in a clone (#3831): one step of it, its exit status, its
/// lines said. A rebase it stops goes on or back as a branch folder's does.
pub fn update_clone(
    at: &CloneAt,
    step: &UpdateStep,
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> u8 {
    let on_a_branch = matches!(step, UpdateStep::Start { .. });
    let ran = Here::of_clone(at, on_a_branch).and_then(|here| {
        let lock = broker::lock_of(&here.git_dir);
        let _held = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match step {
            UpdateStep::Start { base } => start_clone(at, &here, base.as_deref(), reach, say),
            UpdateStep::Continue => go_on(&here, say),
            UpdateStep::Abort => abort(&here, say),
        }
    });
    ran.unwrap_or_else(|why| {
        say(Say::Fail(why.into()));
        1
    })
}

/// `purlis worktree push`: the branch of `piece` pushed to its remote, over the remote's copy
/// where purlis's own update rewrote it (#3895).
pub fn push(piece: &Piece, reach: &Reach, say: &mut dyn FnMut(Say)) -> u8 {
    let ran = Here::of(piece, true).and_then(|here| {
        let lock = broker::lock_of(&here.git_dir);
        let _held = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pushed(piece, &here, reach, say)
    });
    ran.unwrap_or_else(|why| {
        say(Say::Fail(why.into()));
        1
    })
}

/// The folder, its clone, its git directory and its branch.
struct Here {
    folder: PathBuf,
    clone: PathBuf,
    git_dir: PathBuf,
    branch: String,
    /// The folder, as a sentence names it: `<repo> · <piece>`, or `<repo>` for a clone.
    label: String,
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
        let git_dir = PathBuf::from(ran(&folder, &["rev-parse", "--absolute-git-dir"])?);
        // purlis's notes of its own updates are kept in this directory and vouch for a push
        // over the remote (#3895): it must be the clone's own record of this very folder.
        if !own_record_of(&clone, &git_dir, &folder) {
            return Err(format!(
                "{repo} · {piece} keeps its git directory somewhere other than its clone's \
                 `.git/worktrees`, so purlis does not update or push it for a chat: do it from \
                 your own terminal"
            ));
        }
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
            git_dir,
            branch,
            label: format!("{repo} · {piece}"),
        })
    }

    /// The clone `at` itself, its own checkout (#3831), refused where it is on no branch and
    /// `on_a_branch` is asked, or where its git directory is not its own `.git`.
    fn of_clone(at: &CloneAt, on_a_branch: bool) -> Result<Self, String> {
        let CloneAt { root, ws, repo } = *at;
        let clone = worktree::clone_dir(root, ws, repo).map_err(|e| e.to_string())?;
        let git_dir = PathBuf::from(ran(&clone, &["rev-parse", "--absolute-git-dir"])?);
        let own = std::fs::canonicalize(clone.join(".git")).ok();
        if !worktree::is_own_git_dir(&clone.join(".git"))
            || own.is_none()
            || own != std::fs::canonicalize(&git_dir).ok()
        {
            return Err(format!(
                "{repo} keeps its git directory somewhere other than its own `.git`, so purlis \
                 does not update it for a chat: update it from your own terminal"
            ));
        }
        let branch = ran(&clone, &["branch", "--show-current"])?;
        if branch.is_empty() && on_a_branch {
            return Err(format!(
                "{repo} is on no branch (a detached HEAD), so there is no branch to update"
            ));
        }
        Ok(Self {
            folder: clone.clone(),
            clone,
            git_dir,
            branch,
            label: repo.to_owned(),
        })
    }
}

/// Whether `git_dir` is the record the clone at `clone` keeps of the linked worktree at
/// `folder`: a directory right inside the clone's own `.git/worktrees`, whose `gitdir` names
/// that folder's `.git`. Links are followed on both sides before they are compared.
fn own_record_of(clone: &Path, git_dir: &Path, folder: &Path) -> bool {
    let canon = |path: &Path| std::fs::canonicalize(path).ok();
    if !worktree::is_own_git_dir(&clone.join(".git")) {
        return false;
    }
    let (Some(records), Some(record), Some(folder)) = (
        canon(&clone.join(".git").join("worktrees")),
        canon(git_dir),
        canon(folder),
    ) else {
        return false;
    };
    if record.parent() != Some(records.as_path()) {
        return false;
    }
    let Ok(pointer) = std::fs::read_to_string(record.join("gitdir")) else {
        return false;
    };
    Path::new(pointer.trim())
        .parent()
        .and_then(canon)
        .is_some_and(|named| named == folder)
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
fn stopped_said(here: &Here, git_said: &str, say: &mut dyn FnMut(Say)) -> u8 {
    let label = &here.label;
    match stopped_in(here) {
        Some(stopped) if stopped.operation == Some(Operation::Rebase) => {
            let files = if stopped.unmerged.is_empty() {
                String::new()
            } else {
                format!(" on a conflict in {}", stopped.unmerged.join(", "))
            };
            say(Say::Fail(
                format!(
                    "the rebase of {label} stopped{files}. Resolve them in the folder, then \
                 `purlis worktree update --continue`; or `purlis worktree update --abort` puts \
                 the branch back where it was"
                )
                .into(),
            ));
        }
        _ => say(Say::Fail(
            format!("git did not rebase {label}: {}", first_line(git_said)).into(),
        )),
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
    unstopped_and_clean(here)?;
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
    let named = worktree::fresh::named(&here.clone, &base).map_err(|e| e.to_string())?;
    if named.branch == here.branch {
        return Err(format!(
            "{base} follows the folder's own branch: name the branch it is to be brought up to"
        ));
    }
    let remote = match &named.tracking {
        Some((remote, _)) => Some(remote.clone()),
        None => remote_of(&here.clone, &named.branch),
    };
    if let Some(remote) = remote {
        crate::repocmd::sync::fetch_remote(piece.root, &here.clone, &remote, reach.hosts)
            .map_err(|why| format!("{why}. Nothing was moved"))?;
    }
    let fresh = worktree::fresh::bring_up_named(&here.clone, &named).map_err(|e| e.to_string())?;
    for note in fresh.notes {
        say(Say::Warn(note.into()));
    }
    // The base, or its remote-tracking branch where the base is behind it and checked out.
    let onto = fresh
        .instead
        .clone()
        .unwrap_or_else(|| name::as_ref(&named.branch));
    let recorded = worktree::recorded_base(&here.clone, &here.branch);
    // A base named in place of the recorded one is recorded once the piece is on it.
    let new_base =
        (recorded.as_deref() != Some(named.branch.as_str())).then(|| named.branch.clone());
    let own = Own {
        recorded: recorded.as_deref(),
        new_base: new_base.clone(),
    };
    let label = &here.label;
    let base = &named.branch;
    match rebased(piece.root, here, &onto, Leased::Piece, own, reach, say)? {
        Moved::Already => {
            if let Some(base) = &new_base {
                rerecord(here, base, say);
            }
            say(Say::Done(format!("{label} is already on {base}").into()));
        }
        Moved::Stopped(code) => return Ok(code),
        Moved::Forward | Moved::Rebased => {
            let moved = match (&fresh.moved_to, onto.strip_prefix("refs/remotes/")) {
                (Some(up), _) => format!(" ({base} brought up to {up})"),
                (None, Some(up)) => format!(" (as {up} has it)"),
                (None, None) => String::new(),
            };
            say(Say::Done(
                format!("{label} is rebased onto {base}{moved}").into(),
            ));
        }
    }
    Ok(0)
}

/// `purlis worktree update` standing in a workspace's clone (#3831): fetch the remote its
/// branch follows, and fast-forward the branch to it, or rebase the branch's own commits onto
/// it. Never with `--base`: a clone is brought up to its own branch's remote.
fn start_clone(
    at: &CloneAt,
    here: &Here,
    base: Option<&str>,
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> Result<u8, String> {
    let label = &here.label;
    if base.is_some() {
        return Err(format!(
            "{label} is a clone, brought up to the remote branch its own branch follows: \
             `--base` is for a branch folder. Nothing was moved"
        ));
    }
    unstopped_and_clean(here)?;
    let branch = &here.branch;
    let remote = remote_of(&here.clone, branch)
        .ok_or_else(|| format!("{label} has no remote to update {branch} from"))?;
    crate::repocmd::sync::fetch_remote(at.root, &here.clone, &remote, reach.hosts)
        .map_err(|why| format!("{why}. Nothing was moved"))?;
    let up = upstream_of(&here.clone, branch, &remote);
    if commit_of(&here.folder, &up).is_none() {
        return Err(format!(
            "{remote} has no branch {branch} that {label} follows, so nothing was moved"
        ));
    }
    let shown = up.strip_prefix("refs/remotes/").unwrap_or(&up).to_owned();
    match rebased(
        at.root,
        here,
        &up,
        Leased::Clone,
        Own::default(),
        reach,
        say,
    )? {
        Moved::Already => say(Say::Done(
            format!("{label} is already up to date with {shown}").into(),
        )),
        Moved::Stopped(code) => return Ok(code),
        Moved::Forward => say(Say::Done(
            format!("{label}: {branch} fast-forwarded to {shown}").into(),
        )),
        Moved::Rebased => say(Say::Done(
            format!("{label}: {branch} rebased onto {shown}").into(),
        )),
    }
    Ok(0)
}

/// The remote-tracking branch `branch` of `clone` follows: its upstream as the clone's own
/// config names it, else `<remote>/<branch>`.
fn upstream_of(clone: &Path, branch: &str, remote: &str) -> String {
    let local = name::as_ref(branch);
    let said = git::run(
        clone,
        &["for-each-ref", "--format=%(upstream)", &local],
        git::READ,
    );
    match said {
        Ok(run) if run.ok() && run.line().trim().starts_with("refs/remotes/") => {
            run.line().trim().to_owned()
        }
        _ => format!("refs/remotes/{remote}/{branch}"),
    }
}

/// Refused where git has an operation stopped in the folder, or where it holds uncommitted
/// changes: an update never moves the person's or the chat's own work out of the way.
fn unstopped_and_clean(here: &Here) -> Result<(), String> {
    let label = &here.label;
    if let Some(stopped) = stopped_in(here) {
        let what = stopped.operation.map_or("merge", Operation::word);
        return Err(format!(
            "a {what} is stopped in {label}: `purlis worktree update --continue` once its \
             files are resolved, or `purlis worktree update --abort`"
        ));
    }
    let changed = ran(
        &here.folder,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !changed.is_empty() {
        return Err(format!(
            "{label} has uncommitted changes, so nothing was moved: commit them (`purlis \
             worktree commit` in a branch folder, `git commit` in a clone), then update"
        ));
    }
    Ok(())
}

/// How an update's move ended.
enum Moved {
    /// The folder already had every commit of what it is brought up to.
    Already,
    /// The branch was a fast-forward away, and was moved there.
    Forward,
    /// The branch's own commits were replayed on top.
    Rebased,
    /// git stopped, and said why: the status to exit with.
    Stopped(u8),
}

/// Whether an update notes a lease for the branch it rewrites (#3895).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leased {
    /// A branch folder's branch, which `purlis worktree push` publishes: the update notes what
    /// the branch's remote had, so the push can go over exactly that.
    Piece,
    /// A clone's own branch, which purlis never pushes.
    Clone,
}

/// Which of a branch folder's commits are its own (#3836): the base it was recorded as cut
/// from, and the base to record for it once rebased, where that is a new one. A clone's is
/// neither, and its branch's commits after `onto` are its own.
#[derive(Default)]
struct Own<'a> {
    recorded: Option<&'a str>,
    new_base: Option<String>,
}

/// The folder's branch rebased onto `onto`: judged first for a sandboxed chat, refused where
/// it would overwrite a file on disk that the current commit does not track, and run on the
/// ids read once, whatever a ref names by the time git runs. Only its own commits move.
fn rebased(
    root: &Path,
    here: &Here,
    onto: &str,
    leased: Leased,
    own: Own<'_>,
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> Result<Moved, String> {
    let label = &here.label;
    // Each read once as a commit id, and git run on the ids: what is judged is what moves,
    // whatever a ref names by the time git runs.
    let onto_id = commit_of(&here.folder, onto)
        .ok_or_else(|| format!("{onto} names no commit here, so nothing was moved"))?;
    let head_id = commit_of(&here.folder, "HEAD")
        .ok_or_else(|| format!("{label} has no commit, so nothing was moved"))?;
    let from_id = own_commits_after(&here.folder, own.recorded, &onto_id, &head_id)?;
    if from_id == onto_id && is_ancestor(&here.folder, &onto_id, &head_id)? {
        return Ok(Moved::Already);
    }
    let forward = is_ancestor(&here.folder, &head_id, &onto_id)?;
    if let Some(chat_dir) = reach.sandboxed_at {
        let root = root.to_string_lossy();
        let place = crate::cloneupdate::Place {
            root: &root,
            chat_dir: Some(chat_dir),
            alone: true,
            // The app runs exactly this rebase, with nothing beside it.
            sparse: true,
        };
        let folder = here.folder.to_string_lossy();
        // Judged for the two ids the rebase is handed, never for what HEAD names by then.
        if let Some(why) = crate::cloneupdate::judged_rebase_from(
            &folder,
            &onto_id,
            &from_id,
            &head_id,
            &place,
            &crate::cloneupdate::Asked,
        ) {
            return Err(why);
        }
    }
    untracked_kept(here, &head_id, &onto_id)?;
    // The commits judged are those of the head read above: a branch moved since is not
    // rebased.
    if commit_of(&here.folder, "HEAD").as_deref() != Some(head_id.as_str()) {
        return Err(MOVED.to_owned());
    }
    let lease = match leased {
        Leased::Piece => lease_seen(here, &head_id, say)?,
        Leased::Clone => None,
    };
    let record = Record {
        branch: here.branch.clone(),
        head: head_id.clone(),
        lease,
    };
    record.write(&here.git_dir)?;
    NewBase::write(&here.git_dir, own.new_base.as_deref())?;
    let mut args: Vec<&str> = REBASE.to_vec();
    args.extend([
        "rebase",
        "--no-autostash",
        "--onto",
        &onto_id,
        &from_id,
        &head_id,
    ]);
    // Untimed: a rebase checks trees out, and one killed halfway is left half done.
    let went = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    if !went.ok() {
        return Ok(Moved::Stopped(stopped_said(here, &went.err, say)));
    }
    finish(here, &record)?;
    NewBase::recorded(here, say);
    Ok(if forward {
        Moved::Forward
    } else {
        Moved::Rebased
    })
}

/// `base` recorded as the branch the folder's branch was cut from, now that it is on it
/// (#3836); said where git did not write it, since the branch is moved either way.
fn rerecord(here: &Here, base: &str, say: &mut dyn FnMut(Say)) {
    if let Err(why) = worktree::record_base(&here.clone, &here.branch, base) {
        say(Say::Warn(
            format!(
                "{} is on {base}, and purlis could not record that as its base ({why}): its \
                 Changes and merge still count from the base recorded before",
                here.branch
            )
            .into(),
        ));
    }
}

/// The commit after which the piece's own commits start, as a rebase's upstream (#3836):
/// where `head` left the base it was recorded as cut from, where that is not already in
/// `onto`; otherwise `onto` itself, as a plain rebase onto it. A piece cut from a base whose
/// commits `onto` does not have (a clone's branch that moved apart from its remote, or another
/// branch) carries only the commits after it, as `git rebase --onto <onto> <old base>` would.
fn own_commits_after(
    folder: &Path,
    recorded: Option<&str>,
    onto_id: &str,
    head_id: &str,
) -> Result<String, String> {
    let Some(old) = recorded.and_then(|base| commit_of(folder, &name::as_ref(base))) else {
        return Ok(onto_id.to_owned());
    };
    let fork =
        git::run(folder, &["merge-base", &old, head_id], git::READ).map_err(|e| e.to_string())?;
    let fork = fork.line().trim();
    if fork.is_empty() || !full_id(fork) {
        return Ok(onto_id.to_owned());
    }
    Ok(if is_ancestor(folder, fork, onto_id)? {
        onto_id.to_owned()
    } else {
        fork.to_owned()
    })
}

/// Whether `old` is an ancestor of `new`, both commit ids, in `dir`.
fn is_ancestor(dir: &Path, old: &str, new: &str) -> Result<bool, String> {
    let there = git::run(dir, &["merge-base", "--is-ancestor", old, new], git::READ)
        .map_err(|e| e.to_string())?;
    Ok(there.ok())
}

/// The most paths a refusal names.
const NAMED_AT_MOST: usize = 5;

/// Refused where the move would write a path that is on disk and that the current commit does
/// not track: an ignored file (a `.env`, a local config), which git overwrites without a word,
/// or an untracked one, which git stops on halfway (#3831). Either is someone's work.
fn untracked_kept(here: &Here, head_id: &str, onto_id: &str) -> Result<(), String> {
    let added = git::run(
        &here.folder,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=A",
            head_id,
            onto_id,
        ],
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if !added.ok() {
        return Err(format!(
            "git could not say what the update writes: {}. Nothing was moved",
            first_line(&added.err)
        ));
    }
    let held: Vec<&str> = added
        .out
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter(|path| here.folder.join(path).symlink_metadata().is_ok())
        .collect();
    if held.is_empty() {
        return Ok(());
    }
    let mut named = held
        .iter()
        .take(NAMED_AT_MOST)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if held.len() > NAMED_AT_MOST {
        named.push_str(&format!(" and {} more", held.len() - NAMED_AT_MOST));
    }
    Err(format!(
        "the update would overwrite {named} in {}, which {} on disk and not committed. \
         Nothing was moved: move or commit them, then update",
        here.label,
        if held.len() == 1 { "is" } else { "are" }
    ))
}

/// A line of a file that git left a conflict in, as git writes the first marker.
const MARKER: &[u8] = b"<<<<<<< ";

fn go_on(here: &Here, say: &mut dyn FnMut(Say)) -> Result<u8, String> {
    let label = &here.label;
    let Some(stopped) =
        stopped_in(here).filter(|stopped| stopped.operation == Some(Operation::Rebase))
    else {
        return Err(format!(
            "no rebase is stopped in {label}: `purlis worktree update` starts one"
        ));
    };
    let record = Record::of_the_stopped_rebase(&here.git_dir).ok_or_else(|| {
        format!(
            "the rebase stopped in {label} is not one purlis started: finish it from \
             your own terminal"
        )
    })?;
    let files = stopped.unmerged;
    if !files.is_empty() {
        // Each resolved file is staged as the bytes looked at for a marker, read through its
        // one open descriptor, so nothing is read again by its name.
        // As a save: a resolved file with a second name is refused through its descriptor, so
        // its bytes never reach the clone's object store (`planegit::ondisk`).
        let rows = resolved(&here.folder, &files)?;
        let held = ondisk::stage_rows(&here.folder, rows).map_err(|said| {
            format!(
                "git did not stage the resolved files: {}",
                first_line(&said)
            )
        })?;
        if let Some(refusal) = ondisk::refused_unstaged(&held, ondisk::PutBack::Conflicted(&files))
        {
            return Err(not_the_file(&refusal));
        }
        // Staged: where the index cannot be refreshed, whether each is the file on disk cannot
        // be asked, and every one goes back in conflict.
        if let Err(said) = ondisk::refresh(&here.folder, false)
            && let Some(refusal) = ondisk::refused_into(
                &here.folder,
                &Err(said),
                ondisk::PutBack::Conflicted(&files),
            )
        {
            return Err(not_the_file(&refusal));
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
            "no rebase is stopped in {label}: `purlis worktree update` starts one"
        ));
    }
    if !went.ok() {
        return Ok(stopped_said(here, &went.err, say));
    }
    finish(here, &record)?;
    NewBase::recorded(here, say);
    say(Say::Done(format!("{label} is rebased").into()));
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
/// writes: the branch it rebases, the commit that branch was at when it was judged, and what
/// the branch's remote had then, where the update may push over it.
struct Record {
    branch: String,
    head: String,
    lease: Option<Lease>,
}

/// What the branch's remote had when an update rewrote the branch (#3895): the remote, and
/// the commit its copy of the branch was at. Noted only where the folder had every commit of
/// that copy, so a push over it drops nothing but what the update itself rewrote.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Lease {
    remote: String,
    at: String,
}

/// The file [`Record`] is kept in while an update is under way.
const UNDER_WAY: &str = "purlis-update";

/// The file that names, while an update is under way, the base to record for the folder's
/// branch once it is rebased, where that is a new one (#3836): kept apart from [`Record`],
/// whose lines are the lease's too.
const NEW_BASE: &str = "purlis-update-base";

/// The base an update records for the folder's branch once it is on it (#3836).
struct NewBase;

impl NewBase {
    fn write(git_dir: &Path, base: Option<&str>) -> Result<(), String> {
        let path = git_dir.join(NEW_BASE);
        match base {
            Some(base) => std::fs::write(path, format!("{base}\n"))
                .map_err(|why| format!("purlis could not note the base it moves onto: {why}")),
            None => {
                let _ = std::fs::remove_file(path);
                Ok(())
            }
        }
    }

    /// The noted base recorded, now that the rebase is done, and the note removed.
    fn recorded(here: &Here, say: &mut dyn FnMut(Say)) {
        let path = here.git_dir.join(NEW_BASE);
        let noted = std::fs::read_to_string(&path).ok();
        let _ = std::fs::remove_file(&path);
        if let Some(base) = noted
            .as_deref()
            .map(str::trim)
            .filter(|base| name::branch_name_ok(base).is_ok())
        {
            rerecord(here, base, say);
        }
    }
}

/// The file that says which commit purlis's last update left the branch at, and what the
/// branch's remote had then: what a push over the remote's copy is leased to (#3895).
const UPDATED: &str = "purlis-updated";

impl Record {
    /// The record's lines: the branch and the commit, then the remote and its commit where the
    /// update noted a lease.
    fn text(branch: &str, head: &str, lease: Option<&Lease>) -> String {
        match lease {
            Some(Lease { remote, at }) => format!("{branch}\n{head}\n{remote}\n{at}\n"),
            None => format!("{branch}\n{head}\n"),
        }
    }

    fn write(&self, git_dir: &Path) -> Result<(), String> {
        std::fs::write(
            git_dir.join(UNDER_WAY),
            Self::text(&self.branch, &self.head, self.lease.as_ref()),
        )
        .map_err(|why| format!("purlis could not note the update it starts: {why}"))
    }

    /// The record purlis's last update left in `git_dir` ([`UPDATED`]): the branch, the commit
    /// the update left it at, and the lease it noted. `None` where there is none, or it is not
    /// one this purlis wrote (an earlier one wrote the commit alone).
    fn updated(git_dir: &Path) -> Option<Self> {
        Self::parse(&std::fs::read_to_string(git_dir.join(UPDATED)).ok()?)
    }

    fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let (branch, head) = (lines.next()?.to_owned(), lines.next()?.to_owned());
        if name::branch_name_ok(&branch).is_err() || !full_id(&head) {
            return None;
        }
        let lease = match (lines.next(), lines.next()) {
            (None, None) => None,
            (Some(remote), Some(at)) if remote_name_ok(remote) && full_id(at) => Some(Lease {
                remote: remote.to_owned(),
                at: at.to_owned(),
            }),
            _ => return None,
        };
        Some(Self {
            branch,
            head,
            lease,
        })
    }

    /// The record of the rebase stopped in `git_dir` now, only where that rebase is the one
    /// purlis started (#3831): git's own note of the commit it started from (`orig-head`) is the
    /// commit purlis recorded. A record left from an earlier update never vouches for a rebase
    /// the person started since.
    fn of_the_stopped_rebase(git_dir: &Path) -> Option<Self> {
        let record = Self::read(git_dir)?;
        let started_from = ["rebase-merge", "rebase-apply"].iter().find_map(|dir| {
            std::fs::read_to_string(git_dir.join(dir).join("orig-head"))
                .ok()
                .map(|text| text.trim().to_owned())
        })?;
        (started_from == record.head).then_some(record)
    }

    fn read(git_dir: &Path) -> Option<Self> {
        Self::parse(&std::fs::read_to_string(git_dir.join(UNDER_WAY)).ok()?)
    }
}

/// What the remote the folder's branch is pushed to has of it now, as the update's fetch left
/// it, where a push may later go over it (#3895): `None` where the remote has no such branch,
/// or where its copy holds commits that `head` does not, unless an earlier update of purlis's,
/// not pushed since, vouched for that very copy and `head` holds what it made. Otherwise that
/// copy is someone's work the rebase never saw, so the update says so and vouches for nothing.
fn lease_seen(here: &Here, head: &str, say: &mut dyn FnMut(Say)) -> Result<Option<Lease>, String> {
    let branch = &here.branch;
    let Some(remote) = remote_of(&here.clone, branch) else {
        return Ok(None);
    };
    let Some(at) = commit_of(&here.clone, &format!("refs/remotes/{remote}/{branch}")) else {
        return Ok(None);
    };
    if !is_ancestor(&here.folder, &at, head)? {
        // A second update before the first was pushed: the remote's copy is still the one the
        // first update saw and vouched for, and the branch still holds what that update made.
        if let Some(Record {
            branch: noted,
            head: made,
            lease: Some(lease),
        }) = Record::updated(&here.git_dir)
            && noted == *branch
            && lease.remote == remote
            && lease.at == at
            && is_ancestor(&here.folder, &made, head)?
        {
            return Ok(Some(lease));
        }
        say(Say::Warn(
            format!(
                "{remote}'s {branch} has commits that {} did not have, so purlis will not push \
                 over them: bring them in first, from your own terminal",
                here.label
            )
            .into(),
        ));
        return Ok(None);
    }
    Ok(Some(Lease { remote, at }))
}

/// Whether `remote` reads as a remote's name as `git remote add` takes one: not empty, not
/// an option, and no space, control character, `:` or `..`, nor any character a ref may not
/// hold, since git keeps its branches under `refs/remotes/<remote>/`.
fn remote_name_ok(remote: &str) -> bool {
    !remote.is_empty()
        && !remote.starts_with(['-', '.', '/'])
        && !remote.ends_with(['.', '/'])
        && !remote.contains("..")
        && !remote.contains("//")
        && !remote.ends_with(".lock")
        && !remote.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, ':' | '~' | '^' | '?' | '*' | '[' | '\\' | '@')
        })
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
    std::fs::write(
        here.git_dir.join(UPDATED),
        Record::text(&record.branch, &new, record.lease.as_ref()),
    )
    .map_err(|why| format!("purlis could not note the update it made: {why}"))
}

/// The files git named as conflicted, each as it is staged, or why they may not be staged for
/// a chat: each a name inside the folder reached through no link, not another repository nor
/// the file that declares them, not marked for a content filter the person's own git would
/// run, and with no conflict marker left in it. A file is looked at through its one open
/// descriptor, never through a link, and staged from that same descriptor
/// ([`ondisk::stage_rows`]).
fn resolved(folder: &Path, files: &[String]) -> Result<Vec<ondisk::Row>, String> {
    use std::io::Seek;
    broker::paths_ok(files)?;
    let mut rows = Vec::with_capacity(files.len());
    for file in files {
        broker::through_no_link(folder, file)?;
        let name = file.rsplit('/').next().unwrap_or(file);
        if name.eq_ignore_ascii_case(".gitmodules") {
            return Err(format!(
                "{file} declares submodules, and purlis does not resolve it for a chat: finish \
                 the rebase from your own terminal"
            ));
        }
        let from = match ondisk::opened(folder, file) {
            ondisk::OnDisk::File(opened, runs) => {
                // Read through once for a marker, then from the start again as it is staged:
                // a file written over in between is no longer the file the index was told of,
                // and is put back in conflict below.
                let read = |why: std::io::Error| format!("{file} cannot be read: {why}");
                if holds_marker(&opened).map_err(read)? {
                    return Err(still_marked(file));
                }
                (&opened).seek(std::io::SeekFrom::Start(0)).map_err(read)?;
                ondisk::Source::File {
                    file: opened,
                    runs,
                    checked: None,
                }
            }
            ondisk::OnDisk::SecondName => {
                let held = Ok(vec![ondisk::Held {
                    path: file.clone(),
                    why: ondisk::Why::SecondName,
                }]);
                return Err(ondisk::refused_ahead(&held)
                    .map(|refusal| not_the_file(&refusal))
                    .unwrap_or_default());
            }
            ondisk::OnDisk::Folder { .. } => return Err(a_folder(file)),
            ondisk::OnDisk::Link(target) => ondisk::Source::Link(target),
            ondisk::OnDisk::Gone | ondisk::OnDisk::Beyond => ondisk::Source::Gone,
            // Where a folder cannot be held open, looked at by its name, as `git add` stages it.
            ondisk::OnDisk::ByGit => {
                let at = folder.join(file);
                match std::fs::symlink_metadata(&at) {
                    Ok(meta) if meta.is_dir() => return Err(a_folder(file)),
                    Ok(meta) if meta.is_file() => {
                        let read = |why: std::io::Error| format!("{file} cannot be read: {why}");
                        if holds_marker(&std::fs::File::open(&at).map_err(read)?).map_err(read)? {
                            return Err(still_marked(file));
                        }
                    }
                    _ => {}
                }
                ondisk::Source::ByGit
            }
            ondisk::OnDisk::Odd => {
                return Err(format!(
                    "{file} is neither a file, a link nor a folder, so purlis does not stage it \
                     for a chat: finish the rebase from your own terminal"
                ));
            }
            ondisk::OnDisk::Unread(why) => {
                return Err(format!("{file} cannot be read: {why}"));
            }
        };
        rows.push(ondisk::Row {
            path: file.clone(),
            from,
        });
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
    Ok(rows)
}

/// What a resolved file that still holds a conflict marker is refused with.
fn still_marked(file: &str) -> String {
    format!(
        "{file} still holds a conflict marker (`<<<<<<<`): resolve it, then `purlis worktree \
         update --continue`"
    )
}

/// What a conflict git left on a folder is refused with.
fn a_folder(file: &str) -> String {
    format!(
        "{file} is a folder where git left a conflict (another repository, most often): finish \
         the rebase from your own terminal"
    )
}

/// Whether what `file` reads, from where its descriptor stands, holds a conflict marker at the
/// start of a line ([`MARKER`]): read a part at a time, so a large file is never held whole.
fn holds_marker(mut file: &std::fs::File) -> std::io::Result<bool> {
    use std::io::Read;
    let mut wanted = vec![b'\n'];
    wanted.extend_from_slice(MARKER);
    // A line starts the file, so the file is read as if a line ended just before it.
    let mut window = vec![b'\n'];
    let mut part = vec![0; 64 * 1024];
    loop {
        let read = match file.read(&mut part) {
            Ok(0) => return Ok(false),
            Ok(read) => read,
            Err(why) if why.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(why) => return Err(why),
        };
        window.extend_from_slice(&part[..read]);
        if window.windows(wanted.len()).any(|w| w == wanted.as_slice()) {
            return Ok(true);
        }
        // What a marker split across two reads needs of this one.
        let kept = window.len().saturating_sub(wanted.len() - 1);
        window.drain(..kept);
    }
}

fn abort(here: &Here, say: &mut dyn FnMut(Say)) -> Result<u8, String> {
    let label = &here.label;
    if !stopped_in(here).is_some_and(|stopped| stopped.operation == Some(Operation::Rebase)) {
        return Err(format!("no rebase is stopped in {label}"));
    }
    // Only a rebase purlis started for an update, as `--continue` holds it: one the person
    // started is theirs to finish or abort.
    if Record::of_the_stopped_rebase(&here.git_dir).is_none() {
        return Err(format!(
            "the rebase stopped in {label} is not one purlis started: finish or abort it from \
             your own terminal"
        ));
    }
    let mut args: Vec<&str> = REBASE.to_vec();
    args.extend(["rebase", "--abort"]);
    let back = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    // git's own answer settles whether one was stopped, whatever the markers said.
    if back.ok()
        && let Some(record) = Record::read(&here.git_dir)
    {
        let _ = std::fs::remove_file(here.git_dir.join(UNDER_WAY));
        let _ = std::fs::remove_file(here.git_dir.join(NEW_BASE));
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
        return Err(format!("no rebase is stopped in {label}"));
    }
    if !back.ok() {
        return Err(format!(
            "git did not put {label} back: {}",
            first_line(&back.err)
        ));
    }
    say(Say::Done(
        format!("{label} is back where it was before the update").into(),
    ));
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
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> Result<u8, String> {
    let (repo, label) = (piece.repo, &here.label);
    let branch = &here.branch;
    if stopped_in(here).is_some() {
        return Err(format!(
            "an operation is stopped in {label}: finish it (`purlis worktree update \
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
    let ours =
        commit_of(&here.folder, "HEAD").ok_or_else(|| format!("{label} has no commit to push"))?;
    let theirs = commit_of(&here.clone, &format!("refs/remotes/{remote}/{branch}"));
    // Over the remote's copy only where purlis's own update rewrote the branch (#3895).
    let over = match &theirs {
        Some(theirs) if !is_ancestor(&here.folder, theirs, &ours)? => Some(theirs.clone()),
        _ => None,
    };
    if let Some(theirs) = &over {
        vouched(here, &remote, theirs, &ours)?;
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
    // Leased to the very commit the update saw, which [`vouched`] matched to what was fetched:
    // a remote moved since is refused by the remote itself.
    let leased = over
        .as_deref()
        .map(|theirs| format!("--force-with-lease={local_ref}:{theirs}"));
    let mut args = vec!["push"];
    if let Some(leased) = &leased {
        args.push(leased);
    }
    args.extend([remote.as_str(), refspec.as_str()]);
    // Its Git LFS objects first: a hook would have sent them, and purlis's git runs none (#4399).
    crate::lfsupload::before_push(&crate::lfsupload::Push {
        dir: &here.folder,
        helper: helper.as_deref(),
        args: &args,
        tip: &ours,
        remote: &remote,
    })?;
    let sent =
        git::run_network(&here.folder, helper.as_deref(), &args).map_err(|e| e.to_string())?;
    if !sent.ok() {
        return Err(format!(
            "git did not push {branch} to {remote}: {}",
            first_line(&sent.err)
        ));
    }
    if over.is_some() {
        // Spent: the remote has the rewrite now, and a later rewrite needs an update of its own.
        let note = here.git_dir.join(UPDATED);
        if let Err(why) = std::fs::remove_file(&note)
            && why.kind() != std::io::ErrorKind::NotFound
        {
            say(Say::Warn(
                format!(
                    "purlis could not remove its note of the update it pushed ({}): {why}",
                    note.display()
                )
                .into(),
            ));
        }
    }
    say(Say::Done(
        format!("{label}: {branch} pushed to {remote}").into(),
    ));
    Ok(0)
}

/// Refused unless purlis's own update rewrote the branch (#3895): its record names this
/// branch, the commit it left the branch at is in what is pushed (commits added on top are
/// fine), and the remote's copy, `theirs`, is still exactly the one the update saw and had
/// every commit of. Where the record cannot be read, the push is refused, naming the retry.
fn vouched(here: &Here, remote: &str, theirs: &str, ours: &str) -> Result<(), String> {
    let branch = &here.branch;
    let by_hand = format!(
        "{remote}'s {branch} has commits that {branch} here does not, and purlis's own update \
         did not rewrite it to where it is now, so purlis does not push over them. Nothing was \
         pushed. To keep them, bring them in first; to replace {remote}'s copy with yours, push \
         from your own terminal: `git push --force-with-lease=refs/heads/{branch}:{theirs} \
         {remote} {branch}`"
    );
    let Some(record) = Record::updated(&here.git_dir) else {
        return Err(by_hand);
    };
    if record.branch != *branch || !is_ancestor(&here.folder, &record.head, ours)? {
        return Err(by_hand);
    }
    let Some(lease) = record.lease.filter(|lease| lease.remote == remote) else {
        return Err(format!(
            "when purlis's update rewrote {branch}, {remote}'s copy had commits that {} did \
             not, so purlis does not push over them. Nothing was pushed. Bring them in first, \
             from your own terminal",
            here.label
        ));
    };
    if lease.at != theirs {
        return Err(format!(
            "{remote}'s {branch} moved since purlis's update saw it (it was at {}, it is at \
             {theirs} now): someone else pushed to it. Nothing was pushed. Bring their commits \
             in first, from your own terminal",
            lease.at
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file holding `bytes`, opened to read from its start.
    fn holding(bytes: &[u8]) -> (tempfile::NamedTempFile, std::fs::File) {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut file, bytes).unwrap();
        let read = std::fs::File::open(file.path()).unwrap();
        (file, read)
    }

    #[test]
    fn a_marker_is_found_at_the_start_of_any_line_and_across_two_reads() {
        let (_kept, start) = holding(b"<<<<<<< ours\n");
        assert!(holds_marker(&start).unwrap());
        // A marker that starts two bytes before one read ends.
        let mut split = vec![b'a'; 64 * 1024 - 3];
        split.extend_from_slice(b"\n<<<<<<< theirs\n");
        let (_kept, split) = holding(&split);
        assert!(holds_marker(&split).unwrap());
        let (_kept, inside) = holding(b"text with <<<<<<< in a line\n");
        assert!(!holds_marker(&inside).unwrap());
        let (_kept, none) = holding(b"");
        assert!(!holds_marker(&none).unwrap());
    }
}

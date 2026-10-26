// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter worktree` (alias `wt`): cut a piece, declare it done or abandoned, list what exists,
//! read what happened, and remove one — `charter/commands_worktree.py` at `cli-final`
//! (charter#368).
//!
//! **Git decides and the log remembers.** Every mutation is git's: [`crate::worktree::add`]
//! and [`crate::worktree::remove`], through the hardened runner, never a folder deleted by
//! hand. What git cannot know — that charter cut the piece, and what its worker said about it —
//! is appended to the piece log ([`crate::pieces::record`]), and nothing git could answer is
//! written there (ADR 0011).
//!
//! Each verb speaks through a [`Say`] sink and returns its exit code, as the repo commands do,
//! so the binary prints each line as it is known and a test reads the same lines as values.
//!
//! # Where this differs from Python, on purpose
//!
//! - **`--branch` names the NEW branch.** Python's checked out an existing branch.
//!   [`crate::worktree::add`] always cuts a fresh branch, because the branch a piece was cut
//!   from is recorded at cut time and an existing branch has no such record.
//! - **A piece is cut from the remote's default branch** (qw387), or `--base <branch>`, brought
//!   up to its remote where nothing has it checked out ([`crate::worktree::fresh`]); `--base
//!   HEAD` is the clone's HEAD, which is all Python cut from.
//! - **A removal names what it would discard.** Python counted unique commits and said
//!   "uncommitted changes". The refusal here lists the changed paths and the commits, because
//!   `--force` is an informed choice only when the operator can see what it takes.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::pieces::{self, Declaration, Event, Who};
use crate::repocmd::Say;
use crate::tui;
use crate::worktree::{self, Dirt, Refusal};

/// The exit code for "this piece is already claimed", distinct from the generic 1. A worker
/// that loses a race takes the next name from its plan, and must not have to parse English to
/// know that is what happened — nor mistake an invalid name for a lost race.
pub const CLAIM_TAKEN: u8 = 2;

/// What `--base` is given for the clone's own HEAD, whatever branch it is on.
pub const HEAD: &str = "HEAD";

/// The gap between two columns of a listing.
const GAP: usize = 2;

/// Who already holds this piece, or `None` when it is free: its directory exists, or its
/// branch is checked out in another live worktree — git refuses both, and both are the same
/// collision under two names. A branch that merely EXISTS is not a holder: nobody is working
/// in it.
fn holder(plane: &Path, ws: &str, repo: &str, piece: &str, branch: &str) -> Option<String> {
    let path = worktree::path_for(plane, ws, repo, piece).ok()?;
    if path.symlink_metadata().is_ok() {
        return Some(format!("a worktree at {}", shown(plane, &path)));
    }
    worktree::list(plane, ws, repo)
        .ok()?
        .into_iter()
        .find(|p| p.prunable.is_none() && p.branch.as_deref() == Some(branch))
        .map(|p| format!("piece '{}'", p.piece))
}

/// A path as the operator reads it: relative to the plane when it is inside it.
fn shown(plane: &Path, path: &Path) -> String {
    path.strip_prefix(plane)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn taken(piece: &str, held: &str, say: &mut dyn FnMut(Say)) -> u8 {
    say(Say::Fail(
        format!("'{piece}' is already claimed — held by {held}.").into(),
    ));
    say(Say::Info(
        "Take the next unclaimed piece from the plan, or work in the existing worktree.".into(),
    ));
    CLAIM_TAKEN
}

/// `charter worktree add <repo> <piece> [--base <branch>]`: cut a piece off branch `base`
/// brought up to its remote, or with no base off the branch the remote names its default where
/// the clone has one (qw387); off the clone's HEAD for `--base HEAD`, or where there is no
/// default. Wire the layer, and log `claimed`.
#[allow(clippy::too_many_arguments)]
pub fn add(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
    branch: Option<&str>,
    base: Option<&str>,
    vet: Option<worktree::Vet<'_>>,
    hosts: Option<&[String]>,
    who: &Who,
    now: DateTime<Utc>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    say(Say::Info(format!("workspace: {ws}").into()));
    let branch_name = branch.unwrap_or(piece);
    if let Some(held) = holder(plane, ws, repo, piece, branch_name) {
        return taken(piece, &held, say);
    }
    let base = match base {
        Some(HEAD) => None,
        Some(base) => Some(base.to_owned()),
        // Asked of the remote, never read from the clone; a clone with no default of a
        // branch it has, or no answer, is cut from its HEAD.
        None => worktree::clone_dir(plane, ws, repo).ok().and_then(|clone| {
            worktree::fresh::remote_default(plane, &clone, "origin", hosts)
                .filter(|branch| git_has(&clone, &worktree::name::as_ref(branch)))
        }),
    };
    let cut = match base.as_deref() {
        Some(base) => worktree::add_based(plane, ws, repo, piece, branch, base, vet),
        None => worktree::add_vetted(plane, ws, repo, piece, branch, None, vet)
            .map(|added| (added, worktree::fresh::Fresh::default())),
    };
    let (added, fresh) = match cut {
        Ok(cut) => cut,
        Err(refusal) => {
            // The check above is not the mutex — git is. A worker that won between that check
            // and git's own lock leaves a failure that reads like a broken repo unless charter
            // looks again, so it looks: the cause is read off reality, not off git's English.
            if let Some(held) = holder(plane, ws, repo, piece, branch_name) {
                return taken(piece, &held, say);
            }
            say(Say::Fail(refusal.to_string().into()));
            return 1;
        }
    };
    // The worktree is the claim; this records who took it. Best-effort: a claim git granted
    // is not undone by a log that could not be written — but it is said.
    if pieces::record(plane, ws, Event::Claimed, repo, piece, None, who, now).is_none() {
        say(Say::Warn(
            format!(
                "the piece log could not be written, so this claim is not recorded: {}",
                shown(plane, &pieces::log_path(plane, ws, who.file()))
            )
            .into(),
        ));
    }
    for warning in &added.warnings {
        say(Say::Warn(warning.to_string().into()));
    }
    say(Say::Done(
        format!("{repo} · {piece} → {}", shown(plane, &added.path)).into(),
    ));
    for note in &fresh.notes {
        say(Say::Warn(note.clone().into()));
    }
    let shown_base = match (&added.base, &fresh.moved_to) {
        (worktree::Base::Branch(b), Some(up)) => format!("{b} (brought up to {up})"),
        (worktree::Base::Branch(b), None) => b.clone(),
        (worktree::Base::Detached(sha), _) => format!("{sha} (detached)"),
    };
    say(Say::Info(format!("  base:   {shown_base}").into()));
    say(Say::Info(format!("  branch: {}", added.branch).into()));
    say(Say::Info(
        format!(
            "  enter:  cd {} && claude    (or hand this path to EnterWorktree)",
            shown(plane, &added.path)
        )
        .into(),
    ));
    say(Say::Info(
        "  When the work is finished: purlis worktree done — or purlis worktree abandon \
         \"<why>\" if it cannot be."
            .into(),
    ));
    0
}

/// `charter worktree done` / `abandon <reason>`: declare the piece `cwd` stands in.
///
/// Neither verb takes a piece argument, and that absence is the design: a worker naming its
/// own piece is a worker that can name someone else's. The piece is read off the directory.
pub fn declare(
    plane: &Path,
    cwd: &Path,
    what: Declaration<'_>,
    who: &Who,
    now: DateTime<Utc>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    let Some(here) = worktree::locate(plane, cwd) else {
        say(Say::Fail(
            "Not inside a worktree — `done` and `abandon` declare the piece you are standing \
             in, so there is nothing here to declare."
                .into(),
        ));
        say(Say::Info(
            "cd into the piece first; `purlis worktree list` shows where they are.".into(),
        ));
        return 1;
    };
    let (ws, repo, piece) = (&here.workspace, &here.repo, &here.piece);
    match pieces::declare(plane, ws, repo, piece, what, who, now) {
        Ok(declared) => {
            if let Some(over) = &declared.over {
                say(Say::Warn(already_declared(piece, over, declared.event)));
            }
            say(Say::Done(declared_line(repo, piece, declared.event, what)));
            0
        }
        Err(why) => {
            say(Say::Fail(why.to_string().into()));
            1
        }
    }
}

/// The warning that `piece` was declared `over` before, and `event` is recorded over it.
///
/// `over` is read from the log, which any host's worker writes, so it is spliced escaped
/// ([`tui::Markup`], SK-50): an SGR in it is shown as text and never obeyed.
fn already_declared(piece: &str, over: &str, event: Event) -> tui::Markup {
    tui::Markup::new()
        .own("'")
        .text(piece)
        .own("' was already declared ")
        .text(over)
        .own(" — recording ")
        .own(event.word())
        .own(" over it (the earlier one stays in the log).")
}

/// `<repo> · <piece> — done`, or `— abandoned: <reason>`.
///
/// The repo and the piece are names read off the directory, and the reason is what the worker
/// typed, so each is spliced escaped (SK-54, #3225): a control, a bidi or a zero-width
/// character in any of them is shown as its mark. The reason used to keep its SGR.
fn declared_line(repo: &str, piece: &str, event: Event, what: Declaration<'_>) -> tui::Markup {
    let line = tui::Markup::new()
        .text(repo)
        .own(" · ")
        .text(piece)
        .own(" — ")
        .own(event.word());
    match what {
        Declaration::Abandoned { reason } => line.own(": ").text(reason.trim()),
        Declaration::Done => line,
    }
}

/// `charter worktree list [<repo>]`: every piece git has, joined to what the log says of it.
///
/// Rows come from git and only from git; the log only puts a name and a word on what git
/// found. `state` has three answers and a fourth: `clean`, `dirty`, `unknown` when git could
/// not say — never `clean` for a tree charter could not read — and `missing` for a
/// registration whose directory is gone.
pub fn list(
    plane: &Path,
    ws: &str,
    repo: Option<&str>,
    now: DateTime<Utc>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    say(Say::Info(format!("workspace: {ws}").into()));
    let repos: Vec<String> = match repo {
        Some(repo) => vec![repo.to_string()],
        None => match crate::repos::clones(plane, ws) {
            Ok(found) => found.repos.into_iter().map(|r| r.name).collect(),
            Err(why) => {
                say(Say::Fail(why.to_string().into()));
                return 1;
            }
        },
    };
    let claims = pieces::claims(plane, ws);
    let mut code = 0;
    let mut total = 0;
    for repo in repos {
        let found = match worktree::list(plane, ws, &repo) {
            Ok(found) => found,
            Err(why) => {
                say(Say::Fail(format!("{repo}: {why}").into()));
                code = 1;
                continue;
            }
        };
        if found.is_empty() {
            continue;
        }
        say(Say::Info(repo.clone().into()));
        let rows: Vec<[String; 5]> = found
            .iter()
            .map(|p| {
                let state = if p.prunable.is_some() {
                    "missing"
                } else {
                    match worktree::dirt_of(&p.path) {
                        Dirt::Clean => "clean",
                        Dirt::Dirty => "dirty",
                        Dirt::Unknown => "unknown",
                    }
                };
                let branch = p
                    .branch
                    .clone()
                    .unwrap_or_else(|| format!("detached {}", p.path.display()));
                let who = pieces::claimant(claims.get(&(repo.clone(), p.piece.clone())));
                let said = pieces::said(plane, ws, &repo, &p.piece, now);
                [p.piece.clone(), branch, state.to_string(), who, said]
            })
            .collect();
        table(&rows, say);
        total += rows.len();
    }
    if total == 0 && code == 0 {
        say(Say::Info(
            format!("No worktrees. Create one: purlis worktree add <repo> <piece> -w {ws}").into(),
        ));
    }
    code
}

/// `charter worktree history [<repo> [<piece>]]`: what happened to this workspace's pieces,
/// read from the log alone — a piece whose worktree is long gone is the main reason to ask.
pub fn history(
    plane: &Path,
    ws: &str,
    repo: Option<&str>,
    piece: Option<&str>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    say(Say::Info(format!("workspace: {ws}").into()));
    // The log is read from `workspaces/<ws>/pieces`, so the name is checked — and the
    // workspace's directory asked for, a link there refused — before anything is joined.
    if let Err(why) = worktree::confine::workspace_dir(plane, ws) {
        say(Say::Fail(why.to_string().into()));
        return 1;
    }
    let field = |e: &Value, k: &str| {
        e.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let rows: Vec<[String; 5]> = pieces::events(plane, ws)
        .into_iter()
        .filter(|e| repo.is_none_or(|r| field(e, "repo") == r))
        .filter(|e| piece.is_none_or(|p| field(e, "piece") == p))
        .map(|e| {
            let reason = field(&e, "reason");
            let who = pieces::claimant(Some(&e));
            let tail = if reason.is_empty() {
                who
            } else {
                format!("{who} — {reason}")
            };
            [
                field(&e, "ts"),
                field(&e, "repo"),
                field(&e, "piece"),
                field(&e, "event"),
                tail,
            ]
        })
        .collect();
    if rows.is_empty() {
        let scope = match (repo, piece) {
            (Some(r), Some(p)) => format!(" for {r}/{p}"),
            (Some(r), None) => format!(" for {r}"),
            _ => String::new(),
        };
        say(Say::Info(
            format!(
                "No piece history recorded{scope}. Claims are recorded by `purlis worktree add`."
            )
            .into(),
        ));
        return 0;
    }
    table(&rows, say);
    0
}

/// `charter worktree remove <repo> <piece>`: `git worktree remove`, refused while the piece
/// holds uncommitted changes or commits no other ref reaches — each named — unless `force`.
///
/// **Never a branch folder purlis cut for a task** (#1534), `force` or not, with the explorer's
/// two exceptions, and never with `delete_branch`: [`crate::dispatchplace::kept_from_removal`].
pub fn remove(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
    force: bool,
    delete_branch: bool,
    say: &mut dyn FnMut(Say),
) -> u8 {
    say(Say::Info(format!("workspace: {ws}").into()));
    let no_discard_there = || {
        let clone = plane.join("workspaces").join(ws).join(repo);
        worktree::git::isolated(&worktree::git::Isolated::operators(), || {
            crate::gitbroker::runs_a_program(&clone).is_err()
        })
    };
    if let Some(refused) = crate::dispatchplace::kept_from_removal(
        plane,
        ws,
        repo,
        piece,
        delete_branch,
        no_discard_there,
    ) {
        say(Say::Fail(refused.into()));
        return 1;
    }
    match worktree::remove(plane, ws, repo, piece, force, delete_branch) {
        Ok(removed) => {
            if removed.was_stale {
                say(Say::Done(
                    format!(
                        "Cleared stale registration for {repo} · {piece} (its worktree directory \
                     was already gone)."
                    )
                    .into(),
                ));
            } else {
                say(Say::Done(format!("Removed {repo} · {piece}").into()));
            }
            if delete_branch {
                match &removed.branch {
                    Some(b) if removed.branch_deleted => {
                        say(Say::Done(format!("Deleted branch {b}").into()));
                    }
                    Some(b) => say(Say::Warn(
                        format!(
                            "Worktree removed, but branch {b} was kept: git would not delete it."
                        )
                        .into(),
                    )),
                    None => say(Say::Warn(
                        "Worktree was on a detached HEAD — no branch to delete.".into(),
                    )),
                }
            }
            0
        }
        Err(refusal) => {
            say(Say::Fail(refusal.to_string().into()));
            if matches!(refusal, Refusal::NoSuchPiece { .. }) {
                say(Say::Info(
                    format!("See what exists: purlis worktree list {repo} -w {ws}").into(),
                ));
            }
            1
        }
    }
}

/// Rows of five cells, the first four sized to their widest value in terminal cells, the last
/// left unpadded — it has nothing to its right. Every cell is text and goes through
/// [`tui::row`]: these values come out of a log and out of directory names, and a newline in one
/// would shear every column below, as an escape in the last would hide every row after it.
fn table(rows: &[[String; 5]], say: &mut dyn FnMut(Say)) {
    let widths: Vec<usize> = (0..4)
        .map(|i| tui::column("", rows.iter().map(|r| r[i].as_str()), GAP, None))
        .collect();
    for row in rows {
        let cells: Vec<(&str, usize)> = row
            .iter()
            .map(String::as_str)
            .zip(widths.iter().copied())
            .collect();
        say(Say::Out(
            format!("    {}", tui::row(&cells, &row[4]))
                .trim_end()
                .to_string(),
        ));
    }
}

/// Whether the clone at `clone` has the ref `full`.
fn git_has(clone: &Path, full: &str) -> bool {
    crate::worktree::git::run(clone, &["show-ref", "--verify", "--quiet", full], {
        crate::worktree::git::READ
    })
    .is_ok_and(|run| run.ok())
}

#[cfg(test)]
mod tests;

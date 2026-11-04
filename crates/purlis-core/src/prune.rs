// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis worktree prune`** (ADR 0110, #5168): every finished folder of a workspace
//! removed, task folders and plain ones, in every repo or one.
//!
//! A folder is taken only where it is finished ([`crate::finished`]): its task, if any, has
//! ended; no open chat that has declared neither `done` nor `abandoned` holds it; nothing in it
//! would be lost; and no folder that stays links into it. A folder that links into another
//! goes before it, so a lane that shares another's install and that other lane both go. Each
//! removed folder's local branch is deleted, with `git branch -d`, or `-D` where it is carried,
//! unless the branches are to be kept. A dry run says what would go and why each other folder
//! stays, and removes nothing.

use std::path::Path;

use chrono::{DateTime, Utc};

use crate::counts::counted;
use crate::dispatchplace::{self, InFolder};
use crate::finished::{self, Commits, Folder};
use crate::piececlaim::{self, Live};
use crate::pieces::Who;
use crate::repocmd::Say;
use crate::worktree::{self, Located, standing};

/// What a prune was asked.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    pub ws: &'a str,
    /// Only this repo's folders; every repo's where `None`. Links are read across them all.
    pub repo: Option<&'a str>,
    pub dry_run: bool,
    pub keep_branches: bool,
}

/// Who asks, as an open chat's hold on a folder is judged.
#[derive(Debug, Clone, Copy)]
pub struct Asker<'a> {
    pub who: &'a Who,
    /// The chats the app has open; `None` where no app said, and every held folder is kept.
    pub live: Option<&'a [Live]>,
    /// For a chat's prune the app runs: each folder's git is run in the checked clone and
    /// folder, as a chat's removal is ([`crate::gitbroker::in_a_checked_folder`]).
    pub brokered: bool,
    /// The branch folder the asking chat stands in, which is never pruned for it: removing it
    /// would take the chat's folder from under it.
    pub stands_in: Option<&'a Folder>,
}

/// A folder that may go, and how its commits stand.
struct Candidate {
    folder: Folder,
    commits: Commits,
}

/// **Prunes `asked.ws`**, saying what went and what stayed. `0` where every finished folder
/// went (or would, in a dry run), `1` where a removal failed or the workspace could not be read.
pub fn prune(
    asked: &Asked<'_>,
    asker: &Asker<'_>,
    plane: &Path,
    now: DateTime<Utc>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    let ws = asked.ws;
    say(Say::Info(format!("workspace: {ws}").into()));
    let listed = match finished::folders(plane, ws) {
        Ok(listed) => listed,
        Err(why) => {
            say(Say::Fail(why.into()));
            return 1;
        }
    };
    if let Some(repo) = asked.repo
        && !listed.repos.iter().any(|name| name == repo)
    {
        say(Say::Fail(
            format!("'{repo}' is not a repo of workspace '{ws}'. Nothing was removed.").into(),
        ));
        return 1;
    }
    let mut code = 0;
    for (repo, why) in &listed.unread {
        say(Say::Fail(format!("{repo}: {why}").into()));
        code = 1;
    }
    let mut kept: Vec<(Folder, String)> = Vec::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    for (folder, gone) in &listed.folders {
        if asked.repo.is_some_and(|repo| repo != folder.repo) {
            continue;
        }
        let judged = if *gone {
            Err(format!(
                "its folder is already gone, and only git's record of it is left: `purlis \
                 worktree remove {} {}` clears it",
                folder.repo, folder.piece
            ))
        } else if asker.stands_in == Some(folder) {
            Err("this chat stands in it".to_owned())
        } else {
            why_kept(plane, ws, folder, asker)
        };
        match judged {
            Ok(commits) => candidates.push(Candidate {
                folder: folder.clone(),
                commits,
            }),
            Err(why) => kept.push((folder.clone(), why)),
        }
    }
    // Links are read across the whole workspace: a folder of another repo may link in too.
    let links = finished::links(plane, ws);
    let mut remaining: Vec<Folder> = listed
        .folders
        .iter()
        .filter(|(_, gone)| !gone)
        .map(|(folder, _)| folder.clone())
        .collect();
    let mut removed = 0;
    loop {
        let ready: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                !links
                    .iter()
                    .any(|link| link.into == c.folder && remaining.contains(&link.from))
            })
            .map(|(at, _)| at)
            .collect();
        if ready.is_empty() {
            break;
        }
        for at in ready.into_iter().rev() {
            let one = candidates.remove(at);
            let went = asked.dry_run || remove_one(plane, ws, &one, asked, asker, now, say);
            if asked.dry_run {
                say(Say::Info(would_go(&one, asked.keep_branches).into()));
            }
            if went {
                // Only a folder that went lets go of what it linked into.
                remaining.retain(|f| f != &one.folder);
                removed += 1;
            } else {
                code = 1;
            }
        }
    }
    for one in candidates {
        let by = finished::linking_into(&links, &one.folder);
        kept.push((one.folder, finished::NotFinished::LinkedInto(by).why()));
    }
    kept.sort_by(|a, b| a.0.cmp(&b.0));
    for (folder, why) in &kept {
        say(Say::Info(format!("Kept {}: {why}", folder.named()).into()));
    }
    say(Say::Info(
        if asked.dry_run {
            format!(
                "Would remove {} and keep {}. Nothing was removed.",
                counted(removed, "finished folder", "finished folders"),
                kept.len()
            )
        } else {
            format!(
                "Removed {}, kept {}.",
                counted(removed, "finished folder", "finished folders"),
                kept.len()
            )
        }
        .into(),
    ));
    code
}

/// How `folder`'s commits stand where it may go but for links, else why it stays.
fn why_kept(plane: &Path, ws: &str, folder: &Folder, asker: &Asker<'_>) -> Result<Commits, String> {
    match dispatchplace::held_in_folder(plane, ws, &folder.repo, &folder.piece) {
        Some(InFolder::Task(record)) if record.running() => {
            return Err(format!(
                "its task '{}' has not ended",
                crate::shown::short(&record.task.unwrap_or(record.worker.chat.name))
            ));
        }
        Some(InFolder::Unread(id)) => {
            return Err(format!(
                "a dispatch record purlis cannot read ({id}.json) names it: if no task works \
                 there, move that file out of its folder"
            ));
        }
        _ => {}
    }
    let at = Located {
        workspace: ws.to_owned(),
        repo: folder.repo.clone(),
        piece: folder.piece.clone(),
    };
    if let Some(why) = piececlaim::held_open(plane, &at, asker.who.chat.as_deref(), asker.live) {
        return Err(why);
    }
    finished::own(plane, ws, &folder.repo, &folder.piece).map_err(|why| why.why())
}

/// What a dry run says of `one`.
fn would_go(one: &Candidate, keep_branches: bool) -> String {
    let branch = match (one.commits, keep_branches) {
        (_, true) | (Commits::NoBranch, _) => "",
        (Commits::InHistory, false) => ", keeping its branch (its base has changed it since)",
        (Commits::OnBase | Commits::Carried, false) => ", and delete its branch",
    };
    format!("Would remove {}{branch}", one.folder.named())
}

/// Removes `one`, held to its claim, and deletes its branch where asked; whether it went.
fn remove_one(
    plane: &Path,
    ws: &str,
    one: &Candidate,
    asked: &Asked<'_>,
    asker: &Asker<'_>,
    now: DateTime<Utc>,
    say: &mut dyn FnMut(Say),
) -> bool {
    let (repo, piece) = (one.folder.repo.as_str(), one.folder.piece.as_str());
    let at = Located {
        workspace: ws.to_owned(),
        repo: repo.to_owned(),
        piece: piece.to_owned(),
    };
    let judged = if asker.brokered || asker.who.chat.is_some() {
        piececlaim::Judged::ByAChat(asker.live)
    } else {
        piececlaim::Judged::ByThePerson
    };
    let delete = !asked.keep_branches;
    let named = one.folder.named();
    let run = || -> (u8, Vec<Say>) {
        let removed = match one.commits {
            Commits::OnBase | Commits::NoBranch => {
                worktree::remove(plane, ws, repo, piece, false, delete)
            }
            Commits::Carried | Commits::InHistory => {
                worktree::remove_keeping_branch(plane, ws, repo, piece)
            }
        };
        let removed = match removed {
            Err(why) => return (1, vec![Say::Fail(format!("{named}: {why}").into())]),
            Ok(removed) => removed,
        };
        let mut lines = vec![Say::Done(format!("Removed {named}").into())];
        if let Some(branch) = removed.branch.as_deref() {
            let gone = removed.branch_deleted
                || (delete
                    && one.commits == Commits::Carried
                    && standing::drop_carried(plane, ws, repo, branch));
            if gone {
                lines.push(Say::Done(format!("Deleted branch {branch}").into()));
            } else if delete {
                lines.push(Say::Warn(
                    format!(
                        "Kept branch {branch}: git would not delete it. See what only it holds \
                         with `git log {branch} --not --remotes`."
                    )
                    .into(),
                ));
            }
        }
        (0, lines)
    };
    let mut lines: Vec<Say> = Vec::new();
    let ran = piececlaim::removing(plane, &at, asker.who, judged, now, || {
        let (code, said) = if asker.brokered {
            let held = worktree::git::isolation().unwrap_or_default();
            crate::gitbroker::in_a_checked_folder(plane, ws, repo, piece, &held, run)
                .unwrap_or_else(|why| {
                    let why = match why {
                        crate::gitbroker::NotRun::Repo(why) => why,
                        crate::gitbroker::NotRun::Folder(why) => why.to_string(),
                    };
                    (1, vec![Say::Fail(format!("{named}: {why}").into())])
                })
        } else {
            run()
        };
        lines.extend(said);
        code
    });
    let ok = match ran {
        Ok(code) => code == 0,
        Err(why) => {
            lines.push(Say::Fail(format!("{named}: {why}").into()));
            false
        }
    };
    for line in lines {
        say(line);
    }
    ok
}

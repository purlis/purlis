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
//!
//! Folders are read several at once, and the work says it is alive after each folder read, each
//! folder's links read and each removal (#5439): a chat's wait on the app bounds a silence, never the whole prune.

use std::path::Path;

use chrono::{DateTime, Utc};

use crate::counts::counted;
use crate::dispatchplace::{self, InFolder};
use crate::finished::{self, Commits, Folder};
use crate::hookwire::alive;
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
    /// The forge a branch neither on its base nor carried is asked of: its tip may be inside a
    /// merged request's head (#5439).
    pub forge: finished::Forge<'a>,
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
    /// Its folder is already gone, and only git's record of it is left.
    gone: bool,
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
    let in_scope: Vec<&(Folder, bool)> = listed
        .folders
        .iter()
        .filter(|(folder, _)| asked.repo.is_none_or(|repo| repo == folder.repo))
        .collect();
    let judged = finished::each_at_once(&in_scope, |(folder, gone)| {
        if *gone {
            gone_folder(plane, ws, folder, asked.forge)
        } else if asker.stands_in == Some(folder) {
            Err("this chat stands in it".to_owned())
        } else {
            why_kept(plane, ws, folder, asker, asked.forge)
        }
    });
    for ((folder, gone), judged) in in_scope.into_iter().zip(judged) {
        match judged {
            Ok(commits) => candidates.push(Candidate {
                folder: folder.clone(),
                commits,
                gone: *gone,
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
            alive::beat();
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
    if !asked.keep_branches {
        let repos: Vec<&String> = listed
            .repos
            .iter()
            .filter(|repo| asked.repo.is_none_or(|asked| asked == repo.as_str()))
            .collect();
        if branches(plane, ws, &repos, asked, asker, say) != 0 {
            code = 1;
        }
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

/// **The branches purlis cut whose folder is gone, in `repos`** (#5439): each one deleting
/// loses nothing from goes (on its base, carried, or inside a merged request's head), with
/// `git branch -D`, which takes its `branch.<name>` section of the clone's config with it;
/// every other stays, saying why. A branch a task that has not ended names stays too. In a
/// chat's prune the app deletes each in its checked clone, as it removes a folder. `0` where
/// every branch that may go went, or would in a dry run.
fn branches(
    plane: &Path,
    ws: &str,
    repos: &[&String],
    asked: &Asked<'_>,
    asker: &Asker<'_>,
    say: &mut dyn FnMut(Say),
) -> u8 {
    let mut code = 0;
    let (mut went, mut stayed) = (0, 0);
    for repo in repos {
        let found = match finished::branches_without_a_folder(plane, ws, repo) {
            Ok(found) => found,
            Err(why) => {
                say(Say::Fail(format!("{repo}: {why}").into()));
                code = 1;
                continue;
            }
        };
        let judged = finished::each_at_once(&found, |branch| {
            if let Some(InFolder::Task(record)) =
                dispatchplace::held_in_folder(plane, ws, repo, branch)
                && record.running()
            {
                return Some(Err(format!(
                    "its task '{}' has not ended",
                    crate::shown::short(&record.task.unwrap_or(record.worker.chat.name))
                )));
            }
            finished::branch_only(plane, ws, repo, branch, asked.forge)
                .map(|judged| judged.map_err(|why| why.why()))
        });
        for (branch, judged) in found.iter().zip(judged) {
            let commits = match judged {
                None => continue,
                Some(Err(why)) => {
                    stayed += 1;
                    say(Say::Info(
                        format!("Kept branch {branch} of {repo}: {why}").into(),
                    ));
                    continue;
                }
                Some(Ok(commits)) => commits,
            };
            let why = commits.why_its_branch_goes();
            if asked.dry_run {
                went += 1;
                say(Say::Info(
                    format!("Would delete branch {branch} of {repo}, whose folder is gone ({why})")
                        .into(),
                ));
                continue;
            }
            let drop = || standing::drop_carried(plane, ws, repo, branch);
            let dropped = if asker.brokered {
                let held = worktree::git::isolation().unwrap_or_default();
                crate::gitbroker::in_a_checked_clone(plane, ws, repo, &held, drop)
            } else {
                Ok(drop())
            };
            alive::beat();
            match dropped {
                Ok(true) => {
                    went += 1;
                    say(Say::Done(
                        format!("Deleted branch {branch} of {repo}, whose folder was gone ({why})")
                            .into(),
                    ));
                }
                Ok(false) => {
                    code = 1;
                    say(Say::Fail(
                        format!("Kept branch {branch} of {repo}: git would not delete it").into(),
                    ));
                }
                Err(not_run) => {
                    code = 1;
                    say(Say::Fail(
                        format!("Kept branch {branch} of {repo}: {not_run}").into(),
                    ));
                }
            }
        }
    }
    if went + stayed > 0 {
        let deleted = counted(
            went,
            "branch whose folder is gone",
            "branches whose folders are gone",
        );
        say(Say::Info(
            if asked.dry_run {
                format!("Would delete {deleted}, and keep {stayed}.")
            } else {
                format!("Deleted {deleted}, kept {stayed}.")
            }
            .into(),
        ));
    }
    code
}

/// How the branch of `folder`, whose folder is already gone, stands where git's record of it
/// may be cleared and the branch deleted with it (#5439), else why it stays: its task has not
/// ended, or its branch holds what would be lost.
fn gone_folder(
    plane: &Path,
    ws: &str,
    folder: &Folder,
    forge: finished::Forge<'_>,
) -> Result<Commits, String> {
    let clears = format!(
        "its folder is already gone, and only git's record of it is left: `purlis worktree \
         remove {} {}` clears it",
        folder.repo, folder.piece
    );
    if let Some(InFolder::Task(record)) =
        dispatchplace::held_in_folder(plane, ws, &folder.repo, &folder.piece)
        && record.running()
    {
        return Err(clears);
    }
    let Some(branch) = worktree::list(plane, ws, &folder.repo)
        .ok()
        .and_then(|found| found.into_iter().find(|p| p.piece == folder.piece))
        .and_then(|found| found.branch)
    else {
        return Err(clears);
    };
    match finished::branch_only(plane, ws, &folder.repo, &branch, forge) {
        Some(Ok(commits)) => Ok(commits),
        Some(Err(why)) => Err(format!("{clears}, and {}", why.why())),
        None => Err(clears),
    }
}

/// How `folder`'s commits stand where it may go but for links, else why it stays.
fn why_kept(
    plane: &Path,
    ws: &str,
    folder: &Folder,
    asker: &Asker<'_>,
    forge: finished::Forge<'_>,
) -> Result<Commits, String> {
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
    finished::own(plane, ws, &folder.repo, &folder.piece, forge).map_err(|why| why.why())
}

/// What a dry run says of `one`.
fn would_go(one: &Candidate, keep_branches: bool) -> String {
    let branch = match (one.commits, keep_branches) {
        (_, true) | (Commits::NoBranch, _) => "",
        (Commits::InHistory, false) => ", keeping its branch (its base has changed it since)",
        (Commits::OnBase | Commits::Carried, false) => ", and delete its branch",
        (Commits::InMergedRequest(_), false) => {
            return format!(
                "Would remove {}, and delete its branch ({})",
                one.folder.named(),
                one.commits.why_its_branch_goes()
            );
        }
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
            _ if one.gone => worktree::remove_keeping_branch(plane, ws, repo, piece),
            Commits::OnBase | Commits::NoBranch => {
                worktree::remove(plane, ws, repo, piece, false, delete)
            }
            Commits::Carried | Commits::InHistory | Commits::InMergedRequest(_) => {
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
                    && (one.gone
                        || matches!(one.commits, Commits::Carried | Commits::InMergedRequest(_)))
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

#[cfg(test)]
mod tests;

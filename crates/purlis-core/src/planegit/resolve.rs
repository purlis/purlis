// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Resolve a save's conflict from the window** (qw382, #5824 F7): what the window's Resolve,
//! Continue, Skip and Abort press, each the person's own press, under no chat's sandbox.
//!
//! **Nothing here stops the plane root** (#5824 D2). A conflict is settled in a resolve folder
//! off to the side ([`super::resolvefolder`]): [`start`] fetches the target and brings it in as
//! far as it goes by itself, and where what a person wrote conflicts, cuts the folder for it.
//! [`carry_on`] (*Continue*) commits what is settled there and moves the root with one `reset
//! --keep`, [`skip`] drops a side with nothing left to bring, and [`abort`] removes the folder.
//! [`push_after`] is what follows: a push of what is committed, and nothing else.
//!
//! **A stop an older purlis left at the root** (a rebase or a merge stopped there before this
//! build) is never carried on here: git in a terminal does that, or [`abort`] puts it away with
//! one `rebase --abort` or `merge --abort`, a single step.

use std::path::Path;

use super::{Claim, PushResult, changed_paths, git, rebase_in_progress, resolvefolder, tail};
use crate::gitstate;
use crate::repocmd::Sink;

/// How a press of Resolve, Continue, Skip or Abort ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// This machine's commits and the remote's are one line now. What is left is the push
    /// ([`push_after`]).
    Replayed,
    /// These files conflict, and a resolve folder holds them for the person or a chat.
    Stopped(Vec<String>),
    /// What is settled brings nothing the remote lacks: Skip drops this machine's side, as a
    /// replay drops a commit left empty.
    Empty,
    /// The resolve folder, or a stop an older build left at the root, is gone, and nothing else
    /// changed.
    Aborted,
    /// Nothing was changed, and why, as a sentence for the person.
    Refused(String),
}

/// The files of a conflict, by who may settle them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conflicts {
    /// Files git marked the conflict in, still unsettled: a person's or a chat's to edit.
    pub marked: Vec<String>,
    /// Files git could not mark the conflict in, still unsettled: a binary file, a file one
    /// side deleted, a link or a submodule. The person settles each in a terminal, by choosing
    /// a side and staging it.
    pub by_hand: Vec<String>,
    /// The settings file, where both sides changed keys each their own way (#5852): the person
    /// chooses each key's value in the window, and no marker is ever written into it.
    pub keyed: Vec<String>,
}

/// What a press says while a save or a fetch holds the project.
const BUSY: &str = "A save of this project is running. Wait for it to finish, then try again.";

/// What Continue says of a stop an older purlis left at the root.
const AT_THE_ROOT: &str = "A rebase or merge is stopped in the project itself, from before \
     purlis settled conflicts off to the side, so it is not carried on here. Carry it on in a \
     terminal (`git add -- <file>` for each settled file, then `git rebase --continue`, or `git \
     commit` for a merge), or Abort it and press Resolve again.";

/// **Resolve**: fetch the target and bring it in, cutting a resolve folder where what a person
/// wrote conflicts. A resolve already open is found again.
pub fn start(root: &Path, say: Sink) -> Settled {
    resolvefolder::start(root, say)
}

/// **Continue**: commit what is settled in the resolve folder and move the project onto it.
/// `shown` are the files the window showed; `keys` the person's choice for each settings key
/// both sides changed ([`resolvefolder::Chosen`]).
pub fn carry_on(
    root: &Path,
    shown: &[String],
    keys: &[resolvefolder::Chosen],
    say: Sink,
) -> Settled {
    if resolvefolder::held(root).is_none() && stopped_at_the_root(root) {
        return Settled::Refused(AT_THE_ROOT.into());
    }
    resolvefolder::carry_on(root, shown, keys, say)
}

/// **Skip**: drop this machine's side where what is settled brings nothing the remote lacks.
pub fn skip(root: &Path) -> Settled {
    if resolvefolder::held(root).is_none() && stopped_at_the_root(root) {
        return Settled::Refused(AT_THE_ROOT.into());
    }
    resolvefolder::skip(root)
}

/// **Abort**: remove the resolve folder, or put away a stop an older build left at the root.
pub fn abort(root: &Path) -> Settled {
    if resolvefolder::held(root).is_some() || resolvefolder::path(root).exists() {
        return resolvefolder::abort(root);
    }
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if let Err(why) = super::git_files_refused(root) {
        return Settled::Refused(format!("Nothing was started: {why}."));
    }
    if !stopped_at_the_root(root) {
        return Settled::Refused(
            "No resolve is open in this project, so there is nothing to abort.".into(),
        );
    }
    let rebase = rebase_in_progress(root);
    let verb = if rebase { "rebase" } else { "merge" };
    // One step that puts a stop away, allowed at the root (#5824 F2).
    let aborted = if rebase {
        git::run_untimed(root, &["rebase", "--abort"])
    } else {
        git::run_untimed(root, &["merge", "--abort"])
    };
    if stopped_at_the_root(root) {
        return Settled::Refused(format!(
            "git did not abort the {verb}: {}",
            match aborted {
                Ok(run) => one_line(&tail(&run)),
                Err(unavailable) => unavailable.to_string(),
            }
        ));
    }
    super::touch(root);
    Settled::Aborted
}

/// Whether a rebase or a merge is stopped at the root itself.
fn stopped_at_the_root(root: &Path) -> bool {
    rebase_in_progress(root)
        || gitstate::stopped(root).is_some_and(|stopped| {
            stopped.operation.map(gitstate::Operation::kind) == Some("merge")
        })
}

/// **What follows a Resolve that finished**: a push of what is committed, as the project's mode
/// says, and no new commit. Answers the push and the files changed meanwhile, which are left as
/// they are for the person to save or remove.
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

/// The files of the open resolve, by who may settle them; empty where none is open.
pub fn conflicts(root: &Path) -> Conflicts {
    resolvefolder::conflicts(root).unwrap_or_default()
}

/// The two sides of the open resolve, by their commit subjects: the remote's, then this
/// machine's.
pub fn sides(root: &Path) -> Option<(String, String)> {
    resolvefolder::sides(root)
}

/// Of `files`, those a chat's sandbox may not write: settled by the person, never a chat.
pub fn held_from_chats(root: &Path, files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|path| crate::rootupdate::path_is_denied(path, root))
        .cloned()
        .collect()
}

/// Whether `text` has a line that starts with one of git's conflict markers: the opener, the
/// closer, or the common ancestor's (`diff3`, `zdiff3`). The separator alone is not one, since
/// a Markdown heading is underlined the same way.
pub fn markers_in(text: &[u8]) -> bool {
    text.split(|b| *b == b'\n').any(|line| {
        line.starts_with(b"<<<<<<<") || line.starts_with(b">>>>>>>") || line.starts_with(b"|||||||")
    })
}

fn one_line(said: &str) -> String {
    said.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;

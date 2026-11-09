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

pub use super::resolvefolder::{
    Chosen, Conflicts, Resolving, ResolvingKey, Settled, Side, folder, markers_in, resolving,
    said_by_hand,
};
use super::{Claim, PushResult, changed_paths, git, rebase_in_progress, resolvefolder, tail};
use crate::gitstate;
use crate::repocmd::Sink;

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
/// both sides changed ([`Chosen`]).
pub fn carry_on(root: &Path, shown: &[String], keys: &[Chosen], say: Sink) -> Settled {
    if resolvefolder::resolving(root).is_none() && stopped_at_the_root(root) {
        return Settled::Refused(AT_THE_ROOT.into());
    }
    resolvefolder::carry_on(root, shown, keys, say)
}

/// **Skip**: drop this machine's side where what is settled brings nothing the remote lacks.
pub fn skip(root: &Path) -> Settled {
    if resolvefolder::resolving(root).is_none() && stopped_at_the_root(root) {
        return Settled::Refused(AT_THE_ROOT.into());
    }
    resolvefolder::skip(root)
}

/// **Abort**: remove the resolve folder, or put away a stop an older build left at the root.
pub fn abort(root: &Path) -> Settled {
    if resolvefolder::resolving(root).is_some() || resolvefolder::folder(root).exists() {
        return resolvefolder::abort(root);
    }
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(resolvefolder::BUSY.into());
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
    let stop = if rebase { "replay" } else { "stopped merge" };
    // One step that puts a stop away, allowed at the root (#5824 F2).
    let aborted = if rebase {
        git::run_untimed(root, &["rebase", "--abort"])
    } else {
        git::run_untimed(root, &["merge", "--abort"])
    };
    if stopped_at_the_root(root) {
        return Settled::Refused(format!(
            "git did not abort the {stop}: {}",
            match aborted {
                Ok(run) => crate::personas::one_line(&tail(&run)),
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
        || gitstate::stopped(root)
            .is_some_and(|stopped| stopped.operation == Some(gitstate::Operation::Merge))
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

#[cfg(test)]
mod tests;

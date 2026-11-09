// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a repo's own folder changed, rolled up onto its top, kept per clone** (#3489): what the
//! explorer's closed repo rows say, for every repo of a workspace at once.
//!
//! A closed row says how many files its repo changed and of what kind (#3343). Each row used to
//! read its repo's whole status (`branch_status`), and was read again on every refresh, so a
//! workspace of twenty repos read twenty statuses each time. The answer is kept here instead, in
//! the store the save standings share ([`crate::standings::Shared`], FD-11): single-flight per
//! clone, and current while the clone's git files are as they were and nothing touched it. The
//! one watch per repo (`branchwatch.rs` in the app) touches a clone when anything in its working
//! tree moves that git would show ([`crate::planegit::touch`] reaches this store too), and a clone
//! no watch covers yet is asked to be watched, as a shared save standing is. Where nothing watches
//! (the CLI), an answer is kept for at most [`crate::planegit::SHARED_FOR`].
//!
//! The read itself is the status a branch's markers come from ([`super::status`]), passed in by
//! the caller, so what a closed row counts is what the opened repo marks.
//!
//! In memory and in this process only: nothing is written, so there is no store to give a tier
//! (ADR 0069).

use std::path::Path;
use std::sync::LazyLock;

use super::{Branch, Refused, Rolled, Status};
use crate::standings::Shared;

/// What a repo's own folder changed, rolled up onto its top (`None` for nothing changed), or the
/// sentence its read was refused with.
pub type RepoChanges = Result<Option<Rolled>, String>;

static KEPT: LazyLock<Shared<RepoChanges>> =
    LazyLock::new(|| Shared::new(crate::planegit::SHARED_FOR));

/// What the own folder of `repo`, in workspace `ws`, changed against its base, rolled up onto its
/// top: the last answer while it is current, else `read`'s for [`Branch::repo`].
///
/// `read` is the status read ([`super::status`] with the app's bounded reader). It is not called
/// for a repo the workspace does not hold, which is refused in the sentence the window shows.
pub fn repo_changes(
    plane: &Path,
    ws: &str,
    repo: &str,
    read: impl FnOnce(Branch<'_>) -> Result<Status, Refused>,
) -> RepoChanges {
    let clone = crate::worktree::clone_folder(plane, ws, repo)
        .map_err(|refusal| Refused::from(refusal).to_string())?;
    // Kept as long as a watched clone's answer is only while the one watch per repo covers it,
    // and asked for where it does not, as the clone's shared save standing is.
    let covered = crate::reposave::covered(&clone);
    KEPT.cover(&clone, covered);
    if !covered {
        crate::standings::want_watched(&crate::standings::Wanted {
            plane: plane.to_path_buf(),
            workspace: ws.to_string(),
            repo: repo.to_string(),
            path: clone.clone(),
        });
    }
    KEPT.get(&clone, || {
        read(Branch::repo(ws, repo))
            .map(|status| status.folders.into_iter().find(|one| one.folder.is_empty()))
            .map_err(|refused| refused.to_string())
    })
}

/// Something changed at or under `dir`: [`crate::planegit::touch`]'s share of this store.
pub(crate) fn touch_within(dir: &Path) {
    KEPT.touch_within(dir);
}

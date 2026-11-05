// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Delete now** (#5332): deleted skills taken off the Deleted list before their 30 days are up,
//! the one change there with no undo. A skill this machine kept is removed from it, as
//! [`super::super::expire`] removes one past its time; one whose delete was a commit stays in the
//! project's history, and only leaves this machine's list, marked in
//! [`GONE_DIR`] (`docs/plane-format.md`).

use std::path::Path;

use super::super::{DELETED_DIR, Deleted, KEPT_FROM, Source, deleted, store};
use crate::counts::counted;

/// The state folder's folder of the committed deletes Delete now took off the list: an empty
/// file each, named by [`mark_of`].
pub(in crate::skills) const GONE_DIR: &str = "skills-deleted-gone";

/// The name of the mark that takes the delete in commit `sha` of the skill folder `folder_rel`
/// off the list: the commit, a `-`, and a digest of the folder, so the name is one plain word.
fn mark_of(sha: &str, folder_rel: &str) -> String {
    let digest = crate::skills::source::review::sha256(folder_rel.as_bytes());
    let hex = digest.strip_prefix("sha256:").unwrap_or(&digest);
    format!("{sha}-{}", &hex[..hex.len().min(16)])
}

/// The marks Delete now left on this machine.
pub(in crate::skills) fn read(plane: &Path) -> Vec<String> {
    store::files_in(&crate::plane::state_dir(plane), GONE_DIR)
}

/// Whether `gone` takes the delete in commit `sha` of the folder `folder_rel` off the list.
pub(in crate::skills) fn marked(gone: &[String], sha: &str, folder_rel: &str) -> bool {
    !gone.is_empty() && gone.contains(&mark_of(sha, folder_rel))
}

/// What one Delete now did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gone {
    /// Those whose delete was a commit, off the list: the project's history still holds them.
    pub off_the_list: usize,
    /// Those this machine kept, removed from it for good.
    pub removed: usize,
    /// Each that could not be taken off, with why.
    pub failed: Vec<String>,
}

/// **Takes every skill of `set` off the Deleted list now**: refused whole, before anything is
/// done, where one is no longer listed. Each committed one is marked first, as a mark can be taken
/// back; then each this machine kept is removed, which cannot.
pub fn delete_now(plane: &Path, set: &[Deleted]) -> Result<Gone, String> {
    if set.is_empty() {
        return Err("no deleted skill was named".to_owned());
    }
    let listed = deleted(plane);
    if let Some(one) = set.iter().find(|one| !listed.contains(one)) {
        return Err(format!(
            "{} is not in Deleted any more, so nothing was deleted",
            one.name
        ));
    }
    let state = crate::plane::state_dir(plane);
    let mut gone = Gone::default();
    let commits: Vec<(&Deleted, &str)> = set
        .iter()
        .filter_map(|one| match &one.kept.0 {
            Source::Commit(sha) => Some((one, sha.as_str())),
            Source::Here(_) => None,
        })
        .collect();
    if !commits.is_empty() {
        crate::profiletrust::private_dir(&state).map_err(|why| why.to_string())?;
        store::made(&state, &[GONE_DIR], true).map_err(|why| {
            format!("this machine's list of deleted skills could not be changed ({why})")
        })?;
        for (one, sha) in commits {
            let Some(dir) = one.level.rel() else {
                gone.failed
                    .push(format!("{} (its level has no folder)", one.name));
                continue;
            };
            let mark = mark_of(sha, &format!("{dir}/{}", one.folder_name()));
            match store::write_new(&state, &[GONE_DIR], &mark, "") {
                Ok(()) => gone.off_the_list += 1,
                Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {
                    gone.off_the_list += 1;
                }
                Err(why) => gone.failed.push(format!("{} ({why})", one.name)),
            }
        }
    }
    for one in set {
        if let Source::Here(kept) = &one.kept.0 {
            match store::remove_now(&state, DELETED_DIR, KEPT_FROM, kept) {
                Ok(()) => gone.removed += 1,
                Err(why) => gone.failed.push(format!("{} ({why})", one.name)),
            }
        }
    }
    tracing::info!(
        target: "purlis::skills",
        "Delete now: {} off the list, {} removed from this machine",
        counted(gone.off_the_list, "skill", "skills"),
        counted(gone.removed, "skill", "skills"),
    );
    Ok(gone)
}

#[cfg(test)]
mod tests;

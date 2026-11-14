// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Trusting a clone the person made outside purlis**: the stated act that records it.
//!
//! A clone purlis cloned is recorded in the config home ([`super::PUSH_REMOTES`]) as it is made,
//! and only a recorded clone gets the person's sign-in for a save's push or a chat's GitHub App
//! token ([`crate::forge::apptoken::remotes_of_workspace`]). A clone the person made in their own
//! terminal has no record. The window lists each such clone ([`unvouched_clones`]) with the one
//! address its remote names now, and says purlis did not clone it; the person's press on that
//! line ([`vouch_clone`]) records it, while its remote is still the one they were shown.
//!
//! Nothing here is asked from a chat: the press takes the host's [`HostScope`], which a chat's
//! process tree cannot claim, and the record is in the config home, which no chat writes.

use std::path::{Path, PathBuf};

use crate::forge::route::HostScope;

#[cfg(test)]
mod tests;

/// A clone in one of the project's workspaces with a remote on a forge the project knows, and no
/// record: purlis did not clone it, and the person has not trusted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unvouched {
    /// The workspace that holds it.
    pub workspace: String,
    /// Its folder, relative to the project: `workspaces/<ws>/repos/<name>`, or the like.
    pub folder: String,
    /// Its folder, as git is run in it.
    pub path: PathBuf,
    /// The one `<host>/<owner>/<repo>` its `origin` names now, its push URL too; `None` when it
    /// names more than one, which is never trusted (it says to give origin one address).
    pub place: Option<String>,
    /// Every address its `origin` names now, each once, in the order git gives them.
    pub places: Vec<String>,
    /// **When it was made** (R-337): its folder's creation time, in seconds since 1970, where its
    /// disk keeps one; `None` where it keeps none.
    pub made: Option<u64>,
}

impl Unvouched {
    /// **The address its remote names, as the person reads it**: the one address, or, where it
    /// names several, each joined by "and".
    pub fn remote_said(&self) -> String {
        self.place
            .clone()
            .unwrap_or_else(|| self.places.join(" and "))
    }
}

/// Every clone in the workspaces of the project at `root` that has a remote on a forge the
/// project knows and no record in the config home at `config_root`, by workspace and then name.
/// A workspace whose clones cannot be read, and a clone whose remote git cannot read, are left
/// out: there is nothing to show the person about them.
pub fn unvouched_clones(root: &Path, config_root: &Path) -> Vec<Unvouched> {
    let recorded = super::read_push_remotes(config_root);
    let known = crate::forge::known(root);
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let mut out = Vec::new();
    for workspace in plane.workspaces().unwrap_or_default() {
        let Ok(clones) = crate::repos::clones(root, &workspace) else {
            continue;
        };
        for repo in clones.repos {
            if recorded.contains_key(&crate::sandbox::local::clone_key(&repo.path)) {
                continue;
            }
            let Some(named) = super::remote::places_now(&repo.path) else {
                continue;
            };
            let mut places: Vec<String> = Vec::new();
            for place in named {
                if !places.contains(&place) {
                    places.push(place);
                }
            }
            let on_a_forge = places.iter().any(|place| {
                crate::forge::resolve_in(&format!("https://{place}"), &known).is_some()
            });
            if !on_a_forge {
                continue;
            }
            let folder = repo
                .path
                .strip_prefix(root)
                .unwrap_or(&repo.path)
                .to_string_lossy()
                .into_owned();
            let made = created_at(&repo.path);
            out.push(Unvouched {
                workspace: workspace.clone(),
                folder,
                path: repo.path,
                place: (places.len() == 1).then(|| places[0].clone()),
                places,
                made,
            });
        }
    }
    out
}

/// When the folder at `path` was made, in seconds since 1970, where its disk keeps a creation
/// time.
fn created_at(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .and_then(|meta| meta.created())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs())
}

/// The person's press on a clone made outside purlis: records `seen`, the address the window
/// showed them, as the remote of the clone at `clone`, so its saves push with their sign-in and
/// chats in its workspace may use their GitHub access with it. Answers the address trusted.
///
/// Refused, with a sentence and nothing recorded, unless `clone` is one of the project's
/// workspace clones with no record ([`unvouched_clones`]) whose `origin` names exactly `seen`
/// now: a remote that changed after the window showed it is never trusted on a press that named
/// another. A clone already trusted at `seen` answers `seen` again.
pub fn vouch_clone(
    _host: &HostScope,
    config_root: &Path,
    root: &Path,
    clone: &Path,
    seen: &str,
) -> Result<String, String> {
    let key = crate::sandbox::local::clone_key(clone);
    if super::read_push_remotes(config_root)
        .get(&key)
        .map(String::as_str)
        == Some(seen)
    {
        return Ok(seen.to_owned());
    }
    let shown = clone
        .strip_prefix(root)
        .unwrap_or(clone)
        .display()
        .to_string();
    let Some(listed) = unvouched_clones(root, config_root)
        .into_iter()
        .find(|listed| crate::sandbox::local::clone_key(&listed.path) == key)
    else {
        return Err(format!(
            "{shown} is not a clone in this project's workspaces that purlis can trust, so \
             nothing changed. Look at the project's notices again"
        ));
    };
    match listed.place {
        Some(now) if now == seen => super::remote::record(config_root, &listed.path, now),
        Some(_) => Err(format!(
            "The remote of {shown} changed since the window showed it, so purlis trusted \
             nothing. Look at it again and choose"
        )),
        None => Err(format!(
            "The remote of {shown} names more than one address, so purlis trusted nothing. Give \
             origin one address, then trust it"
        )),
    }
}

/// Whether `clone` has a record in the config home at `config_root`: purlis cloned it, or the
/// person trusted it ([`vouch_clone`]). A Save from the window records a new remote only for
/// such a clone.
pub fn trusted(config_root: &Path, clone: &Path) -> bool {
    super::read_push_remotes(config_root).contains_key(&crate::sandbox::local::clone_key(clone))
}

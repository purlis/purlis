// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner never reads for its window** (#7064): purlis's own folders there, every
//! harness's sign-in files, and what no chat there reads. Never read, whatever the reach and
//! whatever the person answered: the runner answers
//! [`purlis_session_protocol::session::FileRead::Kept`] before it reads a byte.
//!
//! - **Named from what purlis already knows of the runner**, never a list of its own: the data
//!   home's folders ([`Homes::data_homes`]) and the config home's
//!   ([`crate::machine::dirs_spelled`]); each harness's sign-in files as its adapter names them
//!   ([`crate::sandbox::signin::credential_files`], for every harness purlis knows); and what a
//!   chat is kept from reading ([`crate::sandbox::kept_from_reading`]): the vaults, purlis's
//!   records, the person's credentials.
//! - **The chat's own project is read**, wherever it is kept: a runner keeps its projects under
//!   its data home, so purlis's own folders are judged only outside the project. A sign-in file
//!   and what a chat is kept from are judged everywhere.
//! - **Judged by spelling and by what it is.** By spelling first, composed and in one case, so a
//!   name a case-folding disk opens as one of them is one of them; then by identity, the
//!   resolved path and every folder above it, and the file opened, by device and number, so a
//!   link or a second spelling that reaches one of them is one of them too.
//! - **A file with several names** is kept where one of its other names is in a kept folder
//!   (#7077), as the host's own guard judges one; otherwise it is placed by the name the chat
//!   printed, as the host places one.
//! - **Where the list cannot be told whole**, the vault registry does not read, nothing is read.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::sandbox::Machine;
use crate::sandbox::signin::{ProfileFolders, credential_files};

/// A file or folder as the file system knows it, whatever it is called.
type Id = (u64, u64);

/// **What the runner keeps from its window**, for one chat's project.
pub(super) struct Kept {
    /// Kept wherever they are: each harness's sign-in files and what no chat reads.
    everywhere: Vec<PathBuf>,
    /// purlis's own folders, kept but where the chat's project is.
    own: Vec<PathBuf>,
    /// The chat's project, resolved.
    project: PathBuf,
    /// The identity of each of `everywhere` that is there.
    everywhere_ids: HashSet<Id>,
    /// The identity of each of `own` that is there.
    own_ids: HashSet<Id>,
    /// Whether the list could not be told whole: then everything is kept.
    untold: bool,
}

impl Kept {
    /// What the runner `machine` keeps from its window, for a chat of the project at `project`.
    pub(super) fn of(project: &Path, machine: &Machine) -> Self {
        let project = std::fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
        let mut everywhere: Vec<PathBuf> = Vec::new();
        for harness in crate::harness::Harness::ALL {
            everywhere.extend(credential_files(
                harness,
                &ProfileFolders::of(harness, &[], machine),
            ));
        }
        let untold = match crate::sandbox::kept_from_reading(&project, machine) {
            Ok(kept) => {
                everywhere.extend(kept);
                false
            }
            Err(_) => true,
        };
        let mut own: Vec<PathBuf> = crate::sandbox::Homes::data_homes(machine);
        let config_root = crate::machine::rooted(
            machine.env.get(crate::machine::HOME_VAR).map(Into::into),
            machine.env.get("XDG_CONFIG_HOME").map(Into::into),
            machine.home.clone(),
        );
        if let Some(config_root) = config_root {
            own.extend(crate::machine::dirs_spelled(&config_root));
        }
        let everywhere = with_resolved(everywhere);
        let own = with_resolved(own);
        Self {
            everywhere_ids: ids_of(&everywhere),
            own_ids: ids_of(&own),
            everywhere,
            own,
            project,
            untold,
        }
    }

    /// **Whether `resolved`, a path with every link resolved, is kept**: by its spelling, and by
    /// what it and every folder above it is.
    pub(super) fn holds(&self, resolved: &Path) -> bool {
        if self.untold {
            return true;
        }
        let in_project = resolved.starts_with(&self.project);
        if self.everywhere.iter().any(|kept| under(resolved, kept)) {
            return true;
        }
        if !in_project && self.own.iter().any(|kept| under(resolved, kept)) {
            return true;
        }
        resolved.ancestors().filter_map(id_of_path).any(|id| {
            self.everywhere_ids.contains(&id) || (!in_project && self.own_ids.contains(&id))
        })
    }

    /// **Whether the open file whose metadata is `meta` is one of the kept files**, under
    /// whatever name it was opened by: it IS one, or it has several names and one of them is in
    /// a kept folder (#7077), as the host judges one (`files::kept`). A file with several names
    /// whose others could not be looked through within [`WALKED`] entries is counted as kept.
    pub(super) fn holds_the_file(&self, meta: &std::fs::Metadata) -> bool {
        if self.untold {
            return true;
        }
        let Some(id) = id_of(meta) else {
            return false;
        };
        if self.everywhere_ids.contains(&id) {
            return true;
        }
        if names_of(meta) < 2 {
            return false;
        }
        let mut left = WALKED;
        // Each folder with whether it is one of purlis's own, whose part that is the chat's
        // project is not looked through: the project is read, so its files' other names there
        // are its own. What no chat reads is looked through wherever it is, in the project too.
        let mut folders: Vec<(PathBuf, bool)> = self
            .everywhere
            .iter()
            .map(|folder| (folder.clone(), false))
            .chain(self.own.iter().map(|folder| (folder.clone(), true)))
            .collect();
        while let Some((folder, own)) = folders.pop() {
            if own && folder.starts_with(&self.project) {
                continue;
            }
            // Never through a link: what a kept folder links to is not what it holds.
            let Ok(listed) = std::fs::read_dir(&folder) else {
                continue;
            };
            for entry in listed.flatten() {
                if left == 0 {
                    return true;
                }
                left -= 1;
                let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                    continue;
                };
                if meta.is_dir() {
                    folders.push((entry.path(), own));
                } else if id_of(&meta) == Some(id) {
                    return true;
                }
            }
        }
        false
    }
}

/// The most entries of the kept folders [`Kept::holds_the_file`] looks at for a file with
/// several names, as the host's own guard does. Past it the file is kept: purlis could not
/// tell, so it does not read it.
const WALKED: usize = 20_000;

/// How many names a file has.
#[cfg(unix)]
fn names_of(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    meta.nlink()
}

#[cfg(not(unix))]
fn names_of(_meta: &std::fs::Metadata) -> u64 {
    1
}

/// `paths`, each as written and as the kernel names it.
fn with_resolved(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for path in paths {
        for path in [crate::sandbox::real(&path), path] {
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// The identity of each of `paths` that is there, and of what it leads to where it is a link.
fn ids_of(paths: &[PathBuf]) -> HashSet<Id> {
    let mut ids = HashSet::new();
    for path in paths {
        for meta in [std::fs::symlink_metadata(path), std::fs::metadata(path)] {
            if let Some(id) = meta.ok().as_ref().and_then(id_of) {
                ids.insert(id);
            }
        }
    }
    ids
}

fn id_of_path(path: &Path) -> Option<Id> {
    std::fs::metadata(path).ok().as_ref().and_then(id_of)
}

#[cfg(unix)]
pub(super) fn id_of(meta: &std::fs::Metadata) -> Option<Id> {
    use std::os::unix::fs::MetadataExt as _;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
pub(super) fn id_of(_meta: &std::fs::Metadata) -> Option<Id> {
    None
}

/// Whether `path` is `kept` or under it, compared composed and in one case
/// ([`crate::browserfiles::folded`]): folding where the disk does not only keeps more.
fn under(path: &Path, kept: &Path) -> bool {
    let (Some(path), Some(kept)) = (path.to_str(), kept.to_str()) else {
        return path.starts_with(kept);
    };
    let path = crate::browserfiles::folded(path);
    let kept = crate::browserfiles::folded(kept);
    let kept = kept.trim_end_matches('/');
    path == kept || path.starts_with(&format!("{kept}/"))
}

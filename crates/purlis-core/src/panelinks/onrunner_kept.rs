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

use crate::files::kept::{Id, a_name_in, id_of, names_of};
use crate::sandbox::Machine;
use crate::sandbox::signin::{ProfileFolders, credential_files};

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
    /// a kept folder (#7077), looked for as the host's own guard looks ([`a_name_in`]).
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
        // The chat's own project is read, so its files' other names there are its own: purlis's
        // own folders are not looked through where they hold it, however they are spelled.
        // What no chat reads is looked through wherever it is, in the project too.
        let in_project = |folder: &Path| {
            folder.starts_with(&self.project)
                || std::fs::canonicalize(folder).is_ok_and(|real| real.starts_with(&self.project))
        };
        a_name_in(self.everywhere.clone(), id, &|_| false)
            || a_name_in(self.own.clone(), id, &in_project)
    }
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

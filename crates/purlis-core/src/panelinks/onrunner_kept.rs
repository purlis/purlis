// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner never reads for its window** (#7064): purlis's own folders there, every
//! harness's sign-in files, and what no chat there reads. Never read, whatever the reach and
//! whatever the person answered: the runner answers
//! [`purlis_session_protocol::session::FileRead::Kept`] before it reads a byte.
//!
//! - **Named from what purlis already knows of the runner**, never a list of its own: the data
//!   home's folders ([`Homes::data_homes`]) and every folder the host keeps for itself
//!   ([`crate::codelink::held::purlis_keeps_for_itself`]: the config home under both
//!   its names, the app's own folder and logs, the host's socket folder, its lock); each harness's sign-in files as its adapter names them
//!   ([`crate::sandbox::signin::credential_files`], for every harness purlis knows); and what a
//!   chat is kept from reading ([`crate::sandbox::kept_from_reading`]): the vaults, purlis's
//!   records, the person's credentials.
//! - **The chat's own project is read** where it is kept inside one of purlis's own folders (a
//!   runner keeps its projects under its data home): that folder is judged only outside the
//!   project. A project that is such a folder, or holds one (a home, or `/`), exempts nothing:
//!   what is purlis's in it stays kept. A sign-in file and what a chat is kept from are judged
//!   everywhere.
//! - **Judged by spelling and by what it is.** By spelling first, composed and in one case, so a
//!   name a case-folding disk opens as one of them is one of them; then by identity, the
//!   resolved path and every folder above it, and the file opened, by device and number, so a
//!   link or a second spelling that reaches one of them is one of them too.
//! - **A file with several names** is kept where one of its other names is in a kept folder
//!   (#7077), as the host's own guard judges one; otherwise it is placed by the name the chat
//!   printed, as the host places one.
//! - **Where it cannot be told**, the vault registry does not read, or a file with several
//!   names has more places to look than the walk's budget, nothing is read, and the answer says
//!   it could not tell ([`Held::Unsure`]) and not that the file is kept.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::files::kept::{Id, Names, id_of, names_in, names_of};
use crate::sandbox::Machine;
use crate::sandbox::signin::{ProfileFolders, credential_files};

/// **What the runner makes of a path**, for its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Held {
    /// Not kept: it may be read.
    No,
    /// Kept from the window.
    Kept,
    /// Not told whether it is kept: it is not read either.
    Unsure,
}

/// One of purlis's own folders on the runner.
struct Own {
    path: PathBuf,
    /// Its identity, where it is there.
    ids: Vec<Id>,
    /// Whether the chat's project is inside it (never it itself): only then is the project
    /// read in it.
    holds_the_project: bool,
}

/// **What the runner keeps from its window**, for one chat's project.
pub(super) struct Kept {
    /// Kept wherever they are: each harness's sign-in files and what no chat reads.
    everywhere: Vec<PathBuf>,
    /// purlis's own folders, kept but where the chat's project is inside them.
    own: Vec<Own>,
    /// The chat's project, resolved.
    project: PathBuf,
    /// The identity of each of `everywhere` that is there.
    everywhere_ids: HashSet<Id>,
    /// Whether the list could not be told whole: then nothing is read.
    untold: bool,
    /// The most entries looked through for the other names of a file.
    most: usize,
}

impl Kept {
    /// What the runner `machine` keeps from its window, for a chat of the project at `project`.
    pub(super) fn of(project: &Path, machine: &Machine, most: usize) -> Self {
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
        if let Some(home) = &machine.home {
            own.extend(crate::codelink::held::purlis_keeps_for_itself(
                &|name| machine.env.get(name),
                home,
            ));
        }
        let everywhere = with_resolved(everywhere);
        let project_ids: Vec<Id> = project.ancestors().skip(1).filter_map(id_of_path).collect();
        let own = with_resolved(own)
            .into_iter()
            .map(|path| {
                let ids = ids_of(std::slice::from_ref(&path));
                let holds_the_project = path != project
                    && (under(&project, &path) || ids.iter().any(|id| project_ids.contains(id)));
                Own {
                    path,
                    ids: ids.into_iter().collect(),
                    holds_the_project,
                }
            })
            .collect();
        Self {
            everywhere_ids: ids_of(&everywhere),
            everywhere,
            own,
            project,
            untold,
            most,
        }
    }

    /// **Whether `resolved`, a path with every link resolved, is kept**: by its spelling, and by
    /// what it and every folder above it is.
    pub(super) fn holds(&self, resolved: &Path) -> Held {
        if self.untold {
            return Held::Unsure;
        }
        let in_project = resolved.starts_with(&self.project);
        let ids: Vec<Id> = resolved.ancestors().filter_map(id_of_path).collect();
        let by_spelling = self.everywhere.iter().any(|kept| under(resolved, kept));
        let by_identity = ids.iter().any(|id| self.everywhere_ids.contains(id));
        let in_own = self.own.iter().any(|own| {
            // The project is read inside the folder that holds it, and nowhere else.
            !(in_project && own.holds_the_project)
                && (under(resolved, &own.path) || ids.iter().any(|id| own.ids.contains(id)))
        });
        if by_spelling || by_identity || in_own {
            Held::Kept
        } else {
            Held::No
        }
    }

    /// **Whether the open file whose metadata is `meta` is one of the kept files**, under
    /// whatever name it was opened by: it IS one, or it has several names and one of them is in
    /// a kept folder (#7077), looked for as the host's own guard looks ([`names_in`]).
    pub(super) fn holds_the_file(&self, meta: &std::fs::Metadata) -> Held {
        if self.untold {
            return Held::Unsure;
        }
        let Some(id) = id_of(meta) else {
            return Held::No;
        };
        if self.everywhere_ids.contains(&id) {
            return Held::Kept;
        }
        if names_of(meta) < 2 {
            return Held::No;
        }
        // The chat's own project is read, so its files' other names in the folder that holds it
        // are its own: that folder is not looked through where the project is. What no chat
        // reads is looked through wherever it is, in the project too.
        let in_project = |folder: &Path| {
            folder.starts_with(&self.project)
                || std::fs::canonicalize(folder).is_ok_and(|real| real.starts_with(&self.project))
        };
        let (inside, elsewhere): (Vec<&Own>, Vec<&Own>) =
            self.own.iter().partition(|own| own.holds_the_project);
        let paths = |own: Vec<&Own>| -> Vec<PathBuf> {
            own.into_iter().map(|own| own.path.clone()).collect()
        };
        let everywhere: Vec<PathBuf> = self
            .everywhere
            .iter()
            .cloned()
            .chain(paths(elsewhere))
            .collect();
        let looks = [
            names_in(everywhere, id, &|_| false, self.most),
            names_in(paths(inside), id, &in_project, self.most),
        ];
        if looks.contains(&Names::Found) {
            Held::Kept
        } else if looks.contains(&Names::Untold) {
            Held::Unsure
        } else {
            Held::No
        }
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

pub(super) fn id_of_path(path: &Path) -> Option<Id> {
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

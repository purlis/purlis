// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner chat's paths, mapped through its container's mounts** (#6221, spec #6208
//! decision 8). A chat on a runner prints its own machine's paths (`/work/src/main.rs`). Such a
//! path names a file on this machine only through a folder of this machine that the container
//! mounts. Then it is a file link under the file kind's rules ([`super::LocalFile`]).
//!
//! - **One function per runner kind.** Docker's reads the mounts purlis made the container
//!   with ([`Mounts::docker`]), never what the chat prints. Other runners map nothing yet
//!   ([`Mounts::none`], #6224).
//! - **The mount with the longest prefix wins**, compared a whole part at a time. `/work/src`
//!   is inside `/work`, and `/work/srcx` is not inside `/work/src`.
//! - **A path that maps to nothing is not a link.** The same goes for a relative one, one with
//!   `..`, and one two mounts claim alike.
//! - **A mapped path never leaves its mount's folder.** The file kind walks it on this machine,
//!   following every link, and keeps it only where the walk ends inside the mount's folder as
//!   recorded. So a mount whose recorded folder is not its own real path (a link, or a folder a
//!   link leads through) maps nothing: Docker resolved it when the container was made, and it
//!   may lead elsewhere now.

use std::path::{Component, Path, PathBuf};

use crate::dockerpool::{Mounted, MountedFrom, Runner};

/// **A folder of this machine that a runner's container mounts**, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// Where it is mounted in the container.
    inside: PathBuf,
    /// The folder (or file) of this machine it mounts.
    source: PathBuf,
}

impl Mount {
    /// `source`, of this machine, mounted at `inside` in the container. `None` where either
    /// is not an absolute path of plain parts (no `..`).
    pub fn bind(inside: impl Into<PathBuf>, source: impl Into<PathBuf>) -> Option<Mount> {
        let (inside, source) = (inside.into(), source.into());
        (plain(&inside) && plain(&source)).then_some(Mount { inside, source })
    }
}

/// **The mounts a runner chat's paths are mapped through.** None maps nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mounts(Vec<Mount>);

impl Mounts {
    /// No mount: every path is unlinked.
    pub fn none() -> Mounts {
        Mounts(Vec::new())
    }

    /// These mounts.
    pub fn of(mounts: Vec<Mount>) -> Mounts {
        Mounts(mounts)
    }

    /// **A Docker pool runner's mounts** ([`Runner::mounts`]): the folders of this machine its
    /// container was made with. A volume is Docker's own and no folder of this machine, so it
    /// maps nothing. Today purlis mounts nothing else, so a Docker chat's paths stay unlinked.
    pub fn docker(runner: &Runner) -> Mounts {
        Self::mounted(runner.mounts())
    }

    /// The binds of `mounted`, a container's mounts as Docker is handed them.
    fn mounted(mounted: Vec<Mounted>) -> Mounts {
        Mounts(
            mounted
                .into_iter()
                .filter_map(|mounted| match mounted.from {
                    MountedFrom::Bind(source) => Mount::bind(mounted.target, source),
                    MountedFrom::Volume(_) => None,
                })
                .collect(),
        )
    }

    /// **`path`, in the container, as the path of this machine it names**, with the folder of
    /// the mount it went through. `None` where it is relative, has `..`, maps through no
    /// mount, or two mounts at the same place claim it.
    pub(super) fn mapped(&self, path: &Path) -> Option<(PathBuf, &Path)> {
        if !plain(path) {
            return None;
        }
        let deepest = |mount: &&Mount| mount.inside.components().count();
        let claiming: Vec<&Mount> = self
            .0
            .iter()
            .filter(|mount| path.starts_with(&mount.inside))
            .collect();
        let most = claiming.iter().map(deepest).max()?;
        let mut at_most = claiming.into_iter().filter(|mount| deepest(mount) == most);
        let mount = at_most.next()?;
        if at_most.next().is_some() {
            return None;
        }
        let rest = path.strip_prefix(&mount.inside).ok()?;
        let host = if rest.as_os_str().is_empty() {
            mount.source.clone()
        } else {
            mount.source.join(rest)
        };
        Some((host, &mount.source))
    }
}

/// Whether `path` is absolute and has no `..`.
fn plain(path: &Path) -> bool {
    path.is_absolute() && !path.components().any(|part| part == Component::ParentDir)
}

#[cfg(test)]
#[path = "mounts_tests.rs"]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the new-chat picker's answers were read from that the plane watch does not follow**
//! (#4737), as it stood when they were read.
//!
//! The app keeps the picker's two answers per project, so the dialog opens again at once with no
//! sandbox compile and no program check. The plane watch drops them on a change to the profiles,
//! the personas or the project's sandbox ([`crate::planechange::moves_a_new_chat`]). What else
//! they were read from is on this machine, outside the plane's watched folders:
//!
//! - this machine's sandbox policy ([`crate::sandbox::policy::MACHINE_FILE`]);
//! - its profile store, which says each profile's sign-in and pinned install;
//! - this project's records of approved commands, in its state folder;
//! - your grants and confirmed sandbox level for this clone, and for every project;
//! - the installed harnesses: each profile's program as the check resolves it, and each folder
//!   on `PATH`, where a harness newly installed lands.
//!
//! And the project's own manifest, local settings and `.gitignore`, which the watch does follow:
//! on a busy Mac it tells a change seconds late (#577), and a profile edited and the dialog
//! opened straight after must not draw the profile as it was.
//!
//! A [`Stamp`] is each of those paths' modified time and length, or its absence. It costs one
//! `stat` a path, no read and no process, and a kept answer is served only while its stamp
//! still holds.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::profiles::Profile;

/// How one path stood: its modified time and length, or `None` where nothing was there.
type Seen = Option<(SystemTime, u64)>;

/// The paths a picker answer was read from outside the plane watch, each as it stood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    seen: Vec<(PathBuf, Seen)>,
}

impl Stamp {
    /// The stamp of a picker answer for the project at `root`, listing `profiles`, on this
    /// machine as it stands now.
    pub fn of(root: &Path, profiles: &[Profile]) -> Self {
        Self::of_paths(read_from(
            root,
            profiles,
            std::env::var_os("PATH").as_deref(),
        ))
    }

    /// The stamp of `paths`, as each stands now.
    pub fn of_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            seen: paths
                .into_iter()
                .map(|path| {
                    let seen = seen(&path);
                    (path, seen)
                })
                .collect(),
        }
    }

    /// Whether every path still stands as it did.
    pub fn holds(&self) -> bool {
        self.seen.iter().all(|(path, was)| seen(path) == *was)
    }
}

/// How `path` stands now, through a link: a harness installed as a link to a versioned file is
/// updated by the link moving.
fn seen(path: &Path) -> Seen {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Every path a picker answer for `root` listing `profiles` was read from outside the plane
/// watch, `path_var` the `PATH` it resolved programs on.
fn read_from(root: &Path, profiles: &[Profile], path_var: Option<&OsStr>) -> Vec<PathBuf> {
    // Every spelling of each: the old one is read where the new one is not there.
    let named = [crate::names::PLANE_MANIFEST, crate::names::LOCAL_SETTINGS];
    let mut paths: Vec<PathBuf> = named
        .iter()
        .flat_map(|name| name.spellings().map(|spelled| root.join(spelled)))
        .collect();
    paths.push(root.join(".gitignore"));
    paths.push(PathBuf::from(crate::sandbox::policy::MACHINE_FILE));
    paths.extend(
        crate::machine::config_root_unheld().map(|config| crate::profilestore::path(&config)),
    );
    paths.extend(crate::profiletrust::records(root));
    paths.push(crate::sandbox::local::path(root));
    paths.push(crate::sandbox::local::everywhere_path());
    paths.extend(path_var.into_iter().flat_map(std::env::split_paths));
    paths.extend(profiles.iter().filter_map(crate::start::program_checked));
    paths.retain(|path| !path.as_os_str().is_empty());
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file's time moved forward, as an edit a second later would move it on any file system.
    fn touched(path: &Path) {
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open")
            .set_modified(later)
            .expect("set its time");
    }

    #[test]
    fn a_stamp_holds_while_nothing_it_names_moves() {
        let dir = tempfile::tempdir().expect("a dir");
        let file = dir.path().join("policy.json");
        std::fs::write(&file, "{}").expect("a file");
        let stamp = Stamp::of_paths([file, dir.path().join("absent")]);

        assert!(stamp.holds());
    }

    #[test]
    fn a_file_edited_made_or_removed_breaks_the_stamp() {
        let dir = tempfile::tempdir().expect("a dir");
        let edited = dir.path().join("profiles.json");
        std::fs::write(&edited, "{}").expect("a file");
        let stamp = Stamp::of_paths([edited.clone()]);
        touched(&edited);
        assert!(!stamp.holds(), "edited");

        let made = dir.path().join("policy.json");
        let stamp = Stamp::of_paths([made.clone()]);
        std::fs::write(&made, "{}").expect("made");
        assert!(!stamp.holds(), "made");

        let stamp = Stamp::of_paths([made.clone()]);
        std::fs::remove_file(&made).expect("removed");
        assert!(!stamp.holds(), "removed");
    }

    #[test]
    fn a_harness_installed_in_a_folder_on_path_breaks_the_stamp() {
        let dir = tempfile::tempdir().expect("a dir");
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).expect("a folder on PATH");
        let root = dir.path().join("plane");
        let path_var = std::env::join_paths([&bin]).expect("a PATH");
        let stamp = Stamp::of_paths(read_from(&root, &[], Some(&path_var)));
        assert!(stamp.holds());

        std::fs::write(bin.join("claude"), "#!/bin/sh\n").expect("installed");
        // A folder's time is its entries' change, kept to the second on some file systems.
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::open(&bin)
            .expect("the folder")
            .set_modified(later)
            .ok();

        assert!(!stamp.holds());
    }

    #[test]
    fn the_stamp_names_the_policy_the_projects_approvals_and_its_settings() {
        let root = Path::new("/home/dev/plane");
        let paths = read_from(root, &[], None);

        for name in [
            "purlis.toml",
            "charter.toml",
            "purlis.local.toml",
            "charter.local.toml",
        ] {
            assert!(paths.contains(&root.join(name)), "{name}");
        }

        assert!(paths.contains(&PathBuf::from(crate::sandbox::policy::MACHINE_FILE)));
        for record in crate::profiletrust::records(root) {
            assert!(paths.contains(&record), "{}", record.display());
        }
    }
}

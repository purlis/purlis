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
//!   the check searches ([`crate::programs::search_dirs`]), `PATH` and the fixed folders after
//!   it, where a harness newly installed lands;
//! - the project's `.git/index` and `.git/info/exclude`, which the check of whether its local
//!   settings are ignored reads;
//! - the profiles' own SVG icons, in the config home's icons folder, and that folder (#4885).
//!
//! And the project's own manifest, local settings and `.gitignore`, and its personas folder and
//! each persona's definition (#4885), which the watch does follow:
//! on a busy Mac it tells a change seconds late (#577), and a profile edited and the dialog
//! opened straight after must not draw the profile as it was.
//!
//! A [`Stamp`] is each of those paths as a `profiles::FileStamp` (inode, length,
//! change and modified times), or its absence. It costs one `stat` a path and one listing of the
//! personas folder, no read and no process, and a kept answer is served only while its stamp
//! still holds. It is taken before the reads it covers (#4885): [`Stamp::before_reading`] before
//! the profiles are read, and [`Stamp::and_named_by`] once they are and before what they name is.

use crate::profiles::{FileStamp, ProfileSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// How one path stood, or `None` where nothing was there.
type Seen = Option<FileStamp>;

/// The paths a picker answer was read from outside the plane watch, each as it stood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    seen: Vec<(PathBuf, Seen)>,
}

impl Stamp {
    /// The stamp of what a picker answer for the project at `root` reads before it knows its
    /// profiles, on this machine as it stands now. **Taken before those reads**, so a change
    /// made while they read breaks it; [`Self::and_named_by`] adds what the profiles name.
    pub fn before_reading(root: &Path) -> Self {
        Self::of_paths(before_reading(root, &Here::this()))
    }

    /// This stamp, and what the profiles `set` holds name, as it stands now: each program the
    /// check resolves. Taken once the profiles are read and before anything they name is.
    #[must_use]
    pub fn and_named_by(mut self, set: &ProfileSet) -> Self {
        self.seen
            .extend(Self::of_paths(named_by(set, &Here::this())).seen);
        self
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
    FileStamp::followed(path)
}

/// Where a picker answer looks on this machine besides the project: the `PATH` it resolves
/// programs on, the home folder whose own program folders it searches after it, and the config
/// home its profile store is in.
struct Here<'a> {
    path_var: Option<&'a OsStr>,
    home: Option<&'a Path>,
    config: Option<&'a Path>,
}

impl Here<'static> {
    fn this() -> Here<'static> {
        Here {
            path_var: None,
            home: None,
            config: None,
        }
    }
}

/// Every path a picker answer for `root` reads outside the plane watch before it knows its
/// profiles, as `here` finds them.
fn before_reading(root: &Path, here: &Here<'_>) -> Vec<PathBuf> {
    let path_var = here
        .path_var
        .map(OsStr::to_os_string)
        .or_else(|| std::env::var_os("PATH"));
    let home = here
        .home
        .map(Path::to_path_buf)
        .or_else(crate::profiles::home);
    let config = here
        .config
        .map(Path::to_path_buf)
        .or_else(crate::machine::config_root_unheld);
    // Every spelling of each: the old one is read where the new one is not there.
    let named = [crate::names::PLANE_MANIFEST, crate::names::LOCAL_SETTINGS];
    let mut paths: Vec<PathBuf> = named
        .iter()
        .flat_map(|name| name.spellings().map(|spelled| root.join(spelled)))
        .collect();
    paths.push(root.join(".gitignore"));
    // What the check of whether the local settings are ignored reads besides `.gitignore`.
    paths.push(root.join(".git").join("index"));
    paths.push(root.join(".git").join("info").join("exclude"));
    paths.push(PathBuf::from(crate::sandbox::policy::MACHINE_FILE));
    paths.extend(config.as_deref().map(crate::profilestore::path));
    paths.extend(crate::profiletrust::records(root));
    paths.push(crate::sandbox::local::path(root));
    paths.push(crate::sandbox::local::everywhere_path());
    // Every folder the program check searches, not only `PATH`: a Finder launch's `PATH` is the
    // system's four, and a harness installed into `~/.local/bin` is found there all the same.
    paths.extend(crate::programs::search_dirs_from(
        path_var.as_deref(),
        home.as_deref(),
    ));
    paths.extend(personas(root));
    // The folder of the profiles' own icons: one made or removed there moves it.
    paths.extend(config.as_deref().map(crate::harness_icon::folder));
    paths.retain(|path| !path.as_os_str().is_empty());
    paths
}

/// The plane's personas folder, and each persona's definition in it, in either layout, as
/// [`crate::workspaces::Plane::personas`] and [`crate::personas::def_path`] find them: one
/// `read_dir` and no file read.
fn personas(root: &Path) -> Vec<PathBuf> {
    let folder = root.join("personas");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                path.join("persona.md")
            } else {
                path
            }
        })
        .collect();
    paths.sort();
    paths.insert(0, folder);
    paths
}

/// Every path the profiles `set` holds name, as `here` finds them: each program the check
/// resolves, and each SVG of the profiles' own in the config home's icons folder.
fn named_by(set: &ProfileSet, here: &Here<'_>) -> Vec<PathBuf> {
    let config = here
        .config
        .map(Path::to_path_buf)
        .or_else(crate::machine::config_root_unheld);
    let icons = config.as_deref().map(crate::harness_icon::folder);
    let own_icons = set.profiles().iter().filter_map(|profile| {
        match crate::harness_icon::named(set.icon_of(&profile.name)?) {
            crate::harness_icon::Named::File(file) => Some(icons.as_ref()?.join(file)),
            _ => None,
        }
    });
    set.profiles()
        .iter()
        .filter_map(crate::start::program_checked)
        .chain(own_icons)
        .filter(|path| !path.as_os_str().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

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
        let stamp = Stamp::of_paths(before_reading(
            &root,
            &Here {
                path_var: Some(&path_var),
                ..Here::this()
            },
        ));
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
    fn a_harness_installed_in_a_folder_searched_off_path_breaks_the_stamp() {
        // A Finder launch: PATH is the system's four folders, and a harness lands in the
        // operator's own ~/.local/bin, which the program check searches all the same.
        let dir = tempfile::tempdir().expect("a dir");
        let home = dir.path().join("home");
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&bin).expect("~/.local/bin");
        let root = dir.path().join("plane");
        let finder = OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin");
        let stamp = Stamp::of_paths(before_reading(
            &root,
            &Here {
                path_var: Some(finder),
                home: Some(&home),
                ..Here::this()
            },
        ));
        assert!(stamp.holds());

        std::fs::write(bin.join("claude"), "#!/bin/sh\n").expect("installed");

        assert!(!stamp.holds());
    }

    #[test]
    fn a_file_rewritten_to_the_same_length_and_time_breaks_the_stamp() {
        // A save that keeps the length, inside the file system's time step, or a tool that puts
        // the time back: the change time still moves.
        let dir = tempfile::tempdir().expect("a dir");
        let file = dir.path().join("purlis.toml");
        std::fs::write(&file, "level = 1").expect("a file");
        let was = std::fs::metadata(&file)
            .expect("its time")
            .modified()
            .expect("a time");
        let stamp = Stamp::of_paths([file.clone()]);
        std::thread::sleep(std::time::Duration::from_millis(20));

        std::fs::write(&file, "level = 2").expect("rewritten");
        std::fs::File::options()
            .write(true)
            .open(&file)
            .expect("open")
            .set_modified(was)
            .expect("its time put back");

        assert!(!stamp.holds());
    }

    #[test]
    fn the_stamp_names_the_git_files_the_ignore_check_reads() {
        let root = Path::new("/home/dev/plane");
        let paths = before_reading(root, &Here::this());

        for git in [".git/index", ".git/info/exclude"] {
            assert!(paths.contains(&root.join(git)), "{git}");
        }
    }

    #[test]
    fn the_stamp_names_the_policy_the_projects_approvals_and_its_settings() {
        let root = Path::new("/home/dev/plane");
        let paths = before_reading(root, &Here::this());

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

    #[test]
    fn a_persona_edited_made_or_removed_breaks_the_stamp() {
        // The plane watch tells a persona's change, seconds late on a busy Mac (#577): the
        // stamp says it at once.
        let dir = tempfile::tempdir().expect("a dir");
        let root = dir.path();
        let personas = root.join("personas");
        std::fs::create_dir_all(personas.join("scribe")).expect("a persona");
        std::fs::write(personas.join("scribe/persona.md"), "# scribe\n").expect("its md");
        std::fs::write(personas.join("flat.md"), "# flat\n").expect("a flat persona");
        std::fs::create_dir_all(personas.join("reviewer")).expect("a folder, no persona yet");
        let stamp = || Stamp::of_paths(before_reading(root, &Here::this()));

        let was = stamp();
        touched(&personas.join("scribe/persona.md"));
        assert!(!was.holds(), "a persona edited");

        let was = stamp();
        touched(&personas.join("flat.md"));
        assert!(!was.holds(), "a flat persona edited");

        let was = stamp();
        std::fs::write(personas.join("reviewer/persona.md"), "# reviewer\n").expect("made");
        assert!(!was.holds(), "a persona made");

        let was = stamp();
        std::fs::remove_file(personas.join("scribe/persona.md")).expect("removed");
        assert!(!was.holds(), "a persona removed");
    }

    #[test]
    fn a_profiles_own_icon_edited_made_or_removed_breaks_the_stamp() {
        let dir = tempfile::tempdir().expect("a dir");
        let root = dir.path().join("plane");
        std::fs::create_dir_all(&root).expect("a plane");
        std::fs::write(
            root.join(crate::profiles::LOCAL_FILE),
            "[harness.mine]\nkind = \"claude\"\ncommand = [\"claude\"]\nicon = \"mine.svg\"\n",
        )
        .expect("a profile with its own icon");
        let set = crate::profiles::current(&root);
        assert_eq!(set.icon_of("mine"), Some("mine.svg"), "{:?}", set.refused);
        let config = dir.path().join("config");
        let icons = crate::harness_icon::folder(&config);
        std::fs::create_dir_all(&icons).expect("the icons folder");
        let here = Here {
            config: Some(&config),
            ..Here::this()
        };
        let stamp = || Stamp::of_paths(named_by(&set, &here));

        let was = stamp();
        std::fs::write(icons.join("mine.svg"), "<svg/>").expect("made");
        assert!(!was.holds(), "an icon made");

        let was = stamp();
        touched(&icons.join("mine.svg"));
        assert!(!was.holds(), "an icon edited");

        let was = stamp();
        std::fs::remove_file(icons.join("mine.svg")).expect("removed");
        assert!(!was.holds(), "an icon removed");
    }
}

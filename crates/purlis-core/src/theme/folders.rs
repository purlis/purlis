// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The levels' theme folders** (spec #2313, #2323): the person's `themes/` in purlis's config
//! folder, the project's at the plane root, and a workspace's inside it — and **which of them a
//! choice reaches**.
//!
//! # A choice reaches its own level and the levels farther out
//!
//! A choice made at a level names a theme folder of that level or of a level farther out, never
//! a nearer one: the machine's choice is read from the person's own folders only; the project's
//! choice and its committed default from the project's and the person's; a workspace's from the
//! workspace's, the project's and the person's. A folder `extends` by the same rule: it reaches
//! its own level and the levels farther out. So what a nearer level holds never stands in for
//! what a farther level chose.
//!
//! # Read through the handles the check opened
//!
//! Each `themes/` folder is opened from its level's root one folder at a time, refusing a link
//! at every step; its folders and their `theme.json` are opened from that handle, refusing a link
//! again; and the file is read from the handle that was checked. Nothing is looked up by path
//! between the check and the read. A theme folder's fonts are read from the same handle
//! (`super::fonts`).
//!
//! # A project's fonts need the project opened
//!
//! A theme folder of the project or of a workspace has its fonts read only when the project is
//! one the window opened through the trust gate (ADR 0035): [`Folders::opened`] says so, and
//! [`Folders::of`] reads every level's `theme.json` but the person's own fonts alone.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::format::FolderRead;
use super::{Complaint, Library, Resolved, THEME_JSON};

/// The reserved folder a level keeps its theme folders in.
pub const THEMES: &str = "themes";

/// The most theme folders one level's `themes/` is read for: a level past it is read for the
/// first this many by name, and the rest are said and left out.
pub const MOST_FOLDERS: usize = 64;

/// The largest `theme.json` read: the one cap of every theme file.
const MOST_BYTES: u64 = crate::windowprefs::MAX_BYTES;

/// **How far out a choice reaches**: the nearest level whose folders it may name. A choice
/// reaches that level and every level farther out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reach {
    /// The workspace's folders, the project's and the person's.
    Workspace,
    /// The project's folders and the person's.
    Project,
    /// The person's own folders alone.
    Machine,
}

/// **One level's `themes/` folder**: the root of its level (the plane, or the config home),
/// which the caller trusts, and the path below it, every step of which is opened refusing a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemesFolder {
    /// What the walk starts from.
    pub root: PathBuf,
    /// The `themes/` folder, relative to [`Self::root`].
    pub below: PathBuf,
    /// Whether its theme folders' fonts are read, or why not: the person's own always; a
    /// project's and a workspace's only for a project the window opened ([`Folders::opened`]).
    pub fonts: Result<(), String>,
}

impl ThemesFolder {
    /// The folder as a path, for a sentence.
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.below)
    }
}

/// What one level's `themes/` holds: each theme folder as read, or why not.
type Held = BTreeMap<String, Result<FolderRead, String>>;

/// **The `themes/` folders of the three levels** a window, a project or a workspace has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folders {
    pub workspace: Option<ThemesFolder>,
    pub project: Option<ThemesFolder>,
    pub machine: Option<ThemesFolder>,
}

/// What [`Folders::find`] found.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// A folder of that name within reach, read: drawn, or refused with why.
    Read(Resolved),
    /// No folder of that name within reach; what reading the levels had to say.
    Missing(Vec<Complaint>),
}

impl Folders {
    /// The levels' folders: the person's under `config_root`'s purlis folder, the project's at
    /// `plane`'s root, and `workspace`'s inside it. A workspace whose name is not one is left
    /// out. The project's and the workspace's fonts are not read: [`Self::opened`] reads them.
    pub fn of(config_root: Option<&Path>, plane: Option<&Path>, workspace: Option<&str>) -> Self {
        Self::at(config_root, plane, workspace, false)
    }

    /// [`Self::of`] for `plane`, a project **the window opened through the trust gate** (ADR
    /// 0035): its and its workspace's theme folders' fonts are read too. Only a caller holding
    /// a project the person opened calls this.
    pub fn opened(
        config_root: Option<&Path>,
        plane: Option<&Path>,
        workspace: Option<&str>,
    ) -> Self {
        Self::at(config_root, plane, workspace, true)
    }

    fn at(
        config_root: Option<&Path>,
        plane: Option<&Path>,
        workspace: Option<&str>,
        opened: bool,
    ) -> Self {
        let plane_fonts = |whose: &str| {
            if opened {
                Ok(())
            } else {
                Err(format!(
                    "a {whose} theme folder's fonts are read only in a project purlis opened"
                ))
            }
        };
        let machine = config_root.map(|root| {
            let dir = crate::machine::dir(root);
            ThemesFolder {
                root: root.to_path_buf(),
                below: dir
                    .strip_prefix(root)
                    .map(Path::to_path_buf)
                    .unwrap_or_default()
                    .join(THEMES),
                fonts: Ok(()),
            }
        });
        let project = plane.map(|root| ThemesFolder {
            root: root.to_path_buf(),
            below: PathBuf::from(THEMES),
            fonts: plane_fonts("project's"),
        });
        let workspace = plane
            .zip(workspace.filter(|name| crate::contain::workspace_name_ok(name)))
            .map(|(root, name)| ThemesFolder {
                root: root.to_path_buf(),
                below: Path::new("workspaces").join(name).join(THEMES),
                fonts: plane_fonts("workspace's"),
            });
        Self {
            workspace,
            project,
            machine,
        }
    }

    /// The levels `reach` reaches, nearest first.
    fn within(&self, reach: Reach) -> Vec<&ThemesFolder> {
        [
            (Reach::Workspace, &self.workspace),
            (Reach::Project, &self.project),
            (Reach::Machine, &self.machine),
        ]
        .into_iter()
        .filter(|(level, _)| *level >= reach)
        .filter_map(|(_, folder)| folder.as_ref())
        .collect()
    }

    /// **The theme folder `name`**, from the nearest level within `reach` that has one, read
    /// with a library of that level and the levels farther out: what it may extend.
    pub fn find(&self, name: &str, reach: Reach) -> Found {
        let (levels, said) = self.read_within(reach);
        match levels.iter().position(|held| held.contains_key(name)) {
            Some(at) => Found::Read(load(&library_of(&levels[at..]), name, said)),
            None => Found::Missing(said),
        }
    }

    /// **The theme folder `name` of the level `at` itself**, read exactly as a choice made at
    /// that level reads it ([`Self::find`]). `Missing` when that level has no folder of that
    /// name: a farther level's folder of the name is not read in its place.
    pub fn find_at(&self, name: &str, at: Reach) -> Found {
        let (levels, said) = self.read_within(at);
        match levels.first() {
            Some(held) if self.level(at).is_some() && held.contains_key(name) => {
                Found::Read(load(&library_of(&levels), name, said))
            }
            _ => Found::Missing(said),
        }
    }

    /// **Every theme folder of the level `at` itself**, by name, each read exactly as
    /// [`Self::find_at`] reads it: what `purlis theme list` lists for that level. Empty when
    /// the level has no `themes/`.
    pub fn every_at(&self, at: Reach) -> Vec<(String, Resolved)> {
        if self.level(at).is_none() {
            return Vec::new();
        }
        let (levels, said) = self.read_within(at);
        let Some(nearest) = levels.first() else {
            return Vec::new();
        };
        let library = library_of(&levels);
        nearest
            .keys()
            .map(|name| (name.clone(), load(&library, name, said.clone())))
            .collect()
    }

    /// The `themes/` folder of the level `at`, when there is one.
    fn level(&self, at: Reach) -> Option<&ThemesFolder> {
        match at {
            Reach::Workspace => self.workspace.as_ref(),
            Reach::Project => self.project.as_ref(),
            Reach::Machine => self.machine.as_ref(),
        }
    }

    /// The levels `reach` reaches, nearest first, each read, and what reading them had to say.
    fn read_within(&self, reach: Reach) -> (Vec<Held>, Vec<Complaint>) {
        let mut said = Vec::new();
        let levels = self
            .within(reach)
            .into_iter()
            .map(|folder| read_level(folder, &mut said))
            .collect();
        (levels, said)
    }

    /// Every theme folder's name within `reach`, nearest first and each once: what a pick there
    /// may name.
    pub fn names(&self, reach: Reach) -> Vec<String> {
        let mut said = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for folder in self.within(reach) {
            for name in read_level(folder, &mut said).into_keys() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }
}

/// The library of the levels read, nearest first: farthest first into it, so a nearer level's
/// folder of a name takes a farther one's place.
fn library_of(levels: &[Held]) -> Library {
    let mut library = Library::new();
    for held in levels.iter().rev() {
        for (one, read) in held {
            library = library.with_read(one, read.clone());
        }
    }
    library
}

/// The theme folder `name` read with `library`, what reading the levels said (`said`) before
/// what reading it says.
fn load(library: &Library, name: &str, mut said: Vec<Complaint>) -> Resolved {
    let mut resolved = library.load_id(name);
    said.append(&mut resolved.complaints);
    resolved.complaints = said;
    resolved
}

/// One level's theme folders, each read, or nothing when the level has no `themes/`; what is
/// wrong with the level itself is said in `said`.
pub(super) fn read_level(folder: &ThemesFolder, said: &mut Vec<Complaint>) -> Held {
    match read::level(folder) {
        Ok((held, notes)) => {
            said.extend(notes.into_iter().map(Complaint::said));
            held
        }
        Err(why) => {
            said.push(Complaint::said(why));
            Held::new()
        }
    }
}

/// Whether `below` is a plain relative path of folder names, which is all a level's `themes/`
/// is below its root.
fn plain_below(below: &Path) -> bool {
    below
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
}

#[cfg(unix)]
mod read {
    //! The reads, from handles: every step opened with `O_NOFOLLOW`, the file read from the
    //! descriptor that was checked.

    use std::io::Read;
    use std::os::fd::OwnedFd;

    use rustix::fs::{Dir, Mode, OFlags, openat};
    use rustix::io::Errno;

    use std::path::{Component, Path};

    use super::{Held, MOST_BYTES, MOST_FOLDERS, THEME_JSON, ThemesFolder, Value, plain_below};
    use crate::theme::fonts::ThemeDir;

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    /// The level's theme folders, read, and what is worth saying about them; `Ok(empty)` when
    /// it has no `themes/`.
    pub(super) fn level(folder: &ThemesFolder) -> Result<(Held, Vec<String>), String> {
        let shown = folder.path();
        let shown = shown.display();
        if !plain_below(&folder.below) {
            return Err(format!("{shown} is not a folder purlis reads themes from"));
        }
        // The level's root is the caller's to trust (a temp directory is itself reached through
        // a link on macOS); every step below it is opened refusing one.
        let mut at: OwnedFd = match rustix::fs::open(
            &folder.root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok((Held::new(), Vec::new())),
            Err(why) => {
                return Err(format!(
                    "purlis could not open {}: {why}",
                    folder.root.display()
                ));
            }
        };
        for step in folder.below.components() {
            at = match openat(&at, step.as_os_str(), DIRECTORY, Mode::empty()) {
                Ok(fd) => fd,
                Err(Errno::NOENT) => return Ok((Held::new(), Vec::new())),
                Err(Errno::LOOP | Errno::NOTDIR | Errno::MLINK) => {
                    return Err(format!(
                        "{shown} is reached through a link or something that is not a folder, \
                         so no theme is read from it"
                    ));
                }
                Err(why) => return Err(format!("purlis could not open {shown}: {why}")),
            };
        }
        let names = names_in(&at).map_err(|why| format!("purlis could not list {shown}: {why}"))?;
        let mut notes = Vec::new();
        if names.len() > MOST_FOLDERS {
            notes.push(format!(
                "{shown} holds {} theme folders, and purlis reads the first {MOST_FOLDERS}",
                names.len()
            ));
        }
        let held = names
            .iter()
            .take(MOST_FOLDERS)
            .map(|name| {
                let read =
                    theme_json(&at, &shown.to_string(), name).map(|(document, icons, dir)| {
                        let path = folder.path().join(name);
                        // Its fonts are read from the handle its `theme.json` was read through.
                        let held = match &folder.fonts {
                            Ok(()) => ThemeDir::held(path, dir),
                            Err(why) => ThemeDir::refusing(path, why.clone()),
                        };
                        (document, icons, held)
                    });
                (name.clone(), read)
            })
            .collect();
        Ok((held, notes))
    }

    /// The names in the folder `at` that are theme folders' names, sorted. A name starting with
    /// `.` (a `.gitkeep`) and any other that is not a theme folder's name are left out.
    fn names_in(at: &OwnedFd) -> rustix::io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in Dir::read_from(at)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().to_str() else {
                continue;
            };
            if name.starts_with('.') || !crate::theme::is_folder_name(name) {
                continue;
            }
            names.push(name.to_owned());
        }
        names.sort();
        Ok(names)
    }

    /// A theme folder opened: its `theme.json`, the icon set it names, and the folder's handle.
    type Opened = (Value, Option<Result<Value, String>>, OwnedFd);

    /// The theme folder `name` in `themes`: its `theme.json`, parsed, the icon set it names,
    /// each read from a handle opened from `themes` refusing a link, and that handle; or why not.
    fn theme_json(themes: &OwnedFd, shown: &str, name: &str) -> Result<Opened, String> {
        let folder = format!("{shown}/{name}");
        let dir = match openat(themes, name, DIRECTORY, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::LOOP | Errno::NOTDIR | Errno::MLINK) => {
                return Err(format!(
                    "{folder} is a link or not a folder, so it is not read"
                ));
            }
            Err(why) => return Err(format!("purlis could not open {folder}: {why}")),
        };
        let file = format!("{folder}/{THEME_JSON}");
        let text = read_capped(&dir, THEME_JSON, &file, || {
            format!("{folder} has no {THEME_JSON}")
        })?;
        let document =
            crate::theme::load_text_document(&text).map_err(|why| format!("{file}: {why}"))?;
        let icons = match document.get("icons") {
            Some(Value::String(named)) => Some(icons(&dir, &folder, named)),
            _ => None,
        };
        Ok((document, icons, dir))
    }

    /// The icon set `named` inside the folder `dir`, parsed, or why not: a relative path of
    /// plain names, each folder on the way opened from the last refusing a link.
    fn icons(dir: &OwnedFd, folder: &str, named: &str) -> Result<Value, String> {
        let path = Path::new(named);
        let inside = !named.is_empty()
            && named.len() <= crate::theme::format::MOST_ICONS_PATH
            && !named.contains(['\\', ':'])
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_)));
        if !inside {
            return Err(format!(
                "icons is {}, which is not a path inside the theme's folder",
                serde_json::to_string(named).unwrap_or_default()
            ));
        }
        let parts: Vec<_> = path.components().map(|part| part.as_os_str()).collect();
        let Some((last, on_the_way)) = parts.split_last() else {
            return Err(format!("{folder} has no {named}"));
        };
        let mut at =
            rustix::io::dup(dir).map_err(|why| format!("purlis could not open {folder}: {why}"))?;
        for step in on_the_way {
            at = match openat(&at, *step, DIRECTORY, Mode::empty()) {
                Ok(fd) => fd,
                Err(Errno::NOENT) => return Err(format!("{folder} has no {named}")),
                Err(Errno::LOOP | Errno::NOTDIR | Errno::MLINK) => {
                    return Err(format!(
                        "{folder}/{named} is reached through a link, so it is not read"
                    ));
                }
                Err(why) => return Err(format!("purlis could not read {folder}/{named}: {why}")),
            };
        }
        let file = format!("{folder}/{named}");
        let text = read_capped(&at, *last, &file, || format!("{folder} has no {named}"))?;
        serde_json::from_str::<Value>(&text)
            .map_err(|why| format!("{file}: an icon set is JSON: {why}"))
    }

    /// The text of the file `name` in `dir`, opened refusing a link, a plain file under the
    /// cap, read from the descriptor that was checked; `missing` says it is not there.
    fn read_capped(
        dir: &OwnedFd,
        name: impl rustix::path::Arg,
        file: &str,
        missing: impl FnOnce() -> String,
    ) -> Result<String, String> {
        let fd = match openat(
            dir,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Err(missing()),
            Err(Errno::LOOP | Errno::MLINK) => {
                return Err(format!("{file} is a link, so it is not read"));
            }
            Err(why) => return Err(format!("purlis could not read {file}: {why}")),
        };
        let mut open = std::fs::File::from(fd);
        let found = open
            .metadata()
            .map_err(|why| format!("purlis could not read {file}: {why}"))?;
        if !found.file_type().is_file() {
            return Err(format!("{file} is not a plain file"));
        }
        let too_big = |bytes: u64| {
            format!("{file} is {bytes} bytes, and a theme's file is never larger than {MOST_BYTES}")
        };
        if found.len() > MOST_BYTES {
            return Err(too_big(found.len()));
        }
        // The file can grow after its size was asked: the read stops one byte past the cap.
        let mut text = String::new();
        (&mut open)
            .take(MOST_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|why| format!("purlis could not read {file}: {why}"))?;
        if text.len() as u64 > MOST_BYTES {
            return Err(too_big(text.len() as u64));
        }
        Ok(text)
    }
}

#[cfg(not(unix))]
mod read {
    //! **No theme folder is read off unix**: the reads open each step refusing a link, which
    //! this build cannot express, and a guard that cannot be expressed refuses rather than
    //! degrades (ADR 0031, as `crate::machine` does for the whole machine store).

    use super::{Held, ThemesFolder};

    pub(super) fn level(folder: &ThemesFolder) -> Result<(Held, Vec<String>), String> {
        Err(format!(
            "{} is not read: theme folders are read on macOS and Linux",
            folder.path().display()
        ))
    }
}

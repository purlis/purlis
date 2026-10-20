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
//! # A project's fonts are read only as approved
//!
//! Every font file a project's or a workspace's theme folders bring is part of what the project
//! contributes ([`plane_fonts`], [`crate::machine::Contribution::fonts`]), so the trust ask
//! (ADR 0035) lists it and asks again when one changes. [`Folders::of`] reads this machine's
//! approval of the project itself, and a face from the project's or a workspace's folder is read
//! only when that approval holds its file with the digest of the bytes read. The person's own
//! folders' faces need no approval.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::format::FolderRead;
use super::{Complaint, Library, Resolved};

/// The reserved folder a level keeps its theme folders in.
pub const THEMES: &str = "themes";

/// The most theme folders one level's `themes/` is read for: a level past it is read for the
/// first this many by name, and the rest are said and left out.
pub const MOST_FOLDERS: usize = 64;

/// **How far out a choice reaches**: the nearest level whose folders it may name. A choice
/// reaches that level and every level farther out.
///
/// It is also **a level a theme folder is of** (#2516): the workspace's, the project's or the
/// person's own, written `workspace`, `project` or `machine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    /// The workspace's folders, the project's and the person's.
    Workspace,
    /// The project's folders and the person's.
    Project,
    /// The person's own folders alone.
    Machine,
}

impl Reach {
    /// The three levels, nearest first.
    pub const EVERY: [Self; 3] = [Self::Workspace, Self::Project, Self::Machine];

    /// **The level's word**, as a choice and the gallery write it: `workspace`, `project` or
    /// `machine`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Project => "project",
            Self::Machine => "machine",
        }
    }

    /// The level a word names ([`Self::word`]); `None` for any other.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::EVERY.into_iter().find(|one| one.word() == word)
    }
}

/// **One level's `themes/` folder**: the root of its level (the plane, or the config home),
/// which the caller trusts, and the path below it, every step of which is opened refusing a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemesFolder {
    /// What the walk starts from.
    pub root: PathBuf,
    /// The `themes/` folder, relative to [`Self::root`].
    pub below: PathBuf,
    /// Which of its theme folders' faces are read.
    pub(crate) fonts: FontGate,
}

/// **Which of a level's faces are read.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FontGate {
    /// The person's own folders: every face.
    Own,
    /// A project's or a workspace's: a face whose file this machine's approval of the project
    /// holds with that digest, keyed by its path in the project. Empty when the project is not
    /// approved here.
    Approved(std::sync::Arc<BTreeMap<String, String>>),
    /// One chat's theme drafts (`super::drafts`): a face is read only when the project's
    /// approval holds it, with that digest, where the draft is kept to: `kept_at` names, for
    /// each draft's folder name, the `themes/` folder in the project it is kept in.
    Draft {
        files: std::sync::Arc<BTreeMap<String, String>>,
        kept_at: std::sync::Arc<BTreeMap<String, PathBuf>>,
    },
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

/// **The font files this machine's approval of the project at `plane` holds**, by their path in
/// the project, read from `config_root`'s store: empty when it is not approved here.
pub(crate) fn approved_fonts(
    config_root: Option<&Path>,
    plane: Option<&Path>,
) -> BTreeMap<String, String> {
    config_root
        .zip(plane)
        .and_then(|(config, plane)| {
            crate::machine::read(config)
                .store
                .recent(plane)
                .and_then(|entry| entry.trust.as_ref())
                .map(|trust| trust.contributed.fonts.clone())
        })
        .unwrap_or_default()
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
    /// out. The project's and the workspace's faces are read only as this machine's approval
    /// of the project, read here from `config_root`'s store, holds them.
    pub fn of(config_root: Option<&Path>, plane: Option<&Path>, workspace: Option<&str>) -> Self {
        let approved = std::sync::Arc::new(approved_fonts(config_root, plane));
        let machine = config_root.map(|root| {
            let dir = crate::machine::dir(root);
            ThemesFolder {
                root: root.to_path_buf(),
                below: dir
                    .strip_prefix(root)
                    .map(Path::to_path_buf)
                    .unwrap_or_default()
                    .join(THEMES),
                fonts: FontGate::Own,
            }
        });
        let project = plane.map(|root| ThemesFolder {
            root: root.to_path_buf(),
            below: PathBuf::from(THEMES),
            fonts: FontGate::Approved(approved.clone()),
        });
        let workspace = plane
            .zip(workspace.filter(|name| crate::contain::workspace_name_ok(name)))
            .map(|(root, name)| ThemesFolder {
                root: root.to_path_buf(),
                below: Path::new("workspaces").join(name).join(THEMES),
                fonts: FontGate::Approved(approved),
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
    /// that level reads it ([`Self::find`]), but only to check ([`Library::checking`]): it is
    /// what `purlis theme check` and `export` read, never what the window draws, so none of its
    /// faces is registered to be served. `Missing` when that level has no folder of that name:
    /// a farther level's folder of the name is not read in its place.
    pub fn find_at(&self, name: &str, at: Reach) -> Found {
        self.found_at(name, at, Library::checking)
    }

    /// **The theme folder `name` of the level `at` itself, to draw** (#2516): what a choice that
    /// names that level's folder draws, read with a library of that level and the levels
    /// farther out, as [`Self::find`] reads the nearest. `Missing` when that level has no folder
    /// of that name: a folder of the name at another level is never read in its place.
    pub fn find_in(&self, name: &str, at: Reach) -> Found {
        self.found_at(name, at, |library| library)
    }

    /// [`Self::find_at`] and [`Self::find_in`]: the folder `name` of the level `at`, read with
    /// the library `library` makes.
    fn found_at(&self, name: &str, at: Reach, library: impl FnOnce(Library) -> Library) -> Found {
        let (levels, said) = self.read_within(at);
        match levels.first() {
            Some(held) if self.level(at).is_some() && held.contains_key(name) => {
                Found::Read(load(&library(library_of(&levels)), name, said))
            }
            _ => Found::Missing(said),
        }
    }

    /// **The theme folder `name` of the level `at` itself, held** (#2670): found as
    /// [`Self::find_at`] finds it, with the handle its `theme.json` was read through, so what is
    /// done with it next ([`FoundFolder::export`]) reads every file from inside the folder that
    /// was found, never by a path looked up again. `Err`, with what reading the level said,
    /// when it has no folder of that name.
    pub fn folder_at(&self, name: &str, at: Reach) -> Result<FoundFolder, Vec<Complaint>> {
        let Some(level) = self.level(at) else {
            return Err(Vec::new());
        };
        let mut said = Vec::new();
        match read_level(level, &mut said).remove(name) {
            Some(read) => Ok(FoundFolder {
                name: name.to_owned(),
                path: level.path().join(name),
                read: read.map(|(_, _, folder)| folder),
            }),
            None => Err(said),
        }
    }

    /// **Every theme folder of the level `at` itself**, by name, each read exactly as
    /// [`Self::find_at`] reads it, only to check: what `purlis theme list` lists for that
    /// level. Empty when
    /// the level has no `themes/`.
    pub fn every_at(&self, at: Reach) -> Vec<(String, Resolved)> {
        if self.level(at).is_none() {
            return Vec::new();
        }
        let (levels, said) = self.read_within(at);
        let Some(nearest) = levels.first() else {
            return Vec::new();
        };
        let library = library_of(&levels).checking();
        nearest
            .keys()
            .map(|name| (name.clone(), load(&library, name, said.clone())))
            .collect()
    }

    /// **The theme draft `name` among `drafts`** (one chat's drafts, `super::drafts`), read as
    /// the nearest level of all, with a library of the drafts and every level a workspace's
    /// choice reaches: what it may extend. `checking` reads it only to check
    /// ([`Library::checking`]), so none of its faces is registered to be served. `Missing` when
    /// the chat has no draft of that name.
    pub fn find_draft(&self, drafts: &ThemesFolder, name: &str, checking: bool) -> Found {
        let mut said = Vec::new();
        let draft = read_level(drafts, &mut said);
        if !draft.contains_key(name) {
            return Found::Missing(said);
        }
        let (mut levels, more) = self.read_within(Reach::Workspace);
        said.extend(more);
        levels.insert(0, draft);
        let library = library_of(&levels);
        let library = if checking {
            library.checking()
        } else {
            library
        };
        Found::Read(load(&library, name, said))
    }

    /// **The library of every level `reach` reaches**, nearest first: what a theme folder read at
    /// that reach may extend.
    pub(crate) fn library_within(&self, reach: Reach) -> Library {
        let (levels, _) = self.read_within(reach);
        library_of(&levels)
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

/// **A level's theme folder as found** ([`Folders::folder_at`]): its name, where it is, and the
/// handle it was read through, or why it could not be read.
#[derive(Debug)]
pub struct FoundFolder {
    name: String,
    path: PathBuf,
    read: Result<super::fonts::ThemeDir, String>,
}

impl FoundFolder {
    /// The folder's name: the theme's id.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where the folder is, for a sentence.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// **The folder as one zip** ([`super::export`]), every file read from the handle it was
    /// found through.
    pub fn export(&self) -> Result<super::Packed, String> {
        let folder = self.read.as_ref().map_err(Clone::clone)?;
        super::archive::export_held(&self.name, folder.dir()?, &self.path.display().to_string())
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

impl Folders {
    /// **Every theme folder's name within `reach`, with the level it is read from**: nearest
    /// level first, by name within one, each name once, at the nearest level that has it, as
    /// [`Self::find`] reads it.
    pub fn named(&self, reach: Reach) -> Vec<(Reach, String)> {
        let mut said = Vec::new();
        let mut named: Vec<(Reach, String)> = Vec::new();
        for (level, folder) in self.levels(reach) {
            for name in read_level(folder, &mut said).into_keys() {
                if !named.iter().any(|(_, one)| *one == name) {
                    named.push((level, name));
                }
            }
        }
        named
    }

    /// **Every theme folder within `reach`, with its level** (#2516): nearest level first, by
    /// name within one, a name that two levels have once at each, so a gallery tells them
    /// apart.
    pub fn each(&self, reach: Reach) -> Vec<(Reach, String)> {
        let mut said = Vec::new();
        self.levels(reach)
            .into_iter()
            .flat_map(|(level, folder)| {
                read_level(folder, &mut said)
                    .into_keys()
                    .map(move |name| (level, name))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// The `themes/` folder of `level`, when there is one.
    pub fn at(&self, level: Reach) -> Option<&ThemesFolder> {
        match level {
            Reach::Workspace => self.workspace.as_ref(),
            Reach::Project => self.project.as_ref(),
            Reach::Machine => self.machine.as_ref(),
        }
    }

    /// The levels `reach` reaches with their folders, nearest first.
    fn levels(&self, reach: Reach) -> Vec<(Reach, &ThemesFolder)> {
        [Reach::Workspace, Reach::Project, Reach::Machine]
            .into_iter()
            .filter(|level| *level >= reach)
            .filter_map(|level| self.at(level).map(|folder| (level, folder)))
            .collect()
    }
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

/// A path in the project, as the trust record names a font file: its folders joined by `/`.
fn in_project(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The most workspaces whose theme folders a project's font files are listed from, by name.
const MOST_WORKSPACES_WALKED: usize = 64;

/// The most theme folders a project's font files are listed from, all levels together.
const MOST_FOLDERS_WALKED: usize = 256;

/// The most files opened when a project's font files are listed, all folders together.
const MOST_FILES_EXAMINED: usize = 256;

/// **Every font file the project at `plane` brings in its theme folders and its workspaces'**,
/// by its path in the project, to the sha-256 of its bytes and its size
/// (`super::fonts::listed_as`): what the trust ask lists and what an approval holds
/// (`crate::machine::Contribution::fonts`). Each is read as a face is, through handles opened
/// refusing a link, a plain woff2 file under the cap.
///
/// **One budget holds the whole listing**: at most `fonts::MOST_PROJECT_FONTS` files and
/// `fonts::MOST_PROJECT_FONT_BYTES` bytes, `fonts::MOST_FOLDER_FONTS` files in one folder,
/// [`MOST_WORKSPACES_WALKED`] workspaces, [`MOST_FOLDERS_WALKED`] theme folders and
/// [`MOST_FILES_EXAMINED`] files opened, the project's own folders first and then each
/// workspace's by name. A file not listed is never approved, so it is never drawn.
pub(crate) fn plane_fonts(plane: &Path) -> BTreeMap<String, String> {
    read::plane_fonts(plane)
}

#[cfg(unix)]
mod read {
    //! The reads, from handles: every step opened with `O_NOFOLLOW`, the file read from the
    //! descriptor that was checked.

    use std::collections::BTreeMap;
    use std::os::fd::OwnedFd;

    use rustix::fs::{Dir, Mode, OFlags, openat};

    use std::path::Path;

    use super::{FontGate, Held, MOST_FOLDERS, ThemesFolder, Value, in_project};
    use crate::theme::fonts;
    use crate::theme::fonts::ThemeDir;
    use crate::theme::opened;

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    /// The level's theme folders, read, and what is worth saying about them; `Ok(empty)` when
    /// it has no `themes/`.
    pub(super) fn level(folder: &ThemesFolder) -> Result<(Held, Vec<String>), String> {
        let shown = folder.path();
        let shown = shown.display();
        // The level's root is the caller's to trust (a temp directory is itself reached through
        // a link on macOS); every step below it is opened refusing one.
        let Some(at) = opened::walk(&folder.root, &folder.below, &shown.to_string())? else {
            return Ok((Held::new(), Vec::new()));
        };
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
                // A name Windows cannot hold is refused, said, and never read.
                let read = match crate::theme::portable::refusal(name, name) {
                    Some(why) => Err(format!("{shown}/{why}")),
                    None => {
                        theme_json(&at, &shown.to_string(), name).map(|(document, icons, dir)| {
                            let path = folder.path().join(name);
                            // Its fonts are read from the handle its `theme.json` was read through.
                            let held = ThemeDir::held(path, dir);
                            let held = match &folder.fonts {
                                FontGate::Own => held,
                                FontGate::Approved(files) => held.approved_only(
                                    files.clone(),
                                    in_project(&folder.below.join(name)),
                                ),
                                // A draft is kept to nowhere else: a face is read as approved
                                // there, never at the draft's own path.
                                FontGate::Draft { files, kept_at } => held.approved_only(
                                    files.clone(),
                                    kept_at
                                        .get(name)
                                        .map(|below| in_project(&below.join(name)))
                                        .unwrap_or_default(),
                                ),
                            };
                            (document, icons, held)
                        })
                    }
                };
                (name.clone(), read)
            })
            .collect();
        Ok((held, notes))
    }

    /// `step` inside `at`, a folder opened refusing a link, or `None`.
    fn folder_in(at: &OwnedFd, step: &str) -> Option<OwnedFd> {
        openat(at, step, DIRECTORY, Mode::empty()).ok()
    }

    /// Every name in the folder `at` that `keep` takes, of the first
    /// [`fonts::MOST_NAMES_SCANNED`] read, sorted.
    fn listed(at: &OwnedFd, keep: impl Fn(&str) -> bool) -> Vec<String> {
        let Ok(dir) = Dir::read_from(at) else {
            return Vec::new();
        };
        let mut names: Vec<String> = dir
            .filter_map(Result::ok)
            .take(fonts::MOST_NAMES_SCANNED)
            .filter_map(|entry| entry.file_name().to_str().ok().map(str::to_owned))
            .filter(|name| !name.starts_with('.') && keep(name))
            .collect();
        names.sort();
        names
    }

    /// What is left of the one budget a project's font files are listed under.
    struct Budget {
        files: usize,
        bytes: u64,
        folders: usize,
        examined: usize,
    }

    impl Budget {
        fn new() -> Self {
            Self {
                files: fonts::MOST_PROJECT_FONTS,
                bytes: fonts::MOST_PROJECT_FONT_BYTES,
                folders: super::MOST_FOLDERS_WALKED,
                examined: super::MOST_FILES_EXAMINED,
            }
        }
    }

    pub(super) fn plane_fonts(plane: &Path) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let Ok(root) = rustix::fs::open(
            plane,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        ) else {
            return out;
        };
        let mut levels: Vec<(String, OwnedFd)> = Vec::new();
        if let Some(themes) = folder_in(&root, super::THEMES) {
            levels.push((super::THEMES.to_owned(), themes));
        }
        if let Some(workspaces) = folder_in(&root, "workspaces") {
            for name in listed(&workspaces, crate::contain::workspace_name_ok)
                .into_iter()
                .take(super::MOST_WORKSPACES_WALKED)
            {
                if let Some(themes) = folder_in(&workspaces, &name)
                    .as_ref()
                    .and_then(|workspace| folder_in(workspace, super::THEMES))
                {
                    levels.push((format!("workspaces/{name}/{}", super::THEMES), themes));
                }
            }
        }
        let mut left = Budget::new();
        for (below, themes) in levels {
            for name in listed(&themes, crate::theme::is_folder_name)
                .into_iter()
                .take(MOST_FOLDERS)
            {
                if left.folders == 0 || left.files == 0 || left.examined == 0 {
                    return out;
                }
                left.folders -= 1;
                let Some(faces) = folder_in(&themes, &name)
                    .as_ref()
                    .and_then(|folder| folder_in(folder, "fonts"))
                else {
                    continue;
                };
                for file in listed(&faces, fonts::is_face_name)
                    .into_iter()
                    .take(fonts::MOST_FOLDER_FONTS)
                {
                    if left.files == 0 || left.examined == 0 {
                        return out;
                    }
                    left.examined -= 1;
                    // A file past what is left of the bytes is not listed, so it is never
                    // approved and never drawn.
                    if let Some((digest, bytes)) = fonts::measured(&faces, &file, left.bytes) {
                        left.files -= 1;
                        left.bytes -= bytes;
                        out.insert(
                            format!("{below}/{name}/fonts/{file}"),
                            fonts::listed_as(&digest, bytes),
                        );
                    }
                }
            }
        }
        out
    }

    /// The names in the folder `at` of a theme folder's grammar, sorted: one Windows cannot
    /// hold among them, so it is refused with why. A name starting with `.` (a `.gitkeep`) and
    /// any other that is not of the grammar are left out.
    fn names_in(at: &OwnedFd) -> rustix::io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in Dir::read_from(at)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().to_str() else {
                continue;
            };
            if name.starts_with('.') || !crate::theme::format::is_name(name) {
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
        let dir = opened::folder(themes, name, &folder)?;
        let (document, icons) = opened::theme_json(&dir, &folder)?;
        Ok((document, icons, dir))
    }
}

#[cfg(not(unix))]
mod read {
    //! **No theme folder is read off unix**: the reads open each step refusing a link, which
    //! this build cannot express, and a guard that cannot be expressed refuses rather than
    //! degrades (ADR 0031, as `crate::machine` does for the whole machine store).

    use std::collections::BTreeMap;
    use std::path::Path;

    use super::{Held, ThemesFolder};

    /// No font file is read off unix, so none is contributed.
    pub(super) fn plane_fonts(_plane: &Path) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    pub(super) fn level(folder: &ThemesFolder) -> Result<(Held, Vec<String>), String> {
        Err(format!(
            "{} is not read: theme folders are read on macOS and Linux",
            folder.path().display()
        ))
    }
}

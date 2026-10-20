// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Live reload** (spec #2313, #2324): which folders the window's theme is read from, whether a
//! change on disk is one of them, and how often a run of changes redraws.
//!
//! A theme folder is data and needs no approval (ADR 0098), so saving any file in one that is in
//! use repaints the window. The app watches what [`Watched::of`] names and asks [`Watched::decide`]
//! about each burst of changes; [`Pace`] turns a run of bursts into redraws. A redraw reads the
//! theme again the one way the window is ever drawn: [`super::resolve_levels`] over
//! [`super::Folders::of`], so the consent floors, the font approvals and every bound hold on a
//! reload as they hold at launch.
//!
//! # What is watched
//!
//! For each level in front, nearest first (the workspace's, the project's, the person's):
//!
//! - its `themes/` folder, and each theme folder in it, and each folder directly inside a theme
//!   folder (its `fonts/`, its icons);
//! - while it has no `themes/`, the folder `themes/` would be made in, for that one name;
//!
//! and the folders of each extension that contributes only themes: its own folder and the
//! folders its themes' files are in.
//!
//! **Each folder is found from its level's root through handles, refusing a link at every
//! step**, as the theme folders themselves are read: the root is the caller's, and every folder
//! below it is opened from the one above. A link, or anything that is not a folder, is not
//! watched. At most [`MOST_WATCHED`] folders are watched, the nearest level's first.
//!
//! # A change only asks
//!
//! A path a change names is used for one thing: deciding whether to read again. It is never read,
//! and nothing is opened by it. What is read is what [`super::Folders::of`] reads, from the roots
//! the caller already holds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::folders::{Folders, MOST_FOLDERS, ThemesFolder};

/// The most folders one window's theme is watched in.
pub const MOST_WATCHED: usize = 128;

/// How long a burst of changes is gathered for before it is decided on.
pub const QUIET_FOR: Duration = Duration::from_millis(100);

/// The least time between two redraws: a run of saves inside it is one redraw at its end.
pub const GAP: Duration = Duration::from_millis(400);

/// How often the folders are looked at instead, when the platform will not watch them.
pub const LOOK_EVERY: Duration = Duration::from_secs(2);

/// **The folders a window's theme is read from**, to be watched, and what a change in each means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watched {
    /// Every folder to watch, non-recursively, as its level spells it, with the root it was
    /// found from and the path below that root: what [`Look`] walks again.
    folders: BTreeMap<PathBuf, (PathBuf, PathBuf)>,
    /// The folders a change directly inside of is the theme's: each level's `themes/`, its theme
    /// folders and the folders in them. Each in every spelling ([`spellings`]).
    theme: BTreeSet<PathBuf>,
    /// The folders a level's `themes/` is not there in yet: only `themes` made in one is heard.
    awaiting: BTreeSet<PathBuf>,
    /// The folders of the extensions that contribute only themes: a change in one is a survey's.
    extensions: BTreeSet<PathBuf>,
    /// Each level's `themes/`, there or not, in every spelling: a change anywhere inside one is
    /// the theme's, so a folder made after the watch was set, and the files in it, redraw
    /// before the watch is found again.
    levels: BTreeSet<PathBuf>,
    /// How many folders were not watched, past [`MOST_WATCHED`].
    left_out: usize,
    /// For a chat's theme drafts not there yet (#2332): the nearest folder on the way to them
    /// that is, in every spelling, and the name made in it that is the next step.
    awaiting_named: BTreeSet<(PathBuf, String)>,
}

/// One extension that contributes only themes: its folder, and its themes' files inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemesOnly {
    pub dir: PathBuf,
    pub files: Vec<String>,
}

impl ThemesOnly {
    /// **Every installed extension on this machine that contributes only themes**
    /// ([`crate::extension::Manifest::themes_only`]), by the record and each one's manifest:
    /// what is watched for the themes it draws without an approval (ADR 0098 §3).
    pub fn installed(config_root: &Path, built_in: &crate::extension::BuiltIn) -> Vec<Self> {
        let loaded = crate::extension::read(config_root, built_in);
        loaded
            .registry
            .entries
            .iter()
            .filter(|(_, entry)| entry.on && entry.source == crate::extension::Source::Installed)
            .filter_map(|(id, entry)| {
                let manifest = crate::extension::manifest_at(&entry.path).ok()?;
                (manifest.id == *id && manifest.themes_only()).then(|| Self {
                    dir: entry.path.clone(),
                    files: manifest
                        .themes
                        .iter()
                        .chain(&manifest.icon_themes)
                        .map(|theme| theme.file.clone())
                        .collect(),
                })
            })
            .collect()
    }
}

/// **What a burst of changes asks for.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reload {
    /// Nothing the window draws its theme from changed.
    Nothing,
    /// The theme is read again; `survey` when an extension's themes are to be read again too.
    Redraw { survey: bool },
}

impl Reload {
    /// Both asks as one.
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Nothing, other) | (other, Self::Nothing) => other,
            (Self::Redraw { survey: one }, Self::Redraw { survey: two }) => {
                Self::Redraw { survey: one || two }
            }
        }
    }
}

impl Watched {
    /// **What to watch for the theme `folders` reads, and `extensions`' themes**: see the
    /// module's header.
    pub fn of(folders: &Folders, extensions: &[ThemesOnly]) -> Self {
        let mut out = Self::default();
        for level in [&folders.workspace, &folders.project, &folders.machine]
            .into_iter()
            .flatten()
        {
            out.level(level);
        }
        for one in extensions {
            out.extension(one);
        }
        out
    }

    /// **And the theme drafts of the chats at the window's place** (#2332, `super::drafts`):
    /// each chat's drafts folder is watched as a level is, nearest of all; while it is not
    /// there, the nearest folder on the way to it is, for the next step's name alone. So a
    /// draft's save redraws through the one path a theme is ever drawn by.
    #[must_use]
    pub fn with_drafts(mut self, drafts: &[ThemesFolder]) -> Self {
        for level in drafts {
            self.draft(level);
        }
        self
    }

    fn draft(&mut self, level: &ThemesFolder) {
        if matches!(walk::level(level), Found::Themes(_)) {
            self.level(level);
            return;
        }
        let Some((parent, next)) = walk::deepest(level) else {
            return;
        };
        self.levels.extend(spellings(&level.root, &level.below));
        if self.take(&level.root, &parent, Kind::Awaiting) {
            for one in spellings(&level.root, &parent) {
                self.awaiting_named.insert((one, next.clone()));
            }
        }
    }

    /// Every folder to watch, non-recursively.
    pub fn folders(&self) -> impl Iterator<Item = &Path> {
        self.folders.keys().map(PathBuf::as_path)
    }

    /// Each level's `themes/` a change anywhere inside of redraws, in every spelling.
    pub fn levels(&self) -> impl Iterator<Item = &Path> {
        self.levels.iter().map(PathBuf::as_path)
    }

    /// How many folders were not watched, past [`MOST_WATCHED`].
    pub fn left_out(&self) -> usize {
        self.left_out
    }

    /// Whether nothing is watched.
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// **What a burst of changes to `paths` asks for**: a redraw when one is directly inside a
    /// folder of the theme's (a name starting with `.`, an editor's scratch file, is not), or is
    /// `themes` made where a level had none; a survey too when one is in an extension's folder.
    /// `everything` is a burst the platform could not name the paths of: it redraws, and
    /// surveys when an extension is watched.
    pub fn decide<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a Path>,
        everything: bool,
    ) -> Reload {
        if self.folders.is_empty() {
            return Reload::Nothing;
        }
        if everything {
            return Reload::Redraw {
                survey: !self.extensions.is_empty(),
            };
        }
        let mut asked = Reload::Nothing;
        for path in paths {
            asked = asked.and(self.one(path));
        }
        asked
    }

    fn one(&self, path: &Path) -> Reload {
        let redraw = |survey| Reload::Redraw { survey };
        // An editor's scratch file is not a save.
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        {
            return Reload::Nothing;
        }
        // Anything inside a level's `themes/`, at any depth, or the folder itself.
        if self.levels.iter().any(|level| path.starts_with(level)) {
            return redraw(false);
        }
        // A folder watched changing itself: made, moved or gone.
        if self.theme.contains(path) {
            return redraw(false);
        }
        if self.extensions.contains(path) {
            return redraw(true);
        }
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Reload::Nothing;
        };
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            return Reload::Nothing;
        }
        if self.theme.contains(parent) {
            return redraw(false);
        }
        if self.extensions.contains(parent) {
            return redraw(true);
        }
        if name == super::folders::THEMES && self.awaiting.contains(parent) {
            return redraw(false);
        }
        if self
            .awaiting_named
            .contains(&(parent.to_path_buf(), name.into_owned()))
        {
            return redraw(false);
        }
        Reload::Nothing
    }

    /// Room for one more folder, or the folder counted as left out.
    fn room(&mut self) -> bool {
        if self.folders.len() < MOST_WATCHED {
            true
        } else {
            self.left_out += 1;
            false
        }
    }

    /// `folder`, found below `root` through handles, watched as `kind` says.
    fn take(&mut self, root: &Path, below: &Path, kind: Kind) -> bool {
        if !self.room() {
            return false;
        }
        self.folders
            .insert(root.join(below), (root.to_path_buf(), below.to_path_buf()));
        let into = match kind {
            Kind::Theme => &mut self.theme,
            Kind::Awaiting => &mut self.awaiting,
            Kind::Extension => &mut self.extensions,
        };
        into.extend(spellings(root, below));
        true
    }

    fn level(&mut self, level: &ThemesFolder) {
        match walk::level(level) {
            Found::Themes(inside) => {
                self.levels.extend(spellings(&level.root, &level.below));
                if !self.take(&level.root, &level.below, Kind::Theme) {
                    return;
                }
                for folder in inside {
                    if !self.take(&level.root, &level.below.join(&folder.name), Kind::Theme) {
                        return;
                    }
                    for sub in folder.folders {
                        let below = level.below.join(&folder.name).join(sub);
                        if !self.take(&level.root, &below, Kind::Theme) {
                            return;
                        }
                    }
                }
            }
            Found::Awaiting(parent) => {
                self.levels.extend(spellings(&level.root, &level.below));
                self.take(&level.root, &parent, Kind::Awaiting);
            }
            Found::Nothing => {}
        }
    }

    fn extension(&mut self, one: &ThemesOnly) {
        for below in walk::extension(one) {
            if !self.take(&one.dir, &below, Kind::Extension) {
                return;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Theme,
    Awaiting,
    Extension,
}

/// `below` under `root` as the level spells it and as the disk does: the platform names a
/// change by the root's real path (on macOS a temporary folder is reached through a link), so
/// both are matched. The real path is only compared with, never watched or read.
fn spellings(root: &Path, below: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.join(below)];
    if let Ok(real) = root.canonicalize()
        && real != root
    {
        out.push(real.join(below));
    }
    out
}

/// What a level's walk found.
enum Found {
    /// Its `themes/`, with its theme folders.
    Themes(Vec<Inside>),
    /// No `themes/` yet: the folder, below the root, it would be made in.
    Awaiting(PathBuf),
    /// Nothing to watch: a step below the root is missing, a link or not a folder.
    Nothing,
}

/// One theme folder, by name, and the folders directly inside it.
struct Inside {
    name: String,
    folders: Vec<String>,
}

#[cfg(unix)]
mod walk {
    //! The walks, from handles: every step opened with `O_NOFOLLOW`, as `super::super::folders`
    //! reads them.

    use std::os::fd::OwnedFd;
    use std::path::{Component, Path, PathBuf};

    use std::collections::BTreeMap;

    use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, openat, statat};
    use rustix::io::Errno;

    use super::{Found, Inside, MOST_FOLDERS, Stamp, ThemesFolder, ThemesOnly};

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    /// The most entries of one folder looked at.
    const MOST_NAMES: usize = 256;

    /// The root, which the caller trusts, as `folders::read::level` opens it.
    fn root(at: &Path) -> Option<OwnedFd> {
        rustix::fs::open(
            at,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()
    }

    pub(super) fn level(level: &ThemesFolder) -> Found {
        let Some(mut at) = root(&level.root) else {
            return Found::Nothing;
        };
        let steps: Vec<_> = level.below.components().collect();
        if steps.is_empty()
            || !steps
                .iter()
                .all(|step| matches!(step, Component::Normal(_)))
        {
            return Found::Nothing;
        }
        let last = steps.len() - 1;
        for (n, step) in steps.iter().enumerate() {
            at = match openat(&at, step.as_os_str(), DIRECTORY, Mode::empty()) {
                Ok(fd) => fd,
                Err(Errno::NOENT) if n == last => {
                    let parent: PathBuf = steps[..last].iter().collect();
                    return Found::Awaiting(parent);
                }
                Err(_) => return Found::Nothing,
            };
        }
        let mut inside = Vec::new();
        for name in folders_in(&at, crate::theme::is_folder_name)
            .into_iter()
            .take(MOST_FOLDERS)
        {
            let Ok(folder) = openat(&at, name.as_str(), DIRECTORY, Mode::empty()) else {
                continue;
            };
            let folders = folders_in(&folder, |_| true);
            inside.push(Inside { name, folders });
        }
        Found::Themes(inside)
    }

    /// The nearest folder on the way from the root to `level` that is there, below the root,
    /// opened refusing a link at every step, and the name of the next step, which is not there:
    /// `None` when the level is there whole, or a step is a link or not a folder.
    pub(super) fn deepest(level: &ThemesFolder) -> Option<(PathBuf, String)> {
        let mut at = root(&level.root)?;
        let mut below = PathBuf::new();
        for step in level.below.components() {
            let Component::Normal(name) = step else {
                return None;
            };
            match openat(&at, name, DIRECTORY, Mode::empty()) {
                Ok(fd) => at = fd,
                Err(Errno::NOENT) => return Some((below, name.to_str()?.to_owned())),
                Err(_) => return None,
            }
            below.push(name);
        }
        None
    }

    /// The extension's folder, `""`, and each folder on the way to its themes' files, below it.
    pub(super) fn extension(one: &ThemesOnly) -> Vec<PathBuf> {
        let Ok(top) = rustix::fs::open(&one.dir, DIRECTORY, Mode::empty()) else {
            return Vec::new();
        };
        let mut out = vec![PathBuf::new()];
        for file in &one.files {
            let path = Path::new(file);
            let Some(parent) = path.parent() else {
                continue;
            };
            if !parent
                .components()
                .all(|step| matches!(step, Component::Normal(_)))
            {
                continue;
            }
            let Ok(mut at) = rustix::io::dup(&top) else {
                continue;
            };
            let mut below = PathBuf::new();
            for step in parent.components() {
                at = match openat(&at, step.as_os_str(), DIRECTORY, Mode::empty()) {
                    Ok(fd) => fd,
                    Err(_) => break,
                };
                below.push(step);
                if !out.contains(&below) {
                    out.push(below.clone());
                }
            }
        }
        out
    }

    /// The names of the folders in `at` that `keep` takes, a link or a name starting with `.`
    /// left out, of the first [`MOST_NAMES`] entries, sorted.
    fn folders_in(at: &OwnedFd, keep: impl Fn(&str) -> bool) -> Vec<String> {
        let Ok(dir) = Dir::read_from(at) else {
            return Vec::new();
        };
        let mut names: Vec<String> = dir
            .filter_map(Result::ok)
            .take(MOST_NAMES)
            .filter_map(|entry| {
                let name = entry.file_name().to_str().ok()?.to_owned();
                let folder = match entry.file_type() {
                    FileType::Directory => true,
                    // A file system that does not say: asked, without following it.
                    FileType::Unknown => statat(at, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
                        .is_ok_and(|found| {
                            FileType::from_raw_mode(found.st_mode) == FileType::Directory
                        }),
                    _ => false,
                };
                folder.then_some(name)
            })
            .filter(|name| !name.starts_with('.') && keep(name))
            .collect();
        names.sort();
        names
    }

    /// `below` under `root`, opened through handles refusing a link at every step, or `None`.
    fn open_below(root_at: &Path, below: &Path) -> Option<OwnedFd> {
        let mut at = rustix::fs::open(
            root_at,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        for step in below.components() {
            let Component::Normal(step) = step else {
                return None;
            };
            at = openat(&at, step, DIRECTORY, Mode::empty()).ok()?;
        }
        Some(at)
    }

    /// Each entry of the folder `below` under `root`, of the first [`MOST_NAMES`], with its
    /// [`Stamp`], asked without following it; nothing when the folder is not there, or is
    /// reached through a link.
    pub(super) fn entries(root_at: &Path, below: &Path) -> BTreeMap<String, Stamp> {
        let mut out = BTreeMap::new();
        let Some(at) = open_below(root_at, below) else {
            return out;
        };
        let Ok(dir) = Dir::read_from(&at) else {
            return out;
        };
        for entry in dir.filter_map(Result::ok).take(MOST_NAMES) {
            let Ok(name) = entry.file_name().to_str() else {
                continue;
            };
            if name == "." || name == ".." {
                continue;
            }
            let Ok(found) = statat(&at, name, AtFlags::SYMLINK_NOFOLLOW) else {
                continue;
            };
            // A stat's size and times are signed on one platform and unsigned on another: each is
            // widened to an `i128`, which holds either exactly.
            out.insert(
                name.to_owned(),
                (
                    FileType::from_raw_mode(found.st_mode) == FileType::Directory,
                    i128::from(found.st_size),
                    i128::from(found.st_mtime),
                    i128::from(found.st_mtime_nsec),
                ),
            );
        }
        out
    }
}

#[cfg(not(unix))]
mod walk {
    //! No theme folder is read off unix (`super::super::folders`), so none is watched.

    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::{Found, Stamp, ThemesFolder, ThemesOnly};

    pub(super) fn level(_level: &ThemesFolder) -> Found {
        Found::Nothing
    }

    pub(super) fn entries(_root: &Path, _below: &Path) -> BTreeMap<String, Stamp> {
        BTreeMap::new()
    }

    pub(super) fn deepest(_level: &ThemesFolder) -> Option<(PathBuf, String)> {
        None
    }

    pub(super) fn extension(_one: &ThemesOnly) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// **How a run of changes is paced into redraws**: the first at once, then at most one each
/// [`GAP`], however many changes come in between. So a burst of saves costs one redraw, and the
/// window is drawn again within a second of the last save.
#[derive(Debug, Clone, Default)]
pub struct Pace {
    last: Option<Instant>,
    owed: Option<Reload>,
}

impl Pace {
    /// A burst that asks for `asked` was decided at `now`.
    pub fn heard(&mut self, asked: Reload) {
        if asked == Reload::Nothing {
            return;
        }
        self.owed = Some(self.owed.map_or(asked, |owed| owed.and(asked)));
    }

    /// The redraw owed, when one is due at `now`, which is taken as drawn then.
    pub fn due(&mut self, now: Instant) -> Option<Reload> {
        let owed = self.owed?;
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < GAP)
        {
            return None;
        }
        self.owed = None;
        self.last = Some(now);
        Some(owed)
    }

    /// How long until the redraw owed is due, from `now`; `None` when none is owed.
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.owed?;
        Some(self.last.map_or(Duration::ZERO, |last| {
            (last + GAP).saturating_duration_since(now)
        }))
    }
}

/// **What the watched folders hold, by name**, for the look taken when the platform will not
/// watch them: each entry's kind, size and time. Each folder is walked again from its root
/// through handles, refusing a link at every step, and each entry is asked about without
/// following it; no file is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Look(BTreeMap<PathBuf, BTreeMap<String, Stamp>>);

/// One entry, as a look sees it: whether it is a folder, its size and when it last changed.
type Stamp = (bool, i128, i128, i128);

impl Look {
    /// The folders of `watched`, looked at now.
    pub fn of(watched: &Watched) -> Self {
        Self(
            watched
                .folders
                .iter()
                .map(|(path, (root, below))| (path.clone(), walk::entries(root, below)))
                .collect(),
        )
    }

    /// The paths that differ between `self`, the look before, and `now`.
    pub fn changed(&self, now: &Self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let empty = BTreeMap::new();
        for (folder, entries) in &now.0 {
            let was = self.0.get(folder).unwrap_or(&empty);
            for (name, stamp) in entries {
                if was.get(name) != Some(stamp) {
                    out.push(folder.join(name));
                }
            }
            for name in was.keys().filter(|name| !entries.contains_key(*name)) {
                out.push(folder.join(name));
            }
        }
        out
    }
}

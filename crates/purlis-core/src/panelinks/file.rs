// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A file path in a pane's output** (#6216, spec #6208 decision 7): `src/main.rs:42:7`,
//! `~/notes.md`, `/etc/hosts:3` and a `file:///` address, read into the file on this machine it
//! names, at its line and column.
//!
//! - **Only a file that is there links.** The text is joined to the folder the chat started in
//!   (a relative path) or the home folder (`~/`), and walked one part at a time, every link on
//!   the way followed by hand ([`LocalFile::walked`]). What is left must be a regular file.
//!   Anything else is no link and says nothing on its hover ([`Resolved::Quiet`]): path-shaped
//!   words are everywhere in a harness's output.
//! - **Some folders are never looked in for a hover** ([`LocalFile::guarded`]): the folders the
//!   operating system asks the person before an app reads them (macOS's Documents, Desktop,
//!   Downloads, Mail…), and removable, network and automounted volumes, where a look can stall.
//!   A path into one is no link, unless it is in the chat's own project or folder.
//! - **Where it is read from** ([`ReadFrom`]): a branch's folder, a repo's own folder, a
//!   workspace's own files or the project's, by the names the light editor reads them by, or
//!   outside the project.
//! - **A file outside the chat's workspace asks first** ([`Opening::Ask`]): the project, for a
//!   chat at the project's root. Opening one only shows it, so a click never runs anything. One
//!   outside the project, opened on an answered ask, is kept as opened for its pane
//!   ([`super::Asks::keep_opened`]), as the file it was then.
//! - **A `file://` address is a file link** under these same rules when it names this machine
//!   (`file:///…`, `file://localhost/…`, spec Q2). One naming another host is not, and says why.
//! - **A branch's file links only when the branch offers it** (#6357,
//!   [`LocalFile::offered_by`]): what git tracks there, or doesn't ignore, which is all the light
//!   editor opens (`files::open`). One git ignores is no link, and its hover says why.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use super::{Asks, Kind, LinkKind, Opening, Pane, Resolved, Runs, Target};
use crate::files::Branch;

/// **A file a path names**, as the core resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAt {
    /// The file, absolute, with every link on the way resolved.
    pub path: PathBuf,
    /// The line the text named, from 1.
    pub line: Option<u32>,
    /// The column the text named, from 1; only with a line.
    pub column: Option<u32>,
    /// Where the window reads it from.
    pub read_from: ReadFrom,
    /// Whether it is in the chat's home ([`Home`]): one that is not asks first.
    pub in_home: bool,
    /// The path in full with its `:line:col`, as an ask shows it and *Copy link* copies it.
    shown: String,
    /// The path from the chat's home with its `:line:col`, for a file in it: what a hover's
    /// first line says (D2). The path in full for a file outside it.
    short: String,
}

impl FileAt {
    /// The file in full with its `:line:col`.
    pub fn shown(&self) -> &str {
        &self.shown
    }

    /// What a hover's first line says: the path from the chat's home for a file in it, the
    /// path in full for one outside it, each with its `:line:col`.
    pub fn short(&self) -> &str {
        &self.short
    }
}

/// **Where a file is read from**: the place the light editor reads it by, and its path there
/// (`/` between its parts), or outside the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadFrom {
    /// A branch's folder, or with no piece the repo's own folder (its clone).
    Branch {
        ws: String,
        repo: String,
        piece: Option<String>,
        path: String,
    },
    /// A workspace's own files: its folder, but its repos and their branches' folders.
    Workspace { ws: String, path: String },
    /// The project's own files: its folder, but its workspaces.
    Project { path: String },
    /// Not in the project: read only once an ask for it was answered.
    Outside,
}

/// **What a chat counts as its own**: its workspace, or the project for a chat at the project's
/// root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeIs {
    Workspace(String),
    Project,
}

/// **The folder a chat counts as its own**, and what it is. A file anywhere else asks first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    pub is: HomeIs,
    /// Its folder, links resolved.
    pub folder: PathBuf,
}

impl Home {
    /// What it is called in a sentence: the workspace's name, or "the project".
    pub fn said(&self) -> &str {
        match &self.is {
            HomeIs::Workspace(name) => name,
            HomeIs::Project => "the project",
        }
    }
}

/// Workspace `ws`'s folder in `project`, as the file kind names it.
fn workspace_folder(project: &Path, ws: &str) -> PathBuf {
    project.join("workspaces").join(ws)
}

/// **The home of `pane`'s chat**: its workspace, or the project when it works at the project's
/// root or in no workspace of it. `None` for a pane in no project.
pub fn home_of(pane: &Pane) -> Option<Home> {
    let project = std::fs::canonicalize(pane.project.as_ref()?).ok()?;
    let in_a_workspace = pane
        .folder
        .as_deref()
        .and_then(|folder| crate::active::workspace_of_tree(&project, folder));
    Some(match in_a_workspace {
        Some(ws) => Home {
            folder: workspace_folder(&project, &ws),
            is: HomeIs::Workspace(ws),
        },
        None => Home {
            is: HomeIs::Project,
            folder: project,
        },
    })
}

/// The most links one path is followed through: what the system's own resolution allows.
const MOST_LINKS: usize = 40;

/// **Whether a branch offers one of its files** (#6357): asked with the project (resolved), the
/// branch, and the file's path in its folder. `Some(true)` for a file git tracks there or does
/// not ignore, `Some(false)` for one it doesn't offer, and `None` where it could not say.
pub type Offers = dyn Fn(&Path, Branch<'_>, &str) -> Option<bool> + Send + Sync;

/// What the hover of a file its branch doesn't offer says (#6357).
pub const NOT_OFFERED: &str =
    "git ignores this file, and the editor opens only a branch's own files";

/// **A file path** (#6216): opened in the light editor, at its line, never by the system.
pub struct LocalFile {
    /// The home folder a `~/` path starts from; `None` where the environment names none.
    home: Option<PathBuf>,
    /// The folders a hover never looks in ([`LocalFile::guarded`]).
    guarded: Vec<PathBuf>,
    /// What says whether a branch offers a file ([`LocalFile::offered_by`]); with none, every
    /// branch's file links and the editor alone decides.
    offers: Option<Box<Offers>>,
}

impl LocalFile {
    /// Paths read with `home` as the home folder, looking in none of `guarded`.
    pub fn with(home: Option<PathBuf>, guarded: Vec<PathBuf>) -> Self {
        Self {
            home,
            guarded,
            offers: None,
        }
    }

    /// **A branch's file links only where `offers` says the branch offers it** (#6357), so the
    /// link and the light editor agree: the editor opens only what git tracks or doesn't ignore.
    /// The app asks its bounded reader, which starts no git in the app's process.
    #[must_use]
    pub fn offered_by(
        mut self,
        offers: impl Fn(&Path, Branch<'_>, &str) -> Option<bool> + Send + Sync + 'static,
    ) -> Self {
        self.offers = Some(Box::new(offers));
        self
    }

    /// Whether the branch `read_from` names doesn't offer the file: only `Some(false)` is.
    fn not_offered(&self, project: Option<&Path>, read_from: &ReadFrom) -> bool {
        let (Some(offers), Some(project)) = (self.offers.as_deref(), project) else {
            return false;
        };
        let ReadFrom::Branch {
            ws,
            repo,
            piece,
            path,
        } = read_from
        else {
            return false;
        };
        let branch = match piece {
            Some(piece) => Branch::piece(ws, repo, piece),
            None => Branch::repo(ws, repo),
        };
        offers(project, branch, path) == Some(false)
    }

    /// Paths read with the person's own home folder, and this system's guarded folders.
    pub fn here() -> Self {
        let home = crate::profiles::home();
        let guarded = guarded_folders(home.as_deref());
        Self::with(home, guarded)
    }

    /// Whether `path` is in a folder a hover never looks in, and not on the way to or inside
    /// one of `allowed` (the chat's project and folder, which purlis reads already).
    fn guarded(&self, path: &Path, allowed: &[PathBuf]) -> bool {
        if allowed
            .iter()
            .any(|allowed| path.starts_with(allowed) || allowed.starts_with(path))
        {
            return false;
        }
        self.guarded.iter().any(|folder| path.starts_with(folder))
    }

    /// **`path` (absolute) walked one part at a time from `/`**, each link on the way followed
    /// by hand, as the system would follow it. `None` where a part is not there, where more
    /// than [`MOST_LINKS`] links are followed, and where the walk reaches a folder it never
    /// looks in: the walk stops before it asks the system anything about it.
    fn walked(&self, path: &Path, allowed: &[PathBuf]) -> Option<PathBuf> {
        let mut todo: VecDeque<OsString> = VecDeque::new();
        push_front(&mut todo, path);
        let mut at = PathBuf::from("/");
        let mut links = 0;
        while let Some(part) = todo.pop_front() {
            if part == ".." {
                at.pop();
                continue;
            }
            let next = at.join(&part);
            if self.guarded(&next, allowed) {
                return None;
            }
            let meta = std::fs::symlink_metadata(&next).ok()?;
            if !meta.file_type().is_symlink() {
                at = next;
                continue;
            }
            links += 1;
            if links > MOST_LINKS {
                return None;
            }
            let target = std::fs::read_link(&next).ok()?;
            if target.is_absolute() {
                at = PathBuf::from("/");
            }
            push_front(&mut todo, &target);
        }
        Some(at)
    }
}

/// `path`'s parts put in front of `todo`, in order: its plain names and `..`; `/` and `.` are
/// no step.
fn push_front(todo: &mut VecDeque<OsString>, path: &Path) {
    for part in path.components().rev() {
        match part {
            Component::Normal(name) => todo.push_front(name.to_os_string()),
            Component::ParentDir => todo.push_front(OsString::from("..")),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
}

/// **The folders a hover never looks in, on this system**: on macOS, those it asks the person
/// before an app reads them, and the removable, network and automounted volumes; elsewhere, the
/// usual places removable and network volumes are mounted.
fn guarded_folders(home: Option<&Path>) -> Vec<PathBuf> {
    let mut guarded = Vec::new();
    if cfg!(target_os = "macos") {
        if let Some(home) = home {
            for asked in [
                "Desktop",
                "Documents",
                "Downloads",
                "Pictures",
                "Movies",
                "Music",
                "Library/Mail",
                "Library/Messages",
                "Library/Safari",
                "Library/Cookies",
                "Library/Calendars",
                "Library/Reminders",
                "Library/HomeKit",
                "Library/Containers",
                "Library/Group Containers",
                "Library/Mobile Documents",
                "Library/CloudStorage",
                "Library/Suggestions",
                "Library/Metadata/CoreSpotlight",
                "Library/Application Support/AddressBook",
                "Library/Application Support/CallHistoryDB",
                "Library/Application Support/com.apple.TCC",
            ] {
                guarded.push(home.join(asked));
            }
        }
        for mounted in [
            "/Volumes",
            "/net",
            "/Network",
            "/home",
            "/private/var/automount",
            "/System/Volumes/Data/Volumes",
            "/System/Volumes/Data/home",
        ] {
            guarded.push(PathBuf::from(mounted));
        }
    } else {
        for mounted in ["/media", "/mnt", "/run/media", "/net", "/misc"] {
            guarded.push(PathBuf::from(mounted));
        }
    }
    guarded
}

impl LinkKind for LocalFile {
    fn kind(&self) -> Kind {
        Kind::File
    }

    fn resolve(&self, text: &str, pane: &Pane) -> Resolved {
        if text.is_empty() || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Resolved::Quiet;
        }
        let (words, line, column) = at_line(text);
        let named = match words.strip_prefix("file:") {
            Some(_) => match on_this_machine(words) {
                Ok(path) => path,
                Err(why) => return why,
            },
            None => match self.joined(words, pane) {
                Some(path) => path,
                None => return Resolved::Quiet,
            },
        };
        // A chat on a runner, or one purlis cannot place, names its own machine's files: mapped
        // through its mounts (#6221), never read as this one's.
        if pane.runs != Runs::Here {
            return Resolved::Quiet;
        }
        let project = pane
            .project
            .as_ref()
            .and_then(|project| std::fs::canonicalize(project).ok());
        let allowed: Vec<PathBuf> = project.iter().chain(pane.folder.iter()).cloned().collect();
        let Some(path) = self.walked(&named, &allowed) else {
            return Resolved::Quiet;
        };
        if !std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) {
            return Resolved::Quiet;
        }
        let Some(read_from) = read_from(project.as_deref(), &path) else {
            return Resolved::Quiet;
        };
        if self.not_offered(project.as_deref(), &read_from) {
            return Resolved::NotALink(NOT_OFFERED.into());
        }
        let home = home_of(pane);
        let in_home = home
            .as_ref()
            .is_some_and(|home| path.starts_with(&home.folder));
        let at = at_suffix(line, column);
        let shown = format!("{}{at}", path.display());
        let short = home
            .as_ref()
            .filter(|_| in_home)
            .and_then(|home| slashed(path.strip_prefix(&home.folder).ok()?))
            .map_or_else(|| shown.clone(), |from_home| format!("{from_home}{at}"));
        Resolved::Link(Target::File(Box::new(FileAt {
            path,
            line,
            column,
            read_from,
            in_home,
            shown,
            short,
        })))
    }

    fn opening(&self, target: Target, text: &str, pane: &Pane) -> Opening {
        let Target::File(file) = &target else {
            return Opening::Refused("purlis does not open that as a file".into());
        };
        if file.in_home {
            return Opening::Open(target);
        }
        let home = home_of(pane);
        Opening::Ask {
            reason: format!(
                "outside {}",
                home.as_ref().map_or("the project", Home::said)
            ),
            shown: text.to_owned(),
            target,
            token: None,
        }
    }

    /// A file outside the project opened on an answered ask is kept as opened for the pane,
    /// as the file it is now, so the window can read it for its tab and nothing else.
    fn accepted(&self, target: &Target, pane: &Pane, asks: &Asks) {
        let Target::File(file) = target else {
            return;
        };
        if file.read_from != ReadFrom::Outside {
            return;
        }
        if let Some(identity) = crate::files::folder::absolute_identity(&file.path) {
            asks.keep_opened(pane, &file.path, identity);
        }
    }
}

impl LocalFile {
    /// `words` as a path on this machine: from `/`, from the home folder for `~/`, and from the
    /// folder the chat started in otherwise. `None` where there is no folder to start from.
    fn joined(&self, words: &str, pane: &Pane) -> Option<PathBuf> {
        if let Some(rest) = words.strip_prefix("~/") {
            return Some(self.home.as_ref()?.join(rest));
        }
        // `~name/` is another person's home, which purlis does not look up.
        if words.starts_with('~') {
            return None;
        }
        let path = Path::new(words);
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
        Some(pane.folder.as_ref()?.join(path))
    }
}

/// A path's `:line[:col]`, as it is written after it.
fn at_suffix(line: Option<u32>, column: Option<u32>) -> String {
    match (line, column) {
        (Some(line), Some(column)) => format!(":{line}:{column}"),
        (Some(line), None) => format!(":{line}"),
        _ => String::new(),
    }
}

/// `text` without its `:line[:col]`, and the line and column it named. A line of 0 is none, and
/// so is a column without one.
fn at_line(text: &str) -> (&str, Option<u32>, Option<u32>) {
    let number = |part: &str| {
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
            .filter(|n| *n > 0)
    };
    let Some((before, last)) = text.rsplit_once(':') else {
        return (text, None, None);
    };
    let Some(last) = number(last) else {
        return (text, None, None);
    };
    if let Some((path, line)) = before.rsplit_once(':')
        && let Some(line) = number(line)
        && !path.is_empty()
    {
        return (path, Some(line), Some(last));
    }
    if before.is_empty() {
        return (text, None, None);
    }
    (before, Some(last), None)
}

/// A `file:` address as the path it names on this machine (spec Q2): `file:///…` and
/// `file://localhost/…`. One naming another host is no link, and says so.
fn on_this_machine(words: &str) -> Result<PathBuf, Resolved> {
    let Ok(url) = url::Url::parse(words) else {
        return Err(Resolved::Quiet);
    };
    match url.host_str() {
        None | Some("") | Some("localhost") => {}
        Some(host) => {
            return Err(Resolved::NotALink(format!(
                "purlis opens only this machine's files, and this file: address names {host}"
            )));
        }
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Resolved::Quiet);
    }
    // `to_file_path` refuses a host it would hand on; `localhost` is this machine.
    let mut local = url.clone();
    if local.set_host(None).is_err() {
        return Err(Resolved::Quiet);
    }
    local.to_file_path().map_err(|()| Resolved::Quiet)
}

/// **Where `path` (resolved) is read from**, in `project` (resolved). `None` for a path the
/// window could not name: one whose parts are not text.
fn read_from(project: Option<&Path>, path: &Path) -> Option<ReadFrom> {
    let Some(project) = project else {
        return Some(ReadFrom::Outside);
    };
    let Ok(in_project) = path.strip_prefix(project) else {
        return Some(ReadFrom::Outside);
    };
    let under = |folder: &Path| -> Option<String> {
        let folder = std::fs::canonicalize(folder).ok()?;
        slashed(path.strip_prefix(folder).ok()?)
    };
    if let Some(at) = crate::worktree::locate(project, path) {
        let folder = crate::worktree::path_for(project, &at.workspace, &at.repo, &at.piece).ok()?;
        return Some(ReadFrom::Branch {
            path: under(&folder)?,
            ws: at.workspace,
            repo: at.repo,
            piece: Some(at.piece),
        });
    }
    if let Some((ws, repo)) = crate::worktree::locate_clone(project, path) {
        let folder = crate::repofolder::folder(project, &ws, &repo).ok()?;
        return Some(ReadFrom::Branch {
            path: under(&folder)?,
            ws,
            repo,
            piece: None,
        });
    }
    if let Some(ws) = crate::active::workspace_of_tree(project, path) {
        return Some(ReadFrom::Workspace {
            path: under(&workspace_folder(project, &ws))?,
            ws,
        });
    }
    Some(ReadFrom::Project {
        path: slashed(in_project)?,
    })
}

/// `path`'s parts with `/` between them, or `None` where one is not text or it is empty.
fn slashed(path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = path
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;

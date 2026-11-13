// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The workspace files** (CONTEXT.md; ADR 0038 and ADR 0081, both amended 2026-11-05): a
//! workspace's own folder as Explorer draws it with the workspace picked, read here (#5375), and
//! written where the files are the person's own ([`write`], #5376).
//!
//! **What they are.** Everything in `workspaces/<ws>/` but its repos and their branches' folders:
//! `repos/`, `.worktrees/`, and each folder at the top that [`crate::repofolder::holdings`] names
//! as a clone there (an old clone, ADR 0114), whether it is a repo of the workspace, shadowed by
//! one in `repos/`, or another clone. Those are never listed, opened, placed or watched here,
//! under any spelling of their names: a repo's files are its own row's, read as a branch's are.
//! The top's listing says the repos apart ([`Listing::repos`]), each with where it is, so the
//! window draws a row for each where it stands on disk.
//!
//! **A folder at the top that `holdings` does not name is the workspace's**, though it holds a
//! `.git`: one in a name no repo may take, or whose `.git` cannot be read. It is listed as the
//! person's files are, with only its `.git` left out, as `.git` is everywhere here.
//!
//! **Read as a skill's folder is** ([`super::folder`]): the workspace's folder checked by
//! [`confine::workspace_dir`], each folder listed and each file opened through a descriptor, one
//! component at a time with no link followed. A link is listed, and never followed, opened or
//! placed. Not by git: the plane's own `.gitignore` ignores every workspace's files but a LIVE
//! one's, so the offered list a branch opens by says nothing here. What purlis keeps to itself in
//! the project ([`super::kept`]) is left out here as in every place.
//!
//! **Each entry says two things of itself**: whether purlis keeps it ([`managed`]), which the
//! window marks and opens read-only, with its own view where it has one ([`view_of`]); and whether
//! it is a dotfile, which the window draws only with Show hidden. In a LIVE workspace it says a
//! third: where it is kept ([`Kept`], #5377), This machine only or Saved with the project, as
//! the plane's git decides it ([`crate::wscmd::saved::Marks`], #5552). **Save with the project**
//! ([`Files::save_with_the_project`]) is the one write here, and it writes the plane's
//! `.gitignore`, never the workspace's folder.

use std::path::{Path, PathBuf};

use super::{Kind, Opened, Placed, Refused};
use crate::repofolder::{self, Layout};
use crate::worktree::confine::{self, Outside};
pub use crate::wscmd::saved::Kept;
use crate::wscmd::saved::{self, Marks, NotSaved};
use crate::youreditor::{Checked, Editor, NotLaunched};

/// The folder of the branches' folders, never one of the workspace files.
const WORKTREES: &str = ".worktrees";

/// The folders purlis keeps in a workspace beside the reserved names: its local skills.
const ALSO_KEPT: [&str; 2] = ["skills.local", ".claude"];

/// The prefixes of the files purlis generates in a workspace (`.purlis-structure`, and the same
/// files from before the rename): its state folder's name, in each spelling it has had.
fn generated() -> impl Iterator<Item = &'static str> {
    crate::names::STATE_DIR.spellings()
}

/// One entry of a folder of the workspace files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its name in the folder, not its path.
    pub name: String,
    pub kind: Kind,
    /// Whether purlis keeps it ([`managed`]).
    pub managed: bool,
    /// Whether its name starts with a dot.
    pub hidden: bool,
    /// Where it is kept, in a LIVE workspace; `None` in a LOCAL one, whose files are all this
    /// machine's and carry no mark.
    pub kept: Option<Kept>,
    /// Why it does not open, as the sentence the window shows; `None` for a file that opens and
    /// a folder that expands.
    pub refused: Option<String>,
}

/// One repo of the workspace, as the top of its folder holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub name: String,
    /// Where it is: in `repos/`, or at the top as an old clone.
    pub layout: Layout,
}

/// One folder of the workspace files: its first [`super::SHOWN`] entries, how many more, and at
/// the top the workspace's repos.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub more: usize,
    /// The repos, sorted by name, for the top folder alone; none below it.
    pub repos: Vec<Repo>,
}

/// A managed file's own view, where the window has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Memory,
    Todos,
    Sessions,
    /// `workspace.md`, whose vision has an editor of its own.
    Vision,
}

/// Why a path of the workspace files was not read, as the sentence the window shows.
#[derive(Debug, thiserror::Error)]
pub enum NotHere {
    #[error(transparent)]
    Workspace(#[from] Outside),
    #[error(transparent)]
    Repos(#[from] repofolder::Trouble),
    #[error(transparent)]
    Files(#[from] Refused),
    #[error(
        "purlis does not read '{0}' among the workspace's files: it is in a repo. Open it from \
         that repo's row in Explorer."
    )]
    OfARepo(String),
    #[error(
        "purlis does not read '{0}' among the workspace's files: it is in a branch's folder. \
         Open it from that branch's row in Explorer."
    )]
    OfABranch(String),
    #[error(
        "purlis does not read '{0}': it is reached through a link, and purlis follows no link \
         in the workspace's files. Open the file the link points to from where it is kept."
    )]
    ThroughALink(String),
    #[error(
        "purlis keeps '{0}' itself, and shares it with the project as the workspace's LIVE \
         switch says. Save with the project is for your own files."
    )]
    KeptByPurlis(String),
    #[error(transparent)]
    NotSaved(#[from] NotSaved),
}

/// Why a write to the workspace files was not made, as the sentence the window shows.
#[derive(Debug, thiserror::Error)]
pub enum NotWritten {
    #[error(transparent)]
    Here(#[from] NotHere),
    #[error(
        "purlis does not change '{0}': it is one of the files kept by purlis in the workspace. \
         Change it from its own view, or leave it as it is."
    )]
    Managed(String),
    #[error(
        "purlis does not change '{0}': it is a link, and purlis writes through no link in the \
         workspace's files. Change it in your file manager or a terminal."
    )]
    ALink(String),
    #[error("purlis does not change the workspace's folder itself. Choose a file or folder in it.")]
    TheFolderItself,
    #[error("'{0}' is not a name a file can have here: {1}. Choose another name.")]
    BadName(String, &'static str),
    #[error("'{0}' is a name kept by purlis for itself in the workspace. Choose another name.")]
    Reserved(String),
    #[error("'{0}' is already taken in this folder. Choose another name.")]
    Taken(String),
    #[error(
        "purlis did not delete '{path}': the system would not put it in the Trash ({why}). \
         purlis never deletes a file for good, so it is still there. Delete it in Finder or a \
         terminal if you mean to."
    )]
    NoTrash { path: String, why: String },
    #[error(
        "purlis imports files, and '{0}' is not one. Pick the files themselves, or copy a folder \
         in your file manager or a terminal."
    )]
    NotAFileToImport(String),
    #[error("purlis copied in {}, and then stopped: {why}", quoted(copied))]
    PartlyImported {
        /// The paths the files copied before it stopped were given, in order.
        copied: Vec<String>,
        why: Box<NotWritten>,
    },
    #[error(
        "purlis did not delete '{0}': it, or a folder it is in, was moved or replaced while \
         purlis was checking it, so purlis left everything where it is. Look again, and try \
         again."
    )]
    Moved(String),
    #[error(
        "purlis could not {what}: {why}. Try again, or do it in your file manager or a terminal."
    )]
    Failed { what: String, why: String },
}

/// `paths`, each in quotes, as a sentence lists them: `'a'`, `'a' and 'b'`, `'a', 'b' and 'c'`.
fn quoted(paths: &[String]) -> String {
    let quoted: Vec<String> = paths.iter().map(|path| format!("'{path}'")).collect();
    match quoted.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// Whether `path`, a path of the workspace files, is one purlis keeps: a reserved workspace name
/// ([`crate::contain::workspace_keeps`]), its local skills, its generated `.claude/` and
/// `.purlis-*` files, or a folder an approved extension keeps there (`extension_folders`, from
/// [`crate::extension::events::carried`]). Judged by the path's first step, in any case, so what a
/// managed folder holds is managed too.
pub fn managed(path: &str, extension_folders: &[String]) -> bool {
    let first = path.split('/').next().unwrap_or("");
    let lower = folded(first);
    crate::contain::workspace_keeps(&lower)
        || ALSO_KEPT.contains(&lower.as_str())
        || generated().any(|prefix| lower == prefix || lower.starts_with(&format!("{prefix}-")))
        || extension_folders
            .iter()
            .any(|folder| folded(folder) == lower)
}

/// **`name` as a disk that ignores case and Unicode's forms reads it** (#5513's review): its
/// decomposed form, each letter's case folded through its capital, so `ſ` (U+017F) is `s` and
/// the Kelvin sign (U+212A) is `k`, as APFS takes them. Rust's lowercase alone keeps both, so a
/// name such as `repoſ` would pass for the person's and reach `repos/` on the disk. Every name
/// purlis keeps, and every repo's at the top, is told by this.
pub(crate) fn folded(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    let decomposed: String = name.nfd().collect();
    decomposed.to_uppercase().to_lowercase().nfd().collect()
}

/// The view a managed file at `path` has of its own, if the window has one.
pub fn view_of(path: &str) -> Option<View> {
    let mut steps = path.split('/');
    let first = steps.next().unwrap_or("").to_lowercase();
    let more = steps.next().is_some();
    match first.as_str() {
        "workspace.md" if !more => Some(View::Vision),
        "memory" => Some(View::Memory),
        "todos" => Some(View::Todos),
        "sessions" => Some(View::Sessions),
        _ => None,
    }
}

/// **One workspace's files**, its folder checked and its repos found once: what every read of
/// them goes through.
#[derive(Debug)]
pub struct Files {
    /// The workspace's folder, as the disk spells it.
    dir: PathBuf,
    /// The names at the top that are no workspace file, lowercased: `repos`, `.worktrees` and
    /// every clone there that `holdings` names.
    outside: Vec<String>,
    repos: Vec<Repo>,
    kept: super::Kept,
    /// The project's folder and the workspace's name, which Save with the project writes by.
    plane: PathBuf,
    ws: String,
    /// Where each file is kept, for a LIVE workspace alone.
    marks: Option<Marks>,
}

impl Files {
    /// Workspace `ws`'s files in the project at `plane`. Refused for a name that is no
    /// workspace's, a folder reached through a link or not there, and one whose repos cannot be
    /// read: without them a repo's folder could not be told from the person's.
    pub fn of(plane: &Path, ws: &str) -> Result<Self, NotHere> {
        let checked = confine::workspace_dir(plane, ws)?;
        let dir = std::fs::canonicalize(&checked).map_err(|e| Refused::Unreadable {
            what: format!("workspace '{ws}'"),
            why: e.to_string(),
        })?;
        let holdings = repofolder::holdings(plane, ws)?;
        let mut outside = vec![repofolder::REPOS.to_string(), WORKTREES.to_string()];
        let at_root = holdings
            .repos
            .iter()
            .chain(&holdings.shadowed)
            .chain(&holdings.others)
            .filter(|row| row.layout == Layout::Root);
        outside.extend(at_root.map(|row| folded(&row.repo.name)));
        // A folder with a `.git` whose name purlis takes for no repo's (`My Tool`), or whose
        // `.git` could not be read, is still a repo to git, and never the person's own file.
        // `refused` also names a link, which is listed as one, so only one holding a `.git`.
        let refused = holdings
            .refused
            .iter()
            .filter(|said| dir.join(&said.name).join(".git").symlink_metadata().is_ok());
        outside.extend(
            refused
                .chain(&holdings.unreadable)
                .filter(|said| said.layout == Layout::Root && !said.name.is_empty())
                .map(|said| folded(&said.name)),
        );
        let repos = holdings
            .repos
            .iter()
            .map(|row| Repo {
                name: row.repo.name.clone(),
                layout: row.layout,
            })
            .collect();
        let kept = super::Kept::in_a_branch(plane, &dir, &crate::sandbox::Machine::this());
        kept.whole()?;
        Ok(Self {
            dir,
            outside,
            repos,
            kept,
            plane: plane.to_path_buf(),
            ws: ws.to_string(),
            marks: Marks::of(plane, ws),
        })
    }

    /// The workspace's folder, as the disk spells it.
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// `path`, a path of the workspace files as the window spells it, judged by its spelling:
    /// inside the folder, not git's, not a repo's or a branch's, and nothing purlis keeps to
    /// itself. `""` is the folder itself.
    fn judged(&self, path: &str) -> Result<String, NotHere> {
        if path.is_empty() {
            return Ok(String::new());
        }
        let spelled = super::spelled_inside(path)?;
        let first = spelled.split('/').next().unwrap_or("");
        if self.is_outside(first) {
            return Err(if folded(first) == WORKTREES {
                NotHere::OfABranch(path.to_string())
            } else {
                NotHere::OfARepo(path.to_string())
            });
        }
        self.kept.refuse(Path::new(&spelled), path)?;
        Ok(spelled)
    }

    /// Whether `name`, at the top, is a repo's or the branches' folders.
    fn is_outside(&self, name: &str) -> bool {
        self.outside.contains(&folded(name))
    }

    /// `relative`, judged, resolved as the disk spells it and required to be that very path: no
    /// link on its way, the leaf included.
    fn resolved(&self, relative: &str, path: &str) -> Result<PathBuf, NotHere> {
        let absolute = self.dir.join(relative);
        match std::fs::canonicalize(&absolute) {
            Ok(resolved) if resolved == absolute => Ok(absolute),
            Ok(_) => Err(NotHere::ThroughALink(path.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(Refused::NotThere(path.to_string()).into())
            }
            Err(e) => Err(Refused::Unreadable {
                what: path.to_string(),
                why: e.to_string(),
            }
            .into()),
        }
    }

    /// **One folder, one level deep** (`""` for the top): folders first, each kind in the order a
    /// person reads names, the first [`super::SHOWN`]. Each entry says whether purlis keeps it,
    /// by `extension_folders` too, and whether it is hidden; a link and anything that is not a
    /// file are listed with why they do not open. At the top, the repos are said apart.
    pub fn tree(&self, folder: &str, extension_folders: &[String]) -> Result<Listing, NotHere> {
        let relative = self.judged(folder)?;
        let mut listed = super::listing(&self.dir, Path::new(&relative)).map_err(|e| {
            if super::swapped(&e) {
                NotHere::ThroughALink(folder.to_string())
            } else if e.kind() == std::io::ErrorKind::NotFound {
                Refused::NotThere(folder.to_string()).into()
            } else {
                Refused::Unreadable {
                    what: folder.to_string(),
                    why: e.to_string(),
                }
                .into()
            }
        })?;
        let top = relative.is_empty();
        listed.retain(|one| {
            let name = one.name.to_string_lossy();
            !super::gits(Path::new(&one.name))
                && !(top && self.is_outside(&name))
                && !self.kept.private_entry(Path::new(&relative), &one.name)
        });
        listed.sort_by(|a, b| {
            (a.kind != Kind::Folder)
                .cmp(&(b.kind != Kind::Folder))
                .then_with(|| super::natural(&a.name.to_string_lossy(), &b.name.to_string_lossy()))
        });
        let more = listed.len().saturating_sub(super::SHOWN);
        listed.truncate(super::SHOWN);
        let paths: Vec<String> = listed
            .iter()
            .map(|one| {
                let name = one.name.to_string_lossy();
                if top {
                    name.into_owned()
                } else {
                    format!("{relative}/{name}")
                }
            })
            .collect();
        // One question to git for the whole folder; no answer marks none.
        let kept = self.marks.as_ref().and_then(|marks| {
            marks.of_paths(&paths.iter().map(String::as_str).collect::<Vec<_>>())
        });
        let entries = listed
            .into_iter()
            .zip(paths)
            .enumerate()
            .map(|(at, (one, path))| {
                let name = one.name.to_string_lossy().into_owned();
                let refused = if one.name.to_str().is_none() {
                    Some("purlis opens only a file whose name is UTF-8".to_string())
                } else {
                    match one.kind {
                        Kind::Link => Some(
                            "a link, and purlis follows no link in the workspace's files"
                                .to_string(),
                        ),
                        Kind::File if !one.file => {
                            Some("not a file: only files open here".to_string())
                        }
                        _ => None,
                    }
                };
                Entry {
                    managed: managed(&path, extension_folders),
                    hidden: name.starts_with('.'),
                    kept: kept.as_ref().map(|kept| kept[at]),
                    name,
                    kind: one.kind,
                    refused,
                }
            })
            .collect();
        Ok(Listing {
            entries,
            more,
            repos: if top { self.repos.clone() } else { Vec::new() },
        })
    }

    /// **Save one entry with the project** (#5377): a file or folder at the top of a LIVE
    /// workspace's folder, which purlis does not keep, gets one line in the LIVE block. Only the
    /// person's press calls this. `true` when the line was added, `false` when the entry was
    /// saved already. Refused for a path below the top, one purlis keeps, a link, one not there,
    /// and in a workspace that is not LIVE.
    pub fn save_with_the_project(
        &self,
        path: &str,
        extension_folders: &[String],
    ) -> Result<bool, NotHere> {
        let spelled = self.judged(path)?;
        if !saved::one_entry(&spelled) {
            return Err(NotSaved::NotOneEntry(path.to_string()).into());
        }
        if managed(&spelled, extension_folders) {
            return Err(NotHere::KeptByPurlis(path.to_string()));
        }
        self.resolved(&spelled, path)?;
        Ok(saved::add(&self.plane, &self.ws, &spelled)?)
    }

    /// **One file**, by its path: refused as [`Self::tree`] refuses a folder, and when a link is
    /// on its way or is the file itself, when it is not a regular file, and when it is a second
    /// name of a file purlis keeps to itself.
    pub fn open(&self, path: &str) -> Result<Opened, NotHere> {
        let spelled = self.judged(path)?;
        if spelled.is_empty() {
            return Err(Refused::NotAFile(path.to_string()).into());
        }
        let held = super::hold_folder(&self.dir, &self.dir).map_err(|e| Refused::Unreadable {
            what: "the workspace's folder".to_string(),
            why: e.to_string(),
        })?;
        let file = super::open_inside(&held, Path::new(&spelled)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                NotHere::from(Refused::NotThere(path.to_string()))
            } else {
                NotHere::ThroughALink(path.to_string())
            }
        })?;
        let meta = file.metadata().map_err(|e| Refused::Unreadable {
            what: path.to_string(),
            why: e.to_string(),
        })?;
        if !meta.is_file() {
            return Err(Refused::NotAFile(path.to_string()).into());
        }
        if self.kept.holds_a_name_of(&file) {
            return Err(Refused::KeptPrivate(path.to_string()).into());
        }
        Ok(super::read_opened(file, meta.len(), path)?)
    }

    /// **One file or folder placed on disk** (Copy path, Reveal, a shell tab, a chat's
    /// reference), or with `""` the workspace's folder itself: its absolute path, which no link
    /// is on.
    pub fn place(&self, path: &str) -> Result<Placed, NotHere> {
        let relative = self.judged(path)?;
        let absolute = if relative.is_empty() {
            self.dir.clone()
        } else {
            self.resolved(&relative, path)?
        };
        let folder = absolute.is_dir();
        Ok(Placed {
            absolute,
            relative,
            folder,
        })
    }

    /// **One file, folder or link by its own path** (Copy relative path), the leaf followed
    /// nowhere: its folder is resolved through no link, and the leaf is only looked at.
    pub fn named(&self, path: &str) -> Result<String, NotHere> {
        let spelled = self.judged(path)?;
        if spelled.is_empty() {
            return Err(Refused::NotInPiece(path.to_string()).into());
        }
        let (parent, leaf) = spelled.rsplit_once('/').unwrap_or(("", spelled.as_str()));
        let folder = if parent.is_empty() {
            self.dir.clone()
        } else {
            self.resolved(parent, path)?
        };
        match std::fs::symlink_metadata(folder.join(leaf)) {
            Ok(_) => Ok(spelled),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(Refused::NotThere(path.to_string()).into())
            }
            Err(e) => Err(Refused::Unreadable {
                what: path.to_string(),
                why: e.to_string(),
            }
            .into()),
        }
    }

    /// **The folders a window's Explorer has open, for its watch**: each resolved as
    /// [`Self::tree`] would list it, or refused.
    pub fn folders(&self, named: &[&str]) -> Vec<Result<PathBuf, NotHere>> {
        named
            .iter()
            .map(|folder| self.listed_folder(folder))
            .collect()
    }

    /// **One folder of the workspace files**, resolved as [`Self::tree`] would list it: where a
    /// shell tab starts, and what a watch watches. Refused for a file.
    pub fn listed_folder(&self, folder: &str) -> Result<PathBuf, NotHere> {
        let placed = self.place(folder)?;
        if placed.folder {
            Ok(placed.absolute)
        } else {
            Err(Refused::NotAFolder(folder.to_string()).into())
        }
    }

    /// **One file handed to your editor** at `line` (RC-20): placed as [`Self::place`] places
    /// it, a regular file, and no second name of a file purlis keeps to itself. Held as it was
    /// checked ([`Checked`], #3185): the app launches only through [`Checked::launch`].
    pub fn in_your_editor(
        &self,
        path: &str,
        line: u32,
        editor: Editor,
        var: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Checked, NotHere> {
        if self.judged(path)?.is_empty() {
            return Err(Refused::NotAFile(path.to_string()).into());
        }
        let placed = self.place(path)?;
        if !placed.absolute.is_file() {
            return Err(Refused::NotAFile(path.to_string()).into());
        }
        // Held by what it is on disk, so what is launched is still this file (#3185).
        let checked = match Checked::new(&self.dir, &placed.absolute, editor, line, var) {
            Ok(checked) => checked,
            Err(NotLaunched::NotHeld(_)) => return Err(NotHere::ThroughALink(path.to_string())),
            Err(other) => return Err(Refused::from(other).into()),
        };
        if self.kept.holds_a_name_of(checked.held()) {
            return Err(Refused::KeptPrivate(path.to_string()).into());
        }
        Ok(checked)
    }

    /// **The files found by name** (the Explorer's filter): every regular file the tree shows
    /// without Show hidden, as paths, the first `cap` in reading order, and how many more there
    /// were. No link, and nothing inside a folder whose name starts with a dot.
    pub fn list(&self, cap: usize) -> (Vec<String>, usize) {
        let mut found = Vec::new();
        let mut more = 0;
        let mut folders = vec![String::new()];
        while let Some(folder) = folders.pop() {
            let Ok(listing) = self.tree(&folder, &[]) else {
                continue;
            };
            let mut inside = Vec::new();
            for entry in listing.entries {
                if entry.hidden || entry.refused.is_some() {
                    continue;
                }
                let path = if folder.is_empty() {
                    entry.name
                } else {
                    format!("{folder}/{}", entry.name)
                };
                match entry.kind {
                    Kind::Folder => inside.push(path),
                    Kind::File if found.len() < cap => found.push(path),
                    Kind::File => more += 1,
                    Kind::Link => {}
                }
            }
            // Depth first, in reading order: the stack takes them back to front.
            folders.extend(inside.into_iter().rev());
        }
        found.sort();
        (found, more)
    }
}

#[path = "workspace/write.rs"]
mod write;
pub use write::{SystemTrash, Trash};

#[cfg(test)]
#[path = "workspace/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace/kept_tests.rs"]
mod kept_tests;

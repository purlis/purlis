// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's own workspace files, written** (ADR 0081, amended 2026-11-05; #5376): new
//! file, new folder, rename, delete to the system's Trash, and import.
//!
//! **Only where the files are the person's.** Every write is judged as a read is
//! ([`Files::judged`]): inside the workspace's folder by its spelling, never in `repos/`, a clone
//! at the top or `.worktrees/`, and nothing purlis keeps to itself in the project. Then it is
//! refused on what purlis keeps in the workspace ([`super::managed`]), and on a link: the folder
//! a write lands in is resolved through no link ([`Files::resolved`]), and a link itself is never
//! renamed, deleted or written through. A name purlis keeps for itself at the top is never given
//! (`workspace.md`, `memory`, `repos` and the rest).
//!
//! **Nothing is overwritten.** A new file or folder is made only where nothing is
//! (`create_new`, `mkdir`); a rename never replaces what has the new name (`RENAME_NOREPLACE`,
//! or `RENAME_EXCL` on macOS); an import whose name is taken gets a ` (2)` suffix, as the
//! system's file manager does.
//!
//! **Nothing is deleted for good.** Delete hands the entry to the system's Trash ([`Trash`]),
//! and when there is none, or it refuses, the entry stays where it is and the refusal says so.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{super::Refused, Files, NotHere, NotWritten, managed};

/// **Where a deleted entry goes**: the system's Trash in the app ([`SystemTrash`]), and a stand-in
/// in a test, which cannot touch the person's own Trash.
pub trait Trash {
    /// Put the file or folder at `path` in the Trash, or say why not. Never deletes it for good.
    fn put(&self, path: &Path) -> Result<(), String>;
}

/// **The system's Trash**: the Finder's on macOS (through `NSFileManager`, which needs no
/// Apple Events), the freedesktop.org trash on Linux. Anywhere else there is none purlis can
/// trust not to delete for good, so every delete is refused.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTrash;

impl Trash for SystemTrash {
    #[cfg(target_os = "macos")]
    fn put(&self, path: &Path) -> Result<(), String> {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path).map_err(|e| e.to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn put(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| e.to_string())
    }

    #[cfg(not(unix))]
    fn put(&self, _path: &Path) -> Result<(), String> {
        Err("purlis knows no Trash on this system".to_string())
    }
}

/// The longest name a file may have, in bytes, on the file systems purlis runs on.
const LONGEST_NAME: usize = 255;

/// `name`, a name the person typed for a new file, folder or rename, or why it cannot be one.
fn checked_name(name: &str) -> Result<&str, NotWritten> {
    let bad = |why| Err(NotWritten::BadName(name.to_string(), why));
    if name.trim().is_empty() {
        return bad("it is empty");
    }
    if name.contains(['/', '\\']) {
        return bad("it has a slash in it, and a name is one step of a path");
    }
    if name.contains('\0') {
        return bad("it has a null character in it");
    }
    if name == "." || name == ".." {
        return bad("it names a folder, not a file in it");
    }
    if name.eq_ignore_ascii_case(".git") {
        return bad("git keeps that name for itself");
    }
    if name.len() > LONGEST_NAME {
        return bad("it is longer than 255 bytes");
    }
    Ok(name)
}

/// `name` in the folder at `relative` (`""` for the top).
fn joined(relative: &str, name: &str) -> String {
    if relative.is_empty() {
        name.to_string()
    } else {
        format!("{relative}/{name}")
    }
}

/// `name` with ` (n)` before its extension: `notes (2).md`, `Makefile (2)`, `.env (2)`.
fn suffixed(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem} ({n}).{extension}"),
        _ => format!("{name} ({n})"),
    }
}

/// Whether two paths are one file: a rename that changes only a name's case, on a disk that
/// does not tell cases apart.
fn same_file(one: &Path, other: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (
            std::fs::symlink_metadata(one),
            std::fs::symlink_metadata(other),
        ) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        one.to_string_lossy()
            .eq_ignore_ascii_case(&other.to_string_lossy())
    }
}

/// The name, in `folder`, of the entry `step` reaches there: the one that is that very file. The
/// step itself when the disk spells it so; nothing when it reaches nothing, or its name is not
/// UTF-8.
fn spelling_in(folder: &Path, step: &str) -> Option<String> {
    let reached = std::fs::symlink_metadata(folder.join(step)).ok()?;
    let entries: Vec<std::fs::DirEntry> = std::fs::read_dir(folder).ok()?.flatten().collect();
    if entries.iter().any(|one| one.file_name() == step) {
        return Some(step.to_string());
    }
    entries
        .into_iter()
        .find(|one| std::fs::symlink_metadata(one.path()).is_ok_and(|meta| same(&meta, &reached)))
        .and_then(|one| one.file_name().into_string().ok())
}

/// Whether two entries' metadata are of one file.
fn same(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        one.dev() == other.dev() && one.ino() == other.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (one, other);
        false
    }
}

/// `from` renamed to `to`, refused when something already has the name `to`, with no moment in
/// which the other is replaced.
fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(std::io::Error::from)
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(to).is_ok() {
            return Err(ErrorKind::AlreadyExists.into());
        }
        std::fs::rename(from, to)
    }
}

impl Files {
    /// **The folder a new entry goes in**, `folder` (`""` for the top): judged as a read is,
    /// none that purlis keeps, and resolved through no link. Its absolute path and its spelling.
    fn folder_to_write(
        &self,
        folder: &str,
        kept: &[String],
    ) -> Result<(PathBuf, String), NotWritten> {
        let relative = self.judged(&self.on_disk(&self.judged(folder)?))?;
        if relative.is_empty() {
            return Ok((self.dir.clone(), relative));
        }
        if managed(&relative, kept) {
            return Err(NotWritten::Managed(folder.to_string()));
        }
        let absolute = self.resolved(&relative, folder)?;
        if !absolute.is_dir() {
            return Err(NotHere::from(Refused::NotAFolder(folder.to_string())).into());
        }
        Ok((absolute, relative))
    }

    /// **`relative` as the disk spells it**, step by step: each step that is there is the name
    /// of the entry in its folder that is that very file (by device and inode), so a spelling the
    /// disk folds (`repoſ` on APFS) is read as the name it reaches (`repos`). A step that is not
    /// there, and every step after it, is kept as it was asked.
    fn on_disk(&self, relative: &str) -> String {
        let mut folder = self.dir.clone();
        let mut spelled: Vec<String> = Vec::new();
        let mut steps = relative.split('/').filter(|step| !step.is_empty());
        for step in steps.by_ref() {
            match spelling_in(&folder, step) {
                Some(name) => {
                    folder.push(&name);
                    spelled.push(name);
                }
                None => {
                    spelled.push(step.to_string());
                    break;
                }
            }
        }
        spelled.extend(steps.map(str::to_string));
        spelled.join("/")
    }

    /// **A new name in the folder at `relative`**: a name a file may have, never one that is a
    /// repo's, the branches' folders' or purlis's own at the top, or anything purlis keeps.
    fn name_to_give(
        &self,
        relative: &str,
        name: &str,
        kept: &[String],
    ) -> Result<String, NotWritten> {
        let name = checked_name(name)?;
        let path = joined(relative, name);
        if (relative.is_empty() && self.is_outside(name)) || managed(&path, kept) {
            return Err(NotWritten::Reserved(name.to_string()));
        }
        self.judged(&path)?;
        Ok(path)
    }

    /// **An entry to rename or delete**, `path`: judged, none that purlis keeps, not the folder
    /// itself, and no link, its own leaf included. Its absolute path, its folder's spelling and
    /// that folder resolved.
    fn entry_to_write(
        &self,
        path: &str,
        kept: &[String],
    ) -> Result<(PathBuf, String, PathBuf), NotWritten> {
        // Judged as the disk spells it, so a spelling the disk folds onto a repo's or a name
        // purlis keeps is judged as that name, whatever folding the disk does.
        let relative = self.judged(&self.on_disk(&self.judged(path)?))?;
        if relative.is_empty() {
            return Err(NotWritten::TheFolderItself);
        }
        if managed(&relative, kept) {
            return Err(NotWritten::Managed(path.to_string()));
        }
        let parent = relative.rsplit_once('/').map_or("", |(parent, _)| parent);
        let folder = if parent.is_empty() {
            self.dir.clone()
        } else {
            self.resolved(parent, path)?
        };
        let leaf = relative.rsplit('/').next().unwrap_or(&relative);
        let absolute = folder.join(leaf);
        match std::fs::symlink_metadata(&absolute) {
            Ok(meta) if meta.file_type().is_symlink() => Err(NotWritten::ALink(path.to_string())),
            Ok(_) => Ok((absolute, parent.to_string(), folder)),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                Err(NotHere::from(Refused::NotThere(path.to_string())).into())
            }
            Err(e) => Err(NotWritten::Failed {
                what: format!("read '{path}'"),
                why: e.to_string(),
            }),
        }
    }

    /// **New file** `name` in `folder` (`""` for the top), empty: its path, or why not. Never
    /// over anything already there.
    pub fn new_file(
        &self,
        folder: &str,
        name: &str,
        kept: &[String],
    ) -> Result<String, NotWritten> {
        let (into, relative) = self.folder_to_write(folder, kept)?;
        let path = self.name_to_give(&relative, name, kept)?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(into.join(name))
            .map_err(|e| made(e, name, "make the file"))?;
        Ok(path)
    }

    /// **New folder** `name` in `folder` (`""` for the top), empty: its path, or why not.
    pub fn new_folder(
        &self,
        folder: &str,
        name: &str,
        kept: &[String],
    ) -> Result<String, NotWritten> {
        let (into, relative) = self.folder_to_write(folder, kept)?;
        let path = self.name_to_give(&relative, name, kept)?;
        std::fs::create_dir(into.join(name)).map_err(|e| made(e, name, "make the folder"))?;
        Ok(path)
    }

    /// **Rename** the file or folder at `path` to `name`, in the same folder: its new path, or
    /// why not. Never over anything that has the name already; a change of case alone is a
    /// rename too.
    pub fn rename(&self, path: &str, name: &str, kept: &[String]) -> Result<String, NotWritten> {
        let (from, parent, folder) = self.entry_to_write(path, kept)?;
        let renamed = self.name_to_give(&parent, name, kept)?;
        let to = folder.join(name);
        let result = if same_file(&from, &to) {
            // Only its case changes, on a disk that does not tell cases apart: the other name
            // is this very entry, so nothing is replaced.
            std::fs::rename(&from, &to)
        } else {
            rename_no_replace(&from, &to)
        };
        result.map_err(|e| made(e, name, &format!("rename '{path}'")))?;
        Ok(renamed)
    }

    /// **Delete** the file or folder at `path`: handed to `trash`, and refused, the entry left
    /// where it is, when the Trash will not take it. Never deleted for good.
    pub fn trash(&self, path: &str, kept: &[String], trash: &dyn Trash) -> Result<(), NotWritten> {
        let (absolute, _, _) = self.entry_to_write(path, kept)?;
        trash.put(&absolute).map_err(|why| NotWritten::NoTrash {
            path: path.to_string(),
            why,
        })
    }

    /// **Import** each of `sources`, files the person picked anywhere on disk, into `folder`
    /// (`""` for the top), copied: the path each was given, in order. A name already taken, or
    /// kept for purlis, gets the first free ` (n)` suffix, from ` (2)`.
    pub fn import(
        &self,
        folder: &str,
        sources: &[PathBuf],
        kept: &[String],
    ) -> Result<Vec<String>, NotWritten> {
        let (into, relative) = self.folder_to_write(folder, kept)?;
        let mut imported = Vec::with_capacity(sources.len());
        for source in sources {
            let shown = source.display().to_string();
            let mut from = match std::fs::File::open(source) {
                Ok(file) if file.metadata().is_ok_and(|meta| meta.is_file()) => file,
                Ok(_) => return Err(NotWritten::NotAFileToImport(shown)),
                Err(e) => {
                    return Err(NotWritten::Failed {
                        what: format!("read '{shown}'"),
                        why: e.to_string(),
                    });
                }
            };
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| NotWritten::NotAFileToImport(shown.clone()))?;
            checked_name(name)?;
            imported.push(self.copied_in(&mut from, &into, &relative, name, kept)?);
        }
        Ok(imported)
    }

    /// `from` copied into the folder `into` (spelled `relative`) as `name`, or the first free
    /// ` (n)` of it: the path it was given.
    fn copied_in(
        &self,
        from: &mut std::fs::File,
        into: &Path,
        relative: &str,
        name: &str,
        kept: &[String],
    ) -> Result<String, NotWritten> {
        for n in 1..=u32::from(u16::MAX) {
            let candidate = if n == 1 {
                name.to_string()
            } else {
                suffixed(name, n)
            };
            let Ok(path) = self.name_to_give(relative, &candidate, kept) else {
                continue;
            };
            let mut to = match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(into.join(&candidate))
            {
                Ok(to) => to,
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(made(e, &candidate, "copy the file in")),
            };
            if let Err(e) = std::io::copy(from, &mut to) {
                drop(to);
                // Only the copy this import just made, never anything that was there.
                let _ = std::fs::remove_file(into.join(&candidate));
                return Err(made(e, &candidate, "copy the file in"));
            }
            return Ok(path);
        }
        Err(NotWritten::Taken(name.to_string()))
    }
}

/// What a write's failure says: a name already taken, or the system's own reason.
fn made(e: std::io::Error, name: &str, what: &str) -> NotWritten {
    if e.kind() == ErrorKind::AlreadyExists {
        NotWritten::Taken(name.to_string())
    } else {
        NotWritten::Failed {
            what: what.to_string(),
            why: e.to_string(),
        }
    }
}

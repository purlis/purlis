// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's own workspace files, written** (ADR 0081, amended 2026-11-05; #5376): new
//! file, new folder, rename, delete to the system's Trash, and import.
//!
//! **Only where the files are the person's.** Every write is judged as a read is
//! ([`Files::judged`]): inside the workspace's folder by its spelling, never in `repos/`, a clone
//! at the top or `.worktrees/`, and nothing purlis keeps to itself in the project. Then it is
//! refused on what purlis keeps in the workspace ([`super::managed`]), and on a link: a link
//! itself is never renamed, deleted or written through. A name purlis keeps for itself at the top
//! is never given (`workspace.md`, `memory`, `repos` and the rest).
//!
//! **The folder checked is the folder written** (#5551). The folder a write lands in is opened
//! from the workspace's folder one step at a time through no link (`O_NOFOLLOW`), and held
//! ([`crate::held::Store`]): the write names its entry relative to that descriptor (`openat`,
//! `mkdirat`, `renameat`), never by a path looked up again. A chat that swaps a folder for a link
//! after the check changes nothing about where the write lands. The system's Trash alone takes a
//! path, so a delete walks the path again just before it hands it over, and is refused when it
//! no longer reaches the very folder and entry checked; what the Trash does with the path after
//! that is the system's, and the one window purlis cannot hold shut without a Trash of its own.
//! On a system with no descriptors to hold (none purlis ships on), every write is refused.
//!
//! **Nothing is overwritten.** A new file or folder is made only where nothing is
//! (`create_new`, `mkdir`); a rename never replaces what has the new name (`RENAME_NOREPLACE`,
//! or `RENAME_EXCL` on macOS); an import whose name is taken gets a ` (2)` suffix, as the
//! system's file manager does.
//!
//! **Nothing is deleted for good.** Delete hands the entry to the system's Trash ([`Trash`]),
//! and when there is none, or it refuses, the entry stays where it is and the refusal says so.

use std::path::{Path, PathBuf};

#[cfg(unix)]
use crate::held::Store as Held;

use super::{super::Refused, Files, NotHere, NotWritten, folded, managed};

#[cfg(test)]
thread_local! {
    /// What a test does between a write's check and the write itself: the race the held folder
    /// answers (#5551).
    static AFTER_THE_CHECK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

/// The moment between a write's check and the write: a test's fault, if it set one.
fn after_the_check() {
    #[cfg(test)]
    AFTER_THE_CHECK.with(|fault| {
        let fault = fault.borrow_mut().take();
        if let Some(fault) = fault {
            fault();
        }
    });
}

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

    /// **Only what the Trash can take whole** (#5551). Where the volume's own trash cannot be
    /// written, `trash` falls back to the home Trash, and across disks it copies rather than
    /// moves: the copy loses permissions, extended attributes and hard links, and a FIFO in a
    /// folder hangs it. So an entry on another disk than the home Trash is refused, and left
    /// where it is.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn put(&self, path: &Path) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;
        let entry = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if home_trash_disk().is_some_and(|disk| disk != entry.dev()) {
            return Err(
                "it is on another disk than your Trash, and across disks the system \
                        copies an entry into the Trash rather than moving it, losing its \
                        permissions and links"
                    .to_string(),
            );
        }
        trash::delete(path).map_err(|e| e.to_string())
    }

    #[cfg(not(unix))]
    fn put(&self, _path: &Path) -> Result<(), String> {
        Err("purlis knows no Trash on this system".to_string())
    }
}

/// The disk the home Trash is on, as the freedesktop.org trash names it (`$XDG_DATA_HOME/Trash`,
/// or `~/.local/share/Trash`): that of the nearest folder of its path that is there.
#[cfg(all(unix, not(target_os = "macos")))]
fn home_trash_disk() -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    let set = |name| std::env::var_os(name).filter(|value| !value.is_empty());
    let data = set("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| set("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    data.join("Trash")
        .ancestors()
        .find_map(|folder| std::fs::metadata(folder).ok())
        .map(|meta| meta.dev())
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

/// An entry to rename or delete, checked: the folder it is in, held, that folder's spelling, and
/// the entry's own name there and which file it is.
#[cfg(unix)]
struct Checked {
    folder: Held,
    parent: String,
    leaf: String,
    entry: crate::held::Entry,
}

#[cfg(unix)]
impl Files {
    /// **`relative`, a folder of the workspace's files, held open** (`""` for the top): opened
    /// from the workspace's folder one step at a time through no link, so what is held is the
    /// very folder that had each name as it was opened. `path` is how the window named it.
    fn held(&self, relative: &str, path: &str) -> Result<Held, NotWritten> {
        let unreadable = |why: String| -> NotWritten {
            NotHere::from(Refused::Unreadable {
                what: path.to_string(),
                why,
            })
            .into()
        };
        let mut folder = Held::project(&self.dir)
            .map_err(|e| unreadable(e.to_string()))?
            .named("the workspace's folder".to_string(), "it".to_string());
        for step in relative.split('/').filter(|step| !step.is_empty()) {
            folder = match folder.sub(step, false) {
                Ok(Some(next)) => next,
                opened => {
                    let not = match folder.entry(step) {
                        Ok(Some(entry)) if entry.kind == rustix::fs::FileType::Symlink => {
                            NotHere::ThroughALink(path.to_string())
                        }
                        Ok(None) => Refused::NotThere(path.to_string()).into(),
                        Ok(Some(entry)) if entry.kind != rustix::fs::FileType::Directory => {
                            Refused::NotAFolder(path.to_string()).into()
                        }
                        Ok(Some(_)) => {
                            let why = opened.err().map_or_else(String::new, |e| e.to_string());
                            return Err(unreadable(why));
                        }
                        Err(why) => return Err(unreadable(why)),
                    };
                    return Err(not.into());
                }
            };
        }
        Ok(folder)
    }

    /// **The folder a new entry goes in**, `folder` (`""` for the top): judged as a read is,
    /// none that purlis keeps, and held through no link. Held, with its spelling.
    fn folder_to_write(&self, folder: &str, kept: &[String]) -> Result<(Held, String), NotWritten> {
        let relative = self.judged(&self.on_disk(&self.judged(folder)?))?;
        if !relative.is_empty() && managed(&relative, kept) {
            return Err(NotWritten::Managed(folder.to_string()));
        }
        Ok((self.held(&relative, folder)?, relative))
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
    /// itself, and no link, its own leaf included; its folder held.
    fn entry_to_write(&self, path: &str, kept: &[String]) -> Result<Checked, NotWritten> {
        // Judged as the disk spells it, so a spelling the disk folds onto a repo's or a name
        // purlis keeps is judged as that name, whatever folding the disk does.
        let relative = self.judged(&self.on_disk(&self.judged(path)?))?;
        if relative.is_empty() {
            return Err(NotWritten::TheFolderItself);
        }
        if managed(&relative, kept) {
            return Err(NotWritten::Managed(path.to_string()));
        }
        let (parent, leaf) = relative.rsplit_once('/').unwrap_or(("", &relative));
        let folder = self.held(parent, path)?;
        match folder.entry(leaf) {
            Ok(Some(entry)) if entry.kind == rustix::fs::FileType::Symlink => {
                Err(NotWritten::ALink(path.to_string()))
            }
            Ok(Some(entry)) => Ok(Checked {
                folder,
                parent: parent.to_string(),
                leaf: leaf.to_string(),
                entry,
            }),
            Ok(None) => Err(NotHere::from(Refused::NotThere(path.to_string())).into()),
            Err(why) => Err(NotWritten::Failed {
                what: format!("read '{path}'"),
                why,
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
        after_the_check();
        into.create_new(name)
            .map_err(|e| made(e.into(), name, "make the file"))?;
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
        after_the_check();
        into.create_dir_new(name)
            .map_err(|e| made(e.into(), name, "make the folder"))?;
        Ok(path)
    }

    /// **Rename** the file or folder at `path` to `name`, in the same folder: its new path, or
    /// why not. Never over anything that has the name already, another name of the same file
    /// (a hard link) included; a change of case alone is a rename too.
    pub fn rename(&self, path: &str, name: &str, kept: &[String]) -> Result<String, NotWritten> {
        let checked = self.entry_to_write(path, kept)?;
        let renamed = self.name_to_give(&checked.parent, name, kept)?;
        after_the_check();
        let (folder, leaf) = (&checked.folder, checked.leaf.as_str());
        let failed = |why: String| NotWritten::Failed {
            what: format!("rename '{path}'"),
            why,
        };
        let taken = || NotWritten::Taken(name.to_string());
        if leaf != name && folded(leaf) == folded(name) {
            // Only its case or its Unicode form changes, which a disk that does not tell them
            // apart takes for the name it already has: through a spare name, each step never
            // over anything, so what has the new name on a disk that does tell them apart is
            // refused rather than replaced.
            let spare = format!(".purlis-renaming-{}", std::process::id());
            if !folder
                .rename_into_new(leaf, folder, &spare)
                .map_err(failed)?
            {
                return Err(failed(format!("'{spare}' is in the way")));
            }
            return match folder.rename_into_new(&spare, folder, name) {
                Ok(true) => Ok(renamed),
                moved => {
                    let back = folder.rename_into_new(&spare, folder, leaf);
                    if !matches!(back, Ok(true)) {
                        return Err(failed(format!(
                            "it could not be given its name back, and is '{}'",
                            joined(&checked.parent, &spare)
                        )));
                    }
                    Err(moved.map_or_else(failed, |_| taken()))
                }
            };
        }
        if folder.rename_into_new(leaf, folder, name).map_err(failed)? {
            Ok(renamed)
        } else {
            Err(taken())
        }
    }

    /// **Delete** the file or folder at `path`: handed to `trash`, and refused, the entry left
    /// where it is, when the Trash will not take it. Never deleted for good.
    ///
    /// The Trash takes a path, never a held folder: so just before it is handed over, the path
    /// is walked again through no link, and must reach the very folder and entry checked.
    pub fn trash(&self, path: &str, kept: &[String], trash: &dyn Trash) -> Result<(), NotWritten> {
        let checked = self.entry_to_write(path, kept)?;
        after_the_check();
        let one = |a: crate::held::Entry, b: crate::held::Entry| a.dev == b.dev && a.ino == b.ino;
        let still = self.held(&checked.parent, path).ok().is_some_and(|again| {
            matches!(
                (again.itself(), checked.folder.itself()),
                (Ok(now), Ok(then)) if one(now, then)
            ) && matches!(
                again.entry(&checked.leaf),
                Ok(Some(now)) if now.kind == checked.entry.kind && one(now, checked.entry)
            )
        });
        if !still {
            return Err(NotWritten::Moved(path.to_string()));
        }
        let absolute = self.dir.join(joined(&checked.parent, &checked.leaf));
        trash.put(&absolute).map_err(|why| NotWritten::NoTrash {
            path: path.to_string(),
            why,
        })
    }

    /// **Import** each of `sources`, files the person picked anywhere on disk, into `folder`
    /// (`""` for the top), copied: the path each was given, in order. A name already taken, or
    /// kept for purlis, gets the first free ` (n)` suffix, from ` (2)`. One that cannot be
    /// imported stops the import there, and the refusal names the files already copied.
    pub fn import(
        &self,
        folder: &str,
        sources: &[PathBuf],
        kept: &[String],
    ) -> Result<Vec<String>, NotWritten> {
        let (into, relative) = self.folder_to_write(folder, kept)?;
        after_the_check();
        let mut imported = Vec::with_capacity(sources.len());
        for source in sources {
            match self.imported(&into, &relative, source, kept) {
                Ok(path) => imported.push(path),
                Err(why) if imported.is_empty() => return Err(why),
                Err(why) => {
                    return Err(NotWritten::PartlyImported {
                        copied: imported,
                        why: Box::new(why),
                    });
                }
            }
        }
        Ok(imported)
    }

    /// `source` copied into the folder `into` (spelled `relative`): the path it was given.
    fn imported(
        &self,
        into: &Held,
        relative: &str,
        source: &Path,
        kept: &[String],
    ) -> Result<String, NotWritten> {
        let shown = source.display().to_string();
        let mut from = match opened_to_import(source) {
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
        self.copied_in(&mut from, into, relative, name, kept)
    }

    /// `from` copied into the folder `into` (spelled `relative`) as `name`, or the first free
    /// ` (n)` of it: the path it was given.
    fn copied_in(
        &self,
        from: &mut std::fs::File,
        into: &Held,
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
            let mut to = match into.create_new(&candidate) {
                Ok(to) => to,
                Err(rustix::io::Errno::EXIST) => continue,
                Err(e) => return Err(made(e.into(), &candidate, "copy the file in")),
            };
            if let Err(e) = std::io::copy(from, &mut to) {
                drop(to);
                // Only the copy this import just made, never anything that was there.
                let _ = into.remove(&candidate);
                return Err(made(e, &candidate, "copy the file in"));
            }
            return Ok(path);
        }
        Err(NotWritten::Taken(name.to_string()))
    }
}

/// `source`, a file the person picked, opened to read without waiting: a FIFO opened so never
/// waits for a writer, and is then refused for not being a file.
#[cfg(unix)]
fn opened_to_import(source: &Path) -> std::io::Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags};
    let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    Ok(rustix::fs::open(source, flags, Mode::empty())?.into())
}

/// What a write's failure says: a name already taken, or the system's own reason.
#[cfg(unix)]
fn made(e: std::io::Error, name: &str, what: &str) -> NotWritten {
    if e.kind() == std::io::ErrorKind::AlreadyExists {
        NotWritten::Taken(name.to_string())
    } else {
        NotWritten::Failed {
            what: what.to_string(),
            why: e.to_string(),
        }
    }
}

/// **No descriptor to hold**, off unix: every write is refused before anything is made.
#[cfg(not(unix))]
impl Files {
    pub fn new_file(
        &self,
        _folder: &str,
        _name: &str,
        _kept: &[String],
    ) -> Result<String, NotWritten> {
        Err(no_descriptors())
    }
    pub fn new_folder(
        &self,
        _folder: &str,
        _name: &str,
        _kept: &[String],
    ) -> Result<String, NotWritten> {
        Err(no_descriptors())
    }
    pub fn rename(&self, _path: &str, _name: &str, _kept: &[String]) -> Result<String, NotWritten> {
        Err(no_descriptors())
    }
    pub fn trash(
        &self,
        _path: &str,
        _kept: &[String],
        _trash: &dyn Trash,
    ) -> Result<(), NotWritten> {
        Err(no_descriptors())
    }
    pub fn import(
        &self,
        _folder: &str,
        _sources: &[PathBuf],
        _kept: &[String],
    ) -> Result<Vec<String>, NotWritten> {
        Err(no_descriptors())
    }
}

#[cfg(not(unix))]
fn no_descriptors() -> NotWritten {
    NotWritten::Failed {
        what: "write the workspace's files".to_string(),
        why: "purlis writes them only on macOS and Linux".to_string(),
    }
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The folders that say a session's program runs** ([`super::Spec::holds`]), worked through
//! handles. The folder two above a hold is opened by its path, as a folder itself (`O_DIRECTORY
//! | O_NOFOLLOW`), once; the folder between and the hold are made and opened relative to it,
//! before the program starts. Its note is then made and removed relative to that
//! handle. A link put in either place afterwards changes nothing: what was checked is what is
//! written. Off unix nothing is noted: whether a noted process runs cannot be asked there, so
//! no caller reads a note.

use std::path::PathBuf;

/// A folder a program's note goes in, opened as a folder itself.
#[derive(Debug)]
pub(super) struct Hold {
    #[cfg(unix)]
    dir: std::os::fd::OwnedFd,
}

/// **A file that says a session's program runs**: `<hold>/<pid>`, made once the program has
/// started and removed when this is dropped, both relative to its folder's handle. A caller
/// reads it as a process id and asks whether that process still runs, and started before the
/// file was written. A note the session could not remove (its app ended without a word) names
/// a process that has gone, or one that started after it.
#[derive(Debug)]
pub(super) struct Note {
    #[cfg(unix)]
    dir: std::os::fd::OwnedFd,
    #[cfg(unix)]
    name: String,
}

/// Opens each of `holds`, making it and the folder it is in where they are not there yet: each
/// a folder itself, never through a link. A link at either, or where either was to be made, is
/// refused, as is a folder that cannot be made, with the hold it was.
pub(super) fn open_all(holds: &[PathBuf]) -> Result<Vec<Hold>, (PathBuf, std::io::Error)> {
    holds
        .iter()
        .map(|hold| Hold::open(hold).map_err(|why| (hold.clone(), why)))
        .collect()
}

/// One note for `pid` in each of `holds`, or the first error, with every note made before it
/// removed again.
pub(super) fn write_all(holds: Vec<Hold>, pid: Option<u32>) -> std::io::Result<Vec<Note>> {
    if holds.is_empty() {
        return Ok(Vec::new());
    }
    let pid = pid.ok_or_else(|| std::io::Error::other("its process has no id"))?;
    holds.into_iter().map(|hold| hold.note(pid)).collect()
}

#[cfg(unix)]
mod at {
    use std::os::fd::{AsFd, OwnedFd};
    use std::path::Path;

    use rustix::fs::{AtFlags, CWD, Mode, OFlags};

    pub(super) fn folder_flags() -> OFlags {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }

    /// `name` in `dir`, opened as a folder itself, made first where it is not there.
    pub(super) fn folder_in(dir: impl AsFd, name: &Path) -> std::io::Result<OwnedFd> {
        match rustix::fs::mkdirat(&dir, name, Mode::from_raw_mode(0o755)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(why) => return Err(why.into()),
        }
        Ok(rustix::fs::openat(
            &dir,
            name,
            folder_flags(),
            Mode::empty(),
        )?)
    }

    /// `at`, an existing folder, opened as a folder itself.
    pub(super) fn open(at: &Path) -> std::io::Result<OwnedFd> {
        Ok(rustix::fs::openat(CWD, at, folder_flags(), Mode::empty())?)
    }

    /// Makes `name` new in `dir`: never through a link, and never one already there.
    pub(super) fn new_file_in(dir: impl AsFd, name: &str) -> std::io::Result<()> {
        // One left by a program with the same id that has gone, or a link in its place, is
        // removed itself first.
        let _ = rustix::fs::unlinkat(&dir, name, AtFlags::empty());
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        rustix::fs::openat(&dir, name, flags, Mode::from_raw_mode(0o644))?;
        Ok(())
    }

    pub(super) fn remove_file_in(dir: impl AsFd, name: &str) {
        let _ = rustix::fs::unlinkat(&dir, name, AtFlags::empty());
    }
}

impl Hold {
    #[cfg(unix)]
    fn open(hold: &std::path::Path) -> std::io::Result<Hold> {
        // The folder the hold's folder is in, opened by its path once; the hold's folder and
        // the hold below it are made and opened through handles, never back through a path.
        let parent = hold.parent();
        let (Some(above), Some(parent_name), Some(name)) = (
            parent.and_then(std::path::Path::parent),
            parent.and_then(std::path::Path::file_name),
            hold.file_name(),
        ) else {
            return Err(std::io::Error::other("it is in no folder"));
        };
        let above = at::open(above).map_err(not_a_folder)?;
        let parent =
            at::folder_in(&above, std::path::Path::new(parent_name)).map_err(not_a_folder)?;
        let dir = at::folder_in(&parent, std::path::Path::new(name)).map_err(not_a_folder)?;
        Ok(Hold { dir })
    }

    #[cfg(not(unix))]
    fn open(_hold: &std::path::Path) -> std::io::Result<Hold> {
        Ok(Hold {})
    }

    #[cfg(unix)]
    fn note(self, pid: u32) -> std::io::Result<Note> {
        let name = pid.to_string();
        at::new_file_in(&self.dir, &name)?;
        Ok(Note {
            dir: self.dir,
            name,
        })
    }

    #[cfg(not(unix))]
    fn note(self, _pid: u32) -> std::io::Result<Note> {
        Ok(Note {})
    }
}

/// A folder that is a link or a file, said as such.
#[cfg(unix)]
fn not_a_folder(why: std::io::Error) -> std::io::Error {
    match why.raw_os_error() {
        Some(code)
            if code == rustix::io::Errno::LOOP.raw_os_error()
                || code == rustix::io::Errno::NOTDIR.raw_os_error() =>
        {
            std::io::Error::other("it is a link or a file, not a folder")
        }
        _ => why,
    }
}

impl Drop for Note {
    fn drop(&mut self) {
        #[cfg(unix)]
        at::remove_file_in(&self.dir, &self.name);
    }
}

#[cfg(all(test, unix))]
mod tests;

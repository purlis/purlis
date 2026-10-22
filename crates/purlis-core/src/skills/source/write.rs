// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Writing an installed skill** (SK-10, #2773): every folder and file made through handles
//! opened one component at a time with `O_NOFOLLOW`, each name one purlis made from the source's
//! tree after [`purlis_skills::tree::plain_name`] passed it, a new file made with `O_EXCL`, and a
//! replaced one renamed over only while it is still a plain file. Nothing is followed, and
//! nothing is removed but the names this write made or a lockfile says an install made.

/// One file to write: its path inside the skill's folder, whether it is executable, its bytes.
pub(super) struct File<'a> {
    pub parts: &'a [String],
    pub executable: bool,
    pub bytes: &'a [u8],
}

#[cfg(unix)]
mod unix {
    use std::io::Write as _;
    use std::path::Path;

    use rustix::fd::OwnedFd;
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, fchmod, mkdirat, openat, renameat, statat, unlinkat};

    use super::File;
    use crate::skills::verbs::store::{FOLDER, open_dir};

    fn mode(executable: bool) -> Mode {
        Mode::from_raw_mode(if executable { 0o755 } else { 0o644 })
    }

    /// The folder `parts` names under `at`, made where it is not there, followed through no link.
    fn folder_in(
        at: &OwnedFd,
        parts: &[String],
        made: &mut Vec<Vec<String>>,
    ) -> std::io::Result<OwnedFd> {
        let mut here = openat(at, ".", FOLDER, Mode::empty())?;
        let mut path: Vec<String> = Vec::new();
        for part in parts {
            path.push(part.clone());
            here = match openat(&here, part.as_str(), FOLDER, Mode::empty()) {
                Ok(next) => next,
                Err(rustix::io::Errno::NOENT) => {
                    mkdirat(&here, part.as_str(), Mode::from_raw_mode(0o755))?;
                    made.push(path.clone());
                    openat(&here, part.as_str(), FOLDER, Mode::empty())?
                }
                Err(why) => return Err(why.into()),
            };
        }
        Ok(here)
    }

    /// Writes `bytes` as the new file `name` in `at`, made with `O_EXCL` and given its mode.
    fn new_file(at: &OwnedFd, name: &str, executable: bool, bytes: &[u8]) -> std::io::Result<()> {
        let file = openat(
            at,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            mode(executable),
        )?;
        fchmod(&file, mode(executable))?;
        let mut file = std::fs::File::from(file);
        file.write_all(bytes)?;
        file.sync_all()
    }

    /// Takes away what one write made, newest first: its files, then its folders, which are
    /// empty by then. A name that is no longer what was made is left.
    fn undo(skill: &OwnedFd, files: &[Vec<String>], folders: &[Vec<String>]) {
        for path in files.iter().rev() {
            let Some((leaf, parents)) = path.split_last() else { continue };
            if let Ok(at) = open_in(skill, parents) {
                let _ = unlinkat(&at, leaf.as_str(), AtFlags::empty());
            }
        }
        for path in folders.iter().rev() {
            let Some((leaf, parents)) = path.split_last() else { continue };
            if let Ok(at) = open_in(skill, parents) {
                let _ = unlinkat(&at, leaf.as_str(), AtFlags::REMOVEDIR);
            }
        }
    }

    /// The folder `parts` names under `at`, following no link and making nothing.
    fn open_in(at: &OwnedFd, parts: &[String]) -> std::io::Result<OwnedFd> {
        let mut here = openat(at, ".", FOLDER, Mode::empty())?;
        for part in parts {
            here = openat(&here, part.as_str(), FOLDER, Mode::empty())?;
        }
        Ok(here)
    }

    /// **Makes the skill folder `name`** in the level folder `level` names under `plane` (making
    /// the level's parts from the `make_from`th on), and writes `files` into it. Refused where
    /// anything already has the name; on any failure, what was made is taken away again.
    pub(in crate::skills::source) fn new_skill(
        plane: &Path,
        level: &[String],
        make_from: usize,
        name: &str,
        files: &[File<'_>],
    ) -> std::io::Result<()> {
        let at = open_dir(plane, level, make_from)?;
        mkdirat(&at, name, Mode::from_raw_mode(0o755))?;
        let skill = match openat(&at, name, FOLDER, Mode::empty()) {
            Ok(skill) => skill,
            Err(why) => {
                let _ = unlinkat(&at, name, AtFlags::REMOVEDIR);
                return Err(why.into());
            }
        };
        let mut folders: Vec<Vec<String>> = Vec::new();
        let mut written: Vec<Vec<String>> = Vec::new();
        let result = (|| {
            for file in files {
                let (leaf, parents) = file
                    .parts
                    .split_last()
                    .ok_or(std::io::ErrorKind::InvalidInput)?;
                let here = folder_in(&skill, parents, &mut folders)?;
                new_file(&here, leaf, file.executable, file.bytes)?;
                written.push(file.parts.to_vec());
            }
            Ok(())
        })();
        if result.is_err() {
            undo(&skill, &written, &folders);
            drop(skill);
            let _ = unlinkat(&at, name, AtFlags::REMOVEDIR);
        }
        result
    }

    /// **Brings the installed skill in the folder `folder` names under `plane` to `files`**:
    /// each written beside its old self and renamed over it, which only replaces a plain file,
    /// and each of `gone` — files the lockfile says the install wrote and the newer commit no
    /// longer has — removed where it is still a plain file. A file the person added is kept.
    pub(in crate::skills::source) fn replace(
        plane: &Path,
        folder: &[String],
        files: &[File<'_>],
        gone: &[Vec<String>],
    ) -> std::io::Result<()> {
        let skill = open_dir(plane, folder, usize::MAX)?;
        let mut made = Vec::new();
        for file in files {
            let (leaf, parents) = file
                .parts
                .split_last()
                .ok_or(std::io::ErrorKind::InvalidInput)?;
            let here = folder_in(&skill, parents, &mut made)?;
            match statat(&here, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile => {
                    return Err(std::io::Error::other(format!(
                        "{} is not a plain file",
                        file.parts.join("/")
                    )));
                }
                Ok(_) | Err(rustix::io::Errno::NOENT) => {}
                Err(why) => return Err(why.into()),
            }
            let temp = format!(".purlis-update-{}", std::process::id());
            let _ = unlinkat(&here, temp.as_str(), AtFlags::empty());
            new_file(&here, &temp, file.executable, file.bytes)?;
            if let Err(why) = renameat(&here, temp.as_str(), &here, leaf.as_str()) {
                let _ = unlinkat(&here, temp.as_str(), AtFlags::empty());
                return Err(why.into());
            }
        }
        for path in gone {
            let Some((leaf, parents)) = path.split_last() else { continue };
            let Ok(here) = open_in(&skill, parents) else { continue };
            if statat(&here, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile)
            {
                unlinkat(&here, leaf.as_str(), AtFlags::empty())?;
            }
        }
        Ok(())
    }

    /// Each plain file in the folder `folder` names under `plane`, with its path inside it and its
    /// bytes, followed through no link: what an installed skill holds now. A file larger than
    /// `most` bytes, or past `most_files` of them, ends the read with `None`.
    pub(in crate::skills::source) fn read_all(
        plane: &Path,
        folder: &[String],
        most: u64,
        most_files: usize,
    ) -> Option<Vec<(Vec<String>, bool, Vec<u8>)>> {
        use std::io::Read as _;
        let skill = open_dir(plane, folder, usize::MAX).ok()?;
        let mut out = Vec::new();
        let mut stack: Vec<(OwnedFd, Vec<String>)> = vec![(skill, Vec::new())];
        while let Some((at, path)) = stack.pop() {
            let dir = rustix::fs::Dir::read_from(&at).ok()?;
            for entry in dir.filter_map(Result::ok) {
                let Ok(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if name == "." || name == ".." {
                    continue;
                }
                let mut here = path.clone();
                here.push(name.clone());
                let Ok(stat) = statat(&at, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) else {
                    continue;
                };
                match FileType::from_raw_mode(stat.st_mode) {
                    FileType::Directory => {
                        let next = openat(&at, name.as_str(), FOLDER, Mode::empty()).ok()?;
                        stack.push((next, here));
                    }
                    FileType::RegularFile => {
                        if stat.st_size as u64 > most || out.len() >= most_files {
                            return None;
                        }
                        let file = openat(
                            &at,
                            name.as_str(),
                            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .ok()?;
                        let mut bytes = Vec::new();
                        std::fs::File::from(file)
                            .take(most + 1)
                            .read_to_end(&mut bytes)
                            .ok()?;
                        let executable = stat.st_mode & 0o111 != 0;
                        out.push((here, executable, bytes));
                    }
                    _ => {}
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Some(out)
    }
}

#[cfg(unix)]
pub(super) use unix::{new_skill, read_all, replace};

/// Where no folder can be held open, nothing is installed.
#[cfg(not(unix))]
mod elsewhere {
    use std::path::Path;

    use super::File;

    fn refused() -> std::io::Error {
        std::io::Error::other("a skill is installed only where its folders can be held open")
    }

    pub(in crate::skills::source) fn new_skill(
        _: &Path,
        _: &[String],
        _: usize,
        _: &str,
        _: &[File<'_>],
    ) -> std::io::Result<()> {
        Err(refused())
    }

    pub(in crate::skills::source) fn replace(
        _: &Path,
        _: &[String],
        _: &[File<'_>],
        _: &[Vec<String>],
    ) -> std::io::Result<()> {
        Err(refused())
    }

    pub(in crate::skills::source) fn read_all(
        _: &Path,
        _: &[String],
        _: u64,
        _: usize,
    ) -> Option<Vec<(Vec<String>, bool, Vec<u8>)>> {
        None
    }
}

#[cfg(not(unix))]
pub(super) use elsewhere::{new_skill, read_all, replace};

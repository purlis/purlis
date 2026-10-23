// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Writing an installed skill** (SK-10, #2773): every folder and file made through handles
//! opened one component at a time with `O_NOFOLLOW`, each name one purlis made from the source's
//! tree after its name rule ([`purlis_skills::tree`]) passed it, a new file made with `O_EXCL`, and a
//! replaced one renamed over only while it is still a plain file. Nothing is followed, and
//! nothing is removed but the names this write made or a lockfile says an install made (and, on
//! a disk that ignores case, never a name that is now a file this write made).

/// One file an installed skill holds now: its path inside its folder, whether it is executable,
/// and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HeldFile {
    pub parts: Vec<String>,
    pub executable: bool,
    pub bytes: Vec<u8>,
}

/// What one new skill's write made, so a failed install takes exactly that away again: the
/// files and the folders, by their paths inside the skill's folder, newest last.
#[derive(Debug, Clone, Default)]
pub(super) struct Made {
    files: Vec<Vec<String>>,
    folders: Vec<Vec<String>>,
}

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
    use rustix::fs::{
        AtFlags, FileType, Mode, OFlags, fchmod, mkdirat, openat, renameat, statat, unlinkat,
    };

    use super::{File, HeldFile, Made};
    use crate::skills::verbs::store::{FOLDER, open_dir};

    /// A file's identity on its disk: its device and inode, whose types differ by platform.
    #[allow(clippy::unnecessary_cast)]
    fn identity(stat: &rustix::fs::Stat) -> (u64, u64) {
        (stat.st_dev as u64, stat.st_ino as u64)
    }

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
            let Some((leaf, parents)) = path.split_last() else {
                continue;
            };
            if let Ok(at) = open_in(skill, parents) {
                let _ = unlinkat(&at, leaf.as_str(), AtFlags::empty());
            }
        }
        for path in folders.iter().rev() {
            let Some((leaf, parents)) = path.split_last() else {
                continue;
            };
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
    ) -> std::io::Result<Made> {
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
        if let Err(why) = result {
            undo(&skill, &written, &folders);
            drop(skill);
            let _ = unlinkat(&at, name, AtFlags::REMOVEDIR);
            return Err(why);
        }
        Ok(Made {
            files: written,
            folders,
        })
    }

    /// **Takes away a skill an install made** — the folder `name` in the level folder `level`
    /// names under `plane` — by exactly what [`new_skill`] says it made: its files, its folders
    /// once empty, then the skill's folder. A name that is no longer what was made is left.
    pub(in crate::skills::source) fn take_back(
        plane: &Path,
        level: &[String],
        name: &str,
        made: &Made,
    ) {
        let Ok(at) = open_dir(plane, level, usize::MAX) else {
            return;
        };
        if let Ok(skill) = openat(&at, name, FOLDER, Mode::empty()) {
            undo(&skill, &made.files, &made.folders);
        }
        let _ = unlinkat(&at, name, AtFlags::REMOVEDIR);
    }

    /// Takes away the file `name` an install made in the level folder `level` names under
    /// `plane` (a grant beside a skill), where it is still a plain file.
    pub(in crate::skills::source) fn take_back_file(plane: &Path, level: &[String], name: &str) {
        let Ok(at) = open_dir(plane, level, usize::MAX) else {
            return;
        };
        if statat(&at, name, AtFlags::SYMLINK_NOFOLLOW)
            .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile)
        {
            let _ = unlinkat(&at, name, AtFlags::empty());
        }
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
        // What each file written is, by its device and inode: on a disk that ignores case,
        // `run.sh` the newer commit lost and `Run.sh` it gained are one file, which stays.
        let mut written: Vec<(u64, u64)> = Vec::new();
        for file in files {
            let (leaf, parents) = file
                .parts
                .split_last()
                .ok_or(std::io::ErrorKind::InvalidInput)?;
            let here = folder_in(&skill, parents, &mut made)?;
            // On a disk that ignores case, the file the newer commit renamed only by its case
            // is this one under its old spelling: it takes the new spelling first.
            for old in gone {
                let Some((old_leaf, old_parents)) = old.split_last() else {
                    continue;
                };
                if old_parents != parents
                    || old_leaf == leaf
                    || !old_leaf.eq_ignore_ascii_case(leaf)
                {
                    continue;
                }
                let same = match (
                    statat(&here, old_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW),
                    statat(&here, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW),
                ) {
                    (Ok(a), Ok(b)) => {
                        a.st_dev == b.st_dev
                            && a.st_ino == b.st_ino
                            && FileType::from_raw_mode(a.st_mode) == FileType::RegularFile
                    }
                    _ => false,
                };
                if same {
                    renameat(&here, old_leaf.as_str(), &here, leaf.as_str())?;
                }
            }
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
            // A name of its own, made new: one already there is never taken or removed.
            let mut temp = String::new();
            for attempt in 0..16u32 {
                let name = format!(".purlis-update-{}-{attempt}", std::process::id());
                match new_file(&here, &name, file.executable, file.bytes) {
                    Ok(()) => {
                        temp = name;
                        break;
                    }
                    Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(why) => return Err(why),
                }
            }
            if temp.is_empty() {
                return Err(std::io::Error::other(
                    "no name of its own was free to write to",
                ));
            }
            if let Err(why) = renameat(&here, temp.as_str(), &here, leaf.as_str()) {
                let _ = unlinkat(&here, temp.as_str(), AtFlags::empty());
                return Err(why.into());
            }
            if let Ok(stat) = statat(&here, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
                written.push(identity(&stat));
            }
        }
        for path in gone {
            let Some((leaf, parents)) = path.split_last() else {
                continue;
            };
            let Ok(here) = open_in(&skill, parents) else {
                continue;
            };
            let gone_now =
                statat(&here, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW).is_ok_and(|stat| {
                    FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                        && !written.contains(&identity(&stat))
                });
            if gone_now {
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
    ) -> Option<Vec<HeldFile>> {
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
                        out.push(HeldFile {
                            parts: here,
                            executable,
                            bytes,
                        });
                    }
                    _ => {}
                }
            }
        }
        out.sort_by(|a, b| a.parts.cmp(&b.parts));
        Some(out)
    }
}

#[cfg(unix)]
pub(super) use unix::{new_skill, read_all, replace, take_back, take_back_file};

/// Where no folder can be held open, nothing is installed.
#[cfg(not(unix))]
mod elsewhere {
    use std::path::Path;

    use super::{File, HeldFile, Made};

    fn refused() -> std::io::Error {
        std::io::Error::other("a skill is installed only where its folders can be held open")
    }

    pub(in crate::skills::source) fn new_skill(
        _: &Path,
        _: &[String],
        _: usize,
        _: &str,
        _: &[File<'_>],
    ) -> std::io::Result<Made> {
        Err(refused())
    }

    pub(in crate::skills::source) fn take_back(_: &Path, _: &[String], _: &str, _: &Made) {}

    pub(in crate::skills::source) fn take_back_file(_: &Path, _: &[String], _: &str) {}

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
    ) -> Option<Vec<HeldFile>> {
        None
    }
}

#[cfg(not(unix))]
pub(super) use elsewhere::{new_skill, read_all, replace, take_back, take_back_file};

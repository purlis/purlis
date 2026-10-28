// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Files across a browser's edge** (#2020, spec #2008): what a chat's browser may take from the
//! chat's folder to upload, and how a download is brought into it.
//!
//! The engine reads and writes files as the person, outside every chat's sandbox, so it is never
//! handed a path a chat named. Everything that crosses goes through here, in the session host:
//!
//! - **An upload is read from the chat's folder alone** ([`stage`]). Each file is named by a
//!   path relative to that folder ([`relative`]: no `..`, no absolute path), and opened one
//!   name at a time from the folder down, following no link at any step, so a link the chat
//!   made, or swapped in meanwhile, leads nowhere. Only a plain file with one name is read (a
//!   second name, a hard link, could be one of a file the chat may not read), each at most
//!   [`MOST_UPLOAD_BYTES`], at most [`MOST_UPLOAD_FILES`] and [`MOST_UPLOAD_TOTAL`] a call. What
//!   is read is copied into the browser's own folder, which no chat reads or writes, and the
//!   engine is handed that copy: what it uploads is what was checked, whatever the chat does
//!   after.
//! - **What purlis keeps from every chat is judged by what a file is, not how it is spelled**
//!   ([`Kept`]): each folder on the way and the file opened are compared, by device and inode,
//!   with every path [`crate::sandbox::kept_from_reading`] names, and with every folder above
//!   the chat's folder, on the very descriptor that is read. A disk that folds names opens one
//!   file by many spellings; its identity is one.
//! - **A download is brought in as a new file** ([`bring`]): written under the chat's folder at
//!   a relative path, its folders walked the same way, never over a file or a link that is
//!   there, and never into a folder purlis keeps from chats. What else is checked of where it
//!   goes is the caller's: the brokered-write guard ([`crate::brokered::guard`]: no later-code
//!   name, no `.gitignore`, no manifest). The chat's other write denials are not judged yet,
//!   and every save is refused until they are (#3746).

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The most bytes one uploaded file may hold.
pub const MOST_UPLOAD_BYTES: u64 = 50 << 20;

/// The most files one upload may put into a file input.
pub const MOST_UPLOAD_FILES: usize = 20;

/// The most bytes one upload may hold, every file's together.
pub const MOST_UPLOAD_TOTAL: u64 = 100 << 20;

/// **What purlis keeps from every chat, as the files and folders they are** (#2020): the
/// device and inode of each path [`crate::sandbox::kept_from_reading`] names that is there.
#[derive(Debug, Clone, Default)]
pub struct Kept {
    ids: std::collections::HashSet<(u64, u64)>,
}

impl Kept {
    /// What purlis keeps from the chats of the project at `project`, on this machine. Refused
    /// where that list cannot be told whole: nothing crosses then.
    /// Why is written to the host's log; the refusal names no path.
    pub fn of(project: &Path) -> Result<Kept, String> {
        let paths = crate::sandbox::kept_from_reading(project, &crate::sandbox::Machine::this())
            .map_err(|why| {
                tracing::warn!("purlis: no file crosses a browser's edge: {why}");
                UNTOLD.to_owned()
            })?;
        Ok(Kept::of_paths(&paths))
    }

    /// The files and folders at `paths` that are there.
    pub fn of_paths(paths: &[PathBuf]) -> Kept {
        Kept {
            ids: paths.iter().filter_map(|path| id_of_path(path)).collect(),
        }
    }

    fn holds(&self, id: (u64, u64)) -> bool {
        self.ids.contains(&id)
    }

    /// Refused where `root`, or any folder above it, is one purlis keeps.
    fn refuses_above(&self, root: &Path) -> Result<(), String> {
        let real = root
            .canonicalize()
            .map_err(|why| format!("the chat's folder could not be opened: {why}"))?;
        if real
            .ancestors()
            .filter_map(id_of_path)
            .any(|id| self.holds(id))
        {
            return Err(KEPT.to_owned());
        }
        Ok(())
    }
}

/// What an upload or a save is refused with where the list of what purlis keeps from chats
/// cannot be told whole (#2020): words alone, no path of this machine's.
pub const UNTOLD: &str = "purlis could not read its list of what it keeps from chats, so no file \
                          crosses: tell the person, whose project's vault settings may need a look";

/// What a file or folder purlis keeps from chats is refused with (#2020).
const KEPT: &str = "it is a file or folder purlis keeps from every chat, so it does not cross";

#[cfg(unix)]
fn id_of_path(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata(path)
        .ok()
        .map(|meta| (meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn id_of_path(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// **`written` as one spelling to judge by** (#2020): compatibility-composed and lower-cased, so
/// spellings a disk may open as one name read as one. For an early refusal by spelling, before
/// a file is opened; the check by what a file is ([`Kept`]) is the one that holds.
pub fn folded(written: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    written.nfkc().collect::<String>().to_lowercase()
}

/// **`written`, a path relative to a chat's folder, as its names**, or why it is not one: no
/// absolute path, no `..`, no empty name and no NUL. `.` names are dropped.
pub fn relative(written: &str) -> Result<Vec<String>, String> {
    let refused = |why: &str| {
        Err(format!(
            "{written:?} is not a path in the chat's folder: {why}"
        ))
    };
    if written.trim().is_empty() {
        return refused("it is empty");
    }
    if written.contains('\0') {
        return refused("it holds a NUL");
    }
    if written.starts_with('/') || written.starts_with('\\') || written.starts_with('~') {
        return refused("give a path relative to the chat's folder");
    }
    let mut names = Vec::new();
    for name in written.split(['/', '\\']) {
        match name {
            "" | "." => {}
            ".." => return refused("`..` leads out of it"),
            name => names.push(name.to_owned()),
        }
    }
    if names.is_empty() {
        return refused("it names the folder itself, not a file");
    }
    Ok(names)
}

/// **Copies each of `files`, paths relative to `root`, into `into`** (made this user's alone),
/// and answers the copies, in order, each under its own name in a folder of its own. Refused,
/// copying nothing it can avoid, where any file is not one [`stage`] reads: outside `root`,
/// through a link, not a plain file, with more than one name, or past the limits.
pub fn stage(
    root: &Path,
    files: &[String],
    into: &Path,
    kept: &Kept,
) -> Result<Vec<PathBuf>, String> {
    if files.is_empty() {
        return Err("give `files`, at least one path in the chat's folder".into());
    }
    if files.len() > MOST_UPLOAD_FILES {
        return Err(format!(
            "an upload takes at most {MOST_UPLOAD_FILES} files; this one gave {}",
            files.len()
        ));
    }
    kept.refuses_above(root)?;
    let mut opened = Vec::new();
    let mut total: u64 = 0;
    for written in files {
        let names = relative(written)?;
        let (file, size) =
            open_beneath(root, &names, kept).map_err(|why| format!("{written}: {why}"))?;
        if size > MOST_UPLOAD_BYTES {
            return Err(format!(
                "{written} is {}, past the {} a file may be to upload",
                bytes_said(size),
                bytes_said(MOST_UPLOAD_BYTES)
            ));
        }
        total = total.saturating_add(size);
        if total > MOST_UPLOAD_TOTAL {
            return Err(format!(
                "these files are past the {} an upload may hold together",
                bytes_said(MOST_UPLOAD_TOTAL)
            ));
        }
        opened.push((names.last().cloned().unwrap_or_default(), file, size));
    }
    private_folder(into)
        .map_err(|why| format!("purlis could not make the upload's folder: {why}"))?;
    let mut copies = Vec::new();
    for (at, (name, file, size)) in opened.into_iter().enumerate() {
        let folder = into.join(at.to_string());
        private_folder(&folder)
            .map_err(|why| format!("purlis could not make the upload's folder: {why}"))?;
        let copy = folder.join(&name);
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copy)
            .map_err(|why| format!("purlis could not copy {name} for the upload: {why}"))?;
        copied(file, &mut out, size).map_err(|why| format!("{name}: {why}"))?;
        copies.push(copy);
    }
    Ok(copies)
}

/// **Writes `from`, a download in the browser's folder, under `root` at `to`** (relative to it),
/// as a new file: its folders must be there, and none of them, nor the file, may be a link.
/// Answers the bytes written. Never over a file that is there.
pub fn bring(from: &Path, root: &Path, to: &str, kept: &Kept) -> Result<u64, String> {
    let names = relative(to)?;
    kept.refuses_above(root)?;
    let source =
        File::open(from).map_err(|why| format!("the download could not be read: {why}"))?;
    let size = source
        .metadata()
        .map_err(|why| format!("the download could not be read: {why}"))?
        .len();
    let mut out = create_beneath(root, &names, kept).map_err(|why| format!("{to}: {why}"))?;
    copied(source, &mut out, size)?;
    Ok(size)
}

/// `bytes`, as a person reads a size.
pub fn bytes_said(bytes: u64) -> String {
    const KIB: u64 = 1 << 10;
    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;
    match bytes {
        b if b >= GIB => format!("{:.1} GB", b as f64 / GIB as f64),
        b if b >= MIB => format!("{:.1} MB", b as f64 / MIB as f64),
        b if b >= KIB => format!("{:.1} KB", b as f64 / KIB as f64),
        b => format!("{b} bytes"),
    }
}

/// Copies `size` bytes of `from` into `to`, and no more: a file that grew meanwhile is cut at
/// what was checked.
fn copied(from: File, to: &mut File, size: u64) -> Result<(), String> {
    let mut limited = from.take(size);
    std::io::copy(&mut limited, to).map_err(|why| format!("it could not be copied: {why}"))?;
    to.flush()
        .map_err(|why| format!("it could not be copied: {why}"))
}

fn private_folder(folder: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        purlis_same_user::private_directory(folder)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(folder)
    }
}

#[cfg(unix)]
fn open_beneath(root: &Path, names: &[String], kept: &Kept) -> Result<(File, u64), String> {
    use rustix::fs::{FileType, Mode, OFlags};

    let (last, folders) = names.split_last().ok_or("it names no file")?;
    let folder = walk(root, folders, kept)?;
    let fd = rustix::fs::openat(
        &folder,
        last.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|why| opened_said(why, "the file"))?;
    let stat = rustix::fs::fstat(&fd).map_err(|why| format!("it could not be read: {why}"))?;
    if kept.holds(stat_id(&stat)) {
        return Err(KEPT.into());
    }
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err("it is not a plain file".into());
    }
    if stat.st_nlink != 1 {
        return Err(
            "it has more than one name (a hard link), so purlis cannot tell it is the chat's to \
             read"
                .into(),
        );
    }
    let size = u64::try_from(stat.st_size).unwrap_or(0);
    Ok((File::from(fd), size))
}

#[cfg(unix)]
fn create_beneath(root: &Path, names: &[String], kept: &Kept) -> Result<File, String> {
    use rustix::fs::{Mode, OFlags};

    let (last, folders) = names.split_last().ok_or("it names no file")?;
    let folder = walk(root, folders, kept)?;
    let fd = rustix::fs::openat(
        &folder,
        last.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o644),
    )
    .map_err(|why| match why {
        rustix::io::Errno::EXIST => "there is a file there already; give another name".to_owned(),
        other => opened_said(other, "the file"),
    })?;
    Ok(File::from(fd))
}

/// `root` opened, then each of `folders` from it, following no link, each refused where it is
/// a folder purlis keeps, as the descriptor opened says.
#[cfg(unix)]
fn walk(root: &Path, folders: &[String], kept: &Kept) -> Result<rustix::fd::OwnedFd, String> {
    use rustix::fs::{Mode, OFlags};

    let dir = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let mut at = rustix::fs::open(root, dir, Mode::empty())
        .map_err(|why| format!("the chat's folder could not be opened: {why}"))?;
    let refuse_kept = |fd: &rustix::fd::OwnedFd| -> Result<(), String> {
        let stat = rustix::fs::fstat(fd).map_err(|why| format!("it could not be read: {why}"))?;
        if kept.holds(stat_id(&stat)) {
            return Err(KEPT.to_owned());
        }
        Ok(())
    };
    refuse_kept(&at)?;
    for name in folders {
        at = rustix::fs::openat(&at, name.as_str(), dir | OFlags::NOFOLLOW, Mode::empty())
            .map_err(|why| opened_said(why, name))?;
        refuse_kept(&at)?;
    }
    Ok(at)
}

/// A file's device and inode, as `fstat` said them.
#[cfg(unix)]
#[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
fn stat_id(stat: &rustix::fs::Stat) -> (u64, u64) {
    (stat.st_dev as u64, stat.st_ino as u64)
}

#[cfg(unix)]
fn opened_said(why: rustix::io::Errno, what: &str) -> String {
    match why {
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => format!(
            "{what} is a link, or reached through one, and purlis follows no link out of the \
             chat's folder"
        ),
        rustix::io::Errno::NOENT => format!("{what} is not there"),
        other => format!("{what} could not be opened: {other}"),
    }
}

#[cfg(not(unix))]
fn open_beneath(_root: &Path, _names: &[String], _kept: &Kept) -> Result<(File, u64), String> {
    Err("uploads from a chat's folder are not made on this system".into())
}

#[cfg(not(unix))]
fn create_beneath(_root: &Path, _names: &[String], _kept: &Kept) -> Result<File, String> {
    Err("downloads are not brought into a chat's folder on this system".into())
}

#[cfg(test)]
mod tests;

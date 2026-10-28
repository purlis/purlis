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
//! - **A download is brought in as a new file** ([`bring`]): written under the chat's folder at
//!   a relative path, its folders walked the same way, never over a file or a link that is
//!   there, and never anywhere the caller's own rules refuse (the window checks the chat's read
//!   and write rules before it asks).

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The most bytes one uploaded file may hold.
pub const MOST_UPLOAD_BYTES: u64 = 50 << 20;

/// The most files one upload may put into a file input.
pub const MOST_UPLOAD_FILES: usize = 20;

/// The most bytes one upload may hold, every file's together.
pub const MOST_UPLOAD_TOTAL: u64 = 100 << 20;

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
pub fn stage(root: &Path, files: &[String], into: &Path) -> Result<Vec<PathBuf>, String> {
    if files.is_empty() {
        return Err("give `files`, at least one path in the chat's folder".into());
    }
    if files.len() > MOST_UPLOAD_FILES {
        return Err(format!(
            "an upload takes at most {MOST_UPLOAD_FILES} files; this one gave {}",
            files.len()
        ));
    }
    let mut opened = Vec::new();
    let mut total: u64 = 0;
    for written in files {
        let names = relative(written)?;
        let (file, size) = open_beneath(root, &names).map_err(|why| format!("{written}: {why}"))?;
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
pub fn bring(from: &Path, root: &Path, to: &str) -> Result<u64, String> {
    let names = relative(to)?;
    let source =
        File::open(from).map_err(|why| format!("the download could not be read: {why}"))?;
    let size = source
        .metadata()
        .map_err(|why| format!("the download could not be read: {why}"))?
        .len();
    let mut out = create_beneath(root, &names).map_err(|why| format!("{to}: {why}"))?;
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
fn open_beneath(root: &Path, names: &[String]) -> Result<(File, u64), String> {
    use rustix::fs::{FileType, Mode, OFlags};

    let (last, folders) = names.split_last().ok_or("it names no file")?;
    let folder = walk(root, folders)?;
    let fd = rustix::fs::openat(
        &folder,
        last.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|why| opened_said(why, "the file"))?;
    let stat = rustix::fs::fstat(&fd).map_err(|why| format!("it could not be read: {why}"))?;
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
fn create_beneath(root: &Path, names: &[String]) -> Result<File, String> {
    use rustix::fs::{Mode, OFlags};

    let (last, folders) = names.split_last().ok_or("it names no file")?;
    let folder = walk(root, folders)?;
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

/// `root` opened, then each of `folders` from it, following no link.
#[cfg(unix)]
fn walk(root: &Path, folders: &[String]) -> Result<rustix::fd::OwnedFd, String> {
    use rustix::fs::{Mode, OFlags};

    let dir = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let mut at = rustix::fs::open(root, dir, Mode::empty())
        .map_err(|why| format!("the chat's folder could not be opened: {why}"))?;
    for name in folders {
        at = rustix::fs::openat(&at, name.as_str(), dir | OFlags::NOFOLLOW, Mode::empty())
            .map_err(|why| opened_said(why, name))?;
    }
    Ok(at)
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
fn open_beneath(_root: &Path, _names: &[String]) -> Result<(File, u64), String> {
    Err("uploads from a chat's folder are not made on this system".into())
}

#[cfg(not(unix))]
fn create_beneath(_root: &Path, _names: &[String]) -> Result<File, String> {
    Err("downloads are not brought into a chat's folder on this system".into())
}

#[cfg(test)]
mod tests;

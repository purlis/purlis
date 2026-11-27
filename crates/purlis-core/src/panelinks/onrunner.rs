// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A file a chat on this host printed, read for the window** (#6387, session protocol 1.93's
//! `read_file`): the runner's half of a runner chat's file link, whose desktop's half is
//! `panelinks::ondesktop` ([`super::RunnerFile`]). The desktop sends the path as
//! the chat printed it, as data; this reads it here, on the machine whose file it names, and
//! never builds a shell line or starts a program from it.
//!
//! - **Where it starts.** A relative path is joined to the chat's own folder; an absolute one is
//!   taken as written; `~` is never looked up, since this host's home holds its sign-ins and
//!   nothing a chat should point the person at without the path in full. A control character
//!   anywhere is no path.
//! - **Every link is resolved here, then the result is judged** (the rule #6383 left for this):
//!   the path is resolved on this side, the side that owns the file, and only the resolved path
//!   is held to the chat's workspace, so `..` and a link that leads out are outside, and one
//!   that stays in is followed.
//! - **Some folders are never looked in** (#7076), as the host's own hover never looks in them
//!   ([`Asked::guarded`]: removable, network and automounted volumes): a path into one, or a
//!   link that leads into one, is not there, unless it is in the chat's project or folder.
//! - **The workspace** is the chat's workspace folder in its project, or the project for a chat
//!   at its root, as the desktop counts a chat's home (`super::home_of`).
//! - **Outside it is not read** unless the person answered an ask for it
//!   ([`FileReach::Anywhere`]): it is answered [`FileRead::Outside`].
//! - **Only a plain file is read**: never a folder, a device, a pipe or a socket, and nothing
//!   under `/proc`, `/sys` or `/dev`, whatever it says it is. Its type is looked at before it is
//!   opened, it is opened one folder at a time with no link followed (so a folder swapped for
//!   a link after the path was resolved is refused, not followed), without waiting on a pipe,
//!   and looked at again once open.
//! - **What the runner keeps is never read** ([`kept`], #7064): purlis's own folders there,
//!   every harness's sign-in files, and what no chat there reads, whatever the reach. Judged
//!   on the resolved path, and again on the file opened, before a byte of it is read: it is
//!   answered [`FileRead::Kept`].
//! - **At most [`MOST_FILE_READ`] bytes of text**: a larger file, or one with a NUL byte or
//!   that is not UTF-8, is answered by its size alone. Only what is asked is read (#7078): a
//!   look reads the file's first [`MOST_FILE_LOOKED`] bytes to tell text, and a piece its own
//!   bytes alone, judged as text but for a character cut at its edges.

use std::io::{Read as _, Seek as _};
use std::path::{Component, Path, PathBuf};

use purlis_session_protocol::session::{
    FileHome, FilePiece, FileReach, FileRead, MOST_FILE_PIECE, MOST_FILE_READ,
};

/// **What the window asked to read**: the chat's project and folder here, the path as the chat
/// printed it, how far it may reach, and where to read from (`None` to look only).
pub struct Asked<'a> {
    pub project: &'a Path,
    pub cwd: Option<&'a Path>,
    pub path: &'a str,
    pub reach: FileReach,
    pub at: Option<u64>,
    /// The runner, as purlis knows its home and its environment: where its own folders and
    /// each harness's sign-in are ([`kept`]).
    pub machine: &'a crate::sandbox::Machine,
    /// The folders a hover never looks in there, as the host's own hover never does
    /// (`file::guarded_folders`, #7076): removable, network and automounted volumes.
    pub guarded: &'a [PathBuf],
}

/// **The most bytes a look reads** (#7078): what tells text from a binary at its start, as git
/// judges a file by its first 8000 bytes.
pub const MOST_FILE_LOOKED: u64 = 8 * 1024;

/// The folders of the system's own files, which are never read, whatever they say they are.
const SYSTEMS_OWN: [&str; 3] = ["/proc", "/sys", "/dev"];

/// **What `asked` names, as this host finds it**, and its bytes from `asked.at`, if any.
pub fn read_for_window(asked: &Asked<'_>) -> FileRead {
    read_checked(asked, &Probe::default())
}

/// What a test puts into a read: a step between what is judged and what is opened, and the
/// budget of the look for a file's other names.
struct Probe<'a> {
    /// Called with the resolved path once it is judged, before anything is opened.
    between: &'a dyn Fn(&Path),
    most: usize,
}

impl Default for Probe<'_> {
    fn default() -> Self {
        Self {
            between: &|_| {},
            most: crate::files::kept::WALKED,
        }
    }
}

fn read_checked(asked: &Asked<'_>, probe: &Probe<'_>) -> FileRead {
    let text = asked.path;
    if text.is_empty() || text.starts_with('~') || text.chars().any(char::is_control) {
        return FileRead::NotThere;
    }
    let written = Path::new(text);
    let joined = if written.is_absolute() {
        written.to_path_buf()
    } else {
        match asked.cwd {
            Some(cwd) => cwd.join(written),
            None => return FileRead::NotThere,
        }
    };
    // Walked as the host walks a path for its hover (#7076): every link followed on this side,
    // and never into a folder a hover never looks in, but the chat's project and folder.
    let allowed: Vec<PathBuf> = std::iter::once(asked.project)
        .chain(asked.cwd)
        .filter_map(|folder| std::fs::canonicalize(folder).ok())
        .collect();
    // Then named as the disk names it, so a name in another case on a disk that folds case is
    // the file it opens, placed where that file is.
    let Some(resolved) = super::file::walked(&joined, asked.guarded, &allowed)
        .and_then(|walked| std::fs::canonicalize(walked).ok())
    else {
        return FileRead::NotThere;
    };
    let Some(path) = resolved.to_str().map(str::to_owned) else {
        return FileRead::NotThere;
    };
    if SYSTEMS_OWN.iter().any(|own| resolved.starts_with(own)) {
        return FileRead::NotPlain { path };
    }
    let kept = kept::Kept::of(asked.project, asked.machine, probe.most);
    match kept.holds(&resolved) {
        kept::Held::No => {}
        kept::Held::Kept => return FileRead::Kept { path },
        kept::Held::Unsure => return FileRead::Unsure { path },
    }
    // The folders as the walk found them, to hold what is opened to them.
    let walked: Vec<_> = resolved.ancestors().skip(1).map(kept::id_of_path).collect();
    let home = home_of(asked);
    let from_workspace = home
        .as_ref()
        .and_then(|(_, workspace)| slashed(resolved.strip_prefix(workspace).ok()?));
    if asked.reach == FileReach::Workspace && from_workspace.is_none() {
        return FileRead::Outside { path };
    }
    if !std::fs::symlink_metadata(&resolved).is_ok_and(|meta| meta.is_file()) {
        return FileRead::NotPlain { path };
    }
    (probe.between)(&resolved);
    let Ok(file) = open_plain(&resolved, &walked) else {
        return FileRead::NotThere;
    };
    let Ok(meta) = file.metadata() else {
        return FileRead::NotThere;
    };
    if !meta.is_file() {
        return FileRead::NotPlain { path };
    }
    match kept.holds_the_file(&meta) {
        kept::Held::No => {}
        kept::Held::Kept => return FileRead::Kept { path },
        kept::Held::Unsure => return FileRead::Unsure { path },
    }
    let size = meta.len();
    if size > MOST_FILE_READ {
        return FileRead::TooLarge { path, size };
    }
    // Only what is asked is read (#7078): a look, the file's first bytes, to tell text; a
    // piece, its own bytes from where it was asked.
    let (from, most) = match asked.at {
        None => (0, MOST_FILE_LOOKED),
        Some(at) => (at.min(size), MOST_FILE_PIECE),
    };
    let mut file = file;
    if from > 0 && file.seek(std::io::SeekFrom::Start(from)).is_err() {
        return FileRead::NotThere;
    }
    let mut bytes = Vec::new();
    if file.take(most).read_to_end(&mut bytes).is_err() {
        return FileRead::NotThere;
    }
    let to_its_end = from + bytes.len() as u64 >= size;
    if !reads_as_text(&bytes, from > 0, !to_its_end) {
        return FileRead::Binary { path, size };
    }
    let piece = asked.at.map(|at| FilePiece { at, bytes });
    FileRead::Plain {
        path,
        from_workspace,
        home: home.and_then(|(workspace, folder)| {
            Some(FileHome {
                workspace,
                folder: folder.to_str()?.to_owned(),
            })
        }),
        size,
        version: version_of(&meta, size),
        piece,
    }
}

/// **Whether `bytes`, a stretch of a file, reads as text**: no NUL, and UTF-8 but for a
/// character cut at either edge, where the stretch starts after the file's start
/// (`cut_at_start`) or ends before its end (`cut_at_end`).
fn reads_as_text(bytes: &[u8], cut_at_start: bool, cut_at_end: bool) -> bool {
    if bytes.contains(&0) {
        return false;
    }
    let mut bytes = bytes;
    if cut_at_start {
        // A character's later bytes, at most three, whose first byte is before the stretch.
        let later = bytes
            .iter()
            .take(3)
            .take_while(|byte| **byte & 0b1100_0000 == 0b1000_0000)
            .count();
        bytes = &bytes[later..];
    }
    match std::str::from_utf8(bytes) {
        Ok(_) => true,
        // A character whose last bytes are after the stretch.
        Err(cut) => cut_at_end && cut.error_len().is_none(),
    }
}

/// **The chat's workspace here, resolved**: its workspace's name and folder in its project, or
/// no name and the project for a chat at its root or with no folder.
fn home_of(asked: &Asked<'_>) -> Option<(Option<String>, PathBuf)> {
    let project = std::fs::canonicalize(asked.project).ok()?;
    let in_a_workspace = asked
        .cwd
        .and_then(|cwd| std::fs::canonicalize(cwd).ok())
        .and_then(|cwd| crate::active::workspace_of_tree(&project, &cwd));
    Some(match in_a_workspace {
        Some(ws) => {
            let folder = std::fs::canonicalize(project.join("workspaces").join(&ws)).ok()?;
            (Some(ws), folder)
        }
        None => (None, project),
    })
}

/// `path`'s parts with `/` between them, `""` for none, or `None` where one is not text.
fn slashed(path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = path
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    Some(parts?.join("/"))
}

/// **`path` (resolved, so holding no link) opened one folder at a time with no link
/// followed**, without waiting on a pipe and never becoming its terminal: a part swapped for a
/// link since the path was resolved is refused. Each folder opened is held to the folder the
/// walk found there (`walked`, the identities of the folders above the file, nearest first),
/// so a folder swapped for another between the walk and the open is refused as well.
#[cfg(unix)]
fn open_plain(
    path: &Path,
    walked: &[Option<crate::files::kept::Id>],
) -> std::io::Result<std::fs::File> {
    use crate::files::kept::id_of;
    use rustix::fs::{CWD, Mode, OFlags, openat};

    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => parts.push(name),
            _ => return Err(std::io::ErrorKind::InvalidInput.into()),
        }
    }
    let Some((last, folders)) = parts.split_last() else {
        return Err(std::io::ErrorKind::InvalidInput.into());
    };
    let folder_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    // `walked` runs from the folder holding the file up to the root: the root, then each
    // folder down, is its reverse.
    let mut expected = walked.iter().rev();
    let mut at = std::fs::File::from(openat(CWD, "/", folder_flags, Mode::empty())?);
    for folder in folders {
        if at.metadata().ok().as_ref().and_then(id_of) != expected.next().copied().flatten() {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        at = std::fs::File::from(openat(&at, *folder, folder_flags, Mode::empty())?);
    }
    if at.metadata().ok().as_ref().and_then(id_of) != expected.next().copied().flatten() {
        return Err(std::io::ErrorKind::NotFound.into());
    }
    let file_flags =
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC;
    Ok(std::fs::File::from(openat(
        &at,
        *last,
        file_flags,
        Mode::empty(),
    )?))
}

#[cfg(not(unix))]
fn open_plain(
    path: &Path,
    _walked: &[Option<crate::files::kept::Id>],
) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

/// **What names the file as it is now**: its device, its number there, its size and when it
/// last changed, so a read in pieces can tell it read one file.
#[cfg(unix)]
fn version_of(meta: &std::fs::Metadata, size: u64) -> String {
    use std::os::unix::fs::MetadataExt as _;
    format!(
        "{}:{}:{size}:{}.{}",
        meta.dev(),
        meta.ino(),
        meta.mtime(),
        meta.mtime_nsec()
    )
}

/// **Which file a [`version_of`] names**, whatever its size and time: its device and its
/// number there. `None` for a version that does not hold them.
pub fn identity_of(version: &str) -> Option<&str> {
    let parts: Vec<&str> = version.split(':').collect();
    if parts.len() != 4 || parts[..2].iter().any(|part| part.is_empty()) {
        return None;
    }
    version.get(..parts[0].len() + 1 + parts[1].len())
}

#[cfg(not(unix))]
fn version_of(meta: &std::fs::Metadata, size: u64) -> String {
    let changed = meta
        .modified()
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    format!("{size}:{changed}")
}

#[path = "onrunner_kept.rs"]
mod kept;

#[cfg(test)]
#[path = "onrunner_tests.rs"]
mod tests;

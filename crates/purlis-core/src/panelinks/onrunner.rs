// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A file a chat on this host printed, read for the window** (#6387, session protocol 1.92's
//! `read_file`): the runner's half of a runner chat's file link. The desktop sends the path as
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
//! - **The workspace** is the chat's workspace folder in its project, or the project for a chat
//!   at its root, as the desktop counts a chat's home (`super::home_of`).
//! - **Outside it is not read** unless the person answered an ask for it
//!   ([`FileReach::Anywhere`]): it is answered [`FileRead::Outside`].
//! - **Only a plain file is read**: never a folder, a device, a pipe or a socket, and nothing
//!   under `/proc`, `/sys` or `/dev`, whatever it says it is. Its type is looked at before it is
//!   opened, it is opened one folder at a time with no link followed (so a folder swapped for
//!   a link after the path was resolved is refused, not followed), without waiting on a pipe,
//!   and looked at again once open.
//! - **At most [`MOST_FILE_READ`] bytes of text**: a larger file, or one with a NUL byte or
//!   that is not UTF-8, is answered by its size alone.

use std::io::Read as _;
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
}

/// The folders of the system's own files, which are never read, whatever they say they are.
const SYSTEMS_OWN: [&str; 3] = ["/proc", "/sys", "/dev"];

/// **What `asked` names, as this host finds it**, and its bytes from `asked.at`, if any.
pub fn read_for_window(asked: &Asked<'_>) -> FileRead {
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
    let Ok(resolved) = std::fs::canonicalize(&joined) else {
        return FileRead::NotThere;
    };
    let Some(path) = resolved.to_str().map(str::to_owned) else {
        return FileRead::NotThere;
    };
    if SYSTEMS_OWN.iter().any(|own| resolved.starts_with(own)) {
        return FileRead::NotPlain { path };
    }
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
    let Ok(file) = open_plain(&resolved) else {
        return FileRead::NotThere;
    };
    let Ok(meta) = file.metadata() else {
        return FileRead::NotThere;
    };
    if !meta.is_file() {
        return FileRead::NotPlain { path };
    }
    let size = meta.len();
    if size > MOST_FILE_READ {
        return FileRead::TooLarge { path, size };
    }
    let mut bytes = Vec::new();
    if file
        .take(MOST_FILE_READ + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return FileRead::NotThere;
    }
    let read = bytes.len() as u64;
    if read > MOST_FILE_READ {
        return FileRead::TooLarge { path, size: read };
    }
    if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
        return FileRead::Binary { path, size: read };
    }
    let piece = asked.at.map(|at| {
        let from = usize::try_from(at.min(read)).unwrap_or(bytes.len());
        let to =
            usize::try_from(at.saturating_add(MOST_FILE_PIECE).min(read)).unwrap_or(bytes.len());
        FilePiece {
            at,
            bytes: bytes[from..to].to_vec(),
        }
    });
    FileRead::Plain {
        path,
        from_workspace,
        home: home.and_then(|(workspace, folder)| {
            Some(FileHome {
                workspace,
                folder: folder.to_str()?.to_owned(),
            })
        }),
        size: read,
        version: version_of(&meta, read),
        piece,
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
/// link since the path was resolved is refused.
#[cfg(unix)]
fn open_plain(path: &Path) -> std::io::Result<std::fs::File> {
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
    let mut at = openat(CWD, "/", folder_flags, Mode::empty())?;
    for folder in folders {
        at = openat(&at, *folder, folder_flags, Mode::empty())?;
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
fn open_plain(path: &Path) -> std::io::Result<std::fs::File> {
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

#[cfg(test)]
#[path = "onrunner_tests.rs"]
mod tests;

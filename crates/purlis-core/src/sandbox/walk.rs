//! **The session host's walks of a profile's folders**, which run outside any wrap while another
//! chat on the same profile may be moving folders under them. Every folder is opened relative to
//! the one above it, by a descriptor, with `O_NOFOLLOW`, and every name is judged and removed
//! relative to the folder it was listed in, so a folder swapped for another in the middle of a
//! walk, or a name swapped for a link, never leads the walk out of the folder it began in: it is
//! only ever told nothing, or a name it may already remove.

use std::ffi::{CStr, CString};
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{AtFlags, FileType, Mode, OFlags};

/// A folder opened for a walk: never a link at its own name, never anything but a folder.
const FOLDER: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// The folder at `rel` below `top`: `top` resolved as named, as the person or purlis named it,
/// and every name below it opened from the folder above it, none through a link.
pub(super) fn open(top: &Path, rel: &Path) -> Option<OwnedFd> {
    let mut at = rustix::fs::open(
        top,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    for part in rel.iter() {
        at = rustix::fs::openat(&at, part, FOLDER, Mode::empty()).ok()?;
    }
    Some(at)
}

/// The names in the folder open at `dir`, but `.` and `..`, each with its type as found, a link
/// as a link. Listed whole before the walk changes the folder.
pub(super) fn listed(dir: &OwnedFd) -> Vec<(CString, FileType)> {
    let Ok(read) = rustix::fs::Dir::read_from(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            if name == c"." || name == c".." {
                return None;
            }
            let stat = rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
            Some((name.to_owned(), FileType::from_raw_mode(stat.st_mode)))
        })
        .collect()
}

/// The folder `name` in the folder open at `dir`, opened from it. Where `name` is no folder by
/// then, nothing; and where it is a link by then, the link is taken out, which every walk here
/// does to a link.
pub(super) fn descend(dir: &OwnedFd, name: &CStr) -> Option<OwnedFd> {
    match rustix::fs::openat(dir, name, FOLDER, Mode::empty()) {
        Ok(below) => Some(below),
        Err(_) => {
            unlink_if_a_link(dir, name);
            None
        }
    }
}

/// `name` in the folder open at `dir` taken out if it is a link.
pub(super) fn unlink_if_a_link(dir: &OwnedFd, name: &CStr) {
    let linked = rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)
        .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::Symlink);
    if linked {
        let _ = rustix::fs::unlinkat(dir, name, AtFlags::empty());
    }
}

/// The folder `name` in the folder open at `dir`, with everything it holds, taken out, each name
/// removed from the folder it was listed in. A link in it is taken out, never followed.
pub(super) fn remove_tree(dir: &OwnedFd, name: &CStr) {
    if let Some(below) = descend(dir, name) {
        for (inner, kind) in listed(&below) {
            if kind == FileType::Directory {
                remove_tree(&below, &inner);
            } else {
                let _ = rustix::fs::unlinkat(&below, inner.as_c_str(), AtFlags::empty());
            }
        }
    }
    let _ = rustix::fs::unlinkat(dir, name, AtFlags::REMOVEDIR);
}

/// Below the folder open at `dir`, every link taken out, never followed.
pub(super) fn no_links_below(dir: &OwnedFd) {
    for (name, kind) in listed(dir) {
        match kind {
            FileType::Symlink => {
                let _ = rustix::fs::unlinkat(dir, name.as_c_str(), AtFlags::empty());
            }
            FileType::Directory => {
                if let Some(below) = descend(dir, &name) {
                    no_links_below(&below);
                }
            }
            _ => {}
        }
    }
}

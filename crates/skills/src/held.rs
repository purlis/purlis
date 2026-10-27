// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Folders held open and read by name through their handle, never through a link** (SK-36,
//! #3017): the one way purlis reads a skill's folder, its `SKILL.md` and the files beside it, and
//! the one folder-handle reader of the workspace (SK-52, #3205): purlis-core lists a harness's
//! configs and plugin folders, and the folders of skills from outside the plane, through it too.
//!
//! **Following a link is the caller's choice at each step** ([`Follow`]): a folder a chat can
//! write is read with no link followed, and the person's own folders (their skills, opencode's
//! global directory) the way their harness reads them. **Each folder's names are read whole or
//! not at all**, at most [`MOST_NAMES`] of them ([`Folder::names`]).
//!
//! A chat can write the plane's skills, so purlis never asks about a path and then opens it
//! again. A level's folder is reached from a folder purlis trusts (the plane's root, the bundle's
//! plugin folder) one part at a time, each opened through the handle of the one above without
//! following a link, and everything in it is opened the same way: a link put in place after a
//! look is never read through. Off unix, by path, with the same answers where nothing changes in
//! between, and the same check that the file read is the file looked at.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

/// Why [`read_plain`] or [`Folder::read`] read nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotRead {
    /// It could not be read: what the system said.
    Unreadable(String),
    /// It is a link, a hard link (a file with another name too), or no plain file, or it changed
    /// into one while it was opened.
    NotPlain,
    /// It is larger than the most asked for.
    TooLarge,
}

/// Whether a file with a second name (a hard link) is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardLinks {
    /// Refused: it reaches past its folder as a link does. A folder a chat can write.
    Refused,
    /// Read: the person's own folders, where the Nix store, `cp -al` and `rsync --link-dest`
    /// leave them (SK-44).
    Read,
}

/// The end of a sentence about the file: "it cannot be read (…)", "it is not a plain file", "it
/// is too large". The one way a file [`read_plain`] read nothing is said.
impl std::fmt::Display for NotRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "it cannot be read ({why})"),
            Self::NotPlain => f.write_str("it is not a plain file"),
            Self::TooLarge => f.write_str("it is too large"),
        }
    }
}

/// What a name in a [`Folder`] is, never following a link to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Folder,
    File,
    /// A link, and whether it leads to a folder.
    Link {
        to_folder: bool,
    },
    Other,
}

/// Whether a step opens a folder through a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Follow {
    /// Through a link, as the person's own harness reads their folders.
    Link,
    /// Never through a link: a folder a chat can write. A link there is [`Opened::Link`].
    NoLink,
}

/// **The most names purlis reads of one folder** (SK-44, #3133; SK-52, #3205): a clone's skills
/// folder, a plane level and a project's plugin folder are a chat's to fill, and each name read
/// is opened, read and linted. A folder holding more is read for none of them ([`NotListed`]).
pub const MOST_NAMES: usize = 2000;

/// Why a folder's names are not listed whole ([`Folder::names`]): nothing of it is read then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotListed {
    /// It holds more than [`MOST_NAMES`].
    Crowded,
    /// Reading it failed, at its start or partway.
    Unreadable,
}

/// What is said of a folder purlis did not list: "a folder of more than 2000 names, the most
/// purlis reads of one", or "a folder purlis could not read whole".
impl std::fmt::Display for NotListed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Crowded => write!(
                f,
                "a folder of more than {MOST_NAMES} names, the most purlis reads of one"
            ),
            Self::Unreadable => f.write_str("a folder purlis could not read whole"),
        }
    }
}

/// **A folder's names, whole or not at all**, by name: [`NotListed::Crowded`] past
/// [`MOST_NAMES`], read no further than one past them, and [`NotListed::Unreadable`] where an
/// entry could not be read, so a short list is never taken for the whole.
fn names_whole<E>(
    names: impl Iterator<Item = Result<OsString, E>>,
) -> Result<Vec<OsString>, NotListed> {
    let mut out = Vec::new();
    for name in names {
        let name = name.map_err(|_| NotListed::Unreadable)?;
        if name == "." || name == ".." {
            continue;
        }
        if out.len() == MOST_NAMES {
            return Err(NotListed::Crowded);
        }
        out.push(name);
    }
    out.sort();
    Ok(out)
}

/// What opening a folder came to.
pub enum Opened {
    Folder(Folder),
    /// A link where a folder should be: never opened through.
    Link,
    /// Nothing there, or nothing that opens as a folder.
    Absent,
}

/// Whether `path` is a folder of its own: not a link to one. The one such question in this crate.
pub(crate) fn is_plain_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

/// The parts of `rel`, each one plain name: `None` where it has a root, a `..` or a prefix,
/// which no level of the plane's is reached by.
fn parts(rel: &Path) -> Option<Vec<&OsStr>> {
    rel.components()
        .filter(|part| *part != Component::CurDir)
        .map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect()
}

/// **The text of `file`, opened as the plain file `meta` says it is**, of at most `most` bytes:
/// refused where it is no plain file, has a second name (a hard link, which reaches past its
/// folder as a link does) and `hard_links` refuses it, or is larger. The one reader under
/// [`read_plain`] and [`Folder::read`].
fn read_opened(file: std::fs::File, most: u64, hard_links: HardLinks) -> Result<String, NotRead> {
    use std::io::Read as _;
    let unreadable = |why: std::io::Error| NotRead::Unreadable(why.to_string());
    let meta = file.metadata().map_err(unreadable)?;
    if !meta.is_file() {
        return Err(NotRead::NotPlain);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if meta.nlink() > 1 && hard_links == HardLinks::Refused {
            return Err(NotRead::NotPlain);
        }
    }
    #[cfg(not(unix))]
    let _ = hard_links;
    if meta.len() > most {
        return Err(NotRead::TooLarge);
    }
    let mut text = String::new();
    file.take(most + 1)
        .read_to_string(&mut text)
        .map_err(unreadable)?;
    if text.len() as u64 > most {
        return Err(NotRead::TooLarge);
    }
    Ok(text)
}

/// **The text of the plain file at `path`, following no link at its last step**, of at most
/// `most` bytes: refused where it is a link or no plain file, is larger, or changed from a plain
/// file into something else while it was opened. For a file whose folder purlis holds no handle
/// to (the file beside a skill's folder); a skill's own files are read through [`Folder::read`].
pub(crate) fn read_plain(path: &Path, most: u64) -> Result<String, NotRead> {
    let unreadable = |why: std::io::Error| NotRead::Unreadable(why.to_string());
    let meta = std::fs::symlink_metadata(path).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(NotRead::NotPlain);
    }
    if meta.len() > most {
        return Err(NotRead::TooLarge);
    }
    let file = std::fs::File::open(path).map_err(unreadable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let opened = file.metadata().map_err(unreadable)?;
        if (opened.dev(), opened.ino()) != (meta.dev(), meta.ino()) {
            return Err(NotRead::NotPlain);
        }
    }
    read_opened(file, most, HardLinks::Refused)
}

/// **A folder held open, and the path it is shown at**: everything in it is opened relative to
/// that one handle, so a link put in place after it was opened is never read through.
pub struct Folder {
    #[cfg(unix)]
    fd: rustix::fd::OwnedFd,
    shown: PathBuf,
}

impl Folder {
    /// The path it is shown at: where a skill's files are said to be.
    pub fn shown(&self) -> &Path {
        &self.shown
    }

    /// **The folder at `path`**, through a link at its last step only where `follow` says, and
    /// as it is above it: for a folder whose path purlis trusts up to its last part (a
    /// workspace's folder, a clone, the person's own folders). `None` where nothing there opens
    /// as a folder so.
    pub fn open(path: &Path, follow: Follow) -> Option<Self> {
        Self::open_at(path, follow)
    }

    /// **`rel` under `root`**: `root` opened as it is (the plane's root, the bundle's plugin
    /// folder, which may sit under links of the person's own), then each part of `rel` through
    /// the handle of the one above, never following a link. [`Opened::Link`] where any part is
    /// one.
    pub fn walk(root: &Path, rel: &Path) -> Opened {
        let Some(parts) = parts(rel) else {
            return Opened::Absent;
        };
        let Some(mut at) = Self::open_at(root, Follow::Link) else {
            return Opened::Absent;
        };
        for part in parts {
            at = match at.child(part, Follow::NoLink) {
                Opened::Folder(next) => next,
                other => return other,
            };
        }
        Opened::Folder(at)
    }

    /// **Every name in it but `.` and `..`, by name**, whole or not at all: at most
    /// [`MOST_NAMES`] ([`NotListed`]).
    pub fn names(&self) -> Result<Vec<OsString>, NotListed> {
        self.listed()
    }

    /// **The child folder `name`**, through a link only where `follow` says: [`Opened::Link`]
    /// where it is a link and none is followed.
    pub fn child(&self, name: impl AsRef<OsStr>, follow: Follow) -> Opened {
        let name = name.as_ref();
        match self.open_child(name, follow) {
            Some(fd) => Opened::Folder(fd),
            None if follow == Follow::NoLink
                && matches!(self.kind(name), Some(Kind::Link { .. })) =>
            {
                Opened::Link
            }
            None => Opened::Absent,
        }
    }

    /// **The text of the plain file `name` in it**, of at most `most` bytes, never through a
    /// link, and a hard link refused ([`Self::read_as`]).
    pub fn read(&self, name: impl AsRef<OsStr>, most: u64) -> Result<String, NotRead> {
        self.read_as(name, most, HardLinks::Refused)
    }
}

#[cfg(unix)]
impl Folder {
    fn folder_flags(follow: bool) -> rustix::fs::OFlags {
        use rustix::fs::OFlags;
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        if follow {
            flags
        } else {
            flags | OFlags::NOFOLLOW
        }
    }

    fn open_at(root: &Path, follow: Follow) -> Option<Self> {
        rustix::fs::open(
            root,
            Self::folder_flags(follow == Follow::Link),
            rustix::fs::Mode::empty(),
        )
        .ok()
        .map(|fd| Self {
            fd,
            shown: root.to_path_buf(),
        })
    }

    fn open_child(&self, name: &OsStr, follow: Follow) -> Option<Self> {
        rustix::fs::openat(
            &self.fd,
            name,
            Self::folder_flags(follow == Follow::Link),
            rustix::fs::Mode::empty(),
        )
        .ok()
        .map(|fd| Self {
            fd,
            shown: self.shown.join(name),
        })
    }

    fn listed(&self) -> Result<Vec<OsString>, NotListed> {
        use std::os::unix::ffi::OsStrExt as _;
        let Ok(dir) = rustix::fs::Dir::read_from(&self.fd) else {
            return Err(NotListed::Unreadable);
        };
        names_whole(dir.map(|entry| {
            entry.map(|entry| OsStr::from_bytes(entry.file_name().to_bytes()).to_owned())
        }))
    }

    /// What `name` is, never following a link to say.
    pub fn kind(&self, name: impl AsRef<OsStr>) -> Option<Kind> {
        use rustix::fs::{AtFlags, FileType};
        let name = name.as_ref();
        let of = |flags| {
            rustix::fs::statat(&self.fd, name, flags)
                .ok()
                .map(|stat| FileType::from_raw_mode(stat.st_mode))
        };
        Some(match of(AtFlags::SYMLINK_NOFOLLOW)? {
            FileType::Directory => Kind::Folder,
            FileType::RegularFile => Kind::File,
            FileType::Symlink => Kind::Link {
                to_folder: of(AtFlags::empty()) == Some(FileType::Directory),
            },
            _ => Kind::Other,
        })
    }

    /// **The text of the plain file `name` in it**, of at most `most` bytes, never through a
    /// link, a hard link read or refused as `hard_links` says ([`read_opened`]).
    pub fn read_as(
        &self,
        name: impl AsRef<OsStr>,
        most: u64,
        hard_links: HardLinks,
    ) -> Result<String, NotRead> {
        use rustix::fs::OFlags;
        let name = name.as_ref();
        let opened = rustix::fs::openat(
            &self.fd,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        );
        match opened {
            Ok(fd) => read_opened(std::fs::File::from(fd), most, hard_links),
            Err(_) if matches!(self.kind(name), Some(Kind::Link { .. })) => Err(NotRead::NotPlain),
            Err(why) => Err(NotRead::Unreadable(why.to_string())),
        }
    }
}

#[cfg(not(unix))]
impl Folder {
    fn open_at(root: &Path, follow: Follow) -> Option<Self> {
        let opens = match follow {
            Follow::Link => root.is_dir(),
            Follow::NoLink => is_plain_dir(root),
        };
        opens.then(|| Self {
            shown: root.to_path_buf(),
        })
    }

    fn open_child(&self, name: &OsStr, follow: Follow) -> Option<Self> {
        Self::open_at(&self.shown.join(name), follow)
    }

    fn listed(&self) -> Result<Vec<OsString>, NotListed> {
        let Ok(entries) = std::fs::read_dir(&self.shown) else {
            return Err(NotListed::Unreadable);
        };
        names_whole(entries.map(|entry| entry.map(|entry| entry.file_name())))
    }

    pub fn kind(&self, name: impl AsRef<OsStr>) -> Option<Kind> {
        let path = self.shown.join(name.as_ref());
        let meta = std::fs::symlink_metadata(&path).ok()?;
        let kind = meta.file_type();
        Some(if kind.is_symlink() {
            Kind::Link {
                to_folder: path.is_dir(),
            }
        } else if kind.is_dir() {
            Kind::Folder
        } else if kind.is_file() {
            Kind::File
        } else {
            Kind::Other
        })
    }

    pub fn read_as(
        &self,
        name: impl AsRef<OsStr>,
        most: u64,
        _hard_links: HardLinks,
    ) -> Result<String, NotRead> {
        read_plain(&self.shown.join(name.as_ref()), most)
    }
}

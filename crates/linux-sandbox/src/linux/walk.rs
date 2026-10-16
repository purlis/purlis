// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A path the chat named, found the way the kernel would find it for the chat**, one name at
//! a time, from descriptors the supervisor holds.
//!
//! Each name is opened beneath the folder before it without following a link, so no step can
//! be swapped for a link between being found and being used: a link is read and its target
//! walked, `..` goes to the parent of the folder actually reached, and the operation that
//! follows is made on the descriptors the walk ends with. What is judged is where the walk
//! really arrived, as the kernel names it, never the text the chat wrote.
//!
//! The chat's own view is kept where the supervisor's would differ: a relative path starts at
//! the chat's working folder or the folder its descriptor names, `/proc/self` and
//! `/proc/thread-self` are the chat's, and a link inside `/proc` (a process's open file, its
//! working folder) is the kernel's to follow. A process folder in `/proc` is reached only for a
//! process inside the sandbox.

use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use nix::errno::Errno;
use nix::fcntl::{OFlag, openat, readlinkat};
use nix::sys::stat::{Mode, SFlag, fstat};

use crate::profile::Kind;

/// The most links a walk follows, as the kernel's own limit.
const MOST_LINKS: usize = 40;

/// Where a walk ended.
pub(super) enum Found {
    /// At something that exists: `fd` holds it (as a path, never opened for reading), and
    /// `entry` is its folder and name where it has one (not `/`, nor a `.` or `..` last).
    There {
        fd: OwnedFd,
        kind: Kind,
        entry: Option<(OwnedFd, OsString)>,
    },
    /// At a name that does not exist yet, in a folder that does.
    Missing { folder: OwnedFd, name: OsString },
}

/// The chat process a walk is for, and what the supervisor knows of the system.
pub(super) struct Chat<'a> {
    /// The thread that made the call.
    pub tid: i32,
    /// Its process.
    pub tgid: i32,
    pub world: &'a World,
}

/// What every walk shares: `/` as the supervisor holds it, and which processes are inside the
/// sandbox.
pub(super) struct World {
    pub root: OwnedFd,
    /// The supervisor's own process: every process inside the sandbox descends from it.
    pub supervisor: i32,
}

impl World {
    pub(super) fn new() -> Result<World, Errno> {
        let root = nix::fcntl::open("/", path_flags() | OFlag::O_DIRECTORY, Mode::empty())?;
        Ok(World {
            root,
            supervisor: nix::unistd::getpid().as_raw(),
        })
    }

    /// Whether `fd` is the top folder of a `/proc`, wherever it is mounted: inode 1 of a
    /// process file system.
    fn is_proc_root(&self, fd: impl AsFd) -> bool {
        in_proc(fd.as_fd()) && fstat(fd).is_ok_and(|st| st.st_ino == 1)
    }

    /// Whether process `pid` is inside the sandbox: below the supervisor, and not it.
    pub(super) fn inside(&self, pid: i32) -> bool {
        let mut at = pid;
        for _ in 0..4096 {
            if at == self.supervisor {
                return at != pid;
            }
            if at <= 1 {
                return false;
            }
            match parent(at) {
                Some(up) => at = up,
                None => return false,
            }
        }
        false
    }
}

/// The parent of process `pid`, from its status.
fn parent(pid: i32) -> Option<i32> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .and_then(|it| it.trim().parse().ok())
}

/// The thread group (process) of thread `tid`.
pub(super) fn tgid(tid: i32) -> Option<i32> {
    let status = std::fs::read_to_string(format!("/proc/{tid}/status")).ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Tgid:"))
        .and_then(|it| it.trim().parse().ok())
}

pub(super) fn path_flags() -> OFlag {
    OFlag::O_PATH | OFlag::O_CLOEXEC
}

/// What `fd` is.
pub(super) fn kind_of(fd: impl AsFd) -> Result<Kind, Errno> {
    Ok(kind(fstat(fd)?.st_mode))
}

pub(super) fn kind(mode: u32) -> Kind {
    match SFlag::from_bits_truncate(mode & SFlag::S_IFMT.bits()) {
        SFlag::S_IFDIR => Kind::Directory,
        SFlag::S_IFLNK => Kind::Symlink,
        SFlag::S_IFIFO => Kind::Fifo,
        SFlag::S_IFSOCK => Kind::Socket,
        SFlag::S_IFCHR => Kind::CharacterDevice,
        SFlag::S_IFBLK => Kind::BlockDevice,
        _ => Kind::Regular,
    }
}

/// The path the kernel names what `fd` holds by, as the supervisor sees it.
pub(super) fn named(fd: impl AsFd) -> Result<PathBuf, Errno> {
    use std::os::fd::AsRawFd;
    let link = format!("/proc/self/fd/{}", fd.as_fd().as_raw_fd());
    std::fs::read_link(link).map_err(|why| Errno::from_raw(why.raw_os_error().unwrap_or(0)))
}

/// `folder`'s path joined with `name`.
pub(super) fn named_in(folder: impl AsFd, name: &OsStr) -> Result<PathBuf, Errno> {
    Ok(named(folder)?.join(name))
}

impl Chat<'_> {
    /// Where a walk starts for a path relative to `dirfd` (`AT_FDCWD` for the working folder).
    pub(super) fn start(&self, dirfd: i32) -> Result<OwnedFd, Errno> {
        let link = if dirfd == libc::AT_FDCWD {
            format!("/proc/{}/cwd", self.tid)
        } else {
            if dirfd < 0 {
                return Err(Errno::EBADF);
            }
            format!("/proc/{}/fd/{dirfd}", self.tid)
        };
        nix::fcntl::open(link.as_str(), path_flags(), Mode::empty())
    }

    /// The thing descriptor `fd` of the chat holds, as a path.
    pub(super) fn descriptor(&self, fd: i32) -> Result<OwnedFd, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }
        nix::fcntl::open(
            format!("/proc/{}/fd/{fd}", self.tid).as_str(),
            path_flags(),
            Mode::empty(),
        )
    }

    /// `path`, walked from `dirfd`; its last name followed when it is a link and `follow`.
    pub(super) fn walk(&self, dirfd: i32, path: &[u8], follow: bool) -> Result<Found, Errno> {
        if path.is_empty() {
            return Err(Errno::ENOENT);
        }
        let mut at = if path.first() == Some(&b'/') {
            nix::unistd::dup(&self.world.root)?
        } else {
            self.start(dirfd)?
        };
        let must_be_folder = path.last() == Some(&b'/');
        let mut names: VecDeque<Vec<u8>> = split(path);
        let mut entry: Option<(OwnedFd, OsString)> = None;
        let mut links = 0;
        while let Some(name) = names.pop_front() {
            let last = names.is_empty();
            if name == b"." {
                entry = None;
                continue;
            }
            if name == b".." {
                at = openat(&at, "..", path_flags() | OFlag::O_DIRECTORY, Mode::empty())?;
                entry = None;
                continue;
            }
            let mut name = OsString::from_vec(name);
            if self.world.is_proc_root(&at) {
                name = self.proc_name(name, &mut names)?;
            }
            let last = last && names.is_empty();
            let next = match openat(
                &at,
                name.as_os_str(),
                path_flags() | OFlag::O_NOFOLLOW,
                Mode::empty(),
            ) {
                Ok(next) => next,
                Err(Errno::ENOENT) if last => {
                    return Ok(Found::Missing { folder: at, name });
                }
                Err(why) => return Err(why),
            };
            let kind = kind_of(&next)?;
            if kind == Kind::Symlink && (!last || follow || must_be_folder) {
                if in_proc(&at) {
                    // A link the kernel makes (`/proc/<pid>/fd/<n>`, `cwd`, `exe`): only the
                    // kernel can follow it, and it leads to what the process holds.
                    let followed = openat(&at, name.as_os_str(), path_flags(), Mode::empty())?;
                    let kind = kind_of(&followed)?;
                    if last {
                        return Ok(Found::There {
                            fd: followed,
                            kind,
                            entry: None,
                        });
                    }
                    if kind != Kind::Directory {
                        return Err(Errno::ENOTDIR);
                    }
                    at = followed;
                    entry = None;
                    continue;
                }
                links += 1;
                if links > MOST_LINKS {
                    return Err(Errno::ELOOP);
                }
                let target = readlinkat(&at, name.as_os_str())?;
                let target = target.as_bytes();
                if target.first() == Some(&b'/') {
                    at = nix::unistd::dup(&self.world.root)?;
                }
                let mut ahead = split(target);
                ahead.extend(names.drain(..));
                names = ahead;
                entry = None;
                continue;
            }
            if last {
                if must_be_folder && kind != Kind::Directory {
                    return Err(Errno::ENOTDIR);
                }
                return Ok(Found::There {
                    fd: next,
                    kind,
                    entry: Some((at, name)),
                });
            }
            if kind != Kind::Directory {
                return Err(Errno::ENOTDIR);
            }
            entry = Some((at, name));
            at = next;
        }
        let kind = kind_of(&at)?;
        Ok(Found::There {
            fd: at,
            kind,
            entry,
        })
        .map(|found| match found {
            // A walk that ends on `.` or `..` names no entry to make or remove.
            Found::There { fd, kind, .. } => Found::There {
                fd,
                kind,
                entry: None,
            },
            missing => missing,
        })
    }

    /// `name` in `/proc` as the chat means it: `self` and `thread-self` are its own, and a
    /// process folder is reached only for a process inside the sandbox.
    fn proc_name(&self, name: OsString, names: &mut VecDeque<Vec<u8>>) -> Result<OsString, Errno> {
        let bytes = name.as_bytes();
        if bytes == b"self" {
            return Ok(OsString::from(self.tgid.to_string()));
        }
        if bytes == b"thread-self" {
            names.push_front(self.tid.to_string().into_bytes());
            names.push_front(b"task".to_vec());
            return Ok(OsString::from(self.tgid.to_string()));
        }
        if let Ok(pid) = std::str::from_utf8(bytes).unwrap_or("").parse::<i32>()
            && !self.world.inside(pid)
        {
            return Err(Errno::EACCES);
        }
        Ok(name)
    }
}

fn in_proc(fd: impl AsFd) -> bool {
    nix::sys::statfs::fstatfs(fd)
        .is_ok_and(|it| it.filesystem_type() == nix::sys::statfs::PROC_SUPER_MAGIC)
}

fn split(path: &[u8]) -> VecDeque<Vec<u8>> {
    path.split(|c| *c == b'/')
        .filter(|it| !it.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

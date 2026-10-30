// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The supervisor: each call the filter hands it, judged and then performed by it.**
//!
//! A call is read from the chat's memory once, into the supervisor's own memory; the path or
//! address in it is walked or parsed there ([`super::walk`]); what the walk arrived at is
//! judged by the profile; and the supervisor makes the call itself on what it holds, then hands
//! the chat the result: the number, the error, or the descriptor it opened or connected. The
//! kernel never goes on with the chat's own call, so what the chat changes in its memory after
//! the supervisor read it changes nothing.
//!
//! Calls that change names (making, moving, linking, removing, a mode) are made one at a time,
//! so no other call of the chat's moves a folder between one being judged and made.
//!
//! **A call the profile refuses is told** (#2344): the operation, where the walk arrived and the
//! process that asked, to the wrap's [`Teller`], after the refusal is decided and whatever the
//! teller does. Nothing told changes an answer. A call refused for anything but the profile (a
//! process outside the sandbox, an abstract socket, a call no profile has a word for) names
//! nothing a person could allow, and is not told; but for a refused listen, which is told as
//! macOS's kernel tells a refused bind, so a server that does not start is never silent.

use std::ffi::OsStr;
use std::io::IoSliceMut;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use nix::errno::Errno;
use nix::fcntl::{AT_FDCWD, AtFlags, OFlag, RenameFlags, openat, renameat2};
use nix::sys::socket::{
    AddressFamily, Backlog, Shutdown, SockFlag, SockType, SockaddrIn, SockaddrIn6, SockaddrLike,
    SockaddrStorage, UnixAddr, connect, getsockname, listen, setsockopt, shutdown, socket, sockopt,
};
use nix::sys::stat::{
    FchmodatFlags, Mode, SFlag, UtimensatFlags, fchmodat, fstat, mkdirat, mknodat, stat, utimensat,
};
use nix::sys::time::TimeSpec;
use nix::sys::uio::{RemoteIoVec, process_vm_readv};
use nix::unistd::{Pid, UnlinkatFlags, linkat, symlinkat, unlinkat};

use super::notify;
use super::walk::{self, Chat, Found, World, named, named_in};
use crate::loopback::{self, Picked};
use crate::profile::{FileOp, Kind, Op, Profile, Remote};
use crate::refusal::Teller;

/// What a call is answered with.
enum Answer {
    /// Its result.
    Value(i64),
    /// Its error.
    Error(Errno),
    /// A descriptor the supervisor opened, put in at the chat's lowest free number, which is
    /// the call's result.
    Opened { fd: OwnedFd, cloexec: bool },
    /// A socket the supervisor connected, put in place of the chat's own descriptor `at`; the
    /// call's result is zero.
    Connected { fd: OwnedFd, at: u32, cloexec: bool },
    /// The call is gone: its process ended or the call was interrupted. Answered with
    /// `EINTR` in case it is not.
    Gone,
    /// The call goes on in the kernel as the chat made it, under the chat's own Landlock
    /// layer ([`notify::go_on`]).
    GoOn,
}

impl From<Errno> for Answer {
    fn from(why: Errno) -> Self {
        Answer::Error(why)
    }
}

/// A refusal: the chat's sandbox does not allow it.
const REFUSED: Errno = Errno::EACCES;

/// Seatbelt's word for a connection out, which a refused connect is told as.
const CONNECT: &str = crate::refusal::DIALLED;

/// Seatbelt's word for a bind, which a refused listen is told as: a listen with no bind of its
/// own is the kernel's bind (#3410).
const BIND: &str = "network-bind";

/// What a listen on an internet socket the chat never bound is told as refused on: Seatbelt's
/// name for a port the kernel picks, on every interface.
const NEVER_BOUND: &str = "*:0";

/// What a listen whose socket could not be taken is told as refused on: why. The container's
/// runtime may refuse `pidfd_getfd` (a runner made before #3410, or a Kubernetes runner), and so
/// may the system (Yama's `ptrace_scope` 3, a process that is not dumpable).
const NOT_TAKEN: &str = "(the chat's socket could not be taken: pidfd_getfd was refused)";

/// The longest path a call may name, as the kernel's own limit.
const PATH_MAX: usize = 4096;

/// The supervisor of one wrapped chat.
pub(super) struct Supervisor {
    listener: OwnedFd,
    profile: Profile,
    world: World,
    changing: Mutex<()>,
    told: Option<Arc<Teller>>,
    /// The ports picked for the chat's test servers, and the socket made on each (#3396).
    picked: Picked,
}

impl Supervisor {
    pub(super) fn new(
        listener: OwnedFd,
        profile: Profile,
        told: Option<Arc<Teller>>,
        picked: Picked,
    ) -> Result<Supervisor, Errno> {
        Ok(Supervisor {
            listener,
            profile,
            world: World::new()?,
            changing: Mutex::new(()),
            told,
            picked,
        })
    }

    /// Tells `operation` on `target`, refused chat process `chat` by the profile. A process's
    /// own files in `/proc` are never told: no person allows those.
    fn tell(&self, chat: &Chat<'_>, operation: &'static str, target: String) {
        let Some(told) = &self.told else {
            return;
        };
        if Path::new(&target).starts_with("/proc") {
            return;
        }
        let tgid = chat.tgid;
        told.refused(
            operation,
            target,
            || process_name(tgid),
            std::time::Instant::now(),
        );
    }

    /// Serves calls until the listener ends: when every process of the chat has gone.
    pub(super) fn serve(&self) {
        let listener = self.listener.as_fd();
        loop {
            let call = match notify::receive(listener) {
                Ok(call) => call,
                // The call went away while it was being received.
                Err(why) if why.raw_os_error() == Some(libc::ENOENT) => continue,
                Err(why) if why.raw_os_error() == Some(libc::EINTR) => continue,
                Err(_) => return,
            };
            let answer = self.answer(&call);
            if let Err(why) = self.send(listener, call.id, answer) {
                // Never left unanswered: a call the supervisor could not complete fails with
                // why, rather than leaving its thread waiting for ever.
                let errno = why.raw_os_error().unwrap_or(libc::EIO);
                let _ = notify::respond(listener, call.id, 0, errno);
            }
        }
    }

    fn send(&self, listener: BorrowedFd<'_>, id: u64, answer: Answer) -> std::io::Result<()> {
        match answer {
            Answer::Value(value) => notify::respond(listener, id, value, 0),
            Answer::Error(why) => notify::respond(listener, id, 0, why as i32),
            Answer::Opened { fd, cloexec } => {
                notify::add_fd(listener, id, fd.as_fd(), None, cloexec, true).map(drop)
            }
            Answer::Connected { fd, at, cloexec } => {
                notify::add_fd(listener, id, fd.as_fd(), Some(at), cloexec, false)?;
                notify::respond(listener, id, 0, 0)
            }
            // Answered all the same: a call that is gone refuses the answer, and one that is not
            // must never be left waiting.
            Answer::Gone => notify::respond(listener, id, 0, libc::EINTR),
            Answer::GoOn => notify::go_on(listener, id),
        }
    }

    /// Whether the call `id` still waits, asked after its memory was read and before anything
    /// is done, so a process that ended and a number used again are never acted for.
    fn alive(&self, id: u64) -> bool {
        notify::valid(self.listener.as_fd(), id)
    }

    fn answer(&self, call: &libc::seccomp_notif) -> Answer {
        let tid = i32::try_from(call.pid).unwrap_or(0);
        let Some(tgid) = walk::tgid(tid) else {
            return Answer::Gone;
        };
        let chat = Chat {
            tid,
            tgid,
            world: &self.world,
        };
        let a = call.data.args;
        let int = |at: usize| a[at] as i32;
        let nr = i64::from(call.data.nr);
        let id = call.id;
        let at = |at: usize| -> Result<Vec<u8>, Errno> { read_path(tid, a[at]) };
        let out = (|| -> Result<Answer, Errno> {
            match nr {
                libc::SYS_openat => {
                    let path = at(1)?;
                    self.open(&chat, id, int(0), &path, int(2), a[3] as u32)
                }
                libc::SYS_mkdirat => {
                    let path = at(1)?;
                    self.make(&chat, id, int(0), &path, Kind::Directory, a[2] as u32, None)
                }
                libc::SYS_mknodat => {
                    let path = at(1)?;
                    self.mknod(&chat, id, int(0), &path, a[2] as u32)
                }
                libc::SYS_symlinkat => {
                    let target = at(0)?;
                    let path = at(2)?;
                    self.make(&chat, id, int(1), &path, Kind::Symlink, 0, Some(&target))
                }
                libc::SYS_unlinkat => {
                    let path = at(1)?;
                    self.unlink(&chat, id, int(0), &path, int(2) & libc::AT_REMOVEDIR != 0)
                }
                libc::SYS_renameat2 => {
                    let (from, to) = (at(1)?, at(3)?);
                    self.rename(&chat, id, (int(0), &from), (int(2), &to), a[4] as u32)
                }
                libc::SYS_linkat => {
                    let (from, to) = (at(1)?, at(3)?);
                    self.link(&chat, id, (int(0), &from), (int(2), &to), int(4))
                }
                libc::SYS_truncate => {
                    let path = at(0)?;
                    self.truncate(&chat, id, &path, a[1] as i64)
                }
                libc::SYS_fchmod => self.fchmod(&chat, id, int(0), a[1] as u32),
                libc::SYS_fchmodat => {
                    let path = at(1)?;
                    self.chmod(&chat, id, int(0), &path, a[2] as u32, 0)
                }
                super::filter::SYS_FCHMODAT2 => {
                    let flags = int(3);
                    // No path, or an empty one, with `AT_EMPTY_PATH` is the descriptor itself.
                    let path = if a[1] == 0 && flags & libc::AT_EMPTY_PATH != 0 {
                        Vec::new()
                    } else {
                        at(1)?
                    };
                    if flags & libc::AT_EMPTY_PATH != 0 && path.is_empty() {
                        return self.fchmod(&chat, id, int(0), a[2] as u32);
                    }
                    self.chmod(&chat, id, int(0), &path, a[2] as u32, flags)
                }
                libc::SYS_utimensat => {
                    // No path is the descriptor itself, as `futimens` asks.
                    let path = if a[1] == 0 { None } else { Some(at(1)?) };
                    let times = if a[2] == 0 {
                        None
                    } else {
                        Some(read_bytes(tid, a[2], 32)?)
                    };
                    self.times(&chat, id, int(0), path.as_deref(), times.as_deref(), int(3))
                }
                libc::SYS_connect => self.connect(&chat, id, int(0), a[1], a[2] as usize),
                libc::SYS_listen => self.listen(&chat, id, int(0), int(1)),
                libc::SYS_bind => self.bind(&chat, id, int(0), a[1], a[2] as usize),
                #[cfg(target_arch = "x86_64")]
                legacy => self.legacy(&chat, id, legacy, &a, tid),
                #[cfg(not(target_arch = "x86_64"))]
                libc::SYS_renameat => {
                    let (from, to) = (at(1)?, at(3)?);
                    self.rename(&chat, id, (int(0), &from), (int(2), &to), 0)
                }
                #[cfg(not(target_arch = "x86_64"))]
                _ => Err(Errno::ENOSYS),
            }
        })();
        out.unwrap_or_else(Answer::Error)
    }

    /// The x86-64 calls that name no folder: each is its `*at` form from the working folder.
    #[cfg(target_arch = "x86_64")]
    fn legacy(
        &self,
        chat: &Chat<'_>,
        id: u64,
        nr: i64,
        a: &[u64; 6],
        tid: i32,
    ) -> Result<Answer, Errno> {
        let here = libc::AT_FDCWD;
        let at = |at: usize| read_path(tid, a[at]);
        match nr {
            libc::SYS_open => self.open(chat, id, here, &at(0)?, a[1] as i32, a[2] as u32),
            libc::SYS_creat => self.open(
                chat,
                id,
                here,
                &at(0)?,
                libc::O_CREAT | libc::O_WRONLY | libc::O_TRUNC,
                a[1] as u32,
            ),
            libc::SYS_mkdir => {
                self.make(chat, id, here, &at(0)?, Kind::Directory, a[1] as u32, None)
            }
            libc::SYS_mknod => self.mknod(chat, id, here, &at(0)?, a[1] as u32),
            libc::SYS_symlink => {
                let target = at(0)?;
                self.make(chat, id, here, &at(1)?, Kind::Symlink, 0, Some(&target))
            }
            libc::SYS_unlink => self.unlink(chat, id, here, &at(0)?, false),
            libc::SYS_rmdir => self.unlink(chat, id, here, &at(0)?, true),
            libc::SYS_rename => {
                let from = at(0)?;
                self.rename(chat, id, (here, &from), (here, &at(1)?), 0)
            }
            libc::SYS_renameat => {
                let (from, to) = (at(1)?, at(3)?);
                self.rename(chat, id, (a[0] as i32, &from), (a[2] as i32, &to), 0)
            }
            libc::SYS_link => {
                let from = at(0)?;
                self.link(chat, id, (here, &from), (here, &at(1)?), 0)
            }
            libc::SYS_chmod => self.chmod(chat, id, here, &at(0)?, a[1] as u32, 0),
            _ => Err(Errno::ENOSYS),
        }
    }

    fn allows(&self, chat: &Chat<'_>, what: FileOp, path: &Path, kind: Kind) -> bool {
        // Wherever a walk arrived from, a process outside the sandbox is read for its `stat`
        // alone.
        if !outside_process_stat_only(chat, path) {
            return false;
        }
        let one = |path: &Path| {
            self.profile.allows(&Op::File { what, path, kind }) || linux_own(chat, what, path, kind)
        };
        // A file no longer linked is named with " (deleted)" after its last name: judged by
        // both, so a rule on that name still holds it.
        let last = path
            .as_os_str()
            .as_bytes()
            .strip_suffix(b" (deleted)")
            .map(|it| Path::new(OsStr::from_bytes(it)));
        one(path) && last.is_none_or(one)
    }

    fn judge(&self, chat: &Chat<'_>, ops: &[(FileOp, &Path, Kind)]) -> Result<(), Errno> {
        match ops
            .iter()
            .find(|(what, path, kind)| !self.allows(chat, *what, path, *kind))
        {
            None => Ok(()),
            Some((what, path, _)) => {
                self.tell(chat, what.name(), path.display().to_string());
                Err(REFUSED)
            }
        }
    }

    fn open(
        &self,
        chat: &Chat<'_>,
        id: u64,
        dirfd: i32,
        path: &[u8],
        flags: i32,
        mode: u32,
    ) -> Result<Answer, Errno> {
        let oflag = OFlag::from_bits_retain(flags);
        let cloexec = oflag.contains(OFlag::O_CLOEXEC);
        let access = flags & libc::O_ACCMODE;
        let reads = access == libc::O_RDONLY || access == libc::O_RDWR;
        let writes = access != libc::O_RDONLY || oflag.contains(OFlag::O_TRUNC);
        let tmpfile = flags & libc::O_TMPFILE == libc::O_TMPFILE;
        let create = oflag.contains(OFlag::O_CREAT) && !tmpfile;
        let exclusive = create && oflag.contains(OFlag::O_EXCL);
        let nofollow = oflag.contains(OFlag::O_NOFOLLOW);
        let path_only = oflag.contains(OFlag::O_PATH);
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let changes = create || tmpfile;
        let _held = changes.then(|| self.changing.lock());

        if tmpfile {
            let Found::There {
                fd: folder, kind, ..
            } = chat.walk(dirfd, path, true)?
            else {
                return Err(Errno::ENOENT);
            };
            if kind != Kind::Directory {
                return Err(Errno::ENOTDIR);
            }
            let made = named(&folder)?.join(".tmpfile");
            self.judge(
                chat,
                &[
                    (FileOp::Create, &made, Kind::Regular),
                    (FileOp::WriteData, &made, Kind::Regular),
                ],
            )?;
            let fd = openat(
                &folder,
                ".",
                oflag | OFlag::O_CLOEXEC,
                Mode::from_bits_truncate(mode & !umask(chat.tid)),
            )?;
            return Ok(Answer::Opened { fd, cloexec });
        }

        match chat.walk(dirfd, path, !nofollow && !exclusive)? {
            Found::There { fd, kind, entry } => {
                if exclusive {
                    return Err(Errno::EEXIST);
                }
                if kind == Kind::Symlink && !path_only {
                    return Err(Errno::ELOOP);
                }
                if oflag.contains(OFlag::O_DIRECTORY)
                    && kind != Kind::Directory
                    && kind != Kind::Symlink
                {
                    return Err(Errno::ENOTDIR);
                }
                let at = match (&entry, kind) {
                    (Some((folder, name)), Kind::Symlink) => named_in(folder, name)?,
                    _ => named(&fd)?,
                };
                if path_only {
                    // The kernel opens a path-only descriptor itself (`filter::supervised`), and
                    // one cannot be handed over.
                    return Err(Errno::EINVAL);
                }
                if writes && kind == Kind::Directory {
                    return Err(Errno::EISDIR);
                }
                let mut ops = Vec::new();
                if reads {
                    ops.push((FileOp::ReadData, at.as_path(), kind));
                }
                if writes {
                    ops.push((FileOp::WriteData, at.as_path(), kind));
                }
                self.judge(chat, &ops)?;
                let again = oflag & !(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW)
                    | OFlag::O_CLOEXEC
                    | OFlag::O_NOCTTY;
                let opened = nix::fcntl::open(
                    format!("/proc/self/fd/{}", fd.as_raw_fd()).as_str(),
                    again,
                    Mode::empty(),
                )?;
                Ok(Answer::Opened {
                    fd: opened,
                    cloexec,
                })
            }
            Found::Missing { folder, name } => {
                if !create {
                    return Err(Errno::ENOENT);
                }
                let made = named_in(&folder, &name)?;
                let mut ops = vec![
                    (FileOp::Create, made.as_path(), Kind::Regular),
                    (FileOp::WriteData, made.as_path(), Kind::Regular),
                ];
                if reads {
                    ops.push((FileOp::ReadData, made.as_path(), Kind::Regular));
                }
                self.judge(chat, &ops)?;
                let fd = openat(
                    &folder,
                    name.as_os_str(),
                    oflag | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NOCTTY,
                    Mode::from_bits_truncate(mode & 0o7777 & !umask(chat.tid)),
                )?;
                Ok(Answer::Opened { fd, cloexec })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make(
        &self,
        chat: &Chat<'_>,
        id: u64,
        dirfd: i32,
        path: &[u8],
        kind: Kind,
        mode: u32,
        target: Option<&[u8]>,
    ) -> Result<Answer, Errno> {
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let Found::Missing { folder, name } = chat.walk(dirfd, path, false)? else {
            return Err(Errno::EEXIST);
        };
        let made = named_in(&folder, &name)?;
        self.judge(chat, &[(FileOp::Create, &made, kind)])?;
        let mode = Mode::from_bits_truncate(mode & 0o7777 & !umask(chat.tid));
        match target {
            Some(target) => symlinkat(OsStr::from_bytes(target), &folder, name.as_os_str())?,
            None => mkdirat(&folder, name.as_os_str(), mode)?,
        }
        Ok(Answer::Value(0))
    }

    fn mknod(
        &self,
        chat: &Chat<'_>,
        id: u64,
        dirfd: i32,
        path: &[u8],
        mode: u32,
    ) -> Result<Answer, Errno> {
        let (kind, flag) = match mode & libc::S_IFMT {
            0 | libc::S_IFREG => (Kind::Regular, SFlag::S_IFREG),
            libc::S_IFIFO => (Kind::Fifo, SFlag::S_IFIFO),
            libc::S_IFSOCK => (Kind::Socket, SFlag::S_IFSOCK),
            _ => return Err(Errno::EPERM),
        };
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let Found::Missing { folder, name } = chat.walk(dirfd, path, false)? else {
            return Err(Errno::EEXIST);
        };
        let made = named_in(&folder, &name)?;
        self.judge(chat, &[(FileOp::Create, &made, kind)])?;
        let perm = Mode::from_bits_truncate(mode & 0o7777 & !umask(chat.tid));
        mknodat(&folder, name.as_os_str(), flag, perm, 0)?;
        Ok(Answer::Value(0))
    }

    fn unlink(
        &self,
        chat: &Chat<'_>,
        id: u64,
        dirfd: i32,
        path: &[u8],
        folder_only: bool,
    ) -> Result<Answer, Errno> {
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let Found::There { kind, entry, .. } = chat.walk(dirfd, path, false)? else {
            return Err(Errno::ENOENT);
        };
        let Some((folder, name)) = entry else {
            return Err(Errno::EBUSY);
        };
        match (folder_only, kind == Kind::Directory) {
            (true, false) => return Err(Errno::ENOTDIR),
            (false, true) => return Err(Errno::EISDIR),
            _ => {}
        }
        let gone = named_in(&folder, &name)?;
        self.judge(chat, &[(FileOp::Unlink, &gone, kind)])?;
        let how = if folder_only {
            UnlinkatFlags::RemoveDir
        } else {
            UnlinkatFlags::NoRemoveDir
        };
        unlinkat(&folder, name.as_os_str(), how)?;
        Ok(Answer::Value(0))
    }

    fn rename(
        &self,
        chat: &Chat<'_>,
        id: u64,
        from: (i32, &[u8]),
        to: (i32, &[u8]),
        flags: u32,
    ) -> Result<Answer, Errno> {
        let flags = RenameFlags::from_bits(flags).ok_or(Errno::EINVAL)?;
        if flags.contains(RenameFlags::RENAME_WHITEOUT) {
            return Err(Errno::EPERM);
        }
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let Found::There {
            kind,
            entry: Some((old_folder, old_name)),
            ..
        } = chat.walk(from.0, from.1, false)?
        else {
            return Err(Errno::ENOENT);
        };
        let old = named_in(&old_folder, &old_name)?;
        let (new_folder, new_name, there) = match chat.walk(to.0, to.1, false)? {
            Found::Missing { folder, name } => (folder, name, None),
            Found::There {
                kind,
                entry: Some((folder, name)),
                ..
            } => (folder, name, Some(kind)),
            Found::There { entry: None, .. } => return Err(Errno::EBUSY),
        };
        let new = named_in(&new_folder, &new_name)?;
        let mut ops = vec![
            (FileOp::Unlink, old.as_path(), kind),
            (FileOp::Create, new.as_path(), kind),
        ];
        if let Some(there) = there {
            ops.push((FileOp::Unlink, new.as_path(), there));
            if flags.contains(RenameFlags::RENAME_EXCHANGE) {
                ops.push((FileOp::Create, old.as_path(), there));
            }
        }
        self.judge(chat, &ops)?;
        renameat2(
            &old_folder,
            old_name.as_os_str(),
            &new_folder,
            new_name.as_os_str(),
            flags,
        )?;
        Ok(Answer::Value(0))
    }

    fn link(
        &self,
        chat: &Chat<'_>,
        id: u64,
        from: (i32, &[u8]),
        to: (i32, &[u8]),
        flags: i32,
    ) -> Result<Answer, Errno> {
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let (old, kind, old_path) = if flags & libc::AT_EMPTY_PATH != 0 && from.1.is_empty() {
            let fd = chat.descriptor(from.0)?;
            let kind = walk::kind_of(&fd)?;
            let path = named(&fd)?;
            (fd, kind, path)
        } else {
            match chat.walk(from.0, from.1, flags & libc::AT_SYMLINK_FOLLOW != 0)? {
                Found::There { fd, kind, entry } => {
                    let path = match (&entry, kind) {
                        (Some((folder, name)), Kind::Symlink) => named_in(folder, name)?,
                        _ => named(&fd)?,
                    };
                    (fd, kind, path)
                }
                Found::Missing { .. } => return Err(Errno::ENOENT),
            }
        };
        if kind == Kind::Directory {
            return Err(Errno::EPERM);
        }
        let Found::Missing { folder, name } = chat.walk(to.0, to.1, false)? else {
            return Err(Errno::EEXIST);
        };
        let new = named_in(&folder, &name)?;
        self.judge(
            chat,
            &[
                (FileOp::WriteData, old_path.as_path(), kind),
                (FileOp::Create, new.as_path(), kind),
            ],
        )?;
        linkat(
            AT_FDCWD,
            format!("/proc/self/fd/{}", old.as_raw_fd()).as_str(),
            &folder,
            name.as_os_str(),
            AtFlags::AT_SYMLINK_FOLLOW,
        )?;
        Ok(Answer::Value(0))
    }

    fn truncate(&self, chat: &Chat<'_>, id: u64, path: &[u8], len: i64) -> Result<Answer, Errno> {
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let Found::There { fd, kind, .. } = chat.walk(libc::AT_FDCWD, path, true)? else {
            return Err(Errno::ENOENT);
        };
        match kind {
            Kind::Directory => return Err(Errno::EISDIR),
            Kind::Regular => {}
            _ => return Err(Errno::EINVAL),
        }
        let at = named(&fd)?;
        self.judge(chat, &[(FileOp::WriteData, &at, kind)])?;
        let opened = nix::fcntl::open(
            format!("/proc/self/fd/{}", fd.as_raw_fd()).as_str(),
            OFlag::O_WRONLY | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
            Mode::empty(),
        )?;
        nix::unistd::ftruncate(&opened, len)?;
        Ok(Answer::Value(0))
    }

    fn chmod(
        &self,
        chat: &Chat<'_>,
        id: u64,
        dirfd: i32,
        path: &[u8],
        mode: u32,
        flags: i32,
    ) -> Result<Answer, Errno> {
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let follow = flags & libc::AT_SYMLINK_NOFOLLOW == 0;
        let Found::There { fd, kind, .. } = chat.walk(dirfd, path, follow)? else {
            return Err(Errno::ENOENT);
        };
        if kind == Kind::Symlink {
            return Err(Errno::EOPNOTSUPP);
        }
        self.mode_of(chat, &fd, kind, mode)
    }

    /// `utimensat`: the times of the file `path` names from `dirfd`, or of `dirfd` itself where
    /// there is no path (or an empty one, with `AT_EMPTY_PATH`); `times` the two `timespec`s,
    /// or now for both.
    fn times(
        &self,
        chat: &Chat<'_>,
        id: u64,
        dirfd: i32,
        path: Option<&[u8]>,
        times: Option<&[u8]>,
        flags: i32,
    ) -> Result<Answer, Errno> {
        let (atime, mtime) = match times {
            None => (TimeSpec::UTIME_NOW, TimeSpec::UTIME_NOW),
            Some(bytes) => {
                let field =
                    |at: usize| i64::from_ne_bytes(bytes[at..at + 8].try_into().unwrap_or([0; 8]));
                (
                    TimeSpec::new(field(0), field(8)),
                    TimeSpec::new(field(16), field(24)),
                )
            }
        };
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let itself = path.is_none_or(|path| path.is_empty() && flags & libc::AT_EMPTY_PATH != 0);
        let (fd, kind, at, entry) = if itself {
            let fd = chat.descriptor(dirfd)?;
            let kind = walk::kind_of(&fd)?;
            let at = named(&fd)?;
            (fd, kind, at, None)
        } else {
            let follow = flags & libc::AT_SYMLINK_NOFOLLOW == 0;
            match chat.walk(dirfd, path.unwrap_or_default(), follow)? {
                Found::There { fd, kind, entry } => {
                    let at = match (&entry, kind) {
                        (Some((folder, name)), Kind::Symlink) => named_in(folder, name)?,
                        _ => named(&fd)?,
                    };
                    (fd, kind, at, entry)
                }
                Found::Missing { .. } => return Err(Errno::ENOENT),
            }
        };
        // A pipe or a socket has no name to judge, as for a mode.
        if at.is_absolute() {
            self.judge(chat, &[(FileOp::Times, &at, kind)])?;
        }
        match (kind, &entry) {
            // A link's own times, in the folder the walk holds.
            (Kind::Symlink, Some((folder, name))) => utimensat(
                folder,
                name.as_os_str(),
                &atime,
                &mtime,
                UtimensatFlags::NoFollowSymlink,
            )?,
            (Kind::Symlink, None) => return Err(Errno::EOPNOTSUPP),
            _ => utimensat(
                AT_FDCWD,
                format!("/proc/self/fd/{}", fd.as_raw_fd()).as_str(),
                &atime,
                &mtime,
                UtimensatFlags::FollowSymlink,
            )?,
        }
        Ok(Answer::Value(0))
    }

    fn fchmod(&self, chat: &Chat<'_>, id: u64, fd: i32, mode: u32) -> Result<Answer, Errno> {
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let _held = self.changing.lock();
        let held = chat.descriptor(fd)?;
        let kind = walk::kind_of(&held)?;
        self.mode_of(chat, &held, kind, mode)
    }

    fn mode_of(
        &self,
        chat: &Chat<'_>,
        fd: &OwnedFd,
        kind: Kind,
        mode: u32,
    ) -> Result<Answer, Errno> {
        let at = named(fd)?;
        // A pipe or a socket has no name to judge: its mode reaches no file.
        if at.is_absolute() {
            self.judge(chat, &[(FileOp::Mode, &at, kind)])?;
        }
        fchmodat(
            AT_FDCWD,
            format!("/proc/self/fd/{}", fd.as_raw_fd()).as_str(),
            Mode::from_bits_truncate(mode & 0o7777),
            FchmodatFlags::FollowSymlink,
        )?;
        Ok(Answer::Value(0))
    }

    fn connect(
        &self,
        chat: &Chat<'_>,
        id: u64,
        fd: i32,
        addr: u64,
        len: usize,
    ) -> Result<Answer, Errno> {
        if len < 2 || len > std::mem::size_of::<libc::sockaddr_storage>() {
            return Err(Errno::EINVAL);
        }
        let bytes = read_bytes(chat.tid, addr, len)?;
        let family = i32::from(u16::from_ne_bytes([bytes[0], bytes[1]]));
        let at = u32::try_from(fd).map_err(|_| Errno::EBADF)?;
        // The chat's descriptor's flags: whether it is non-blocking and closed on a start.
        let flags = descriptor_flags(chat.tid, fd)?;
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let socket_fd = match family {
            libc::AF_UNIX => {
                let path = &bytes[2..];
                if path.first().is_none_or(|first| *first == 0) {
                    // An abstract or unnamed socket names no path, and no profile allows one.
                    return Err(REFUSED);
                }
                let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
                // Found as the kernel would find it for the chat, through any link on the way,
                // and judged by the name the kernel gives the socket itself, as Seatbelt judges
                // it: a link to an allowed socket reaches it, and a link to another does not.
                let Found::There {
                    fd: found, kind, ..
                } = chat.walk(libc::AT_FDCWD, &path[..end], true)?
                else {
                    return Err(Errno::ENOENT);
                };
                if kind != Kind::Socket {
                    return Err(Errno::ECONNREFUSED);
                }
                let absolute = named(&found)?;
                if !self
                    .profile
                    .allows(&Op::Connect(Remote::Unix(Some(&absolute))))
                {
                    self.tell(chat, CONNECT, absolute.display().to_string());
                    return Err(REFUSED);
                }
                let socket = socket(
                    AddressFamily::Unix,
                    SockType::Stream,
                    SockFlag::SOCK_CLOEXEC,
                    None,
                )?;
                // Through the descriptor the walk holds, so the socket connected is the one
                // judged, whatever its path's length.
                let held = format!("/proc/self/fd/{}", found.as_raw_fd());
                connect(socket.as_raw_fd(), &UnixAddr::new(held.as_str())?)?;
                socket
            }
            libc::AF_INET | libc::AF_INET6 => {
                let remote = ip_of(&bytes)?;
                let allowed = self.profile.allows(&Op::Connect(Remote::Ip {
                    ip: remote.ip(),
                    port: remote.port(),
                }));
                // A test server of the chat's own, on a port picked for it (#3396): reached
                // while the socket made for it alone listens there, read again once connected.
                let own = !allowed && self.chat_s_own(remote);
                if !allowed && !own {
                    // A service on this machine; an internet address dialled past the proxy,
                    // told apart, once a minute (#2423): the proxy tells each host by name.
                    if is_this_machine(remote.ip()) {
                        self.tell(chat, CONNECT, remote.to_string());
                    } else if let Some(told) = &self.told {
                        let tgid = chat.tgid;
                        told.past_the_proxy(
                            remote.to_string(),
                            || process_name(tgid),
                            std::time::Instant::now(),
                        );
                    }
                    return Err(REFUSED);
                }
                let (family, address): (AddressFamily, Box<dyn nix::sys::socket::SockaddrLike>) =
                    match remote {
                        SocketAddr::V4(v4) => (AddressFamily::Inet, Box::new(SockaddrIn::from(v4))),
                        SocketAddr::V6(v6) => {
                            (AddressFamily::Inet6, Box::new(SockaddrIn6::from(v6)))
                        }
                    };
                let socket = socket(family, SockType::Stream, SockFlag::SOCK_CLOEXEC, None)?;
                let _ = setsockopt(&socket, sockopt::TcpNoDelay, &true);
                connect(socket.as_raw_fd(), address.as_ref())?;
                // Its server let the port go between the judging and the connect, and another
                // socket took it: the connection is dropped before the chat holds it.
                if own && !self.chat_s_own(remote) {
                    drop(socket);
                    self.tell(chat, CONNECT, remote.to_string());
                    return Err(REFUSED);
                }
                socket
            }
            _ => return Err(REFUSED),
        };
        if flags & libc::O_NONBLOCK != 0 {
            nix::fcntl::fcntl(&socket_fd, nix::fcntl::FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
        }
        Ok(Answer::Connected {
            fd: socket_fd,
            at,
            cloexec: flags & libc::O_CLOEXEC != 0,
        })
    }

    /// **A bind, judged and made by the supervisor** (#2019). A chat serves on a socket of its
    /// own, in the folder purlis made for it, behind purlis's door on the chat's port:
    ///
    /// - **A unix socket at a path** is bound only at an absolute path the profile names to serve
    ///   on (`local unix-socket (path-literal …)`) and lets the chat make and take away a socket
    ///   at ([`crate::profile::Profile::serves_at`]), by the supervisor, on a socket of its own
    ///   put in place of the chat's descriptor; what was left at the path is taken away first.
    ///   Any other path is refused, at every level: the floor's `(allow default)` names no bind.
    /// - **An internet address on a port the chat serves on** (any address: `127.0.0.1`,
    ///   `0.0.0.0`, `::`) is bound instead to that port's socket, `<port>.sock`, the same way: a
    ///   server started for a port, on whatever address, is served through the door. Any other
    ///   internet bind is refused, as the chat's Landlock layer refuses every TCP bind, and told,
    ///   so a server that asks for a port raises the Ask to serve on it.
    /// - **A port the system picks on 127.0.0.1** (#3396, ruled 2026-10-30), which a test
    ///   server asks for, is bound on a TCP socket of the supervisor's own, on a port of the
    ///   pool picked for the chat, put in place of the chat's descriptor ([`Self::bind_picked`]).
    ///   A port of zero on any other address (every interface, `::1`) is refused and told: no
    ///   person allows one, and its Notice says what to do instead.
    /// - **A netlink socket (what `getifaddrs` binds) and an abstract or unnamed unix socket go
    ///   on in the kernel** ([`Answer::GoOn`]), as before; any other family is refused. Nothing
    ///   the chat changes after the address was read carries such a call further: the call the kernel
    ///   makes is judged by the chat's own Landlock layer, which binds no TCP port and makes no
    ///   socket file.
    /// - **The socket put in the chat's place is a unix one**, bound here at its path, **or an
    ///   internet one bound on 127.0.0.1 at a picked port**: no internet socket the supervisor
    ///   hands the chat is ever bound without a port of its own, so a listen on it is never the
    ///   kernel's to bind (see [`Self::listen`]).
    fn bind(
        &self,
        chat: &Chat<'_>,
        id: u64,
        fd: i32,
        addr: u64,
        len: usize,
    ) -> Result<Answer, Errno> {
        if len < 2 || len > std::mem::size_of::<libc::sockaddr_storage>() {
            return Err(Errno::EINVAL);
        }
        let bytes = read_bytes(chat.tid, addr, len)?;
        let family = i32::from(u16::from_ne_bytes([bytes[0], bytes[1]]));
        let socket = match family {
            libc::AF_UNIX => {
                let path = &bytes[2..];
                // An abstract or unnamed socket names no file: the kernel's, as before.
                if path.first().is_none_or(|first| *first == 0) {
                    return Ok(Answer::GoOn);
                }
                let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
                let named = Path::new(OsStr::from_bytes(&path[..end]));
                named.to_path_buf()
            }
            libc::AF_INET | libc::AF_INET6 => {
                let local = ip_of(&bytes)?;
                if loopback::picks_on_the_loopback(local) {
                    return self.bind_picked(chat, id, fd);
                }
                if local.port() == 0 {
                    self.tell(chat, BIND, local.to_string());
                    return Err(REFUSED);
                }
                let wanted = format!("{}.sock", local.port());
                let Some(served) = self.profile.reach().serves.into_iter().find(|path| {
                    path.file_name() == Some(OsStr::new(&wanted)) && self.profile.serves_at(path)
                }) else {
                    self.tell(chat, BIND, local.to_string());
                    return Err(REFUSED);
                };
                served
            }
            // What `getifaddrs` binds goes on; any other family, `AF_UNSPEC` (which the kernel
            // reads as an IPv4 bind on an IPv4 socket) included, is refused.
            libc::AF_NETLINK => return Ok(Answer::GoOn),
            _ => return Err(REFUSED),
        };
        // Only where the profile names a socket to serve on, and lets it be made and taken
        // away: never another socket of this user's, whatever else the profile allows.
        if !self.profile.serves_at(&socket) {
            return Err(REFUSED);
        }
        let at = u32::try_from(fd).map_err(|_| Errno::EBADF)?;
        let flags = descriptor_flags(chat.tid, fd)?;
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        // What a server that ended left there, or anything else but a folder, is taken away.
        match std::fs::symlink_metadata(&socket) {
            Ok(meta) if meta.is_dir() => return Err(Errno::EADDRINUSE),
            Ok(_) => std::fs::remove_file(&socket).map_err(|_| REFUSED)?,
            Err(_) => {}
        }
        let served = nix::sys::socket::socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;
        nix::sys::socket::bind(served.as_raw_fd(), &UnixAddr::new(socket.as_path())?)?;
        if flags & libc::O_NONBLOCK != 0 {
            nix::fcntl::fcntl(&served, nix::fcntl::FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
        }
        Ok(Answer::Connected {
            fd: served,
            at,
            cloexec: flags & libc::O_CLOEXEC != 0,
        })
    }

    /// **A bind on a port the system picks, on 127.0.0.1** (#3396): a TCP socket of the
    /// supervisor's own, bound on the first port of the pool picked for the chat that is free,
    /// recorded as the chat's, and put in place of the chat's descriptor `fd`. Made here rather
    /// than on the chat's own socket, so no option the chat set first (a port shared with another
    /// process's socket, `SO_REUSEPORT`) goes with the bind. A pool with no port free is
    /// `EADDRINUSE`, as the kernel says it, and told.
    fn bind_picked(&self, chat: &Chat<'_>, id: u64, fd: i32) -> Result<Answer, Errno> {
        let at = u32::try_from(fd).map_err(|_| Errno::EBADF)?;
        let flags = descriptor_flags(chat.tid, fd)?;
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let served = socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;
        // As a server sets it before its bind, so a port a server let go is bound again while
        // its last connections close. On Linux it never shares a port another socket listens on.
        setsockopt(&served, sockopt::ReuseAddr, &true)?;
        let mut bound = None;
        for port in self.picked.turn() {
            match nix::sys::socket::bind(served.as_raw_fd(), &SockaddrIn::new(127, 0, 0, 1, port)) {
                Ok(()) => {
                    bound = Some(port);
                    break;
                }
                Err(Errno::EADDRINUSE) => {}
                Err(why) => return Err(why),
            }
        }
        let Some(port) = bound else {
            self.tell(chat, BIND, "127.0.0.1:0".to_owned());
            return Err(Errno::EADDRINUSE);
        };
        self.picked.own(port, fstat(&served)?.st_ino);
        if flags & libc::O_NONBLOCK != 0 {
            nix::fcntl::fcntl(&served, nix::fcntl::FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
        }
        Ok(Answer::Connected {
            fd: served,
            at,
            cloexec: flags & libc::O_CLOEXEC != 0,
        })
    }

    /// Whether `remote` is a test server of the chat's own (#3396): on 127.0.0.1, at a port
    /// picked for it where the socket the supervisor made for it alone listens, as this
    /// machine's table of TCP sockets says now.
    fn chat_s_own(&self, remote: SocketAddr) -> bool {
        let SocketAddr::V4(v4) = remote else {
            return false;
        };
        if *v4.ip() != Ipv4Addr::LOCALHOST || !self.picked.ports().contains(&v4.port()) {
            return false;
        }
        std::fs::read_to_string("/proc/self/net/tcp")
            .is_ok_and(|table| self.picked.owns(v4.port(), &table))
    }

    /// **A listen, made by the supervisor on the socket it judged**: the calling thread's own
    /// socket, taken from its own descriptor table (`pidfd_open` with `PIDFD_THREAD`, then
    /// `pidfd_getfd`) and checked to be the one its descriptor names, listens only where it was
    /// bound. A thread's own table is the one read, whether it holds one apart from its
    /// process's (`unshare(CLONE_FILES)`) or its process's first thread has ended.
    ///
    /// An internet socket the chat never bound is refused: the kernel would otherwise bind it
    /// first, to a port it picks on every interface, since Landlock judges a bind the chat makes
    /// and not one the kernel makes for it. An internet socket whose address cannot be read is
    /// taken as bound to none. Where an internet socket may be bound is the bind's to judge, and
    /// the chat's Landlock layer binds none today. Any other socket listens as it would have.
    ///
    /// The listen is made on the socket judged, never on whatever the descriptor holds by then,
    /// and the address it listens on is read again after: one other than the address judged undoes
    /// the listen and refuses it (and where it cannot be undone, ends the chat's process). Where the socket cannot be taken, nothing listens. A refusal is
    /// told, as macOS's kernel tells a refused bind, with why in what it names.
    ///
    /// Two of the chat's calls a listen relies on are refused elsewhere: a `connect` of any
    /// family but unix and the internet's (`AF_UNSPEC` would unbind a connected socket), and a
    /// connected socket is only ever handed back once its connect is made.
    fn listen(&self, chat: &Chat<'_>, id: u64, fd: i32, backlog: i32) -> Result<Answer, Errno> {
        // The calling thread's own table (Linux 6.9; the sandbox needs 6.12).
        let thread = rustix::process::Pid::from_raw(chat.tid).ok_or(REFUSED)?;
        let flags = rustix::process::PidfdFlags::from_bits_retain(libc::PIDFD_THREAD);
        let caller = match rustix::process::pidfd_open(thread, flags) {
            Ok(caller) => caller,
            // The thread ended, and its call with it.
            Err(rustix::io::Errno::SRCH) if !self.alive(id) => return Ok(Answer::Gone),
            Err(why) => return Err(self.not_taken(chat, why)),
        };
        // The thread the call came from, not another that took its number since.
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        let socket =
            rustix::process::pidfd_getfd(&caller, fd, rustix::process::PidfdGetfdFlags::empty())
                .map_err(|why| self.not_taken(chat, why))?;
        // The same open socket the thread's descriptor names, read a second way.
        let theirs = stat(format!("/proc/{}/fd/{fd}", chat.tid).as_str()).map_err(|why| {
            if why == Errno::ENOENT {
                Errno::EBADF
            } else {
                REFUSED
            }
        })?;
        let taken = fstat(&socket).map_err(|_| REFUSED)?;
        if (taken.st_dev, taken.st_ino) != (theirs.st_dev, theirs.st_ino) {
            return Err(REFUSED);
        }
        let judged = getsockname::<SockaddrStorage>(socket.as_raw_fd())?;
        if port_of(&judged) == Some(0) {
            self.tell(chat, BIND, NEVER_BOUND.to_owned());
            return Err(REFUSED);
        }
        if !self.alive(id) {
            return Ok(Answer::Gone);
        }
        // Below zero, as above the system's own cap, the kernel's largest queue.
        let backlog = Backlog::new(backlog).unwrap_or(Backlog::MAXALLOWABLE);
        listen(&socket, backlog)?;
        if getsockname::<SockaddrStorage>(socket.as_raw_fd()).ok() != Some(judged) {
            // Unbound between the judging and the listen, and bound by the kernel now, to an
            // address it picked: a listening socket shut for reading is unconnected again, its
            // address let go. A socket that cannot be shut would go on listening where nothing
            // judged it, so its process is ended instead.
            if shutdown(socket.as_raw_fd(), Shutdown::Read).is_err() {
                let _ = rustix::process::pidfd_send_signal(&caller, rustix::process::Signal::KILL);
            }
            self.tell(chat, BIND, NEVER_BOUND.to_owned());
            return Err(REFUSED);
        }
        Ok(Answer::Value(0))
    }

    /// The error a listen whose socket could not be taken (`why`) answers: `EBADF` for a
    /// descriptor that is not there, and otherwise a refusal, told.
    fn not_taken(&self, chat: &Chat<'_>, why: rustix::io::Errno) -> Errno {
        if why == rustix::io::Errno::BADF {
            return Errno::EBADF;
        }
        self.tell(chat, BIND, NOT_TAKEN.to_owned());
        REFUSED
    }
}

/// The port an internet socket's address `name` holds, port 0 where it cannot be read; None for
/// any other socket.
fn port_of(name: &SockaddrStorage) -> Option<u16> {
    match name.family() {
        Some(AddressFamily::Inet) => Some(name.as_sockaddr_in().map_or(0, SockaddrIn::port)),
        Some(AddressFamily::Inet6) => Some(name.as_sockaddr_in6().map_or(0, SockaddrIn6::port)),
        _ => None,
    }
}

/// What purlis's own profile language has no word for on Linux, and every wrapped chat needs:
/// its terminal and the system's character devices, POSIX semaphores (Seatbelt's
/// `ipc-posix-sem`), and the files of its own processes in `/proc`. None is ever a path a
/// denial class names.
fn linux_own(chat: &Chat<'_>, what: FileOp, path: &Path, kind: Kind) -> bool {
    const DEVICES: [&str; 7] = [
        "/dev/null",
        "/dev/zero",
        "/dev/full",
        "/dev/tty",
        "/dev/ptmx",
        "/dev/random",
        "/dev/urandom",
    ];
    if kind == Kind::CharacterDevice
        && matches!(what, FileOp::ReadData | FileOp::WriteData)
        && (DEVICES.iter().any(|it| path == Path::new(it))
            || path.parent() == Some(Path::new("/dev/pts")))
    {
        return true;
    }
    if path.parent() == Some(Path::new("/dev/shm"))
        && kind == Kind::Regular
        && path
            .file_name()
            .is_some_and(|name| name.as_bytes().starts_with(b"sem."))
    {
        return true;
    }
    if let Ok(rest) = path.strip_prefix("/proc")
        && let Some(first) = rest.components().next()
        && let Ok(pid) = first.as_os_str().to_string_lossy().parse::<i32>()
        && matches!(what, FileOp::WriteData | FileOp::ReadData)
    {
        return chat.world.inside(pid);
    }
    false
}

/// Whether `path` is no file of a process outside the sandbox but its `stat`: `/proc/<n>/…`
/// and `/proc/<n>/task/<t>/…` of a process `n` outside are refused unless the rest is `stat`.
fn outside_process_stat_only(chat: &Chat<'_>, path: &Path) -> bool {
    let Ok(rest) = path.strip_prefix("/proc") else {
        return true;
    };
    let parts: Vec<&OsStr> = rest.iter().collect();
    let Some(pid) = parts
        .first()
        .and_then(|it| it.to_str())
        .and_then(|it| it.parse::<i32>().ok())
    else {
        return true;
    };
    if chat.world.inside(pid) {
        return true;
    }
    let stat = OsStr::new("stat");
    match parts.as_slice() {
        [_, last] => *last == stat,
        [_, task, tid, last] => {
            *task == OsStr::new("task")
                && tid.to_str().is_some_and(|it| it.parse::<i32>().is_ok())
                && *last == stat
        }
        _ => false,
    }
}

/// Whether `ip` is this machine's own loopback address, as a grant of a service here names it.
fn is_this_machine(ip: IpAddr) -> bool {
    ip == IpAddr::V4(Ipv4Addr::LOCALHOST) || ip == IpAddr::V6(Ipv6Addr::LOCALHOST)
}

/// The name the kernel keeps for process `tgid` (its `comm`), where it can be read.
fn process_name(tgid: i32) -> Option<String> {
    let name = std::fs::read_to_string(format!("/proc/{tgid}/comm")).ok()?;
    let name = name.trim_end_matches('\n');
    (!name.is_empty()).then(|| name.to_owned())
}

/// The process's file mode creation mask.
fn umask(tid: i32) -> u32 {
    std::fs::read_to_string(format!("/proc/{tid}/status"))
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Umask:"))
                .and_then(|it| u32::from_str_radix(it.trim(), 8).ok())
        })
        .unwrap_or(0o022)
}

/// The status flags of the chat's descriptor `fd`, with `O_CLOEXEC` for its close-on-start.
fn descriptor_flags(tid: i32, fd: i32) -> Result<i32, Errno> {
    let info =
        std::fs::read_to_string(format!("/proc/{tid}/fdinfo/{fd}")).map_err(|_| Errno::EBADF)?;
    info.lines()
        .find_map(|line| line.strip_prefix("flags:"))
        .and_then(|it| i32::from_str_radix(it.trim(), 8).ok())
        .ok_or(Errno::EBADF)
}

fn ip_of(bytes: &[u8]) -> Result<SocketAddr, Errno> {
    let family = i32::from(u16::from_ne_bytes([bytes[0], bytes[1]]));
    let port = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]);
    match family {
        libc::AF_INET if bytes.len() >= 8 => Ok(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(bytes[4], bytes[5], bytes[6], bytes[7])),
            port(2),
        )),
        libc::AF_INET6 if bytes.len() >= 24 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&bytes[8..24]);
            Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port(2)))
        }
        _ => Err(Errno::EINVAL),
    }
}

/// `len` bytes of the chat's memory at `addr`, read into the supervisor's own.
fn read_bytes(tid: i32, addr: u64, len: usize) -> Result<Vec<u8>, Errno> {
    if addr == 0 {
        return Err(Errno::EFAULT);
    }
    let mut out = vec![0u8; len];
    let got = process_vm_readv(
        Pid::from_raw(tid),
        &mut [IoSliceMut::new(&mut out)],
        &[RemoteIoVec {
            base: usize::try_from(addr).map_err(|_| Errno::EFAULT)?,
            len,
        }],
    )
    .map_err(|_| Errno::EFAULT)?;
    if got != len {
        return Err(Errno::EFAULT);
    }
    Ok(out)
}

/// The path at `addr` in the chat's memory, up to its end, read a page at a time so a path
/// that ends just before memory that is not mapped is still read.
fn read_path(tid: i32, addr: u64) -> Result<Vec<u8>, Errno> {
    if addr == 0 {
        return Err(Errno::EFAULT);
    }
    const PAGE: u64 = 4096;
    let mut out = Vec::new();
    let mut at = addr;
    while out.len() <= PATH_MAX {
        let len = usize::try_from(PAGE - at % PAGE).map_err(|_| Errno::EFAULT)?;
        let chunk = read_bytes(tid, at, len)?;
        if let Some(end) = chunk.iter().position(|c| *c == 0) {
            out.extend_from_slice(&chunk[..end]);
            if out.len() >= PATH_MAX {
                return Err(Errno::ENAMETOOLONG);
            }
            return Ok(out);
        }
        out.extend_from_slice(&chunk);
        at += len as u64;
    }
    Err(Errno::ENAMETOOLONG)
}

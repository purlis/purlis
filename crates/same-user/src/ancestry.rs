// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Who a process was started under: the one parent-chain walker charter has (V82's commit
//! stamping and HP-6's permission hook both climb it), the host check a hook makes with it, and
//! the check `charterd` makes that a client is not inside a chat ([`inside_a_chat`], FD-27),
//! which reads the same parents and fails closed where the others answer no.
//!
//! **Read from the kernel, in this process.** A process's parent and when it started come from
//! `/proc` on Linux and from `proc_pidinfo` on macOS, one pid at a time: no program is started
//! and no table is listed. The macOS call is this workspace's second audited `unsafe` block
//! (the operator's ruling D-1407-8, 2026-10-07; see [`Parents`] and `AGENTS.md`). Which
//! sockets a process holds is still a tool by its absolute path with no environment on macOS
//! (`/usr/sbin/lsof`). Every doubt answers no.

use std::io;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::{NotThisUser, Uid, admit_peer, peer_process_of};

/// The most generations a walk climbs. A real chain is a handful; a table that loops, or a tree
/// deeper than this, is not one to vouch for.
pub const MOST_GENERATIONS: usize = 64;

/// Every process's parent and when it started, as the kernel says now, asked one pid at a time.
///
/// - **Linux:** `/proc/<pid>/stat`.
/// - **macOS:** `proc_pidinfo`, in this process (D-1407-8). It was one `/bin/ps` run per
///   question before: a program started, under the fork lock, to list every process on the
///   machine for the handful asked about.
/// - **Any other unix:** nothing is known, so every walk ends where it began and every doubt
///   answers no.
///
/// **A process that has exited is gone, reaped or not.** On macOS neither reading answers for
/// a zombie (`/bin/ps` listed one, with its start, until its parent reaped it), so a chat's
/// program reads as gone from the moment it exits and not from the moment the app collects
/// it: a process left behind in its session is refused from then on, where it was admitted
/// for that moment before. Linux still answers for a zombie from `/proc`.
///
/// **Not one instant.** Each answer is the kernel's as it was asked, and a chain of them is
/// read over a few microseconds. A number that changed hands in between is told apart where
/// it matters by [`Parents::started`], which callers that bind to a process compare with.
#[derive(Debug, Clone, Copy)]
pub struct Parents;

impl Parents {
    /// The parent of `pid`, where it is known: any user's process, since a walk from a
    /// person's terminal passes through `login`, which is root's.
    pub fn of(pid: u32) -> Option<u32> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            proc_parent(pid)
        }
        #[cfg(target_os = "macos")]
        {
            pidinfo::parent(pid)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            let _ = pid;
            None
        }
    }

    /// When `pid` started, as the kernel says, where it is known: what tells the process with
    /// that number now from one that had it before. Only ever compared for equality, and
    /// exact: clock ticks since boot on Linux, and seconds and microseconds since 1970 on
    /// macOS (`<seconds>.<microseconds>`), where the kernel records it once as the process is
    /// made, so no time zone and no rounding moves it.
    ///
    /// On macOS only this user's processes answer (the kernel refuses the reading for another
    /// user's), which is every process a chat's program can be.
    pub fn started(pid: u32) -> Option<String> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            proc_started(pid)
        }
        #[cfg(target_os = "macos")]
        {
            pidinfo::started(pid)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            let _ = pid;
            None
        }
    }

    /// When `pid` started, as a time of day, where the kernel says one: what tells whether a
    /// process can be one a record written at a known time names (#1542, #1021). `None` is a
    /// doubt, which each caller takes in the direction that fails closed.
    ///
    /// - **macOS:** from the same reading as [`Self::started`], to the microsecond.
    /// - **Linux:** the boot's time of day (`btime` in `/proc/stat`) plus the start's clock
    ///   ticks ([`Self::started`]) at the system's tick rate (`sysconf(_SC_CLK_TCK)`). The boot
    ///   time is whole seconds, and the kernel moves it when the clock is stepped, so this is
    ///   good to a second or two: a caller compares it with a slack, never for equality (which
    ///   [`Self::started`] is for).
    /// - **Any other unix:** `None`.
    pub fn started_at(pid: u32) -> Option<std::time::SystemTime> {
        #[cfg(target_os = "macos")]
        {
            let (seconds, micros) = pidinfo::start(pid)?;
            std::time::UNIX_EPOCH.checked_add(
                std::time::Duration::from_secs(seconds) + std::time::Duration::from_micros(micros),
            )
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let ticks = proc_started(pid)?.parse().ok()?;
            let boot = boot_time_in(&std::fs::read_to_string("/proc/stat").ok()?)?;
            let per_second = nix::unistd::sysconf(nix::unistd::SysconfVar::CLK_TCK)
                .ok()
                .flatten()
                .and_then(|rate| u64::try_from(rate).ok())?;
            time_of_day(boot, ticks, per_second)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            let _ = pid;
            None
        }
    }

    /// `start`'s ancestors, its parent first, up to and without pid 1 (`init`, `launchd`), which
    /// is above everything and so vouches for nothing.
    pub fn chain(start: u32) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut at = start;
        for _ in 0..MOST_GENERATIONS {
            match Self::of(at) {
                Some(up) if up > 1 && up != at && !chain.contains(&up) => {
                    chain.push(up);
                    at = up;
                }
                _ => break,
            }
        }
        chain
    }
}

/// macOS: what the kernel says of one process, through `proc_pidinfo`.
#[cfg(target_os = "macos")]
mod pidinfo {
    use nix::libc;

    /// The larger of the two readings taken: `proc_bsdinfo`, 136 bytes.
    const MOST: usize = size_of::<libc::proc_bsdinfo>();

    /// The parent of `pid`, from `PROC_PIDT_SHORTBSDINFO`: the one reading of a parent the
    /// kernel gives for any user's process.
    pub(super) fn parent(pid: u32) -> Option<u32> {
        let bytes = read(
            pid,
            libc::PROC_PIDT_SHORTBSDINFO,
            size_of::<libc::proc_bsdshortinfo>(),
        )?;
        let named = u32_at(
            &bytes,
            std::mem::offset_of!(libc::proc_bsdshortinfo, pbsi_pid),
        )?;
        (named == pid).then(|| {
            u32_at(
                &bytes,
                std::mem::offset_of!(libc::proc_bsdshortinfo, pbsi_ppid),
            )
        })?
    }

    /// When `pid` started, from `PROC_PIDTBSDINFO`: seconds and microseconds since 1970,
    /// written `<seconds>.<microseconds>`. The kernel answers it for this user's processes.
    pub(super) fn started(pid: u32) -> Option<String> {
        let (seconds, micros) = start(pid)?;
        Some(format!("{seconds}.{micros:06}"))
    }

    /// When `pid` started, from `PROC_PIDTBSDINFO`: seconds and microseconds since 1970.
    pub(super) fn start(pid: u32) -> Option<(u64, u64)> {
        let bytes = read(pid, libc::PROC_PIDTBSDINFO, MOST)?;
        let named = u32_at(&bytes, std::mem::offset_of!(libc::proc_bsdinfo, pbi_pid))?;
        let seconds = u64_at(
            &bytes,
            std::mem::offset_of!(libc::proc_bsdinfo, pbi_start_tvsec),
        )?;
        let micros = u64_at(
            &bytes,
            std::mem::offset_of!(libc::proc_bsdinfo, pbi_start_tvusec),
        )?;
        (named == pid).then_some((seconds, micros))
    }

    fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_ne_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    }

    fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
        Some(u64::from_ne_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
    }

    /// The `flavor` reading of process `pid`, which is exactly `size` bytes, at the front of a
    /// zeroed buffer; or `None` where the kernel gave anything but the whole of it (the
    /// process has gone, is another user's and the flavor is refused, or `size` is not what
    /// this kernel writes).
    ///
    /// **The second of this workspace's two audited `unsafe` blocks**, allowed by the operator
    /// on 2026-10-07 (D-1407-8), and the safe wrapper around it: nothing else calls
    /// `proc_pidinfo` here, and what it wrote is read back as plain bytes.
    #[allow(
        unsafe_code,
        reason = "the second audited block the operator allowed (D-1407-8, 2026-10-07); see the SAFETY comment"
    )]
    fn read(pid: u32, flavor: libc::c_int, size: usize) -> Option<[u8; MOST]> {
        let pid = libc::c_int::try_from(pid).ok().filter(|pid| *pid > 0)?;
        let wanted = libc::c_int::try_from(size)
            .ok()
            .filter(|_| size > 0 && size <= MOST)?;
        let mut bytes = [0u8; MOST];
        // SAFETY: `proc_pidinfo` is one `proc_info` system call that copies at most
        // `buffersize` bytes out to `buffer` and touches nothing else of this process.
        // - `buffer` is `bytes`, a live, exclusively borrowed, zero-initialised array on this
        //   stack frame of `MOST` bytes, and `buffersize` is `wanted`, checked just above to be
        //   in `1..=MOST`: the kernel cannot write past the array, and the pointer does not
        //   outlive the call.
        // - A byte array has no alignment and no invalid values, so whatever the kernel writes,
        //   or does not write, the array stays a valid `[u8; MOST]`. No struct is ever
        //   conjured from it: fields are read back with safe, bounds-checked slicing.
        // - `pid` is only a number to the kernel: a process that has gone, or one this user may
        //   not read, is an error return (0 or less), never undefined behaviour. `arg` is unused
        //   by both flavors asked here and is 0.
        // The return is the bytes written; anything but exactly `wanted` is refused below, so
        // a short or failed reading is never taken for a process.
        let got = unsafe { libc::proc_pidinfo(pid, flavor, 0, bytes.as_mut_ptr().cast(), wanted) };
        (got == wanted).then_some(bytes)
    }
}

/// Climbs from `start` through `parent` until it meets `ancestor`, runs out, or loops.
pub fn walk(start: u32, ancestor: u32, parent: impl Fn(u32) -> Option<u32>) -> bool {
    let mut at = start;
    for _ in 0..MOST_GENERATIONS {
        if at == ancestor {
            return true;
        }
        match parent(at) {
            // Pid 0 is the kernel's, the parent of `init` and of `launchd`: the top.
            Some(up) if up != 0 && up != at => at = up,
            _ => return false,
        }
    }
    false
}

/// The session process `pid` is in: the pid of its session's leader, from the kernel
/// (`getsid`).
pub fn session_of(pid: u32) -> io::Result<u32> {
    let pid = i32::try_from(pid).map_err(|_| io::Error::other("not a pid"))?;
    let sid = nix::unistd::getsid(Some(nix::unistd::Pid::from_raw(pid)))?;
    u32::try_from(sid.as_raw()).map_err(|_| io::Error::other("not a session id"))
}

/// Which of `chats` (each chat's program, by pid) process `peer` runs inside, or `None` when it
/// is in none of them (V16a, ADR 0068 §5). Inside is any of:
///
/// - it IS a chat's program;
/// - its session is a chat's: every chat's program leads its own session, so a process it
///   started stays in that session after its parent has gone;
/// - a chat's program is among its ancestors.
///
/// **Every doubt is an error, never `None`:** a peer whose parent or session cannot be read (it
/// has gone), or whose ancestry loops or runs deeper than [`MOST_GENERATIONS`], is not vouched
/// for as outside. A process that leaves its session AND its parents on purpose is not caught;
/// this is the second layer, and the credential the sandbox keeps from a chat is the first.
pub fn inside_a_chat(
    peer: u32,
    chats: &[u32],
    parent: impl Fn(u32) -> Option<u32>,
    session: impl Fn(u32) -> io::Result<u32>,
) -> io::Result<Option<u32>> {
    if chats.contains(&peer) {
        return Ok(Some(peer));
    }
    let sid = session(peer)?;
    if chats.contains(&sid) {
        return Ok(Some(sid));
    }
    let mut at = peer;
    for _ in 0..MOST_GENERATIONS {
        let up = parent(at).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("process {at} has no parent this host can read"),
            )
        })?;
        if chats.contains(&up) {
            return Ok(Some(up));
        }
        // Pid 1 (`init`, `launchd`) and pid 0 are the top: nothing above them is a chat.
        if up <= 1 {
            return Ok(None);
        }
        if up == at {
            break;
        }
        at = up;
    }
    Err(io::Error::other(format!(
        "process {peer}'s ancestry loops or runs deeper than {MOST_GENERATIONS} generations"
    )))
}

/// When `pid` started, in clock ticks since boot: the 22nd field of `/proc/<pid>/stat`.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn proc_started(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, after) = stat.rsplit_once(')')?;
    // Field 3 is the first after the command; the start time is field 22.
    after.split_whitespace().nth(19).map(str::to_owned)
}

/// The boot's time of day, in seconds since 1970: the `btime` line of Linux's `/proc/stat`.
pub fn boot_time_in(stat: &str) -> Option<u64> {
    stat.lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()
}

/// The time of day `ticks` clock ticks after a boot at `boot` (seconds since 1970), at
/// `per_second` ticks a second; `None` for no tick rate, or a time past what one can hold.
pub fn time_of_day(boot: u64, ticks: u64, per_second: u64) -> Option<std::time::SystemTime> {
    if per_second == 0 {
        return None;
    }
    let whole = ticks / per_second;
    let nanos = (ticks % per_second).checked_mul(1_000_000_000)? / per_second;
    std::time::UNIX_EPOCH
        .checked_add(std::time::Duration::from_secs(boot.checked_add(whole)?))?
        .checked_add(std::time::Duration::from_nanos(nanos))
}

/// The parent `/proc/<pid>/stat` names.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn proc_parent(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat_parent(&stat)
}

/// The parent in one `/proc/<pid>/stat` line: the fourth field, counted after the command's
/// closing parenthesis, because the command itself may hold spaces and parentheses.
pub fn stat_parent(stat: &str) -> Option<u32> {
    let (_, after) = stat.rsplit_once(')')?;
    after.split_whitespace().nth(1)?.parse().ok()
}

/// Why the far end of a socket this process connected to is not the host it expects.
#[derive(Debug, thiserror::Error)]
pub enum NotOurHost {
    #[error("{0}")]
    NotThisUser(NotThisUser),
    /// The process listening is not one of this process's ancestors.
    #[error("the process listening there (pid {pid}) is not one this process was started under")]
    NotAnAncestor { pid: u32 },
    /// The ancestor the socket's peer pid names holds no socket bound at the path: the pid is
    /// one a dead listener left behind, now another process's.
    #[error(
        "the process the socket names (pid {pid}) holds no socket at that path, so it is not \
         the one listening there"
    )]
    HoldsNoSocketThere { pid: u32 },
    #[error("which sockets the listener holds could not be read: {0}")]
    NoSocketTable(io::Error),
}

/// `Ok` when the process listening at the far end of `socket`, connected at `path`, runs as this
/// user, IS one of this process's ancestors, and HOLDS a socket bound at `path` now (FD-6's
/// mutual check, for a socket whose reply carries authority, ADR 0068 amended by HP-6).
///
/// **The peer pid alone is not an identity.** On both kernels it is the number of the process
/// that listened, kept after that process is gone (`SO_PEERCRED`'s pid, macOS's
/// `LOCAL_PEERPID`), and a later process can be given the same number. So the process with that
/// number must also hold the listening socket itself, read from the kernel's own tables:
/// `/proc/net/unix` and `/proc/<pid>/fd` on Linux, `/usr/sbin/lsof` on macOS. A process that was
/// given a dead listener's pid holds no such socket, and nothing a chat runs can hand one to
/// an ancestor. No timestamp is trusted: the socket's are its owner's to set.
///
/// No secret is needed, and none would hold: whatever this process knows, its siblings started
/// from the same environment know too. This process itself never counts: a hook only connects.
pub fn admit_host(
    socket: &impl AsFd,
    path: &Path,
    run: impl FnOnce(&mut Command) -> io::Result<Output>,
) -> Result<(), NotOurHost> {
    let (uid, pid) = match peer_process_of(socket) {
        Ok(peer) => peer,
        Err(why) => {
            return Err(NotOurHost::NotThisUser(NotThisUser::Unidentified(why)));
        }
    };
    admit_peer(Ok(uid), Uid::effective()).map_err(NotOurHost::NotThisUser)?;
    let ancestors = Parents::chain(std::process::id());
    if !ancestors.contains(&pid) {
        return Err(NotOurHost::NotAnAncestor { pid });
    }
    let holds = match holds_a_socket_at(pid, path, run) {
        Ok(holds) => holds,
        // **Inside purlis's own Linux sandbox** (#1040) no process outside it may have its
        // descriptors read (Landlock keeps a sandboxed process from every right to trace one
        // outside). There the listener is told by the pidfd the kernel gives for the far end:
        // a pidfd names one process for its whole life and is never handed to another, so the
        // listener is the ancestor `pid` names, alive, and no process given its number since.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Err(why) if why.kind() == io::ErrorKind::PermissionDenied => {
            listener_is(socket, pid).map_err(NotOurHost::NoSocketTable)?
        }
        Err(why) => return Err(NotOurHost::NoSocketTable(why)),
    };
    judge(pid, &ancestors, holds)
}

/// Whether the process at the far end of `socket` is process `pid` now, by the pidfd the kernel
/// gives for it (`SO_PEERPIDFD`, Linux 6.5): `false` where that process has ended.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn listener_is(socket: &impl AsFd, pid: u32) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    let pidfd = nix::sys::socket::getsockopt(socket, nix::sys::socket::sockopt::PeerPidfd)
        .map_err(io::Error::from)?;
    let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()))?;
    Ok(pidfd_pid(&info) == Some(i64::from(pid)))
}

/// The process a pidfd's `/proc/self/fdinfo` text names: `-1` once it has ended.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn pidfd_pid(info: &str) -> Option<i64> {
    info.lines()
        .find_map(|line| line.strip_prefix("Pid:"))
        .and_then(|it| it.trim().parse().ok())
}

/// The decision [`admit_host`] makes, given what it read: the listener's pid, this process's
/// ancestors, and whether the process with that pid holds a socket bound at the path.
pub fn judge(pid: u32, ancestors: &[u32], holds_the_socket: bool) -> Result<(), NotOurHost> {
    if !ancestors.contains(&pid) {
        return Err(NotOurHost::NotAnAncestor { pid });
    }
    if !holds_the_socket {
        return Err(NotOurHost::HoldsNoSocketThere { pid });
    }
    Ok(())
}

/// The process that listens on the unix socket at `path` now, where it is this user's and holds
/// that socket itself (#1542): the app, at a project's hook socket. `None` where nothing does,
/// or nothing can be vouched for.
///
/// **The same liveness [`admit_host`] reads, without the ancestry.** It connects, takes the
/// listener's uid and pid from the kernel, and asks that the process with that pid holds a
/// socket bound at the path ([`holds_a_socket_at`]), so a pid a crashed app left behind, now
/// another process's, is no holder. A connection that writes nothing is ended by the app's
/// channel unread. Never through a link at the path itself, and only a socket file is
/// connected to: no FIFO or device is opened.
///
/// Every doubt is `None`: what a caller adds from a holder only widens a check it makes from
/// other readings, so a holder missed leaves that check as it was.
pub fn socket_holder(
    path: &Path,
    run: impl FnOnce(&mut Command) -> io::Result<Output>,
) -> Option<u32> {
    use std::os::unix::fs::FileTypeExt;
    if !std::fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_socket()) {
        return None;
    }
    let socket = std::os::unix::net::UnixStream::connect(path).ok()?;
    let (uid, pid) = peer_process_of(&socket).ok()?;
    drop(socket);
    admit_peer(Ok(uid), Uid::effective()).ok()?;
    holds_a_socket_at(pid, path, run)
        .ok()
        .filter(|holds| *holds)
        .map(|_| pid)
}

/// Whether process `pid` holds a unix socket bound at `path` now. Paths are compared once both
/// are resolved, so `/tmp` and `/private/tmp`, or a link to the socket, name the same one.
pub fn holds_a_socket_at(
    pid: u32,
    path: &Path,
    run: impl FnOnce(&mut Command) -> io::Result<Output>,
) -> io::Result<bool> {
    let target = std::fs::canonicalize(path)?;
    if cfg!(any(target_os = "linux", target_os = "android")) {
        let table = std::fs::read_to_string("/proc/net/unix")?;
        let inodes = listening_at(&table, &target);
        if inodes.is_empty() {
            return Ok(false);
        }
        for fd in std::fs::read_dir(format!("/proc/{pid}/fd"))? {
            let Ok(link) = std::fs::read_link(fd?.path()) else {
                continue;
            };
            if let Some(inode) = socket_inode(&link)
                && inodes.contains(&inode)
            {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let out = run(&mut lsof_command(pid))?;
    // `lsof` exits 1 when the process holds no unix socket at all: an answer, not a failure,
    // as long as it printed nothing. Any other failure is one.
    if !out.status.success() && !(out.status.code() == Some(1) && out.stdout.is_empty()) {
        return Err(io::Error::other("lsof did not list the process's sockets"));
    }
    Ok(lsof_lists(
        &String::from_utf8_lossy(&out.stdout),
        path,
        &target,
    ))
}

/// Whether `lsof -F n`'s output `text` lists a socket bound at `path`, which resolves to
/// `target` (#1146). `lsof` prints a name with every byte that is not printable ASCII escaped
/// ([`lsof_spelling`]): a newline as `\n`, a byte past ASCII as `\xNN`. So a name is the
/// socket's when it resolves to `target` as printed, or when it is the escaped spelling of
/// `path` or of `target` exactly. Nothing shorter or near is taken for it.
pub fn lsof_lists(text: &str, path: &Path, target: &Path) -> bool {
    let spelled = [lsof_spelling(path), lsof_spelling(target)];
    lsof_names(text)
        .iter()
        .any(|name| spelled.contains(name) || resolves_to(name, target))
}

/// `path` as `lsof` prints a name, in the C locale it is run in ([`lsof_command`]): each
/// printable ASCII byte as it is, a backspace, form feed, newline, carriage return and tab as
/// `\b`, `\f`, `\n`, `\r`, `\t`, every other control byte as `^` and the letter 0x40 above
/// it, `0xff` as `^?`, and every other byte as `\x` and its two hex digits.
pub fn lsof_spelling(path: &Path) -> String {
    let mut spelled = String::new();
    for byte in path.as_os_str().as_encoded_bytes() {
        match *byte {
            b' '..=b'~' => spelled.push(char::from(*byte)),
            0x08 => spelled.push_str("\\b"),
            0x0c => spelled.push_str("\\f"),
            b'\n' => spelled.push_str("\\n"),
            b'\r' => spelled.push_str("\\r"),
            b'\t' => spelled.push_str("\\t"),
            control @ 0..=0x1f => {
                spelled.push('^');
                spelled.push(char::from(control + 0x40));
            }
            0xff => spelled.push_str("^?"),
            other => spelled.push_str(&format!("\\x{other:02x}")),
        }
    }
    spelled
}

/// `lsof` asked for the unix sockets of process `pid` alone, by its full path, with no
/// environment.
pub fn lsof_command(pid: u32) -> Command {
    let mut lsof = Command::new("/usr/sbin/lsof");
    lsof.args(["-nP", "-a", "-p", &pid.to_string(), "-U", "-F", "n"])
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    lsof
}

/// The names `lsof -F n` printed (`n<name>` lines) that are paths: a socket bound at a path,
/// listening or accepted from it. A connected client's is `->0x…`, never a path.
pub fn lsof_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix('n'))
        .filter(|name| name.starts_with('/'))
        .map(str::to_owned)
        .collect()
}

/// The inodes of `/proc/net/unix`'s listening sockets bound at a path that resolves to
/// `target`. A listening socket carries `__SO_ACCEPTCON` (`0x10000`) in its flags.
///
/// **A path is read whole, as the kernel wrote it** (#1146): everything after the one space
/// that follows the inode, its own spaces kept, and a path with a newline in it, which the
/// kernel writes raw, runs on into the lines after it until the next line that opens an entry.
pub fn listening_at(table: &str, target: &Path) -> Vec<u64> {
    let mut entries: Vec<(u64, u64, String)> = Vec::new();
    for line in table.lines().skip(1) {
        match entry_of(line) {
            Some(entry) => entries.push(entry),
            None => {
                if let Some((_, _, path)) = entries.last_mut() {
                    path.push('\n');
                    path.push_str(line);
                }
            }
        }
    }
    entries
        .into_iter()
        .filter(|(flags, _, path)| flags & 0x10000 != 0 && resolves_to(path, target))
        .map(|(_, inode, _)| inode)
        .collect()
}

/// One line of `/proc/net/unix` that opens an entry, as its flags, inode and path (empty for a
/// socket bound at none): `Num: RefCount Protocol Flags Type St Inode[ Path]`, where `Num` is
/// hex and ends in a colon and the six fields after it are numbers.
fn entry_of(line: &str) -> Option<(u64, u64, String)> {
    let mut rest = line;
    let mut fields = Vec::with_capacity(7);
    for _ in 0..7 {
        let start = rest.len() - rest.trim_start_matches(' ').len();
        rest = &rest[start..];
        let end = rest.find(' ').unwrap_or(rest.len());
        fields.push(&rest[..end]);
        rest = &rest[end..];
    }
    let num = fields[0].strip_suffix(':')?;
    if num.is_empty() || !num.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    for hex in &fields[1..6] {
        if hex.is_empty() || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
    }
    let flags = u64::from_str_radix(fields[3], 16).ok()?;
    let inode = fields[6].parse().ok()?;
    let path = rest.strip_prefix(' ').unwrap_or(rest).to_owned();
    Some((flags, inode, path))
}

/// The inode of a `/proc/<pid>/fd` link that is a socket: `socket:[<inode>]`.
fn socket_inode(link: &Path) -> Option<u64> {
    link.to_str()?
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// Whether `name`, a path a socket was bound at, resolves to `target`. A path that no longer
/// resolves names nothing.
fn resolves_to(name: &str, target: &Path) -> bool {
    !name.is_empty() && std::fs::canonicalize(PathBuf::from(name)).is_ok_and(|at| at == target)
}

#[cfg(test)]
mod tests;

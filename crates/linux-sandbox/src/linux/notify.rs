// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The seccomp notification calls: the workspace's third audited `unsafe` block.**
//!
//! The operator's ruling of 2026-10-16 (#1040): the low-level seccomp notification calls live
//! in one more small, isolated, audited `unsafe` module, with no new C dependency. Neither the
//! standard library, nor nix, nor seccompiler has a safe call for them: installing a filter
//! that returns a listener, and the four `ioctl`s on that listener (receive a notification,
//! answer it, ask whether it is still valid, hand the notifying process a descriptor).
//!
//! Everything else is safe code around [`raw`], the one function that holds the block. Each
//! call is a variant of [`Call`] that carries Rust references of exactly the type the kernel
//! reads or writes, so the block never builds a pointer from an integer and never chooses a
//! size. [`check_sizes`] refuses to serve at all where the running kernel's structures are
//! larger than the ones this crate was built with, which is what makes `Receive` sound.

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

/// One notification call, with the memory the kernel reads or writes.
pub(super) enum Call<'a> {
    /// `seccomp(SECCOMP_SET_MODE_FILTER, SECCOMP_FILTER_FLAG_NEW_LISTENER, program)`: install
    /// `program` on the calling thread and return the listener's descriptor.
    Install { program: &'a [libc::sock_filter] },
    /// `seccomp(SECCOMP_GET_NOTIF_SIZES, 0, out)`: the sizes of the kernel's structures.
    Sizes {
        out: &'a mut libc::seccomp_notif_sizes,
    },
    /// `SECCOMP_IOCTL_NOTIF_RECV`: wait for the next notification and write it into `out`.
    Receive {
        listener: BorrowedFd<'a>,
        out: &'a mut libc::seccomp_notif,
    },
    /// `SECCOMP_IOCTL_NOTIF_SEND`: answer a notification.
    Respond {
        listener: BorrowedFd<'a>,
        answer: &'a mut libc::seccomp_notif_resp,
    },
    /// `SECCOMP_IOCTL_NOTIF_ID_VALID`: whether the notifying call `id` is still waiting.
    Valid {
        listener: BorrowedFd<'a>,
        id: &'a mut u64,
    },
    /// `SECCOMP_IOCTL_NOTIF_ADDFD`: put a descriptor into the notifying process.
    AddFd {
        listener: BorrowedFd<'a>,
        add: &'a mut libc::seccomp_notif_addfd,
    },
    /// Take ownership of descriptor `fd`, which nothing in this process owns: the listener as
    /// it arrives over the hand-over socket, and that socket itself in the chat's side.
    Adopt { fd: RawFd },
}

/// What a call returned.
pub(super) enum Got {
    /// A number: a size, a descriptor's number in the notifying process, or zero.
    Number(i64),
    /// A descriptor the kernel made for this process: the listener.
    Fd(OwnedFd),
}

/// `call`, made. Its result as the kernel returns it, or the error it set.
#[allow(
    unsafe_code,
    reason = "the third audited block the operator allowed (2026-10-16, #1040); see the SAFETY comment"
)]
fn raw(call: Call<'_>) -> io::Result<Got> {
    // SAFETY: every pointer handed to the kernel is made here from a Rust reference that lives
    // for the whole call, of exactly the C type the kernel reads or writes for that request:
    // - `Install`: a `sock_fprog` built on the stack from `program`, a slice whose length is
    //   checked to fit `sock_fprog.len` (a `u16`) before the call; the kernel copies the
    //   program and keeps no pointer to it. The filter it installs is the caller's: it can
    //   only narrow what this thread may do. No memory of ours is written. The number it
    //   returns is a descriptor the kernel has just made for this process, which nothing else
    //   holds, so adopting it as an `OwnedFd` gives it exactly one owner.
    // - `Sizes`: `out` is a `seccomp_notif_sizes`, the struct the kernel writes (three
    //   `u16`s, never larger: the request has no size of its own and the kernel writes that
    //   struct exactly).
    // - `Receive`: `out` is a `seccomp_notif`. The kernel writes `sizeof(struct seccomp_notif)`
    //   as it knows it; `check_sizes` has already refused to serve where that is larger than
    //   ours, and `out` is zeroed first as the kernel requires.
    // - `Respond`, `Valid`, `AddFd`: `answer`, `id` and `add` are a `seccomp_notif_resp`, a
    //   `u64` and a `seccomp_notif_addfd`, which the kernel only reads (`check_sizes` holds the
    //   first to its size likewise); `AddFd` names a descriptor the caller owns, which the
    //   kernel duplicates into the notifying process and does not take from us.
    // - `Adopt`: `fd` is a descriptor no Rust value in this process owns, so the `OwnedFd`
    //   made from it is its one owner. Two callers make it, and only these: the supervisor,
    //   with a number the kernel has just written into this process with `SCM_RIGHTS`
    //   (received, so new, and handed to nothing else); and the chat's side, at its start,
    //   with the number of the socket the supervisor gave it at `exec`, which no value of a
    //   process just started owns. Numbers 0 to 2 are refused before the block: the standard
    //   library uses those. A number that is not open is refused by `fcntl(F_GETFD)` before
    //   it is adopted.
    // The listener is a `BorrowedFd`, so it is open for the whole call. `syscall` and `ioctl`
    // are variadic C functions; every argument passed has the width the kernel reads.
    let got: i64 = unsafe {
        match call {
            Call::Install { program } => {
                let len = u16::try_from(program.len())
                    .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
                let prog = libc::sock_fprog {
                    len,
                    filter: program.as_ptr().cast_mut(),
                };
                let fd = libc::syscall(
                    libc::SYS_seccomp,
                    libc::SECCOMP_SET_MODE_FILTER as libc::c_ulong,
                    libc::SECCOMP_FILTER_FLAG_NEW_LISTENER,
                    &raw const prog,
                );
                if let Ok(fd) = i32::try_from(fd)
                    && fd >= 0
                {
                    return Ok(Got::Fd(OwnedFd::from_raw_fd(fd)));
                }
                fd
            }
            Call::Sizes { out } => libc::syscall(
                libc::SYS_seccomp,
                libc::SECCOMP_GET_NOTIF_SIZES as libc::c_ulong,
                0 as libc::c_ulong,
                &raw mut *out,
            ),
            Call::Receive { listener, out } => {
                *out = std::mem::zeroed();
                i64::from(libc::ioctl(
                    listener.as_raw_fd(),
                    libc::SECCOMP_IOCTL_NOTIF_RECV,
                    &raw mut *out,
                ))
            }
            Call::Respond { listener, answer } => i64::from(libc::ioctl(
                listener.as_raw_fd(),
                libc::SECCOMP_IOCTL_NOTIF_SEND,
                &raw mut *answer,
            )),
            Call::Valid { listener, id } => i64::from(libc::ioctl(
                listener.as_raw_fd(),
                libc::SECCOMP_IOCTL_NOTIF_ID_VALID,
                &raw mut *id,
            )),
            Call::AddFd { listener, add } => i64::from(libc::ioctl(
                listener.as_raw_fd(),
                libc::SECCOMP_IOCTL_NOTIF_ADDFD,
                &raw mut *add,
            )),
            Call::Adopt { fd } => {
                if fd <= 2 {
                    return Err(io::Error::from_raw_os_error(libc::EBADF));
                }
                if libc::fcntl(fd, libc::F_GETFD) < 0 {
                    return Err(io::Error::last_os_error());
                }
                return Ok(Got::Fd(OwnedFd::from_raw_fd(fd)));
            }
        }
    };
    if got < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Got::Number(got))
}

/// The number a call returned, which every call but `Install` does.
fn number(got: io::Result<Got>) -> io::Result<i64> {
    match got? {
        Got::Number(it) => Ok(it),
        Got::Fd(_) => Err(io::Error::from_raw_os_error(libc::EINVAL)),
    }
}

/// Installs `program` on this thread, and returns its listener.
pub(super) fn install(program: &[libc::sock_filter]) -> io::Result<OwnedFd> {
    match raw(Call::Install { program })? {
        Got::Fd(fd) => Ok(fd),
        Got::Number(_) => Err(io::Error::from_raw_os_error(libc::EBADF)),
    }
}

/// Descriptor `fd`, which nothing in this process owns, owned (see `Call::Adopt`).
pub(super) fn adopt(fd: RawFd) -> io::Result<OwnedFd> {
    match raw(Call::Adopt { fd })? {
        Got::Fd(fd) => Ok(fd),
        Got::Number(_) => Err(io::Error::from_raw_os_error(libc::EBADF)),
    }
}

/// The sizes the kernel uses, refused where any is larger than this crate's.
pub(super) fn check_sizes() -> io::Result<()> {
    let mut sizes = libc::seccomp_notif_sizes {
        seccomp_notif: 0,
        seccomp_notif_resp: 0,
        seccomp_data: 0,
    };
    number(raw(Call::Sizes { out: &mut sizes }))?;
    let fits = usize::from(sizes.seccomp_notif) <= std::mem::size_of::<libc::seccomp_notif>()
        && usize::from(sizes.seccomp_notif_resp) <= std::mem::size_of::<libc::seccomp_notif_resp>()
        && usize::from(sizes.seccomp_data) <= std::mem::size_of::<libc::seccomp_data>();
    if fits {
        Ok(())
    } else {
        Err(io::Error::other(
            "this kernel's seccomp notifications are larger than purlis knows",
        ))
    }
}

/// The next notification on `listener`.
pub(super) fn receive(listener: BorrowedFd<'_>) -> io::Result<libc::seccomp_notif> {
    let mut out = libc::seccomp_notif {
        id: 0,
        pid: 0,
        flags: 0,
        data: libc::seccomp_data {
            nr: 0,
            arch: 0,
            instruction_pointer: 0,
            args: [0; 6],
        },
    };
    number(raw(Call::Receive {
        listener,
        out: &mut out,
    }))?;
    Ok(out)
}

/// Answers notification `id` with `value`, or with the error `errno` when it is not zero.
pub(super) fn respond(listener: BorrowedFd<'_>, id: u64, value: i64, errno: i32) -> io::Result<()> {
    let mut answer = libc::seccomp_notif_resp {
        id,
        val: value,
        error: -errno,
        flags: 0,
    };
    number(raw(Call::Respond {
        listener,
        answer: &mut answer,
    }))
    .map(drop)
}

/// Lets notification `id`'s call go on in the kernel as the process made it
/// (`SECCOMP_USER_NOTIF_FLAG_CONTINUE`). Only for a call nothing the process changes after it
/// was read could carry past what holds it anyway: the chat's own Landlock layer judges the
/// call the kernel then makes.
pub(super) fn go_on(listener: BorrowedFd<'_>, id: u64) -> io::Result<()> {
    let mut answer = libc::seccomp_notif_resp {
        id,
        val: 0,
        error: 0,
        flags: libc::SECCOMP_USER_NOTIF_FLAG_CONTINUE as u32,
    };
    number(raw(Call::Respond {
        listener,
        answer: &mut answer,
    }))
    .map(drop)
}

/// Whether notification `id` still waits: its process has not gone, nor its call been
/// interrupted, since it was received.
pub(super) fn valid(listener: BorrowedFd<'_>, id: u64) -> bool {
    let mut id = id;
    number(raw(Call::Valid {
        listener,
        id: &mut id,
    }))
    .is_ok()
}

/// Puts `fd` into the process of notification `id`: at the number `at` where given, replacing
/// what is there, or at its lowest free number. With `send`, that also answers the call with
/// the number. The number it got.
pub(super) fn add_fd(
    listener: BorrowedFd<'_>,
    id: u64,
    fd: BorrowedFd<'_>,
    at: Option<u32>,
    cloexec: bool,
    send: bool,
) -> io::Result<i64> {
    let mut flags = 0;
    if at.is_some() {
        flags |= libc::SECCOMP_ADDFD_FLAG_SETFD as u32;
    }
    if send {
        flags |= libc::SECCOMP_ADDFD_FLAG_SEND as u32;
    }
    let mut add = libc::seccomp_notif_addfd {
        id,
        flags,
        srcfd: u32::try_from(fd.as_raw_fd())
            .map_err(|_| io::Error::from_raw_os_error(libc::EBADF))?,
        newfd: at.unwrap_or(0),
        newfd_flags: if cloexec { libc::O_CLOEXEC as u32 } else { 0 },
    };
    number(raw(Call::AddFd {
        listener,
        add: &mut add,
    }))
}

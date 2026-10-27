// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seccomp filters a wrapped chat applies to itself before its program starts.
//!
//! - **Handed to the supervisor** ([`supervised`]): every call a path or an address decides
//!   (opening, making, moving, linking, removing a file, changing its mode or times, and
//!   connecting a socket, and listening on one), which the supervisor judges and performs
//!   itself.
//! - **Refused** ([`refused`]), each with the error a program can take in its stride: every
//!   call of the x32 ABI; a change of a file's owner or extended attributes; datagram, raw and packet sockets, and a stream socket of any family but unix, IPv4, IPv6
//!   and the kernel's own routing table; TCP Fast Open, which connects without `connect`;
//!   io_uring and `openat2`, whose arguments a filter cannot read; the kernel's key store;
//!   opening a file by handle; and a seccomp listener of the chat's own, which would take the
//!   supervisor's calls.
//! - **Answered as done** without being done: making the process undumpable, which would stop
//!   the supervisor reading its calls.

use std::collections::BTreeMap;

use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen as Len, SeccompCmpOp as Cmp, SeccompCondition,
    SeccompFilter, SeccompRule, TargetArch,
};

/// The marker the supervised filter is compiled with, then turned into a user notification:
/// seccompiler has no action for one.
const MARK: u32 = 0x1040;

/// `fchmodat2`, the same number on every architecture (Linux 6.6), which `libc` does not name on
/// each of them.
pub(super) const SYS_FCHMODAT2: i64 = 452;

/// `setxattrat` and `removexattrat` (Linux 6.13), the same numbers on every architecture.
const SYS_SETXATTRAT: i64 = 463;
const SYS_REMOVEXATTRAT: i64 = 466;

/// `SOCK_PACKET`, the old packet socket type, by its number: `libc` marks the name deprecated.
const SOCK_PACKET: i32 = 10;

/// The calls handed to the supervisor on this architecture.
pub(super) fn supervised_calls() -> Vec<i64> {
    let mut calls = vec![
        libc::SYS_openat,
        libc::SYS_mkdirat,
        libc::SYS_mknodat,
        libc::SYS_renameat2,
        libc::SYS_linkat,
        libc::SYS_symlinkat,
        libc::SYS_unlinkat,
        libc::SYS_truncate,
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        SYS_FCHMODAT2,
        libc::SYS_utimensat,
        libc::SYS_connect,
        libc::SYS_listen,
        libc::SYS_bind,
    ];
    #[cfg(target_arch = "x86_64")]
    calls.extend([
        libc::SYS_open,
        libc::SYS_creat,
        libc::SYS_mkdir,
        libc::SYS_mknod,
        libc::SYS_rename,
        libc::SYS_renameat,
        libc::SYS_link,
        libc::SYS_symlink,
        libc::SYS_unlink,
        libc::SYS_rmdir,
        libc::SYS_chmod,
    ]);
    #[cfg(target_arch = "aarch64")]
    calls.push(libc::SYS_renameat);
    calls
}

fn arch() -> Result<TargetArch, String> {
    TargetArch::try_from(std::env::consts::ARCH).map_err(|why| why.to_string())
}

fn compiled(
    rules: BTreeMap<i64, Vec<SeccompRule>>,
    action: SeccompAction,
) -> Result<BpfProgram, String> {
    let filter = SeccompFilter::new(rules, SeccompAction::Allow, action, arch()?)
        .map_err(|why| why.to_string())?;
    filter
        .try_into()
        .map_err(|why: seccompiler::BackendError| why.to_string())
}

/// A rule matching when every condition holds. Each compares an argument's low 32 bits, which
/// are all the kernel reads of an `int`: what the upper half holds cannot steer it past a rule.
fn when(conditions: Vec<(u8, Cmp, u64)>) -> Result<SeccompRule, String> {
    let conditions = conditions
        .into_iter()
        .map(|(arg, cmp, value)| SeccompCondition::new(arg, Len::Dword, cmp, value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|why| why.to_string())?;
    SeccompRule::new(conditions).map_err(|why| why.to_string())
}

/// The filter that hands each supervised call to the supervisor, as `libc`'s filter type.
///
/// A path-only open (`O_PATH`) is the kernel's, under the chat's own Landlock layer; what is
/// done through its descriptor later is supervised.
pub(super) fn supervised() -> Result<Vec<libc::sock_filter>, String> {
    let path_only = u64::try_from(libc::O_PATH).unwrap_or(u64::MAX);
    let not_path_only = |flags: u8| when(vec![(flags, Cmp::MaskedEq(path_only), 0)]);
    let rules = supervised_calls()
        .into_iter()
        .map(|call| {
            #[cfg(target_arch = "x86_64")]
            if call == libc::SYS_open {
                return not_path_only(1).map(|rule| (call, vec![rule]));
            }
            if call == libc::SYS_openat {
                return not_path_only(2).map(|rule| (call, vec![rule]));
            }
            Ok((call, Vec::new()))
        })
        .collect::<Result<_, String>>()?;
    let program = compiled(rules, SeccompAction::Trace(MARK))?;
    let traced = libc::SECCOMP_RET_TRACE | MARK;
    Ok(program
        .into_iter()
        .map(|it| libc::sock_filter {
            code: it.code,
            jt: it.jt,
            jf: it.jf,
            k: if it.code == RET_K && it.k == traced {
                libc::SECCOMP_RET_USER_NOTIF
            } else {
                it.k
            },
        })
        .collect())
}

/// `BPF_RET | BPF_K`: the instruction that returns a constant action.
const RET_K: u16 = 0x06;

const SOCK_TYPE: u64 = 0xf;

/// `__X32_SYSCALL_BIT`: a call numbered at or above it is one of the x32 ABI's.
#[cfg(target_arch = "x86_64")]
pub(super) const X32_BIT: u32 = 0x4000_0000;

/// The filter that refuses every x32 call with `ENOSYS`: the other filters name x86-64's
/// numbers, so a call of the other ABI is never one they hand over or refuse.
#[cfg(target_arch = "x86_64")]
pub(super) fn x32_refused() -> BpfProgram {
    use seccompiler::sock_filter;
    const LD_W_ABS: u16 = 0x20;
    const JEQ_K: u16 = 0x15;
    const JGE_K: u16 = 0x35;
    const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
    let at = |code, k, jt, jf| sock_filter { code, jt, jf, k };
    vec![
        // The architecture, at offset 4 of `seccomp_data`: any other is killed, as the other
        // filters do.
        at(LD_W_ABS, 4, 0, 0),
        at(JEQ_K, AUDIT_ARCH_X86_64, 1, 0),
        at(RET_K, libc::SECCOMP_RET_KILL_PROCESS, 0, 0),
        // The call's number, at offset 0.
        at(LD_W_ABS, 0, 0, 0),
        at(JGE_K, X32_BIT, 0, 1),
        at(
            RET_K,
            libc::SECCOMP_RET_ERRNO | u32::try_from(libc::ENOSYS).unwrap_or(38),
            0,
            0,
        ),
        at(RET_K, libc::SECCOMP_RET_ALLOW, 0, 0),
    ]
}

/// The refusing filters, each with its error, to apply in turn.
pub(super) fn refused() -> Result<Vec<BpfProgram>, String> {
    let af = |family: i32| u64::try_from(family).unwrap_or(u64::MAX);
    let ty = |kind: i32| u64::try_from(kind).unwrap_or(u64::MAX);
    let mut out = Vec::new();
    #[cfg(target_arch = "x86_64")]
    out.push(x32_refused());

    // A file's owner and its extended attributes: no harness sets either, and an attribute can
    // carry a file's permissions.
    let mut not_supported = BTreeMap::new();
    for call in [
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        SYS_SETXATTRAT,
        SYS_REMOVEXATTRAT,
    ] {
        not_supported.insert(call, Vec::new());
    }
    out.push(compiled(
        not_supported,
        SeccompAction::Errno(u32::try_from(libc::EOPNOTSUPP).unwrap_or(95)),
    )?);
    let mut owner = BTreeMap::new();
    #[cfg(target_arch = "x86_64")]
    for call in [libc::SYS_chown, libc::SYS_lchown] {
        owner.insert(call, Vec::new());
    }
    for call in [libc::SYS_fchown, libc::SYS_fchownat] {
        owner.insert(call, Vec::new());
    }
    out.push(compiled(
        owner,
        SeccompAction::Errno(u32::try_from(libc::EPERM).unwrap_or(1)),
    )?);

    // Sockets: unix and internet streams, and the kernel's routing table, only.
    let mut socket = vec![when(vec![
        (0, Cmp::Ne, af(libc::AF_UNIX)),
        (0, Cmp::Ne, af(libc::AF_INET)),
        (0, Cmp::Ne, af(libc::AF_INET6)),
        (0, Cmp::Ne, af(libc::AF_NETLINK)),
    ])?];
    for family in [libc::AF_UNIX, libc::AF_INET, libc::AF_INET6] {
        for kind in [
            libc::SOCK_DGRAM,
            libc::SOCK_RAW,
            libc::SOCK_RDM,
            libc::SOCK_SEQPACKET,
            libc::SOCK_DCCP,
            SOCK_PACKET,
        ] {
            socket.push(when(vec![
                (0, Cmp::Eq, af(family)),
                (1, Cmp::MaskedEq(SOCK_TYPE), ty(kind)),
            ])?);
        }
    }
    for family in [libc::AF_INET, libc::AF_INET6] {
        socket.push(when(vec![
            (0, Cmp::Eq, af(family)),
            (2, Cmp::Ne, 0),
            (2, Cmp::Ne, ty(libc::IPPROTO_TCP)),
        ])?);
    }
    socket.push(when(vec![
        (0, Cmp::Eq, af(libc::AF_NETLINK)),
        (2, Cmp::Ne, ty(libc::NETLINK_ROUTE)),
    ])?);
    // A connected pair of unix sockets: streams and sequenced packets, never datagrams, which
    // a send can address to any socket on this machine.
    let pair = vec![
        when(vec![(0, Cmp::Ne, af(libc::AF_UNIX))])?,
        when(vec![(1, Cmp::MaskedEq(SOCK_TYPE), ty(libc::SOCK_DGRAM))])?,
    ];
    let fast_open = u64::try_from(libc::MSG_FASTOPEN).unwrap_or(u64::MAX);
    let mut access = BTreeMap::new();
    access.insert(libc::SYS_socket, socket);
    access.insert(libc::SYS_socketpair, pair);
    access.insert(
        libc::SYS_sendto,
        vec![when(vec![(3, Cmp::MaskedEq(fast_open), fast_open)])?],
    );
    access.insert(
        libc::SYS_sendmsg,
        vec![when(vec![(2, Cmp::MaskedEq(fast_open), fast_open)])?],
    );
    access.insert(
        libc::SYS_sendmmsg,
        vec![when(vec![(3, Cmp::MaskedEq(fast_open), fast_open)])?],
    );
    out.push(compiled(
        access,
        SeccompAction::Errno(u32::try_from(libc::EACCES).unwrap_or(13)),
    )?);

    // Calls whose arguments live in memory a filter cannot read, so none is supervised:
    // programs fall back to the calls that are.
    let mut nosys = BTreeMap::new();
    #[cfg(target_arch = "x86_64")]
    for call in [libc::SYS_utime, libc::SYS_utimes, libc::SYS_futimesat] {
        nosys.insert(call, Vec::new());
    }
    for call in [
        libc::SYS_openat2,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
    ] {
        nosys.insert(call, Vec::new());
    }
    out.push(compiled(
        nosys,
        SeccompAction::Errno(u32::try_from(libc::ENOSYS).unwrap_or(38)),
    )?);

    // The kernel's key store, a file opened by handle or as a library, and a listener of the
    // chat's own.
    let mut perm = BTreeMap::new();
    #[cfg(target_arch = "x86_64")]
    perm.insert(libc::SYS_uselib, Vec::new());
    for call in [
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_name_to_handle_at,
        libc::SYS_open_by_handle_at,
    ] {
        perm.insert(call, Vec::new());
    }
    let listener: u64 = libc::SECCOMP_FILTER_FLAG_NEW_LISTENER;
    perm.insert(
        libc::SYS_seccomp,
        vec![when(vec![
            (0, Cmp::Eq, u64::from(libc::SECCOMP_SET_MODE_FILTER)),
            (1, Cmp::MaskedEq(listener), listener),
        ])?],
    );
    out.push(compiled(
        perm,
        SeccompAction::Errno(u32::try_from(libc::EPERM).unwrap_or(1)),
    )?);

    // Undumpable: answered as done, so the supervisor can still read the process's calls.
    let mut dumpable = BTreeMap::new();
    dumpable.insert(
        libc::SYS_prctl,
        vec![when(vec![
            (0, Cmp::Eq, ty(libc::PR_SET_DUMPABLE)),
            (1, Cmp::Eq, 0),
        ])?],
    );
    out.push(compiled(dumpable, SeccompAction::Errno(0))?);
    Ok(out)
}

#[cfg(test)]
mod tests;

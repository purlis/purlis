// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seccomp filters a wrapped chat applies to itself before its program starts.
//!
//! - **Handed to the supervisor** ([`supervised`]): every call a path or an address decides
//!   (opening, making, moving, linking, removing and changing the mode of a file, and
//!   connecting a socket), which the supervisor judges and performs itself.
//! - **Refused** ([`refused`]), each with the error a program can take in its stride:
//!   datagram, raw and packet sockets, and a stream socket of any family but unix, IPv4, IPv6
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
        libc::SYS_connect,
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
/// **But a path-only open.** `O_PATH` gives a descriptor that reads, writes and runs nothing,
/// as `stat` does; what is done through it later (opening it again by `/proc/self/fd`, linking
/// it) is a supervised call of its own. The kernel opens it directly, under the chat's own
/// Landlock layer, since a path-only descriptor is one the kernel will not hand over to another
/// process.
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

/// The refusing filters, each with its error, to apply in turn.
pub(super) fn refused() -> Result<Vec<BpfProgram>, String> {
    let af = |family: i32| u64::try_from(family).unwrap_or(u64::MAX);
    let ty = |kind: i32| u64::try_from(kind).unwrap_or(u64::MAX);
    let mut out = Vec::new();

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

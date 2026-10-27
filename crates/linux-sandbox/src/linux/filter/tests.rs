// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The compiled filters, read: each run on a call the way the kernel runs a classic BPF program
//! on `seccomp_data`, and the strongest action of them all taken, as the kernel takes it.

use super::*;

/// The `seccomp_data` of a call `nr` with `args`, on this architecture.
fn data(nr: u32, args: [u64; 6]) -> Vec<u8> {
    let arch: u32 = if cfg!(target_arch = "x86_64") {
        0xc000_003e
    } else {
        0xc000_00b7
    };
    let mut out = Vec::new();
    out.extend(nr.to_ne_bytes());
    out.extend(arch.to_ne_bytes());
    out.extend(0u64.to_ne_bytes());
    for arg in args {
        out.extend(arg.to_ne_bytes());
    }
    out
}

/// What one program returns for `data`.
fn run(program: &[seccompiler::sock_filter], data: &[u8]) -> u32 {
    let word = |at: u32| {
        let at = at as usize;
        u32::from_ne_bytes(data[at..at + 4].try_into().expect("a word"))
    };
    let (mut acc, mut pc) = (0u32, 0usize);
    loop {
        let it = &program[pc];
        pc += 1;
        let jump = |taken: bool| usize::from(if taken { it.jt } else { it.jf });
        match it.code {
            0x20 => acc = word(it.k),
            0x54 => acc &= it.k,
            0x05 => pc += it.k as usize,
            0x15 => pc += jump(acc == it.k),
            0x25 => pc += jump(acc > it.k),
            0x35 => pc += jump(acc >= it.k),
            0x45 => pc += jump(acc & it.k != 0),
            0x06 => return it.k,
            other => panic!("an instruction this reading does not know: {other:#x}"),
        }
    }
}

/// What every filter the chat applies returns for a call, the strongest first, as the kernel
/// ranks actions.
fn verdict(nr: i64, args: [u64; 6]) -> u32 {
    let data = data(u32::try_from(nr).expect("a call number"), args);
    let mut all: Vec<Vec<seccompiler::sock_filter>> = refused().expect("compiled");
    all.push(
        supervised()
            .expect("compiled")
            .into_iter()
            .map(|it| seccompiler::sock_filter {
                code: it.code,
                jt: it.jt,
                jf: it.jf,
                k: it.k,
            })
            .collect(),
    );
    all.iter()
        .map(|program| run(program, &data))
        .min_by_key(|action| (*action & libc::SECCOMP_RET_ACTION_FULL) as i32)
        .expect("a filter")
}

fn errno(code: i32) -> u32 {
    libc::SECCOMP_RET_ERRNO | u32::try_from(code).expect("an error")
}

#[test]
fn every_file_call_a_path_decides_goes_to_the_supervisor() {
    for nr in [
        libc::SYS_openat,
        libc::SYS_mkdirat,
        libc::SYS_renameat2,
        libc::SYS_linkat,
        libc::SYS_symlinkat,
        libc::SYS_unlinkat,
        libc::SYS_fchmodat,
        libc::SYS_utimensat,
        libc::SYS_connect,
        libc::SYS_listen,
    ] {
        assert_eq!(
            verdict(nr, [0; 6]),
            libc::SECCOMP_RET_USER_NOTIF,
            "call {nr}"
        );
    }
    // A path-only open is the kernel's.
    let path_only = u64::try_from(libc::O_PATH).expect("a flag");
    assert_eq!(
        verdict(libc::SYS_openat, [0, 0, path_only, 0, 0, 0]),
        libc::SECCOMP_RET_ALLOW
    );
}

#[test]
fn a_file_s_owner_and_extended_attributes_are_never_changed() {
    for nr in [
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        SYS_SETXATTRAT,
        SYS_REMOVEXATTRAT,
    ] {
        assert_eq!(verdict(nr, [0; 6]), errno(libc::EOPNOTSUPP), "call {nr}");
    }
    for nr in [libc::SYS_fchown, libc::SYS_fchownat] {
        assert_eq!(verdict(nr, [0; 6]), errno(libc::EPERM), "call {nr}");
    }
    #[cfg(target_arch = "x86_64")]
    for nr in [libc::SYS_chown, libc::SYS_lchown] {
        assert_eq!(verdict(nr, [0; 6]), errno(libc::EPERM), "call {nr}");
    }
}

#[cfg(target_arch = "x86_64")]
#[test]
fn every_call_of_the_x32_abi_is_refused() {
    for nr in [
        libc::SYS_openat,
        libc::SYS_open,
        libc::SYS_connect,
        libc::SYS_rename,
        libc::SYS_read,
    ] {
        let x32 = i64::from(X32_BIT) | nr;
        assert_eq!(verdict(x32, [0; 6]), errno(libc::ENOSYS), "x32 call {nr}");
    }
    for nr in [libc::SYS_utime, libc::SYS_utimes, libc::SYS_futimesat] {
        assert_eq!(verdict(nr, [0; 6]), errno(libc::ENOSYS), "call {nr}");
    }
    assert_eq!(verdict(libc::SYS_read, [0; 6]), libc::SECCOMP_RET_ALLOW);
}

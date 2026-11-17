// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis _landlock`** (#7084): the hidden command Pool setup runs in a pool's runner image to
//! learn whether chats can start on that pool's host. It prints one number, the Landlock ABI the
//! kernel grants (`0` with none, and on any system but Linux), and exits 0.

use crate::support;

#[test]
fn purlis_landlock_prints_the_kernels_landlock_version_and_exits_0() {
    let out = support::purlis()
        .arg(purlis_linux_sandbox::LANDLOCK)
        .output()
        .expect("purlis runs");
    assert!(out.status.success(), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let printed = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        printed.trim().parse::<i32>().ok(),
        Some(expected()),
        "{printed:?}"
    );
}

#[cfg(target_os = "linux")]
fn expected() -> i32 {
    purlis_linux_sandbox::abi()
}

#[cfg(not(target_os = "linux"))]
fn expected() -> i32 {
    0
}

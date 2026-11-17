// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis policy show --default` prints the shipped default** (M75 slice 01, #7009; spec
//! §7.1): the `default.cedar` compiled into this build, byte for byte, with nothing of a
//! project's read.

use crate::support;

#[test]
fn policy_show_default_prints_the_shipped_default() {
    let nowhere = tempfile::tempdir().expect("a directory");
    let out = support::purlis()
        .args(["policy", "show", "--default"])
        .current_dir(nowhere.path())
        .output()
        .expect("purlis runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        purlis_core::policy::DEFAULT
    );
}

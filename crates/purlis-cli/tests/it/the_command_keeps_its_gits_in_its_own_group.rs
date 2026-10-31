// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `purlis` command keeps the gits it runs in its own process group.
//!
//! Whoever started the command ends it by that group: a terminal's Ctrl-C, a harness ending a
//! hook, the app ending a chat. The core's default is the app's, where each git leads a group
//! of its own, so the command has to say otherwise as it starts, before it runs any git. This
//! fails if it stops saying so.

use crate::support;

#[test]
fn the_command_says_its_gits_stay_in_its_own_group_before_it_does_anything() {
    let dir = tempfile::tempdir().expect("a directory");

    let said = support::purlis()
        .arg("--version")
        .current_dir(dir.path())
        .env_remove("CHARTER_ROOT")
        .env("CHARTER_CONFIG_HOME", dir.path())
        .env("PURLIS_TEST_SAYS_ITS_GITS_GROUP", "1")
        .output()
        .expect("the binary runs");

    assert!(said.status.success(), "{said:?}");
    assert_eq!(
        String::from_utf8_lossy(&said.stdout),
        "each git stays in this command's group\n"
    );
}

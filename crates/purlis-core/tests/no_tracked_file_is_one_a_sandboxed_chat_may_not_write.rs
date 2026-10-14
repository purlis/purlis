// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #2089: no file this repository tracks is one a sandboxed chat may not write.
//!
//! A chat's sandbox never lets it write what a program run later loads code from (an editor's
//! tasks, an agent's settings, a hook manager's folder; ADR 0067 §5), and purlis's guard
//! refuses a checkout that would write one. A file of that name tracked here would stop every
//! sandboxed chat from bringing a clone of this repository across a change to it. A test
//! fixture that needs such a name is tracked under a neutral one (`<name>.fixture`) and copied
//! to its real name at test time ([`purlis_core::testfixture`]).

use std::path::Path;

use purlis_core::cloneupdate::{Asked, Denies};

#[test]
fn no_tracked_path_is_a_name_the_sandbox_denies_a_chat() {
    purlis_core::unsteered!();
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let top = std::fs::canonicalize(&top).expect("the repository's top");
    let mut listed = std::process::Command::new("git");
    listed
        .args(["ls-files", "-z"])
        .current_dir(&top)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let out = purlis_core::forklock::output(&mut listed).expect("git runs");
    assert!(out.status.success(), "git ls-files failed");
    let top_s = top.to_string_lossy();
    // No project root: only the later-code names count, never a manifest of a plane this is not.
    let denies = Denies::of(&top_s, "/nonexistent-project", None);
    let denied: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter(|path| denies.path(path, &Asked))
        .map(str::to_owned)
        .collect();
    assert!(
        denied.is_empty(),
        "tracked under a name a sandboxed chat may not write: {denied:?}. Track a fixture as \
         `<name>.fixture` and copy it with purlis_core::testfixture"
    );
}

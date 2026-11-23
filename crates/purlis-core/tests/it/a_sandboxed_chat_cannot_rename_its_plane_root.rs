// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sandboxed chat cannot rename its plane root** (M75 slice 11, #7020; spec §8.10).
//!
//! At the sandbox level `floor` or `full` the plane root's own folder entry is held against
//! rename where the sandbox can hold it, so a chat cannot `mv` its plane away and link the old
//! path to it. What is below the root is written as usual. The first test reads the list a start
//! compiles its sandbox from; the second runs the floor's profile for real on macOS, where the
//! floor lets a chat write every folder and so is where only this hold stops the move.

use purlis_core::sandbox::{Access, Denied, Machine, Os, Reach, seatbelt};

fn machine() -> Machine {
    Machine {
        env: purlis_core::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/nonexistent-home/op")),
        os: Os::MacOs,
    }
}

#[test]
fn a_sandboxed_chat_cannot_rename_its_plane_root() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().expect("a plane root");
    let denied = Denied::of(root.path(), &machine());
    let held = denied
        .paths
        .iter()
        .find(|denial| denial.path == root.path())
        .unwrap_or_else(|| panic!("the plane root's entry is held: {:#?}", denied.paths));
    // The entry alone: never created, renamed, moved or removed, and what is below it is not
    // held by this.
    assert_eq!(held.reach, Reach::Itself);
    assert_eq!(held.access, Access::Write);
    let rules = seatbelt::denial_rules(held).expect("rules");
    let name = root.path().display().to_string();
    assert!(
        rules
            .iter()
            .any(|rule| rule.starts_with("(deny file-write*") && rule.contains(&name)),
        "{rules:#?}"
    );
    assert!(
        rules.iter().all(|rule| rule.contains("(literal ")),
        "only the entry, never what is below it: {rules:#?}"
    );
}

/// Whether this process can apply a Seatbelt profile at all: not where it is already sandboxed.
#[cfg(target_os = "macos")]
fn can_apply() -> bool {
    purlis_core::forklock::status(std::process::Command::new("/usr/bin/sandbox-exec").args([
        "-p",
        "(version 1)(allow default)",
        "/usr/bin/true",
    ]))
    .is_ok_and(|status| status.success())
}

#[cfg(target_os = "macos")]
#[test]
fn the_floors_profile_refuses_the_move_of_a_plane_root_and_writes_below_it() {
    purlis_core::unsteered!();
    if !can_apply() {
        // On CI nothing sandboxes the test, so a skip there is a failure.
        assert!(
            std::env::var_os("CI").is_none(),
            "this process cannot apply a Seatbelt profile on CI"
        );
        return;
    }
    let parent = tempfile::tempdir().expect("a folder");
    let root = parent.path().join("plane");
    std::fs::create_dir(&root).expect("a plane root");
    let profile = seatbelt::floor_profile(
        &[purlis_core::sandbox::plane_root_held(&root)],
        &[],
        &seatbelt::Own::default(),
        &[],
        None,
    )
    .expect("a profile");
    let run = |program: &str, args: &[&std::path::Path]| {
        purlis_core::forklock::status(
            std::process::Command::new("/usr/bin/sandbox-exec")
                .args(["-p", &profile, program])
                .args(args),
        )
        .expect("it runs")
        .success()
    };
    let moved = parent.path().join("moved");
    assert!(
        !run("/bin/mv", &[root.as_path(), moved.as_path()]),
        "the plane root was renamed"
    );
    assert!(root.is_dir() && !moved.exists());
    // What is below it is written as usual.
    let inside = root.join("inside");
    assert!(run("/usr/bin/touch", &[inside.as_path()]));
    assert!(inside.is_file());
}

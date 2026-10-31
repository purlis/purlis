// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat runs on its profile's pinned harness version (#2038, ADR 0095): the start asks the
//! program it runs `--version`, pins a profile on its first chat, lets a profile purlis made
//! follow a newer version of the same major, refuses it on another major (a resumed
//! conversation included) with the way out, and lets an adopted profile follow its harness,
//! saying so on the chat's tab.
//!
//! The profile store is found through this process's config home and data home, so each body
//! runs in a child whose homes are the test's own folder, with a stand-in Claude Code first on
//! its `PATH` that answers `--version` as the real one does.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::SessionId;
use purlis_core::profilestore::{self, Homes, MachineSettings, pin};
use purlis_core::start::{self, Start};

/// Set on the child: the data home it was started with.
const DATA: &str = "PINNED_VERSION_DATA_HOME";

/// The version the stand-in answers.
const INSTALLED: &str = "2.1.288";

/// Runs `test` again in a child whose homes are a folder of its own, or answers that folder
/// where this is that child.
fn in_homes_of_its_own(test: &str) -> Option<PathBuf> {
    if let Some(data) = std::env::var_os(DATA) {
        return Some(PathBuf::from(data));
    }
    let data = tempfile::tempdir().unwrap();
    let programs = tempfile::tempdir().unwrap();
    stand_in::program(
        programs.path(),
        "claude",
        &format!("#!/bin/sh\necho '{INSTALLED} (Claude Code)'\n"),
    );
    let path = std::env::join_paths(std::iter::once(programs.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    purlis_core::testrun::rerun_steered(
        &[test],
        &[
            (purlis_core::datahome::HOME_VAR, data.path().as_os_str()),
            (purlis_core::machine::HOME_VAR, data.path().as_os_str()),
            (DATA, data.path().as_os_str()),
            ("PATH", path.as_os_str()),
        ],
    );
    None
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

fn start_on(name: &str, root: &Path, resume: Option<&str>) -> Start {
    Start {
        profile: Some(name.to_owned()),
        persona: None,
        name: "ide.7".to_owned(),
        cwd: Some(root.to_path_buf()),
        resume: resume.map(|id| SessionId::new(id).unwrap()),
        show_footer: false,
        resuming: None,
        without_sandbox: None,
        held: None,
        grants: Default::default(),
        made: purlis_core::sandbox::Made::Window,
    }
}

/// Makes the Claude Code profile `name`, with no settings to import.
fn made(name: &str) {
    let store = Homes::here().expect("the test's data home");
    let home = tempfile::tempdir().unwrap();
    let env = |_: &str| None;
    profilestore::create(
        &store,
        name,
        "claude",
        &MachineSettings {
            home: home.path(),
            env: &env,
        },
        1,
    )
    .expect("the profile is made");
}

fn config_root() -> PathBuf {
    Homes::here().unwrap().config_root().to_path_buf()
}

fn pinned(name: &str) -> Option<String> {
    profilestore::list(&config_root())
        .unwrap()
        .into_iter()
        .find(|p| p.name == name)
        .and_then(|p| p.pinned)
        .map(|p| p.version)
}

#[test]
fn the_first_chat_on_a_profile_pins_the_version_installed_and_the_next_runs_on_it() {
    purlis_core::unsteered!();
    let Some(_homes) = in_homes_of_its_own(
        "a_chat_runs_on_its_profiles_pinned_version::the_first_chat_on_a_profile_pins_the_version_installed_and_the_next_runs_on_it",
    ) else {
        return;
    };
    made("work");
    let plane = project();

    start::ready(&start_on("work", plane.path(), None), plane.path()).expect("it starts");
    assert_eq!(pinned("work").as_deref(), Some(INSTALLED));

    start::ready(&start_on("work", plane.path(), Some("s-1")), plane.path())
        .expect("its conversation resumes on its pinned version");
}

#[test]
fn a_chat_on_a_profile_pinned_to_another_major_version_is_refused_with_the_way_out() {
    purlis_core::unsteered!();
    let Some(_homes) = in_homes_of_its_own(
        "a_chat_runs_on_its_profiles_pinned_version::a_chat_on_a_profile_pinned_to_another_major_version_is_refused_with_the_way_out",
    ) else {
        return;
    };
    made("work");
    pin::update_pin(&config_root(), "work", "1.0.40", 2).unwrap();
    let plane = project();

    let new = start::ready(&start_on("work", plane.path(), None), plane.path()).unwrap_err();
    assert!(new.contains("pinned to Claude Code 1.0.40"), "{new}");
    assert!(new.contains("Update the pin"), "{new}");

    let resumed =
        start::ready(&start_on("work", plane.path(), Some("s-1")), plane.path()).unwrap_err();
    assert!(
        resumed.contains(
            "resumes a conversation of the profile 'work', which is pinned to Claude Code 1.0.40"
        ),
        "{resumed}"
    );
    assert!(resumed.contains("Update the pin"), "{resumed}");
    assert_eq!(pinned("work").as_deref(), Some("1.0.40"));

    // The person's Update the pin is the way out.
    pin::update_pin(&config_root(), "work", INSTALLED, 3).unwrap();
    start::ready(&start_on("work", plane.path(), Some("s-1")), plane.path())
        .expect("it resumes on the version the person pinned");
}

#[test]
fn a_made_profile_follows_a_patch_update_of_its_harness_and_its_tab_says_so() {
    purlis_core::unsteered!();
    let Some(_homes) = in_homes_of_its_own(
        "a_chat_runs_on_its_profiles_pinned_version::a_made_profile_follows_a_patch_update_of_its_harness_and_its_tab_says_so",
    ) else {
        return;
    };
    made("work");
    pin::update_pin(&config_root(), "work", "2.1.283", 2).unwrap();
    let plane = project();

    let ready = start::ready(&start_on("work", plane.path(), Some("s-1")), plane.path())
        .expect("a patch update of the same major resumes");

    assert!(
        ready
            .notices
            .iter()
            .any(|n| n.contains("moved from Claude Code 2.1.283 to 2.1.288")),
        "{:?}",
        ready.notices
    );
    assert_eq!(pinned("work").as_deref(), Some(INSTALLED));
}

#[test]
fn an_adopted_profile_follows_its_harness_to_a_new_version_and_its_tab_says_so() {
    purlis_core::unsteered!();
    let Some(_homes) = in_homes_of_its_own(
        "a_chat_runs_on_its_profiles_pinned_version::an_adopted_profile_follows_its_harness_to_a_new_version_and_its_tab_says_so",
    ) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let adoption = profilestore::adopt(&config_root(), home.path(), &|_| None).unwrap();
    assert_eq!(adoption.adopted.len(), 1, "{adoption:?}");
    pin::update_pin(&config_root(), "claude", "2.1.283", 2).unwrap();
    let plane = project();

    let ready = start::ready(&start_on("claude", plane.path(), Some("s-1")), plane.path())
        .expect("an adopted profile never refuses a chat for its version");

    assert!(
        ready
            .notices
            .iter()
            .any(|n| n.contains("moved from 2.1.283 to 2.1.288")),
        "{:?}",
        ready.notices
    );
    assert_eq!(pinned("claude").as_deref(), Some(INSTALLED));
}

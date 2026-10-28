// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat started on a profile this machine made (#2034, ADR 0095 §1) runs its harness in the
//! profile's own folder: the harness's declared folder variables are set to it, so the harness
//! finds the settings the profile imported there, and keeps its sign-in and conversations there.
//!
//! The profile store is found through this process's config home and data home, so each body
//! runs in a child whose `PURLIS_CONFIG_HOME` and `PURLIS_DATA_HOME` are the test's own folder,
//! with a stand-in for each harness first on its `PATH`, so the test is about the start and not
//! about what the machine has installed.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::profilestore::{self, Homes, MachineSettings};
use purlis_core::start::{self, Start};

/// Set on the child: the data home it was started with.
const DATA: &str = "PROFILE_MADE_HERE_DATA_HOME";

/// Runs `test` again in a child whose data home is a folder of its own, or answers that data
/// home where this is that child.
fn in_a_data_home_of_its_own(test: &str) -> Option<PathBuf> {
    if let Some(data) = std::env::var_os(DATA) {
        return Some(PathBuf::from(data));
    }
    let data = tempfile::tempdir().unwrap();
    // Outside the system's temp folders, which a sandboxed chat may write: a sandboxed start
    // refuses a program there.
    let programs = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    // Each answers `--version` as its harness does, as a sandboxed start asks it to.
    for (harness, version) in [("claude", "2.1.288 (Claude Code)"), ("opencode", "1.18.32")] {
        stand_in::program(
            programs.path(),
            harness,
            &format!("#!/bin/sh\necho '{version}'\n"),
        );
    }
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

fn start_on(name: &str, root: &Path) -> Start {
    Start {
        profile: Some(name.to_owned()),
        persona: None,
        name: "ide.7".to_owned(),
        cwd: Some(root.to_path_buf()),
        resume: None,
        show_footer: false,
        resuming: None,
        without_sandbox: None,
        held: None,
        grants: Default::default(),
        made: purlis_core::sandbox::Made::Window,
    }
}

/// Makes the profile `name` of `kind`, with `settings` as this machine's settings folder for
/// `variable`.
fn made(name: &str, kind: &str, variable: &str, settings: &Path) -> profilestore::MachineProfile {
    let store = Homes::here().expect("the test's data home");
    let home = settings.parent().unwrap().to_path_buf();
    let named = settings.display().to_string();
    let env = move |asked: &str| (asked == variable).then(|| named.clone());
    profilestore::create(
        &store,
        name,
        kind,
        &MachineSettings {
            home: &home,
            env: &env,
        },
        1,
    )
    .expect("the profile is made")
    .record
}

#[test]
fn a_claude_chat_on_a_profile_made_here_runs_with_its_config_folder_at_the_profiles() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_made_here_runs_in_its_folder::a_claude_chat_on_a_profile_made_here_runs_with_its_config_folder_at_the_profiles",
    ) else {
        return;
    };
    let machine = tempfile::tempdir().unwrap();
    let settings = machine.path().join("claude-settings");
    fs::create_dir_all(&settings).unwrap();
    fs::write(settings.join("settings.json"), "{\"theme\": \"dark\"}\n").unwrap();
    let record = made("work", "claude", "CLAUDE_CONFIG_DIR", &settings);
    let plane = project();

    let ready = start::ready(&start_on("work", plane.path()), plane.path()).expect("it starts");

    let config = ready
        .env
        .iter()
        .find(|(name, _)| name == "CLAUDE_CONFIG_DIR")
        .map(|(_, value)| PathBuf::from(value))
        .expect("the harness is pointed at a folder");
    assert_eq!(config, record.folders.config);
    assert_eq!(
        fs::read_to_string(config.join("settings.json")).unwrap(),
        "{\"theme\": \"dark\"}\n",
        "the harness finds the imported settings in its folder"
    );
    assert_eq!(
        ready.harness,
        Some(purlis_core::harness::Harness::ClaudeCode)
    );
    assert!(
        ready
            .env
            .iter()
            .any(|(name, value)| name == "PURLIS_HARNESS_PROFILE" && value == "work"),
        "{:?}",
        ready.env
    );
}

#[test]
fn an_opencode_chat_on_a_profile_made_here_has_both_its_folders_at_the_profiles() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_made_here_runs_in_its_folder::an_opencode_chat_on_a_profile_made_here_has_both_its_folders_at_the_profiles",
    ) else {
        return;
    };
    let machine = tempfile::tempdir().unwrap();
    let settings = machine.path().join("xdg-config");
    fs::create_dir_all(settings.join("opencode")).unwrap();
    let record = made("work", "opencode", "XDG_CONFIG_HOME", &settings);
    let plane = project();

    let ready = start::ready(&start_on("work", plane.path()), plane.path()).expect("it starts");

    let set = |variable: &str| {
        ready
            .env
            .iter()
            .find(|(name, _)| name == variable)
            .map(|(_, value)| PathBuf::from(value))
    };
    assert_eq!(set("XDG_CONFIG_HOME"), Some(record.folders.config));
    assert_eq!(set("XDG_DATA_HOME"), record.folders.data);
}

#[test]
fn a_sandboxed_claude_chat_on_a_profile_made_here_keeps_its_sign_in_file_from_its_commands() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_made_here_runs_in_its_folder::a_sandboxed_claude_chat_on_a_profile_made_here_keeps_its_sign_in_file_from_its_commands",
    ) else {
        return;
    };
    let machine = tempfile::tempdir().unwrap();
    let settings = machine.path().join("claude-settings");
    fs::create_dir_all(&settings).unwrap();
    let record = made("work", "claude", "CLAUDE_CONFIG_DIR", &settings);
    let plane = project();
    fs::write(
        plane.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();

    let ready = start::ready(&start_on("work", plane.path()), plane.path()).expect("it starts");

    let file = record.folders.config.join(".credentials.json");
    let applied = ready.sandbox.as_ref().expect("sandboxed");
    assert!(
        applied
            .confines()
            .denied
            .iter()
            .any(|denial| denial.path == file
                && denial.access == purlis_core::sandbox::Access::ReadWrite),
        "{:?}",
        applied.confines().denied
    );
    let purlis_core::sandbox::Form::ClaudeCode(settings) = applied.form() else {
        panic!("Claude Code's settings");
    };
    assert!(
        settings
            .deny
            .contains(&format!("Read(/{})", file.display())),
        "{:?}",
        settings.deny
    );
}

#[test]
fn a_chat_on_a_sandbox_only_profile_never_starts_without_the_sandbox() {
    purlis_core::unsteered!();
    // #2037, ruled 2026-10-15: a profile for sandboxed chats only runs no chat outside it,
    // whether the project runs it at the floor or the full sandbox, when the person opts out.
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_made_here_runs_in_its_folder::a_chat_on_a_sandbox_only_profile_never_starts_without_the_sandbox",
    ) else {
        return;
    };
    let store = Homes::here().expect("the test's data home");
    profilestore::create_sandbox_only(&store, "sandboxed", "claude", 1).expect("made");
    for toml in ["", "[sandbox]\nmode = \"on\"\n"] {
        let plane = project();
        fs::write(plane.path().join("charter.toml"), toml).unwrap();
        let start = Start {
            without_sandbox: Some(purlis_core::sandbox::OptOut { reason: None }),
            ..start_on("sandboxed", plane.path())
        };

        let refused = start::ready(&start, plane.path()).expect_err("refused");

        assert!(
            refused.contains("for sandboxed chats only"),
            "{toml:?}: {refused}"
        );
        assert!(
            refused.contains("Start it sandboxed"),
            "{toml:?}: {refused}"
        );
    }
}

#[test]
fn a_profile_the_project_declares_under_the_same_name_is_the_one_its_chats_run() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_made_here_runs_in_its_folder::a_profile_the_project_declares_under_the_same_name_is_the_one_its_chats_run",
    ) else {
        return;
    };
    let machine = tempfile::tempdir().unwrap();
    let settings = machine.path().join("claude-settings");
    fs::create_dir_all(&settings).unwrap();
    made("work", "claude", "CLAUDE_CONFIG_DIR", &settings);
    let plane = project();
    fs::write(
        plane.path().join(purlis_core::profiles::LOCAL_FILE),
        "[harness.work]\nkind = \"claude\"\ncommand = [\"claude\"]\nenv = { CLAUDE_CONFIG_DIR = \"/elsewhere\" }\n",
    )
    .unwrap();

    let set = purlis_core::profiles::current(plane.path());

    let work = set.get("work").expect("work is a profile");
    assert_eq!(work.source, purlis_core::profiles::Source::Local);
    assert_eq!(
        work.env,
        [("CLAUDE_CONFIG_DIR".to_owned(), "/elsewhere".to_owned())]
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host's integration tests, as one test binary (#3613, #3752).
//!
//! Cargo builds every `tests/*.rs` as a binary of its own, and each one links the crate's whole
//! dependency graph: 32 of them here, until 2026-10-29. Here each former file is a module, so
//! the crate links one binary, as `purlis-cli` (#3614) and `purlis-core` (#3749) do. A test's
//! name is its old name behind its file's: `the_kill_switch_is_held_by_the_host::a_test`, run
//! with `cargo test -p purlis-host --test it the_kill_switch_is_held_by_the_host::` or
//! nextest's `-E 'binary(it) & test(/^the_kill_switch_is_held_by_the_host::/)'`.
//!
//! **What one binary changes.** Under nextest, which CI's `rust` job runs, nothing: every test
//! still runs in a process of its own. Under `cargo test` the tests now run as threads of one
//! process instead of 32. No test here changes what its process shares: none re-runs itself,
//! changes the working directory or a limit, or installs a hook. Each module mints its own
//! `HELD` credentials once, as each binary did.

mod a_browser_is_a_hosted_program_that_makes_frames_only_while_watched;
mod a_browser_refused_for_the_engines_sandbox_says_so_by_its_code;
mod a_browsers_engine_download_is_told_as_it_goes_and_cancelled;
mod a_chat_runs_in_the_host_from_a_held_project;
mod a_chats_confinement_lives_in_the_host;
mod a_connector_is_a_command_checked_before_each_connection;
mod a_held_project_is_saved_in_the_host_by_one_saver;
mod a_held_projects_board_is_kept_in_the_host;
mod a_held_projects_hooks_are_heard_in_the_host;
mod a_host_relays_a_window_to_another_host;
mod a_host_started_after_one_that_crashed_settles_what_it_left;
mod a_relay_links_again_to_the_other_host_on_its_own;
mod a_runner_answers_its_desktop_on_its_link_socket;
mod a_runner_confines_a_chat_as_its_desktop_relays_it;
mod a_runner_left_without_a_link_suspends_itself;
mod a_runner_runs_its_windows_browsers_as_its_desktop_relays_them;
mod a_runners_event_log_is_read_over_the_link_from_a_cursor;
mod a_start_the_host_composes_has_what_the_window_starts_beside_it;
mod a_terminal_asks_the_window_holding_a_project_for_a_grade;
mod a_window_gone_past_the_grace_period_ends_its_chats_as_a_quit;
mod a_window_takes_back_its_own_runner_chats_sandboxes;
mod a_window_that_comes_back_takes_its_browsers_back;
mod a_window_that_comes_back_takes_its_programs_back;
mod purlis_logs_a_browser_in_and_hands_it_back_to_the_agent;
mod the_event_log_and_a_programs_end_are_kept_in_the_host;
mod the_host_admits_the_windows_scope_and_refuses_the_rest;
mod the_host_hears_a_stop_written_while_no_window_is_there;
mod the_host_links_no_git_http_or_keyring_code;
mod the_host_runs_no_more_browsers_than_its_devices_budget;
mod the_kill_switch_is_held_by_the_host;
mod the_kill_switch_reaches_every_runner_this_machine_keeps;
mod the_person_inspects_a_browser_they_drive;
mod the_person_takes_a_browser_over_and_hands_it_back;

/// A `.rs` file straight under `tests/` is a test binary of its own, which links the whole
/// graph again: the cost this layout exists to remove. A new test file goes in this folder,
/// with a `mod` line above.
#[test]
fn every_test_file_is_a_module_of_this_one_binary() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut binaries: Vec<String> = std::fs::read_dir(&tests)
        .expect("the tests folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();
    binaries.sort();
    assert!(
        binaries.is_empty(),
        "each of these is a test binary of its own; move it into tests/it/ as a module: {binaries:?}"
    );
}

/// A file in this folder that no `mod` line above names is never compiled, so its tests would
/// quietly stop running.
#[test]
fn every_file_in_this_folder_is_a_module_above() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/it");
    let declared = include_str!("main.rs");
    let mut missing: Vec<String> = std::fs::read_dir(&here)
        .expect("this folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
        .filter(|name| name != "main" && !declared.contains(&format!("\nmod {name};\n")))
        .collect();
    missing.sort();
    assert!(
        missing.is_empty(),
        "add `mod <name>;` above for: {missing:?}"
    );
}

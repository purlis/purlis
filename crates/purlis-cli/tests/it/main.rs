// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The CLI's integration tests, as one test binary (#3613).
//!
//! Cargo builds every `tests/*.rs` as a binary of its own, and each one links the CLI's whole
//! dependency graph. With 91 such files, linking was most of what `Build the tests` spent on
//! this crate in every `rust` part of CI (#3600's sccache report, 2026-10-28: 442 of 461 rustc
//! calls produced a linked crate type). Here each former file is a module, so the crate links
//! one binary. A test's name is its old name behind its file's: `hook::a_test`, run with
//! `cargo test -p purlis-cli --test it hook::` or nextest's `-E 'binary(it) & test(/^hook::/)'`.
//!
//! **What one binary changes.** Under nextest, which CI's `rust` job runs, nothing: every test
//! still runs in a process of its own. Under `cargo test` (the Linux normal-user job) the
//! crate's tests now run as threads of one process instead of 91. None of them changes what a
//! process shares: the workspace denies `unsafe`, so no test sets a variable or the working
//! directory in-process (one whose subject is the environment re-runs itself as a child,
//! `purlis_core::testrun::rerun`), and every program they start is a child.
//!
//! **A re-run names its test by its full libtest path**, module first (`brokered_git::…`), as
//! `--exact` matches it.
//!
//! `recorded_behaviour/` stays a binary of its own: it is a harness of its own
//! (`harness = false`), run by name in CI.

// How every test starts the binary (#1284), shared with `recorded_behaviour`.
#[path = "../support/mod.rs"]
mod support;

mod a_brokered_run_is_held_on_linux;
mod a_file_tool_names_its_path_to_the_app_and_nowhere_else;
mod a_finder_launch_finds_the_harness;
mod a_guard_decides_within_its_budget;
mod a_harness_with_mcp_and_no_skill_format_gets_its_skills_through_purlis_s_tools;
mod a_judge_is_refused_a_read_outside_its_folder;
mod a_permission_prompt_is_answered_from_the_window;
mod a_project_charter_cannot_write_is_read_only;
mod a_proposed_change_is_handed_to_the_app;
mod a_refusal_names_the_kind_and_never_the_credential;
mod a_sandbox_block_reaches_the_app_as_an_operation_and_a_kind;
mod a_stop_hook_hands_the_app_its_turn_s_last_message;
mod a_tool_hook_says_what_its_chat_is_doing_in_a_kind_and_one_name;
mod a_workspace_store_reached_through_a_link_is_refused;
mod a_wrapped_chat_a_session_host_starts_tells_its_refusals;
mod a_wrapped_chat_holds_on_linux;
mod a_write_the_linux_wrap_refuses_raises_the_block_notice;
mod active;
mod both_names_run_the_cli;
mod brokered_git;
mod brokered_writes;
mod browser_install;
mod change;
mod commit_scan;
mod commit_trailers;
mod commitment_gate;
mod core_diagnostics;
mod curation;
mod dispatch_from_inside_the_app;
mod doctor;
mod every_command_charter_names_is_one_it_has;
mod every_memory_write_a_shipped_text_teaches_names_its_scope;
mod every_test_starts_the_binary_unsteered;
mod extension_commands;
mod extensions;
mod first_task_script;
mod guard;
mod handback;
mod handoff_from_inside_the_app;
mod harness_list;
mod harness_show;
mod hook;
mod init;
mod keyring_vault;
mod mcp_server;
mod memory;
mod memory_edit_and_archive;
mod memory_move;
mod migrate;
mod news_and_adoption;
mod persona_lifecycle;
mod persona_upkeep;
mod pieces;
mod plain_git_in_a_held_branch_folder_is_refused_by_the_hook;
mod plane_resolution;
mod plane_root;
mod plugin_install;
mod profile_create;
mod profile_list;
mod profile_sign_in;
mod purlis_serve_confines_a_chat_with_its_own_proxy;
mod purlis_serve_hears_a_held_projects_hooks;
mod purlis_serve_is_this_users_one_session_host;
mod purlis_serve_keeps_the_event_log_as_the_windows_host_does;
mod purlis_serve_writes_the_run_of_a_chat_it_composes;
mod purlis_skill_grade_shows_the_plan_and_the_grades;
mod purlis_skill_grant_decides_where_a_project_skill_reaches;
mod purlis_skill_install_and_update_pin_a_reviewed_copy;
mod purlis_skill_lists_and_shows_a_projects_skills;
mod purlis_skill_lists_harness_and_repo_skills;
mod purlis_skill_new_edit_mv_rm_and_restore_change_a_skill;
mod purlis_skill_pin_keeps_a_skill_listed_past_the_budget;
mod purlis_skill_revert_asks_before_a_version_that_widens;
mod purlis_skill_runs_lists_runs_with_their_signals;
mod repo_commands;
mod report;
mod save_pull;
mod secrets;
mod session;
mod shell_guard;
mod status;
mod statusline;
mod steering_names_that_disagree;
mod stop;
mod the_command_keeps_its_gits_in_its_own_group;
mod the_floor_refuses_an_unattended_merge_through_the_hook;
mod the_mcp_server_on_a_read_only_project_changes_nothing;
mod the_suite_cannot_reach_a_plane_it_did_not_make;
mod theme;
mod todo_promote;
mod tool_events;
mod where_a_chat_is_working;
mod workspace_rename;
mod worktree_remove_keeps_off_a_task_s_folder;

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
        .map(|path| path.display().to_string())
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

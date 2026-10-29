// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The core's integration tests, as one test binary (#3613).
//!
//! Cargo builds every `tests/*.rs` as a binary of its own, and each one links the core's whole
//! dependency graph. With 160 such files, linking them was the largest share of what `Build the
//! tests` spent in every `rust` part of CI (#3600's sccache report, 2026-10-28: 442 of 461 rustc
//! calls produced a linked crate type). Here each former file is a module, so the crate links
//! one binary. A test's name is its old name behind its file's: `forge_contract::a_test`, run
//! with `cargo test -p purlis-core --test it forge_contract::` or nextest's
//! `-E 'binary(it) & test(/^forge_contract::/)'`.
//!
//! **What one binary changes.** Under nextest, which CI's `rust` job runs, nothing: every test
//! still runs in a process of its own. Under `cargo test` (the nightly `fedora` job, mutants'
//! baseline and every mutant, and the jobs that run a few files by name) the tests now run as
//! threads of one process instead of 160. The workspace denies `unsafe`, so no test sets an
//! environment variable in-process: one whose subject is the environment re-runs itself as a
//! child (`purlis_core::testrun::rerun`). The two that changed something else process-wide, the
//! working directory (`the_leak_guard_answers_what_the_python_answers`) and the open-file limit
//! (`a_launchd_started_process_raises_its_open_file_limit`), were binaries of their own for it,
//! and now re-run themselves in a child the same way.
//!
//! **A re-run names its test by its full libtest path**, module first
//! (`a_branch_marks_what_it_changed::reader_child`), which is what `--exact` matches; a re-run
//! by filter (`support::forge_cli::in_a_child`) starts its filter with the module too, so it
//! cannot pick up another file's tests.
//!
//! `design_tokens_memory.rs` stays a binary of its own: it installs a counting
//! `#[global_allocator]`, which is the whole process's.

// The shared helpers, in the folders they always had; each module reaches one as
// `crate::support` and so on.
#[path = "../old_names/mod.rs"]
mod old_names;
#[path = "../oracle_corpus/mod.rs"]
mod oracle_corpus;
#[path = "../planeroot_answer/mod.rs"]
mod planeroot_answer;
#[path = "../shipped_literals/mod.rs"]
mod shipped_literals;
#[path = "../support/mod.rs"]
mod support;

mod a_branch_expands_into_its_files;
mod a_branch_folder_is_kept_current;
mod a_branch_folder_keeps_its_branch_across_a_workspace_rename;
mod a_branch_marks_what_it_changed;
mod a_branch_path_leaves_the_window_only_resolved_and_inside;
mod a_branch_says_how_far_it_is_from_its_base;
mod a_change_is_shown_with_each_members_request_and_checks;
mod a_chat_in_a_worktree_gets_the_planes_layer;
mod a_chat_in_its_own_worktree_gets_an_agents_md;
mod a_chat_moves_to_a_docker_pools_runner_and_back;
mod a_chat_on_a_profile_made_here_runs_in_its_folder;
mod a_chat_on_a_profile_not_signed_in_is_refused_with_sign_in;
mod a_chat_on_a_profile_starts_as_its_sign_in_says;
mod a_chat_runs_on_a_cluster_runner_through_kubectl_exec;
mod a_chat_runs_on_its_profiles_pinned_version;
mod a_chats_process_tree_is_handed_no_forge_sign_in;
mod a_checkout_refreshes_only_what_purlis_can_confirm_it_wrote;
mod a_checkout_withdraws_what_the_project_stopped_mirroring;
mod a_cli_login_is_imported_only_when_asked;
mod a_command_too_big_to_check_is_refused_before_it_is_read;
mod a_comparison_matches_git_diff;
mod a_cut_says_branch_in_the_window;
mod a_dispatched_chat_works_in_a_worktree_of_its_own;
mod a_docker_pool_runner_is_linked_through_docker_exec;
mod a_doctor_finding_is_fixed_by_its_fix_id;
mod a_drafts_sweep_under_a_shared_home_adopts_no_folder;
mod a_fifty_request_workspace_polls_inside_its_budget;
mod a_file_command_reads_what_git_would_in_the_reader;
mod a_file_is_found_by_fuzzy_name;
mod a_file_of_a_piece_opens_in_your_editor_at_a_line;
mod a_file_shows_what_its_branch_changed;
mod a_folder_too_deep_to_read_costs_only_itself;
mod a_folders_git_link_is_checked_before_git_runs_there;
mod a_forge_cli_is_asked_exactly_what_python_asked;
mod a_forge_token_appears_in_no_log_record_or_crash_report;
mod a_forge_token_never_reaches_a_chat;
mod a_gitlab_sign_in_through_the_browser;
mod a_gone_project_is_located;
mod a_grades_chat_has_only_purlis_s_plugin_on;
mod a_grades_chat_holds_none_of_the_persons_kept_allows;
mod a_grades_chat_runs_on_the_model_picked;
mod a_handoff_todo_is_recorded_once_per_piece_of_work;
mod a_harness_is_found_the_way_a_shell_finds_it;
mod a_host_with_no_window_arms_a_chat_with_what_is_installed_beside_it;
mod a_launchd_started_process_raises_its_open_file_limit;
mod a_legacy_task_memo_is_indexed_by_the_rules_every_append_keeps;
mod a_level_3_chat_starts_from_its_own_start;
mod a_local_file_that_would_travel_decides_nothing;
mod a_memory_is_edited_in_place_and_archived_reversibly;
mod a_memory_moves_between_scopes_whole;
mod a_memory_op_acts_on_the_file_it_names_and_no_other;
mod a_missing_git_identity_is_fixed_from_a_form;
mod a_per_device_log_is_named_by_the_device_id;
mod a_persona_brings_its_skills_into_every_workspace;
mod a_persona_on_a_profile_made_here_keeps_the_rules_that_stay;
mod a_persona_store_is_reached_with_no_link_on_the_way;
mod a_planted_repo_config_runs_nothing;
mod a_pool_container_is_checked_on_this_host;
mod a_pr_is_opened_or_updated_and_set_to_auto_merge;
mod a_pr_mode_save_keeps_one_pr_open_from_the_save_branch;
mod a_profile_asks_before_its_command_runs;
mod a_profile_starts_with_nothing_installed;
mod a_program_started_while_a_terminal_opens_cannot_hold_it_open;
mod a_project_is_held_by_its_stored_id_or_one_derived_from_where_it_is;
mod a_project_skill_reaches_a_chat_on_every_harness;
mod a_reference_handed_to_a_chat_names_only_a_confined_path;
mod a_repo_is_saved_by_its_mode;
mod a_report_is_filed_only_with_the_purlis_sign_in;
mod a_repos_network_settings_never_reach_the_windows_git;
mod a_sandboxed_chat_commits_in_its_branch_folder_through_the_app;
mod a_sandboxed_chat_listens_nowhere_under_seatbelt;
mod a_signed_in_forge_token_never_reaches_a_chat;
mod a_skill_can_only_narrow_a_chat;
mod a_stale_clone_is_told_and_fixed_without_losing_work;
mod a_start_the_host_composes_runs_as_the_window_would_start_it;
mod a_steering_variable_set_on_the_process_is_what_the_library_reads;
mod a_subscriber_misses_and_duplicates_nothing_across_host_kills_and_seals;
mod a_substitution_is_read_wherever_the_shell_runs_it;
mod a_terminal_only_harness_runs_from_a_declaration_alone;
mod a_todo_is_recorded_once_and_forgotten_without_a_trace;
mod a_turn_that_ends_while_its_background_agents_work_needs_nobody;
mod a_workspace_file_is_written_with_no_link_on_the_way;
mod a_workspace_store_is_reached_with_no_link_on_the_way;
mod a_workspaces_repos_are_read_where_the_plane_keeps_them;
mod a_worktree_keeps_the_work_in_it;
mod a_writing_chat_gets_a_branch_of_its_own;
mod an_archived_memory_is_listed_from_its_own_store;
mod an_ask_rule_never_weakens_a_deny;
mod an_issue_body_is_always_sent_as_a_literal;
mod an_operators_rule_reads_every_spelling_of_its_program;
mod an_unattended_run_cannot_merge_a_request_by_any_route;
mod any_file_in_a_piece_opens_in_the_light_editor;
mod claude_codes_own_sandbox_holds_the_settings_purlis_writes;
mod code_crosses_to_a_docker_pools_runner_and_back;
mod content_is_searched_across_branches;
mod every_deferral_names_its_ticket;
mod every_first_party_source_file_carries_its_licence_header;
mod every_git_call_goes_through_the_runner;
mod every_guard_reads_a_hostile_command_in_linear_time;
mod every_shipped_skill_names_only_purlis;
mod every_store_the_plane_format_names_has_a_tier;
mod every_test_here_runs_without_the_shells_steering;
mod every_third_party_call_is_listed_in_the_network_log;
mod fixture_planes;
mod forge_accounts;
mod forge_contract;
mod forge_device_sign_in;
mod forge_one_start_asks_the_keychain_once;
mod forge_state_is_read_from_a_cache_and_never_fetched;
mod four_unsafe_blocks;
mod harness_profiles;
mod no_automatic_read_takes_the_index_lock;
mod no_count_says_thing_s;
mod no_old_name_is_spelled_in_the_window_or_its_assets;
mod no_old_name_is_spelled_outside_the_name_module;
mod no_tracked_file_is_one_a_sandboxed_chat_may_not_write;
mod nothing_escapes;
mod nothing_escapes_a_worktree;
mod nothing_escapes_while_a_writer_races;
mod only_the_resolver_builds_the_native_transport;
mod only_the_updater_and_report_name_charters_own_address;
mod profiles_are_refused_when_git_would_carry_them;
mod project_skills_move_without_a_chat_losing_one;
mod project_templates;
mod recall_reads_only_the_plane;
mod recorded_corpus;
mod rename_local_copies_keychain_items_before_switching;
mod rename_local_follows_the_app_to_its_purlis_identity;
mod rename_local_keeps_file_vaults_whole;
mod rename_local_moves_local_state_and_puts_it_back;
mod rename_local_moves_the_harness_plugin;
mod rename_plane_is_one_commit;
mod resuming_a_session_record;
mod search_is_measured_on_large_repos;
mod set_key_never_writes_a_charter_toml_that_does_not_read;
mod shown_is_the_pythons_readable;
mod starting_a_chat_on_a_profile;
mod the_app_reads_a_kept_token_once_per_run;
mod the_browser_skills_terminal_flow_keeps_playwrights_output_out_of_the_project;
mod the_golden_rule_and_the_prose_guards_answer_what_the_python_answers;
mod the_guards_know_the_cli_by_both_its_names;
mod the_handoff_guard_answers_what_the_python_answers;
mod the_index_is_read_and_repaired_by_the_rules_an_edit_keeps;
mod the_leak_guard_answers_what_the_python_answers;
mod the_local_layer_has_one_reader;
mod the_local_project_runs_sandboxed_chats;
mod the_native_transport_asks_conditionally_and_once;
mod the_one_route_is_taught_in_the_same_words;
mod the_plane_root_guards_answer_what_the_python_answers;
mod the_plane_root_lists_the_project_but_what_purlis_keeps;
mod the_profiles_a_wrapped_chat_starts_under_are_written_out;
mod the_record_brings_a_profile_back;
mod the_refresher_writes_the_key_the_panel_reads;
mod the_repo_list_puts_projects_first;
mod the_shell_is_read_the_way_python_reads_it;
mod the_window_asks_its_forge_with_the_purlis_sign_in;
mod this_machine_forgets_and_revokes;
mod upgrading_leaves_every_chat_start_as_it_was_and_records_its_folder;
mod when_the_forge_cache_is_refreshed;
mod workspace_remove_clears_only_its_own_exclude_lines;

/// Set on the child [`alone`] starts.
const ALONE: &str = "CORE_TEST_ALONE";

/// For a test that changes what its whole process shares: the working directory, a resource
/// limit, the forge sign-ins handed over, the mark that this process is the app. Whether it has
/// just been run again in a process of its own, so that the caller has nothing left to do and
/// returns.
///
/// Each of those tests was a binary of its own until #3613. nextest already runs every test in a
/// process of its own, so there this answers `false` at once; `cargo test` runs this binary's
/// tests as threads of one process, and there it re-runs the test by its libtest name, which is
/// the name its thread was given.
pub(crate) fn alone() -> bool {
    if std::env::var_os(ALONE).is_some()
        || std::env::var_os("NEXTEST_EXECUTION_MODE").is_some_and(|mode| mode == "process-per-test")
    {
        return false;
    }
    let me = std::thread::current();
    let name = me
        .name()
        .filter(|name| *name != "main")
        .expect("libtest names the thread a test runs on after the test");
    purlis_core::testrun::rerun(&[name], &[(ALONE, std::ffi::OsStr::new("1"))]);
    true
}

/// The `.rs` files straight under `tests/` that are binaries of their own on purpose.
const OWN_BINARIES: [&str; 1] = ["design_tokens_memory.rs"];

/// A `.rs` file straight under `tests/` is a test binary of its own, which links the whole
/// graph again: the cost this layout exists to remove. A new test file goes in this folder,
/// with a `mod` line above.
#[test]
fn every_test_file_is_a_module_of_this_one_binary() {
    purlis_core::unsteered!();
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut binaries: Vec<String> = std::fs::read_dir(&tests)
        .expect("the tests folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .filter(|name| !OWN_BINARIES.contains(&name.as_str()))
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
    purlis_core::unsteered!();
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

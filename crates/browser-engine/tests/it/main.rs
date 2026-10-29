// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The browser engine's integration tests, as one test binary (#3613, #3752).
//!
//! Cargo builds every `tests/*.rs` as a binary of its own, and each one links the crate's whole
//! dependency graph: 41 of them here, until 2026-10-29. Here each former file is a module, so
//! the crate links one binary, as `purlis-cli` (#3614) and `purlis-core` (#3749) do. A test's
//! name is its old name behind its file's: `only_http_and_https_addresses_open::a_test`, run
//! with `cargo test -p purlis-browser-engine --test it only_http_and_https_addresses_open::` or
//! nextest's `-E 'binary(it) & test(/^only_http_and_https_addresses_open::/)'`.
//!
//! **What one binary changes.** Under nextest (CI's `rust` and `macos-tests` jobs) nothing:
//! every test still runs in a process of its own. Under `cargo test` (`browser-engine-macos`,
//! which runs the real Chrome two tests at a time) the tests now run as threads of one process
//! instead of 41, so two tests of different files can run side by side, as two of one file
//! always could. A host's part (`common::HostPart`) re-runs its test by its full libtest name,
//! module first, which is what `--exact` matches. The one test that leaves something of its own
//! in the whole process, a descriptor open across exec
//! (`the_engine_inherits_only_its_pipe_and_standard_streams`), re-runs itself in a child of its
//! own through [`alone`], so no other test's child inherits it. `common::site` installs, on CI
//! only, a panic hook that annotates the job with a failed test's name: it is the process's, so
//! under `cargo test` it now names a failure of any test here, which is what it is for.

// The shared helpers, in the folder they always had; each module reaches them as
// `crate::common`.
#[path = "../common/mod.rs"]
mod common;

mod a_blanked_field_in_a_frame_the_page_transforms_is_painted_over;
mod a_browser_opened_from_kept_logins_is_signed_in;
mod a_browser_says_how_its_engine_ended;
mod a_call_ends_within_its_limit_when_the_engine_stops_reading;
mod a_call_on_a_session_that_detaches_ends_as_it_detaches;
mod a_code_typed_across_several_boxes_stays_out_whole;
mod a_document_focused_beside_a_frames_field_does_not_blank_the_field;
mod a_download_that_stops_answering_is_given_up_on;
mod a_field_in_a_shadow_root_is_kept_out_as_its_page_changes_it;
mod a_frame_that_moves_to_another_site_takes_no_input_aimed_before;
mod a_judged_engine_tells_a_read_an_address_and_a_form_apart;
mod a_link_or_redirect_to_a_file_or_app_address_does_not_open;
mod a_login_purlis_performs_shows_the_agent_nothing_it_filled;
mod a_native_drag_source_is_dragged_as_a_person_drags_it;
mod a_page_that_redefines_what_purlis_reads_is_read_as_it_is;
mod a_page_that_retitles_itself_after_the_load_is_listed_under_its_new_title;
mod a_page_whose_script_cut_an_emoji_in_two_is_read;
mod a_real_engine_paints_frames_only_while_shown;
mod a_real_engine_runs_with_no_port_and_ends_with_its_host;
mod a_real_engines_clipboard_gives_nothing_back_after_devtools;
mod a_real_engines_devtools_speak_to_the_page_through_purlis;
mod a_screenshot_paints_over_what_is_kept_out_wherever_the_page_draws_it;
mod a_snapshot_marks_what_a_scrolling_box_or_frame_does_not_show;
mod a_tapped_session_is_relayed_whole_and_in_order;
mod an_act_in_a_frame_the_page_transforms_lands_where_the_element_is_drawn;
mod an_action_answers_what_changed_and_a_snapshot_marks_what_is_out_of_view;
mod an_engine_answer_too_large_fails_only_its_own_call;
mod an_engine_update_is_taken_only_as_the_manifest_workflow_signed_it;
mod an_old_engine_is_warned_then_refused_unless_allowed;
mod only_http_and_https_addresses_open;
mod the_connection_reads_half_pairs_and_says_why_it_closed;
mod the_engine_inherits_only_its_pipe_and_standard_streams;
mod the_engine_is_spoken_to_over_a_nul_framed_pipe;
mod the_engine_starts_with_its_protections_on;
mod the_engines_group_is_gone_within_a_second_of_its_host;
mod the_field_with_the_focus_is_blanked_beside_an_active_option;
mod the_fixture_sites_download_and_file_input_pages_pass;
mod the_fixture_sites_hard_pages_pass;
mod the_manifest_is_verified_before_the_engine_runs;
mod the_person_types_into_the_field_the_agent_clicked;
mod the_persons_keys_reach_a_field_while_another_frame_reloads;
mod typing_while_driving_stays_quick_on_a_large_page;

/// Set on the child [`alone`] starts.
#[cfg(unix)]
const ALONE: &str = "BROWSER_ENGINE_TEST_ALONE";

/// For a test that changes what its whole process shares. Whether it has just been run again in
/// a process of its own, so that the caller has nothing left to do and returns.
///
/// nextest already runs every test in a process of its own, so there this answers `false` at
/// once; `cargo test` runs this binary's tests as threads of one process, and there it re-runs
/// the test by its libtest name, which is the name its thread was given.
#[cfg(unix)]
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
    let out = std::process::Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(ALONE, "1")
        .output()
        .expect("this test binary runs again");
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // The child's own count: a name that matched nothing would pass with nothing run.
    assert!(
        out.status.success() && said.contains("test result: ok. 1 passed"),
        "{name}, alone in a process of its own:\n{said}"
    );
    true
}

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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session protocol's integration tests, as one test binary (#3613, #3752).
//!
//! Cargo builds every `tests/*.rs` as a binary of its own, and each one links the crate's whole
//! dependency graph: 37 of them here, until 2026-10-29. Here each former file is a module, so
//! the crate links one binary for 35 of them, as `purlis-cli` (#3614) and `purlis-core` (#3749)
//! do. A test's name is its old name behind its file's: `a_link_keeps_itself_up::a_test`, run
//! with `cargo test -p purlis-session-protocol --test it a_link_keeps_itself_up::` or nextest's
//! `-E 'binary(it) & test(/^a_link_keeps_itself_up::/)'`.
//!
//! **What one binary changes.** Under nextest, which CI's `rust` job runs, nothing: every test
//! still runs in a process of its own. Under `cargo test` the tests now run as threads of one
//! process instead of 35. No test here changes what its process shares: none re-runs itself,
//! changes the working directory or a limit, or installs a hook; `common::HELD` and
//! `common::hosts`' keys are minted once per process, as they were once per binary.
//!
//! Two files stay binaries of their own, because each measures the heap of its whole process
//! with dhat's `#[global_allocator]`:
//! `a_peer_that_opens_thousands_of_streams_cannot_grow_the_hosts_memory.rs` and
//! `fifty_chats_share_one_shaped_link_fairly_with_bounded_memory.rs`.

// The shared helpers, in the folder they always had; each module reaches them as
// `crate::common`.
#[path = "../common/mod.rs"]
mod common;

mod a_browser_views_bytes_are_whole_frames_however_they_are_cut;
mod a_client_killed_outright_loses_its_views_and_the_next_one_reattaches_over_a_real_socket;
mod a_client_speaks_the_session_protocol_to_the_host;
mod a_client_started_inside_a_chat_is_refused_every_human_scope;
mod a_client_starts_hosted_programs_and_attaches_views_of_a_kind;
mod a_client_that_acknowledges_more_than_it_was_sent_loses_the_view;
mod a_client_that_stops_reading_its_events_holds_a_bounded_queue;
mod a_command_that_waits_delays_no_other_answer_on_its_link;
mod a_connection_without_its_scopes_credential_is_refused;
mod a_frame_view_has_one_frame_in_flight_and_then_the_newest;
mod a_keystroke_gets_through_ten_flooding_panes;
mod a_keystroke_to_a_remote_browser_is_never_queued_behind_its_frames;
mod a_link_carries_a_control_lane_and_streams;
mod a_link_from_inside_a_chat_is_closed_unread;
mod a_link_keeps_itself_up;
mod a_link_key_that_is_not_pinned_is_refused;
mod a_peer_that_stalls_the_handshake_is_refused_in_time;
mod a_remote_link_client_in_another_process_lists_snapshots_and_stops;
mod a_subscriber_reads_on_from_its_cursor_and_an_attach_opens_a_view;
mod a_view_is_a_snapshot_then_the_live_stream;
mod a_view_is_attached_again_by_epoch_with_backoff;
mod a_view_of_any_kind_follows_its_program_and_is_snapshotted_again_when_behind;
mod a_view_pauses_at_its_watermark_until_the_client_draws;
mod a_view_that_falls_behind_is_dropped_and_snapshotted_again;
mod a_view_whose_program_ends_is_said_to_have_ended_for_good;
mod each_scope_may_call_only_what_its_row_grants;
mod each_scopes_credential_is_a_file_only_this_user_can_read;
mod every_version_the_protocol_names_is_one_it_speaks;
mod hosts_link_only_within_one_major_version;
mod the_control_lane_goes_ahead_of_fifty_busy_streams;
mod the_session_protocol_keeps_every_frame_it_promised;
mod the_session_protocol_runs_through_a_link_between_two_hosts;
mod the_ui_rpc_rides_beside_the_session_protocol_for_one_build;
mod the_version_is_negotiated_and_an_unknown_major_is_refused;
mod the_version_is_negotiated_over_a_socket_a_pipe_and_a_childs_stdio;

/// The `.rs` files straight under `tests/` that are binaries of their own on purpose.
const OWN_BINARIES: [&str; 2] = [
    "a_peer_that_opens_thousands_of_streams_cannot_grow_the_hosts_memory.rs",
    "fifty_chats_share_one_shaped_link_fairly_with_bounded_memory.rs",
];

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

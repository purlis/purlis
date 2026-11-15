// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's key proxy, served through a start path** (T1a-05, T1a-08): a request carrying the
//! run's token, on its port, reaches the fake provider with the stand-in key in its place, its
//! bodies passed through as they came; one without the token is refused, and the window hears
//! nothing of it (R-042, R-045, R-063, R-064, R-065, R-745). The run is bound to its route at its
//! start, and only what core's Messages allowlist lists goes upstream; Claude Code's recorded
//! startup starts the chat (R-037, R-077, R-087, R-089, R-479, R-712).
//!
//! **The start path is a parameter** (D-3). Each test is a function of a [`StartPath`], run here
//! through [`start_path::Desktop`], the window's start through its session host: T1a-66 runs the
//! same functions through the runner's start.
//!
//! Every test here binds `127.0.0.1`. Run them with
//! `cargo test -p purlis-host --test it the_key_proxy_through_a_start_path::`.

#![cfg(unix)]

mod a_declaration_edited_mid_run_changes_nothing_for_that_run;
mod a_disallowed_messages_path_is_refused_before_the_key_is_swapped_in;
mod a_listed_path_with_a_query_passes;
mod a_token_on_another_chats_port_is_refused;
mod a_tokenless_request_is_refused_and_the_run_goes_on;
mod a_tokenless_request_raises_nothing_in_the_window;
mod any_request_on_the_listener_counts_as_the_route_used;
mod bodies_pass_through_byte_for_byte;
mod claude_code_startup_calls_are_answered_as_the_spike_recorded;
mod count_tokens_is_refused_with_no_credential_upstream;
mod each_request_runs_only_in_memory_checks;
mod expected_refused_calls_write_no_stop_and_no_notice;
mod no_include_usage_is_injected;
mod no_startup_call_carries_the_credential;
pub mod start_path;
pub mod startup;
mod the_listener_is_bound_to_ipv4_loopback_only;
mod the_startup_with_an_http_proxy_configured_skips_api_hello_and_still_starts;
mod the_token_dies_with_the_run;
mod tokens_are_32_random_bytes_and_never_repeat;

pub(crate) use start_path::StartPath;

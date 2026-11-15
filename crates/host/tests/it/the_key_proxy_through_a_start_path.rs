// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's key proxy, served through a start path** (T1a-05): a request carrying the run's
//! token, on its port, reaches the fake provider with the stand-in key in its place, its bodies
//! passed through as they came; one without the token is refused, and the window hears nothing
//! of it (R-042, R-045, R-063, R-064, R-065, R-745).
//!
//! **The start path is a parameter** (D-3). Each test is a function of a [`StartPath`], run here
//! through [`start_path::Desktop`], the window's start through its session host: T1a-66 runs the
//! same functions through the runner's start.
//!
//! Every test here binds `127.0.0.1`. Run them with
//! `cargo test -p purlis-host --test it the_key_proxy_through_a_start_path::`.

#![cfg(unix)]

mod a_token_on_another_chats_port_is_refused;
mod a_tokenless_request_is_refused_and_the_run_goes_on;
mod a_tokenless_request_raises_nothing_in_the_window;
mod bodies_pass_through_byte_for_byte;
mod no_include_usage_is_injected;
pub mod start_path;
mod the_listener_is_bound_to_ipv4_loopback_only;
mod the_token_dies_with_the_run;
mod tokens_are_32_random_bytes_and_never_repeat;

pub(crate) use start_path::StartPath;

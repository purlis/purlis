// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A program in its own pseudo-terminal, and the headless engine that keeps its screen, with
//! nothing of git, HTTP or the keyring (#1995): a session host owns terminals with this crate
//! alone, and `tests/the_terminal_links_no_git_http_or_keyring_code.rs` holds it to that.
//!
//! The core re-exports [`engine`] and [`session`] as `purlis_core::engine` and
//! `purlis_core::session`, and starts every other program under this crate's [`forklock`].

pub mod engine;
pub mod exec;
pub mod forklock;
pub mod process;
pub mod session;

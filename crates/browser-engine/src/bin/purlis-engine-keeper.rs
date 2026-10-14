// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The keeper of one engine's process group: see `purlis_browser_engine::keeper`.

#[cfg(unix)]
fn main() {
    purlis_browser_engine::keeper::run()
}

#[cfg(not(unix))]
fn main() {
    std::process::exit(2);
}

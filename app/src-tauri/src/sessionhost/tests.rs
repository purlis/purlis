// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

#[test]
fn the_window_starts_its_own_binary_as_its_session_host_holding_its_input() {
    let command = command(Path::new(
        "/Applications/purlis.app/Contents/MacOS/charter-app",
    ));
    assert_eq!(
        command.get_program(),
        "/Applications/purlis.app/Contents/MacOS/charter-app"
    );
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["purlis-serve-for-the-window"]);
}

#[test]
fn a_host_that_found_another_serving_is_said_so_and_not_connected_to() {
    assert!(why_not_ready(Some(3)).contains("another session host already serves"));
    for code in [None, Some(0), Some(1)] {
        assert_eq!(
            why_not_ready(code),
            "the session host ended before it was ready",
            "{code:?}"
        );
    }
}

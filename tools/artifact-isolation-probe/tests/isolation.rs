// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! ADR 0120 §3's five tests, against both renderings: each launches the probe once, in its own
//! process, so a page that loops or navigates away cannot touch the next question.

use std::process::Command;

fn asks(rendering: &str, test: &str) {
    let out = Command::new(env!("CARGO_BIN_EXE_artifact-isolation-probe"))
        .args([rendering, test])
        .output()
        .expect("the probe starts");
    let said = String::from_utf8_lossy(&out.stdout);
    let result = said
        .lines()
        .filter(|line| line.starts_with("RESULT ") || line.starts_with("NOTE "))
        .collect::<Vec<_>>()
        .join("\n");
    println!("{result}");
    assert!(
        out.status.success(),
        "{rendering} {test}: {result}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

mod native_webview {
    use super::asks;

    #[test]
    fn a_page_reaches_no_tauri_command_not_even_the_vault_values() {
        asks("webview", "reach");
    }

    #[test]
    fn an_endless_loop_in_a_page_leaves_the_window_responding() {
        asks("webview", "loop");
    }

    #[test]
    fn neither_a_second_artifact_nor_a_nested_frame_speaks_on_a_pages_channel() {
        asks("webview", "speak");
    }

    #[test]
    fn a_script_free_page_loads_and_navigates_to_nothing_off_its_folder() {
        asks("webview", "escape");
    }

    #[test]
    fn no_request_walks_out_of_the_folder() {
        asks("webview", "walk");
    }
}

/// The rendering ADR 0120 rejected on this evidence: on macOS it failed the loop (WebKit runs the
/// frame on the window's own thread) and the escape (the window's `frame-src` cannot tell one
/// artifact's origin from another's). Kept to measure again, not run by default.
mod sandboxed_frame {
    use super::asks;

    #[test]
    #[ignore = "the rejected rendering: `cargo test -- --ignored` measures it again"]
    fn a_page_reaches_no_tauri_command_not_even_the_vault_values() {
        asks("frame", "reach");
    }

    #[test]
    #[ignore = "the rejected rendering: `cargo test -- --ignored` measures it again"]
    fn an_endless_loop_in_a_page_leaves_the_window_responding() {
        asks("frame", "loop");
    }

    #[test]
    #[ignore = "the rejected rendering: `cargo test -- --ignored` measures it again"]
    fn neither_a_second_artifact_nor_a_nested_frame_speaks_on_a_pages_channel() {
        asks("frame", "speak");
    }

    #[test]
    #[ignore = "the rejected rendering: `cargo test -- --ignored` measures it again"]
    fn a_script_free_page_loads_and_navigates_to_nothing_off_its_folder() {
        asks("frame", "escape");
    }

    #[test]
    #[ignore = "the rejected rendering: `cargo test -- --ignored` measures it again"]
    fn no_request_walks_out_of_the_folder() {
        asks("frame", "walk");
    }
}

/// The control: the native webview with every guard off (its label granted what the window is,
/// no CSP, no navigation guard, files read from disk at request time). Each test that the
/// guards make pass must fail here, or it is not testing them.
mod native_webview_unguarded {
    use std::process::Command;

    fn fails(test: &str) {
        let out = Command::new(env!("CARGO_BIN_EXE_artifact-isolation-probe"))
            .args(["webview", test, "unguarded"])
            .output()
            .expect("the probe starts");
        let said = String::from_utf8_lossy(&out.stdout);
        let result = said
            .lines()
            .find(|l| l.starts_with("RESULT "))
            .unwrap_or("no result");
        println!("{result}");
        assert!(
            result.contains(" FAIL "),
            "unguarded, {test} should fail for its reason, and said: {result}"
        );
    }

    #[test]
    fn a_page_granted_the_windows_commands_reaches_the_vault_values() {
        fails("reach");
    }

    #[test]
    fn a_nested_frame_with_no_csp_speaks_on_its_pages_channel() {
        fails("speak");
    }

    #[test]
    fn a_page_with_no_csp_or_navigation_guard_gets_off_its_folder() {
        fails("escape");
    }

    #[test]
    fn a_page_served_from_disk_walks_out_of_its_folder() {
        fails("walk");
    }
}

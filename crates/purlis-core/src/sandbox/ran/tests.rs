// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How a run is sandboxed, read from what its host starts (#2636).

use std::ffi::OsString;

use purlis_session_protocol::session::RunSandbox;

use super::applied_to;
use crate::sandbox::Os;

fn words(all: &[&str]) -> Vec<OsString> {
    all.iter().map(OsString::from).collect()
}

fn by(word: &str) -> RunSandbox {
    RunSandbox::By(word.to_owned())
}

#[test]
fn a_program_seatbelt_wraps_runs_in_seatbelt() {
    let ran = applied_to(
        "/usr/bin/sandbox-exec".as_ref(),
        &words(&["-p", "(version 1)", "codex"]),
        true,
        Os::MacOs,
    );
    assert_eq!(ran, by("seatbelt"));
}

#[test]
fn a_program_purlis_wraps_on_linux_runs_in_landlock() {
    let ran = applied_to(
        "/opt/purlis/bin/purlis".as_ref(),
        &words(&["_sandbox", "--profile", "{}", "--", "codex"]),
        true,
        Os::Linux,
    );
    assert_eq!(ran, by("landlock"));
}

#[test]
fn a_claude_code_line_carrying_its_sandbox_runs_in_claude_codes_own() {
    let settings = r#"{"sandbox":{"enabled":true},"permissions":{"deny":[]}}"#;
    for (os, word) in [(Os::MacOs, "seatbelt"), (Os::Linux, "bubblewrap")] {
        assert_eq!(
            applied_to(
                "/usr/local/bin/claude".as_ref(),
                &words(&["--settings", settings]),
                false,
                os
            ),
            by(word)
        );
    }
}

#[test]
fn a_run_held_by_its_confinement_alone_is_sandboxed_however_it_looks() {
    // Confined, with nothing on its line that names a sandbox: a start cannot make a confined
    // run read as off.
    let ran = applied_to(
        "/bin/sh".as_ref(),
        &words(&[
            "-c",
            "exec claude --settings '{\"sandbox\":{\"enabled\":false}}'",
        ]),
        true,
        Os::MacOs,
    );
    assert_eq!(ran, by("proxy"));
}

#[test]
fn a_run_with_no_confinement_and_nothing_wrapping_it_is_off() {
    for line in [
        words(&["-c", "echo"]),
        words(&["--settings", r#"{"sandbox":{"enabled":false}}"#]),
        words(&["--settings", "not json"]),
        words(&["--settings"]),
        words(&["--profile", "_sandbox"]),
    ] {
        assert_eq!(
            applied_to("/bin/sh".as_ref(), &line, false, Os::MacOs),
            RunSandbox::Off,
            "{line:?}"
        );
    }
}

/// A Claude Code line whose settings went through a file purlis wrote is read from that file,
/// and only while it holds what was written: one that holds anything else is read as sandboxed,
/// in doubt, whatever it now says.
#[test]
fn a_claude_code_line_whose_settings_file_changed_is_read_as_sandboxed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let long = |enabled: bool| {
        format!(
            r#"{{"sandbox":{{"enabled":{enabled}}},"deny":["{}"]}}"#,
            "x".repeat(crate::hoststart::SETTINGS_IN_A_WORD * 2)
        )
    };
    let start = |settings: &str| {
        crate::hoststart::settings_in_a_file(
            root,
            "01J0000000000000000000000A",
            vec!["--settings".to_owned(), settings.to_owned()],
        )
        .unwrap()
    };
    let ran = |args: &[String]| {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        applied_to(
            "/usr/local/bin/claude".as_ref(),
            &words(&args),
            false,
            Os::MacOs,
        )
    };

    let off = start(&long(false));
    assert_eq!(ran(&off), RunSandbox::Off);
    // The same file, holding what purlis did not write there.
    std::fs::write(&off[1], long(true).replacen("x", "y", 1)).unwrap();
    assert_eq!(ran(&off), by("seatbelt"));

    let on = start(&long(true));
    assert_eq!(ran(&on), by("seatbelt"));
    std::fs::write(&on[1], long(false)).unwrap();
    assert_eq!(ran(&on), by("seatbelt"), "never read as off");
}

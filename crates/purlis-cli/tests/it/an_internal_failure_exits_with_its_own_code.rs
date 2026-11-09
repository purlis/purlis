// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A `purlis` command that fails inside purlis exits 70, and only then** (#5697, spec #5683
//! Q23): the one code purlis's own hook drafts a Report of. A refusal and a usage error exit 1.

use crate::support;

use purlis_core::caught::INTERNAL_EXIT;

/// What makes a debug build's `purlis` panic before it answers (#349).
const PANICS: (&str, &str) = ("CHARTER_TEST_HOOK_PANICS", "1");

fn run(args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String) {
    let dir = tempfile::tempdir().expect("a directory");
    let mut command = support::purlis();
    command
        .args(args)
        .current_dir(dir.path())
        .env("HOME", dir.path());
    for (name, value) in env {
        command.env(name, value);
    }
    let out = command.output().expect("purlis runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_command_that_panics_exits_with_the_internal_failure_code() {
    let (code, err) = run(&["version"], &[PANICS]);
    assert_eq!(code, Some(i32::from(INTERNAL_EXIT)), "{err}");
    assert!(err.starts_with("purlis: internal failure: "), "{err}");
}

#[test]
fn a_usage_error_never_exits_with_the_internal_failure_code() {
    let (code, err) = run(&["worktree", "comit"], &[]);
    assert_eq!(code, Some(1), "{err}");
}

/// A plane whose `workspaces/` is a link to itself: a listing of it fails with an error no
/// command has an arm for (ELOOP), whoever runs the suite, root too.
fn a_plane_whose_workspaces_cannot_be_listed() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("plane");
    std::fs::create_dir_all(&root).expect("a plane");
    std::fs::create_dir_all(dir.path().join("home")).expect("a home");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("its manifest");
    let workspaces = root.join("workspaces");
    std::os::unix::fs::symlink(&workspaces, &workspaces).expect("a link to itself");
    dir
}

/// `purlis <args>` on the plane in `dir`, as a person's terminal runs it.
fn on_the_plane(dir: &tempfile::TempDir, args: &[&str]) -> (Option<i32>, String) {
    let root = dir.path().join("plane");
    let mut command = support::purlis();
    command
        .args(args)
        .current_dir(&root)
        .env("PURLIS_ROOT", &root)
        .env("HOME", dir.path().join("home"))
        .env("NO_COLOR", "1");
    for name in [
        "PURLIS_WORKSPACE",
        "PURLIS_PERSONA",
        "CLAUDE_CODE_SESSION_ID",
    ] {
        for spelling in purlis_core::envvar::spellings(name) {
            command.env_remove(spelling);
        }
    }
    let out = command.output().expect("purlis runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// What Claude Code hands purlis's hook for the shell command `line` that exited `code` and
/// said `stderr`: the payload layer 1 drafts from (#5697).
fn failed_on_the_shell(line: &str, code: Option<i32>, stderr: &str) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": line, "description": "run it"},
        "tool_use_id": "toolu_01",
        "error": format!("Exit code {}\n{stderr}", code.unwrap_or_default()),
        "is_interrupt": false,
    })
}

#[cfg(unix)]
#[test]
fn an_unhandled_io_error_exits_with_the_internal_failure_code_and_is_drafted() {
    let plane = a_plane_whose_workspaces_cannot_be_listed();

    for args in [&["workspace", "list"][..], &["workspace", "optimize"]] {
        let (code, err) = on_the_plane(&plane, args);

        assert_eq!(code, Some(i32::from(INTERNAL_EXIT)), "{args:?}: {err}");
        assert!(
            err.starts_with("purlis: internal failure: workspaces/ could not be listed: "),
            "{args:?}: {err}"
        );
        let line = format!("purlis {}", args.join(" "));
        let caught = purlis_core::caught::of_hook(
            purlis_core::caught::COMMAND_WORD,
            &failed_on_the_shell(&line, code, &err),
        )
        .expect("layer 1 drafts it");
        assert_eq!(caught.call, line);
        assert_eq!(caught.exit, Some(i32::from(INTERNAL_EXIT)));
        assert!(caught.error.contains("could not be listed"), "{caught:?}");
    }
}

#[cfg(unix)]
#[test]
fn a_refusal_and_a_usage_error_of_a_moved_command_exit_one_and_are_never_drafted() {
    let plane = a_plane_whose_workspaces_cannot_be_listed();

    for args in [
        // A refusal: the workspace named is not there.
        &["workspace", "optimize", "nowhere"][..],
        // A refusal the core says, through the `String` path a moved command keeps.
        &["workspace", "vision", "-w", "../out"],
        // A usage error clap answers.
        &["workspace", "optimize", "--no-such-flag"],
        &["skill", "edit"],
    ] {
        let (code, err) = on_the_plane(&plane, args);

        assert_eq!(code, Some(1), "{args:?}: {err}");
        assert!(!err.contains("internal failure"), "{args:?}: {err}");
        let line = format!("purlis {}", args.join(" "));
        assert_eq!(
            purlis_core::caught::of_hook(
                purlis_core::caught::COMMAND_WORD,
                &failed_on_the_shell(&line, code, &err),
            ),
            None,
            "{args:?}"
        );
    }
}

#[test]
fn a_hook_that_panics_keeps_its_own_end() {
    // A guard's crash is a refusal (exit 2), and a reporting hook's any other failure: neither is
    // a command a chat ran, and neither is drafted.
    let (code, err) = run(&["hook", "stop"], &[PANICS]);
    assert_ne!(code, Some(i32::from(INTERNAL_EXIT)), "{err}");
    assert_ne!(code, Some(0), "{err}");
}

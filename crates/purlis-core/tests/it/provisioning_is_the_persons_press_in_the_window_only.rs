// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Provisioning is the person's press, in the window** (ADR 0128 D5).
//!
//! Signing in, pinning a host's application, choosing the account that serves a repo, trusting a
//! clone and the second token's sign-in are never done from a chat, the command line or a socket.
//! Three things hold that, and each is checked here:
//!
//! 1. **A chat's process tree is refused.** The one place the window claims the host's scope
//!    ([`provision::on_the_persons_press`]) answers the refusal inside a process that carries a
//!    chat's marks, and the scope outside one. Every act that provisions takes that scope, so a
//!    caller that cannot claim it cannot call.
//! 2. **No link serves a provisioning command**, and each is in the list of the window's own.
//! 3. **No source but the window's calls a provisioning entry point.** The `purlis` binary, the
//!    session host and every module of the core outside the forge's own are read for the names.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use purlis_core::forge::provision::{
    PROVISIONING_COMMANDS, PROVISIONING_ENTRY_POINTS, REFUSAL, on_the_persons_press,
};
use purlis_session_protocol::ui;

const EXPECT: &str = "PROVISIONING_EXPECTED";

#[test]
fn inside_a_chats_process_tree_a_provisioning_act_is_refused_with_where_to_do_it() {
    purlis_core::unsteered!();
    let child = "provisioning_is_the_persons_press_in_the_window_only::a_child_claims_the_persons_press_only_outside_a_chat";
    for chat_mark in ["PURLIS_CHAT", "PURLIS_SESSION_ID"] {
        purlis_core::testrun::rerun(
            &[child],
            &[
                (chat_mark, OsStr::new("7")),
                (EXPECT, OsStr::new("refused")),
            ],
        );
    }
    purlis_core::testrun::rerun(&[child], &[(EXPECT, OsStr::new("pressed"))]);
}

/// The child: claims the person's press as the window does; in any run this binary did not start
/// for the test it does nothing.
#[test]
fn a_child_claims_the_persons_press_only_outside_a_chat() {
    purlis_core::unsteered!();
    let Ok(expected) = std::env::var(EXPECT) else {
        return;
    };
    match (expected.as_str(), on_the_persons_press()) {
        ("refused", Err(said)) => {
            assert_eq!(said, REFUSAL);
            assert!(said.contains("window"), "{said}");
        }
        ("pressed", Ok(_)) => {}
        (wanted, got) => panic!("wanted {wanted}, got {:?}", got.map(|_| "a scope")),
    }
}

#[test]
fn every_provisioning_command_is_the_windows_alone_and_no_link_serves_one() {
    purlis_core::unsteered!();
    for command in PROVISIONING_COMMANDS {
        assert!(
            ui::window_only(command),
            "{command} provisions, so it must be one of the commands only the window runs"
        );
        assert!(
            !ui::served_on_a_link(command),
            "{command} provisions and is served on a link"
        );
    }
    // The commands this slice adds are among them.
    for command in [
        "forge_use_application",
        "forge_choose_project_account",
        "forge_trust_clone_repo",
        "forge_ci_permission_start",
        "forge_ci_permission_wait",
    ] {
        assert!(PROVISIONING_COMMANDS.contains(&command), "{command}");
    }
}

/// Every `.rs` file under `dir`, but not a test's.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == "tests" || n == "target")
            {
                continue;
            }
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains("tests"))
        {
            out.push(path);
        }
    }
}

/// The lines of `text` that are code: a comment line holds a name without calling it.
fn code(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| !l.trim_start().starts_with("//"))
}

#[test]
fn no_source_but_the_windows_and_the_forge_modules_calls_a_provisioning_entry_point() {
    purlis_core::unsteered!();
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    // The command line and the session host: what a chat's `purlis` runs, and what serves a link.
    for dir in ["purlis-cli/src", "host/src", "session-protocol/src"] {
        sources(&crates.join(dir), &mut found);
    }
    // The core outside the forge module: nothing there provisions, but the window's own helpers
    // (`windowonly`, which the app calls with its scope) and the forge's own modules.
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut core_sources = Vec::new();
    sources(&core, &mut core_sources);
    let allowed_in_core = [
        "src/forge/",
        "src/windowonly.rs",
        "src/signingsetup.rs",
        "src/repolist.rs",
        "src/testrun.rs",
        "src/applog.rs",
    ];
    found.extend(core_sources.into_iter().filter(|path| {
        let shown = path.to_string_lossy();
        !allowed_in_core.iter().any(|ok| shown.contains(ok))
    }));
    assert!(found.len() > 50, "the scan found {} sources", found.len());
    let mut calls = Vec::new();
    for path in &found {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for name in PROVISIONING_ENTRY_POINTS {
            if code(&text).any(|line| line.contains(name)) {
                calls.push(format!("{} calls {name}", path.display()));
            }
        }
    }
    assert!(
        calls.is_empty(),
        "a source that is not the window's provisions:\n{}",
        calls.join("\n")
    );
}

#[test]
fn the_scan_finds_a_provisioning_call_where_there_is_one() {
    purlis_core::unsteered!();
    // The control: the window's own file, which the scan skips, does call them.
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/src/onelogin.rs");
    let text = std::fs::read_to_string(app).expect("the app's file");
    assert!(
        PROVISIONING_ENTRY_POINTS
            .iter()
            .any(|name| code(&text).any(|line| line.contains(name))),
        "the window's file calls a provisioning entry point"
    );
}

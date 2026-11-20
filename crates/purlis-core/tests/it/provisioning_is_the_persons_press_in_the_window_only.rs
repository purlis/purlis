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

/// The sources of one directory, which must exist and hold at least `at_least` of them: a scan
/// of a folder that was renamed or moved reads nothing and must not pass for a clean one.
fn scanned(crates: &Path, dir: &str, at_least: usize) -> Vec<PathBuf> {
    let at = crates.join(dir);
    assert!(
        at.is_dir(),
        "the scanned directory {} is missing",
        at.display()
    );
    let mut found = Vec::new();
    sources(&at, &mut found);
    assert!(
        found.len() >= at_least,
        "the scan found {} sources in {dir}, and wants at least {at_least}",
        found.len()
    );
    found
}

/// The core's own files that may hold a provisioning entry point's name: the forge's modules that
/// define or use one, **each named** (a new file in `src/forge/` is not exempt until it is added
/// here, with the reason it calls one), and the window's helpers.
const ALLOWED_IN_CORE: &[&str] = &[
    "src/forge/accounts.rs",
    "src/forge/accounts/workflow.rs",
    "src/forge/confirmed.rs",
    "src/forge/device.rs",
    "src/forge/gitcredential/vouch.rs",
    "src/forge/provision.rs",
    "src/forge/refresh.rs",
    "src/forge/registered/owner.rs",
    "src/forge/signin.rs",
    "src/windowonly.rs",
    "src/signingsetup.rs",
    "src/repolist.rs",
    "src/testrun.rs",
    "src/applog.rs",
];

fn is_allowed_in_core(path: &Path) -> bool {
    let shown = path.to_string_lossy().replace('\\', "/");
    ALLOWED_IN_CORE.iter().any(|ok| shown.ends_with(ok))
}

#[test]
fn no_source_but_the_windows_and_the_named_forge_files_calls_a_provisioning_entry_point() {
    purlis_core::unsteered!();
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // Each directory is held to its own count, so one that emptied cannot hide behind the others.
    let mut found = Vec::new();
    for (dir, at_least) in [
        ("purlis-cli/src", 30),
        ("host/src", 15),
        ("session-protocol/src", 12),
        ("purlis-core/src", 600),
    ] {
        let all = scanned(&crates, dir, at_least);
        found.extend(all.into_iter().filter(|path| !is_allowed_in_core(path)));
    }
    for ok in ALLOWED_IN_CORE {
        assert!(
            crates.join("purlis-core").join(ok).is_file(),
            "{ok} is allowed to call a provisioning entry point and is not a file"
        );
    }
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

/// Whether `line` names `symbol` as an identifier (an import, a path or a call), however it is
/// spelled: `workflow::for_ci_push(`, `use ...::{for_ci_push, ...}`, `as`-renamed or bare.
fn names_symbol(line: &str, symbol: &str) -> bool {
    line.match_indices(symbol).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + symbol.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}

#[test]
fn only_the_window_names_the_ci_push_credential_and_nothing_reads_the_second_token_by_name() {
    purlis_core::unsteered!();
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    for (dir, at_least) in [
        ("purlis-cli/src", 30),
        ("host/src", 15),
        ("session-protocol/src", 12),
        ("purlis-core/src", 600),
    ] {
        found.extend(scanned(&crates, dir, at_least));
    }
    let mut bad = Vec::new();
    for path in &found {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let shown = path.to_string_lossy().replace('\\', "/");
        // `for_ci_push` is defined in `workflow.rs` and listed in `provision.rs`; the window and
        // slice F2's held push call it, from the app, which is not scanned here.
        let owns_it = shown.ends_with("src/forge/accounts/workflow.rs")
            || shown.ends_with("src/forge/provision.rs");
        for line in code(&text) {
            if !owns_it && names_symbol(line, "for_ci_push") {
                bad.push(format!("{shown} names for_ci_push: {}", line.trim()));
            }
            // The keyring item of the second token is read by its name in `workflow.rs` alone.
            let reads_item = ["get(", "exists(", "delete(", "set("]
                .iter()
                .any(|call| line.contains(call))
                && line.contains("\"workflow\"");
            if !shown.ends_with("src/forge/accounts/workflow.rs") && reads_item {
                bad.push(format!("{shown} reads the workflow item: {}", line.trim()));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn the_symbol_scan_sees_an_import_a_rename_and_a_call_and_not_a_longer_name() {
    purlis_core::unsteered!();
    for hit in [
        "use purlis_core::forge::accounts::workflow::for_ci_push;",
        "use x::workflow::{keep, for_ci_push as push};",
        "    workflow::for_ci_push(&scope, k, &a)",
        "let f = for_ci_push;",
    ] {
        assert!(names_symbol(hit, "for_ci_push"), "{hit}");
    }
    for miss in ["let x = for_ci_push_later(1);", "let my_for_ci_push = 1;"] {
        assert!(!names_symbol(miss, "for_ci_push"), "{miss}");
    }
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

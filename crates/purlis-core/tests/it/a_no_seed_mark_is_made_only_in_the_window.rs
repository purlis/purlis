// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat can't mark a key as no 2FA seed, whichever route it tries** (#5237). The mark lets a
//! value that looks like a seed into a page, so it is the person's word alone: one window command
//! makes it, and that command is served on no link. A CLI verb, a chat tool, a session protocol
//! method or a settings reader that made one would be a route for a chat, so this reads every Rust
//! file in the workspace and fails on any second caller.

use std::path::{Path, PathBuf};

/// The one file outside the store's own that may make a mark: the window's commands.
const THE_WINDOWS: &str = "app/src-tauri/src/vaults.rs";

/// What a caller of the store's writer, or of the commands, is spelled with.
const CALL: &str = "no_seed::mark(";

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" || name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, into);
        } else if name.ends_with(".rs") {
            into.push(path);
        }
    }
}

#[test]
fn the_only_caller_of_the_marks_writer_is_the_windows_command() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("app/src-tauri/src"), &mut files);
    assert!(
        files.len() > 100,
        "the workspace was found: {}",
        files.len()
    );

    let mut callers: Vec<String> = files
        .iter()
        .filter(|file| !file.ends_with("a_no_seed_mark_is_made_only_in_the_window.rs"))
        .filter(|file| std::fs::read_to_string(file).is_ok_and(|text| text.contains(CALL)))
        .map(|file| {
            file.strip_prefix(&root)
                .expect("under the workspace")
                .display()
                .to_string()
        })
        .collect();
    callers.sort();
    assert_eq!(callers, [THE_WINDOWS]);
}

#[test]
fn the_commands_that_make_and_take_off_a_mark_reach_no_link() {
    use purlis_session_protocol::ui;
    for command in ["vault_secret_mark_no_seed", "vault_secret_unmark_no_seed"] {
        assert!(ui::window_only(command), "{command}");
        assert!(!ui::SERVED_ON_A_LINK.contains(&command), "{command}");
    }
}

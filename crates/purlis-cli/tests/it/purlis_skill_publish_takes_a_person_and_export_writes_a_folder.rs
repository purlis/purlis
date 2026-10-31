// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill publish` and `export` (SK-18, #2781). The publish itself is the core's seam test:
//! here, that nothing but a person at a terminal reaches it.

use crate::support;

use std::path::Path;
use std::process::{Output, Stdio};

fn purlis(at: &Path, args: &[&str], chat: bool) -> Output {
    let mut command = support::purlis();
    command
        .args(["skill"])
        .args(args)
        .current_dir(at)
        .env("CHARTER_ROOT", at)
        .env("HOME", at.join("home"))
        .env("PURLIS_CONFIG_HOME", at.join("home/config"))
        .env("PURLIS_SKILL_STUDIO", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null());
    for spelling in purlis_core::envvar::spellings("PURLIS_SESSION_ID") {
        command.env_remove(spelling);
    }
    if chat {
        command.env("PURLIS_SESSION_ID", "s1");
    }
    command.output().unwrap()
}

fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "").unwrap();
    let dir = tmp.path().join("skills/deploy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: D\n---\n",
    )
    .unwrap();
    tmp
}

const TO: &str = "https://github.com/acme/skills";

#[test]
fn publish_with_no_terminal_sends_nothing_and_asks_nothing_of_the_source() {
    let tmp = project();
    let out = purlis(tmp.path(), &["publish", "deploy", "--to", TO], false);
    assert_eq!(out.status.code(), Some(1));
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("from a person at a terminal"), "{said}");
    assert!(!String::from_utf8_lossy(&out.stdout).contains("Publish to"));
}

#[test]
fn publish_in_a_chat_is_refused() {
    let tmp = project();
    let out = purlis(tmp.path(), &["publish", "deploy", "--to", TO], true);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn publish_has_no_flag_to_skip_the_question() {
    let tmp = project();
    for flag in ["--yes", "-y", "--digest"] {
        let out = purlis(tmp.path(), &["publish", "deploy", "--to", TO, flag], false);
        assert_eq!(out.status.code(), Some(2), "{flag}");
    }
}

#[test]
fn export_writes_a_folder_and_a_zip_and_never_over_what_is_there() {
    let tmp = project();
    let (folder, zip) = (tmp.path().join("out"), tmp.path().join("out.zip"));
    let (folder_arg, zip_arg) = (folder.to_str().unwrap(), zip.to_str().unwrap());

    assert!(
        purlis(tmp.path(), &["export", "deploy", "--to", folder_arg], false)
            .status
            .success()
    );
    assert!(folder.join("deploy/SKILL.md").is_file());
    assert!(
        purlis(
            tmp.path(),
            &["export", "deploy", "--to", zip_arg, "--zip"],
            false
        )
        .status
        .success()
    );
    assert!(zip.is_file());
    assert!(
        !purlis(tmp.path(), &["export", "deploy", "--to", folder_arg], false)
            .status
            .success()
    );
}

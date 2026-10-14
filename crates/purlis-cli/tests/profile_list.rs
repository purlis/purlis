// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis profile list` — every profile purlis read, with the kind of sign-in each has
//! (#2031, spec #2030), the file it came from, and why any was refused.

mod support;

use std::fs;
use std::path::Path;

fn purlis(root: &Path) -> String {
    let out = support::purlis()
        .args(["profile", "list"])
        .current_dir(root)
        .env("CHARTER_ROOT", root)
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "",
        "the listing is a report, and purlis prints its reports on stderr"
    );
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn plane(local: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("charter.toml"), "").unwrap();
    if !local.is_empty() {
        fs::write(dir.path().join("charter.local.toml"), local).unwrap();
    }
    dir
}

#[test]
fn each_built_in_profile_is_listed_with_the_sign_in_its_harness_gives_it() {
    let dir = plane("");

    assert_eq!(
        purlis(dir.path()),
        "  NAME      KIND      SIGN-IN       COMMAND   FROM\n\
         \x20 claude    claude    subscription  claude    built-in\n\
         \x20 opencode  opencode  subscription  opencode  built-in\n\
         \x20 codex     codex     subscription  codex     built-in\n"
    );
}

#[test]
fn a_profile_of_a_declared_harness_shows_the_sign_in_its_declaration_gives_or_unknown() {
    let dir = plane(
        "[harness]\ndefault = \"claude-work\"\n\n\
         [harness.claude-work]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
    );
    fs::create_dir_all(dir.path().join("harnesses")).unwrap();
    fs::write(
        dir.path().join("harnesses/llamas.toml"),
        "name = \"llamas\"\nprogram = \"llamas\"\n[sign_in]\nkinds = [\"local\"]\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("harnesses/aider.toml"),
        "name = \"aider\"\nprogram = \"aider\"\n",
    )
    .unwrap();

    assert_eq!(
        purlis(dir.path()),
        "  NAME         KIND      SIGN-IN       COMMAND   FROM\n\
         \x20 claude       claude    subscription  claude    built-in\n\
         \x20 opencode     opencode  subscription  opencode  built-in\n\
         \x20 codex        codex     subscription  codex     built-in\n\
         \x20 aider        aider     unknown       aider     harnesses\n\
         * claude-work  claude    subscription  claude    charter.local.toml\n\
         \x20 llamas       llamas    local         llamas    harnesses\n"
    );
}

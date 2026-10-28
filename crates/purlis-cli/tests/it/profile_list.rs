// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis profile list` — every profile purlis read, with the kind of sign-in each has
//! (#2031, spec #2030), the file it came from, and why any was refused.

use crate::support;

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
fn a_profile_whose_sign_in_purlis_cannot_see_is_listed_as_unknown() {
    // A subscription is signed in through the harness's own flow, and purlis never looks.
    let dir = plane("");

    assert_eq!(
        purlis(dir.path()),
        "  NAME      KIND      SIGN-IN  COMMAND   FROM\n\
         \x20 claude    claude    unknown  claude    built-in\n\
         \x20 opencode  opencode  unknown  opencode  built-in\n\
         \x20 codex     codex     unknown  codex     built-in\n"
    );
}

#[test]
fn each_profile_shows_the_sign_in_purlis_can_see() {
    let dir = plane(
        "[chat_env]\npass = [\"OPENAI_API_KEY\"]\n\n\
         [harness]\ndefault = \"claude-work\"\n\n\
         [harness.claude-work]\nkind = \"claude\"\ncommand = [\"claude\"]\n\n\
         [harness.claude-aws]\nkind = \"claude\"\ncommand = [\"claude\"]\n\
         env = { CLAUDE_CODE_USE_BEDROCK = \"1\" }\n",
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
        "  NAME         KIND      SIGN-IN  COMMAND                           FROM\n\
         \x20 claude       claude    unknown  claude                            built-in\n\
         \x20 opencode     opencode  api-key  opencode                          built-in\n\
         \x20 codex        codex     api-key  codex                             built-in\n\
         \x20 aider        aider     unknown  aider                             harnesses\n\
         \x20 claude-aws   claude    cloud    CLAUDE_CODE_USE_BEDROCK=1 claude  charter.local.toml\n\
         * claude-work  claude    unknown  claude                            charter.local.toml\n\
         \x20 llamas       llamas    local    llamas                            harnesses\n"
    );
}

/// `purlis profile list` on a machine whose home and config home are `machine`'s own.
fn purlis_on(root: &Path, machine: &Path) -> String {
    let out = support::purlis()
        .args(["profile", "list"])
        .current_dir(root)
        .env("CHARTER_ROOT", root)
        .env("HOME", machine.join("home"))
        .env("PURLIS_CONFIG_HOME", machine.join("config"))
        .env_remove("CHARTER_CONFIG_HOME")
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{out:?}");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_harness_folder_this_machine_adopted_is_listed_by_its_title_and_folder() {
    let dir = plane("");
    let machine = tempfile::tempdir().unwrap();
    let home = machine.path().join("home");
    fs::create_dir_all(home.join(".claude")).unwrap();
    purlis_core::profilestore::adopt(&machine.path().join("config"), &home, &|_: &str| None)
        .unwrap();

    let listed = purlis_on(dir.path(), machine.path());

    let here = purlis_core::profilestore::this_device();
    assert!(
        listed.contains("built-in\n")
            && listed.contains(&format!(
                "claude    claude    unknown  claude    Claude Code ({here}) at ~/.claude\n"
            )),
        "{listed}"
    );
}

#[test]
fn listing_the_profiles_adopts_nothing() {
    // The app adopts, from the environment its chats start in; a terminal's may differ.
    let dir = plane("");
    let machine = tempfile::tempdir().unwrap();
    fs::create_dir_all(machine.path().join("home/.claude")).unwrap();

    let listed = purlis_on(dir.path(), machine.path());

    assert!(listed.contains("claude    built-in\n"), "{listed}");
    assert!(
        !purlis_core::profilestore::path(&machine.path().join("config")).exists(),
        "a listing writes no store"
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The profiles a wrapped chat starts under, applied on Linux by `purlis _sandbox`** (#1040).
//!
//! Each profile is built by the core as a start builds it ([`sandbox::decide`], then the
//! harness's own `profile`), for Codex and opencode, at the full sandbox and at the floor, and
//! applied whole by this build's own `purlis` to a shell that tries what a chat would: read a
//! vault, write a name a later program loads, write what nothing holds, and write outside the
//! project. So a change in how a profile is assembled is applied where it ships, on Linux as on
//! macOS (`tools/seatbelt-case.sh`).
//!
//! Never skipped on Linux: a kernel without Landlock ABI 6 fails here, saying so.
//!
//! With `PURLIS_LINUX_PROFILES_OUT` set, the sample projects, the home and each profile
//! (`<out>/<harness>-<level>.sb`) are kept there, so a real harness can be started under them by
//! hand.

#![cfg(target_os = "linux")]

use crate::support;

use std::fs;
use std::path::Path;

use purlis_core::harness::Harness;
use purlis_core::sandbox::{self, Decided, Form, Level, Machine, Os};

const LEVELS: [(Level, &str, &str); 2] = [
    (
        Level::Full,
        "on",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ),
    (Level::Floor, "floor", "schema = 1\n"),
];

fn lay_out(root: &Path, manifest: &str) {
    fs::create_dir_all(root.join(".purlis/vaults")).expect("the vaults folder");
    fs::create_dir_all(root.join("repo/.git/hooks")).expect("a clone");
    fs::write(root.join(".purlis/vaults/ops.json"), "{}\n").expect("a vault");
    fs::write(root.join("notes.json"), "{}\n").expect("a file nothing holds");
    fs::write(root.join("purlis.toml"), manifest).expect("the manifest");
}

/// What `script` printed, run by `sh` in `root` under `profile` by `purlis _sandbox`.
fn under(profile: &str, root: &Path, script: &str) -> String {
    let words = purlis_linux_sandbox::line(
        profile,
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), script.to_owned()],
    );
    let out = support::purlis()
        .args(&words)
        .current_dir(root)
        .env_clear()
        .envs(support::carrying(support::unsteered_with(&[])))
        .output()
        .expect("purlis ran");
    assert!(
        out.status.success(),
        "{:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn tries(what: &str) -> String {
    format!("if {what} 2>/dev/null; then echo yes; else echo no; fi")
}

#[test]
fn each_profile_a_wrapped_chat_starts_under_holds_on_linux() {
    purlis_core::unsteered!();
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {} (Linux 6.12 or \
         later)",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    let scratch = tempfile::tempdir().expect("a folder");
    let kept = std::env::var_os("PURLIS_LINUX_PROFILES_OUT").map(std::path::PathBuf::from);
    let out = kept.unwrap_or_else(|| scratch.path().to_path_buf());
    fs::create_dir_all(&out).expect("the folder written to");
    let out = out.canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    fs::create_dir_all(&home).expect("a home");
    fs::create_dir_all(&bin).expect("a folder on the PATH");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::Linux,
    };
    for (level, name, manifest) in LEVELS {
        let root = out.join(name).join("project");
        let tmp = out.join(name).join("tmp");
        lay_out(&root, manifest);
        fs::create_dir_all(&tmp).expect("a temporary folder");
        for harness in [Harness::Codex, Harness::Opencode] {
            let decided = sandbox::decide(harness, &root, &machine, &|_| true, None, None)
                .expect("decided")
                .expect("a sandbox in force");
            let Decided::Sandboxed(applied) = decided else {
                panic!("not sandboxed: {decided:?}");
            };
            assert_eq!(applied.level(), level, "{harness:?} {name}");
            let profile = match applied.form() {
                Form::Codex(wrap) => sandbox::codex::profile(wrap, &root, &tmp, &[4040], None),
                Form::Opencode(wrap) => {
                    sandbox::opencode::profile(wrap, &root, &tmp, &[4040], None)
                }
                Form::ClaudeCode(_) => panic!("Claude Code is not wrapped"),
            }
            .expect("a profile");
            fs::write(out.join(format!("{}-{name}.sb", harness.name())), &profile)
                .expect("the profile");
            let at = format!("{harness:?} at {name}");
            let script = [
                tries("head -c 1 .purlis/vaults/ops.json >/dev/null"),
                tries("touch repo/.git/hooks/pre-commit"),
                tries("mkdir -p deep/er && touch deep/er/.envrc"),
                tries("echo x >> notes.json"),
                tries(&format!("touch {}/outside", home.display())),
            ]
            .join("; ");
            let said = under(&profile, &root, &script);
            let said: Vec<&str> = said.lines().collect();
            let outside = if level == Level::Floor { "yes" } else { "no" };
            assert_eq!(
                said,
                vec!["no", "no", "no", "yes", outside],
                "{at}: a vault read, a hook written, a later-code name at depth, a file nothing \
                 holds, a file outside the project"
            );
            let _ = fs::remove_file(home.join("outside"));
        }
    }
}

/// At the floor, the plugin and the git hooks a wrapped chat is armed with are never written by
/// it, applied for real; what nothing holds beside them still is.
#[test]
fn a_wrapped_chat_at_the_floor_never_writes_the_plugin_or_git_hooks_it_is_armed_with() {
    purlis_core::unsteered!();
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {}",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    let plugin = out.join("install/plugin");
    let git_hooks = out.join("data/git-hooks");
    for folder in [&home, &bin, &plugin.join("hooks"), &git_hooks] {
        fs::create_dir_all(folder).expect("a folder");
    }
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::Linux,
    };
    let root = out.join("project");
    let tmp = out.join("tmp");
    lay_out(&root, "schema = 1\n");
    fs::create_dir_all(&tmp).expect("a temporary folder");
    for harness in [Harness::Codex, Harness::Opencode] {
        let decided = sandbox::decide(harness, &root, &machine, &|_| true, None, None)
            .expect("decided")
            .expect("a sandbox in force");
        let Decided::Sandboxed(mut applied) = decided else {
            panic!("not sandboxed: {decided:?}");
        };
        assert_eq!(applied.level(), Level::Floor, "{harness:?}");
        applied
            .hold_kit(
                sandbox::ArmedWith {
                    binary: None,
                    plugin: Some(&plugin),
                    git_hooks: Some(&git_hooks),
                    skill_sets: None,
                },
                &root,
                &[],
            )
            .expect("a kit outside what the chat writes");
        let profile = match applied.form() {
            Form::Codex(wrap) => sandbox::codex::profile(wrap, &root, &tmp, &[4040], None),
            Form::Opencode(wrap) => sandbox::opencode::profile(wrap, &root, &tmp, &[4040], None),
            Form::ClaudeCode(_) => panic!("Claude Code is not wrapped"),
        }
        .expect("a profile");
        let script = [
            tries(&format!("touch {}/hooks/hooks.json", plugin.display())),
            tries(&format!("touch {}/pre-commit", git_hooks.display())),
            tries(&format!(
                "mv {0} {0}.aside",
                plugin.parent().expect("a parent").display()
            )),
            tries(&format!("touch {}/outside", home.display())),
        ]
        .join("; ");
        let said = under(&profile, &root, &script);
        assert_eq!(
            said.lines().collect::<Vec<_>>(),
            vec!["no", "no", "no", "yes"],
            "{harness:?}: the plugin, the git hooks, the folder above the plugin, a file outside \
             nothing holds"
        );
    }
}

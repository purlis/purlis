// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Seatbelt profiles purlis emits, written out to be applied** (#2128).
//!
//! `tools/seatbelt-case.sh` applies, on the macOS runner, the profiles a wrapped chat (Codex,
//! opencode) starts under: the full sandbox's and the floor's, each built here the way a start
//! builds it ([`sandbox::decide`], then the harness's own `profile`), never copied by hand. So a
//! change in how a profile is assembled, a rule's order or a later allow among them, is applied
//! where it ships.
//!
//! With `PURLIS_SEATBELT_OUT` set, everything is written there and kept: for each level a sample
//! project at `<out>/<level>/project`, its chat's temporary folder at `<out>/<level>/tmp`, and
//! each profile at `<out>/<harness>-<level>.sb`. Without it, the same is built in a temporary
//! folder and only checked here.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::Harness;
use purlis_core::sandbox::{self, Decided, Form, Level, Machine, Os};

/// Each level, its folder's name and the manifest of a project that starts its chats there.
const LEVELS: [(Level, &str, &str); 2] = [
    (
        Level::Full,
        "on",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ),
    (Level::Floor, "floor", "schema = 1\n"),
];

/// Each wrapped harness and its profile's name.
const HARNESSES: [(Harness, &str); 2] =
    [(Harness::Codex, "codex"), (Harness::Opencode, "opencode")];

/// A sample project at `root` with `manifest`: a vault, a folder of what macOS starts at login
/// and one beside it, and a file nothing holds.
fn lay_out(root: &Path, manifest: &str) {
    fs::create_dir_all(root.join(".purlis/vaults")).expect("the vaults folder");
    fs::create_dir_all(root.join("Library/LaunchAgents")).expect("a held folder");
    fs::create_dir_all(root.join("Library/LaunchAgentz")).expect("a folder beside it");
    fs::write(root.join(".purlis/vaults/ops.json"), "{}\n").expect("a vault");
    fs::write(root.join("notes.json"), "{}\n").expect("a file nothing holds");
    fs::write(root.join("purlis.toml"), manifest).expect("the manifest");
}

/// The profile a chat of `harness` in the project at `root`, with its temporary folder `tmp`,
/// starts under on `machine`, and the level it starts at.
fn emitted(harness: Harness, root: &Path, tmp: &Path, machine: &Machine) -> (Level, String) {
    let decided = sandbox::decide(harness, root, machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    // No hook socket (the last argument): no app listens in this test, and a socket only adds
    // that one path to what a command may connect to, beside what the profile already holds,
    // which no probe of `tools/seatbelt-case.sh` touches.
    let profile = match applied.form() {
        Form::Codex(wrap) => sandbox::codex::profile(wrap, root, tmp, &[4040], None),
        Form::Opencode(wrap) => sandbox::opencode::profile(wrap, root, tmp, &[4040], None),
        Form::ClaudeCode(_) => panic!("Claude Code is not wrapped"),
    }
    .expect("a profile");
    (applied.level(), profile)
}

#[test]
fn the_profiles_a_wrapped_chat_starts_under_are_written_out() {
    purlis_core::unsteered!();
    let kept = std::env::var_os("PURLIS_SEATBELT_OUT").map(PathBuf::from);
    let scratch = tempfile::tempdir().expect("a folder");
    let out = kept.clone().unwrap_or_else(|| scratch.path().to_path_buf());
    fs::create_dir_all(&out).expect("the folder written to");
    let out = out.canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    fs::create_dir_all(&home).expect("a home");
    fs::create_dir_all(&bin).expect("a folder on the PATH");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home),
        os: Os::MacOs,
    };
    let mut written = 0;
    for (level, name, manifest) in LEVELS {
        let root = out.join(name).join("project");
        let tmp = out.join(name).join("tmp");
        lay_out(&root, manifest);
        fs::create_dir_all(&tmp).expect("a temporary folder");
        for (harness, called) in HARNESSES {
            let (at, profile) = emitted(harness, &root, &tmp, &machine);
            assert_eq!(at, level, "{called} in the {name} project");
            // The floor starts from everything allowed and the full sandbox from nothing.
            let base = match level {
                Level::Floor => "(allow default)",
                _ => "(deny default)",
            };
            assert!(profile.starts_with("(version 1)\n"), "{called}-{name}");
            assert!(profile.contains(base), "{called}-{name}: {base}");
            // No project's vaults are read, last of all.
            let last = sandbox::seatbelt::vaults_everywhere_rules()
                .expect("rules")
                .pop()
                .expect("a rule");
            assert!(
                profile.trim_end().ends_with(&last),
                "{called}-{name} ends with what no later rule gives back"
            );
            fs::write(out.join(format!("{called}-{name}.sb")), profile).expect("the profile");
            written += 1;
        }
    }
    assert_eq!(written, 4, "each wrapped harness at each level");
}

/// `/bin/ln from to` under `profile`: `Ok` where the link was made, else what `ln` said.
#[cfg(target_os = "macos")]
fn linked(profile: &str, from: &Path, to: &Path) -> Result<(), String> {
    let done = purlis_core::forklock::output(
        std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", profile, "/bin/ln"])
            .arg(from)
            .arg(to),
    )
    .expect("sandbox-exec runs");
    if done.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&done.stderr).into_owned())
    }
}

/// `/bin/cat path` under `profile`: whether the file was read.
#[cfg(target_os = "macos")]
fn read_under(profile: &str, path: &Path) -> bool {
    purlis_core::forklock::output(
        std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", profile, "/bin/cat"])
            .arg(path),
    )
    .expect("sandbox-exec runs")
    .status
    .success()
}

/// **A second name is made only for a file in what a chat may write**: under the full sandbox's
/// profile and the floor's, applied for real, `ln` from a folder held from the chat into the
/// folder it works in is refused with `EPERM`, and from a folder the full sandbox does not let
/// it write too, while a link inside the folder it works in is made as ever (git, cargo and npm
/// make those in their own folders).
///
/// A file the chat reads but may not write, the project's manifest and a workspace's, is given
/// no second name in the folder it works in either. `ln` looks at its source before it links,
/// so a source held from reading is refused before any link is asked for; these sources are
/// read under the same profile first, so the refusal is the link rule's.
#[cfg(target_os = "macos")]
#[test]
fn the_macos_profiles_limit_file_link_to_the_folders_a_chat_may_write() {
    purlis_core::unsteered!();
    let applies =
        purlis_core::forklock::status(std::process::Command::new("/usr/bin/sandbox-exec").args([
            "-p",
            "(version 1)(allow default)",
            "/usr/bin/true",
        ]))
        .is_ok_and(|status| status.success());
    if !applies {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this process is already sandboxed, so no second profile applies"
        );
        return;
    }
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    let elsewhere = out.join("elsewhere");
    fs::create_dir_all(&home).expect("a home");
    fs::create_dir_all(&bin).expect("a folder on the PATH");
    fs::create_dir_all(&elsewhere).expect("a folder no chat works in");
    fs::write(elsewhere.join("notes.txt"), "elsewhere\n").expect("a file there");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home),
        os: Os::MacOs,
    };
    let refused = |said: Result<(), String>, what: &str| match said {
        Ok(()) => panic!("{what}: the link was made"),
        Err(said) => assert!(said.contains("Operation not permitted"), "{what}: {said}"),
    };
    for (level, name, manifest) in LEVELS {
        let root = out.join(name).join("project");
        let tmp = out.join(name).join("tmp");
        lay_out(&root, manifest);
        fs::create_dir_all(&tmp).expect("a temporary folder");
        let workspace = root.join("workspaces/demo");
        fs::create_dir_all(&workspace).expect("a workspace");
        fs::write(workspace.join("workspace.json"), "{}\n").expect("its manifest");
        for (harness, called) in HARNESSES {
            let (_, profile) = emitted(harness, &root, &tmp, &machine);
            let what = format!("{called} at {name}");

            let again = root.join("notes.again");
            assert_eq!(
                linked(&profile, &root.join("notes.json"), &again),
                Ok(()),
                "{what}: a link inside the folder it works in"
            );
            fs::remove_file(&again).expect("the link taken away");

            refused(
                linked(
                    &profile,
                    &root.join(".purlis/vaults/ops.json"),
                    &root.join("ops.again"),
                ),
                &format!("{what}: a link from a held folder"),
            );
            assert!(!root.join("ops.again").exists(), "{what}");
            if level == Level::Full {
                refused(
                    linked(
                        &profile,
                        &elsewhere.join("notes.txt"),
                        &root.join("elsewhere.again"),
                    ),
                    &format!("{what}: a link from a folder it may not write"),
                );
            }

            for (held, kept) in [
                (root.join("purlis.toml"), "the project's manifest"),
                (workspace.join("workspace.json"), "a workspace's manifest"),
            ] {
                assert!(read_under(&profile, &held), "{what}: {kept} is read");
                let again = root.join("x");
                refused(
                    linked(&profile, &held, &again),
                    &format!("{what}: a link from {kept}"),
                );
                assert!(!again.exists(), "{what}: no second name for {kept}");
            }
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis migrate [--undo]` and `doctor --fix rename-local` as processes (RN-5): what they move
//! on a temporary home, what they print, and that running either again is safe.
//!
//! What each move does is `purlis-core`'s test (`rename_local_moves_local_state_and_puts_it_back`);
//! these are the parts only a real process has: the environment it reads its homes from, its
//! output and its exit.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::Output;

struct Machine {
    _dir: tempfile::TempDir,
    home: PathBuf,
    plane: PathBuf,
}

fn machine() -> Machine {
    let dir = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(dir.path()).unwrap();
    let plane = home.join("plane");
    std::fs::create_dir_all(plane.join(".charter")).unwrap();
    let init = support::command("git")
        .args(["init", "-q"])
        .current_dir(&plane)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(init.status.success(), "{init:?}");
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(plane.join("charter.local.toml"), "\n").unwrap();
    std::fs::write(plane.join(".charter/kept"), "state\n").unwrap();
    std::fs::create_dir_all(home.join(".config/charter")).unwrap();
    std::fs::write(home.join(".config/charter/kept"), "config\n").unwrap();
    std::fs::create_dir_all(home.join("data/charter")).unwrap();
    Machine {
        _dir: dir,
        home,
        plane,
    }
}

/// `charter <args>` in the project, with the temporary home and nothing inherited.
fn charter(m: &Machine, args: &[&str]) -> Output {
    support::purlis()
        .args(args)
        .current_dir(&m.plane)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &m.home)
        .env("XDG_DATA_HOME", m.home.join("data"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("CHARTER_ROOT", &m.plane)
        .output()
        .expect("charter runs")
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn migrate_moves_the_local_names_and_undo_puts_them_back() {
    let m = machine();

    let out = charter(&m, &["migrate"]);

    assert!(out.status.success(), "{}", said(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("✓ the config home"), "{stdout}");
    assert!(stdout.contains("purlis migrate --undo"), "{stdout}");
    assert_eq!(read(&m.home.join(".config/purlis/kept")), "config\n");
    assert!(m.home.join("data/purlis").is_dir());
    assert_eq!(read(&m.plane.join(".purlis/kept")), "state\n");
    assert!(m.plane.join("purlis.local.toml").is_file());

    let out = charter(&m, &["migrate", "--undo"]);

    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(read(&m.home.join(".config/charter/kept")), "config\n");
    assert!(!m.home.join(".config/purlis").exists());
    assert!(m.home.join("data/charter").is_dir());
    assert_eq!(read(&m.plane.join(".charter/kept")), "state\n");
    assert!(m.plane.join("charter.local.toml").is_file());
    assert!(!m.plane.join(".purlis").exists());
}

#[test]
fn the_rename_local_fix_runs_and_runs_again_safely() {
    let m = machine();

    let out = charter(&m, &["doctor", "--fix", "rename-local"]);

    let first = String::from_utf8_lossy(&out.stderr);
    assert!(first.contains("fix rename-local:"), "{first}");
    assert!(first.contains("✓ the config home"), "{first}");
    assert_eq!(read(&m.plane.join(".purlis/kept")), "state\n");

    let again = charter(&m, &["doctor", "--fix", "rename-local"]);

    let second = String::from_utf8_lossy(&again.stderr);
    assert!(second.contains("nothing to move"), "{second}");
    assert!(!second.contains('✗'), "{second}");
    assert_eq!(read(&m.plane.join(".purlis/kept")), "state\n");
    assert_eq!(read(&m.home.join(".config/purlis/kept")), "config\n");
}

#[test]
fn a_bare_fix_never_moves_this_machines_folders() {
    let m = machine();

    let _ = charter(&m, &["doctor", "--fix"]);

    assert!(m.home.join(".config/charter").is_dir());
    assert!(m.plane.join(".charter").is_dir());
    assert!(!m.plane.join(".purlis").exists());
}

/// The project's stub keyring (a test build never reaches the operating system's store), as JSON.
fn stub_of(m: &Machine, state: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&read(&m.plane.join(state).join("keyring-stub.json"))).unwrap()
}

#[test]
fn migrate_copies_a_keyring_vaults_items_and_undo_reads_the_old_ones_again() {
    let m = machine();
    let out = charter(&m, &["vault", "add", "ops"]);
    assert!(out.status.success(), "{}", said(&out));
    let out = support::purlis()
        .args(["secret", "set", "ops", "API_TOKEN", "--stdin"])
        .current_dir(&m.plane)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &m.home)
        .env("CHARTER_ROOT", &m.plane)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"kr-cli-5e8a")?;
            child.wait_with_output()
        })
        .unwrap();
    assert!(out.status.success(), "{}", said(&out));
    // As a build from before the rename left it.
    for file in [
        ".charter/vaults/ops.keys.json",
        ".charter/keyring-stub.json",
    ] {
        let path = m.plane.join(file);
        std::fs::write(
            &path,
            read(&path).replace("\"purlis/ops/", "\"charter/ops/"),
        )
        .unwrap();
    }

    let out = charter(&m, &["migrate"]);

    assert!(out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("vault 'ops': 1 secret copied"),
        "{}",
        said(&out)
    );
    let stub = stub_of(&m, ".purlis");
    let names: Vec<&str> = stub.keys().map(|k| k.split('/').next().unwrap()).collect();
    assert!(
        names.contains(&"charter") && names.contains(&"purlis"),
        "{names:?}"
    );
    assert!(read(&m.plane.join(".purlis/vaults/ops.keys.json")).contains("\"purlis/ops/"));
    let get = charter(
        &m,
        &["secret", "get", "ops", "API_TOKEN", "--reveal", "--force"],
    );
    assert!(said(&get).contains("kr-cli-5e8a"), "{}", said(&get));

    let out = charter(&m, &["migrate", "--undo"]);

    assert!(out.status.success(), "{}", said(&out));
    assert!(read(&m.plane.join(".charter/vaults/ops.keys.json")).contains("\"charter/ops/"));
    assert_eq!(stub_of(&m, ".charter").len(), 2, "both items are kept");
    let get = charter(
        &m,
        &["secret", "get", "ops", "API_TOKEN", "--reveal", "--force"],
    );
    assert!(said(&get).contains("kr-cli-5e8a"), "{}", said(&get));
}

/// The harness plugin moved by the process (#1305): the core test drives the same step with the
/// plugin named; only a process finds the plugin beside its own binary.
#[cfg(unix)]
mod the_plugin {
    use super::*;

    /// A machine where `purlis plugin install` ran before the rename (as `charter@charter-app`, the
    /// opencode shim as `charter.ts`, the Codex guard running an older charter that is gone), and a
    /// `purlis` with the app's plugin beside it, as a bundle ships them. Nothing but the temporary
    /// home's harness folders is written.
    struct Shipped {
        _dir: tempfile::TempDir,
        _bin: tempfile::TempDir,
        home: PathBuf,
        /// The `purlis` beside the plugin: this build, linked where the plugin is.
        purlis: PathBuf,
    }

    /// The plugin the app ships, in this repository.
    fn the_apps_plugin() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/src-tauri/plugin")
            .canonicalize()
            .unwrap()
    }

    fn shipped_after_an_install_before_the_rename() -> Shipped {
        use purlis_core::plugin_install::{self as install, Machine as Harnesses, Verb};

        let dir = tempfile::tempdir().unwrap();
        let home = std::fs::canonicalize(dir.path()).unwrap();
        for folder in [".config/charter", "claude", "codex", ".config/opencode"] {
            std::fs::create_dir_all(home.join(folder)).unwrap();
        }
        let older = Harnesses {
            claude_config: home.join("claude"),
            codex_home: home.join("codex"),
            opencode_config: home.join(".config/opencode"),
            charter_dir: home.join(".config/charter"),
            binary: home.join("old/charter"),
            bundle: Some(the_apps_plugin()),
        };
        let out = install::run(&older, Verb::Install, &[], false);
        assert!(!install::failed(&out), "{}", install::render(&out, false));
        let out = install::move_ids(&older, &install::BEFORE);
        assert!(!install::failed(&out), "{}", install::render(&out, false));

        // This build where a bundle keeps it, the plugin beside it (`plugin_install::bundle_beside`).
        // A link, not a copy, where the file system allows one: the same file, under another name.
        let bin = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
        let purlis = std::fs::canonicalize(bin.path()).unwrap().join("purlis");
        if std::fs::hard_link(support::PURLIS, &purlis).is_err() {
            std::fs::copy(support::PURLIS, &purlis).unwrap();
        }
        std::os::unix::fs::symlink(the_apps_plugin(), purlis.with_file_name("plugin")).unwrap();

        let shipped = Shipped {
            _dir: dir,
            _bin: bin,
            home,
            purlis,
        };
        assert_eq!(shipped.enabled(), ["charter@charter-app"]);
        shipped
    }

    impl Shipped {
        /// `purlis migrate <args>` from the linked binary, in the home, with nothing inherited.
        fn migrate(&self, args: &[&str]) -> Output {
            support::command(&self.purlis)
                .arg("migrate")
                .args(args)
                .current_dir(&self.home)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", &self.home)
                // The harnesses' own variables, as `plugin_install.rs` hands them.
                .env("CLAUDE_CONFIG_DIR", self.home.join("claude"))
                .env("CODEX_HOME", self.home.join("codex"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                // This test build is fenced (charter-app#129), and a cleared environment names no
                // temporary directory, so the fence is named: the home, and nothing else.
                .env("CHARTER_PLANE_FENCE", &self.home)
                .output()
                .expect("purlis runs")
        }

        /// The plugin ids Claude Code's user settings turn on.
        fn enabled(&self) -> Vec<String> {
            let settings: serde_json::Value =
                serde_json::from_str(&read(&self.home.join("claude/settings.json"))).unwrap();
            let mut on: Vec<String> = settings["enabledPlugins"]
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, on)| **on == serde_json::Value::Bool(true))
                .map(|(id, _)| id.clone())
                .collect();
            on.sort();
            on
        }

        fn shim(&self, file: &str) -> PathBuf {
            self.home.join(".config/opencode/plugin").join(file)
        }
    }

    #[test]
    fn migrate_moves_the_harness_plugin_from_the_one_beside_it_and_undo_puts_it_back() {
        let m = shipped_after_an_install_before_the_rename();

        let out = m.migrate(&[]);

        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{}", said(&out));
        for harness in ["claude", "opencode", "codex"] {
            assert!(
                stdout.contains(&format!("✓ the {harness} plugin: ")),
                "{harness}: {stdout}"
            );
        }
        // Installed from the plugin beside this purlis, under the purlis names, and the old ones
        // gone, so the two never load together.
        assert_eq!(m.enabled(), ["purlis@purlis-app"]);
        let manifest: serde_json::Value = serde_json::from_str(&read(
            &m.home
                .join(".config/purlis/plugin/.claude-plugin/plugin.json"),
        ))
        .unwrap();
        assert_eq!(manifest["name"], "purlis");
        assert!(m.shim("purlis.ts").is_file());
        assert!(!m.shim("charter.ts").exists());
        // The guard runs the purlis that migrated, not the older charter.
        let codex = read(&m.home.join("codex/config.toml"));
        assert!(
            codex.contains(&format!("'{}' hook pretooluse", m.purlis.display())),
            "{codex}"
        );

        let out = m.migrate(&["--undo"]);

        assert!(out.status.success(), "{}", said(&out));
        assert_eq!(m.enabled(), ["charter@charter-app"]);
        assert!(m.home.join(".config/charter/plugin").is_dir());
        assert!(!m.home.join(".config/purlis").exists());
        assert!(m.shim("charter.ts").is_file());
        assert!(!m.shim("purlis.ts").exists());
    }
}

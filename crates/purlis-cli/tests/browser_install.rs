// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis browser install` is retired (#2575): a chat purlis starts drives a browser with
//! purlis's own browser tools, which the `browser` skill teaches. The words are still taken,
//! with whatever a script still passes, and answer one sentence naming what to use instead.
//!
//! A stand-in `npx` on `$PATH` records any run, so the tests hold that nothing is generated.

#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Output;

struct Scene {
    _dir: tempfile::TempDir,
    root: PathBuf,
    bin: PathBuf,
    log: PathBuf,
}

fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let root = base.join("plane");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let bin = base.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = base.join("npx.log");
    let npx = bin.join("npx");
    std::fs::write(
        &npx,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&npx, std::fs::Permissions::from_mode(0o755)).unwrap();
    Scene {
        _dir: dir,
        root,
        bin,
        log,
    }
}

fn purlis(s: &Scene, args: &[&str]) -> Output {
    support::purlis()
        .args(args)
        .current_dir(&s.root)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", s.bin.display()))
        .env("HOME", s.root.join("home"))
        .env("CHARTER_ROOT", &s.root)
        .env("NO_COLOR", "1")
        .output()
        .expect("the binary runs")
}

fn err(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn out(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

const RETIRED: &str = "✗ `purlis browser install` is retired, and nothing was written. A chat \
                       purlis starts drives a real browser with purlis's own browser tools \
                       (`browser_open`, `browser_snapshot`, `browser_click`, `browser_login` \
                       and the rest), which purlis's `browser` skill teaches.\n";

#[test]
fn install_is_retired_and_says_what_to_use_instead_whatever_it_is_given() {
    let s = scene();
    for args in [
        vec!["browser", "install"],
        vec!["browser", "install", "--version", "0.1.18"],
    ] {
        let said = purlis(&s, &args);
        assert_eq!(said.status.code(), Some(1), "{args:?}: {}", err(&said));
        assert_eq!(err(&said), RETIRED, "{args:?}");
        assert_eq!(out(&said), "", "{args:?}");
    }
    assert!(!s.log.exists(), "npx never ran");
    assert!(!s.root.join(".claude").exists(), "no skill is written");
    assert!(
        !s.root.join(".gitignore").exists(),
        "no ignore line is written"
    );
}

#[test]
fn the_retired_command_is_not_offered() {
    let s = scene();
    let help = out(&purlis(&s, &["--help"]));
    assert!(help.contains("Usage"), "{help}");
    assert!(!help.contains("browser"), "{help}");
}

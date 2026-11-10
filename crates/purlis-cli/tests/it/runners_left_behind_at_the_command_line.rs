// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis pool left-behind`** (#2587): the runners a removal left on another Docker engine,
//! each with the engine that holds it and the commands that remove it there. Removing them goes
//! through `purlis_core::runners::left`, whose tests drive a Docker.
#![cfg(unix)]

use crate::support;

use std::path::Path;
use std::process::{Output, Stdio};

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ID: &str = "bedaed78395a1217511b3b656d2e4d5d3b9f279c0c7270290596f86688c01fe4";

fn purlis(config: &Path, args: &[&str]) -> Output {
    support::purlis()
        .args(args)
        .current_dir(config)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", config)
        .env("PURLIS_CONFIG_HOME", config)
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Pool `builds`'s runner, kept as left on engine `engine-a`, as `docs/plane-format.md` spells
/// the record.
fn one_left_behind(config: &Path) {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let dir = purlis_core::machine::dir(config);
    std::fs::create_dir_all(&dir).unwrap();
    let text = serde_json::json!({
        "version": 1,
        "runners": [{
            "name": "builds",
            "pool": "builds",
            "container": ID,
            "image": IMAGE,
            "user": "10001:10001",
            "home": "/home/purlis",
            "engine": "engine-a",
            "since": 1_800_000_000_u64,
        }],
    });
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dir.join(purlis_core::runners::left::FILE))
        .unwrap()
        .write_all(text.to_string().as_bytes())
        .unwrap();
}

#[test]
fn a_runner_left_on_another_engine_is_listed_with_its_engine_and_the_commands_that_remove_it() {
    let config = tempfile::tempdir().unwrap();
    one_left_behind(config.path());

    let out = purlis(config.path(), &["pool", "left-behind"]);

    let said = said(&out);
    assert!(out.status.success(), "{said}");
    assert!(
        said.contains("Runner builds of pool builds is on Docker engine engine-a"),
        "{said}"
    );
    assert!(
        said.contains(&format!("docker container rm --force {ID}")),
        "{said}"
    );
    assert!(
        said.contains("docker volume rm --force purlis-runner-builds-home"),
        "{said}"
    );
}

#[test]
fn with_no_runner_left_behind_it_says_so() {
    let config = tempfile::tempdir().unwrap();

    let out = purlis(config.path(), &["pool", "left-behind"]);

    let said = said(&out);
    assert!(out.status.success(), "{said}");
    assert!(
        said.contains("No runner is left on another Docker engine."),
        "{said}"
    );
}

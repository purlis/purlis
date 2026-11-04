// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis pool add-profile`** (#4718, Q30): what Pool setup's Sign-ins button records, by
//! hand. It records and sends nothing, with the window's checks and words, and a chat can't run
//! it.
#![cfg(unix)]

use crate::support;

use std::path::Path;
use std::process::{Output, Stdio};

use purlis_core::hookwire::{CHAT_ENV, SOCKET_ENV};
use purlis_core::profilestore::{self, Homes, MachineSettings};
use purlis_core::runners;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A config home keeping the Docker pool `builds`, which names `work`, and the profiles `work`
/// and `home`.
fn a_machine() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let pool = runners::Pool::new(
        "builds",
        purlis_core::dockerpool::Image::pinned(IMAGE).unwrap(),
        runners::Set::default(),
    )
    .unwrap();
    runners::record_pool(dir.path(), &pool).unwrap();
    let store = Homes::at(dir.path(), &dir.path().join("data"));
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = |_: &str| -> Option<String> { None };
    for name in ["work", "home"] {
        profilestore::create(
            &store,
            name,
            "claude",
            &MachineSettings {
                home: &home,
                env: &env,
            },
            1,
        )
        .expect("made");
    }
    runners::add_pool_profile(dir.path(), "builds", "work", None).unwrap();
    dir
}

fn purlis(config: &Path, args: &[&str], env: &[(String, String)]) -> Output {
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
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
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

fn named(config: &Path) -> Option<Vec<String>> {
    runners::profiles_of(config, "builds").unwrap()
}

#[test]
fn a_profile_is_recorded_on_the_pool_and_pool_setup_is_pointed_to() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(config, &["pool", "add-profile", "builds", "home"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(named(config), Some(vec!["work".into(), "home".into()]));
    assert!(said(&out).contains("Pool setup"), "{}", said(&out));
}

#[test]
fn a_profile_this_machine_hasnt_got_is_refused_and_nothing_is_written() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(
        config,
        &["pool", "add-profile", "builds", "home", "other"],
        &[],
    );
    assert!(!out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("no profile named 'other'"),
        "{}",
        said(&out)
    );
    assert_eq!(named(config), Some(vec!["work".into()]));
}

#[test]
fn in_a_chat_nothing_is_recorded() {
    let machine = a_machine();
    let config = machine.path();
    let env = [
        (
            SOCKET_ENV.to_owned(),
            config.join("none.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "3".to_owned()),
    ];
    let out = purlis(config, &["pool", "add-profile", "builds", "home"], &env);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("own terminal"), "{}", said(&out));
    assert_eq!(named(config), Some(vec!["work".into()]));
}

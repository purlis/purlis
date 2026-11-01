// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis pool setup` (#4478): what Pool setup says of a pool, on the command line. Which state Docker
//! is in depends on the machine, and `purlis_core::poolsetup`'s tests and the parity job hold
//! each state; here it is the command's shape.

use crate::support;

use std::path::Path;

fn pool(config: &Path, args: &[&str]) -> std::process::Output {
    support::purlis()
        .args(["pool", "setup"])
        .args(args)
        .current_dir(config)
        .env("PURLIS_CONFIG_HOME", config.join("config"))
        .env("HOME", config)
        .output()
        .expect("the binary runs")
}

#[test]
fn a_new_pool_lists_the_five_docker_steps_in_order() {
    let config = tempfile::tempdir().unwrap();
    let out = pool(config.path(), &[]);
    assert!(out.status.success(), "{out:?}");
    let said = String::from_utf8_lossy(&out.stderr);
    let places: Vec<usize> = [
        "Where chats run",
        "Pool",
        "Sign-ins",
        "Agent identity",
        "Try it",
    ]
    .iter()
    .map(|name| {
        said.find(name)
            .unwrap_or_else(|| panic!("{name} in {said}"))
    })
    .collect();
    assert!(places.is_sorted(), "{said}");
    assert!(said.contains("\nNext: "), "{said}");
}

#[test]
fn its_json_has_each_step_with_its_state_and_next_action() {
    let config = tempfile::tempdir().unwrap();
    let out = pool(config.path(), &["--json"]);
    assert!(out.status.success(), "{out:?}");
    let setup: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON on stdout");
    assert_eq!(setup["kind"], "docker");
    assert_eq!(setup["pool"], serde_json::Value::Null);
    let steps = setup["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 5);
    assert_eq!(steps[0]["id"], "where");
    assert!(steps[0]["next"]["label"].is_string(), "{setup}");
    assert_eq!(steps[4]["state"], "later");
}

#[test]
fn a_pool_this_machine_has_not_is_refused_in_words() {
    let config = tempfile::tempdir().unwrap();
    let out = pool(config.path(), &["nowhere"]);
    assert!(!out.status.success(), "{out:?}");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("no pool named nowhere"), "{said}");
}

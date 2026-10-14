// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host links nothing of git, HTTP or the keyring, and not the core crate that holds
//! them (#1997): this crate's own dependency tree, as cargo resolves it. So the window's binary,
//! started again as its session host, opens no keyring item and asks nothing of the Keychain.

use std::collections::{BTreeSet, HashMap};

/// A crate is refused when its name is one of these, or starts with one followed by `-`.
const REFUSED: [&str; 16] = [
    // The core, which holds all three.
    "purlis-core",
    // Git.
    "gix",
    "git2",
    "libgit2-sys",
    // HTTP.
    "ureq",
    "reqwest",
    "hyper",
    "http",
    "h2",
    "curl",
    "rustls",
    "graphql_client",
    // The keyring.
    "keyring",
    "security-framework",
    "secret-service",
    "dbus-secret-service",
];

fn refused(name: &str) -> bool {
    REFUSED.iter().any(|refused| {
        name == *refused
            || name
                .strip_prefix(refused)
                .is_some_and(|rest| rest.starts_with('-'))
    })
}

/// What the host crate links, by name: every crate reachable from it through normal and
/// build dependencies, never through a dev-dependency, on this host's platform.
fn linked() -> BTreeSet<String> {
    // Only this host's platform: cargo has downloaded no package that only another platform
    // builds, and `--offline` would fail on one (main CI, 2026-10-04).
    let out = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .args(["--filter-platform", host_triple()])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "run `cargo fetch --locked` first: this guard reads cargo's resolution of the whole \
         workspace offline, and a build of only some of its packages has not downloaded every \
         crate the lock names.\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let name_of: HashMap<&str, &str> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| {
            (
                package["id"].as_str().unwrap(),
                package["name"].as_str().unwrap(),
            )
        })
        .collect();
    let nodes: HashMap<&str, &serde_json::Value> = metadata["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| (node["id"].as_str().unwrap(), node))
        .collect();
    let root = name_of
        .iter()
        .find(|(_, name)| **name == env!("CARGO_PKG_NAME"))
        .map(|(id, _)| *id)
        .expect("this crate is in the workspace");

    let mut seen = BTreeSet::new();
    let mut next = vec![root];
    while let Some(id) = next.pop() {
        for dep in nodes[id]["deps"].as_array().unwrap() {
            let linked = dep["dep_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|kind| kind["kind"].as_str() != Some("dev"));
            let dep_id = dep["pkg"].as_str().unwrap();
            if linked && seen.insert(dep_id) {
                next.push(dep_id);
            }
        }
    }
    seen.into_iter().map(|id| name_of[id].to_string()).collect()
}

#[test]
fn the_session_host_links_no_git_http_or_keyring_crate() {
    let linked = linked();
    assert!(
        linked.contains("purlis-session-protocol") && linked.contains("tokio"),
        "the walk found the host's own crates: {linked:?}"
    );
    let refused: Vec<&String> = linked.iter().filter(|name| refused(name)).collect();
    assert!(
        refused.is_empty(),
        "the host crate links {refused:?}, which the session host must not carry (#1997)"
    );
}

#[test]
fn a_crate_is_refused_by_its_name_or_a_name_it_prefixes() {
    assert!(refused("gix"));
    assert!(refused("gix-transport"));
    assert!(refused("purlis-core"));
    assert!(refused("security-framework-sys"));
    assert!(refused("http-body"));
    assert!(!refused("httparse"), "a longer word is not the crate");
    assert!(!refused("purlis-session-protocol"));
}

fn host_triple() -> &'static str {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("aarch64", "windows") => "aarch64-pc-windows-msvc",
        (arch, os) => panic!("no target triple known for {arch} on {os}"),
    }
}

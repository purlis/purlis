// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-053: the key proxy is a crate of its own, linked by the desktop's binary (`purlis-app`,
//! whose session host starts it) and the runner's (`purlis`, of `purlis-cli`): read from cargo's
//! resolution of the workspace, as a normal dependency of each, never only a dev-dependency.

use std::collections::HashMap;

/// The workspace packages that depend on this crate as a normal dependency, on this host.
fn dependents() -> Vec<String> {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .args(["--filter-platform", host_triple()])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "run `cargo fetch --locked` first: this check reads cargo's resolution of the whole \
         workspace offline.\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let name_of: HashMap<&str, &str> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    let mut dependents = Vec::new();
    for node in metadata["resolve"]["nodes"].as_array().unwrap() {
        let links = node["deps"].as_array().unwrap().iter().any(|dep| {
            name_of[dep["pkg"].as_str().unwrap()] == env!("CARGO_PKG_NAME")
                && dep["dep_kinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|kind| kind["kind"].is_null())
        });
        if links {
            dependents.push(name_of[node["id"].as_str().unwrap()].to_string());
        }
    }
    dependents.sort();
    dependents
}

#[test]
fn both_binaries_link_the_key_proxy_crate() {
    let dependents = dependents();
    for binary in ["purlis-app", "purlis-cli"] {
        assert!(
            dependents.iter().any(|d| d == binary),
            "{binary} does not link the key proxy; these do: {dependents:?}"
        );
    }
    assert!(
        !dependents.iter().any(|d| d == "purlis-host"),
        "the session host's crate links the key proxy, which it must start through its trait"
    );
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

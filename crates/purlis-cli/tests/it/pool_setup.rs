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

fn pool_add(config: &Path, args: &[&str]) -> std::process::Output {
    support::purlis()
        .args(["pool", "add"])
        .args(args)
        .current_dir(config)
        .env("PURLIS_CONFIG_HOME", config.join("config"))
        .env("HOME", config)
        .env("PATH", config)
        .output()
        .expect("the binary runs")
}

#[test]
fn adding_a_pool_needs_its_image_by_flag() {
    let config = tempfile::tempdir().unwrap();
    let out = pool_add(config.path(), &["dev-box"]);
    assert!(!out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("--image"));
}

/// With no Docker on the machine it says so; with one, this image isn't to be had. Either way
/// nothing is kept.
#[test]
fn a_pool_that_cant_be_made_keeps_nothing() {
    let config = tempfile::tempdir().unwrap();
    let image = format!("ghcr.io/purlis/purlis-runner@sha256:{}", "0".repeat(64));
    let out = pool_add(config.path(), &["dev-box", "--image", &image]);
    assert!(!out.status.success(), "{out:?}");
    let kept = purlis_core::runners::pools(&config.path().join("config")).unwrap_or_default();
    assert!(kept.is_empty(), "{kept:?}");
}

#[test]
fn adding_a_pool_in_a_chat_is_refused_in_words() {
    let config = tempfile::tempdir().unwrap();
    let image = format!("ghcr.io/purlis/purlis-runner@sha256:{}", "0".repeat(64));
    let out = support::purlis()
        .args(["pool", "add", "dev-box", "--image", &image])
        .current_dir(config.path())
        .env("PURLIS_CONFIG_HOME", config.path().join("config"))
        .env("HOME", config.path())
        .env(purlis_core::hookwire::CHAT_ENV, "1")
        .output()
        .expect("the binary runs");
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("a chat doesn't reach"),
        "{out:?}"
    );
}

#[test]
fn no_fetch_belongs_to_a_check() {
    let config = tempfile::tempdir().unwrap();
    let out = support::purlis()
        .args(["profile", "sign-in", "work", "--no-fetch"])
        .current_dir(config.path())
        .env("PURLIS_CONFIG_HOME", config.path().join("config"))
        .env("HOME", config.path())
        .output()
        .expect("the binary runs");
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("required arguments were not provided"),
        "{out:?}"
    );
}

fn doctor(config: &Path, args: &[&str]) -> std::process::Output {
    support::purlis()
        .args(["pool", "doctor"])
        .args(args)
        .current_dir(config)
        .env("PURLIS_CONFIG_HOME", config.join("config"))
        .env("HOME", config)
        .output()
        .expect("the binary runs")
}

/// A CA of its own in `dir`, as `<name>.pem`, made by the system's openssl.
fn a_ca(dir: &Path, name: &str) -> std::path::PathBuf {
    let run = |args: &[&str]| {
        let ran = std::process::Command::new("openssl")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("the system's openssl");
        assert!(ran.status.success(), "openssl {args:?}: {ran:?}");
    };
    run(&[
        "req",
        "-x509",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:prime256v1",
        "-nodes",
        "-keyout",
        &format!("{name}-key.pem"),
        "-subj",
        &format!("/CN={name}"),
        "-days",
        "1",
        "-out",
        &format!("{name}.pem"),
    ]);
    dir.join(format!("{name}.pem"))
}

/// **`purlis pool doctor` says a changed pin in the Notice's words and exits 1** (#6270, ADR 0123
/// §3): a pool over Docker's TLS API whose CA file now holds another CA than the one pinned. The
/// file is checked before anything runs, so no machine is contacted.
#[test]
fn doctor_says_a_pool_whose_ca_changed_and_exits_1() {
    let config = tempfile::tempdir().unwrap();
    let tls = config.path().join("tls");
    std::fs::create_dir_all(&tls).unwrap();
    let pinned = a_ca(&tls, "pinned");
    let ca = a_ca(&tls, "ca");
    let fingerprint = |path: &Path| {
        purlis_core::tlsroute::read_ca(path, "build-box")
            .unwrap()
            .fingerprint
    };
    let image = purlis_core::dockerpool::Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .unwrap();
    let remote = purlis_core::tlsroute::Remote {
        route: purlis_core::tlsroute::Route::parse("tcp://build-box:2376").unwrap(),
        files: purlis_core::tlsroute::Files::new(
            ca.clone(),
            tls.join("ca.pem"),
            purlis_core::tlsroute::KeyFrom::File(tls.join("ca-key.pem")),
        )
        .unwrap(),
        pin: purlis_core::tlsroute::Pin {
            ca: fingerprint(&pinned),
            engine: "ABCD:EFGH:IJKL".to_owned(),
        },
    };
    let pool =
        purlis_core::runners::Pool::new("builds", image, purlis_core::runners::Set::default())
            .unwrap()
            .on_engine(Some(remote.into()));
    purlis_core::runners::record_pool(&config.path().join("config"), &pool).unwrap();

    let out = doctor(config.path(), &["builds"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains("Pool builds: build-box's CA changed"),
        "{said}"
    );
    assert!(said.contains(&fingerprint(&pinned).shown()), "{said}");
    assert!(said.contains(&fingerprint(&ca).shown()), "{said}");
    assert!(said.contains("Check and trust again"), "{said}");
}

#[test]
fn doctor_of_a_pool_this_machine_has_not_is_refused_in_words() {
    let config = tempfile::tempdir().unwrap();
    let out = doctor(config.path(), &["nowhere"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("no pool named nowhere"), "{said}");
}

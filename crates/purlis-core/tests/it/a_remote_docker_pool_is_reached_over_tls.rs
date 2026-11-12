// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Docker on another machine, reached over Docker's TLS API, for real** (#6267, ADR 0123 §2–§4
//! and §7): against the remote parity job's fixture, `docker:dind` serving TLS on its port 2376
//! with the CA it generated. The machine is met by purlis's own handshake against that CA and
//! trusted with its engine's id; a pool's runs reach it over TLS; a CA file that no longer holds
//! the pinned CA refuses the run, and a CA that didn't sign the server's certificate is said to;
//! a client key kept in a vault entry reaches the engine and is left on no disk after its run.
//!
//! It needs the fixture: `REMOTE_DOCKER_TEST_TLS_ROUTE` (`tcp://localhost:2376`) and
//! `REMOTE_DOCKER_TEST_TLS_DIR`, holding dind's client `ca.pem`, `cert.pem` and `key.pem` and a
//! second CA, `other-ca.pem`, of `tests/remote-docker/up.sh`. Without them it runs nothing and
//! passes.

#![cfg(unix)]

use std::path::{Path, PathBuf};

use purlis_core::dockerpool::Docker;
use purlis_core::poolsetup::connecting;
use purlis_core::sshroute::connect::Finding;
use purlis_core::tlsroute::{self, Files, KeyFrom, Remote};

struct Fixture {
    route: String,
    dir: PathBuf,
    docker: Docker,
    settings: tempfile::TempDir,
}

fn fixture() -> Option<Fixture> {
    let route = std::env::var("REMOTE_DOCKER_TEST_TLS_ROUTE").ok()?;
    let dir = PathBuf::from(std::env::var_os("REMOTE_DOCKER_TEST_TLS_DIR")?);
    let settings = tempfile::tempdir().expect("a folder");
    Some(Fixture {
        route,
        docker: Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
            .with_settings_in(settings.path().join("s")),
        dir,
        settings,
    })
}

impl Fixture {
    fn files(&self, ca: &str) -> Files {
        Files::new(
            self.dir.join(ca),
            self.dir.join("cert.pem"),
            KeyFrom::File(self.dir.join("key.pem")),
        )
        .expect("the files")
    }

    fn trusted(&self, files: &Files) -> Remote {
        let met = connecting::meet_tls(&self.route, files).expect("met");
        connecting::trust_tls(&self.docker, &self.route, files, &met.fingerprint.spelled())
            .expect("trusted")
    }
}

/// Every file under `root`.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(at) = todo.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path.clone());
            }
            out.push(path);
        }
    }
    out
}

#[test]
fn a_machine_is_met_by_dinds_own_ca_and_trusted_with_its_engine() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let files = fixture.files("ca.pem");
    let met = connecting::meet_tls(&fixture.route, &files).expect("met");
    let ca = tlsroute::read_ca(&files.ca, "dind").expect("the CA");
    assert_eq!(met.fingerprint, ca.fingerprint);
    let remote = fixture.trusted(&files);
    assert_eq!(remote.pin.ca, ca.fingerprint);
    assert!(!remote.pin.engine.is_empty());
    // A pool's runs reach that engine over TLS, checked against the pin.
    let through =
        tlsroute::pool::docker_over(&fixture.docker, "met", &remote).expect("the pool's docker");
    assert_eq!(through.engine_id_said().expect("its id"), remote.pin.engine);
    assert!(
        walk(fixture.settings.path())
            .iter()
            .all(|path| !path.ends_with(tlsroute::KEY_PEM)),
        "no key is left after the runs"
    );
}

#[test]
fn a_ca_file_that_no_longer_matches_the_pinned_fingerprint_refuses_the_run() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = fixture.trusted(&fixture.files("ca.pem"));
    // The pool's CA file now holds a second CA, as a file swapped under it would.
    let swapped = Remote {
        files: fixture.files("other-ca.pem"),
        ..remote
    };
    match tlsroute::pool::docker_over(&fixture.docker, "swapped", &swapped) {
        Err(Finding::CaChanged { file, .. }) => {
            assert_eq!(file, fixture.dir.join("other-ca.pem").display().to_string());
        }
        Err(other) => panic!("refused otherwise: {other:?}"),
        Ok(_) => panic!("a run was given a CA other than the pinned one"),
    }
}

#[test]
fn a_ca_that_didnt_sign_dinds_certificate_is_said_naming_the_file() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    match connecting::meet_tls(&fixture.route, &fixture.files("other-ca.pem")) {
        Err(Finding::NotSignedByCa { ca, .. }) => {
            assert_eq!(ca, fixture.dir.join("other-ca.pem").display().to_string());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn plain_tcp_to_dind_is_refused_with_adr_0123s_words() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    assert_eq!(
        tlsroute::route_with(&fixture.route, None).unwrap_err(),
        purlis_core::sshroute::PLAIN_TCP
    );
}

#[test]
fn a_key_kept_in_a_vault_entry_reaches_the_engine_and_is_left_on_no_disk() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    // The vault is the test's own plane's; never a state folder of the person's.
    assert!(
        std::env::var_os("PURLIS_HOME").is_none() && std::env::var_os("CHARTER_HOME").is_none(),
        "this test registers a vault, so it runs only where no state folder is named"
    );
    let plane = tempfile::tempdir().expect("a plane");
    let ctx =
        purlis_core::secrets::Ctx::new(plane.path(), purlis_core::secrets::Env::from_process());
    purlis_core::secrets::registry::add_vault(
        &ctx,
        "dind",
        "plain-file",
        serde_json::json!({ "file": "dind.json" })
            .as_object()
            .cloned()
            .expect("a map"),
        None,
        false,
        false,
    )
    .expect("the vault");
    let key = std::fs::read_to_string(fixture.dir.join("key.pem")).expect("the key");
    std::fs::write(
        plane.path().join("dind.json"),
        serde_json::json!({ "TLS_KEY": key }).to_string(),
    )
    .expect("the vault's file");
    let files = Files::new(
        fixture.dir.join("ca.pem"),
        fixture.dir.join("cert.pem"),
        KeyFrom::Vault {
            plane: plane.path().to_path_buf(),
            vault: "dind".to_owned(),
            key: "TLS_KEY".to_owned(),
        },
    )
    .expect("the files");
    let remote = fixture.trusted(&files);
    let through =
        tlsroute::pool::docker_over(&fixture.docker, "vaulted", &remote).expect("its docker");
    through.engine_id_said().expect("the engine answers");
    assert!(
        walk(fixture.settings.path())
            .iter()
            .all(|path| !path.ends_with(tlsroute::KEY_PEM)),
        "a key from the vault was left on disk"
    );
    tlsroute::forget_vault_keys();
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Rootless Podman on another machine, reached over SSH, for real** (#6269, ADR 0123 §2 and
//! §3): against the remote parity job's Podman fixture (`tests/remote-podman/up.sh`), rootless
//! Podman's API service behind a dockerised sshd that shares its socket and its store at the
//! same paths, with simulated latency and loss. Docker's client finds a Podman there, and the
//! pin is read with Podman's own: the socket and the store's graph root. A runner is made there,
//! checked as the measured words of rootless Podman hold it (its runtime config read on that
//! machine over the pinned SSH), and removed. Another graph root under the same host key is a
//! pin change.
//!
//! It needs the fixture: `REMOTE_PODMAN_TEST_ROUTE` (`ssh://pool@127.0.0.1:2223`), the runner
//! image in `REMOTE_PODMAN_TEST_IMAGE`, this machine's `docker` and `podman`, and the agent the
//! fixture's key is in. Without them it runs nothing and passes. Its job runs it through
//! `tests/remote-podman/without-its-init.sh`, so that machine's Podman's init
//! (`REMOTE_PODMAN_TEST_INIT`) is not on this one and is judged there.

#![cfg(unix)]

use std::path::PathBuf;

use purlis_core::dockerpool::{Docker, Image, Runner};
use purlis_core::runners::Remote;
use purlis_core::sshroute::connect::{self, Finding};
use purlis_core::sshroute::{self, EnginePin, Pin, Route, forward, pool};

struct Fixture {
    route: Route,
    image: String,
    ssh: PathBuf,
    docker: Docker,
    folders: tempfile::TempDir,
}

impl Fixture {
    fn engine_links(&self) -> PathBuf {
        self.folders.path().join("l")
    }

    fn settings_in(&self) -> PathBuf {
        self.folders.path().join("s")
    }
}

fn fixture() -> Option<Fixture> {
    let route = std::env::var("REMOTE_PODMAN_TEST_ROUTE").ok()?;
    let image = std::env::var("REMOTE_PODMAN_TEST_IMAGE").ok()?;
    // A short folder: a socket's path is held to 103 bytes.
    let folders = tempfile::Builder::new()
        .prefix("rp")
        .tempdir_in("/tmp")
        .expect("a folder");
    Some(Fixture {
        route: Route::parse(&route).expect("a route"),
        image,
        ssh: sshroute::ssh_here().expect("the system's ssh"),
        docker: Docker::at("docker").with_settings_in(folders.path().join("s")),
        folders,
    })
}

/// The machine met, its socket found and its engine read through a forward: Trust this machine.
fn trusted(fixture: &Fixture, pool: &str) -> Remote {
    let key = connect::meet(&fixture.ssh, &fixture.route, &fixture.settings_in()).expect("met");
    let found = connect::probe(
        &fixture.ssh,
        &fixture.route,
        pool,
        &key,
        &fixture.settings_in(),
        "pool",
    )
    .expect("its socket is found");
    let opening = forward::Opening {
        ssh: &fixture.ssh,
        route: &found,
        pool,
        key: &key,
        engine_links: &fixture.engine_links(),
        settings_in: &fixture.settings_in(),
    };
    let engine = connect::engine_pin(&fixture.docker, &opening).expect("its engine answers");
    Remote {
        route: found,
        pin: Pin {
            host_key: key,
            engine,
        },
    }
}

fn through(fixture: &Fixture, pool: &str, remote: &Remote) -> Result<Docker, Finding> {
    pool::docker_through(
        &fixture.docker,
        &fixture.ssh,
        pool,
        remote,
        &fixture.engine_links(),
        &fixture.settings_in(),
    )
}

/// What this machine's own rootless Podman says its graph root is: the store the fixture's sshd
/// shares at the same path.
fn graph_root_here() -> String {
    let out = purlis_core::forklock::output(std::process::Command::new("podman").args([
        "info",
        "--format",
        "{{.Store.GraphRoot}}",
    ]))
    .expect("podman runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn a_podman_found_by_its_rootless_socket_is_pinned_by_that_socket_and_its_graph_root() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "pmet");
    let socket = remote.route.socket().expect("a socket found").to_owned();
    assert!(
        socket.ends_with("/podman/podman.sock") && socket.starts_with("/run/user/"),
        "rootless Podman's socket, found by the probe: {socket}"
    );
    assert_eq!(
        remote.pin.engine,
        EnginePin::Podman {
            socket,
            graph_root: graph_root_here(),
        }
    );
    let docker = through(&fixture, "pmet", &remote).expect("the forward opens");
    assert!(
        docker.program().ends_with("podman"),
        "a Podman pool's runs use Podman's own client: {}",
        docker.program().display()
    );
    assert!(docker.answers().is_ok(), "the engine answers through it");
    forward::let_go("pmet");
}

#[test]
fn a_runner_made_on_rootless_podman_there_is_checked_as_rootless_podman_writes_it() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "pmake");
    let docker = through(&fixture, "pmake", &remote).expect("the forward opens");
    let image = Image::pinned(&fixture.image).expect("an image by digest");
    let (user, home) = docker.declared(&image).expect("its image is read there");
    let runner = Runner::new("pmake-runner", image, user, &home).expect("a runner");
    // The other machine's init is that machine's alone (`without-its-init.sh`): judged there.
    if let Ok(init) = std::env::var("REMOTE_PODMAN_TEST_INIT") {
        assert!(
            !std::path::Path::new(&init).exists(),
            "the other machine's init {init} is not seen here"
        );
    }
    let made = docker
        .make(&runner)
        .expect("made, and checked as it was made");
    // The check before every link, its runtime config read on that machine.
    let checked = docker.check(&made);
    let removed = docker.remove(&made);
    forward::let_go("pmake");
    checked.expect("a runner made the pool's way passes the check");
    removed.expect("removed with its home");
}

#[test]
fn another_graph_root_under_the_same_host_key_is_a_pin_change() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let mut remote = trusted(&fixture, "pstore");
    forward::let_go("pstore");
    let EnginePin::Podman { socket, .. } = remote.pin.engine.clone() else {
        panic!("a Podman's pin: {:?}", remote.pin.engine)
    };
    remote.pin.engine = EnginePin::Podman {
        socket,
        graph_root: "/srv/another/storage".to_owned(),
    };
    match through(&fixture, "pstore", &remote) {
        Err(Finding::EngineChanged {
            podman: true,
            pinned,
            now,
            ..
        }) => {
            assert!(pinned.contains("/srv/another/storage"), "{pinned}");
            assert!(now.contains(&graph_root_here()), "{now}");
        }
        other => panic!("another graph root is refused as a pin change, not {other:?}"),
    }
    assert!(
        !fixture
            .engine_links()
            .join("pstore")
            .join("docker.sock")
            .exists(),
        "no forward stands to an engine that isn't the pinned one"
    );
}

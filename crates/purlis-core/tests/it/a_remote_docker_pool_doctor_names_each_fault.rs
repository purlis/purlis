// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Doctor names each fault, for real** (#6272, ADR 0123 §7): against the remote parity job's
//! fixture (`tests/remote-docker/up.sh`: a dockerised sshd sharing `docker:dind`'s socket, with
//! latency and loss, and dind's TLS port), each fault of the record's table is put in place and
//! `poolsetup::doctoring::check`, which Check again and `purlis pool doctor` both run, finds it as
//! its own finding, said in Pool setup's words:
//!
//! wrong name, port closed, no agent key, no socket, no docker group, plain TCP, bad TLS,
//! unsupported architecture, full disk, image unreachable, context drift and engine ID changed.
//! A pool with none of them passes, with its link's round trip measured.
//!
//! The architecture's and the disk's faults are images `up.sh` builds there: one built for
//! `linux/riscv64` alone, from scratch, and one of the parity image whose `df` says its disk is
//! nearly full. Without the fixture it runs nothing and passes. The job runs it one test at a
//! time, as it runs the SSH route's.

#![cfg(unix)]

use std::path::PathBuf;

use purlis_core::dockerpool::{Docker, Image};
use purlis_core::poolsetup::{connecting, doctoring};
use purlis_core::runners::{Pool, Remote, Set};
use purlis_core::sshroute::connect::{self, Finding};
use purlis_core::sshroute::{self, EnginePin, HostKey, Pin, Route, forward};

struct Fixture {
    route: Route,
    sshd: String,
    image: String,
    ssh: PathBuf,
    program: PathBuf,
    docker: Docker,
    folders: tempfile::TempDir,
}

fn fixture() -> Option<Fixture> {
    let route = std::env::var("REMOTE_DOCKER_TEST_ROUTE").ok()?;
    let sshd = std::env::var("REMOTE_DOCKER_TEST_SSHD").ok()?;
    let image = std::env::var("DOCKER_POOL_TEST_IMAGE").ok()?;
    let program =
        PathBuf::from(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()));
    // A short folder: a socket's path is held to 103 bytes.
    let folders = tempfile::Builder::new()
        .prefix("rf")
        .tempdir_in("/tmp")
        .expect("a folder");
    Some(Fixture {
        route: Route::parse(&route).expect("a route"),
        sshd,
        image,
        ssh: sshroute::ssh_here().expect("the system's ssh"),
        docker: Docker::at(&program).with_settings_in(folders.path().join("s")),
        program,
        folders,
    })
}

impl Fixture {
    fn settings_in(&self) -> PathBuf {
        self.folders.path().join("s")
    }

    fn engine_links(&self) -> PathBuf {
        self.folders.path().join(sshroute::DIR)
    }

    /// The machine met, its socket found and its engine read: Trust this machine.
    fn trusted(&self, pool: &str) -> Remote {
        let key = connect::meet(&self.ssh, &self.route, &self.settings_in()).expect("met");
        let found = connect::probe(
            &self.ssh,
            &self.route,
            pool,
            &key,
            &self.settings_in(),
            "pool",
        )
        .expect("its socket is found");
        let opening = forward::Opening {
            ssh: &self.ssh,
            route: &found,
            pool,
            key: &key,
            engine_links: &self.engine_links(),
            settings_in: &self.settings_in(),
        };
        let engine = connect::engine_pin(&self.docker, &opening).expect("its engine answers");
        Remote {
            route: found,
            pin: Pin {
                host_key: key,
                engine,
            },
        }
    }

    /// The sshd's host key, as its file holds it.
    fn sshd_key(&self) -> HostKey {
        let out = purlis_core::forklock::output(std::process::Command::new("docker").args([
            "exec",
            &self.sshd,
            "cat",
            "/etc/ssh/ssh_host_ed25519_key.pub",
        ]))
        .expect("docker runs");
        HostKey::parse(String::from_utf8_lossy(&out.stdout).trim()).expect("the sshd's key")
    }

    /// Pool `name` of `image`, reached at `route` and pinned to `pin`.
    fn pool(&self, name: &str, route: &str, pin: Pin, image: &str) -> Pool {
        Pool::new(
            name,
            Image::pinned(image).expect("an image"),
            Set::default(),
        )
        .expect("a pool")
        .on(Some(Remote {
            route: Route::parse(route).expect("a route"),
            pin,
        }))
    }

    /// The pin a pool trusted on the fixture's machine: its key, and the engine there.
    fn pin(&self, name: &str) -> Pin {
        let remote = self.trusted(name);
        forward::let_go(name);
        remote.pin
    }

    /// What doctor finds of `pool`, the person's Docker contexts being `contexts`.
    fn doctor(
        &self,
        pool: &Pool,
        contexts: Vec<purlis_core::dockercontext::DockerContext>,
    ) -> Result<doctoring::Healthy, Finding> {
        sshroute::pool::look_again();
        let found = doctoring::check_with(&self.docker, pool, &|| contexts.clone());
        forward::let_go(pool.name());
        found
    }
}

/// `finding` is said in Pool setup's words, as `purlis pool doctor` prints them for pool `pool`.
fn said_as_pool_setup_says(pool: &Pool, finding: &Finding) {
    let machine = doctoring::machine_of(pool).expect("a pool on another machine");
    let (said, code) = doctoring::said_for(pool.name(), &machine, &Err(finding.clone()));
    assert_eq!(code, 1, "{said}");
    // A changed pin is said in its Notice's words, any other finding in the step's.
    let words = purlis_core::poolsetup::pinchange::of(pool.name(), finding)
        .map_or_else(|| connecting::said(finding), |change| change.said);
    assert!(said.contains(&words), "{said}\nwithout {words:?}");
    assert!(said.contains("Next: "), "{said}");
    assert!(said.contains("See what this does: "), "{said}");
}

fn expect(pool: &Pool, found: Result<doctoring::Healthy, Finding>, cause: &str) -> Finding {
    let finding = found.expect_err("doctor finds the fault");
    assert_eq!(connecting::cause(&finding), cause, "{finding:?}");
    said_as_pool_setup_says(pool, &finding);
    finding
}

#[test]
fn a_pool_with_no_fault_passes_with_its_links_round_trip_measured() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = fixture.trusted("healthy");
    forward::let_go("healthy");
    let pool = fixture.pool(
        "healthy",
        &remote.route.spelled(),
        remote.pin,
        &fixture.image,
    );
    let healthy = fixture
        .doctor(&pool, Vec::new())
        .expect("every check passes");
    assert_eq!(healthy.host, fixture.route.host());
    assert!(healthy.round_trip > std::time::Duration::ZERO);
    let link = doctoring::link_of(&fixture.docker, &pool).expect("a link to measure");
    forward::let_go("healthy");
    assert_eq!(
        link.verdict,
        purlis_core::poolsetup::Verdict::Ok,
        "{link:?}"
    );
    assert!(link.title.starts_with("A round trip to "), "{link:?}");
}

#[test]
fn wrong_name() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let pin = fixture.pin("named");
    let pool = fixture.pool(
        "named",
        "ssh://pool@no-such-machine.invalid/var/run/docker.sock",
        pin,
        &fixture.image,
    );
    expect(&pool, fixture.doctor(&pool, Vec::new()), "no-such-name");
}

#[test]
fn port_closed() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let pin = fixture.pin("closed");
    let pool = fixture.pool(
        "closed",
        "ssh://pool@127.0.0.1:2223/var/run/docker.sock",
        pin,
        &fixture.image,
    );
    let finding = expect(&pool, fixture.doctor(&pool, Vec::new()), "no-answer");
    assert!(
        matches!(finding, Finding::NoAnswer { port: 2223, .. }),
        "{finding:?}"
    );
}

/// The sshd's `nokey` has no key the agent holds among those it accepts: none at all.
#[test]
fn no_agent_key() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let pin = Pin {
        host_key: fixture.sshd_key(),
        engine: fixture.pin("unkeyed").engine,
    };
    let pool = fixture.pool(
        "unkeyed",
        "ssh://nokey@127.0.0.1:2222/var/run/docker.sock",
        pin,
        &fixture.image,
    );
    expect(&pool, fixture.doctor(&pool, Vec::new()), "sign-in-refused");
}

#[test]
fn no_socket() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let pin = fixture.pin("socketless");
    let pool = fixture.pool(
        "socketless",
        "ssh://pool@127.0.0.1:2222/run/no-engine-here.sock",
        pin,
        &fixture.image,
    );
    expect(&pool, fixture.doctor(&pool, Vec::new()), "no-engine");
}

/// The sshd's `outsider` signs in, and isn't in the group that may use Docker's socket there.
#[test]
fn no_docker_group() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let pin = fixture.pin("outside");
    let pool = fixture.pool(
        "outside",
        "ssh://outsider@127.0.0.1:2222/var/run/docker.sock",
        pin,
        &fixture.image,
    );
    expect(&pool, fixture.doctor(&pool, Vec::new()), "not-allowed");
}

/// Plain `tcp://` is refused before anything is contacted, and the Connect step says why.
#[test]
fn plain_tcp() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let config = fixture.folders.path().join("config");
    let chosen = purlis_core::poolsetup::Chosen {
        route: "tcp://localhost:2375".to_owned(),
        identity: None,
        trusted: None,
        tls: None,
        trusted_tls: None,
        docker_context: None,
    };
    let pressed = connecting::meet_chosen(&config, &fixture.docker, &chosen);
    let purlis_core::poolsetup::Pressed::Found(finding) = pressed else {
        panic!("plain TCP is met: {pressed:?}");
    };
    assert_eq!(connecting::said(&finding), sshroute::PLAIN_TCP);
}

/// A pool over TLS whose CA file holds a CA (pinned) that signed nothing of the engine's.
#[test]
fn bad_tls() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let (Ok(route), Some(dir)) = (
        std::env::var("REMOTE_DOCKER_TEST_TLS_ROUTE"),
        std::env::var_os("REMOTE_DOCKER_TEST_TLS_DIR").map(PathBuf::from),
    ) else {
        return;
    };
    use purlis_core::tlsroute::{Files, KeyFrom};
    let files = Files::new(
        dir.join("other-ca.pem"),
        dir.join("cert.pem"),
        KeyFrom::File(dir.join("key.pem")),
    )
    .expect("the files");
    let ca = purlis_core::tlsroute::read_ca(&files.ca, "localhost").expect("the other CA");
    let remote = purlis_core::tlsroute::Remote {
        route: purlis_core::tlsroute::Route::parse(&route).expect("a route"),
        files,
        pin: purlis_core::tlsroute::Pin {
            ca: ca.fingerprint,
            engine: "NOT:THE:ENGINE".to_owned(),
        },
    };
    let pool = Pool::new(
        "badtls",
        Image::pinned(&fixture.image).expect("an image"),
        Set::default(),
    )
    .expect("a pool")
    .on_engine(Some(remote.into()));
    expect(&pool, fixture.doctor(&pool, Vec::new()), "tls-not-signed");
}

#[test]
fn unsupported_architecture() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let Ok(riscv) = std::env::var("REMOTE_DOCKER_TEST_RISCV_IMAGE") else {
        return;
    };
    let remote = fixture.trusted("riscv");
    forward::let_go("riscv");
    let pool = fixture.pool("riscv", &remote.route.spelled(), remote.pin, &riscv);
    let finding = expect(
        &pool,
        fixture.doctor(&pool, Vec::new()),
        "unsupported-architecture",
    );
    let Finding::Architecture {
        platform,
        built_for,
        ..
    } = finding
    else {
        unreachable!()
    };
    assert_eq!(built_for, vec!["linux/riscv64".to_owned()]);
    assert_ne!(platform, "linux/riscv64");
}

#[test]
fn full_disk() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let Ok(full) = std::env::var("REMOTE_DOCKER_TEST_FULL_DISK_IMAGE") else {
        return;
    };
    let remote = fixture.trusted("fulldisk");
    forward::let_go("fulldisk");
    let pool = fixture.pool("fulldisk", &remote.route.spelled(), remote.pin, &full);
    let finding = expect(&pool, fixture.doctor(&pool, Vec::new()), "disk-full");
    assert!(
        matches!(finding, Finding::DiskFull { free: Some(free), .. } if free < 1_000_000_000),
        "{finding:?}"
    );
}

#[test]
fn image_unreachable() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = fixture.trusted("unreached");
    forward::let_go("unreached");
    let image = format!("registry.invalid/purlis-runner@sha256:{}", "0".repeat(64));
    let pool = fixture.pool("unreached", &remote.route.spelled(), remote.pin, &image);
    let finding = expect(
        &pool,
        fixture.doctor(&pool, Vec::new()),
        "image-unreachable",
    );
    assert!(
        matches!(&finding, Finding::ImageUnreachable { registry, .. } if registry == "registry.invalid"),
        "{finding:?}"
    );
}

/// A pool read from a Docker context, the context then pointed at another machine.
#[test]
fn context_drift() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let person = tempfile::tempdir().expect("the person's Docker settings");
    let context = |args: &[&str]| {
        let out = purlis_core::forklock::output(
            std::process::Command::new(&fixture.program)
                .arg("--config")
                .arg(person.path())
                .arg("context")
                .args(args),
        )
        .expect("docker runs");
        assert!(out.status.success(), "docker context {args:?}: {out:?}");
    };
    let route = fixture.route.spelled();
    context(&["create", "builder", "--docker", &format!("host={route}")]);
    let remote = fixture.trusted("drifted");
    forward::let_go("drifted");
    let pool = fixture
        .pool(
            "drifted",
            &remote.route.spelled(),
            remote.pin,
            &fixture.image,
        )
        .from_docker_context(Some("builder".to_owned()));
    let read = || purlis_core::dockercontext::contexts_with(&fixture.program, Some(person.path()));
    fixture
        .doctor(&pool, read())
        .expect("the context names the pool's machine");
    context(&[
        "update",
        "builder",
        "--docker",
        "host=ssh://pool@127.0.0.1:2224",
    ]);
    let finding = expect(&pool, fixture.doctor(&pool, read()), "context-moved");
    assert_eq!(
        finding,
        Finding::ContextMoved {
            name: "builder".to_owned()
        }
    );
}

#[test]
fn engine_id_changed() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let mut remote = fixture.trusted("rebuilt");
    forward::let_go("rebuilt");
    remote.pin.engine = EnginePin::Docker("NOT:THE:ENGINE".to_owned());
    let pool = fixture.pool(
        "rebuilt",
        &remote.route.spelled(),
        remote.pin,
        &fixture.image,
    );
    expect(
        &pool,
        fixture.doctor(&pool, Vec::new()),
        "pool-engine-changed",
    );
}

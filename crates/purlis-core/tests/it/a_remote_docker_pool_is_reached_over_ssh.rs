// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Docker on another machine, reached over SSH, for real** (#6266, ADR 0123 §2, §3 and §8):
//! against the remote parity job's fixture, a dockerised sshd sharing `docker:dind`'s socket, with
//! simulated latency and loss. The machine is met with nothing signed in and its key read from
//! what ssh wrote; the engine's socket is found and its id read through a forward; a host key
//! other than the pinned one refuses the forward, also when the person's own `known_hosts` holds
//! it; a changed engine is refused before anything runs; a killed connection is opened again.
//!
//! It needs the fixture: `REMOTE_DOCKER_TEST_ROUTE` (`ssh://pool@127.0.0.1:2222`), the sshd's
//! container in `REMOTE_DOCKER_TEST_SSHD` and dind's in `REMOTE_DOCKER_TEST_DIND`, reached with
//! this machine's `docker`. Without them it runs nothing and passes. The tests change the sshd's
//! host key and connections, so the job runs them one at a time.

#![cfg(unix)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use purlis_core::dockerpool::Docker;
use purlis_core::runners::Remote;
use purlis_core::sshroute::connect::{self, Finding};
use purlis_core::sshroute::{self, HostKey, Pin, Route, forward, pool};

struct Fixture {
    route: Route,
    sshd: String,
    dind: String,
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
    let route = std::env::var("REMOTE_DOCKER_TEST_ROUTE").ok()?;
    let sshd = std::env::var("REMOTE_DOCKER_TEST_SSHD").ok()?;
    let dind = std::env::var("REMOTE_DOCKER_TEST_DIND").ok()?;
    // A short folder: a socket's path is held to 103 bytes.
    let folders = tempfile::Builder::new()
        .prefix("rd")
        .tempdir_in("/tmp")
        .expect("a folder");
    Some(Fixture {
        route: Route::parse(&route).expect("a route"),
        sshd,
        dind,
        ssh: sshroute::ssh_here().expect("the system's ssh"),
        docker: Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
            .with_settings_in(folders.path().join("s")),
        folders,
    })
}

/// What `docker exec <container> sh -c <script>` printed, on this machine's own Docker.
fn exec_in(container: &str, script: &str) -> String {
    let out = purlis_core::forklock::output(
        std::process::Command::new("docker").args(["exec", container, "sh", "-c", script]),
    )
    .expect("docker runs");
    assert!(out.status.success(), "{script}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// The sshd's host key, as its file holds it.
fn sshd_key(fixture: &Fixture) -> HostKey {
    HostKey::parse(&exec_in(
        &fixture.sshd,
        "cat /etc/ssh/ssh_host_ed25519_key.pub",
    ))
    .expect("the sshd's key")
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
    let engine = connect::engine_id(&fixture.docker, &opening).expect("its engine answers");
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

#[test]
fn a_machine_is_met_by_the_key_its_sshd_holds_and_trusted_with_its_engine() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "met");
    assert_eq!(remote.pin.host_key, sshd_key(&fixture));
    assert_eq!(remote.route.socket(), Some("/var/run/docker.sock"));
    assert_eq!(
        remote.pin.engine,
        exec_in(&fixture.dind, "docker info --format '{{.ID}}'")
    );
    let docker = through(&fixture, "met", &remote).expect("the forward opens");
    assert!(docker.answers().is_ok(), "the engine answers through it");
    forward::let_go("met");
}

/// Puts the sshd's original key back, however the test ends.
struct KeyBack<'a>(&'a Fixture);

impl Drop for KeyBack<'_> {
    fn drop(&mut self) {
        let _ = purlis_core::forklock::output(std::process::Command::new("docker").args([
            "exec",
            &self.0.sshd,
            "sh",
            "-c",
            "cp /root/kept/ssh_host_ed25519_key* /etc/ssh/ && kill -HUP 1",
        ]));
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn a_host_key_other_than_the_pinned_one_is_refused_even_from_the_persons_known_hosts() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "rotated");
    forward::let_go("rotated");
    let _back = KeyBack(&fixture);
    exec_in(
        &fixture.sshd,
        "mkdir -p /root/kept && cp -n /etc/ssh/ssh_host_ed25519_key* /root/kept/ && \
         rm /etc/ssh/ssh_host_ed25519_key* && ssh-keygen -q -t ed25519 -N '' \
         -f /etc/ssh/ssh_host_ed25519_key && kill -HUP 1",
    );
    std::thread::sleep(Duration::from_secs(1));
    let now = sshd_key(&fixture);
    assert_ne!(now, remote.pin.host_key, "the key rotated");
    // The person's own ssh trusts the new key: purlis does not.
    if std::env::var_os("REMOTE_DOCKER_TEST_MAY_WRITE_KNOWN_HOSTS").is_some() {
        let home = PathBuf::from(std::env::var_os("HOME").expect("a home"));
        let known = home.join(".ssh/known_hosts");
        std::fs::create_dir_all(known.parent().expect("a folder")).expect("~/.ssh");
        let port = fixture.route.port().unwrap_or(22);
        let line = format!("[{}]:{port} {}\n", fixture.route.host(), now.spelled());
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&known)
            .expect("known_hosts");
        std::io::Write::write_all(&mut file, line.as_bytes()).expect("written");
    }
    match through(&fixture, "rotated", &remote) {
        Err(Finding::KeyChanged {
            pinned, now: shown, ..
        }) => {
            assert_eq!(pinned, remote.pin.host_key);
            assert_eq!(shown, now);
        }
        other => panic!("a changed key is refused, not {other:?}"),
    }
    assert!(
        !fixture
            .engine_links()
            .join("rotated")
            .join("docker.sock")
            .exists(),
        "no forward stands"
    );
}

#[test]
fn an_engine_other_than_the_pinned_one_is_refused_before_anything_runs() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let mut remote = trusted(&fixture, "engine");
    forward::let_go("engine");
    remote.pin.engine = "NOT:THE:ENGINE".to_owned();
    assert!(matches!(
        through(&fixture, "engine", &remote),
        Err(Finding::EngineChanged { .. })
    ));
}

#[test]
fn a_killed_connection_is_noticed_and_opened_again() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "dropped");
    through(&fixture, "dropped", &remote).expect("the forward opens");
    // Every session of the pool's user ends there, as a lost connection ends it.
    exec_in(&fixture.sshd, "pkill -u pool || true");
    let until = Instant::now() + Duration::from_secs(90);
    while !forward::dropped("dropped") {
        assert!(Instant::now() < until, "the dropped forward is noticed");
        std::thread::sleep(Duration::from_millis(200));
    }
    let docker = through(&fixture, "dropped", &remote).expect("opened again");
    assert!(docker.answers().is_ok(), "the engine answers again");
    forward::let_go("dropped");
}

#[test]
fn an_account_that_may_not_use_docker_there_is_told_so() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let Ok(outsider) = std::env::var("REMOTE_DOCKER_TEST_OUTSIDER_ROUTE") else {
        return;
    };
    let route = Route::parse(&outsider).expect("a route");
    let key = connect::meet(&fixture.ssh, &route, &fixture.settings_in()).expect("met");
    assert!(matches!(
        connect::probe(
            &fixture.ssh,
            &route,
            "outsider",
            &key,
            &fixture.settings_in(),
            "outsider"
        ),
        Err(Finding::NotAllowed { .. })
    ));
}

#[test]
fn a_machine_that_isnt_there_is_said_so_before_ssh_is_asked() {
    purlis_core::unsteered!();
    if fixture().is_none() {
        return;
    }
    let route = Route::parse("ssh://pool@127.0.0.1:1").expect("a route");
    let ssh = sshroute::ssh_here().expect("ssh");
    let resolved = connect::resolve(&ssh, &route).expect("ssh -G answers");
    assert!(matches!(
        connect::reachable(&route, &resolved),
        Err(Finding::NoAnswer { port: 1, .. })
    ));
}

/// Puts the person's own SSH settings back, however the test ends.
struct SettingsBack(PathBuf, Option<Vec<u8>>);

impl Drop for SettingsBack {
    fn drop(&mut self) {
        match &self.1 {
            Some(was) => {
                let _ = std::fs::write(&self.0, was);
            }
            None => {
                let _ = std::fs::remove_file(&self.0);
            }
        }
    }
}

/// The person's `~/.ssh/config` with `block` before what it held, put back as the test ends.
fn with_ssh_settings(block: &str) -> Option<SettingsBack> {
    std::env::var_os("REMOTE_DOCKER_TEST_MAY_WRITE_KNOWN_HOSTS")?;
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let config = home.join(".ssh/config");
    std::fs::create_dir_all(config.parent()?).ok()?;
    let was = std::fs::read(&config).ok();
    let mut now = block.as_bytes().to_vec();
    now.extend_from_slice(was.as_deref().unwrap_or_default());
    std::fs::write(&config, now).ok()?;
    Some(SettingsBack(config, was))
}

#[test]
fn nothing_of_the_persons_ssh_settings_rides_on_the_forward() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let ran = fixture.folders.path().join("local-command-ran");
    let block = format!(
        "Host commanding\n  HostName {host}\n  Port {port}\n  User pool\n  \
         RemoteCommand touch /tmp/remote-command-ran\n  PermitLocalCommand yes\n  \
         LocalCommand touch {ran}\n  RequestTTY force\n\n\
         Host carrying\n  HostName {host}\n  Port {port}\n  User pool\n  \
         RemoteForward 127.0.0.1:2299 127.0.0.1:22\n  RemoteCommand touch /tmp/remote-command-ran\n\n",
        host = fixture.route.host(),
        port = fixture.route.port().unwrap_or(22),
        ran = ran.display(),
    );
    let Some(_back) = with_ssh_settings(&block) else {
        return;
    };
    let _ = exec_in(&fixture.sshd, "rm -f /tmp/remote-command-ran; true");
    // A command, a local command and a terminal the settings ask for are not run: the forward
    // opens and its engine answers.
    let commanding = Fixture {
        route: Route::parse("ssh://commanding").expect("a route"),
        ..fixture
    };
    let remote = trusted(&commanding, "commanding");
    let docker = through(&commanding, "commanding", &remote).expect("the forward opens");
    assert!(docker.answers().is_ok(), "the engine answers through it");
    forward::let_go("commanding");
    assert!(!ran.exists(), "the settings' LocalCommand ran");
    assert_eq!(
        exec_in(
            &commanding.sshd,
            "test -e /tmp/remote-command-ran && echo ran || echo not"
        ),
        "not",
        "the settings' RemoteCommand ran"
    );
    // A forward the settings add is refused, named, and opens nothing.
    let carrying = Fixture {
        route: Route::parse("ssh://carrying").expect("a route"),
        ..commanding
    };
    let remote = Remote {
        route: Route::parse(&format!(
            "ssh://carrying{}",
            remote.route.socket().expect("a socket")
        ))
        .expect("a route"),
        pin: remote.pin,
    };
    match through(&carrying, "carrying", &remote) {
        Err(Finding::Here(said)) => assert!(said.contains("RemoteForward"), "{said}"),
        other => panic!("a forward the settings add is refused, not {other:?}"),
    }
    assert!(
        !carrying
            .engine_links()
            .join("carrying")
            .join("docker.sock")
            .exists(),
        "no forward stands"
    );
    assert_eq!(
        exec_in(
            &carrying.sshd,
            "(command -v ss >/dev/null && ss -ltn | grep -c ':2299 ') || echo 0"
        ),
        "0",
        "the settings' RemoteForward listens there"
    );
}

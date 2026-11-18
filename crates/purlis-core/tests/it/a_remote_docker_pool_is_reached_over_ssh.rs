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
//! this machine's `docker`. Without them it runs nothing and passes, except the step-timing test under CI (`CI` set),
//! where a missing fixture fails. The tests change the sshd's
//! host key and connections, so the job runs them one at a time.

#![cfg(unix)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use purlis_core::dockerpool::Docker;
use purlis_core::runners::Remote;
use purlis_core::sshroute::connect::{self, Finding};
use purlis_core::sshroute::{self, EnginePin, HostKey, Pin, Route, forward, pool};

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
    let key = connect::meet(
        &fixture.ssh,
        &fixture.route,
        &connect::resolve(&fixture.ssh, &fixture.route).expect("resolved"),
        &fixture.settings_in(),
    )
    .expect("met");
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

#[test]
fn a_machine_is_met_by_the_key_its_sshd_holds_and_trusted_with_its_engine() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "met");
    assert_eq!(remote.pin.host_key, sshd_key(&fixture));
    assert_eq!(remote.route.socket(), Some("/var/run/docker.sock"));
    assert_eq!(
        remote.pin.engine,
        EnginePin::Docker(exec_in(&fixture.dind, "docker info --format '{{.ID}}'"))
    );
    let docker = through(&fixture, "met", &remote).expect("the forward opens");
    assert!(docker.answers().is_ok(), "the engine answers through it");
    forward::let_go("met");
}

/// macOS keeps purlis's data under `~/Library/Application Support`, and ssh reads
/// `UserKnownHostsFile` as a list of files split at spaces: the key met was written to a stray
/// `~/Library/Application` and the run's own file stayed empty.
#[test]
fn a_machine_is_met_and_signed_in_to_from_a_settings_folder_whose_path_has_a_space() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let settings_in = fixture.folders.path().join("Application Support").join("s");
    std::fs::create_dir_all(&settings_in).expect("the settings folder");
    let key = connect::meet(
        &fixture.ssh,
        &fixture.route,
        &connect::resolve(&fixture.ssh, &fixture.route).expect("resolved"),
        &settings_in,
    )
    .expect("met");
    assert_eq!(key, sshd_key(&fixture));
    assert!(
        !fixture.folders.path().join("Application").exists(),
        "ssh wrote the key it met beside the settings folder"
    );
    let found = connect::probe(
        &fixture.ssh,
        &fixture.route,
        "spaced",
        &key,
        &settings_in,
        "pool",
    )
    .expect("signed in against the pinned key");
    assert_eq!(found.socket(), Some("/var/run/docker.sock"));
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
    remote.pin.engine = EnginePin::Docker("NOT:THE:ENGINE".to_owned());
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
    let key = connect::meet(
        &fixture.ssh,
        &route,
        &connect::resolve(&fixture.ssh, &route).expect("resolved"),
        &fixture.settings_in(),
    )
    .expect("met");
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

/// What the holder below names: the ssh holding its forward, and the `docker exec` it keeps open.
const HOLDER_MARKER: &str = "REMOTE_DOCKER_TEST_HOLDER_MARKER";

/// **A process holding a pool's forward with a `docker exec` open over it**, run by
/// [`the_forward_ends_with_the_process_holding_it_even_with_a_docker_exec_open`] as a process of
/// its own. Without its marker it runs nothing.
#[test]
fn holder_of_a_forward_with_an_exec_open() {
    purlis_core::unsteered!();
    let Ok(marker) = std::env::var(HOLDER_MARKER) else {
        return;
    };
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "held");
    let docker = through(&fixture, "held", &remote).expect("the forward opens");
    let socket = docker.forwarded().expect("a forward").to_path_buf();
    let ssh = forward::ssh_of("held").expect("this process holds the forward");
    // A channel of the connection's own, open for as long as `sleep` runs there.
    // Left running as this process is killed: the test ends it.
    #[allow(clippy::zombie_processes)]
    let exec = purlis_core::forklock::spawn(
        std::process::Command::new("docker")
            .env("DOCKER_HOST", format!("unix://{}", socket.display()))
            .env_remove("DOCKER_CONTEXT")
            .args(["exec", "registry", "sleep", "297"])
            .stdin(std::process::Stdio::null()),
    )
    .expect("docker exec runs");
    let until = Instant::now() + Duration::from_secs(60);
    while !exec_in(&fixture.dind, "docker top registry").contains("sleep 297") {
        assert!(Instant::now() < until, "the exec started there");
        std::thread::sleep(Duration::from_millis(200));
    }
    std::fs::write(&marker, format!("{ssh} {}\n", exec.id())).expect("the marker");
    // Held until the test kills this process outright.
    std::thread::sleep(Duration::from_secs(300));
}

#[test]
fn the_forward_ends_with_the_process_holding_it_even_with_a_docker_exec_open() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let marker = fixture.folders.path().join("held");
    let mut holder = purlis_core::forklock::spawn(
        std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "--exact",
                "a_remote_docker_pool_is_reached_over_ssh::holder_of_a_forward_with_an_exec_open",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(HOLDER_MARKER, &marker),
    )
    .expect("the holder runs");
    let ssh = stand_in::pid_named_in(&marker, Duration::from_secs(120))
        .expect("the holder named its ssh");
    let exec: u32 = std::fs::read_to_string(&marker)
        .expect("the marker")
        .split_whitespace()
        .nth(1)
        .and_then(|pid| pid.parse().ok())
        .expect("the holder named its exec");
    // However it goes: killed outright, with nothing run as it goes.
    holder.kill().expect("the holder is killed");
    holder.wait().expect("the holder ended");
    let gone = stand_in::is_gone(ssh, Duration::from_secs(30));
    // The exec the holder left behind, ended here whatever was found.
    let _ = purlis_core::forklock::status(
        std::process::Command::new("kill").args(["-9", &exec.to_string()]),
    );
    assert!(gone, "ssh {ssh} outlived the process holding its forward");
}

#[test]
fn every_forward_this_process_holds_ends_as_it_lets_go_of_all() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "ending");
    let docker = through(&fixture, "ending", &remote).expect("the forward opens");
    let socket = docker.forwarded().expect("a forward").to_path_buf();
    let ssh = forward::ssh_of("ending").expect("this process holds it");
    forward::let_go_of_all();
    assert!(stand_in::is_gone(ssh, Duration::from_secs(10)), "ssh ended");
    assert!(!socket.exists(), "its socket went with it");
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
            "if command -v ss >/dev/null; then ss -ltn | grep -c ':2299 ' || true; else echo 0; fi"
        ),
        "0",
        "the settings' RemoteForward listens there"
    );
}

/// Puts the sshd's own settings back, however the test ends.
struct SshdSettingsBack<'a>(&'a Fixture);

impl Drop for SshdSettingsBack<'_> {
    fn drop(&mut self) {
        let _ = purlis_core::forklock::output(std::process::Command::new("docker").args([
            "exec",
            &self.0.sshd,
            "sh",
            "-c",
            "[ -f /root/kept/sshd_config ] && cp /root/kept/sshd_config /etc/ssh/sshd_config && \
             kill -HUP 1",
        ]));
        std::thread::sleep(Duration::from_secs(1));
        pool::look_again();
    }
}

/// The sshd made to read `setting` first (its first value of a keyword is the one it takes),
/// before the rest of its own settings.
fn sshd_reads_first(fixture: &Fixture, setting: &str) {
    exec_in(
        &fixture.sshd,
        &format!(
            "mkdir -p /root/kept && cp -n /etc/ssh/sshd_config /root/kept/ && \
             {{ echo '{setting}'; cat /root/kept/sshd_config; }} > /etc/ssh/sshd_config && \
             sshd -t && kill -HUP 1"
        ),
    );
    std::thread::sleep(Duration::from_secs(1));
}

/// Connect's Trust this machine against the fixture: met, signed in, its socket found, and its
/// engine read through a forward, as [`trusted`] does, answering what that found.
fn trust_found(fixture: &Fixture, pool: &str) -> Result<EnginePin, Finding> {
    let key = connect::meet(
        &fixture.ssh,
        &fixture.route,
        &connect::resolve(&fixture.ssh, &fixture.route)?,
        &fixture.settings_in(),
    )?;
    let found = connect::probe(
        &fixture.ssh,
        &fixture.route,
        pool,
        &key,
        &fixture.settings_in(),
        "pool",
    )?;
    connect::engine_pin(
        &fixture.docker,
        &forward::Opening {
            ssh: &fixture.ssh,
            route: &found,
            pool,
            key: &key,
            engine_links: &fixture.engine_links(),
            settings_in: &fixture.settings_in(),
        },
    )
}

#[test]
fn an_sshd_that_refuses_forwarding_is_named_at_connect_and_for_a_pool_already_there() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "unforwarded");
    forward::let_go("unforwarded");
    let _back = SshdSettingsBack(&fixture);
    let refused = Finding::ForwardingRefused {
        host: "127.0.0.1".to_owned(),
    };
    // `AllowTcpForwarding no` too: sshd turns off its local forwards, the socket's among them.
    for setting in [
        "DisableForwarding yes",
        "AllowStreamLocalForwarding no",
        "AllowTcpForwarding no",
    ] {
        sshd_reads_first(&fixture, setting);
        // Connect: the sign-in and the socket hold, and the forward is named, not Docker.
        assert_eq!(
            trust_found(&fixture, "connecting"),
            Err(refused.clone()),
            "{setting}"
        );
        // The pool already there, as `purlis pool setup <pool>` and the window read it.
        pool::look_again();
        assert_eq!(
            through(&fixture, "unforwarded", &remote).err(),
            Some(refused.clone()),
            "{setting}"
        );
        // Its next ask, within its wait, is answered with what was found, and opens nothing.
        assert_eq!(
            through(&fixture, "unforwarded", &remote).err(),
            Some(refused.clone()),
            "{setting}"
        );
        assert!(forward::ssh_of("unforwarded").is_none(), "{setting}");
        assert!(
            purlis_core::poolsetup::connect_said(&refused).contains("doesn't allow forwarding")
        );
        pool::look_again();
    }
}

/// Pool `name`, kept under `config` as on the fixture's machine, pinned to `remote`.
fn kept_pool(config: &std::path::Path, name: &str, remote: &Remote) -> purlis_core::runners::Pool {
    let image = purlis_core::dockerpool::Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .expect("an image");
    let pool = purlis_core::runners::Pool::new(name, image, purlis_core::runners::Set::default())
        .expect("a pool")
        .on(Some(remote.clone()));
    purlis_core::runners::record_pool(config, &pool).expect("kept");
    pool
}

/// The pool `name` under `config`, as it is kept now.
fn pool_now(config: &std::path::Path, name: &str) -> purlis_core::runners::Pool {
    purlis_core::runners::pools(config)
        .expect("the pools")
        .into_iter()
        .find(|pool| pool.name() == name)
        .expect("the pool")
}

/// **A machine whose host key changed is trusted again by the key the person confirmed, and the
/// pool works again** (#6270, ADR 0123 §3): the rotated key refuses every run and is found for
/// the window; Check and trust again shows the key the machine shows now; a key other than that
/// is refused with nothing kept; the one shown is kept as the pool's pin, its route as it was.
#[test]
fn a_pool_whose_host_key_changed_is_trusted_again_by_the_key_confirmed_and_works() {
    purlis_core::unsteered!();
    use purlis_core::poolsetup::{Pressed, connecting, pinchange};
    let Some(fixture) = fixture() else { return };
    let remote = trusted(&fixture, "again");
    forward::let_go("again");
    let config = fixture.folders.path().join("c");
    let pool = kept_pool(&config, "again", &remote);
    let _back = KeyBack(&fixture);
    exec_in(
        &fixture.sshd,
        "mkdir -p /root/kept && cp -n /etc/ssh/ssh_host_ed25519_key* /root/kept/ && \
         rm /etc/ssh/ssh_host_ed25519_key* && ssh-keygen -q -t ed25519 -N '' \
         -f /etc/ssh/ssh_host_ed25519_key && kill -HUP 1",
    );
    std::thread::sleep(Duration::from_secs(1));
    let now = sshd_key(&fixture);
    assert!(matches!(
        pool::docker_of(&fixture.docker, &pool),
        Err(Finding::KeyChanged { .. })
    ));
    let found = pinchange::found_now();
    let change = found
        .iter()
        .find(|one| one.pool == "again")
        .expect("found for the window");
    assert_eq!(change.what, pinchange::Changed::Key);
    assert_eq!(change.now, now.fingerprint());

    let Pressed::Met(met) =
        connecting::meet_pool(&config, &fixture.docker, "again").expect("met again")
    else {
        panic!("the key it shows now is met");
    };
    assert_eq!(met.key, now);
    let unconfirmed = connecting::trust_pool(
        &config,
        &fixture.docker,
        "again",
        &remote.pin.host_key.fingerprint(),
    )
    .expect("asked");
    assert!(unconfirmed.is_err(), "a key it doesn't show is not kept");
    assert_eq!(pool_now(&config, "again"), pool, "nothing was kept");

    connecting::trust_pool(&config, &fixture.docker, "again", &now.fingerprint())
        .expect("asked")
        .expect("trusted again");
    let again = pool_now(&config, "again");
    let kept = again.remote().expect("still on the machine");
    assert_eq!(kept.pin.host_key, now);
    assert_eq!(kept.route, remote.route, "its route is as it was");
    assert!(
        pinchange::found_now().iter().all(|one| one.pool != "again"),
        "the change is gone once trusted again"
    );
    let docker = pool::docker_of(&fixture.docker, &again).expect("it works again");
    assert!(docker.answers().is_ok());
    forward::let_go("again");
}

/// **An engine that changed under the same key is trusted again with its new id** (#6270): the
/// key confirmed is the one trusted, and the engine there now is kept.
#[test]
fn a_pool_whose_engine_changed_is_trusted_again_with_the_engine_there_now() {
    purlis_core::unsteered!();
    use purlis_core::poolsetup::{Pressed, connecting, pinchange};
    let Some(fixture) = fixture() else { return };
    let mut remote = trusted(&fixture, "reset");
    forward::let_go("reset");
    let engine = remote.pin.engine.clone();
    // The engine as it was before Docker there was reset.
    remote.pin.engine = EnginePin::Docker("NOT:THE:ENGINE".to_owned());
    let config = fixture.folders.path().join("c");
    let pool = kept_pool(&config, "reset", &remote);
    assert!(matches!(
        pool::docker_of(&fixture.docker, &pool),
        Err(Finding::EngineChanged { .. })
    ));
    assert!(
        pinchange::found_now()
            .iter()
            .any(|one| one.pool == "reset" && one.what == pinchange::Changed::Engine)
    );
    let Pressed::Met(met) =
        connecting::meet_pool(&config, &fixture.docker, "reset").expect("met again")
    else {
        panic!("the key it shows is met");
    };
    assert_eq!(met.key, remote.pin.host_key, "the key is the one trusted");
    connecting::trust_pool(&config, &fixture.docker, "reset", &met.key.fingerprint())
        .expect("asked")
        .expect("trusted again");
    let again = pool_now(&config, "reset");
    assert_eq!(again.remote().expect("there").pin.engine, engine);
    pool::docker_of(&fixture.docker, &again).expect("it works again");
    forward::let_go("reset");
}

// ---- How long each step of a start on this pool takes (#7182) ----------------------------------
//
// The spans of `purlis_session_protocol::steps`, read back: a first start after the app is
// launched (the forward not held), a second start right after (forward held, engine checked),
// and one after the runner's container stopped. Each is a link made as a chat's start makes it,
// through the pool's forward. The fixture's sshd answers 40 ms ± 10 ms late (`up.sh`, netem), and
// the table is printed and, on CI, written to the job's summary: it is the baseline of
// `docs/research/2026-11-remote-start-baseline.md`.

/// The fixture, or nothing where this machine has none. **Under CI (`CI` set) a missing fixture
/// fails the test**: a parity job that ran nothing and passed would say nothing was checked.
fn fixture_or_fail_in_ci(needs: &[&str]) -> Option<Fixture> {
    let missing: Vec<&str> = needs
        .iter()
        .copied()
        .filter(|name| std::env::var_os(name).is_none())
        .collect();
    if !missing.is_empty() {
        assert!(
            std::env::var_os("CI").is_none(),
            "the remote parity fixture is missing under CI: {} not set",
            missing.join(", ")
        );
        return None;
    }
    fixture()
}

/// One step line: its name, the step it is inside (if any), its outcome and milliseconds.
struct StepLine {
    name: String,
    parent: Option<String>,
    refused: bool,
    ms: u64,
}

/// Every step line this process wrote, in the order each step ended.
#[derive(Clone, Default)]
struct StepLines(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for StepLines {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for StepLines {
    type Writer = StepLines;
    fn make_writer(&'a self) -> StepLines {
        self.clone()
    }
}

impl StepLines {
    fn steps(&self) -> Vec<StepLine> {
        let text = String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned();
        text.lines()
            .filter_map(|line| {
                let rest = line.split("purlis: step ").nth(1)?;
                let (name, rest) = rest.split_once(' ')?;
                let (outcome, rest) = rest.split_once(" after ")?;
                let ms = rest.split_whitespace().next()?.parse().ok()?;
                let parent = rest.split("parent: ").nth(1).map(|p| {
                    p.trim_end_matches(')')
                        .split([',', ')'])
                        .next()
                        .unwrap_or("")
                        .to_owned()
                });
                Some(StepLine {
                    name: name.to_owned(),
                    parent,
                    refused: outcome == "refused",
                    ms,
                })
            })
            .collect()
    }
}

/// The median of five TCP connects to the sshd: one round trip, as the fixture delays it.
fn round_trip_ms(route: &str) -> u64 {
    let at = route.rsplit('@').next().expect("an address");
    let mut times: Vec<u64> = (0..5)
        .map(|_| {
            let began = Instant::now();
            drop(std::net::TcpStream::connect(at).expect("the sshd answers"));
            u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX)
        })
        .collect();
    times.sort_unstable();
    times[2]
}

/// One link to the runner through `docker`, made as a start makes it (the check before, the
/// preparing, `docker exec`, the handshake), and dropped.
async fn linked_once(
    docker: &Docker,
    made: &purlis_core::dockerpool::Made,
    ours: purlis_session_protocol::hostlink::Identity,
    runner: purlis_session_protocol::hostlink::Pinned,
) {
    use purlis_session_protocol::hostlink::{self, Handshake, Linked, Liveness};
    let connector = purlis_host::CommandConnector::new(docker.program(), made.connect_args(), {
        let (docker, made) = (docker.clone(), made.clone());
        std::sync::Arc::new(move || {
            // What the pool's own link runs before each connection (`runnerlinks`): the engine's
            // check is `through`'s, this is the container's.
            docker
                .resumed(&made)
                .map(|_| ())
                .map_err(|why| why.to_string())
        })
    })
    .preparing({
        let docker = docker.clone();
        std::sync::Arc::new(move || {
            let (env, cleared, held) = docker
                .prepared()
                .map_err(|why| why.to_string())?
                .into_parts();
            Ok(purlis_host::Prepared { env, cleared, held })
        })
    });
    let mut links = hostlink::keep_linked(
        connector,
        Handshake::THIS_BUILD,
        ours,
        runner,
        Liveness::default(),
    );
    match tokio::time::timeout(Duration::from_secs(90), links.next()).await {
        Ok(Some(Linked::Up(_link))) => {}
        other => panic!("no link: {:?}", other.map(|linked| linked.is_some())),
    }
}

#[test]
fn a_start_on_a_remote_pool_times_each_of_its_steps() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture_or_fail_in_ci(&[
        "REMOTE_DOCKER_TEST_ROUTE",
        "REMOTE_DOCKER_TEST_SSHD",
        "REMOTE_DOCKER_TEST_DIND",
        "DOCKER_POOL_TEST_IMAGE",
    ]) else {
        return;
    };
    let image = std::env::var("DOCKER_POOL_TEST_IMAGE").expect("checked above");
    use purlis_core::dockerpool::{Image, Runner};
    use purlis_session_protocol::hostlink::{Identity, LinkKey, Pinned};

    let lines = StepLines::default();
    // Scoped to this test, never the process's global default (the binary is shared with every
    // other test): this thread takes it, and so does each thread the runtime starts.
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(lines.clone())
            .with_ansi(false)
            // `debug`: each `docker` call is a line at that level, the rows of the table's calls.
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    );
    thread_local! {
        static HELD: std::cell::RefCell<Option<tracing::dispatcher::DefaultGuard>> =
            const { std::cell::RefCell::new(None) };
    }
    let _scoped = tracing::dispatcher::set_default(&dispatch);
    let rtt = round_trip_ms(&std::env::var("REMOTE_DOCKER_TEST_ROUTE").unwrap());

    let pool = "timed";
    let remote = trusted(&fixture, pool);
    forward::let_go_of_all();
    let docker = through(&fixture, pool, &remote).expect("the forward opens");
    let image = Image::pinned(&image).expect("the image is named by digest");
    let (user, home) = docker
        .declared(&image)
        .expect("the image declares its user");
    let name = format!(
        "timed-{}",
        ulid::Ulid::generate().to_string().to_lowercase()
    );
    let runner = Runner::new(&name, image, user, &home).unwrap();
    let made = docker.make(&runner).expect("the runner is made there");
    struct Removes(Docker, purlis_core::dockerpool::Made);
    impl Drop for Removes {
        fn drop(&mut self) {
            let _ = self.0.remove(&self.1);
        }
    }
    let _removes = Removes(docker.clone(), made.clone());
    let key = LinkKey::generate().unwrap();
    let ours = || Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::from_private(key.private_half().unwrap()).unwrap(),
    };
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: key.public(),
    };
    let runner_pinned = docker.pair(&made, &desktop).expect("the runner is paired");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .on_thread_start({
            let dispatch = dispatch.clone();
            move || {
                let guard = tracing::dispatcher::set_default(&dispatch);
                HELD.with(|held| *held.borrow_mut() = Some(guard));
            }
        })
        .enable_all()
        .build()
        .unwrap();
    let mut table = Vec::new();
    let mut scenario = |label: &str, then: &mut dyn FnMut()| {
        let before = lines.steps().len();
        let began = Instant::now();
        then();
        let total = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
        table.push((label.to_owned(), total, lines.steps().split_off(before)));
    };

    // The first start after the app is launched: no forward held.
    forward::let_go_of_all();
    scenario("cold: first start after launch", &mut || {
        let docker = through(&fixture, pool, &remote).expect("the forward opens");
        runtime.block_on(linked_once(&docker, &made, ours(), runner_pinned.clone()));
    });
    // A start straight after: the forward held, the engine checked a moment ago.
    scenario("warm: a link", &mut || {
        let docker = through(&fixture, pool, &remote).expect("the engine answers");
        runtime.block_on(linked_once(&docker, &made, ours(), runner_pinned.clone()));
    });
    // A chat with a profile makes a second link of its own after the first (audit B3).
    scenario("warm: the second link of a start", &mut || {
        let docker = through(&fixture, pool, &remote).expect("the engine answers");
        runtime.block_on(linked_once(&docker, &made, ours(), runner_pinned.clone()));
    });
    exec_in(
        &fixture.dind,
        &format!("docker stop {}", made.container.as_str()),
    );
    scenario("runner stopped: its container is started", &mut || {
        let docker = through(&fixture, pool, &remote).expect("the engine answers");
        runtime.block_on(linked_once(&docker, &made, ours(), runner_pinned.clone()));
    });

    let mut out = format!(
        "### Start on a remote Docker pool over SSH (round trip {rtt} ms, TCP connect, median of 5)\n\n"
    );
    for (label, total, steps) in &table {
        out += &format!(
            "**{label}**: {total} ms in all (a step with a parent, or a call, is inside another: leave it out of a sum)\n\n| step | inside | runs | ms (each) |\n|---|---|---|---|\n"
        );
        let mut names: Vec<(&str, Option<&str>)> = Vec::new();
        for line in steps {
            let key = (line.name.as_str(), line.parent.as_deref());
            if !names.contains(&key) {
                names.push(key);
            }
        }
        for (name, parent) in names {
            let each: Vec<String> = steps
                .iter()
                .filter(|line| line.name == name && line.parent.as_deref() == parent)
                .map(|line| {
                    format!(
                        "{}{}",
                        line.ms,
                        if line.refused { " (refused)" } else { "" }
                    )
                })
                .collect();
            out += &format!(
                "| {name} | {} | {} | {} |\n",
                parent.unwrap_or(if name.starts_with("docker_") {
                    "(a call)"
                } else {
                    ""
                }),
                each.len(),
                each.join(", ")
            );
        }
        out += "\n";
    }
    let _ = std::io::Write::write_all(&mut std::io::stderr(), out.as_bytes());
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY")
        && let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(summary)
    {
        let _ = std::io::Write::write_all(&mut file, out.as_bytes());
    }

    // Every step of the link path was timed, in the scenario it belongs to.
    let said = |at: usize, step: &str| table[at].2.iter().any(|line| line.name == step);
    for step in ["ssh_forward_open", "engine_pin_check", "docker_info"] {
        assert!(said(0, step), "the cold start timed no {step}: {out}");
    }
    for (at, (label, ..)) in table.iter().enumerate() {
        for step in [
            "container_check",
            "docker_container",
            "docker_volume",
            "docker_exec",
            "link_check",
            "link_prepare",
            "link_exec_spawn",
            "link_handshake",
        ] {
            assert!(said(at, step), "{label} timed no {step}: {out}");
        }
    }
    assert!(
        said(3, "runner_start"),
        "a stopped runner's start was not timed: {out}"
    );
    assert!(
        !said(1, "ssh_forward_open"),
        "a warm start reopened the forward: {out}"
    );
}

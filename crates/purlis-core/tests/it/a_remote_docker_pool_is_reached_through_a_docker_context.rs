// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Docker on another machine, from a Docker context, for real** (#6268, ADR 0123 §2 and §7):
//! against the remote parity job's fixture, contexts made with `docker context create` pointing
//! at #6266's sshd and at #6267's TLS port. Each is read once into the route its endpoint names,
//! with the TLS files Docker keeps for it, and a pool's runs reach the engine there by that route.
//! A context changed afterwards changes nothing a pool runs on, and doctor's check says it moved.
//! A context on a socket here is this machine.
//!
//! It needs the fixture: `REMOTE_DOCKER_TEST_ROUTE` (`ssh://pool@127.0.0.1:2222`),
//! `REMOTE_DOCKER_TEST_TLS_ROUTE` (`tcp://localhost:2376`) and `REMOTE_DOCKER_TEST_TLS_DIR` of
//! `tests/remote-docker/up.sh`. Without them it runs nothing and passes.

#![cfg(unix)]

use std::path::{Path, PathBuf};

use purlis_core::dockercontext::{self, DockerContext, Endpoint};
use purlis_core::dockerpool::Docker;
use purlis_core::poolsetup::connecting;
use purlis_core::sshroute::{self, connect, forward};

struct Fixture {
    ssh_route: String,
    tls_route: String,
    tls_dir: PathBuf,
    program: PathBuf,
    docker: Docker,
    folders: tempfile::TempDir,
    /// The person's own Docker settings: beside the fixture's TLS files, never under `/tmp`,
    /// which chats write, so purlis refuses a CA kept there.
    person: tempfile::TempDir,
}

fn fixture() -> Option<Fixture> {
    let ssh_route = std::env::var("REMOTE_DOCKER_TEST_ROUTE").ok()?;
    let tls_route = std::env::var("REMOTE_DOCKER_TEST_TLS_ROUTE").ok()?;
    let tls_dir = PathBuf::from(std::env::var_os("REMOTE_DOCKER_TEST_TLS_DIR")?);
    let program =
        PathBuf::from(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()));
    // A short folder: a socket's path is held to 103 bytes.
    let folders = tempfile::Builder::new()
        .prefix("rc")
        .tempdir_in("/tmp")
        .expect("a folder");
    let person = tempfile::Builder::new()
        .prefix("person")
        .tempdir_in(tls_dir.parent().expect("the TLS files' folder"))
        .expect("a folder");
    Some(Fixture {
        person,
        ssh_route,
        tls_route,
        tls_dir,
        docker: Docker::at(&program).with_settings_in(folders.path().join("s")),
        program,
        folders,
    })
}

impl Fixture {
    /// The person's own Docker settings, where their contexts are kept: the test's own.
    fn person(&self) -> PathBuf {
        self.person.path().to_path_buf()
    }

    /// `docker context <args>`, in the person's settings.
    fn context(&self, args: &[&str]) {
        let out = purlis_core::forklock::output(
            std::process::Command::new(&self.program)
                .arg("--config")
                .arg(self.person())
                .arg("context")
                .args(args),
        )
        .expect("docker runs");
        assert!(out.status.success(), "docker context {args:?}: {out:?}");
    }

    fn tls_host(&self, route: &str) -> String {
        let file = |name: &str| self.tls_dir.join(name).display().to_string();
        format!(
            "host={route},ca={},cert={},key={}",
            file("ca.pem"),
            file("cert.pem"),
            file("key.pem")
        )
    }

    fn read(&self) -> Vec<DockerContext> {
        dockercontext::contexts_with(&self.program, Some(&self.person()))
    }
}

fn named<'a>(contexts: &'a [DockerContext], name: &str) -> &'a DockerContext {
    contexts
        .iter()
        .find(|one| one.name == name)
        .unwrap_or_else(|| panic!("no context {name} in {contexts:?}"))
}

/// Every file under `root`.
fn files_under(root: &Path) -> Vec<PathBuf> {
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
fn an_ssh_context_is_read_into_its_route_and_a_pool_reaches_the_engine_by_it() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    let host = format!("host={}", fixture.ssh_route);
    fixture.context(&["create", "ssh-box", "--docker", &host]);
    fixture.context(&["use", "ssh-box"]);
    let read = fixture.read();
    let context = named(&read, "ssh-box");
    assert!(context.current);
    let Endpoint::Ssh(route) = &context.endpoint else {
        panic!("not read as SSH: {context:?}");
    };
    assert_eq!(route.spelled(), fixture.ssh_route);

    // Connect, on the route read: met, its socket found and its engine read through a forward.
    let ssh = sshroute::ssh_here().expect("the system's ssh");
    let settings_in = fixture.folders.path().join("s");
    let key = connect::meet(&ssh, route, &settings_in).expect("met");
    let found = connect::probe(&ssh, route, "ctx", &key, &settings_in, "pool").expect("its socket");
    let opening = forward::Opening {
        ssh: &ssh,
        route: &found,
        pool: "ctx",
        key: &key,
        engine_links: &fixture.folders.path().join("l"),
        settings_in: &settings_in,
    };
    connect::engine_pin(&fixture.docker, &opening).expect("its engine answers by that route");

    // The context changed afterwards: the pool's route is what it was, and doctor says it moved.
    let moved = fixture.ssh_route.replace("pool@", "outsider@");
    let host = format!("host={moved}");
    fixture.context(&["update", "ssh-box", "--docker", &host]);
    assert_eq!(
        dockercontext::moved(&fixture.read(), "ssh-box", &found.spelled()),
        Some(moved)
    );
}

#[test]
fn a_tls_context_is_read_into_its_route_and_the_files_docker_keeps_for_it() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    fixture.context(&[
        "create",
        "tls-box",
        "--docker",
        &fixture.tls_host(&fixture.tls_route),
    ]);
    let read = fixture.read();
    let context = named(&read, "tls-box");
    let Endpoint::Tls { route, files } = &context.endpoint else {
        panic!("not read as TLS: {context:?}");
    };
    assert_eq!(route.spelled(), fixture.tls_route);
    // The files are Docker's own copies, in the person's settings, never the ones it was made from.
    assert!(files.ca.starts_with(fixture.person()), "{files:?}");
    assert!(files_under(&fixture.person()).contains(&files.ca));

    // Connect, with the context's files: met by its CA, trusted with its engine, and a pool's
    // runs reach it.
    let config = fixture.folders.path().join("config");
    let met = connecting::meet_tls(&config, &route.spelled(), files).expect("met");
    let remote = connecting::trust_tls(
        &fixture.docker,
        &route.spelled(),
        files,
        &met.fingerprint.spelled(),
        &config,
    )
    .expect("trusted");
    let through = purlis_core::tlsroute::pool::docker_over(&fixture.docker, "ctx-tls", &remote)
        .expect("the pool's docker");
    assert_eq!(through.engine_id_said().expect("its id"), remote.pin.engine);

    // The context now names another address: the pool still runs on its own route, and doctor
    // says the context moved.
    let other = fixture.tls_route.replace("localhost", "127.0.0.1");
    fixture.context(&["update", "tls-box", "--docker", &fixture.tls_host(&other)]);
    assert_eq!(
        dockercontext::moved(&fixture.read(), "tls-box", &remote.route.spelled()),
        Some(
            purlis_core::tlsroute::Route::parse(&other)
                .unwrap()
                .spelled()
        )
    );
    let still = purlis_core::tlsroute::pool::docker_over(&fixture.docker, "ctx-tls", &remote)
        .expect("the pool's docker, by its own route");
    assert_eq!(still.engine_id_said().expect("its id"), remote.pin.engine);
}

#[test]
fn a_context_on_a_socket_here_is_this_machine_and_plain_tcp_is_refused() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else { return };
    fixture.context(&[
        "create",
        "here",
        "--docker",
        "host=unix:///var/run/docker.sock",
    ]);
    fixture.context(&["create", "open", "--docker", "host=tcp://localhost:2375"]);
    let read = fixture.read();
    assert_eq!(named(&read, "here").endpoint, Endpoint::Here);
    assert_eq!(named(&read, "default").endpoint, Endpoint::Here);
    assert_eq!(
        named(&read, "open").endpoint,
        Endpoint::Refused(sshroute::PLAIN_TCP.to_owned())
    );
}
/// What Docker does to a context's files after Connect trusted them (#6268): `docker context
/// update` rewrites them in place; `docker context rm` deletes them.
enum Afterwards {
    Rewritten,
    Removed,
}

/// **A TLS pool runs on purlis's own copies of the context's files** (#6268, the 2026-11-14
/// amendment): picked on Connect and trusted there, and through `purlis pool add --context`,
/// the route each makes reaches the engine after Docker rewrote or deleted the context's files.
fn a_tls_pool_still_runs_where_it_did(afterwards: Afterwards) {
    let Some(fixture) = fixture() else { return };
    fixture.context(&[
        "create",
        "tls-box",
        "--docker",
        &fixture.tls_host(&fixture.tls_route),
    ]);
    let read = fixture.read();
    let context = named(&read, "tls-box");
    let Endpoint::Tls {
        files: docker_s, ..
    } = &context.endpoint
    else {
        panic!("not read as TLS: {context:?}");
    };
    // The machine store, never under /tmp, which chats write.
    let config = tempfile::Builder::new()
        .prefix("config")
        .tempdir_in(fixture.tls_dir.parent().expect("the TLS files' folder"))
        .expect("a folder");
    let config = config.path();

    // The window: the context picked on Connect, its CA met and trusted.
    purlis_core::poolsetup::choose_docker_context(config, context).expect("picked");
    let chosen = purlis_core::poolsetup::read_choices(config)
        .unwrap()
        .connect
        .expect("chosen");
    let purlis_core::poolsetup::Pressed::MetCa(met) =
        connecting::meet_chosen(config, &fixture.docker, &chosen)
    else {
        panic!("not met");
    };
    connecting::trust_chosen(config, &fixture.docker, &chosen, &met.fingerprint.spelled())
        .expect("read")
        .expect("trusted");
    let chosen = purlis_core::poolsetup::read_choices(config)
        .unwrap()
        .connect
        .expect("chosen");
    assert_eq!(chosen.docker_context.as_deref(), Some("tls-box"));
    let window = purlis_core::tlsroute::Remote {
        route: purlis_core::tlsroute::Route::parse(&chosen.route).unwrap(),
        files: chosen.tls.clone().expect("its files"),
        pin: chosen.trusted_tls.clone().expect("its pin"),
    };

    // The command line: `purlis pool add --context tls-box --trust <the CA>`.
    let asked = purlis_core::poolsetup::asked::Asked {
        trust: Some(met.fingerprint.spelled()),
        ..Default::default()
    }
    .reading_docker_context(context)
    .expect("read");
    let Some(purlis_core::runners::RemoteEngine::Tls(command)) =
        purlis_core::poolsetup::asked::remote_of(config, &fixture.docker, &asked).expect("trusted")
    else {
        panic!("not over TLS");
    };

    for remote in [&window, &command] {
        let kept = dockercontext::kept_tls_dir(config);
        assert!(remote.files.ca.starts_with(&kept), "{:?}", remote.files);
        assert!(remote.files.cert.starts_with(&kept), "{:?}", remote.files);
        assert!(
            remote
                .files
                .key_file()
                .is_some_and(|key| key.starts_with(&kept)),
            "{:?}",
            remote.files
        );
    }

    match afterwards {
        Afterwards::Rewritten => {
            for file in [&docker_s.ca, &docker_s.cert, docker_s.key_file().unwrap()] {
                std::fs::write(file, b"rewritten by docker context update").unwrap();
            }
        }
        Afterwards::Removed => {
            fixture.context(&["rm", "--force", "tls-box"]);
            assert!(!docker_s.ca.exists(), "Docker kept {docker_s:?}");
        }
    }
    for remote in [&window, &command] {
        let still = purlis_core::tlsroute::pool::docker_over(&fixture.docker, "ctx-tls", remote)
            .expect("the pool's docker, by its own files");
        assert_eq!(still.engine_id_said().expect("its id"), remote.pin.engine);
    }
}

#[test]
fn a_tls_pool_still_runs_where_it_did_after_docker_rewrites_the_contexts_files() {
    purlis_core::unsteered!();
    a_tls_pool_still_runs_where_it_did(Afterwards::Rewritten);
}

#[test]
fn a_tls_pool_still_runs_where_it_did_after_the_context_is_removed() {
    purlis_core::unsteered!();
    a_tls_pool_still_runs_where_it_did(Afterwards::Removed);
}

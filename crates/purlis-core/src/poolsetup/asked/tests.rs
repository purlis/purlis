// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dockerpool::PURLIS_RUNNER_IMAGE as OURS;
use crate::dockerpool::tests::{DIGEST, FakeDocker, inspected};

fn image() -> String {
    format!("{OURS}@{DIGEST}")
}

#[test]
fn the_command_says_only_what_was_changed() {
    let asked = Asked {
        name: "dev-box".into(),
        image: image(),
        ..Asked::default()
    };
    assert_eq!(
        asked.command(),
        format!("purlis pool add dev-box --image {}", image())
    );
}

#[test]
fn the_command_says_every_setting_the_person_set() {
    let asked = Asked {
        name: "dev-box".into(),
        image: image(),
        cpus: Some("1.5".into()),
        memory_mib: Some(8192),
        processes: Some(512),
        tmp_mib: Some(256),
        suspend_after: Some("12h".into()),
        frames: Some(5),
        width: Some(1280),
        ..Asked::default()
    };
    assert_eq!(
        asked.command(),
        format!(
            "purlis pool add dev-box --image {} --cpus 1.5 --memory-mib 8192 --processes 512 \
             --tmp-mib 256 --suspend-after 12h --frames 5 --width 1280",
            image()
        )
    );
}

#[test]
fn a_name_not_given_yet_is_a_stand_in_and_never_an_empty_word() {
    let said = Asked::default().command();
    assert_eq!(said, "purlis pool add NAME --image IMAGE");
}

#[test]
fn what_no_pool_could_have_never_reaches_the_command() {
    let asked = Asked {
        name: "x\\".into(),
        image: "; rm -rf ~ #".into(),
        cpus: Some("2; reboot".into()),
        suspend_after: Some("$(id)".into()),
        ..Asked::default()
    };
    assert_eq!(asked.command(), "purlis pool add NAME --image IMAGE");
}

#[test]
fn only_purlis_s_own_repository_is_its_own_image() {
    let ours = Asked {
        image: image(),
        ..Asked::default()
    };
    assert!(ours.is_ours());
    let named_alike = Asked {
        image: format!("{OURS}-fork@{DIGEST}"),
        ..Asked::default()
    };
    assert!(!named_alike.is_ours());
    let inside_a_path = Asked {
        image: format!("registry.example/{OURS}@{DIGEST}"),
        ..Asked::default()
    };
    assert!(!inside_a_path.is_ours());
}

#[test]
fn a_time_or_a_count_of_cpus_a_pool_cant_have_is_refused_in_words() {
    let asked = Asked {
        name: "dev-box".into(),
        image: image(),
        cpus: Some("lots".into()),
        ..Asked::default()
    };
    assert!(asked.pool().unwrap_err().contains("number of CPUs"));
    let asked = Asked {
        suspend_after: Some("soon".into()),
        cpus: None,
        ..asked
    };
    assert!(asked.pool().unwrap_err().contains("is not a time"));
}

#[test]
fn a_team_s_image_that_breaks_the_contract_makes_nothing() {
    let config = tempfile::tempdir().unwrap();
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "image",
        &serde_json::json!([{ "Os": "linux", "Config": { "User": "0:0", "Env": [] } }]).to_string(),
    );
    let asked = Asked {
        name: "dev-box".into(),
        image: format!("registry.example/runner@{DIGEST}"),
        ..Asked::default()
    };
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: purlis_session_protocol::hostlink::LinkKey::generate()
            .unwrap()
            .public(),
    };
    let made = create(config.path(), &fake.docker(), &asked, &desktop).expect("an answer");
    let Created::Breaks(problems) = made else {
        panic!("{made:?}");
    };
    assert!(
        problems.iter().any(|one| one.contains("user")),
        "{problems:?}"
    );
    assert!(
        crate::runners::pools(config.path()).unwrap().is_empty(),
        "nothing was recorded"
    );
    assert!(
        !fake.file("log").contains("run -d"),
        "no runner was made: {}",
        fake.file("log")
    );
}

#[test]
fn a_pool_on_another_machine_names_it_and_the_key_trusted_in_the_command() {
    let asked = Asked {
        name: "build-box".into(),
        image: image(),
        on: Some("ssh://me@build-box/var/run/docker.sock".into()),
        trust: Some("SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA".into()),
        ..Asked::default()
    };
    assert_eq!(
        asked.command(),
        format!(
            "purlis pool add build-box --image {} --on \
             ssh://me@build-box/var/run/docker.sock --trust \
             SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA",
            image()
        )
    );
}

#[test]
fn a_route_or_fingerprint_that_isnt_one_never_reaches_the_command() {
    let asked = Asked {
        name: "build-box".into(),
        image: image(),
        on: Some("ssh://-oProxyCommand=x".into()),
        trust: Some("SHA256:$(id)".into()),
        ..Asked::default()
    };
    assert_eq!(
        asked.command(),
        format!("purlis pool add build-box --image {}", image())
    );
}

/// The machine Connect kept is the one `--on` names whether or not the socket or the key file is
/// named again (#6269): the kept route, with both, is what the pool is made with, and nothing is
/// contacted.
#[test]
fn the_machine_connect_kept_is_taken_with_its_socket_and_key_file_wherever_the_ask_leaves_them_out()
{
    let config = tempfile::tempdir().unwrap();
    let identity = std::path::Path::new("/home/me/.ssh/id_ed25519");
    let kept = crate::sshroute::Route::parse("ssh://me@build-box:2222/run/podman/podman.sock")
        .unwrap()
        .with_identity(identity)
        .unwrap();
    let remote = crate::runners::Remote {
        route: kept.clone(),
        pin: crate::sshroute::Pin {
            host_key: crate::sshroute::HostKey::parse(
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
            )
            .unwrap(),
            engine: crate::sshroute::EnginePin::Podman {
                socket: "/run/podman/podman.sock".into(),
                graph_root: "/var/lib/containers/storage".into(),
            },
        },
    };
    super::super::trusted(config.path(), &remote).unwrap();
    // A docker that is never run: nothing is contacted for a machine already trusted.
    let docker = Docker::at("/nonexistent/docker");
    for (on, with) in [
        ("ssh://me@build-box:2222", None),
        ("ssh://me@build-box:2222/run/podman/podman.sock", None),
        ("ssh://me@build-box:2222", Some("/home/me/.ssh/id_ed25519")),
    ] {
        let asked = Asked {
            on: Some(on.into()),
            identity: with.map(str::to_owned),
            ..Asked::default()
        };
        let found = remote_of(config.path(), &docker, &asked).unwrap().unwrap();
        assert_eq!(found, remote.clone().into(), "{on} {with:?}");
    }
}

fn tls_files(key: crate::tlsroute::KeyFrom) -> crate::tlsroute::Files {
    crate::tlsroute::Files::new(
        "/home/me/.docker/ca.pem".into(),
        "/home/me/.docker/cert.pem".into(),
        key,
    )
    .unwrap()
}

#[test]
fn the_command_names_a_tcp_machines_files_and_a_vault_entry_by_its_names_alone() {
    let asked = Asked {
        name: "dev-box".into(),
        image: image(),
        on: Some("tcp://build-box:2376".into()),
        trust: Some(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        ),
        tls: Some(tls_files(crate::tlsroute::KeyFrom::Vault {
            plane: "/home/me/plane".into(),
            vault: "docker-box".into(),
            key: "TLS_KEY".into(),
        })),
        ..Asked::default()
    };
    assert_eq!(
        asked.command(),
        format!(
            "purlis pool add dev-box --image {} --on tcp://build-box:2376 --trust \
             sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --tls-ca \
             /home/me/.docker/ca.pem --tls-cert /home/me/.docker/cert.pem --tls-key-in \
             docker-box/TLS_KEY",
            image()
        )
    );
}

#[test]
fn a_tcp_machine_without_its_tls_files_is_refused_as_plain_tcp_and_contacts_nothing() {
    let config = tempfile::tempdir().unwrap();
    let docker = FakeDocker::new(&inspected());
    let asked = Asked {
        name: "dev-box".into(),
        image: image(),
        on: Some("tcp://build-box:2375".into()),
        ..Asked::default()
    };
    match remote_of(config.path(), &docker.docker(), &asked) {
        Err(Refused::Other(why)) => assert_eq!(why, crate::sshroute::PLAIN_TCP),
        other => panic!("{other:?}"),
    }
    assert_eq!(docker.file("log"), "", "docker was run");
}

fn docker_context(
    name: &str,
    endpoint: crate::dockercontext::Endpoint,
) -> crate::dockercontext::DockerContext {
    crate::dockercontext::DockerContext {
        name: name.to_owned(),
        current: false,
        endpoint,
    }
}

/// `purlis pool add --context` (#6268): the context named is read once into the route its
/// endpoint names, with its TLS files, and its name kept for the pool; this machine's own
/// socket, or an endpoint purlis doesn't use, is refused and makes nothing.
#[test]
fn a_docker_context_named_is_read_into_the_route_and_files_and_kept_by_name() {
    let files = crate::tlsroute::Files::new(
        "/home/me/.docker/contexts/tls/9a8b/docker/ca.pem".into(),
        "/home/me/.docker/contexts/tls/9a8b/docker/cert.pem".into(),
        crate::tlsroute::KeyFrom::File("/home/me/.docker/contexts/tls/9a8b/docker/key.pem".into()),
    )
    .unwrap();
    let lab = docker_context(
        "lab",
        crate::dockercontext::Endpoint::Tls {
            route: crate::tlsroute::Route::parse("tcp://lab.example").unwrap(),
            files: files.clone(),
        },
    );
    let asked = Asked {
        name: "lab".into(),
        image: image(),
        ..Asked::default()
    }
    .reading_docker_context(&lab)
    .unwrap();
    assert_eq!(asked.on.as_deref(), Some("tcp://lab.example:2376"));
    assert_eq!(asked.tls, Some(files));
    assert_eq!(asked.docker_context.as_deref(), Some("lab"));
    assert_eq!(
        asked.command(),
        format!("purlis pool add lab --image {} --context lab", image())
    );
    let ssh = Asked::default()
        .reading_docker_context(&docker_context(
            "build-box",
            crate::dockercontext::Endpoint::Ssh(
                crate::sshroute::Route::parse("ssh://me@build-box").unwrap(),
            ),
        ))
        .unwrap();
    assert_eq!(ssh.on.as_deref(), Some("ssh://me@build-box"));
    assert_eq!(ssh.tls, None);
    let here = Asked::default().reading_docker_context(&docker_context(
        "desktop-linux",
        crate::dockercontext::Endpoint::Here,
    ));
    assert!(
        matches!(&here, Err(Refused::Other(why)) if why.contains("this machine")),
        "{here:?}"
    );
    let open = Asked::default().reading_docker_context(&docker_context(
        "open",
        crate::dockercontext::Endpoint::Refused(crate::sshroute::PLAIN_TCP.to_owned()),
    ));
    assert!(matches!(&open, Err(Refused::Other(why)) if why == crate::sshroute::PLAIN_TCP));
}

/// The pool the Pool step makes keeps the context Connect read its route from, where it is made
/// on that same machine; a context named on the command comes first.
#[test]
fn the_pool_keeps_the_context_connect_read_its_route_from() {
    let config = tempfile::tempdir().unwrap();
    super::super::choose_docker_context(
        config.path(),
        &docker_context(
            "build-box",
            crate::dockercontext::Endpoint::Ssh(
                crate::sshroute::Route::parse("ssh://me@build-box").unwrap(),
            ),
        ),
    )
    .unwrap();
    let asked = Asked::default();
    assert_eq!(
        context_for(
            config.path(),
            &asked,
            "ssh://me@build-box/var/run/docker.sock"
        )
        .unwrap(),
        Some("build-box".to_owned())
    );
    assert_eq!(
        context_for(
            config.path(),
            &asked,
            "ssh://me@other-box/var/run/docker.sock"
        )
        .unwrap(),
        None
    );
    let named = Asked {
        docker_context: Some("named".into()),
        ..Asked::default()
    };
    assert_eq!(
        context_for(
            config.path(),
            &named,
            "ssh://me@other-box/var/run/docker.sock"
        )
        .unwrap(),
        Some("named".to_owned())
    );
}

/// **A pool made from what Connect kept is marked as the person answered there** (#6271), and only
/// for the pin they answered for.
#[test]
fn a_pool_made_from_what_connect_kept_is_marked_only_for_the_pin_answered_for() {
    let config = tempfile::tempdir().unwrap();
    let remote = crate::runners::Remote {
        route: crate::sshroute::Route::parse("ssh://me@build-box/var/run/docker.sock").unwrap(),
        pin: crate::sshroute::Pin {
            host_key: crate::sshroute::HostKey::parse(
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
            )
            .unwrap(),
            engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
        },
    };
    super::super::trusted(config.path(), &remote).unwrap();
    let engine: crate::runners::RemoteEngine = remote.clone().into();
    assert!(!marked_on_connect(config.path(), &engine).unwrap(), "not asked");
    super::super::choose_owned(config.path(), true).unwrap();
    assert!(marked_on_connect(config.path(), &engine).unwrap());
    let mut other = remote;
    other.pin.engine = crate::sshroute::EnginePin::Docker("WXYZ".to_owned());
    assert!(!marked_on_connect(config.path(), &other.into()).unwrap());
}

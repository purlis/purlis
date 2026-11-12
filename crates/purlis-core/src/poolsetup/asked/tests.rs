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

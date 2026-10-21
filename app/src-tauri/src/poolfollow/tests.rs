// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use purlis_core::dockerpool::{ContainerId, Docker, Image, Made, Runner, User};
use purlis_core::runners::{self, Pool, Set};
use purlis_core::secrets::keyring::FileStore;
use purlis_session_protocol::hostlink::{LinkKey, Pinned};

use super::*;

const OLD: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NEW: &str = "ghcr.io/purlis/purlis-runner@sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// A machine in `config` whose `docker` removes whatever it is asked to, and logs it.
fn a_machine(config: &std::path::Path) -> Machine {
    let docker = config.join("docker");
    std::fs::write(
        &docker,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nexit 0\n",
            config.join("log").display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755)).unwrap();
    Machine {
        config: config.to_path_buf(),
        docker: Docker::at(docker).with_settings_in(config.join("docker-settings")),
        keyring: Arc::new(FileStore::at(config.join("keyring.json"))),
    }
}

fn made(name: &str, image: &str, id: char) -> Made {
    Made {
        container: ContainerId::new(&id.to_string().repeat(64)).unwrap(),
        runner: Runner::new(
            name,
            Image::pinned(image).unwrap(),
            User::new(10001, 10001).unwrap(),
            "/home/purlis",
        )
        .unwrap(),
    }
}

fn answers(device: &str) -> Pinned {
    Pinned {
        device: device.into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

/// Pool `first`, its runner `first` draining beside `first-fedcba98`, of the new image.
fn a_pool_draining(machine: &Machine) {
    let ours = machine.desktop_made().unwrap().pinned();
    let config = &machine.config;
    runners::record_pool(
        config,
        &Pool::new("first", Image::pinned(OLD).unwrap(), Set::default()).unwrap(),
    )
    .unwrap();
    let locked = runners::lock(config);
    runners::keep_for_its_pool(
        config,
        &made("first", OLD, 'a'),
        &answers("01JB0RUNNER0000000000000000"),
        &ours,
        &locked,
    )
    .unwrap();
    runners::succeed_locked(
        config,
        "first",
        &made("first-fedcba98", NEW, 'b'),
        &answers("01JB0RUNNER0000000000000001"),
        &ours,
        &locked,
    )
    .unwrap();
}

fn names(machine: &Machine) -> Vec<String> {
    runners::read(&machine.config)
        .unwrap()
        .iter()
        .map(|kept| kept.name().to_owned())
        .collect()
}

#[test]
fn a_draining_runner_is_removed_once_no_chat_is_left_on_it_and_not_before() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);

    // A chat still runs on it: it stays.
    follow_and_drain(&machine, None, &|_| Ok(true));
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    // It does not answer: not known to be idle, so it stays.
    let said = follow_and_drain(&machine, None, &|_| Err("down".into()));
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    assert!(
        said.iter()
            .any(|line| line.contains("stays until it answers")),
        "{said:?}"
    );

    // No chat is left: it is removed, its container and home with it, and the pool's own stays.
    let said = follow_and_drain(&machine, None, &|_| Ok(false));
    assert_eq!(names(&machine), ["first-fedcba98"]);
    assert!(
        said.iter().any(|line| line.contains("is removed")),
        "{said:?}"
    );
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(
        log.contains(&format!(
            "container rm --force --volumes {}",
            "a".repeat(64)
        )),
        "{log}"
    );
}

#[test]
fn a_pool_already_of_this_versions_image_or_of_a_teams_own_is_left_as_it_is() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let team = "registry.example/team/runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    runners::record_pool(
        &machine.config,
        &Pool::new("team", Image::pinned(team).unwrap(), Set::default()).unwrap(),
    )
    .unwrap();

    let said = follow_and_drain(&machine, Some(&Image::pinned(NEW).unwrap()), &|_| Ok(true));

    assert!(said.is_empty(), "{said:?}");
    assert!(
        !config.path().join("log").exists(),
        "docker was asked to make something"
    );
}

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

/// A machine in `config` whose `docker` removes whatever it is asked to, and logs it. Every
/// container it is asked of is still there: none was removed outside purlis (#4149).
fn a_machine(config: &std::path::Path) -> Machine {
    let docker = config.join("docker");
    std::fs::write(
        &docker,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$1 $2\" in 'container ls') \
             for a in \"$@\"; do case $a in id=*) echo \"${{a#id=}}\" ;; esac; done ;; esac\n\
             exit 0\n",
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

/// Whether docker was asked nothing but to look at a container, if anything.
fn only_looked(config: &std::path::Path) -> bool {
    std::fs::read_to_string(config.join("log"))
        .unwrap_or_default()
        .lines()
        .all(|line| line.starts_with("container inspect ") || line.starts_with("container ls "))
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

    // Chats still run on it: it stays, and the pool says how many it waits on.
    follow_and_drain(&machine, None, &|_| Ok(2));
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    assert_eq!(waiting_on(&machine.config, "first"), 2);
    // Nor is a sign-in of it carried to the pool's runner while they run (#4055): a new chat
    // there asks to sign in, and one runner holds each sign-in.
    let log = std::fs::read_to_string(config.path().join("log")).unwrap_or_default();
    assert!(!log.contains("-copy "), "{log}");
    // It does not answer: not known to be idle, so it stays.
    let said = follow_and_drain(&machine, None, &|_| Err("down".into()));
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    assert!(
        said.iter()
            .any(|line| line.contains("stays until it answers")),
        "{said:?}"
    );

    // No chat is left: it is removed, its container and home with it, and the pool's own stays.
    let said = follow_and_drain(&machine, None, &|_| Ok(0));
    assert_eq!(names(&machine), ["first-fedcba98"]);
    assert!(
        said.iter().any(|line| line.contains("is removed")),
        "{said:?}"
    );
    assert_eq!(
        waiting_on(&machine.config, "first"),
        0,
        "a removed runner waits on nothing"
    );
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(
        log.contains(&format!(
            "container rm --force --volumes {}",
            "a".repeat(64)
        )),
        "{log}"
    );
    // Its sign-ins went to the pool's runner only now, with no chat left on it (#4055).
    let carried = log
        .find("--name purlis-runner-first-fedcba98-copy ")
        .expect(&log);
    assert!(
        carried < log.find("container rm --force --volumes").unwrap(),
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

    let said = follow_and_drain(&machine, Some(&Image::pinned(NEW).unwrap()), &|_| Ok(2));

    assert!(said.is_empty(), "{said:?}");
    // Asking whether a draining runner's container runs (#2051) is all docker was asked.
    assert!(
        only_looked(config.path()),
        "docker was asked to make something"
    );
}

/// **A release that names no runner image leaves every pool as it is** (#2880): the look notes
/// that none was named, and nothing is made or moved.
#[test]
fn a_release_with_no_runner_image_leaves_the_pools_where_they_are() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let before = runners::read(&machine.config).unwrap();
    let pools = runners::pools(&machine.config).unwrap();

    note_named(&Ok(Found::NoneNamed));
    assert_eq!(this_versions_image_named(), Some(false));
    follow_and_drain(&machine, None, &|_| Ok(1));

    assert_eq!(runners::read(&machine.config).unwrap(), before);
    assert_eq!(runners::pools(&machine.config).unwrap(), pools);
    // Asking whether a draining runner's container runs (#2051) is all docker was asked.
    assert!(
        only_looked(config.path()),
        "docker was asked to do something"
    );

    // A look that could not be made, or one of another version, notes nothing; one that found
    // this version's image notes it.
    note_named(&Err("offline".into()));
    assert_eq!(this_versions_image_named(), Some(false));
    note_named(&Ok(Found::OtherVersion));
    assert_eq!(this_versions_image_named(), Some(false));
    note_named(&Ok(Found::Named(Image::pinned(NEW).unwrap())));
    assert_eq!(this_versions_image_named(), Some(true));
    note_named(&Ok(Found::OtherVersion));
    assert_eq!(this_versions_image_named(), Some(true));
    // The image found is kept, for a new pool's Image box: it is not read again.
    assert_eq!(
        this_versions_image_noted(),
        Some(Image::pinned(NEW).unwrap())
    );
}

/// **The Add a pool dialog is told which release's image it is offered** (the person's ruling
/// of 2026-10-30): this version's, a newer version's with the update to it waiting, or none;
/// and why where it was refused.
#[test]
fn the_dialog_is_told_which_releases_image_a_new_pool_is_offered() {
    use purlis_core::runnerimage::Offered;
    let image = Image::pinned(NEW).unwrap();
    assert_eq!(
        said(Ok(Offered::Ours(image.clone()))),
        Ok(RunnerImageOffered::Ours {
            image: NEW.to_owned()
        })
    );
    assert_eq!(
        said(Ok(Offered::Newer {
            image,
            version: "0.4.3".to_owned()
        })),
        Ok(RunnerImageOffered::Newer {
            image: NEW.to_owned(),
            version: "0.4.3".to_owned()
        })
    );
    assert_eq!(said(Ok(Offered::None)), Ok(RunnerImageOffered::NoneNamed));
    assert_eq!(said(Err("not signed".into())), Err("not signed".to_owned()));
}

/// **Only a manifest of this very version says its release has no runner image** (#2880): one
/// of another version, a desktop behind or ahead of its channel, says nothing of it.
#[test]
fn only_a_manifest_of_this_version_says_its_release_has_no_runner_image() {
    let ours = "0.4.2-dev.17";
    assert_eq!(found_in(None, "", ours), Ok(Found::OtherVersion));
    for other in ["0.4.2-dev.16", "0.4.2-dev.18", "0.4.3"] {
        assert_eq!(
            found_in(Some(&serde_json::json!({ "version": other })), "", ours),
            Ok(Found::OtherVersion),
            "{other}"
        );
    }
    assert_eq!(
        found_in(Some(&serde_json::json!({ "version": ours })), "", ours),
        Ok(Found::NoneNamed)
    );
}

/// A removed pool waits on nothing: what its draining runners waited on goes with it (#2865).
#[test]
fn a_removed_pools_waiting_chats_are_let_go_of() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    follow_and_drain(&machine, None, &|_| Ok(3));
    assert_eq!(waiting_on(&machine.config, "first"), 3);

    forget_waiting(&machine.config, "first");
    assert_eq!(waiting_on(&machine.config, "first"), 0);
}

/// A machine in `config` whose `docker` says every container is stopped, as a suspended
/// runner's is, and removes whatever it is asked to.
fn a_machine_whose_runners_are_suspended(config: &std::path::Path) -> Machine {
    let machine = a_machine(config);
    std::fs::write(
        config.join("docker"),
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$1 $2\" in\n  'container inspect') \
             echo '[{{\"State\":{{\"Running\":false}}}}]' ;;\nesac\nexit 0\n",
            config.join("log").display()
        ),
    )
    .unwrap();
    machine
}

/// A suspended draining runner runs no chat (#2051): it is removed without being woken to ask.
/// Seen red: with the look at its container taken out, the runner is asked.
#[test]
fn a_suspended_draining_runner_is_removed_without_being_asked() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);

    let said = follow_and_drain(&machine, None, &|_| {
        panic!("a suspended runner is not asked")
    });

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(!log.contains("start "), "{log}");
}

/// A runner that answered its unpairing cannot be asked, and takes no chat (#4146): the next
/// look removes what is left without asking it.
#[test]
fn an_unpaired_runner_is_removed_without_being_asked_again() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(config.path(), "first", |kept| kept.unpaired = true, &locked)
        .unwrap();
    drop(locked);

    let said = follow_and_drain(&machine, None, &|_| panic!("it is not asked"));

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
}

/// Carried from but not unpaired, it could still take a link, so each look still asks (#4146): a
/// chat there keeps it.
#[test]
fn a_runner_carried_from_but_not_unpaired_is_still_asked() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(config.path(), "first", |kept| kept.carried = true, &locked)
        .unwrap();
    drop(locked);

    follow_and_drain(&machine, None, &|_| Ok(1));

    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// **Two links' `docker`s, each pointed at its pool's forward, never share what their pins
/// said** (#7922): [`pointed`], which every [`docker_of`] result is made with, gives each its
/// own slot, even from a `docker` that already keeps one, so one link's pin check never hands
/// its `docker info` to another link's daemon check.
/// Seen red: with `keeping_what_its_pin_said` keeping a slot it was handed, the second link's
/// check took the first's options and ran no `docker info` of its own.
#[test]
fn two_links_pointed_at_a_forward_never_share_what_their_pins_said() {
    use crate::dockerpool::tests::{FakeDocker, info_of, inspected, made, runs_of};
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "info",
        &info_of(
            "PINNED",
            &serde_json::json!(["name=seccomp,profile=builtin"]),
        ),
    );
    let remote = Remote {
        route: crate::sshroute::Route::parse("ssh://me@box").unwrap(),
        pin: crate::sshroute::Pin {
            host_key: crate::sshroute::HostKey::parse(
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
            )
            .unwrap(),
            engine: crate::sshroute::EnginePin::Docker("PINNED".into()),
        },
    };
    let here = tempfile::tempdir().unwrap();
    let link = |docker: &Docker| {
        pointed(
            docker,
            Path::new("/usr/bin/ssh"),
            "far",
            &remote,
            here.path().join("forward.sock"),
            here.path(),
        )
        .unwrap()
    };
    // Already keeping: the slot it holds is never handed to a link.
    let base = fake.docker().keeping_what_its_pin_said();
    let first = link(&base);
    let second = link(&base);
    assert_eq!(
        connect::engine_holds(&first, &remote.route, &remote.pin),
        Ok(())
    );
    let read = runs_of(&fake, "info ");
    // The second link's check reads the daemon itself, here as the whole info it can't pass.
    assert!(second.check(&made()).is_err(), "{}", fake.file("log"));
    assert!(runs_of(&fake, "info ") > read, "{}", fake.file("log"));
    assert!(base.check(&made()).is_err(), "{}", fake.file("log"));
    // The first link's is still its own.
    let read = runs_of(&fake, "info ");
    assert!(first.check(&made()).is_ok(), "{}", fake.file("log"));
    assert_eq!(runs_of(&fake, "info "), read, "{}", fake.file("log"));
}

fn silent() -> Finding {
    Finding::EngineSilent {
        host: "box".to_owned(),
        said: "no".to_owned(),
    }
}

/// **A config that cannot be read is Refused with its reason, not Unreachable** (#7187,
/// steward decision 2026-11-19): waiting does not mend a runners file that is not as purlis
/// writes it, so the link stops, says why and offers Check pool.
#[cfg(unix)]
#[test]
fn a_config_that_cannot_be_read_is_refused_with_its_reason() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    let folder = crate::machine::dir(config.path());
    std::fs::create_dir_all(&folder).unwrap();
    // A file others may read is refused as unsafe, whatever it holds.
    let file = folder.join(crate::machine::RUNNERS_FILE);
    std::fs::write(&file, "{}").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let local = Docker::at("/usr/local/bin/docker");
    let (said, state) =
        docker_of_named_in_state(config.path(), &local, "box").expect_err("unreadable");
    assert_eq!(state, connect::RemoteState::Refused, "{said}");
    assert!(said.contains("could not be read safely"), "{said}");
}

#[test]
fn a_failing_forward_waits_twice_as_long_each_time_up_to_a_cap() {
    assert_eq!(retry_after(1), RETRY_FIRST);
    assert_eq!(retry_after(2), RETRY_FIRST * 2);
    assert_eq!(retry_after(3), RETRY_FIRST * 4);
    assert_eq!(retry_after(30), RETRY_MOST);
    assert_eq!(retry_after(u32::MAX), RETRY_MOST);
}

#[test]
fn within_its_wait_a_failing_forward_is_answered_with_what_was_found_and_nothing_is_contacted() {
    let now = Instant::now();
    let failing = Failing::first(silent(), now);
    assert_eq!(failing.ask(now), Ask::Answer(silent()));
    assert_eq!(
        failing.ask(now + RETRY_FIRST - Duration::from_millis(1)),
        Ask::Answer(silent())
    );
    assert_eq!(failing.ask(now + RETRY_FIRST), Ask::Try);
}

#[test]
fn try_now_forgets_the_pools_remembered_failure_and_leaves_other_pools_waiting() {
    let pin = crate::sshroute::Pin {
        host_key: crate::sshroute::HostKey::parse(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
        )
        .unwrap(),
        engine: crate::sshroute::EnginePin::Docker("ABCD:EFGH:IJKL".to_owned()),
    };
    let now = Instant::now();
    note_failing("try-now-a", &pin, Failing::first(silent(), now));
    note_failing("try-now-b", &pin, Failing::first(silent(), now));
    let waiting = failing("try-now-a", &pin).unwrap();
    assert_eq!(waiting.ask(now), Ask::Answer(silent()), "it is waiting");
    let began = Instant::now();
    try_now("try-now-a");
    assert!(
        cleared_since("try-now-a", began),
        "a try begun before the click is not kept"
    );
    assert!(!cleared_since("try-now-b", began));
    assert_eq!(failing("try-now-a", &pin), None, "the click dials at once");
    assert!(
        failing("try-now-b", &pin).is_some(),
        "another pool keeps its wait"
    );
    forget_failing("try-now-b");
}

#[test]
fn a_pools_try_now_mark_goes_when_the_pool_is_gone() {
    let marked = |pool: &str| {
        CLEARED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(pool)
    };
    try_now("prune-gone");
    try_now("prune-kept");
    assert!(marked("prune-gone"));
    pool_gone("prune-gone");
    assert!(!marked("prune-gone"), "a pool that is gone needs no mark");
    assert!(marked("prune-kept"), "another pool keeps its mark");
    pool_gone("prune-kept");
}

#[test]
fn a_try_that_fails_again_waits_longer_and_keeps_why_for_minutes() {
    let now = Instant::now();
    let failing = Failing::first(silent(), now);
    let later = now + RETRY_FIRST;
    assert!(
        !failing.looks_again(later),
        "why is kept, not looked at again"
    );
    let again = failing.failed_again(later, None);
    assert_eq!(again.failures, 2);
    assert_eq!(again.finding, silent());
    assert_eq!(again.found_at, now);
    assert_eq!(again.next_try, later + RETRY_FIRST * 2);
    assert_eq!(again.ask(later + RETRY_FIRST), Ask::Answer(silent()));
    // Past the minutes why is kept for, a failed try is looked at again, and what it finds
    // is kept from then.
    let much_later = now + FINDING_KEPT_FOR;
    assert!(again.looks_again(much_later));
    let refused = Finding::SignInRefused {
        host: "box".to_owned(),
    };
    let looked = again.failed_again(much_later, Some(refused.clone()));
    assert_eq!(looked.finding, refused);
    assert_eq!(looked.found_at, much_later);
    assert_eq!(looked.failures, 3);
}
/// **A link to a runner of a pool on another machine is refused where its `docker` can't be
/// pointed there**, over SSH to a Docker or to a Podman alike: never made with this machine's.
#[test]
fn a_link_to_a_pool_on_another_machine_never_falls_back_to_this_machines_docker() {
    use crate::runners::{Pool, Set, record_pool};
    let image = crate::dockerpool::Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .unwrap();
    let host_key = crate::sshroute::HostKey::parse(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
    )
    .unwrap();
    // Settings this deep leave no room for the forward's socket beside them.
    let config = tempfile::tempdir().unwrap();
    let deep = config.path().join("d".repeat(120)).join("docker-settings");
    let local = Docker::at("/usr/local/bin/docker").with_settings_in(&deep);
    for (name, engine) in [
        (
            "on-docker",
            crate::sshroute::EnginePin::Docker("ABCD:EFGH:IJKL".to_owned()),
        ),
        (
            "on-podman",
            crate::sshroute::EnginePin::Podman {
                socket: "/run/podman/podman.sock".to_owned(),
                graph_root: "/var/lib/containers/storage".to_owned(),
            },
        ),
    ] {
        let remote = crate::runners::Remote {
            route: crate::sshroute::Route::parse("ssh://me@build-box/var/run/docker.sock").unwrap(),
            pin: crate::sshroute::Pin {
                host_key: host_key.clone(),
                engine,
            },
        };
        let pool = Pool::new(name, image.clone(), Set::default())
            .unwrap()
            .on(Some(remote));
        record_pool(config.path(), &pool).unwrap();
        assert!(is_remote(config.path(), name));
        let linked = docker_for_kept_link(config.path(), &local, name);
        assert!(linked.is_err(), "{name}: linked with {linked:?}");
    }
    // Over Docker's TLS API the link's docker is pointed at the pool's route, never here.
    let tls = crate::tlsroute::Remote {
        route: crate::tlsroute::Route::parse("tcp://build-box").unwrap(),
        files: crate::tlsroute::Files::new(
            "/home/me/.docker/ca.pem".into(),
            "/home/me/.docker/cert.pem".into(),
            crate::tlsroute::KeyFrom::File("/home/me/.docker/key.pem".into()),
        )
        .unwrap(),
        pin: crate::tlsroute::Pin {
            ca: crate::tlsroute::Fingerprint::of_der(b"ca"),
            engine: "ABCD:EFGH:IJKL".to_owned(),
        },
    };
    let pool = Pool::new("on-tls", image.clone(), Set::default())
        .unwrap()
        .on_engine(Some(crate::runners::RemoteEngine::Tls(tls.clone())));
    record_pool(config.path(), &pool).unwrap();
    let linked = docker_for_kept_link(config.path(), &local, "on-tls").unwrap();
    assert_eq!(
        linked.over_tls_to(),
        Some(&crate::tlsroute::Through::of(&tls))
    );
    // A pool on this machine is linked with this machine's docker, as before.
    record_pool(
        config.path(),
        &Pool::new("here", image, Set::default()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        docker_for_kept_link(config.path(), &local, "here"),
        Ok(local)
    );
}

/// **A record of pools that can't be read refuses the link**: it may name a pool on another
/// machine, so this machine's docker is never taken for it.
#[test]
fn a_link_is_refused_where_the_pools_cant_be_read() {
    let config = tempfile::tempdir().unwrap();
    let file = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    // Readable by others, so purlis won't read it.
    std::fs::write(&file, r#"{"version":1,"pools":[],"runners":[]}"#).unwrap();
    assert!(crate::runners::pools(config.path()).is_err());
    let local = Docker::at("/usr/local/bin/docker");
    let linked = docker_for_kept_link(config.path(), &local, "build-box");
    assert!(linked.is_err(), "linked with {linked:?}");
}

#[test]
fn a_forward_held_from_before_that_went_silent_is_opened_once_again_before_it_refuses() {
    // Its machine restarted: ssh ran on and its socket stayed, and nothing crossed.
    let check = |through: &u32| if *through == 1 { Err(silent()) } else { Ok(()) };
    let mut opened = 0;
    let through = checked_or_opened_again(1, false, &check, &mut || {
        opened += 1;
        Ok(Some(2))
    });
    assert_eq!(
        through,
        Through {
            docker: 2,
            opened: true,
            checked: Ok(())
        }
    );
    assert_eq!(opened, 1, "opened again once");

    // Silent again through the new one: said, with nothing more opened.
    let mut opened = 0;
    let through = checked_or_opened_again(1, false, &|_: &u32| Err(silent()), &mut || {
        opened += 1;
        Ok(Some(2))
    });
    assert_eq!((through.opened, through.checked), (true, Err(silent())));
    assert_eq!(opened, 1);
}

#[test]
fn a_forward_whose_ssh_runs_on_is_kept_for_its_links_when_the_engine_is_only_slow() {
    let through = checked_or_opened_again(1, false, &|_: &u32| Err(silent()), &mut || Ok(None));
    assert_eq!(
        through,
        Through {
            docker: 1,
            opened: false,
            checked: Err(silent())
        }
    );
}

#[test]
fn a_forward_just_opened_or_an_engine_not_the_pinned_one_is_never_opened_again() {
    let mut opened = 0;
    let through = checked_or_opened_again(1, true, &|_: &u32| Err(silent()), &mut || {
        opened += 1;
        Ok(Some(2))
    });
    assert_eq!(through.checked, Err(silent()));
    let changed = Finding::Here("another engine".to_owned());
    let through = checked_or_opened_again(1, false, &|_: &u32| Err(changed.clone()), &mut || {
        opened += 1;
        Ok(Some(2))
    });
    assert_eq!(through.checked, Err(changed));
    assert_eq!(opened, 0);
}

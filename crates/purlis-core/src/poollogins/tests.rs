// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dockerpool::{ContainerId, Image, Made, Runner, User};
use crate::runners::Set;

fn bound(pool: &str, definition: &str, image: &str) -> Bound {
    Bound {
        pool: pool.into(),
        definition: definition.into(),
        image: image.into(),
        runner: "01JB0RUNNER0000000000000000".into(),
        runner_key: "ab".repeat(32),
        runner_image: image.into(),
        engine: "this machine's Docker (/var/run/docker.sock)".into(),
    }
}

#[test]
fn nobody_is_opted_in_until_the_person_says() {
    let opt_ins = OptIns::default();
    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("builds", "d1", "i1")),
        Standing::Unasked
    );
}

#[test]
fn an_opt_in_holds_for_its_personas_login_on_its_pool_as_it_was() {
    let opt_ins = OptIns::default();
    opt_ins.allow("steward", "id-shop", bound("builds", "d1", "i1"));

    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("builds", "d1", "i1")),
        Standing::Allowed
    );
    assert_eq!(
        opt_ins.standing("steward", "id-bank", &bound("builds", "d1", "i1")),
        Standing::Unasked,
        "another login of the persona is asked apart"
    );
    assert_eq!(
        opt_ins.standing("reviewer", "id-shop", &bound("builds", "d1", "i1")),
        Standing::Unasked,
        "another persona is asked apart"
    );
    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("tests", "d1", "i1")),
        Standing::Unasked,
        "another pool is asked apart"
    );
}

#[test]
fn a_changed_image_definition_or_runner_asks_again() {
    let opt_ins = OptIns::default();
    opt_ins.allow("steward", "id-shop", bound("builds", "d1", "i1"));
    let paired_again = Bound {
        runner: "01JB0OTHER00000000000000000".into(),
        runner_key: "cd".repeat(32),
        ..bound("builds", "d1", "i1")
    };

    for now in [
        bound("builds", "d1", "i2"),
        bound("builds", "d2", "i1"),
        paired_again,
    ] {
        assert_eq!(
            opt_ins.standing("steward", "id-shop", &now),
            Standing::Changed,
            "{now:?}"
        );
    }
}

#[test]
fn kept_off_stands_until_the_pool_changes() {
    let opt_ins = OptIns::default();
    opt_ins.keep_off("steward", "id-shop", bound("builds", "d1", "i1"));

    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("builds", "d1", "i1")),
        Standing::KeptOff
    );
    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("builds", "d1", "i2")),
        Standing::Changed
    );
}

#[test]
fn a_runner_is_shown_to_be_yours_only_on_this_machines_docker() {
    let named = owned(Ok("/var/run/docker.sock".into())).unwrap();
    let refused = owned(Err(
        "this machine's DOCKER_HOST is tcp://build-box.example:2376, an engine purlis cannot \
         tell is on this machine"
            .into(),
    ))
    .unwrap_err();

    assert_eq!(named, "this machine's Docker");
    assert!(
        refused.contains("tcp://build-box.example:2376"),
        "{refused}"
    );
    assert!(refused.contains("sends no vault value there"), "{refused}");
}

fn image(hex: char) -> Image {
    Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        hex.to_string().repeat(64)
    ))
    .unwrap()
}

fn kept(device: &str, key: u8, of: Image) -> Kept {
    Kept {
        made: Made {
            container: ContainerId::new(&"e".repeat(64)).unwrap(),
            runner: Runner::new(
                "builds",
                of,
                User::new(10001, 10001).unwrap(),
                "/home/purlis",
            )
            .unwrap(),
        },
        pinned: purlis_session_protocol::hostlink::Pinned {
            device: device.into(),
            key: purlis_session_protocol::hostlink::PublicKey::from_bytes([key; 32]),
        },
        paired: 0,
        of: None,
        draining: false,
    }
}

#[test]
fn a_pools_bound_follows_its_image_its_limits_and_its_runner() {
    let one = Pool::new("builds", image('a'), Set::default()).unwrap();
    let moved = Pool::new("builds", image('b'), Set::default()).unwrap();
    let limited = Pool::new(
        "builds",
        image('a'),
        Set {
            memory_mib: Some(8192),
            ..Set::default()
        },
    )
    .unwrap();
    let runner = kept("01JB0RUNNER0000000000000000", 1, image('a'));
    let again = kept("01JB0OTHER00000000000000000", 2, image('a'));
    let engine = "this machine's Docker (/var/run/docker.sock)";

    let a = Bound::of(&one, &runner, engine);
    assert_eq!(
        a,
        Bound::of(&one, &runner, engine),
        "the same pool, the same bound"
    );
    assert_eq!(a.image, "a".repeat(64));
    assert_eq!(a.runner_key, "01".repeat(32));
    assert_ne!(a.definition, Bound::of(&moved, &runner, engine).definition);
    assert_ne!(
        a.definition,
        Bound::of(&limited, &runner, engine).definition
    );
    assert_ne!(a, Bound::of(&one, &again, engine), "a runner paired again");
}

/// A pool `first` that moved on to a new runner, its runner before kept draining (#2749).
fn a_pool_with_a_runner_draining() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let image = format!("ghcr.io/purlis/purlis-runner@sha256:{}", "a".repeat(64));
    let row = |name: &str, device: &str, extra: serde_json::Value| {
        let mut row = serde_json::json!({
            "name": name,
            "pool": "docker",
            "container": "e".repeat(64),
            "image": image,
            "user": "10001:10001",
            "home": "/home/purlis",
            "device": device,
            "key": "12".repeat(32),
            "paired": 1,
        });
        for (key, value) in extra.as_object().unwrap() {
            row[key] = value.clone();
        }
        row
    };
    let text = serde_json::json!({
        "version": 1,
        "pools": [{ "name": "first", "kind": "docker", "image": image }],
        "runners": [
            row("first", "01JB0DRAINING000000000000000", serde_json::json!({ "draining": true })),
            row("first-01234567", "01JB0NEW00000000000000000000", serde_json::json!({ "of": "first" })),
        ],
    });
    std::fs::write(&path, text.to_string()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    config
}

/// A chat that stays on a draining runner is reached over a link its host says reached
/// `<pool>/<runner>` (#3417): it is judged by its pool and that runner as they are kept, by the
/// same check as any runner chat, never by the pool's runner now.
#[test]
fn a_chat_on_a_link_relayed_before_its_pool_moved_on_is_judged_by_the_runner_it_reached() {
    let config = a_pool_with_a_runner_draining();
    let here = || Ok("/var/run/docker.sock".into());

    let draining =
        Bound::over_the_link_with(here(), config.path(), "first", Some("first/first")).unwrap();
    let new =
        Bound::over_the_link_with(here(), config.path(), "first", Some("first/first-01234567"))
            .unwrap();

    assert_eq!(draining.pool, "first", "its pool, by its own name");
    assert_eq!(draining.runner, "01JB0DRAINING000000000000000");
    assert_eq!(new.runner, "01JB0NEW00000000000000000000");
    assert_eq!(draining.definition, new.definition, "the one pool");
}

#[test]
fn a_link_whose_host_named_no_runner_is_refused_while_its_pool_has_one_draining() {
    let config = a_pool_with_a_runner_draining();
    let refused = Bound::over_the_link_with(
        Ok("/var/run/docker.sock".into()),
        config.path(),
        "first",
        None,
    )
    .unwrap_err();
    assert!(refused.contains("did not say which"), "{refused}");
}

#[test]
fn a_link_whose_host_named_no_runner_is_the_pools_runner_while_none_drains() {
    let config = a_pool_with_a_runner_draining();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
    let mut text: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    text["runners"].as_array_mut().unwrap().remove(0);
    std::fs::write(&path, text.to_string()).unwrap();

    let bound = Bound::over_the_link_with(
        Ok("/var/run/docker.sock".into()),
        config.path(),
        "first",
        None,
    )
    .unwrap();
    assert_eq!(bound.runner, "01JB0NEW00000000000000000000");
}

#[test]
fn a_runner_reached_that_is_not_kept_or_not_the_pools_or_not_yours_is_refused() {
    let config = a_pool_with_a_runner_draining();
    for reached in ["first/none", "other/first", "first", "other"] {
        assert!(
            Bound::over_the_link_with(
                Ok("/var/run/docker.sock".into()),
                config.path(),
                "first",
                Some(reached)
            )
            .is_err(),
            "{reached}"
        );
    }
    assert!(
        Bound::over_the_link_with(
            Ok("/var/run/docker.sock".into()),
            config.path(),
            "none",
            Some("none/first")
        )
        .is_err(),
        "a pool not kept"
    );
    let refused = Bound::over_the_link_with(
        Err("this machine's DOCKER_HOST is tcp://build-box.example:2376".into()),
        config.path(),
        "first",
        Some("first/first"),
    )
    .unwrap_err();
    assert!(refused.contains("sends no vault value there"), "{refused}");
}

/// Rewrites the runners file of `config` by `change`.
fn change_runners(config: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let path = crate::machine::dir(config).join(crate::machine::RUNNERS_FILE);
    let mut text: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    change(&mut text);
    std::fs::write(&path, text.to_string()).unwrap();
}

/// A move's pool that is not ready says which way it is not (#3646), and one whose runners are
/// not shown to be yours says that, apart from the rest.
#[test]
fn a_pool_not_ready_for_a_new_chat_says_which_way_it_is_not() {
    let config = a_pool_with_a_runner_draining();
    let here = || Ok("/var/run/docker.sock".into());

    let bound = Bound::read_with(here(), config.path(), "first").unwrap();
    assert_eq!(
        bound.runner, "01JB0NEW00000000000000000000",
        "never the draining one"
    );

    assert_eq!(
        Bound::read_with(here(), config.path(), "none"),
        Err(Unbound::NotHere)
    );
    assert!(matches!(
        Bound::read_with(
            Err("this machine's DOCKER_HOST is tcp://build-box.example:2376".into()),
            config.path(),
            "first"
        ),
        Err(Unbound::NotYours(why)) if why.contains("tcp://build-box.example:2376")
    ));

    // Its new runner gone: only the draining one is left.
    change_runners(config.path(), |text| {
        text["runners"].as_array_mut().unwrap().remove(1);
    });
    assert_eq!(
        Bound::read_with(here(), config.path(), "first"),
        Err(Unbound::Draining)
    );

    // No runner at all of its image.
    change_runners(config.path(), |text| {
        text["runners"].as_array_mut().unwrap().clear();
    });
    assert_eq!(
        Bound::read_with(here(), config.path(), "first"),
        Err(Unbound::NoRunnerOfItsImage)
    );
}

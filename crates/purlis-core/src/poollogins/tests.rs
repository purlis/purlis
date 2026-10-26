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
    assert!(
        refused.contains("fills no browser login there"),
        "{refused}"
    );
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

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
        pod: String::new(),
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
fn a_pools_answers_are_forgotten_with_its_mark() {
    let opt_ins = OptIns::default();
    opt_ins.allow("steward", "id-shop", bound("builds", "d1", "i1"));
    opt_ins.allow("steward", "id-shop", bound("tests", "d1", "i1"));
    opt_ins.forget_pool("builds");
    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("builds", "d1", "i1")),
        Standing::Unasked
    );
    assert_eq!(
        opt_ins.standing("steward", "id-shop", &bound("tests", "d1", "i1")),
        Standing::Allowed
    );
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
        carried: false,
        carry_failed: false,
        unpaired: false,
        signed_out: Vec::new(),
        last_stop: None,
        held: false,
        engine: None,
        home_created: None,
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

/// **A chat that ran without the sandbox on a Docker pool's runner bars tools and tokens there**,
/// as it bars browser logins, whichever runner of the pool the link reached.
#[test]
fn a_docker_runner_a_chat_ran_on_without_the_sandbox_is_sent_nothing() {
    let config = a_pool_with_a_runner_draining();
    let here = || Ok("/var/run/docker.sock".into());
    let reached = Some("first/first-01234567");
    Bound::over_the_link_with(here(), config.path(), "first", reached)
        .expect("nothing has run there unconfined");
    crate::runnertaint::mark_pool(config.path(), "first").unwrap();
    let refused = Bound::over_the_link_with(here(), config.path(), "first", reached).unwrap_err();
    assert!(refused.contains("without the sandbox"), "{refused}");
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

    // A file that cannot be read says so, never to remove the pool (#3814).
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
    std::fs::write(&path, "not json").unwrap();
    assert!(matches!(
        Bound::read_with(here(), config.path(), "first"),
        Err(Unbound::Unread(_))
    ));
}

/// **A pool on another machine is never sent a vault value** (#6266, ADR 0123 §5): whatever this
/// machine's own engine is, until the person's mark (#6271). Its chats still move there, as a
/// Docker pool's do (§1).
#[test]
fn a_pool_on_another_machine_is_sent_no_vault_value_and_chats_still_move_there() {
    let config = a_pool_with_a_runner_draining();
    change_runners(config.path(), |text| {
        let pool = &mut text["pools"][0];
        pool["kind"] = "remote-docker".into();
        pool["route"] = "ssh://me@build-box/var/run/docker.sock".into();
        pool["pin"] = serde_json::json!({
            "hostKey": "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
            "engine": "ABCD",
        });
    });
    let sent = Bound::over_the_link_with(
        Ok("/var/run/docker.sock".into()),
        config.path(),
        "first",
        None,
    );
    assert!(
        matches!(&sent, Err(why) if why.contains("on another machine") && why.contains("no vault value")),
        "{sent:?}"
    );
    assert!(Bound::read_with(Ok("/var/run/docker.sock".into()), config.path(), "first").is_ok());
}

const HOST_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";

/// **A pool `name` on another machine, reached over SSH and pinned to engine `engine`**, its one
/// runner `<name>/<name>`, marked owned for that pin where `owned` (#6271).
fn a_pool_on_another_machine(name: &str, engine: &str, owned: bool) -> tempfile::TempDir {
    let config = a_pool_with_a_runner_draining();
    change_runners(config.path(), |text| {
        let pool = &mut text["pools"][0];
        pool["name"] = name.into();
        pool["kind"] = "remote-docker".into();
        pool["route"] = "ssh://me@build-box/var/run/docker.sock".into();
        pool["pin"] = serde_json::json!({ "hostKey": HOST_KEY, "engine": engine });
        if owned {
            pool["owned"] = pool["pin"].clone();
        }
        let runners = text["runners"].as_array_mut().unwrap();
        runners.remove(1);
        runners[0]["name"] = name.into();
        runners[0].as_object_mut().unwrap().remove("draining");
    });
    config
}

/// **A marked pool on another machine's runner may be sent a value, bound to its pin** (#6271,
/// ADR 0123 §5): whatever this machine's own engine is, since it is not the one its runner runs
/// on, and the Ask names the machine as pinned.
#[test]
fn a_marked_pool_on_another_machine_is_bound_to_its_pin_whatever_this_machines_engine_is() {
    let config = a_pool_on_another_machine("marked-box", "ABCD", true);
    let reached = Some("marked-box/marked-box");
    let bound = Bound::over_the_link_with(
        Err("purlis finds no Docker engine on this machine".to_owned()),
        config.path(),
        "marked-box",
        reached,
    )
    .unwrap();
    let key = crate::sshroute::HostKey::parse(HOST_KEY).unwrap();
    assert!(bound.engine.contains("build-box"), "{}", bound.engine);
    assert!(bound.engine.contains(&key.fingerprint()), "{}", bound.engine);
    assert!(bound.engine.contains("ABCD"), "{}", bound.engine);
    assert_eq!(bound.runner, "01JB0DRAINING000000000000000");
}

/// **The opt-in is bound to the pin** (#6271): the same pool and runner pinned to another engine
/// is another bound, so an opt-in given before a pin change asks again.
#[test]
fn a_pool_on_another_machine_pinned_again_is_another_bound() {
    let one = a_pool_on_another_machine("pinned-box", "ABCD", true);
    let two = a_pool_on_another_machine("pinned-box", "WXYZ", true);
    let bound = |config: &tempfile::TempDir| {
        Bound::over_the_link_with(
            Ok("/var/run/docker.sock".into()),
            config.path(),
            "pinned-box",
            Some("pinned-box/pinned-box"),
        )
        .unwrap()
    };
    let (one, two) = (bound(&one), bound(&two));
    assert_eq!(one.runner, two.runner);
    assert_ne!(one.engine, two.engine);
    let opt_ins = OptIns::default();
    opt_ins.allow("steward", "forge", one);
    assert_eq!(opt_ins.standing("steward", "forge", &two), Standing::Changed);
}

/// **An unmarked pool on another machine is sent nothing, and the chat is told why and how the
/// person marks it** (#6271): not yours, never to be asked again for now.
#[test]
fn an_unmarked_pool_on_another_machine_is_sent_nothing_and_says_how_to_mark_it() {
    let config = a_pool_on_another_machine("unmarked-box", "ABCD", false);
    let sent = Bound::over_the_link_with_kinded(
        Ok("/var/run/docker.sock".into()),
        config.path(),
        "unmarked-box",
        Some("unmarked-box/unmarked-box"),
    );
    let Err(NotSent::NotYours(why)) = &sent else {
        panic!("{sent:?}");
    };
    assert!(why.contains("another machine"), "{why}");
    assert!(why.contains("no vault value"), "{why}");
    assert!(why.contains("This is my own"), "{why}");
}

/// **A marked pool whose pin no longer holds is sent nothing** (#6270, #6271): the change found
/// by a run, until the person trusts the machine again.
#[test]
fn a_marked_pool_on_another_machine_whose_pin_changed_is_sent_nothing() {
    let config = a_pool_on_another_machine("changed-box", "ABCD", true);
    let sent = || {
        Bound::over_the_link_with_kinded(
            Ok("/var/run/docker.sock".into()),
            config.path(),
            "changed-box",
            Some("changed-box/changed-box"),
        )
    };
    assert!(sent().is_ok());
    let key = crate::sshroute::HostKey::parse(HOST_KEY).unwrap();
    let other = crate::sshroute::HostKey::parse(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB4FjtV1Gw3x8aKkx0vZ6m1XzjF7gZbJ0sqvM2Wl3e7k",
    )
    .unwrap();
    crate::poolsetup::pinchange::found(
        "changed-box",
        &crate::sshroute::connect::Finding::KeyChanged {
            host: "build-box".into(),
            pinned: key,
            now: other,
        },
    );
    let refused = sent();
    crate::poolsetup::pinchange::holds("changed-box");
    let Err(NotSent::NotYours(why)) = &refused else {
        panic!("{refused:?}");
    };
    assert!(why.contains("no vault value"), "{why}");
    assert!(sent().is_ok(), "trusted again");
}

/// **A socket forwarded from another machine by anything but this kind is not this machine's**
/// (#6271, ADR 0123 §5): a Docker pool on the window's engine stays refused where that engine is
/// `ssh://` or `tcp://`, however a pool on another machine is marked.
#[test]
fn a_local_pool_on_an_engine_elsewhere_is_refused_though_a_pool_on_another_machine_is_marked() {
    let config = a_pool_with_a_runner_draining();
    crate::runners::record_pool(
        config.path(),
        &Pool::new("marked-elsewhere", image('a'), Set::default())
            .unwrap()
            .on(Some(crate::runners::Remote {
                route: crate::sshroute::Route::parse("ssh://me@build-box/var/run/docker.sock")
                    .unwrap(),
                pin: crate::sshroute::Pin {
                    host_key: crate::sshroute::HostKey::parse(HOST_KEY).unwrap(),
                    engine: crate::sshroute::EnginePin::Docker("ABCD".to_owned()),
                },
            })),
    )
    .unwrap();
    crate::runners::set_remote_owned(config.path(), "marked-elsewhere", true).unwrap();
    for host in ["ssh://me@build-box", "tcp://build-box:2376"] {
        let engine = crate::dockerpool::settings::engine_on_this_machine(
            Some(host),
            &crate::dockerpool::settings::Places {
                home: None,
                tmp: None,
                runtime: None,
                system: Path::new("/var/run/docker.sock"),
            },
        );
        let sent = Bound::over_the_link_with(engine, config.path(), "first", Some("first/first"));
        assert!(
            matches!(&sent, Err(why) if why.contains("no vault value")),
            "{host}: {sent:?}"
        );
    }
}

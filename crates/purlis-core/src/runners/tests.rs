// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dockerpool::tests::{FakeDocker, ID, inspected, made};
use purlis_session_protocol::hostlink::LinkKey;

/// This desktop: its device id and the public half of the link key it links with.
static OURS: std::sync::LazyLock<Pinned> = std::sync::LazyLock::new(|| Pinned {
    device: "01JA0DESKT0P00000000000000".into(),
    key: LinkKey::generate().unwrap().public(),
});

fn answered(device: &str) -> Pinned {
    Pinned {
        device: device.into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

fn other_made(name: &str, id: &str) -> Made {
    let first = made();
    Made {
        container: ContainerId::new(id).unwrap(),
        runner: Runner::new(
            name,
            first.runner.image().clone(),
            first.runner.user(),
            first.runner.home(),
        )
        .unwrap(),
    }
}

#[test]
fn a_kept_runner_is_read_back_as_it_was_paired() {
    let config = tempfile::tempdir().unwrap();
    let runner = answered("01JB0RUNNER0000000000000000");
    let kept = keep(config.path(), &made(), &runner, &OURS).unwrap();
    assert_eq!(read(config.path()).unwrap(), vec![kept.clone()]);
    assert_eq!(kept.pinned, runner);
    assert_eq!(kept.made, made());
    assert!(kept.paired > 1_700_000_000);
}

#[test]
fn the_kept_file_holds_the_ruled_keys_for_its_user_alone() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(value["version"], 1);
    let row = value["runners"][0].as_object().unwrap();
    let mut keys: Vec<&str> = row.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "container",
            "device",
            "home",
            "image",
            "key",
            "name",
            "paired",
            "pool",
            "user"
        ]
    );
    assert_eq!(row["pool"], "docker");
    assert_eq!(row["container"], ID);
    assert_eq!(row["user"], "10001:10001");
    assert_eq!(row["home"], "/home/purlis");
}

#[test]
fn a_second_answer_with_another_key_for_a_kept_runner_is_refused_and_the_pin_stays() {
    let config = tempfile::tempdir().unwrap();
    let first = answered("01JB0RUNNER0000000000000000");
    let kept = keep(config.path(), &made(), &first, &OURS).unwrap();
    for later in [
        answered("01JB0RUNNER0000000000000000"),
        answered("01JC0ANOTHER000000000000000"),
    ] {
        let refused = keep(config.path(), &made(), &later, &OURS).unwrap_err();
        assert_eq!(
            refused,
            Refused::KeyChanged {
                name: "first".into()
            }
        );
        assert!(
            refused.to_string().contains("remove the runner"),
            "{refused}"
        );
    }
    // Another container under the kept name is refused the same way.
    let elsewhere = other_made("first", &"a".repeat(64));
    assert!(matches!(
        keep(config.path(), &elsewhere, &first, &OURS),
        Err(Refused::KeyChanged { .. })
    ));
    assert_eq!(read(config.path()).unwrap(), vec![kept.clone()]);
    // The same answer again is the runner as it was kept.
    assert_eq!(keep(config.path(), &made(), &first, &OURS).unwrap(), kept);
}

#[test]
fn a_runner_answering_another_runners_or_this_machines_device_id_or_key_is_not_kept() {
    let config = tempfile::tempdir().unwrap();
    let first = answered("01JB0RUNNER0000000000000000");
    keep(config.path(), &made(), &first, &OURS).unwrap();
    let second = other_made("second", &"b".repeat(64));
    let same_key = Pinned {
        device: "01JC0SECOND0000000000000000".into(),
        key: first.key,
    };
    for refused in [
        answered("01JB0RUNNER0000000000000000"),
        same_key,
        answered(&OURS.device),
    ] {
        assert!(
            matches!(
                keep(config.path(), &second, &refused, &OURS),
                Err(Refused::Other(_))
            ),
            "{refused:?} was kept"
        );
    }
    assert_eq!(read(config.path()).unwrap().len(), 1);
}

/// A device id is a ULID, which reads the same in either case, so two spellings of one id
/// differing only in case are one id to every check [`keep`] makes (#2741).
#[test]
fn device_ids_differing_only_in_case_are_one_id() {
    let config = tempfile::tempdir().unwrap();
    let first = answered("01JB0RUNNER0000000000000000");
    let kept = keep(config.path(), &made(), &first, &OURS).unwrap();
    // The kept runner answering its own id in lower case, with its own key, is as it was kept.
    let lower = Pinned {
        device: first.device.to_ascii_lowercase(),
        key: first.key,
    };
    assert_eq!(keep(config.path(), &made(), &lower, &OURS).unwrap(), kept);
    // Another runner answering the first one's id, or this desktop's, in lower case is refused.
    let second = other_made("second", &"b".repeat(64));
    for refused in [
        answered(&first.device.to_ascii_lowercase()),
        answered(&OURS.device.to_ascii_lowercase()),
    ] {
        assert!(
            matches!(
                keep(config.path(), &second, &refused, &OURS),
                Err(Refused::Other(_))
            ),
            "{refused:?} was kept"
        );
    }
    assert_eq!(read(config.path()).unwrap(), vec![kept]);
}

/// A runner that answers with this desktop's own link key is refused, and nothing is kept
/// (#2683): the key is the one this desktop presents on its links.
#[test]
fn a_runner_answering_this_desktops_own_link_key_is_not_kept() {
    let config = tempfile::tempdir().unwrap();
    let ours_again = Pinned {
        device: "01JB0RUNNER0000000000000000".into(),
        key: OURS.key,
    };
    let refused = keep(config.path(), &made(), &ours_again, &OURS).unwrap_err();
    assert!(
        matches!(&refused, Refused::Other(why) if why.contains("this machine's own link key")),
        "{refused}"
    );
    assert!(read(config.path()).unwrap().is_empty());
}

#[test]
fn a_kept_file_of_another_shape_is_refused_and_never_replaced() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    for text in [
        "not json".to_owned(),
        serde_json::json!({ "version": 2, "runners": [] }).to_string(),
        serde_json::json!({ "version": 1, "runners": [{ "name": "first" }] }).to_string(),
    ] {
        folder
            .replace_file(crate::machine::RUNNERS_FILE, text.as_bytes())
            .unwrap();
        assert!(read(config.path()).is_err(), "{text} was read");
        assert!(
            keep(
                config.path(),
                &made(),
                &answered("01JB0RUNNER0000000000000000"),
                &OURS
            )
            .is_err()
        );
        let there =
            std::fs::read_to_string(folder.path().join(crate::machine::RUNNERS_FILE)).unwrap();
        assert_eq!(there, text, "the file was replaced");
    }
}

#[test]
fn removing_a_runner_unpairs_it_removes_its_container_and_volume_and_forgets_it() {
    let config = tempfile::tempdir().unwrap();
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let fake = FakeDocker::new(&inspected());
    let asked = std::cell::Cell::new(false);
    let unpair = |kept: &Kept| {
        assert_eq!(kept.name(), "first");
        asked.set(true);
        Ok(())
    };
    let removed = remove(config.path(), &fake.docker(), "first", Some(&unpair)).unwrap();
    assert!(asked.get());
    assert_eq!(removed.peer, Some(Ok(())));
    assert_eq!(removed.left(), None);
    assert!(removed.forgotten);
    assert!(read(config.path()).unwrap().is_empty());
    let log = fake.file("log");
    assert!(
        log.contains(&format!("container rm --force --volumes {ID}")),
        "{log}"
    );
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
}

#[test]
fn a_runner_whose_link_is_down_is_still_removed_with_its_container() {
    let config = tempfile::tempdir().unwrap();
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let fake = FakeDocker::new(&inspected());
    let removed = remove(config.path(), &fake.docker(), "first", None).unwrap();
    assert_eq!(removed.peer, None);
    assert!(removed.forgotten);
    // A peer that could not be taken out goes with the container's home.
    let failing = |_: &Kept| Err("the link ended".to_owned());
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let removed = remove(config.path(), &fake.docker(), "first", Some(&failing)).unwrap();
    assert!(removed.forgotten && removed.left().is_none(), "{removed:?}");
}

#[test]
fn a_removal_that_fails_says_what_is_left_and_keeps_the_runner_to_remove_again() {
    let config = tempfile::tempdir().unwrap();
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let fake = FakeDocker::new(&inspected());
    fake.set("rm-fails-volume", "1");
    fake.set("volumes", "purlis-runner-first-home\n");
    let removed = remove(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(!removed.forgotten);
    let left = removed.left().expect("what is left is said");
    assert!(
        left.contains("docker volume rm --force purlis-runner-first-home"),
        "{left}"
    );
    assert!(left.contains("removed again"), "{left}");
    assert_eq!(read(config.path()).unwrap().len(), 1);

    std::fs::remove_file(fake.docker().program().with_file_name("rm-fails-volume")).unwrap();
    let removed = remove(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(removed.forgotten, "{removed:?}");
}

#[test]
fn removing_a_runner_that_is_not_kept_is_refused() {
    let config = tempfile::tempdir().unwrap();
    let fake = FakeDocker::new(&inspected());
    assert!(remove(config.path(), &fake.docker(), "first", None).is_err());
    assert_eq!(fake.file("log"), "");
}

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

pub(crate) fn pool(name: &str, set: Set) -> Pool {
    Pool::new(name, Image::pinned(IMAGE).unwrap(), set).unwrap()
}

fn the_file(config: &Path) -> serde_json::Value {
    let path = crate::machine::dir(config).join(crate::machine::RUNNERS_FILE);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn a_pool_is_kept_with_the_limits_the_person_set_and_purlis_defaults_for_the_rest() {
    let config = tempfile::tempdir().unwrap();
    let set = Set {
        millicpus: Some(1500),
        memory_mib: Some(2048),
        ..Set::default()
    };
    record_pool(config.path(), &pool("builds", set)).unwrap();
    let pools = pools(config.path()).unwrap();
    assert_eq!(pools, vec![pool("builds", set)]);
    let limits = pools[0].limits();
    assert_eq!(
        (
            limits.millicpus(),
            limits.memory_mib(),
            limits.pids(),
            limits.tmp_mib()
        ),
        (1500, 2048, 1024, 1024)
    );
}

#[test]
fn a_pool_is_written_with_the_ruled_keys_and_only_the_limits_the_person_set() {
    let config = tempfile::tempdir().unwrap();
    record_pool(
        config.path(),
        &pool(
            "builds",
            Set {
                millicpus: Some(1500),
                processes: Some(512),
                ..Set::default()
            },
        ),
    )
    .unwrap();
    record_pool(config.path(), &pool("plain", Set::default())).unwrap();
    let file = the_file(config.path());
    assert_eq!(file["version"], 1);
    assert_eq!(file["runners"], serde_json::json!([]));
    assert_eq!(
        file["pools"],
        serde_json::json!([
            { "name": "builds", "kind": "docker", "image": IMAGE, "cpus": 1.5, "processes": 512 },
            { "name": "plain", "kind": "docker", "image": IMAGE },
        ])
    );
}

#[test]
fn a_pools_own_time_before_its_runner_suspends_is_kept_and_a_day_is_left_out() {
    let config = tempfile::tempdir().unwrap();
    let set = Set {
        suspend_after: Some(std::time::Duration::from_secs(90 * 60)),
        ..Set::default()
    };
    record_pool(config.path(), &pool("nightly", set)).unwrap();
    record_pool(config.path(), &pool("plain", Set::default())).unwrap();
    assert_eq!(
        the_file(config.path())["pools"],
        serde_json::json!([
            { "name": "nightly", "kind": "docker", "image": IMAGE, "suspendAfter": "90m" },
            { "name": "plain", "kind": "docker", "image": IMAGE },
        ])
    );
    let pools = pools(config.path()).unwrap();
    assert_eq!(pools[0].set(), set);
    assert_eq!(
        pools[0].suspend_after(),
        std::time::Duration::from_secs(5400)
    );
    assert_eq!(
        pools[1].suspend_after(),
        std::time::Duration::from_secs(86_400)
    );
    // A day said in so many words is the default, and left out as one.
    let a_day = Set {
        suspend_after: Some(std::time::Duration::from_secs(86_400)),
        ..Set::default()
    };
    assert_eq!(pool("daily", a_day).set(), Set::default());
}

#[test]
fn a_pools_suspend_after_is_read_as_the_person_spelled_it_in_hours() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    let text = serde_json::json!({
        "version": 1,
        "runners": [],
        "pools": [{ "name": "builds", "kind": "docker", "image": IMAGE, "suspendAfter": "12h" }],
    })
    .to_string();
    folder
        .replace_file(crate::machine::RUNNERS_FILE, text.as_bytes())
        .unwrap();
    assert_eq!(
        pools(config.path()).unwrap()[0].suspend_after(),
        std::time::Duration::from_secs(12 * 60 * 60)
    );
}

#[test]
fn a_file_with_no_pool_is_written_as_it_was_before_pools() {
    let config = tempfile::tempdir().unwrap();
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let file = the_file(config.path());
    let mut keys: Vec<&str> = file
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["runners", "version"]);
    assert!(pools(config.path()).unwrap().is_empty());
}

#[test]
fn a_pools_row_whole_cpus_and_set_limits_read_back_as_written() {
    let config = tempfile::tempdir().unwrap();
    let set = Set {
        millicpus: Some(3000),
        memory_mib: Some(8192),
        processes: Some(2048),
        tmp_mib: Some(512),
        suspend_after: Some(std::time::Duration::from_secs(48 * 60 * 60)),
        frames: Some(30),
        width: Some(1920),
        chosen: crate::poollimits::Chosen::default()
            .with(crate::poollimits::LimitId::ProfileSize, Some(256)),
    };
    record_pool(config.path(), &pool("big", set)).unwrap();
    let row = &the_file(config.path())["pools"][0];
    assert_eq!(row["cpus"], 3);
    assert_eq!((&row["frames"], &row["width"]), (&30.into(), &1920.into()));
    assert_eq!(pools(config.path()).unwrap()[0].set(), set);
}

/// #3297: a pool's browser frame limits over the link are its own, and one that sets neither
/// keeps spec #2041's 15 a second and 1280 pixels.
#[test]
fn a_pools_browsers_are_watched_over_the_link_at_its_own_frames_or_the_defaults() {
    let image = || Image::pinned(IMAGE).unwrap();
    let plain = Pool::new("plain", image(), Set::default()).unwrap();
    assert_eq!(
        plain.link_frames(),
        LinkFrames {
            frames: 15,
            width: 1280
        }
    );
    let fast = Pool::new(
        "fast",
        image(),
        Set {
            frames: Some(60),
            width: Some(3840),
            ..Set::default()
        },
    )
    .unwrap();
    assert_eq!(
        fast.link_frames(),
        LinkFrames {
            frames: 60,
            width: 3840
        }
    );
    let slow = Pool::new(
        "slow",
        image(),
        Set {
            frames: Some(1),
            ..Set::default()
        },
    )
    .unwrap();
    assert_eq!(
        slow.link_frames(),
        LinkFrames {
            frames: 1,
            width: 1280
        }
    );
    for set in [
        Set {
            frames: Some(0),
            ..Set::default()
        },
        Set {
            frames: Some(61),
            ..Set::default()
        },
        Set {
            width: Some(319),
            ..Set::default()
        },
        Set {
            width: Some(3841),
            ..Set::default()
        },
    ] {
        assert!(Pool::new("builds", image(), set).is_err(), "{set:?}");
    }
    // Its definition, which a login's opt-in is bound to, changes with them.
    assert_ne!(
        plain.definition_digest(),
        Pool::new(
            "plain",
            image(),
            Set {
                width: Some(1920),
                ..Set::default()
            }
        )
        .unwrap()
        .definition_digest()
    );
}

#[test]
fn a_file_of_an_unknown_shape_is_refused_whole_and_never_replaced() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    let row = serde_json::json!({ "name": "builds", "kind": "docker", "image": IMAGE });
    let with = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut row = row.clone();
        change(&mut row);
        serde_json::json!({ "version": 1, "runners": [], "pools": [row] }).to_string()
    };
    for text in [
        serde_json::json!({ "version": 1, "runners": [], "extra": [] }).to_string(),
        serde_json::json!({ "version": 1, "pools": [] }).to_string(),
        serde_json::json!({ "version": 1, "runners": [], "pools": {} }).to_string(),
        serde_json::json!({ "version": 1, "runners": [], "pools": [row, row] }).to_string(),
        with(&|row| row["kind"] = "podman".into()),
        with(&|row| row["owned"] = true.into()),
        with(&|row| row["mintInCluster"] = true.into()),
        with(&|row| row["image"] = "ghcr.io/purlis/purlis-runner:latest".into()),
        with(&|row| row["name"] = "Builds".into()),
        with(&|row| row["cpus"] = "2".into()),
        with(&|row| row["cpus"] = 1.0005.into()),
        with(&|row| row["cpus"] = (-1).into()),
        with(&|row| row["memory"] = 100.into()),
        with(&|row| row["memory"] = 1.5.into()),
        with(&|row| row["processes"] = 8.into()),
        with(&|row| row["tmp"] = 999_999.into()),
        with(&|row| row["suspendAfter"] = 86_400.into()),
        with(&|row| row["suspendAfter"] = "1d".into()),
        with(&|row| row["suspendAfter"] = "1h30m".into()),
        with(&|row| row["suspendAfter"] = "30s".into()),
        with(&|row| row["suspendAfter"] = "721h".into()),
        with(&|row| row["suspendAfter"] = "suspendafter".into()),
        // A price is a decimal in a string and an ISO 4217 code, and nothing else (#3491).
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": 0.12, "currency": "EUR"})),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "-1", "currency": "EUR"})),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "1e3", "currency": "EUR"})),
        with(&|row| {
            row["pricePerHour"] = serde_json::json!({"amount": "0.1234567", "currency": "EUR"})
        }),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "012", "currency": "EUR"})),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "1.", "currency": "EUR"})),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "1", "currency": "eur"})),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "1", "currency": "EURO"})),
        with(&|row| row["pricePerHour"] = serde_json::json!({"amount": "1"})),
        with(&|row| {
            row["pricePerHour"] =
                serde_json::json!({"amount": "1", "currency": "EUR", "per": "hour"});
        }),
        with(&|row| row["pricePerHour"] = "1 EUR".into()),
        // A pool's profiles are a list of profile names, each once (#4047).
        with(&|row| row["profiles"] = "work".into()),
        with(&|row| row["profiles"] = serde_json::json!([1])),
        with(&|row| row["profiles"] = serde_json::json!(["Work Things"])),
        with(&|row| row["profiles"] = serde_json::json!(["work", "work"])),
        with(&|row| row["frames"] = 0.into()),
        with(&|row| row["frames"] = "15".into()),
        with(&|row| row["width"] = 70_000.into()),
        with(&|row| row["width"] = 100.into()),
        with(&|row| {
            row.as_object_mut().unwrap().remove("kind");
        }),
    ] {
        folder
            .replace_file(crate::machine::RUNNERS_FILE, text.as_bytes())
            .unwrap();
        assert!(pools(config.path()).is_err(), "{text} was read");
        assert!(read(config.path()).is_err(), "{text} was read");
        assert!(record_pool(config.path(), &pool("other", Set::default())).is_err());
        let there =
            std::fs::read_to_string(folder.path().join(crate::machine::RUNNERS_FILE)).unwrap();
        assert_eq!(there, text, "the file was replaced");
    }
}

#[test]
fn a_pool_is_not_recorded_under_a_name_a_pool_or_a_runner_holds() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    for taken in ["builds", "first"] {
        assert!(
            record_pool(config.path(), &pool(taken, Set::default())).is_err(),
            "{taken}"
        );
    }
    assert_eq!(pools(config.path()).unwrap().len(), 1);
}

#[test]
fn a_pool_name_is_a_runner_name() {
    for usable in ["builds", "a", "pool-2", &"a".repeat(40)] {
        assert_eq!(usable_pool_name(usable), Ok(()), "{usable}");
    }
    for refused in [
        "",
        "-a",
        "a-",
        "Builds",
        "a b",
        "a_b",
        "a/b",
        &"a".repeat(41),
    ] {
        assert!(usable_pool_name(refused).is_err(), "{refused:?} was taken");
        assert!(
            Pool::new(refused, Image::pinned(IMAGE).unwrap(), Set::default()).is_err(),
            "{refused:?} was taken"
        );
    }
}

#[test]
fn a_pools_limits_are_held_to_what_a_runner_needs() {
    let image = || Image::pinned(IMAGE).unwrap();
    for set in [
        Set {
            memory_mib: Some(256),
            ..Set::default()
        },
        Set {
            processes: Some(10),
            ..Set::default()
        },
        Set {
            millicpus: Some(1),
            ..Set::default()
        },
        // A /tmp larger than the default memory it counts against.
        Set {
            tmp_mib: Some(8192),
            ..Set::default()
        },
        // Suspended before a link that drops can come back, or never in practice.
        Set {
            suspend_after: Some(std::time::Duration::from_secs(59)),
            ..Set::default()
        },
        Set {
            suspend_after: Some(std::time::Duration::from_secs(31 * 24 * 60 * 60)),
            ..Set::default()
        },
    ] {
        assert!(Pool::new("builds", image(), set).is_err(), "{set:?}");
    }
}

#[test]
fn a_workspace_reaches_only_a_runner_whose_pool_and_runner_are_both_kept() {
    let config = tempfile::tempdir().unwrap();
    // Nothing kept: no pool.
    assert!(runner_for(config.path(), "first").is_err());
    // A runner kept with no pool of its name is not a pool's runner.
    let kept = keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap();
    let refused = runner_for(config.path(), "first").unwrap_err();
    assert!(
        refused.to_string().contains("not on this machine"),
        "{refused}"
    );
    // A pool whose runner is not kept starts nothing.
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    let refused = runner_for(config.path(), "builds").unwrap_err();
    assert!(refused.to_string().contains("has no runner"), "{refused}");
    // Both kept: its runner.
    forget(config.path(), "first").unwrap();
    forget_pool(config.path(), "builds").unwrap();
    // The runner is gone, so its name is free for a pool, which reaches it once it is kept as
    // the pool's runner.
    record_pool(config.path(), &pool("first", Set::default())).unwrap();
    let locked = lock(config.path());
    keep_for_its_pool(config.path(), &made(), &kept.pinned, &OURS, &locked).unwrap();
    drop(locked);
    assert_eq!(
        runner_for(config.path(), "first").unwrap().pinned,
        kept.pinned
    );
}

#[test]
fn a_runner_is_not_kept_under_a_pools_name_but_as_the_pools_own() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("first", Set::default())).unwrap();
    let refused = keep(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
    )
    .unwrap_err();
    assert!(
        refused.to_string().contains("named as pool first"),
        "{refused}"
    );
    assert!(read(config.path()).unwrap().is_empty());
}

#[test]
fn a_pools_runner_of_another_image_is_not_its_runner() {
    let config = tempfile::tempdir().unwrap();
    let other = Image::pinned(&format!("ghcr.io/team/runner@sha256:{}", "1".repeat(64))).unwrap();
    record_pool(
        config.path(),
        &Pool::new("first", other, Set::default()).unwrap(),
    )
    .unwrap();
    let locked = lock(config.path());
    keep_for_its_pool(
        config.path(),
        &made(),
        &answered("01JB0RUNNER0000000000000000"),
        &OURS,
        &locked,
    )
    .unwrap();
    drop(locked);
    let refused = runner_for(config.path(), "first").unwrap_err();
    assert!(
        refused.to_string().contains("no runner of its image"),
        "{refused}"
    );
}

#[test]
fn a_runner_kept_of_a_pool_and_draining_reads_back_as_it_was_written() {
    let config = tempfile::tempdir().unwrap();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let row = |name: &str, extra: serde_json::Value| {
        let mut row = serde_json::json!({
            "name": name,
            "pool": "docker",
            "container": "a".repeat(64),
            "image": IMAGE,
            "user": "10001:10001",
            "home": "/home/purlis",
            "device": format!("01JB0RUNNER000000000000000{}", name.len() % 10),
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
        "pools": [{ "name": "first", "kind": "docker", "image": IMAGE }],
        "runners": [
            row("first", serde_json::json!({ "draining": true })),
            row("first-01234567", serde_json::json!({ "of": "first" })),
        ],
    });
    write_private(&path, &text);

    let kept = read(config.path()).unwrap();
    assert!(kept[0].draining && kept[0].of.is_none());
    assert_eq!(kept[0].pool(), "first");
    assert_eq!(kept[1].of.as_deref(), Some("first"));
    assert!(!kept[1].draining);
    assert_eq!(
        runner_for(config.path(), "first").unwrap().name(),
        "first-01234567"
    );
    assert_eq!(draining(config.path()).unwrap().len(), 1);

    // Written back exactly: `of` and `draining` only where they say something.
    let locked = lock(config.path());
    forget_pool_locked(config.path(), "none", &locked).unwrap();
    let held = load(config.path()).unwrap();
    write(config.path(), &held).unwrap();
    assert_eq!(the_file(config.path())["runners"], text["runners"]);
}

#[test]
fn a_runner_row_with_a_draining_that_is_not_true_or_an_unusable_pool_is_refused() {
    for extra in [
        serde_json::json!({ "draining": false }),
        serde_json::json!({ "draining": "yes" }),
        serde_json::json!({ "of": "Not A Pool" }),
        serde_json::json!({ "of": 3 }),
        serde_json::json!({ "else": 1 }),
    ] {
        let config = tempfile::tempdir().unwrap();
        let path = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut row = serde_json::json!({
            "name": "first",
            "pool": "docker",
            "container": "a".repeat(64),
            "image": IMAGE,
            "user": "10001:10001",
            "home": "/home/purlis",
            "device": "01JB0RUNNER0000000000000000",
            "key": "12".repeat(32),
            "paired": 1,
        });
        for (key, value) in extra.as_object().unwrap() {
            row[key] = value.clone();
        }
        write_private(
            &path,
            &serde_json::json!({ "version": 1, "runners": [row] }),
        );
        assert!(read(config.path()).is_err(), "{extra}");
    }
}

fn write_private(path: &Path, text: &serde_json::Value) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, text.to_string()).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

/// A pool declares its price per hour in the file, and purlis keeps it as it was written
/// through every write (#3491): its amount the decimal it was, never a float.
#[test]
fn a_pools_price_per_hour_is_read_and_kept_as_it_was_declared() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    let mut text = the_file(config.path());
    text["pools"][0]["pricePerHour"] = serde_json::json!({"amount": "0.10", "currency": "EUR"});
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path()))
        .unwrap()
        .replace_file(crate::machine::RUNNERS_FILE, text.to_string().as_bytes())
        .unwrap();
    let read = pools(config.path()).unwrap();
    let price = read[0].price().expect("a price");
    assert_eq!((price.amount(), price.currency()), ("0.10", "EUR"));
    // Another write of the file keeps it, as declared.
    record_pool(config.path(), &pool("tests", Set::default())).unwrap();
    assert_eq!(
        the_file(config.path())["pools"][0]["pricePerHour"],
        serde_json::json!({"amount": "0.10", "currency": "EUR"})
    );
    assert!(
        the_file(config.path())["pools"][1]
            .get("pricePerHour")
            .is_none(),
        "a pool that declares none is written with none"
    );
}

/// A price is not how a pool is defined: declaring one leaves its definition's digest, which
/// its logins are held to, as it was (#3491).
#[test]
fn a_pools_price_leaves_its_definition_digest_as_it_was() {
    let plain = pool("builds", Set::default());
    let priced = plain.clone().priced(Some(Price::new("3", "USD").unwrap()));
    assert_eq!(plain.definition_digest(), priced.definition_digest());
}

#[test]
fn a_price_is_a_plain_decimal_and_an_iso_4217_code() {
    for amount in ["0", "0.5", "12", "123456789.123456", "0.000001"] {
        assert!(Price::new(amount, "JPY").is_ok(), "{amount}");
    }
    for amount in [
        "",
        ".5",
        "1.",
        "01",
        "+1",
        "-1",
        "1e2",
        "1,5",
        "1234567890",
        "0.1234567",
    ] {
        assert!(Price::new(amount, "EUR").is_err(), "{amount}");
    }
    for currency in ["eur", "EU", "EURO", "E1R", ""] {
        assert!(Price::new("1", currency).is_err(), "{currency}");
    }
}

/// A price typed by hand that is not one refuses the whole file, as any other bad value does,
/// and says which pool's price it is and how to write it (#3491).
#[test]
fn a_bad_price_refuses_the_file_and_says_which_pool_and_how_to_write_it() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    for (price, said) in [
        (
            serde_json::json!({"amount": 0.40, "currency": "EUR"}),
            "the amount in a string",
        ),
        (
            serde_json::json!({"amount": "0,40", "currency": "EUR"}),
            "\"0,40\" is not a price per hour",
        ),
        (
            serde_json::json!({"amount": "0.40", "currency": "euro"}),
            "ISO 4217",
        ),
    ] {
        let mut text = the_file(config.path());
        text["pools"][0]["pricePerHour"] = price;
        folder
            .replace_file(crate::machine::RUNNERS_FILE, text.to_string().as_bytes())
            .unwrap();
        let why = pools(config.path()).unwrap_err().to_string();
        assert!(why.contains("the pricePerHour of pool builds"), "{why}");
        assert!(why.contains(said), "{why}");
        assert!(read(config.path()).is_err(), "the whole file is refused");
        // Put back as purlis writes it, for the next case.
        text["pools"][0]
            .as_object_mut()
            .unwrap()
            .remove("pricePerHour");
        folder
            .replace_file(crate::machine::RUNNERS_FILE, text.to_string().as_bytes())
            .unwrap();
    }
}

#[test]
fn a_cluster_pools_row_of_another_shape_refuses_the_file_whole_and_never_replaces_it() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    let key = format!("ed25519:{}", "ab".repeat(32));
    let pool_row = serde_json::json!({
        "name": "team", "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
        "context": "team", "pool": "default", "namespace": "ada", "operatorKey": key,
        "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
        "holds": ["/opt/homebrew/bin/kubectl"],
    });
    let runner_row = serde_json::json!({
        "name": "team", "pool": "cluster", "namespace": "ada", "claim": "team-1",
        "claimUid": "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60", "requester": "ada@example.com",
        "pod": "team-1", "device": "01JB0RUNNER0000000000000000", "key": "4d".repeat(32),
        "paired": 1_800_000_000_u64,
    });
    let file = |pool: &serde_json::Value, runner: &serde_json::Value| {
        serde_json::json!({ "version": 1, "runners": [runner], "pools": [pool] }).to_string()
    };
    // As purlis writes it, it is read.
    folder
        .replace_file(
            crate::machine::RUNNERS_FILE,
            file(&pool_row, &runner_row).as_bytes(),
        )
        .unwrap();
    let (cluster, kept) = cluster_runner_for(config.path(), "team").unwrap().unwrap();
    assert_eq!(cluster.operator().spelled(), key);
    // A pool kept with its key alone, before its cluster was pinned (#3305), is read, and
    // reaches no runner: it is to be added again.
    let mut keyed_alone = pool_row.clone();
    keyed_alone.as_object_mut().unwrap().remove("cluster");
    folder
        .replace_file(
            crate::machine::RUNNERS_FILE,
            file(&keyed_alone, &runner_row).as_bytes(),
        )
        .unwrap();
    assert_eq!(cluster_pools(config.path()).unwrap().len(), 1);
    let refused = cluster_runner_for(config.path(), "team").unwrap_err();
    assert!(refused.to_string().contains("add it again"), "{refused}");
    assert_eq!(kept.pod, "team-1");
    let pool_with = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut row = pool_row.clone();
        change(&mut row);
        file(&row, &runner_row)
    };
    let runner_with = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut row = runner_row.clone();
        change(&mut row);
        file(&pool_row, &row)
    };
    for text in [
        pool_with(&|row| row["kubeconfig"] = "kube/config".into()),
        pool_with(&|row| row["context"] = "--server=evil".into()),
        pool_with(&|row| row["pool"] = "Default".into()),
        pool_with(&|row| row["namespace"] = "a.b".into()),
        pool_with(&|row| row["operatorKey"] = "ab".repeat(32).into()),
        pool_with(&|row| row["cluster"] = "kube-system".into()),
        pool_with(&|row| row["holds"] = serde_json::json!(["kubectl"])),
        pool_with(&|row| {
            row.as_object_mut().unwrap().remove("holds");
        }),
        pool_with(&|row| row["image"] = IMAGE.into()),
        pool_with(&|row| {
            row.as_object_mut().unwrap().remove("operatorKey");
        }),
        runner_with(&|row| row["name"] = "other".into()),
        runner_with(&|row| row["pod"] = "pod/team-1".into()),
        runner_with(&|row| row["claimUid"] = "1".into()),
        runner_with(&|row| row["container"] = ID.into()),
        runner_with(&|row| {
            row.as_object_mut().unwrap().remove("requester");
        }),
    ] {
        folder
            .replace_file(crate::machine::RUNNERS_FILE, text.as_bytes())
            .unwrap();
        assert!(pools(config.path()).is_err(), "{text} was read");
        assert!(cluster_pools(config.path()).is_err(), "{text} was read");
        assert!(record_pool(config.path(), &pool("other", Set::default())).is_err());
        let there =
            std::fs::read_to_string(folder.path().join(crate::machine::RUNNERS_FILE)).unwrap();
        assert_eq!(there, text, "the file was replaced");
    }
}

#[test]
fn every_cluster_pools_programs_are_listed_once() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    let row = |name: &str, holds: serde_json::Value| {
        serde_json::json!({
            "name": name, "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
            "context": "team", "pool": "default", "namespace": "ada",
            "operatorKey": format!("ed25519:{}", "ab".repeat(32)),
            "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b", "holds": holds,
        })
    };
    let file = serde_json::json!({ "version": 1, "runners": [], "pools": [
        row("team", serde_json::json!(["/opt/homebrew/bin/kubectl", "/opt/homebrew/bin/gke-gcloud-auth-plugin"])),
        row("lab", serde_json::json!(["/opt/homebrew/bin/kubectl"])),
    ]});
    folder
        .replace_file(crate::machine::RUNNERS_FILE, file.to_string().as_bytes())
        .unwrap();
    assert_eq!(
        cluster_holds(config.path()),
        [
            std::path::PathBuf::from("/opt/homebrew/bin/kubectl"),
            std::path::PathBuf::from("/opt/homebrew/bin/gke-gcloud-auth-plugin"),
        ]
    );
}

/// **A pool keeps the profiles chosen for it** (#4047, `profiles`, ruled 2026-10-30): read and
/// written as they were chosen, in their order; a pool that names none is written with none.
#[test]
fn a_pools_profiles_are_read_and_kept_as_they_were_chosen() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    record_pool(config.path(), &pool("tests", Set::default())).unwrap();

    set_pool_profiles(
        config.path(),
        "builds",
        Some(vec!["work".to_owned(), "claude".to_owned()]),
    )
    .unwrap();

    assert_eq!(
        the_file(config.path())["pools"][0]["profiles"],
        serde_json::json!(["work", "claude"])
    );
    assert!(
        the_file(config.path())["pools"][1]
            .get("profiles")
            .is_none()
    );
    let read = pools(config.path()).unwrap();
    assert_eq!(
        read[0].profiles(),
        Some(&["work".to_owned(), "claude".to_owned()][..])
    );
    assert_eq!(read[1].profiles(), None);
    // Set back to none, the key goes.
    set_pool_profiles(config.path(), "builds", None).unwrap();
    assert!(
        the_file(config.path())["pools"][0]
            .get("profiles")
            .is_none()
    );
}

/// A pool that names its profiles keeps exactly those; one that names none keeps the profile a
/// new chat in the open project gets, and with no such profile, none (#4047).
#[test]
fn a_pool_keeps_its_named_profiles_or_else_the_projects_default() {
    let plain = pool("builds", Set::default());
    assert_eq!(plain.profiles_kept(Some("work")), vec!["work".to_owned()]);
    assert_eq!(plain.profiles_kept(None), Vec::<String>::new());
    let named = plain.keeping_profiles(Some(vec!["home".to_owned()]));
    assert_eq!(named.profiles_kept(Some("work")), vec!["home".to_owned()]);
    let none = pool("builds", Set::default()).keeping_profiles(Some(Vec::new()));
    assert_eq!(none.profiles_kept(Some("work")), Vec::<String>::new());
}

/// **Adding and removing a profile change the pool's record** (#4047): the first change of a
/// pool that named none starts from the project's default, so adding another keeps it and
/// removing it leaves an empty list, never the default again.
#[test]
fn adding_and_removing_a_profile_start_from_the_default_and_change_the_record() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    let named = |config: &Path| pools(config).unwrap()[0].profiles().map(<[String]>::to_vec);

    add_pool_profile(config.path(), "builds", "home", Some("work")).unwrap();
    assert_eq!(
        named(config.path()),
        Some(vec!["work".to_owned(), "home".to_owned()])
    );
    add_pool_profile(config.path(), "builds", "home", Some("work")).unwrap();
    assert_eq!(named(config.path()).unwrap().len(), 2, "kept once");

    remove_pool_profile(config.path(), "builds", "work", Some("work")).unwrap();
    remove_pool_profile(config.path(), "builds", "home", Some("work")).unwrap();
    assert_eq!(named(config.path()), Some(Vec::new()));
    assert_eq!(
        pools(config.path()).unwrap()[0].profiles_kept(Some("work")),
        Vec::<String>::new()
    );

    let refused = add_pool_profile(config.path(), "nowhere", "home", None).unwrap_err();
    assert!(
        refused.to_string().contains("no pool named nowhere"),
        "{refused}"
    );
    let refused = add_pool_profile(config.path(), "builds", "Not A Name", None).unwrap_err();
    assert!(
        refused.to_string().contains("not a profile's name"),
        "{refused}"
    );
}

/// **A pool that names none is given its profiles once** (#4060); one that names its own
/// keeps them as they are.
#[test]
fn a_pool_that_names_none_is_given_its_profiles_and_one_that_names_its_own_keeps_them() {
    let config = tempfile::tempdir().unwrap();
    record_pool(config.path(), &pool("builds", Set::default())).unwrap();
    let named = |config: &Path| pools(config).unwrap()[0].profiles().map(<[String]>::to_vec);

    name_pool_profiles_where_none(config.path(), "builds", vec!["work".to_owned()]).unwrap();
    assert_eq!(named(config.path()), Some(vec!["work".to_owned()]));

    name_pool_profiles_where_none(config.path(), "builds", vec!["home".to_owned()]).unwrap();
    assert_eq!(named(config.path()), Some(vec!["work".to_owned()]));
}

/// The profiles a pool keeps are not how it is defined: choosing them leaves its definition's
/// digest as it was, as a price does.
#[test]
fn a_pools_profiles_leave_its_definition_digest_as_it_was() {
    let plain = pool("builds", Set::default());
    let named = plain
        .clone()
        .keeping_profiles(Some(vec!["work".to_owned()]));
    assert_eq!(plain.definition_digest(), named.definition_digest());
}

/// A cluster pool as purlis writes it, with its one runner, in `config`'s `runners.json`.
fn a_cluster_pool_in(config: &Path) {
    let folder = purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config)).unwrap();
    let file = serde_json::json!({
        "version": 1,
        "pools": [{
            "name": "team", "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
            "context": "team", "pool": "default", "namespace": "ada",
            "operatorKey": format!("ed25519:{}", "ab".repeat(32)),
            "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
            "holds": ["/opt/homebrew/bin/kubectl"],
        }],
        "runners": [{
            "name": "team", "pool": "cluster", "namespace": "ada", "claim": "team-1",
            "claimUid": "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60", "requester": "ada@example.com",
            "pod": "team-1", "device": "01JB0RUNNER0000000000000000", "key": "4d".repeat(32),
            "paired": 1_800_000_000_u64,
        }],
    });
    folder
        .replace_file(crate::machine::RUNNERS_FILE, file.to_string().as_bytes())
        .unwrap();
}

/// **A cluster pool keeps the profiles chosen for it, as a Docker pool does** (#3819, `profiles`
/// in its record, ruled 2026-10-30): the same key, read and written the same way, changed by the
/// same adds and removes, and a list of another shape refuses the file.
#[test]
fn a_cluster_pools_profiles_are_kept_in_its_record_as_a_docker_pools_are() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool_in(config.path());
    assert_eq!(profiles_of(config.path(), "team").unwrap(), None);
    assert_eq!(
        profiles_kept_of(config.path(), "team", Some("work")).unwrap(),
        vec!["work".to_owned()]
    );

    add_pool_profile(config.path(), "team", "home", Some("work")).unwrap();
    assert_eq!(
        the_file(config.path())["pools"][0]["profiles"],
        serde_json::json!(["work", "home"])
    );
    assert_eq!(
        profiles_of(config.path(), "team").unwrap(),
        Some(vec!["work".to_owned(), "home".to_owned()])
    );
    // What else the row holds stays as it was: the runner is still reached.
    assert!(cluster_runner_for(config.path(), "team").unwrap().is_some());

    remove_pool_profile(config.path(), "team", "work", Some("work")).unwrap();
    assert_eq!(
        profiles_kept_of(config.path(), "team", Some("work")).unwrap(),
        vec!["home".to_owned()]
    );
    set_pool_profiles(config.path(), "team", None).unwrap();
    assert!(
        the_file(config.path())["pools"][0]
            .get("profiles")
            .is_none()
    );

    let refused = profiles_of(config.path(), "nowhere").unwrap_err();
    assert!(
        refused.to_string().contains("no pool named nowhere"),
        "{refused}"
    );

    // A list of another shape is no list, and the file is refused whole.
    let mut text = the_file(config.path());
    text["pools"][0]["profiles"] = serde_json::json!(["work", "work"]);
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path()))
        .unwrap()
        .replace_file(crate::machine::RUNNERS_FILE, text.to_string().as_bytes())
        .unwrap();
    assert!(cluster_pools(config.path()).is_err());
}

/// **A pool's limit is the person's once set, follows purlis's default while unset, and changes
/// nothing about the pool's runner** (spec #4275, Q11, Q13), on either kind of pool's row.
#[test]
fn a_pools_limit_is_saved_marked_as_the_persons_and_makes_no_new_runner() {
    use crate::poollimits::LimitId::ProfileFiles;
    let config = tempfile::tempdir().unwrap();
    record_pool(
        config.path(),
        &Pool::new("builds", Image::pinned(IMAGE).unwrap(), Set::default()).unwrap(),
    )
    .unwrap();
    let before = pools(config.path()).unwrap()[0].definition_digest();
    let chosen = |config: &Path| chosen_limits_of(config, "builds").unwrap();
    assert_eq!(chosen(config.path()).get(ProfileFiles), None);
    assert_eq!(chosen(config.path()).in_force(ProfileFiles), 8192);

    set_pool_limit(config.path(), "builds", ProfileFiles, Some(11_264)).unwrap();

    assert_eq!(chosen(config.path()).get(ProfileFiles), Some(11_264));
    assert_eq!(
        pools(config.path()).unwrap()[0].definition_digest(),
        before,
        "it applies from the next attempt"
    );

    assert!(set_pool_limit(config.path(), "builds", ProfileFiles, Some(0)).is_err());
    assert!(set_pool_limit(config.path(), "none", ProfileFiles, Some(9000)).is_err());
    assert_eq!(chosen(config.path()).get(ProfileFiles), Some(11_264));

    set_pool_limit(config.path(), "builds", ProfileFiles, None).unwrap();
    assert_eq!(chosen(config.path()).get(ProfileFiles), None, "a reset");
}

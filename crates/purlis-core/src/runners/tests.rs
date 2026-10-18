// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dockerpool::tests::{FakeDocker, ID, inspected, made};
use purlis_session_protocol::hostlink::LinkKey;

const OURS: &str = "01JA0DESKT0P00000000000000";

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
    let kept = keep(config.path(), &made(), &runner, OURS).unwrap();
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
        OURS,
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
    let kept = keep(config.path(), &made(), &first, OURS).unwrap();
    for later in [
        answered("01JB0RUNNER0000000000000000"),
        answered("01JC0ANOTHER000000000000000"),
    ] {
        let refused = keep(config.path(), &made(), &later, OURS).unwrap_err();
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
        keep(config.path(), &elsewhere, &first, OURS),
        Err(Refused::KeyChanged { .. })
    ));
    assert_eq!(read(config.path()).unwrap(), vec![kept.clone()]);
    // The same answer again is the runner as it was kept.
    assert_eq!(keep(config.path(), &made(), &first, OURS).unwrap(), kept);
}

#[test]
fn a_runner_answering_another_runners_or_this_machines_device_id_or_key_is_not_kept() {
    let config = tempfile::tempdir().unwrap();
    let first = answered("01JB0RUNNER0000000000000000");
    keep(config.path(), &made(), &first, OURS).unwrap();
    let second = other_made("second", &"b".repeat(64));
    let same_key = Pinned {
        device: "01JC0SECOND0000000000000000".into(),
        key: first.key.clone(),
    };
    for refused in [
        answered("01JB0RUNNER0000000000000000"),
        same_key,
        answered(OURS),
    ] {
        assert!(
            matches!(
                keep(config.path(), &second, &refused, OURS),
                Err(Refused::Other(_))
            ),
            "{refused:?} was kept"
        );
    }
    assert_eq!(read(config.path()).unwrap().len(), 1);
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
                OURS
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
        OURS,
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
        OURS,
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
        OURS,
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
        OURS,
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

    std::fs::remove_file(fake.docker().0.with_file_name("rm-fails-volume")).unwrap();
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

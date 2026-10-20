// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dockerpool::tests::{FakeDocker, ID, inspected};
use crate::runners::Set;
use crate::runners::tests::pool;
use purlis_session_protocol::hostlink::LinkKey;

const OURS: &str = "01JA0DESKT0P00000000000000";

fn desktop() -> Pinned {
    Pinned {
        device: OURS.into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

fn runner_answers() -> Pinned {
    Pinned {
        device: "01JB0RUNNER0000000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

/// A Docker that makes a runner called `first` of the runner image, and pairs it as `answers`.
fn a_docker(answers: &Pinned) -> FakeDocker {
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "image",
        r#"[{"Config":{"User":"10001:10001","Env":["PATH=/usr/bin","HOME=/home/purlis"]}}]"#,
    );
    fake.set(
        "paired",
        &format!("{}\n", crate::runnerlink::to_line(answers)),
    );
    fake
}

#[test]
fn adding_a_pool_makes_its_runner_held_to_its_limits_pairs_it_and_keeps_it() {
    let config = tempfile::tempdir().unwrap();
    let (answers, ours) = (runner_answers(), desktop());
    let fake = a_docker(&answers);
    let kept = add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &ours,
    )
    .unwrap();
    assert_eq!(kept.name(), "first");
    assert_eq!(kept.pinned, answers);
    assert_eq!(kept.made.container.as_str(), ID);
    assert_eq!(runners::read(config.path()).unwrap(), vec![kept]);
    assert_eq!(runners::pools(config.path()).unwrap().len(), 1);
    let log = fake.file("log");
    assert!(log.contains("--cpus 2 --memory 4096m"), "{log}");
    assert!(log.contains("--user 10001:10001"), "{log}");
    // The runner was paired with this desktop's own pin, through the bridge.
    assert!(
        log.contains(&format!(
            "exec -i --user 10001:10001 {ID} /usr/local/bin/purlis bridge --pair"
        )),
        "{log}"
    );
    assert_eq!(fake.file("input").trim(), crate::runnerlink::to_line(&ours));
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().pinned,
        answers
    );
}

#[test]
fn a_pool_whose_runner_does_not_pair_is_taken_back_whole() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("paired", "not a pin\n");
    let refused = add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap_err();
    assert!(refused.to_string().contains("not a device id"), "{refused}");
    let log = fake.file("log");
    assert!(
        log.contains(&format!("container rm --force --volumes {ID}")),
        "{log}"
    );
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
    assert!(runners::pools(config.path()).unwrap().is_empty());
    assert!(runners::read(config.path()).unwrap().is_empty());
}

#[test]
fn a_pool_whose_runner_answers_this_desktops_own_device_id_is_taken_back() {
    let config = tempfile::tempdir().unwrap();
    let mut answers = runner_answers();
    answers.device = OURS.into();
    let fake = a_docker(&answers);
    assert!(
        add(
            config.path(),
            &fake.docker(),
            &pool("first", Set::default()),
            &desktop(),
        )
        .is_err()
    );
    assert!(fake.file("log").contains("container rm --force"));
    assert!(runners::pools(config.path()).unwrap().is_empty());
    assert!(runners::read(config.path()).unwrap().is_empty());
}

#[test]
fn a_pool_whose_image_names_no_user_makes_nothing_and_is_forgotten() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set(
        "image",
        r#"[{"Config":{"User":"","Env":["HOME=/home/purlis"]}}]"#,
    );
    let refused = add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap_err();
    assert!(refused.to_string().contains("USER"), "{refused}");
    assert!(!fake.file("log").contains("run "), "{}", fake.file("log"));
    assert!(runners::pools(config.path()).unwrap().is_empty());
}

#[test]
fn a_pool_named_as_one_kept_already_makes_nothing() {
    let config = tempfile::tempdir().unwrap();
    runners::record_pool(config.path(), &pool("first", Set::default())).unwrap();
    let fake = a_docker(&runner_answers());
    assert!(
        add(
            config.path(),
            &fake.docker(),
            &pool("first", Set::default()),
            &desktop(),
        )
        .is_err()
    );
    assert_eq!(fake.file("log"), "");
    // The pool that was there stays.
    assert_eq!(runners::pools(config.path()).unwrap().len(), 1);
}

#[test]
fn removing_a_pool_removes_its_runner_whole_and_then_the_pool() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap();
    let unpaired = std::cell::Cell::new(false);
    let unpair = |_: &Kept| {
        unpaired.set(true);
        Ok(())
    };
    let removed = remove(config.path(), &fake.docker(), "first", Some(&unpair))
        .unwrap()
        .expect("its runner was removed");
    assert!(removed.forgotten && unpaired.get());
    assert!(runners::pools(config.path()).unwrap().is_empty());
    assert!(runners::read(config.path()).unwrap().is_empty());
}

#[test]
fn a_pool_whose_runner_could_not_all_be_removed_stays_to_be_removed_again() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap();
    fake.set("rm-fails-volume", "1");
    fake.set("volumes", "purlis-runner-first-home\n");
    let removed = remove(config.path(), &fake.docker(), "first", None)
        .unwrap()
        .unwrap();
    assert!(!removed.forgotten);
    assert_eq!(runners::pools(config.path()).unwrap().len(), 1);
    std::fs::remove_file(fake.docker().0.with_file_name("rm-fails-volume")).unwrap();
    assert!(
        remove(config.path(), &fake.docker(), "first", None)
            .unwrap()
            .unwrap()
            .forgotten
    );
    assert!(runners::pools(config.path()).unwrap().is_empty());
}

#[test]
fn a_pool_with_no_runner_is_forgotten_and_one_never_added_is_refused() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    assert!(remove(config.path(), &fake.docker(), "first", None).is_err());
    runners::record_pool(config.path(), &pool("first", Set::default())).unwrap();
    assert_eq!(
        remove(config.path(), &fake.docker(), "first", None).unwrap(),
        None
    );
    assert!(runners::pools(config.path()).unwrap().is_empty());
    assert_eq!(fake.file("log"), "");
}

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

/// What the runner made beside a pool's after an update answers: a device id of its own.
fn successor_answers() -> Pinned {
    Pinned {
        device: "01JB0RUNNER0000000000000001".into(),
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
    std::fs::remove_file(fake.docker().program().with_file_name("rm-fails-volume")).unwrap();
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

#[test]
fn a_workspace_set_to_run_on_a_pool_runs_there_once_the_pool_is_kept_and_nowhere_before() {
    let config = tempfile::tempdir().unwrap();
    let plane = std::path::Path::new("/planes/a");
    crate::machine::update(config.path(), |store| {
        store.remember(plane, 1);
        store.set_chats_run_on(plane, "ide", Some("first")).unwrap();
    })
    .unwrap();
    // Another workspace, and a chat at the project's root, run here.
    assert_eq!(runs_on(config.path(), plane, Some("fleet")), Ok(None));
    assert_eq!(runs_on(config.path(), plane, None), Ok(None));
    // A pool this machine does not keep runs nothing, and says it is missing.
    let refused = runs_on(config.path(), plane, Some("ide")).unwrap_err();
    assert!(refused.contains("not on this machine"), "{refused}");
    assert_eq!(
        workspaces_on_pools(config.path(), plane),
        [("first".to_owned(), vec!["ide".to_owned()])].into()
    );
    // A pool it keeps runs the workspace's chats.
    let fake = a_docker(&runner_answers());
    add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap();
    assert_eq!(
        runs_on(config.path(), plane, Some("ide")),
        Ok(Some("first".to_owned()))
    );
    // Removing the pool runs the workspace's chats here again.
    remove(config.path(), &fake.docker(), "first", None).unwrap();
    assert_eq!(runs_on(config.path(), plane, Some("ide")), Ok(None));
    assert!(workspaces_on_pools(config.path(), plane).is_empty());
}

/// The runner image of the desktop's new version, purlis's own.
const NEW: &str = "ghcr.io/purlis/purlis-runner@sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// `fake` makes the runner `name` of `image` from now on, and pairs it as `answers`.
fn makes(fake: &FakeDocker, name: &str, image: &str, answers: &Pinned) {
    let mut inspect = inspected();
    inspect[0]["Config"]["Image"] = image.into();
    inspect[0]["Image"] = image.rsplit_once('@').unwrap().1.into();
    inspect[0]["Config"]["Labels"]["purlis.runner"] = name.into();
    inspect[0]["Mounts"][0]["Name"] = format!("purlis-runner-{name}-home").into();
    fake.set("inspect", &inspect.to_string());
    fake.set(
        "volume",
        &serde_json::json!([{
            "Name": format!("purlis-runner-{name}-home"),
            "Driver": "local",
            "Options": null,
            "Labels": null
        }])
        .to_string(),
    );
    fake.set(
        "paired",
        &format!("{}\n", crate::runnerlink::to_line(answers)),
    );
}

/// A pool of purlis's own image, added, whose runner is `first`.
fn a_pool_added(config: &Path, fake: &FakeDocker, ours: &Pinned) -> Kept {
    add(config, &fake.docker(), &pool("first", Set::default()), ours).unwrap()
}

#[test]
fn after_an_update_a_pool_gets_a_runner_of_the_desktops_version_beside_the_old_one() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    assert_eq!(successor, "first-fedcba98");
    let answers = successor_answers();
    makes(&fake, &successor, NEW, &answers);

    let kept = follow(config.path(), &fake.docker(), "first", &new_image, &ours)
        .unwrap()
        .expect("a runner made beside the old one");

    assert_eq!(kept.name(), successor);
    assert_eq!(kept.of.as_deref(), Some("first"));
    assert_eq!(kept.pinned, answers);
    // New chats start on the new runner; the old one drains, and is kept.
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().name(),
        successor
    );
    let draining = runners::draining(config.path()).unwrap();
    assert_eq!(draining.len(), 1);
    assert_eq!(draining[0].made, old.made);
    assert_eq!(
        runners::pools(config.path()).unwrap()[0].image(),
        &new_image
    );
    // Asked again, nothing more is made.
    let log = fake.file("log");
    assert!(
        follow(config.path(), &fake.docker(), "first", &new_image, &ours)
            .unwrap()
            .is_none()
    );
    assert_eq!(fake.file("log"), log);
}

#[test]
fn a_pool_of_a_teams_own_image_is_never_moved() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let team = "registry.example/team/runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut inspect = inspected();
    inspect[0]["Config"]["Image"] = team.into();
    fake.set("inspect", &inspect.to_string());
    let pool = Pool::new(
        "first",
        crate::dockerpool::Image::pinned(team).unwrap(),
        Set::default(),
    )
    .unwrap();
    add(config.path(), &fake.docker(), &pool, &ours).unwrap();
    let log = fake.file("log");

    let refused = follow(
        config.path(),
        &fake.docker(),
        "first",
        &crate::dockerpool::Image::pinned(NEW).unwrap(),
        &ours,
    )
    .unwrap_err();
    assert!(
        refused.to_string().contains("not purlis's own"),
        "{refused}"
    );
    assert_eq!(fake.file("log"), log, "nothing was made");
    assert!(runners::draining(config.path()).unwrap().is_empty());
}

#[test]
fn a_new_runner_that_does_not_pair_is_taken_back_and_the_pool_stays_as_it_was() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    makes(
        &fake,
        &crate::dockerpool::successor_name("first", &new_image),
        NEW,
        &successor_answers(),
    );
    fake.set("paired", "not a pin\n");

    assert!(follow(config.path(), &fake.docker(), "first", &new_image, &ours).is_err());

    assert!(fake.file("log").contains("container rm --force"));
    assert_eq!(runners::read(config.path()).unwrap(), vec![old.clone()]);
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().made,
        old.made
    );
}

/// **A pool's runner suspends after the pool's own time** (#3471): the runner made as it is
/// added, and the one made beside it after an update.
#[test]
fn a_pools_runners_are_made_to_suspend_after_the_pools_own_time() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let twelve_hours = Set {
        suspend_after: Some(std::time::Duration::from_secs(12 * 60 * 60)),
        ..Set::default()
    };
    add(
        config.path(),
        &fake.docker(),
        &pool("first", twelve_hours),
        &ours,
    )
    .unwrap();
    let said = |log: &str| log.matches("serve --suspends-unlinked=12h").count();
    assert_eq!(said(&fake.file("log")), 1, "{}", fake.file("log"));

    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();
    assert_eq!(said(&fake.file("log")), 2, "{}", fake.file("log"));
}

#[test]
fn only_a_draining_runner_is_drained_and_removing_the_pool_removes_every_runner_of_it() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();

    assert!(drain(config.path(), &fake.docker(), &successor, None).is_err());
    let removed = drain(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(removed.forgotten);
    assert_eq!(runners::read(config.path()).unwrap().len(), 1);

    // A pool with a runner draining beside its own is removed whole.
    makes(&fake, "first", IMAGE_OF_FIRST, &runner_answers());
    remove(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(runners::read(config.path()).unwrap().is_empty());
    assert!(runners::pools(config.path()).unwrap().is_empty());
}

const IMAGE_OF_FIRST: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The id Docker gave a container a follow left behind.
const LEFT: &str = "1eff0000000000000000000000000000000000000000000000000000000000aa";

/// What a follow that stopped after Docker made the runner `name` left: its container, labelled
/// as that runner made by `desktop`, and its home volume.
fn left_behind(fake: &FakeDocker, name: &str, desktop: &str) {
    fake.set("containers", &format!("purlis-runner-{name}\n"));
    fake.set(
        "containers-full",
        &format!("{LEFT} purlis-runner-{name} {desktop}\n"),
    );
    fake.set("volumes", &format!("purlis-runner-{name}-home\n"));
}

/// **A follow leaves no runner behind** (#2869): a container this desktop made, and the home
/// volume, of the new runner's name that a follow stopped short of keeping are removed by the
/// next one, the container by its id, which then makes the runner.
#[test]
fn a_container_and_volume_a_follow_left_behind_are_removed_by_the_next() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    left_behind(&fake, &successor, &runners::desktop_label(config.path()));

    let kept = follow(config.path(), &fake.docker(), "first", &new_image, &ours)
        .unwrap()
        .expect("the runner is made");

    assert_eq!(kept.name(), successor);
    let log = fake.file("log");
    assert!(
        log.contains(&format!("container rm --force --volumes {LEFT}")),
        "{log}"
    );
    assert!(
        log.contains(&format!("volume rm --force purlis-runner-{successor}-home")),
        "{log}"
    );
    // The runner it made carries this desktop's label.
    assert!(
        log.contains(&format!(
            "--label purlis.desktop={}",
            runners::desktop_label(config.path())
        )),
        "{log}"
    );
}

/// **Another desktop's runner on the same Docker is never taken for a leftover** (#2869): a
/// container the same in every way but made by another config home, and its home volume, stay;
/// the follow is refused as it was before.
#[test]
fn a_runner_another_desktop_made_is_left_as_it_is() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    left_behind(&fake, &successor, "0123456789abcdef");

    assert!(follow(config.path(), &fake.docker(), "first", &new_image, &ours).is_err());

    let log = fake.file("log");
    assert!(!log.contains("container rm"), "{log}");
    assert!(!log.contains("volume rm"), "{log}");
}

/// A container made before runners were labelled with their desktop keeps the refusal.
#[test]
fn a_runner_made_before_the_desktop_label_is_left_as_it_is() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    left_behind(&fake, &successor, "<no value>");

    assert!(follow(config.path(), &fake.docker(), "first", &new_image, &ours).is_err());
    assert!(!fake.file("log").contains(" rm "), "{}", fake.file("log"));
}

/// **A container or volume a kept runner claims is never removed** (#2869): a runner kept under
/// the successor's name already is the pool's, and nothing of it is cleared.
#[test]
fn what_a_kept_runner_claims_is_left_as_it_is() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();
    let before = fake.file("log");

    // Moved to the image again, as a later look would after the pool's image was put back.
    assert!(
        clear_leftovers_of(config.path(), &fake.docker(), &successor)
            .unwrap()
            .is_none()
    );
    let after = fake.file("log");
    assert_eq!(
        after, before,
        "a kept runner's container or volume was touched"
    );
}

/// **A draining runner is reached as `<pool>/<runner>` until it is removed** (#2865): a link to
/// it carries on with its chats, while the pool's name still reaches only the pool's runner of
/// this version. A runner of the pool that is not draining, or one of another pool, is not
/// reached so.
#[test]
fn a_draining_runner_is_reached_under_its_pool_until_it_is_removed() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();

    let (kept, draining) = runners::reachable(config.path(), "first").unwrap();
    assert_eq!((kept.name(), draining), (successor.as_str(), false));
    let (kept, draining) = runners::reachable(config.path(), "first/first").unwrap();
    assert_eq!((kept.name(), draining), ("first", true));
    for refused in [
        format!("first/{successor}"),
        "other/first".to_owned(),
        "first/nothing".to_owned(),
        "nothing-kept".to_owned(),
    ] {
        assert!(
            runners::reachable(config.path(), &refused).is_err(),
            "{refused}"
        );
    }

    // Once it is removed, it is reached no more.
    drain(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(runners::reachable(config.path(), "first/first").is_err());
}

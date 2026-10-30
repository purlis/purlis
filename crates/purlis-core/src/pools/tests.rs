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

/// **A follow carries no sign-in while the old runner has chats** (#4055, ruled 2026-10-30): the
/// new runner is made and paired with nothing of the old runner's home, so a sign-in is never
/// held by both; a new chat on the new runner asks to sign in there.
#[test]
fn a_follow_carries_no_sign_in_to_the_new_runner() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    std::fs::write(fake.file_path("log"), "").unwrap();

    follow(config.path(), &fake.docker(), "first", &new_image, &ours)
        .unwrap()
        .expect("moved");

    let log = fake.file("log");
    assert!(log.contains("--pair"), "{log}");
    assert!(!log.contains("-copy "), "{log}");
    assert!(!log.contains("target=/from"), "{log}");
}

/// **A draining runner's sign-ins go to the pool's runner once it has no chat left** (#4055,
/// ruled 2026-10-30): carried from its home volume to the new runner's on the Docker host, and
/// only then is it removed with its volume, so the old copy goes and one runner holds each
/// sign-in.
#[test]
fn a_drained_runner_carries_its_sign_ins_to_the_pools_runner_then_goes_with_its_home() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();
    std::fs::write(fake.file_path("log"), "").unwrap();

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    let log = fake.file("log");
    let lines: Vec<&str> = log.lines().collect();
    let at = |what: &dyn Fn(&str) -> bool| lines.iter().position(|line| what(line));
    let copying = format!("--name purlis-runner-{successor}-copy ");
    let copied = at(&|line| line.contains(&copying)).expect(&log);
    let removed = at(&|line| line == "volume rm --force purlis-runner-first-home").expect(&log);
    assert!(copied < removed, "{log}");
    let copy = lines[copied];
    assert!(copy.starts_with("run --rm --network none"), "{copy}");
    assert!(
        copy.contains("type=volume,source=purlis-runner-first-home,target=/from,readonly"),
        "{copy}"
    );
    assert!(
        copy.contains(&format!(
            "type=volume,source=purlis-runner-{successor}-home,target=/to "
        )),
        "{copy}"
    );
    assert!(copy.contains(&format!(
        "purlis.desktop={}",
        runners::desktop_label(config.path())
    )));
    assert!(copy.contains(NEW), "{copy}");
}

/// **A sign-in that cannot be carried never holds the drain back** (#4055): the draining runner
/// is removed all the same, and its profiles are signed in again on the pool's runner.
#[test]
fn a_drained_runner_whose_sign_ins_are_not_carried_is_removed_all_the_same() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();
    fake.set("copy-fails", "");

    let removed = drain(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(removed.forgotten);
    assert!(
        fake.file("log")
            .contains("volume rm --force purlis-runner-first-home"),
        "{}",
        fake.file("log")
    );
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().name(),
        successor
    );
}

/// **A copy container an earlier carry left holds no later one back** (#4055): one of this
/// desktop's, of the pool's runner's copy name, is removed by its id before the carry runs.
#[test]
fn a_copy_container_an_earlier_carry_left_is_removed_before_the_next_carry() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();
    fake.set(
        "containers-full",
        &format!(
            "{LEFT} purlis-runner-{successor}-copy {}\n",
            runners::desktop_label(config.path())
        ),
    );
    std::fs::write(fake.file_path("log"), "").unwrap();

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    let log = fake.file("log");
    let left = log
        .find(&format!("container rm --force --volumes {LEFT}"))
        .expect(&log);
    let copied = log
        .find(&format!("--name purlis-runner-{successor}-copy "))
        .expect(&log);
    assert!(left < copied, "{log}");
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

/// **A runner that cannot be made for a pool with its own time says the likely cause**
/// (#3471): an image of an older purlis refuses the time and stops as it starts.
#[test]
fn a_runner_not_made_for_a_pool_with_its_own_time_names_an_older_image_as_the_likely_cause() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("run-fails", "");
    let own = Set {
        suspend_after: Some(std::time::Duration::from_secs(12 * 60 * 60)),
        ..Set::default()
    };
    let refused = add(
        config.path(),
        &fake.docker(),
        &pool("first", own),
        &desktop(),
    )
    .unwrap_err()
    .to_string();
    assert!(refused.contains("older than that setting"), "{refused}");
    assert!(runners::pools(config.path()).unwrap().is_empty());
    // A pool on the default says Docker's words alone.
    let refused = add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap_err()
    .to_string();
    assert!(!refused.contains("older than that setting"), "{refused}");
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

#[cfg(unix)]
mod cluster {
    use super::*;
    use crate::clusterpool::tests::FakeCluster;
    use crate::runners::{ClusterPool, Reached};
    use std::time::Duration;

    fn team(cluster: &FakeCluster) -> ClusterPool {
        ClusterPool::new(
            "team",
            Path::new("/home/ada/.kube/config"),
            "team",
            "default",
            "ada",
            cluster.pin(),
        )
        .unwrap()
    }

    fn file(config: &Path) -> serde_json::Value {
        let path = crate::machine::dir(config).join(crate::machine::RUNNERS_FILE);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// What an add holds of the fake cluster's kubectl, a plain file: the name it is found by
    /// and, where a link on the way leads elsewhere (`/tmp` on macOS), its real file.
    fn held_kubectl(cluster: &FakeCluster) -> Vec<std::path::PathBuf> {
        let named = cluster.kubectl().program().to_path_buf();
        let real = named.canonicalize().expect("kubectl's real file");
        if real == named {
            vec![named]
        } else {
            vec![named, real]
        }
    }

    #[test]
    fn a_cluster_pool_is_added_with_its_runner_claimed_vouched_for_and_paired() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        let kept = add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(kept.pinned, cluster.runner);
        assert_eq!(kept.pod, claim);
        assert_eq!(kept.claimed.requester, crate::clusterpool::tests::REQUESTER);
        match runners::runner_of(config.path(), "team").unwrap() {
            Reached::Cluster(pool, runner) => {
                // Kept pointing at its own copy of the context.
                let copy = runners::kubeconfig_kept_at(config.path(), "team");
                let held = held_kubectl(&cluster);
                assert_eq!(pool, team(&cluster).kept_at(&copy).holding(held).unwrap());
                assert_eq!(runner, kept);
            }
            other => panic!("{other:?}"),
        }
        let file = file(config.path());
        let copy = runners::kubeconfig_kept_at(config.path(), "team");
        assert_eq!(
            file["pools"][0],
            serde_json::json!({
                "name": "team",
                "kind": "cluster",
                "kubeconfig": copy.display().to_string(),
                "context": "team",
                "pool": "default",
                "namespace": "ada",
                "operatorKey": cluster.operator.key().spelled(),
                "cluster": crate::clusterpool::tests::CLUSTER,
                "holds": held_kubectl(&cluster)
                    .iter()
                    .map(|it| it.display().to_string())
                    .collect::<Vec<_>>(),
            })
        );
        assert_eq!(file["runners"][0]["pool"], "cluster");
        assert_eq!(file["runners"][0]["claim"], claim.as_str());
        assert_eq!(
            runners::pool_names(config.path()).unwrap(),
            ["team".to_owned()]
        );
        // The claim is a kept one, made through the pool's own copy of the context.
        let calls = cluster.calls();
        assert!(
            calls.lines().any(|call| call.starts_with(&format!(
                "--kubeconfig {} --context team create",
                copy.display()
            ))),
            "{calls}"
        );
    }

    #[test]
    fn a_cluster_pool_naming_another_key_than_the_one_checked_is_not_added() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let checked = team(&cluster);
        cluster.names_key(&crate::clusterpool::tests::Operator::new().key());
        let why = add_cluster(
            config.path(),
            &cluster.kubectl(),
            &checked,
            &desktop(),
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(why.to_string().contains("check the key again"), "{why}");
        assert!(runners::cluster_pools(config.path()).unwrap().is_empty());
        assert!(!cluster.calls().contains(" create "), "{}", cluster.calls());
    }

    #[test]
    fn a_runner_answering_another_key_than_the_vouched_one_is_not_kept_and_its_claim_goes() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, LinkKey::generate().unwrap().public());
        let why = add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(why.to_string().contains("another key"), "{why}");
        assert!(runners::cluster_pools(config.path()).unwrap().is_empty());
        assert!(runners::cluster_runners(config.path()).unwrap().is_empty());
        assert!(
            cluster
                .calls()
                .contains(&format!("delete claims.runners.purlis.dev {claim}")),
            "{}",
            cluster.calls()
        );
    }

    #[test]
    fn removing_a_cluster_pool_deletes_its_claim_and_runs_its_workspaces_here_again() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        let unpaired = std::cell::Cell::new(false);
        let unpair = |_: &ClusterPool, _: &runners::ClusterKept| {
            unpaired.set(true);
            Err("not linked".to_owned())
        };
        let left = remove_cluster(
            config.path(),
            Some(&cluster.kubectl()),
            "team",
            Some(OURS),
            Some(&unpair),
        )
        .unwrap();
        assert_eq!(left, None);
        assert!(unpaired.get());
        assert!(
            cluster
                .calls()
                .contains(&format!("delete claims.runners.purlis.dev {claim}")),
            "{}",
            cluster.calls()
        );
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
        assert!(runners::cluster_runners(config.path()).unwrap().is_empty());
    }

    #[test]
    fn a_cluster_pool_runs_on_its_own_copy_of_the_context_and_its_removal_takes_the_copy() {
        use std::os::unix::fs::PermissionsExt;
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let source = config.path().join("kubeconfig");
        std::fs::write(&source, "the person's kubeconfig\n").unwrap();
        let pool =
            ClusterPool::new("team", &source, "team", "default", "ada", cluster.pin()).unwrap();
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        add_cluster(
            config.path(),
            &cluster.kubectl(),
            &pool,
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        let copy = runners::kubeconfig_kept_at(config.path(), "team");
        assert_eq!(
            std::fs::read_to_string(&copy).unwrap(),
            crate::clusterpool::tests::CONTEXT_KEPT
        );
        assert_eq!(
            std::fs::metadata(&copy).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // The kubeconfig it was taken from, changed later, changes nothing it runs.
        std::fs::write(&source, "changed since\n").unwrap();
        std::fs::remove_file(cluster.dir.path().join("calls")).unwrap();
        before_cluster_link(config.path(), &cluster.kubectl(), "team", runners::now()).unwrap();
        let calls = cluster.calls();
        assert!(!calls.contains(&source.display().to_string()), "{calls}");
        assert!(
            calls
                .lines()
                .all(|call| call.starts_with(&format!("--kubeconfig {} ", copy.display()))),
            "{calls}"
        );
        assert_eq!(
            std::fs::read_to_string(&copy).unwrap(),
            crate::clusterpool::tests::CONTEXT_KEPT
        );
        remove_cluster(
            config.path(),
            Some(&cluster.kubectl()),
            "team",
            Some(OURS),
            None,
        )
        .unwrap();
        assert!(!copy.exists());
    }

    #[test]
    fn a_pin_moved_by_a_roll_is_kept_and_a_claim_not_yet_signed_again_is_tried_again() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        let next = crate::clusterpool::tests::Operator::new();
        let now = runners::now();
        cluster.rolls_to(&next, now + 24 * 3600);
        // The Claim not reconciled since the roll: this link alone is refused, not as a changed
        // key, and the moved pin is kept.
        let first = before_cluster_link(config.path(), &cluster.kubectl(), "team", now);
        assert!(
            matches!(first, Err(crate::clusterpool::Refused::NotVouched(_))),
            "{first:?}"
        );
        let kept = runners::cluster_pools(config.path()).unwrap();
        assert_eq!(kept[0].pin().unwrap().key, next.key());
        // Signed again, and the roll over: the kept pin links.
        cluster.vouch_signed("ada", &claim, &claim, cluster.runner.key, Some(&next));
        assert_eq!(
            before_cluster_link(config.path(), &cluster.kubectl(), "team", now),
            Ok(())
        );
        assert!(runners::cluster_runners(config.path()).unwrap().len() == 1);
    }

    /// The pool `team` added on the stand-in's cluster, its runner kept.
    fn added(config: &Path, cluster: &FakeCluster) {
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        add_cluster(
            config,
            &cluster.kubectl(),
            &team(cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
    }

    fn deleted(cluster: &FakeCluster) -> bool {
        cluster.calls().contains("delete claims.runners.purlis.dev")
    }

    use crate::clusterpool::Refused as NotVouched;
    use crate::poollogins::Bound;

    fn vouches(_: &Path, _: &str) -> Result<(), NotVouched> {
        Ok(())
    }

    /// What a link to the runner kept for `team` under `config` reports.
    fn reached(config: &Path) -> String {
        runners::cluster_runner_for(config, "team")
            .unwrap()
            .expect("a cluster pool")
            .1
            .reached()
    }

    fn gate(config: &Path) -> Result<Option<Bound>, String> {
        let reached = reached(config);
        Bound::of_a_cluster_pool(config, "team", Some(reached.as_str()), &vouches)
    }

    /// **A cluster runner's sign-in runs its own `purlis profile sign-in` in the pod its pairing
    /// named** (#3819), through kubectl pointed at the pool's context, after the check every link
    /// makes; a runner not vouched for now runs nothing.
    #[test]
    fn a_cluster_runners_sign_in_runs_in_its_vouched_pod_after_the_links_check() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let claim = claim_name(OURS, "team");
        let login = cluster_sign_in(
            config.path(),
            &cluster.kubectl(),
            "team",
            "work",
            None,
            crate::dockerpool::Typed::Driven,
        )
        .unwrap();
        assert_eq!(
            std::path::Path::new(&login.program),
            cluster.kubectl().program()
        );
        let pod = format!("pod/{claim}");
        let runners = crate::clusterpool::runner_namespace(crate::clusterpool::tests::REQUESTER);
        let at = login.args.iter().position(|arg| arg == "exec").unwrap();
        assert_eq!(
            login.args[at..],
            [
                "exec",
                "-i",
                "--namespace",
                &runners,
                "--container",
                "runner",
                &pod,
                "--",
                crate::dockerpool::PURLIS,
                "profile",
                "sign-in",
                "work",
            ]
        );
        assert!(login.args.contains(&"--context".to_owned()));
        assert!(login.cleared.contains(&"KUBECONFIG".to_owned()));
        assert!(cluster.calls().contains("get claims.runners.purlis.dev"));

        let typed = cluster_sign_in(
            config.path(),
            &cluster.kubectl(),
            "team",
            "work",
            Some("github-copilot"),
            crate::dockerpool::Typed::InATerminal,
        )
        .unwrap();
        assert!(typed.args.contains(&"-it".to_owned()));
        assert!(
            typed
                .args
                .ends_with(&["--provider".to_owned(), "github-copilot".to_owned()])
        );

        // Its claim no longer vouched for: nothing to run.
        cluster.write("claim.json", "{}");
        assert!(
            cluster_sign_in(
                config.path(),
                &cluster.kubectl(),
                "team",
                "work",
                None,
                crate::dockerpool::Typed::Driven,
            )
            .is_err()
        );
    }

    /// **A cluster runner's claim says whether its home outlives its pod** (#3819): a kept claim's
    /// does, a fresh one's does not, and only the claim this desktop kept is believed.
    #[test]
    fn a_cluster_runners_claim_says_whether_a_sign_in_there_outlives_its_pod() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let keeps = || claim_keeps_its_home_with(config.path(), &cluster.kubectl(), "team");
        let mut claim: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(cluster.dir.path().join("claim.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            keeps(),
            Ok(false),
            "a claim that does not say kept is fresh"
        );
        claim["spec"]["kept"] = true.into();
        cluster.write("claim.json", &claim.to_string());
        assert_eq!(keeps(), Ok(true));
        claim["spec"]["kept"] = "yes".into();
        cluster.write("claim.json", &claim.to_string());
        assert_eq!(keeps(), Ok(false), "kept is true alone");
        claim["spec"]["kept"] = true.into();
        claim["metadata"]["uid"] = "another".into();
        cluster.write("claim.json", &claim.to_string());
        assert!(keeps().is_err(), "another claim of its name says nothing");
        cluster.write("claim.json", "");
        assert!(keeps().is_err(), "a claim gone says nothing");
    }

    #[test]
    fn a_cluster_pool_is_added_unmarked_so_its_runner_is_sent_nothing_and_nothing_asks_its_cluster()
    {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let asked = std::cell::Cell::new(false);
        let link = reached(config.path());
        let refused = Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &|_, _| {
            asked.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(refused.contains("Only you can mark it"), "{refused}");
        assert!(refused.contains("This is my own"), "{refused}");
        assert!(!asked.get(), "an unmarked pool's cluster is never asked");
    }

    #[test]
    fn a_marked_pool_with_its_claim_vouched_for_now_is_bound_to_the_runner_the_cluster_names() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        let bound = gate(config.path()).unwrap().expect("a cluster pool");
        let (pool, kept) = runners::cluster_runner_for(config.path(), "team")
            .unwrap()
            .unwrap();
        assert_eq!(bound.pool, "team");
        assert_eq!(bound.runner, kept.pinned.device);
        assert_eq!(bound.definition, pool.bound_digest(&kept));
        assert!(bound.image.is_empty() && bound.runner_image.is_empty());
        assert!(bound.on_a_cluster());
        assert_eq!(
            bound.engine,
            format!(
                "cluster {}, context \"team\"",
                &crate::clusterpool::tests::CLUSTER[..8]
            )
        );
    }

    #[test]
    fn a_change_to_anything_a_cluster_runner_is_bound_to_changes_what_is_bound() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let (pool, kept) = runners::cluster_runner_for(config.path(), "team")
            .unwrap()
            .unwrap();
        let was = Bound::of_cluster(&pool, &kept);
        let mut claim = kept.clone();
        claim.claimed.uid = "another".into();
        let mut pod = kept.clone();
        pod.pod = "another".into();
        let mut device = kept.clone();
        device.pinned.device = "another".into();
        let mut key = kept.clone();
        key.pinned.key = LinkKey::generate().unwrap().public();
        for changed in [claim, pod, device, key] {
            assert_ne!(Bound::of_cluster(&pool, &changed), was, "{changed:?}");
        }
        let other = |context: &str, pin: crate::clusterpool::PoolPin| {
            ClusterPool::new(
                "team",
                Path::new("/home/ada/.kube/config"),
                context,
                "default",
                "ada",
                pin,
            )
            .unwrap()
        };
        let pin = pool.pin().unwrap();
        assert_ne!(
            Bound::of_cluster(&other("elsewhere", pin.clone()), &kept),
            was
        );
        let moved = crate::clusterpool::PoolPin {
            cluster: "4e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b".into(),
            ..pin.clone()
        };
        assert_ne!(Bound::of_cluster(&other("team", moved), &kept), was);
        let rolled = crate::clusterpool::PoolPin {
            key: crate::clusterpool::tests::Operator::new().key(),
            ..pin
        };
        assert_ne!(Bound::of_cluster(&other("team", rolled), &kept), was);
    }

    #[test]
    fn a_vouch_refused_or_unanswered_sends_nothing_and_tells_the_chat_none_of_the_clusters_words() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        let link = reached(config.path());
        let refused = Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &|_, _| {
            Err(NotVouched::Other("kubectl said secret-host:6443".into()))
        })
        .unwrap_err();
        assert!(refused.contains("sent no vault value"), "{refused}");
        assert!(!refused.contains("secret-host"), "{refused}");
    }

    #[test]
    fn a_mark_taken_back_or_a_runner_changed_while_the_cluster_was_asked_sends_nothing() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        let link = reached(config.path());
        let refused =
            Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &|config, name| {
                runners::set_cluster_owned(config, name, false).unwrap();
                Ok(())
            })
            .unwrap_err();
        assert!(refused.contains("not marked"), "{refused}");

        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &|config, name| {
            remove_cluster(config, None, name, None, None).unwrap();
            Ok(())
        })
        .unwrap_err();
    }

    #[test]
    fn a_link_to_any_runner_but_the_one_kept_now_is_refused() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        let link = reached(config.path());
        assert!(Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &vouches).is_ok());
        // Under the pool's name and the runner's, a link pinned to another key is another runner.
        let earlier = format!("team/team@{}", "ab".repeat(32));
        for other in ["team/team", "team/other", "other/team", "team", &earlier] {
            Bound::of_a_cluster_pool(config.path(), "team", Some(other), &vouches).unwrap_err();
        }
        Bound::of_a_cluster_pool(config.path(), "team", None, &vouches).unwrap_err();
    }

    #[test]
    fn a_marked_runner_a_chat_ran_on_without_the_sandbox_is_sent_nothing() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        crate::runnertaint::mark_pool(config.path(), "team").unwrap();
        let refused = gate(config.path()).unwrap_err();
        assert!(refused.contains("without the sandbox"), "{refused}");
    }

    #[test]
    fn a_name_no_cluster_pool_has_is_left_to_the_docker_pools() {
        let config = tempfile::tempdir().unwrap();
        assert_eq!(
            Bound::of_a_cluster_pool(config.path(), "team", None, &vouches),
            Ok(None)
        );
    }

    #[test]
    fn a_claim_being_deleted_vouches_for_nothing_though_its_statement_is_still_there() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let at = cluster.dir.path().join("claim.json");
        let mut claim: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&at).unwrap()).unwrap();
        claim["metadata"]["deletionTimestamp"] = "2026-10-30T12:00:00Z".into();
        std::fs::write(&at, claim.to_string()).unwrap();
        assert!(matches!(
            before_cluster_link(config.path(), &cluster.kubectl(), "team", runners::now()),
            Err(NotVouched::NotVouched(_))
        ));
    }

    #[test]
    fn the_mark_is_the_persons_row_key_true_alone_and_a_new_add_starts_without_it() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        assert!(file(config.path())["pools"][0].get("owned").is_none());
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        assert_eq!(file(config.path())["pools"][0]["owned"], true);
        assert!(runners::cluster_pools(config.path()).unwrap()[0].owned());
        // A key roll keeps it: the pool is the same one.
        let pool = &runners::cluster_pools(config.path()).unwrap()[0];
        let rolled = crate::clusterpool::PoolPin {
            key: crate::clusterpool::tests::Operator::new().key(),
            ..pool.pin().unwrap()
        };
        runners::keep_cluster_pin(config.path(), "team", &rolled).unwrap();
        assert!(runners::cluster_pools(config.path()).unwrap()[0].owned());

        runners::set_cluster_owned(config.path(), "team", false).unwrap();
        assert!(file(config.path())["pools"][0].get("owned").is_none());
        runners::set_cluster_owned(config.path(), "nobody", true).unwrap_err();

        let mut spelled = file(config.path());
        spelled["pools"][0]["owned"] = false.into();
        std::fs::write(
            crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE),
            spelled.to_string(),
        )
        .unwrap();
        runners::cluster_pools(config.path()).unwrap_err();
    }

    #[test]
    fn a_chat_with_no_sandbox_on_a_cluster_runner_is_kept_across_a_restart_until_its_claim_goes() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        assert!(!crate::runnertaint::pool_ran_unsandboxed(
            config.path(),
            "team"
        ));
        crate::runnertaint::mark_pool(config.path(), "team").unwrap();
        assert!(crate::runnertaint::pool_ran_unsandboxed(
            config.path(),
            "team"
        ));
    }

    #[test]
    fn a_kubectl_that_does_not_answer_is_ended_and_refused_in_its_time() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let kubectl = dir.path().join("kubectl");
        std::fs::write(&kubectl, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(&kubectl, std::fs::Permissions::from_mode(0o755)).unwrap();
        let began = std::time::Instant::now();
        let refused = crate::clusterpool::Kubectl::at(&kubectl)
            .within(Duration::from_millis(200))
            .run(&["get".to_owned()], None, 1024);
        assert!(matches!(refused, Err(NotVouched::Other(_))));
        assert!(began.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_removal_unpairs_before_it_locks_so_a_link_that_keeps_a_moved_pin_finishes() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let next = crate::clusterpool::tests::Operator::new();
        cluster.rolls_to(&next, runners::now() + 24 * 3600);
        let claim = claim_name(OURS, "team");
        cluster.vouch_signed("ada", &claim, &claim, cluster.runner.key, Some(&next));
        // The unpairing's link checks first, as the connector does, and keeps the moved pin.
        let kubectl = cluster.kubectl();
        let unpair = |_: &ClusterPool, _: &runners::ClusterKept| {
            before_cluster_link(config.path(), &kubectl, "team", runners::now())
                .map_err(|why| why.to_string())?;
            let kept = runners::cluster_pools(config.path()).map_err(|why| why.to_string())?;
            assert_eq!(
                kept[0].pin().unwrap().key,
                next.key(),
                "the moved pin was kept"
            );
            Ok(())
        };
        let left = remove_cluster(
            config.path(),
            Some(&kubectl),
            "team",
            Some(OURS),
            Some(&unpair),
        )
        .unwrap();
        assert_eq!(left, None);
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
        assert!(deleted(&cluster));
    }

    #[test]
    fn a_pool_whose_cluster_cannot_be_reached_is_forgotten_here_and_says_what_is_left() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        // No kubectl on this machine any more.
        let left = remove_cluster(config.path(), None, "team", Some(OURS), None)
            .unwrap()
            .expect("it says what may be left");
        assert!(
            left.contains("kubectl delete claims.runners.purlis.dev"),
            "{left}"
        );
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
        assert!(!runners::kubeconfig_kept_at(config.path(), "team").exists());

        // The cluster not answering as purlis reads it.
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        cluster.write("claim.json", "the cluster is not there");
        let left = remove_cluster(
            config.path(),
            Some(&cluster.kubectl()),
            "team",
            Some(OURS),
            None,
        )
        .unwrap()
        .expect("it says what may be left");
        assert!(left.contains(&claim_name(OURS, "team")), "{left}");
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
        assert!(!deleted(&cluster));
    }

    #[test]
    fn a_claim_is_deleted_only_where_it_is_the_one_this_desktop_made() {
        // Kept with another uid than the cluster's claim of that name: left alone.
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        cluster.write(
            "claim.json",
            &serde_json::json!({"metadata": {"uid": "99999999-9999-4999-8999-999999999999"}})
                .to_string(),
        );
        let left = remove_cluster(
            config.path(),
            Some(&cluster.kubectl()),
            "team",
            Some(OURS),
            None,
        )
        .unwrap();
        assert!(left.is_some_and(|left| left.contains("not the claim this desktop made")));
        assert!(!deleted(&cluster));

        // An add that stopped before its runner was kept: the claim marked as this desktop's,
        // asked for by who the kubeconfig is, is deleted; another desktop's is not.
        for (device, deletes) in [(OURS, true), ("01JB0THERDESKT0P0000000000", false)] {
            let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
            let pool = team(&cluster)
                .kept_at(&runners::kubeconfig_kept_at(config.path(), "team"))
                .holding(vec![cluster.kubectl().real_program().unwrap()])
                .unwrap();
            runners::record_cluster_pool_locked(
                config.path(),
                &pool,
                &runners::lock(config.path()),
            )
            .unwrap();
            runners::keep_kubeconfig(config.path(), "team", "{}").unwrap();
            let claim = claim_name(device, "team");
            cluster.vouch("ada", &claim, &claim, cluster.runner.key);
            let left = remove_cluster(
                config.path(),
                Some(&cluster.kubectl()),
                "team",
                Some(device),
                None,
            )
            .unwrap();
            assert_eq!(deleted(&cluster), deletes, "{device}: {left:?}");
            assert_eq!(left.is_none(), deletes, "{device}: {left:?}");
            assert!(runners::pool_names(config.path()).unwrap().is_empty());
        }
    }

    #[test]
    fn a_claims_name_is_its_desktops_device_ids_however_the_config_home_is_found() {
        assert_eq!(
            claim_name(OURS, "team"),
            claim_name(&OURS.to_lowercase(), "team")
        );
        assert_ne!(
            claim_name(OURS, "team"),
            claim_name("01JB0THERDESKT0P0000000000", "team")
        );
        assert!(claim_name(OURS, &"t".repeat(40)).len() <= 63);
    }

    #[test]
    fn a_plugin_where_others_may_write_refuses_the_add_and_one_held_is_kept_with_the_pool() {
        use std::os::unix::fs::PermissionsExt;
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let open = tempfile::tempdir().unwrap();
        let plugin = open.path().join("auth-plugin");
        std::fs::write(&plugin, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&plugin, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(open.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        let naming = |plugin: &Path| {
            serde_json::json!({"apiVersion": "v1", "kind": "Config", "users": [
                {"name": "ada", "user": {"exec": {"command": plugin.display().to_string()}}}
            ]})
            .to_string()
        };
        cluster.write("context.yaml", &naming(&plugin));
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        let why = add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(
            why.to_string().contains("can be written by others"),
            "{why}"
        );
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
        assert!(!cluster.calls().contains("create --filename"));

        std::fs::set_permissions(open.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        let held = runners::cluster_holds(config.path());
        assert!(held.contains(&plugin), "as named: {held:?}");
        assert!(held.contains(&plugin.canonicalize().unwrap()), "{held:?}");
        assert!(
            held.contains(&cluster.kubectl().real_program().unwrap()),
            "{held:?}"
        );
    }

    #[test]
    fn a_removal_waits_for_an_add_of_the_pool_still_under_way() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        // An add of it under way holds this until it ends.
        let under_way = adding(config.path(), "team");
        let (done, finished) = std::sync::mpsc::channel();
        let (root, kubectl) = (config.path().to_path_buf(), cluster.kubectl());
        let removing = std::thread::spawn(move || {
            let left = remove_cluster(&root, Some(&kubectl), "team", Some(OURS), None);
            done.send(()).unwrap();
            left
        });
        assert!(
            finished.recv_timeout(Duration::from_millis(300)).is_err(),
            "the removal did not wait for the add"
        );
        drop(under_way);
        finished.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(removing.join().unwrap().unwrap(), None);
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
    }

    #[test]
    fn a_removal_that_cannot_tell_which_claim_is_its_own_says_what_may_be_left() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let pool = team(&cluster)
            .kept_at(&runners::kubeconfig_kept_at(config.path(), "team"))
            .holding(vec![cluster.kubectl().real_program().unwrap()])
            .unwrap();
        runners::record_cluster_pool_locked(config.path(), &pool, &runners::lock(config.path()))
            .unwrap();
        let left = remove_cluster(config.path(), Some(&cluster.kubectl()), "team", None, None)
            .unwrap()
            .expect("it says what may be left");
        assert!(
            left.contains("kubectl get claims.runners.purlis.dev"),
            "{left}"
        );
        assert!(runners::pool_names(config.path()).unwrap().is_empty());
        assert!(!deleted(&cluster));
    }

    #[test]
    fn a_program_whose_package_would_be_the_home_or_above_it_refuses_the_add() {
        let home = Path::new("/home/ada");
        for wide in ["/home/ada/plugin", "/home/plugin", "/plugin"] {
            let why = package_holdable(Path::new(wide), Some(home)).unwrap_err();
            assert!(
                why.to_string().contains("a folder of its own"),
                "{wide}: {why}"
            );
        }
        for held in [
            "/home/ada/bin/plugin",
            "/opt/homebrew/Cellar/kubernetes-cli/1.36.0/bin/kubectl",
            "/usr/local/bin/aws",
        ] {
            assert_eq!(
                package_holdable(Path::new(held), Some(home)),
                Ok(()),
                "{held}"
            );
        }
    }

    #[test]
    fn a_plugin_reached_through_links_is_held_at_every_link_on_the_way() {
        let (_config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let place = tempfile::tempdir().unwrap();
        let (real, middle, first) = (
            place.path().join("keg/bin/plugin"),
            place.path().join("opt/plugin"),
            place.path().join("bin/plugin"),
        );
        for folder in ["keg/bin", "opt", "bin"] {
            std::fs::create_dir_all(place.path().join(folder)).unwrap();
        }
        std::fs::write(&real, "#!/bin/sh\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::os::unix::fs::symlink(&real, &middle).unwrap();
        std::os::unix::fs::symlink("../opt/plugin", &first).unwrap();
        let context = serde_json::json!({"users": [
            {"name": "ada", "user": {"exec": {"command": first.display().to_string()}}}
        ]})
        .to_string();
        let held =
            programs_run(&cluster.kubectl(), &context, Some(Path::new("/home/ada"))).unwrap();
        for link in [&first, &place.path().join("bin/../opt/plugin"), &real] {
            assert!(held.contains(link), "{} in {held:?}", link.display());
        }
        assert!(held.contains(&real.canonicalize().unwrap()), "{held:?}");
    }

    #[test]
    fn a_cluster_pools_name_is_never_a_docker_pools_and_never_reaches_one() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        let docker = FakeDocker::new(&inspected());
        let refused = add(
            config.path(),
            &docker.docker(),
            &pool("team", Set::default()),
            &desktop(),
        )
        .unwrap_err();
        assert!(refused.to_string().contains("already"), "{refused}");
        // A Docker pool's own lookup never answers with a cluster pool's runner.
        assert!(runners::runner_for(config.path(), "team").is_err());
    }
}

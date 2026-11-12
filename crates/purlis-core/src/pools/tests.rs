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
    let unpair = |_: &runners::Kept| {
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
fn a_chat_is_kept_only_on_a_pool_this_machine_keeps_and_goes_with_its_end() {
    let config = tempfile::tempdir().unwrap();
    let plane = std::path::Path::new("/planes/a");
    crate::machine::update(config.path(), |store| store.remember(plane, 1)).unwrap();
    keep_chat_on_pool(config.path(), plane, 7, "first", "ide").unwrap();
    assert_eq!(
        chat_on_pool(config.path(), plane, 7),
        None,
        "a keep that lost a race with the pool's removal keeps nothing"
    );

    let fake = a_docker(&runner_answers());
    add(
        config.path(),
        &fake.docker(),
        &pool("first", Set::default()),
        &desktop(),
    )
    .unwrap();
    keep_chat_on_pool(config.path(), plane, 7, "first", "ide").unwrap();
    assert_eq!(
        chat_on_pool(config.path(), plane, 7).map(|chat| chat.pool),
        Some("first".to_owned())
    );

    leave_chat_on_pool(config.path(), plane, 7).unwrap();
    assert_eq!(chat_on_pool(config.path(), plane, 7), None);
    assert_eq!(
        branches_to_fetch(config.path(), plane)["first"],
        std::collections::BTreeSet::from(["ide".to_owned()])
    );
    branches_fetched(config.path(), plane, "first", "ide").unwrap();
    assert!(branches_to_fetch(config.path(), plane).is_empty());
}

/// **A draining runner's work is owed while a project may still have some of it to fetch**
/// (#5393), and a chat kept on the pool's next runner holds nothing back.
#[test]
fn work_is_owed_from_a_draining_runner_until_its_fetch_and_a_chat_on_the_next_runner_owes_none() {
    let config = tempfile::tempdir().unwrap();
    let plane = std::path::Path::new("/planes/a");
    let chat = |runner: Option<&str>| crate::machine::ChatOnPool {
        pool: "first".to_owned(),
        workspace: "ide".to_owned(),
        runner: runner.map(str::to_owned),
    };
    let owed = || -> Vec<std::path::PathBuf> {
        work_owed_from(config.path(), "first", "first-old")
            .unwrap()
            .into_iter()
            .map(|owing| owing.plane)
            .collect()
    };
    crate::machine::update(config.path(), |store| store.remember(plane, 1)).unwrap();
    assert!(owed().is_empty(), "nothing ran there");

    crate::machine::update(config.path(), |store| {
        store.keep_chat_on_pool(plane, 1, chat(Some("first-new")));
    })
    .unwrap();
    assert!(owed().is_empty(), "that chat is on the pool's next runner");

    crate::machine::update(config.path(), |store| {
        store.keep_chat_on_pool(plane, 2, chat(Some("first-old")));
    })
    .unwrap();
    assert_eq!(owed(), [plane], "a chat that ran there may have left work");
    assert_eq!(
        work_owed_from(config.path(), "first", "first-old").unwrap()[0].opened,
        1,
        "it says when the project was last opened (#5461)"
    );

    crate::machine::update(config.path(), |store| store.forget_chat_on_pool(plane, 2)).unwrap();
    crate::machine::update(config.path(), |store| {
        store.keep_chat_on_pool(plane, 3, chat(None));
    })
    .unwrap();
    assert_eq!(owed(), [plane], "a record that can't say counts for each");

    crate::machine::update(config.path(), |store| store.leave_chat_on_pool(plane, 3)).unwrap();
    assert_eq!(owed(), [plane], "its chat ended, and its fetch has not run");

    crate::machine::update(config.path(), |store| {
        store.branches_fetched(plane, "first", "ide");
    })
    .unwrap();
    assert!(owed().is_empty(), "its fetch ran");
}

/// A chat started again on another of its pool's runners has left work on the one it was on.
#[test]
fn a_chat_kept_again_on_another_runner_of_its_pool_owes_what_it_left_on_the_first() {
    let mut store = crate::machine::Store::default();
    let plane = std::path::Path::new("/planes/a");
    store.remember(plane, 1);
    let chat = |runner: Option<&str>| crate::machine::ChatOnPool {
        pool: "first".to_owned(),
        workspace: "ide".to_owned(),
        runner: runner.map(str::to_owned),
    };
    store.keep_chat_on_pool(plane, 1, chat(Some("first-old")));
    assert!(store.recent(plane).unwrap().branches_to_fetch.is_empty());

    store.keep_chat_on_pool(plane, 1, chat(Some("first-new")));

    let recent = store.recent(plane).unwrap();
    assert_eq!(recent.chats_on_pools[&1], chat(Some("first-new")));
    assert!(recent.branches_to_fetch["first"].contains("ide"));
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
    // What its removal asks about names the workspace, in its project, and nothing else.
    crate::machine::update(config.path(), |store| {
        store.remember(std::path::Path::new("/planes/b"), 2);
    })
    .unwrap();
    assert_eq!(
        named_by(config.path(), "first").unwrap(),
        [(
            plane.to_path_buf(),
            crate::machine::NamingPool {
                project: false,
                workspaces: vec!["ide".to_owned()],
            }
        )]
    );
    // Removing the pool runs the workspace's chats here again.
    remove(config.path(), &fake.docker(), "first", None).unwrap();
    assert_eq!(runs_on(config.path(), plane, Some("ide")), Ok(None));
    assert!(workspaces_on_pools(config.path(), plane).is_empty());
    assert!(named_by(config.path(), "first").unwrap().is_empty());
}

#[test]
fn a_machine_store_that_cannot_be_read_never_says_nothing_names_a_pool() {
    let config = tempfile::tempdir().unwrap();
    let elsewhere = config.path().join("elsewhere.json");
    std::fs::write(&elsewhere, br#"{"version":1,"at":0,"recents":[]}"#).unwrap();
    std::fs::create_dir_all(crate::machine::dir(config.path())).unwrap();
    std::os::unix::fs::symlink(&elsewhere, crate::machine::file(config.path())).unwrap();
    assert!(named_by(config.path(), "first").is_err());
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
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
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

/// A pool `first`, followed to a new image: its old runner `first` drains beside the successor.
/// The log is emptied.
fn a_pool_draining(config: &std::path::Path, fake: &FakeDocker) -> String {
    let ours = desktop();
    a_pool_added(config, fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(fake, &successor, NEW, &successor_answers());
    follow(config, &fake.docker(), "first", &new_image, &ours).unwrap();
    std::fs::write(fake.file_path("log"), "").unwrap();
    successor
}

/// [`a_pool_draining`], its draining runner's row keeping its engine and its home's making, as a
/// runner made since the mark does.
fn a_pool_draining_since_the_mark(config: &std::path::Path, fake: &FakeDocker) -> String {
    fake.set("engine", "engine-a\n");
    home_prints(fake, HOME_CREATED);
    a_pool_draining(config, fake)
}

/// **A runner made before the mark carries nothing** (ruled 2026-11-08): its row keeps no engine
/// or home, its image's purlis is older than the first a desktop can mark, or that cannot be read.
#[test]
fn a_runner_made_before_the_mark_carries_no_sign_ins() {
    let carried = |set_up: &dyn Fn(&std::path::Path, &FakeDocker)| {
        let config = tempfile::tempdir().unwrap();
        let fake = a_docker(&runner_answers());
        set_up(config.path(), &fake);
        drain(config.path(), &fake.docker(), "first", None).unwrap();
        fake.file("log").contains("-copy ")
    };
    assert!(
        carried(&|config, fake| {
            a_pool_draining_since_the_mark(config, fake);
        }),
        "made since the mark, it carries"
    );
    assert!(
        !carried(&|config, fake| {
            a_pool_draining(config, fake);
        }),
        "a row with no engine or home"
    );
    assert!(
        !carried(&|config, fake| {
            a_pool_draining_since_the_mark(config, fake);
            fake.set("version", "purlis 0.4.1 (release profile)\n");
        }),
        "an image older than the mark"
    );
    assert!(
        !carried(&|config, fake| {
            a_pool_draining_since_the_mark(config, fake);
            fake.set("version", "no purlis here\n");
        }),
        "an image whose version cannot be read"
    );
}

/// **The version is asked of the runner's image, never of its container** (ruled 2026-11-08): a
/// chat could have changed what the container's purlis says, and nothing changes the image pinned
/// by its digest.
#[test]
fn a_carry_asks_the_runners_image_its_version_and_not_its_container() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining_since_the_mark(config.path(), &fake);

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    let log = fake.file("log");
    let asked = log
        .lines()
        .find(|line| line.ends_with(" --version"))
        .expect("the version was asked");
    assert!(asked.starts_with("run --rm --network none "), "{asked}");
    assert!(
        asked.contains(&format!(
            "--read-only --entrypoint {} ",
            crate::dockerpool::PURLIS
        )),
        "{asked}"
    );
    assert!(
        !log.contains(&format!(
            "exec -i --user 10001:10001 {ID} /usr/local/bin/purlis --version"
        )),
        "{log}"
    );
}

#[test]
fn the_first_purlis_a_desktop_can_mark_is_0_5_0() {
    for since in ["0.5.0", "0.5.1", "0.6.0-rc.1", "1.0.0", "0.10.0+build"] {
        assert!(marked_since(since), "{since}");
    }
    for before in ["0.4.1", "0.4.99", "0.5", "", "v0.5.0", "0.5.0.1", "zero"] {
        assert!(!marked_since(before), "{before}");
    }
}

/// **A drained runner is unpaired before its sign-ins are carried** (#4146): nothing can link to
/// it while they go.
#[test]
fn a_drained_runner_is_unpaired_before_its_sign_ins_are_carried() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining_since_the_mark(config.path(), &fake);
    let log_at_unpair = std::cell::RefCell::new(None);
    let unpair = |_: &runners::Kept| {
        *log_at_unpair.borrow_mut() = Some(fake.file("log"));
        Ok(())
    };

    let removed = drain(config.path(), &fake.docker(), "first", Some(&unpair)).unwrap();

    assert_eq!(removed.peer, Some(Ok(())));
    let before = log_at_unpair.borrow().clone().expect("it was unpaired");
    assert!(!before.contains("-copy "), "{before}");
    assert!(fake.file("log").contains("-copy "));
}

/// **A drain whose removal failed does not carry again, nor unpair again** (#4146): the next
/// look only removes what is left.
#[test]
fn a_drain_whose_removal_failed_does_not_carry_again_at_the_next_look() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    fake.set("rm-fails-volume", "1");
    fake.set("volumes", "purlis-runner-first-home\n");

    let unpaired = std::cell::Cell::new(0);
    let unpair = |_: &runners::Kept| {
        unpaired.set(unpaired.get() + 1);
        Ok(())
    };
    let first = drain(config.path(), &fake.docker(), "first", Some(&unpair)).unwrap();
    assert!(!first.forgotten, "{first:?}");
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(
        kept.carried && kept.unpaired,
        "marks kept across a read of the file"
    );

    std::fs::write(fake.file_path("log"), "").unwrap();
    drain(config.path(), &fake.docker(), "first", Some(&unpair)).unwrap();

    let log = fake.file("log");
    assert!(!log.contains("-copy "), "{log}");
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
    assert_eq!(unpaired.get(), 1);
}

/// **A running runner that was not unpaired stays, and nothing is carried from it** (#4146).
#[test]
fn a_running_runner_that_was_not_unpaired_stays_and_carries_nothing() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let unpair = |_: &runners::Kept| Err("it did not answer".to_owned());

    drain(config.path(), &fake.docker(), "first", Some(&unpair)).unwrap_err();

    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(!kept.carried && !kept.unpaired);
    let log = fake.file("log");
    assert!(
        !log.contains("-copy ") && !log.contains("volume rm"),
        "{log}"
    );
}

/// **A stopped runner cannot be unpaired, and is still drained** (#4146).
#[test]
fn a_stopped_runner_that_could_not_be_unpaired_is_drained() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining_since_the_mark(config.path(), &fake);
    let mut stopped = inspected();
    stopped[0]["State"]["Running"] = false.into();
    fake.set("inspect", &stopped.to_string());
    let unpair = |_: &runners::Kept| Err("it is stopped".to_owned());

    let removed = drain(config.path(), &fake.docker(), "first", Some(&unpair)).unwrap();

    assert!(removed.forgotten, "{removed:?}");
    assert!(fake.file("log").contains("-copy "));
}

fn ran(running: bool, finished: Option<u64>) -> crate::dockerpool::Ran {
    crate::dockerpool::Ran {
        running,
        started: Some(1),
        finished,
    }
}

/// **What a look makes of a draining runner, by what Docker says now against what it said as the
/// drain began.** `None` is a row from before that was noted, and `Some(0)` a container that had
/// never stopped.
#[test]
fn only_a_stop_noted_before_its_drain_began_says_a_stopped_runner_ran_no_chat() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    let cases = [
        // Ran through its drain so far: it alone can say.
        (Some(0), ran(true, None), DrainLook::Ask),
        (Some(7), ran(true, Some(7)), DrainLook::Ask),
        (None, ran(true, Some(7)), DrainLook::Ask),
        // Had stopped already, and has not run since.
        (Some(7), ran(false, Some(7)), DrainLook::StoppedBefore),
        // Stopped since, and still stopped.
        (Some(0), ran(false, Some(9)), DrainLook::Held),
        (Some(7), ran(false, Some(9)), DrainLook::Held),
        // Stopped since, and started again: what it lists says nothing of what the stop ended.
        (Some(0), ran(true, Some(9)), DrainLook::Held),
        (Some(7), ran(true, Some(9)), DrainLook::Held),
        // Docker no longer says when, or nothing was noted: not known to have run no chat.
        (Some(0), ran(false, None), DrainLook::Held),
        (None, ran(false, Some(7)), DrainLook::Held),
    ];
    for (last_stop, now, look) in cases {
        let kept = Kept {
            last_stop,
            ..kept.clone()
        };
        assert_eq!(drain_look(&kept, now), look, "{last_stop:?} {now:?}");
    }
}

/// Once no chat was left its removal began, and what follows of it is only its removal.
#[test]
fn a_runner_whose_removal_began_is_not_held() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    for removal in [
        Kept {
            carried: true,
            ..kept.clone()
        },
        Kept {
            carry_failed: true,
            ..kept.clone()
        },
    ] {
        assert_eq!(drain_look(&removal, ran(false, None)), DrainLook::NoChat);
    }
    let unpaired = Kept {
        unpaired: true,
        ..kept
    };
    assert_eq!(drain_look(&unpaired, ran(true, Some(9))), DrainLook::NoChat);
}

/// **A drain notes when its runner last stopped**, read by Docker as it begins, and the row keeps
/// it: `0` for a container that never stopped.
#[test]
fn a_drain_notes_when_its_runner_last_stopped() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    let ours = desktop();
    a_pool_added(config.path(), &fake, &ours);
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
    let successor = crate::dockerpool::successor_name("first", &new_image);
    makes(&fake, &successor, NEW, &successor_answers());
    let mut stopped: serde_json::Value = serde_json::from_str(&fake.file("inspect")).unwrap();
    stopped[0]["State"]["FinishedAt"] = "2026-11-01T00:00:00Z".into();
    fake.set("inspect", &stopped.to_string());

    follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();

    let kept = runners::named(config.path(), "first").unwrap().unwrap();

    assert_eq!(kept.last_stop, Some(1_793_491_200));
}

/// A pool moved again notes only what this drain took: a row draining since before leaves as it
/// stands, or the stop that held it would read as one from before its drain.
#[test]
fn a_later_drain_leaves_an_earlier_runners_row_as_it_is() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let locked = runners::lock(config.path());
    runners::note_locked(
        config.path(),
        "first",
        |kept| kept.last_stop = None,
        &locked,
    )
    .unwrap();
    let before = before_a_drain(config.path(), &fake.docker(), "first");
    note_drains_began(config.path(), &before, &locked);
    drop(locked);

    let kept = runners::named(config.path(), "first").unwrap().unwrap();

    assert_eq!(kept.last_stop, None);
    assert!(before.iter().all(|(name, _)| name != "first"), "{before:?}");
}

/// **Neither an update nor a Replace holds a runner already stopped on its own** (#4633, ruled
/// 2026-11-02): each notes its stop, and what its home holds decides.
#[test]
fn no_drain_holds_a_runner_already_stopped_on_its_own() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let stopped = Some(ran(false, Some(7)));
    let before = [("first".to_owned(), stopped)];
    let locked = runners::lock(config.path());

    note_drains_began(config.path(), &before, &locked);

    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(!kept.held);
    assert_eq!(
        drain_look(&kept, ran(false, Some(7))),
        DrainLook::StoppedBefore
    );
}

/// **A held runner is the person's**: a look's drain refuses it, whatever Docker says now.
#[test]
fn a_look_never_removes_a_held_runner() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    hold(config.path(), "first").unwrap();

    let refused = drain(config.path(), &fake.docker(), "first", None).unwrap_err();

    assert!(refused.to_string().contains("is held"), "{refused}");
    assert!(runners::named(config.path(), "first").unwrap().is_some());
    assert!(remove_held(config.path(), &fake.docker(), "first", None).is_ok());
}

/// **Replace notes the stop of a runner it finds stopped, as it was before anything was made**:
/// read then, the answers after it are of the new runner. It is not held: its home decides.
#[test]
fn replacing_a_stopped_runner_notes_its_stop_and_leaves_it_to_its_home() {
    use crate::poollimits::LimitId;
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    runners::set_pool_limit(config.path(), "first", LimitId::Cpus, Some(1500)).unwrap();
    let next = "first-0a1b2c3d";
    makes(&fake, next, IMAGE_OF_FIRST, &successor_answers());
    let mut inspect: serde_json::Value = serde_json::from_str(&fake.file("inspect")).unwrap();
    inspect[0]["HostConfig"]["NanoCpus"] = 1_500_000_000_u64.into();
    fake.set("inspect", &inspect.to_string());
    let mut stopped = inspected();
    stopped[0]["State"]["Running"] = false.into();
    stopped[0]["State"]["FinishedAt"] = "2026-11-01T00:00:00Z".into();
    fake.set("inspect-once", &stopped.to_string());

    replace_as(config.path(), &fake.docker(), "first", next, &ours)
        .unwrap()
        .unwrap();

    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.draining && !kept.held, "{kept:?}");
    assert_eq!(kept.last_stop, Some(1_793_491_200));
}

/// A held runner whose container a prune took is still the person's to remove: what is left is
/// its home.
#[test]
fn a_held_runner_with_no_container_left_can_be_removed() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    hold(config.path(), "first").unwrap();
    fake.set("inspect", "no such container");
    fake.set("volumes", "purlis-runner-first-home\n");

    let removed = remove_held(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(removed.forgotten, "{removed:?}");
}

/// **Nor does it take sign-ins from a home made since**, as a runner removed outside purlis does
/// not (#4167): a prune took its home too, and a volume of that name is someone else's.
#[test]
fn a_held_runner_gives_nothing_from_a_home_made_since() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
    a_pool_draining(config.path(), &fake);
    hold(config.path(), "first").unwrap();
    fake.set("inspect", "no such container");
    fake.set("volumes", "purlis-runner-first-home\n");
    home_prints(&fake, "2026-01-01T00:00:00Z");

    remove_held(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
}

/// **With no engine kept, a Docker that lists neither its container nor its home may be another
/// engine** (#4149): the press is refused, and the row stays to find it again.
#[test]
fn a_held_runner_docker_may_only_have_lost_sight_of_is_not_forgotten() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    hold(config.path(), "first").unwrap();
    fake.set("inspect", "no such container");
    fake.set("volumes", "");
    fake.set("containers", "");

    let refused = remove_held(config.path(), &fake.docker(), "first", None).unwrap_err();

    assert!(refused.to_string().contains("another"), "{refused}");
    assert!(runners::named(config.path(), "first").unwrap().is_some());
}

/// **The person removes a held runner, and only a held one**: its home goes as a drain's does.
#[test]
fn a_held_runner_is_removed_by_the_person_and_any_other_is_refused() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);

    let refused = remove_held(config.path(), &fake.docker(), "first", None).unwrap_err();
    assert!(refused.to_string().contains("is not held"), "{refused}");

    let mut stopped = inspected();
    stopped[0]["State"]["Running"] = false.into();
    stopped[0]["State"]["FinishedAt"] = "2026-11-01T00:00:00Z".into();
    fake.set("inspect", &stopped.to_string());
    let removed = remove_held(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(removed.forgotten, "{removed:?}");
    assert!(runners::named(config.path(), "first").unwrap().is_none());
}

/// **A profile signed out of the pool is signed out on its draining runners too, and no drain
/// carries it** (#4121, #4145), what the sign-out did said runner by runner.
#[test]
fn signing_out_of_a_pool_reaches_its_draining_runners_and_nothing_is_carried_for_it() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining_since_the_mark(config.path(), &fake);
    // Docker answers for the draining runner, as it was made.
    fake.set("inspect", &inspected().to_string());
    fake.set(
        "volume",
        r#"[{"Name":"purlis-runner-first-home","Driver":"local","Options":null,"Labels":null}]"#,
    );

    let said = sign_out_draining(config.path(), &fake.docker(), "first", "work", &[]).unwrap();

    assert_eq!(said.len(), 1, "only the draining runner: {said:?}");
    assert_eq!(said[0].0, "first");
    said[0].1.as_ref().unwrap();
    assert!(fake.file("log").contains("profile sign-out work"));

    std::fs::write(fake.file_path("log"), "").unwrap();
    drain(config.path(), &fake.docker(), "first", None).unwrap();
    let log = fake.file("log");
    assert!(log.contains("-copy "), "{log}");
    assert!(
        log.split_whitespace().any(|word| word == "work"),
        "the carry names what it leaves: {log}"
    );
}

/// **A draining runner the sign-out could not reach carries nothing for it either** (#4145): the
/// mark is written first.
#[test]
fn a_draining_runner_a_sign_out_did_not_reach_carries_nothing_for_it() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    // Its container no longer answers as it was made, so its sign-out is refused.
    fake.set("inspect", "[]");

    let said = sign_out_draining(config.path(), &fake.docker(), "first", "work", &[]).unwrap();

    assert!(said[0].1.is_err(), "{said:?}");
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert_eq!(kept.signed_out, ["work"]);
}

/// **A second look of one sign-out runs the harness's on no runner the first answered for**
/// (#4298), and one that began to drain since is still reached.
#[test]
fn a_sign_out_runs_once_on_each_draining_runner() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    fake.set("inspect", &inspected().to_string());
    fake.set(
        "volume",
        r#"[{"Name":"purlis-runner-first-home","Driver":"local","Options":null,"Labels":null}]"#,
    );
    let first = sign_out_draining(config.path(), &fake.docker(), "first", "work", &[]).unwrap();
    assert_eq!(first.len(), 1);
    std::fs::write(fake.file_path("log"), "").unwrap();

    let second = sign_out_draining(config.path(), &fake.docker(), "first", "work", &first).unwrap();

    assert!(second.is_empty(), "{second:?}");
    let log = fake.file("log");
    assert!(!log.contains("profile sign-out"), "{log}");

    let unseen = [("another".to_owned(), Ok(()))];
    let reached = sign_out_draining(config.path(), &fake.docker(), "first", "work", &unseen);
    assert_eq!(reached.unwrap().len(), 1);
    assert!(fake.file("log").contains("profile sign-out work"));
}

/// A pool with one runner draining whose home its row knows, and whose carry and volume removal
/// both fail on Docker's error.
fn a_drain_docker_failed(config: &std::path::Path, fake: &FakeDocker) {
    fake.set("engine", "engine-a\n");
    home_prints(fake, HOME_CREATED);
    a_pool_draining(config, fake);
    // The fake has one volume, and making the successor printed over it.
    home_prints(fake, HOME_CREATED);
    fake.set("copy-fails", "");
    fake.set("rm-fails-volume", "1");
    fake.set("volumes", "purlis-runner-first-home\n");
    let first = drain(config, &fake.docker(), "first", None).unwrap();
    assert!(!first.forgotten, "{first:?}");
    let kept = runners::named(config, "first").unwrap().unwrap();
    assert!(kept.carry_failed && !kept.carried, "kept across a read");
    std::fs::remove_file(fake.docker().program().with_file_name("copy-fails")).unwrap();
    std::fs::write(fake.file_path("log"), "").unwrap();
}

/// **A carry that failed on Docker's error is tried again at the next look** (#4298), where the
/// removal failed too and the runner is still there, and then not again.
#[test]
fn a_carry_docker_failed_is_tried_once_more_at_the_next_look() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_drain_docker_failed(config.path(), &fake);

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(fake.file("log").contains("-copy "), "{}", fake.file("log"));
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.carried && !kept.carry_failed, "{kept:?}");

    std::fs::write(fake.file_path("log"), "").unwrap();
    drain(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(!fake.file("log").contains("-copy "), "{}", fake.file("log"));
}

/// A second failure of Docker's ends the tries: a carry can hold the runners' lock for minutes.
#[test]
fn a_carry_docker_failed_twice_is_not_tried_again() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_drain_docker_failed(config.path(), &fake);
    fake.set("copy-fails", "");

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(fake.file("log").contains("-copy "), "{}", fake.file("log"));
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.carried && !kept.carry_failed, "{kept:?}");
}

/// **A second try takes only the runner's own home** (#4298): its container may be gone by then,
/// and a volume of that name made since is not its.
#[test]
fn a_second_try_of_a_carry_takes_nothing_from_a_home_made_since() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_drain_docker_failed(config.path(), &fake);
    home_prints(&fake, "2026-01-01T00:00:00Z");

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    assert!(!fake.file("log").contains("-copy "), "{}", fake.file("log"));
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.carried && !kept.carry_failed, "{kept:?}");
}

/// **A carry stopped by a rule is not Docker's failure**, and is not tried again (#4298).
#[test]
fn a_carry_a_rule_stopped_is_not_tried_again() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    crate::runnertaint::mark(config.path(), &kept.made.container).unwrap();
    fake.set("rm-fails-volume", "1");
    fake.set("volumes", "purlis-runner-first-home\n");

    drain(config.path(), &fake.docker(), "first", None).unwrap();

    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.carried && !kept.carry_failed, "{kept:?}");
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

/// **No sign-in crosses to or from a runner a chat ran on without the sandbox**: not from one, so
/// a sign-in it could have changed reaches no later runner, and not to one, so none it never had
/// reaches it. The runner is removed all the same, and its profiles are signed in again.
#[test]
fn a_drain_carries_no_sign_in_to_or_from_a_runner_a_chat_ran_on_without_the_sandbox() {
    for marked_drained in [true, false] {
        let config = tempfile::tempdir().unwrap();
        let ours = desktop();
        let fake = a_docker(&runner_answers());
        a_pool_added(config.path(), &fake, &ours);
        let drained = runners::runner_for(config.path(), "first").unwrap();
        let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();
        let successor = crate::dockerpool::successor_name("first", &new_image);
        makes(&fake, &successor, NEW, &successor_answers());
        follow(config.path(), &fake.docker(), "first", &new_image, &ours).unwrap();
        let new = runners::runner_for(config.path(), "first").unwrap();
        let marked = if marked_drained { &drained } else { &new };
        crate::runnertaint::mark(config.path(), &marked.made.container).unwrap();
        std::fs::write(fake.file_path("log"), "").unwrap();

        let removed = drain(config.path(), &fake.docker(), "first", None).unwrap();

        assert!(removed.forgotten);
        let log = fake.file("log");
        assert!(!log.contains("-copy "), "{marked_drained}: {log}");
        assert!(
            log.contains("volume rm --force purlis-runner-first-home"),
            "{log}"
        );
    }
}

/// **A copy container an earlier carry left holds no later one back** (#4055): one of this
/// desktop's, of the pool's runner's copy name, is removed by its id before the carry runs.
#[test]
fn a_copy_container_an_earlier_carry_left_is_removed_before_the_next_carry() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
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

    /// `team` added under `config`, and a second runner the fake cluster answers as from now on.
    fn added_and_a_second_runner(config: &Path, cluster: &FakeCluster) -> Pinned {
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
        let second = Pinned {
            device: "01JB0RUNNER000000000000001".into(),
            key: LinkKey::generate().unwrap().public(),
        };
        cluster.write("paired.txt", &crate::runnerlink::to_line(&second));
        second
    }

    /// **A chat's fresh runner is its own** (#3709): nothing that asks for the pool's runner
    /// finds it, the file names its chat, and it is asked for as a fresh claim.
    #[test]
    fn a_chats_fresh_runner_is_kept_beside_its_pools_own_and_found_by_the_chat_alone() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let second = added_and_a_second_runner(config.path(), &cluster);
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", second.key);

        let kept = claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            "01CHAT",
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(kept.pinned, second);
        assert_eq!(kept.chat.as_deref(), Some("01CHAT"));
        assert!(
            kept.reached().starts_with("team/fresh-"),
            "{}",
            kept.reached()
        );
        let (_, pools_own) = runners::cluster_runner_for(config.path(), "team")
            .unwrap()
            .unwrap();
        assert_eq!(pools_own.pinned, cluster.runner);
        assert_eq!(
            runners::fresh_runner_of(config.path(), "01CHAT")
                .unwrap()
                .map(|(_, one)| one),
            Some(kept)
        );
        assert_eq!(file(config.path())["runners"][1]["chat"], "01CHAT");
        // Read back whole, from the file, by a purlis that knows the key.
        assert_eq!(runners::fresh_runners(config.path()).unwrap().len(), 1);
    }

    /// **A link to a chat's own runner is checked as one to its pool's is** (#3709): named whole, it
    /// finds that runner, the pool's pin is what vouches for it, and no other pool's name does.
    #[test]
    fn a_fresh_runner_is_reached_by_its_own_name_and_no_other() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let second = added_and_a_second_runner(config.path(), &cluster);
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", second.key);
        let kept = claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            "01CHAT",
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();

        let name = kept.link_name();
        let (_, found) = runners::cluster_runner_for(config.path(), &name)
            .unwrap()
            .unwrap();
        assert_eq!(found, kept);
        assert!(runners::cluster_runner_for(config.path(), "team/fresh-other").is_err());
        assert_eq!(
            runners::cluster_runner_for(config.path(), "other/fresh-x").unwrap(),
            None
        );
        // A link made for the pool's own runner is not this one's, nor the other way round.
        let (_, own) = runners::cluster_runner_for(config.path(), "team")
            .unwrap()
            .unwrap();
        assert_ne!(own.reached(), kept.reached());
    }

    #[test]
    fn a_fresh_runner_goes_with_its_chat_and_a_claim_left_keeps_its_record() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let second = added_and_a_second_runner(config.path(), &cluster);
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", second.key);
        let chat = "01CHAT";
        claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            chat,
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();

        release_fresh(config.path(), &cluster.kubectl(), chat).unwrap();

        assert!(deleted(&cluster));
        assert_eq!(runners::fresh_runner_of(config.path(), chat).unwrap(), None);
        assert!(
            runners::cluster_runner_for(config.path(), "team")
                .unwrap()
                .is_some()
        );
        // A chat with none has nothing to release.
        release_fresh(config.path(), &cluster.kubectl(), chat).unwrap();
    }

    #[test]
    fn a_runner_answering_as_the_pools_own_is_not_kept_as_a_fresh_one() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let _ = added_and_a_second_runner(config.path(), &cluster);
        cluster.write("paired.txt", &crate::runnerlink::to_line(&cluster.runner));
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", cluster.runner.key);

        claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            "01CHAT",
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .expect_err("one runner is never two");

        assert_eq!(runners::fresh_runners(config.path()).unwrap(), []);
    }

    #[test]
    fn removing_a_pool_takes_its_chats_fresh_runners_with_it() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let second = added_and_a_second_runner(config.path(), &cluster);
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", second.key);
        claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            "01CHAT",
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();

        // Made again by someone: its cluster says what it held is gone.
        let mut another: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cluster.path("claim.json")).unwrap())
                .unwrap();
        another["metadata"]["uid"] = serde_json::json!("0b2f1a54-4d7b-4a64-9c0e-000000000000");
        cluster.write("claim.json", &another.to_string());

        remove_cluster(
            config.path(),
            Some(&cluster.kubectl()),
            "team",
            Some(OURS),
            None,
        )
        .unwrap();

        assert!(
            cluster
                .calls()
                .contains(&format!("delete claims.runners.purlis.dev {claim}"))
        );
        assert_eq!(runners::fresh_runners(config.path()).unwrap(), []);
    }

    /// **No removal deletes a Claim that may hold a task's commits** (#4569): the person is told
    /// what is left and how to delete it.
    #[test]
    fn removing_a_pool_leaves_a_fresh_runner_still_at_work_and_says_so() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let second = added_and_a_second_runner(config.path(), &cluster);
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", second.key);
        claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            "01CHAT",
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();

        let left = remove_cluster(
            config.path(),
            Some(&cluster.kubectl()),
            "team",
            Some(OURS),
            None,
        )
        .unwrap();

        assert!(
            !cluster
                .calls()
                .contains(&format!("delete claims.runners.purlis.dev {claim}"))
        );
        assert!(left.expect("it says so").contains(&claim));
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

    /// #4204: the opt-in is read from the admin's Pool as it is added, and the person chooses
    /// nothing. Whatever the pool handed in says, the cluster's word is what is recorded.
    #[test]
    fn a_cluster_pool_records_the_mint_opt_in_its_pool_says_as_it_is_added() {
        for (forge, recorded) in [
            (
                serde_json::json!({"appKeySecretRef": {"name": "org-app"}, "repos": ["acme/*"], "mintInCluster": true}),
                Some(true),
            ),
            (
                serde_json::json!({"appKeySecretRef": {"name": "org-app"}, "repos": ["acme/*"]}),
                None,
            ),
        ] {
            let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
            let claim = claim_name(OURS, "team");
            cluster.vouch("ada", &claim, &claim, cluster.runner.key);
            cluster.pool_says_forge(forge);
            add_cluster(
                config.path(),
                &cluster.kubectl(),
                &team(&cluster).minting_in_cluster(recorded.is_none()),
                &desktop(),
                Duration::ZERO,
            )
            .unwrap();
            let row = &file(config.path())["pools"][0];
            assert_eq!(
                row.get("mintInCluster").and_then(|it| it.as_bool()),
                recorded
            );
            match runners::runner_of(config.path(), "team").unwrap() {
                Reached::Cluster(pool, _) => {
                    assert_eq!(pool.mints_in_cluster(), recorded == Some(true));
                }
                other => panic!("{other:?}"),
            }
        }
    }

    /// A Pool may opt in after it was added, and the opt-in reaches this machine at a link (#4330).
    #[test]
    fn a_pool_that_opts_in_later_is_kept_so_and_nothing_else_of_it_moves() {
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
        let before = file(config.path());
        assert!(runners::keep_cluster_minting(config.path(), "team", true).unwrap());
        let mut after = file(config.path());
        assert_eq!(after["pools"][0]["mintInCluster"], true);
        assert_eq!(after["pools"][0]["mintNoticeDue"], true);
        let row = after["pools"][0].as_object_mut().unwrap();
        row.remove("mintInCluster");
        row.remove("mintNoticeDue");
        assert_eq!(after, before);
        assert!(!runners::keep_cluster_minting(config.path(), "no-such-pool", true).unwrap());
    }

    /// The Notice of #4442: one for each change from asking to not asking, nothing for a change
    /// the other way or for reading the same again, and it lasts until it is put away.
    #[test]
    fn a_pool_that_begins_minting_has_one_notice_for_that_change() {
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
        let due = || runners::cluster_pools(config.path()).unwrap()[0].mint_notice_due();
        let keep =
            |opted_in| runners::keep_cluster_minting(config.path(), "team", opted_in).unwrap();
        assert!(!due(), "added as it asks, nothing is due");
        assert!(!keep(false), "still asking");
        assert!(keep(true), "the change is told");
        assert!(due());
        assert!(!keep(true), "read again, it is told no more");
        assert!(due(), "and stays due until it is put away");
        runners::put_away_mint_notice(config.path(), "team").unwrap();
        assert!(!due());
        assert!(
            !keep(true),
            "put away, the same state does not bring it back"
        );
        assert!(!due());
        assert!(
            keep(false),
            "back to asking is told, so a Notice standing goes"
        );
        assert!(!due());
        assert!(keep(true), "and the next change is told again");
        assert!(due());
        assert!(keep(false));
        assert!(!due(), "back to asking, what was due is not true now");
        runners::put_away_mint_notice(config.path(), "no-such-pool").unwrap();
    }

    /// A Pool that already mints when first seen follows what adding a pool says (#4442).
    #[test]
    fn a_pool_that_mints_as_it_is_added_has_no_notice() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let claim = claim_name(OURS, "team");
        cluster.vouch("ada", &claim, &claim, cluster.runner.key);
        cluster.pool_says_forge(serde_json::json!({
            "appKeySecretRef": {"name": "org-app"}, "repos": ["acme/*"], "mintInCluster": true,
        }));
        add_cluster(
            config.path(),
            &cluster.kubectl(),
            &team(&cluster),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(file(config.path())["pools"][0]["mintInCluster"], true);
        assert!(!runners::keep_cluster_minting(config.path(), "team", true).unwrap());
        assert!(
            file(config.path())["pools"][0]
                .get("mintNoticeDue")
                .is_none()
        );
    }

    /// A chat's start waits for its repos, and past its time goes on, the work finishing by itself
    /// (#4331).
    #[test]
    fn work_is_waited_for_at_most_its_time_and_goes_on_past_it() {
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = std::sync::Arc::clone(&finished);
        assert!(
            waited_at_most(Duration::from_secs(30), move || {
                done.store(true, std::sync::atomic::Ordering::SeqCst);
            })
            .unwrap()
        );
        assert!(finished.load(std::sync::atomic::Ordering::SeqCst));

        let (go, held) = std::sync::mpsc::channel::<()>();
        let (came, ended) = std::sync::mpsc::channel::<()>();
        assert!(
            !waited_at_most(Duration::from_millis(10), move || {
                let _ = held.recv();
                let _ = came.send(());
            })
            .unwrap()
        );
        go.send(()).unwrap();
        ended
            .recv_timeout(Duration::from_secs(30))
            .expect("it goes on past its time");
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

    /// **A changed key asks again** (#4495): a Pool that names another key than the one pinned
    /// here, and that the pinned key didn't vouch for, reads as changed, with the key it names
    /// now; one the pinned key rolled to holds; one in another cluster says so.
    #[test]
    fn a_pool_naming_a_key_the_pinned_one_never_vouched_for_reads_as_changed() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let kubectl = cluster.kubectl();
        assert_eq!(
            pin_now(config.path(), &kubectl, "team", runners::now()),
            PinNow::Holds
        );
        let next = crate::clusterpool::tests::Operator::new();
        cluster.rolls_to(&next, runners::now() + 3600);
        assert_eq!(
            pin_now(config.path(), &kubectl, "team", runners::now()),
            PinNow::Holds
        );
        let other = crate::clusterpool::tests::Operator::new().key();
        cluster.names_key(&other);
        assert_eq!(
            pin_now(config.path(), &kubectl, "team", runners::now()),
            PinNow::Changed {
                named: other.spelled()
            }
        );
        cluster.write(
            "pool.json",
            &serde_json::json!({"status": {
                "signingKey": cluster.operator.key().spelled(),
                "cluster": "0f0f0f0f-0f0f-4f0f-8f0f-0f0f0f0f0f0f",
            }})
            .to_string(),
        );
        assert_eq!(
            pin_now(config.path(), &kubectl, "team", runners::now()),
            PinNow::OtherCluster
        );
        // Reading changes nothing kept.
        assert_eq!(
            runners::cluster_pools(config.path()).unwrap()[0]
                .pin()
                .unwrap(),
            cluster.pin()
        );
    }

    /// **Pinning again takes only the key the person confirmed** (#4495): the key the Pool names
    /// now, and only where it is the one they pasted. Another key, or another cluster, leaves the
    /// pin as it was; the key pinned again is what the next link checks.
    #[test]
    fn a_changed_key_is_pinned_again_only_as_the_person_confirmed_it_and_in_the_same_cluster() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let kubectl = cluster.kubectl();
        let pinned = || {
            runners::cluster_pools(config.path()).unwrap()[0]
                .pin()
                .unwrap()
        };
        let new = crate::clusterpool::tests::Operator::new();
        cluster.names_key(&new.key());

        let wrong = crate::clusterpool::tests::Operator::new().key().spelled();
        let why = pin_again(config.path(), &kubectl, "team", &wrong).unwrap_err();
        assert!(why.to_string().contains("isn't the one"), "{why}");
        assert_eq!(pinned(), cluster.pin(), "a refusal leaves the pin");

        cluster.write(
            "pool.json",
            &serde_json::json!({"status": {
                "signingKey": new.key().spelled(),
                "cluster": "0f0f0f0f-0f0f-4f0f-8f0f-0f0f0f0f0f0f",
            }})
            .to_string(),
        );
        let why = pin_again(config.path(), &kubectl, "team", &new.key().spelled()).unwrap_err();
        assert!(why.to_string().contains("another cluster"), "{why}");
        assert_eq!(pinned(), cluster.pin(), "another cluster leaves the pin");

        cluster.names_key(&new.key());
        let pasted = new
            .key()
            .spelled()
            .trim_start_matches("ed25519:")
            .to_uppercase();
        pin_again(config.path(), &kubectl, "team", &pasted).unwrap();
        assert_eq!(pinned().key, new.key());
        assert_eq!(
            pin_now(config.path(), &kubectl, "team", runners::now()),
            PinNow::Holds
        );
    }

    /// Nothing is pinned again for a pool this machine doesn't keep.
    #[test]
    fn a_pool_is_not_pinned_again_where_none_is_kept_by_that_name() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let why = pin_again(
            config.path(),
            &cluster.kubectl(),
            "team",
            &cluster.operator.key().spelled(),
        )
        .unwrap_err();
        assert!(
            why.to_string().contains("no cluster pool named team"),
            "{why}"
        );
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

    /// **Only the cluster's word that a fresh runner is gone lets its work be given up** (#4569).
    #[test]
    fn a_fresh_runner_is_lost_only_where_its_claim_is_gone() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let second = added_and_a_second_runner(config.path(), &cluster);
        let claim = fresh_claim_name(OURS, "team", "01CHAT");
        cluster.vouch("ada", &claim, "fresh-pod", second.key);
        claim_fresh(
            config.path(),
            &cluster.kubectl(),
            "team",
            "01CHAT",
            of_a_project(),
            &desktop(),
            Duration::ZERO,
        )
        .unwrap();

        assert!(
            !fresh_runner_lost(config.path(), &cluster.kubectl(), "01CHAT").unwrap(),
            "its claim is there, and the operator vouches for its pod"
        );
        // Made again by someone: not the claim that held its work.
        let mut another: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cluster.path("claim.json")).unwrap())
                .unwrap();
        another["metadata"]["uid"] = serde_json::json!("0b2f1a54-4d7b-4a64-9c0e-000000000000");
        cluster.write("claim.json", &another.to_string());
        assert!(fresh_runner_lost(config.path(), &cluster.kubectl(), "01CHAT").unwrap());
    }

    /// **A task's fresh runner relies on its operator's signed word, holding now, and silence is
    /// a no** (#4569).
    #[test]
    fn only_an_operator_that_says_so_keeps_a_fresh_runner_through_idle() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        let _ = added_and_a_second_runner(config.path(), &cluster);
        let pool = runners::cluster_pools(config.path()).unwrap().remove(0);
        let now = runners::now();
        assert!(fresh_runners_stay(&pool, &cluster.kubectl(), now).is_err());

        let word = crate::clusterpool::OperatorWord {
            cluster: crate::clusterpool::tests::CLUSTER.to_owned(),
            capabilities: vec![crate::clusterpool::FRESH_RUNNERS_STAY.to_owned()],
            until: now + 600,
        }
        .text();
        let mut said: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cluster.path("pool.json")).unwrap())
                .unwrap();
        said["status"]["capabilitiesVouch"] = serde_json::json!({
            "statement": word,
            "signature": cluster.operator.sign(&word),
        });
        cluster.write("pool.json", &said.to_string());
        assert_eq!(fresh_runners_stay(&pool, &cluster.kubectl(), now), Ok(()));
        // Read again later, once it ended: an operator rolled back signs it no more.
        assert!(fresh_runners_stay(&pool, &cluster.kubectl(), now + 600).is_err());

        cluster.write("pool.json", "not an answer");
        assert!(fresh_runners_stay(&pool, &cluster.kubectl(), now).is_err());
    }

    /// **A fresh Claim a crash left with no record goes once it is old, and nothing else does**
    /// (#4569): a record is written before any chat starts there, so it never held work. One an
    /// earlier pool of the name left as it was removed, with its cluster out of reach, stays.
    #[test]
    fn a_fresh_claim_no_record_holds_is_deleted_once_old_and_only_then() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let added_at = judged(config.path()).paired;
        let at = |seconds: u64| {
            chrono::DateTime::from_timestamp(i64::try_from(seconds).unwrap(), 0)
                .unwrap()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        };
        let now = added_at + 4 * 3600;
        let ours = crate::clusterpool::desktop_label(OURS);
        let named = |hash: &str| format!("{}-{hash}", claim_name(OURS, "team"));
        let item = |name: &str, desktop: &str, kept: bool, made: &str| {
            serde_json::json!({
                "metadata": {
                    "name": name,
                    "uid": "0b2f1a54-4d7b-4a64-9c0e-000000000001",
                    "creationTimestamp": made,
                    "annotations": {crate::clusterpool::DESKTOP_ANNOTATION: desktop},
                },
                "spec": {"pool": "team", "requester": "ada@example.com", "kept": kept},
            })
        };
        let old = at(added_at + 3600);
        cluster.write(
            "claims.json",
            &serde_json::json!({"items": [
                item(&named("0123abcd"), &ours, false, &old),
                // Made within the hour: a start may still be making it.
                item(&named("4567abcd"), &ours, false, &at(now - 60)),
                // Another desktop's, a kept one, the pool's own, another pool's.
                item(&named("89abcdef"), "ffffffffffffffff", false, &old),
                item(&named("fedcba98"), &ours, true, &old),
                item(&claim_name(OURS, "team"), &ours, false, &old),
                item("someone-elses-0123abcd", &ours, false, &old),
                // Left by an earlier pool of this name as it was removed: it may hold work.
                item(&named("aaaabbbb"), &ours, false, &at(added_at - 3600)),
            ]})
            .to_string(),
        );

        let deleted =
            sweep_orphan_fresh_claims(config.path(), &cluster.kubectl(), OURS, now).unwrap();

        assert_eq!(deleted, vec![named("0123abcd")]);
    }

    /// Where a fresh runner's work goes back to.
    fn of_a_project() -> runners::FreshOf {
        runners::FreshOf {
            plane: "01PROJECT".to_owned(),
            workspace: "app".to_owned(),
            root: None,
        }
    }

    fn deleted(cluster: &FakeCluster) -> bool {
        cluster.calls().contains("delete claims.runners.purlis.dev")
    }

    /// The runner kept for `team` under `config`, as a gate judged it.
    fn judged(config: &Path) -> runners::ClusterKept {
        runners::cluster_runner_for(config, "team")
            .unwrap()
            .expect("a cluster pool")
            .1
    }

    use crate::clusterpool::Refused as NotVouched;
    use crate::poollogins::Bound;

    fn vouches(_: &Path, _: &str) -> Result<String, NotVouched> {
        Ok("pod-uid-1".to_owned())
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
            &judged(config.path()),
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
                "--until-input-ends",
            ]
        );
        assert!(login.args.contains(&"--context".to_owned()));
        assert!(login.cleared.contains(&"KUBECONFIG".to_owned()));
        assert!(cluster.calls().contains("get claims.runners.purlis.dev"));

        let typed = cluster_sign_in(
            config.path(),
            &cluster.kubectl(),
            "team",
            &judged(config.path()),
            "work",
            Some("github-copilot"),
            crate::dockerpool::Typed::InATerminal,
        )
        .unwrap();
        assert!(typed.args.contains(&"-it".to_owned()));
        assert!(!typed.args.contains(&"--until-input-ends".to_owned()));
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
                &judged(config.path()),
                "work",
                None,
                crate::dockerpool::Typed::Driven,
            )
            .is_err()
        );
    }

    /// **A cluster runner's sign-in is pinned to the uid its pod had right before the exec**
    ///: the pod of that name read again once the exec reached it holds only where it is
    /// the same pod, not one made again under the name.
    #[test]
    fn a_cluster_sign_in_is_pinned_to_the_pod_it_had_right_before_the_exec() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let login = cluster_sign_in(
            config.path(),
            &cluster.kubectl(),
            "team",
            &judged(config.path()),
            "work",
            None,
            crate::dockerpool::Typed::Driven,
        )
        .unwrap();
        let pin = login.pin.clone().expect("a cluster sign-in is pinned");
        assert!(
            cluster.calls().contains("get pods"),
            "its uid read before: {}",
            cluster.calls()
        );
        assert_eq!(pin.holds(), Ok(()));
        assert!(login.line().is_err(), "never typed into a shell");

        cluster.write("pod.json", r#"{"metadata": {"uid": "pod-uid-2"}}"#);
        assert!(pin.holds().is_err_and(|why| why.contains("made again")));

        cluster.write("pod.json", "{}");
        assert!(pin.holds().is_err());
    }

    /// **A cluster runner's standing is its own `--check` in the vouched pod** (#4164); a runner
    /// not vouched for now is not asked.
    #[test]
    fn a_cluster_runners_standing_is_asked_in_its_vouched_pod_after_the_links_check() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        // What it answers is read by `standing_by_login`'s own test; here, what is asked.
        let _ = cluster_standing(
            config.path(),
            &cluster.kubectl(),
            "team",
            &judged(config.path()),
            "work",
        );
        assert!(
            cluster.calls().contains("sign-in work --check"),
            "{}",
            cluster.calls()
        );

        cluster.write("claim.json", "{}");
        assert!(
            cluster_standing(
                config.path(),
                &cluster.kubectl(),
                "team",
                &judged(config.path()),
                "work",
            )
            .is_err()
        );
    }

    /// **A cluster runner's version and sign-out are asked of its own `purlis` in the vouched pod**
    /// (#3819), after the check every link makes.
    #[test]
    fn a_cluster_runners_version_and_sign_out_run_its_own_purlis_after_the_links_check() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        cluster.write("paired.txt", "purlis 0.4.2 (release profile)\n");
        assert_eq!(
            cluster_runner_version(
                config.path(),
                &cluster.kubectl(),
                "team",
                &judged(config.path())
            ),
            Ok("0.4.2".to_owned())
        );
        assert!(cluster.calls().contains("/usr/local/bin/purlis --version"));
        cluster_sign_out(
            config.path(),
            &cluster.kubectl(),
            "team",
            &judged(config.path()),
            "work",
        )
        .unwrap();
        assert!(
            cluster
                .calls()
                .contains("/usr/local/bin/purlis profile sign-out work")
        );
        // A runner other than the one the gate judged: nothing runs there.
        let mut other = judged(config.path());
        other.pod = "another".into();
        let ran = cluster.calls().len();
        assert!(
            cluster_sign_out(config.path(), &cluster.kubectl(), "team", &other, "work").is_err()
        );
        assert!(!cluster.calls()[ran..].contains("sign-out"));
        cluster.write("paired.txt", "not a version\n");
        assert!(
            cluster_runner_version(
                config.path(),
                &cluster.kubectl(),
                "team",
                &judged(config.path())
            )
            .is_err()
        );
        cluster.write("claim.json", "{}");
        assert!(
            cluster_sign_out(
                config.path(),
                &cluster.kubectl(),
                "team",
                &judged(config.path()),
                "work"
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
            Ok("pod-uid-1".to_owned())
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
        let was = Bound::of_cluster(&pool, &kept, "pod-uid-1");
        let mut claim = kept.clone();
        claim.claimed.uid = "another".into();
        let mut pod = kept.clone();
        pod.pod = "another".into();
        let mut device = kept.clone();
        device.pinned.device = "another".into();
        let mut key = kept.clone();
        key.pinned.key = LinkKey::generate().unwrap().public();
        for changed in [claim, pod, device, key] {
            assert_ne!(
                Bound::of_cluster(&pool, &changed, "pod-uid-1"),
                was,
                "{changed:?}"
            );
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
            Bound::of_cluster(&other("elsewhere", pin.clone()), &kept, "pod-uid-1"),
            was
        );
        let moved = crate::clusterpool::PoolPin {
            cluster: "4e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b".into(),
            ..pin.clone()
        };
        assert_ne!(
            Bound::of_cluster(&other("team", moved), &kept, "pod-uid-1"),
            was
        );
        let rolled = crate::clusterpool::PoolPin {
            key: crate::clusterpool::tests::Operator::new().key(),
            ..pin
        };
        assert_ne!(
            Bound::of_cluster(&other("team", rolled), &kept, "pod-uid-1"),
            was
        );
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

    /// A cluster that does not vouch now is not a runner that can't receive (#5077): told apart
    /// by kind, never by the words, which are the same.
    #[test]
    fn an_unanswered_vouch_is_unconfirmed_and_an_unmarked_pool_or_a_refused_statement_is_not_yours()
    {
        use crate::poollogins::NotSent;
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let link = reached(config.path());
        let kind = |vouch: &dyn Fn(&Path, &str) -> Result<String, NotVouched>| {
            Bound::of_a_cluster_pool_kinded(config.path(), "team", Some(&link), vouch).unwrap_err()
        };
        assert!(matches!(
            kind(&|_, _| Ok("pod-uid-1".to_owned())),
            NotSent::NotYours(_)
        ));
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        for unanswered in [
            NotVouched::WaitRanOut("no statement yet".into()),
            NotVouched::Other("the cluster did not answer".into()),
        ] {
            let not = kind(&|_, _| Err(unanswered.clone()));
            assert!(
                matches!(not, NotSent::Unconfirmed(_)),
                "{unanswered}: {not:?}"
            );
        }
        for refused in [
            NotVouched::OperatorKeyChanged {
                pool: "team".into(),
            },
            NotVouched::NotVouched("a statement about another requester".into()),
        ] {
            let not = kind(&|_, _| Err(refused.clone()));
            assert!(matches!(not, NotSent::NotYours(_)), "{refused}: {not:?}");
        }
    }

    /// What the cluster says beside the statement only ever refuses a value, as it is unsigned.
    #[test]
    fn a_stopped_or_no_longer_vouched_runner_is_sent_no_value() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let kubectl = cluster.kubectl();
        let asked = || vouched_now_with(config.path(), &kubectl, "team", runners::now());
        assert_eq!(asked().unwrap(), "pod-uid-1", "vouched and serving");
        let claim_was = std::fs::read_to_string(cluster.dir.path().join("claim.json")).unwrap();
        let change = |change: &dyn Fn(&mut serde_json::Value)| {
            let mut claim: serde_json::Value = serde_json::from_str(&claim_was).unwrap();
            change(&mut claim);
            cluster.write("claim.json", &claim.to_string());
        };
        for said in [
            serde_json::json!({"type": "KeyPublished", "status": "False", "reason": "NotVouched"}),
            serde_json::json!({"type": "KeyPublished", "status": "False", "reason": "Vouched"}),
            serde_json::json!({"type": "KeyPublished", "status": "True", "reason": "Starting"}),
            serde_json::json!({"type": "Ready", "status": "True", "reason": "Vouched"}),
        ] {
            change(&|claim| {
                claim["status"]["conditions"] = serde_json::json!([said]);
            });
            assert!(matches!(asked(), Err(NotVouched::NotVouched(_))), "{said}");
        }
        change(&|claim| {
            claim["spec"]["stopped"] = true.into();
        });
        assert!(matches!(asked(), Err(NotVouched::NotVouched(_))), "claim");
        cluster.write("claim.json", &claim_was);
        assert!(asked().is_ok());
        let pool_was = std::fs::read_to_string(cluster.dir.path().join("pool.json")).unwrap();
        let mut stopped: serde_json::Value = serde_json::from_str(&pool_was).unwrap();
        stopped["spec"] = serde_json::json!({"stopped": true});
        cluster.write("pool.json", &stopped.to_string());
        assert!(matches!(asked(), Err(NotVouched::NotVouched(_))), "pool");
        cluster.write("pool.json", &pool_was);
        // A pod on its way out, or naming no uid, is no runner to send a value to.
        for pod in [
            r#"{"metadata": {}}"#,
            r#"{"metadata": {"uid": "pod-uid-1", "deletionTimestamp": "2026-10-31T00:00:00Z"}}"#,
        ] {
            cluster.write("pod.json", pod);
            assert!(matches!(asked(), Err(NotVouched::NotVouched(_))), "{pod}");
        }
    }

    /// A kept claim's pod made again under its name, with its key, is not what the person allowed.
    #[test]
    fn a_pod_made_again_under_its_name_asks_again() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        let link = reached(config.path());
        let of_pod = |uid: &'static str| {
            Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &move |_, _| {
                Ok(uid.to_owned())
            })
            .unwrap()
            .unwrap()
        };
        let opt_ins = crate::poollogins::OptIns::default();
        opt_ins.allow("devops", "site", of_pod("pod-uid-1"));
        assert_eq!(
            opt_ins.standing("devops", "site", &of_pod("pod-uid-1")),
            crate::poollogins::Standing::Allowed
        );
        assert_eq!(
            opt_ins.standing("devops", "site", &of_pod("pod-uid-2")),
            crate::poollogins::Standing::Changed
        );
    }

    /// A link is checked against the runner it was made for, under whatever name.
    #[test]
    fn a_link_made_for_a_runner_no_longer_kept_is_refused_before_it_connects() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let kubectl = cluster.kubectl();
        let now = runners::now();
        let link = reached(config.path());
        before_cluster_link_to(config.path(), &kubectl, "team", &link, now).unwrap();
        let earlier = format!("team/team@{}", "ab".repeat(32));
        let refused =
            before_cluster_link_to(config.path(), &kubectl, "team", &earlier, now).unwrap_err();
        assert!(refused.to_string().contains("no longer keeps"), "{refused}");
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
                Ok("pod-uid-1".to_owned())
            })
            .unwrap_err();
        assert!(refused.contains("not marked"), "{refused}");

        runners::set_cluster_owned(config.path(), "team", true).unwrap();
        Bound::of_a_cluster_pool(config.path(), "team", Some(&link), &|config, name| {
            remove_cluster(config, None, name, None, None).unwrap();
            Ok("pod-uid-1".to_owned())
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

    /// `mintInCluster` is `true` alone (#4204): spelled any other way, the file is refused.
    #[test]
    fn a_mint_opt_in_spelled_any_other_way_refuses_the_file() {
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        added(config.path(), &cluster);
        let at = crate::machine::dir(config.path()).join(crate::machine::RUNNERS_FILE);
        for spelled in [serde_json::json!(false), serde_json::json!("true")] {
            let mut file = file(config.path());
            let kept = file.clone();
            file["pools"][0]["mintInCluster"] = spelled;
            std::fs::write(&at, file.to_string()).unwrap();
            runners::cluster_pools(config.path()).unwrap_err();
            std::fs::write(&at, kept.to_string()).unwrap();
        }
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
    fn a_plugin_named_by_its_bare_name_is_kept_in_the_pools_copy_by_the_path_it_was_found_at() {
        use std::os::unix::fs::PermissionsExt;
        let (config, cluster) = (tempfile::tempdir().unwrap(), FakeCluster::new());
        // Found on the PATH its runs are given: the folder kubectl is in.
        let plugin = cluster.dir.path().join("bin").join("auth-plugin");
        std::fs::write(&plugin, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&plugin, std::fs::Permissions::from_mode(0o755)).unwrap();
        cluster.write(
            "context.yaml",
            &serde_json::json!({"apiVersion": "v1", "kind": "Config", "users": [
                {"name": "ada", "user": {"exec": {"command": "auth-plugin", "args": ["token"]}}}
            ]})
            .to_string(),
        );
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
        let kept: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(runners::kubeconfig_kept_at(config.path(), "team")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            kept.pointer("/users/0/user/exec/command"),
            Some(&serde_json::json!(plugin.display().to_string())),
            "{kept}"
        );
        assert_eq!(
            kept.pointer("/users/0/user/exec/args"),
            Some(&serde_json::json!(["token"]))
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

/// The id Docker gives the container a runner is made again in (#4149), and its name.
const AGAIN: &str = "a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1a9a1";
const AGAIN_NAMED: &str = "first-0a0b0c0d";

/// A pool `first` added, whose container the person then removed outside purlis: Docker lists
/// no container of its id, and its home volume where `home_made` says when it was made. What
/// Docker makes next answers as runner [`AGAIN_NAMED`].
fn removed_outside_purlis(
    config: &Path,
    fake: &FakeDocker,
    ours: &Pinned,
    home_made: Option<&str>,
) -> Kept {
    let old = a_pool_added(config, fake, ours);
    fake.set("containers", "");
    fake.set(
        "volumes",
        if home_made.is_some() {
            "purlis-runner-first-home\n"
        } else {
            ""
        },
    );
    fake.set("id", &format!("{AGAIN}\n"));
    makes(fake, AGAIN_NAMED, IMAGE_OF_FIRST, &successor_answers());
    let mut inspect: serde_json::Value = serde_json::from_str(&fake.file("inspect")).unwrap();
    inspect[0]["Id"] = AGAIN.into();
    fake.set("inspect", &inspect.to_string());
    let mut volume: serde_json::Value = serde_json::from_str(&fake.file("volume")).unwrap();
    volume[0]["CreatedAt"] = home_made.unwrap_or("2026-01-01T00:00:00Z").into();
    fake.set("volume", &volume.to_string());
    // Its home holds no conversation, unless a test says otherwise (#4859).
    fake.set("conversations", "conversations 0 0 0\n");
    std::fs::write(fake.file_path("log"), "").unwrap();
    old
}

fn make_again_here(config: &Path, fake: &FakeDocker, ours: &Pinned) -> Result<MadeAgain, Refused> {
    make_again_as(config, &fake.docker(), "first", AGAIN_NAMED, ours)
}

/// **A runner made before the mark is made again with nothing carried** (ruled 2026-11-08), as a
/// drain carries nothing from it: its row keeps no engine or home, or its image's purlis is older.
#[test]
fn a_runner_made_before_the_mark_is_made_again_with_nothing_carried() {
    for older in [false, true] {
        let config = tempfile::tempdir().unwrap();
        let ours = desktop();
        let fake = a_docker(&runner_answers());
        if older {
            removed_with_its_making_kept(config.path(), &fake, &ours);
            fake.set("version", "purlis 0.4.1 (release profile)\n");
        } else {
            removed_outside_purlis(config.path(), &fake, &ours, Some("2026-01-01T00:00:00Z"));
        }

        let again = make_again_here(config.path(), &fake, &ours).unwrap();

        assert!(!again.sign_ins_kept, "older image: {older}");
        // The carry writes the new home; the read of the old one for conversations does not.
        assert!(
            !fake.file("log").contains("target=/to"),
            "{}",
            fake.file("log")
        );
    }
}

/// **Make the runner again** (#4149): a runner removed outside purlis is made again beside it,
/// from the pool's own image and limits, on a home of its own with the sign-ins of the old home
/// carried over where that home is the one purlis made for it, paired, and kept as the pool's
/// runner; the gone one is removed, its home with it.
#[test]
fn a_runner_removed_outside_purlis_is_made_again_with_its_sign_ins_and_paired() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = removed_with_its_making_kept(config.path(), &fake, &ours);

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert!(again.sign_ins_kept);
    assert_eq!(again.kept.name(), AGAIN_NAMED);
    assert_eq!(again.kept.of.as_deref(), Some("first"));
    assert_eq!(again.kept.made.container.as_str(), AGAIN);
    assert_eq!(again.kept.pinned.device, successor_answers().device);
    // Only the runner made again is kept: the gone one is removed, and never drains.
    assert_eq!(
        runners::read(config.path()).unwrap(),
        vec![again.kept.clone()]
    );
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().name(),
        AGAIN_NAMED
    );
    let log = fake.file("log");
    assert!(log.contains("--cpus 2 --memory 4096m"), "{log}");
    assert!(
        log.contains("type=volume,source=purlis-runner-first-home,target=/from,readonly"),
        "the sign-ins were carried from the old home: {log}"
    );
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
    assert_ne!(old.made, again.kept.made);
}

/// A runner a chat ran on without the sandbox is made again with nothing carried, its home there
/// or not: a new runner starts on no sign-in that chat could have changed.
#[test]
fn a_runner_a_chat_ran_on_without_the_sandbox_is_made_again_with_nothing_carried() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_outside_purlis(config.path(), &fake, &ours, Some("2026-01-01T00:00:00Z"));
    crate::runnertaint::mark_pool(config.path(), "first").unwrap();
    std::fs::write(fake.file_path("log"), "").unwrap();

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert!(!again.sign_ins_kept);
    let log = fake.file("log");
    assert!(!log.contains("target=/to"), "{log}");
}

/// Its home gone too, the runner is made again with nothing carried: its profiles are signed in
/// again.
#[test]
fn a_runner_whose_home_went_with_it_is_made_again_to_sign_in_again() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_outside_purlis(config.path(), &fake, &ours, None);

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert!(!again.sign_ins_kept);
    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
}

/// **A runner made again keeps the gone one held where its home holds a chat's conversation**
/// (#4859, the rule of #4633): its home is read by a container of the runner made again, and it
/// stays, draining and held, for the person to bring them back or discard them in Devices.
#[test]
fn a_runner_made_again_keeps_the_gone_ones_home_where_it_holds_a_conversation() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_with_its_making_kept(config.path(), &fake, &ours);
    fake.set("conversations", "conversations 3 3 0\n");

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert_eq!(again.old_home, OldHome::Held);
    let gone = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(gone.draining && gone.held, "{gone:?}");
    // Its sign-ins came already, so a later Discard or bring back carries none again.
    assert!(gone.carried, "{gone:?}");
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().name(),
        again.kept.name()
    );
    let log = fake.file("log");
    assert!(!log.contains("volume rm"), "{log}");
    // Read alone: the home mounted read-only, and no home mounted to write to.
    let read = log
        .split("run --rm")
        .find(|run| run.contains(" seen /from - "))
        .expect(&log);
    assert!(
        read.contains("source=purlis-runner-first-home,target=/from,readonly"),
        "{read}"
    );
    assert!(!read.contains("target=/to"), "{read}");
}

/// **A gone runner's home that holds no conversation goes as it did**: removed once the runner
/// is made again.
#[test]
fn a_runner_made_again_removes_the_gone_ones_home_where_it_holds_none() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_outside_purlis(config.path(), &fake, &ours, Some("2026-01-01T00:00:00Z"));
    fake.set("conversations", "conversations 0 0 0\n");

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert_eq!(again.old_home, OldHome::Removed);
    assert_eq!(runners::read(config.path()).unwrap(), vec![again.kept]);
    let log = fake.file("log");
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
}

/// **A gone runner's home that could not be read is kept, held** (#4859): Make the runner again
/// is pressed once, and no later look is sure to read it, so the person decides in Devices.
#[test]
fn a_runner_made_again_holds_the_gone_one_whose_home_could_not_be_read() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_outside_purlis(config.path(), &fake, &ours, Some("2026-01-01T00:00:00Z"));
    fake.set("conversations", "");

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert_eq!(again.old_home, OldHome::Unread);
    let gone = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(gone.draining && gone.held, "{gone:?}");
    assert!(
        !fake.file("log").contains("volume rm"),
        "{}",
        fake.file("log")
    );
}

/// What a home printed as it was made in the tests below (#4167).
const HOME_CREATED: &str = "2099-06-01T10:00:00+02:00";

/// Docker's home volume, as it is, printing `at` as when it was made.
fn home_prints(fake: &FakeDocker, at: &str) {
    let mut volume: serde_json::Value = serde_json::from_str(&fake.file("volume")).unwrap();
    volume[0]["CreatedAt"] = at.into();
    fake.set("volume", &volume.to_string());
}

/// A pool added on engine `engine-a` whose home printed [`HOME_CREATED`] as it was made (#4166,
/// #4167), then its container removed outside purlis with its home left.
fn removed_with_its_making_kept(config: &Path, fake: &FakeDocker, ours: &Pinned) -> Kept {
    fake.set("engine", "engine-a\n");
    home_prints(fake, HOME_CREATED);
    removed_outside_purlis(config, fake, ours, Some(HOME_CREATED))
}

/// **A runner keeps the engine that made it and its home's `CreatedAt`** (#4166, #4167), and
/// they stay when it is read back.
#[test]
fn a_runner_keeps_its_engine_and_its_homes_making_through_the_file() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);

    let kept = a_pool_added(config.path(), &fake, &desktop());

    assert_eq!(kept.engine.as_deref(), Some("engine-a"));
    assert_eq!(kept.home_created.as_deref(), Some(HOME_CREATED));
    let read = runners::runner_for(config.path(), "first").unwrap();
    assert_eq!(read, kept);
}

/// **Sign-ins come from the runner's own home whatever the clocks say** (#4167): this home's
/// time is far past its pairing, which the clock rule would refuse, and it prints what was kept.
#[test]
fn a_home_still_printing_what_was_kept_gives_its_sign_ins_whatever_the_clocks_say() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_with_its_making_kept(config.path(), &fake, &ours);

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert!(again.sign_ins_kept, "{}", fake.file("log"));
}

/// **A volume made again under the same name gives nothing** (#4167), even with a time the clock
/// rule would pass.
#[test]
fn a_volume_made_again_under_the_runners_name_gives_nothing() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_with_its_making_kept(config.path(), &fake, &ours);
    home_prints(&fake, "2026-01-01T00:00:00Z");

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert!(!again.sign_ins_kept);
    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
}

/// **Make the runner again makes nothing on another engine** (#4166).
#[test]
fn a_runner_is_not_made_again_on_another_engine() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = removed_with_its_making_kept(config.path(), &fake, &ours);
    fake.set("engine", "engine-b\n");

    let refused = make_again_here(config.path(), &fake, &ours).unwrap_err();

    assert!(refused.to_string().contains("another engine"), "{refused}");
    assert!(!fake.file("log").contains("run "), "{}", fake.file("log"));
    assert_eq!(runners::runner_for(config.path(), "first").unwrap(), old);
}

/// **An update makes no runner on another engine** (#4166): the pool stays on the one it runs on.
#[test]
fn a_follow_makes_nothing_on_another_engine() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    let old = a_pool_added(config.path(), &fake, &ours);
    fake.set("engine", "engine-b\n");
    std::fs::write(fake.file_path("log"), "").unwrap();
    let new_image = crate::dockerpool::Image::pinned(NEW).unwrap();

    let refused = follow(config.path(), &fake.docker(), "first", &new_image, &ours);

    assert!(refused.is_err());
    assert!(!fake.file("log").contains("run "), "{}", fake.file("log"));
    assert_eq!(runners::runner_for(config.path(), "first").unwrap(), old);
}

/// **Removing a pool whose runner is on another engine touches nothing there** (#4325), and names
/// what stays and how to remove it. The pool is forgotten, so one whose engine is gone for good
/// can still be removed.
#[test]
fn a_pool_removed_while_another_engine_answers_names_what_stays_there() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    let kept = a_pool_added(config.path(), &fake, &ours);
    fake.set("engine", "engine-b\n");
    std::fs::write(fake.file_path("log"), "").unwrap();

    assert!(on_another_engine(config.path(), &fake.docker(), "first"));
    let removed = remove(config.path(), &fake.docker(), "first", None)
        .unwrap()
        .unwrap();

    assert!(!fake.file("log").contains(" rm"), "{}", fake.file("log"));
    assert!(removed.forgotten);
    let left = removed.left().unwrap();
    assert!(
        left.contains(&format!(
            "docker container rm --force {}",
            kept.made.container.as_str()
        )),
        "{left}"
    );
    assert!(
        left.contains(&format!(
            "docker volume rm --force {}",
            kept.made.runner.volume()
        )),
        "{left}"
    );
    assert!(left.contains("stays"), "{left}");
    assert!(left.contains("engine-a"), "names the engine: {left}");
    assert!(left.contains("chats"), "{left}");
    assert!(left.contains("sign-ins"), "{left}");
    assert!(runners::runner_for(config.path(), "first").is_err());
    assert!(
        !runners::pools(config.path())
            .unwrap()
            .iter()
            .any(|pool| pool.name() == "first")
    );
}

/// **A pool's draining runner on another engine is named too**, beside the pool's own (#4325).
#[test]
fn a_pool_removed_on_another_engine_names_each_of_its_runners() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    a_pool_draining(config.path(), &fake);
    let names: Vec<String> = runners::read(config.path())
        .unwrap()
        .iter()
        .map(|kept| kept.made.runner.volume())
        .collect();
    assert_eq!(names.len(), 2);
    fake.set("engine", "engine-b\n");

    let left = remove(config.path(), &fake.docker(), "first", None)
        .unwrap()
        .unwrap()
        .left()
        .unwrap();

    for volume in names {
        assert!(left.contains(&volume), "{left}");
    }
}

/// **Where nothing says which engine answers, a removal touches nothing and keeps the row**
/// (#4325): what is by name might be another engine's.
#[test]
fn a_runner_is_not_removed_where_no_engine_says_which_it_is() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    a_pool_added(config.path(), &fake, &ours);
    assert!(!on_another_engine(config.path(), &fake.docker(), "first"));
    fake.set("engine", "\n");
    assert!(!on_another_engine(config.path(), &fake.docker(), "first"));
    std::fs::write(fake.file_path("log"), "").unwrap();

    let removed = remove(config.path(), &fake.docker(), "first", None)
        .unwrap()
        .unwrap();

    assert!(!fake.file("log").contains(" rm"), "{}", fake.file("log"));
    assert!(!removed.forgotten);
    assert!(removed.left().unwrap().contains("can be removed again"));
    assert!(runners::runner_for(config.path(), "first").is_ok());
}

/// **A draining runner is not drained on another engine's word** (#4166): its row stays.
#[test]
fn a_draining_runner_stays_while_another_engine_answers() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    a_pool_draining(config.path(), &fake);
    let name = "first";
    let kept = runners::named(config.path(), name).unwrap().unwrap();
    assert!(kept.draining);
    assert_eq!(kept.engine.as_deref(), Some("engine-a"));
    fake.set("engine", "engine-b\n");
    std::fs::write(fake.file_path("log"), "").unwrap();

    assert!(drain(config.path(), &fake.docker(), name, None).is_err());

    assert!(!fake.file("log").contains(" rm"), "{}", fake.file("log"));
    assert_eq!(runners::named(config.path(), name).unwrap(), Some(kept));
}

/// **A draining runner the engine that made it no longer lists is removed** (#4168), with what
/// its own home holds carried; another engine's word removes nothing.
#[test]
fn a_draining_runner_removed_outside_purlis_goes_with_its_own_homes_sign_ins_carried() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
    a_pool_draining(config.path(), &fake);
    // The fake has one volume, and making the successor printed over it.
    home_prints(&fake, HOME_CREATED);
    fake.set("volumes", "purlis-runner-first-home\n");
    fake.set("containers", "");

    fake.set("engine", "engine-b\n");
    assert!(drain_removed(config.path(), &fake.docker(), "first").is_err());
    assert!(runners::named(config.path(), "first").unwrap().is_some());

    fake.set("engine", "engine-a\n");
    drain_removed(config.path(), &fake.docker(), "first").unwrap();
    assert!(
        fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
    assert_eq!(runners::named(config.path(), "first").unwrap(), None);
}

/// A home that is not its own gives nothing as a removed draining runner goes (#4168).
#[test]
fn a_draining_runner_removed_outside_purlis_takes_nothing_from_a_home_made_since() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
    a_pool_draining(config.path(), &fake);
    // The fake has one volume, and making the successor printed over it.
    home_prints(&fake, HOME_CREATED);
    fake.set("volumes", "purlis-runner-first-home\n");
    fake.set("containers", "");
    home_prints(&fake, "2026-01-01T00:00:00Z");

    drain_removed(config.path(), &fake.docker(), "first").unwrap();

    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
    assert_eq!(runners::named(config.path(), "first").unwrap(), None);
}

/// A removal that failed does not carry again at the next look (#4168).
#[test]
fn a_draining_runner_removed_outside_purlis_carries_once() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
    a_pool_draining(config.path(), &fake);
    // The fake has one volume, and making the successor printed over it.
    home_prints(&fake, HOME_CREATED);
    fake.set("volumes", "purlis-runner-first-home\n");
    fake.set("containers", "");
    fake.set("rm-fails-volume", "1");

    drain_removed(config.path(), &fake.docker(), "first").unwrap();
    assert!(
        fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
    assert!(
        runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .carried
    );
    std::fs::write(fake.file_path("log"), "").unwrap();
    fake.set("volumes", "purlis-runner-first-home\n");
    drain_removed(config.path(), &fake.docker(), "first").unwrap();

    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
}

/// With no engine kept, a runner is not taken as removed (#4168).
#[test]
fn a_draining_runner_with_no_engine_kept_is_not_taken_as_removed() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    fake.set("containers", "");

    assert!(drain_removed(config.path(), &fake.docker(), "first").is_err());
    assert!(runners::named(config.path(), "first").unwrap().is_some());
}

/// A runner the engine still lists is not drained this way (#4168).
#[test]
fn a_draining_runner_still_listed_is_not_taken_as_removed() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    a_pool_draining(config.path(), &fake);
    fake.set("containers", &format!("{ID}\n"));

    assert!(drain_removed(config.path(), &fake.docker(), "first").is_err());
    assert!(runners::named(config.path(), "first").unwrap().is_some());
}

/// A home made after the runner was paired is not the one purlis made for it, so none of it is
/// carried: whoever made it could have put anything there.
#[test]
fn a_home_made_since_the_runner_was_paired_gives_it_nothing() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    removed_outside_purlis(config.path(), &fake, &ours, Some("2099-01-01T00:00:00Z"));

    let again = make_again_here(config.path(), &fake, &ours).unwrap();

    assert!(!again.sign_ins_kept);
    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
}

/// A runner Docker still lists, only stopped or suspended, is never made again: it resumes.
#[test]
fn a_runner_docker_still_lists_is_not_made_again() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = a_pool_added(config.path(), &fake, &ours);
    fake.set("containers", &format!("{ID}\n"));
    std::fs::write(fake.file_path("log"), "").unwrap();

    let refused = make_again_here(config.path(), &fake, &ours).unwrap_err();

    assert!(refused.to_string().contains("still there"), "{refused}");
    assert!(!fake.file("log").contains("run "), "{}", fake.file("log"));
    assert_eq!(runners::read(config.path()).unwrap(), vec![old]);
}

/// A runner made again that does not pair is taken back whole, and the pool stays as it was.
#[test]
fn a_runner_made_again_that_does_not_pair_is_taken_back_and_the_pool_stays() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = removed_outside_purlis(config.path(), &fake, &ours, Some("2026-01-01T00:00:00Z"));
    fake.set("paired", "not a pin\n");

    assert!(make_again_here(config.path(), &fake, &ours).is_err());

    let log = fake.file("log");
    assert!(
        log.contains(&format!("container rm --force --volumes {AGAIN}")),
        "{log}"
    );
    assert!(
        log.contains(&format!(
            "volume rm --force purlis-runner-{AGAIN_NAMED}-home"
        )),
        "{log}"
    );
    assert!(
        !log.contains("volume rm --force purlis-runner-first-home\n"),
        "{log}"
    );
    assert_eq!(runners::read(config.path()).unwrap(), vec![old]);
}

/// The name a runner made again is drawn: the pool's, and eight hex digits.
#[test]
fn a_runner_made_again_is_named_for_its_pool() {
    let named = crate::dockerpool::remade_name("first").unwrap();
    assert!(named.starts_with("first-"), "{named}");
    assert_eq!(named.len(), "first-".len() + 8, "{named}");
    let image = crate::dockerpool::Image::pinned(IMAGE_OF_FIRST).unwrap();
    let user = crate::dockerpool::User::new(10001, 10001).unwrap();
    assert!(Runner::new(&named, image, user, "/home/purlis").is_ok());
}

/// **A kept runner reads back as it was made** (#4286): every link checks its container against
/// these, so a pool of its own limits must not read as the defaults.
#[test]
fn a_runner_of_a_pool_with_its_own_limits_reads_back_with_them() {
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let mut inspect = inspected();
    inspect[0]["HostConfig"]["NanoCpus"] = 1_500_000_000_u64.into();
    fake.set("inspect", &inspect.to_string());
    let set = Set {
        millicpus: Some(1500),
        ..Set::default()
    };
    add(config.path(), &fake.docker(), &pool("first", set), &ours).unwrap();
    let kept = runners::runner_for(config.path(), "first").unwrap();
    assert_eq!(kept.made.runner.limits().millicpus(), 1500);
    assert!(
        runners::changes_waiting(config.path(), "first")
            .unwrap()
            .is_empty()
    );
}

/// **A changed limit waits, and nothing moves until the runner is replaced** (#4286): the running
/// runner stays checked as it was made, Replace leaves it draining with its chats, and new chats
/// start on the new one.
#[test]
fn replacing_a_runner_holds_the_new_one_to_the_change_and_drains_the_old() {
    use crate::poollimits::LimitId;
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    let old = a_pool_added(config.path(), &fake, &ours);
    let next = "first-0a1b2c3d";

    // Nothing waits: Replace makes nothing.
    assert!(
        replace_as(config.path(), &fake.docker(), "first", next, &ours)
            .unwrap()
            .is_none()
    );

    runners::set_pool_limit(config.path(), "first", LimitId::Cpus, Some(1500)).unwrap();
    assert_eq!(
        runners::changes_waiting(config.path(), "first").unwrap(),
        vec![LimitId::Cpus]
    );
    // The change alone leaves the running runner as it was made, so its links still pass.
    let running = runners::runner_for(config.path(), "first").unwrap();
    assert_eq!(running.made, old.made);
    assert_eq!(running.made.runner.limits().millicpus(), 2000);
    assert!(runners::draining(config.path()).unwrap().is_empty());

    let answers = successor_answers();
    makes(&fake, next, IMAGE_OF_FIRST, &answers);
    let mut inspect: serde_json::Value = serde_json::from_str(&fake.file("inspect")).unwrap();
    inspect[0]["HostConfig"]["NanoCpus"] = 1_500_000_000_u64.into();
    fake.set("inspect", &inspect.to_string());
    let kept = replace_as(config.path(), &fake.docker(), "first", next, &ours)
        .unwrap()
        .expect("a runner made beside the old one");

    assert_eq!(kept.name(), next);
    assert_eq!(kept.of.as_deref(), Some("first"));
    assert_eq!(
        runners::runner_for(config.path(), "first").unwrap().name(),
        next
    );
    let draining = runners::draining(config.path()).unwrap();
    assert_eq!(draining.len(), 1);
    assert_eq!(draining[0].made.runner.name(), old.made.runner.name());
    // The old one still reads as it was made, so a link to its running chats still passes.
    assert_eq!(draining[0].made.runner.limits().millicpus(), 2000);
    assert!(
        runners::changes_waiting(config.path(), "first")
            .unwrap()
            .is_empty()
    );
    assert!(
        fake.file("log").contains("--cpus 1.500"),
        "{}",
        fake.file("log")
    );
}

/// **A limit's checks hold across limits** (#4286): `/tmp` no larger than the memory it counts on.
#[test]
fn a_shape_change_is_refused_as_a_new_pool_would_be_and_a_reset_is_a_change_back() {
    use crate::poollimits::LimitId;
    let config = tempfile::tempdir().unwrap();
    let ours = desktop();
    let fake = a_docker(&runner_answers());
    a_pool_added(config.path(), &fake, &ours);
    let refused =
        runners::set_pool_limit(config.path(), "first", LimitId::Tmp, Some(8192)).unwrap_err();
    assert!(
        refused.to_string().contains("no larger than its memory"),
        "{refused}"
    );
    assert!(
        runners::changes_waiting(config.path(), "first")
            .unwrap()
            .is_empty()
    );
    // A reset is a change back: nothing waits.
    runners::set_pool_limit(config.path(), "first", LimitId::Memory, Some(8192)).unwrap();
    assert_eq!(
        runners::changes_waiting(config.path(), "first").unwrap(),
        vec![LimitId::Memory]
    );
    runners::set_pool_limit(config.path(), "first", LimitId::Memory, None).unwrap();
    assert!(
        runners::changes_waiting(config.path(), "first")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_project_pool_this_machine_does_not_keep_starts_nothing_and_never_runs_here() {
    let config = tempfile::tempdir().unwrap();
    let plane = tempfile::tempdir().unwrap();
    let plane = plane.path();
    for workspace in ["ide", "web", "here"] {
        let folder = plane.join("workspaces").join(workspace);
        std::fs::create_dir_all(&folder).unwrap();
    }
    crate::machine::update(config.path(), |store| {
        store.remember(plane, 1);
        store
            .set_project_chats_run_on(plane, Some("first"))
            .unwrap();
        store
            .set_chats_run_on(plane, "web", Some("second"))
            .unwrap();
        store.set_chats_run_here(plane, "here").unwrap();
    })
    .unwrap();
    let refused = runs_on(config.path(), plane, Some("ide")).unwrap_err();
    assert!(
        refused.starts_with("This project runs its chats on pool first"),
        "{refused}"
    );
    assert_eq!(runs_on(config.path(), plane, Some("here")), Ok(None));
    // A chat at the project's root moves no clone, so it stays here.
    assert_eq!(runs_on(config.path(), plane, None), Ok(None));
    // A put-back closes what nothing took back on the project's pool, as `ide` follows it.
    assert_eq!(
        workspaces_on_pools(config.path(), plane),
        [
            ("first".to_owned(), vec!["ide".to_owned()]),
            ("second".to_owned(), vec!["web".to_owned()]),
        ]
        .into()
    );
    // Fetch-back asks about `ide` there only once it has run a chat there.
    assert_eq!(
        workspaces_to_fetch(config.path(), plane),
        [("second".to_owned(), vec!["web".to_owned()])].into()
    );
    let chosen = crate::machine::StartChoice::Pool("gone".to_owned());
    let refused = chat_place(config.path(), plane, Some("here"), Some(&chosen)).unwrap_err();
    assert!(
        refused.starts_with("This chat was to start on pool gone"),
        "{refused}"
    );
}

#[test]
fn a_start_on_a_pool_at_the_projects_root_is_refused_and_never_runs_here() {
    let config = tempfile::tempdir().unwrap();
    let plane = std::path::Path::new("/planes/a");
    let chosen = crate::machine::StartChoice::Pool("team".to_owned());
    let refused = chat_place(config.path(), plane, None, Some(&chosen)).unwrap_err();
    assert!(refused.contains("Start it in a workspace"), "{refused}");
}

/// A pool `first` draining its runner `first`, which had stopped before its drain began, and
/// whose home Docker says holds `found` conversations. The log is emptied.
fn a_stopped_runner_draining(config: &std::path::Path, fake: &FakeDocker, found: u32) {
    a_pool_draining(config, fake);
    let mut stopped = inspected();
    stopped[0]["State"]["Running"] = false.into();
    stopped[0]["State"]["FinishedAt"] = "2026-11-01T00:00:00Z".into();
    fake.set("inspect", &stopped.to_string());
    fake.set(
        "conversations",
        &format!("conversations {found} {found} 0\n"),
    );
    fake.set("containers", &format!("{ID}\n"));
    std::fs::write(fake.file_path("log"), "").unwrap();
}

/// **A stopped runner whose home holds a chat's conversation is held** (#4633, ruled 2026-11-02):
/// read by a container of the pool's runner's image, its home mounted read-only, and the runner
/// itself never started.
#[test]
fn a_stopped_runner_whose_home_holds_a_conversation_is_held() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);

    assert!(held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap());

    assert!(
        runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
    let log = fake.file("log");
    let read = log
        .lines()
        .find(|line| line.starts_with("run --rm"))
        .expect(&log);
    assert!(read.contains("target=/from,readonly"), "{read}");
    assert!(read.contains(NEW), "{read}");
    assert!(!read.contains("target=/to"), "{read}");
    assert!(!log.contains("start "), "{log}");
}

/// **A stopped runner whose home holds no conversation is not held**, and goes as it did.
#[test]
fn a_stopped_runner_whose_home_holds_none_is_not_held() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 0);

    assert!(!held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap());

    assert!(
        !runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
}

/// **A home that could not be read says nothing of it**: refused, and the runner stays as it was,
/// neither held nor removed.
#[test]
fn a_home_that_could_not_be_read_leaves_its_runner_as_it_was() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 0);
    fake.set("conversations", "");

    assert!(held_for_its_conversations(config.path(), &fake.docker(), "first").is_err());

    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.draining && !kept.held, "{kept:?}");
}

/// **Removing a pool discards a held runner's conversations only where the person was asked about
/// that runner** (#4860, D-4860-1): its question names each held runner, and a removal that names
/// none, or not this one, removes nothing. Named, the held runner goes with the pool.
#[test]
fn a_pool_with_a_held_runner_is_removed_only_once_the_person_was_asked_about_it() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    assert!(held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap());
    assert_eq!(
        held_of(config.path(), &fake.docker(), "first").unwrap(),
        ["first"]
    );

    let refused = remove(config.path(), &fake.docker(), "first", None).unwrap_err();
    assert!(refused.to_string().contains("runner first"), "{refused}");
    let refused = remove_discarding(
        config.path(),
        &fake.docker(),
        "first",
        None,
        &["another".to_owned()],
    )
    .unwrap_err();
    assert!(refused.to_string().contains("runner first"), "{refused}");
    assert_eq!(runners::read(config.path()).unwrap().len(), 2);
    assert!(
        !fake.file("log").contains("volume rm"),
        "{}",
        fake.file("log")
    );

    remove_discarding(
        config.path(),
        &fake.docker(),
        "first",
        None,
        &["first".to_owned()],
    )
    .unwrap();

    assert!(runners::read(config.path()).unwrap().is_empty());
    assert!(runners::pools(config.path()).unwrap().is_empty());
}

/// **A runner that stopped during its drain is held as the window shows it**, before any look
/// noted it, so removing its pool asks about it too.
#[test]
fn a_runner_a_look_would_hold_is_named_among_the_held_before_it_is_noted() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    let mut stopped = inspected();
    stopped[0]["State"]["Running"] = false.into();
    stopped[0]["State"]["FinishedAt"] = "2026-11-01T00:00:00Z".into();
    fake.set("inspect", &stopped.to_string());
    runners::note_locked(
        config.path(),
        "first",
        |kept| kept.last_stop = Some(1),
        &runners::lock(config.path()),
    )
    .unwrap();

    assert_eq!(
        held_of(config.path(), &fake.docker(), "first").unwrap(),
        ["first"]
    );
}

/// **A draining runner whose container went with its home kept may hold a conversation**, so
/// removing its pool asks about it; one whose home went too holds none, and is not named (#4860).
#[test]
fn a_gone_runner_is_named_only_where_its_home_is_still_there() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_pool_draining(config.path(), &fake);
    fake.set("inspect", "no such container");
    fake.set("containers", "");
    fake.set("volumes", "purlis-runner-first-home\n");

    assert_eq!(
        held_of(config.path(), &fake.docker(), "first").unwrap(),
        ["first"]
    );

    fake.set("volumes", "");
    assert!(
        held_of(config.path(), &fake.docker(), "first")
            .unwrap()
            .is_empty()
    );
}

/// **A runner discarded while its home was read is not said to be held** (#4861): the home is
/// read outside the runners lock, so what changed meanwhile is looked at again under it, and a
/// runner that no longer stands as it did is left to the next look.
#[test]
fn a_runner_discarded_while_its_home_was_read_is_not_said_to_be_held() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    // Discard pressed during the read: the runner's row is gone once the read ends.
    let file = |root: &Path| crate::machine::dir(root).join(crate::machine::RUNNERS_FILE);
    let discarded = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(crate::machine::dir(discarded.path())).unwrap();
    std::fs::copy(file(config.path()), file(discarded.path())).unwrap();
    runners::forget(discarded.path(), "first").unwrap();
    fake.set(
        "during-run",
        &format!(
            "cat '{}' > '{}'\n",
            file(discarded.path()).display(),
            file(config.path()).display()
        ),
    );

    // Docker lists the home the read's mount made again of its name.
    fake.set("volumes", "purlis-runner-first-home\n");

    assert!(!held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap());

    assert!(runners::named(config.path(), "first").unwrap().is_none());
    // A home the read's mount made again of its name goes with it.
    let log = fake.file("log");
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
}

/// **A runner that changed while its home was read is read again**, not held on a read of how
/// it stood before.
#[test]
fn a_runner_that_changed_while_its_home_was_read_is_read_again() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    let file = |root: &Path| crate::machine::dir(root).join(crate::machine::RUNNERS_FILE);
    let changed = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(crate::machine::dir(changed.path())).unwrap();
    std::fs::copy(file(config.path()), file(changed.path())).unwrap();
    runners::note_locked(
        changed.path(),
        "first",
        |kept| kept.last_stop = Some(7),
        &runners::lock(changed.path()),
    )
    .unwrap();
    fake.set(
        "during-run",
        &format!(
            "cat '{}' > '{}'\n",
            file(changed.path()).display(),
            file(config.path()).display()
        ),
    );

    let refused = held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap_err();

    assert!(refused.to_string().contains("changed while"), "{refused}");
    assert!(
        !runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
}

/// **A read that finishes clears its pace**: the next failure waits ten minutes again, not twice
/// the last.
#[test]
fn a_read_that_finishes_clears_its_pace() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 0);
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    let whose = (
        config.path().to_path_buf(),
        kept.made.container.as_str().to_owned(),
    );
    let at = std::time::Instant::now();
    conversation_reads().failed(&whose, "it failed", at - read_again_after(1) * 2);
    assert!(conversation_reads().waiting(&whose, at).is_none());

    assert!(!held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap());

    conversation_reads().failed(&whose, "it failed", at);
    let waiting = conversation_reads().waiting(&whose, at).unwrap();
    assert!(waiting.contains("in about 10 minutes"), "{waiting}");
}

/// **A runner stopped before its drain that no look has read may hold a conversation**, so
/// removing its pool asks about it (#4860).
#[test]
fn a_runner_no_look_has_read_yet_is_named_among_those_that_may_hold_conversations() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    // Its stop was noted as its drain began: it stopped before it.
    let before = runners::named(config.path(), "first").unwrap().unwrap();
    let ran = fake.docker().ran(&before.made).unwrap();
    runners::note_locked(
        config.path(),
        "first",
        |kept| kept.last_stop = Some(ran.finished.unwrap_or(0)),
        &runners::lock(config.path()),
    )
    .unwrap();
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert_eq!(
        drain_look(&kept, fake.docker().ran(&kept.made).unwrap()),
        DrainLook::StoppedBefore
    );

    assert_eq!(
        held_of(config.path(), &fake.docker(), "first").unwrap(),
        ["first"]
    );
    assert!(remove(config.path(), &fake.docker(), "first", None).is_err());
}

/// **A home whose read failed is not read again at once** (#4861): each failure waits longer
/// before the next, so a Docker that cannot read it is not asked at every look.
#[test]
fn a_home_whose_read_failed_is_not_read_again_at_the_next_look() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    fake.set("conversations", "");
    held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap_err();
    fake.set("conversations", "conversations 2 2 0\n");
    std::fs::write(fake.file_path("log"), "").unwrap();

    let refused = held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap_err();

    assert!(refused.to_string().contains("read again"), "{refused}");
    let log = fake.file("log");
    assert!(!log.contains("run --rm"), "{log}");
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    assert!(kept.draining && !kept.held, "{kept:?}");
}

/// **A read that keeps failing waits twice as long each time**, from ten minutes to four hours.
#[test]
fn a_read_that_keeps_failing_waits_twice_as_long_each_time_up_to_four_hours() {
    use std::time::Duration;
    let minutes = |n: u64| Duration::from_secs(n * 60);
    assert_eq!(read_again_after(1), minutes(10));
    assert_eq!(read_again_after(2), minutes(20));
    assert_eq!(read_again_after(5), minutes(160));
    assert_eq!(read_again_after(6), minutes(240));
    assert_eq!(read_again_after(u32::MAX), minutes(240));
}

/// **A home made of its name since its container went holds nothing of it** (#4167), so it is
/// not read for its conversations.
#[test]
fn a_home_made_since_its_container_went_is_not_read_for_conversations() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    fake.set("engine", "engine-a\n");
    home_prints(&fake, HOME_CREATED);
    a_stopped_runner_draining(config.path(), &fake, 2);
    fake.set("inspect", "no such container");
    fake.set("containers", "");
    fake.set("volumes", "purlis-runner-first-home\n");
    home_prints(&fake, "2026-01-01T00:00:00Z");

    assert!(!held_for_its_conversations(config.path(), &fake.docker(), "first").unwrap());

    assert!(
        !fake.file("log").contains("target=/from"),
        "{}",
        fake.file("log")
    );
}

/// **Bringing back is for a held runner alone**, and only once none of its conversations is left
/// behind is it removed, with its home; one left keeps it held.
#[test]
fn a_held_runner_goes_once_its_conversations_are_all_brought_back() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    let stopped = fake.file("inspect");
    // Running as it was when its drain began: only it can say what it runs, so it is not held.
    fake.set("inspect", &inspected().to_string());

    let refused = bring_back(config.path(), &fake.docker(), "first", None).unwrap_err();
    assert!(refused.to_string().contains("held"), "{refused}");

    fake.set("inspect", &stopped);
    hold(config.path(), "first").unwrap();
    fake.set("conversations", "conversations 2 1 0\n");
    let brought = bring_back(config.path(), &fake.docker(), "first", None).unwrap();
    assert_eq!((brought.found, brought.left), (2, 1));
    assert!(brought.removed.is_none());
    assert!(
        runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
    let into = fake.file("log");
    let into = into
        .lines()
        .find(|line| line.contains("target=/to"))
        .expect(&into);
    assert!(into.contains("first-fedcba98-home,target=/to"), "{into}");

    fake.set("conversations", "conversations 2 0 0\n");
    let brought = bring_back(config.path(), &fake.docker(), "first", None).unwrap();
    assert!(brought.removed.is_some_and(|removed| removed.forgotten));
    assert!(runners::named(config.path(), "first").unwrap().is_none());
}

/// **Nothing is brought back from or to a runner a chat ran on without the sandbox**, as no
/// sign-in is carried: the press is refused, and the runner stays held.
#[test]
fn nothing_is_brought_back_across_a_runner_that_ran_without_the_sandbox() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 2);
    hold(config.path(), "first").unwrap();
    let kept = runners::named(config.path(), "first").unwrap().unwrap();
    crate::runnertaint::mark(config.path(), &kept.made.container).unwrap();

    let refused = bring_back(config.path(), &fake.docker(), "first", None).unwrap_err();

    assert!(
        refused.to_string().contains("without the sandbox"),
        "{refused}"
    );
    assert!(
        !fake.file("log").contains("target=/to"),
        "{}",
        fake.file("log")
    );
    assert!(
        runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
}

/// **Bring back never removes a home that holds no conversation**, as a runner a stop during its
/// drain held may: what is in it goes only by Discard, asked twice.
#[test]
fn bringing_back_from_a_home_with_no_conversation_removes_nothing() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 0);
    hold(config.path(), "first").unwrap();

    let refused = bring_back(config.path(), &fake.docker(), "first", None).unwrap_err();

    assert!(refused.to_string().contains("no conversation"), "{refused}");
    assert!(
        runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
    assert!(
        !fake.file("log").contains("volume rm"),
        "{}",
        fake.file("log")
    );
}

/// **A held runner a link started again is stopped before its conversations are copied**, so
/// none changes under the copy and none is cut short by the removal after it.
#[test]
fn a_held_runner_that_runs_again_is_stopped_before_its_conversations_come_back() {
    let config = tempfile::tempdir().unwrap();
    let fake = a_docker(&runner_answers());
    a_stopped_runner_draining(config.path(), &fake, 1);
    hold(config.path(), "first").unwrap();
    fake.set("inspect", &inspected().to_string());
    fake.set("conversations", "conversations 1 0 0\n");

    bring_back(config.path(), &fake.docker(), "first", None).unwrap();

    let log = fake.file("log");
    let stopped = log.find(&format!("container stop {ID}")).expect(&log);
    let copied = log.find("target=/to").expect(&log);
    assert!(stopped < copied, "{log}");
}

#[test]
fn fetch_back_asks_a_workspace_following_the_projects_pool_only_once_it_ran_there() {
    let config = tempfile::tempdir().unwrap();
    let plane = tempfile::tempdir().unwrap();
    let plane = plane.path();
    for workspace in ["ide", "web", "docs", "named"] {
        std::fs::create_dir_all(plane.join("workspaces").join(workspace)).unwrap();
    }
    let on = |workspace: &str| crate::machine::ChatOnPool {
        pool: "first".to_owned(),
        workspace: workspace.to_owned(),
        runner: None,
    };
    crate::machine::update(config.path(), |store| {
        store.remember(plane, 1);
        store
            .set_project_chats_run_on(plane, Some("first"))
            .unwrap();
        store
            .set_chats_run_on(plane, "named", Some("second"))
            .unwrap();
        // A chat of `ide` runs on the pool now, and one of `web` ran there and ended.
        store.keep_chat_on_pool(plane, 1, on("ide"));
        store.keep_chat_on_pool(plane, 2, on("web"));
        store.leave_chat_on_pool(plane, 2);
    })
    .unwrap();
    // `docs` follows the project too, and never ran a chat there.
    assert_eq!(
        workspaces_to_fetch(config.path(), plane),
        [
            ("first".to_owned(), vec!["ide".to_owned(), "web".to_owned()]),
            ("second".to_owned(), vec!["named".to_owned()]),
        ]
        .into()
    );
    // Once `web`'s branches came back, nothing more of it is owed from there.
    crate::machine::update(config.path(), |store| {
        store.branches_fetched(plane, "first", "web");
    })
    .unwrap();
    assert_eq!(
        workspaces_to_fetch(config.path(), plane)["first"],
        vec!["ide".to_owned()]
    );
}

#[test]
fn a_first_chat_on_a_pool_is_one_nothing_of_the_workspace_has_run_or_come_back_from() {
    let config = tempfile::tempdir().unwrap();
    let plane = tempfile::tempdir().unwrap();
    let plane = plane.path();
    let a_clone = |workspace: &str, clone: &str| {
        let at = plane.join("workspaces").join(workspace).join(clone);
        std::fs::create_dir_all(&at).unwrap();
        let git = move |args: &[&str]| {
            let done = crate::testgit::run(&at, args);
            assert!(done.ok(), "git {args:?} failed: {done:?}");
        };
        git(&["init", "-q", "-b", "main", "."]);
        git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ]);
        git
    };
    let _ide = a_clone("ide", "web");
    let app = a_clone("app", "api");
    std::fs::create_dir_all(plane.join("workspaces").join("notes")).unwrap();
    crate::machine::update(config.path(), |store| {
        store.remember(plane, 1);
        store.keep_chat_on_pool(
            plane,
            1,
            crate::machine::ChatOnPool {
                pool: "first".to_owned(),
                workspace: "ide".to_owned(),
                runner: None,
            },
        );
    })
    .unwrap();
    // What came back from a pool's runner stays in the clone once nothing is owed.
    app(&["update-ref", "refs/remotes/second/main", "HEAD"]);
    let pools = ["first".to_owned(), "second".to_owned()];
    let run_on = |workspace| pools_run_on(config.path(), plane, workspace, &pools);
    assert_eq!(run_on("ide"), ["first"], "a chat of it is kept there");
    assert_eq!(
        run_on("app"),
        ["second"],
        "its branches came back from there"
    );
    // A workspace with no clone has nothing to send, so no chat of it is a first anywhere.
    assert_eq!(run_on("notes"), ["first", "second"]);
}

/// **What a draining runner holds that no fetch brings** (ruled 2026-11-07): each project it keeps
/// is found here by its id among the projects this machine knows, and the runner's every
/// repository is proved home against that folder, as a fresh runner's is.
mod left_beyond_a_fetch {
    use std::path::{Path, PathBuf};

    use crate::codelink::held::repos_held;

    const PROJECT: &str = "01J9PRJCT00000000000000000";

    fn git(dir: &Path, args: &[&str]) -> String {
        let done = crate::testgit::run(dir, args);
        assert!(done.ok(), "git {args:?} failed: {done:?}");
        done.out.trim().to_owned()
    }

    fn commit(dir: &Path, file: &str, text: &str) {
        std::fs::write(dir.join(file), text).unwrap();
        git(dir, &["add", file]);
        git(
            dir,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                text,
            ],
        );
    }

    fn init(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main", "."]);
    }

    /// A draining runner keeping project [`PROJECT`] with a clone `workspaces/app/web`, and this
    /// machine's folder of that project, which it knows, holding all of it.
    struct World {
        _dir: tempfile::TempDir,
        config: PathBuf,
        projects: PathBuf,
        runner: PathBuf,
        here: PathBuf,
    }

    impl World {
        fn new() -> World {
            let dir = tempfile::tempdir().unwrap();
            let top = dir.path().canonicalize().unwrap();
            let projects = top.join("runner/projects");
            let runner = projects.join(PROJECT);
            init(&runner);
            std::fs::write(runner.join(".gitignore"), "workspaces/\n").unwrap();
            commit(&runner, ".gitignore", "workspaces/\n");
            commit(
                &runner,
                "charter.toml",
                &format!("[project]\nid = \"{PROJECT}\"\n"),
            );
            let web = runner.join("workspaces/app/web");
            init(&web);
            commit(&web, "index.html", "web\n");
            let here = top.join("alpha");
            git(&top, &["clone", "-q", runner.to_str().unwrap(), "alpha"]);
            let app = here.join("workspaces/app");
            std::fs::create_dir_all(&app).unwrap();
            git(&app, &["clone", "-q", web.to_str().unwrap(), "web"]);
            let config = top.join("config");
            std::fs::create_dir_all(&config).unwrap();
            crate::machine::update(&config, |store| store.remember(&here, 1)).unwrap();
            World {
                _dir: dir,
                config,
                projects,
                runner,
                here,
            }
        }

        fn web(&self) -> PathBuf {
            self.runner.join("workspaces/app/web")
        }

        fn left(&self) -> Vec<String> {
            let (held, unread) = repos_held(&self.projects);
            super::left_beyond_a_fetch(&self.config, &held, &unread).unwrap()
        }
    }

    fn says(left: &[String], words: &str) -> bool {
        left.iter().any(|one| one.contains(words))
    }

    #[test]
    fn a_draining_runner_whose_every_commit_is_in_its_project_here_leaves_nothing() {
        assert_eq!(World::new().left(), Vec::<String>::new());
    }

    #[test]
    fn a_stash_on_a_draining_runner_is_left_and_names_its_project() {
        let world = World::new();
        std::fs::write(world.web().join("index.html"), "stashed\n").unwrap();
        git(
            &world.web(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "stash",
                "-q",
            ],
        );

        let left = world.left();

        assert!(says(&left, "refs/stash"), "{left:?}");
        assert!(says(&left, "alpha"), "{left:?}");
    }

    #[test]
    fn a_commit_on_no_branch_on_a_draining_runner_is_left() {
        let world = World::new();
        git(&world.web(), &["checkout", "-q", "--detach"]);
        commit(&world.web(), "index.html", "detached\n");

        let left = world.left();

        assert!(says(&left, "HEAD"), "{left:?}");
    }

    #[test]
    fn a_clone_made_on_a_draining_runner_and_not_here_is_left() {
        let world = World::new();
        let api = world.runner.join("workspaces/app/api");
        init(&api);
        commit(&api, "main.rs", "fn main() {}\n");

        let left = world.left();

        assert!(says(&left, "workspaces/app/api"), "{left:?}");
    }

    #[test]
    fn a_clone_removed_here_is_left_on_a_draining_runner() {
        let world = World::new();
        std::fs::remove_dir_all(world.here.join("workspaces/app/web")).unwrap();

        let left = world.left();

        assert!(says(&left, "workspaces/app/web"), "{left:?}");
    }

    #[test]
    fn the_work_of_a_workspace_renamed_here_is_left_on_a_draining_runner() {
        let world = World::new();
        std::fs::rename(
            world.here.join("workspaces/app"),
            world.here.join("workspaces/apps"),
        )
        .unwrap();
        commit(&world.web(), "index.html", "after the rename\n");

        let left = world.left();

        assert!(says(&left, "workspaces/app/web"), "{left:?}");
    }

    #[test]
    fn the_work_of_a_project_this_machine_no_longer_knows_is_left() {
        let world = World::new();
        crate::machine::update(&world.config, |store| {
            store.forget(&world.here);
        })
        .unwrap();

        let left = world.left();

        assert!(says(&left, PROJECT), "{left:?}");
    }

    #[test]
    fn a_project_with_two_folders_here_needs_its_work_in_only_one() {
        let world = World::new();
        let other = world.here.with_file_name("beta");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(
            other.join("charter.toml"),
            format!("[project]\nid = \"{PROJECT}\"\n"),
        )
        .unwrap();
        crate::machine::update(&world.config, |store| store.remember(&other, 2)).unwrap();

        assert_eq!(world.left(), Vec::<String>::new());
    }

    #[test]
    fn a_project_whose_folder_is_no_repository_is_proved_by_its_clones_alone() {
        let world = World::new();
        std::fs::remove_dir_all(world.runner.join(".git")).unwrap();
        std::fs::remove_dir_all(world.here.join(".git")).unwrap();

        assert_eq!(world.left(), Vec::<String>::new());

        commit(&world.web(), "index.html", "only on the runner\n");
        let left = world.left();
        assert!(says(&left, "refs/heads/main"), "{left:?}");
    }

    /// What the branch `main` of the runner's clone `web` is at, recorded as fetched from it.
    fn fetched_once(world: &World) -> String {
        let tip = git(&world.web(), &["rev-parse", "HEAD"]);
        crate::tipsfetched::record(&world.config, PROJECT, std::slice::from_ref(&tip)).unwrap();
        tip
    }

    #[test]
    fn a_branch_fetched_once_and_squashed_here_since_is_not_left() {
        let world = World::new();
        commit(&world.web(), "index.html", "squashed here since\n");
        assert!(says(&world.left(), "refs/heads/main"), "not yet fetched");

        fetched_once(&world);

        assert_eq!(world.left(), Vec::<String>::new());
    }

    #[test]
    fn a_branch_moved_on_the_runner_after_its_fetch_is_left() {
        let world = World::new();
        commit(&world.web(), "index.html", "fetched\n");
        fetched_once(&world);
        commit(&world.web(), "index.html", "after the fetch\n");

        let left = world.left();

        assert!(says(&left, "refs/heads/main"), "{left:?}");
    }

    #[test]
    fn the_work_of_a_workspace_renamed_here_and_fetched_once_is_not_left() {
        let world = World::new();
        std::fs::rename(
            world.here.join("workspaces/app"),
            world.here.join("workspaces/apps"),
        )
        .unwrap();
        commit(&world.web(), "index.html", "after the rename\n");

        fetched_once(&world);

        assert_eq!(world.left(), Vec::<String>::new());
    }

    #[test]
    fn a_stash_is_left_though_its_branch_was_fetched_once() {
        let world = World::new();
        commit(&world.web(), "index.html", "fetched\n");
        fetched_once(&world);
        std::fs::write(world.web().join("index.html"), "stashed\n").unwrap();
        git(
            &world.web(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "stash",
                "-q",
            ],
        );

        let left = world.left();

        assert!(says(&left, "refs/stash"), "{left:?}");
    }

    #[test]
    fn a_tip_fetched_into_another_project_does_not_count() {
        let world = World::new();
        commit(&world.web(), "index.html", "only on the runner\n");
        let tip = git(&world.web(), &["rev-parse", "HEAD"]);
        crate::tipsfetched::record(&world.config, "01J9PRJCT00000000000000001", &[tip]).unwrap();

        let left = world.left();

        assert!(says(&left, "refs/heads/main"), "{left:?}");
    }

    #[test]
    fn what_the_runner_could_not_look_through_is_left() {
        let world = World::new();
        let (held, _) = repos_held(&world.projects);

        let left =
            super::left_beyond_a_fetch(&world.config, &held, &["a folder ran out".to_owned()])
                .unwrap();

        assert!(says(&left, "a folder ran out"), "{left:?}");
    }
}

/// **Where the machine store can't be read, a start in a workspace is refused** (ruled
/// 2026-11-08): where its chats run is not known, so none runs on this machine in a pool's place.
/// The refusal names the file and what to do.
#[test]
fn a_start_in_a_workspace_is_refused_while_the_machine_store_cannot_be_read() {
    let config = tempfile::tempdir().unwrap();
    let plane = std::path::Path::new("/planes/a");
    let store = crate::machine::file(config.path());
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    let elsewhere = config.path().join("elsewhere.json");
    std::fs::write(&elsewhere, "{}").unwrap();
    std::os::unix::fs::symlink(&elsewhere, &store).unwrap();

    let refused = chat_place(config.path(), plane, Some("app"), None).unwrap_err();

    assert!(refused.contains(&store.display().to_string()), "{refused}");
    assert!(refused.contains("Where this chat runs"), "{refused}");
    let here = crate::machine::StartChoice::ThisMachine;
    assert!(chat_place(config.path(), plane, Some("app"), Some(&here)).is_ok());
    assert!(chat_place(config.path(), plane, None, None).is_ok());
}

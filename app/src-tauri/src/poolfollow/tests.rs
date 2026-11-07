// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use purlis_core::dockerpool::{ContainerId, Docker, Image, Made, Runner, User};
use purlis_core::runners::{self, Pool, Set};
use purlis_core::secrets::keyring::FileStore;
use purlis_session_protocol::hostlink::{LinkKey, Pinned};

use super::*;

const OLD: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NEW: &str = "ghcr.io/purlis/purlis-runner@sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// A machine in `config` whose `docker` removes whatever it is asked to, and logs it. Every
/// container it is asked of is still there: none was removed outside purlis (#4149).
fn a_machine(config: &std::path::Path) -> Machine {
    let docker = config.join("docker");
    std::fs::write(
        &docker,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$1 $2\" in 'container ls') \
             for a in \"$@\"; do case $a in id=*) echo \"${{a#id=}}\" ;; esac; done ;; esac\n\
             exit 0\n",
            config.join("log").display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755)).unwrap();
    Machine {
        config: config.to_path_buf(),
        docker: Docker::at(docker).with_settings_in(config.join("docker-settings")),
        keyring: Arc::new(FileStore::at(config.join("keyring.json"))),
    }
}

fn made(name: &str, image: &str, id: char) -> Made {
    Made {
        container: ContainerId::new(&id.to_string().repeat(64)).unwrap(),
        runner: Runner::new(
            name,
            Image::pinned(image).unwrap(),
            User::new(10001, 10001).unwrap(),
            "/home/purlis",
        )
        .unwrap(),
    }
}

fn answers(device: &str) -> Pinned {
    Pinned {
        device: device.into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

/// Pool `first`, its runner `first` draining beside `first-fedcba98`, of the new image.
fn a_pool_draining(machine: &Machine) {
    let ours = machine.desktop_made().unwrap().pinned();
    let config = &machine.config;
    runners::record_pool(
        config,
        &Pool::new("first", Image::pinned(OLD).unwrap(), Set::default()).unwrap(),
    )
    .unwrap();
    let locked = runners::lock(config);
    runners::keep_for_its_pool(
        config,
        &made("first", OLD, 'a'),
        &answers("01JB0RUNNER0000000000000000"),
        &ours,
        &locked,
    )
    .unwrap();
    runners::succeed_locked(
        config,
        "first",
        &made("first-fedcba98", NEW, 'b'),
        &answers("01JB0RUNNER0000000000000001"),
        &ours,
        &locked,
    )
    .unwrap();
}

/// Whether docker was asked nothing but to look at a container, if anything.
fn only_looked(config: &std::path::Path) -> bool {
    std::fs::read_to_string(config.join("log"))
        .unwrap_or_default()
        .lines()
        .all(|line| line.starts_with("container inspect ") || line.starts_with("container ls "))
}

/// A runner that holds nothing a fetch does not bring.
fn nothing_held(_: &ToARunner) -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

fn names(machine: &Machine) -> Vec<String> {
    runners::read(&machine.config)
        .unwrap()
        .iter()
        .map(|kept| kept.name().to_owned())
        .collect()
}

/// **A draining runner goes only with its work fetched** (#5393): no chat is left on it, and until
/// the project's fetch has run it stays, however often it is looked at.
#[test]
fn a_draining_runner_with_no_chat_left_stays_until_its_work_is_fetched() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let plane = std::path::Path::new("/planes/a");
    purlis_core::machine::update(&machine.config, |store| {
        store.remember(plane, 1);
        store.keep_chat_on_pool(
            plane,
            7,
            purlis_core::machine::ChatOnPool {
                pool: "first".to_owned(),
                workspace: "ide".to_owned(),
                runner: Some("first".to_owned()),
            },
        );
        store.leave_chat_on_pool(plane, 7);
    })
    .unwrap();

    let said = follow_and_drain(&machine, None, &|_| Ok(0), &nothing_held);

    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    assert!(
        said.iter()
            .any(|line| line.contains("stays until its work is fetched")),
        "{said:?}"
    );
    assert!(
        only_looked(config.path()),
        "nothing of it is carried or removed"
    );

    purlis_core::machine::update(&machine.config, |store| {
        store.branches_fetched(plane, "first", "ide");
    })
    .unwrap();
    let said = follow_and_drain(&machine, None, &|_| Ok(0), &nothing_held);

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
}

/// **A draining runner goes only once all it holds is here** (ruled 2026-11-07): its fetch ran,
/// and still it holds work of a project no folder here holds, so it stays, and the Pools group
/// says what. Once it holds nothing a fetch does not bring, it goes, and nothing is said.
#[test]
fn a_draining_runner_holding_work_no_fetch_brings_stays_and_the_pools_group_says_what() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let project = "01J9PRJCT00000000000000000";
    let holds_work = |_: &ToARunner| -> Result<Vec<String>, String> {
        Ok(vec![format!(
            "work of a project no folder on this machine holds, which the runner keeps as {project}"
        )])
    };

    let said = follow_and_drain(&machine, None, &|_| Ok(0), &holds_work);

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    assert!(
        said.iter().any(|line| line.contains("no fetch brings")),
        "{said:?}"
    );
    assert!(
        only_looked(config.path()),
        "nothing of it is carried or removed"
    );
    let left = left_on(&machine.config, "first");
    assert!(left.iter().any(|one| one.contains(project)), "{left:?}");

    let said = follow_and_drain(&machine, None, &|_| Ok(0), &nothing_held);

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
    assert_eq!(left_on(&machine.config, "first"), Vec::<String>::new());
}

/// What a look found on a runner that then holds nothing more is said no more.
#[test]
fn a_draining_runner_that_lists_again_and_holds_nothing_goes_and_is_said_no_more() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    follow_and_drain(&machine, None, &|_| Ok(0), &|_| Err("down".into()));
    assert_eq!(left_on(&machine.config, "first").len(), 1);

    let said = follow_and_drain(&machine, None, &|_| Ok(0), &nothing_held);

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
    assert_eq!(left_on(&machine.config, "first"), Vec::<String>::new());
}

/// A draining runner that does not list what it holds may hold anything, so it stays, and the
/// Pools group says why.
#[test]
fn a_draining_runner_that_does_not_list_what_it_holds_stays() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);

    let said = follow_and_drain(&machine, None, &|_| Ok(0), &|_| Err("down".into()));

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    let left = left_on(&machine.config, "first");
    assert!(
        left.iter()
            .any(|one| one.contains("did not list what it holds: down")),
        "{left:?}"
    );
    forget_waiting(&machine.config, "first");
    assert_eq!(
        left_on(&machine.config, "first"),
        Vec::<String>::new(),
        "a removed pool says nothing"
    );
}

/// **The Pools group names the project a draining runner waits on** (#5461): the one the drain
/// keeps it for, once, with when it was last opened and whether it is open now; none once its
/// fetch has run.
#[test]
fn the_pools_group_names_the_project_a_draining_runner_waits_on_until_its_fetch_runs() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let (plane, open) = (
        std::path::Path::new("/planes/a"),
        std::path::Path::new("/planes/b"),
    );
    purlis_core::machine::update(&machine.config, |store| {
        store.remember(plane, 1_700_000_000);
        store.remember(open, 1_700_000_100);
        for (at, number) in [(plane, 7), (plane, 8), (open, 9)] {
            store.keep_chat_on_pool(
                at,
                number,
                purlis_core::machine::ChatOnPool {
                    pool: "first".to_owned(),
                    workspace: "ide".to_owned(),
                    runner: Some("first".to_owned()),
                },
            );
            store.leave_chat_on_pool(at, number);
        }
    })
    .unwrap();
    let older = runners::draining(&machine.config).unwrap();
    let owed =
        |open: &[std::path::PathBuf]| crate::dockerpools::owed_by(&machine.config, &older, open);

    let (said, why) = owed(&[open.to_path_buf()]);

    assert_eq!(why, None);
    let shown: Vec<_> = said
        .iter()
        .map(|one| {
            (
                one.name.as_str(),
                one.path.as_str(),
                one.opened,
                one.open_now,
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("b", "/planes/b", 1_700_000_100, true),
            ("a", "/planes/a", 1_700_000_000, false)
        ],
        "each project once, the open one marked"
    );

    purlis_core::machine::update(&machine.config, |store| {
        store.branches_fetched(plane, "first", "ide");
        store.branches_fetched(open, "first", "ide");
    })
    .unwrap();
    assert_eq!(owed(&[]), (vec![], None), "their fetches ran");
}

/// A chat kept on the pool's next runner has nothing on the draining one, so it does not hold it.
#[test]
fn a_chat_on_the_pools_next_runner_does_not_hold_a_draining_one_back() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let plane = std::path::Path::new("/planes/a");
    purlis_core::machine::update(&machine.config, |store| {
        store.remember(plane, 1);
        store.keep_chat_on_pool(
            plane,
            7,
            purlis_core::machine::ChatOnPool {
                pool: "first".to_owned(),
                workspace: "ide".to_owned(),
                runner: Some("first-fedcba98".to_owned()),
            },
        );
    })
    .unwrap();

    let said = follow_and_drain(&machine, None, &|_| Ok(0), &nothing_held);

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
}

#[test]
fn a_draining_runner_is_removed_once_no_chat_is_left_on_it_and_not_before() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);

    // Chats still run on it: it stays, and the pool says how many it waits on.
    follow_and_drain(&machine, None, &|_| Ok(2), &nothing_held);
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    assert_eq!(waiting_on(&machine.config, "first"), 2);
    // Nor is a sign-in of it carried to the pool's runner while they run (#4055): a new chat
    // there asks to sign in, and one runner holds each sign-in.
    let log = std::fs::read_to_string(config.path().join("log")).unwrap_or_default();
    assert!(!log.contains("-copy "), "{log}");
    // It does not answer: not known to be idle, so it stays.
    let said = follow_and_drain(&machine, None, &|_| Err("down".into()), &nothing_held);
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    assert!(
        said.iter()
            .any(|line| line.contains("stays until it answers")),
        "{said:?}"
    );

    // No chat is left: it is removed, its container and home with it, and the pool's own stays.
    let said = follow_and_drain(&machine, None, &|_| Ok(0), &nothing_held);
    assert_eq!(names(&machine), ["first-fedcba98"]);
    assert!(
        said.iter().any(|line| line.contains("is removed")),
        "{said:?}"
    );
    assert_eq!(
        waiting_on(&machine.config, "first"),
        0,
        "a removed runner waits on nothing"
    );
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(
        log.contains(&format!(
            "container rm --force --volumes {}",
            "a".repeat(64)
        )),
        "{log}"
    );
    // Its sign-ins went to the pool's runner only now, with no chat left on it (#4055).
    let carried = log
        .find("--name purlis-runner-first-fedcba98-copy ")
        .expect(&log);
    assert!(
        carried < log.find("container rm --force --volumes").unwrap(),
        "{log}"
    );
}

#[test]
fn a_pool_already_of_this_versions_image_or_of_a_teams_own_is_left_as_it_is() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let team = "registry.example/team/runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    runners::record_pool(
        &machine.config,
        &Pool::new("team", Image::pinned(team).unwrap(), Set::default()).unwrap(),
    )
    .unwrap();

    let said = follow_and_drain(
        &machine,
        Some(&Image::pinned(NEW).unwrap()),
        &|_| Ok(2),
        &nothing_held,
    );

    assert!(said.is_empty(), "{said:?}");
    // Asking whether a draining runner's container runs (#2051) is all docker was asked.
    assert!(
        only_looked(config.path()),
        "docker was asked to make something"
    );
}

/// **A release that names no runner image leaves every pool as it is** (#2880): the look notes
/// that none was named, and nothing is made or moved.
#[test]
fn a_release_with_no_runner_image_leaves_the_pools_where_they_are() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let before = runners::read(&machine.config).unwrap();
    let pools = runners::pools(&machine.config).unwrap();

    note_named(&Ok(Found::NoneNamed));
    assert_eq!(this_versions_image_named(), Some(false));
    follow_and_drain(&machine, None, &|_| Ok(1), &nothing_held);

    assert_eq!(runners::read(&machine.config).unwrap(), before);
    assert_eq!(runners::pools(&machine.config).unwrap(), pools);
    // Asking whether a draining runner's container runs (#2051) is all docker was asked.
    assert!(
        only_looked(config.path()),
        "docker was asked to do something"
    );

    // A look that could not be made, or one of another version, notes nothing; one that found
    // this version's image notes it.
    note_named(&Err("offline".into()));
    assert_eq!(this_versions_image_named(), Some(false));
    note_named(&Ok(Found::OtherVersion));
    assert_eq!(this_versions_image_named(), Some(false));
    note_named(&Ok(Found::Named(Image::pinned(NEW).unwrap())));
    assert_eq!(this_versions_image_named(), Some(true));
    note_named(&Ok(Found::OtherVersion));
    assert_eq!(this_versions_image_named(), Some(true));
    // The image found is kept, for a new pool's Image box: it is not read again.
    assert_eq!(
        this_versions_image_noted(),
        Some(Image::pinned(NEW).unwrap())
    );
}

/// **The Add a pool dialog is told which release's image it is offered** (the person's ruling
/// of 2026-10-30): this version's, a newer version's with the update to it waiting, or none;
/// and why where it was refused.
#[test]
fn the_dialog_is_told_which_releases_image_a_new_pool_is_offered() {
    use purlis_core::runnerimage::Offered;
    let image = Image::pinned(NEW).unwrap();
    assert_eq!(
        said(Ok(Offered::Ours(image.clone()))),
        Ok(RunnerImageOffered::Ours {
            image: NEW.to_owned()
        })
    );
    assert_eq!(
        said(Ok(Offered::Newer {
            image,
            version: "0.4.3".to_owned()
        })),
        Ok(RunnerImageOffered::Newer {
            image: NEW.to_owned(),
            version: "0.4.3".to_owned()
        })
    );
    assert_eq!(said(Ok(Offered::None)), Ok(RunnerImageOffered::NoneNamed));
    assert_eq!(said(Err("not signed".into())), Err("not signed".to_owned()));
}

/// **Only a manifest of this very version says its release has no runner image** (#2880): one
/// of another version, a desktop behind or ahead of its channel, says nothing of it.
#[test]
fn only_a_manifest_of_this_version_says_its_release_has_no_runner_image() {
    let ours = "0.4.2-dev.17";
    assert_eq!(found_in(None, "", ours), Ok(Found::OtherVersion));
    for other in ["0.4.2-dev.16", "0.4.2-dev.18", "0.4.3"] {
        assert_eq!(
            found_in(Some(&serde_json::json!({ "version": other })), "", ours),
            Ok(Found::OtherVersion),
            "{other}"
        );
    }
    assert_eq!(
        found_in(Some(&serde_json::json!({ "version": ours })), "", ours),
        Ok(Found::NoneNamed)
    );
}

/// A removed pool waits on nothing: what its draining runners waited on goes with it (#2865).
#[test]
fn a_removed_pools_waiting_chats_are_let_go_of() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    follow_and_drain(&machine, None, &|_| Ok(3), &nothing_held);
    assert_eq!(waiting_on(&machine.config, "first"), 3);

    forget_waiting(&machine.config, "first");
    assert_eq!(waiting_on(&machine.config, "first"), 0);
}

/// When every container [`a_machine_whose_runners_are_suspended`] says stopped did, and in seconds.
const STOPPED_AT: &str = "2026-11-01T00:00:00Z";
const STOPPED_AT_SECS: u64 = 1_793_491_200;

/// Notes that the runner `first` last stopped at `at` as its drain began.
fn drained_having_stopped_at(machine: &Machine, at: u64) {
    let locked = runners::lock(&machine.config);
    runners::note_locked(
        &machine.config,
        "first",
        |kept| kept.last_stop = Some(at),
        &locked,
    )
    .unwrap();
}

/// A machine in `config` whose `docker` says every container is stopped, as a suspended
/// runner's is, and removes whatever it is asked to.
fn a_machine_whose_runners_are_suspended(config: &std::path::Path) -> Machine {
    let machine = a_machine(config);
    std::fs::write(
        config.join("docker"),
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$1 $2\" in\n  'container inspect') \
             echo '[{{\"State\":{{\"Running\":false,\"FinishedAt\":\"{STOPPED_AT}\"}}}}]' ;;\n\
             'container ls') for a in \"$@\"; do case $a in id=*) echo \"${{a#id=}}\" ;; esac; done ;;\n\
             'run --rm') cat '{conversations}' 2>/dev/null || echo 'conversations 0 0 0' ;;\n\
             esac\nexit 0\n",
            config.join("log").display(),
            conversations = config.join("conversations").display(),
        ),
    )
    .unwrap();
    machine
}

/// A runner that had stopped before its drain began ran no chat then, and starts none since
/// (#2051): it is removed without being woken to ask. Seen red: with the look at its container
/// taken out, the runner is asked.
#[test]
fn a_runner_that_stopped_before_its_drain_began_is_removed_without_being_asked() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);
    drained_having_stopped_at(&machine, STOPPED_AT_SECS);

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a suspended runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(!log.contains("start "), "{log}");
}

/// **A stopped runner is not an empty one**: found stopped with nothing to say it stopped before
/// its drain began, it may have run chats until then, so it stays with its home, and is not
/// woken to be asked.
#[test]
fn a_draining_runner_found_stopped_keeps_its_home() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a stopped runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(!log.contains("rm "), "{log}");
    assert!(!log.contains("start "), "{log}");
}

/// **A runner that stopped during its drain stays**, though it ran when its drain began.
#[test]
fn a_runner_that_stopped_during_its_drain_keeps_its_home() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);
    drained_having_stopped_at(&machine, 0);

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a stopped runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    assert!(said[0].contains("discarded in Devices"), "{said:?}");
}

/// **A link started it again, so it runs and lists no chat, and that says nothing**: what the stop
/// ended is not listed by the host that came after it, so it stays with its home.
#[test]
fn a_runner_a_link_started_again_during_its_drain_keeps_its_home() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    std::fs::write(
        config.path().join("docker"),
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$1 $2\" in\n  'container inspect') \
             echo '[{{\"State\":{{\"Running\":true,\"FinishedAt\":\"{STOPPED_AT}\"}}}}]' ;;\n\
             esac\nexit 0\n",
            config.path().join("log").display()
        ),
    )
    .unwrap();
    a_pool_draining(&machine);
    drained_having_stopped_at(&machine, 0);

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("its answer says nothing"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
}

/// **A hold lasts**: a row with nothing noted is held as a look finds it stopped, and it stays
/// held once a link has started it again and it answers no chat.
#[test]
fn a_held_runner_stays_held_once_it_runs_again() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);
    follow_and_drain(
        &machine,
        None,
        &|_| panic!("a stopped runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    let again = a_machine(config.path());
    let said = follow_and_drain(
        &again,
        None,
        &|_| panic!("its answer says nothing"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&again), ["first", "first-fedcba98"], "{said:?}");
    assert!(said.is_empty(), "a hold is said once: {said:?}");
}

/// A runner that answered its unpairing cannot be asked, and takes no chat (#4146): the next
/// look removes what is left without asking it.
#[test]
fn an_unpaired_runner_is_removed_without_being_asked_again() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(config.path(), "first", |kept| kept.unpaired = true, &locked)
        .unwrap();
    drop(locked);

    let said = follow_and_drain(&machine, None, &|_| panic!("it is not asked"), &|_| {
        panic!("it is not asked what it holds")
    });

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
}

/// A machine in `config` whose `docker` lists no container and still lists each runner's home,
/// unless `no-home` is there: removing a home fails while `volume-stays` is there.
fn a_machine_with_only_a_home_left(config: &std::path::Path) -> Machine {
    let machine = a_machine(config);
    std::fs::write(
        config.join("docker"),
        format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$1 $2\" in\n  'volume ls')              [ -e '{no_home}' ] || echo purlis-runner-first-home ;;\n  'volume rm')              [ -e '{stays}' ] && exit 1 ;;\n  'container inspect') echo 'no such container' >&2;              exit 1 ;;\nesac\nexit 0\n",
            log = config.join("log").display(),
            stays = config.join("volume-stays").display(),
            no_home = config.join("no-home").display(),
        ),
    )
    .unwrap();
    std::fs::write(config.join("volume-stays"), "").unwrap();
    machine
}

/// **A runner with only its home left is removed without being asked for its chats** (#4298), on
/// a row from before its engine was kept, which no engine's word says was removed. Nothing is
/// left to answer: before, each look said it "stays until it answers" and tried nothing.
#[test]
fn a_runner_with_only_its_home_left_is_only_removed() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_with_only_a_home_left(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(config.path(), "first", |kept| kept.carried = true, &locked)
        .unwrap();
    drop(locked);

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("nothing is left to ask"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert!(said[0].contains("is not all removed"), "{said:?}");
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
    std::fs::remove_file(config.path().join("volume-stays")).unwrap();

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("nothing is left to ask"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert!(said[0].contains("had no chat left"), "{said:?}");
    assert_eq!(names(&machine), ["first-fedcba98"]);
}

/// **A held runner with only its home left stays the person's**, though a carry had begun: a look
/// neither removes it nor says so again.
#[test]
fn a_held_runner_with_only_its_home_left_is_left_alone() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_with_only_a_home_left(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(
        config.path(),
        "first",
        |kept| {
            kept.carried = true;
            kept.held = true;
        },
        &locked,
    )
    .unwrap();
    drop(locked);
    std::fs::remove_file(config.path().join("volume-stays")).unwrap();

    let said = follow_and_drain(&machine, None, &|_| panic!("it is not asked"), &|_| {
        panic!("it is not asked what it holds")
    });

    assert!(said.is_empty(), "{said:?}");
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
}

/// **A Docker that lists neither its container nor its home may be another engine** (#4149): a row
/// with no engine kept stays, though its drain began, and what is on its engine is not orphaned.
#[test]
fn a_runner_whose_home_is_not_listed_either_is_not_removed_on_dockers_word() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_with_only_a_home_left(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(config.path(), "first", |kept| kept.carried = true, &locked)
        .unwrap();
    drop(locked);
    std::fs::write(config.path().join("no-home"), "").unwrap();

    let said = follow_and_drain(&machine, None, &|_| Err("down".into()), &nothing_held);

    assert!(said[0].contains("stays until it answers"), "{said:?}");
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
}

/// A row no drain began on is not removed on Docker's word alone: without an engine kept, Docker
/// may reach another one, where its chats still run (#4149).
#[test]
fn a_runner_whose_drain_did_not_begin_is_still_asked_when_docker_lists_no_container() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_with_only_a_home_left(config.path());
    a_pool_draining(&machine);

    let said = follow_and_drain(&machine, None, &|_| Err("down".into()), &nothing_held);

    assert!(said[0].contains("stays until it answers"), "{said:?}");
    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
}

/// Carried from but not unpaired, it could still take a link, so each look still asks (#4146): a
/// chat there keeps it.
#[test]
fn a_runner_carried_from_but_not_unpaired_is_still_asked() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path());
    a_pool_draining(&machine);
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::note_locked(config.path(), "first", |kept| kept.carried = true, &locked)
        .unwrap();
    drop(locked);

    follow_and_drain(&machine, None, &|_| Ok(1), &nothing_held);

    assert_eq!(names(&machine), ["first", "first-fedcba98"]);
}

/// **An update's runner that had stopped before its drain, its home holding a chat's
/// conversation, is kept** (#4633, ruled 2026-11-02): not woken, not removed, and said once.
#[test]
fn a_runner_that_stopped_before_its_drain_keeps_a_home_that_holds_a_conversation() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);
    drained_having_stopped_at(&machine, STOPPED_AT_SECS);
    std::fs::write(config.path().join("conversations"), "conversations 3 3 0\n").unwrap();

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a suspended runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    assert!(said[0].contains("conversations"), "{said:?}");
    assert!(
        purlis_core::runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(!log.contains("start "), "{log}");
    assert!(
        follow_and_drain(&machine, None, &|_| panic!("not asked"), &|_| panic!(
            "it is not asked what it holds"
        ))
        .is_empty()
    );
}

/// **A home that could not be read keeps its runner as it is**, neither held nor removed, until a
/// look reads it.
#[test]
fn a_stopped_runner_whose_home_could_not_be_read_stays() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_runners_are_suspended(config.path());
    a_pool_draining(&machine);
    drained_having_stopped_at(&machine, STOPPED_AT_SECS);
    std::fs::write(config.path().join("conversations"), "").unwrap();

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a suspended runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    assert!(said[0].contains("its home is read"), "{said:?}");
    assert!(
        !purlis_core::runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
}

/// A machine in `config` whose `docker` reaches engine `engine-a`, lists no container, and still
/// lists each runner's home, made at [`HOME_MADE`], holding what `conversations` says.
fn a_machine_whose_draining_runner_was_pruned(config: &std::path::Path) -> Machine {
    let machine = a_machine(config);
    std::fs::write(
        config.join("docker"),
        format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$1 $2\" in\n  \
             'volume ls') echo purlis-runner-first-home ;;\n  \
             'volume inspect') echo '[{{\"CreatedAt\":\"{HOME_MADE}\"}}]' ;;\n  \
             'info --format') echo engine-a ;;\n  \
             'run --rm') cat '{conversations}' ;;\n  \
             'container inspect') echo 'no such container' >&2; exit 1 ;;\n\
             esac\nexit 0\n",
            log = config.join("log").display(),
            conversations = config.join("conversations").display(),
        ),
    )
    .unwrap();
    machine
}

const HOME_MADE: &str = "2026-10-01T00:00:00Z";

/// Pool `first` draining its runner `first`, made on `engine-a` with its home made at
/// [`HOME_MADE`], whose container was then removed outside purlis.
fn a_pool_draining_a_pruned_runner(machine: &Machine) {
    a_pool_draining(machine);
    let locked = purlis_core::runners::lock(&machine.config);
    purlis_core::runners::note_locked(
        &machine.config,
        "first",
        |kept| {
            kept.engine = Some("engine-a".into());
            kept.home_created = Some(HOME_MADE.into());
        },
        &locked,
    )
    .unwrap();
}

/// **A draining runner not yet held, its container removed outside purlis, is kept where its home
/// holds a conversation** (#4168, #4633): the same rule as after an update.
#[test]
fn a_pruned_draining_runner_keeps_a_home_that_holds_a_conversation() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_draining_runner_was_pruned(config.path());
    a_pool_draining_a_pruned_runner(&machine);
    std::fs::write(config.path().join("conversations"), "conversations 1 1 0\n").unwrap();

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a pruned runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first", "first-fedcba98"], "{said:?}");
    assert!(said[0].contains("conversations"), "{said:?}");
    assert!(
        purlis_core::runners::named(config.path(), "first")
            .unwrap()
            .unwrap()
            .held
    );
    let log = std::fs::read_to_string(config.path().join("log")).unwrap();
    assert!(!log.contains("volume rm"), "{log}");
}

/// **One whose home holds none goes as before** (#4168): forgotten, what is left removed.
#[test]
fn a_pruned_draining_runner_whose_home_holds_none_is_forgotten() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine_whose_draining_runner_was_pruned(config.path());
    a_pool_draining_a_pruned_runner(&machine);
    std::fs::write(config.path().join("conversations"), "conversations 0 0 0\n").unwrap();

    let said = follow_and_drain(
        &machine,
        None,
        &|_| panic!("a pruned runner is not asked"),
        &|_| panic!("it is not asked what it holds"),
    );

    assert_eq!(names(&machine), ["first-fedcba98"], "{said:?}");
    assert!(said[0].contains("removed outside purlis"), "{said:?}");
}

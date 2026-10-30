// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn work() -> onrunner::Kept {
    onrunner::Kept {
        name: "work".to_owned(),
        harness: "claude".to_owned(),
        sandbox_only: false,
        pinned: None,
        settings_in: None,
    }
}

/// **A pool not shown to be the person's own is sent nothing** (#2049, D-2049-5): keeping a
/// profile there answers that the runner is its own to say, and reads nothing of the pool, so a
/// config home that holds none is never asked.
#[test]
fn a_pool_not_shown_to_be_owned_is_sent_nothing_and_is_its_own_to_say() {
    let config = tempfile::tempdir().unwrap();

    let said = keep_on_as(
        Err("DOCKER_HOST names tcp://builds:2375".to_owned()),
        &config.path().join("nowhere"),
        "builds",
        &work(),
        Waking::No,
        &Default::default(),
    );

    assert_eq!(said, Ok(None));
}

/// **A pool not shown to be owned is sent no setting, and none is read** (#3448): a profile
/// whose settings could not be read at all (larger than a link carries) is the runner's own to
/// say there, so nothing of its folder was read.
#[test]
fn a_pool_not_shown_to_be_owned_is_sent_no_setting_and_none_is_read() {
    let config = tempfile::tempdir().unwrap();
    let settings = tempfile::tempdir().unwrap();
    std::fs::write(
        settings.path().join("settings.json"),
        vec![b' '; onrunner::MOST_SETTINGS_BYTES + 1],
    )
    .unwrap();
    let kept = onrunner::Kept {
        settings_in: Some(settings.path().to_path_buf()),
        ..work()
    };
    assert!(onrunner::to_send(&kept).is_err(), "reading it would refuse");

    let said = keep_on_as(
        Err("DOCKER_HOST names tcp://builds:2375".to_owned()),
        &config.path().join("nowhere"),
        "builds",
        &kept,
        Waking::Yes,
        &Default::default(),
    );

    assert_eq!(said, Ok(None));
}

/// The same keep on a pool shown to be the person's own goes on to the pool, and is refused
/// where this machine keeps no such pool.
#[test]
fn on_an_owned_pool_it_goes_on_to_the_pool() {
    let config = tempfile::tempdir().unwrap();

    let said = keep_on_as(
        Ok(PathBuf::from("/var/run/docker.sock")),
        config.path(),
        "builds",
        &work(),
        Waking::No,
        &Default::default(),
    );

    assert!(said.is_err(), "{said:?}");
}

/// **One watcher per pool and profile** (#2049): a second Sign in while one asks claims none,
/// and another profile or pool claims its own.
#[test]
fn a_watcher_is_claimed_once_per_pool_and_profile() {
    let mut watched = HashSet::new();
    let key = ("builds".to_owned(), "work".to_owned());

    assert!(claimed(&mut watched, &key));
    assert!(!claimed(&mut watched, &key), "a second Sign in starts none");
    assert!(claimed(
        &mut watched,
        &("builds".to_owned(), "home".to_owned())
    ));
    assert!(claimed(
        &mut watched,
        &("other".to_owned(), "work".to_owned())
    ));
}

/// One of this machine's made profiles, `name`, of `harness`.
fn record(name: &str, harness: &str) -> profilestore::MachineProfile {
    profilestore::MachineProfile {
        name: name.to_owned(),
        harness: harness.to_owned(),
        title: format!("{name} (this Mac)"),
        folders: profilestore::Folders {
            config: PathBuf::from("/profiles").join(name).join("config"),
            data: None,
        },
        adopted: None,
        imported: None,
        sign_in: None,
        pinned: None,
        sandbox_only: false,
        kept_for: None,
        rest: serde_json::Map::default(),
    }
}

/// **A pool offers only the profiles chosen for it** (#4047), in their order, and Add a
/// profile… offers this machine's others that go to a runner: never one already on it, nor one
/// no runner keeps.
#[test]
fn a_pool_offers_its_chosen_profiles_and_add_offers_the_rest() {
    let records = vec![
        record("work", "claude"),
        record("home", "codex"),
        record("tests", "claude"),
        record("local", "made-up-harness"),
    ];

    let (on_pool, addable) = chosen(&records, &["tests".to_owned(), "work".to_owned()]);

    assert_eq!(
        on_pool
            .iter()
            .map(|one| one.as_ref().map(|r| r.name.as_str()))
            .collect::<Vec<_>>(),
        vec![Ok("tests"), Ok("work")]
    );
    assert_eq!(
        addable.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        vec!["home"]
    );
}

/// A profile chosen for a pool that this machine no longer has is still listed, so it can be
/// removed, and says so.
#[test]
fn a_chosen_profile_this_machine_no_longer_has_is_listed_to_be_removed() {
    let records = vec![record("work", "claude")];

    let (on_pool, addable) = chosen(&records, &["gone".to_owned()]);

    assert_eq!(on_pool.len(), 1);
    let missing = on_pool[0].as_ref().unwrap_err();
    assert!(missing.contains("no profile named 'gone'"), "{missing}");
    assert_eq!(addable.len(), 1);
}

/// **A project's default that this machine keeps no record of is the runner's own** (#4047): a
/// built-in such as `claude`, never adopted here, is not one of this machine's to keep there,
/// so a pool that names none keeps nothing of this machine's rather than a missing row.
#[test]
fn a_default_this_machine_keeps_no_record_of_is_the_runners_own() {
    let records = vec![record("work", "claude")];

    assert_eq!(
        of_this_machine(Some("work".to_owned()), &records),
        Some("work".to_owned())
    );
    assert_eq!(of_this_machine(Some("claude".to_owned()), &records), None);
    assert_eq!(of_this_machine(None, &records), None);
}

/// A config home that keeps the Docker pool `builds`, naming `profiles` (`None`: none).
fn a_pool_naming(profiles: Option<&[&str]>) -> tempfile::TempDir {
    let config = tempfile::tempdir().unwrap();
    let image = purlis_core::dockerpool::Image::pinned(
        "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .unwrap();
    let pool = purlis_core::runners::Pool::new("builds", image, purlis_core::runners::Set::default())
        .unwrap()
        .keeping_profiles(profiles.map(|names| names.iter().map(|&n| n.to_owned()).collect()));
    purlis_core::runners::record_pool(config.path(), &pool).unwrap();
    config
}

/// The profiles the pool `builds` under `config` names now.
fn named(config: &Path) -> Option<Vec<String>> {
    purlis_core::runners::pools(config).unwrap()[0]
        .profiles()
        .map(<[String]>::to_vec)
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|&n| n.to_owned()).collect()
}

/// **A runner keeps only the profiles chosen for its pool** (#4060, #3849): it is asked to keep
/// the chosen ones this machine still has, and forgets the rest. One chosen that this machine no
/// longer has is forgotten and leaves the pool's record; one the runner would not forget, under
/// a running chat, is left to ask again.
#[test]
fn a_runner_forgets_what_is_no_longer_chosen_and_a_removed_profile_leaves_the_record() {
    let config = a_pool_naming(Some(&["work", "gone"]));
    let records = vec![record("work", "claude"), record("home", "claude")];

    let forgotten = tidy(config.path(), "builds", &records, Some("home"), |keep| {
        assert_eq!(keep, names(&["work"]), "only what is chosen and still here");
        Ok(Some(vec![
            ("home".to_owned(), Ok(())),
            ("gone".to_owned(), Ok(())),
            ("busy".to_owned(), Err("a chat on that profile runs".to_owned())),
        ]))
    })
    .unwrap();

    assert_eq!(forgotten, Some(names(&["home", "gone"])));
    assert_eq!(named(config.path()), Some(names(&["work"])));
}

/// **A pool that names none keeps the open project's default** (#4060), and once its runner
/// forgets another, it names that default from now on, so a project opened next with another
/// default does not have the runner forget this one.
#[test]
fn a_pool_that_names_none_keeps_the_default_and_names_it_once_something_is_forgotten() {
    let config = a_pool_naming(None);
    let records = vec![record("work", "claude"), record("home", "claude")];

    let forgotten = tidy(config.path(), "builds", &records, Some("work"), |keep| {
        assert_eq!(keep, names(&["work"]));
        Ok(Some(vec![("home".to_owned(), Ok(()))]))
    })
    .unwrap();

    assert_eq!(forgotten, Some(names(&["home"])));
    assert_eq!(named(config.path()), Some(names(&["work"])));
}

/// A runner with nothing to forget leaves a pool that names none as it is.
#[test]
fn a_runner_with_nothing_to_forget_leaves_the_pools_record_as_it_is() {
    let config = a_pool_naming(None);
    let records = vec![record("work", "claude")];

    let forgotten = tidy(config.path(), "builds", &records, Some("work"), |_| Ok(Some(Vec::new())))
        .unwrap();

    assert_eq!(forgotten, Some(Vec::new()));
    assert_eq!(named(config.path()), None);
}

/// **Nothing is forgotten where nothing can be judged**: a pool that names none with no project
/// open asks the runner nothing, and a runner before 1.59, which cannot say what it keeps, is
/// left as it is.
#[test]
fn nothing_is_forgotten_with_no_default_to_judge_by_or_on_a_runner_before_1_59() {
    let config = a_pool_naming(None);
    let records = vec![record("work", "claude")];

    let asked = tidy(config.path(), "builds", &records, None, |_| {
        panic!("a pool that names none, with no default, asks its runner nothing")
    })
    .unwrap();
    assert_eq!(asked, None);

    let config = a_pool_naming(Some(&["gone"]));
    let older = tidy(config.path(), "builds", &records, None, |_| Ok(None)).unwrap();
    assert_eq!(older, None);
    assert_eq!(named(config.path()), Some(names(&["gone"])), "kept to remove by hand");
}

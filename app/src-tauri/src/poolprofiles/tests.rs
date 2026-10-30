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
        Ok(Yours::Docker(PathBuf::from("/var/run/docker.sock"))),
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
    let pool =
        purlis_core::runners::Pool::new("builds", image, purlis_core::runners::Set::default())
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

/// A runner of 1.57 that keeps `kept` for this machine and refuses to forget `busy`, under a
/// chat running on it: what `forget_all_but` answers, by the predicate it is given.
fn a_runner_keeping(
    kept: &[&str],
    busy: &[&str],
) -> impl FnOnce(Keep) -> Result<Option<crate::runnerlinks::Listed>, String> {
    let (kept, busy) = (names(kept), names(busy));
    move |keep| {
        let mut listed = crate::runnerlinks::Listed::default();
        for name in kept {
            if keep(&name) {
                listed.kept.push(name);
            } else {
                let said = if busy.contains(&name) {
                    Err("a chat on that profile runs".to_owned())
                } else {
                    Ok(())
                };
                listed.forgot.push((name, said));
            }
        }
        Ok(Some(listed))
    }
}

/// **A runner keeps only the profiles chosen for its pool** (#4060, #3849): of a pool that
/// names its own, one not chosen is forgotten, and one chosen that this machine no longer has is
/// forgotten and leaves the record. One the runner would not forget, under a running chat, is
/// left to ask again.
#[test]
fn a_runner_forgets_what_is_no_longer_chosen_and_a_removed_profile_leaves_the_record() {
    let config = a_pool_naming(Some(&["work", "gone"]));
    let records = vec![
        record("work", "claude"),
        record("home", "claude"),
        record("busy", "claude"),
    ];

    let tidied = tidy(
        config.path(),
        "builds",
        &records,
        Some("home"),
        a_runner_keeping(&["work", "home", "gone", "busy"], &["busy"]),
    )
    .unwrap();

    assert_eq!(
        tidied,
        Tidied::Done {
            forgotten: names(&["home", "gone"]),
            refused: true
        }
    );
    assert_eq!(named(config.path()), Some(names(&["work"])));
}

/// **A pool made before #4047, which names none, forgets only what this machine no longer
/// has** (#4060): what its runner keeps of this machine's is named on the pool beside the open
/// project's default, so its row shows each one, and no project opened next has the runner
/// forget another's.
#[test]
fn a_pool_that_names_none_names_what_its_runner_keeps_and_forgets_only_what_is_gone() {
    let config = a_pool_naming(None);
    let records = vec![record("work", "claude"), record("home", "claude")];

    let tidied = tidy(
        config.path(),
        "builds",
        &records,
        Some("work"),
        a_runner_keeping(&["home", "work", "gone"], &[]),
    )
    .unwrap();

    assert_eq!(
        tidied,
        Tidied::Done {
            forgotten: names(&["gone"]),
            refused: false
        }
    );
    assert_eq!(named(config.path()), Some(names(&["work", "home"])));
}

/// A runner that keeps only the default leaves a pool that names none as it is.
#[test]
fn a_runner_that_keeps_only_the_default_leaves_the_pools_record_as_it_is() {
    let config = a_pool_naming(None);
    let records = vec![record("work", "claude")];

    tidy(
        config.path(),
        "builds",
        &records,
        Some("work"),
        a_runner_keeping(&["work"], &[]),
    )
    .unwrap();

    assert_eq!(named(config.path()), None);
}

/// **No tidy forgets this machine's adopted folder or a profile a chat's start readies**: the
/// harness's own folder on the runner is used by a chat with no profile there too, and a
/// readied profile is about to start a chat, which names it on the pool. The adopted folder is
/// not named back on a pool the person took it off.
#[test]
fn an_adopted_folder_and_a_profile_being_readied_are_never_forgotten_by_a_tidy() {
    let config = a_pool_naming(Some(&["work"]));
    let mut adopted = record("claude", "claude");
    adopted.adopted = Some(profilestore::Adopted {
        at: 1,
        variables: Vec::new(),
    });
    let records = vec![record("work", "claude"), record("next", "claude"), adopted];
    let _readying = Readying::of("builds", "next");

    let tidied = tidy(
        config.path(),
        "builds",
        &records,
        None,
        a_runner_keeping(&["work", "claude", "next"], &[]),
    )
    .unwrap();

    assert_eq!(
        tidied,
        Tidied::Done {
            forgotten: Vec::new(),
            refused: false
        }
    );
    assert_eq!(named(config.path()), Some(names(&["work", "next"])));
}

/// **Nothing is forgotten where nothing can be judged**: a pool that names none with no project
/// open asks the runner nothing, and a runner before 1.57, which cannot say what it keeps, is
/// left as it is.
#[test]
fn nothing_is_forgotten_with_no_default_to_judge_by_or_on_a_runner_before_1_57() {
    let config = a_pool_naming(None);
    let records = vec![record("work", "claude")];

    let asked = tidy(config.path(), "builds", &records, None, |_| {
        panic!("a pool that names none, with no default, asks its runner nothing")
    })
    .unwrap();
    assert_eq!(asked, Tidied::NothingToJudge);

    let config = a_pool_naming(Some(&["gone"]));
    let older = tidy(config.path(), "builds", &records, None, |_| Ok(None)).unwrap();
    assert_eq!(older, Tidied::Older);
    assert_eq!(
        named(config.path()),
        Some(names(&["gone"])),
        "kept to remove by hand"
    );
}

/// The pools whose runner keeps a removed profile are named in one sentence, and none is said
/// where there is none.
#[test]
fn the_pools_whose_runner_keeps_a_removed_profile_are_named_in_one_sentence() {
    assert_eq!(older_note(&[], "work"), None);
    assert_eq!(
        older_note(&names(&["builds"]), "work").unwrap(),
        "Pool builds keeps the profile 'work' and its sign-in on its runner, which runs a \
         purlis before 1.52, until the runner is made again."
    );
    assert!(
        older_note(&names(&["builds", "tests", "gpu"]), "work")
            .unwrap()
            .starts_with("Pools builds, tests and gpu keep the profile 'work'")
    );
}

/// The cluster pool `team` and its runner, kept under `config`, as this machine reads them.
#[cfg(unix)]
fn a_cluster_pool(
    config: &Path,
) -> (
    purlis_core::runners::ClusterPool,
    purlis_core::runners::ClusterKept,
) {
    crate::clusterpools::tests::a_cluster_pool_kept(config);
    purlis_core::runners::cluster_runner_for(config, "team")
        .unwrap()
        .unwrap()
}

/// **A cluster pool's runner keeps a profile only where the person marked the pool and its
/// claim is vouched for now** (#3819, the gate of #4071): unmarked, nothing asks its cluster and
/// nothing is sent; marked, the cluster is asked, and a refusal there sends nothing and tells
/// none of the cluster's words.
#[cfg(unix)]
#[test]
fn a_cluster_pool_keeps_a_profile_only_marked_and_vouched_for_now() {
    let config = tempfile::tempdir().unwrap();
    let (cluster, kept) = a_cluster_pool(config.path());
    let asked = std::cell::Cell::new(0);
    let vouches = |_: &Path, _: &str| {
        asked.set(asked.get() + 1);
        Ok(())
    };

    let unmarked = yours_on_a_cluster(config.path(), "team", &cluster, &kept, &vouches);
    let why = unmarked.clone().unwrap_err();
    assert!(why.contains("This is my own"), "{why}");
    assert_eq!(asked.get(), 0, "an unmarked pool's cluster is never asked");
    assert_eq!(
        keep_on_as(
            unmarked,
            config.path(),
            "team",
            &work(),
            Waking::No,
            &RefCell::default()
        ),
        Ok(None),
        "nothing is sent"
    );

    purlis_core::runners::set_cluster_owned(config.path(), "team", true).unwrap();
    let (cluster, kept) = purlis_core::runners::cluster_runner_for(config.path(), "team")
        .unwrap()
        .unwrap();
    let judged = yours_on_a_cluster(config.path(), "team", &cluster, &kept, &vouches).unwrap();
    assert_eq!(judged, Yours::Cluster(Box::new(kept.clone())));
    assert_eq!(asked.get(), 1);
    assert!(still_yours(config.path(), "team", &kept).is_ok());

    let refused = yours_on_a_cluster(config.path(), "team", &cluster, &kept, &|_, _| {
        Err(purlis_core::clusterpool::Refused::Other(
            "kubectl said secret-host:6443".into(),
        ))
    })
    .unwrap_err();
    assert!(refused.contains("could not confirm"), "{refused}");
    assert!(!refused.contains("secret-host"), "{refused}");

    // The mark taken back while its cluster was asked: nothing is kept there.
    let taken_back = yours_on_a_cluster(config.path(), "team", &cluster, &kept, &|config, pool| {
        purlis_core::runners::set_cluster_owned(config, pool, false).unwrap();
        Ok(())
    });
    assert!(taken_back.is_err(), "{taken_back:?}");
    // Taken back after the gate: nothing reaches the runner it judged, by link or sign-in.
    let refused = still_yours(config.path(), "team", &kept).unwrap_err();
    assert!(
        refused.contains("changed since purlis confirmed it"),
        "{refused}"
    );
    let to = to_a_runner(config.path(), "team", Waking::No, &judged);
    assert!(to.is_err());
}

/// **A cluster pool's profiles are its own record's** (#3819): the same `profiles` a Docker pool
/// keeps, so its row lists what was chosen for it.
#[cfg(unix)]
#[test]
fn a_cluster_pools_profiles_are_the_ones_chosen_for_it() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool(config.path());
    assert_eq!(
        kept_on(config.path(), "team", Some("work")),
        Ok(vec!["work".to_owned()])
    );
    purlis_core::runners::add_pool_profile(config.path(), "team", "home", Some("work")).unwrap();
    assert_eq!(
        kept_on(config.path(), "team", None),
        Ok(vec!["work".to_owned(), "home".to_owned()])
    );
    assert!(changeable(config.path(), "team", None).is_ok());
    assert!(kept_on(config.path(), "nowhere", None).is_err());
}

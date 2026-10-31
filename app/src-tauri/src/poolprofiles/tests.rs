// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::cell::RefCell;

fn work() -> onrunner::Kept {
    onrunner::Kept {
        name: "work".to_owned(),
        harness: "claude".to_owned(),
        sandbox_only: false,
        pinned: None,
        settings_in: None,
        limits: Default::default(),
        highest: Default::default(),
    }
}

/// **A wait let go everywhere tells each project whose wait went** (#4228), and nothing is told
/// where none did.
#[test]
fn a_wait_let_go_everywhere_tells_each_project_it_went_from() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let store = profilestore::Homes::at(&config, &dir.path().join("data"));
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = |_: &str| -> Option<String> { None };
    profilestore::create(
        &store,
        "work",
        "claude",
        &profilestore::MachineSettings {
            home: &home,
            env: &env,
        },
        1,
    )
    .expect("made");
    let (one, two) = (dir.path().join("one"), dir.path().join("two"));
    for root in [&one, &two] {
        onrunner::ready_on(&config, root, "tells", "work", &|_| {
            Ok(Some(Standing::SignedOut))
        })
        .unwrap_err();
    }

    let told = RefCell::new(Vec::new());
    let_go_telling("tells", "work", |went| told.borrow_mut().extend(went));
    let mut told = told.into_inner();
    told.sort();
    assert_eq!(told, [one, two]);

    let_go_telling("tells", "work", |_| panic!("none was left to tell"));
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
    );

    assert!(said.is_err(), "{said:?}");
}

/// **Only signed in counts for a watcher** (#4009): every other answer keeps the Ask.
#[test]
fn only_a_signed_in_answer_says_signed_in() {
    assert!(says_signed_in(&Ok(Some(Standing::SignedIn))));
    for said in [
        Ok(Some(Standing::SignedOut)),
        Ok(Some(Standing::CannotTell)),
        Ok(Some(Standing::Fetching)),
        Ok(None),
        Err("the runner did not answer".to_owned()),
    ] {
        assert!(!says_signed_in(&said), "{said:?}");
    }
}

/// **A profile off this machine or off the pool, or a pool removed, is gone for a watcher**; a
/// pool naming none keeps every profile, and what cannot be read is not gone.
#[test]
fn a_watched_profile_is_there_until_it_is_taken_away() {
    let none = || -> Result<Vec<profilestore::MachineProfile>, String> { Ok(Vec::new()) };
    let unread = || -> Result<Vec<profilestore::MachineProfile>, String> { Err("no".to_owned()) };
    type Named = Result<Option<Option<Vec<String>>>, String>;
    let names = |names: &[&str]| -> Named {
        Ok(Some(Some(
            names.iter().map(|one| (*one).to_owned()).collect(),
        )))
    };
    let every: Named = Ok(Some(None));
    let no_pool: Named = Ok(None);
    let cannot_read: Named = Err("no".to_owned());

    assert!(!still_on_the_pool(none(), every.clone(), "work"));
    assert!(still_on_the_pool(unread(), every, "work"));
    assert!(still_on_the_pool(unread(), names(&["work"]), "work"));
    assert!(!still_on_the_pool(unread(), names(&["home"]), "work"));
    assert!(!still_on_the_pool(unread(), no_pool, "work"));
    assert!(still_on_the_pool(unread(), cannot_read, "work"));
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
        Ok("pod-uid-1".to_owned())
    };

    let unmarked = yours_on_a_cluster(config.path(), "team", &cluster, &kept, &vouches);
    let why = unmarked.clone().unwrap_err();
    assert!(why.contains("This is my own"), "{why}");
    assert_eq!(asked.get(), 0, "an unmarked pool's cluster is never asked");
    assert_eq!(
        keep_on_as(unmarked, config.path(), "team", &work(), Waking::No,),
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
        Ok("pod-uid-1".to_owned())
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

/// **A sign-out says which draining runners it reached** (#4121).
#[test]
fn a_sign_out_names_the_draining_runners_it_reached_and_the_ones_it_did_not() {
    let (a, b, c) = ("old-a".to_owned(), "old-b".to_owned(), "old-c".to_owned());
    assert_eq!(draining_said("work", &[]), None);
    assert_eq!(
        draining_said(
            "work",
            &[(a, Ok(())), (b, Err("stopped".to_owned())), (c, Ok(()))]
        )
        .unwrap(),
        "Also signed out on draining runners old-a and old-c: its chats there sign in again. \
         Not reached on draining runner old-b: its sign-in stays there until the runner is \
         removed, and is not carried."
    );
}

/// **A sign-out is done only once the runner's status check says signed out**: every other
/// answer keeps the person from relying on it.
#[test]
fn a_sign_out_is_done_only_where_the_harness_says_signed_out() {
    let said = |asked| signed_out_by_its_harness("work", "builds", asked);

    assert_eq!(said(Ok(Some(Standing::SignedOut))), Ok(()));

    let still = said(Ok(Some(Standing::SignedIn))).expect_err("still signed in");
    assert!(
        still.contains("work is still signed in on pool builds"),
        "{still}"
    );

    for asked in [
        Ok(Some(Standing::CannotTell)),
        Ok(Some(Standing::Fetching)),
        Ok(None),
        Err("the runner did not answer".to_owned()),
    ] {
        let why = said(asked.clone()).expect_err("not confirmed");
        assert!(why.contains("could not confirm"), "{asked:?}: {why}");
    }
}

/// A Docker runner of pool `builds`, in container `container`, answering as `device`.
fn a_runner_in(container: char, device: &str) -> purlis_core::runners::Kept {
    purlis_core::runners::Kept {
        made: purlis_core::dockerpool::Made {
            container: purlis_core::dockerpool::ContainerId::new(&container.to_string().repeat(64))
                .unwrap(),
            runner: purlis_core::dockerpool::Runner::new(
                "builds",
                purlis_core::dockerpool::Image::pinned(
                    "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )
                .unwrap(),
                purlis_core::dockerpool::User::new(10001, 10001).unwrap(),
                "/home/purlis",
            )
            .unwrap(),
        },
        pinned: purlis_session_protocol::hostlink::Pinned {
            device: device.into(),
            key: purlis_session_protocol::hostlink::PublicKey::from_bytes([7; 32]),
        },
        paired: 0,
        of: None,
        draining: false,
        carried: false,
        unpaired: false,
        signed_out: Vec::new(),
        engine: None,
        home_created: None,
    }
}

/// **A sign-in's redirect goes only to the Docker runner it started on**: one made again since,
/// or answering as another device, holds no login of it.
#[test]
fn a_runner_made_again_is_not_the_one_a_sign_in_started_on() {
    let started = a_runner_in('a', "01JB0RUNNER0000000000000000");

    let mut paired_again = started.clone();
    paired_again.paired = 9;
    assert!(the_same_runner(&paired_again, &started));

    assert!(!the_same_runner(
        &a_runner_in('b', "01JB0RUNNER0000000000000000"),
        &started
    ));
    assert!(!the_same_runner(
        &a_runner_in('a', "01JB0RUNNER0000000000000001"),
        &started
    ));
}

/// **The carry refuses a Docker runner made again since the sign-in started**, by what this
/// machine keeps of its pool's runner.
#[test]
fn a_carry_goes_on_only_while_the_kept_runner_is_the_one_it_started_on() {
    let config = a_pool_naming(None);
    let ours = purlis_session_protocol::hostlink::Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: purlis_session_protocol::hostlink::PublicKey::from_bytes([1; 32]),
    };
    let runner = a_runner_in('a', "01JB0RUNNER0000000000000000");
    let locked = purlis_core::runners::lock(config.path());
    let kept = purlis_core::runners::keep_for_its_pool(
        config.path(),
        &runner.made,
        &runner.pinned,
        &ours,
        &locked,
    )
    .unwrap();
    drop(locked);

    assert_eq!(
        still_the_runner(config.path(), "builds", &kept).as_ref(),
        Ok(&kept)
    );
    let other = a_runner_in('b', "01JB0RUNNER0000000000000000");
    let why = still_the_runner(config.path(), "builds", &other).unwrap_err();
    assert!(why.contains("made again"), "{why}");
}

/// **A cluster runner keeps only what is chosen for its pool** (#4183): what it keeps beyond the
/// pool's choice is forgotten, as on a Docker pool.
#[cfg(unix)]
#[test]
fn a_cluster_runner_forgets_what_is_no_longer_chosen() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool(config.path());
    purlis_core::runners::add_pool_profile(config.path(), "team", "work", None).unwrap();
    let records = vec![record("work", "claude"), record("home", "claude")];

    let tidied = tidy(
        config.path(),
        "team",
        &records,
        Some("work"),
        a_runner_keeping(&["work", "home", "gone"], &[]),
    )
    .unwrap();

    assert_eq!(
        tidied,
        Tidied::Done {
            forgotten: names(&["home", "gone"]),
            refused: false
        }
    );
    assert_eq!(
        purlis_core::runners::profiles_of(config.path(), "team").unwrap(),
        Some(names(&["work"]))
    );
}

/// **A cluster runner's word never widens what goes to it** (#4183): a pool that names none comes
/// to name the project's default, not what its runner says it keeps, and the rest is forgotten.
#[cfg(unix)]
#[test]
fn a_cluster_pool_that_names_none_names_its_default_and_not_what_its_runner_says_it_keeps() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool(config.path());
    let records = vec![record("work", "claude"), record("private", "claude")];

    let tidied = tidy(
        config.path(),
        "team",
        &records,
        Some("work"),
        a_runner_keeping(&["work", "private"], &[]),
    )
    .unwrap();

    assert_eq!(
        tidied,
        Tidied::Done {
            forgotten: names(&["private"]),
            refused: false
        }
    );
    assert_eq!(
        purlis_core::runners::profiles_of(config.path(), "team").unwrap(),
        Some(names(&["work"]))
    );
}

/// **An unmarked cluster pool is asked nothing** (#4183): not by a chat's start's tidy, nor when
/// a profile is removed on this Mac, and neither reaches for its cluster.
#[cfg(unix)]
#[test]
fn an_unmarked_cluster_pool_is_asked_nothing_by_a_tidy_or_a_removal() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool(config.path());

    let refused = forget_on(config.path(), "team", "work").unwrap_err();
    assert!(refused.contains("not marked as your own"), "{refused}");

    let not_yours = yours(config.path(), "team");
    assert!(not_yours.is_err());
    tidy_once(&not_yours, config.path(), Path::new("/nowhere"), "team");
    assert!(
        !TIDIED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|one| one == "team"),
        "a tidy that asked nothing is not counted as done"
    );
}

/// **A profile removed on this Mac goes off each pool whose runner forgot it, of either kind**
/// (#4183); a pool whose runner was not shown to be yours keeps its name, to be asked again.
#[cfg(unix)]
#[test]
fn a_removed_profile_leaves_a_cluster_pool_only_where_its_runner_forgot_it() {
    let config = tempfile::tempdir().unwrap();
    a_cluster_pool(config.path());
    purlis_core::runners::add_pool_profile(config.path(), "team", "work", None).unwrap();
    let pools = names(&["team"]);

    let note = forget_off(config.path(), &pools, "work", &|_| {
        Err("not vouched for now".to_owned())
    });
    assert_eq!(note, None);
    assert_eq!(
        purlis_core::runners::profiles_of(config.path(), "team").unwrap(),
        Some(names(&["work"]))
    );

    let asked = std::sync::Mutex::new(Vec::new());
    let note = forget_off(config.path(), &pools, "work", &|pool| {
        asked.lock().unwrap().push(pool.to_owned());
        Ok(Some(()))
    });
    assert_eq!(note, None);
    assert_eq!(*asked.lock().unwrap(), names(&["team"]));
    assert_eq!(
        purlis_core::runners::profiles_of(config.path(), "team").unwrap(),
        Some(Vec::new())
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's chats routed by workspace (#2046): this machine's host and a pool's runner, each
//! a host that runs nothing, so where each chat and each command goes is what is seen.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_core::engine::Size;
use purlis_core::session::Exit;

use super::*;
use crate::host::pretend::Pretend;

/// `app` runs on pool `builds`, `stale` names a pool this machine does not keep, every other
/// workspace runs here.
fn runs_on() -> RunsOn {
    Arc::new(|workspace| match workspace {
        "app" => Ok(Some("builds".to_owned())),
        "stale" => Err("Workspace stale runs its chats on pool gone: it is not kept".to_owned()),
        _ => Ok(None),
    })
}

fn on_pools() -> OnPools {
    Arc::new(|| BTreeMap::from([("builds".to_owned(), vec!["app".to_owned()])]))
}

struct World {
    here: Pretend,
    builds: Pretend,
    made: Arc<Mutex<Vec<String>>>,
    sessions: RoutedSessions,
}

fn a_world() -> World {
    let (here, builds) = (Pretend::default(), Pretend::default());
    builds.on_a_runner();
    let made: Arc<Mutex<Vec<String>>> = Arc::default();
    let to_a_pool: ToAPool = {
        let (builds, made) = (builds.clone(), Arc::clone(&made));
        Arc::new(move |pool| {
            made.lock().unwrap().push(pool.to_owned());
            match pool {
                "builds" => Ok(Box::new(builds.clone()) as Box<dyn SessionHost>),
                other => Err(format!("pool {other} is not kept")),
            }
        })
    };
    let sessions = RoutedSessions::new(
        Path::new("/planes/a"),
        Box::new(here.clone()),
        runs_on(),
        on_pools(),
        to_a_pool,
    );
    World {
        here,
        builds,
        made,
        sessions,
    }
}

fn named_in(workspace: &str) -> Named {
    Named {
        workspace: Some(workspace.to_owned()),
        ..crate::hostedsessions::tests::by_its_names()
    }
}

fn a_sink() -> Sink {
    Box::new(|_| {})
}

/// What this machine's store keeps of the project's runner chats, which outlives a window.
#[derive(Default)]
struct TheStore {
    chats: Mutex<BTreeMap<u32, purlis_core::machine::ChatOnPool>>,
    owed: Mutex<BTreeMap<String, BTreeSet<String>>>,
}

impl TheStore {
    fn owes(&self, pool: &str, workspace: &str) {
        self.owed
            .lock()
            .unwrap()
            .entry(pool.to_owned())
            .or_default()
            .insert(workspace.to_owned());
    }
}

impl KeptPlaces for TheStore {
    fn keep(&self, number: u32, pool: &str, workspace: &str) {
        self.chats.lock().unwrap().insert(
            number,
            purlis_core::machine::ChatOnPool {
                pool: pool.to_owned(),
                workspace: workspace.to_owned(),
            },
        );
    }

    fn leave(&self, number: u32) {
        let left = self.chats.lock().unwrap().remove(&number);
        if let Some(chat) = left {
            self.owes(&chat.pool, &chat.workspace);
        }
    }

    fn forget(&self, number: u32) {
        self.chats.lock().unwrap().remove(&number);
    }

    fn fetched(&self, pool: &str, workspace: &str) {
        if let Some(workspaces) = self.owed.lock().unwrap().get_mut(pool) {
            workspaces.remove(workspace);
        }
    }

    fn chats(&self) -> BTreeMap<u32, purlis_core::machine::ChatOnPool> {
        self.chats.lock().unwrap().clone()
    }

    fn owed(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.owed.lock().unwrap().clone()
    }
}

/// A machine with two pools, whose store and hosts outlive each window started on it, and whose
/// setting moves: each `window` is a new router, as a restart makes.
struct AMachine {
    here: Pretend,
    builds: Pretend,
    tests: Pretend,
    made: Arc<Mutex<Vec<String>>>,
    store: Arc<TheStore>,
    setting: Arc<Mutex<BTreeMap<String, String>>>,
}

impl AMachine {
    /// `app` runs on pool `builds`.
    fn new() -> AMachine {
        let (builds, tests) = (Pretend::default(), Pretend::default());
        builds.on_a_runner();
        tests.on_a_runner();
        AMachine {
            here: Pretend::default(),
            builds,
            tests,
            made: Arc::default(),
            store: Arc::default(),
            setting: Arc::new(Mutex::new(BTreeMap::from([(
                "app".to_owned(),
                "builds".to_owned(),
            )]))),
        }
    }

    /// `app` runs on `pool` from now on, or on this machine.
    fn moves_app_to(&self, pool: Option<&str>) {
        let mut setting = self.setting.lock().unwrap();
        match pool {
            Some(pool) => setting.insert("app".to_owned(), pool.to_owned()),
            None => setting.remove("app"),
        };
    }

    fn window(&self) -> RoutedSessions {
        let runs_on: RunsOn = {
            let setting = Arc::clone(&self.setting);
            Arc::new(move |workspace| Ok(setting.lock().unwrap().get(workspace).cloned()))
        };
        let on_pools: OnPools = {
            let setting = Arc::clone(&self.setting);
            Arc::new(move || {
                let mut on: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for (workspace, pool) in setting.lock().unwrap().iter() {
                    on.entry(pool.clone()).or_default().push(workspace.clone());
                }
                on
            })
        };
        let to_a_pool: ToAPool = {
            let (builds, tests) = (self.builds.clone(), self.tests.clone());
            let made = Arc::clone(&self.made);
            Arc::new(move |pool| {
                made.lock().unwrap().push(pool.to_owned());
                match pool {
                    "builds" => Ok(Box::new(builds.clone()) as Box<dyn SessionHost>),
                    "tests" => Ok(Box::new(tests.clone()) as Box<dyn SessionHost>),
                    other => Err(format!("pool {other} is not kept")),
                }
            })
        };
        let store: Arc<dyn KeptPlaces> = self.store.clone();
        RoutedSessions::new(
            Path::new("/planes/a"),
            Box::new(self.here.clone()),
            runs_on,
            on_pools,
            to_a_pool,
        )
        .keeping_chats(ChatsKept::in_places(store))
    }

    /// A chat of `app` running on its pool, its window gone.
    fn a_chat_left_on_builds(&self) -> u32 {
        let window = self.window();
        let number = window
            .open_named(None, &named_in("app"), Default::default(), &|_| {})
            .expect("it starts on the pool")
            .number;
        window.chats_kept.settled();
        self.builds.leaves_running(number, None);
        number
    }

    fn pools_reached(&self) -> Vec<String> {
        self.made.lock().unwrap().clone()
    }

    fn kept(&self) -> Vec<u32> {
        self.store.chats().into_keys().collect()
    }

    fn owed_from(&self, pool: &str) -> Vec<String> {
        self.store
            .owed()
            .remove(pool)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }
}

#[test]
fn a_chat_is_taken_back_from_the_pool_it_ran_on_by_a_window_started_after_the_setting_moved() {
    for moved_to in [None, Some("tests")] {
        let machine = AMachine::new();
        let number = machine.a_chat_left_on_builds();
        machine.moves_app_to(moved_to);
        machine.made.lock().unwrap().clear();

        let back = machine.window();
        assert!(back.left_running_in(number, Some("app")));
        assert_eq!(back.pool_of(number).as_deref(), Some("builds"));
        assert_eq!(back.reattach(number, &|_| {}), Ok(number));
        assert_eq!(machine.builds.reattached(), vec![number]);
        assert_eq!(machine.pools_reached(), vec!["builds".to_owned()]);
        assert_eq!(machine.kept(), vec![number], "it runs there still");
    }
}

#[test]
fn a_kept_chat_is_asked_of_its_one_pool_and_not_of_the_one_its_workspace_moved_to() {
    let machine = AMachine::new();
    let number = machine.a_chat_left_on_builds();
    machine.builds.close_what_was_not_reattached();
    machine.moves_app_to(Some("tests"));
    machine.tests.leaves_running(number, None);
    machine.made.lock().unwrap().clear();

    let back = machine.window();
    assert!(!back.left_running_in(number, Some("app")));
    assert_eq!(machine.pools_reached(), vec!["builds".to_owned()]);
    assert_eq!(back.pool_of(number), None);
}

#[test]
fn a_kept_pool_is_asked_only_for_the_workspace_the_chat_ran_in() {
    let machine = AMachine::new();
    let number = machine.a_chat_left_on_builds();
    machine.moves_app_to(None);
    machine.made.lock().unwrap().clear();

    let back = machine.window();
    assert!(!back.left_running_in(number, Some("fleet")));
    assert!(!back.left_running_in(number, None));
    assert!(machine.pools_reached().is_empty(), "no pool was asked");
    assert_eq!(back.pool_of(number), None);
}

#[test]
fn a_chat_closed_on_a_pool_is_never_asked_for_again_and_its_branches_stay_owed() {
    let machine = AMachine::new();
    let window = machine.window();
    let number = window
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    window.close(number).expect("it closes");
    window.chats_kept.settled();
    assert!(machine.kept().is_empty());
    assert_eq!(machine.owed_from("builds"), vec!["app".to_owned()]);

    // A runner that says otherwise, with the setting moved, is not asked.
    machine.moves_app_to(None);
    machine.builds.leaves_running(number, None);
    machine.made.lock().unwrap().clear();
    assert!(!machine.window().left_running_in(number, Some("app")));
    assert!(machine.pools_reached().is_empty(), "no pool was asked");
}

#[test]
fn a_start_that_lands_here_takes_its_number_out_of_what_a_restart_asks() {
    let machine = AMachine::new();
    machine.store.keep(7, "builds", "app");
    machine.store.keep(8, "builds", "app");
    machine.store.keep(9, "builds", "app");
    machine.moves_app_to(None);
    machine.here.on_a_runner();
    let window = machine.window();

    window.open(Some(7), &Opening::default(), &|_| {}).unwrap();
    window
        .open_named(Some(8), &named_in("fleet"), Default::default(), &|_| {})
        .unwrap();
    machine.here.leaves_running(9, None);
    assert!(window.left_running_in(9, Some("app")));
    window.reattach(9, &|_| {}).unwrap();
    window.chats_kept.settled();

    assert!(machine.kept().is_empty());
    assert_eq!(
        machine.owed_from("builds"),
        vec!["app".to_owned()],
        "what ran there still comes back"
    );
    for number in [7, 8, 9] {
        assert_eq!(window.pool_of(number), None);
    }
}

#[test]
fn a_start_a_runner_refuses_keeps_no_record() {
    let machine = AMachine::new();
    let window = machine.window();
    machine.builds.refuses(Some("the runner is full"));
    window
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .map(|composed| composed.number)
        .expect_err("refused");
    window.chats_kept.settled();
    assert!(machine.kept().is_empty());
    assert!(
        machine.owed_from("builds").is_empty(),
        "nothing ran there to come back"
    );
}

#[test]
fn a_chat_is_kept_before_its_runner_starts_it_so_a_quick_end_leaves_no_record() {
    let machine = AMachine::new();
    let window = machine.window();
    window.when_one_ends(Arc::new(|_, _| {}));
    let number = window
        .open_named(None, &named_in("app"), Default::default(), &|announced| {
            window.chats_kept.settled();
            assert_eq!(machine.kept(), vec![announced]);
        })
        .unwrap()
        .number;
    machine.builds.program_ends(number, Exit::Code(0));
    window.chats_kept.settled();
    assert!(machine.kept().is_empty());
}

#[test]
fn a_chat_that_ends_comes_back_from_its_own_pool_alone_and_only_a_clean_fetch_settles_it() {
    let machine = AMachine::new();
    let window = machine.window();
    window.when_one_ends(Arc::new(|_, _| {}));
    let number = window
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    // Another workspace on the pool the setting moved to, and a chat of it.
    machine.moves_app_to(Some("tests"));
    let there = machine.window();
    let other = there
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    there.chats_kept.settled();

    machine.builds.fetch_fails_with(Some("the link went"));
    machine.builds.program_ends(number, Exit::Code(0));
    let wait_for = |fetches: usize| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while machine.builds.fetched().len() < fetches {
            assert!(std::time::Instant::now() < deadline, "never fetched");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _one_at_a_time = lock(&window.shared.fetching);
        window.chats_kept.settled();
    };
    wait_for(1);
    assert_eq!(
        machine.builds.fetched(),
        vec![("app".to_owned(), "builds".to_owned())],
        "from the pool it ran on, whatever the setting says now"
    );
    assert!(machine.tests.fetched().is_empty(), "from no other pool");
    assert_eq!(
        machine.owed_from("builds"),
        vec!["app".to_owned()],
        "a fetch that failed leaves it owed"
    );

    // The next one, as the window's focus asks for it, finds it owed and settles it.
    machine.builds.fetch_fails_with(None);
    let failed = fetch_back_all(
        &window.shared,
        &window.on_pools,
        &window.to_a_pool,
        &window.chats_kept,
    );
    assert_eq!(failed, Vec::<String>::new());
    window.chats_kept.settled();
    assert!(machine.owed_from("builds").is_empty());
    assert!(
        machine
            .builds
            .fetched()
            .iter()
            .filter(|one| one.1 == "builds")
            .count()
            >= 2
    );
    assert_eq!(machine.kept(), vec![other], "a chat running stays kept");
}

#[test]
fn a_chat_whose_link_went_stays_kept_for_the_next_window_to_ask() {
    let machine = AMachine::new();
    let window = machine.window();
    window.when_one_ends(Arc::new(|_, _| {}));
    let number = window
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    machine.builds.program_ends_with_its_host(number);
    window.chats_kept.settled();
    assert_eq!(machine.kept(), vec![number]);
}

#[test]
fn a_program_a_runner_says_ended_by_a_hangup_is_an_end_like_any_other() {
    let machine = AMachine::new();
    let window = machine.window();
    window.when_one_ends(Arc::new(|_, _| {}));
    let number = window
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    machine.builds.program_ends(
        number,
        Exit::Signal(crate::hostedsessions::ENDED_WITH_THE_HOST.to_owned()),
    );
    window.chats_kept.settled();
    assert!(machine.kept().is_empty());
}

#[test]
fn a_put_back_keeps_a_record_where_a_pools_draining_runners_cant_be_told() {
    let machine = AMachine::new();
    machine.store.keep(7, "builds", "app");
    machine.moves_app_to(None);
    let window =
        machine
            .window()
            .taking_back_from(Arc::new(|_: &str| -> Vec<Result<String, String>> {
                vec![Err("docker did not answer".to_owned())]
            }));
    window.close_what_was_not_reattached();
    window.chats_kept.settled();
    assert_eq!(machine.kept(), vec![7]);
}

#[test]
fn a_chat_whose_runner_went_leaves_its_record_and_fetches_nothing() {
    let machine = AMachine::new();
    let window = machine.window();
    window.when_one_ends(Arc::new(|_, _| {}));
    let number = window
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    machine.builds.program_ends(
        number,
        Exit::Signal(crate::hostedsessions::ITS_RUNNER_WENT.to_owned()),
    );
    window.chats_kept.settled();
    assert!(machine.kept().is_empty());
    assert!(machine.builds.fetched().is_empty());
}

#[test]
fn a_put_back_settles_a_record_its_pool_said_no_to_and_keeps_one_it_could_not_ask() {
    let machine = AMachine::new();
    machine.store.keep(7, "builds", "app");
    machine.moves_app_to(None);
    let window = machine.window();
    window.close_what_was_not_reattached();
    window.chats_kept.settled();
    assert!(machine.kept().is_empty(), "its pool says it isn't there");

    machine.store.keep(8, "builds", "app");
    machine.builds.cannot_say_what_it_left();
    let window = machine.window();
    window.close_what_was_not_reattached();
    window.chats_kept.settled();
    assert_eq!(machine.kept(), vec![8], "not while its answer is unknown");

    machine.store.keep(9, "gone", "app");
    let window = machine.window();
    window.close_what_was_not_reattached();
    window.chats_kept.settled();
    assert_eq!(machine.kept(), vec![8, 9], "a pool that can't be reached");
}

#[test]
fn a_put_back_closes_only_what_was_kept_on_a_pool_no_workspace_runs_on() {
    let machine = AMachine::new();
    let taken_back = machine.a_chat_left_on_builds();
    let no_tab = machine.a_chat_left_on_builds();
    let not_ours = 900;
    machine.builds.leaves_running(not_ours, None);
    machine.moves_app_to(None);

    let back = machine.window();
    assert!(back.left_running_in(taken_back, Some("app")));
    back.reattach(taken_back, &|_| {}).unwrap();
    back.close_what_was_not_reattached();
    back.chats_kept.settled();

    assert_eq!(machine.builds.closed_left(), vec![no_tab]);
    assert!(
        machine.builds.left_running(not_ours),
        "it isn't this window's"
    );
    assert_eq!(machine.kept(), vec![taken_back]);
}

#[test]
fn a_put_back_on_a_pool_a_workspace_runs_on_still_closes_all_it_did_not_take_back() {
    let machine = AMachine::new();
    let no_tab = machine.a_chat_left_on_builds();
    let unrecorded = 900;
    machine.builds.leaves_running(unrecorded, None);
    let back = machine.window();
    assert!(back.left_running_in(unrecorded, Some("app")));
    machine.builds.leaves_running(901, None);
    back.close_what_was_not_reattached();
    let closed: BTreeSet<u32> = machine.builds.closed_left().into_iter().collect();
    assert!(
        closed.contains(&no_tab) && closed.contains(&901),
        "{closed:?}"
    );
}

#[test]
fn a_chat_in_a_workspace_set_to_a_pool_starts_on_its_runner_and_every_command_follows_it() {
    let world = a_world();
    assert!(world.sessions.composes_its_harness_chats_in(Some("app")));
    assert!(!world.sessions.composes_its_harness_chats_in(Some("fleet")));
    assert!(
        world.made.lock().unwrap().is_empty(),
        "no runner is reached to say so"
    );

    let on_the_runner = world
        .sessions
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .expect("started on the runner")
        .number;
    assert_eq!(world.builds.named().len(), 1, "the runner composed it");
    assert!(world.here.named().is_empty());

    // Its view, its input, its size, its readiness and its stop go to the runner.
    world.sessions.watch(on_the_runner, a_sink()).unwrap();
    world.sessions.input(on_the_runner, b"go\r").unwrap();
    world
        .sessions
        .resize(
            on_the_runner,
            Size {
                columns: 100,
                rows: 30,
            },
        )
        .unwrap();
    world.sessions.readiness(on_the_runner).unwrap();
    assert_eq!(world.builds.typed(on_the_runner), vec![b"go\r".to_vec()]);
    assert!(world.here.typed(on_the_runner).is_empty());
    assert_eq!(world.sessions.running(), vec![on_the_runner]);
    world.sessions.close(on_the_runner).unwrap();
    assert!(world.builds.running().is_empty());

    // A shell in that workspace runs here, where the window composed it.
    let shell = world
        .sessions
        .open(None, &Opening::default(), &|_| {})
        .unwrap();
    assert_eq!(world.here.running(), vec![shell]);
    world.sessions.input(shell, b"ls\r").unwrap();
    assert_eq!(world.here.typed(shell), vec![b"ls\r".to_vec()]);
}

#[test]
fn chat_numbers_never_collide_between_this_machine_and_a_pool() {
    let world = a_world();
    let mut numbers = Vec::new();
    for workspace in ["app", "fleet", "app", "fleet", "app"] {
        let number = world
            .sessions
            .open_named(None, &named_in(workspace), Default::default(), &|_| {})
            .map(|composed| composed.number)
            .or_else(|_| {
                // This machine's pretend host composes nothing: a chat here starts from its
                // window's launch.
                world.sessions.open(None, &Opening::default(), &|_| {})
            })
            .unwrap();
        numbers.push(number);
    }
    let mut unique = numbers.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), numbers.len(), "{numbers:?}");
    // Each runner chat's number was dealt by this machine's host, which knows it went out.
    assert!(world.sessions.dealt() >= *numbers.iter().max().unwrap());
    // A number that went out is said to every host, a pool's made after it included.
    world.sessions.already_dealt(40);
    assert!(world.builds.dealt() >= 40);
    assert!(world.sessions.deal() > 40);
}

#[test]
fn a_workspace_naming_a_pool_this_machine_does_not_keep_starts_nothing_anywhere() {
    let world = a_world();
    let refused = world
        .sessions
        .open_named(None, &named_in("stale"), Default::default(), &|_| {})
        .map(|composed| composed.number)
        .expect_err("refused");
    assert!(refused.contains("is not kept"), "{refused}");
    assert!(world.here.named().is_empty() && world.builds.named().is_empty());
    assert!(world.here.running().is_empty() && world.builds.running().is_empty());
}

#[test]
fn a_runner_chat_left_running_before_this_window_is_taken_back_from_its_pool() {
    let world = a_world();
    world.builds.leaves_running(7, None);
    assert!(world.sessions.left_running_in(7, Some("app")));
    assert_eq!(world.sessions.reattach(7, &|_| {}).unwrap(), 7);
    assert_eq!(world.builds.reattached(), vec![7]);
    assert!(world.here.reattached().is_empty());
}

#[test]
fn a_pool_is_never_asked_for_a_chat_of_a_workspace_that_does_not_run_on_it() {
    let world = a_world();
    // The runner says it runs 7 and 8; neither worked in a workspace that runs on it.
    world.builds.leaves_running(7, None);
    world.builds.leaves_running(8, None);
    assert!(!world.sessions.left_running_in(7, Some("fleet")));
    assert!(!world.sessions.left_running_in(8, None));
    assert!(!world.sessions.left_running(7));
    assert!(world.made.lock().unwrap().is_empty(), "no runner was asked");
    world.sessions.input(7, b"x").unwrap_or_default();
    assert!(
        world.builds.typed(7).is_empty(),
        "7 is not routed to the pool"
    );
}

#[test]
fn the_kill_switch_and_every_end_reach_every_host() {
    let mut world = a_world();
    let switch = crate::killswitch::KillSwitch::unthrown();
    world.sessions.stopped_by(Arc::clone(&switch));
    let ended: Arc<Mutex<Vec<u32>>> = Arc::default();
    world.sessions.when_one_ends({
        let ended = Arc::clone(&ended);
        Arc::new(move |id, _| ended.lock().unwrap().push(id))
    });
    let there = world
        .sessions
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    let here = world
        .sessions
        .open(None, &Opening::default(), &|_| {})
        .unwrap();
    assert_eq!(world.sessions.stop_every_program(), 2);
    world.builds.program_ends(there, Exit::Code(0));
    world.here.program_ends(here, Exit::Code(0));
    assert_eq!(*ended.lock().unwrap(), vec![there, here]);
}

#[test]
fn a_runner_chats_pool_is_known_before_it_starts_and_forgotten_where_its_start_fails() {
    let world = a_world();
    let routes = routes_of(&world.sessions.shared);
    let seen: Mutex<Vec<Option<String>>> = Mutex::default();
    let number = world
        .sessions
        .open_named(None, &named_in("app"), Default::default(), &|announced| {
            seen.lock().unwrap().push(routes(announced));
        })
        .unwrap()
        .number;
    assert_eq!(*seen.lock().unwrap(), vec![Some("builds".to_owned())]);
    assert_eq!(routes(number).as_deref(), Some("builds"));

    world.builds.refuses(Some("the runner is full"));
    let refused = world
        .sessions
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .map(|composed| composed.number)
        .expect_err("refused");
    assert!(refused.contains("full"), "{refused}");
    assert_eq!(
        lock(&world.sessions.shared.chats).len(),
        1,
        "a start that failed runs nowhere"
    );
}

#[test]
fn runner_chats_that_end_together_fetch_their_branches_once_at_a_time() {
    let world = a_world();
    world.sessions.when_one_ends(Arc::new(|_, _| {}));
    let numbers: Vec<u32> = (0..4)
        .map(|_| {
            world
                .sessions
                .open_named(None, &named_in("app"), Default::default(), &|_| {})
                .unwrap()
                .number
        })
        .collect();
    for number in &numbers {
        world.builds.program_ends(*number, Exit::Code(0));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while world.builds.fetched().is_empty() || !lock(&world.sessions.shared.ending).is_empty() {
        assert!(std::time::Instant::now() < deadline, "never fetched");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    let fetched = world.builds.fetched();
    assert!(
        (1..=2).contains(&fetched.len()),
        "four ends fetched {} times",
        fetched.len()
    );
    assert!(
        fetched
            .iter()
            .all(|one| *one == ("app".to_owned(), "builds".to_owned()))
    );
    assert_eq!(world.builds.most_fetching_at_once(), 1);
}

#[test]
fn a_chat_on_a_pool_is_named_by_its_pool_and_one_here_by_none() {
    let world = a_world();
    let there = world
        .sessions
        .open_named(None, &named_in("app"), Default::default(), &|_| {})
        .unwrap()
        .number;
    let here = world
        .sessions
        .open(None, &Opening::default(), &|_| {})
        .unwrap();
    // What the window holds a chat's writes to the project for the person by (#2046).
    assert_eq!(world.sessions.pool_of(there).as_deref(), Some("builds"));
    assert_eq!(world.sessions.pool_of(here), None);
    world.sessions.close(there).unwrap();
    assert_eq!(world.sessions.pool_of(there), None);
}

/// #2025: the browsers this machine's host kept from before this window reach the put back
/// through the routing, which every app's project is held through: none was taken back before.
#[test]
fn the_browsers_left_on_this_machines_host_reach_the_put_back_through_the_routing() {
    let world = a_world();
    world
        .here
        .leaves_a_browser(purlis_session_protocol::session::HeldProgram {
            program: "01J9BRWSR000000000000000000".into(),
            kind: purlis_session_protocol::session::ProgramKind::Browser,
            number: 0,
            process: Some(4343),
            run: None,
            exit: None,
            confined: None,
            plan: None,
            sandbox: None,
            cols: 1280,
            rows: 800,
            driver: None,
            note: None,
            logins: Vec::new(),
            prompt_key: None,
        });

    let left = world.sessions.browsers_left();

    assert_eq!(left.len(), 1);
    assert_eq!(left[0].program, "01J9BRWSR000000000000000000");
    assert!(world.sessions.browsers_left().is_empty(), "each once");
}

/// Every method of the window's chats' host that has a default is one the routing says, so no
/// answer of this machine's host is lost to the default on the way (#2025: `browsers_left` was).
#[test]
fn the_routing_says_every_method_of_a_chats_host() {
    let trait_source = include_str!("../host.rs");
    let routed_source = include_str!("../routedsessions.rs");
    let methods = |source: &str, opening: &str| -> Vec<String> {
        let at = source.find(opening).expect("the block");
        let block = &source[at..];
        let end = block.find("\n}\n").expect("its end");
        block[..end]
            .lines()
            .filter_map(|line| line.strip_prefix("    fn "))
            .filter_map(|rest| rest.split(['(', '<']).next())
            .map(str::to_owned)
            .collect()
    };
    let every = methods(trait_source, "pub trait SessionHost");
    let said = methods(routed_source, "impl SessionHost for RoutedSessions");
    // `fetch_back` and `fetch_back_left` are the routing's own, through its saver
    // (`FetchingBack`).
    let unsaid: Vec<&String> = every
        .iter()
        .filter(|one| {
            !said.contains(one) && !matches!(one.as_str(), "fetch_back" | "fetch_back_left")
        })
        .collect();
    assert!(unsaid.is_empty(), "the routing does not say {unsaid:?}");
}

#[test]
fn a_move_is_asked_of_the_pool_it_goes_to_or_comes_from_and_never_of_this_machine() {
    let world = a_world();
    let chat = crate::host::Moving {
        workspace: "fleet".into(),
        profile: "claude".into(),
        conversation: "0d6f3c1e".into(),
    };
    world.sessions.move_ahead("builds", "fleet").unwrap();
    world
        .sessions
        .move_in("builds", std::slice::from_ref(&chat))
        .unwrap();
    world
        .sessions
        .move_out("builds", std::slice::from_ref(&chat))
        .unwrap();
    assert_eq!(
        world.builds.moved(),
        vec![
            ("ahead", "builds".to_owned(), "fleet".to_owned()),
            ("in", "builds".to_owned(), "fleet".to_owned()),
            ("out", "builds".to_owned(), "fleet".to_owned()),
        ]
    );
    assert!(world.here.moved().is_empty());
    assert!(
        world
            .sessions
            .move_in("gone", std::slice::from_ref(&chat))
            .is_err(),
        "a pool this machine does not keep moves nothing"
    );
}

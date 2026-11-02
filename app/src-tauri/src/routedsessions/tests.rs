// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's chats routed by workspace (#2046): this machine's host and a pool's runner, each
//! a host that runs nothing, so where each chat and each command goes is what is seen.

use std::collections::BTreeMap;
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

const FRESH: &str = "builds/fresh-0123456789abcdef";

/// A world with a chat's fresh runner, reached by [`FRESH`], beside pool `builds`.
fn a_world_with_a_fresh_runner() -> (World, Pretend) {
    let mut world = a_world();
    let fresh = Pretend::default();
    fresh.on_a_runner();
    let to_a_pool: ToAPool = {
        let (builds, fresh) = (world.builds.clone(), fresh.clone());
        Arc::new(move |pool| match pool {
            "builds" => Ok(Box::new(builds.clone()) as Box<dyn SessionHost>),
            FRESH => Ok(Box::new(fresh.clone()) as Box<dyn SessionHost>),
            other => Err(format!("pool {other} is not kept")),
        })
    };
    world.sessions = RoutedSessions::new(
        Path::new("/planes/a"),
        Box::new(world.here.clone()),
        runs_on(),
        on_pools(),
        to_a_pool,
    );
    (world, fresh)
}

/// A chat left running on its fresh runner is its pool's for what asks, and every command after
/// reaches that runner (#4569).
#[test]
fn a_chat_left_running_on_its_own_runner_is_taken_back_from_there() {
    let (world, fresh) = a_world_with_a_fresh_runner();
    fresh.leaves_running(7, None);
    assert!(
        !world.sessions.left_running_in(7, Some("app")),
        "the pool's own runner does not have it"
    );
    assert!(world.sessions.left_running_on(7, FRESH));
    assert_eq!(world.sessions.reattach(7, &|_| {}).unwrap(), 7);
    assert_eq!(fresh.reattached(), vec![7]);
    assert_eq!(world.sessions.pool_of(7).as_deref(), Some("builds"));
    assert!(!world.sessions.left_running_on(8, FRESH));
}

/// Waits for what a fresh runner's end is told, by the thread it runs on.
fn told_of_its_end(sessions: &RoutedSessions, goes: bool) -> Vec<String> {
    let (tell, told) = std::sync::mpsc::channel();
    sessions.take_back_fresh(
        FRESH,
        Some("app"),
        Box::new(move |failed| {
            tell.send(failed).expect("the test waits");
            goes
        }),
    );
    let failed = told
        .recv_timeout(Duration::from_secs(30))
        .expect("the end is told");
    // The host is let go of on that thread, after it is told.
    std::thread::sleep(Duration::from_millis(200));
    failed
}

/// **A fresh runner's work is fetched first, and what ends it hears whether it came** (#4569), and
/// the way to a runner that went goes with it.
#[test]
fn a_fresh_runners_work_comes_back_before_it_goes_and_its_host_with_it() {
    let (world, fresh) = a_world_with_a_fresh_runner();
    assert_eq!(told_of_its_end(&world.sessions, true), Vec::<String>::new());
    assert_eq!(
        fresh.fetched(),
        vec![("app".to_owned(), FRESH.to_owned())],
        "from that runner, by its name"
    );
    assert_eq!(fresh.let_go(), 1, "its link goes with it");
    assert!(!lock(&world.sessions.shared.pools).contains_key(FRESH));
}

/// A runner nothing ran on is asked nothing on its way out (#4569): a refused start's runner goes
/// though the link that would fetch from it is what failed.
#[test]
fn a_runner_nothing_ran_on_goes_with_nothing_fetched() {
    let (world, fresh) = a_world_with_a_fresh_runner();
    fresh.fetch_fails("the link went");
    let (tell, told) = std::sync::mpsc::channel();
    world.sessions.take_back_fresh(
        FRESH,
        None,
        Box::new(move |failed| {
            tell.send(failed).expect("the test waits");
            true
        }),
    );
    let failed: Vec<String> = told
        .recv_timeout(Duration::from_secs(30))
        .expect("the end is told");
    assert_eq!(failed, Vec::<String>::new());
    assert!(fresh.fetched().is_empty());
}

/// A runner whose work did not come keeps its host, and is said to have failed: what ends it holds
/// on to the claim.
#[test]
fn a_fresh_runner_whose_work_did_not_come_is_told_so_and_stays() {
    let (world, fresh) = a_world_with_a_fresh_runner();
    fresh.fetch_fails("the link went");
    assert_eq!(
        told_of_its_end(&world.sessions, false),
        vec!["the link went".to_owned()]
    );
    assert_eq!(fresh.let_go(), 0);
    assert!(lock(&world.sessions.shared.pools).contains_key(FRESH));
}

/// A chat with a runner of its own stays on it whichever pool, if any, its workspace runs on now.
#[test]
fn a_chat_with_a_runner_of_its_own_starts_there_though_its_workspace_moved() {
    let (world, fresh) = a_world_with_a_fresh_runner();
    let named = Named {
        runner: Some(FRESH.to_owned()),
        ..named_in("runs-here-now")
    };
    let composed = world
        .sessions
        .open_named(
            None,
            &named,
            purlis_core::sandbox::plan::Hearing::default(),
            &|_| {},
        )
        .expect("it starts");
    assert!(fresh.running().contains(&composed.number));
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
    // `fetch_back` is the routing's own, through its saver (`FetchingBack`), and the two a fresh
    // runner's end asks are its own host's.
    let its_own = ["fetch_back", "fetch_back_whole", "let_go_of_runner"];
    let unsaid: Vec<&String> = every
        .iter()
        .filter(|one| !said.contains(one) && !its_own.contains(&one.as_str()))
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

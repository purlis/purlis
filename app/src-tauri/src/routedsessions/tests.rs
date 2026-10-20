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
    assert!(world.sessions.left_running(7));
    assert_eq!(world.sessions.reattach(7, &|_| {}).unwrap(), 7);
    assert_eq!(world.builds.reattached(), vec![7]);
    assert!(world.here.reattached().is_empty());
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

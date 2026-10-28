// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A move's stages: prepared in the background, switched only at a pause, and a failed one
//! leaving the chat where it was.

use std::sync::mpsc;
use std::time::Duration;

use super::*;

/// Moves whose every stage is heard on the channel answered.
fn heard() -> (Arc<Moves>, mpsc::Receiver<ChatMove>) {
    let moves = Arc::new(Moves::default());
    let (tell, told) = mpsc::channel();
    let tell = Mutex::new(tell);
    moves.telling(Arc::new(move |one| {
        let _ = lock(&tell).send(one);
    }));
    (moves, told)
}

fn next(told: &mpsc::Receiver<ChatMove>) -> ChatMove {
    told.recv_timeout(Duration::from_secs(5))
        .expect("a stage was told")
}

#[test]
fn a_prepared_move_switches_at_a_pause_and_never_mid_turn() {
    let (moves, told) = heard();
    moves
        .begin(3, None, Some("builds".into()), || Ok(()))
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "ready");

    assert_eq!(
        moves.due(3, false),
        Due::NotYet(NOT_AT_A_PAUSE.to_owned()),
        "mid-turn, it waits"
    );
    assert_eq!(
        moves.due(3, true),
        Due::Now(Move {
            from: None,
            to: Some("builds".into()),
            stage: Stage::Ready,
        })
    );
    moves.moved(3);
    assert_eq!(moves.due(3, true), Due::NotMoving);
}

#[test]
fn a_move_still_being_prepared_waits_even_at_a_pause() {
    let (moves, told) = heard();
    let (go, gate) = mpsc::channel::<()>();
    moves
        .begin(4, None, Some("builds".into()), move || {
            let _ = gate.recv();
            Ok(())
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert!(matches!(moves.due(4, true), Due::NotYet(_)));
    go.send(()).unwrap();
    assert_eq!(next(&told).stage, "ready");
}

#[test]
fn a_failed_preparation_never_switches_and_says_why_until_put_away() {
    let (moves, told) = heard();
    moves
        .begin(5, None, Some("builds".into()), || {
            Err("the runner of pool builds did not answer".into())
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    let failed = next(&told);
    assert_eq!(
        (failed.stage.as_str(), failed.why.as_deref()),
        ("failed", Some("the runner of pool builds did not answer"))
    );
    assert_eq!(
        moves.due(5, true),
        Due::NotMoving,
        "the chat stays where it is"
    );
    assert_eq!(moves.shown(), vec![failed]);

    moves.put_away(5);
    assert!(moves.shown().is_empty());
}

#[test]
fn a_chat_moves_once_at_a_time_and_again_after_a_failure() {
    let (moves, told) = heard();
    moves.begin(6, None, Some("a".into()), || Ok(())).unwrap();
    assert!(moves.begin(6, None, Some("b".into()), || Ok(())).is_err());
    let _ = (next(&told), next(&told));
    moves.failed(6, "the switch was refused".into());
    assert_eq!(next(&told).stage, "failed");
    moves.begin(6, None, Some("b".into()), || Ok(())).unwrap();
}

#[test]
fn a_move_put_away_while_it_was_prepared_is_not_brought_back_by_its_preparation() {
    let (moves, told) = heard();
    let (go, gate) = mpsc::channel::<()>();
    moves
        .begin(7, None, Some("builds".into()), move || {
            let _ = gate.recv();
            Ok(())
        })
        .unwrap();
    let _ = next(&told);
    moves.put_away(7);
    go.send(()).unwrap();
    assert!(told.recv_timeout(Duration::from_millis(300)).is_err());
    assert_eq!(moves.due(7, true), Due::NotMoving);
}

fn bound() -> purlis_core::poollogins::Bound {
    purlis_core::poollogins::Bound {
        pool: "builds".into(),
        definition: "d".into(),
        image: "i".into(),
        runner: "r".into(),
        runner_key: "k".into(),
        runner_image: "ri".into(),
        engine: "this machine's Docker".into(),
    }
}

#[test]
fn work_comes_back_only_from_a_runner_shown_to_be_the_persons_own_and_says_why_not() {
    assert_eq!(brings_work_back("builds", || Ok(bound())), Ok(()));
    let refused = brings_work_back("far", || {
        Err("Docker's engine here is tcp://10.0.0.4:2375, not a socket on this machine".into())
    })
    .expect_err("a pool not shown to be yours brings no work back");
    assert!(refused.contains("pool far"), "{refused}");
    assert!(refused.contains("not known to be yours"), "{refused}");
}

#[test]
fn a_tab_names_what_came_back_that_a_local_tool_runs_and_nothing_when_none_did() {
    assert_eq!(came_back_said(Some("builds"), &[]), None);
    let said = came_back_said(
        Some("builds"),
        &[
            "workspaces/app/web/.envrc".into(),
            "workspaces/app/web/package.json".into(),
        ],
    )
    .unwrap();
    assert!(said.contains("pool builds"), "{said}");
    assert!(
        said.contains("workspaces/app/web/.envrc, workspaces/app/web/package.json"),
        "{said}"
    );
}

fn moving() -> crate::host::Moving {
    crate::host::Moving {
        workspace: "app".into(),
        profile: "work".into(),
        conversation: "0d6f3c1e".into(),
    }
}

/// What a move asked of its profile on a pool, and what it answers (#2049).
#[derive(Default)]
struct ProfilesOnAPool {
    asked: Mutex<Vec<(std::path::PathBuf, String, String)>>,
    refusing: Mutex<Option<String>>,
}

impl crate::poolprofiles::ProfilesOnPools for ProfilesOnAPool {
    fn ready(&self, _: &std::path::Path, _: &str, _: &str) -> Result<(), String> {
        Err("asked as a start, where a move was meant".into())
    }

    fn ready_to_move(
        &self,
        root: &std::path::Path,
        pool: &str,
        profile: &str,
    ) -> Result<(), String> {
        lock(&self.asked).push((root.to_path_buf(), pool.to_owned(), profile.to_owned()));
        match lock(&self.refusing).clone() {
            Some(why) => Err(why),
            None => Ok(()),
        }
    }
}

const PROJECT: &str = "/planes/a";

/// Chats on a host that runs nothing, readying their profiles on pools through `on_pools`.
fn chats_readying(
    on_pools: &Arc<ProfilesOnAPool>,
) -> (crate::chats::Chats, crate::host::pretend::Pretend) {
    let host = crate::host::pretend::Pretend::default();
    let mut chats = crate::chats::Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        std::path::PathBuf::from(PROJECT),
    );
    chats.readying_profiles_on_pools(Arc::clone(on_pools) as _);
    (chats, host)
}

#[test]
fn a_move_keeps_its_profile_on_the_runner_before_its_work_and_conversation_go() {
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let (chats, host) = chats_readying(&on_pools);

    move_to_a_runner(&chats, "builds", &moving()).unwrap();
    assert_eq!(
        *lock(&on_pools.asked),
        vec![(PROJECT.into(), "builds".to_owned(), "work".to_owned())]
    );
    assert_eq!(
        host.moved(),
        vec![("in", "builds".to_owned(), "app".to_owned())]
    );
}

#[test]
fn a_profile_the_runner_does_not_keep_moves_nothing_there() {
    let on_pools = Arc::new(ProfilesOnAPool::default());
    *lock(&on_pools.refusing) = Some("the profile 'work' is not signed in on pool builds".into());
    let (chats, host) = chats_readying(&on_pools);

    assert!(move_to_a_runner(&chats, "builds", &moving()).is_err());
    assert!(
        host.moved().is_empty(),
        "nothing moved before the profile was kept"
    );
}

#[test]
fn a_moves_preparation_keeps_its_profile_before_it_sends_anything_ahead() {
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let (chats, host) = chats_readying(&on_pools);
    prepare_on_a_runner(&chats, "builds", "app", "work").unwrap();
    assert_eq!(lock(&on_pools.asked).len(), 1);
    assert_eq!(
        host.moved(),
        vec![("ahead", "builds".to_owned(), "app".to_owned())]
    );

    let refusing = Arc::new(ProfilesOnAPool::default());
    *lock(&refusing.refusing) = Some("pool builds's runner did not keep the profile 'work'".into());
    let (chats, host) = chats_readying(&refusing);
    assert!(prepare_on_a_runner(&chats, "builds", "app", "work").is_err());
    assert!(host.moved().is_empty(), "nothing went ahead");
}

#[test]
fn ready_profile_on_asks_the_pools_readiness_for_this_project_and_nothing_without_one() {
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let (chats, _) = chats_readying(&on_pools);
    chats.ready_profile_on("builds", "work").unwrap();
    assert_eq!(
        *lock(&on_pools.asked),
        vec![(PROJECT.into(), "builds".to_owned(), "work".to_owned())]
    );

    let bare = crate::chats::Chats::on_host(
        Box::new(|_| {}),
        Box::new(crate::host::pretend::Pretend::default()),
        std::path::PathBuf::from(PROJECT),
    );
    assert_eq!(bare.ready_profile_on("builds", "work"), Ok(()));
}

#[test]
fn a_chat_moves_only_to_a_runner_shown_to_be_the_persons_own_and_says_why_not() {
    assert_eq!(goes_to_a_runner_of_yours("builds", || Ok(bound())), Ok(()));
    let refused = goes_to_a_runner_of_yours("far", || {
        Err("Docker's engine here is tcp://10.0.0.4:2375, not a socket on this machine".into())
    })
    .expect_err("a pool not shown to be yours is moved to by no chat");
    assert!(refused.contains("pool far"), "{refused}");
    assert!(refused.contains("not known to be yours"), "{refused}");
}

#[test]
fn a_move_that_did_not_start_and_could_not_set_its_workspace_back_says_where_the_chat_runs() {
    let to: Place = Some("builds".into());
    let put_back = did_not_start_said("the runner refused it.", Ok(()), &None, &to, "app");
    assert!(
        put_back.contains("runs on this machine as before"),
        "{put_back}"
    );
    assert!(!put_back.contains("Runs on"), "{put_back}");

    let stuck = did_not_start_said(
        "the runner refused it.",
        Err("machine.json could not be written".into()),
        &None,
        &to,
        "app",
    );
    assert!(stuck.contains("runs on this machine as before"), "{stuck}");
    assert!(stuck.contains("still set to run on pool builds"), "{stuck}");
    assert!(stuck.contains("set Runs on to this machine"), "{stuck}");
}

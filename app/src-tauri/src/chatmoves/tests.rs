// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A move's stages: prepared in the background, switched only at a pause, and a failed one
//! leaving the chat where it was.

use std::sync::mpsc;
use std::time::Duration;

use super::*;
use purlis_core::poollogins::Unbound;
use purlis_core::profilestore::onrunner::{NotYet, SignInAsked};

use crate::host::MadeHere;

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

/// Chat `session` alone in its workspace, named `chat <session>`.
fn alone(session: u32) -> Vec<Member> {
    vec![member(session)]
}

fn member(session: u32) -> Member {
    Member {
        session,
        name: format!("chat {session}"),
    }
}

fn next(told: &mpsc::Receiver<ChatMove>) -> ChatMove {
    told.recv_timeout(Duration::from_secs(5))
        .expect("a stage was told")
}

#[test]
fn a_prepared_move_switches_at_a_pause_and_never_mid_turn() {
    let (moves, told) = heard();
    moves
        .begin(alone(3), None, Some("builds".into()), || Ok(()))
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "ready");

    assert_eq!(
        moves.due(3, |_| false),
        Due::NotYet(NOT_AT_A_PAUSE.to_owned()),
        "mid-turn, it waits"
    );
    assert_eq!(
        moves.due(3, |_| true),
        Due::Now(Move {
            from: None,
            to: Some("builds".into()),
            stage: Stage::Switching,
            group: alone(3),
        })
    );
    moves.moved(3);
    assert_eq!(moves.due(3, |_| true), Due::NotMoving);
}

#[test]
fn a_move_still_being_prepared_waits_even_at_a_pause() {
    let (moves, told) = heard();
    let (go, gate) = mpsc::channel::<()>();
    moves
        .begin(alone(4), None, Some("builds".into()), move || {
            let _ = gate.recv();
            Ok(())
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert!(matches!(moves.due(4, |_| true), Due::NotYet(_)));
    go.send(()).unwrap();
    assert_eq!(next(&told).stage, "ready");
}

#[test]
fn a_failed_preparation_never_switches_and_says_why_until_put_away() {
    let (moves, told) = heard();
    moves
        .begin(alone(5), None, Some("builds".into()), || {
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
        moves.due(5, |_| true),
        Due::NotMoving,
        "the chat stays where it is"
    );
    assert_eq!(moves.shown(), vec![failed]);

    moves.put_away(5).unwrap();
    assert!(moves.shown().is_empty());
}

#[test]
fn a_chat_moves_once_at_a_time_and_again_after_a_failure() {
    let (moves, told) = heard();
    moves
        .begin(alone(6), None, Some("a".into()), || Ok(()))
        .unwrap();
    assert!(
        moves
            .begin(alone(6), None, Some("b".into()), || Ok(()))
            .is_err()
    );
    let _ = (next(&told), next(&told));
    moves.failed(6, "the switch was refused".into());
    assert_eq!(next(&told).stage, "failed");
    moves
        .begin(alone(6), None, Some("b".into()), || Ok(()))
        .unwrap();
}

#[test]
fn a_move_put_away_while_it_was_prepared_is_not_brought_back_by_its_preparation() {
    let (moves, told) = heard();
    let (go, gate) = mpsc::channel::<()>();
    moves
        .begin(alone(7), None, Some("builds".into()), move || {
            let _ = gate.recv();
            Ok(())
        })
        .unwrap();
    let _ = next(&told);
    moves.put_away(7).unwrap();
    assert_eq!(next(&told).stage, "over");
    go.send(()).unwrap();
    assert!(told.recv_timeout(Duration::from_millis(300)).is_err());
    assert_eq!(moves.due(7, |_| true), Due::NotMoving);
}

fn bound() -> purlis_core::poollogins::Bound {
    purlis_core::poollogins::Bound {
        pool: "builds".into(),
        definition: "d".into(),
        image: "i".into(),
        runner: "r".into(),
        runner_key: "k".into(),
        runner_image: "ri".into(),
        pod: String::new(),
        engine: "this machine's Docker".into(),
    }
}

#[test]
fn work_comes_back_only_from_a_runner_shown_to_be_the_persons_own_and_says_why_not() {
    assert_eq!(brings_work_back("builds", || Ok(bound())), Ok(()));
    let refused = brings_work_back("far", || {
        Err(Unbound::NotYours(
            "Docker's engine here is tcp://10.0.0.4:2375, not a socket on this machine".into(),
        ))
    })
    .expect_err("a pool not shown to be yours brings no work back");
    assert!(refused.contains("pool far"), "{refused}");
    assert!(refused.contains("not known to be yours"), "{refused}");
}

#[test]
fn a_tab_names_what_came_back_that_a_local_tool_runs_and_nothing_when_none_did() {
    assert_eq!(came_back_said(Some("builds"), &CameBack::default()), None);
    let said = came_back_said(
        Some("builds"),
        &CameBack {
            watch: vec![
                "workspaces/app/web/.envrc".into(),
                "workspaces/app/web/package.json".into(),
            ],
            made: Vec::new(),
        },
    )
    .unwrap();
    assert!(said.contains("pool builds"), "{said}");
    assert!(
        said.contains("workspaces/app/web/.envrc, workspaces/app/web/package.json"),
        "{said}"
    );
    assert!(!said.contains("made there"), "{said}");
}

/// A clone the runner's chat made is said to be whole from the runner (#3990): every file of
/// it was made there, not only the ones named.
#[test]
fn a_tab_says_a_clone_made_fresh_is_whole_from_the_runner() {
    let said = came_back_said(
        Some("builds"),
        &CameBack {
            watch: vec!["workspaces/app/api/CLAUDE.md".into()],
            made: vec![MadeHere {
                clone: "workspaces/app/api".into(),
                remote: Ok("https://github.com/owner/api.git".into()),
            }],
        },
    )
    .unwrap();
    assert!(
        said.contains("workspaces/app/api was cloned on pool builds"),
        "{said}"
    );
    assert!(said.contains("all of it was made there"), "{said}");
    assert!(said.contains("workspaces/app/api/CLAUDE.md"), "{said}");
}

/// The runner's remote is offered with the command that adds it, so the person gives it, and
/// one with none says why and how to give it one (#3985).
#[test]
fn a_tab_offers_the_runners_remote_for_a_fresh_clone_or_says_why_none_is() {
    let said = came_back_said(
        None,
        &CameBack {
            watch: Vec::new(),
            made: vec![
                MadeHere {
                    clone: "workspaces/app/api".into(),
                    remote: Ok("git@github.com:owner/api.git".into()),
                },
                MadeHere {
                    clone: "workspaces/app/web".into(),
                    remote: Err("the runner named no origin a forge reaches for it".into()),
                },
            ],
        },
    )
    .unwrap();
    assert!(
        said.contains("run `git remote add origin 'git@github.com:owner/api.git'` in it"),
        "{said}"
    );
    assert!(
        said.contains("workspaces/app/web was cloned on a runner"),
        "{said}"
    );
    assert!(
        said.contains("It has no remote here (the runner named no origin a forge reaches for it)"),
        "{said}"
    );
    assert!(
        said.contains("`git remote add origin <URL>` in it"),
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
    refusing: Mutex<Option<NotYet>>,
}

impl crate::poolprofiles::ProfilesOnPools for ProfilesOnAPool {
    fn ready(&self, _: &std::path::Path, _: &str, _: &str) -> Result<Option<String>, String> {
        Err("asked as a start, where a move was meant".into())
    }

    fn ready_to_move(
        &self,
        root: &std::path::Path,
        pool: &str,
        profile: &str,
    ) -> Result<(), NotYet> {
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

    move_to_a_runner(&chats, "builds", &[moving()]).unwrap();
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
    *lock(&on_pools.refusing) = Some(NotYet::SignIn(work_on_builds()));
    let (chats, host) = chats_readying(&on_pools);

    let refused = move_to_a_runner(&chats, "builds", &[moving()]).unwrap_err();
    assert!(
        refused.contains("not signed in on pool builds"),
        "{refused}"
    );
    assert!(
        host.moved().is_empty(),
        "nothing moved before the profile was kept"
    );
}

#[test]
fn a_moves_preparation_keeps_its_profile_before_it_sends_anything_ahead() {
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let (chats, host) = chats_readying(&on_pools);
    prepare_on_a_runner(&chats, "builds", "app", &["work".to_owned()]).unwrap();
    assert_eq!(lock(&on_pools.asked).len(), 1);
    assert_eq!(
        host.moved(),
        vec![("ahead", "builds".to_owned(), "app".to_owned())]
    );

    let refusing = Arc::new(ProfilesOnAPool::default());
    *lock(&refusing.refusing) = Some("pool builds's runner did not keep the profile 'work'".into());
    let (chats, host) = chats_readying(&refusing);
    assert!(prepare_on_a_runner(&chats, "builds", "app", &["work".to_owned()]).is_err());
    assert!(host.moved().is_empty(), "nothing went ahead");

    // A profile signed out there is the sign-in the move waits on, and nothing goes ahead.
    let signed_out = Arc::new(ProfilesOnAPool::default());
    *lock(&signed_out.refusing) = Some(NotYet::SignIn(work_on_builds()));
    let (chats, host) = chats_readying(&signed_out);
    assert_eq!(
        prepare_on_a_runner(&chats, "builds", "app", &["work".to_owned()]),
        Err(NotYet::SignIn(work_on_builds()))
    );
    assert!(host.moved().is_empty(), "nothing went ahead");
}

fn work_on_builds() -> SignInAsked {
    SignInAsked {
        pool: "builds".into(),
        profile: "work".into(),
        kind: "claude".into(),
        harness: "Claude Code".into(),
    }
}

/// **A move whose profile is not signed in on the pool waits for it** (#3477): it says the
/// sign-in it waits on, never switches meanwhile, and once the window says it signed in, it is
/// prepared again and goes ahead by itself.
#[test]
fn a_move_waits_for_its_profiles_sign_in_and_goes_ahead_once_signed_in() {
    let (moves, told) = heard();
    let tries = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counted = Arc::clone(&tries);
    moves
        .begin(alone(3), None, Some("builds".into()), move || {
            if counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                Err(NotYet::SignIn(work_on_builds()))
            } else {
                Ok(())
            }
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    let waits = next(&told);
    assert_eq!(waits.stage, "signing-in");
    assert_eq!(
        waits.sign_in,
        Some(MoveSignIn {
            profile: "work".into(),
            kind: "claude".into(),
            harness: "Claude Code".into(),
        })
    );
    assert!(matches!(moves.due(3, |_| true), Due::NotYet(_)), "it waits");
    assert_eq!(moves.shown()[0].stage, "signing-in");

    moves.signed_in(3).unwrap();
    assert_eq!(next(&told).stage, "preparing");
    let ready = next(&told);
    assert_eq!(ready.stage, "ready");
    assert_eq!(ready.sign_in, None);
    assert_eq!(tries.load(std::sync::atomic::Ordering::SeqCst), 2);
}

/// A move still signed out when asked again waits again; one not waiting on a sign-in is not
/// prepared again by a stray "signed in" (#3477).
#[test]
fn signed_in_only_prepares_again_a_move_that_waits_on_a_sign_in() {
    let (moves, told) = heard();
    moves
        .begin(alone(3), None, Some("builds".into()), || {
            Err(NotYet::SignIn(work_on_builds()))
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "signing-in");
    moves.signed_in(3).unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "signing-in");

    let (moves, told) = heard();
    moves
        .begin(alone(4), None, Some("builds".into()), || Ok(()))
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "ready");
    assert!(moves.signed_in(4).is_err());
    assert!(moves.signed_in(9).is_err(), "no move at all");
    assert!(told.recv_timeout(Duration::from_millis(200)).is_err());
}

/// **Cancel while it waits on a sign-in cancels the move, and the chat stays here** (#3477).
#[test]
fn cancelling_a_move_that_waits_on_a_sign_in_keeps_the_chat_where_it_is() {
    let (moves, told) = heard();
    moves
        .begin(alone(3), None, Some("builds".into()), || {
            Err(NotYet::SignIn(work_on_builds()))
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "signing-in");
    moves.put_away(3).unwrap();
    assert_eq!(next(&told).stage, "over");
    assert!(moves.shown().is_empty());
    assert!(moves.signed_in(3).is_err(), "nothing is prepared again");
    assert_eq!(moves.due(3, |_| true), Due::NotMoving);
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
        Err(Unbound::NotYours(
            "Docker's engine here is tcp://10.0.0.4:2375, not a socket on this machine".into(),
        ))
    })
    .expect_err("a pool not shown to be yours is moved to by no chat");
    assert!(refused.contains("pool far"), "{refused}");
    assert!(refused.contains("not known to be yours"), "{refused}");
}

/// The pane says where the chat still runs (#3646), so the core's sentence does not say it
/// again; where the setting could not be put back, it says how to set it back.
#[test]
fn a_move_that_did_not_start_says_why_and_how_to_set_its_workspace_back() {
    let to: Place = Some("builds".into());
    let put_back = did_not_start_said("the runner refused it.", Ok(()), &None, &to, "app");
    assert_eq!(
        put_back,
        "It did not start on pool builds: the runner refused it."
    );

    let stuck = did_not_start_said(
        "the runner refused it.",
        Err("machine.json could not be written".into()),
        &None,
        &to,
        "app",
    );
    assert!(!stuck.contains("runs on this machine"), "{stuck}");
    assert!(stuck.contains("still set to run on pool builds"), "{stuck}");
    assert!(stuck.contains("set Runs on to this machine"), "{stuck}");
}

/// A move refused for a pool that is not ready says the pool's own reason (#3646), both ways,
/// and never that its runner is not known to be yours.
#[test]
fn a_pool_not_ready_says_its_own_reason_for_a_move_either_way() {
    let cases = [
        (Unbound::NotHere, "no pool of that name is on this machine"),
        (
            Unbound::NoRunnerOfItsImage,
            "has no runner of its image, so remove the pool and add it again",
        ),
        (Unbound::Draining, "its runner is draining"),
        (
            Unbound::Unread("runners.json is not valid JSON".into()),
            "runners.json is not valid JSON",
        ),
    ];
    for (why, said) in cases {
        let to = goes_to_a_runner_of_yours("builds", || Err(why.clone())).unwrap_err();
        let back = brings_work_back("builds", || Err(why.clone())).unwrap_err();
        for refused in [&to, &back] {
            assert!(refused.contains(said), "{why:?}: {refused}");
            assert!(refused.contains("pool builds"), "{refused}");
            assert!(
                !refused.contains("not known to be yours"),
                "{why:?}: {refused}"
            );
        }
        assert!(
            to.starts_with("This chat does not move to pool builds"),
            "{to}"
        );
        assert!(
            back.starts_with("This chat's work does not come back"),
            "{back}"
        );
    }
}

/// The chats of a workspace move together (#3453): one prepared move for them all, which
/// switches only once every one of them is at a pause, and once.
#[test]
fn a_workspaces_chats_switch_together_once_each_is_at_a_pause() {
    let (moves, told) = heard();
    moves
        .begin(
            vec![member(3), member(4)],
            None,
            Some("builds".into()),
            || Ok(()),
        )
        .unwrap();
    let preparing = [next(&told), next(&told)];
    assert_eq!(
        preparing.iter().map(|one| one.session).collect::<Vec<_>>(),
        vec![3, 4],
        "each chat of the move is told"
    );
    assert_eq!(preparing[0].with, vec!["chat 4".to_owned()]);
    assert_eq!(preparing[1].with, vec!["chat 3".to_owned()]);
    let _ = (next(&told), next(&told));

    // Chat 3 is at a pause, chat 4 mid-turn: neither moves yet, and chat 3 says what it waits on.
    let Due::NotYet(why) = moves.due(3, |one| one == 3) else {
        panic!("one chat of the move is mid-turn");
    };
    assert!(why.contains("chat 4"), "{why}");
    assert_eq!(
        moves.due(4, |one| one == 3),
        Due::NotYet(NOT_AT_A_PAUSE.to_owned())
    );

    let Due::Now(one) = moves.due(4, |_| true) else {
        panic!("every chat of the move is at a pause");
    };
    assert_eq!(one.group, vec![member(3), member(4)]);
    assert!(
        matches!(moves.due(3, |_| true), Due::NotYet(_)),
        "the move is made once, not again for its other chat"
    );
    moves.moved(4);
    assert!(moves.shown().is_empty(), "the move is over for them all");
}

/// A move that fails fails for every chat of it (#3453): each says why, and stays where it was.
#[test]
fn a_move_that_fails_for_one_chat_fails_for_every_chat_of_it() {
    let (moves, told) = heard();
    moves
        .begin(
            vec![member(3), member(4)],
            None,
            Some("builds".into()),
            || Err("the runner of pool builds did not answer".into()),
        )
        .unwrap();
    let stages: Vec<String> = (0..4).map(|_| next(&told).stage).collect();
    assert_eq!(stages, ["preparing", "preparing", "failed", "failed"]);
    assert_eq!(moves.due(3, |_| true), Due::NotMoving);
    assert_eq!(moves.due(4, |_| true), Due::NotMoving);

    // Dismissing one chat's reason leaves the other's.
    moves.put_away(3).unwrap();
    assert_eq!(
        moves
            .shown()
            .iter()
            .map(|one| one.session)
            .collect::<Vec<_>>(),
        vec![4]
    );
}

/// Cancelling the move on any chat of it cancels it for them all, and none of them is asked to
/// move again while it is moving (#3453).
#[test]
fn a_workspaces_move_is_cancelled_whole_and_asked_once_at_a_time() {
    let (moves, told) = heard();
    moves
        .begin(
            vec![member(3), member(4)],
            None,
            Some("builds".into()),
            || Ok(()),
        )
        .unwrap();
    let _ = (next(&told), next(&told), next(&told), next(&told));
    let refused = moves
        .begin(vec![member(4), member(5)], None, Some("big".into()), || {
            Ok(())
        })
        .unwrap_err();
    assert!(refused.contains("chat 4"), "{refused}");

    moves.put_away(4).unwrap();
    assert!(moves.shown().is_empty());
    assert_eq!(moves.due(3, |_| true), Due::NotMoving);
}

/// A move put away is told over on every tab of it, so none keeps offering it (#3814), and one
/// that moved is told over on every tab too, a chat that did not take its place among them.
#[test]
fn every_tab_of_a_move_hears_it_is_over_when_it_is_put_away_or_made() {
    let (moves, told) = heard();
    moves
        .begin(
            vec![member(3), member(4)],
            None,
            Some("builds".into()),
            || Ok(()),
        )
        .unwrap();
    let _ = (next(&told), next(&told), next(&told), next(&told));
    moves.put_away(3).unwrap();
    let over = [next(&told), next(&told)];
    assert_eq!(
        over.iter()
            .map(|one| (one.session, one.stage.as_str()))
            .collect::<Vec<_>>(),
        vec![(3, "over"), (4, "over")]
    );

    moves
        .begin(
            vec![member(3), member(4)],
            None,
            Some("builds".into()),
            || Ok(()),
        )
        .unwrap();
    let _ = (next(&told), next(&told), next(&told), next(&told));
    assert!(matches!(moves.due(3, |_| true), Due::Now(_)));
    let switching = [next(&told), next(&told)];
    assert!(switching.iter().all(|one| one.stage == "switching"));
    moves.moved(3);
    let over = [next(&told), next(&told)];
    assert!(over.iter().all(|one| one.stage == "over"), "{over:?}");
}

/// A move being made is past cancelling: a Cancel then is refused in words, and the move goes
/// on (#3814).
#[test]
fn a_move_being_made_is_not_cancelled() {
    let (moves, told) = heard();
    moves
        .begin(
            vec![member(3), member(4)],
            None,
            Some("builds".into()),
            || Ok(()),
        )
        .unwrap();
    let _ = (next(&told), next(&told), next(&told), next(&told));
    assert!(matches!(moves.due(4, |_| true), Due::Now(_)));
    assert_eq!(moves.put_away(3), Err(SWITCHING_NOT_CANCELLED.to_owned()));
    assert_eq!(moves.shown().len(), 2, "still being made, for both");
}

/// A chat of the move that began a turn while the move was made stops every chat taking its
/// place, and says which (#3814).
#[test]
fn a_chat_that_began_a_turn_while_its_move_was_made_stops_it() {
    let group = vec![member(3), member(4)];
    assert_eq!(still_at_a_pause(&group, &|_| true), Ok(()));
    let refused = still_at_a_pause(&group, &|one| one != 4).unwrap_err();
    assert!(refused.starts_with("chat 4 began a turn"), "{refused}");
    assert!(refused.contains("none moved"), "{refused}");
}

/// **A move prepared again after a sign-in asks again whether the pool's runner is yours, and
/// whether its chats are still open, before anything is kept or sent there** (#3477): it may run
/// long after the move was asked for.
#[test]
fn a_preparation_run_again_asks_again_that_the_runner_is_yours_and_its_chats_are_open() {
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let (chats, host) = chats_readying(&on_pools);
    let profiles = ["work".to_owned()];

    let not_yours = prepare_on_a_runner_of_yours(&chats, "builds", "app", &profiles, true, || {
        Err(Unbound::NotYours("another engine".into()))
    })
    .unwrap_err();
    assert!(
        matches!(&not_yours, NotYet::Refused(why) if why.contains("does not move to pool builds")),
        "{not_yours:?}"
    );
    let closed =
        prepare_on_a_runner_of_yours(&chats, "builds", "app", &profiles, false, || Ok(bound()))
            .unwrap_err();
    assert!(matches!(closed, NotYet::Refused(_)), "{closed:?}");
    assert!(
        lock(&on_pools.asked).is_empty(),
        "no profile was kept there"
    );
    assert!(host.moved().is_empty(), "nothing went ahead");

    prepare_on_a_runner_of_yours(&chats, "builds", "app", &profiles, true, || Ok(bound())).unwrap();
    assert_eq!(
        host.moved(),
        vec![("ahead", "builds".to_owned(), "app".to_owned())]
    );
}

/// **A preparation of a move put away never says the stage of a new move of the same chats**
/// (#3477): it finishes late, and the new move keeps its own stage.
#[test]
fn a_late_preparation_of_a_move_put_away_leaves_a_new_move_alone() {
    let (moves, told) = heard();
    let (go, gate) = mpsc::channel::<()>();
    moves
        .begin(alone(3), None, Some("a".into()), move || {
            let _ = gate.recv();
            Ok(())
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    moves.put_away(3).unwrap();
    assert_eq!(next(&told).stage, "over");

    moves
        .begin(alone(3), None, Some("b".into()), || {
            Err(NotYet::SignIn(work_on_builds()))
        })
        .unwrap();
    assert_eq!(next(&told).stage, "preparing");
    assert_eq!(next(&told).stage, "signing-in");

    go.send(()).unwrap();
    assert!(
        told.recv_timeout(Duration::from_millis(300)).is_err(),
        "the old preparation said nothing"
    );
    assert_eq!(moves.shown()[0].stage, "signing-in");
    assert_eq!(moves.shown()[0].to.as_deref(), Some("b"));
}

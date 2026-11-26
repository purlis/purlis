// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The fault suite's cases that need the app** (#7816, of #7188's fifteen): Pool forgotten
//! (case 7) and the runner browsers' B1 to B4, on the remote parity job's pool, through what the
//! window runs: Forget ([`crate::dockerpools::forget_on`], the command's own body) and the
//! window's browsers ([`crate::browsers::WindowBrowsers`]). Each asserts what the window is told.
//! The rest of the suite is purlis-core's
//! (`crates/purlis-core/tests/it/a_live_remote_chat_survives_the_faults_that_define_trust.rs`).
//!
//! **The outage rules** (#7188), as that suite states them: nothing silent for more than
//! [`QUIET_LIMIT`] after a fault, and nothing waits past [`START_LIMIT`] to come back.
//!
//! **B4 asserts #2022's ruling**, not the case as #7188 first wrote it: "Quit ends every
//! browser", a runner's too, and the next start offers each address once. A runner's browser
//! outliving a quit would be a new ruling against #2022; what survives is a crash of the window
//! (#2025, `hostedsessions/tests/browsers_back.rs`).
//!
//! **B5** (two runners starting browsers at once, each on a machine of its own) is not here: the
//! rig has one other machine (#7836).
//!
//! The drop is the one [`super::ssh_reconnect`] puts: this pool's SSH forward killed outright, so
//! only this pool's link goes and the job's other tests run beside it. Run by the remote parity
//! job; elsewhere each runs nothing.

use super::ssh_reconnect::{the_link_drops, until};
use super::*;
use crate::browsers::{BrowserViewRole, WindowBrowsers};
use crate::reconnecting::Reconnection;
use purlis_core::eventlog::browsing::{Closer, Said};

/// Nothing silent for more than this after a fault (#7188's outage rules).
const QUIET_LIMIT: Duration = Duration::from_secs(30);

/// Nothing that started waits past this to come back (#7188's outage rules).
const START_LIMIT: Duration = Duration::from_secs(180);

/// How long a browser's engine has to be gone from its runner once it is ended.
const ENDED_WITHIN: Duration = Duration::from_secs(20);

/// A chat on the pool's runner, open and drawn.
fn a_chat(target: &Target) -> u32 {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");
    Pane::on(target, id).shows("HARNESS-READY");
    id
}

/// The window's browsers, on the target's link to its host, and every line they say for the
/// event log.
fn the_windows_browsers(target: &Target) -> (WindowBrowsers, Arc<Mutex<Vec<Said>>>) {
    let said: Arc<Mutex<Vec<Said>>> = Arc::default();
    let saying = Arc::clone(&said);
    let browsers = WindowBrowsers::new(Arc::clone(
        target.host.as_ref().expect("the window's link to its host"),
    ))
    .saying(Arc::new(move |_, one| {
        saying.lock().expect("what was said").push(one);
    }));
    (browsers, said)
}

/// A browser of the project, started on the pool's runner as a chat's is.
fn a_browser_on_the_runner(target: &Target, browsers: &WindowBrowsers) -> String {
    use crate::chatbrowsers::Driver as _;
    let pool = &target.runner.as_ref().expect("a pool's runner").pool;
    browsers
        .start(
            &target.root,
            Some(pool),
            purlis_session_protocol::browsing::BrowserReach::default(),
            None,
        )
        .unwrap_or_else(|why| panic!("a browser starts on the runner: {why:?}"))
}

/// How many of the engine's processes run on `runner`: what a browser ended there leaves none of.
fn engines_on(runner: &OnTheRunner) -> usize {
    runner
        .prints(
            "n=0; for s in /proc/[0-9]*/status; do \
             [ \"$(sed -n 's/^Name:[[:space:]]*//p' \"$s\" 2>/dev/null)\" = chrome ] && n=$((n+1)); \
             done; echo $n",
        )
        .trim()
        .parse()
        .expect("a count")
}

/// What a view of `program` was told of its link, in order, and how many frames it drew.
struct Viewed {
    told: Arc<Mutex<Vec<(Instant, Reconnection)>>>,
    frames: Arc<Mutex<Vec<Instant>>>,
    /// When it was opened.
    since: Instant,
    /// Whether it was told it ended for good: an empty frame.
    ended: Arc<std::sync::atomic::AtomicBool>,
    watching: crate::browsers::BrowserWatching,
}

impl Viewed {
    fn told_since(&self, since: Instant) -> Vec<Reconnection> {
        self.told
            .lock()
            .expect("what was told")
            .iter()
            .filter(|(at, _)| *at >= since)
            .map(|(_, said)| *said)
            .collect()
    }

    fn ended(&self) -> bool {
        self.ended.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn drew_since(&self, since: Instant) -> bool {
        self.frames
            .lock()
            .expect("the frames")
            .iter()
            .any(|at| *at >= since)
    }
}

/// A focused view of `program`, as the window draws one.
fn a_view_of(browsers: &WindowBrowsers, program: &str) -> Viewed {
    let told: Arc<Mutex<Vec<(Instant, Reconnection)>>> = Arc::default();
    let frames: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let ended: Arc<std::sync::atomic::AtomicBool> = Arc::default();
    let (telling, drawing, ending) = (Arc::clone(&told), Arc::clone(&frames), Arc::clone(&ended));
    let watching = browsers
        .watch(
            program,
            BrowserViewRole::Focused,
            Box::new(move |_aim, image| {
                if image.is_empty() {
                    ending.store(true, std::sync::atomic::Ordering::SeqCst);
                } else {
                    drawing.lock().expect("the frames").push(Instant::now());
                }
            }),
            Box::new(|_| {}),
            Box::new(|_| {}),
            Arc::new(move |said| {
                telling
                    .lock()
                    .expect("what was told")
                    .push((Instant::now(), said));
            }),
        )
        .expect("the browser is viewed");
    Viewed {
        since: Instant::now(),
        told,
        frames,
        ended,
        watching,
    }
}

/// **Case 7: a pool forgotten during a chat ends it as Pool forgotten, and Forget lists what is
/// left on the machine** (#7187, CONTEXT.md *Pool forgotten*): the chat ends within the quiet
/// limit, as Pool forgotten and not with its runner, and Forget's answer names the runner's
/// container and home volume that stay on the pool's machine.
mod a_pool_forgotten_during_a_chat_ends_it_as_pool_forgotten_and_lists_what_is_left_on_the_machine {
    use super::*;

    #[test]
    fn in_a_remote_docker_pool() {
        let Some((target, machine, pool, _again)) = super::super::a_remote_docker_pool() else {
            return;
        };
        let id = a_chat(&target);
        let made = purlis_core::runners::runner_for(&machine.config, &pool)
            .expect("its runner is kept")
            .made;
        let host = purlis_core::runners::pools(&machine.config)
            .expect("the pools read")
            .into_iter()
            .find(|one| one.name() == pool)
            .and_then(|one| one.remote_engine().map(|remote| remote.host().to_owned()))
            .expect("the pool is on another machine");

        let began = Instant::now();
        let left = crate::dockerpools::forget_on(
            &machine.config,
            &pool,
            &[],
            Err(&"no browser in this case".to_owned()),
            target.host.as_deref(),
        )
        .expect("Forget works on a pool in any state")
        .expect("what stays on the machine is listed");

        let forgot = began.elapsed();
        assert!(forgot <= QUIET_LIMIT, "Forget took {forgot:?}");
        // Told within the quiet limit of the press, Forget's own time counted in it.
        until(
            QUIET_LIMIT - forgot,
            "the chat ends as Pool forgotten within the quiet limit",
            || target.sessions.lost_with_its_pool_forgotten(id),
        );
        assert!(
            !target.sessions.lost_with_its_runner(id),
            "a forgotten pool's runner is not gone: its chat does not say Runner gone"
        );
        let volume = made.runner.volume();
        for named in [made.container.as_str(), volume.as_str(), host.as_str()] {
            assert!(left.contains(named), "Forget's list names {named}: {left}");
        }
    }
}

/// **B1: a browser's link drops and returns: its view says why, and draws again** (#7190, #2542):
/// the view is dimmed (told Reconnecting) within the quiet limit, told Reconnected once the link
/// is back by itself within the start limit, and the same browser draws again.
mod b1_a_browsers_link_drops_and_returns_the_view_says_why_and_the_page_comes_back {
    use super::*;

    #[test]
    fn in_a_remote_docker_pool() {
        let Some(target) = super::super::in_a_remote_docker_pool() else {
            return;
        };
        let (browsers, _said) = the_windows_browsers(&target);
        let program = a_browser_on_the_runner(&target, &browsers);
        let view = a_view_of(&browsers, &program);
        let began = Instant::now();
        until(START_LIMIT, "the browser draws its page", || {
            view.drew_since(began)
        });

        let dropped = the_link_drops(&target);
        until(
            QUIET_LIMIT,
            "the view says the link is down within the quiet limit",
            || {
                view.told_since(dropped)
                    .contains(&Reconnection::Reconnecting)
            },
        );
        until(START_LIMIT, "the view is back by itself", || {
            view.told_since(dropped)
                .ends_with(&[Reconnection::Reconnected])
        });
        let back = Instant::now();
        until(START_LIMIT, "the same browser draws again", || {
            view.drew_since(back)
        });
        assert_eq!(
            browsers.pool_of(&program).as_deref(),
            Some(&*target.runner.as_ref().expect("a pool's runner").pool)
        );
        browsers.close(&program).expect("the browser ends");
    }
}

/// **B2: a takeover during a drop keeps control, or the view says why** (#7190, #2014): the
/// person takes the browser over while its link is down; the window either does it or says why
/// it could not, and once the link is back a view opened again says the same of who drives.
mod b2_a_takeover_during_a_drop_keeps_control_or_the_view_says_why {
    use super::*;

    #[test]
    fn in_a_remote_docker_pool() {
        let Some(target) = super::super::in_a_remote_docker_pool() else {
            return;
        };
        let (browsers, _said) = the_windows_browsers(&target);
        let program = a_browser_on_the_runner(&target, &browsers);
        let view = a_view_of(&browsers, &program);
        assert!(!view.watching.person_drives, "the agent drives it first");

        let dropped = the_link_drops(&target);
        until(QUIET_LIMIT, "the view says the link is down", || {
            view.told_since(dropped)
                .contains(&Reconnection::Reconnecting)
        });
        let took = browsers.take_over(&program);
        if let Err(why) = &took {
            assert!(
                !why.trim().is_empty(),
                "control was not taken, and no reason was said"
            );
        }
        until(START_LIMIT, "the view is back by itself", || {
            view.told_since(dropped)
                .ends_with(&[Reconnection::Reconnected])
        });

        // Where control is now: the person, where the takeover was done, else still the agent.
        browsers
            .unwatch(&program, view.watching.view)
            .expect("the view is closed");
        let again = a_view_of(&browsers, &program);
        assert_eq!(
            again.watching.person_drives,
            took.is_ok(),
            "a view opened again says who drives as the takeover's answer did: {took:?}"
        );
        browsers.close(&program).expect("the browser ends");
    }
}

/// **B3: a pool forgotten with a browser open ends it on its machine** (#7187 item 6, CONTEXT.md
/// *Pool forgotten*): Forget stops the browser where its machine answers, so nothing of it is
/// listed as left there; it is said closed by the person, never crashed; and its engine no longer
/// runs on the runner. The chat beside it ends as Pool forgotten.
mod b3_a_pool_forgotten_with_a_browser_open_ends_it_as_pool_forgotten {
    use super::*;

    #[test]
    fn in_a_remote_docker_pool() {
        let Some((target, machine, pool, _again)) = super::super::a_remote_docker_pool() else {
            return;
        };
        let id = a_chat(&target);
        let (browsers, said) = the_windows_browsers(&target);
        let program = a_browser_on_the_runner(&target, &browsers);
        let runner = target.runner.as_ref().expect("a pool's runner");
        assert!(engines_on(runner) > 0, "its engine runs on the runner");
        let view = a_view_of(&browsers, &program);

        let left = crate::dockerpools::forget_on(
            &machine.config,
            &pool,
            &[],
            Ok(&browsers),
            target.host.as_deref(),
        )
        .expect("Forget works on a pool in any state");

        let one_left = crate::browsers::left_on_the_machine(1).expect("said of one");
        assert!(
            left.as_deref().is_none_or(|left| !left.contains(&one_left)),
            "the browser was stopped on its machine, so none is listed as left: {left:?}"
        );
        // Its view is told it ended, and is not left dimmed as though the link might come back.
        until(QUIET_LIMIT, "the browser's view is told it ended", || {
            view.ended()
        });
        assert_ne!(
            view.told_since(view.since).last(),
            Some(&Reconnection::Reconnecting),
            "a forgotten pool's browser is ended, not reconnecting"
        );
        assert_eq!(browsers.pool_of(&program), None, "forgotten with its pool");
        until(ENDED_WITHIN, "its engine ends on the runner", || {
            engines_on(runner) == 0
        });
        until(QUIET_LIMIT, "the chat ends as Pool forgotten", || {
            target.sessions.lost_with_its_pool_forgotten(id)
        });
        let ends: Vec<Said> = said
            .lock()
            .expect("what was said")
            .iter()
            .filter(|one| !matches!(one, Said::Opened { .. }))
            .cloned()
            .collect();
        assert_eq!(
            ends,
            [Said::Closed {
                browser: program,
                by: Closer::Person
            }],
            "said closed by the person, never crashed"
        );
    }
}

/// **B4: a quit ends a runner's browser too** (#2022, ruled 2026-10-18: "Quit ends every
/// browser"): the quit lets go of each project, and every browser the window started in it ends
/// ([`WindowBrowsers::let_go`], what `planes` runs for each project at quit), a runner's on its
/// runner: said closed by the person, and its engine runs there no more. That the next start
/// offers its address once is the window's record of this machine
/// (`chatbrowsers/tests.rs`, `purlis_core::browsersatquit`), the same for a runner's browser.
/// #7188 first wrote B4 as a browser outliving the quit; that would be a ruling against #2022.
mod b4_an_app_quit_with_a_runner_browser_open_ends_it_on_its_runner {
    use super::*;

    #[test]
    fn in_a_remote_docker_pool() {
        let Some(target) = super::super::in_a_remote_docker_pool() else {
            return;
        };
        let (browsers, said) = the_windows_browsers(&target);
        let program = a_browser_on_the_runner(&target, &browsers);
        let runner = target.runner.as_ref().expect("a pool's runner");
        assert!(engines_on(runner) > 0, "its engine runs on the runner");

        assert_eq!(browsers.let_go(&target.root), 1, "the quit ends it");

        until(ENDED_WITHIN, "its engine ends on the runner", || {
            engines_on(runner) == 0
        });
        assert!(!browsers.started_in(&target.root, &program));
        assert!(
            said.lock().expect("what was said").contains(&Said::Closed {
                browser: program,
                by: Closer::Person
            }),
            "said closed by the person, never crashed"
        );
    }
}

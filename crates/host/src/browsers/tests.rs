// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use purlis_session_protocol::session::{Exit, ViewKind, ViewRole};

use crate::recording::Stopped;

use super::{Browser, FrameSink, NotActed, Screen};
use crate::hosting::Running;

/// A screen whose `hide` waits at a barrier the first time, so a test can put an attach in the
/// middle of the last view's detach.
#[derive(Default)]
struct Slow {
    showing: AtomicBool,
    hold_hide: Mutex<Option<Arc<Barrier>>>,
    /// The widest frame the engine was last asked for: `None` until asked.
    narrowed: Mutex<Option<Option<u16>>>,
    /// The least time between frames the engine was last asked for.
    paced: Mutex<Option<Duration>>,
}

impl Screen for Slow {
    fn reach_now(
        &self,
        _reach: &purlis_session_protocol::browsing::BrowserReach,
    ) -> Result<(), String> {
        Ok(())
    }

    fn show(&self, _frames: FrameSink) {
        self.showing.store(true, Ordering::SeqCst);
    }

    fn pace(&self, every: Duration) {
        *self.paced.lock().unwrap() = Some(every);
    }

    fn narrow(&self, most: Option<u16>) {
        *self.narrowed.lock().unwrap() = Some(most);
    }

    fn hide(&self) {
        let held = self.hold_hide.lock().unwrap().take();
        if let Some(barrier) = held {
            barrier.wait();
            barrier.wait();
        }
        self.showing.store(false, Ordering::SeqCst);
    }

    fn size(&self) -> (u16, u16) {
        (1280, 800)
    }

    fn process(&self) -> Option<u32> {
        None
    }

    fn stop(&self) {}

    fn when_it_ends(&self, _tell: Box<dyn FnOnce(super::ScreenEnded) + Send>) {}

    fn act(
        &self,
        _call_id: &str,
        _act: purlis_session_protocol::browsing::Act,
        _epoch: u64,
    ) -> Result<purlis_session_protocol::browsing::Browsed, NotActed> {
        Err(NotActed::Refused("a stand-in".into()))
    }

    fn take_over(&self) -> u64 {
        1
    }

    fn hand_back(&self) -> u64 {
        2
    }

    fn input(
        &self,
        _input: &purlis_session_protocol::browsing::Input,
    ) -> Result<(), super::NotTaken> {
        Ok(())
    }

    fn size_to(&self, _width: u16, _height: u16) -> Result<(), super::NotTaken> {
        Ok(())
    }

    fn log_in(
        &self,
        _login: &purlis_session_protocol::browsing::LogIn,
    ) -> Result<super::LoginEnded, NotActed> {
        Err(NotActed::Refused("a stand-in".into()))
    }
}

#[test]
fn a_view_attached_while_the_last_one_detaches_is_still_shown_frames() {
    let screen = Arc::new(Slow::default());
    let browser = Browser::new(screen.clone(), "b-1", Arc::default());
    let source = browser
        .view(
            ViewKind::Browser,
            Some(ViewRole::Focused),
            None,
            super::Reads {
                aims: true,
                origins: true,
                between_hosts: false,
            },
        )
        .expect("a browser view");
    let first = source.attach();
    assert!(screen.showing.load(Ordering::SeqCst));

    // The last view detaches, and its hide is caught half way.
    let barrier = Arc::new(Barrier::new(2));
    *screen.hold_hide.lock().unwrap() = Some(barrier.clone());
    let detaching = std::thread::spawn(move || drop(first));
    barrier.wait();

    // A new view attaches meanwhile, on another thread; then the hide is let go.
    let attaching = {
        let source = source.clone();
        std::thread::spawn(move || source.attach())
    };
    std::thread::sleep(Duration::from_millis(100));
    barrier.wait();
    detaching.join().unwrap();
    let second = attaching.join().unwrap();

    assert!(
        screen.showing.load(Ordering::SeqCst),
        "a view is attached, so its browser makes frames"
    );
    drop(second);
}

/// A view of `browser`, attached over a link between hosts or not.
fn a_view(
    browser: &Browser,
    between_hosts: bool,
) -> Arc<dyn purlis_session_protocol::view::Source> {
    browser
        .view(
            ViewKind::Browser,
            Some(ViewRole::Focused),
            None,
            super::Reads {
                aims: true,
                origins: true,
                between_hosts,
            },
        )
        .expect("a browser view")
}

#[test]
fn a_view_on_the_runner_itself_does_not_widen_the_frames_a_view_over_the_link_is_sent() {
    // #3297: the engine's frames are as wide as one view asks for all of them. A view not held
    // to the link's width, attached beside one that is, must not widen what crosses the link.
    let screen = Arc::new(Slow::default());
    let browser = Browser::new(screen.clone(), "b-1", Arc::default());
    let local = a_view(&browser, false).attach();
    assert_eq!(
        *screen.narrowed.lock().unwrap(),
        Some(None),
        "as wide as its pages"
    );

    let linked = a_view(&browser, true).attach();
    assert_eq!(
        *screen.narrowed.lock().unwrap(),
        Some(Some(1280)),
        "no wider than the link's frames while a view over the link is attached"
    );

    drop(linked);
    assert_eq!(
        *screen.narrowed.lock().unwrap(),
        Some(None),
        "as wide as its pages again once the view over the link has gone"
    );
    drop(local);
}

#[test]
fn a_browser_started_with_its_pools_limits_holds_its_views_over_a_link_to_them() {
    // #3297: a pool's own frames a second and widest frame, given with the browser's start.
    let screen = Arc::new(Slow::default());
    let browser = Browser::held_over_links_to(
        screen.clone(),
        "b-1",
        Arc::default(),
        purlis_session_protocol::browsing::LinkFrames {
            frames: 30,
            width: 1920,
        },
    );
    let linked = a_view(&browser, true).attach();
    assert_eq!(*screen.narrowed.lock().unwrap(), Some(Some(1920)));
    assert_eq!(
        *screen.paced.lock().unwrap(),
        Some(Duration::from_secs(1) / 30)
    );
    drop(linked);

    // One started without them is held to spec #2041's defaults.
    let screen = Arc::new(Slow::default());
    let browser = Browser::new(screen.clone(), "b-2", Arc::default());
    let linked = a_view(&browser, true).attach();
    assert_eq!(*screen.narrowed.lock().unwrap(), Some(Some(1280)));
    assert_eq!(
        *screen.paced.lock().unwrap(),
        Some(Duration::from_secs(1) / 15)
    );
    drop(linked);
}

#[test]
fn a_browsers_end_says_the_hosts_act_and_how_its_engine_ended() {
    let ended = super::ScreenEnded {
        exit: Exit::Signal("SIGKILL".into()),
        why: "purlis ended its engine (signal SIGKILL) because its DevTools connection closed: \
              the engine sent an event (Page.frameNavigated) longer than 67108864 bytes, which \
              purlis does not read"
            .into(),
    };
    assert_eq!(
        super::ended_said("01K2", None, &ended),
        "browser 01K2 ended (not stopped by the host): purlis ended its engine (signal SIGKILL) \
         because its DevTools connection closed: the engine sent an event (Page.frameNavigated) \
         longer than 67108864 bytes, which purlis does not read"
    );
    let stopped = super::ScreenEnded {
        exit: Exit::Code(0),
        why: "purlis stopped its engine (exit code 0)".into(),
    };
    assert_eq!(
        super::ended_said("01K2", Some(Stopped::Operator), &stopped),
        "browser 01K2 ended (stopped at its link's ask): purlis stopped its engine (exit code 0)"
    );
    assert!(super::ended_said("b", Some(Stopped::Quit), &stopped).contains("as the host ended"));
    assert!(super::ended_said("b", Some(Stopped::Killed), &stopped).contains("kill switch"));
    assert!(super::ended_said("b", Some(Stopped::Closed), &stopped).contains("closed at"));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2745): whoever waits on a browser's end is told who ended it and how
//! its engine's process ended.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::drive::next_id;
use common::site::started;
use purlis_browser_engine::drive::{Browser, Call};
use purlis_browser_engine::ending::{EndedBy, Ending, EngineExit};

/// What `browser` is told as its engine ends.
fn told_of(browser: &Browser) -> Arc<Mutex<Option<Ending>>> {
    let told: Arc<Mutex<Option<Ending>>> = Arc::default();
    let telling = Arc::clone(&told);
    browser.when_it_ends(Box::new(move |ending| {
        *telling.lock().unwrap() = Some(ending)
    }));
    told
}

fn waited(told: &Mutex<Option<Ending>>) -> Ending {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(ending) = told.lock().unwrap().clone() {
            return ending;
        }
        assert!(Instant::now() < deadline, "never told how the engine ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_stopped_browser_says_purlis_stopped_it_and_how_its_engine_exited() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let told = told_of(&browser);
    browser.stop(Duration::from_secs(5));
    let ending = waited(&told);
    assert_eq!(ending.by, EndedBy::Stopped, "{ending}");
    assert!(ending.exit.is_some(), "{ending}");
}

#[test]
fn an_engine_a_signal_ended_says_it_ended_by_itself_with_that_signal() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let told = told_of(&browser);
    let pid = browser.process().expect("the engine runs");
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap()),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    let ending = waited(&told);
    assert_eq!(ending.by, EndedBy::Itself, "{ending}");
    assert_eq!(
        ending.exit,
        Some(EngineExit::Signal("SIGKILL".into())),
        "{ending}"
    );
    // The browser holds no engine any more: its group was ended and reaped.
    assert_eq!(browser.process(), None);
    // An act answers that the browser has ended.
    let address =
        purlis_browser_engine::WebAddress::parse("http://fixture.test/frames.html").unwrap();
    assert!(browser.call(&next_id(), Call::Open { address }).is_err());
}

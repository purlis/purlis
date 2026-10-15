// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use purlis_session_protocol::session::{Exit, ViewKind, ViewRole};

use super::{Browser, FrameSink, Screen};
use crate::hosting::Running;

/// A screen whose `hide` waits at a barrier the first time, so a test can put an attach in the
/// middle of the last view's detach.
#[derive(Default)]
struct Slow {
    showing: AtomicBool,
    hold_hide: Mutex<Option<Arc<Barrier>>>,
}

impl Screen for Slow {
    fn show(&self, _frames: FrameSink) {
        self.showing.store(true, Ordering::SeqCst);
    }

    fn pace(&self, _every: Duration) {}

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

    fn when_it_ends(&self, _tell: Box<dyn FnOnce(Exit) + Send>) {}

    fn act(
        &self,
        _call_id: &str,
        _act: purlis_session_protocol::browsing::Act,
    ) -> Result<purlis_session_protocol::browsing::Browsed, String> {
        Err("a stand-in".into())
    }
}

#[test]
fn a_view_attached_while_the_last_one_detaches_is_still_shown_frames() {
    let screen = Arc::new(Slow::default());
    let browser = Browser::new(screen.clone());
    let source = browser
        .view(ViewKind::Browser, Some(ViewRole::Focused), None)
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

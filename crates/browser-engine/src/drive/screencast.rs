// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser's frames, for the person to watch (#2012): the engine's screencast of the current
//! page, as JPEG.
//!
//! **Only while shown.** Nothing is asked of the engine until [`super::Browser::show`], and
//! [`super::Browser::hide`] stops it, so a browser nobody watches paints nothing for anyone.
//!
//! **Paced by its acknowledgement.** The engine sends its next frame only once the last one is
//! acknowledged, so acknowledging no sooner than the pace ([`super::Browser::pace`]) is what
//! makes it make fewer: a browser watched only as a thumbnail costs about a frame a second.
//!
//! **One thread, never the event reader's.** The calls into the engine (start, stop,
//! acknowledge) are made on a thread of the screencast's own, so neither the reader of the
//! engine's events nor the caller waits on them. The screencast follows the current page.
//!
//! **No wider than asked** (#2050): [`super::Browser::narrow`] has the engine scale its frames
//! down to a width, as a view over a link between hosts wants them; a change of it starts the
//! screencast again at the new width.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{Value, json};

use super::browser::Inner;
use super::{Aim, Painted, lock};

/// Where each frame goes: its image, and what it showed.
pub(crate) type Sink = Arc<dyn Fn(Painted) + Send + Sync>;

/// The JPEG quality frames are made at: what the frames prototype measured (2026-10-14).
pub const QUALITY: u8 = 60;

/// How often the screencast looks whether the current page has changed.
const LOOK: Duration = Duration::from_millis(250);

enum Ask {
    Show,
    Hide,
    Pace(Duration),
    Narrow(Option<u16>),
    Frame { session: String, ack: i64 },
}

/// The screencast's side of a browser: the sink frames go to while shown, and its thread.
pub(crate) struct Screencast {
    sink: Mutex<Option<Sink>>,
    asks: Mutex<Option<Sender<Ask>>>,
}

impl Screencast {
    pub(crate) fn new() -> Self {
        Self {
            sink: Mutex::new(None),
            asks: Mutex::new(None),
        }
    }

    /// Starts the screencast's thread for `inner`, held weakly: it ends with the browser.
    pub(crate) fn start(inner: &Arc<Inner>) {
        let (asks, asked) = mpsc::channel();
        *lock(&inner.cast.asks) = Some(asks);
        let weak = Arc::downgrade(inner);
        let _ = thread::Builder::new()
            .name("browser-screencast".to_owned())
            .spawn(move || run(&weak, &asked));
    }

    fn ask(&self, ask: Ask) {
        if let Some(asks) = lock(&self.asks).as_ref() {
            let _ = asks.send(ask);
        }
    }

    pub(crate) fn show(&self, sink: Sink) {
        *lock(&self.sink) = Some(sink);
        self.ask(Ask::Show);
    }

    pub(crate) fn hide(&self) {
        *lock(&self.sink) = None;
        self.ask(Ask::Hide);
    }

    pub(crate) fn pace(&self, every: Duration) {
        self.ask(Ask::Pace(every));
    }

    pub(crate) fn narrow(&self, most: Option<u16>) {
        self.ask(Ask::Narrow(most));
    }

    /// A `Page.screencastFrame` event, from the event reader: handed over while shown, with
    /// `aim`, what it showed (#2014), and `address`, the engine's record of its page's document
    /// (#2300), and acknowledged on the screencast's thread.
    pub(crate) fn frame(&self, params: &Value, session: Option<&str>, aim: Aim, address: String) {
        let Some(sink) = lock(&self.sink).clone() else {
            return;
        };
        let (Some(data), Some(ack), Some(session)) = (
            params.get("data").and_then(Value::as_str),
            params.get("sessionId").and_then(Value::as_i64),
            session,
        ) else {
            return;
        };
        if let Ok(jpeg) = base64::engine::general_purpose::STANDARD.decode(data) {
            sink(Painted { aim, address, jpeg });
        }
        self.ask(Ask::Frame {
            session: session.to_owned(),
            ack,
        });
    }
}

/// The screencast's thread: start and stop it on the current page, follow that page, and
/// acknowledge each frame no sooner than the pace.
fn run(weak: &Weak<Inner>, asked: &Receiver<Ask>) {
    let mut showing = false;
    let mut on: Option<String> = None;
    let mut every = Duration::ZERO;
    let mut acked: Option<Instant> = None;
    let mut waiting: Option<(String, i64)> = None;
    let mut most_width: Option<u16> = None;
    loop {
        let due = waiting
            .as_ref()
            .map(|_| acked.map_or_else(Instant::now, |at| at + every));
        let wait = due.map_or(LOOK, |due| {
            due.saturating_duration_since(Instant::now()).min(LOOK)
        });
        let next = asked.recv_timeout(wait);
        let Some(inner) = weak.upgrade() else {
            return;
        };
        if inner.ended() {
            return;
        }
        match next {
            Ok(Ask::Show) => {
                showing = true;
                follow(&inner, &mut on, most_width);
            }
            Ok(Ask::Hide) => {
                showing = false;
                waiting = None;
                if let Some(session) = on.take() {
                    let _ = inner
                        .devtools
                        .call("Page.stopScreencast", json!({}), Some(&session));
                }
            }
            Ok(Ask::Pace(pace)) => every = pace,
            Ok(Ask::Narrow(most)) => {
                if most != most_width {
                    most_width = most;
                    // Started again at the new width, on the page it casts.
                    if let Some(session) = on.take() {
                        waiting = None;
                        let _ =
                            inner
                                .devtools
                                .call("Page.stopScreencast", json!({}), Some(&session));
                    }
                    if showing {
                        follow(&inner, &mut on, most_width);
                    }
                }
            }
            Ok(Ask::Frame { session, ack }) => {
                if showing {
                    waiting = Some((session, ack));
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if showing {
                    follow(&inner, &mut on, most_width);
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if let (Some(due), Some((session, ack))) = (
            waiting
                .as_ref()
                .map(|_| acked.map_or_else(Instant::now, |at| at + every)),
            waiting.as_ref(),
        ) && Instant::now() >= due
        {
            let _ = inner.devtools.call(
                "Page.screencastFrameAck",
                json!({ "sessionId": ack }),
                Some(session),
            );
            acked = Some(Instant::now());
            waiting = None;
        }
    }
}

/// Casts the current page, no wider than `most_width`: stops the screencast on a page that is
/// no longer current, and starts it on the one that is.
fn follow(inner: &Inner, on: &mut Option<String>, most_width: Option<u16>) {
    let current = inner.current_session();
    if *on == current {
        return;
    }
    if let Some(old) = on.take() {
        let _ = inner
            .devtools
            .call("Page.stopScreencast", json!({}), Some(&old));
    }
    if let Some(session) = current {
        let mut asked = json!({ "format": "jpeg", "quality": QUALITY, "everyNthFrame": 1 });
        if let Some(most) = most_width {
            asked["maxWidth"] = json!(most);
        }
        let started = inner
            .devtools
            .call("Page.startScreencast", asked, Some(&session));
        if started.is_ok() {
            *on = Some(session);
        }
    }
}

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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{Value, json};

use super::browser::Inner;
use super::{DownloadInfo, ListedPage, PageLoad, Painted, lock};

/// Where each frame goes: its image, and what it showed.
pub(crate) type Sink = Arc<dyn Fn(Painted) + Send + Sync>;

/// The JPEG quality frames are made at: what the frames prototype measured (2026-10-14).
pub const QUALITY: u8 = 60;

/// How often the screencast looks whether the current page has changed.
const LOOK: Duration = Duration::from_millis(250);

/// How often a view is told at most how far a download under way came (#6990): the engine says
/// it far more often than a person reads it.
const PROGRESS_TOLD_EVERY: Duration = Duration::from_millis(250);

enum Ask {
    Show,
    Hide,
    Pace(Duration),
    Narrow(Option<u16>),
    Frame { session: String, ack: i64 },
}

/// The screencast's side of a browser: the sink frames go to while shown, what it was last told
/// the page is (#6938), and its thread.
pub(crate) struct Screencast {
    sink: Mutex<Option<Sink>>,
    told: Mutex<Option<Told>>,
    /// When the sink was last told what the page is with no image (#6990): what a download's
    /// progress is paced by.
    told_at: Mutex<Option<Instant>>,
    /// A download's progress was left untold for its pace: the thread tells it once the pace
    /// allows, so the view is never left behind by an engine that has gone quiet.
    owed: AtomicBool,
    asks: Mutex<Option<Sender<Ask>>>,
}

/// What the sink was last told the page is: its address, title and load (#6938), and the
/// browser's pages (#6989) and downloads (#6990).
type Told = (String, String, PageLoad, Vec<ListedPage>, Vec<DownloadInfo>);

/// What `shown` tells a view, besides its image and its aim.
fn told(shown: &Painted) -> Told {
    (
        shown.address.clone(),
        shown.title.clone(),
        shown.load.clone(),
        shown.pages.clone(),
        shown.downloads.clone(),
    )
}

impl Screencast {
    pub(crate) fn new() -> Self {
        Self {
            sink: Mutex::new(None),
            told: Mutex::new(None),
            told_at: Mutex::new(None),
            owed: AtomicBool::new(false),
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
        *lock(&self.told) = None;
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

    /// Whether frames are shown now: whether a view is told what the page is (#6938).
    pub(crate) fn showing(&self) -> bool {
        lock(&self.sink).is_some()
    }

    /// **What the page is now, with no image** (#6938): handed over while shown, and only where
    /// it is not what the sink was last told.
    pub(crate) fn page_now(&self, shown: Painted) {
        let Some(sink) = lock(&self.sink).clone() else {
            return;
        };
        let now = told(&shown);
        {
            let mut told = lock(&self.told);
            if told.as_ref() == Some(&now) {
                return;
            }
            *told = Some(now);
        }
        *lock(&self.told_at) = Some(Instant::now());
        sink(shown);
    }

    /// **Whether a download's progress may be told now** (#6990): not where the sink was told
    /// what the page is within [`PROGRESS_TOLD_EVERY`], and then it is owed, for the thread to
    /// tell once the pace allows.
    pub(crate) fn progress_may_be_told(&self) -> bool {
        if lock(&self.told_at).is_some_and(|at| at.elapsed() < PROGRESS_TOLD_EVERY) {
            self.owed.store(true, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// Whether progress is owed and its pace now allows it, which then is owed no more.
    fn owed_now(&self) -> bool {
        self.owed.load(Ordering::SeqCst)
            && !lock(&self.told_at).is_some_and(|at| at.elapsed() < PROGRESS_TOLD_EVERY)
            && self.owed.swap(false, Ordering::SeqCst)
    }

    /// A `Page.screencastFrame` event, from the event reader: handed over while shown, with
    /// what `shown` says of it (its aim, #2014, the engine's record of its page's document,
    /// #2300, its title and its load, #6938), and acknowledged on the screencast's thread.
    pub(crate) fn frame(&self, params: &Value, session: Option<&str>, shown: Painted) {
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
            *lock(&self.told) = Some(told(&shown));
            sink(Painted { jpeg, ..shown });
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
        // A download's progress left untold for its pace (#6990).
        if showing && inner.cast.owed_now() {
            inner.tell_now();
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
/// no longer current, and starts it on the one that is, and tells its views so.
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
    // Another page is current, whatever made it so (a call, a page it opened, a login, the
    // person's pick): its views are told at once which, before it paints, and whether it paints
    // at all (#6989).
    inner.tell_now();
}

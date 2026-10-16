// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser as a hosted program (#2012, spec #2008): started by the binary, viewed through
//! browser views, ended like every other program.
//!
//! **What starts a browser is the binary's** ([`Browsers`]): the engine (a pinned Chrome over
//! an inherited pipe, in a process group its keeper ends) and the reach proxy that is its only
//! route out are `purlis-browser-engine`'s and the core's, which this crate does not link. The
//! host gives each browser a data folder of its own under its store, fresh: removed once the
//! browser has ended.
//!
//! **Frames only while watched.** A browser makes frames only while a browser view of it is
//! attached: the first view asks its engine for frames ([`Screen::show`]), the last one to go
//! stops them ([`Screen::hide`]). A view is focused (every frame) or a thumbnail (about one a
//! second, [`THUMBNAIL_EVERY`]), and may ask for fewer still (`rate`); the engine is asked for
//! frames no more often than the view that wants them most often ([`Screen::pace`]).
//!
//! **A view's snapshot is the latest frame.** Each frame is framed whole on the view's stream
//! (`purlis_session_protocol::view::frames`). A view that falls behind is dropped and attached
//! again, as every view is, and its fresh snapshot is the latest frame: it starts on a whole
//! one. A view between frames is sent only the newest, never a backlog.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};
use purlis_session_protocol::session::{Exit, Readiness, ViewKind, ViewRole};
use purlis_session_protocol::view::frames::{JPEG, frame};
use purlis_session_protocol::view::{Attachment, Source};
use tokio::sync::watch;

use crate::hosting::Running;

/// Where a browser's engine hands each frame it makes: a JPEG image.
pub type FrameSink = Arc<dyn Fn(Bytes) + Send + Sync>;

/// The least time between two frames sent to a thumbnail: about one a second.
pub const THUMBNAIL_EVERY: Duration = Duration::from_secs(1);

/// A running browser, as the host drives it. Every call returns at once: one that has to wait
/// on the engine is carried out on the binary's own thread, never the host's runtime.
pub trait Screen: Send + Sync + 'static {
    /// Start making frames, each handed to `frames`.
    fn show(&self, frames: FrameSink);
    /// Make frames no more often than `every` (zero: as often as the page changes).
    fn pace(&self, every: Duration);
    /// Stop making frames.
    fn hide(&self);
    /// The size of its page, in CSS pixels: what a view's frames are drawn at.
    fn size(&self) -> (u16, u16);
    /// The engine's process, while it runs.
    fn process(&self) -> Option<u32>;
    /// End the engine and its whole process group; returns once they are gone.
    fn stop(&self);
    /// Calls `tell` once, as the engine ends.
    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>);
    /// Carries out `act` under its own id `call_id` (#2013), and answers what the browser
    /// answered, or why it did not act. It waits on the engine, so the host calls it off its
    /// runtime.
    fn act(&self, call_id: &str, act: Act) -> Result<Browsed, NotActed>;
}

/// Why a browser did not carry an act out (#2013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotActed {
    /// The browser has ended: stopped, closed, or its engine ended. Refused
    /// `browser_ended`, in [`AN_ENDED_BROWSER`]'s words.
    Ended,
    /// The driver refused it, or could not tell whether it acted, in words that say what to do.
    Refused(String),
}

/// What an act on a browser that has ended is refused with (#2013).
pub const AN_ENDED_BROWSER: &str = "the browser has ended: open a new one";

/// Starts browsers: the binary's, built on the engine and the core.
pub trait Browsers: Send + Sync + 'static {
    /// The folder each browser's data folder is made in, under the host's store. Asked as a
    /// browser starts, never before, so a host that starts none reads no store for them.
    fn data_folders(&self) -> Result<PathBuf, String>;
    /// Start a browser whose data folder is `data` (made by the binary, this user's alone),
    /// reaching what `reach` says, or with none, nothing listed and every site new to the
    /// project refused (#2013).
    fn start(&self, data: &Path, reach: Option<&BrowserReach>) -> Result<Arc<dyn Screen>, String>;
}

/// A host whose binary starts no browser: every start of one is refused with a sentence.
pub struct NoBrowsers;

impl Browsers for NoBrowsers {
    fn data_folders(&self) -> Result<PathBuf, String> {
        Err("this session host was not given a way to start a browser".into())
    }

    fn start(
        &self,
        _data: &Path,
        _reach: Option<&BrowserReach>,
    ) -> Result<Arc<dyn Screen>, String> {
        Err("this session host was not given a way to start a browser".into())
    }
}

/// What a browser view is sent: nothing yet, the latest frame (framed), or the end.
#[derive(Clone)]
enum Latest {
    Nothing,
    Frame(Bytes),
    Ended,
}

/// One browser's frames, and the views that want them.
struct Hub {
    screen: Arc<dyn Screen>,
    latest: watch::Sender<Latest>,
    /// Each attached view's least time between frames, by its number here.
    viewers: Mutex<Vec<(u64, Duration)>>,
    next: AtomicU64,
}

impl Hub {
    fn viewers(&self) -> MutexGuard<'_, Vec<(u64, Duration)>> {
        self.viewers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A view that wants a frame no more often than `every` is attached.
    ///
    /// The engine is asked to show, pace and hide under the viewers' lock (each returns at once,
    /// [`Screen`]), so a view attached while the last one detaches is never left with its
    /// browser hidden.
    fn watched(self: &Arc<Self>, every: Duration) -> Watching {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mut viewers = self.viewers();
        viewers.push((id, every));
        Self::repace(&self.screen, &viewers);
        if viewers.len() == 1 {
            let latest = self.latest.clone();
            self.screen.show(Arc::new(move |image: Bytes| {
                latest.send_if_modified(|now| {
                    if matches!(now, Latest::Ended) {
                        return false;
                    }
                    *now = Latest::Frame(frame(JPEG, &image));
                    true
                });
            }));
        }
        drop(viewers);
        Watching {
            hub: Arc::clone(self),
            id,
        }
    }

    /// Ask the engine for frames as often as the view that wants them most often.
    fn repace(screen: &Arc<dyn Screen>, viewers: &[(u64, Duration)]) {
        if let Some(every) = viewers.iter().map(|(_, every)| *every).min() {
            screen.pace(every);
        }
    }

    fn ended(&self) {
        self.latest.send_replace(Latest::Ended);
    }
}

/// One attached view's hold on its browser's frames: let go when its live bytes are.
struct Watching {
    hub: Arc<Hub>,
    id: u64,
}

impl Drop for Watching {
    fn drop(&mut self) {
        let mut viewers = self.hub.viewers();
        viewers.retain(|(id, _)| *id != self.id);
        if viewers.is_empty() {
            self.hub.screen.hide();
        } else {
            Hub::repace(&self.hub.screen, &viewers);
        }
    }
}

/// A browser as one view of it shows it: the latest frame, then each newer one, no more often
/// than `every`.
struct BrowserSource {
    hub: Arc<Hub>,
    every: Duration,
}

impl Source for BrowserSource {
    fn attach(&self) -> Attachment {
        let watching = self.hub.watched(self.every);
        let mut latest = self.hub.latest.subscribe();
        let snapshot = match &*latest.borrow_and_update() {
            Latest::Frame(framed) => framed.clone(),
            Latest::Nothing | Latest::Ended => Bytes::new(),
        };
        let every = self.every;
        let live = futures::stream::unfold(
            (latest, watching, None::<tokio::time::Instant>),
            move |(mut latest, watching, sent)| async move {
                loop {
                    latest.changed().await.ok()?;
                    if let Some(sent) = sent {
                        tokio::time::sleep_until(sent + every).await;
                    }
                    let next = latest.borrow_and_update().clone();
                    match next {
                        Latest::Frame(framed) => {
                            let now = tokio::time::Instant::now();
                            return Some((framed, (latest, watching, Some(now))));
                        }
                        Latest::Ended => return None,
                        Latest::Nothing => {}
                    }
                }
            },
        );
        Attachment {
            snapshot,
            live: live.boxed(),
        }
    }
}

/// The least time between frames a view asks for: its role's, or fewer still by its `rate`.
pub fn every(role: Option<ViewRole>, rate: Option<u32>) -> Duration {
    let by_role = match role {
        Some(ViewRole::Thumbnail) => THUMBNAIL_EVERY,
        Some(ViewRole::Focused) | None => Duration::ZERO,
    };
    match rate {
        Some(rate) => by_role.max(Duration::from_secs(1) / rate.max(1)),
        None => by_role,
    }
}

/// A browser, as the host runs it beside its terminals.
pub(crate) struct Browser {
    hub: Arc<Hub>,
}

impl Browser {
    pub(crate) fn new(screen: Arc<dyn Screen>) -> Browser {
        let (latest, _) = watch::channel(Latest::Nothing);
        Browser {
            hub: Arc::new(Hub {
                screen,
                latest,
                viewers: Mutex::new(Vec::new()),
                next: AtomicU64::new(1),
            }),
        }
    }
}

impl Running for Browser {
    fn write(&self, _bytes: &[u8]) -> Result<(), String> {
        Err("a browser is typed into by taking it over, not written to".into())
    }

    fn resize(&self, _size: purlis_terminal::engine::Size) -> Result<(), String> {
        Err("a browser is not resized as a terminal is".into())
    }

    fn source(&self) -> Arc<dyn Source> {
        Arc::new(BrowserSource {
            hub: Arc::clone(&self.hub),
            every: Duration::ZERO,
        })
    }

    fn view(
        &self,
        kind: ViewKind,
        role: Option<ViewRole>,
        rate: Option<u32>,
    ) -> Option<Arc<dyn Source>> {
        (kind == ViewKind::Browser).then(|| {
            Arc::new(BrowserSource {
                hub: Arc::clone(&self.hub),
                every: every(role, rate),
            }) as Arc<dyn Source>
        })
    }

    fn view_size(&self) -> Option<(u16, u16)> {
        Some(self.hub.screen.size())
    }

    fn act(&self, call_id: &str, act: Act) -> Result<Browsed, NotActed> {
        self.hub.screen.act(call_id, act)
    }

    fn stop(&self) {
        self.hub.screen.stop();
        self.hub.ended();
    }

    fn process(&self) -> Option<u32> {
        self.hub.screen.process()
    }

    fn readiness(&self) -> Readiness {
        Readiness {
            edits_lines: None,
            quiet_for_ms: 0,
        }
    }

    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>) {
        let hub = Arc::clone(&self.hub);
        self.hub.screen.when_it_ends(Box::new(move |exit| {
            hub.ended();
            tell(exit);
        }));
    }
}

#[cfg(test)]
mod tests;

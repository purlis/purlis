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
//! **Over a link between hosts a view is held to the link's frames** (#2050, spec #2041): a view
//! attached by `remote-link`, a runner's browser watched from its desktop, is sent at most
//! [`LINK_MOST_FRAMES_A_SECOND`] and frames at most [`LINK_MOST_FRAME_WIDTH`] wide, so a remote
//! browser stays smooth over a link that also carries what the person types. The engine is asked
//! for frames as wide as the widest view wants them ([`Screen::narrow`]). **Every browser view
//! keeps one frame in flight**: the next frame is sent once the client has drawn the last, and
//! it is the newest, so the person's input never waits behind a queue of frames
//! ([`purlis_session_protocol::view::Source::one_frame_in_flight`]).
//!
//! **A view's snapshot is the latest frame.** Each frame is framed whole on the view's stream
//! (`purlis_session_protocol::view::frames`). A view that falls behind is dropped and attached
//! again, as every view is, and its fresh snapshot is the latest frame: it starts on a whole
//! one. A view between frames is sent only the newest, never a backlog.
//!
//! **One driver at a time** (#2014). The host holds each browser's driver lease: the agent's
//! until the person takes the browser over, then the person's until they hand it back, and
//! nothing else returns it (not the window's link ending, not a crash of the window). While the
//! person drives, every act is refused in [`A_BROWSER_THE_PERSON_DRIVES`]'s words and only their
//! input reaches the browser; an act is carried out under the epoch of the lease it was checked
//! against, so one the take-over overtook is cut by the engine. While purlis performs a login
//! (#2016) the engine holds the lease itself, refusing the agent's acts until it hands the
//! browser back under a new epoch, which the host takes as the agent's lease from then on; a
//! take-over meanwhile cuts the login and leaves the browser the person's. A view of a 1.14
//! link is sent frames that name what they show (`view::frames::AIMED_JPEG`), which the
//! person's input names back.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::browsing::{
    Act, Aim, Browsed, BrowserReach, Driver, Input, KeptLogin, LogIn, origin_of,
};
use purlis_session_protocol::session::{self, Exit, Readiness, Refusal, ViewKind, ViewRole};
use purlis_session_protocol::view::frames::{JPEG, aimed, frame, origin};
use purlis_session_protocol::view::{Attachment, Source};
use tokio::sync::watch;

use crate::hosting::Running;
use crate::recording::Stopped;

/// Where a browser's engine hands each frame it makes: what it shows, the address of the
/// document its page has loaded as the engine records it (#2300), and a JPEG image of it. The
/// host keeps only the address's origin ([`origin_of`]).
pub type FrameSink = Arc<dyn Fn(Aim, String, Bytes) + Send + Sync>;

/// **How a view's link reads a browser's frames**: whether they name what they show (1.14),
/// whether it is sent the page's origin (1.35, #2300), and whether it is a link between hosts,
/// held to the link's frames (#2050).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reads {
    pub aims: bool,
    pub origins: bool,
    pub between_hosts: bool,
}

impl Reads {
    /// How a link of `scope` that agreed on `version` reads them: one between hosts is held to
    /// the link's frames (#2050).
    pub fn of(
        version: purlis_session_protocol::version::Version,
        scope: purlis_session_protocol::auth::Scope,
    ) -> Reads {
        use purlis_session_protocol::version::Version;
        Reads {
            aims: version
                >= Version {
                    major: 1,
                    minor: 14,
                },
            origins: version
                >= Version {
                    major: 1,
                    minor: 35,
                },
            between_hosts: scope == purlis_session_protocol::auth::Scope::RemoteLink,
        }
    }

    /// The least time between frames a view of this link that asks for `every` is sent, and the
    /// widest frame it is sent: over a link between hosts, the link's (#2050).
    fn held_to(self, every: Duration) -> (Duration, Option<u16>) {
        if self.between_hosts {
            (
                every.max(Duration::from_secs(1) / LINK_MOST_FRAMES_A_SECOND),
                Some(LINK_MOST_FRAME_WIDTH),
            )
        } else {
            (every, None)
        }
    }
}

/// **The most frames a second a view over a link between hosts is sent** (#2050, spec #2041's
/// default): a runner's browser watched from its desktop.
pub const LINK_MOST_FRAMES_A_SECOND: u32 = 15;

/// **The widest frame a view over a link between hosts is sent, in pixels** (#2050, spec
/// #2041's default): the engine scales a wider page down to it.
pub const LINK_MOST_FRAME_WIDTH: u16 = 1280;

/// The least time between two frames sent to a thumbnail: about one a second.
pub const THUMBNAIL_EVERY: Duration = Duration::from_secs(1);

/// A running browser, as the host drives it. Every call returns at once: one that has to wait
/// on the engine is carried out on the binary's own thread, never the host's runtime.
pub trait Screen: Send + Sync + 'static {
    /// Start making frames, each handed to `frames`.
    fn show(&self, frames: FrameSink);
    /// Make frames no more often than `every` (zero: as often as the page changes).
    fn pace(&self, every: Duration);
    /// Make frames no wider than `most` pixels, scaled down where its pages are wider, or as
    /// wide as its pages with none (#2050).
    fn narrow(&self, most: Option<u16>);
    /// Stop making frames.
    fn hide(&self);
    /// The size of its pages now, in CSS pixels: what a view's frames are drawn at.
    fn size(&self) -> (u16, u16);
    /// The engine's process, while it runs.
    fn process(&self) -> Option<u32>;
    /// End the engine and its whole process group; returns once they are gone.
    fn stop(&self);
    /// Calls `tell` once, as the engine ends, with how it ended (#2745).
    fn when_it_ends(&self, tell: Box<dyn FnOnce(ScreenEnded) + Send>);
    /// Carries out `act` under its own id `call_id` (#2013), as the agent's under the lease of
    /// `epoch` (#2014), and answers what the browser answered, or why it did not act: refused
    /// unless the agent holds that lease, and cut if the person takes the browser over before
    /// it answers. It waits on the engine, so the host calls it off its runtime.
    fn act(&self, call_id: &str, act: Act, epoch: u64) -> Result<Browsed, NotActed>;
    /// What it may reach from now on (#2018): its chat's reach and its browser sites as the
    /// window says they are now. One it cannot read is refused, and it reaches what it reached
    /// before.
    fn reach_now(&self, reach: &BrowserReach) -> Result<(), String>;
    /// The person takes it over (#2014): an act in flight is cut. Answers the new lease's
    /// epoch. Returns once no act of the agent's can reach a page.
    fn take_over(&self) -> u64;
    /// The person hands it back (#2014): every reference dealt before is void. Answers the new
    /// lease's epoch.
    fn hand_back(&self) -> u64;
    /// One event of the person's, while they drive (#2014). It may read the page first (to
    /// blank the field a key goes into), so the host calls it off its runtime.
    fn input(&self, input: &Input) -> Result<(), NotTaken>;
    /// The person sets its pages' size, in CSS pixels, while they drive (#2014).
    fn size_to(&self, width: u16, height: u16) -> Result<(), NotTaken>;
    /// purlis performs `login` (#2016), taking the browser from the agent and handing it back:
    /// who drives it after, under which epoch, and what the login came to. Refused, acting on
    /// nothing, while the person drives. It waits on the page, so the host calls it off its
    /// runtime.
    fn log_in(&self, login: &LogIn) -> Result<LoginEnded, NotActed>;
    /// The person opens the DevTools of the page it shows (#2028), their front end `width` by
    /// `height`, while they drive; `ended` is called once, as they close. Opened already, it
    /// only sizes them. It waits on the engine, so the host calls it off its runtime.
    fn inspect(
        &self,
        width: u16,
        height: u16,
        ended: Box<dyn FnOnce() + Send>,
    ) -> Result<(), NotTaken> {
        let _ = (width, height, ended);
        Err(NotTaken::Refused(NO_DEVTOOLS.into()))
    }
    /// Start making frames of its open DevTools, each handed to `frames` (#2028).
    fn show_inspector(&self, frames: FrameSink) {
        let _ = frames;
    }
    /// Make DevTools' frames no more often than `every`.
    fn pace_inspector(&self, every: Duration) {
        let _ = every;
    }
    /// Stop making DevTools' frames.
    fn hide_inspector(&self) {}
    /// One event of the person's into its open DevTools, while they drive (#2028).
    fn inspector_input(&self, input: &Input) -> Result<(), NotTaken> {
        let _ = input;
        Err(NotTaken::Missed)
    }
}

/// What `inspect` is refused with by a browser whose engine has no DevTools (#2028).
pub const NO_DEVTOOLS: &str = "this browser has no DevTools purlis can open";

/// **What makes a hub's frames**: a browser's pages, or its open DevTools (#2028).
trait Paints: Send + Sync + 'static {
    fn show(&self, frames: FrameSink);
    fn pace(&self, every: Duration);
    fn narrow(&self, most: Option<u16>);
    fn hide(&self);
}

/// A browser's pages, as a hub's frames.
struct Pages(Arc<dyn Screen>);

impl Paints for Pages {
    fn show(&self, frames: FrameSink) {
        self.0.show(frames);
    }
    fn pace(&self, every: Duration) {
        self.0.pace(every);
    }
    fn narrow(&self, most: Option<u16>) {
        self.0.narrow(most);
    }
    fn hide(&self) {
        self.0.hide();
    }
}

/// A browser's open DevTools, as a hub's frames: never narrowed, since they are only watched
/// in the person's own window.
struct DevTools(Arc<dyn Screen>);

impl Paints for DevTools {
    fn show(&self, frames: FrameSink) {
        self.0.show_inspector(frames);
    }
    fn pace(&self, every: Duration) {
        self.0.pace_inspector(every);
    }
    fn narrow(&self, _most: Option<u16>) {}
    fn hide(&self) {
        self.0.hide_inspector();
    }
}

/// **How a browser's engine ended** (#2745): its exit, and why, in words for the host's log that
/// hold nothing a page showed: who ended it, and why its DevTools connection closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenEnded {
    pub exit: Exit,
    pub why: String,
}

/// **The host's line for a browser's end** (#2745): which browser, the host's own act that ended
/// it where one did, and how its engine ended.
pub(crate) fn ended_said(program: &str, stopped: Option<Stopped>, ended: &ScreenEnded) -> String {
    let by_the_host = match stopped {
        Some(Stopped::Closed) => "closed at its link's ask",
        Some(Stopped::Operator) => "stopped at its link's ask",
        Some(Stopped::Quit) => "stopped as the host ended",
        Some(Stopped::Killed) => "stopped by the kill switch",
        None => "not stopped by the host",
    };
    format!("browser {program} ended ({by_the_host}): {}", ended.why)
}

/// **How a login purlis performed ended** (#2016): who drives the browser now (the agent,
/// handed it back, or the person, who took it over meanwhile), under which epoch, and the page
/// it left or why it did not go through, in words that carry no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginEnded {
    pub driver: Driver,
    pub epoch: u64,
    pub outcome: Result<Browsed, NotActed>,
}

/// Why a browser did not take the person's input or size (#2014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotTaken {
    /// The person does not drive it. Refused `not_driving`.
    NotDriving,
    /// The page no longer shows what the input was aimed at. Refused `input_missed`.
    Missed,
    /// The browser has ended. Refused `browser_ended`.
    Ended,
    /// The driver refused it, in words that say why.
    Refused(String),
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

/// What an act is refused with while the person drives the browser (#2014).
pub const A_BROWSER_THE_PERSON_DRIVES: &str = "the person has taken this browser over, so it takes no call from you: do something else, and use the browser again once they hand it back";

/// What the person's input is refused with before they take the browser over (#2014).
pub const NOT_DRIVING: &str = "the person is not driving this browser: take it over first";

/// What the person's input is refused with where the page has moved on (#2014).
pub const INPUT_MISSED: &str = "the page changed before this input reached it, so it was dropped";

/// **A browser's engine download, as it starts** (#2207): told how far the download has come,
/// and asked whether the person cancelled it. The binary tells it no more than a few times a
/// second; the host pushes each to the links that know of downloads (1.37).
#[derive(Clone)]
pub struct Download {
    told: Arc<dyn Fn(u64, Option<u64>) + Send + Sync>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl Download {
    /// A download whose progress is told to `told`: the bytes done, and of how many where that
    /// is known.
    pub fn telling(told: Arc<dyn Fn(u64, Option<u64>) + Send + Sync>) -> Self {
        Download {
            told,
            cancelled: Arc::default(),
        }
    }

    /// A download nobody watches, and nobody cancels.
    pub fn unwatched() -> Self {
        Download::telling(Arc::new(|_, _| {}))
    }

    /// The download has come to `done` bytes, of `total` where that is known.
    pub fn tell(&self, done: u64, total: Option<u64>) {
        (self.told)(done, total);
    }

    /// The person cancelled it: the next read of it fails, and nothing of it is kept.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether the person cancelled it.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

/// Starts browsers: the binary's, built on the engine and the core.
pub trait Browsers: Send + Sync + 'static {
    /// The folder each browser's data folder is made in, under the host's store. Asked as a
    /// browser starts, never before, so a host that starts none reads no store for them.
    fn data_folders(&self) -> Result<PathBuf, String>;
    /// **The most browsers this device runs at once** (#2611), read at each start: the host
    /// counts every browser it runs, whoever started it, and refuses a start past it
    /// `device_full`. `None`: no budget, as for a host whose binary starts none.
    fn budget(&self) -> Option<usize> {
        None
    }
    /// Start a browser whose data folder is `data` (made by the binary, this user's alone),
    /// reaching what `reach` says, or with none, nothing listed and every site new to the
    /// project refused (#2013); opened from the kept login `kept`, where one is named (#2021).
    /// Its engine's download, where the engine is not there yet, is told to `download`, and
    /// stops where it is cancelled (#2207), refused in [`ENGINE_DOWNLOAD_CANCELLED`]'s words.
    fn start(
        &self,
        data: &Path,
        reach: Option<&BrowserReach>,
        kept: Option<&KeptLogin>,
        download: &Download,
    ) -> Result<Arc<dyn Screen>, NotStarted>;
}

/// **Why a browser did not start**, as the binary says it (#2899): its engine cannot make its
/// sandbox on this machine, which the host refuses `no_engine_sandbox` (1.36), or anything
/// else, refused `not_started`. Each in words for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotStarted {
    /// The engine cannot make its sandbox here, and purlis never runs it without.
    NoEngineSandbox(String),
    /// It did not start, for the reason given.
    Failed(String),
}

impl From<String> for NotStarted {
    fn from(why: String) -> Self {
        NotStarted::Failed(why)
    }
}

impl From<&str> for NotStarted {
    fn from(why: &str) -> Self {
        NotStarted::Failed(why.to_owned())
    }
}

impl std::fmt::Display for NotStarted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotStarted::NoEngineSandbox(why) | NotStarted::Failed(why) => f.write_str(why),
        }
    }
}

/// What a browser's start is refused with where the person cancelled its engine's download
/// (#2207).
pub const ENGINE_DOWNLOAD_CANCELLED: &str = "the person cancelled the browser engine's download, \
     and nothing of it was kept";

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
        _kept: Option<&KeptLogin>,
        _download: &Download,
    ) -> Result<Arc<dyn Screen>, NotStarted> {
        Err("this session host was not given a way to start a browser".into())
    }
}

/// What a browser view is sent: nothing yet, the latest frame (what it shows, its page's
/// origin, and its image), or the end.
#[derive(Clone)]
enum Latest {
    Nothing,
    Frame(Aim, Option<String>, Bytes),
    Ended,
}

impl Latest {
    /// The latest frame, framed as a view whose link `reads` it: for one sent origins (1.35),
    /// after an origin frame where its origin is not `sent`, the one the view was sent last,
    /// which it becomes.
    fn framed(&self, reads: Reads, sent: &mut Option<Option<String>>) -> Option<Bytes> {
        let Latest::Frame(aim, at, image) = self else {
            return None;
        };
        let image = if reads.aims {
            aimed(aim, image)
        } else {
            frame(JPEG, image)
        };
        if !reads.origins || sent.as_ref() == Some(at) {
            return Some(image);
        }
        *sent = Some(at.clone());
        let mut both = bytes::BytesMut::from(&origin(at.as_deref())[..]);
        both.extend_from_slice(&image);
        Some(both.freeze())
    }
}

/// One browser's frames, and the views that want them.
struct Hub {
    screen: Arc<dyn Paints>,
    latest: watch::Sender<Latest>,
    /// Each attached view's least time between frames and widest frame, by its number here.
    viewers: Mutex<Vec<Viewer>>,
    next: AtomicU64,
}

/// One attached view, as its browser makes frames for it.
#[derive(Debug, Clone, Copy)]
struct Viewer {
    id: u64,
    every: Duration,
    most_width: Option<u16>,
}

impl Hub {
    fn viewers(&self) -> MutexGuard<'_, Vec<Viewer>> {
        self.viewers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A view that wants a frame no more often than `every`, and none wider than `most_width`,
    /// is attached.
    ///
    /// The engine is asked to show, pace and hide under the viewers' lock (each returns at once,
    /// [`Screen`]), so a view attached while the last one detaches is never left with its
    /// browser hidden.
    fn watched(self: &Arc<Self>, every: Duration, most_width: Option<u16>) -> Watching {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mut viewers = self.viewers();
        viewers.push(Viewer {
            id,
            every,
            most_width,
        });
        Self::repace(&self.screen, &viewers);
        if viewers.len() == 1 {
            let latest = self.latest.clone();
            self.screen
                .show(Arc::new(move |aim: Aim, address: String, image: Bytes| {
                    // Its origin alone, cut here: never a path, a query or a user.
                    let at = origin_of(&address);
                    latest.send_if_modified(|now| {
                        if matches!(now, Latest::Ended) {
                            return false;
                        }
                        *now = Latest::Frame(aim, at, image);
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

    /// Ask the engine for frames as often as the view that wants them most often, and as wide
    /// as the view that wants them widest: no narrower than its pages where one view wants
    /// them so.
    fn repace(screen: &Arc<dyn Paints>, viewers: &[Viewer]) {
        if let Some(every) = viewers.iter().map(|viewer| viewer.every).min() {
            screen.pace(every);
            let widest = viewers
                .iter()
                .map(|viewer| viewer.most_width)
                .try_fold(0, |widest, most| most.map(|most| most.max(widest)));
            screen.narrow(widest);
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
        viewers.retain(|viewer| viewer.id != self.id);
        if viewers.is_empty() {
            self.hub.screen.hide();
        } else {
            Hub::repace(&self.hub.screen, &viewers);
        }
    }
}

/// A browser as one view of it shows it: the latest frame, then each newer one, no more often
/// than `every`; each naming what it shows, and its page's origin where it changed, as the
/// view's link `reads` them.
struct BrowserSource {
    hub: Arc<Hub>,
    every: Duration,
    reads: Reads,
}

impl Source for BrowserSource {
    fn attach(&self) -> Attachment {
        let (every, most_width) = self.reads.held_to(self.every);
        let watching = self.hub.watched(every, most_width);
        let mut latest = self.hub.latest.subscribe();
        let reads = self.reads;
        // The snapshot says the page's origin whatever this view was sent before it: a view
        // attached again starts from nothing.
        let mut origin_sent = None;
        let snapshot = latest
            .borrow_and_update()
            .framed(reads, &mut origin_sent)
            .unwrap_or_default();
        let live = futures::stream::unfold(
            (latest, watching, None::<tokio::time::Instant>, origin_sent),
            move |(mut latest, watching, sent, mut origin_sent)| async move {
                loop {
                    latest.changed().await.ok()?;
                    if let Some(sent) = sent {
                        tokio::time::sleep_until(sent + every).await;
                    }
                    let next = latest.borrow_and_update().clone();
                    match next {
                        Latest::Frame(..) => {
                            let framed = next.framed(reads, &mut origin_sent).unwrap_or_default();
                            let now = tokio::time::Instant::now();
                            return Some((framed, (latest, watching, Some(now), origin_sent)));
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

    /// The newest frame once the last is drawn: never a queue of older ones (#2050).
    fn one_frame_in_flight(&self) -> bool {
        true
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
    screen: Arc<dyn Screen>,
    /// Its open DevTools' frames and their size, while the person has them open (#2028).
    inspector: Arc<Mutex<Option<OpenDevTools>>>,
    /// Its id in the host, and the host's own act that ends it, for the line its end leaves in
    /// the host's log (#2745).
    program: String,
    stopping: Arc<Mutex<Option<Stopped>>>,
    /// Who drives it, and the epoch of their lease (#2014): the host's, so no link's end
    /// changes it.
    lease: Mutex<(Driver, u64)>,
}

impl Browser {
    pub(crate) fn new(
        screen: Arc<dyn Screen>,
        program: impl Into<String>,
        stopping: Arc<Mutex<Option<Stopped>>>,
    ) -> Browser {
        Browser {
            hub: Hub::of(Arc::new(Pages(Arc::clone(&screen)))),
            inspector: Arc::new(Mutex::new(None)),
            screen,
            lease: Mutex::new((Driver::Agent, 0)),
            program: program.into(),
            stopping,
        }
    }

    fn lease(&self) -> MutexGuard<'_, (Driver, u64)> {
        self.lease
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn inspector(&self) -> MutexGuard<'_, Option<OpenDevTools>> {
        self.inspector
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Its open DevTools' frames end, and the views of them with them.
    fn devtools_closed(&self) {
        if let Some((hub, _)) = self.inspector().take() {
            hub.ended();
        }
    }
}

impl Hub {
    fn of(screen: Arc<dyn Paints>) -> Arc<Hub> {
        let (latest, _) = watch::channel(Latest::Nothing);
        Arc::new(Hub {
            screen,
            latest,
            viewers: Mutex::new(Vec::new()),
            next: AtomicU64::new(1),
        })
    }
}

/// A browser's open DevTools: their frames, and the size they are drawn at (#2028).
type OpenDevTools = (Arc<Hub>, (u16, u16));

/// DevTools of `hub` closed: their views end, and `slot` forgets them where it still holds them.
fn closed(slot: &Mutex<Option<OpenDevTools>>, hub: &Arc<Hub>) {
    let mut open = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if open.as_ref().is_some_and(|(now, _)| Arc::ptr_eq(now, hub)) {
        *open = None;
    }
    drop(open);
    hub.ended();
}

/// The refusal the person's input or size is answered with.
fn not_taken(why: NotTaken) -> Refusal {
    let (code, why) = match why {
        NotTaken::NotDriving => (session::NOT_DRIVING, NOT_DRIVING.to_owned()),
        NotTaken::Missed => (session::INPUT_MISSED, INPUT_MISSED.to_owned()),
        NotTaken::Ended => (session::BROWSER_ENDED, AN_ENDED_BROWSER.to_owned()),
        NotTaken::Refused(why) => (session::BROWSER_REFUSED, why),
    };
    Refusal {
        code: code.into(),
        why,
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
            reads: Reads::default(),
        })
    }

    fn view(
        &self,
        kind: ViewKind,
        role: Option<ViewRole>,
        rate: Option<u32>,
        reads: Reads,
    ) -> Option<Arc<dyn Source>> {
        let hub = match kind {
            ViewKind::Browser => Arc::clone(&self.hub),
            // DevTools are watched only while open (#2028).
            ViewKind::Inspector => Arc::clone(&self.inspector().as_ref()?.0),
            ViewKind::Terminal => return None,
        };
        Some(Arc::new(BrowserSource {
            hub,
            every: every(role, rate),
            reads,
        }) as Arc<dyn Source>)
    }

    fn view_size(&self) -> Option<(u16, u16)> {
        Some(self.screen.size())
    }

    fn view_size_of(&self, kind: ViewKind) -> Option<(u16, u16)> {
        match kind {
            ViewKind::Inspector => self.inspector().as_ref().map(|(_, size)| *size),
            _ => self.view_size(),
        }
    }

    fn reach_now(&self, reach: &BrowserReach) -> Result<(), String> {
        self.screen.reach_now(reach)
    }

    fn act(&self, call_id: &str, act: Act) -> Result<Browsed, NotActed> {
        let epoch = match *self.lease() {
            (Driver::Person, _) => {
                return Err(NotActed::Refused(A_BROWSER_THE_PERSON_DRIVES.into()));
            }
            (Driver::Agent, epoch) => epoch,
        };
        self.screen.act(call_id, act, epoch)
    }

    fn log_in(&self, login: LogIn) -> Result<Browsed, NotActed> {
        let epoch = match *self.lease() {
            (Driver::Person, _) => {
                return Err(NotActed::Refused(A_BROWSER_THE_PERSON_DRIVES.into()));
            }
            (Driver::Agent, epoch) => epoch,
        };
        let ended = self.screen.log_in(&login)?;
        drop(login);
        // The lease the engine handed back is the agent's from now on, unless the person took
        // the browser over meanwhile: their take-over set the host's lease already.
        let mut lease = self.lease();
        if *lease == (Driver::Agent, epoch) {
            *lease = (ended.driver, ended.epoch);
        }
        ended.outcome
    }

    fn take_over(&self) -> Result<Driver, Refusal> {
        let mut lease = self.lease();
        if lease.0 != Driver::Person {
            *lease = (Driver::Person, self.screen.take_over());
        }
        Ok(Driver::Person)
    }

    fn hand_back(&self) -> Result<Driver, Refusal> {
        let mut lease = self.lease();
        if lease.0 != Driver::Agent {
            *lease = (Driver::Agent, self.screen.hand_back());
        }
        drop(lease);
        // DevTools are the person's while they drive (#2028).
        self.devtools_closed();
        Ok(Driver::Agent)
    }

    fn inspect(&self, width: u16, height: u16) -> Result<(), Refusal> {
        if self.lease().0 != Driver::Person {
            return Err(not_taken(NotTaken::NotDriving));
        }
        let mut open = self.inspector();
        if let Some((hub, size)) = open.as_mut() {
            let hub = Arc::clone(hub);
            let slot = Arc::clone(&self.inspector);
            self.screen
                .inspect(width, height, Box::new(move || closed(&slot, &hub)))
                .map_err(not_taken)?;
            *size = (width, height);
            return Ok(());
        }
        let hub = Hub::of(Arc::new(DevTools(Arc::clone(&self.screen))));
        let slot = Arc::clone(&self.inspector);
        let theirs = Arc::clone(&hub);
        // Held while the engine opens them, so an end it says at once finds them here.
        *open = Some((Arc::clone(&hub), (width, height)));
        drop(open);
        if let Err(why) =
            self.screen
                .inspect(width, height, Box::new(move || closed(&slot, &theirs)))
        {
            closed(&self.inspector, &hub);
            return Err(not_taken(why));
        }
        Ok(())
    }

    fn inspector_input(&self, input: &Input) -> Result<(), Refusal> {
        if self.lease().0 != Driver::Person {
            return Err(not_taken(NotTaken::NotDriving));
        }
        self.screen.inspector_input(input).map_err(not_taken)
    }

    fn input(&self, input: &Input) -> Result<(), Refusal> {
        if self.lease().0 != Driver::Person {
            return Err(not_taken(NotTaken::NotDriving));
        }
        self.screen.input(input).map_err(not_taken)
    }

    fn size_browser(&self, width: u16, height: u16) -> Result<(), Refusal> {
        if self.lease().0 != Driver::Person {
            return Err(not_taken(NotTaken::NotDriving));
        }
        self.screen.size_to(width, height).map_err(not_taken)
    }

    fn driver(&self) -> Option<Driver> {
        Some(self.lease().0)
    }

    fn stop(&self) {
        self.screen.stop();
        self.hub.ended();
        self.devtools_closed();
    }

    fn process(&self) -> Option<u32> {
        self.screen.process()
    }

    fn readiness(&self) -> Readiness {
        Readiness {
            edits_lines: None,
            quiet_for_ms: 0,
        }
    }

    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>) {
        let hub = Arc::clone(&self.hub);
        let program = self.program.clone();
        let stopping = Arc::clone(&self.stopping);
        self.screen.when_it_ends(Box::new(move |ended| {
            hub.ended();
            let stopped = *stopping
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            crate::say(&ended_said(&program, stopped, &ended));
            tell(ended.exit);
        }));
    }
}

#[cfg(test)]
mod tests;

//! The host a paired phone talks to (iOS design §4.3, §5.2, §5.5): the protocol crate's
//! `session::Host`, answered from the app's own seam, [`crate::host::SessionHost`] and
//! [`crate::host::ChatBoard`], one `Board` per link.
//!
//! A chat is named on the wire by its id (`reopen::Identity::id`), a project by its stable id
//! (`[project].id`, V76), and a chat without an id yet is not listed: it has nothing a phone
//! could name it by. Every answer or prompt from the phone is recorded as metadata through
//! the board's [`Audit`], never with its text (D9).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bytes::Bytes;
use purlis_core::harness::asks::{Admitted, Answerer, AskId, Raised};
use purlis_core::harness::model::ChoiceScope;
use purlis_core::remote::audit::Act;
use purlis_session_protocol::session::{
    self, Answer, Attached, Chat, Command, Host, NO_SUCH_CHAT, NOT_AN_AGENT, Peer, Refusal,
    TimelineAttached,
};
use purlis_session_protocol::view::{Attacher, Feed, Limits, ViewId};

use crate::planes::{Held, PlaneId, Planes};

/// Where the planes are: the app's managed state, or a test's own.
pub trait Opened: Send + Sync + 'static {
    /// Calls `f` with the planes, or not at all when the app has none yet.
    fn with_planes(&self, f: &mut dyn FnMut(&Planes));
}

impl Opened for Arc<Planes> {
    fn with_planes(&self, f: &mut dyn FnMut(&Planes)) {
        f(self);
    }
}

impl Opened for tauri::AppHandle {
    fn with_planes(&self, f: &mut dyn FnMut(&Planes)) {
        use tauri::Manager;
        if let Some(planes) = self.try_state::<Planes>() {
            f(&planes);
        }
    }
}

/// Which phone this board serves, for the audit.
#[derive(Debug, Clone)]
pub struct DeviceOf {
    pub key: [u8; 32],
    pub device: String,
}

/// Where the board's acts go: the audit (`remote.*` events), or nowhere in a test.
pub type Audit = Arc<dyn Fn(Act) + Send + Sync>;

/// An audit that records nothing: for a test of the board alone.
#[cfg(test)]
pub fn silent() -> Audit {
    Arc::new(|_| {})
}

/// One chat as the board found it: where it is, and how much the host knows of it.
#[derive(Debug, Clone)]
pub(super) struct Found {
    pub(super) plane: PlaneId,
    pub(super) session: u32,
    pub(super) level: u8,
}

/// Where a view's bytes go: a buffer until the protocol's view is open, then its feed.
enum Slot {
    Buffering(Vec<Bytes>),
    Feeding(Feed),
}

/// The app's own view behind one the phone was told of, and where its bytes go.
struct AppView {
    plane: PlaneId,
    session: u32,
    view: u32,
    slot: Arc<Mutex<Slot>>,
}

impl AppView {
    /// Whether the protocol's view closed under it (the phone fell behind): the app's view is
    /// then open for nothing, and the next attach of the chat closes it.
    fn closed(&self) -> bool {
        matches!(&*lock(&self.slot), Slot::Feeding(feed) if feed.closed().is_some())
    }
}

/// Closes the app's view `view` of `session` on `plane` unless disarmed: what an attach that
/// is cut short (the link dropping while the protocol's view opens) leaves behind otherwise.
struct Unwatch<'a> {
    board: &'a Board,
    plane: PlaneId,
    session: u32,
    view: u32,
    armed: bool,
}

impl Drop for Unwatch<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let (plane, session, view) = (self.plane.clone(), self.session, self.view);
        self.board.planes(|planes| {
            if let Ok(held) = planes.held(&plane) {
                let _ = held.chats().sessions().unwatch(session, view);
            }
        });
    }
}

struct Inner {
    opened: Arc<dyn Opened>,
    device: DeviceOf,
    audit: Audit,
    /// The phone's views, by chat and the view number the phone was told, to the app's own.
    views: Mutex<HashMap<(String, u32), AppView>>,
    /// Timelines the phone attached, by chat and number, each a task that ends on detach.
    timelines: Mutex<HashMap<(String, u32), tokio::task::JoinHandle<()>>>,
    /// Each plane's stable project id, read once per board.
    projects: Mutex<HashMap<PlaneId, String>>,
    /// The asks this phone answered, so its timelines say so when each closes. Shared with
    /// the timelines' tasks, which must hold nothing that keeps this alive.
    answered: Arc<Mutex<HashSet<String>>>,
    next_number: Mutex<u32>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // The link ended: every view it held open in the app closes with it.
        let views: Vec<_> = lock(&self.views).drain().map(|(_, to)| to).collect();
        for AppView {
            plane,
            session,
            view,
            ..
        } in views
        {
            self.opened.with_planes(&mut |planes| {
                if let Ok(held) = planes.held(&plane) {
                    let _ = held.chats().sessions().unwatch(session, view);
                }
            });
        }
        for (_, task) in lock(&self.timelines).drain() {
            task.abort();
        }
    }
}

/// The host, cheap to clone: one per link.
#[derive(Clone)]
pub struct Board(Arc<Inner>);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn refuse(code: &str, why: impl Into<String>) -> Refusal {
    Refusal {
        code: code.into(),
        why: why.into(),
    }
}

/// The chat's state as the protocol words it (V76): `working`, `needs_you`, `idle`, `ended`.
fn state_word(state: purlis_core::state::State, asking: bool) -> &'static str {
    use purlis_core::state::State;
    match (state, asking) {
        (_, true) => "needs_you",
        (State::Running, _) => "working",
        (State::Waiting | State::Unknown, _) => "idle",
        (State::Done | State::Failed, _) => "ended",
    }
}

impl Board {
    pub fn new(opened: Arc<dyn Opened>, device: DeviceOf, audit: Audit) -> Board {
        Board(Arc::new(Inner {
            opened,
            device,
            audit,
            views: Mutex::new(HashMap::new()),
            timelines: Mutex::new(HashMap::new()),
            projects: Mutex::new(HashMap::new()),
            answered: Arc::new(Mutex::new(HashSet::new())),
            next_number: Mutex::new(0),
        }))
    }

    /// The asks this phone answered, shared: what a timeline reads to say who closed one,
    /// without holding the board itself alive.
    pub(super) fn answered_handle(&self) -> Arc<Mutex<HashSet<String>>> {
        Arc::clone(&self.0.answered)
    }

    /// The host's event log — its directory and this device's id — where the app has one.
    /// One per process, so the first open plane's is every plane's.
    pub(super) fn event_log(&self) -> Option<(PathBuf, String)> {
        self.planes(|planes| {
            planes
                .open_now()
                .into_iter()
                .filter_map(|plane| planes.held(&plane).ok())
                .find_map(|held| held.hooks().event_log())
        })
        .flatten()
    }

    fn record(&self, act: Act) {
        (self.0.audit)(act);
    }

    fn next_number(&self) -> u32 {
        let mut next = lock(&self.0.next_number);
        *next += 1;
        *next
    }

    /// `f` over the planes, or `None` when the app has none yet.
    fn planes<R>(&self, f: impl FnOnce(&Planes) -> R) -> Option<R> {
        let mut f = Some(f);
        let mut out = None;
        self.0.opened.with_planes(&mut |planes| {
            if let Some(f) = f.take() {
                out = Some(f(planes));
            }
        });
        out
    }

    /// `f` over the plane `found` is on, or [`NO_SUCH_CHAT`] when that plane is closed.
    pub(super) fn with_held<R>(
        &self,
        found: &Found,
        f: impl FnOnce(&Held) -> R,
    ) -> Result<R, Refusal> {
        self.planes(|planes| planes.held(&found.plane).ok().map(|held| f(&held)))
            .flatten()
            .ok_or_else(|| refuse(NO_SUCH_CHAT, "the chat's project is no longer open"))
    }

    fn project_id(&self, plane: &PlaneId, held: &Held) -> String {
        if let Some(id) = lock(&self.0.projects).get(plane) {
            return id.clone();
        }
        let id = held.project_id().unwrap_or_default();
        if !id.is_empty() {
            lock(&self.0.projects).insert(plane.clone(), id.clone());
        }
        id
    }

    /// Every chat a phone may name, with what the board knows of it.
    pub(super) fn found(&self) -> Vec<(Chat, Found)> {
        self.planes(|planes| {
            let mut out = Vec::new();
            for plane in planes.open_now() {
                let Ok(held) = planes.held(&plane) else {
                    continue;
                };
                let project = self.project_id(&plane, &held);
                let project_path = held.root().to_string_lossy().into_owned();
                for open in held.chats().open_now() {
                    let Some(id) = held.chats().chat_at(open.session).and_then(|at| at.id) else {
                        continue;
                    };
                    let level = match open.harness {
                        None => 1,
                        Some(_) => 2,
                    };
                    let glance = held.board().glance(open.session);
                    out.push((
                        Chat {
                            chat: id,
                            project: project.clone(),
                            project_path: Some(project_path.clone()),
                            title: Some(open.name.clone()),
                            state: state_word(glance.state, glance.asking).to_owned(),
                            workspace: open.workspace.clone(),
                            level: Some(level),
                        },
                        Found {
                            plane: plane.clone(),
                            session: open.session,
                            level,
                        },
                    ));
                }
            }
            out
        })
        .unwrap_or_default()
    }

    pub(super) fn find(&self, chat: &str) -> Result<Found, Refusal> {
        self.found()
            .into_iter()
            .find(|(c, _)| c.chat == chat)
            .map(|(_, found)| found)
            .ok_or_else(|| refuse(NO_SUCH_CHAT, format!("no chat {chat} on this host")))
    }

    /// The open ask `ask` of `chat`, if it is still open.
    fn ask_of(&self, chat: &str, ask: &str) -> Option<Raised> {
        let found = self.find(chat).ok()?;
        let id: AskId = ask.parse().ok()?;
        self.with_held(&found, |held| {
            held.hooks()
                .asks()
                .pending(std::time::Instant::now())
                .into_iter()
                .find(|raised| raised.id == id && raised.chat == found.session.to_string())
        })
        .ok()
        .flatten()
    }

    /// Open a view of `chat` for the phone: the app's own view, whose sink feeds the
    /// protocol's. What the app's view sent before the protocol's was open — the screen as it
    /// already is — becomes the snapshot.
    async fn attach(&self, chat: String, peer: &Peer) -> Result<Answer, Refusal> {
        let found = self.find(&chat)?;
        // A view of this chat the phone fell behind on is open in the app for nothing: it
        // closes now, before a fresh one opens, so a re-attach never adds a view.
        let fallen: Vec<(String, u32)> = lock(&self.0.views)
            .iter()
            .filter(|((of, _), app)| *of == chat && app.closed())
            .map(|(key, _)| key.clone())
            .collect();
        for (chat, view) in fallen {
            self.detach(chat, view);
        }
        let number = self.next_number();
        let slot = Arc::new(Mutex::new(Slot::Buffering(Vec::new())));
        let feeding = Arc::clone(&slot);
        let watching = self
            .with_held(&found, |held| {
                held.chats().sessions().watch(
                    found.session,
                    Box::new(move |text: String| match &mut *lock(&feeding) {
                        Slot::Buffering(buffer) => buffer.push(Bytes::from(text)),
                        Slot::Feeding(feed) => {
                            // A view that fell behind is closed by its feed; the phone attaches
                            // it again for a fresh snapshot, and this view closes then.
                            let _ = feed.push(Bytes::from(text));
                        }
                    }),
                )
            })?
            .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
        // Until the protocol's view is open, the app's is closed again if this is cut short.
        let mut guard = Unwatch {
            board: self,
            plane: found.plane.clone(),
            session: found.session,
            view: watching.view,
            armed: true,
        };
        // What the app's view sent so far — the screen as it already is — is the snapshot,
        // taken out of the buffer so nothing in it is sent twice.
        let snapshot = match &mut *lock(&slot) {
            Slot::Buffering(buffer) => std::mem::take(buffer).concat(),
            Slot::Feeding(_) => Vec::new(),
        };
        let attacher = Attacher::new(peer.views().clone(), Limits::default());
        let feed = attacher
            .attach(ViewId(number), Bytes::from(snapshot))
            .await
            .map_err(|e| refuse("link_failed", e.to_string()))?;
        {
            let mut slot = lock(&slot);
            if let Slot::Buffering(buffer) = &mut *slot {
                for late in buffer.drain(..) {
                    let _ = feed.push(late);
                }
            }
            *slot = Slot::Feeding(feed);
        }
        guard.armed = false;
        lock(&self.0.views).insert(
            (chat, number),
            AppView {
                plane: found.plane.clone(),
                session: found.session,
                view: watching.view,
                slot,
            },
        );
        Ok(Answer::Attached(Attached {
            view: number,
            cols: watching.size.columns,
            rows: watching.size.rows,
        }))
    }

    fn detach(&self, chat: String, view: u32) {
        if let Some(app) = lock(&self.0.views).remove(&(chat, view)) {
            self.planes(|planes| {
                if let Ok(held) = planes.held(&app.plane) {
                    let _ = held.chats().sessions().unwatch(app.session, app.view);
                }
            });
        }
    }
}

impl Host for Board {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::List => Ok(Answer::Chats(
                self.found().into_iter().map(|(c, _)| c).collect(),
            )),
            Command::Attach { chat } => self.attach(chat, peer).await,
            Command::Detach { chat, view } => {
                self.detach(chat, view);
                Ok(Answer::Done)
            }
            // Through the app's own entry point, as the window's keys go: a pending curation
            // paste is dropped first, a smart close is cancelled, a task's report says the
            // person stepped in (SI-2, #1442).
            Command::Write { chat, bytes } => {
                let found = self.find(&chat)?;
                self.with_held(&found, |held| held.operator_input(found.session, &bytes))?
                    .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                Ok(Answer::Done)
            }
            Command::Prompt { chat, text } => {
                let found = self.find(&chat)?;
                if found.level < 2 {
                    return Err(refuse(
                        NOT_AN_AGENT,
                        "a shell tab takes keystrokes, not a prompt",
                    ));
                }
                let pasted = format!("{}\r", purlis_core::curation::bracketed(&text));
                self.with_held(&found, |held| {
                    held.operator_input(found.session, pasted.as_bytes())
                })?
                .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                self.record(Act::Prompted {
                    device: self.0.device.device.clone(),
                    key: self.0.device.key,
                    chat,
                    bytes: text.len(),
                });
                Ok(Answer::Done)
            }
            Command::Answer { chat, ask, answer } => {
                let found = self.find(&chat)?;
                let by = Answerer::admitted(Admitted::RemoteDevice).expect("a human scope");
                // Through the plane, as the window's answer goes: the board and the window are
                // told, and the needs-you item clears (#1525, #1644).
                self.with_held(&found, |held| {
                    held.answer_ask_as(found.session, &ask, &answer, by)
                })?
                .map_err(|why| {
                    refuse(
                        "ask_closed",
                        format!("{why}; it may have been answered on the desktop"),
                    )
                })?;
                lock(&self.0.answered).insert(ask.clone());
                self.record(Act::Answered {
                    device: self.0.device.device.clone(),
                    key: self.0.device.key,
                    chat,
                    ask,
                    choice: answer,
                });
                Ok(Answer::Done)
            }
            // The plane's own close, as the window's ×: the board is told, the chat above a
            // task hears it stopped, and nothing waits on a report that never comes (#1443).
            Command::Stop { chat } => {
                let found = self.find(&chat)?;
                self.with_held(&found, |held| held.close_chat(found.session))?
                    .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                Ok(Answer::Done)
            }
            Command::AttachTimeline { chat } => {
                let found = self.find(&chat)?;
                let number = self.next_number();
                let task =
                    super::timeline::attach(self, peer.views().clone(), &found, number).await?;
                lock(&self.0.timelines).insert((chat, number), task);
                Ok(Answer::TimelineAttached(TimelineAttached {
                    timeline: number,
                    level: found.level,
                }))
            }
            Command::DetachTimeline { chat, timeline } => {
                if let Some(task) = lock(&self.0.timelines).remove(&(chat, timeline)) {
                    task.abort();
                }
                Ok(Answer::Done)
            }
            // Refused by the grants table before they reach here; answered for completeness.
            Command::Resize { .. } | Command::Start(_) => {
                Err(refuse(session::NOT_ALLOWED, "a device never does this"))
            }
            Command::Subscribe { .. } | Command::Unsubscribe => Ok(Answer::Done),
        }
    }

    /// An ask that is not open is taken not to: its id was minted once, so it can never open
    /// again under that id, and the answer that follows is refused with the real reason (it
    /// was answered on the desktop, timed out, or never was) rather than as not allowed.
    async fn elicits_a_secret(&self, chat: &str, ask: &str) -> bool {
        self.ask_of(chat, ask)
            .is_some_and(|raised| raised.ask.elicits_secret)
    }

    /// A choice the open ask does not offer is taken to last: fail closed. An ask that is not
    /// open is taken not to, for [`Self::elicits_a_secret`]'s reason.
    async fn choice_lasts(&self, chat: &str, ask: &str, answer: &str) -> bool {
        self.ask_of(chat, ask).is_some_and(|raised| {
            raised
                .ask
                .options
                .iter()
                .find(|choice| choice.id == answer)
                .is_none_or(|choice| choice.scope == ChoiceScope::Always)
        })
    }

    async fn subscribe(
        &self,
        since: session::Since,
        _peer: &Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, Refusal> {
        super::timeline::subscribe(self.clone(), since).await
    }
}

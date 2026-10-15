// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session protocol: what a control lane's frames mean (FD-26, ADR 0068 §4).
//!
//! **It is small, public and versioned.** A handful of commands (list, start a hosted
//! program, attach and detach a view of a kind, write, resize, answer, stop, subscribe and
//! unsubscribe) and the events the host pushes. It is the half of the control lane that carries the compatibility promise (E5): a
//! client and a host one version apart (N−1) always talk. `charter attach`, the fleet MCP and
//! a runner's link are its clients, and LV-2a publishes it on its own.
//!
//! **The other half is the UI RPC** ([`crate::ui`]): what the app's window needs beyond this,
//! private to one build of the app and excluded from the compatibility promise. Both ride the
//! same control lane, each frame tagged with which it is, so a link carries both side by side.
//!
//! **Its version is the link's.** [`speaks`] is what a client offers and a host answers with
//! ([`crate::version`]), so the version negotiated before admission is this protocol's. A
//! minor only adds: a command, a field, an event kind. So a reader ignores fields it does not
//! know, and a host answers a command it does not know with [`UNKNOWN_COMMAND`] and carries on,
//! which is how a newer client learns an older host lacks it. A new major is a break, and a
//! host keeps speaking the major before its own.
//!
//! **On the wire,** each control frame is one JSON object with one key saying what it is:
//!
//! - `{"call":{"id":7,"command":"stop","chat":"<ULID>"}}`, a command, from the client;
//! - `{"reply":{"re":7,"ok":"done"}}` or `{"reply":{"re":7,"refused":{"code":"…","why":"…"}}}`,
//!   its answer, from the host, in any order relative to other answers ([`serve`] answers a
//!   link's calls side by side, and keeps the order of the commands about one chat);
//! - `{"pushed":{"event":{…}}}` or `{"pushed":{"missed":{…}}}`, from the host to a client that
//!   subscribed: ADR 0066's envelope, or word that events were missed, never a silent gap;
//! - `{"ui":{…}}`, a frame of the UI RPC ([`crate::ui`]).
//!
//! Every word on the wire is snake_case: keys, command words, answer kinds, refusal codes and
//! the words a value carries, such as a chat's state (`needs_you`). The one exception is a
//! pushed event's `kind`, which keeps ADR 0066's dotted names (`run.started`). A project is named by its
//! stable id (`[project] id` in its `charter.toml`), never by a path. The whole format is
//! recorded in ADR 0068, *Amended by FD-26* (V76).
//!
//! Terminal bytes never cross as JSON: an attach opens a view ([`crate::view`]) on a stream of
//! its own, and only the bytes typed into a chat ride the lane, in base64.
//!
//! **The host holds hosted programs, each with views of a kind** (ADR 0068, amended for
//! #1994). A chat's harness and a shell are hosted programs, and so is a browser from spec 2;
//! a terminal view is one kind of view, and a browser view another. So the commands say "start
//! a hosted program" ([`Command::StartProgram`]) and "attach a view of kind X"
//! ([`Command::AttachView`]), never "start a chat". The 1.0 words a client one version behind
//! still sends, `start`, `attach` and `detach`, are carried out as those, and answered in their
//! own words: a [`Host`] never hears them.
//!
//! `tests/fixtures/session-protocol.jsonl` records a frame of every kind this version says.
//! A test reads each back as it was recorded, so a change that would stop an older peer
//! reading one fails CI: that file is the compatibility promise, written down. Each variant's
//! word comes from an exhaustive `match` ([`Command::word`] and its kin), so a new variant does
//! not compile without one, and fails that test until its frame is recorded.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use bytes::Bytes;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

use crate::auth::Scope;
use crate::grants::Power;
use crate::link::{Acceptor, Link, LinkError, Opener};
use crate::ui;
use crate::version::{Speaks, Version};
use crate::view::{Limits, Views};

/// The session protocol's current version.
///
/// 1.1 added hosted programs and views of a kind (#1994): `start_program`, `attach_view`,
/// `detach_view`, the `program_started` answer and an `attached` view's `kind`.
pub const VERSION: Version = Version { major: 1, minor: 1 };

/// What a client offers and a host answers with: this version, and from the second major on,
/// the major before it too (N−1).
pub fn speaks() -> Speaks {
    Speaks::new([VERSION])
}

/// Every command word this version knows: [`Command::WORDS`].
pub const COMMANDS: &[&str] = Command::WORDS;

/// Gives an enum its wire words from ONE list: `word()`, an exhaustive `match`, so a variant
/// added without a word does not compile, and `WORDS`, every word in the same order. The words
/// are what serde writes (`rename_all = "snake_case"`), and a test holds the two to each other
/// over every recorded frame.
macro_rules! wire_words {
    ($ty:ident { $($variant:ident => $word:literal),* $(,)? }) => {
        impl $ty {
            /// Every word this version says for this kind of frame.
            pub const WORDS: &'static [&'static str] = &[$($word),*];

            /// This variant's word on the wire.
            pub fn word(&self) -> &'static str {
                match self {
                    $($ty::$variant { .. } => $word,)*
                }
            }
        }
    };
}

/// A command this host does not know: a newer client's, which it can do without.
pub const UNKNOWN_COMMAND: &str = "unknown_command";
/// A command this host knows, sent in a shape it cannot read.
pub const MALFORMED: &str = "malformed";
/// The chat named is not one the host holds.
pub const NO_SUCH_CHAT: &str = "no_such_chat";
/// The project named is not one the host holds.
pub const NO_SUCH_PROJECT: &str = "no_such_project";
/// The link's scope may not do this (FD-27).
pub const NOT_ALLOWED: &str = "not_allowed";

/// A command, as the client sends it. Unknown fields are ignored, so a later minor may add
/// some.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    /// The chats the host holds.
    List,
    /// 1.0's word for [`Command::AttachView`] of kind [`ViewKind::Terminal`]. Never reaches a
    /// [`Host`].
    Attach { chat: String },
    /// 1.0's word for [`Command::DetachView`]. Never reaches a [`Host`].
    Detach { chat: String, view: u32 },
    /// Bytes typed into a chat's terminal, as they are: text, or a mouse report that is not
    /// text at all.
    Write {
        chat: String,
        #[serde(with = "base64_bytes")]
        bytes: Vec<u8>,
    },
    /// A chat's terminal has a new size.
    Resize { chat: String, cols: u16, rows: u16 },
    /// The answer to one of a chat's asks (a needs-you). Only a human scope answers one
    /// (`local-ui` and `approval`), and an ask that elicits a secret only `local-ui`
    /// ([`crate::grants`]).
    Answer {
        chat: String,
        ask: String,
        answer: String,
    },
    /// End a chat and everything it started.
    Stop { chat: String },
    /// 1.0's word for [`Command::StartProgram`] of kind [`ProgramKind::Harness`], answered with
    /// [`Answer::Started`]. Never reaches a [`Host`].
    Start(Start),
    /// Start a hosted program of a kind, and answer [`Answer::ProgramStarted`] with its id.
    StartProgram(StartProgram),
    /// Open a view of `kind` on a hosted program: the host opens a stream for it
    /// ([`crate::view::Views::open`]), whose snapshot then live bytes follow, and answers
    /// [`Answer::Attached`] with the view's `kind`.
    AttachView { program: String, kind: ViewKind },
    /// Close a view the client attached. The program keeps running.
    DetachView { program: String, view: u32 },
    /// Every event after `since`, then each as it is written ([`Pushed`]): ADR 0066's
    /// `subscribe(since)`.
    Subscribe { since: Since },
    /// Stop pushing events.
    Unsubscribe,
}

/// What a [`Command::Start`] asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    /// The project the chat is in, by its stable id: the ULID in its `charter.toml`
    /// (`[project].id`, V76). Never a path, which differs from machine to machine.
    pub project: String,
    /// Its workspace, or the project's root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Its harness, or the project's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
}

/// What a [`Command::StartProgram`] asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartProgram {
    /// The project it is in, by its stable id ([`Start::project`]).
    pub project: String,
    /// What kind of hosted program it is.
    pub kind: ProgramKind,
    /// Its workspace, or the project's root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// For a [`ProgramKind::Harness`], which harness, or the project's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
}

/// What kind of program a session host holds. A later minor adds one (a browser, spec 2); a
/// host that does not know a kind refuses the command it came in as `malformed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramKind {
    /// A chat's harness.
    Harness,
    /// A shell, with no harness.
    Shell,
}

/// What kind of view a client attaches: what its stream's snapshot and live bytes are. A
/// later minor adds one (a browser view, spec 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// A terminal's bytes: the snapshot draws its scrollback, screen, cursor and modes.
    Terminal,
}

/// A subscription's cursor: for each device, the last `seq` the client holds of it (ADR 0066).
/// Empty is from the start.
pub type Since = BTreeMap<String, u64>;

/// What the host answers a command with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// The chats it holds: [`Command::List`].
    Chats(Vec<Chat>),
    /// The view it opened: [`Command::Attach`].
    Attached(Attached),
    /// The chat it started: 1.0's answer to [`Command::Start`].
    Started { chat: String },
    /// The hosted program it started: [`Command::StartProgram`].
    ProgramStarted { program: String },
    /// Events follow, as [`Pushed`] frames: [`Command::Subscribe`].
    Subscribed,
    /// Done, with nothing to say.
    Done,
}

/// One chat, as [`Command::List`] lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    /// Its id, a ULID (ADR 0066).
    pub chat: String,
    /// The project it is in, by its stable id ([`Start::project`]).
    pub project: String,
    /// Where that project is on the host's machine. A hint to show a person, never a key: the
    /// same project is at another path on another machine (V76).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    /// What the window calls it, where it has a name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its state as its hooks report it, as a snake_case word (`working`, `needs_you`, `idle`,
    /// `ended`); kebab-case is UI text only (V76). A word and not a closed set, so a later
    /// minor may add one.
    pub state: String,
}

/// The view an attach opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attached {
    /// The view's number, which its stream's header carries ([`crate::view::ViewId`]).
    pub view: u32,
    /// The size its snapshot was drawn for.
    pub cols: u16,
    pub rows: u16,
    /// Its kind, in the answer to [`Command::AttachView`]. Absent from 1.0's answer to
    /// [`Command::Attach`], whose view is a terminal's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ViewKind>,
}

/// Why the host refused a command: a code a program reads, and a sentence a person reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{why} ({code})")]
pub struct Refusal {
    pub code: String,
    pub why: String,
}

/// What the host pushes to a client that subscribed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pushed {
    Event(Event),
    /// The events of `device` after `after` up to `resumes_at` are no longer kept: the stream
    /// carries on from `resumes_at`, and a client rebuilds what it built from them.
    ///
    /// **It carries no snapshot (V76).** A client that missed events and shows a chat's
    /// terminal attaches it again ([`Command::Attach`]): the view's own stream opens with the
    /// terminal's snapshot, so there is one way to get a screen back, for a fallen-behind view,
    /// a reconnect and a gap alike. Doing so is the client's duty.
    Missed {
        device: String,
        after: u64,
        resumes_at: u64,
    },
}

wire_words!(Command {
    List => "list",
    Attach => "attach",
    Detach => "detach",
    Write => "write",
    Resize => "resize",
    Answer => "answer",
    Stop => "stop",
    Start => "start",
    StartProgram => "start_program",
    AttachView => "attach_view",
    DetachView => "detach_view",
    Subscribe => "subscribe",
    Unsubscribe => "unsubscribe",
});

impl Command {
    /// What a scope must be granted to send this command ([`crate::grants`]). An answer's power
    /// depends on the ask, which only the host knows, so it is [`Power::Answer`] here and
    /// [`serve`] asks the host whether it is a secret's.
    pub fn power(&self) -> Power {
        match self {
            Command::List => Power::List,
            Command::Attach { .. }
            | Command::Detach { .. }
            | Command::AttachView { .. }
            | Command::DetachView { .. } => Power::View,
            Command::Write { .. } => Power::Write,
            Command::Resize { .. } => Power::Resize,
            Command::Answer { .. } => Power::Answer,
            Command::Stop { .. } => Power::Stop,
            Command::Start(_) | Command::StartProgram(_) => Power::Start,
            Command::Subscribe { .. } | Command::Unsubscribe => Power::Subscribe,
        }
    }
}

wire_words!(Answer {
    Chats => "chats",
    Attached => "attached",
    Started => "started",
    ProgramStarted => "program_started",
    Subscribed => "subscribed",
    Done => "done",
});

wire_words!(Frame {
    Call => "call",
    Reply => "reply",
    Pushed => "pushed",
    Ui => "ui",
});

wire_words!(Outcome {
    Ok => "ok",
    Refused => "refused",
});

wire_words!(Pushed {
    Event => "event",
    Missed => "missed",
});

/// One event, in ADR 0066's envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    pub device_id: String,
    pub seq: u64,
    pub ulid: String,
    pub chat: Option<String>,
    pub run: Option<String>,
    pub parent_run: Option<String>,
    pub kind: String,
    pub body: serde_json::Value,
}

/// One frame of the control lane: the session protocol's, or the UI RPC's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frame {
    Call(Call),
    Reply(Reply),
    Pushed(Pushed),
    Ui(ui::Frame),
}

/// A command and the number its answer comes back under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub id: u64,
    #[serde(flatten)]
    pub command: Command,
}

/// The answer to the call numbered `re`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub re: u64,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok(Answer),
    Refused(Refusal),
}

impl Frame {
    pub(crate) fn to_bytes(&self) -> Bytes {
        Bytes::from(serde_json::to_vec(self).expect("a frame is always JSON"))
    }
}

/// Who a command came from, as the host's [`Host`] sees it.
pub struct Peer {
    scope: Scope,
    version: Version,
    opener: Opener,
    views: Views,
}

impl Peer {
    /// The scope the link was admitted as ([`crate::auth`]).
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// The version the link agreed on.
    pub fn version(&self) -> Version {
        self.version
    }

    /// The link's views, of any kind: where an attach opens one ([`Views::open`]) and a detach
    /// closes it. Every one is closed when the link ends, before [`Host::link_ended`].
    pub fn views(&self) -> &Views {
        &self.views
    }

    /// What opens a stream on the link, for a host that holds its views itself
    /// ([`crate::view::Attacher::new`]).
    ///
    /// **One numberer of views per link.** A view's number is what a client tells its views
    /// apart by, so a link's views are numbered by [`Peer::views`] or by the host's own
    /// [`crate::view::Attacher`], never by both: a host that opens views itself on a link opens
    /// none through [`Peer::views`] on it.
    pub fn opener(&self) -> &Opener {
        &self.opener
    }
}

/// What answers the session protocol: the host (FD-5).
pub trait Host: Send + Sync + 'static {
    /// Answer one command. [`Command::Subscribe`] and [`Command::Unsubscribe`] never come
    /// here: they go to [`Host::subscribe`], and the link holds the subscription. Nor do 1.0's
    /// [`Command::Start`], [`Command::Attach`] and [`Command::Detach`]: they come as
    /// [`Command::StartProgram`], [`Command::AttachView`] and [`Command::DetachView`].
    fn call(
        &self,
        command: Command,
        peer: &Peer,
    ) -> impl Future<Output = Result<Answer, Refusal>> + Send;

    /// Whether `chat`'s ask `ask` elicits a secret, which only `local-ui` may answer (V75).
    /// A host that does not say is taken to say yes, so the check fails closed.
    fn elicits_a_secret(&self, chat: &str, ask: &str) -> impl Future<Output = bool> + Send {
        let _ = (chat, ask);
        async { true }
    }

    /// The link `peer` came on has ended (the client closed it, or died). By now every view
    /// [`Peer::views`] holds on it has been cancelled, and its stream ends where it is, whatever
    /// it had queued; what the link started runs on. Calls it sent may still be carried out
    /// after this ([`serve`]).
    fn link_ended(&self, peer: &Peer) -> impl Future<Output = ()> + Send {
        let _ = peer;
        async {}
    }

    /// Every event after `since`, then each as it is written, until the receiver is dropped.
    fn subscribe(
        &self,
        since: Since,
        peer: &Peer,
    ) -> impl Future<Output = Result<mpsc::Receiver<Pushed>, Refusal>> + Send {
        let _ = (since, peer);
        async {
            Err(Refusal {
                code: UNKNOWN_COMMAND.into(),
                why: "this host keeps no events".into(),
            })
        }
    }
}

/// The most calls one link has in flight: taken up by the host and not yet answered, the
/// session protocol's and the UI RPC's together. Past it the host reads no further call from
/// the link until one is answered, so the rest wait in the link, whose own flow control holds
/// the client back: no call is refused or dropped for it, and the host holds a bounded number
/// of them. It matches [`crate::link::MOST_UNACCEPTED_STREAMS`], the link's other bound on
/// what one peer may leave open, and is far more than a window sends at once.
pub const MOST_CALLS_IN_FLIGHT: usize = 32;

/// Serve the session protocol, and the UI RPC beside it when `ui` is given, on a link the
/// host admitted, until the client closes it.
///
/// **A link's calls are answered side by side** (#1081), each on a task of its own, and each
/// answer is sent when it is ready: replies come in any order, matched by `re`. So a command
/// that waits (a `start` that waits on a harness) holds back neither another command's answer
/// nor a pushed event, and neither does a UI RPC command that waits (#1095).
///
/// **What has an order keeps it.** The commands about one chat are carried out one after
/// another, in the order they came: keystrokes are sent call after call without waiting for
/// each answer, and one must not overtake another. `subscribe` and `unsubscribe` keep theirs
/// too. Commands about different chats, and those about none (`list`, `start`), wait for
/// nobody. A client that needs one of those after another waits for the first one's answer.
///
/// **At most [`MOST_CALLS_IN_FLIGHT`] at once**, counting those waiting their chat's turn.
///
/// **When the link ends** (the client closes it, or dies and the transport closes), every view
/// on it is closed at once ([`Views::end`]) and the host is told ([`Host::link_ended`]).
///
/// **A command the host took up is carried out** even when its link ends first: `serve`
/// returns once each has finished, and what a link asked for is not undone by its closing (a
/// `start` cut off halfway would leave a chat nobody holds). A host that must stop at once
/// drops this future, which stops them where they wait.
pub async fn serve<H: Host>(
    mut link: Link,
    host: H,
    ui: Option<ui::Server>,
) -> Result<(), LinkError> {
    let peer = Arc::new(Peer {
        scope: link.scope(),
        version: link.version(),
        opener: link.opener(),
        views: Views::new(link.opener(), Limits::default()),
    });
    let host = Arc::new(host);
    let mut calls = Calls::default();
    let served = answer(&mut link, &host, &peer, ui::Serving::new(ui), &mut calls).await;
    // The client is gone: nobody watches its views, so they close now, not once a view's
    // queue has filled; then the host is told.
    peer.views().end();
    host.link_ended(&peer).await;
    // Nobody is left to hear these answers; the commands are carried out all the same.
    while calls.next().await.is_some() {}
    served
}

/// [`serve`]'s loop: read a call and start it, send an answer that is ready, push an event.
async fn answer<H: Host>(
    link: &mut Link,
    host: &Arc<H>,
    peer: &Arc<Peer>,
    mut ui: ui::Serving,
    calls: &mut Calls,
) -> Result<(), LinkError> {
    let mut events: Option<mpsc::Receiver<Pushed>> = None;
    loop {
        let next = tokio::select! {
            frame = link.control().next(), if !calls.full() => Next::Frame(frame),
            Some(finished) = calls.next() => Next::Finished(finished),
            pushed = next_pushed(&mut events) => Next::Pushed(pushed),
        };
        let answer = match next {
            Next::Pushed(Some(pushed)) => Some(Frame::Pushed(pushed)),
            Next::Pushed(None) => {
                events = None;
                None
            }
            Next::Finished(Finished::Answered(frame)) => Some(frame),
            Next::Finished(Finished::Subscribed(id, Ok(receiver))) => {
                events = Some(receiver);
                Some(reply(id, Ok(Answer::Subscribed)))
            }
            Next::Finished(Finished::Subscribed(id, Err(refusal))) => Some(reply(id, Err(refusal))),
            Next::Finished(Finished::Unsubscribed(id)) => {
                events = None;
                Some(reply(id, Ok(Answer::Done)))
            }
            Next::Frame(None) => return Ok(()),
            Next::Frame(Some(frame)) => {
                let frame = frame?;
                match serde_json::from_slice::<Frame>(&frame) {
                    Ok(Frame::Call(Call { id, command })) => {
                        let (host, peer) = (Arc::clone(host), Arc::clone(peer));
                        calls.take(
                            Lane::of(&command),
                            Box::pin(async move { carry_out(id, command, &*host, &peer).await }),
                        );
                        None
                    }
                    Ok(Frame::Ui(frame)) => match ui.answer(frame, peer) {
                        ui::Answered::Now(frame) => frame.map(Frame::Ui),
                        ui::Answered::Later(answer) => {
                            calls.take(
                                None,
                                Box::pin(
                                    async move { Finished::Answered(Frame::Ui(answer.await)) },
                                ),
                            );
                            None
                        }
                    },
                    // Only a host sends these; a client that does is ignored, not believed.
                    Ok(Frame::Reply(_) | Frame::Pushed(_)) => None,
                    Err(_) => unreadable_call(&frame),
                }
            }
        };
        if let Some(answer) = answer {
            link.control().send(answer.to_bytes()).await?;
        }
    }
}

/// Carry out one call: the grants' check first ([`refused`]), then the host.
async fn carry_out<H: Host>(id: u64, command: Command, host: &H, peer: &Peer) -> Finished {
    if let Some(refusal) = refused(&command, peer, host).await {
        return Finished::Answered(reply(id, Err(refusal)));
    }
    match command {
        // The link holds the subscription, so its loop takes these two up when they finish.
        Command::Subscribe { since } => Finished::Subscribed(id, host.subscribe(since, peer).await),
        Command::Unsubscribe => Finished::Unsubscribed(id),
        // 1.0's words: carried out as the hosted-program commands, answered in their own.
        Command::Start(Start {
            project,
            workspace,
            harness,
        }) => {
            let start = Command::StartProgram(StartProgram {
                project,
                kind: ProgramKind::Harness,
                workspace,
                harness,
            });
            let answer = host.call(start, peer).await.map(|answer| match answer {
                Answer::ProgramStarted { program } => Answer::Started { chat: program },
                other => other,
            });
            Finished::Answered(reply(id, answer))
        }
        Command::Attach { chat } => {
            let attach = Command::AttachView {
                program: chat,
                kind: ViewKind::Terminal,
            };
            let answer = host.call(attach, peer).await.map(|answer| match answer {
                Answer::Attached(attached) => Answer::Attached(Attached {
                    kind: None,
                    ..attached
                }),
                other => other,
            });
            Finished::Answered(reply(id, answer))
        }
        Command::Detach { chat, view } => {
            let detach = Command::DetachView {
                program: chat,
                view,
            };
            Finished::Answered(reply(id, host.call(detach, peer).await))
        }
        command => Finished::Answered(reply(id, host.call(command, peer).await)),
    }
}

fn reply(re: u64, answer: Result<Answer, Refusal>) -> Frame {
    Frame::Reply(Reply {
        re,
        outcome: match answer {
            Ok(answer) => Outcome::Ok(answer),
            Err(refusal) => Outcome::Refused(refusal),
        },
    })
}

/// What a call's task ends with.
enum Finished {
    /// Its answer, ready to send.
    Answered(Frame),
    /// A `subscribe` the host answered: the link holds the receiver.
    Subscribed(u64, Result<mpsc::Receiver<Pushed>, Refusal>),
    /// An `unsubscribe`: the link lets go of the receiver.
    Unsubscribed(u64),
}

/// What a link's calls keep their order within ([`serve`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Lane {
    /// The commands about one hosted program (a chat is one).
    Program(String),
    /// `subscribe` and `unsubscribe`.
    Subscription,
}

impl Lane {
    /// The lane `command` keeps its order in, or `None` when it waits for nobody. An
    /// exhaustive `match`, so a command added later says which it is.
    fn of(command: &Command) -> Option<Lane> {
        match command {
            Command::List | Command::Start(_) | Command::StartProgram(_) => None,
            Command::Attach { chat: program }
            | Command::Detach { chat: program, .. }
            | Command::AttachView { program, .. }
            | Command::DetachView { program, .. }
            | Command::Write { chat: program, .. }
            | Command::Resize { chat: program, .. }
            | Command::Answer { chat: program, .. }
            | Command::Stop { chat: program } => Some(Lane::Program(program.clone())),
            Command::Subscribe { .. } | Command::Unsubscribe => Some(Lane::Subscription),
        }
    }
}

type Work = BoxFuture<'static, Finished>;

/// The calls one link has in flight: each running on a task of its own, and behind each lane's
/// running call, the later ones of that lane in the order they came.
#[derive(Default)]
struct Calls {
    running: JoinSet<(Option<Lane>, Finished)>,
    /// A lane is here while one of its calls runs, with those waiting their turn behind it.
    waiting: HashMap<Lane, VecDeque<Work>>,
    in_flight: usize,
}

impl Calls {
    fn full(&self) -> bool {
        self.in_flight >= MOST_CALLS_IN_FLIGHT
    }

    /// Take up one call: run it now, or behind its lane's earlier ones.
    fn take(&mut self, lane: Option<Lane>, work: Work) {
        self.in_flight += 1;
        let Some(lane) = lane else {
            return self.run(None, work);
        };
        match self.waiting.get_mut(&lane) {
            Some(behind) => behind.push_back(work),
            None => {
                self.waiting.insert(lane.clone(), VecDeque::new());
                self.run(Some(lane), work);
            }
        }
    }

    fn run(&mut self, lane: Option<Lane>, work: Work) {
        self.running.spawn(async move { (lane, work.await) });
    }

    /// The next call to finish, or `None` when none is in flight. Its lane's next call starts.
    /// A host that panicked answering a call panics here, as it did when `serve` answered each
    /// call itself: the link ends, and nothing is answered in the host's name.
    async fn next(&mut self) -> Option<Finished> {
        let (lane, finished) = match self.running.join_next().await? {
            Ok(done) => done,
            Err(gone) => match gone.try_into_panic() {
                Ok(panic) => std::panic::resume_unwind(panic),
                // Stopped with the runtime: nothing of this link runs on.
                Err(_) => {
                    self.waiting.clear();
                    self.in_flight = self.running.len();
                    return None;
                }
            },
        };
        self.in_flight -= 1;
        if let Some(lane) = lane {
            match self.waiting.get_mut(&lane).and_then(VecDeque::pop_front) {
                Some(work) => self.run(Some(lane), work),
                None => {
                    self.waiting.remove(&lane);
                }
            }
        }
        Some(finished)
    }
}

/// Why `peer` may not send `command`, by [`crate::grants`]' table, or `None` when it may. Asked
/// once, here, before any host hears of the command.
async fn refused<H: Host>(command: &Command, peer: &Peer, host: &H) -> Option<Refusal> {
    let scope = peer.scope();
    let word = command.word();
    if !scope.may(command.power()) {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!("a `{scope}` link may not `{word}`"),
        });
    }
    if let Command::Answer { chat, ask, .. } = command
        && !scope.may(Power::AnswerASecret)
        && host.elicits_a_secret(chat, ask).await
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "this ask elicits a secret, which is answered in purlis's window only, and \
                 this link is `{scope}`"
            ),
        });
    }
    None
}

enum Next {
    Frame(Option<Result<Bytes, LinkError>>),
    Finished(Finished),
    Pushed(Option<Pushed>),
}

async fn next_pushed(events: &mut Option<mpsc::Receiver<Pushed>>) -> Option<Pushed> {
    match events {
        Some(events) => events.recv().await,
        None => std::future::pending().await,
    }
}

/// A frame this host cannot read. A call it can find the number of is refused, so the client
/// hears why and the link carries on: a command from a later minor is [`UNKNOWN_COMMAND`]. Any
/// other is a frame kind from a later version, and is ignored.
fn unreadable_call(frame: &[u8]) -> Option<Frame> {
    let value: serde_json::Value = serde_json::from_slice(frame).ok()?;
    let call = value.get("call")?;
    let id = call.get("id")?.as_u64()?;
    let word = call.get("command").and_then(|c| c.as_str());
    let refusal = match word {
        Some(word) if COMMANDS.contains(&word) => Refusal {
            code: MALFORMED.into(),
            why: format!("a `{word}` command this host cannot read"),
        },
        Some(word) => Refusal {
            code: UNKNOWN_COMMAND.into(),
            why: format!("this host does not know the command `{word}`"),
        },
        None => Refusal {
            code: MALFORMED.into(),
            why: "a call that names no command".into(),
        },
    };
    Some(Frame::Reply(Reply {
        re: id,
        outcome: Outcome::Refused(refusal),
    }))
}

/// Why a call got no answer.
#[derive(Debug, thiserror::Error)]
pub enum CallError {
    /// The host refused it.
    #[error(transparent)]
    Refused(#[from] Refusal),
    /// The link ended before the answer came.
    #[error("the link ended before the host answered: {0}")]
    Closed(String),
    /// The host answered with something that is not an answer to this command.
    #[error("the host answered {0}, which is not an answer to this command")]
    Unexpected(String),
}

impl CallError {
    /// The host's refusal, when that is what this is.
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            CallError::Refused(refusal) => Some(refusal),
            _ => None,
        }
    }
}

/// What a client asks its link's driver to do.
pub(crate) enum Ask {
    Session(Command, oneshot::Sender<Result<Answer, CallError>>),
    Ui(ui::Ask),
}

/// The client's end of the session protocol: calls from any task, answered in any order.
pub struct Client {
    asks: mpsc::UnboundedSender<Ask>,
    version: Version,
    scope: Scope,
}

/// What a subscription brings a client, in order: each thing the host pushed, and word of
/// where the client fell behind ([`Told`]). It holds [`MOST_EVENTS_QUEUED`] of them at the
/// most, or what [`Client::holding`] was given.
pub type Events = mpsc::Receiver<Told>;

/// What a client reads from its [`Events`].
///
/// This is the client's own word to its reader, made on the client's side of the link: it is
/// not a frame, and nothing of it crosses the wire.
// Nearly every one is the large variant, an event: boxing it would cost an allocation an event
// to save space in the one word of falling behind a queue ever holds.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Told {
    /// What the host pushed.
    Pushed(Pushed),
    /// The reader fell behind: it left more unread than the queue holds, so the client let go
    /// of the subscription ([`Command::Unsubscribe`]) and kept nothing after `since`, which
    /// is, for each device, the last `seq` this queue did keep (or the one the subscription
    /// was made from, for a device nothing was kept of). Every event after it is what was
    /// missed, and [`Client::subscribe`] from `since` brings each of them back, or the host's
    /// word that it no longer keeps some ([`Pushed::Missed`]). Never a silent gap, and never
    /// a queue that grows (#1081).
    FellBehind { since: Since },
}

/// The most events a client's queue holds for a reader that is not reading. A burst that a
/// reader takes a moment over fits; a reader that has stopped holds this many and no more. It
/// counts events, each of which is one control frame
/// ([`crate::link::MOST_CONTROL_FRAME_BYTES`] at the most).
pub const MOST_EVENTS_QUEUED: usize = 1024;

impl Client {
    /// Speak the session protocol on `link`. Answers with the client, the streams the host
    /// opens (each attach's view, [`crate::view::Viewer::accept`]), and the events it pushes
    /// once subscribed.
    pub fn new(link: Link) -> (Client, Acceptor, Events) {
        Client::holding(link, MOST_EVENTS_QUEUED)
    }

    /// [`Client::new`], with a queue that holds `most_events` (one at the least) for a reader
    /// that is not reading, where [`Client::new`] holds [`MOST_EVENTS_QUEUED`].
    pub fn holding(mut link: Link, most_events: usize) -> (Client, Acceptor, Events) {
        let views = link.acceptor();
        let (asks, asked) = mpsc::unbounded_channel();
        let (pushed, events) = Subscribed::queue(most_events);
        let client = Client {
            asks,
            version: link.version(),
            scope: link.scope(),
        };
        tokio::spawn(drive(link, asked, pushed));
        (client, views, events)
    }

    /// The version the link agreed on.
    pub fn version(&self) -> Version {
        self.version
    }

    /// The scope the link was admitted as.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Send one command and wait for its answer.
    pub async fn call(&self, command: Command) -> Result<Answer, CallError> {
        let (reply, answer) = oneshot::channel();
        self.asks
            .send(Ask::Session(command, reply))
            .map_err(|_| CallError::Closed("the link is closed".into()))?;
        answer
            .await
            .map_err(|_| CallError::Closed("the link is closed".into()))?
    }

    pub async fn list(&self) -> Result<Vec<Chat>, CallError> {
        match self.call(Command::List).await? {
            Answer::Chats(chats) => Ok(chats),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn attach(&self, chat: &str) -> Result<Attached, CallError> {
        match self.call(Command::Attach { chat: chat.into() }).await? {
            Answer::Attached(attached) => Ok(attached),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn detach(&self, chat: &str, view: u32) -> Result<(), CallError> {
        self.done(Command::Detach {
            chat: chat.into(),
            view,
        })
        .await
    }

    pub async fn write(&self, chat: &str, bytes: &[u8]) -> Result<(), CallError> {
        self.done(Command::Write {
            chat: chat.into(),
            bytes: bytes.to_vec(),
        })
        .await
    }

    pub async fn resize(&self, chat: &str, cols: u16, rows: u16) -> Result<(), CallError> {
        self.done(Command::Resize {
            chat: chat.into(),
            cols,
            rows,
        })
        .await
    }

    pub async fn answer(&self, chat: &str, ask: &str, answer: &str) -> Result<(), CallError> {
        self.done(Command::Answer {
            chat: chat.into(),
            ask: ask.into(),
            answer: answer.into(),
        })
        .await
    }

    pub async fn stop(&self, chat: &str) -> Result<(), CallError> {
        self.done(Command::Stop { chat: chat.into() }).await
    }

    /// Start a chat, and answer with its id.
    pub async fn start(&self, start: Start) -> Result<String, CallError> {
        match self.call(Command::Start(start)).await? {
            Answer::Started { chat } => Ok(chat),
            other => Err(unexpected(&other)),
        }
    }

    /// Start a hosted program, and answer with its id. A host one version behind refuses it
    /// `unknown_command`; [`Client::start`] is its word for a harness.
    pub async fn start_program(&self, start: StartProgram) -> Result<String, CallError> {
        match self.call(Command::StartProgram(start)).await? {
            Answer::ProgramStarted { program } => Ok(program),
            other => Err(unexpected(&other)),
        }
    }

    /// Attach a view of `kind` on `program`: its stream comes on the [`Acceptor`]
    /// [`Client::new`] gave. A host one version behind refuses it `unknown_command`;
    /// [`Client::attach`] is its word for a terminal view.
    pub async fn attach_view(&self, program: &str, kind: ViewKind) -> Result<Attached, CallError> {
        let attach = Command::AttachView {
            program: program.into(),
            kind,
        };
        match self.call(attach).await? {
            Answer::Attached(attached) => Ok(attached),
            other => Err(unexpected(&other)),
        }
    }

    /// Close a view this client attached. The program keeps running.
    pub async fn detach_view(&self, program: &str, view: u32) -> Result<(), CallError> {
        self.done(Command::DetachView {
            program: program.into(),
            view,
        })
        .await
    }

    /// Subscribe from `since`: the events come on the [`Events`] [`Client::new`] gave. A
    /// reader that leaves too many unread is told so there ([`Told::FellBehind`]), and
    /// subscribes again from the cursor it is given.
    pub async fn subscribe(&self, since: Since) -> Result<(), CallError> {
        match self.call(Command::Subscribe { since }).await? {
            Answer::Subscribed => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn unsubscribe(&self) -> Result<(), CallError> {
        self.done(Command::Unsubscribe).await
    }

    /// Open the UI RPC beside the session protocol, as one build of the app: the host serves
    /// it only to `local-ui`, and only to its own build ([`crate::ui`]).
    pub async fn ui(&self, build: &str) -> Result<ui::Client, CallError> {
        ui::Client::open(self.asks.clone(), build).await
    }

    async fn done(&self, command: Command) -> Result<(), CallError> {
        match self.call(command).await? {
            Answer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }
}

fn unexpected(answer: &Answer) -> CallError {
    CallError::Unexpected(serde_json::to_string(answer).unwrap_or_default())
}

/// The client's side of its subscription: the bounded queue its reader reads, and where the
/// reader would read on from.
struct Subscribed {
    queue: mpsc::Sender<Told>,
    /// Whether the host's pushes are being kept: from a `subscribe` the host answered until
    /// the reader falls behind.
    live: bool,
    /// For each device, the last `seq` queued, or the one subscribed from.
    cursor: Since,
    /// The cursor of each `subscribe` not yet answered, by its call's number.
    asked: HashMap<u64, Since>,
}

impl Subscribed {
    /// A queue of `most` events (one at the least), and one place more: the last place is
    /// kept for the word that the reader fell behind, so that word always fits, in its order.
    fn queue(most: usize) -> (Subscribed, Events) {
        let (queue, events) = mpsc::channel(most.max(1).saturating_add(1));
        let subscribed = Subscribed {
            queue,
            live: false,
            cursor: Since::new(),
            asked: HashMap::new(),
        };
        (subscribed, events)
    }

    /// The host answered the call numbered `re`: when that is a `subscribe` it accepted, its
    /// events are kept from here on, and the cursor is the one it was made from.
    fn answered(&mut self, re: u64, accepted: bool) {
        if let Some(since) = self.asked.remove(&re)
            && accepted
        {
            self.cursor = since;
            self.live = true;
        }
    }

    /// Queue what the host pushed. Answers `true` when the reader has just fallen behind, and
    /// the host is to be told to stop: the queue has only its last place left, which takes the
    /// word of it. What comes after that is not kept, and is asked for again from the cursor.
    fn pushed(&mut self, pushed: Pushed) -> bool {
        if !self.live {
            return false;
        }
        if self.queue.capacity() > 1 {
            match &pushed {
                Pushed::Event(event) => self.cursor.insert(event.device_id.clone(), event.seq),
                // The gap is told by this very frame: reading on from before it would only
                // tell it again.
                Pushed::Missed {
                    device, resumes_at, ..
                } => self
                    .cursor
                    .insert(device.clone(), resumes_at.saturating_sub(1)),
            };
            let _ = self.queue.try_send(Told::Pushed(pushed));
            return false;
        }
        self.live = false;
        // With no place left at all, the last thing queued is the word from the time before,
        // nothing has been read or kept since, and its cursor is still where to read on from.
        let _ = self.queue.try_send(Told::FellBehind {
            since: self.cursor.clone(),
        });
        true
    }
}

/// The client's driver: it numbers each call, sends it, and hands each answer to whoever
/// waits for it, and each pushed event to [`Events`], which it holds to a bound.
async fn drive(mut link: Link, mut asked: mpsc::UnboundedReceiver<Ask>, mut pushed: Subscribed) {
    let mut next_id: u64 = 1;
    let mut waiting: HashMap<u64, oneshot::Sender<Result<Answer, CallError>>> = HashMap::new();
    let mut ui = ui::Waiting::default();
    let why = loop {
        let next = tokio::select! {
            ask = asked.recv() => Driven::Ask(ask),
            frame = link.control().next() => Driven::Frame(frame),
        };
        match next {
            // Every handle is gone: nobody is left to answer.
            Driven::Ask(None) => return,
            Driven::Ask(Some(ask)) => {
                let id = next_id;
                next_id += 1;
                let frame = match ask {
                    Ask::Session(command, reply) => {
                        if let Command::Subscribe { since } = &command {
                            pushed.asked.insert(id, since.clone());
                        }
                        waiting.insert(id, reply);
                        Frame::Call(Call { id, command })
                    }
                    Ask::Ui(ask) => Frame::Ui(ui.ask(id, ask)),
                };
                if let Err(e) = link.control().send(frame.to_bytes()).await {
                    break e.to_string();
                }
            }
            Driven::Frame(None) => break "the host closed the link".to_owned(),
            Driven::Frame(Some(Err(e))) => break e.to_string(),
            Driven::Frame(Some(Ok(frame))) => match serde_json::from_slice::<Frame>(&frame) {
                Ok(Frame::Reply(Reply { re, outcome })) => {
                    pushed.answered(re, outcome == Outcome::Ok(Answer::Subscribed));
                    if let Some(reply) = waiting.remove(&re) {
                        let _ = reply.send(match outcome {
                            Outcome::Ok(answer) => Ok(answer),
                            Outcome::Refused(refusal) => Err(CallError::Refused(refusal)),
                        });
                    }
                }
                Ok(Frame::Pushed(event)) => {
                    if pushed.pushed(event) {
                        // The reader fell behind: the host is told to stop pushing, in a call
                        // of the driver's own, whose answer nobody waits for. It is sent
                        // before any call made on reading that word, so a subscription made
                        // again on it comes after.
                        let id = next_id;
                        next_id += 1;
                        let stop = Frame::Call(Call {
                            id,
                            command: Command::Unsubscribe,
                        });
                        if let Err(e) = link.control().send(stop.to_bytes()).await {
                            break e.to_string();
                        }
                    }
                }
                Ok(Frame::Ui(frame)) => ui.answered(frame),
                // A reply this client cannot read fails its call, rather than leaving it to
                // wait for an answer that has come.
                Err(e) => {
                    if let Some(re) = unreadable_reply(&frame) {
                        pushed.answered(re, false);
                        if let Some(reply) = waiting.remove(&re) {
                            let _ = reply.send(Err(CallError::Unexpected(format!(
                                "a reply this client cannot read ({e})"
                            ))));
                        }
                    }
                }
                // A call from the host: not this client's to answer.
                Ok(Frame::Call(_)) => {}
            },
        }
    };
    for (_, reply) in waiting.drain() {
        let _ = reply.send(Err(CallError::Closed(why.clone())));
    }
    ui.closed(&why);
}

/// The number of the call a frame that did not read as a frame answers, if it is a reply.
fn unreadable_reply(frame: &[u8]) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_slice(frame).ok()?;
    value.get("reply")?.get("re")?.as_u64()
}

enum Driven {
    Ask(Option<Ask>),
    Frame(Option<Result<Bytes, LinkError>>),
}

/// Bytes as standard base64 in JSON, so a keystroke that is not UTF-8 crosses whole.
mod base64_bytes {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], to: S) -> Result<S::Ok, S::Error> {
        to.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(from)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

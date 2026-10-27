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
///
/// 1.2 added what a chat needs to run in the host (#1998): holding and letting go of a project
/// (`hold_project`, `let_go_of_project`), the full start (a program's id, run and number, its
/// persona, profile, size, conversation to resume, first message and the person's opt-out of
/// the sandbox, and the window's own [`Launch`]), `close` beside `stop`, `readiness`, a started
/// program's number and process, a listed chat's number and run, and two pushed notices: a
/// program `ended`, and a view `view_ended` for good.
///
/// 1.3 added what moves a chat's hooks into the host (#2000): the host deals each chat's
/// number (`deal`, answered `dealt`, and `dealt_already` for numbers that went out before), and
/// each line a held project's hooks write is pushed to the window that holds it (`heard`) and
/// answered from there (`reply_heard`).
///
/// 1.4 added a chat's confinement in the host (#1999): `confine` starts a chat's proxy, and
/// what runs beside it, from the window's plan, answered `confined` with where it is; a start's
/// launch names the confinement it runs in; `change_reach` carries the window's answers to its
/// live asks and a port withdrawn, answered `reach_changed`; `reach_of` reads its board,
/// answered `reach`; `let_go_of_confinement` ends one; and the pushed notice `sandbox` tells
/// the window what each proxy refused, carried and asks about.
///
/// 1.5 added the kill switch held by the host (#2001, ADR 0071): `stop_all`, `rearm` (the
/// window's alone) and `kill_switch`, each answered with the switch's state (`kill_switch`), and
/// the `stopped` refusal of a start while every agent is stopped.
///
/// 1.6 added browsers (#2012): the program kind `browser`, the view kind `browser` whose stream
/// carries whole frames ([`crate::view::frames`]), a view's `role` and `rate` on `attach_view`,
/// and a listed program's `kind`.
///
/// 1.7 added a project's saving in the host (#2002): `start_saving`, `stop_saving` and
/// `fetch_project`, the window's alone, and the pushed notice `saving`, what one look of the
/// project's auto-save did; and the host's question to the window of who is mid-turn in a
/// workspace (`who_is_mid_turn`), which the window answers (`mid_turn`).
///
/// 1.8 added the event log written in the host (#2003): `record` carries one write the window
/// decided to the host, which writes it in the log it alone holds while chats run in it, in the
/// order the window sent them.
///
/// 1.9 added to what `record` carries (#2252): a held run's `begin` may hold the start's trust
/// event, written just before its `run.started`, and `taken_back` says a held start was put
/// aside. A window sends a host before 1.9 the trust event on its own, and no `taken_back`.
///
/// 1.10 added the kept board (#2194, CONTEXT.md): `board_moved` tells the host each move the
/// window made on its copy of a held project's chats and Needs you, and `board`, answered
/// `board`, reads the host's kept board whole.
///
/// 1.11 added browsers for chats (#2013): `browse` carries one act to a browser
/// ([`crate::browsing::Act`]), answered `browsed`, or refused `browser_refused` in the driver's
/// words or `browser_ended`; a browser's start may say what it reaches (`reach`,
/// [`crate::browsing::BrowserReach`]).
///
/// 1.13 added what a window that comes back to a host that outlived it needs (#2006):
/// `programs_of`, answered `programs`, says each program a held project still has, running or
/// ended, with what the window takes it back by; `quit` says the window is quitting, so the host
/// ends every program as a quit and stops, where a link that ends without it is a crash the host
/// waits out (ADR 0068 §2); and `board` is refused `too_large` where the kept board is more than
/// one answer carries.
///
/// 1.14 added taking a browser over (#2014): `take_over` and `hand_back`, answered `driving`
/// with who drives now; `browser_input`, one event of the person's aimed at what a frame showed
/// ([`crate::browsing::Input`]), refused `not_driving` unless the person drives and
/// `input_missed` where the page has moved on; `size_browser`; a listed browser's `driver`; and
/// a browser view's frames name what they show ([`crate::view::frames::AIMED_JPEG`]).
///
/// 1.15 added a browser's reach Asks (#2018): `browse` may carry what the browser reaches from
/// that act on (`reach`), and `browsed` says the sites new to the project it asked about
/// (`asked`).
///
/// 1.16 added the window hearing a chat its host composes by its names (#2407): a start with no
/// launch may name the id of the confinement the host composes for it (`confinement`), which
/// the host then names to the link the start came on, whose proxy asks that link, and whose
/// place `program_started` answers (`confined`).
///
/// 1.17 added a relaying host telling its client when its link to the other host is down and
/// when it is back (#2496, ADR 0078 as amended 2026-10-17): the pushed notices `link_down`, with
/// why, and `link_back`.
///
/// 1.18 added logins purlis performs in a chat's browser (#2016): `log_in` carries a declared
/// login, the address and the values from the vault ([`crate::browsing::LogIn`]), to a browser,
/// answered `browsed` once the browser is handed back, or refused `browser_refused` in words
/// that carry no value. The window's alone ([`crate::grants::Power::LogIn`]): no relayed link
/// sends it.
///
/// 1.21 added what changed on a browser's page (#2137): the act `changes` answers the lines of
/// the page's tree new or gone since it was last read; and a snapshot's or a find's lines mark
/// each element laid out wholly outside what the page's viewport shows (`(out of view)`). A
/// host before 1.21 refuses `changes`.
///
/// 1.22 added a window taking back its runner chats' sandboxes after a re-link (#2511, ruled
/// 2026-10-17): `take_back`, sent by a relaying host on each link it makes for one window,
/// names that window, and is answered `taken_back` with the confinements of the chats that
/// window started over the links before, which the host names to this link.
///
/// 1.23 added what a host leaves out of a plan made on another machine (#2510): `confined` may
/// name the services of the host's machine the plan gave the chat for it alone, which the host
/// started it without (`left_out`), and the name of that machine (`host_name`).
///
/// 1.24 is unused: the change it was kept for needed no new word on the wire.
///
/// 1.25 added a desktop removing a runner (#2525, ADR 0078 §3): `unpair`, sent by the desktop
/// on its link to the runner, takes it out of the runner's peers. A host before 1.25 refuses
/// it `unknown_command`, and the runner's container and home, where its peers are, are
/// removed all the same.
///
/// 1.26 added what the window allowed a chat alone, for a chat its host composes by its names
/// (#2590): a start that names its `confinement` may name those services of the host's machine
/// (`alone`), which the host leaves out of the confinement it composes where its own sandbox
/// does not give them, names in `left_out`, and asks about.
///
/// 1.27 added kept browser logins (#2021, the person's rulings of 2026-10-18): a browser's
/// start may name the kept login it opens from (`kept`), and `log_in` the kept login the browser
/// keeps once it ends (`keep`) ([`crate::browsing::KeptLogin`]). Both name a profile and a login,
/// never a value, and are the window's alone ([`crate::grants::Power::LogIn`]): no relayed link
/// sends either.
///
/// 1.28 added how a hosted program's run is sandboxed (#2636): each program `programs` answers
/// says how its host applied the sandbox to its run (`sandbox`), read by the host from what it
/// started, never from the start.
///
/// 1.29 added that record to a program's start (#2692): `program_started` says how its host
/// applied the sandbox to the run it started (`sandbox`), so a window hears it of a chat its
/// host composed by its names as it starts.
///
/// 1.30 added a relaying host telling its client a project it no longer holds for it (#2645,
/// ADR 0078 as amended 2026-10-20): the pushed notice `not_held`, with the project and the
/// other host's words, said when a link to the other host could not hold a project the client
/// holds as what its link stands on was carried over to it.
///
/// 1.31 added a window naming itself to its own host (#2623): `name_window`, the first command
/// on a link, carries the name the window keeps for that link across its restarts, so a
/// relaying host names the window by it to the other host and the window takes back its runner
/// chats after a restart. The window's alone, never sent on, and never shown by `Debug`. A host
/// before 1.31 refuses it `unknown_command`, and its window takes nothing back after a restart.
///
/// 1.32 added the host holding every client to its device's browser budget (#2611): a browser's
/// start past it is refused `device_full`. A host before 1.32 starts it; the window's own count
/// kept its starts within the budget there.
///
/// 1.33 added the window reaching a pool's runner through its own host (#2679, ADR 0078 §4):
/// `relay_to`, the first command on a window's link after its `name_window`, has the desktop's
/// host relay that link to the runner it keeps under the name, and `link_down` says when the
/// runner answered with another key than the one pinned for it (`key_changed`).
///
/// 1.34 added code over the link (#2046, ADR 0078 §7): `take_code` has the host take a git
/// bundle the client writes on a code stream into a repository of a project it holds, and
/// `give_code` has it write one of that repository's branches back, each answered `code` with the
/// refs the transfer left ([`crate::code`]); and a `hold_project` that names no `project_path`,
/// which a host that keeps its own projects holds where it keeps them. Both code commands are the
/// window's alone, relayed for it. A host before 1.34 refuses each `unknown_command`, and a hold
/// with no path `malformed`.
///
/// 1.35 added the page a browser view shows (#2300): an origin frame on the view's stream
/// (`view::frames::ORIGIN`), the origin of the page the browser shows and nothing else of its
/// address, sent where it changes and before the image in the view's snapshot. A view of a link
/// before 1.35 is sent images alone.
///
/// 1.36 added a refusal code of its own for a browser whose engine cannot make its sandbox on
/// the host's machine (#2899): `no_engine_sandbox`, its `why` the binary's words for it. A host
/// before 1.36 refuses it `not_started` in the same words, which its window reads by them.
///
/// 1.37 added a browser engine's first download, told as it goes (#2207): the pushed
/// `engine_download` says how far the download for a browser's start has come (`done`, of `total`
/// where that is known), a few times a second at most, and `cancel_engine_download` stops it,
/// keeping nothing of it. The window's alone ([`crate::grants::Power::Browser`]): a host pushes
/// it only to a link of 1.37 or later that may start a browser.
///
/// 1.38 added a runner's browsers (#2050, spec #2041): `remote-link` may start a browser, attach
/// a view of one, send a chat's acts to it, drive it for the person and have purlis log it in,
/// each as the desktop's host relaying its window ([`crate::grants`]), never from a kept login
/// or keeping one, which stay on the device they were made on. A runner before 1.38 refuses each
/// `not_allowed`, and its window's chats there have no browser.
///
/// 1.39 added a window taking its browsers back after a crash (#2025): `programs_of` answers a
/// held project's browsers too, each with who drives it (`driver`) and the note the window keeps
/// of it in the host (`note`), which a browser's start may carry and `note_program` replaces. A
/// note is the window's own, which the host keeps for it and never reads, at most
/// [`ProgramNote::MOST`] bytes. And each browser says the identity of every login the host
/// performed in it, or that it opened from (`logins`): the host's own record, which no note can
/// unsay; a login named with no identity is recorded as an empty one. A login names its
/// identity (`identity`). The window's alone ([`crate::grants::Power::Reattach`]). A host of an
/// earlier minor answers no browser, and a window that comes back to it ends them as nothing
/// takes them back.
///
/// 1.40 added a host saying it records a hook line under the project as it holds it (#3400):
/// each `heard` it pushes says so (`pathless`), and the window's reply to that line names no
/// project path, which such a host never used. A host before 1.40 says nothing of it, and is
/// sent the path as before.
///
/// 1.41 added a host's own event log over a link (#3393, ADR 0078 §7): `log_end` answers the
/// host's device and the last `seq` of its log, and a `subscribe` whose `since` names that device
/// is pushed each line of the log after it, then each as it is written, as `event`, and a gap as
/// `missed`. A link that may read it, the window's or its host's relaying it, is pushed it; one
/// whose `since` names no such device is pushed none of it, as before. A host before 1.41 refuses
/// `log_end` `unknown_command`, and its window reads none of its log. And the kill switch says
/// which runners a desktop's stop stopped (`runners`), and that a stop found a host stopped
/// already (`already`), so a runner stopped from its own command line is not re-armed by its
/// desktop (#2051).
pub const VERSION: Version = Version {
    major: 1,
    minor: 41,
};

/// The version that first pushes a host's own event log to a link that asks with a cursor for its
/// device, and answers `log_end` (#3393).
pub const KNOWS_EVENT_LOG: Version = Version {
    major: 1,
    minor: 41,
};

/// The version that first knows browsers relayed for a window (#2050): a runner before it refuses
/// a browser from `remote-link`, so a relaying host sends one on, and a login's values, only to
/// a runner of this version or later.
pub const KNOWS_RELAYED_BROWSERS: Version = Version {
    major: 1,
    minor: 38,
};

/// The version that first answers a held project's browsers to `programs_of` and keeps a note
/// of each program for the window (#2025).
pub const KNOWS_BROWSERS_TAKEN_BACK: Version = Version {
    major: 1,
    minor: 39,
};

/// **What the window keeps of a program in its host** (1.39, #2025): the window's own words, as
/// JSON, which the host keeps beside the program and never reads, and answers back to
/// `programs_of`. At most [`ProgramNote::MOST`] bytes: one past it is refused [`TOO_LARGE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProgramNote(pub serde_json::Value);

impl ProgramNote {
    /// The most bytes a note is, as JSON.
    pub const MOST: usize = 16 * 1024;

    /// How many bytes it is, as JSON.
    pub fn bytes(&self) -> usize {
        serde_json::to_vec(&self.0).map_or(usize::MAX, |bytes| bytes.len())
    }

    /// Whether it is within [`Self::MOST`].
    pub fn fits(&self) -> bool {
        self.bytes() <= Self::MOST
    }
}

/// The version that first knows a browser engine's download (#2207): no link before it is
/// pushed [`Pushed::EngineDownload`].
pub const KNOWS_ENGINE_DOWNLOADS: Version = Version {
    major: 1,
    minor: 37,
};

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
/// The link's scope may not do this (FD-27); or a chat's confinement planned on another machine
/// asks for a folder or a service of this one that is not the project's own here (1.4, #2445),
/// for a reason its `why` gives.
pub const NOT_ALLOWED: &str = "not_allowed";
/// The host cannot carry this out now, for a reason its `why` gives (1.2): a start the host
/// cannot compose, or a program it could not run.
pub const NOT_STARTED: &str = "not_started";
/// Every agent is stopped by the kill switch, and nothing starts until the window re-arms it
/// (1.5, ADR 0071); or a re-arm that could not be kept, so every agent stays stopped.
pub const STOPPED: &str = "stopped";
/// The host keeps no event log, or could not write this in it durably, for a reason its `why`
/// gives (1.8): what the window asked recorded is not, and the act it records does not go on.
pub const NOT_RECORDED: &str = "not_recorded";
/// A browser's driver refused an act, or could not tell whether it acted (1.11): its `why` is
/// the driver's sentence, which says what to do.
pub const BROWSER_REFUSED: &str = "browser_refused";
/// The browser an act was for has ended: stopped, closed, or its engine ended (1.11). Nothing
/// was done; a new browser is opened for the next act.
pub const BROWSER_ENDED: &str = "browser_ended";
/// The person's input, or a size, for a browser they have not taken over (1.14): nothing reached
/// the page.
pub const NOT_DRIVING: &str = "not_driving";
/// The person's input was aimed at a page or document the browser no longer shows (1.14): it was
/// dropped, and nothing reached the page.
pub const INPUT_MISSED: &str = "input_missed";

/// What the host would answer is more than one answer carries, for a reason its `why` gives
/// (1.13): a held project's kept board too large to hand over whole.
pub const TOO_LARGE: &str = "too_large";

/// The device runs as many browsers as its budget already (1.32, #2611): the host counts
/// every browser it runs, whoever started it, and starts no more. Its `why` names the budget.
pub const DEVICE_FULL: &str = "device_full";

/// The runner a relaying host links to answered with another key than the one pinned at its
/// pairing (1.33, #2679): nothing was sent to it, and it is not linked until the person removes
/// it and adds it anew.
pub const KEY_CHANGED: &str = "key_changed";

/// The browser's engine cannot make its sandbox on the host's machine, and purlis never runs it
/// without (1.36, #2899): nothing started. Its `why` says so in the binary's words.
pub const NO_ENGINE_SANDBOX: &str = "no_engine_sandbox";

/// Every refusal code this version says. A later minor may add one; a client treats one it
/// does not know as a refusal and shows its `why`.
pub const REFUSALS: &[&str] = &[
    UNKNOWN_COMMAND,
    MALFORMED,
    NO_SUCH_CHAT,
    NO_SUCH_PROJECT,
    NOT_ALLOWED,
    NOT_STARTED,
    STOPPED,
    NOT_RECORDED,
    BROWSER_REFUSED,
    BROWSER_ENDED,
    TOO_LARGE,
    NOT_DRIVING,
    INPUT_MISSED,
    DEVICE_FULL,
    KEY_CHANGED,
    NO_ENGINE_SANDBOX,
];

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
    /// End a chat's program and everything it started. The hosted program stays, with its last
    /// screen, until it is closed ([`Command::Close`]).
    Stop { chat: String },
    /// 1.0's word for [`Command::StartProgram`] of kind [`ProgramKind::Harness`], answered with
    /// [`Answer::Started`]. Never reaches a [`Host`].
    Start(Start),
    /// Start a hosted program of a kind, and answer [`Answer::ProgramStarted`] with its id.
    /// Boxed: the full start is the largest command by far.
    StartProgram(Box<StartProgram>),
    /// Open a view of `kind` on a hosted program: the host opens a stream for it
    /// ([`crate::view::Views::open`]), whose snapshot then live bytes follow, and answers
    /// [`Answer::Attached`] with the view's `kind`. A browser view is also given its `role` and
    /// the most frames a second it wants (1.6); a terminal view takes neither.
    AttachView {
        program: String,
        kind: ViewKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<ViewRole>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rate: Option<u32>,
    },
    /// Close a view the client attached. The program keeps running.
    DetachView { program: String, view: u32 },
    /// Every event after `since`, then each as it is written ([`Pushed`]): ADR 0066's
    /// `subscribe(since)`. **The host's own event log is pushed from its device's cursor**
    /// (1.41, #3393): to a link that may read it ([`crate::grants::Power::EventLog`]) and whose
    /// `since` names the host's device ([`Command::LogEnd`] says which), each of its lines after
    /// that cursor, then each as it is written, as [`Pushed::Event`], and a gap as
    /// [`Pushed::Missed`]. A `since` that does not name it is pushed none of its log.
    Subscribe { since: Since },
    /// Stop pushing events.
    Unsubscribe,
    /// Hold a project on this host: the host may now start programs in it. A project is
    /// named by its stable id, and `project_path` says where it is on the host's machine
    /// (1.2). Left out (1.34), the host holds it where it keeps its own projects, a runner's
    /// `<data>/projects/<project id>`, made there if it is not; a host that keeps none refuses
    /// it [`NO_SUCH_PROJECT`], and an id that is not a ULID [`NOT_ALLOWED`].
    HoldProject {
        project: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        project_path: String,
    },
    /// Let go of a project this link held (1.2). What runs in it runs on until it is closed.
    LetGoOfProject { project: String },
    /// End a hosted program and everything it started, and forget it: its views end, and it is
    /// no longer listed (1.2). [`Command::Stop`] ends the program and keeps it.
    Close { program: String },
    /// Whether a hosted program is ready to be typed at, as its terminal and the clock say it,
    /// never what it wrote (1.2): answered [`Answer::Readiness`].
    Readiness { program: String },
    /// Deal the next number in a held project, for a chat about to be started with it: one no
    /// chat of the project has had, so a client may name the chat by it first (1.3). Answered
    /// [`Answer::Dealt`].
    Deal { project: String },
    /// The numbers up to `dealt` went out in a held project before this host dealt any, so none
    /// of them is dealt again (1.3).
    DealtAlready { project: String, dealt: u32 },
    /// The reply to the line a held project's hooks wrote that the host pushed as `heard`
    /// ([`Pushed::Heard`]): what the window answered it with, in the shape the line's kind takes
    /// (1.3).
    ReplyHeard {
        project: String,
        heard: u64,
        reply: serde_json::Value,
    },
    /// Save a held project from now on: the host runs its watcher and its auto-save, as the
    /// project's settings say, until [`Command::StopSaving`] or the host ends (1.7). A link
    /// that ends does not stop it, so the project goes on being saved while the window is
    /// gone. A project the host already saves is not saved twice. Each look that did something
    /// is pushed as [`Pushed::Saving`].
    StartSaving { project: String },
    /// Stop saving a project, and answer once its auto-save has been told to stop (1.7).
    StopSaving { project: String },
    /// Fetch what came in for a project the host saves, unless it fetched a moment ago: the
    /// window came back into focus (1.7).
    FetchProject { project: String },
    /// The window's answer to [`Pushed::WhoIsMidTurn`] (1.7): the chats mid-turn that could be
    /// writing the workspace, by the names the window shows, or none said where the window
    /// cannot tell, which the host takes as every chat it runs there.
    MidTurn {
        question: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chats: Option<Vec<String>>,
    },
    /// A move the window made of chat `chat` on its copy of a held project's kept board, for the
    /// host to make on the kept board too, in the order the window made them (1.10): `moved` is
    /// the move as the core says it. Answered once made.
    BoardMoved {
        project: String,
        chat: u32,
        moved: serde_json::Value,
    },
    /// A held project's kept board, whole (1.10): answered [`Answer::Board`].
    Board { project: String },
    /// Start a chat's confinement, its proxy and what runs beside it, from the window's plan
    /// (1.4): answered [`Answer::Confined`]. The window's alone ([`crate::grants::Power::Confine`]).
    /// A plan from a link other than this machine's window is refused [`NOT_ALLOWED`] unless it
    /// fits this machine (#2445). Boxed: a plan lists every host a chat reaches.
    Confine(Box<Confine>),
    /// End a confinement, whether or not a program runs in it (1.4). One the host does not hold
    /// is let go of already; one the link may not act on is refused [`NO_SUCH_CHAT`] (#2444).
    LetGoOfConfinement { confinement: String },
    /// The window's answer to a confinement's live ask, or a port withdrawn from it (1.4):
    /// answered [`Answer::ReachChanged`], or refused [`NO_SUCH_CHAT`] for a confinement the
    /// link may not act on (#2444).
    ChangeReach {
        confinement: String,
        #[serde(flatten)]
        change: ReachChange,
    },
    /// What a confinement's board of live asks holds now (1.4): answered [`Answer::Reach`], or
    /// refused [`NO_SUCH_CHAT`] for a confinement the link may not act on (#2444).
    ReachOf { confinement: String },
    /// Throw the kill switch (1.5, ADR 0071): end every program the host runs, in every
    /// project, and start none until the window re-arms it. The host keeps the stop across its
    /// own restart. Answered [`Answer::KillSwitch`].
    StopAll,
    /// Let programs start again (1.5): the window's alone, on a `local-ui` link
    /// ([`crate::grants::Power::Rearm`]). Nothing that was stopped is started again. Answered
    /// [`Answer::KillSwitch`].
    Rearm,
    /// Whether every agent is stopped (1.5): answered [`Answer::KillSwitch`].
    KillSwitch,
    /// Write `entry` in the event log the host holds, durably, and answer once it is kept
    /// (1.8): one write the window decided (the run a start begins, a person's grant, a kill
    /// switch line), in the shape the binary's log reads. The window's alone
    /// ([`crate::grants::Power::Record`]), in the order it sent them.
    Record { entry: serde_json::Value },
    /// One act of a browser's (1.11, #2013), carried out by its driver: answered
    /// [`Answer::Browsed`], or refused [`BROWSER_REFUSED`] in the driver's words. `call_id` is
    /// the act's own id: a retry with the same id answers again and acts on nothing. The
    /// window's alone ([`crate::grants::Power::Browser`]).
    Browse {
        program: String,
        call_id: String,
        act: crate::browsing::Act,
        /// What the browser may reach from this act on (1.15, #2018): its chat's reach as it is
        /// now, with what the person allowed since the browser started. Absent, it reaches what
        /// it reached before.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reach: Option<crate::browsing::BrowserReach>,
    },
    /// Every program a held project still has, running or ended, that no client has closed
    /// (1.13): what a window that comes back to the host takes back. Answered
    /// [`Answer::Programs`]. The window's alone ([`crate::grants::Power::Reattach`]). Each
    /// confinement it answers is named to the link that asked, which may act on it and hear it
    /// from then on (#2444).
    ProgramsOf { project: String },
    /// The window is quitting (1.13, ADR 0068 §2): the host ends every program it runs as a quit
    /// would, answers once they are gone, and stops. A window's link that ends without it is a
    /// crash, after which the host keeps its programs for the grace period. The window's alone
    /// ([`crate::grants::Power::Quit`]).
    Quit,
    /// The desktop on this link is removing the runner (1.25, #2525, ADR 0078 §3): the runner's
    /// host takes it out of its peers, so no later link from it is admitted, and answers
    /// [`Answer::Done`]. Only a runner's host, on the link between hosts from the desktop it is
    /// paired with, carries it out; every other host refuses it [`NOT_ALLOWED`]
    /// ([`crate::grants::Power::Unpair`]).
    Unpair,
    /// The person takes browser `program` over (1.14, #2014): an act in flight answers that it
    /// was interrupted, and every act is refused in words until [`Command::HandBack`]. The lease
    /// is the host's: it stays with the person whatever becomes of the link that took it, until
    /// they hand it back. Answered [`Answer::Driving`]. The window's alone
    /// ([`crate::grants::Power::Browser`]).
    TakeOver { program: String },
    /// The person hands browser `program` back (1.14): every reference dealt before it is void.
    /// Answered [`Answer::Driving`].
    HandBack { program: String },
    /// One event of the person's in a browser they drive (1.14): refused [`NOT_DRIVING`] unless
    /// they took it over, and [`INPUT_MISSED`] where the page no longer shows what the event was
    /// aimed at. Nothing is replayed: an event that does not reach the page is dropped.
    BrowserInput {
        program: String,
        input: crate::browsing::Input,
    },
    /// The person sets the size of browser `program`'s pages, in CSS pixels (1.14): refused
    /// [`NOT_DRIVING`] unless they drive it.
    SizeBrowser {
        program: String,
        width: u16,
        height: u16,
    },
    /// purlis performs a login in browser `program` (1.18, #2016): it takes the browser from the
    /// agent, goes to the login's address, fills it only on that address's origin, submits,
    /// waits for the password field to go, and hands the browser back, which then reaches that
    /// origin alone. Answered [`Answer::Browsed`] with the page it left, or refused
    /// [`BROWSER_REFUSED`] in words that carry no value. The values never show in this
    /// command's `Debug`. The window's alone ([`crate::grants::Power::LogIn`]), relayed by its
    /// host to a runner's browser from 1.38 (#2050), and never one that keeps a kept login there
    /// ([`crate::grants::Power::KeptLogin`]).
    LogIn {
        program: String,
        login: crate::browsing::LogIn,
    },
    /// A relaying host's word on each link it makes to another host for one window (1.22,
    /// #2511): this link is `window`'s, by the name the relaying host gives that window on
    /// every link it makes for it. Answered [`Answer::TakenBack`] with the confinements of the
    /// chats that window started over the links before, each named to this link from then on
    /// ([`crate::grants::Power::TakeBack`]). A link names one window: another name is refused
    /// [`MALFORMED`].
    TakeBack { window: String },
    /// The window names itself on a link to its own host (1.31, #2623), as the link's first
    /// command: the name it keeps for this link across its restarts, on this machine alone.
    /// Answered [`Answer::Done`]. A relaying host names the window to the other host by a digest
    /// of it, never the name ([`Command::TakeBack`]); a host that relays nothing has nothing to
    /// do with it. The window's alone ([`crate::grants::Power::NameWindow`]), never sent on, and
    /// never shown by `Debug`.
    NameWindow { name: crate::browsing::Secret },
    /// The window asks the desktop's host to relay this link to `runner` (1.33, #2679, ADR 0078
    /// §4): the runner this machine keeps under that name, reached through its connector and
    /// linked with the key pinned at its pairing. Sent only as a link's first command but its
    /// `name_window`, before `subscribe`; from then on the host relays every command on the link
    /// to that runner, as a relaying host does, its `name_window` first. Answered [`Answer::Done`], or refused [`NOT_ALLOWED`] on a link that
    /// has sent anything else and [`NOT_STARTED`] for a runner this machine does not keep. The
    /// window's alone ([`crate::grants::Power::RelayTo`]).
    RelayTo { runner: String },
    /// The client writes a git bundle into `repo` of a held project (1.34, #2046, ADR 0078 §7):
    /// the host opens a code stream named `transfer` ([`crate::code::Header`]), reads the bundle
    /// the client writes on it until the client shuts it, at most
    /// [`crate::code::MOST_BUNDLE_BYTES`], and takes into the repository exactly the refs
    /// `refs` names, each at the commit it names: from the bundle, or already in the repository
    /// where the bundle is empty of it. A branch (`refs/heads/…`) or a snapshot of a branch's
    /// uncommitted work (`refs/purlis/snapshots/…`), and nothing else. `repo` is the
    /// repository's folder in the project, relative to it: empty for the project's own, or
    /// `workspaces/<workspace>/<repo>`. Answered [`Answer::Code`] with every branch and
    /// snapshot the repository holds after it; refused [`NOT_ALLOWED`] for a ref or a folder
    /// that is not one of those, or a bundle that carries a ref it did not name, and
    /// [`TOO_LARGE`] past the bound. The window's alone ([`crate::grants::Power::Code`]),
    /// relayed for it.
    ///
    /// `checkout`, where it names a branch among `refs`, has a repository with nothing checked
    /// out yet (one the host has just made) check its own work tree out on that branch, with the
    /// branch's snapshot's uncommitted work in it where `refs` carries one: what the runner's
    /// chats then work in. A repository with something checked out is left as it is, and a
    /// branch one of its work trees has checked out is the runner's own and is never moved.
    TakeCode {
        project: String,
        #[serde(default)]
        repo: String,
        transfer: u64,
        refs: Vec<CodeRef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkout: Option<String>,
    },
    /// The host writes a git bundle of `repo`'s branches back to the client (1.34, #2046): on a
    /// code stream named `transfer`, leaving out every commit `have` reaches, and shuts it once
    /// written, an empty stream where there is nothing the client lacks. Answered
    /// [`Answer::Code`] with the branches the bundle is of. The window's alone
    /// ([`crate::grants::Power::Code`]), relayed for it.
    GiveCode {
        project: String,
        #[serde(default)]
        repo: String,
        transfer: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        have: Vec<String>,
    },
    /// The person cancels the engine's download for browser `program`'s start (1.37, #2207):
    /// the download stops, nothing of it is kept, and the start is refused in words that say
    /// so. Answered [`Answer::Done`], also where nothing is downloading for it. The window's
    /// alone ([`crate::grants::Power::Browser`]).
    CancelEngineDownload { program: String },
    /// The window's note of `program` is `note` from now on (1.39, #2025): what `programs_of`
    /// answers with the program to a window that comes back. The host keeps it and never reads
    /// it. Answered [`Answer::Done`]; refused [`TOO_LARGE`] past [`ProgramNote::MOST`] bytes, keeping
    /// the one before, and [`NO_SUCH_CHAT`] for a program the host does not hold. The window's
    /// alone ([`crate::grants::Power::Reattach`]).
    NoteProgram { program: String, note: ProgramNote },
    /// **Where the host's own event log ends now** (1.41, #3393): its device and the `seq` of
    /// its last line, 0 for none, answered [`Answer::LogEnd`]: the cursor a client that holds
    /// none of it subscribes from, so it is pushed what is written from now on. Refused
    /// [`NOT_STARTED`] by a host whose binary keeps no event log. The window's, and its host's
    /// relaying it ([`crate::grants::Power::EventLog`]).
    LogEnd,
}

/// One ref of a repository and the commit it is at (1.34): what `take_code` names and `code`
/// answers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CodeRef {
    /// Its full name: `refs/heads/<branch>` or `refs/purlis/snapshots/<branch>`.
    pub name: String,
    /// The commit, as forty hexadecimal digits.
    pub oid: String,
}

/// What a [`Command::Confine`] asks for (1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confine {
    /// The id the window gives it, which a start's [`Launch::confinement`] names.
    pub confinement: String,
    /// The project it is for, by its stable id, which the host holds.
    pub project: String,
    /// What its proxy carries and what runs beside it.
    pub plan: ConfinePlan,
    /// Whether its proxy holds a connection to a host nothing lists while the person is asked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub asks: bool,
}

/// A chat's confinement as the window planned it (1.4): its proxy's reach and ports, and what
/// runs beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfinePlan {
    /// Each host its sandbox lists, with the layer that lists it, in the order they decide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listed: Vec<Listed>,
    /// The hosts policy pins as never allowed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub never: Vec<String>,
    /// The ports of the machine withdrawn while the chat runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withdrawn: Vec<u16>,
    /// Every host is carried: the floor.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub anything: bool,
    /// The ports a tunnel, and a plain request, is carried to for a host listed without one.
    pub tunnel_ports: Vec<u16>,
    pub plain_ports: Vec<u16>,
    /// A host listed without a port is carried on every port.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub any_port: bool,
    /// What runs beside the proxy.
    pub beside: Beside,
    /// The services on the machine the chat was given, each by its port.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub given: Vec<u16>,
}

/// A host a confinement's proxy carries, and the layer that lists it (1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listed {
    pub host: String,
    pub layer: Layer,
}

/// The layer that lists a host (1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Open,
    Persona,
    You,
    Chat,
    Project,
}

/// What runs beside a confinement's proxy (1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Beside {
    /// A temp folder of the chat's own, with its ssh route.
    Wrapped,
    /// Nothing but its ssh route, in a folder of its own inside `routes`.
    ProxyOnly { routes: String },
}

/// The window's change to a confinement's reach (1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ReachChange {
    /// The person allowed `host` at `layer`.
    Allow { host: String, layer: Layer },
    /// An Allow of `host` at `layer` was removed.
    Forget { host: String, layer: Layer },
    /// The person kept `host` blocked.
    KeepBlocked { host: String },
    /// `port` of the machine is carried no longer.
    WithdrawPort { port: u16 },
}

/// Where a confinement is, as the host started it (1.4): its proxy's ports on the loopback
/// interface, and the folders beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confined {
    pub confinement: String,
    pub http: u16,
    pub socks: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<String>,
    /// The services of the host's machine, by port, that the plan gave the chat for it alone and
    /// the host started it without, since the host's own sandbox for the project does not give
    /// them (1.23, #2510).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left_out: Vec<u16>,
    /// The name of the machine the host runs on, as its system names it, where it left a
    /// service out (1.23, #2510): what the window names the runner by when it asks about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_name: Option<String>,
}

/// What a confinement's board of live asks holds now (1.4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachNow {
    /// The hosts and ports the person is being asked about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asked: Vec<String>,
    /// The hosts allowed live, each with its layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_live: Vec<Listed>,
}

/// What a confinement's proxy did, as the host tells the window (1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "told", rename_all = "snake_case")]
pub enum SandboxTold {
    /// It refused `host` on `port`, which nothing lists.
    Refused { host: String, port: u16 },
    /// It refused `host` on `port` as an address of the machine, or a link-local one.
    RefusedLocal { host: String, port: u16 },
    /// It has refused `count` hosts past the ones it names.
    RefusedMore { count: u64 },
    /// It carried `times` connections to `target` (none for the hosts past those it tells
    /// apart), let through by `by`: the decision's word.
    Reached {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        by: String,
        times: u64,
    },
    /// Ask the person about `targets`, each a host and port: one ask.
    Asked { targets: Vec<String> },
    /// A connection to `target` was held and nobody answered.
    TimedOut { target: String },
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

/// What a [`Command::StartProgram`] asks for: the full start (1.2).
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
    /// The id the program is to have: its chat's own (ADR 0066), so a client that knows its
    /// chats finds each again. A host mints one where none is given (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// The run this start begins (ADR 0066), where the client minted one (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// The chat's number in its project: what its hooks carry and its per-chat files are named
    /// by. A host deals the next where none is given, and never one a program it holds in the
    /// project already has (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// The persona it runs as, or the project's default (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// The profile it runs on, or none (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The size its terminal starts at, in columns and rows (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
    /// The harness's conversation to carry on, by its own id (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
    /// What is typed to the harness once it is ready (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_message: Option<String>,
    /// The person chose to run this one without the sandbox (ADR 0067 §7, 1.2): taken from a
    /// `local-ui` link alone ([`crate::grants::Power::OptOut`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub without_sandbox: bool,
    /// The window's own start, composed above its seam: given only by the window, on a
    /// `local-ui` link, or by its host relaying it over the link between hosts
    /// ([`crate::grants::Power::Launch`]) (1.2). A start with none is composed by the host from
    /// what the rest of it names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<Launch>,
    /// For a [`ProgramKind::Browser`], what it may reach (1.11): absent, nothing listed and
    /// every site new to the project refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<crate::browsing::BrowserReach>,
    /// For a start the host composes (no `launch`), the id the confinement it starts the
    /// program in is to have (1.16): that confinement is named to the link the start came on,
    /// its proxy asks that link about each host nothing lists, and the answer says where it is
    /// ([`Answer::ProgramStarted`]'s `confined`). Taken from a link that may confine alone
    /// ([`crate::grants::Power::Confine`]). Absent, the host names it to no link and its proxy
    /// asks nobody.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confinement: Option<String>,
    /// For a start that names its `confinement`, the services of the host's machine, by port,
    /// the person allowed this chat for it alone where the chat was planned (1.26, #2590): the
    /// host leaves out of the confinement it composes each one its own sandbox does not give,
    /// names them in that confinement's `left_out` ([`Confined::left_out`]), and its proxy asks
    /// the link it is named to about each. Taken from a link that may confine alone
    /// ([`crate::grants::Power::Confine`]), and only beside a `confinement`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alone: Vec<u16>,
    /// For a [`ProgramKind::Browser`], the kept login it opens from (1.27, #2021): it starts
    /// signed in from that login's store, held to the login's site from its first moment. The
    /// window's alone, on its own device ([`crate::grants::Power::KeptLogin`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<crate::browsing::KeptLogin>,
    /// What the window keeps of the program in its host (1.39, #2025): its own words, which the
    /// host never reads and answers back with the program to `programs_of`, at most
    /// [`ProgramNote::MOST`] bytes. Taken from a link that may take programs back alone
    /// ([`crate::grants::Power::Reattach`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<ProgramNote>,
}

impl StartProgram {
    /// A start of `kind` in `project`, with nothing else said.
    pub fn of(project: impl Into<String>, kind: ProgramKind) -> StartProgram {
        StartProgram {
            project: project.into(),
            kind,
            workspace: None,
            harness: None,
            program: None,
            run: None,
            number: None,
            persona: None,
            profile: None,
            cols: None,
            rows: None,
            resume: None,
            first_message: None,
            without_sandbox: false,
            launch: None,
            reach: None,
            confinement: None,
            alone: Vec::new(),
            kept: None,
            note: None,
        }
    }
}

/// What the window composed for a start above its seam: the program and its words, and what
/// its environment is made of (1.2). The host composes the environment from it the way the
/// window did, against its own, which it was started with
/// ([`crate::grants::Power::Launch`]: the window's alone, or relayed by its host).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launch {
    /// The program, by path or name.
    pub program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Where it starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// The chat's own variables, in order: a later one of a name wins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<(String, String)>,
    /// Variable names kept out of the chat, set or inherited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strip: Vec<String>,
    /// The person's own additions to what a chat is started with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pass: Vec<String>,
    /// A declared harness's namespace, each variable judged on its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared: Vec<String>,
    /// The harness it runs, whose own variables it keeps, by its word; none for a shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// The folder of purlis's git hooks its git is armed with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_hooks: Option<String>,
    /// Where its hooks report, and the chat's token they carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporting: Option<Reporting>,
    /// The confinement it runs in ([`Command::Confine`]), which then lives as long as the
    /// program does, whether or not the link that made it does (1.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confinement: Option<String>,
}

/// Where a chat's hooks report, and the token they carry (1.2).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reporting {
    pub socket: String,
    pub token: String,
}

/// Never prints the token.
impl std::fmt::Debug for Reporting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporting")
            .field("socket", &self.socket)
            .finish_non_exhaustive()
    }
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
    /// A browser, which only purlis drives, watched through browser views (1.6).
    Browser,
}

/// What kind of view a client attaches: what its stream's snapshot and live bytes are. A
/// later minor adds one (a browser view, spec 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// A terminal's bytes: the snapshot draws its scrollback, screen, cursor and modes.
    Terminal,
    /// A browser's frames (1.6): the snapshot is its latest frame, and the live bytes the frames
    /// after it, each one whole ([`crate::view::frames`]). Frames are made only while a browser
    /// view is attached.
    Browser,
}

/// What a browser view is to the person (1.6): it decides how many frames a second it is sent
/// when the attach names no `rate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRole {
    /// The view the person is looking at: every frame the browser makes.
    Focused,
    /// A small view at a glance (a tab's thumbnail, a row): about one frame a second.
    Thumbnail,
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
    /// The hosted program it started: [`Command::StartProgram`], with its number in its project
    /// and its process, where the host says them (1.2).
    ProgramStarted {
        program: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        number: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        process: Option<u32>,
        /// Where the confinement the host composed for it is, named to the link the start came
        /// on, where the start asked for it by [`StartProgram::confinement`] (1.16).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confined: Option<Confined>,
        /// How its run is sandboxed, as the host applied it (1.29): the same record `programs`
        /// answers. Absent from a host before 1.29, whose window takes the run as sandboxed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sandbox: Option<RunSandbox>,
    },
    /// Whether a program is ready to be typed at: [`Command::Readiness`] (1.2).
    Readiness(Readiness),
    /// The number dealt: [`Command::Deal`] (1.3).
    Dealt { number: u32 },
    /// A held project's kept board, as the core says it: [`Command::Board`] (1.10).
    Board(serde_json::Value),
    /// The programs a held project still has: [`Command::ProgramsOf`] (1.13).
    Programs(Vec<HeldProgram>),
    /// Where the confinement it started is: [`Command::Confine`] (1.4).
    Confined(Confined),
    /// The hosts held connections gave up on before an Allow, which the chat is told to retry:
    /// [`Command::ChangeReach`] (1.4).
    ReachChanged { retry: Vec<String> },
    /// What a confinement's board holds now: [`Command::ReachOf`] (1.4).
    Reach(ReachNow),
    /// The kill switch as it stands: [`Command::StopAll`], [`Command::Rearm`] and
    /// [`Command::KillSwitch`] (1.5).
    KillSwitch(KillSwitch),
    /// What a browser answered an act with: [`Command::Browse`] (1.11).
    Browsed(Box<crate::browsing::Browsed>),
    /// Who drives a browser now: [`Command::TakeOver`] and [`Command::HandBack`] (1.14).
    Driving { driver: crate::browsing::Driver },
    /// The confinements named to the link again, by id: [`Command::TakeBack`] (1.22).
    TakenBack { confinements: Vec<String> },
    /// The refs a transfer of code left (1.34): [`Command::TakeCode`] and
    /// [`Command::GiveCode`].
    Code { heads: Vec<CodeRef> },
    /// Events follow, as [`Pushed`] frames: [`Command::Subscribe`].
    Subscribed,
    /// Where the host's own event log ends: [`Command::LogEnd`] (1.41).
    LogEnd { device: String, seq: u64 },
    /// Done, with nothing to say.
    Done,
}

/// One program a held project still has, as [`Command::ProgramsOf`] answers it (1.13): what a
/// window that comes back takes it back by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldProgram {
    /// Its id in the host.
    pub program: String,
    /// What kind of program it is: a harness or a shell, or a browser to a link of 1.39 or
    /// later (#2025).
    pub kind: ProgramKind,
    /// Its number in its project; a browser has none, and is `0`.
    pub number: u32,
    /// Its process, while it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<u32>,
    /// The run its start began (ADR 0066), where its start said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// How it ended, where it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Exit>,
    /// The confinement it runs in, and where that is, where it runs in one (1.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confined: Option<Confined>,
    /// The plan that confinement was started from, as the window that confined it sent it: what
    /// a window that comes back compares with the plan it would make now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<ConfinePlan>,
    /// How its run is sandboxed, as the host applied it (1.28). Absent from a host before 1.28,
    /// whose window takes the run as sandboxed and offers it a restart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<RunSandbox>,
    /// The size its terminal is now; a browser's pages', in CSS pixels.
    pub cols: u16,
    pub rows: u16,
    /// Who drives it, where it is a browser (1.39, #2025): the lease the host holds, so a window
    /// that comes back draws it as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<crate::browsing::Driver>,
    /// The note the window last kept of it in the host (1.39, #2025), where it kept one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<ProgramNote>,
    /// The identity of each login purlis performed in a browser, or the kept login it opened
    /// from, in order, each once (1.39, #2025): the host's own record, so a window that comes
    /// back knows every login it holds whatever its note says. One performed with no identity
    /// named is an empty one, which no note names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logins: Vec<String>,
}

/// The kill switch as the host holds it (1.5, ADR 0071).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KillSwitch {
    /// Every agent is stopped, and nothing starts until the window re-arms.
    pub stopped: bool,
    /// What the host has to say of the switch, when it has something: a stop it could not keep
    /// across its restart, or a state it could not read and so holds as stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// **It was stopped already when this stop came** (1.41, #2051): a runner stopped from its own
    /// command line, which its desktop's re-arm leaves stopped. Said only in answer to a stop.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub already: bool,
    /// **The runners this host's stop stopped, which its window's re-arm re-arms** (1.41,
    /// #2051), each `<pool>/<runner>`: what the window lists before it re-arms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runners: Vec<String>,
}

/// What a host said of a program it started ([`Client::start_program_told`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStarted {
    pub program: String,
    /// Its number in its project.
    pub number: Option<u32>,
    /// Its process, while it runs.
    pub process: Option<u32>,
    /// Where the confinement the host composed for it is, where the start named one to this
    /// link (1.16).
    pub confined: Option<Confined>,
    /// How its run is sandboxed, as the host applied it, where the host says (1.29).
    pub sandbox: Option<RunSandbox>,
}

/// One chat, as [`Command::List`] lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Its number in its project (1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// The run it is in (ADR 0066, 1.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// What kind of hosted program it is, where it is not a chat's (1.6): a browser. Listed only
    /// to a link that speaks 1.6, so a client one version behind never takes one for a chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProgramKind>,
    /// Who drives it, where it is a browser (1.14): the person from their take-over to their
    /// hand-back, so a window that comes back draws the browser as still theirs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<crate::browsing::Driver>,
}

/// Whether a hosted program is ready to be typed at (1.2): the terminal's line discipline and
/// the clock, never what the program wrote (ADR 0061).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    /// Whether its terminal still edits lines (canonical mode) rather than handing each key to
    /// the program; absent where the platform cannot say.
    pub edits_lines: Option<bool>,
    /// How long it has written nothing, in milliseconds.
    pub quiet_for_ms: u64,
}

/// **How a hosted program's run is sandboxed, as its host applied it** (1.28, #2636): read by
/// the host from what it started, the confinement it runs in and the program that wraps it,
/// never from anything a start said of itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunSandbox {
    /// It runs in no sandbox: no confinement holds it and nothing wraps it.
    Off,
    /// It runs in this sandbox, by its backend's word (`seatbelt`, `landlock`, `bubblewrap`),
    /// or `proxy` for one held by its confinement's proxy alone.
    By(String),
}

/// How a hosted program ended (1.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exit {
    /// It exited with this code.
    Code(u32),
    /// A signal ended it, named as the system names it.
    Signal(String),
}

/// Why a view ended for good (1.2): no stream of it comes again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewEnd {
    /// Its program ended, and the view sent its last screen first.
    ProgramEnded,
    /// The client acknowledged more than it was sent.
    OverAcknowledged,
    /// The host closed it.
    Closed,
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
    /// A hosted program ended (1.2), with the run it was in where the host knows it. Its views
    /// end once they have sent what it wrote; it stays, with its last screen, until closed.
    Ended {
        program: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run: Option<String>,
        exit: Exit,
    },
    /// A view on this link ended for good (1.2): no stream of it comes again, where a view that
    /// fell behind comes back at its next epoch. Pushed to the link the view is on, subscribed
    /// or not.
    ViewEnded {
        view: u32,
        why: ViewEnd,
    },
    /// A line a held project's chat's hooks wrote, admitted by the host (1.3): pushed, numbered
    /// `heard`, to the subscribed links that hold the project, which reply with
    /// [`Command::ReplyHeard`] where its kind takes a reply. It never carries the chat's token.
    Heard {
        project: String,
        heard: u64,
        line: serde_json::Value,
        /// The host records the line under the project as it holds it, so the reply's record
        /// of it names no project path (1.40, #3400). A host before 1.40 never says it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pathless: bool,
    },
    /// What a confinement's proxy did (1.4). Pushed only to a link that may confine
    /// ([`crate::grants::Power::Confine`]) and that the host named the confinement to (#2444):
    /// the link that made it, one a start's `confinement` named it to (1.16), and a window that
    /// took its chat back with [`Command::ProgramsOf`], or a relayed window's link that took it
    /// back with [`Command::TakeBack`] (1.22).
    Sandbox {
        confinement: String,
        #[serde(flatten)]
        told: SandboxTold,
    },
    /// One look of a project's auto-save did something (1.7): it `saved` the project, or only
    /// fetched, pushed or looked at a request; `moved_tree` when what it did moved files in the
    /// tree (a fast-forward, a save's rebase), so a client reads everything again, and only the
    /// git standing otherwise. Pushed only to a link that may save ([`crate::grants::Power::Save`])
    /// and holds the project.
    Saving {
        project: String,
        saved: bool,
        moved_tree: bool,
    },
    /// The host asks the window which chats are mid-turn in a workspace of a project it saves
    /// (1.7): a repo's auto-save waits for them (ADR 0051), and the turns are on the window's
    /// board. Pushed to the window's links only; answered with [`Command::MidTurn`].
    WhoIsMidTurn {
        question: u64,
        project: String,
        workspace: String,
    },
    /// The link a relaying host reaches the other host over is down, and why (1.17, #2496):
    /// what the client has there, its chats and browsers among them, is on that other host,
    /// which this host is linking to again. Until [`Pushed::LinkBack`] every command it would
    /// send on is refused; nothing is sent later in its place. Pushed once for each time the
    /// link goes down, to every subscriber of the relaying host.
    LinkDown {
        why: String,
        /// The other host answered with another key than the one pinned at its pairing (1.33,
        /// #2679): it is not linked again until the person removes it and adds it anew.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        key_changed: bool,
    },
    /// The link a relaying host reaches the other host over is back (1.17, #2496), with what the
    /// client's link stands on carried over to it: its views draw again, each from a snapshot.
    LinkBack {},
    /// The link a relaying host reaches the other host over could not hold `project` as what
    /// the client's link stands on was carried over to it, in the other host's words (1.30,
    /// #2645): the relaying host no longer holds it there for the client, and the other host
    /// refuses a command for it until the client holds it again. The relaying host's own word,
    /// pushed only to a link that holds the project, and never one the other host said.
    NotHeld {
        project: String,
        why: String,
    },
    /// The engine's download for browser `program`'s start has come to `done` bytes, of `total`
    /// where its server said (1.37, #2207): told a few times a second at most, and once more as
    /// it ends. Pushed only to a link of 1.37 or later that may start a browser
    /// ([`KNOWS_ENGINE_DOWNLOADS`]).
    EngineDownload {
        program: String,
        done: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
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
    HoldProject => "hold_project",
    LetGoOfProject => "let_go_of_project",
    Close => "close",
    Readiness => "readiness",
    Deal => "deal",
    DealtAlready => "dealt_already",
    ReplyHeard => "reply_heard",
    BoardMoved => "board_moved",
    Board => "board",
    Confine => "confine",
    LetGoOfConfinement => "let_go_of_confinement",
    ChangeReach => "change_reach",
    ReachOf => "reach_of",
    StopAll => "stop_all",
    Rearm => "rearm",
    KillSwitch => "kill_switch",
    StartSaving => "start_saving",
    StopSaving => "stop_saving",
    FetchProject => "fetch_project",
    MidTurn => "mid_turn",
    Record => "record",
    Browse => "browse",
    ProgramsOf => "programs_of",
    Quit => "quit",
    TakeOver => "take_over",
    HandBack => "hand_back",
    BrowserInput => "browser_input",
    SizeBrowser => "size_browser",
    LogIn => "log_in",
    TakeBack => "take_back",
    NameWindow => "name_window",
    Unpair => "unpair",
    RelayTo => "relay_to",
    TakeCode => "take_code",
    GiveCode => "give_code",
    CancelEngineDownload => "cancel_engine_download",
    NoteProgram => "note_program",
    LogEnd => "log_end",
});

impl Command {
    /// What a scope must be granted to send this command ([`crate::grants`]). An answer's power
    /// depends on the ask, which only the host knows, so it is [`Power::Answer`] here and
    /// [`serve`] asks the host whether it is a secret's.
    pub fn power(&self) -> Power {
        match self {
            Command::List | Command::Readiness { .. } | Command::KillSwitch => Power::List,
            Command::Attach { .. }
            | Command::Detach { .. }
            | Command::AttachView { .. }
            | Command::DetachView { .. } => Power::View,
            Command::Write { .. } => Power::Write,
            Command::Resize { .. } => Power::Resize,
            Command::Answer { .. } => Power::Answer,
            Command::Stop { .. } | Command::Close { .. } | Command::StopAll => Power::Stop,
            Command::Rearm => Power::Rearm,
            Command::Start(_) | Command::StartProgram(_) => Power::Start,
            Command::HoldProject { .. }
            | Command::LetGoOfProject { .. }
            | Command::Deal { .. }
            | Command::DealtAlready { .. } => Power::Hold,
            Command::StartSaving { .. }
            | Command::StopSaving { .. }
            | Command::FetchProject { .. }
            | Command::MidTurn { .. } => Power::Save,
            Command::ReplyHeard { .. } => Power::Heard,
            Command::BoardMoved { .. } | Command::Board { .. } => Power::Board,
            Command::Confine(_)
            | Command::LetGoOfConfinement { .. }
            | Command::ChangeReach { .. }
            | Command::ReachOf { .. } => Power::Confine,
            Command::Subscribe { .. } | Command::Unsubscribe => Power::Subscribe,
            Command::Record { .. } => Power::Record,
            Command::Browse { .. }
            | Command::TakeOver { .. }
            | Command::HandBack { .. }
            | Command::BrowserInput { .. }
            | Command::SizeBrowser { .. }
            | Command::CancelEngineDownload { .. } => Power::Browser,
            Command::LogIn { .. } => Power::LogIn,
            Command::ProgramsOf { .. } | Command::NoteProgram { .. } => Power::Reattach,
            Command::Quit => Power::Quit,
            Command::TakeBack { .. } => Power::TakeBack,
            Command::NameWindow { .. } => Power::NameWindow,
            Command::Unpair => Power::Unpair,
            Command::RelayTo { .. } => Power::RelayTo,
            Command::TakeCode { .. } | Command::GiveCode { .. } => Power::Code,
            Command::LogEnd => Power::EventLog,
        }
    }
}

impl Command {
    /// **Whether it opens a browser from a kept login, or keeps one** (1.27, #2021): what only
    /// the window on its own device sends ([`crate::grants::Power::KeptLogin`]).
    pub fn uses_kept_login(&self) -> bool {
        match self {
            Command::StartProgram(start) => start.kept.is_some(),
            Command::LogIn { login, .. } => login.keep.is_some(),
            _ => false,
        }
    }

    /// **Whether it is about a browser**: one's start or view, an act of it, its driving, its
    /// login, or its engine's download (1.38, #2050): what a relaying host sends on only to a
    /// host of [`KNOWS_RELAYED_BROWSERS`] or later.
    pub fn is_about_a_browser(&self) -> bool {
        match self {
            Command::StartProgram(start) => start.kind == ProgramKind::Browser,
            Command::AttachView { kind, .. } => *kind == ViewKind::Browser,
            command => matches!(command.power(), Power::Browser | Power::LogIn),
        }
    }
}

impl Pushed {
    /// **The power a link needs to be pushed this** ([`crate::grants`]), or none for what every
    /// subscriber is pushed: a held project's hook lines need [`Power::Heard`], how saving it
    /// goes [`Power::Save`], and what a chat's sandbox did [`Power::Confine`].
    pub fn heard_with(&self) -> Option<Power> {
        match self {
            Pushed::Heard { .. } => Some(Power::Heard),
            Pushed::NotHeld { .. } => Some(Power::Hold),
            Pushed::Saving { .. } | Pushed::WhoIsMidTurn { .. } => Some(Power::Save),
            Pushed::Sandbox { .. } => Some(Power::Confine),
            Pushed::Event(_)
            | Pushed::EngineDownload { .. }
            | Pushed::Missed { .. }
            | Pushed::Ended { .. }
            | Pushed::ViewEnded { .. }
            | Pushed::LinkDown { .. }
            | Pushed::LinkBack {} => None,
        }
    }
}

wire_words!(Answer {
    Chats => "chats",
    Attached => "attached",
    Started => "started",
    ProgramStarted => "program_started",
    Readiness => "readiness",
    Dealt => "dealt",
    Board => "board",
    Programs => "programs",
    Confined => "confined",
    ReachChanged => "reach_changed",
    Reach => "reach",
    KillSwitch => "kill_switch",
    Browsed => "browsed",
    Driving => "driving",
    TakenBack => "taken_back",
    Code => "code",
    Subscribed => "subscribed",
    LogEnd => "log_end",
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
    Ended => "ended",
    ViewEnded => "view_ended",
    Heard => "heard",
    Sandbox => "sandbox",
    Saving => "saving",
    WhoIsMidTurn => "who_is_mid_turn",
    LinkDown => "link_down",
    LinkBack => "link_back",
    NotHeld => "not_held",
    EngineDownload => "engine_download",
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
    /// This link's number in this process: never another link's, live or ended.
    link: u64,
    /// The link has ended ([`Host::link_ended`]).
    ended: std::sync::atomic::AtomicBool,
}

/// The number the next link [`serve`] serves is given.
static NEXT_LINK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Peer {
    /// The scope the link was admitted as ([`crate::auth`]).
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// The version the link agreed on.
    pub fn version(&self) -> Version {
        self.version
    }

    /// **Which link this is**: a number no other link served in this process has, live or
    /// ended, so a host tells links apart by it, never by where a peer is in memory.
    pub fn link(&self) -> u64 {
        self.link
    }

    /// Whether the link has ended. Set before [`Host::link_ended`] is called, so a call the host
    /// finishes after that sees it, and undoes what it made for a link nobody holds.
    pub fn has_ended(&self) -> bool {
        self.ended.load(std::sync::atomic::Ordering::SeqCst)
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
        link: NEXT_LINK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ended: std::sync::atomic::AtomicBool::new(false),
    });
    let host = Arc::new(host);
    let mut calls = Calls::default();
    let served = answer(&mut link, &host, &peer, ui::Serving::new(ui), &mut calls).await;
    // The client is gone: nobody watches its views, so they close now, not once a view's
    // queue has filled; then the host is told.
    peer.views().end();
    peer.ended.store(true, std::sync::atomic::Ordering::SeqCst);
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
    // Each view of this link that ended for good unasked, told whether or not it subscribed.
    let mut view_ends = peer.views().ends();
    loop {
        let next = tokio::select! {
            frame = link.control().next(), if !calls.full() => Next::Frame(frame),
            Some(finished) = calls.next() => Next::Finished(finished),
            pushed = next_pushed(&mut events) => Next::Pushed(pushed),
            Some((view, why)) = next_view_end(&mut view_ends) => {
                Next::Pushed(Some(Pushed::ViewEnded { view: view.0, why }))
            }
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
            let start = Command::StartProgram(Box::new(StartProgram {
                workspace,
                harness,
                ..StartProgram::of(project, ProgramKind::Harness)
            }));
            let answer = host.call(start, peer).await.map(|answer| match answer {
                Answer::ProgramStarted { program, .. } => Answer::Started { chat: program },
                other => other,
            });
            Finished::Answered(reply(id, answer))
        }
        Command::Attach { chat } => {
            let attach = Command::AttachView {
                program: chat,
                kind: ViewKind::Terminal,
                role: None,
                rate: None,
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
    /// The replies to one line a held project's hooks wrote (1.3).
    Heard(u64),
    /// The commands about one confinement (1.4).
    Confinement(String),
    /// The commands about one project's saving (1.7): a stop is never overtaken by the start
    /// before it.
    Saving(String),
    /// What the window asks the event log to write, in the order it asked (1.8).
    Record,
    /// The moves and reads of one held project's kept board, by the project (1.10).
    Board(String),
    /// The acts asked of one browser (1.11): in order among themselves, and never ahead of or
    /// behind its views, so a long wait for text holds up no view of it.
    Browsing(String),
}

impl Lane {
    /// The lane `command` keeps its order in, or `None` when it waits for nobody. An
    /// exhaustive `match`, so a command added later says which it is.
    fn of(command: &Command) -> Option<Lane> {
        match command {
            Command::List
            | Command::Start(_)
            | Command::StartProgram(_)
            | Command::HoldProject { .. }
            | Command::LetGoOfProject { .. }
            | Command::Deal { .. }
            | Command::DealtAlready { .. }
            | Command::StopAll
            | Command::Rearm
            | Command::KillSwitch
            | Command::MidTurn { .. }
            | Command::Quit
            | Command::TakeBack { .. }
            | Command::NameWindow { .. }
            | Command::Unpair
            | Command::RelayTo { .. }
            | Command::TakeCode { .. }
            | Command::GiveCode { .. }
            // Never behind the start it cancels, which waits on the download.
            | Command::CancelEngineDownload { .. } => None,
            Command::Attach { chat: program }
            | Command::Detach { chat: program, .. }
            | Command::AttachView { program, .. }
            | Command::DetachView { program, .. }
            | Command::Write { chat: program, .. }
            | Command::Resize { chat: program, .. }
            | Command::Answer { chat: program, .. }
            | Command::Stop { chat: program }
            | Command::Close { program }
            | Command::Readiness { program }
            // A note follows what the window did to the program before it.
            | Command::NoteProgram { program, .. } => Some(Lane::Program(program.clone())),
            // A confinement's own commands keep their order: an Allow, then what it holds.
            Command::Confine(confine) => Some(Lane::Confinement(confine.confinement.clone())),
            Command::LetGoOfConfinement { confinement }
            | Command::ChangeReach { confinement, .. }
            | Command::ReachOf { confinement } => Some(Lane::Confinement(confinement.clone())),
            // Where the log ends is asked before the subscribe that starts from it (1.41).
            Command::Subscribe { .. } | Command::Unsubscribe | Command::LogEnd => {
                Some(Lane::Subscription)
            }
            // The replies to one line keep their order: a command's frames, one after another.
            Command::ReplyHeard { heard, .. } => Some(Lane::Heard(*heard)),
            Command::StartSaving { project }
            | Command::StopSaving { project }
            | Command::FetchProject { project } => Some(Lane::Saving(project.clone())),
            // A run's start before what is written under it.
            Command::Record { .. } => Some(Lane::Record),
            // A board's moves are made in the order the window made them, and a read of it
            // follows every move sent before it.
            // And what a window takes back follows them too (1.13).
            Command::BoardMoved { project, .. }
            | Command::Board { project }
            | Command::ProgramsOf { project } => Some(Lane::Board(project.clone())),
            // A login is one more of the browser's acts, in their order.
            Command::Browse { program, .. } | Command::LogIn { program, .. } => {
                Some(Lane::Browsing(program.clone()))
            }
            // The person's take-over, input and hand-back keep their order with each other and
            // with the browser's views, never waiting behind an act in flight, which the
            // take-over cuts.
            Command::TakeOver { program }
            | Command::HandBack { program }
            | Command::BrowserInput { program, .. }
            | Command::SizeBrowser { program, .. } => Some(Lane::Program(program.clone())),
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
    if let Command::Rearm = command
        && !scope.may(Power::Rearm)
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "re-arming the kill switch is done in purlis's window only; a `{scope}` link may \
                 stop every agent, never re-arm"
            ),
        });
    }
    if !scope.may(command.power()) {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!("a `{scope}` link may not `{word}`"),
        });
    }
    let a_browser = match command {
        Command::StartProgram(start) => start.kind == ProgramKind::Browser,
        Command::AttachView { kind, .. } => *kind == ViewKind::Browser,
        _ => false,
    };
    // A kept login is the window's alone, on its own device (1.27, 1.38).
    if command.uses_kept_login() && !scope.may(Power::KeptLogin) {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a kept login is opened and kept from purlis's window on its own device only, \
                 and this link is `{scope}`"
            ),
        });
    }
    // A note kept for the window is the window's alone, as taking programs back is (1.39).
    if let Command::StartProgram(start) = command
        && start.note.is_some()
        && !scope.may(Power::Reattach)
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a program's note is kept for purlis's window only, and this link is `{scope}`"
            ),
        });
    }
    if a_browser && !scope.may(Power::Browser) {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a browser is started and watched from purlis's window only, and this link is \
                 `{scope}`"
            ),
        });
    }
    if let Command::StartProgram(start) = command
        && start.launch.is_some()
        && !scope.may(Power::Launch)
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a start the window composed is taken from purlis's window, or its host's relay, \
                 only, and this link is `{scope}`"
            ),
        });
    }
    if let Command::StartProgram(start) = command
        && start
            .launch
            .as_ref()
            .is_some_and(|launch| launch.harness.is_some())
        && !scope.may(Power::LaunchAHarness)
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a chat's harness on this host is started by its names, composed here with this \
                 machine's own sandbox, and never from a launch a `{scope}` link hands it"
            ),
        });
    }
    if let Command::StartProgram(start) = command
        && (start.confinement.is_some() || !start.alone.is_empty())
        && !scope.may(Power::Confine)
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a chat's confinement is named to purlis's window, or its host's relay, only, and \
                 this link is `{scope}`"
            ),
        });
    }
    if let Command::StartProgram(start) = command
        && start.without_sandbox
        && !scope.may(Power::OptOut)
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "a chat is started without the sandbox only from purlis's window, and this link \
                 is `{scope}`"
            ),
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

async fn next_view_end(
    ends: &mut Option<mpsc::UnboundedReceiver<(crate::view::ViewId, ViewEnd)>>,
) -> Option<(crate::view::ViewId, ViewEnd)> {
    match ends {
        Some(ends) => ends.recv().await,
        None => std::future::pending().await,
    }
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
        self.start_program_told(start)
            .await
            .map(|started| started.program)
    }

    /// [`Client::start_program`], answering with all the host said of what it started: its
    /// number and its process too, where it says them (1.2).
    pub async fn start_program_told(
        &self,
        start: StartProgram,
    ) -> Result<ProgramStarted, CallError> {
        match self.call(Command::StartProgram(Box::new(start))).await? {
            Answer::ProgramStarted {
                program,
                number,
                process,
                confined,
                sandbox,
            } => Ok(ProgramStarted {
                program,
                number,
                process,
                confined,
                sandbox,
            }),
            other => Err(unexpected(&other)),
        }
    }

    /// Hold `project`, which is at `path` on the host's machine, so programs start in it (1.2).
    pub async fn hold_project(&self, project: &str, path: &str) -> Result<(), CallError> {
        self.done(Command::HoldProject {
            project: project.into(),
            project_path: path.into(),
        })
        .await
    }

    /// Let go of `project` (1.2).
    pub async fn let_go_of_project(&self, project: &str) -> Result<(), CallError> {
        self.done(Command::LetGoOfProject {
            project: project.into(),
        })
        .await
    }

    /// Save `project` from now on, in the host (1.7).
    pub async fn start_saving(&self, project: &str) -> Result<(), CallError> {
        self.done(Command::StartSaving {
            project: project.into(),
        })
        .await
    }

    /// Stop saving `project` in the host (1.7).
    pub async fn stop_saving(&self, project: &str) -> Result<(), CallError> {
        self.done(Command::StopSaving {
            project: project.into(),
        })
        .await
    }

    /// Fetch what came in for `project`, which the host saves (1.7).
    pub async fn fetch_project(&self, project: &str) -> Result<(), CallError> {
        self.done(Command::FetchProject {
            project: project.into(),
        })
        .await
    }

    /// End `program` and forget it (1.2).
    pub async fn close(&self, program: &str) -> Result<(), CallError> {
        self.done(Command::Close {
            program: program.into(),
        })
        .await
    }

    /// Whether `program` is ready to be typed at (1.2).
    pub async fn readiness(&self, program: &str) -> Result<Readiness, CallError> {
        match self
            .call(Command::Readiness {
                program: program.into(),
            })
            .await?
        {
            Answer::Readiness(readiness) => Ok(readiness),
            other => Err(unexpected(&other)),
        }
    }

    /// Deal the next number in `project` (1.3).
    pub async fn deal(&self, project: &str) -> Result<u32, CallError> {
        match self
            .call(Command::Deal {
                project: project.into(),
            })
            .await?
        {
            Answer::Dealt { number } => Ok(number),
            other => Err(unexpected(&other)),
        }
    }

    /// Keeps `note` as the window's note of `program` in the host (1.39, #2025).
    pub async fn note_program(&self, program: &str, note: ProgramNote) -> Result<(), CallError> {
        self.done(Command::NoteProgram {
            program: program.into(),
            note,
        })
        .await
    }

    /// Every program `project` still has, running or ended, as a window that comes back takes
    /// them back (1.13).
    pub async fn programs_of(&self, project: &str) -> Result<Vec<HeldProgram>, CallError> {
        match self
            .call(Command::ProgramsOf {
                project: project.into(),
            })
            .await?
        {
            Answer::Programs(programs) => Ok(programs),
            other => Err(unexpected(&other)),
        }
    }

    /// This link is `window`'s (1.22): the confinements of the chats that window started over
    /// the links before, named to this link again.
    pub async fn take_back(&self, window: &str) -> Result<Vec<String>, CallError> {
        match self
            .call(Command::TakeBack {
                window: window.into(),
            })
            .await?
        {
            Answer::TakenBack { confinements } => Ok(confinements),
            other => Err(unexpected(&other)),
        }
    }

    /// The window is quitting: the host ends every program as a quit and stops (1.13).
    pub async fn quit(&self) -> Result<(), CallError> {
        self.done(Command::Quit).await
    }

    /// Take this link's desktop out of the runner's peers (1.25, #2525): no later link from it
    /// is admitted. Refused by every host but a runner's, on its link from that desktop.
    pub async fn unpair(&self) -> Result<(), CallError> {
        self.done(Command::Unpair).await
    }

    /// Ask the desktop's host to relay this link to the runner it keeps as `runner` (1.33,
    /// #2679): the link's first command, before [`Client::subscribe`].
    pub async fn relay_to(&self, runner: &str) -> Result<(), CallError> {
        self.done(Command::RelayTo {
            runner: runner.to_owned(),
        })
        .await
    }

    /// Have the host take into `repo` of `project` the refs `refs` names (1.34, #2046), from the
    /// bundle this client writes on the code stream the host opens for `transfer`, which the
    /// client takes from its streams ([`crate::view::Viewer::accept_any`]), checking out
    /// `checkout` in a repository that has nothing checked out yet. Answers every branch and
    /// snapshot the repository holds after it.
    pub async fn take_code(
        &self,
        project: &str,
        repo: &str,
        transfer: u64,
        refs: Vec<CodeRef>,
        checkout: Option<String>,
    ) -> Result<Vec<CodeRef>, CallError> {
        self.code(Command::TakeCode {
            project: project.to_owned(),
            repo: repo.to_owned(),
            transfer,
            refs,
            checkout,
        })
        .await
    }

    /// Have the host write a bundle of `repo`'s branches in `project` on the code stream it
    /// opens for `transfer` (1.34, #2046), leaving out what `have` reaches. Answers the branches
    /// the bundle is of.
    pub async fn give_code(
        &self,
        project: &str,
        repo: &str,
        transfer: u64,
        have: Vec<String>,
    ) -> Result<Vec<CodeRef>, CallError> {
        self.code(Command::GiveCode {
            project: project.to_owned(),
            repo: repo.to_owned(),
            transfer,
            have,
        })
        .await
    }

    async fn code(&self, command: Command) -> Result<Vec<CodeRef>, CallError> {
        match self.call(command).await? {
            Answer::Code { heads } => Ok(heads),
            other => Err(unexpected(&other)),
        }
    }

    /// Cancel the engine's download for browser `program`'s start (1.37, #2207): nothing of it
    /// is kept.
    pub async fn cancel_engine_download(&self, program: &str) -> Result<(), CallError> {
        self.done(Command::CancelEngineDownload {
            program: program.to_owned(),
        })
        .await
    }

    /// Throw the kill switch: every program the host runs ends, and none starts until the
    /// window re-arms (1.5).
    pub async fn stop_all(&self) -> Result<KillSwitch, CallError> {
        self.kill_switch_after(Command::StopAll).await
    }

    /// Let programs start again (1.5): refused on every link but the window's.
    pub async fn rearm(&self) -> Result<KillSwitch, CallError> {
        self.kill_switch_after(Command::Rearm).await
    }

    /// **Where the host's own event log ends now** (1.41, #3393): its device, and the `seq` of
    /// its last line.
    pub async fn log_end(&self) -> Result<(String, u64), CallError> {
        match self.call(Command::LogEnd).await? {
            Answer::LogEnd { device, seq } => Ok((device, seq)),
            other => Err(unexpected(&other)),
        }
    }

    /// Whether every agent is stopped (1.5).
    pub async fn kill_switch(&self) -> Result<KillSwitch, CallError> {
        self.kill_switch_after(Command::KillSwitch).await
    }

    async fn kill_switch_after(&self, command: Command) -> Result<KillSwitch, CallError> {
        match self.call(command).await? {
            Answer::KillSwitch(switch) => Ok(switch),
            other => Err(unexpected(&other)),
        }
    }

    /// Send `command` now, behind every command sent before it on this client, and answer with
    /// a future of its answer: what keeps the order of commands sent from one thread without
    /// waiting for each answer (a host keeps the order of those of one lane).
    pub fn call_now(
        &self,
        command: Command,
    ) -> impl Future<Output = Result<Answer, CallError>> + Send + 'static {
        let (reply, answer) = oneshot::channel();
        let sent = self.asks.send(Ask::Session(command, reply));
        async move {
            sent.map_err(|_| CallError::Closed("the link is closed".into()))?;
            answer
                .await
                .map_err(|_| CallError::Closed("the link is closed".into()))?
        }
    }

    /// Attach a view of `kind` on `program`: its stream comes on the [`Acceptor`]
    /// [`Client::new`] gave. A host one version behind refuses it `unknown_command`;
    /// [`Client::attach`] is its word for a terminal view.
    pub async fn attach_view(&self, program: &str, kind: ViewKind) -> Result<Attached, CallError> {
        let attach = Command::AttachView {
            program: program.into(),
            kind,
            role: None,
            rate: None,
        };
        match self.call(attach).await? {
            Answer::Attached(attached) => Ok(attached),
            other => Err(unexpected(&other)),
        }
    }

    /// Attach a browser view of `program` as `role`, at most `rate` frames a second when given
    /// (1.6): its stream of whole frames comes on the [`Acceptor`] [`Client::new`] gave.
    pub async fn attach_browser_view(
        &self,
        program: &str,
        role: ViewRole,
        rate: Option<u32>,
    ) -> Result<Attached, CallError> {
        let attach = Command::AttachView {
            program: program.into(),
            kind: ViewKind::Browser,
            role: Some(role),
            rate,
        };
        match self.call(attach).await? {
            Answer::Attached(attached) => Ok(attached),
            other => Err(unexpected(&other)),
        }
    }

    /// One act of browser `program`'s, under the act's own id `call_id` (1.11): what the browser
    /// answered, or its driver's refusal ([`BROWSER_REFUSED`]).
    pub async fn browse(
        &self,
        program: &str,
        call_id: &str,
        act: crate::browsing::Act,
    ) -> Result<crate::browsing::Browsed, CallError> {
        match self
            .call(Command::Browse {
                program: program.into(),
                call_id: call_id.into(),
                act,
                reach: None,
            })
            .await?
        {
            Answer::Browsed(browsed) => Ok(*browsed),
            other => Err(unexpected(&other)),
        }
    }

    /// purlis performs `login` in browser `program` (1.18, #2016): the page it left once it
    /// handed the browser back, or why the login did not go through ([`BROWSER_REFUSED`]).
    pub async fn log_in(
        &self,
        program: &str,
        login: crate::browsing::LogIn,
    ) -> Result<crate::browsing::Browsed, CallError> {
        match self
            .call(Command::LogIn {
                program: program.into(),
                login,
            })
            .await?
        {
            Answer::Browsed(browsed) => Ok(*browsed),
            other => Err(unexpected(&other)),
        }
    }

    /// The person takes browser `program` over (1.14): who drives it now.
    pub async fn take_over(&self, program: &str) -> Result<crate::browsing::Driver, CallError> {
        self.driving(Command::TakeOver {
            program: program.into(),
        })
        .await
    }

    /// The person hands browser `program` back (1.14): who drives it now.
    pub async fn hand_back(&self, program: &str) -> Result<crate::browsing::Driver, CallError> {
        self.driving(Command::HandBack {
            program: program.into(),
        })
        .await
    }

    async fn driving(&self, command: Command) -> Result<crate::browsing::Driver, CallError> {
        match self.call(command).await? {
            Answer::Driving { driver } => Ok(driver),
            other => Err(unexpected(&other)),
        }
    }

    /// One event of the person's in browser `program`, which they drive (1.14).
    pub async fn browser_input(
        &self,
        program: &str,
        input: crate::browsing::Input,
    ) -> Result<(), CallError> {
        self.done(Command::BrowserInput {
            program: program.into(),
            input,
        })
        .await
    }

    /// The person sets the size of browser `program`'s pages, in CSS pixels (1.14).
    pub async fn size_browser(
        &self,
        program: &str,
        width: u16,
        height: u16,
    ) -> Result<(), CallError> {
        self.done(Command::SizeBrowser {
            program: program.into(),
            width,
            height,
        })
        .await
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
        // A view's end is the link's own word, not an event: it is kept whether or not the
        // client subscribed, while the queue has room for it beside the word it may need.
        if let Pushed::ViewEnded { .. } = pushed {
            if self.queue.capacity() > 1 {
                let _ = self.queue.try_send(Told::Pushed(pushed));
            }
            return false;
        }
        if !self.live {
            return false;
        }
        if self.queue.capacity() > 1 {
            match &pushed {
                Pushed::Event(event) => {
                    self.cursor.insert(event.device_id.clone(), event.seq);
                }
                // The gap is told by this very frame: reading on from before it would only
                // tell it again.
                Pushed::Missed {
                    device, resumes_at, ..
                } => {
                    self.cursor
                        .insert(device.clone(), resumes_at.saturating_sub(1));
                }
                // None is an event of the log: the cursor stays where it is.
                Pushed::Ended { .. }
                | Pushed::ViewEnded { .. }
                | Pushed::Heard { .. }
                | Pushed::Sandbox { .. }
                | Pushed::Saving { .. }
                | Pushed::WhoIsMidTurn { .. }
                | Pushed::LinkDown { .. }
                | Pushed::LinkBack {}
                | Pushed::NotHeld { .. }
                | Pushed::EngineDownload { .. } => {}
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Driving a browser's pages: the agent's view of a page and its actions (#2010).
//!
//! A [`Browser`] takes typed [`Call`]s and gives [`Answer`]s. It speaks DevTools to its engine
//! itself and hands none of it out: what a caller gets is a snapshot of a page with a
//! reference for each element, the elements a find matched, a screenshot, and the outcome of
//! an action. No call runs script the caller wrote; the small scripts purlis runs in a page are
//! constants, and the caller's words reach them only as values. Every one of them runs in a
//! world of purlis's own in the frame it acts on (#2138, #2613), so what it reads and does is
//! the document's own, whatever the page's scripts redefine.
//!
//! - **References.** A snapshot deals each element a reference (`e12`). It names the same
//!   element in every later snapshot of the same document, and dies when its frame loads
//!   another document: an action on it is then refused in words.
//! - **Pages in pages.** A snapshot is read from a line, so many lines at a time, and scoped to
//!   one element; a find by text or role answers references with the line to read from. Frames
//!   are named in the tree, and their elements are acted on like any other.
//! - **What changed, and what is not shown** (#2137). [`Call::Changes`] answers only the lines
//!   new or gone since the page was last read, so an action need not be followed by a whole
//!   snapshot; and a snapshot or a find marks each element the page lays out wholly outside
//!   what its viewport shows `(out of view)`, so the agent knows to scroll to it, and each one
//!   wholly outside what a box it is in that scrolls, or the frame it is in, shows (#2564), so
//!   it knows to scroll that box.
//! - **Settling.** Every action answers once its page has settled (nothing loading, nothing
//!   requested still open, quiet a while, its document complete), or says what it still waited
//!   for when the time allowed ran out. An action that opens a page answers with that page,
//!   which becomes the current one.
//! - **One act per call id.** A call carries a [`CallId`]; a retry with the same id answers
//!   again what the first answered and acts on nothing.
//! - **Blanked elements.** [`Browser::blank`] adds an element to the set every snapshot,
//!   find, wait and screenshot honours: purlis fills it for a login, or the person typed into
//!   it. Its value appears in no answer.
//! - **Only web addresses.** Every navigation takes a [`WebAddress`]; a page the engine opens at
//!   any other address (a link or a redirect a page offered) is closed and never listed.
//! - **Where it may go is judged** (#2018). A browser made with a [`Judge`] holds every http and
//!   https request of every page until the judge has heard what it is ([`Outbound`]): a read,
//!   an address the agent wrote, or something sent; and the agent's typing is judged as
//!   something sent to the site of the document it goes into. A request the judge asks about is
//!   failed, and an act it asks about does nothing; the sites it asked about are in the next
//!   answer.
//! - **One driver at a time** (#2014). The agent drives through its calls until the person
//!   takes the browser over ([`Browser::take_over`]); from then every call is refused, a call in
//!   flight answers that it was interrupted, and only the person's own input
//!   ([`Browser::person_input`]) reaches the page, each field they type into blanked before
//!   their keys reach it. The lease never comes back by itself: [`Browser::hand_back`] returns
//!   it, and every reference dealt before it is void.
//! - **Logins are purlis's** (#2016). [`Browser::log_in`] is the third driver: it takes the
//!   lease, goes to the declared address, fills only while the page is on the declared origin,
//!   submits, waits for the password field to go, clears what it filled where the login did not
//!   go through, and hands the browser back. What it filled is kept out of every answer about
//!   every page of the browser for as long as the browser lives, and each field it filled is
//!   blanked before the value goes in.

mod blank;
mod browser;
mod clipped;
mod drawn;
pub mod fit;
mod kept;
mod keys;
mod ledger;
mod screencast;
mod settle;
mod site;
mod tree;

use std::fmt;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

pub use browser::{Browser, LOGIN_WAIT, LoginEnded, origin_of};
pub use screencast::QUALITY;

use crate::WebAddress;

/// The id a call carries, so that a retry after a lost answer never acts twice.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallId(String);

/// Why a call's id was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a call's id is between 1 and {MAX_CALL_ID} characters")]
pub struct NotCallId;

/// The longest call id, in characters.
pub const MAX_CALL_ID: usize = 128;

impl CallId {
    /// `id`, when it is between 1 and [`MAX_CALL_ID`] characters.
    pub fn new(id: impl Into<String>) -> Result<Self, NotCallId> {
        let id = id.into();
        if id.is_empty() || id.chars().count() > MAX_CALL_ID {
            return Err(NotCallId);
        }
        Ok(Self(id))
    }

    /// The id as given.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A reference to one element of a page, as a snapshot or a find dealt it (`e12`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ref(String);

impl Ref {
    /// The reference written as it was dealt.
    pub fn new(written: impl Into<String>) -> Self {
        Self(written.into())
    }

    /// The reference as dealt.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Who drives a browser (#2014): the agent, through its calls, the person who took it over, or
/// purlis while it performs a login (#2016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Agent,
    Person,
    Purlis,
}

/// **A login purlis performs** (#2016): the declared address and the origin it fills only on,
/// and the values from the vault. The values are wiped from memory when the login is dropped,
/// and never shown by `Debug`.
pub struct Login {
    /// The declared login page.
    pub address: WebAddress,
    /// The origin it fills on alone (`https://host[:port]`), as a page's `URL.origin` spells it.
    pub origin: String,
    /// The user name, where the page asks for one.
    pub user: Option<zeroize::Zeroizing<String>>,
    pub password: zeroize::Zeroizing<String>,
    /// Whether the browser may be signed in already, opened from kept logins (#2021): then
    /// nothing is filled where the page asks for no password.
    pub only_if_asked: bool,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("address", &self.address.as_str())
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

/// What a frame showed: its page, and its documents (a number that changes each time the page
/// or any frame inside it loads another document, or a frame in it goes). Input aimed at a frame
/// names it, and input whose page has moved on, in any of its frames, is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aim {
    pub page: PageId,
    pub document: u64,
}

/// One frame of the current page for the person to watch: a JPEG, and what it showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Painted {
    pub aim: Aim,
    /// The address of the document the page has loaded, as the engine records it (#2300): what
    /// the host cuts to the origin a browser view's chip names.
    pub address: String,
    pub jpeg: Vec<u8>,
}

/// What the person's mouse did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Moved,
    Pressed,
    Released,
    Wheel,
}

/// The mouse button an event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    None,
    Left,
    Middle,
    Right,
}

/// The keys held with an input event, as bits: [`ALT`], [`CONTROL`], [`META`], [`SHIFT`].
pub type Modifiers = u8;

/// Alt (Option on a Mac) held.
pub const ALT: Modifiers = 1;
/// Control held.
pub const CONTROL: Modifiers = 2;
/// Meta (Command on a Mac) held.
pub const META: Modifiers = 4;
/// Shift held.
pub const SHIFT: Modifiers = 8;

/// One event of the person's, aimed at what a frame showed (#2014). Points are CSS pixels of
/// the page's viewport.
#[derive(Debug, Clone, PartialEq)]
pub enum PersonInput {
    Mouse {
        aim: Aim,
        kind: MouseKind,
        x: f64,
        y: f64,
        button: MouseButton,
        /// 1 for a click, 2 for the second of a double click; 0 for a move or the wheel.
        clicks: u8,
        modifiers: Modifiers,
        /// How far the wheel turned, in CSS pixels.
        delta_x: f64,
        delta_y: f64,
    },
    Key {
        aim: Aim,
        /// Pressed (`true`) or let go.
        down: bool,
        /// The key as the window names it (`a`, `Enter`, `ArrowLeft`).
        key: String,
        /// The physical key (`KeyA`).
        code: String,
        /// The platform's key code for it.
        key_code: u32,
        modifiers: Modifiers,
    },
}

impl PersonInput {
    /// What the event was aimed at.
    pub fn aim(&self) -> &Aim {
        match self {
            PersonInput::Mouse { aim, .. } | PersonInput::Key { aim, .. } => aim,
        }
    }
}

/// One of a browser's pages, as [`Call::Pages`] lists it (`p2`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageId(String);

impl PageId {
    /// The page's id written as it was listed.
    pub fn new(written: impl Into<String>) -> Self {
        Self(written.into())
    }

    /// The id as listed.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a [`Call::Type`] types into: an element, or whatever has the focus.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// The element, or `None` for the one that has the focus.
    pub into: Option<Ref>,
    /// The text, typed as it is.
    pub text: String,
    /// Whether what the field holds is replaced rather than added to.
    pub replace: bool,
}

/// Where a drag ends.
#[derive(Debug, Clone, PartialEq)]
pub enum DragTo {
    /// On an element's centre.
    Element(Ref),
    /// So many CSS pixels right (`x`) and down (`y`) of where it started.
    Offset { x: f64, y: f64 },
}

/// One thing the agent asks of its browser. A call that names no page acts on the current one.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// Opens a new page at `address`, which becomes the current page.
    Open {
        address: WebAddress,
    },
    /// Sends a page to `address`.
    Navigate {
        page: Option<PageId>,
        address: WebAddress,
    },
    /// The page's tree from line `from` (1 for the start), at most `lines` lines (the browser's
    /// default when `None`), inside `within` when given.
    Snapshot {
        page: Option<PageId>,
        within: Option<Ref>,
        from: usize,
        lines: Option<usize>,
    },
    /// The elements whose name or value holds `text`, or whose role is `role`, or both.
    Find {
        page: Option<PageId>,
        text: Option<String>,
        role: Option<String>,
        within: Option<Ref>,
    },
    /// The lines of the page's tree new or gone since the page was last read, by a snapshot, a
    /// find or an earlier one of these (#2137): each new one with its reference and the line it
    /// is read at. A page not read before, or since gone on to another document, answers its
    /// tree's first lines. Kept by id like an act: a retry answers what the first answered.
    Changes {
        page: Option<PageId>,
    },
    Click {
        target: Ref,
    },
    DoubleClick {
        target: Ref,
    },
    Hover {
        target: Ref,
    },
    /// Types into each field in turn.
    Type {
        page: Option<PageId>,
        fields: Vec<Field>,
    },
    /// Presses each key or chord in turn (`Enter`, `Shift+Tab`, `Control+a`).
    Press {
        page: Option<PageId>,
        keys: Vec<String>,
    },
    /// Chooses `options` (by value or by label) in a drop-down.
    Select {
        target: Ref,
        options: Vec<String>,
    },
    Drag {
        from: Ref,
        to: DragTo,
    },
    /// Scrolls by `x` and `y` CSS pixels over `at` (the page's middle when `None`); with no
    /// distance, brings `at` into view.
    Scroll {
        page: Option<PageId>,
        at: Option<Ref>,
        x: f64,
        y: f64,
    },
    /// Goes back to the page's previous address.
    Back {
        page: Option<PageId>,
    },
    /// Waits up to `within` for `text` to show on the page, or with `gone`, to show no longer.
    WaitForText {
        page: Option<PageId>,
        text: String,
        gone: bool,
        within: Duration,
    },
    /// The browser's pages.
    Pages,
    /// Makes `page` the current page.
    SwitchPage {
        page: PageId,
    },
    /// A picture of what the page shows now, as PNG, with every blanked element painted over.
    Screenshot {
        page: Option<PageId>,
    },
}

impl Call {
    /// Whether the call only reads (and so acts on nothing a retry could act on twice).
    pub fn only_reads(&self) -> bool {
        matches!(
            self,
            Call::Snapshot { .. }
                | Call::Find { .. }
                | Call::WaitForText { .. }
                | Call::Pages
                | Call::Screenshot { .. }
        )
    }
}

/// Whether a page had settled when its call answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    Yes,
    /// Not within the time allowed: what it still waited for, in words.
    No(Vec<String>),
}

/// One page, as an answer names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    pub id: PageId,
    pub address: String,
    pub title: String,
    /// Whether it is the browser's current page.
    pub current: bool,
}

/// What an answer holds beyond its page and settling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    Nothing,
    /// A snapshot's or a find's lines.
    Text(String),
    Pages(Vec<PageInfo>),
    /// A screenshot, PNG.
    Image(Vec<u8>),
    /// Whether a waited-for text showed (or, waiting for it to go, went).
    Seen(bool),
}

/// A call's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The page the call acted on, as it is now.
    pub page: Option<PageInfo>,
    pub settled: Settled,
    /// Pages the call opened; the last of them is now the current page.
    pub opened: Vec<PageInfo>,
    pub content: Content,
    /// More the agent should know, in words (a page that did not load, say).
    pub note: Option<String>,
    /// Whether this answers a retry: the call did not act again.
    pub repeated: bool,
    /// The sites the judge asked the person about since the last answer, each once (#2018):
    /// what the browser did not reach or send to, or where it typed nothing, until they say.
    pub asked: Vec<String>,
}

/// **What a request a page is about to send is**, as the driver tells the browser's judge
/// (#2018): its address, its method, why it is sent, and who drives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outbound {
    pub address: String,
    pub method: String,
    pub why: Why,
    pub by: Driver,
}

/// Why a page sends a request (#2018).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The page reads: a link it offered, an image, a script, a frame.
    Reads,
    /// An address the agent wrote: its open or navigate, and where that address redirects,
    /// while the act lasts.
    Addressed,
    /// It carries something: a form submitted, a request with a body, or the agent's typing
    /// on the page.
    Sends,
}

/// What the judge says of a request (#2018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// Sent.
    Go,
    /// Not sent: the person is asked about `site` (`host:port`), and the act's answer says so.
    Ask { site: String },
}

/// **The browser's judge** (#2018): told each request a page is about to send, and the page an
/// agent is about to type on, and says whether it goes. It decides; the driver only tells it
/// what each request is.
pub type Judge = std::sync::Arc<dyn Fn(&Outbound) -> Gate + Send + Sync>;

/// A call refused, or one whose effect is not known, in words that say what to do.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the browser has no page open: open one")]
    NoPage,
    #[error("nothing on the page has the focus")]
    NothingFocused,
    #[error("the page cannot tell which field has the focus")]
    FocusUnclear,
    /// The page's tree, read to keep what the person types out of the agent's answers, was not
    /// read within the key's limit (#3043): the key was not sent.
    #[error(
        "purlis could not read this page in time to keep what you type out of the agent's \
         answers, so the key was held back: type it again in a moment"
    )]
    PageUnreadInTime,
    #[error(
        "the page has something focused that purlis cannot see, so the key was held back: click the field you mean to type into"
    )]
    FocusUnseen,
    #[error(
        "the call acted in part, so the page's state is not known ({done}): {why}. Take a snapshot"
    )]
    Partly {
        /// What was done before it stopped.
        done: String,
        /// Why it stopped.
        why: String,
    },
    #[error("there is no page {0}: list the pages to see them")]
    NoSuchPage(PageId),
    #[error("{0} is not a reference this browser dealt: take a snapshot")]
    NoSuchRef(Ref),
    #[error("{0} is from an earlier document of its page: take a new snapshot")]
    StaleRef(Ref),
    #[error("{0} is not shown on the page now (hidden, or gone): take a new snapshot")]
    NotShown(Ref),
    #[error("{0} is not a drop-down: click its options instead")]
    NotADropDown(Ref),
    #[error("{target} has no option {missing}; its options are: {options}")]
    NoSuchOption {
        target: Ref,
        missing: String,
        options: String,
    },
    #[error("{0} takes one option at a time")]
    OneOption(Ref),
    #[error("{0}")]
    NotAKey(String),
    #[error("the page has no earlier address to go back to")]
    NoEarlierPage,
    #[error("the earlier address is not a web address, so the browser does not go back to it")]
    EarlierNotWeb,
    #[error("the call's id {0} was given to a different call: give each call its own id")]
    IdReused(String),
    #[error("the engine refused: {0}")]
    Engine(String),
    /// What the engine answered was too large to read, and only this call failed (#2694).
    #[error(
        "the page's answer to this call was too large for purlis to read, so this call failed; \
         the browser still answers: read a part of the page (a snapshot within an element), or \
         go on with another call"
    )]
    TooLarge,
    #[error("the browser has ended: open a new one")]
    Ended,
    #[error(
        "the engine did not answer in time, so whether the call acted is not known: take a snapshot"
    )]
    Unknown,
    #[error("the screenshot is withheld because a blanked field could not be found on it: {0}")]
    Withheld(String),
    #[error(
        "the person has taken this browser over, so it takes no call from you: do something else, and use the browser again once they hand it back"
    )]
    PersonDriving,
    #[error(
        "the person took this browser over while the call ran, so it was interrupted and the page's state is not known: once they hand it back, take a new snapshot"
    )]
    Interrupted,
    #[error(
        "{0} was dealt before the browser last changed hands (the person took it over, or purlis logged it in): take a new snapshot"
    )]
    HandedBack(Ref),
    #[error("the person is not driving this browser: take it over first")]
    NotDriving,
    #[error(
        "purlis is logging this browser in, so it takes no call from you until the login answers: wait for it, then take a new snapshot"
    )]
    PurlisDriving,
    /// A login purlis performed did not go through (#2016), in words that name no value.
    #[error("{0}")]
    NotLoggedIn(String),
    #[error("the page changed before this input reached it, so it was dropped")]
    Missed,
    /// What the field being typed into holds could not be read (#2138): the key waits, since
    /// what it puts in could not be kept out.
    #[error("the field being typed into could not be read, so the key waits")]
    FieldUnread,
    /// The key reached the field, and what the field held then could not be read (#2138): it
    /// is read again as the browser is handed back.
    #[error(
        "the key reached the page, but the field it went into could not be read after it; it is \
         read again when the browser is handed back"
    )]
    FieldUnreadAfterKey,
    /// A native drag carried a text kept out of the agent's answers (#2136): cancelled.
    #[error("what this drag carries is kept out of your answers, so the drag was cancelled")]
    DragKeptOut,
}

/// How long and how much, all configurable (from Settings, by the session host).
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    /// How long a page must be quiet to have settled.
    pub quiet: Duration,
    /// How long an action waits for its page to settle before it says it has not.
    pub settle_within: Duration,
    /// How many lines a snapshot gives when the call does not say.
    pub snapshot_lines: usize,
    /// The most lines a snapshot gives, whatever the call says.
    pub most_snapshot_lines: usize,
    /// The longest a wait for text may wait.
    pub longest_wait: Duration,
    /// How many answers to actions are kept by call id for retries (an action's answer holds
    /// no page content, so each is small).
    pub answers_kept: usize,
    /// How long a login purlis performs waits for its form to show, and then for its password
    /// field to go (#2016).
    pub login_within: Duration,
    /// How long a drag waits after its last move for the engine to say its own drag started
    /// (#2136).
    pub drag_starts_within: Duration,
    /// How long a key of the person's waits for its page's tree to be read before it is held
    /// back, and the person told (#3043): less than the window waits for the key's answer.
    pub key_reads_within: Duration,
    /// How long a read of a page's whole tree for the person's keys may take, past the key it
    /// was made for (#3043): a large page's read goes on, for the next key.
    pub tree_reads_within: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            quiet: Duration::from_millis(500),
            settle_within: Duration::from_secs(10),
            snapshot_lines: 300,
            most_snapshot_lines: 2000,
            longest_wait: Duration::from_secs(60),
            answers_kept: 256,
            login_within: LOGIN_WAIT,
            drag_starts_within: Duration::from_millis(150),
            key_reads_within: Duration::from_secs(20),
            tree_reads_within: Duration::from_secs(120),
        }
    }
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

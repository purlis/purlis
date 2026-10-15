// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Driving a browser's pages: the agent's view of a page and its actions (#2010).
//!
//! A [`Browser`] takes typed [`Call`]s and gives [`Answer`]s. It speaks DevTools to its engine
//! itself and hands none of it out: what a caller gets is a snapshot of a page with a
//! reference for each element, the elements a find matched, a screenshot, and the outcome of
//! an action. No call runs script the caller wrote; the few small scripts purlis runs in a page
//! (whether its document has loaded; choosing a drop-down's options) are constants in this
//! module, and the caller's words reach them only as values.
//!
//! - **References.** A snapshot deals each element a reference (`e12`). It names the same
//!   element in every later snapshot of the same document, and dies when its frame loads
//!   another document: an action on it is then refused in words.
//! - **Pages in pages.** A snapshot is read from a line, so many lines at a time, and scoped to
//!   one element; a find by text or role answers references with the line to read from. Frames
//!   are named in the tree, and their elements are acted on like any other.
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

mod blank;
mod browser;
mod keys;
mod ledger;
mod screencast;
mod settle;
mod tree;

use std::fmt;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

pub use browser::Browser;
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
}

/// A call refused, or one whose effect is not known, in words that say what to do.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the browser has no page open: open one")]
    NoPage,
    #[error("nothing on the page has the focus")]
    NothingFocused,
    #[error("the page cannot tell which field has the focus")]
    FocusUnclear,
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
    #[error("the browser has ended: open a new one")]
    Ended,
    #[error(
        "the engine did not answer in time, so whether the call acted is not known: take a snapshot"
    )]
    Unknown,
    #[error("the screenshot is withheld because a blanked field could not be found on it: {0}")]
    Withheld(String),
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
        }
    }
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

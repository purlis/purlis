// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser's life, who drives it, and what it was asked, never what it holds** (#2293,
//! #2842, spec #2008's Events).
//!
//! The window tells the event log's one writer what becomes of a browser, each a [`Said`]:
//!
//! - that it opened (`browser.opened`), and how it ended: closed (`browser.closed`) by the chat,
//!   the person, going unused (`idle`) or the kill switch, or crashed (`browser.crashed`, ended
//!   `by` `crash`);
//! - that the person pinned it past its chat, or unpinned it (`browser.pinned`,
//!   `browser.unpinned`);
//! - who drives it now (`browser.driver_changed`: the agent, the person who took it over, or
//!   purlis while it performs a login), that the person or purlis typed into it
//!   (`browser.typed`, with the page's origin), and that the person handed it to a chat
//!   (`browser.handed_over`);
//! - each call a chat's browser tool made of it (`browser.call`): which call, the origin of the
//!   page it acted on, and whether it was done or refused.
//!
//! Each is written under the chat whose browser it is, and with no chat for a browser no chat
//! uses.
//!
//! **A closed set, composed here.** The window sends a variant, a browser's id and words from
//! closed sets, and the body is written from them alone: there is no field for a key, a value,
//! an argument, a title or any of a page's text, a field more is not read, and a browser named
//! by anything but a program's id is refused. An address reaches the log only as its origin
//! (`scheme://host[:port]`), and one sent with anything more, a path, a query or a user, is
//! refused. So nothing the person, purlis or the agent typed, nothing a page holds and no
//! address beyond its origin reaches the log by this way.

use std::io;
use std::path::Path;

use super::{Event, Recorder};

/// A browser opened.
pub const OPENED: &str = "browser.opened";
/// A browser was closed: by its chat, the person, going unused, or the kill switch.
pub const CLOSED: &str = "browser.closed";
/// A browser's engine ended by itself.
pub const CRASHED: &str = "browser.crashed";
/// The person pinned a chat's browser: it outlives its chat.
pub const PINNED: &str = "browser.pinned";
/// The person unpinned a chat's browser.
pub const UNPINNED: &str = "browser.unpinned";
/// A chat's browser tool called on a browser.
pub const CALL: &str = "browser.call";
/// A browser's driver changed: who drives it now.
pub const DRIVER_CHANGED: &str = "browser.driver_changed";
/// The person or purlis typed into a browser, never what.
pub const TYPED: &str = "browser.typed";
/// The person handed a browser no chat used to a chat.
pub const HANDED_OVER: &str = "browser.handed_over";

/// The longest a browser's id may be: a program's id is a ULID, or a plain name the window gave.
const LONGEST_ID: usize = 64;

/// Who drives a browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    /// The chat's agent, through its browser tools.
    Agent,
    /// The person, who took it over.
    Person,
    /// purlis, while it performs a declared login (#2016).
    Purlis,
}

/// Who typed into a browser: the person while they drive it, or purlis filling a login. The
/// agent's own acts are its tool calls, not typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Typist {
    Person,
    Purlis,
}

/// Who ended a browser: [`Said::Closed`] says one of the first four, [`Said::Crashed`] the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Closer {
    /// Its chat: a browser tool's close, or the chat ending with it.
    Chat,
    /// The person, from the window.
    Person,
    /// purlis, for going unused (#2023).
    Idle,
    /// The kill switch, which stops every browser.
    KillSwitch,
    /// Nobody: its engine ended by itself. Said by [`Said::Crashed`] alone.
    Crash,
}

/// **Which call a chat's browser tool made**, by its word, never its arguments: every act the
/// session protocol carries to a browser, and the chat's own asks of its browsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Called {
    Open,
    Navigate,
    Snapshot,
    Find,
    Changes,
    Click,
    DoubleClick,
    Hover,
    Type,
    Press,
    Select,
    Drag,
    Scroll,
    Back,
    WaitForText,
    Pages,
    SwitchPage,
    Screenshot,
    /// The browser's downloads (#2020).
    Downloads,
    /// Files of the chat's folder put into a page's file input (#2020).
    Upload,
    /// A download saved into the chat's folder (#2020).
    SaveDownload,
    /// A new browser for the chat.
    New,
    /// The chat closes a browser.
    Close,
    /// The chat asks the person for the browser (#2015).
    NeedsYou,
    /// The chat asks purlis to log the browser in (#2016).
    LogIn,
}

/// What came of a browser tool's call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The browser answered it.
    Done,
    /// It was refused, by purlis or the browser, and the chat told why.
    Refused,
}

/// **One thing the window says about a browser**, by its program's id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "said", rename_all = "snake_case", deny_unknown_fields)]
pub enum Said {
    /// `browser` opened.
    Opened { browser: String },
    /// `by` closed `browser`.
    Closed { browser: String, by: Closer },
    /// `browser`'s engine ended by itself.
    Crashed { browser: String },
    /// The person pinned `browser` past its chat.
    Pinned { browser: String },
    /// The person unpinned `browser`.
    Unpinned { browser: String },
    /// A chat's browser tool called `call` on `browser`, on a page of `origin` (none where the
    /// page has no web origin, or none was known), and it came to `outcome`.
    Call {
        browser: String,
        call: Called,
        #[serde(default)]
        origin: Option<String>,
        outcome: Outcome,
    },
    /// `driver` drives `browser` now.
    DriverChanged { browser: String, driver: Driver },
    /// `by` typed into `browser`, on a page of `origin` (none where it has no web origin, or
    /// none was known; and from a window before #2842).
    Typed {
        browser: String,
        by: Typist,
        #[serde(default)]
        origin: Option<String>,
    },
    /// The person handed `browser` to the chat this is written under.
    HandedOver { browser: String },
}

impl Said {
    /// The kind it is written as.
    pub fn kind(&self) -> &'static str {
        match self {
            Said::Opened { .. } => OPENED,
            Said::Closed { .. } => CLOSED,
            Said::Crashed { .. } => CRASHED,
            Said::Pinned { .. } => PINNED,
            Said::Unpinned { .. } => UNPINNED,
            Said::Call { .. } => CALL,
            Said::DriverChanged { .. } => DRIVER_CHANGED,
            Said::Typed { .. } => TYPED,
            Said::HandedOver { .. } => HANDED_OVER,
        }
    }

    /// The browser it is about.
    pub fn browser(&self) -> &str {
        match self {
            Said::Opened { browser }
            | Said::Closed { browser, .. }
            | Said::Crashed { browser }
            | Said::Pinned { browser }
            | Said::Unpinned { browser }
            | Said::Call { browser, .. }
            | Said::DriverChanged { browser, .. }
            | Said::Typed { browser, .. }
            | Said::HandedOver { browser } => browser,
        }
    }

    /// The origin it names, where it names one.
    fn origin(&self) -> Option<&str> {
        match self {
            Said::Call { origin, .. } | Said::Typed { origin, .. } => origin.as_deref(),
            _ => None,
        }
    }

    /// The body it is written with: the browser, and who, which call, the origin and what came
    /// of it, and nothing else.
    fn body(&self) -> serde_json::Value {
        match self {
            Said::Opened { browser }
            | Said::Pinned { browser }
            | Said::Unpinned { browser }
            | Said::HandedOver { browser } => serde_json::json!({ "browser": browser }),
            Said::Closed { browser, by } => serde_json::json!({ "browser": browser, "by": by }),
            Said::Crashed { browser } => {
                serde_json::json!({ "browser": browser, "by": Closer::Crash })
            }
            Said::Call {
                browser,
                call,
                origin,
                outcome,
            } => serde_json::json!({
                "browser": browser,
                "call": call,
                "origin": origin,
                "outcome": outcome,
            }),
            Said::DriverChanged { browser, driver } => {
                serde_json::json!({ "browser": browser, "driver": driver })
            }
            Said::Typed {
                browser,
                by,
                origin,
            } => serde_json::json!({ "browser": browser, "by": by, "origin": origin }),
        }
    }
}

/// Whether `browser` is a program's id: letters, digits, `-` and `_`, at most [`LONGEST_ID`].
fn a_programs_id(browser: &str) -> bool {
    !browser.is_empty()
        && browser.len() <= LONGEST_ID
        && browser
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl Recorder {
    /// **Writes what the window said of a browser** ([`Said`]), under chat `number`'s current
    /// run where the browser is a chat's, made durable before it answers. Refused, and nothing
    /// written, for a browser named by anything but a program's id, or an origin that is
    /// anything more than an origin.
    pub fn browser(&mut self, plane: &Path, number: Option<u32>, said: &Said) -> io::Result<Event> {
        if !a_programs_id(said.browser()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a browser is named in the event log by its program's id alone, so nothing was \
                 written",
            ));
        }
        if let Some(origin) = said.origin()
            && purlis_session_protocol::browsing::origin_of(origin).as_deref() != Some(origin)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a page is named in the event log by its origin alone, so nothing was written",
            ));
        }
        if matches!(
            said,
            Said::Closed {
                by: Closer::Crash,
                ..
            }
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a crash is said as one, never as a close, so nothing was written",
            ));
        }
        self.audited(plane, number, said.kind(), said.body())
    }
}

#[cfg(test)]
mod tests;

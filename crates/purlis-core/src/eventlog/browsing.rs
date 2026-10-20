// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Who drives a browser, and that someone typed, never what** (#2293, spec #2008's Events).
//!
//! The window tells the event log's one writer three things about a browser, each a [`Said`]:
//! who drives it now (`browser.driver_changed`: the agent, the person who took it over, or
//! purlis while it performs a login), that the person or purlis typed into it
//! (`browser.typed`), and that the person handed it to a chat (`browser.handed_over`). Each is
//! written under the chat whose browser it is, and with no chat for a browser no chat uses.
//!
//! **A closed set, composed here.** The window sends a variant and a browser's id, and the body
//! is written from them alone: there is no field for a key, a value, an address, a title or any
//! of a page's text, a field more is not read, and a browser named by anything but a program's
//! id is refused. So nothing the person or purlis typed, nothing a page holds and no address
//! reaches the log by this way.

use std::io;
use std::path::Path;

use super::{Event, Recorder};

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

/// **One thing the window says about a browser**, by its program's id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "said", rename_all = "snake_case", deny_unknown_fields)]
pub enum Said {
    /// `driver` drives `browser` now.
    DriverChanged { browser: String, driver: Driver },
    /// `by` typed into `browser`.
    Typed { browser: String, by: Typist },
    /// The person handed `browser` to the chat this is written under.
    HandedOver { browser: String },
}

impl Said {
    /// The kind it is written as.
    pub fn kind(&self) -> &'static str {
        match self {
            Said::DriverChanged { .. } => DRIVER_CHANGED,
            Said::Typed { .. } => TYPED,
            Said::HandedOver { .. } => HANDED_OVER,
        }
    }

    /// The browser it is about.
    pub fn browser(&self) -> &str {
        match self {
            Said::DriverChanged { browser, .. }
            | Said::Typed { browser, .. }
            | Said::HandedOver { browser } => browser,
        }
    }

    /// The body it is written with: the browser, and who, and nothing else.
    fn body(&self) -> serde_json::Value {
        match self {
            Said::DriverChanged { browser, driver } => {
                serde_json::json!({ "browser": browser, "driver": driver })
            }
            Said::Typed { browser, by } => serde_json::json!({ "browser": browser, "by": by }),
            Said::HandedOver { browser } => serde_json::json!({ "browser": browser }),
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
    /// written, for a browser named by anything but a program's id.
    pub fn browser(&mut self, plane: &Path, number: Option<u32>, said: &Said) -> io::Result<Event> {
        if !a_programs_id(said.browser()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a browser is named in the event log by its program's id alone, so nothing was \
                 written",
            ));
        }
        self.audited(plane, number, said.kind(), said.body())
    }
}

#[cfg(test)]
mod tests;

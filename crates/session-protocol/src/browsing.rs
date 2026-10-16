// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a browser is asked to do, and what it answers** (1.11, #2013): the words a chat's
//! browser tools, the window and the session host share.
//!
//! A chat's agent drives its browser through typed tools (spec #2008): a snapshot of a page
//! with a reference for each element, actions on those references, and a screenshot. The tool
//! server hands each call to the window over the chat's hook socket, the window decides whether
//! the chat may have a browser at all, and hands the call on to the session host that runs the
//! browser ([`crate::session::Command::Browse`]), where the browser's driver carries it out.
//! These are the one vocabulary for all three hops, so nothing is translated on the way but at
//! the driver itself.
//!
//! **No act runs script.** Every act is one of the driver's own; text an agent writes reaches a
//! page only as typed text or as a value to match, never as code.

use serde::{Deserialize, Serialize};

use crate::session::Listed;

/// One thing a browser is asked to do. An act that names no page acts on the browser's current
/// page; a page is named as [`Act::Pages`] lists it (`p2`), an element as a snapshot or a find
/// dealt it (`e12`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Act {
    /// Opens a new page at `address` (http or https), which becomes the current page.
    Open {
        address: String,
    },
    /// Sends a page to `address`.
    Navigate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        address: String,
    },
    /// The page as a tree of elements with references, from line `from` (1 for the start), at
    /// most `lines` lines, inside `within` where given.
    Snapshot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        within: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lines: Option<u32>,
    },
    /// The elements whose name or value holds `text`, or whose role is `role`, or both.
    Find {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        within: Option<String>,
    },
    Click {
        target: String,
    },
    DoubleClick {
        target: String,
    },
    Hover {
        target: String,
    },
    /// Types into each field in turn.
    Type {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        fields: Vec<Typed>,
    },
    /// Presses each key or chord in turn (`Enter`, `Shift+Tab`, `Control+a`).
    Press {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        keys: Vec<String>,
    },
    /// Chooses `options`, by value or by label, in a drop-down.
    Select {
        target: String,
        options: Vec<String>,
    },
    Drag {
        from: String,
        to: DragTo,
    },
    /// Scrolls by `x` and `y` CSS pixels over `at` (the page's middle where none); with no
    /// distance, brings `at` into view.
    Scroll {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
        #[serde(default, skip_serializing_if = "is_zero")]
        x: i32,
        #[serde(default, skip_serializing_if = "is_zero")]
        y: i32,
    },
    /// Goes back to the page's previous address.
    Back {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
    },
    /// Waits up to `within_ms` for `text` to show on the page, or with `gone`, to show no
    /// longer.
    WaitForText {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        text: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        gone: bool,
        within_ms: u64,
    },
    /// The browser's pages.
    Pages,
    /// Makes `page` the current page.
    SwitchPage {
        page: String,
    },
    /// A picture of what the page shows now, as PNG, every blanked element painted over.
    Screenshot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
    },
}

fn is_zero(n: &i32) -> bool {
    *n == 0
}

impl Act {
    /// Whether it only reads, so a retry acts on nothing twice.
    pub fn only_reads(&self) -> bool {
        matches!(
            self,
            Act::Snapshot { .. }
                | Act::Find { .. }
                | Act::WaitForText { .. }
                | Act::Pages
                | Act::Screenshot { .. }
        )
    }
}

/// One field a [`Act::Type`] types into: an element, or whatever has the focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Typed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub into: Option<String>,
    pub text: String,
    /// Whether what the field holds is replaced rather than added to.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replace: bool,
}

/// Where a drag ends: on an element, or so many CSS pixels right and down of where it began.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DragTo {
    Element(String),
    Offset { x: i32, y: i32 },
}

/// One page, as an answer names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSaid {
    pub page: String,
    pub address: String,
    pub title: String,
    /// Whether it is the browser's current page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub current: bool,
}

/// What a browser answered an act with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Browsed {
    /// The page it acted on, as it is now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<PageSaid>,
    /// Whether the page had settled when it answered; where not, what it still waited for.
    pub settled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiting_for: Vec<String>,
    /// Pages the act opened; the last is now the current page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opened: Vec<PageSaid>,
    /// A snapshot's or a find's lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The browser's pages, for [`Act::Pages`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageSaid>,
    /// Whether a waited-for text showed (or, waiting for it to go, went).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<bool>,
    /// A screenshot, PNG, in base64 on the wire.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_base64"
    )]
    pub png: Option<Vec<u8>>,
    /// More the agent should know, in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Whether this answers a retry: the act did not act again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeated: bool,
}

/// What a browser may reach, given as it starts (1.11): the hosts its chat's sandbox lists, each
/// with the layer that lists it, the hosts policy pins as never allowed, the ports of this
/// machine the person withdrew from the chat while it runs, and whether a public site nothing
/// lists is carried too (the open web). Absent from a start, a browser reaches nothing listed
/// and every site new to the project is refused.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserReach {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listed: Vec<Listed>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub never: Vec<String>,
    /// Ports of this machine carried no longer, whatever lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withdrawn: Vec<u16>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open_web: bool,
}

mod optional_base64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, to: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => to.serialize_str(&STANDARD.encode(bytes)),
            None => to.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Option<Vec<u8>>, D::Error> {
        let text = Option::<String>::deserialize(from)?;
        text.map(|text| STANDARD.decode(text).map_err(serde::de::Error::custom))
            .transpose()
    }
}

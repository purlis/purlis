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
//!
//! **One driver at a time** (1.14, #2014). The person takes a browser over from its view
//! ([`crate::session::Command::TakeOver`]); while they drive, every act is refused and only their
//! own input reaches the page ([`Input`]), each event naming what the frame it was aimed at showed
//! ([`Aim`]). Only their hand-back ([`crate::session::Command::HandBack`]) returns the browser to
//! the agent.

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
    /// The lines of the page's tree new or gone since the page was last read (1.21, #2137):
    /// each new one with its reference and the line it is read at, in [`Browsed::text`]. A
    /// page not read before answers its first lines. Kept by its call id like an act.
    Changes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
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
    /// Every download of the browser's (1.46, #2020), in [`Browsed::downloads`].
    Downloads,
    /// Puts `files` into the file input `target` (1.46, #2020): each a path relative to the
    /// chat's folder, as the agent wrote it. The window names that folder (`from`), its own
    /// record of where the chat runs, never the chat's word; the host reads each file there
    /// following no link and copies it into the browser's own folder, and the engine reads only
    /// that copy. A host refuses one that names no folder, or one outside the browser's project.
    Upload {
        target: String,
        files: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// Brings download `download` (`d1`) into the chat's folder as `to`, a path relative to it
    /// (1.46, #2020), once the person allowed downloads from its origin into that chat. The
    /// window names the folder (`into`), as for [`Act::Upload`]; the host writes a new file there
    /// following no link, and never over one that is there.
    SaveDownload {
        download: String,
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        into: Option<String>,
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
                | Act::Downloads
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
    /// The sites, each `host:port`, the browser did not go to or send to, or where it typed
    /// nothing, because each is new to the project and the person is to be asked (1.15,
    /// #2018): each once, since the act before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asked: Vec<String>,
    /// The downloads that began or moved on since the act before (1.46, #2020), or for
    /// [`Act::Downloads`] every one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub downloads: Vec<DownloadSaid>,
}

/// **One download of a browser's, as an answer says it** (1.46, #2020): its name in the browser
/// (`d1`), the name its page suggested made plain, the origin it came from, its size, how far it
/// has come, and the engine's own name for its file in the browser's holding folder, which no
/// chat reads. Never a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadSaid {
    pub download: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    pub state: DownloadState,
    pub file: String,
}

/// How far a download has come (1.46, #2020).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Going,
    Done,
    /// The page or the engine cancelled it.
    Cancelled,
    /// purlis cancelled it and removed its file: larger than a download may be.
    TooLarge,
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

/// **How a browser's views over a link between hosts are held** (1.49, #3297): at most `frames`
/// a second, and no frame wider than `width` pixels, the engine scaling a wider page down. A
/// pool's own settings, given with the browser's start on its runner; a start that gives none is
/// held to purlis's defaults ([`LinkFrames::DEFAULT`]). Bounded both ways ([`LinkFrames::new`]):
/// a host refuses a start whose `link_frames` are out of bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkFrames {
    pub frames: u32,
    pub width: u16,
}

impl LinkFrames {
    /// Spec #2041's defaults: 15 frames a second, 1280 pixels wide.
    pub const DEFAULT: LinkFrames = LinkFrames {
        frames: 15,
        width: 1280,
    };
    /// The fewest and the most frames a second a pool may set.
    pub const FRAMES: std::ops::RangeInclusive<u32> = 1..=60;
    /// The narrowest and the widest frame a pool may set, in pixels.
    pub const WIDTH: std::ops::RangeInclusive<u16> = 320..=3840;

    /// `frames` a second and `width` pixels, each within its bounds, or why not.
    pub fn new(frames: u32, width: u16) -> Result<LinkFrames, String> {
        LinkFrames { frames, width }.checked()
    }

    /// These, where each is within its bounds; or why not, in words a person reads.
    pub fn checked(self) -> Result<LinkFrames, String> {
        if !Self::FRAMES.contains(&self.frames) {
            return Err(format!(
                "a pool's browser frames are {} to {} a second",
                Self::FRAMES.start(),
                Self::FRAMES.end()
            ));
        }
        if !Self::WIDTH.contains(&self.width) {
            return Err(format!(
                "a pool's widest browser frame is {} to {} pixels",
                Self::WIDTH.start(),
                Self::WIDTH.end()
            ));
        }
        Ok(self)
    }
}

impl Default for LinkFrames {
    fn default() -> LinkFrames {
        LinkFrames::DEFAULT
    }
}

/// **A value from a vault, on its way to the browser that fills it** (1.18, #2016): written on
/// the wire as the text it is, never shown by `Debug`, and wiped from memory when dropped.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The value itself, for the one place that fills it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(withheld)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

/// **A declared login, as purlis performs it in a browser** (1.18, #2016): the login page's
/// address, whose origin is the one place it is filled, and the values from the vault. The
/// user name is absent for a page that asks for a password alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogIn {
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<Secret>,
    pub password: Secret,
    /// The kept login this browser keeps once it ends, where it is to keep one (1.27, #2021).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<KeptLogin>,
    /// The login's identity (a digest of the persona, the project and the login's declaration
    /// as allowed, #2699), which the host records of the browser it performs it in, so a window
    /// that comes back after a crash knows every login it holds (1.39, #2025). Never a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// **A chat chose this page and these keys** (#4595): a user name goes only into a field
    /// that says it's one, or beside a password field. A host that doesn't read this fills as it
    /// always has, on the same origin.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suggested: bool,
}

/// **A kept login** (1.27, #2021, the person's rulings of 2026-10-18): a profile, the persona
/// whose declared browser login it is, the login's name and its identity (a digest of the
/// persona, the project and the login's declaration as allowed), whose browser data this device
/// keeps sealed between browsers; and the login's page, whose site a browser opened from it is
/// held to. Names and a digest, never a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptLogin {
    pub profile: String,
    pub persona: String,
    pub login: String,
    pub identity: String,
    pub address: String,
}

/// **The origin of `address`** (1.35, #2300): `scheme://host[:port]`, lower-cased, for an http
/// or https address whose host is written in ASCII, as the engine records a page's; nothing for
/// any other, or one longer than [`MOST_ORIGIN_BYTES`]. Never its user, path, query or
/// fragment: what a browser view's chip may name. An origin it answers is its own origin, so a
/// reader checks what it was sent by it.
pub fn origin_of(address: &str) -> Option<String> {
    let (scheme, rest) = address.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#', '\\']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let plain = host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._:[]".contains(&b));
    if host.is_empty() || !plain || host.starts_with(':') || !one_host_and_a_port(host) {
        return None;
    }
    let origin = format!("{scheme}://{}", host.to_ascii_lowercase());
    (origin.len() <= MOST_ORIGIN_BYTES).then_some(origin)
}

/// Whether `host` is one name, or one bracketed address, and at most a port after it: a port
/// of digits, at most 65535 (#2842). Nothing else may follow a host in an origin.
fn one_host_and_a_port(host: &str) -> bool {
    let (name, port) = match host.strip_prefix('[') {
        Some(bracketed) => {
            let Some((address, after)) = bracketed.split_once(']') else {
                return false;
            };
            if address.is_empty() || address.contains(['[', ']']) {
                return false;
            }
            match after {
                "" => return true,
                port => (address, port.strip_prefix(':').unwrap_or("x")),
            }
        }
        None => match host.split_once(':') {
            Some((name, port)) => (name, port),
            None => (host, "0"),
        },
    };
    !name.contains(['[', ']'])
        && (host.starts_with('[') || !name.contains(':'))
        && !port.is_empty()
        && port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u32>().is_ok_and(|port| port <= 65_535)
}

/// The longest origin [`origin_of`] answers (1.35): a whole DNS name and a port fit in it, so
/// one longer is no web page's.
pub const MOST_ORIGIN_BYTES: usize = 255;

/// Who drives a browser (1.14): the agent, through its acts, or the person who took it over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    Agent,
    Person,
}

/// What a frame of a browser showed (1.14): its page (`p1`), and its documents, a number that
/// changes each time the page or any frame inside it loads another document. Input names the
/// frame it was aimed at by it, and the browser drops input whose page has moved on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Aim {
    pub page: String,
    pub document: u64,
}

/// What the person's mouse did (1.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
    Moved,
    Pressed,
    Released,
    Wheel,
}

/// The mouse button a mouse event is about (1.14).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    #[default]
    None,
    Left,
    Middle,
    Right,
}

/// What the person's key did (1.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    Down,
    Up,
}

/// One event of the person's, in a browser they drive (1.14): a mouse's or a key's, aimed at
/// what a frame showed. Points are CSS pixels of the page's viewport; `modifiers` are the keys
/// held, as bits (Alt 1, Control 2, Meta 4, Shift 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    Mouse {
        aim: Aim,
        action: MouseAction,
        x: i32,
        y: i32,
        #[serde(default, skip_serializing_if = "is_no_button")]
        button: Button,
        /// 1 for a click, 2 for the second of a double click.
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        clicks: u8,
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        modifiers: u8,
        /// How far the wheel turned, in CSS pixels.
        #[serde(default, skip_serializing_if = "is_zero")]
        delta_x: i32,
        #[serde(default, skip_serializing_if = "is_zero")]
        delta_y: i32,
    },
    Key {
        aim: Aim,
        action: KeyAction,
        /// The key as the window names it (`a`, `Enter`, `ArrowLeft`).
        key: String,
        /// The physical key, as the platform names it (`KeyA`).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        physical: String,
        /// The platform's key code for it.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        key_code: u32,
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        modifiers: u8,
    },
}

fn is_no_button(button: &Button) -> bool {
    *button == Button::None
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
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

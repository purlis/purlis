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
//! - **One way out** (#5310). Every answer and refusal to the agent's calls leaves through
//!   `browser/told.rs`, which takes each text kept out of every string in it, in every form it is
//!   commonly written in (`scrub.rs`).
//! - **Inspect is the person's** (#2028). [`Browser::inspect`] opens the engine's own DevTools
//!   for the page the browser shows, only while the person drives: a front end the driver never
//!   lists as a page, watched as frames and spoken to through a tapped session over the one
//!   pipe, reaching that page alone. Handing the browser back closes it.

mod blank;
mod browser;
mod clipped;
mod drawn;
pub mod fit;
mod kept;
mod keys;
mod ledger;
mod screencast;
mod scrub;
mod settle;
mod site;
mod tree;

use std::fmt;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

pub use browser::{
    Browser, FRONT_END, INSPECTOR_PAGE, InspectEnded, LOGIN_WAIT, LoginEnded, MOST_QUEUED,
    origin_of,
};
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
    /// Whether the browser may be signed in already, opened from kept logins (#2021) or this
    /// login having gone through in it, held to its page (#5121): then nothing is filled where the page asks for no
    /// password.
    pub only_if_asked: bool,
    /// A chat chose this page and these keys (#4595): the user name goes only into a field that
    /// says it's one, or beside a password field.
    pub suggested: bool,
    /// **The browser has held nothing until this login** (#5272): nobody has acted in it, and it
    /// opened from no kept login and tried no login before. Its first load is as clean as a
    /// load can be, so nothing is cleared. Anything else starts clean on its own site.
    pub clean_already: bool,
    /// **The page's own origin, where the form is on another** (#4995): a page that has gone
    /// back there since the login was submitted is a login that went through, and its holder is
    /// told ([`Hold::back`]). Nothing is ever filled on it.
    pub back_to: Option<String>,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("address", &self.address.as_str())
            .field("origin", &self.origin)
            .field("back_to", &self.back_to)
            .finish_non_exhaustive()
    }
}

/// **What holds a browser to the login purlis performs in it** (#2016, #4995): the caller's.
/// [`Hold::begins`] is told as the login begins, and [`Hold::bind`] holds what the browser
/// reaches to the login's sites just before the first value goes in; where it cannot, no value
/// goes in. A login whose form is on another origin than its page ([`Login::back_to`]) is told
/// [`Hold::back`] once its own page leaves the form's origin, whenever that is: during the login
/// or after it.
pub trait Hold: Send + Sync {
    /// **The login begins**, purlis driving: told once, before its page is opened, so a browser
    /// held to a login already can reach the login's form (#4995). Why not, where it cannot; the
    /// login then fills nothing.
    fn begins(&self) -> Result<(), String> {
        Ok(())
    }

    /// Holds the browser to the login's sites, or why it cannot.
    fn bind(&self) -> Result<(), String>;

    /// **The login's own page has left its form's origin** since the browser was held: the page
    /// purlis opened for the login, never another, shows a document back on its page's origin,
    /// or anywhere else. Told at most once per login, on the thread that reads the engine's
    /// events, so it must not wait on the browser.
    fn back(&self);
}

/// **What holds a browser to the site a secret was filled on, while the page holding it lives**
/// (#5117; the person's ruling SF-3 and decision SF-9 on #5096): the caller's.
/// [`FillHold::hold`] is told once every check of the fill has passed and just before the value
/// goes in; where it cannot hold, nothing goes in. [`FillHold::released`] is told once, when the
/// page the field is on has loaded another document in its main frame or closed, and every window
/// related to it has closed. A route change that loads no document keeps the hold, and so does any
/// change in a frame inside the page.
pub trait FillHold: Send + Sync {
    /// Holds the browser to the fill's site from now, or why it cannot.
    fn hold(&self) -> Result<(), String>;

    /// **The page holding the value is gone**: a new document in its main frame, or its tab
    /// closed, and its related windows with it. Told at most once, after [`Self::hold`] answered, and on the thread that reads the
    /// engine's events, so it must not wait on the browser.
    fn released(&self);

    /// **[`Self::released`], telling `then` once the hold is over** (#5230): the page is leaving
    /// for another site, and goes there only once the browser may. Told in place of it.
    fn released_then(&self, then: Box<dyn FnOnce() + Send>) {
        self.released();
        then();
    }

    /// **Whether a redirect from the fill's site's own server, after a form sent there, may take
    /// the page to `address`** (#5230, SF-14), `chain_began` saying the chain leading to this
    /// sign-in began on that site. Refused unless its caller says otherwise. Told on a thread that
    /// may wait on no browser.
    fn may_leave_for(&self, address: &str, chain_began: bool) -> Leaving {
        let _ = (address, chain_began);
        Leaving::No {
            why: "this browser's page holds a secret purlis filled, and it leaves for no other \
                  site"
                .to_owned(),
        }
    }

    /// **Whether the key filled is approved for `address`'s site too** (#5230, SF-15), so a form
    /// may be sent there. Not unless its caller says so.
    fn approved_for(&self, address: &str) -> bool {
        let _ = address;
        false
    }
}

/// **Whether a filled page may leave for another site** (#5230, SF-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leaving {
    /// It goes on by itself.
    Go,
    /// It waits on the person about `site` (`host:port`), and the act's answer says so.
    Ask { site: String },
    /// It stays, and `why` is said.
    No { why: String },
}

/// A login that only binds, told nothing of its page coming back: what a caller with one site
/// passes.
impl<F: Fn() -> Result<(), String> + Send + Sync> Hold for F {
    fn bind(&self) -> Result<(), String> {
        self()
    }

    fn back(&self) {}
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
    /// **Where `address` sends the browser to sign in** (#4995): a new page opened at `address`
    /// and followed until its own document shows a login form. Its answer's page is that
    /// document, as the engine records it. Nothing is typed.
    FindLoginForm {
        address: WebAddress,
    },
    /// **What a field is, before a secret goes into it** (#5115): the origin of the document of
    /// the frame that holds `target`, as the engine records it, its label and its kind. Nothing
    /// is typed.
    DescribeField {
        target: Ref,
    },
    /// **Whether the page the browser shows asks to sign in now** (#5121): nothing is typed,
    /// and nothing moves.
    AsksToSignIn,
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
    /// Every download of the browser's, as [`Answer::downloads`] says them (#2020).
    Downloads,
    /// Puts `files` into the file input `target` (#2020), as the page's own file chooser would.
    /// Each is a file purlis made for the browser alone, never one an agent named: the caller
    /// copies what it may read into the browser's own folder first.
    Upload {
        target: Ref,
        files: Vec<std::path::PathBuf>,
    },
}

impl Call {
    /// Whether the call only reads (and so acts on nothing a retry could act on twice).
    pub fn only_reads(&self) -> bool {
        matches!(
            self,
            Call::Snapshot { .. }
                | Call::DescribeField { .. }
                | Call::AsksToSignIn
                | Call::Find { .. }
                | Call::WaitForText { .. }
                | Call::Pages
                | Call::Screenshot { .. }
                | Call::Downloads
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
    /// The field a [`Call::DescribeField`] read (#5115).
    Field(FieldInfo),
    /// What a [`Call::AsksToSignIn`] read (#5121).
    AsksToSignIn(bool),
}

/// **A field a secret may go into, as the engine reads it** (#5115): never what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    /// The origin of the document of the frame that holds it ([`origin_of`]), as the engine
    /// recorded that document, never what the page says; none for one of no web origin.
    pub origin: Option<String>,
    /// What the page labels it, made plain and at most [`MOST_LABEL_CHARS`] characters.
    pub label: String,
    pub kind: FieldKind,
}

/// The longest label a [`FieldInfo`] gives, in characters.
pub const MOST_LABEL_CHARS: usize = 80;

/// **What kind of field it is** (#5115): a one-line field takes a secret as approved, a
/// multi-line one or an editable area only where the person was asked about that fill (#5120).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// An input of type password, text, email, number, tel or url that takes typing.
    OneLine,
    /// A text area.
    MultiLine,
    /// An editable area of the page.
    Editable,
    /// Anything else, or a field that takes no typing (disabled, or read-only).
    Other,
}

/// **A vault value purlis puts into one field** (#5115): the field, the one origin the person
/// approved, the key's name and the value. Its `Debug` never shows the value.
pub struct SecretFill {
    pub target: Ref,
    pub origin: String,
    pub key: String,
    pub value: zeroize::Zeroizing<String>,
    /// **The person answered a notice for this fill that named a multi-line text box or an
    /// editable area** (#5120, SF-5): only then does one take it. False, a one-line field alone.
    pub multi_line: bool,
}

impl fmt::Debug for SecretFill {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretFill")
            .field("target", &self.target)
            .field("origin", &self.origin)
            .field("key", &self.key)
            .field("multi_line", &self.multi_line)
            .finish_non_exhaustive()
    }
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
    /// The downloads that began or moved on since the last answer (#2020), or for
    /// [`Call::Downloads`], every one.
    pub downloads: Vec<DownloadInfo>,
}

/// **One download of a browser's** (#2020): held in the folder the browser was given
/// ([`Browser::hold_downloads_in`]), under the name the engine made for it, never the page's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadInfo {
    /// Its name in the browser (`d1`), in the order they began.
    pub id: String,
    /// The name the page suggested, made plain ([`plain_file_name`]): words to show, never a
    /// path purlis writes.
    pub name: String,
    /// The origin it came from (`https://host[:port]`): its own address's, or for one at no web
    /// address (a `blob:` or `data:` one), the page's that began it. None where neither is one.
    pub origin: Option<String>,
    /// The bytes received so far.
    pub received: u64,
    /// The bytes it has in all, where the site said.
    pub total: Option<u64>,
    pub state: DownloadState,
    /// The engine's name for its file in the holding folder: what the host opens it by.
    pub file: String,
}

/// How far a download has come (#2020).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Going,
    Done,
    /// The engine or the page cancelled it.
    Cancelled,
    /// purlis cancelled it and removed its file: past [`Limits::most_download_bytes`], or past
    /// [`Limits::most_held_bytes`] with the browser's others.
    TooLarge,
}

/// The longest name [`plain_file_name`] keeps, in characters.
pub const MOST_FILE_NAME_CHARS: usize = 120;

/// **A name a page suggested for a file, made plain** (#2020): no path, no character a reader
/// does not see ([`purlis_unicode::invisible`]: controls, format characters such as a direction
/// mark or a zero-width space, line and paragraph separators), no leading dot, at most [`MOST_FILE_NAME_CHARS`] characters;
/// `download` where nothing is left.
pub fn plain_file_name(suggested: &str) -> String {
    let plain = plain_file_name_uncut(suggested);
    let cut: String = plain.chars().take(MOST_FILE_NAME_CHARS).collect();
    let cut = cut.trim();
    if cut.is_empty() {
        "download".to_owned()
    } else {
        cut.to_owned()
    }
}

/// [`plain_file_name`] of any length, and empty where nothing is left.
fn plain_file_name_uncut(suggested: &str) -> String {
    let kept: String = suggested
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') {
                '_'
            } else {
                c
            }
        })
        .filter(|c| !purlis_unicode::invisible(*c))
        .collect();
    kept.trim().trim_start_matches('.').trim().to_owned()
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

/// **The site the agent is kept off at an address, now** (#5152): told a document's address (or
/// its origin's), the site (`host:port`) whose documents the agent may neither read nor act on
/// there, if any. The caller's, asked at each of the agent's calls and of each request a page
/// sends while the agent drives ([`Browser::keep_off`]): the browser loads that site's documents,
/// the agent's calls leave them alone.
pub type KeptOff = std::sync::Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// **Why the caller refused a load, now** (#5150): told the address a page failed to load, the
/// words the agent is told, where the caller's own reach refused it
/// ([`Browser::say_why_refused`]). What a page sent may be in that address, so the words are never
/// to repeat it.
pub type RefusedLoad = std::sync::Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A call refused, or one whose effect is not known, in words that say what to do.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the browser has no page open: open one")]
    NoPage,
    /// The engine made the page and its event about it was not read in time (#4472): the page
    /// stays blank and is not the current one.
    #[error(
        "the browser made a blank page but has not said so yet, so it did not go to the address: \
         list the pages, then go there on that page"
    )]
    PageUntold,
    /// A frame this would reach has not said what address it shows, so the judge cannot be told
    /// where it would go (#4472).
    #[error(
        "nothing was sent to the page: a frame on it has not said what address it shows yet, so \
         where this would go is not known: try again in a moment"
    )]
    FrameUnread,
    /// A frame this would reach shows a document with no web origin of its own, so the judge
    /// cannot be told where it would go.
    #[error(
        "nothing was sent to the page: a frame on it shows a document with no web origin of its \
         own, so where this would go cannot be told"
    )]
    FrameOpaque,
    /// **The page, or the frame acted on, is on the site the browser signs in on** (#5152, the
    /// person's ruling of 2026-11-04), outside a sign-in ([`KeptOff`]): nothing was read or sent.
    /// Where the page shows its sign-in form again, it says so.
    #[error(
        "nothing was read or sent: this is a page of {site}, the site this browser signs in on, \
         and purlis lets you read or act on its pages only while it logs this browser in{}",
        kept_off_then(.asks_to_sign_in)
    )]
    KeptOff { site: String, asks_to_sign_in: bool },
    /// The tab is a fresh blank one, with no page in it to type into (#4889).
    #[error(
        "nothing was sent: this is a blank tab, with no page to type into: open a page in it \
         first"
    )]
    BlankTab,
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
    /// The page's tree, read for an agent's call, was not read within the call's limit
    /// (#3277): a read acts on nothing, so the call did nothing, and the read goes on.
    #[error(
        "the page is too large to read within this call's limit, so this call read nothing and \
         did nothing; purlis goes on reading it: make the call again in a moment"
    )]
    PageReadTooLong,
    /// The page's tree, read for an agent's call, did not finish within the read's own limit
    /// (#3277): nothing goes on reading it.
    #[error(
        "the page is too large to read: purlis's read of it did not finish within its own limit, \
         so this call read nothing and did nothing; try again later, or go on without reading it"
    )]
    PageReadPastItsLimit,
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
    #[error("{0} is not a file input: find the page's file input and upload into it")]
    NotAFileInput(Ref),
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
    /// The earlier address carries a value purlis keeps out (#5117): going back would send it
    /// again, wherever the address goes.
    #[error(
        "the earlier address carries a value purlis keeps out of this browser, so the browser \
         does not go back to it: go to the page you need by its address"
    )]
    EarlierHoldsKept,
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
    /// A page DevTools were open on could not be read as the browser was handed back, so what
    /// they wrote into it is not yet known to be kept out (#2028): nothing is answered until it
    /// is.
    #[error(
        "purlis could not read a page the person had DevTools open on, to keep what they did \
         there out of your answers, so this call was not carried out: go back, or go to another \
         address in that page, or ask the person to take the browser over and hand it back"
    )]
    NotKeptOutYet,
    /// DevTools were open on this browser, and what the person copied there may be what a paste
    /// gets (#2028): the agent's pastes are withheld from then on.
    #[error(
        "pasting is not available in this browser since the person used DevTools in it, so this \
         call was not carried out: type the text instead"
    )]
    PasteWithheld,
    /// purlis filled a secret into a page of this browser (#5115): the agent's copies, cuts and
    /// drags are withheld for its life.
    #[error(
        "copying, cutting and dragging are not available in this browser since purlis filled a \
         secret into it, so this call was not carried out: read what you need with a snapshot"
    )]
    CopyWithheld,
    /// purlis filled a secret into a field of this page (#5115): while the page holding it is
    /// open, the agent presses no key, and types and chooses nothing, in it.
    #[error(
        "purlis filled a secret into a field of this page, so while the page is open no key is \
         pressed and nothing is typed or chosen in it, and this call was not carried out: click \
         the page's own button to send the form, and type into other fields before purlis fills \
         the secret"
    )]
    KeysWithheld,
    #[error(
        "purlis is logging this browser in, so it takes no call from you until the login answers: wait for it, then take a new snapshot"
    )]
    PurlisDriving,
    /// A login purlis performed did not go through (#2016), in words that name no value.
    #[error("{0}")]
    NotLoggedIn(String),
    /// A login's site was cleared, and its form didn't appear on the load after (#5272): a
    /// browser that has held nothing may show it where this one, held to the login, can't.
    /// Nothing was filled.
    #[error("{0}")]
    NoFormOnCleanLoad(String),
    /// A secret purlis was to fill was not filled (#5115), in words that name no value.
    #[error("{0}")]
    NotFilled(String),
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

/// What the agent may do next on a page it is kept off (#5152).
fn kept_off_then(asks_to_sign_in: &bool) -> &'static str {
    if *asks_to_sign_in {
        ": it asks to sign in again, so log this browser in again"
    } else {
        ": go back, or go to another address"
    }
}

/// The most a read of a page's whole tree holds by default (#3277): two of the engine's largest
/// answers.
pub const MOST_TREE_READ_BYTES: usize = 2 * crate::cdp::MAX_MESSAGE_BYTES;

/// The most bytes one download holds by default (#2020): 512 MiB.
pub const MOST_DOWNLOAD_BYTES: u64 = 512 << 20;

/// The most downloads one browser makes by default (#2020).
pub const MOST_DOWNLOADS: usize = 64;

/// The most bytes a browser's downloads hold together by default (#2020): 1 GiB.
pub const MOST_HELD_BYTES: u64 = 1 << 30;

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
    /// How long a read of a page's whole tree for the person's keys is used after it came
    /// (#3043): an older one is read anew.
    pub tree_read_kept_for: Duration,
    /// How long an agent's snapshot, find or wait waits for its page's tree to be read before it
    /// says the page took too long (#3277): with its settling, under the 85 s the window waits
    /// for an act.
    pub page_reads_within: Duration,
    /// How long a read of a page's whole tree for an agent's call, which outlived the call it
    /// was made for, is used by the next call after it came (#3277): an older one is read anew.
    pub page_read_kept_for: Duration,
    /// The most a read of a page's whole tree for an agent's call, or for the person's keys,
    /// holds of the engine's answers, every frame's in all (#3277): past it, the read fails as
    /// too large.
    pub most_tree_read_bytes: usize,
    /// The most bytes one download may hold (#2020): past it, purlis cancels it.
    pub most_download_bytes: u64,
    /// The most downloads one browser makes (#2020): any after it is cancelled as it begins,
    /// listed nowhere, and only counted in the next answer's note.
    pub most_downloads: usize,
    /// The most bytes a browser's downloads hold together (#2020): the one that passes it is
    /// cancelled.
    pub most_held_bytes: u64,
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
            tree_read_kept_for: Duration::from_secs(10),
            page_reads_within: Duration::from_secs(60),
            page_read_kept_for: Duration::from_secs(60),
            most_tree_read_bytes: MOST_TREE_READ_BYTES,
            most_download_bytes: MOST_DOWNLOAD_BYTES,
            most_downloads: MOST_DOWNLOADS,
            most_held_bytes: MOST_HELD_BYTES,
        }
    }
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

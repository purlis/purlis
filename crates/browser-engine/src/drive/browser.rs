// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One browser's pages, driven over DevTools: what the engine's events say about its pages and
//! frames, and each call made into the engine's own calls.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{Value, json};

use super::blank::{Rect, paint_over};
use super::drawn::{STYLES, drawn};
use super::kept::Kept;
use super::keys::key_events;
use super::ledger::{Begin, Ledger};
use super::screencast::Screencast;
use super::settle::{Activity, Settle, activity};
use super::site::site;
use super::tree::{FrameAx, NodeKey, Query, Scrub, Snapshot, View, build_kept};
use super::{
    Aim, Answer, Call, CallId, Content, DragTo, Driver, Field, Gate, Judge, Limits, Outbound,
    PageId, PageInfo, Painted, Ref, Refusal, Settled, Why, lock,
};
use crate::WebAddress;
use crate::cdp::{CALL_TIMEOUT, CallError, Closed, Connection, Event, Tap, TapSink};
use crate::ending::{EndedBy, Ending, EngineExit};

mod changes;
mod downloads;
mod drawing;
mod inspect;
mod login;
mod reading;
mod takeover;
mod view;
mod world;

pub use inspect::{FRONT_END, INSPECTOR_PAGE, InspectEnded, MOST_QUEUED};
pub use login::{LOGIN_WAIT, LoginEnded, origin_of};

use drawing::Drawing;
use takeover::{Lease, Typing};
use world::Worlds;

/// DevTools as the driver needs it: the engine's connection, or a stand-in in tests.
pub(crate) trait DevTools: Send + Sync {
    /// Calls `method` on `session` (the browser itself when `None`), waiting up to `timeout`.
    fn call_within(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, CallError>;

    /// [`DevTools::call_within`], its answer held to `most` bytes: a longer one fails this call
    /// alone (#2694).
    fn call_within_bounded(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
        most: usize,
    ) -> Result<Value, CallError>;

    /// How many of the engine's events were dropped because nobody read them in time.
    fn dropped_events(&self) -> u64;

    /// Why the connection closed, once it has (#2745).
    fn closed(&self) -> Option<Closed> {
        None
    }

    fn call(&self, method: &str, params: Value, session: Option<&str>) -> Result<Value, CallError> {
        self.call_within(method, params, session, CALL_TIMEOUT)
    }

    /// Taps `session` (#2028): its messages, and those of the sessions under it, go to `sink`.
    /// `None` where it cannot be tapped.
    fn tap(&self, _session: &str, _sink: TapSink) -> Option<Tap> {
        None
    }

    /// Ends `tap`.
    fn untap(&self, _tap: &Tap) {}

    /// Sends a call on one of `tap`'s sessions, its answer the tap's under `id`.
    fn send_tapped(
        &self,
        _tap: &Tap,
        _id: u64,
        _method: &str,
        _params: Value,
        _session: &str,
    ) -> Result<(), CallError> {
        Err(CallError::Ended)
    }
}

impl DevTools for Connection {
    fn call_within(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, CallError> {
        Connection::call_within(self, method, params, session, timeout)
    }

    fn call_within_bounded(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
        most: usize,
    ) -> Result<Value, CallError> {
        Connection::call_within_bounded(self, method, params, session, timeout, most)
    }

    fn dropped_events(&self) -> u64 {
        Connection::dropped_events(self)
    }

    fn closed(&self) -> Option<Closed> {
        Connection::closed(self)
    }

    fn tap(&self, session: &str, sink: TapSink) -> Option<Tap> {
        Some(Connection::tap(self, session, sink))
    }

    fn untap(&self, tap: &Tap) {
        Connection::untap(self, tap);
    }

    fn send_tapped(
        &self,
        tap: &Tap,
        id: u64,
        method: &str,
        params: Value,
        session: &str,
    ) -> Result<(), CallError> {
        Connection::send_tapped(self, tap, id, method, params, session)
    }
}

/// How long an engine whose DevTools pipe closed has to end by itself before purlis ends it
/// (#2745): its process ends just after its pipe closes.
pub const ENDS_BY_ITSELF_WITHIN: Duration = Duration::from_secs(2);

/// **The engine a browser holds, as the driver ends it** (#2745): the real one's process
/// group, or a stand-in's in a test.
pub(crate) trait EngineHold: Send {
    /// Its own pid.
    fn pid(&self) -> u32;
    /// Asks it to close, waits up to `grace`, then ends it: how its process ended.
    fn stop(self: Box<Self>, grace: Duration) -> Option<EngineExit>;
    /// Waits up to `grace` for it to end by itself, then ends it: how its process ended, and
    /// whether it ended by itself.
    fn wait_or_end(self: Box<Self>, grace: Duration) -> (Option<EngineExit>, bool);
}

#[cfg(unix)]
impl EngineHold for crate::process::Engine {
    fn pid(&self) -> u32 {
        crate::process::Engine::pid(self)
    }

    fn stop(self: Box<Self>, grace: Duration) -> Option<EngineExit> {
        crate::process::Engine::stop(*self, grace)
    }

    fn wait_or_end(self: Box<Self>, grace: Duration) -> (Option<EngineExit>, bool) {
        crate::process::Engine::wait_or_end(*self, grace)
    }
}

/// Who is told once the engine has ended, or how it ended, once it has.
enum Ends {
    Waiting(Option<Box<dyn FnOnce(Ending) + Send>>),
    Ended(Ending),
}

/// The script that says whether a document has loaded: a constant, never the caller's words.
pub(crate) const READY_STATE: &str = "document.readyState";

/// The expression that reads a document's title (#2753), in purlis's own world.
pub(crate) const TITLE: &str = "document.title";

/// The script that answers once its document has drawn two frames (#2690): what a scroll moved
/// is then drawn, and the browser routes input by where it is drawn now. A constant.
pub(crate) const DRAWN_TWICE: &str =
    "new Promise((drawn) => requestAnimationFrame(() => requestAnimationFrame(() => drawn(true))))";

/// How long a document is given to draw those two frames.
const DRAWN_WITHIN: Duration = Duration::from_secs(1);

/// The function that chooses a drop-down's options, given the drop-down as it is in purlis's
/// own world (#2138). A constant: the options the caller named reach it only as its argument,
/// a value.
pub(crate) const SELECT_OPTIONS: &str = r"function (select, wanted) {
  if (!(select instanceof HTMLSelectElement)) return { notSelect: true };
  const options = Array.from(select.options);
  const named = (o, w) => o.value === w || o.label === w || o.text.trim() === w;
  const missing = wanted.filter((w) => !options.some((o) => named(o, w)));
  if (missing.length) return { missing, options: options.map((o) => o.label || o.text) };
  if (!select.multiple && wanted.length > 1) return { notMultiple: true };
  for (const o of options) o.selected = wanted.some((w) => named(o, w));
  select.dispatchEvent(new Event('input', { bubbles: true }));
  select.dispatchEvent(new Event('change', { bubbles: true }));
  return { picked: options.filter((o) => o.selected).map((o) => o.label || o.text) };
}";

/// The function that reads what a field holds, given the field as it is in purlis's own world
/// (#2014, #2138): a constant, whatever the field.
pub(crate) const READ_VALUE: &str = r"function (field) {
  if (typeof field.value === 'string') return field.value;
  if (field.isContentEditable) return field.innerText;
  return '';
}";

/// The script that says what the document has focused (#2014): a tag name, or nothing for the
/// document itself. A constant.
pub(crate) const ACTIVE_ELEMENT: &str = r"(() => {
  const at = document.activeElement;
  return at && at !== document.body && at !== document.documentElement ? at.tagName : '';
})()";

/// The expression that answers the element that has the focus, where the document it runs in has
/// the focus, or `null` (#2295). Run in purlis's own world, not by value.
pub(crate) const FOCUSED_HERE: &str = "document.hasFocus() ? document.activeElement : null";

/// Whether a key event pastes: it carries the paste command, or it is Shift+Insert (#2028).
fn pastes(event: &Value) -> bool {
    let commanded = event["commands"].as_array().is_some_and(|commands| {
        commands
            .iter()
            .any(|c| c == "paste" || c == "pasteAndMatchStyle")
    });
    let insert = event["key"] == "Insert" && event["modifiers"].as_i64().unwrap_or(0) & 8 != 0;
    commanded || insert
}

/// The first [`MOST_TITLE_CHARS`] of `title`: of a title already scrubbed, as a text cut first
/// could leave a part of what the scrub takes out.
fn kept_title(title: &str) -> String {
    title.chars().take(MOST_TITLE_CHARS).collect()
}

/// The drag event that cancels the engine's own drag carrying `data`, at `at`.
fn drag_cancel(at: (f64, f64), data: &Value) -> Value {
    json!({ "type": "dragCancel", "x": at.0, "y": at.1, "modifiers": 0, "data": data })
}

/// How long the driver gives the engine to say whether a document has loaded.
const READY_STATE_WITHIN: Duration = Duration::from_secs(2);
/// How long listing the pages gives the engine to read every page's title from its document,
/// all of them together (#2753): past it, the pages left keep the title the engine told of.
const TITLES_WITHIN: Duration = Duration::from_secs(1);
/// The most bytes the answer to one page's title read may take (#2753).
const MOST_TITLE_BYTES: usize = 64 << 10;
/// The most characters of a page's title kept, as the engine cuts the titles it tells of.
const MOST_TITLE_CHARS: usize = 4096;
/// How often a wait looks again.
const POLL: Duration = Duration::from_millis(50);
/// How often a wait for text reads the page again.
const READ_AGAIN: Duration = Duration::from_millis(250);
/// How many moves a drag makes between its ends.
const DRAG_STEPS: usize = 10;
/// How long apart two reads of where a scrolled frame is drawn are taken, to see it has come to
/// rest (#2557).
const AT_REST_GAP: Duration = Duration::from_millis(30);
/// How many reads a scrolled frame has to come to rest in before an act into it is not shown.
const AT_REST_READS: usize = 10;
/// The most form submissions, and links followed, remembered until their request is seen
/// (#2018).
const FORMS_KEPT: usize = 16;
/// The most document requests remembered for the redirects they may lead to (#2018).
const DOCUMENTS_KEPT: usize = 64;
/// The most sites asked about that wait for the next answer (#2018).
const ASKED_KEPT: usize = 16;
/// How long the driver waits for the engine to take its word on a held request (#2018).
const JUDGED_WITHIN: Duration = Duration::from_secs(5);

/// Whether a page may stay open at `address`: a web address, an empty page (or one with no
/// address yet), or the engine's own page for a load that failed. Every other page is closed.
fn may_stay_open(address: &str) -> bool {
    address.is_empty()
        || address == "about:blank"
        || address.starts_with("chrome-error://")
        || WebAddress::parse(address).is_ok()
}

struct Frame {
    session: String,
    parent: Option<String>,
    name: String,
    url: String,
    generation: u64,
    /// The page's document number when this frame last loaded another document, or when it
    /// was first seen (#2638).
    loaded: u64,
}

struct Page {
    id: PageId,
    target: String,
    session: Option<String>,
    url: String,
    title: String,
    settle: Settle,
    /// What blanked elements of this page held, kept out of every answer while the page lasts,
    /// whatever became of the elements.
    scrubbed: Kept,
    /// What the page shows, as a number that changes each time it or any frame in it loads
    /// another document, or a frame in it goes (#2014): the document a frame's aim names.
    shows: u64,
    /// The document number at which a frame of it last went, 0 if none has (#2638).
    gone: u64,
    frames: HashMap<String, Frame>,
    /// The page that opened this one, if one did.
    opener: Option<String>,
}

impl Page {
    /// `text` with what this page's blanked elements held taken out.
    fn shown(&self, text: &str) -> String {
        Scrub::of(&self.scrubbed).text(text)
    }

    /// This page's title as it is listed: [`Page::shown`], then cut.
    fn shown_title(&self) -> String {
        kept_title(&self.shown(&self.title))
    }

    /// [`Page::shown`] for an address, read as its reader would read it (decoded).
    fn shown_address(&self, address: &str) -> String {
        Scrub::of(&self.scrubbed).address(address)
    }

    /// The site this page is on: the one of the document it has loaded, not of an address it
    /// is still loading; until its document is known, the one of its address.
    fn site(&self) -> Option<String> {
        match self.frames.get(&self.target) {
            Some(frame) => site(&frame.url),
            None => site(&self.url),
        }
    }

    /// Keeps `held` out of every later answer about this page.
    fn keep_out(&mut self, held: &Kept) {
        self.scrubbed.take_in(held);
    }

    /// The frames between this page and `frame` (itself included) that have sessions of their
    /// own, outermost first: each one's id and its parent's session.
    fn boundaries(&self, frame: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut inner_id = frame.to_owned();
        let Some(mut inner) = self.frames.get(frame) else {
            return out;
        };
        while let Some(parent_id) = &inner.parent {
            let Some(parent) = self.frames.get(parent_id) else {
                break;
            };
            if parent.session != inner.session {
                out.push((inner_id.clone(), parent.session.clone()));
            }
            inner_id.clone_from(parent_id);
            inner = parent;
        }
        out.reverse();
        out
    }
}

enum Session {
    Page(String),
    Frame { page: String, frame: String },
}

/// The references dealt, both ways.
#[derive(Default)]
struct Refs {
    by_node: HashMap<(NodeKey, u64), Ref>,
    by_ref: HashMap<Ref, (String, NodeKey, u64)>,
    dealt: u64,
    /// Every reference up to this one was dealt before the person last handed the browser
    /// back (#2014): void.
    void_through: u64,
}

impl Refs {
    fn deal(&mut self, page: &str, key: &NodeKey, generation: u64) -> Ref {
        if let Some(dealt) = self.by_node.get(&(key.clone(), generation)) {
            return dealt.clone();
        }
        self.dealt += 1;
        let dealt = Ref(format!("e{}", self.dealt));
        self.by_node
            .insert((key.clone(), generation), dealt.clone());
        self.by_ref
            .insert(dealt.clone(), (page.to_owned(), key.clone(), generation));
        dealt
    }

    /// Forgets the references into `frame`'s documents.
    fn forget_frame(&mut self, frame: &str) {
        self.by_node.retain(|(key, _), _| key.frame != frame);
        self.by_ref.retain(|_, (_, key, _)| key.frame != frame);
    }

    /// Whether `written` names a reference this browser dealt, live or not.
    fn was_dealt(&self, written: &Ref) -> bool {
        number(written).is_some_and(|n| n >= 1 && n <= self.dealt)
    }

    /// Whether `written` was dealt before the person last handed the browser back.
    fn was_voided(&self, written: &Ref) -> bool {
        number(written).is_some_and(|n| n >= 1 && n <= self.void_through)
    }

    /// Voids every reference dealt so far: the next snapshot deals new ones.
    fn void(&mut self) {
        self.by_node.clear();
        self.by_ref.clear();
        self.void_through = self.dealt;
    }
}

/// The number a reference was dealt as (`e12`: 12).
fn number(written: &Ref) -> Option<u64> {
    written.as_str().strip_prefix('e')?.parse().ok()
}

#[derive(Default)]
struct State {
    pages: Vec<Page>,
    current: Option<String>,
    sessions: HashMap<String, Session>,
    refs: Refs,
    excluded: HashSet<NodeKey>,
    pages_listed: u64,
    generations: u64,
    ended: bool,
    /// The size the person set its pages to, in CSS pixels (#2014): every page is given it.
    size: Option<(u16, u16)>,
    /// What the person typed while driving (#2014): kept out of every answer about every page
    /// of the browser, a page opened later included.
    kept: Kept,
    /// The page an act of the agent's is sending somewhere, while that act lasts, and what its
    /// documents are (#2018): the agent's address, or a read for going back.
    addressing: Option<(String, Why)>,
    /// Addresses forms were submitted to, each until its request is seen (#2018).
    forms: VecDeque<String>,
    /// Addresses a page asked to go to by a link or a script, each until its request is seen.
    offered: VecDeque<String>,
    /// Each document request held lately, and why it was sent: what a redirect from it is.
    documents: VecDeque<(String, Why)>,
    /// The sites the judge asked about since the last answer, each once (#2018).
    asked: Vec<String>,
    /// The sites a text was kept out on (#2303): their pages' screenshots paint over what
    /// cannot be read.
    kept_sites: HashSet<String>,
    /// A text was kept out on a page on no site with no opener: every page's screenshot paints
    /// over what cannot be read.
    kept_off_site: bool,
    /// What the engine's own drag carries, once a drag of the agent's started one (#2136), by
    /// the session it started on.
    dragged: Option<(String, Value)>,
    /// Where a drag of the agent's ended without the engine's word that its own drag started,
    /// by session (#2839): a word that comes later is answered by cancelling that drag there.
    cancels_late: Option<(String, (f64, f64))>,
    /// Every download the browser began, and which are still to be told (#2020).
    downloads: downloads::Downloads,
    /// What the driver leaves to an Inspect (#2028).
    inspecting: Inspecting,
}

/// **What the driver leaves to an Inspect** (#2028): the engine's word of these is the front
/// end's, never the driver's, so no page of DevTools is ever one of the browser's pages.
#[derive(Default)]
struct Inspecting {
    /// The contexts DevTools' front ends run in.
    contexts: HashSet<String>,
    /// Each context's front end (or blank page of DevTools'), by context: every other page of
    /// that context is closed at once.
    front_ends: HashMap<String, String>,
    /// The pages a session is being made on for DevTools to speak to them.
    bridging: HashSet<String>,
    /// The pages a session for DevTools was asked of and not answered in time: one the engine
    /// tells of later is let go.
    orphans: HashSet<String>,
}

impl Inspecting {
    /// Whether the target `info` tells of is DevTools' own.
    fn owns(&self, info: &Value) -> bool {
        info.get("browserContextId")
            .and_then(Value::as_str)
            .is_some_and(|context| self.contexts.contains(context))
            || info
                .get("targetId")
                .and_then(Value::as_str)
                .is_some_and(|target| self.front_ends.values().any(|one| one == target))
    }
}

impl State {
    /// **Why a held request is sent** (#2018). A body sends. A document is judged by how its
    /// navigation began: a redirect as the request it came from; a form the page submitted
    /// sends; the page an act of the agent's sends to an address is that address (or, going
    /// back, a read); a link or a script of the page reads. A document the page asked for with
    /// nothing said of how, whose address carries a query, sends, so a submission whose word
    /// came late is never taken for a read. Anything else reads.
    fn why(&mut self, held: &Held<'_>) -> Why {
        let bare = without_fragment(held.address);
        let why = if !matches!(held.method, "GET" | "HEAD" | "OPTIONS") {
            Why::Sends
        } else if !held.document {
            return Why::Reads;
        } else if let Some(from) = held
            .redirected_from
            .and_then(|from| self.documents.iter().find(|(id, _)| id == from))
        {
            from.1
        } else if take(&mut self.forms, bare) {
            Why::Sends
        } else if let Some((page, why)) = &self.addressing
            && held.frame == Some(page.as_str())
        {
            *why
        } else if take(&mut self.offered, bare) {
            Why::Reads
        } else if bare.contains('?') {
            Why::Sends
        } else {
            Why::Reads
        };
        if held.document {
            if self.documents.len() >= DOCUMENTS_KEPT {
                self.documents.pop_front();
            }
            self.documents.push_back((held.request.to_owned(), why));
        }
        why
    }

    /// **The addresses what is in document `frame` of `target`'s page goes to** (#2018): the
    /// frame's own address where it is a web address; for a frame at none (`about:blank`, a
    /// `srcdoc`), its nearest ancestor's that is, since that document holds it; and for a
    /// frame the driver does not know, every document the page shows.
    fn documents_of(&self, target: &str, frame: &str) -> Vec<String> {
        let Some(page) = self.page(target) else {
            return Vec::new();
        };
        let web = |url: &str| WebAddress::parse(url).is_ok();
        if frame == target {
            return vec![page.url.clone()];
        }
        let Some(mut at) = page.frames.get(frame) else {
            return self.every_document(target);
        };
        for _ in 0..=page.frames.len() {
            if web(&at.url) {
                return vec![at.url.clone()];
            }
            match at.parent.as_deref() {
                Some(parent) if parent == target => break,
                Some(parent) => match page.frames.get(parent) {
                    Some(up) => at = up,
                    None => return self.every_document(target),
                },
                None => break,
            }
        }
        vec![page.url.clone()]
    }

    /// Every document `target`'s page shows, its own first.
    fn every_document(&self, target: &str) -> Vec<String> {
        self.page(target).map_or_else(Vec::new, |page| {
            let mut every: Vec<String> = page.frames.values().map(|f| f.url.clone()).collect();
            every.sort();
            every.dedup();
            every.retain(|one| *one != page.url);
            every.insert(0, page.url.clone());
            every
        })
    }

    /// Keeps `site` to be said in the next answer, once.
    fn ask(&mut self, site: &str) {
        if !self.asked.iter().any(|one| one == site) && self.asked.len() < ASKED_KEPT {
            self.asked.push(site.to_owned());
        }
    }

    /// The site `target`'s page is on ([`Page::site`]), or for a page on none, the site of the
    /// page that opened it.
    fn site_of(&self, target: &str) -> Option<String> {
        let mut at = self.page(target)?;
        for _ in 0..=self.pages.len() {
            if let Some(site) = at.site() {
                return Some(site);
            }
            at = self.page(at.opener.as_deref()?)?;
        }
        None
    }

    /// A text was kept out on `target`'s page: its site's pages paint over what cannot be read.
    fn kept_on(&mut self, target: &str) {
        match self.site_of(target) {
            Some(site) => {
                self.kept_sites.insert(site);
            }
            None => self.kept_off_site = true,
        }
    }

    /// Whether a text was kept out anywhere in the browser.
    fn kept_anything(&self) -> bool {
        !self.kept_sites.is_empty() || self.kept_off_site
    }

    /// Whether a page showing `address` paints over what cannot be read: its site is one a text
    /// was kept out on, or it has none while anything is kept out.
    fn greys_at(&self, address: &str) -> bool {
        self.kept_off_site
            || match site(address) {
                Some(site) => self.kept_sites.contains(&site),
                None => self.kept_anything(),
            }
    }

    /// Whether screenshots of `target`'s page paint over what cannot be read (the person's
    /// ruling of 2026-10-17): it, or any page in the chain that opened it, is on a site a text
    /// was kept out on; or a page in that chain is on no site and was opened by none it can
    /// still tell, while anything is kept out.
    fn greys(&self, target: &str) -> bool {
        if !self.kept_anything() {
            return false;
        }
        if self.kept_off_site {
            return true;
        }
        let Some(mut at) = self.page(target) else {
            return true;
        };
        for _ in 0..=self.pages.len() {
            let own = at.site();
            if own.as_ref().is_some_and(|s| self.kept_sites.contains(s)) {
                return true;
            }
            match at.opener.as_deref() {
                None => return own.is_none(),
                Some(opener) => match self.page(opener) {
                    Some(by) => at = by,
                    // The page that opened it is gone: what it was on cannot be told.
                    None => return true,
                },
            }
        }
        true
    }

    fn page(&self, target: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.target == target)
    }

    fn page_mut(&mut self, target: &str) -> Option<&mut Page> {
        self.pages.iter_mut().find(|p| p.target == target)
    }

    fn page_of_session(&self, session: &str) -> Option<String> {
        match self.sessions.get(session)? {
            Session::Page(target) => Some(target.clone()),
            Session::Frame { page, .. } => Some(page.clone()),
        }
    }

    /// Keeps `held` out of every later answer about every page, and every page opened later.
    fn keep_out_everywhere(&mut self, held: &Kept) {
        self.kept.take_in(held);
        let kept = self.kept.clone();
        for page in &mut self.pages {
            page.keep_out(&kept);
        }
    }

    /// What a frame of `target`'s page shows now: the page, and its main frame's document.
    /// The address of the document page `target` has loaded, as the engine records it, or of
    /// the address it is loading until its document is known (#2300): never read from the page.
    fn document_address(&self, target: &str) -> Option<String> {
        let page = self.page(target)?;
        Some(match page.frames.get(&page.target) {
            Some(frame) => frame.url.clone(),
            None => page.url.clone(),
        })
    }

    fn aim(&self, target: &str) -> Option<Aim> {
        let page = self.page(target)?;
        page.frames.get(target)?;
        Some(Aim {
            page: page.id.clone(),
            document: page.shows,
        })
    }

    /// Whether what `target`'s page shows has moved on since the document number `seen`, at
    /// the frame `reached` (#2638): that frame, or one around it, loaded another document since,
    /// or, `with_inside`, one inside it did. A frame not known now has moved on.
    fn moved_at(&self, target: &str, seen: u64, reached: &str, with_inside: bool) -> bool {
        let Some(page) = self.page(target) else {
            return true;
        };
        let Some(frame) = page.frames.get(reached) else {
            return true;
        };
        // The frame and every frame around it.
        let mut at = Some((reached, frame));
        let mut steps = 0;
        while let Some((_, frame)) = at {
            steps += 1;
            if frame.loaded > seen || steps > page.frames.len() {
                return true;
            }
            at = frame
                .parent
                .as_deref()
                .and_then(|parent| page.frames.get_key_value(parent))
                .map(|(id, frame)| (id.as_str(), frame));
        }
        // Every frame inside it, where what is under a point may be any of them.
        with_inside
            && page.frames.iter().any(|(id, frame)| {
                frame.loaded > seen && id != reached && {
                    let mut up = frame.parent.as_deref();
                    let mut steps = 0;
                    loop {
                        steps += 1;
                        match up {
                            Some(parent) if parent == reached => break true,
                            Some(parent) if steps <= page.frames.len() => {
                                up = page.frames.get(parent).and_then(|f| f.parent.as_deref());
                            }
                            _ => break false,
                        }
                    }
                }
            })
    }

    fn info(&self, target: &str) -> Option<PageInfo> {
        self.page(target).map(|p| PageInfo {
            id: p.id.clone(),
            address: p.shown_address(&p.url),
            title: p.shown_title(),
            current: self.current.as_deref() == Some(target),
        })
    }

    /// Records `frame` (as the engine describes frames) as seen in `session`.
    fn frame_seen(&mut self, session: &str, frame: &Value, navigated: bool) {
        let Some(target) = self.page_of_session(session) else {
            return;
        };
        let Some(id) = frame.get("id").and_then(Value::as_str) else {
            return;
        };
        // A frame with a session of its own is that session's, whatever its parent says.
        let owned_elsewhere = self.sessions.iter().any(|(s, kind)| {
            s != session && matches!(kind, Session::Frame { frame, .. } if frame == id)
        });
        self.generations += 1;
        let generation = self.generations;
        let text = |field: &str| {
            frame
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let Some(page) = self.page_mut(&target) else {
            return;
        };
        let parent = frame
            .get("parentId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let shown = page.shows;
        let entry = page.frames.entry(id.to_owned()).or_insert_with(|| Frame {
            session: session.to_owned(),
            parent: None,
            name: String::new(),
            url: String::new(),
            generation,
            loaded: shown,
        });
        if parent.is_some() {
            entry.parent = parent;
        }
        if owned_elsewhere {
            // Only where it sits is the parent's to say.
            return;
        }
        entry.session = session.to_owned();
        entry.name = text("name");
        entry.url = text("url");
        if navigated {
            page.shows = generation;
            entry.generation = generation;
            entry.loaded = generation;
            if id == target {
                page.url = text("url");
            }
            self.refs.forget_frame(id);
            self.excluded.retain(|key| key.frame != id);
        }
    }

    /// `frame` of `target`'s page now lives in `session`, a process of its own (#2973): where
    /// purlis knew the frame before, it counts as having loaded another document, as a
    /// navigation does, whatever else the engine says of the swap.
    fn frame_swapped(&mut self, target: &str, frame: &str, session: &str) {
        let Some(page) = self.page_mut(target) else {
            return;
        };
        let Some(entry) = page.frames.get_mut(frame) else {
            return;
        };
        if entry.session == session {
            return;
        }
        entry.session = session.to_owned();
        self.generations += 1;
        let generation = self.generations;
        if let Some(page) = self.page_mut(target)
            && let Some(entry) = page.frames.get_mut(frame)
        {
            entry.generation = generation;
            entry.loaded = generation;
            page.shows = generation;
        }
        self.refs.forget_frame(frame);
        self.excluded.retain(|key| key.frame != frame);
    }

    /// Forgets `frame` and every frame inside it, and answers them.
    fn frame_gone(&mut self, target: &str, frame: &str) -> Vec<String> {
        self.generations += 1;
        let shows = self.generations;
        let Some(page) = self.page_mut(target) else {
            return Vec::new();
        };
        let mut gone = vec![frame.to_owned()];
        let mut at = 0;
        while at < gone.len() {
            let parent = gone[at].clone();
            gone.extend(
                page.frames
                    .iter()
                    .filter(|(_, f)| f.parent.as_deref() == Some(parent.as_str()))
                    .map(|(id, _)| id.clone()),
            );
            at += 1;
        }
        for id in &gone {
            page.frames.remove(id);
        }
        page.shows = shows;
        page.gone = shows;
        for id in &gone {
            self.refs.forget_frame(id);
            self.excluded.retain(|key| &key.frame != id);
        }
        gone
    }
}

/// What the event reader does after it lets go of the state.
enum FollowUp {
    Attach(String),
    Close(String),
    SetUp(String),
    Release(String),
    /// The engine's own drag, told of only after the agent's drag that started it ended
    /// (#2839): cancelled at where that drag ended.
    CancelDrag {
        session: String,
        at: (f64, f64),
        data: Value,
    },
    /// A download past the browser's limits, cancelled where it still downloads and its file
    /// removed (#2020).
    DiscardDownload(downloads::Discard),
    /// A request of DevTools' front end, which never goes (#2028).
    Refuse(String),
    /// A request held until the judge says whether it goes (#2018).
    Judge {
        paused: Paused,
        outbound: Outbound,
    },
}

pub(crate) struct Inner {
    pub(crate) devtools: Arc<dyn DevTools>,
    /// The person's open DevTools, if any (#2028).
    inspection: Mutex<Option<Arc<inspect::Inspection>>>,
    /// Whether DevTools were open while the person drove, since they last handed it back.
    devtools_opened: AtomicBool,
    /// Whether the agent's pastes are withheld: DevTools were open on this browser, and what
    /// the person copied there may be what a paste gets.
    paste_withheld: AtomicBool,
    /// What DevTools wrote into pages since then, kept out at the hand-back.
    devtools_writes: Mutex<Vec<String>>,
    /// The pages that could not be read at the hand-back after DevTools: the agent's calls
    /// wait until each can be.
    devtools_unread: Mutex<HashSet<String>>,
    state: Mutex<State>,
    changed: Condvar,
    limits: Limits,
    /// Frames for the person to watch (#2012).
    pub(crate) cast: Screencast,
    /// Who is told once the engine has ended, however it ended, and how it ended (#2745).
    ends: Mutex<Ends>,
    /// The engine, until it is stopped or has ended: behind a lock of its own, so a stop never
    /// waits for a call in flight. Whoever takes it out tells how it ended.
    engine: Mutex<Option<Box<dyn EngineHold>>>,
    /// Whether it was ever given an engine: only then does a stop wait for an end it did not
    /// make.
    held_one: AtomicBool,
    /// Whether whoever waits on its end has been told, and has returned.
    told: Mutex<bool>,
    /// Woken as `told` is set.
    told_now: Condvar,
    /// Whether purlis is stopping the engine: its pipe's end is then the stop's to tell.
    stopping: AtomicBool,
    /// Who judges where its pages may go (#2018), when anyone does.
    judge: Option<Judge>,
    /// Whether the person drives (#2014): every request is then theirs (#2018).
    person_drives: AtomicBool,
    /// Whether purlis drives, performing a login (#2016): every request is then purlis's.
    purlis_drives: AtomicBool,
    /// Where each held request is sent to be judged, on a thread of its own (#2018).
    judging: Mutex<Option<std::sync::mpsc::Sender<(Paused, Outbound)>>>,
    /// Where a judged browser's other follow-ups are carried out, in order, on a thread of their
    /// own (#2018): each waits on the engine, and the event reader must stay free to hear every
    /// held request, or a page's load waits on a call that waits on the load.
    following: Mutex<Option<std::sync::mpsc::Sender<FollowUp>>>,
    /// purlis's own world in each frame its scripts run in, as the engine told of each (#2138).
    worlds: Worlds,
    /// The folder its downloads are held in, once it was given one (#2020).
    held_in: Mutex<Option<std::path::PathBuf>>,
}

/// A frame of a page to read: where its tree comes from and where its holder is.
struct ToRead {
    frame: FrameAx,
    session: String,
    parent_session: Option<String>,
}

/// The frame at the top of one of a page's sessions: its parent's session, and the frames
/// between it and the page that have sessions of their own.
struct Root {
    frame: String,
    session: String,
    parent: Option<String>,
    boundaries: Vec<(String, String)>,
}

/// Where an element is, for acting on it.
struct Located {
    target: String,
    page_session: String,
    node_session: String,
    /// The frame whose document holds the element.
    frame: String,
    backend: i64,
    /// Frames between the page and the element that have sessions of their own, outermost
    /// first: each frame's id and its parent's session.
    boundaries: Vec<(String, String)>,
}

/// One browser's pages, driven by typed calls.
pub struct Browser {
    inner: Arc<Inner>,
    ledger: Ledger<Call, Result<Answer, Refusal>>,
    /// One call acts at a time.
    acting: Mutex<()>,
    /// Input events sent to pages, for an answer that says how far an action got.
    inputs: AtomicU64,
    /// Who drives (#2014), and the lease's epoch: every act the agent's calls send checks it
    /// under this lock, so none reaches a page once the person has taken it over.
    lease: Mutex<Lease>,
    /// The epoch of the lease the call now acting was made under.
    acting_epoch: AtomicU64,
    /// Counts the hand-backs, each under the lease's lock: an Inspect that began before one
    /// never finishes opening (#2028).
    inspect_cut: AtomicU64,
    /// What the person typed while driving, kept out of later answers (#2014).
    typing: Mutex<Typing>,
    /// What each login the person cut by taking the browser over filled, cleared as they hand
    /// it back (#2016): the page's target, the login's origin, and the fields.
    to_clear: Mutex<Vec<(String, String, Vec<NodeKey>)>>,
    /// Each page's tree as it was last read, to say what changed since (#2137).
    read_last: changes::ReadLast,
    /// The read of a page's trees for the agent's calls under way, or come and not yet used,
    /// that a call which outlived it left (#3277): the next call that reads the page waits on it.
    reading: Mutex<reading::Slot>,
}

impl Browser {
    /// Drives `engine`'s pages within `limits`. The engine's events are the browser's from now.
    #[cfg(unix)]
    pub fn new(engine: crate::process::Engine, limits: Limits) -> Result<Self, Refusal> {
        Self::driving(engine, limits, None)
    }

    /// [`Browser::new`], every request of every page held until `judge` has said whether it
    /// goes (#2018). Refused where the engine will not hold them, so a browser never runs
    /// unjudged that was meant to be judged.
    #[cfg(unix)]
    pub fn judged(
        engine: crate::process::Engine,
        limits: Limits,
        judge: Judge,
    ) -> Result<Self, Refusal> {
        Self::driving(engine, limits, Some(judge))
    }

    #[cfg(unix)]
    fn driving(
        mut engine: crate::process::Engine,
        limits: Limits,
        judge: Option<Judge>,
    ) -> Result<Self, Refusal> {
        let events = engine.take_events().ok_or(Refusal::Ended)?;
        let devtools: Arc<dyn DevTools> = engine.devtools_shared();
        let browser = Self::over(devtools, events, limits, judge)?;
        browser.inner.hold(Box::new(engine));
        Ok(browser)
    }

    /// Drives the pages `devtools` reaches, reading their events from `events`, each request
    /// told to `judge` where there is one.
    pub(crate) fn over(
        devtools: Arc<dyn DevTools>,
        events: Receiver<Event>,
        limits: Limits,
        judge: Option<Judge>,
    ) -> Result<Self, Refusal> {
        let inner = Arc::new(Inner {
            devtools,
            inspection: Mutex::new(None),
            devtools_opened: AtomicBool::new(false),
            paste_withheld: AtomicBool::new(false),
            devtools_writes: Mutex::new(Vec::new()),
            devtools_unread: Mutex::new(HashSet::new()),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            limits: limits.clone(),
            cast: Screencast::new(),
            ends: Mutex::new(Ends::Waiting(None)),
            engine: Mutex::new(None),
            held_one: AtomicBool::new(false),
            told: Mutex::new(false),
            told_now: Condvar::new(),
            stopping: AtomicBool::new(false),
            judge,
            person_drives: AtomicBool::new(false),
            purlis_drives: AtomicBool::new(false),
            judging: Mutex::new(None),
            following: Mutex::new(None),
            worlds: Worlds::default(),
            held_in: Mutex::new(None),
        });
        Screencast::start(&inner);
        if inner.judge.is_some() {
            let (judging, held) = std::sync::mpsc::channel::<(Paused, Outbound)>();
            let judge_for = Arc::downgrade(&inner);
            thread::Builder::new()
                .name("browser-judge".to_owned())
                .spawn(move || {
                    for (paused, outbound) in held {
                        let Some(inner) = judge_for.upgrade() else {
                            break;
                        };
                        inner.judged(&paused, &outbound);
                    }
                })
                .map_err(|e| Refusal::Engine(e.to_string()))?;
            *lock(&inner.judging) = Some(judging);
            let (following, steps) = std::sync::mpsc::channel::<FollowUp>();
            let follow_for = Arc::downgrade(&inner);
            thread::Builder::new()
                .name("browser-follow-ups".to_owned())
                .spawn(move || {
                    for step in steps {
                        let Some(inner) = follow_for.upgrade() else {
                            break;
                        };
                        inner.follow_up(step);
                    }
                })
                .map_err(|e| Refusal::Engine(e.to_string()))?;
            *lock(&inner.following) = Some(following);
        }
        let reading = inner.clone();
        thread::Builder::new()
            .name("browser-events".to_owned())
            .spawn(move || {
                for event in events {
                    reading.take_in(&event);
                }
                lock(&reading.state).ended = true;
                reading.changed.notify_all();
                reading.connection_ended();
            })
            .map_err(|e| Refusal::Engine(e.to_string()))?;
        // Held browser-wide, before any page is known, so no page sends anything unjudged:
        // every frame's, every worker's. Only web addresses go out through the reach proxy,
        // so only they are held: the engine's own pages and extensions load as they are.
        if inner.judge.is_some() {
            inner
                .devtools
                .call(
                    "Fetch.enable",
                    json!({ "patterns": [
                        { "urlPattern": "http://*", "requestStage": "Request" },
                        { "urlPattern": "https://*", "requestStage": "Request" },
                    ] }),
                    None,
                )
                .map_err(refusal)?;
        }
        inner
            .devtools
            .call(
                "Target.setDiscoverTargets",
                json!({ "discover": true }),
                None,
            )
            .map_err(refusal)?;
        // The engine starts with a page: it is the current one once it is attached and its
        // frames are known.
        let deadline = Instant::now() + limits.settle_within;
        let mut state = lock(&inner.state);
        let ready = |state: &State| {
            state
                .current
                .as_deref()
                .and_then(|t| state.page(t))
                .is_some_and(|p| p.frames.contains_key(&p.target))
        };
        while !ready(&state) && !state.ended && Instant::now() < deadline {
            state = inner
                .changed
                .wait_timeout(state, POLL)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        drop(state);
        Ok(Self {
            inner,
            ledger: Ledger::new(limits.answers_kept),
            acting: Mutex::new(()),
            inputs: AtomicU64::new(0),
            lease: Mutex::new(Lease::default()),
            acting_epoch: AtomicU64::new(0),
            inspect_cut: AtomicU64::new(0),
            typing: Mutex::new(Typing::default()),
            to_clear: Mutex::new(Vec::new()),
            read_last: changes::ReadLast::default(),
            reading: Mutex::new(reading::Slot::default()),
        })
    }

    /// Asks the engine to close, waits up to `grace`, then ends it. A call in flight then
    /// answers that the browser has ended. Whoever waits on its end is told purlis stopped it
    /// (#2745). An engine ending already, its pipe closed, is waited for until whoever waits
    /// has been told (#2745): a stop returns once the end is wholly told, however it came.
    pub fn stop(&self, grace: Duration) {
        let engine = {
            let mut engine = lock(&self.inner.engine);
            let taken = engine.take();
            if taken.is_some() {
                self.inner.stopping.store(true, Ordering::SeqCst);
            }
            taken
        };
        match engine {
            Some(engine) => {
                let exit = engine.stop(grace);
                self.inner.ended_with(Ending {
                    by: EndedBy::Stopped,
                    exit,
                    closed: None,
                });
            }
            None => self.inner.until_told(grace + ENDS_BY_ITSELF_WITHIN),
        }
    }

    /// Holds `engine` as this browser's, in a test.
    #[cfg(test)]
    pub(crate) fn holding(&self, engine: Box<dyn EngineHold>) {
        self.inner.hold(engine);
    }

    /// Answers `call`. A call whose `id` was answered before answers the same again and acts on
    /// nothing; an `id` given to a different call is refused. Refused while the person drives
    /// (#2014).
    pub fn call(&self, id: &CallId, call: Call) -> Result<Answer, Refusal> {
        self.call_at(self.epoch(), id, call)
    }

    /// [`Browser::call`], made under the lease of `epoch` (#2014): refused unless the agent
    /// holds that lease now, and cut, answering [`Refusal::Interrupted`], if the person takes
    /// the browser over before it answers.
    pub fn call_at(&self, epoch: u64, id: &CallId, call: Call) -> Result<Answer, Refusal> {
        self.agent_holds(epoch)?;
        // A call that only reads acts on nothing, so a retry of it reads again; only what acts
        // is kept by id.
        if call.only_reads() {
            let _one = lock(&self.acting);
            let every = matches!(call, Call::Downloads);
            return self.run_at(epoch, call).map(|answer| {
                let mut answer = self.with_asked(answer);
                if every {
                    answer.downloads = lock(&self.inner.state).downloads.all();
                }
                answer
            });
        }
        match self.ledger.begin(id, &call) {
            Begin::Fresh(ticket) => {
                let answer = {
                    let _one = lock(&self.acting);
                    // A call that acts leaves a read an earlier call left running of the page
                    // before it for no later call (#3277): forgotten once no read-only call
                    // runs, so none begun before the act is kept past it.
                    lock(&self.reading).forget();
                    self.run_at(epoch, call)
                        .map(|answer| self.with_asked(answer))
                };
                ticket.finish(answer.clone());
                answer
            }
            Begin::Repeat(answer) => answer.map(|mut a| {
                a.repeated = true;
                a
            }),
            Begin::Reused => Err(Refusal::IdReused(id.as_str().to_owned())),
        }
    }

    /// `answer`, with the sites the judge asked about since the last answer (#2018).
    /// And the downloads that began or moved on since (#2020).
    fn with_asked(&self, mut answer: Answer) -> Answer {
        let mut state = lock(&self.inner.state);
        answer.asked = std::mem::take(&mut state.asked);
        answer.downloads = state.downloads.untold();
        if let Some(refused) = state.downloads.refused_since() {
            let said = format!(
                "purlis cancelled {refused} more {} the page began: a browser makes at most {} \
                 downloads",
                if refused == 1 {
                    "download"
                } else {
                    "downloads"
                },
                self.inner.limits.most_downloads
            );
            answer.note = Some(match answer.note.take() {
                Some(note) => format!("{note}; {said}"),
                None => said,
            });
        }
        answer
    }

    /// Every download the browser began, as it is now (#2020): told to nobody by this.
    pub fn downloads(&self) -> Vec<super::DownloadInfo> {
        lock(&self.inner.state).downloads.all()
    }

    /// **Holds every download of the browser's in `folder`** (#2020), each under the engine's
    /// own name for it, and has the engine tell of each as it goes. Browser-wide: every page's,
    /// one opened later included. Refused where the engine will not, so a browser never runs
    /// downloading where it was not told to.
    pub fn hold_downloads_in(&self, folder: &std::path::Path) -> Result<(), Refusal> {
        *lock(&self.inner.held_in) = Some(folder.to_path_buf());
        self.devtools()
            .call(
                "Browser.setDownloadBehavior",
                json!({
                    "behavior": "allowAndName",
                    "downloadPath": folder.to_string_lossy(),
                    "eventsEnabled": true,
                }),
                None,
            )
            .map(drop)
            .map_err(refusal)
    }

    /// **Where the agent is about to type, keys or choices into the documents at `addresses`
    /// of `target`'s page** (#2018): each judged as something sent to its site. Where the judge
    /// asks about any, nothing is sent to the page, and the answer says so.
    fn typing_held(&self, target: &str, addresses: &[String]) -> Option<Answer> {
        let site = self.sends_to_a_new_site(addresses)?;
        let mut answer = self.answer_now(target, Content::Nothing);
        answer.note = Some(format!(
            "nothing was typed: this page shows {site}, a site new to this project, where this \
             would have gone, and purlis asked the person whether this browser may send to it"
        ));
        Some(answer)
    }

    /// **Whether sending to the documents at `addresses` waits on the person** (#2018): each is
    /// judged as something sent to its site; the first site the judge asks about, now asked.
    fn sends_to_a_new_site(&self, addresses: &[String]) -> Option<String> {
        let judge = self.inner.judge.as_ref()?;
        let site = addresses.iter().find_map(|address| {
            let outbound = Outbound {
                address: address.clone(),
                method: String::new(),
                why: Why::Sends,
                by: Driver::Agent,
            };
            match judge(&outbound) {
                Gate::Ask { site } => Some(site),
                Gate::Go => None,
            }
        })?;
        lock(&self.inner.state).ask(&site);
        Some(site)
    }

    /// The addresses what is typed into the element `written` names goes to: its document's.
    fn addresses_of(&self, written: &Ref) -> Result<Vec<String>, Refusal> {
        let state = lock(&self.inner.state);
        let (page, key) = live_ref(&state, written)?;
        Ok(state.documents_of(&page, &key.frame))
    }

    /// The addresses of the documents what has the focus on `target`'s page may be in: the one
    /// document of the focused element, where one is focused, and otherwise every document the
    /// page shows, since a key goes wherever the focus is.
    fn focus_addresses(&self, target: &str) -> Vec<String> {
        if self.inner.judge.is_none() {
            return Vec::new();
        }
        let focused = self
            .snapshot_of(target)
            .ok()
            .and_then(|snapshot| match snapshot.focused() {
                [one] => Some(one.frame.clone()),
                _ => None,
            });
        let state = lock(&self.inner.state);
        match focused {
            Some(frame) => state.documents_of(target, &frame),
            None => state.every_document(target),
        }
    }

    /// Marks `target`'s page as the one the agent's act is sending to an address, until the
    /// mark is dropped (#2018).
    fn addressing(&self, target: &str, why: Why) -> Addressing<'_> {
        lock(&self.inner.state).addressing = Some((target.to_owned(), why));
        Addressing(&self.inner)
    }

    /// The engine's own pid, while this browser holds its engine.
    pub fn process(&self) -> Option<u32> {
        lock(&self.inner.engine).as_ref().map(|engine| engine.pid())
    }

    /// Calls `tell` once the engine has ended, however it ended, with how it ended (#2745): at
    /// once if it has.
    pub fn when_it_ends(&self, tell: Box<dyn FnOnce(Ending) + Send>) {
        let now = {
            let mut ends = lock(&self.inner.ends);
            match &mut *ends {
                Ends::Ended(ending) => Some(ending.clone()),
                Ends::Waiting(waiting) => {
                    *waiting = Some(tell);
                    return;
                }
            }
        };
        if let Some(ending) = now {
            tell(ending);
        }
    }

    /// Starts the frames of its current page for the person to watch, each JPEG handed to
    /// `frames` as the engine paints it, until [`Browser::hide`] (#2012). Returns at once.
    pub fn show(&self, frames: Arc<dyn Fn(Painted) + Send + Sync>) {
        self.inner.cast.show(frames);
    }

    /// Asks the engine for a frame no more often than `every` (zero: as often as the page
    /// changes). Returns at once.
    pub fn pace(&self, every: Duration) {
        self.inner.cast.pace(every);
    }

    /// Asks the engine for frames no wider than `most` pixels, scaled down where its pages are
    /// wider, or as wide as its pages with none (#2050). Returns at once.
    pub fn narrow(&self, most: Option<u16>) {
        self.inner.cast.narrow(most);
    }

    /// Stops its frames. Returns at once.
    pub fn hide(&self) {
        self.inner.cast.hide();
    }

    /// Adds the element `target` names to the blanked set: from now on no snapshot, find, wait
    /// or screenshot shows it while it exists, and what it holds is kept out of every answer
    /// about its page while the page lasts. The hook a login, and the person's typing, fill.
    pub fn blank(&self, target: &Ref) -> Result<(), Refusal> {
        let (page, key) = live_ref(&lock(&self.inner.state), target)?;
        self.blank_node(&page, key).map(|_| ())
    }

    /// [`Browser::blank`] for the element that has the focus on `page` (the current page when
    /// `None`), whatever it is: the field the person is typing into, even one that is no form
    /// field and has no reference.
    pub fn blank_focused(&self, page: Option<&PageId>) -> Result<(), Refusal> {
        self.blank_what_has_focus(page).map(|_| ())
    }

    /// [`Browser::blank_focused`], answering the element it blanked and its page's target, and
    /// the page as it was read once the element was blanked.
    fn blank_what_has_focus(
        &self,
        page: Option<&PageId>,
    ) -> Result<((String, NodeKey), Snapshot), Refusal> {
        let target = self.target(page)?;
        self.blank_focus_in(&target, None)
    }

    /// [`Browser::blank_what_has_focus`] on `target`'s page, from `read` where the page's trees
    /// were read for this key already (#2974): the page is read once, and the element blanked
    /// in a build of the same read.
    pub(super) fn blank_focus_in(
        &self,
        target: &str,
        read: Option<Vec<FrameAx>>,
    ) -> Result<((String, NodeKey), Snapshot), Refusal> {
        let target = target.to_owned();
        let trees = match read {
            Some(trees) => trees,
            None => self.trees_of(&target)?,
        };
        let snapshot = self.snapshot_from(&target, &trees);
        let key = match snapshot.focused() {
            [] => return Err(Refusal::NothingFocused),
            // The tree may mark an element the page calls active (`aria-activedescendant`)
            // while a field has the document's focus: the field, which the key goes to, is the
            // one blanked (#2972). Where the document cannot say, the tree's is.
            [one] => self.document_s_own(&target, one),
            // The tree marks more than one element focused (a real engine may mark a field's
            // inner editor, or its document, with it): the one its document says has the
            // focus is the one (#2295).
            many => self
                .the_one_focused(&target, many)
                .ok_or(Refusal::FocusUnclear)?,
        };
        {
            let mut state = lock(&self.inner.state);
            state.excluded.insert(key.clone());
            state.kept_on(&target);
        }
        // What the element holds is kept out from this moment: the key is not sent yet, so the
        // page as read holds what it holds now.
        let read = self.snapshot_from(&target, &trees);
        Ok(((target, key), read))
    }

    /// The element that has the focus of `marked`'s document, where that document says one other
    /// than `marked` does and `marked` is not inside it, else `marked` (#2972). A field in a shadow
    /// root (open or closed) is the one its keys go to while its document names the shadow's
    /// host: `marked` inside what the document names, its shadow roots included, stays.
    fn document_s_own(&self, target: &str, marked: &NodeKey) -> NodeKey {
        let session = lock(&self.inner.state)
            .page(target)
            .and_then(|page| page.frames.get(&marked.frame))
            .map(|frame| frame.session.clone());
        let Some(session) = session else {
            return marked.clone();
        };
        match self.focused_in(&session, &marked.frame) {
            Some(backend)
                if backend != marked.backend && !self.holds(&session, backend, marked.backend) =>
            {
                NodeKey {
                    frame: marked.frame.clone(),
                    backend,
                }
            }
            _ => marked.clone(),
        }
    }

    /// Whether the element `inside` is within the element `outer` of `session`'s document, its
    /// shadow roots included and frames' documents not; where the engine cannot say, it is taken
    /// to be, so the tree's own element stays.
    fn holds(&self, session: &str, outer: i64, inside: i64) -> bool {
        let Ok(described) = self.devtools().call(
            "DOM.describeNode",
            json!({ "backendNodeId": outer, "depth": -1, "pierce": true }),
            Some(session),
        ) else {
            return true;
        };
        let mut stack = vec![&described["node"]];
        while let Some(node) = stack.pop() {
            if node["backendNodeId"] == inside {
                return true;
            }
            for list in ["children", "shadowRoots"] {
                if let Some(nodes) = node[list].as_array() {
                    stack.extend(nodes);
                }
            }
        }
        false
    }

    /// Of `many`, the elements the tree marks focused on `target`'s page, the one its own
    /// document says has the focus, where exactly one is.
    fn the_one_focused(&self, target: &str, many: &[NodeKey]) -> Option<NodeKey> {
        let mut frames: Vec<&str> = many.iter().map(|key| key.frame.as_str()).collect();
        frames.sort_unstable();
        frames.dedup();
        let mut found = Vec::new();
        for frame in frames {
            let session = lock(&self.inner.state)
                .page(target)?
                .frames
                .get(frame)?
                .session
                .clone();
            if let Some(backend) = self.focused_in(&session, frame) {
                found.extend(
                    many.iter()
                        .filter(|key| key.frame == frame && key.backend == backend)
                        .cloned(),
                );
            }
        }
        // One element the tree lists twice is still one.
        found.sort_by(|a, b| (&a.frame, a.backend).cmp(&(&b.frame, b.backend)));
        found.dedup();
        match found.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        }
    }

    fn blank_node(&self, target: &str, key: NodeKey) -> Result<Snapshot, Refusal> {
        {
            let mut state = lock(&self.inner.state);
            state.excluded.insert(key);
            state.kept_on(target);
        }
        // Read the page now, so what the element holds is kept out from this moment.
        self.snapshot_of(target)
    }

    fn run(&self, call: Call) -> Result<Answer, Refusal> {
        match call {
            Call::Open { address } => self.open(&address),
            Call::Navigate { page, address } => {
                let target = self.target(page.as_ref())?;
                let session = self.page_session(&target)?;
                let _addressing = self.addressing(&target, Why::Addressed);
                self.acting_on(&target, || self.navigate(&session, &address))
            }
            Call::Snapshot {
                page,
                within,
                from,
                lines,
            } => {
                let target = self.target(page.as_ref())?;
                let lines = lines
                    .unwrap_or(self.inner.limits.snapshot_lines)
                    .clamp(1, self.inner.limits.most_snapshot_lines);
                let view = |state: &State| -> Result<View, Refusal> {
                    let scope = within
                        .as_ref()
                        .map(|r| live_ref(state, r).map(|(_, key)| key))
                        .transpose()?;
                    Ok(View {
                        within: scope,
                        from,
                        lines,
                    })
                };
                let shown = |snapshot: &Snapshot, state: &State| {
                    Ok(view(state)
                        .ok()
                        .and_then(|view| snapshot.keys_in(&view).ok())
                        .unwrap_or_default())
                };
                self.reading_shown(&target, shown, |snapshot, state| {
                    let view = view(state)?;
                    let mut refer = refer_in(state, &target);
                    snapshot.page(&view, &mut refer).map_err(|_| {
                        Refusal::NotShown(within.clone().unwrap_or_else(|| Ref::new("the scope")))
                    })
                })
            }
            Call::Find {
                page,
                text,
                role,
                within,
            } => {
                let target = self.target(page.as_ref())?;
                let query = Query { text, role };
                let scope = |state: &State| {
                    within
                        .as_ref()
                        .map(|r| live_ref(state, r).map(|(_, key)| key))
                        .transpose()
                };
                let shown = |snapshot: &Snapshot, state: &State| {
                    Ok(scope(state)
                        .ok()
                        .and_then(|scope| snapshot.keys_found(&query, scope.as_ref()).ok())
                        .unwrap_or_default())
                };
                self.reading_shown(&target, shown, |snapshot, state| {
                    let scope = scope(state)?;
                    let mut refer = refer_in(state, &target);
                    snapshot
                        .find(&query, scope.as_ref(), &mut refer)
                        .map_err(|_| {
                            Refusal::NotShown(
                                within.clone().unwrap_or_else(|| Ref::new("the scope")),
                            )
                        })
                })
            }
            Call::Changes { page } => {
                let target = self.target(page.as_ref())?;
                self.changes(&target)
            }
            Call::Click { target } => self.click(&target, 1),
            Call::DoubleClick { target } => self.click(&target, 2),
            Call::Hover { target } => {
                let at = self.locate(&target)?;
                let page = at.target.clone();
                self.acting_on(&page, || {
                    let (x, y) = self.point(&target, &at, true)?;
                    self.mouse(&at.page_session, "mouseMoved", x, y, 0, 0)?;
                    Ok(None)
                })
            }
            Call::Type { page, fields } => self.type_into(page.as_ref(), &fields),
            Call::Press { page, keys } => {
                let target = self.target(page.as_ref())?;
                if let Some(held) = self.typing_held(&target, &self.focus_addresses(&target)) {
                    return Ok(held);
                }
                let session = self.page_session(&target)?;
                let mut events = Vec::new();
                for chord in &keys {
                    events.extend(key_events(chord).map_err(|e| Refusal::NotAKey(e.to_string()))?);
                }
                // What the person copied in DevTools may be what a paste gets (#2028).
                if self.inner.paste_withheld.load(Ordering::SeqCst) && events.iter().any(pastes) {
                    return Err(Refusal::PasteWithheld);
                }
                self.acting_on(&target, || {
                    for event in events {
                        self.input("Input.dispatchKeyEvent", event, &session)?;
                    }
                    Ok(None)
                })
            }
            Call::Select { target, options } => self.select(&target, &options),
            Call::Drag { from, to } => self.drag(&from, &to),
            Call::Scroll { page, at, x, y } => self.scroll(page.as_ref(), at.as_ref(), x, y),
            Call::Back { page } => self.back(page.as_ref()),
            Call::WaitForText {
                page,
                text,
                gone,
                within,
            } => self.wait_for_text(page.as_ref(), &text, gone, within),
            Call::Pages => {
                let target = self.target(None).ok();
                self.read_titles();
                let pages = {
                    let state = lock(&self.inner.state);
                    state
                        .pages
                        .iter()
                        .filter_map(|p| state.info(&p.target))
                        .collect()
                };
                let mut answer = match &target {
                    Some(target) => self.answer_now(target, Content::Nothing),
                    None => Answer {
                        page: None,
                        settled: Settled::Yes,
                        opened: Vec::new(),
                        content: Content::Nothing,
                        note: None,
                        repeated: false,
                        asked: Vec::new(),
                        downloads: Vec::new(),
                    },
                };
                answer.content = Content::Pages(pages);
                Ok(answer)
            }
            Call::SwitchPage { page } => {
                let target = self.target(Some(&page))?;
                lock(&self.inner.state).current = Some(target.clone());
                let _ = self.gated(|| {
                    self.devtools().call(
                        "Target.activateTarget",
                        json!({ "targetId": target }),
                        None,
                    )
                })?;
                Ok(self.answer_now(&target, Content::Nothing))
            }
            Call::Screenshot { page } => self.screenshot(page.as_ref()),
            Call::Downloads => {
                let mut answer = match self.target(None) {
                    Ok(target) => self.answer_now(&target, Content::Nothing),
                    Err(_) => Answer {
                        page: None,
                        settled: Settled::Yes,
                        opened: Vec::new(),
                        content: Content::Nothing,
                        note: None,
                        repeated: false,
                        asked: Vec::new(),
                        downloads: Vec::new(),
                    },
                };
                answer.content = Content::Nothing;
                Ok(answer)
            }
            Call::Upload { target, files } => self.upload(&target, &files),
        }
    }

    /// Reads each page's title from its document, in purlis's own world (#2753). The engine
    /// tells of a title by `Target.targetInfoChanged`, and Chrome does not always send it after
    /// a script sets `document.title`. Every read together is given [`TITLES_WITHIN`], a world
    /// made for one included, the current page's first (#4353): a background page's first world
    /// can take most of that. A page not read by then, or whose answer is past
    /// [`MOST_TITLE_BYTES`], keeps the title it has. A read is kept only while the page still
    /// shows the document it was read from, whole: it is cut as it is listed
    /// ([`Page::shown_title`]), once it has been scrubbed.
    fn read_titles(&self) {
        let deadline = Instant::now() + TITLES_WITHIN;
        let pages: Vec<(String, String, u64)> = {
            let state = lock(&self.inner.state);
            let current = state.current.as_deref();
            let mut pages: Vec<_> = state
                .pages
                .iter()
                .filter_map(|p| Some((p.target.clone(), p.session.clone()?, p.shows)))
                .collect();
            pages.sort_by_key(|(target, ..)| Some(target.as_str()) != current);
            pages
        };
        for (target, session, shows) in pages {
            if Instant::now() >= deadline {
                break;
            }
            let read = self.in_world_by(&session, &target, deadline, |world| {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(CallError::TimedOut);
                }
                self.devtools().call_within_bounded(
                    "Runtime.evaluate",
                    json!({
                        "expression": TITLE,
                        "uniqueContextId": world.unique,
                        "returnByValue": true,
                    }),
                    Some(&session),
                    left,
                    MOST_TITLE_BYTES,
                )
            });
            if let Some(title) = read
                .ok()
                .and_then(|r| r.pointer("/result/value")?.as_str().map(str::to_owned))
                && let Some(page) = lock(&self.inner.state).page_mut(&target)
                && page.shows == shows
            {
                page.title = title;
            }
        }
    }

    fn devtools(&self) -> &dyn DevTools {
        &*self.inner.devtools
    }

    /// The target of `page`, or of the current page.
    fn target(&self, page: Option<&PageId>) -> Result<String, Refusal> {
        let state = lock(&self.inner.state);
        if state.ended {
            return Err(Refusal::Ended);
        }
        match page {
            Some(id) => state
                .pages
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.target.clone())
                .ok_or_else(|| Refusal::NoSuchPage(id.clone())),
            None => state.current.clone().ok_or(Refusal::NoPage),
        }
    }

    /// The session `target`'s page is driven on, once it is attached.
    fn page_session(&self, target: &str) -> Result<String, Refusal> {
        let deadline = Instant::now() + self.inner.limits.settle_within;
        let mut state = lock(&self.inner.state);
        loop {
            if state.ended {
                return Err(Refusal::Ended);
            }
            if self.cut() {
                return Err(Refusal::Interrupted);
            }
            match state.page(target) {
                None => return Err(Refusal::NoPage),
                Some(Page {
                    session: Some(session),
                    frames,
                    ..
                }) if frames.contains_key(target) => return Ok(session.clone()),
                Some(_) if Instant::now() >= deadline => return Err(Refusal::Unknown),
                Some(_) => {}
            }
            state = self
                .inner
                .changed
                .wait_timeout(state, POLL)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    fn open(&self, address: &WebAddress) -> Result<Answer, Refusal> {
        // The page starts empty and is sent to `address` once it is set up, so every event of
        // its load is seen.
        let created = self
            .gated(|| {
                self.devtools()
                    .call("Target.createTarget", json!({ "url": "about:blank" }), None)
            })?
            .map_err(refusal)?;
        let target = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| Refusal::Engine("no page was made".into()))?
            .to_owned();
        // The engine says it made the page before its event about the page has been read.
        let deadline = Instant::now() + self.inner.limits.settle_within;
        let mut state = lock(&self.inner.state);
        while state.page(&target).is_none()
            && !state.ended
            && !self.cut()
            && Instant::now() < deadline
        {
            state = self
                .inner
                .changed
                .wait_timeout(state, POLL)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        // The page was made: its event is late, so it is not missing (#4472).
        if state.page(&target).is_none() && !state.ended && !self.cut() {
            return Err(Refusal::PageUntold);
        }
        drop(state);
        let session = self.page_session(&target)?;
        lock(&self.inner.state).current = Some(target.clone());
        let _addressing = self.addressing(&target, Why::Addressed);
        self.acting_on(&target, || self.navigate(&session, address))
    }

    /// Sends the page on `session` to `address`; a load that failed is said in the note.
    fn navigate(&self, session: &str, address: &WebAddress) -> Result<Option<String>, Refusal> {
        let went = self
            .gated(|| {
                self.devtools().call(
                    "Page.navigate",
                    json!({ "url": address.as_str() }),
                    Some(session),
                )
            })?
            .map_err(refusal)?;
        Ok(went
            .get("errorText")
            .and_then(Value::as_str)
            .filter(|e| !e.is_empty())
            .map(|e| format!("the page did not load: {e}")))
    }

    fn targets(&self) -> HashSet<String> {
        lock(&self.inner.state)
            .pages
            .iter()
            .map(|p| p.target.clone())
            .collect()
    }

    /// Runs `act` on `target`'s page, then waits for it, and for any page it opened, to settle.
    fn acting_on(
        &self,
        target: &str,
        act: impl FnOnce() -> Result<Option<String>, Refusal>,
    ) -> Result<Answer, Refusal> {
        let before = self.targets();
        let dropped = self.devtools().dropped_events();
        let inputs = self.inputs.load(Ordering::SeqCst);
        let note = match act() {
            Ok(note) => note,
            // Once input has reached the page, a failure leaves it acted on in part.
            Err(why) => {
                let sent = self.inputs.load(Ordering::SeqCst) - inputs;
                return Err(match why {
                    Refusal::Partly { .. } => why,
                    why if sent > 0 => Refusal::Partly {
                        done: format!("{sent} input events reached the page"),
                        why: why.to_string(),
                    },
                    why => why,
                });
            }
        };
        let (settled, opened) = self.settle(
            target,
            &before,
            self.inner.limits.settle_within,
            dropped,
            true,
        );
        let shown = lock(&self.inner.state)
            .current
            .clone()
            .unwrap_or_else(|| target.to_owned());
        Ok(self.answer(&shown, settled, opened, Content::Nothing, note))
    }

    fn answer(
        &self,
        target: &str,
        settled: Settled,
        opened: Vec<PageInfo>,
        content: Content,
        note: Option<String>,
    ) -> Answer {
        let state = lock(&self.inner.state);
        let opened = opened
            .into_iter()
            .filter_map(|o| {
                state
                    .pages
                    .iter()
                    .find(|p| p.id == o.id)
                    .and_then(|p| state.info(&p.target))
            })
            .collect();
        Answer {
            page: state.info(target),
            settled,
            opened,
            content,
            note,
            repeated: false,
            asked: Vec::new(),
            downloads: Vec::new(),
        }
    }

    /// An answer that says whether `target` has settled now, without waiting.
    fn answer_now(&self, target: &str, content: Content) -> Answer {
        let dropped = self.devtools().dropped_events();
        let (settled, _) = self.settle(target, &self.targets(), Duration::ZERO, dropped, false);
        self.answer(target, settled, Vec::new(), content, None)
    }

    /// Waits up to `within` for `target` and every page it opened that was not in `before` to
    /// settle; the last page opened becomes the current one.
    fn settle(
        &self,
        target: &str,
        before: &HashSet<String>,
        within: Duration,
        dropped_before: u64,
        acted: bool,
    ) -> (Settled, Vec<PageInfo>) {
        let acted_at = Instant::now();
        let deadline = acted_at + within;
        let quiet = self.inner.limits.quiet;
        loop {
            let (mut waiting, sessions, opened) = {
                let mut state = lock(&self.inner.state);
                let opened: Vec<String> = state
                    .pages
                    .iter()
                    .filter(|p| !before.contains(&p.target))
                    .map(|p| p.target.clone())
                    .collect();
                if let Some(last) = opened.last() {
                    state.current = Some(last.clone());
                }
                let now = Instant::now();
                let mut waiting = Vec::new();
                let mut sessions = Vec::new();
                for page in std::iter::once(target)
                    .chain(opened.iter().map(String::as_str))
                    .filter_map(|t| state.page(t))
                {
                    let prefix = if page.target == target {
                        String::new()
                    } else {
                        format!("{}: ", page.id)
                    };
                    match &page.session {
                        None => waiting.push(format!("{prefix}the page is still opening")),
                        // A page the action opened is made empty and given its address once
                        // it has gone on: its empty document reads as loaded before then.
                        Some(_) if page.target != target && page.url.is_empty() => {
                            waiting.push(format!("{prefix}the page has no address yet"));
                        }
                        Some(session) => {
                            sessions.push((prefix.clone(), session.clone(), page.target.clone()));
                            waiting.extend(
                                page.settle
                                    .waiting_for(now, quiet)
                                    .into_iter()
                                    .map(|w| format!("{prefix}{w}")),
                            );
                        }
                    }
                }
                // What an action set going may not have reached the engine's events yet: the
                // page is quiet only a quiet while after the action, whatever came before.
                let since = now.saturating_duration_since(acted_at);
                if acted && waiting.is_empty() && since < quiet {
                    waiting.push(format!("the page changed {} ms ago", since.as_millis()));
                }
                let opened = opened.iter().filter_map(|t| state.info(t)).collect();
                if state.ended {
                    return (Settled::No(vec!["the browser has ended".into()]), opened);
                }
                // The person took the browser over: the call that waits is cut, and answers
                // so ([`Browser::call_at`]).
                if self.cut() {
                    return (Settled::No(Vec::new()), opened);
                }
                (waiting, sessions, opened)
            };
            if waiting.is_empty() {
                for (prefix, session, frame) in &sessions {
                    let ready = self.in_world(session, frame, |world| {
                        self.devtools().call_within(
                            "Runtime.evaluate",
                            json!({
                                "expression": READY_STATE,
                                "uniqueContextId": world.unique,
                                "returnByValue": true,
                            }),
                            Some(session),
                            READY_STATE_WITHIN,
                        )
                    });
                    let complete = ready
                        .ok()
                        .and_then(|r| r.pointer("/result/value").cloned())
                        .is_some_and(|v| v == "complete");
                    if !complete {
                        waiting.push(format!("{prefix}the page's document is still loading"));
                    }
                }
                if waiting.is_empty() {
                    if self.devtools().dropped_events() > dropped_before {
                        return (
                            Settled::No(vec![
                                "some of the engine's events were lost, so the page may still be changing"
                                    .into(),
                            ]),
                            opened,
                        );
                    }
                    return (Settled::Yes, opened);
                }
            }
            let now = Instant::now();
            if now >= deadline {
                return (Settled::No(waiting), opened);
            }
            let state = lock(&self.inner.state);
            let _ = self
                .inner
                .changed
                .wait_timeout(state, POLL.min(deadline - now))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Waits for `target` to settle, builds its snapshot and reads it with `read`, the elements
    /// `shown` names (those `read` shows) marked where the page lays them out of view (#2137).
    fn reading_shown(
        &self,
        target: &str,
        shown: impl FnOnce(&Snapshot, &State) -> Result<Vec<NodeKey>, Refusal>,
        read: impl FnOnce(&Snapshot, &mut State) -> Result<String, Refusal>,
    ) -> Result<Answer, Refusal> {
        let dropped = self.devtools().dropped_events();
        let (settled, opened) = self.settle(
            target,
            &self.targets(),
            self.inner.limits.settle_within,
            dropped,
            false,
        );
        let by = Instant::now() + self.inner.limits.page_reads_within;
        let (mut snapshot, began) = self.agent_snapshot(target, by, None)?;
        let keys = shown(&snapshot, &lock(&self.inner.state))?;
        snapshot.mark_out_of_view(self.out_of_view(target, &keys));
        let (text, document) = {
            let mut state = lock(&self.inner.state);
            let body = read(&snapshot, &mut state)?;
            let page = state.page_mut(target).ok_or(Refusal::NoPage)?;
            // The tree names the document's title as it is now, which the engine may not have
            // told of (#2753): the pages are listed under it from here on.
            if !snapshot.title().is_empty() {
                page.title = kept_title(snapshot.title());
            }
            let page = &*page;
            let title = if snapshot.title().is_empty() {
                page.shown_title()
            } else {
                snapshot.title().to_owned()
            };
            let text = format!(
                "page {} \"{}\" {}\n{body}",
                page.id,
                title.replace('"', "\\\""),
                page.shown_address(&page.url)
            );
            (text, changes::document_of(&state, target))
        };
        // What was read is what the next changes are told against (#2137).
        self.read_last.remember(target, document, snapshot.seen());
        Ok(self.answer(
            target,
            settled,
            opened,
            Content::Text(text),
            began.map(read_began),
        ))
    }

    /// `target`'s tree for an agent's snapshot, find or wait, read by `by` from a read begun after
    /// `since` where it is given (#3277), and how long before now the read began where an earlier
    /// call began it.
    fn agent_snapshot(
        &self,
        target: &str,
        by: Instant,
        since: Option<Instant>,
    ) -> Result<(Snapshot, Option<Duration>), Refusal> {
        let (trees, began) = self.agent_trees(target, by, since)?;
        Ok((self.snapshot_from(target, &trees), began))
    }

    /// Every frame's tree of `target`'s page for an agent's call, read by `by` (#3277). The read
    /// runs on past the call, each frame's within
    /// [`Limits::tree_reads_within`](crate::drive::Limits): a call it outlives answers
    /// [`Refusal::PageReadTooLong`], since a read acts on nothing, never [`Refusal::Unknown`], and
    /// the next call that reads the page waits on the same read rather than asking the engine for
    /// another behind it. That read is used only under the lease and while the page shows the
    /// document it began under, no call that acts came between, within
    /// [`Limits::page_read_kept_for`](crate::drive::Limits) after it came or, still under way,
    /// within that and [`Limits::page_reads_within`](crate::drive::Limits) after it began, and,
    /// with `since`, only where it began after then; once used, or failed, the next call reads
    /// anew. A read no call will use that is still under way is let end before another is asked
    /// for. A read that does not finish within its own limit answers
    /// [`Refusal::PageReadPastItsLimit`]. Answers how long before now the read began where an
    /// earlier call began it.
    fn agent_trees(
        &self,
        target: &str,
        by: Instant,
        since: Option<Instant>,
    ) -> Result<(Vec<FrameAx>, Option<Duration>), Refusal> {
        let epoch = self.epoch();
        let shows = lock(&self.inner.state)
            .page(target)
            .map_or(0, |page| page.shows);
        let kept_for = self.inner.limits.page_read_kept_for;
        // A read is used within `kept_for` after it came, or, still under way, within as long as
        // a call waits for one and `kept_for` after it began; and, for a wait, only one that
        // began after the wait was made.
        let under_way_for = self.inner.limits.page_reads_within + kept_for;
        let usable = |reading: &reading::Reading| {
            let fresh = match reading.came() {
                Some(came) => came.elapsed() <= kept_for,
                None => reading.began.elapsed() <= under_way_for,
            };
            fresh && since.is_none_or(|since| reading.began >= since)
        };
        let (reading, begun) = loop {
            let next = lock(&self.reading).next(target, epoch, shows, usable);
            match next {
                reading::Next::Join(reading) => break (reading, false),
                reading::Next::Begin(reading) => {
                    reading.begin(
                        Arc::clone(&self.inner.devtools),
                        self.frames_to_read(target),
                        self.inner.limits.tree_reads_within,
                        self.inner.limits.most_tree_read_bytes,
                        Refusal::PageReadPastItsLimit,
                    );
                    break (reading, true);
                }
                // The engine still reads the page for a read no call will use: another is
                // asked for once it has ended, never beside it.
                reading::Next::After(running) => {
                    if running.wait(by, || self.cut()).is_none() {
                        return Err(Refusal::PageReadTooLong);
                    }
                }
            }
        };
        let Some((_, answer)) = reading.wait(by, || self.cut()) else {
            // The read goes on, for the next call.
            return Err(Refusal::PageReadTooLong);
        };
        // Used, or failed: the next call reads anew.
        lock(&self.reading).used(&reading);
        let trees = answer?;
        Ok((
            trees.as_ref().clone(),
            (!begun).then(|| reading.began.elapsed()),
        ))
    }

    /// `target`'s tree as it is now, every frame in it.
    fn snapshot_of(&self, target: &str) -> Result<Snapshot, Refusal> {
        let trees = self.trees_of(target)?;
        Ok(self.snapshot_from(target, &trees))
    }

    /// `target`'s page built from `trees` as they were read, with what is blanked and kept out
    /// now (#2974): built again from the same read, nothing is asked of the engine.
    fn snapshot_from(&self, target: &str, trees: &[FrameAx]) -> Snapshot {
        let (excluded, scrub) = {
            let state = lock(&self.inner.state);
            match state.page(target) {
                Some(page) => (state.excluded.clone(), page.scrubbed.clone()),
                None => (state.excluded.clone(), Kept::default()),
            }
        };
        let snapshot = build_kept(trees, &excluded, &scrub);
        if let Some(page) = lock(&self.inner.state).page_mut(target) {
            page.keep_out(snapshot.blanked_text());
        }
        snapshot
    }

    /// Every frame's tree of `target`'s page, as the engine has it now: the one read of the
    /// page a snapshot is built from.
    fn trees_of(&self, target: &str) -> Result<Vec<FrameAx>, Refusal> {
        let frames = self.frames_to_read(target)?;
        read_trees(
            self.devtools(),
            frames,
            CALL_TIMEOUT,
            self.inner.limits.most_tree_read_bytes,
        )
        .map_err(refusal)
    }

    /// Every frame's tree of `target`'s page, every call of the read made by `by` (#3277), or
    /// why not, in words for the log.
    pub(super) fn trees_by(&self, target: &str, by: Instant) -> Result<Vec<FrameAx>, String> {
        let frames = self.frames_to_read(target).map_err(|why| why.to_string())?;
        read_trees_by(
            self.devtools(),
            frames,
            CALL_TIMEOUT,
            self.inner.limits.most_tree_read_bytes,
            Some(by),
        )
        .map_err(|why| match why {
            CallError::TimedOut => "it was not read within its limit".to_owned(),
            other => refusal(other).to_string(),
        })
    }

    /// The frames of `target`'s page to read the trees of, the page's own first and each frame
    /// after the frame around it.
    fn frames_to_read(&self, target: &str) -> Result<Vec<ToRead>, Refusal> {
        let session = self.page_session(target)?;
        let (frames, _excluded, _scrub) = {
            let state = lock(&self.inner.state);
            let page = state.page(target).ok_or(Refusal::NoPage)?;
            let mut ordered: Vec<(String, &Frame)> = Vec::new();
            if let Some(main) = page.frames.get(target) {
                ordered.push((target.to_owned(), main));
            }
            let mut at = 0;
            while at < ordered.len() {
                let parent = ordered[at].0.clone();
                let mut children: Vec<(String, &Frame)> = page
                    .frames
                    .iter()
                    .filter(|(_, f)| f.parent.as_deref() == Some(parent.as_str()))
                    .map(|(id, f)| (id.clone(), f))
                    .collect();
                children.sort_by_key(|(_, f)| f.generation);
                ordered.extend(children);
                at += 1;
            }
            let frames: Vec<ToRead> = ordered
                .iter()
                .map(|(id, f)| ToRead {
                    session: f.session.clone(),
                    parent_session: f
                        .parent
                        .as_ref()
                        .and_then(|p| page.frames.get(p))
                        .map(|p| p.session.clone()),
                    frame: FrameAx {
                        id: id.clone(),
                        name: f.name.clone(),
                        url: f.url.clone(),
                        parent: f.parent.clone(),
                        owner: None,
                        nodes: Value::Null,
                    },
                })
                .collect();
            (frames, state.excluded.clone(), page.scrubbed.clone())
        };
        let frames = if frames.is_empty() {
            vec![ToRead {
                session: session.clone(),
                parent_session: None,
                frame: FrameAx {
                    id: target.to_owned(),
                    name: String::new(),
                    url: String::new(),
                    parent: None,
                    owner: None,
                    nodes: Value::Null,
                },
            }]
        } else {
            frames
        };
        Ok(frames)
    }

    /// Where `target` is, for acting on it.
    fn locate(&self, target: &Ref) -> Result<Located, Refusal> {
        let state = lock(&self.inner.state);
        if state.ended {
            return Err(Refusal::Ended);
        }
        let (page_target, key) = live_ref(&state, target)?;
        let page = state.page(&page_target).ok_or_else(|| stale(target))?;
        let page_session = page.session.clone().ok_or(Refusal::Unknown)?;
        let frame = page.frames.get(&key.frame).ok_or_else(|| stale(target))?;
        let boundaries = page.boundaries(&key.frame);
        Ok(Located {
            target: page_target,
            page_session,
            node_session: frame.session.clone(),
            frame: key.frame.clone(),
            backend: key.backend,
            boundaries,
        })
    }

    /// Where frames with sessions of their own put `at`'s frame on the page, in CSS pixels;
    /// `None` when the engine does not say where a frame between them is drawn.
    fn frame_place(&self, at: &Located) -> Result<Option<FramePlace>, CallError> {
        let mut place = FramePlace {
            corner: (0.0, 0.0),
            within: None,
            steps: Vec::new(),
            owners: Vec::new(),
        };
        for (frame, parent_session) in &at.boundaries {
            let owner = self.devtools().call(
                "DOM.getFrameOwner",
                json!({ "frameId": frame }),
                Some(parent_session),
            )?;
            let Some(owner) = owner.get("backendNodeId").and_then(Value::as_i64) else {
                return Ok(None);
            };
            let model = self.devtools().call(
                "DOM.getBoxModel",
                json!({ "backendNodeId": owner }),
                Some(parent_session),
            )?;
            let quad = |name: &str| -> Option<[f64; 8]> {
                let numbers: Vec<f64> = model
                    .pointer(&format!("/model/{name}"))?
                    .as_array()?
                    .iter()
                    .map(Value::as_f64)
                    .collect::<Option<_>>()?;
                numbers.try_into().ok()
            };
            let size = |name: &str| model.pointer(&format!("/model/{name}"))?.as_f64();
            let (Some(content), Some(border), Some(width), Some(height)) = (
                quad("content"),
                quad("border"),
                size("width"),
                size("height"),
            ) else {
                return Ok(None);
            };
            // A frame drawn other than upright at its own size (turned, scaled, zoomed, by its
            // element or anything around it) is painted over whole: what its document draws is
            // drawn within its box. An act aimed inside it is carried through the map from the
            // frame element's laid-out box to where it is drawn (#2557).
            let as_laid_out = upright(&border, width, height);
            if place.within.is_none() && !as_laid_out {
                place.within = Some(place.on_page(quad_bounds(&border)));
            }
            place.owners.push(owner);
            place.steps.push(if as_laid_out {
                Step::Moved {
                    at: (border[0], border[1]),
                    inside: (content[0] - border[0], content[1] - border[1]),
                    shows: (content[2] - content[0], content[5] - content[1]),
                }
            } else {
                Drawing::of(&border, width, height)
                    .and_then(|drawing| {
                        let inside = drawing.in_box((content[0], content[1]))?;
                        let far = drawing.in_box((content[4], content[5]))?;
                        Some(Step::Drawn {
                            drawing,
                            inside,
                            shows: (far.0 - inside.0, far.1 - inside.1),
                        })
                    })
                    .unwrap_or(Step::Flat)
            });
            let inner = place.on_page(Rect {
                x: content[0],
                y: content[1],
                width: 0.0,
                height: 0.0,
            });
            place.corner = (inner.x, inner.y);
        }
        Ok(Some(place))
    }

    /// The page point at the centre of `at`, brought into view first when `scroll`: the
    /// element in its own document, then, outermost first, each frame between around the point
    /// where the element is in it. Where the frames are drawn is read again after those scrolls.
    /// Where the element is not shown through every frame on the first read, each frame element
    /// is scrolled into view, outermost first, and read again before it is answered "not shown"
    /// (#2690). The point is only ever one the page's own window shows.
    fn point(&self, target: &Ref, at: &Located, scroll: bool) -> Result<(f64, f64), Refusal> {
        let point = self.aimed(target, at, scroll)?;
        if self.in_window(&at.page_session, point) {
            Ok(point)
        } else {
            Err(Refusal::NotShown(target.clone()))
        }
    }

    /// Whether `point`, in CSS pixels of the page on `session`, lies inside what the page's own
    /// window shows of it, at least half a pixel in from every edge: the viewport events are
    /// dispatched in. Unknown is not inside.
    fn in_window(&self, session: &str, (x, y): (f64, f64)) -> bool {
        self.viewport_of(session, true)
            .is_some_and(|window| shown(x, y, (window.width, window.height)))
    }

    /// [`Browser::point`], before it is held to the page's window.
    fn aimed(&self, target: &Ref, at: &Located, scroll: bool) -> Result<(f64, f64), Refusal> {
        if scroll {
            let _ = self.gated(|| {
                self.devtools().call(
                    "DOM.scrollIntoViewIfNeeded",
                    json!({ "backendNodeId": at.backend }),
                    Some(&at.node_session),
                )
            })?;
        }
        let (mut place, mut centre) = self.placed_centre(target, at)?;
        if !scroll || at.boundaries.is_empty() {
            return place
                .drawn_at(centre)
                .ok_or_else(|| Refusal::NotShown(target.clone()));
        }
        if place.through(centre).is_none() {
            // A frame below the fold of the frame around it hides the element until it is
            // scrolled to: each frame element is brought into view, outermost first, and the
            // frames read again.
            for (owner, (_, parent_session)) in place.owners.iter().zip(&at.boundaries) {
                let _ = self.gated(|| {
                    self.devtools().call(
                        "DOM.scrollIntoViewIfNeeded",
                        json!({ "backendNodeId": owner }),
                        Some(parent_session),
                    )
                })?;
            }
            (place, centre) = self.placed_centre(target, at)?;
        }
        let (_, boxes) = place
            .through(centre)
            .ok_or_else(|| Refusal::NotShown(target.clone()))?;
        for ((owner, (_, parent_session)), (x, y)) in
            place.owners.iter().zip(&at.boundaries).zip(boxes)
        {
            let gone_on = self.gated(|| {
                self.devtools().call(
                    "DOM.scrollIntoViewIfNeeded",
                    json!({
                        "backendNodeId": owner,
                        "rect": { "x": x - 1.0, "y": y - 1.0, "width": 2.0, "height": 2.0 },
                    }),
                    Some(parent_session),
                )
            });
            if gone_on.is_err() {
                return Err(refusal(CallError::TimedOut));
            }
        }
        // The browser routes a press by where each document is drawn, which follows a scroll by
        // a frame or more: each document between the page and the element draws the scroll
        // before it is aimed at.
        self.drawn_twice(at);
        // A scroll into view may still be carried to the documents around the element after it
        // answers: aim only once two reads apart say the same.
        let mut last = self.placed_centre(target, at)?;
        let mut at_rest = false;
        for _ in 0..AT_REST_READS {
            std::thread::sleep(AT_REST_GAP);
            let now = self.placed_centre(target, at)?;
            if now == last {
                at_rest = true;
                break;
            }
            last = now;
        }
        let (place, centre) = last;
        at_rest
            .then(|| place.drawn_at(centre))
            .flatten()
            .ok_or_else(|| Refusal::NotShown(target.clone()))
    }

    /// Waits, in purlis's own world in each document from the page to `at`'s, for that document
    /// to draw two frames, each for at most [`DRAWN_WITHIN`]. A document that draws nothing in
    /// that time is not waited for longer: where it is drawn is still read and contained.
    fn drawn_twice(&self, at: &Located) {
        let inner_sessions = at
            .boundaries
            .iter()
            .skip(1)
            .map(|(_, parent)| parent.as_str())
            .chain(std::iter::once(at.node_session.as_str()));
        let documents = std::iter::once((at.page_session.as_str(), at.target.as_str()))
            .chain(inner_sessions.zip(at.boundaries.iter().map(|(frame, _)| frame.as_str())));
        for (session, frame) in documents {
            let _ = self.in_world(session, frame, |world| {
                self.devtools().call_within(
                    "Runtime.evaluate",
                    json!({
                        "expression": DRAWN_TWICE,
                        "uniqueContextId": world.unique,
                        "awaitPromise": true,
                        "returnByValue": true,
                    }),
                    Some(session),
                    DRAWN_WITHIN,
                )
            });
        }
    }

    /// Where the frames between the page and `at` are drawn, and `at`'s centre in its own
    /// document, as the engine says them now.
    fn placed_centre(
        &self,
        target: &Ref,
        at: &Located,
    ) -> Result<(FramePlace, (f64, f64)), Refusal> {
        let shown = |why: CallError| match why {
            CallError::Engine { .. } => Refusal::NotShown(target.clone()),
            other => refusal(other),
        };
        let place = self
            .frame_place(at)
            .map_err(shown)?
            .ok_or_else(|| Refusal::NotShown(target.clone()))?;
        let quads = self
            .devtools()
            .call(
                "DOM.getContentQuads",
                json!({ "backendNodeId": at.backend }),
                Some(&at.node_session),
            )
            .map_err(shown)?;
        let centre = centre(&quads).ok_or_else(|| Refusal::NotShown(target.clone()))?;
        Ok((place, centre))
    }

    fn mouse(
        &self,
        session: &str,
        kind: &str,
        x: f64,
        y: f64,
        clicks: i64,
        buttons: i64,
    ) -> Result<(), Refusal> {
        let mut event = json!({
            "type": kind, "x": x, "y": y, "modifiers": 0, "buttons": buttons,
        });
        if kind != "mouseMoved" || buttons != 0 {
            event["button"] = "left".into();
        }
        if clicks > 0 {
            event["clickCount"] = clicks.into();
        }
        self.input("Input.dispatchMouseEvent", event, session)
    }

    /// Sends one input event (or text) to the page on `session`, counted for an answer that says
    /// how far an action got.
    fn input(&self, method: &str, event: Value, session: &str) -> Result<(), Refusal> {
        self.gated(|| {
            self.inputs.fetch_add(1, Ordering::SeqCst);
            self.devtools().call(method, event, Some(session))
        })?
        .map(|_| ())
        .map_err(refusal)
    }

    fn click(&self, target: &Ref, clicks: i64) -> Result<Answer, Refusal> {
        let at = self.locate(target)?;
        let page = at.target.clone();
        self.acting_on(&page, || {
            let (x, y) = self.point(target, &at, true)?;
            let session = &at.page_session;
            self.mouse(session, "mouseMoved", x, y, 0, 0)?;
            for count in 1..=clicks {
                self.mouse(session, "mousePressed", x, y, count, 1)?;
                self.mouse(session, "mouseReleased", x, y, count, 0)?;
            }
            Ok(None)
        })
    }

    fn type_into(&self, page: Option<&PageId>, fields: &[Field]) -> Result<Answer, Refusal> {
        let target = match fields.iter().find_map(|f| f.into.as_ref()) {
            Some(first) if page.is_none() => self.locate(first)?.target,
            _ => self.target(page)?,
        };
        let mut into = Vec::new();
        for field in fields {
            match &field.into {
                Some(written) => into.extend(self.addresses_of(written)?),
                None => into.extend(self.focus_addresses(&target)),
            }
        }
        if let Some(held) = self.typing_held(&target, &into) {
            return Ok(held);
        }
        let session = self.page_session(&target)?;
        let mut located = Vec::new();
        for field in fields {
            located.push(match &field.into {
                Some(r) => Some((r.clone(), self.locate(r)?)),
                None => None,
            });
        }
        let mut answer = self.acting_on(&target, || {
            let total = fields.len();
            for (done, (field, at)) in fields.iter().zip(&located).enumerate() {
                let typed = self.type_field(field, at.as_ref(), &session);
                match typed {
                    Ok(()) => {}
                    Err(why) if done > 0 => {
                        return Err(Refusal::Partly {
                            done: format!(
                                "typed into {done} of {total} fields, and maybe part of the next"
                            ),
                            why: why.to_string(),
                        });
                    }
                    Err(why) => return Err(why),
                }
            }
            Ok(None)
        })?;
        // A blanked field holds what was just typed into it: once the page has settled it is
        // read again, so that is kept out of this answer and every later one.
        if self.blanks_on(&target) {
            let _ = self.snapshot_of(&target);
            let state = lock(&self.inner.state);
            answer.page = answer.page.and_then(|page| {
                let at = state.pages.iter().find(|p| p.id == page.id)?;
                state.info(&at.target)
            });
        }
        Ok(answer)
    }

    /// Whether any element of `target`'s page is blanked.
    fn blanks_on(&self, target: &str) -> bool {
        let state = lock(&self.inner.state);
        state.page(target).is_some_and(|page| {
            state
                .excluded
                .iter()
                .any(|key| page.frames.contains_key(&key.frame))
        })
    }

    fn type_field(
        &self,
        field: &Field,
        at: Option<&(Ref, Located)>,
        session: &str,
    ) -> Result<(), Refusal> {
        if let Some((r, at)) = at {
            self.gated(|| {
                self.devtools().call(
                    "DOM.focus",
                    json!({ "backendNodeId": at.backend }),
                    Some(&at.node_session),
                )
            })?
            .map_err(|why| match why {
                CallError::Engine { .. } => Refusal::NotShown(r.clone()),
                other => refusal(other),
            })?;
        }
        if field.replace {
            let mut keys = key_events("Control+a").expect("a known chord");
            if field.text.is_empty() {
                keys.extend(key_events("Delete").expect("a known key"));
            }
            for event in keys {
                self.input("Input.dispatchKeyEvent", event, session)?;
            }
        }
        if !field.text.is_empty() {
            self.input("Input.insertText", json!({ "text": field.text }), session)?;
        }
        Ok(())
    }

    fn select(&self, target: &Ref, options: &[String]) -> Result<Answer, Refusal> {
        let at = self.locate(target)?;
        let page = at.target.clone();
        if let Some(held) = self.typing_held(&page, &self.addresses_of(target)?) {
            return Ok(held);
        }
        self.acting_on(&page, || {
            // The drop-down is resolved into purlis's world, and the function run there by the
            // world's unique id: an element of any other world is refused by the engine, which
            // runs nothing, and the world is made again once.
            let cut = std::cell::RefCell::new(None);
            let chose = self.in_world(&at.node_session, &at.frame, |world| {
                let resolved = self.devtools().call(
                    "DOM.resolveNode",
                    json!({ "backendNodeId": at.backend, "executionContextId": world.id }),
                    Some(&at.node_session),
                )?;
                let Some(object) = resolved.pointer("/object/objectId").and_then(Value::as_str)
                else {
                    return Ok(Value::Null);
                };
                let chose = self.gated(|| {
                    self.devtools().call(
                        "Runtime.callFunctionOn",
                        json!({
                            "uniqueContextId": world.unique,
                            "functionDeclaration": SELECT_OPTIONS,
                            "arguments": [{ "objectId": object }, { "value": options }],
                            "returnByValue": true,
                        }),
                        Some(&at.node_session),
                    )
                });
                let _ = self.devtools().call(
                    "Runtime.releaseObject",
                    json!({ "objectId": object }),
                    Some(&at.node_session),
                );
                match chose {
                    Ok(chose) => chose,
                    Err(why) => {
                        cut.replace(Some(why));
                        Ok(Value::Null)
                    }
                }
            });
            if let Some(why) = cut.into_inner() {
                return Err(why);
            }
            let chose = chose.map_err(|why| match why {
                CallError::Engine { .. } => Refusal::NotShown(target.clone()),
                other => refusal(other),
            })?;
            if chose.is_null() {
                return Err(Refusal::NotShown(target.clone()));
            }
            let said = chose.pointer("/result/value").cloned().unwrap_or_default();
            if said.get("notSelect").is_some() {
                return Err(Refusal::NotADropDown(target.clone()));
            }
            if said.get("notMultiple").is_some() {
                return Err(Refusal::OneOption(target.clone()));
            }
            if let Some(missing) = said.get("missing").and_then(Value::as_array) {
                let words = |list: Option<&Vec<Value>>| {
                    list.into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                return Err(Refusal::NoSuchOption {
                    target: target.clone(),
                    missing: words(Some(missing)),
                    options: words(said.get("options").and_then(Value::as_array)),
                });
            }
            Ok(None)
        })
    }

    /// **Puts `files` into the file input `target`** (#2020): refused, putting nothing, for an
    /// element that is not one. The files go to the page's site as anything typed there does,
    /// so a page on a site new to the project waits on the person first.
    fn upload(&self, target: &Ref, files: &[std::path::PathBuf]) -> Result<Answer, Refusal> {
        let at = self.locate(target)?;
        let page = at.target.clone();
        if let Some(held) = self.typing_held(&page, &self.addresses_of(target)?) {
            return Ok(held);
        }
        let described = self
            .devtools()
            .call(
                "DOM.describeNode",
                json!({ "backendNodeId": at.backend }),
                Some(&at.node_session),
            )
            .map_err(|why| match why {
                CallError::Engine { .. } => Refusal::NotShown(target.clone()),
                other => refusal(other),
            })?;
        if !is_file_input(&described["node"]) {
            return Err(Refusal::NotAFileInput(target.clone()));
        }
        let files: Vec<String> = files
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect();
        self.acting_on(&page, || {
            self.gated(|| {
                self.devtools().call(
                    "DOM.setFileInputFiles",
                    json!({ "files": files, "backendNodeId": at.backend }),
                    Some(&at.node_session),
                )
            })?
            .map_err(|why| match why {
                CallError::Engine { .. } => Refusal::NotAFileInput(target.clone()),
                other => refusal(other),
            })?;
            Ok(None)
        })
    }

    fn drag(&self, from: &Ref, to: &DragTo) -> Result<Answer, Refusal> {
        let start_at = self.locate(from)?;
        let page = start_at.target.clone();
        let end_at = match to {
            DragTo::Element(r) => Some((r.clone(), self.locate(r)?)),
            DragTo::Offset { .. } => None,
        };
        self.acting_on(&page, || {
            let (start, end) = match (to, &end_at) {
                (DragTo::Element(_), Some((r, at))) => {
                    let end = self.point(r, at, true)?;
                    (self.point(from, &start_at, false)?, end)
                }
                (DragTo::Offset { x, y }, _) => {
                    let start = self.point(from, &start_at, true)?;
                    (start, (start.0 + x, start.1 + y))
                }
                (DragTo::Element(r), None) => return Err(Refusal::NoSuchRef(r.clone())),
            };
            let session = &start_at.page_session;
            // Where what a native drag carries may go when it drops (#2018): into whichever
            // document is drawn at the drop, which may be any of the page's, whatever element
            // the drag aims at.
            let drops_into = lock(&self.inner.state).every_document(&page);
            // A native drag source starts the engine's own drag, which the engine hands over
            // while this drag lasts, and never after (#2136): once asked to, it is told to stop
            // however the drag ends.
            {
                let mut state = lock(&self.inner.state);
                state.dragged = None;
                state.cancels_late = None;
            }
            let mut taken = false;
            let dragged = self
                .devtools()
                .call(
                    "Input.setInterceptDrags",
                    json!({ "enabled": true }),
                    Some(session),
                )
                .map_err(refusal)
                .and_then(|_| self.dragging(&page, session, start, end, &drops_into, &mut taken));
            let handed_back = self.devtools().call(
                "Input.setInterceptDrags",
                json!({ "enabled": false }),
                Some(session),
            );
            // A drag the engine told of only after this one stopped waiting is cancelled, now
            // or as the word comes, however late (#2839).
            let late = {
                let mut state = lock(&self.inner.state);
                let told = state.dragged.take();
                match told {
                    Some((on, data)) if !taken && on == *session => Some(data),
                    _ if !taken => {
                        state.cancels_late = Some((session.clone(), end));
                        None
                    }
                    _ => None,
                }
            };
            if let Some(data) = late {
                let _ = self.devtools().call(
                    "Input.dispatchDragEvent",
                    drag_cancel(end, &data),
                    Some(session),
                );
            }
            // Said where the drag's own failure is what its call answers.
            if let (Err(_), Err(why)) = (&dragged, &handed_back) {
                tracing::warn!(
                    "purlis: a browser was not told to stop handing its drags over after a drag \
                     that failed: {why}"
                );
            }
            let note = dragged?;
            handed_back.map_err(refusal)?;
            Ok(note)
        })
    }

    /// Presses at `start`, moves to `end` with the button held and releases there; once the
    /// engine's own drag starts on the way, the rest of the way is its drag events, the drop at
    /// `end` into the documents at `drops_into`. A drag that fails on the way, or is cut by the
    /// person taking over, is cancelled and its button let go.
    fn dragging(
        &self,
        target: &str,
        session: &str,
        start: (f64, f64),
        end: (f64, f64),
        drops_into: &[String],
        taken: &mut bool,
    ) -> Result<Option<String>, Refusal> {
        self.mouse(session, "mouseMoved", start.0, start.1, 0, 0)?;
        self.mouse(session, "mousePressed", start.0, start.1, 1, 1)?;
        let mut carried = None;
        let dragged = self.dragged_on(session, start, end, drops_into, target, &mut carried);
        *taken = carried.is_some();
        if dragged.is_err() || matches!(dragged, Ok(Some(_))) {
            self.let_go(session, end, carried.as_ref());
        }
        dragged
    }

    /// [`Browser::dragging`] once the button is pressed: `carried` is what the engine's own
    /// drag carries, once it has started. A note where the drop was held back.
    fn dragged_on(
        &self,
        session: &str,
        start: (f64, f64),
        end: (f64, f64),
        drops_into: &[String],
        target: &str,
        carried: &mut Option<Value>,
    ) -> Result<Option<String>, Refusal> {
        let at = |step: usize| {
            let part = step as f64 / DRAG_STEPS as f64;
            (
                start.0 + (end.0 - start.0) * part,
                start.1 + (end.1 - start.1) * part,
            )
        };
        let mut step = 1;
        while step <= DRAG_STEPS {
            let (x, y) = at(step);
            self.mouse(session, "mouseMoved", x, y, 0, 1)?;
            step += 1;
            *carried = self.engine_drag(session, Duration::ZERO);
            if carried.is_some() {
                break;
            }
        }
        // The engine says its drag started a moment after the move that started it.
        if carried.is_none() {
            *carried = self.engine_drag(session, self.inner.limits.drag_starts_within);
        }
        if let Some(data) = carried.as_mut() {
            // A drag of an element carries what the page put in it, never files.
            if let Some(data) = data.as_object_mut() {
                data.remove("files");
            }
            if self.carries_kept_text(target, data) {
                return Err(Refusal::DragKeptOut);
            }
            if let Some(site) = self.sends_to_a_new_site(drops_into) {
                return Ok(Some(format!(
                    "nothing was dropped: the drop would have gone to {site}, a site new to this \
                     project, and purlis asked the person whether this browser may send to it"
                )));
            }
            let data = data.clone();
            let mut kind = "dragEnter";
            for step in step..=DRAG_STEPS {
                let (x, y) = at(step);
                self.drag_event(session, kind, x, y, &data)?;
                kind = "dragOver";
            }
            self.drag_event(session, kind, end.0, end.1, &data)?;
            self.drag_event(session, "drop", end.0, end.1, &data)?;
        }
        self.mouse(session, "mouseReleased", end.0, end.1, 1, 0)?;
        Ok(None)
    }

    /// Cancels the engine's own drag where one carries `carried`, and lets the button go at
    /// `end`: whoever drives next starts from nothing held.
    fn let_go(&self, session: &str, end: (f64, f64), carried: Option<&Value>) {
        if let Some(data) = carried {
            let _ = self.devtools().call(
                "Input.dispatchDragEvent",
                drag_cancel(end, data),
                Some(session),
            );
        }
        let _ = self.devtools().call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased", "x": end.0, "y": end.1, "modifiers": 0, "buttons": 0,
                "button": "left", "clickCount": 1,
            }),
            Some(session),
        );
    }

    /// Whether what a drag on `target`'s page carries holds a text kept out of the agent's
    /// answers.
    fn carries_kept_text(&self, target: &str, data: &Value) -> bool {
        let state = lock(&self.inner.state);
        let mut kept = state.kept.clone();
        if let Some(page) = state.page(target) {
            kept.take_in(&page.scrubbed);
        }
        let scrub = Scrub::of(&kept);
        data["items"].as_array().into_iter().flatten().any(|item| {
            ["data", "title", "baseURL"]
                .iter()
                .filter_map(|field| item[*field].as_str())
                .any(|text| scrub.holds(text))
        })
    }

    /// What the engine's own drag on `session` carries, once it has started, waiting up to
    /// `within` for it.
    fn engine_drag(&self, session: &str, within: Duration) -> Option<Value> {
        let deadline = Instant::now() + within;
        let mut state = lock(&self.inner.state);
        loop {
            if let Some((on, data)) = &state.dragged
                && on == session
            {
                return Some(data.clone());
            }
            let now = Instant::now();
            if now >= deadline || state.ended {
                return None;
            }
            state = self
                .inner
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// One of the engine's own drag events at (`x`, `y`), carrying `data`.
    fn drag_event(
        &self,
        session: &str,
        kind: &str,
        x: f64,
        y: f64,
        data: &Value,
    ) -> Result<(), Refusal> {
        self.input(
            "Input.dispatchDragEvent",
            json!({ "type": kind, "x": x, "y": y, "modifiers": 0, "data": data }),
            session,
        )
    }

    fn scroll(
        &self,
        page: Option<&PageId>,
        at: Option<&Ref>,
        x: f64,
        y: f64,
    ) -> Result<Answer, Refusal> {
        let located = at.map(|r| self.locate(r)).transpose()?;
        let target = match &located {
            Some(l) => l.target.clone(),
            None => self.target(page)?,
        };
        let session = self.page_session(&target)?;
        self.acting_on(&target, || {
            let point = match (at, &located) {
                (Some(r), Some(l)) => self.point(r, l, true)?,
                _ => {
                    let metrics = self
                        .devtools()
                        .call("Page.getLayoutMetrics", json!({}), Some(&session))
                        .map_err(refusal)?;
                    let half = |f: &str| {
                        metrics
                            .pointer(&format!("/cssLayoutViewport/{f}"))
                            .and_then(Value::as_f64)
                            .unwrap_or_default()
                            / 2.0
                    };
                    (half("clientWidth"), half("clientHeight"))
                }
            };
            if x != 0.0 || y != 0.0 {
                self.input(
                    "Input.dispatchMouseEvent",
                    json!({
                        "type": "mouseWheel", "x": point.0, "y": point.1,
                        "deltaX": x, "deltaY": y, "modifiers": 0,
                    }),
                    &session,
                )?;
            }
            Ok(None)
        })
    }

    fn back(&self, page: Option<&PageId>) -> Result<Answer, Refusal> {
        let target = self.target(page)?;
        let session = self.page_session(&target)?;
        let history = self
            .devtools()
            .call("Page.getNavigationHistory", json!({}), Some(&session))
            .map_err(refusal)?;
        let current = history
            .get("currentIndex")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        let earlier = current
            .checked_sub(1)
            .and_then(|i| history.get("entries")?.get(i))
            .ok_or(Refusal::NoEarlierPage)?;
        let url = earlier.get("url").and_then(Value::as_str).unwrap_or("");
        WebAddress::parse(url).map_err(|_| Refusal::EarlierNotWeb)?;
        let entry = earlier.get("id").cloned().unwrap_or_default();
        // A page gone back to was read before: its documents read again (#2018).
        let _revisiting = self.addressing(&target, Why::Reads);
        self.acting_on(&target, || {
            self.gated(|| {
                self.devtools().call(
                    "Page.navigateToHistoryEntry",
                    json!({ "entryId": entry }),
                    Some(&session),
                )
            })?
            .map_err(refusal)?;
            Ok(None)
        })
    }

    fn wait_for_text(
        &self,
        page: Option<&PageId>,
        text: &str,
        gone: bool,
        within: Duration,
    ) -> Result<Answer, Refusal> {
        let target = self.target(page)?;
        let start = Instant::now();
        let deadline = start + within.min(self.inner.limits.longest_wait);
        // Each read is waited for until the wait's end, and for a page slow to read at least
        // as long as any call waits for its page (#3277).
        let by = deadline.max(start + self.inner.limits.page_reads_within);
        let mut read_once = false;
        let seen = loop {
            // Only a read that began once the wait was made says whether the text showed.
            let shows = match self.agent_snapshot(&target, by, Some(start)) {
                Ok((snapshot, _)) => snapshot.shows_text(text),
                // A read that does not come in time after one that did: the text did not show
                // in what was read.
                Err(Refusal::PageReadTooLong | Refusal::PageReadPastItsLimit) if read_once => {
                    break false;
                }
                Err(why) => return Err(why),
            };
            read_once = true;
            if shows != gone {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            self.pause(READ_AGAIN.min(deadline.saturating_duration_since(Instant::now())))?;
        };
        let dropped = self.devtools().dropped_events();
        let (settled, opened) = self.settle(
            &target,
            &self.targets(),
            self.inner.limits.quiet,
            dropped,
            false,
        );
        Ok(self.answer(&target, settled, opened, Content::Seen(seen), None))
    }

    fn screenshot(&self, page: Option<&PageId>) -> Result<Answer, Refusal> {
        let target = self.target(page)?;
        let session = self.page_session(&target)?;
        let dropped = self.devtools().dropped_events();
        let (settled, opened) = self.settle(
            &target,
            &self.targets(),
            self.inner.limits.settle_within,
            dropped,
            false,
        );
        // Where each blanked element is, and each element that shows a text kept out, before and
        // after the picture: each is painted over the whole way from where it was to where it is,
        // so one that moves while the picture is taken is covered wherever the picture caught it.
        let mut placings = Placings::default();
        let before = self.blanked_boxes(&target, &mut placings)?;
        let shot = self
            .devtools()
            .call(
                "Page.captureScreenshot",
                json!({ "format": "png" }),
                Some(&session),
            )
            .map_err(refusal)?;
        let mut placings = Placings::default();
        let mut boxes = self.blanked_boxes(&target, &mut placings)?;
        for (item, was) in before {
            let now = boxes.entry(item).or_insert(was);
            *now = hull(now, &was);
        }
        let boxes: Vec<Rect> = boxes.into_values().collect();
        let png = shot
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
            .ok_or_else(|| Refusal::Engine("the screenshot came back empty".into()))?;
        let rects: Vec<Rect> = if boxes.is_empty() {
            Vec::new()
        } else {
            let metrics = self
                .devtools()
                .call("Page.getLayoutMetrics", json!({}), Some(&session))
                .map_err(|why| Refusal::Withheld(why.to_string()))?;
            let place = Placing::from_metrics(&metrics).ok_or_else(|| {
                Refusal::Withheld("the page's scale on the picture is not known".into())
            })?;
            boxes.iter().map(|r| place.on_picture(r)).collect()
        };
        let painted = paint_over(&png, &rects).map_err(|why| Refusal::Withheld(why.to_string()))?;
        Ok(self.answer(&target, settled, opened, Content::Image(painted), None))
    }

    /// The box of every blanked element on `target`'s page, of every element that shows a text
    /// kept out (#2014), and of everything else the page draws that may show one (#2303), in CSS
    /// pixels, each by its session and node. An element the engine says it cannot place is not
    /// drawn; any other failure withholds the screenshot.
    fn blanked_boxes(&self, target: &str, placings: &mut Placings) -> Result<Boxes, Refusal> {
        let mut keys: Vec<NodeKey> = {
            let state = lock(&self.inner.state);
            let page = state.page(target).ok_or(Refusal::NoPage)?;
            state
                .excluded
                .iter()
                .filter(|k| page.frames.contains_key(&k.frame))
                .cloned()
                .collect()
        };
        let echoes = self
            .snapshot_of(target)
            .map_err(|why| Refusal::Withheld(why.to_string()))?;
        keys.extend(echoes.echoes().iter().cloned());
        let mut boxes = Boxes::new();
        for key in keys {
            let (session, boundaries) = {
                let state = lock(&self.inner.state);
                let page = state.page(target).ok_or(Refusal::NoPage)?;
                let Some(frame) = page.frames.get(&key.frame) else {
                    continue;
                };
                (frame.session.clone(), page.boundaries(&key.frame))
            };
            if let Some(rect) = self.placed(placings, &session, &boundaries, key.backend)? {
                boxes.insert((session, key.backend), rect);
            }
        }
        self.drawn_boxes(target, placings, &mut boxes)?;
        Ok(boxes)
    }

    /// Where the node `backend` of `session` is drawn, in CSS pixels, its session sitting where
    /// `boundaries` put it; `None` when the engine says it is not drawn. Where each session sits
    /// is asked once in `placings`.
    fn placed(
        &self,
        placings: &mut Placings,
        session: &str,
        boundaries: &[(String, String)],
        backend: i64,
    ) -> Result<Option<Rect>, Refusal> {
        let place = match placings.get(boundaries) {
            Some(place) => place.clone(),
            None => {
                let at = Located {
                    target: String::new(),
                    page_session: String::new(),
                    node_session: session.to_owned(),
                    frame: String::new(),
                    backend,
                    boundaries: boundaries.to_vec(),
                };
                let place = match self.frame_place(&at) {
                    Ok(Some(place)) => Some(place),
                    Ok(None) => {
                        return Err(Refusal::Withheld(
                            "where a frame of the page is drawn is not known".into(),
                        ));
                    }
                    Err(why) => undrawn(why)?,
                };
                placings.insert(boundaries.to_vec(), place.clone());
                place
            }
        };
        let Some(place) = place else {
            return Ok(None);
        };
        match self.devtools().call(
            "DOM.getContentQuads",
            json!({ "backendNodeId": backend }),
            Some(session),
        ) {
            Ok(quads) => {
                Ok(bounds(&quads).map(|rect| place.within.unwrap_or_else(|| place.on_page(rect))))
            }
            Err(why) => undrawn(why),
        }
    }

    /// Adds to `boxes` the box of everything `target`'s page draws that may show a text it keeps
    /// out (#2303), and on a page whose site a text was kept out on, of everything it draws that
    /// cannot be read: nothing while neither holds. Each of the page's sessions is read for
    /// what its documents lay out; one that cannot be read withholds the screenshot.
    fn drawn_boxes(
        &self,
        target: &str,
        placings: &mut Placings,
        boxes: &mut Boxes,
    ) -> Result<(), Refusal> {
        let (kept, greys, roots, blanked) = {
            let state = lock(&self.inner.state);
            let page = state.page(target).ok_or(Refusal::NoPage)?;
            let greys = state.greys(target);
            if page.scrubbed.is_empty() && !state.kept_anything() {
                return Ok(());
            }
            let page_session = page.session.clone().ok_or(Refusal::Unknown)?;
            let mut roots: Vec<Root> = page
                .frames
                .iter()
                .filter_map(|(id, frame)| {
                    let parent = frame.parent.as_ref().and_then(|p| page.frames.get(p));
                    match parent {
                        Some(parent) if parent.session == frame.session => None,
                        _ => Some(Root {
                            frame: id.clone(),
                            session: frame.session.clone(),
                            parent: parent.map(|p| p.session.clone()),
                            boundaries: page.boundaries(id),
                        }),
                    }
                })
                .collect();
            if !roots.iter().any(|root| root.session == page_session) {
                roots.push(Root {
                    frame: target.to_owned(),
                    session: page_session,
                    parent: None,
                    boundaries: Vec::new(),
                });
            }
            // The blanked elements each session draws, by the engine's ids.
            let mut blanked: HashMap<String, HashSet<i64>> = HashMap::new();
            for key in &state.excluded {
                if let Some(frame) = page.frames.get(&key.frame) {
                    blanked
                        .entry(frame.session.clone())
                        .or_default()
                        .insert(key.backend);
                }
            }
            (page.scrubbed.clone(), greys, roots, blanked)
        };
        let loose: Vec<&str> = kept.loose();
        let kept: Vec<&str> = kept.texts();
        // The frame elements whose documents are read in sessions of their own.
        let read_apart: HashSet<(String, i64)> = roots
            .iter()
            .filter_map(|root| {
                let parent = root.parent.as_ref()?;
                let owner = self
                    .devtools()
                    .call(
                        "DOM.getFrameOwner",
                        json!({ "frameId": root.frame }),
                        Some(parent),
                    )
                    .ok()?;
                let owner = owner.get("backendNodeId").and_then(Value::as_i64)?;
                Some((parent.clone(), owner))
            })
            .collect();
        let mut reads = Vec::new();
        for root in &roots {
            let answer = self
                .devtools()
                .call(
                    "DOMSnapshot.captureSnapshot",
                    json!({ "computedStyles": STYLES }),
                    Some(&root.session),
                )
                .map_err(|why| Refusal::Withheld(why.to_string()))?;
            let none = HashSet::new();
            let found = drawn(
                &answer,
                &kept,
                &loose,
                blanked.get(&root.session).unwrap_or(&none),
            )
            .map_err(|why| Refusal::Withheld(why.to_string()))?;
            reads.push((root, found));
        }
        // What the page shows is the top document read, whatever address the page was last
        // told: a page that has not yet left a kept site's document still paints it over.
        let opaque = greys || {
            let state = lock(&self.inner.state);
            let shown = reads
                .iter()
                .find(|(root, _)| root.frame == target)
                .and_then(|(_, found)| found.address.clone())
                .unwrap_or_default();
            state.greys_at(&shown)
        };
        for (root, found) in reads {
            let session = &root.session;
            let frames = found.frames.into_iter().filter(|chain| {
                chain
                    .first()
                    .is_none_or(|own| !read_apart.contains(&(session.clone(), *own)))
            });
            let pictured = found.pictured.into_iter().filter(|_| opaque);
            for chain in found.paint.into_iter().chain(pictured).chain(frames) {
                let Some(item) = chain.first().copied() else {
                    continue;
                };
                for backend in chain {
                    if let Some(rect) = self.placed(placings, session, &root.boundaries, backend)? {
                        boxes.insert((session.clone(), item), rect);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

/// What is painted over in a screenshot, in CSS pixels, by the session and node it is.
type Boxes = HashMap<(String, i64), Rect>;

/// Where each session sits on its page, by the frames between them, once asked: `None` where the
/// engine says it is not drawn.
type Placings = HashMap<Vec<(String, String)>, Option<FramePlace>>;

/// Where a frame's document is drawn on its page, in CSS pixels.
#[derive(Debug, Clone, PartialEq)]
struct FramePlace {
    /// Where the document's top left corner is drawn, frame by frame moved by each frame's
    /// corner: exact while every frame between is upright at its own size, and what `within`
    /// is placed by otherwise.
    corner: (f64, f64),
    /// The box of the outermost frame between the page and the document that is drawn other
    /// than upright at its own size: everything the document draws is drawn within it.
    within: Option<Rect>,
    /// How each frame between them draws the next document, the page's own frame first.
    steps: Vec<Step>,
    /// The element of each of those frames, by its backend node id in the document around it.
    owners: Vec<i64>,
}

/// Where a point of a frame's document is drawn on the page, and where it is in each frame
/// element's own box on the way there, the page's own frame first.
type Through = ((f64, f64), Vec<(f64, f64)>);

/// How a frame draws its document in the document around it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// Upright at its own size: the frame element's box with its top left corner at `at`, the
    /// document's top left corner at `inside` of that box, showing `shows` (width and height) of
    /// the document.
    Moved {
        at: (f64, f64),
        inside: (f64, f64),
        shows: (f64, f64),
    },
    /// Turned, scaled, zoomed or otherwise: the frame element's box drawn as `drawing`, the
    /// document's top left corner at `inside` of that box, showing `shows` of the document.
    Drawn {
        drawing: Drawing,
        inside: (f64, f64),
        shows: (f64, f64),
    },
    /// Drawn with no area, so no point of the document is drawn anywhere to aim at.
    Flat,
}

impl FramePlace {
    /// Where `point`, in CSS pixels of the frame's document, is drawn on the page; `None` when
    /// it is drawn nowhere, or outside what a frame between shows of its document: an act lands
    /// only inside the frame that shows its element (#2557).
    fn drawn_at(&self, point: (f64, f64)) -> Option<(f64, f64)> {
        self.through(point).map(|(on_page, _)| on_page)
    }

    /// Where `point` is drawn on the page, and where it is in each frame element's own box on
    /// the way, the page's own frame first; `None` as for [`FramePlace::drawn_at`].
    fn through(&self, point: (f64, f64)) -> Option<Through> {
        let mut boxes = Vec::with_capacity(self.steps.len());
        let mut at = point;
        for step in self.steps.iter().rev() {
            let (x, y) = at;
            at = match *step {
                Step::Moved {
                    at: corner,
                    inside,
                    shows,
                } => {
                    if !shown(x, y, shows) {
                        return None;
                    }
                    let in_box = (x + inside.0, y + inside.1);
                    boxes.push(in_box);
                    (in_box.0 + corner.0, in_box.1 + corner.1)
                }
                Step::Drawn {
                    drawing,
                    inside,
                    shows,
                } => {
                    if !shown(x, y, shows) {
                        return None;
                    }
                    let in_box = (x + inside.0, y + inside.1);
                    boxes.push(in_box);
                    drawing.on_page(in_box)?
                }
                Step::Flat => return None,
            };
        }
        boxes.reverse();
        Some((at, boxes))
    }

    /// `rect`, a box in CSS pixels of the frame's document, on the page.
    fn on_page(&self, rect: Rect) -> Rect {
        Rect {
            x: rect.x + self.corner.0,
            y: rect.y + self.corner.1,
            ..rect
        }
    }
}

/// Whether (`x`, `y`) of a document lies inside the `shows` (width and height) of it its frame
/// shows, at least half a pixel in from every edge.
fn shown(x: f64, y: f64, shows: (f64, f64)) -> bool {
    const EDGE: f64 = 0.5;
    (EDGE..=shows.0 - EDGE).contains(&x) && (EDGE..=shows.1 - EDGE).contains(&y)
}

/// Whether `border`, an element's border box as drawn, is upright and the size the element is
/// laid out at (`width` by `height`), to within a pixel: drawn as laid out, not turned, scaled
/// or zoomed.
fn upright(border: &[f64; 8], width: f64, height: f64) -> bool {
    let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
    let [x0, y0, x1, y1, x2, y2, x3, y3] = *border;
    near(y0, y1)
        && near(x1, x2)
        && near(y2, y3)
        && near(x3, x0)
        && near(x1 - x0, width)
        && near(y3 - y0, height)
}

/// `None` when `why` says a node is not drawn; any other failure withholds the screenshot.
fn undrawn<T>(why: CallError) -> Result<Option<T>, Refusal> {
    match why {
        CallError::Engine { message, .. } if not_drawn(&message) => Ok(None),
        why => Err(Refusal::Withheld(why.to_string())),
    }
}

/// The box around `a` and `b`.
fn hull(a: &Rect, b: &Rect) -> Rect {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    Rect {
        x: left,
        y: top,
        width: (a.x + a.width).max(b.x + b.width) - left,
        height: (a.y + a.height).max(b.y + b.height) - top,
    }
}

/// **A browser dropped unstopped ends its engine** (#2745): its event reader holds what the
/// engine lives in until the engine's pipe closes, so nothing else would.
impl Drop for Browser {
    fn drop(&mut self) {
        self.stop(Duration::ZERO);
    }
}

impl Inner {
    /// **Its DevTools connection ended** (#2745): unless purlis is stopping the engine, which
    /// tells of it itself, the engine is given a moment to end by itself where its pipe closed,
    /// and ended at once where the connection closed on a message, and whoever waits is told how.
    fn connection_ended(&self) {
        self.stop_inspecting(inspect::InspectEnded::Ended);
        let closed = self.devtools.closed();
        let pipe_ended = matches!(
            closed,
            None | Some(Closed::PipeEnded | Closed::ReadFailed(_))
        );
        let Some((exit, by_itself)) = self.engine_ended(pipe_ended) else {
            return;
        };
        self.ended_with(Ending {
            by: if by_itself {
                EndedBy::Itself
            } else {
                EndedBy::LostConnection
            },
            exit,
            closed,
        });
    }

    /// The engine's exit, and whether it ended by itself, once it has ended: `None` where a
    /// stop has it. One whose pipe closed has a moment to end by itself; one whose connection
    /// closed on a message is ended at once. With no engine held (a stand-in's), it ended by
    /// itself where its pipe did.
    fn engine_ended(&self, pipe_ended: bool) -> Option<(Option<EngineExit>, bool)> {
        let engine = {
            let mut engine = lock(&self.engine);
            if self.stopping.load(Ordering::SeqCst) {
                return None;
            }
            engine.take()
        };
        Some(match engine {
            Some(engine) => engine.wait_or_end(if pipe_ended {
                ENDS_BY_ITSELF_WITHIN
            } else {
                Duration::ZERO
            }),
            None => (None, pipe_ended),
        })
    }

    /// Holds `engine` as the browser's.
    fn hold(&self, engine: Box<dyn EngineHold>) {
        *lock(&self.engine) = Some(engine);
        self.held_one.store(true, Ordering::SeqCst);
    }

    /// Tells whoever waits how the engine ended, once: the first word of it stands. Marked told
    /// once they have returned.
    fn ended_with(&self, ending: Ending) {
        let tell = {
            let mut ends = lock(&self.ends);
            match std::mem::replace(&mut *ends, Ends::Ended(ending.clone())) {
                Ends::Waiting(tell) => tell,
                Ends::Ended(first) => {
                    *ends = Ends::Ended(first);
                    return;
                }
            }
        };
        if let Some(tell) = tell {
            tell(ending);
        }
        *lock(&self.told) = true;
        self.told_now.notify_all();
    }

    /// Waits up to `most` until whoever waits on the engine's end has been told, where the
    /// browser ever held an engine.
    fn until_told(&self, most: Duration) {
        if !self.held_one.load(Ordering::SeqCst) {
            return;
        }
        let deadline = Instant::now() + most;
        let mut told = lock(&self.told);
        while !*told {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            told = self
                .told_now
                .wait_timeout(told, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}

impl Inner {
    /// Whether the engine has ended.
    pub(crate) fn ended(&self) -> bool {
        lock(&self.state).ended
    }

    /// The DevTools session of the current page, once it is attached.
    pub(crate) fn current_session(&self) -> Option<String> {
        let state = lock(&self.state);
        let current = state.current.as_deref()?;
        state.page(current)?.session.clone()
    }

    /// Why the open Inspect ends, where `event` says it does: its front end's target went, or
    /// the page it inspects did.
    fn inspection_gone(&self, event: &Event) -> Option<inspect::InspectEnded> {
        let open = self.inspection()?;
        let target = event.params["targetId"].as_str();
        match event.method.as_str() {
            "Target.targetDestroyed" | "Target.targetCrashed"
                if target == Some(open.front_end.as_str()) =>
            {
                Some(inspect::InspectEnded::Ended)
            }
            // The page's own end is the driver's to hear too: the Inspect ends beside it.
            "Target.targetDestroyed" | "Target.targetCrashed"
                if target == Some(open.inspected.as_str()) =>
            {
                open.ends(inspect::InspectEnded::PageGone);
                None
            }
            _ => None,
        }
    }

    /// Takes in one event from the engine.
    fn take_in(&self, event: &Event) {
        // The front end of the person's DevTools is DevTools' own, never one of the browser's
        // pages (#2028): its session is tapped, so nothing of it is the driver's.
        if let Some(session) = event.session_id.as_deref()
            && self.inspection_on(session).is_some()
        {
            return;
        }
        if let Some(why) = self.inspection_gone(event)
            && let Some(open) = self.inspection()
        {
            open.ends(why);
            return;
        }
        if event.method == "Page.screencastFrame" {
            let shown = event.session_id.as_deref().and_then(|session| {
                let state = lock(&self.state);
                let target = state.page_of_session(session)?;
                Some((state.aim(&target)?, state.document_address(&target)?))
            });
            if let Some((aim, address)) = shown {
                self.cast
                    .frame(&event.params, event.session_id.as_deref(), aim, address);
            }
            return;
        }
        let now = Instant::now();
        let mut follow = Vec::new();
        {
            let mut state = lock(&self.state);
            let params = &event.params;
            let text = |path: &str| {
                params
                    .pointer(path)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            };
            match event.method.as_str() {
                "Browser.downloadWillBegin" => {
                    let frame = text("/frameId").unwrap_or_default();
                    let page_address = state
                        .pages
                        .iter()
                        .find_map(|page| page.frames.get(&frame))
                        .map(|frame| frame.url.clone());
                    if let Some(guid) = text("/guid")
                        && let Some(cancel) = state.downloads.began(
                            &guid,
                            &text("/url").unwrap_or_default(),
                            &text("/suggestedFilename").unwrap_or_default(),
                            page_address.as_deref(),
                            &self.limits,
                        )
                    {
                        follow.push(FollowUp::DiscardDownload(cancel));
                    }
                }
                "Browser.downloadProgress" => {
                    let bytes = |key: &str| {
                        params
                            .get(key)
                            .and_then(Value::as_f64)
                            .filter(|n| n.is_finite() && *n >= 0.0)
                            .map_or(0, |n| n as u64)
                    };
                    if let Some(guid) = text("/guid")
                        && let Some(cancel) = state.downloads.moved(
                            &guid,
                            bytes("receivedBytes"),
                            bytes("totalBytes"),
                            &text("/state").unwrap_or_default(),
                            &self.limits,
                        )
                    {
                        follow.push(FollowUp::DiscardDownload(cancel));
                    }
                }
                "Target.targetCreated" | "Target.targetInfoChanged"
                    if state.inspecting.owns(&params["targetInfo"]) =>
                {
                    let target = text("/targetInfo/targetId").unwrap_or_default();
                    let front = text("/targetInfo/browserContextId")
                        .and_then(|context| state.inspecting.front_ends.get(&context).cloned());
                    match front {
                        // A page DevTools' context opened that is not its front end goes at once.
                        Some(front) if front != target => follow.push(FollowUp::Close(target)),
                        // The front end stays at its own page: one that leaves it is closed.
                        Some(_) => {
                            let url = text("/targetInfo/url").unwrap_or_default();
                            if !inspect::front_end_may_show(&url)
                                && let Some(open) = lock(&self.inspection)
                                    .clone()
                                    .filter(|open| open.front_end == target)
                            {
                                open.ends(inspect::InspectEnded::Moved);
                            }
                        }
                        None => {}
                    }
                }
                "Target.attachedToTarget" if state.inspecting.owns(&params["targetInfo"]) => {}
                // The session made for DevTools to speak to the page on: theirs, and the mark
                // goes as it is heard.
                "Target.attachedToTarget"
                    if text("/targetInfo/targetId")
                        .is_some_and(|target| state.inspecting.bridging.remove(&target)) => {}
                "Target.attachedToTarget"
                    if text("/targetInfo/targetId")
                        .is_some_and(|target| state.inspecting.orphans.remove(&target)) =>
                {
                    if let Some(session) = text("/sessionId") {
                        follow.push(FollowUp::Release(session));
                    }
                }
                "Target.targetCreated" | "Target.targetInfoChanged" => {
                    let info = &params["targetInfo"];
                    if info.get("type").and_then(Value::as_str) == Some("page")
                        && let Some(target) = text("/targetInfo/targetId")
                    {
                        let url = text("/targetInfo/url").unwrap_or_default();
                        let created = state.page(&target).is_none();
                        if created {
                            state.pages_listed += 1;
                            let opener = text("/targetInfo/openerId");
                            if let Some(opener) = &opener
                                && let Some(by) = state.page_mut(opener)
                            {
                                by.settle.observe("", Activity::Changed, now);
                            }
                            let id = PageId(format!("p{}", state.pages_listed));
                            let kept = state.kept.clone();
                            state.pages.push(Page {
                                id,
                                target: target.clone(),
                                session: None,
                                url: url.clone(),
                                title: String::new(),
                                settle: Settle::new(now),
                                scrubbed: kept,
                                shows: 0,
                                gone: 0,
                                frames: HashMap::new(),
                                opener,
                            });
                        }
                        if let Some(page) = state.page_mut(&target) {
                            page.url.clone_from(&url);
                            page.title = text("/targetInfo/title").unwrap_or_default();
                        }
                        if !may_stay_open(&url) {
                            state.pages.retain(|p| p.target != target);
                            if state.current.as_deref() == Some(target.as_str()) {
                                state.current = state.pages.last().map(|p| p.target.clone());
                            }
                            follow.push(FollowUp::Close(target));
                        } else if created {
                            follow.push(FollowUp::Attach(target));
                        }
                    }
                }
                "Target.targetDestroyed" => {
                    if let Some(target) = text("/targetId") {
                        state.pages.retain(|p| p.target != target);
                        state.sessions.retain(|_, s| match s {
                            Session::Page(t) => t != &target,
                            Session::Frame { page, .. } => page != &target,
                        });
                        if state.current.as_deref() == Some(target.as_str()) {
                            state.current = state.pages.last().map(|p| p.target.clone());
                        }
                    }
                }
                "Target.attachedToTarget" => {
                    let session = text("/sessionId");
                    let target = text("/targetInfo/targetId");
                    let kind = text("/targetInfo/type").unwrap_or_default();
                    if let (Some(session), Some(target)) = (session, target) {
                        let parent_page = event
                            .session_id
                            .as_deref()
                            .and_then(|s| state.page_of_session(s));
                        match (kind.as_str(), parent_page) {
                            ("page", _) if state.page(&target).is_some() => {
                                if let Some(page) = state.page_mut(&target) {
                                    page.session = Some(session.clone());
                                }
                                state
                                    .sessions
                                    .insert(session.clone(), Session::Page(target.clone()));
                                if state.current.is_none() {
                                    state.current = Some(target);
                                }
                                follow.push(FollowUp::SetUp(session));
                            }
                            ("iframe", Some(page)) => {
                                state.sessions.insert(
                                    session.clone(),
                                    Session::Frame {
                                        page: page.clone(),
                                        frame: target.clone(),
                                    },
                                );
                                // A frame purlis knew moved to a process of its own: it shows
                                // another document from now on (#2973).
                                state.frame_swapped(&page, &target, &session);
                                follow.push(FollowUp::SetUp(session));
                            }
                            _ => follow.push(FollowUp::Release(session)),
                        }
                    }
                }
                "Target.detachedFromTarget" => {
                    if let Some(session) = text("/sessionId") {
                        self.worlds.cleared(&session);
                        if let Some(target) = state.page_of_session(&session)
                            && let Some(page) = state.page_mut(&target)
                        {
                            page.settle.forget_session(&session, now);
                        }
                        // A drag ended on a session that is gone is answered by nobody (#2839).
                        if state
                            .cancels_late
                            .as_ref()
                            .is_some_and(|(on, _)| *on == session)
                        {
                            state.cancels_late = None;
                        }
                        state.sessions.remove(&session);
                    }
                }
                "Page.frameAttached" => {
                    if let Some(session) = &event.session_id {
                        let frame = json!({
                            "id": params.get("frameId"),
                            "parentId": params.get("parentFrameId"),
                        });
                        let known = state
                            .page_of_session(session)
                            .and_then(|t| state.page(&t))
                            .zip(frame["id"].as_str())
                            .is_some_and(|(p, id)| p.frames.contains_key(id));
                        if !known {
                            state.frame_seen(session, &frame, false);
                        }
                    }
                }
                "Page.frameNavigated" => {
                    if let Some(session) = &event.session_id {
                        if let Some(frame) = text("/frame/id") {
                            self.worlds.left(&frame);
                        }
                        // A page left unread at the hand-back that loads another document is
                        // read anew: what DevTools wrote into the old one went with it (#2028).
                        if params.pointer("/frame/parentId").is_none()
                            && let Some(target) = state.page_of_session(session)
                        {
                            lock(&self.devtools_unread).remove(&target);
                        }
                        state.frame_seen(session, &params["frame"], true);
                    }
                }
                // purlis's own worlds, as the engine tells of them (#2138).
                "Runtime.executionContextCreated" => {
                    if let Some(session) = &event.session_id {
                        self.worlds.told(session, &params["context"]);
                    }
                }
                "Runtime.executionContextDestroyed" => {
                    if let Some(session) = &event.session_id {
                        self.worlds.ended(session, params);
                    }
                }
                "Runtime.executionContextsCleared" => {
                    if let Some(session) = &event.session_id {
                        self.worlds.cleared(session);
                    }
                }
                // A form submitted (#2018): its request, when it comes, sends what was typed.
                "Page.frameRequestedNavigation" => {
                    if let (Some(reason), Some(url)) = (text("/reason"), text("/url")) {
                        let marks = if reason.starts_with("formSubmission") {
                            &mut state.forms
                        } else {
                            &mut state.offered
                        };
                        if marks.len() >= FORMS_KEPT {
                            marks.pop_front();
                        }
                        marks.push_back(without_fragment(&url).to_owned());
                    }
                }
                // The front end loads its own files alone: what it asks of the web fails (#2028).
                "Fetch.requestPaused"
                    if text("/frameId").is_some_and(|frame| {
                        state
                            .inspecting
                            .front_ends
                            .values()
                            .any(|one| *one == frame)
                    }) =>
                {
                    if let Some(request) = text("/requestId") {
                        follow.push(FollowUp::Refuse(request));
                    }
                }
                "Fetch.requestPaused" => {
                    if let Some(request) = text("/requestId") {
                        let address = text("/request/url").unwrap_or_default();
                        let method = text("/request/method").unwrap_or_default();
                        let frame = text("/frameId");
                        let redirected_from = text("/redirectedRequestId");
                        let document = text("/resourceType").as_deref() == Some("Document");
                        let why = state.why(&Held {
                            request: &request,
                            address: &address,
                            method: &method,
                            document,
                            frame: frame.as_deref(),
                            redirected_from: redirected_from.as_deref(),
                        });
                        let by = if self.person_drives.load(Ordering::SeqCst) {
                            Driver::Person
                        } else if self.purlis_drives.load(Ordering::SeqCst) {
                            Driver::Purlis
                        } else {
                            Driver::Agent
                        };
                        follow.push(FollowUp::Judge {
                            paused: Paused {
                                id: request,
                                document,
                            },
                            outbound: Outbound {
                                address,
                                method,
                                why,
                                by,
                            },
                        });
                    }
                }
                "Page.frameDetached" => {
                    if let Some(frame) = text("/frameId") {
                        self.worlds.left(&frame);
                    }
                    let swapped = text("/reason").as_deref() == Some("swap");
                    if !swapped
                        && let (Some(session), Some(frame)) = (&event.session_id, text("/frameId"))
                        && let Some(target) = state.page_of_session(session)
                    {
                        let gone = state.frame_gone(&target, &frame);
                        // What they had loading or open went with them (#2841).
                        if let Some(page) = state.page_mut(&target) {
                            page.settle.frames_gone(session, &gone, now);
                        }
                    }
                }
                // A drag of the agent's started the engine's own (#2136): it is carried on by
                // drag events from here.
                "Input.dragIntercepted" => {
                    if let Some(session) = &event.session_id {
                        match state.cancels_late.take() {
                            // Told of after the drag that started it ended (#2839).
                            Some((on, at)) if on == *session => follow.push(FollowUp::CancelDrag {
                                session: on,
                                at,
                                data: params["data"].clone(),
                            }),
                            other => {
                                state.cancels_late = other;
                                state.dragged = Some((session.clone(), params["data"].clone()));
                            }
                        }
                    }
                }
                _ => {}
            }
            if let (Some(session), Some(seen)) =
                (&event.session_id, activity(&event.method, &event.params))
                && let Some(target) = state.page_of_session(session)
                && let Some(page) = state.page_mut(&target)
            {
                page.settle.observe(session, seen, now);
            }
        }
        self.changed.notify_all();
        for step in follow {
            let step = match step {
                // Handed to the judge's own thread at once.
                judge @ FollowUp::Judge { .. } => judge,
                other => match &*lock(&self.following) {
                    Some(following) => match following.send(other) {
                        Ok(()) => continue,
                        Err(unsent) => unsent.0,
                    },
                    None => other,
                },
            };
            self.follow_up(step);
        }
    }

    /// Judges held request `paused` (#2018), and lets it on or holds it back as the judge says:
    /// a document held back is answered with no content, so its frame stays on the page it
    /// showed; anything else is failed.
    fn judged(&self, paused: &Paused, outbound: &Outbound) {
        let request = paused.id.as_str();
        // Requests are held only where there is a judge, so this always has one.
        let gate = self
            .judge
            .as_ref()
            .map_or(Gate::Go, |judge| judge(outbound));
        let said = match gate {
            Gate::Go => self.devtools.call_within(
                "Fetch.continueRequest",
                json!({ "requestId": request }),
                None,
                JUDGED_WITHIN,
            ),
            Gate::Ask { site } => {
                lock(&self.state).ask(&site);
                if paused.document {
                    self.devtools.call_within(
                        "Fetch.fulfillRequest",
                        json!({ "requestId": request, "responseCode": 204 }),
                        None,
                        JUDGED_WITHIN,
                    )
                } else {
                    self.devtools.call_within(
                        "Fetch.failRequest",
                        json!({ "requestId": request, "errorReason": "BlockedByClient" }),
                        None,
                        JUDGED_WITHIN,
                    )
                }
            }
        };
        // A request whose page has gone went with it: there is nothing more to say.
        let _ = said;
    }

    fn follow_up(&self, step: FollowUp) {
        let devtools = &*self.devtools;
        match step {
            FollowUp::Attach(target) => {
                let _ = devtools.call(
                    "Target.attachToTarget",
                    json!({ "targetId": target, "flatten": true }),
                    None,
                );
            }
            FollowUp::DiscardDownload(discard) => {
                if discard.going {
                    let _ = devtools.call(
                        "Browser.cancelDownload",
                        json!({ "guid": discard.guid }),
                        None,
                    );
                }
                // The engine names each file by its guid, so the guid is one plain name.
                let held = lock(&self.held_in).clone();
                if let Some(folder) = held
                    && !discard.guid.is_empty()
                    && !discard.guid.contains(['/', '\\', '\0'])
                    && discard.guid != ".."
                    && discard.guid != "."
                {
                    let file = folder.join(&discard.guid);
                    if std::fs::symlink_metadata(&file).is_ok_and(|there| there.is_file()) {
                        let _ = std::fs::remove_file(file);
                    }
                }
            }
            FollowUp::Close(target) => {
                let _ = devtools.call("Target.closeTarget", json!({ "targetId": target }), None);
            }
            FollowUp::SetUp(session) => {
                let on = Some(session.as_str());
                let _ = devtools.call("Page.enable", json!({}), on);
                // So the engine tells of each world it makes (#2138).
                let _ = devtools.call("Runtime.enable", json!({}), on);
                let size = {
                    let state = lock(&self.state);
                    let is_page = matches!(state.sessions.get(&session), Some(Session::Page(_)));
                    state.size.filter(|_| is_page)
                };
                if let Some((width, height)) = size {
                    let _ = devtools.call(
                        "Emulation.setDeviceMetricsOverride",
                        takeover::metrics(width, height),
                        on,
                    );
                }
                let _ = devtools.call("Network.enable", json!({}), on);
                // Nothing is held at its start: Chrome ends itself when a frame held so is swapped
                // away, as one that reloads quickly is (#4472). Requests are judged browser-wide
                // and what a frame already did is read back below, so holding it gains neither.
                let _ = devtools.call(
                    "Target.setAutoAttach",
                    json!({ "autoAttach": true, "waitForDebuggerOnStart": false, "flatten": true }),
                    on,
                );
                if let Ok(tree) = devtools.call("Page.getFrameTree", json!({}), on) {
                    let mut state = lock(&self.state);
                    let mut stack = vec![&tree["frameTree"]];
                    while let Some(node) = stack.pop() {
                        state.frame_seen(&session, &node["frame"], false);
                        if let Some(children) = node.get("childFrames").and_then(Value::as_array) {
                            stack.extend(children);
                        }
                    }
                }
                self.changed.notify_all();
                let _ = devtools.call("Runtime.runIfWaitingForDebugger", json!({}), on);
            }
            FollowUp::Judge { paused, outbound } => {
                // Judged on a thread of its own, so a request is never held up behind another
                // follow-up's calls; where that thread is gone, judged here.
                let unsent = match &*lock(&self.judging) {
                    Some(judging) => judging.send((paused, outbound)).err().map(|e| e.0),
                    None => Some((paused, outbound)),
                };
                if let Some((paused, outbound)) = unsent {
                    self.judged(&paused, &outbound);
                }
            }
            FollowUp::Refuse(request) => {
                let _ = devtools.call_within(
                    "Fetch.failRequest",
                    json!({ "requestId": request, "errorReason": "BlockedByClient" }),
                    None,
                    JUDGED_WITHIN,
                );
            }
            FollowUp::CancelDrag { session, at, data } => {
                let _ = devtools.call(
                    "Input.dispatchDragEvent",
                    drag_cancel(at, &data),
                    Some(&session),
                );
            }
            FollowUp::Release(session) => {
                let on = Some(session.as_str());
                let _ = devtools.call("Runtime.runIfWaitingForDebugger", json!({}), on);
                let _ = devtools.call(
                    "Target.detachFromTarget",
                    json!({ "sessionId": session }),
                    None,
                );
            }
        }
    }
}

/// The mark of an act addressing a page (#2018), taken off when the act is done.
struct Addressing<'a>(&'a Inner);

impl Drop for Addressing<'_> {
    fn drop(&mut self) {
        lock(&self.0.state).addressing = None;
    }
}

/// A request the engine holds for the judge (#2018): its id, and whether it is a document.
struct Paused {
    id: String,
    document: bool,
}

/// A request held for the judge, as the engine said it (#2018).
struct Held<'a> {
    request: &'a str,
    address: &'a str,
    method: &'a str,
    document: bool,
    frame: Option<&'a str>,
    redirected_from: Option<&'a str>,
}

/// Takes `address` out of `marks`, answering whether it was there.
fn take(marks: &mut VecDeque<String>, address: &str) -> bool {
    marks
        .iter()
        .position(|one| one == address)
        .and_then(|at| marks.remove(at))
        .is_some()
}

/// `address` without its fragment, which no request carries.
fn without_fragment(address: &str) -> &str {
    address.split_once('#').map_or(address, |(bare, _)| bare)
}

/// The live element `written` names: its page and node.
fn live_ref(state: &State, written: &Ref) -> Result<(String, NodeKey), Refusal> {
    let Some((page, key, generation)) = state.refs.by_ref.get(written) else {
        return Err(if state.refs.was_voided(written) {
            Refusal::HandedBack(written.clone())
        } else if state.refs.was_dealt(written) {
            stale(written)
        } else {
            Refusal::NoSuchRef(written.clone())
        });
    };
    let live = state
        .page(page)
        .and_then(|p| p.frames.get(&key.frame))
        .is_some_and(|f| f.generation == *generation);
    if !live {
        return Err(stale(written));
    }
    Ok((page.clone(), key.clone()))
}

/// Deals references for `target`'s page, each for its frame's current document.
fn refer_in<'s>(state: &'s mut State, target: &'s str) -> impl FnMut(&NodeKey) -> String + 's {
    move |key| {
        let generation = state
            .page(target)
            .and_then(|p| p.frames.get(&key.frame))
            .map_or(0, |f| f.generation);
        state.refs.deal(target, key, generation).0
    }
}

/// Every tree of `frames`, read through `devtools`, each frame's within `within`, holding at most
/// `most` bytes of the engine's answers in all (#3277): the page's own (the first) failing the
/// read, any other frame that cannot be read shown empty.
fn read_trees(
    devtools: &dyn DevTools,
    frames: Vec<ToRead>,
    within: Duration,
    most: usize,
) -> Result<Vec<FrameAx>, CallError> {
    read_trees_by(devtools, frames, within, most, None)
}

/// [`read_trees`], every call of it also ending by `by`, where it is given (#3277); then a frame
/// whose tree does not come by then, or does not fit in what is left of `most`, fails the read,
/// so no frame is taken for empty for want of time or room.
fn read_trees_by(
    devtools: &dyn DevTools,
    frames: Vec<ToRead>,
    within: Duration,
    most: usize,
    by: Option<Instant>,
) -> Result<Vec<FrameAx>, CallError> {
    let left = |within: Duration| {
        by.map_or(within, |by| {
            within.min(by.saturating_duration_since(Instant::now()))
        })
    };
    let mut room = most;
    let mut trees = Vec::new();
    for (index, read) in frames.into_iter().enumerate() {
        let mut frame = read.frame;
        frame.owner = read.parent_session.as_deref().and_then(|s| {
            let within = left(CALL_TIMEOUT);
            if within.is_zero() {
                return None;
            }
            devtools
                .call_within(
                    "DOM.getFrameOwner",
                    json!({ "frameId": frame.id }),
                    Some(s),
                    within,
                )
                .ok()
                .and_then(|o| o.get("backendNodeId").and_then(Value::as_i64))
        });
        let within = left(within);
        let ax = if within.is_zero() {
            Err(CallError::TimedOut)
        } else if room == 0 {
            Err(CallError::TooLarge { most })
        } else {
            devtools.call_within_bounded(
                "Accessibility.getFullAXTree",
                json!({ "frameId": frame.id }),
                Some(&read.session),
                within,
                room,
            )
        };
        frame.nodes = match ax {
            Ok(ax) => {
                let nodes = ax.get("nodes").cloned().unwrap_or(Value::Array(Vec::new()));
                // What this frame's tree holds counts against what the read may hold.
                let held = serde_json::to_vec(&nodes).map_or(room, |bytes| bytes.len());
                if held > room {
                    return Err(CallError::TooLarge { most });
                }
                room -= held;
                nodes
            }
            // The page's own tree is the snapshot; a frame that cannot be read is shown
            // empty rather than failing the page.
            Err(why) if index == 0 => return Err(why),
            Err(why @ (CallError::TimedOut | CallError::TooLarge { .. })) if by.is_some() => {
                return Err(why);
            }
            Err(_) => Value::Array(Vec::new()),
        };
        trees.push(frame);
    }
    Ok(trees)
}

/// What an answer built from a read an earlier call began says of it (#3277).
fn read_began(began: Duration) -> String {
    format!(
        "the page is slow to read, so this is the page as a read that began {} s ago found it",
        began.as_secs()
    )
}

/// Whether a described node is an `<input type=file>`.
fn is_file_input(node: &Value) -> bool {
    let local = node["localName"].as_str().unwrap_or_default();
    let attributes: Vec<&str> = node["attributes"]
        .as_array()
        .map(|all| all.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    local.eq_ignore_ascii_case("input")
        && attributes.chunks(2).any(|pair| {
            pair.len() == 2
                && pair[0].eq_ignore_ascii_case("type")
                && pair[1].trim().eq_ignore_ascii_case("file")
        })
}

fn stale(written: &Ref) -> Refusal {
    Refusal::StaleRef(written.clone())
}

fn refusal(why: CallError) -> Refusal {
    match why {
        CallError::Engine { message, .. } => Refusal::Engine(message),
        CallError::Ended | CallError::Write(_) => Refusal::Ended,
        CallError::TimedOut => Refusal::Unknown,
        CallError::TooLarge { .. } => Refusal::TooLarge,
    }
}

/// Whether the engine's words say an element is not laid out (so it shows nowhere).
fn not_drawn(message: &str) -> bool {
    message.contains("Could not compute content quads")
        || message.contains("No node with given id")
        || message.contains("No node found")
}

fn quads(answer: &Value) -> Vec<[f64; 8]> {
    answer
        .get("quads")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|q| {
            let q = q.as_array()?;
            let mut out = [0.0; 8];
            for (at, v) in q.iter().take(8).enumerate() {
                out[at] = v.as_f64()?;
            }
            (q.len() >= 8).then_some(out)
        })
        .collect()
}

/// The centre of the first quad with an area.
fn centre(answer: &Value) -> Option<(f64, f64)> {
    quads(answer).into_iter().find_map(|q| {
        let xs = [q[0], q[2], q[4], q[6]];
        let ys = [q[1], q[3], q[5], q[7]];
        let width = xs.iter().copied().fold(f64::MIN, f64::max)
            - xs.iter().copied().fold(f64::MAX, f64::min);
        let height = ys.iter().copied().fold(f64::MIN, f64::max)
            - ys.iter().copied().fold(f64::MAX, f64::min);
        (width > 0.0 && height > 0.0)
            .then(|| (xs.iter().sum::<f64>() / 4.0, ys.iter().sum::<f64>() / 4.0))
    })
}

/// How a page's CSS pixels map to its screenshot's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placing {
    /// Device pixels a CSS pixel takes, the visual viewport's zoom included.
    scale: f64,
    /// Where the visual viewport sits in the layout viewport, in CSS pixels.
    offset: (f64, f64),
}

impl Placing {
    /// From `Page.getLayoutMetrics`: the layout viewport in device pixels over the same in CSS
    /// pixels (the device pixel ratio, scrollbars left out of both), and the visual viewport's
    /// zoom and offset.
    pub(crate) fn from_metrics(metrics: &Value) -> Option<Self> {
        let number = |path: &str| metrics.pointer(path).and_then(Value::as_f64);
        let device = number("/layoutViewport/clientWidth")?;
        let css = number("/cssLayoutViewport/clientWidth")?;
        let zoom = number("/cssVisualViewport/zoom").unwrap_or(1.0);
        if !(device > 0.0 && css > 0.0 && zoom > 0.0) {
            return None;
        }
        Some(Self {
            scale: device / css * zoom,
            offset: (
                number("/cssVisualViewport/offsetX").unwrap_or_default(),
                number("/cssVisualViewport/offsetY").unwrap_or_default(),
            ),
        })
    }

    /// `css`, a box in CSS pixels of the layout viewport, on the picture, with a margin of two
    /// pixels.
    pub(crate) fn on_picture(&self, css: &Rect) -> Rect {
        Rect {
            x: (css.x - self.offset.0) * self.scale - 2.0,
            y: (css.y - self.offset.1) * self.scale - 2.0,
            width: css.width * self.scale + 4.0,
            height: css.height * self.scale + 4.0,
        }
    }
}

/// The box around every quad.
fn bounds(answer: &Value) -> Option<Rect> {
    quads(answer)
        .iter()
        .map(quad_bounds)
        .reduce(|a, b| hull(&a, &b))
}

/// The box around one quad.
fn quad_bounds(quad: &[f64; 8]) -> Rect {
    let (mut left, mut top, mut right, mut bottom) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for i in 0..4 {
        left = left.min(quad[2 * i]);
        right = right.max(quad[2 * i]);
        top = top.min(quad[2 * i + 1]);
        bottom = bottom.max(quad[2 * i + 1]);
    }
    Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

#[cfg(test)]
mod tests;

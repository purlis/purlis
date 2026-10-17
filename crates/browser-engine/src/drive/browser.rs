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
use super::keys::key_events;
use super::ledger::{Begin, Ledger};
use super::screencast::Screencast;
use super::settle::{Activity, Settle, activity};
use super::tree::{FrameAx, NodeKey, Query, Snapshot, View, build, scrubbed, scrubbed_address};
use super::{
    Aim, Answer, Call, CallId, Content, DragTo, Driver, Field, Gate, Judge, Limits, Outbound,
    PageId, PageInfo, Painted, Ref, Refusal, Settled, Why, lock,
};
use crate::WebAddress;
use crate::cdp::{CALL_TIMEOUT, CallError, Connection, Event};

mod takeover;

use takeover::{Lease, Typing};

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

    /// How many of the engine's events were dropped because nobody read them in time.
    fn dropped_events(&self) -> u64;

    fn call(&self, method: &str, params: Value, session: Option<&str>) -> Result<Value, CallError> {
        self.call_within(method, params, session, CALL_TIMEOUT)
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

    fn dropped_events(&self) -> u64 {
        Connection::dropped_events(self)
    }
}

/// The script that says whether a document has loaded: a constant, never the caller's words.
pub(crate) const READY_STATE: &str = "document.readyState";

/// The function that chooses a drop-down's options, run on the drop-down itself. A constant:
/// the options the caller named reach it only as its argument, a value.
pub(crate) const SELECT_OPTIONS: &str = r"function (wanted) {
  if (!(this instanceof HTMLSelectElement)) return { notSelect: true };
  const options = Array.from(this.options);
  const named = (o, w) => o.value === w || o.label === w || o.text.trim() === w;
  const missing = wanted.filter((w) => !options.some((o) => named(o, w)));
  if (missing.length) return { missing, options: options.map((o) => o.label || o.text) };
  if (!this.multiple && wanted.length > 1) return { notMultiple: true };
  for (const o of options) o.selected = wanted.some((w) => named(o, w));
  this.dispatchEvent(new Event('input', { bubbles: true }));
  this.dispatchEvent(new Event('change', { bubbles: true }));
  return { picked: options.filter((o) => o.selected).map((o) => o.label || o.text) };
}";

/// The function that reads what a field holds, run on the field itself (#2014): a constant,
/// whatever the field.
pub(crate) const READ_VALUE: &str = r"function () {
  if (typeof this.value === 'string') return this.value;
  if (this.isContentEditable) return this.innerText;
  return '';
}";

/// The script that says what the document has focused (#2014): a tag name, or nothing for the
/// document itself. A constant.
pub(crate) const ACTIVE_ELEMENT: &str = r"(() => {
  const at = document.activeElement;
  return at && at !== document.body && at !== document.documentElement ? at.tagName : '';
})()";

/// How long the driver gives the engine to say whether a document has loaded.
const READY_STATE_WITHIN: Duration = Duration::from_secs(2);
/// How often a wait looks again.
const POLL: Duration = Duration::from_millis(50);
/// How often a wait for text reads the page again.
const READ_AGAIN: Duration = Duration::from_millis(250);
/// How many moves a drag makes between its ends.
const DRAG_STEPS: usize = 10;
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
    scrubbed: Vec<String>,
    /// What the page shows, as a number that changes each time it or any frame in it loads
    /// another document, or a frame in it goes (#2014): the document a frame's aim names.
    shows: u64,
    frames: HashMap<String, Frame>,
}

impl Page {
    /// `text` with what this page's blanked elements held taken out.
    fn shown(&self, text: &str) -> String {
        let blanked: Vec<&str> = self.scrubbed.iter().map(String::as_str).collect();
        scrubbed(text, &blanked)
    }

    /// [`Page::shown`] for an address, read as its reader would read it (decoded).
    fn shown_address(&self, address: &str) -> String {
        let blanked: Vec<&str> = self.scrubbed.iter().map(String::as_str).collect();
        scrubbed_address(address, &blanked)
    }

    /// Keeps `held` out of every later answer about this page.
    fn keep_out(&mut self, held: &[String]) {
        for text in held {
            if !self.scrubbed.iter().any(|t| t.eq_ignore_ascii_case(text)) {
                self.scrubbed.push(text.clone());
            }
        }
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
    kept: Vec<String>,
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

    /// The address of document `frame` of `target`'s page, as its frame last navigated: the
    /// page's own where the frame is its main one or is not known.
    fn document_of(&self, target: &str, frame: &str) -> String {
        let Some(page) = self.page(target) else {
            return String::new();
        };
        match page.frames.get(frame) {
            Some(known) if frame != target && !known.url.is_empty() => known.url.clone(),
            _ => page.url.clone(),
        }
    }

    /// Keeps `site` to be said in the next answer, once.
    fn ask(&mut self, site: &str) {
        if !self.asked.iter().any(|one| one == site) && self.asked.len() < ASKED_KEPT {
            self.asked.push(site.to_owned());
        }
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
    fn keep_out_everywhere(&mut self, held: &[String]) {
        for text in held.iter().filter(|text| !text.is_empty()) {
            if !self.kept.iter().any(|kept| kept.eq_ignore_ascii_case(text)) {
                self.kept.push(text.clone());
            }
        }
        let kept = self.kept.clone();
        for page in &mut self.pages {
            page.keep_out(&kept);
        }
    }

    /// What a frame of `target`'s page shows now: the page, and its main frame's document.
    fn aim(&self, target: &str) -> Option<Aim> {
        let page = self.page(target)?;
        page.frames.get(target)?;
        Some(Aim {
            page: page.id.clone(),
            document: page.shows,
        })
    }

    fn info(&self, target: &str) -> Option<PageInfo> {
        self.page(target).map(|p| PageInfo {
            id: p.id.clone(),
            address: p.shown_address(&p.url),
            title: p.shown(&p.title),
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
        let entry = page.frames.entry(id.to_owned()).or_insert_with(|| Frame {
            session: session.to_owned(),
            parent: None,
            name: String::new(),
            url: String::new(),
            generation,
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
            if id == target {
                page.url = text("url");
            }
            self.refs.forget_frame(id);
            self.excluded.retain(|key| key.frame != id);
        }
    }

    /// Forgets `frame` and every frame inside it.
    fn frame_gone(&mut self, target: &str, frame: &str) {
        self.generations += 1;
        let shows = self.generations;
        let Some(page) = self.page_mut(target) else {
            return;
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
        for id in &gone {
            self.refs.forget_frame(id);
            self.excluded.retain(|key| &key.frame != id);
        }
    }
}

/// What the event reader does after it lets go of the state.
enum FollowUp {
    Attach(String),
    Close(String),
    SetUp(String),
    Release(String),
    /// A request held until the judge says whether it goes (#2018).
    Judge {
        request: String,
        outbound: Outbound,
    },
}

pub(crate) struct Inner {
    pub(crate) devtools: Arc<dyn DevTools>,
    state: Mutex<State>,
    changed: Condvar,
    limits: Limits,
    /// Frames for the person to watch (#2012).
    pub(crate) cast: Screencast,
    /// Who is told once the engine has ended, however it ended.
    ends: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// Who judges where its pages may go (#2018), when anyone does.
    judge: Option<Judge>,
    /// Whether the person drives (#2014): every request is then theirs (#2018).
    person_drives: AtomicBool,
    /// Where each held request is sent to be judged, on a thread of its own (#2018).
    judging: Mutex<Option<std::sync::mpsc::Sender<(String, Outbound)>>>,
}

/// A frame of a page to read: where its tree comes from and where its holder is.
struct ToRead {
    frame: FrameAx,
    session: String,
    parent_session: Option<String>,
}

/// Where an element is, for acting on it.
struct Located {
    target: String,
    page_session: String,
    node_session: String,
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
    /// What the person typed while driving, kept out of later answers (#2014).
    typing: Mutex<Typing>,
    /// The engine, until it is stopped: behind a lock of its own, so a stop never waits for a
    /// call in flight.
    #[cfg(unix)]
    engine: Mutex<Option<crate::process::Engine>>,
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
        let mut browser = Self::over(devtools, events, limits, judge)?;
        browser.engine = Mutex::new(Some(engine));
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
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            limits: limits.clone(),
            cast: Screencast::new(),
            ends: Mutex::new(None),
            judge,
            person_drives: AtomicBool::new(false),
            judging: Mutex::new(None),
        });
        Screencast::start(&inner);
        if inner.judge.is_some() {
            let (judging, held) = std::sync::mpsc::channel::<(String, Outbound)>();
            let judge_for = Arc::downgrade(&inner);
            thread::Builder::new()
                .name("browser-judge".to_owned())
                .spawn(move || {
                    for (request, outbound) in held {
                        let Some(inner) = judge_for.upgrade() else {
                            break;
                        };
                        inner.judged(&request, &outbound);
                    }
                })
                .map_err(|e| Refusal::Engine(e.to_string()))?;
            *lock(&inner.judging) = Some(judging);
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
                let tell = lock(&reading.ends).take();
                if let Some(tell) = tell {
                    tell();
                }
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
            typing: Mutex::new(Typing::default()),
            #[cfg(unix)]
            engine: Mutex::new(None),
        })
    }

    /// Asks the engine to close, waits up to `grace`, then ends it. A call in flight then
    /// answers that the browser has ended.
    #[cfg(unix)]
    pub fn stop(&self, grace: Duration) {
        let engine = lock(&self.engine).take();
        if let Some(engine) = engine {
            engine.stop(grace);
        }
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
            return self
                .run_at(epoch, call)
                .map(|answer| self.with_asked(answer));
        }
        match self.ledger.begin(id, &call) {
            Begin::Fresh(ticket) => {
                let answer = {
                    let _one = lock(&self.acting);
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
    fn with_asked(&self, mut answer: Answer) -> Answer {
        answer.asked = std::mem::take(&mut lock(&self.inner.state).asked);
        answer
    }

    /// **Where the agent is about to type, keys or choices into the documents at `addresses`
    /// of `target`'s page** (#2018): each judged as something sent to its site. Where the judge
    /// asks about any, nothing is sent to the page, and the answer says so.
    fn typing_held(&self, target: &str, addresses: &[String]) -> Option<Answer> {
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
        let mut answer = self.answer_now(target, Content::Nothing);
        answer.note = Some(format!(
            "nothing was typed: this page shows {site}, a site new to this project, where this \
             would have gone, and purlis asked the person whether this browser may send to it"
        ));
        Some(answer)
    }

    /// The address of the document `written` is in: its frame's.
    fn address_of(&self, written: &Ref) -> Result<String, Refusal> {
        let state = lock(&self.inner.state);
        let (page, key) = live_ref(&state, written)?;
        Ok(state.document_of(&page, &key.frame))
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
            Some(frame) => vec![state.document_of(target, &frame)],
            None => state.page(target).map_or_else(Vec::new, |page| {
                let mut every: Vec<String> = page.frames.values().map(|f| f.url.clone()).collect();
                every.push(page.url.clone());
                every.sort();
                every.dedup();
                every
            }),
        }
    }

    /// Marks `target`'s page as the one the agent's act is sending to an address, until the
    /// mark is dropped (#2018).
    fn addressing(&self, target: &str, why: Why) -> Addressing<'_> {
        lock(&self.inner.state).addressing = Some((target.to_owned(), why));
        Addressing(&self.inner)
    }

    /// The engine's own pid, while this browser holds its engine.
    #[cfg(unix)]
    pub fn process(&self) -> Option<u32> {
        lock(&self.engine).as_ref().map(crate::process::Engine::pid)
    }

    /// Calls `tell` once the engine has ended, however it ended: at once if it has.
    pub fn when_it_ends(&self, tell: Box<dyn FnOnce() + Send>) {
        let now = {
            let state = lock(&self.inner.state);
            if state.ended {
                Some(tell)
            } else {
                *lock(&self.inner.ends) = Some(tell);
                None
            }
        };
        if let Some(tell) = now {
            tell();
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

    /// Stops its frames. Returns at once.
    pub fn hide(&self) {
        self.inner.cast.hide();
    }

    /// Adds the element `target` names to the blanked set: from now on no snapshot, find, wait
    /// or screenshot shows it while it exists, and what it holds is kept out of every answer
    /// about its page while the page lasts. The hook a login, and the person's typing, fill.
    pub fn blank(&self, target: &Ref) -> Result<(), Refusal> {
        let (page, key) = live_ref(&lock(&self.inner.state), target)?;
        self.blank_node(&page, key)
    }

    /// [`Browser::blank`] for the element that has the focus on `page` (the current page when
    /// `None`), whatever it is: the field the person is typing into, even one that is no form
    /// field and has no reference.
    pub fn blank_focused(&self, page: Option<&PageId>) -> Result<(), Refusal> {
        self.blank_what_has_focus(page).map(|_| ())
    }

    /// [`Browser::blank_focused`], answering the element it blanked and its page's target.
    fn blank_what_has_focus(&self, page: Option<&PageId>) -> Result<(String, NodeKey), Refusal> {
        let target = self.target(page)?;
        let snapshot = self.snapshot_of(&target)?;
        let key = match snapshot.focused() {
            [] => return Err(Refusal::NothingFocused),
            [one] => one.clone(),
            _ => return Err(Refusal::FocusUnclear),
        };
        self.blank_node(&target, key.clone())?;
        Ok((target, key))
    }

    fn blank_node(&self, target: &str, key: NodeKey) -> Result<(), Refusal> {
        lock(&self.inner.state).excluded.insert(key);
        // Read the page now, so what the element holds is kept out from this moment.
        self.snapshot_of(target).map(|_| ())
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
                self.reading(&target, |snapshot, state| {
                    let scope = within
                        .as_ref()
                        .map(|r| live_ref(state, r).map(|(_, key)| key))
                        .transpose()?;
                    let view = View {
                        within: scope,
                        from,
                        lines,
                    };
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
                self.reading(&target, |snapshot, state| {
                    let scope = within
                        .as_ref()
                        .map(|r| live_ref(state, r).map(|(_, key)| key))
                        .transpose()?;
                    let mut refer = refer_in(state, &target);
                    snapshot
                        .find(
                            &Query {
                                text: text.clone(),
                                role: role.clone(),
                            },
                            scope.as_ref(),
                            &mut refer,
                        )
                        .map_err(|_| {
                            Refusal::NotShown(
                                within.clone().unwrap_or_else(|| Ref::new("the scope")),
                            )
                        })
                })
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
                            sessions.push((prefix.clone(), session.clone()));
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
                for (prefix, session) in &sessions {
                    let ready = self.devtools().call_within(
                        "Runtime.evaluate",
                        json!({ "expression": READY_STATE, "returnByValue": true }),
                        Some(session),
                        READY_STATE_WITHIN,
                    );
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

    /// Waits for `target` to settle, builds its snapshot and reads it with `read`.
    fn reading(
        &self,
        target: &str,
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
        let snapshot = self.snapshot_of(target)?;
        let text = {
            let mut state = lock(&self.inner.state);
            let body = read(&snapshot, &mut state)?;
            let page = state.page(target).ok_or(Refusal::NoPage)?;
            let title = if snapshot.title().is_empty() {
                page.shown(&page.title)
            } else {
                snapshot.title().to_owned()
            };
            format!(
                "page {} \"{}\" {}\n{body}",
                page.id,
                title.replace('"', "\\\""),
                page.shown_address(&page.url)
            )
        };
        Ok(self.answer(target, settled, opened, Content::Text(text), None))
    }

    /// `target`'s tree as it is now, every frame in it.
    fn snapshot_of(&self, target: &str) -> Result<Snapshot, Refusal> {
        let session = self.page_session(target)?;
        let (frames, excluded, scrub) = {
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
        let mut trees = Vec::new();
        for (index, read) in frames.into_iter().enumerate() {
            let mut frame = read.frame;
            frame.owner = read.parent_session.as_deref().and_then(|s| {
                self.devtools()
                    .call("DOM.getFrameOwner", json!({ "frameId": frame.id }), Some(s))
                    .ok()
                    .and_then(|o| o.get("backendNodeId").and_then(Value::as_i64))
            });
            let ax = self.devtools().call(
                "Accessibility.getFullAXTree",
                json!({ "frameId": frame.id }),
                Some(&read.session),
            );
            frame.nodes = match ax {
                Ok(ax) => ax.get("nodes").cloned().unwrap_or(Value::Array(Vec::new())),
                // The page's own tree is the snapshot; a frame that cannot be read is shown
                // empty rather than failing the page.
                Err(why) if index == 0 => return Err(refusal(why)),
                Err(_) => Value::Array(Vec::new()),
            };
            trees.push(frame);
        }
        let snapshot = build(&trees, &excluded, &scrub);
        if let Some(page) = lock(&self.inner.state).page_mut(target) {
            page.keep_out(snapshot.blanked_text());
        }
        Ok(snapshot)
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
            backend: key.backend,
            boundaries,
        })
    }

    /// Where frames with sessions of their own put `at`'s frame on the page, in CSS pixels.
    fn frame_offset(&self, at: &Located, scroll: bool) -> Result<(f64, f64), CallError> {
        let (mut x, mut y) = (0.0, 0.0);
        for (frame, parent_session) in &at.boundaries {
            let owner = self.devtools().call(
                "DOM.getFrameOwner",
                json!({ "frameId": frame }),
                Some(parent_session),
            )?;
            let owner = owner
                .get("backendNodeId")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            if scroll {
                let gone_on = self.gated(|| {
                    self.devtools().call(
                        "DOM.scrollIntoViewIfNeeded",
                        json!({ "backendNodeId": owner }),
                        Some(parent_session),
                    )
                });
                if gone_on.is_err() {
                    return Err(CallError::TimedOut);
                }
            }
            let model = self.devtools().call(
                "DOM.getBoxModel",
                json!({ "backendNodeId": owner }),
                Some(parent_session),
            )?;
            let content = model.pointer("/model/content").and_then(Value::as_array);
            let corner = |i: usize| {
                content
                    .and_then(|c| c.get(i))
                    .and_then(Value::as_f64)
                    .unwrap_or_default()
            };
            x += corner(0);
            y += corner(1);
        }
        Ok((x, y))
    }

    /// The page point at the centre of `at`, brought into view first when `scroll`.
    fn point(&self, target: &Ref, at: &Located, scroll: bool) -> Result<(f64, f64), Refusal> {
        let shown = |why: CallError| match why {
            CallError::Engine { .. } => Refusal::NotShown(target.clone()),
            other => refusal(other),
        };
        let (dx, dy) = self.frame_offset(at, scroll).map_err(shown)?;
        if scroll {
            let _ = self.gated(|| {
                self.devtools().call(
                    "DOM.scrollIntoViewIfNeeded",
                    json!({ "backendNodeId": at.backend }),
                    Some(&at.node_session),
                )
            })?;
        }
        let quads = self
            .devtools()
            .call(
                "DOM.getContentQuads",
                json!({ "backendNodeId": at.backend }),
                Some(&at.node_session),
            )
            .map_err(shown)?;
        let (x, y) = centre(&quads).ok_or_else(|| Refusal::NotShown(target.clone()))?;
        Ok((x + dx, y + dy))
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
                Some(written) => into.push(self.address_of(written)?),
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
        self.acting_on(&target, || {
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
        if let Some(held) = self.typing_held(&page, &[self.address_of(target)?]) {
            return Ok(held);
        }
        let resolved = self
            .devtools()
            .call(
                "DOM.resolveNode",
                json!({ "backendNodeId": at.backend }),
                Some(&at.node_session),
            )
            .map_err(|why| match why {
                CallError::Engine { .. } => Refusal::NotShown(target.clone()),
                other => refusal(other),
            })?;
        let object = resolved
            .pointer("/object/objectId")
            .and_then(Value::as_str)
            .ok_or_else(|| Refusal::NotShown(target.clone()))?
            .to_owned();
        let answer = self.acting_on(&page, || {
            let chose = self
                .gated(|| {
                    self.devtools().call(
                        "Runtime.callFunctionOn",
                        json!({
                            "objectId": object,
                            "functionDeclaration": SELECT_OPTIONS,
                            "arguments": [{ "value": options }],
                            "returnByValue": true,
                        }),
                        Some(&at.node_session),
                    )
                })?
                .map_err(refusal)?;
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
        });
        let _ = self.devtools().call(
            "Runtime.releaseObject",
            json!({ "objectId": object }),
            Some(&at.node_session),
        );
        answer
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
            self.mouse(session, "mouseMoved", start.0, start.1, 0, 0)?;
            self.mouse(session, "mousePressed", start.0, start.1, 1, 1)?;
            for step in 1..=DRAG_STEPS {
                let part = step as f64 / DRAG_STEPS as f64;
                let x = start.0 + (end.0 - start.0) * part;
                let y = start.1 + (end.1 - start.1) * part;
                self.mouse(session, "mouseMoved", x, y, 0, 1)?;
            }
            self.mouse(session, "mouseReleased", end.0, end.1, 1, 0)?;
            Ok(None)
        })
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
        let deadline = Instant::now() + within.min(self.inner.limits.longest_wait);
        let seen = loop {
            let shows = self.snapshot_of(&target)?.shows_text(text);
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
        // after the picture: both are painted.
        let mut boxes = self.blanked_boxes(&target)?;
        let shot = self
            .devtools()
            .call(
                "Page.captureScreenshot",
                json!({ "format": "png" }),
                Some(&session),
            )
            .map_err(refusal)?;
        boxes.extend(self.blanked_boxes(&target)?);
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

    /// The box of every blanked element on `target`'s page, and of every element that shows a
    /// text kept out (#2014), in CSS pixels. An element the engine says it cannot place is not
    /// drawn; any other failure withholds the screenshot.
    fn blanked_boxes(&self, target: &str) -> Result<Vec<Rect>, Refusal> {
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
        let mut boxes = Vec::new();
        for key in keys {
            let at = {
                let state = lock(&self.inner.state);
                let page = state.page(target).ok_or(Refusal::NoPage)?;
                let Some(frame) = page.frames.get(&key.frame) else {
                    continue;
                };
                let boundaries = page.boundaries(&key.frame);
                Located {
                    target: target.to_owned(),
                    page_session: page.session.clone().unwrap_or_default(),
                    node_session: frame.session.clone(),
                    backend: key.backend,
                    boundaries,
                }
            };
            let placed = self.frame_offset(&at, false).and_then(|offset| {
                self.devtools()
                    .call(
                        "DOM.getContentQuads",
                        json!({ "backendNodeId": at.backend }),
                        Some(&at.node_session),
                    )
                    .map(|quads| (offset, quads))
            });
            match placed {
                Ok(((dx, dy), quads)) => {
                    if let Some(mut rect) = bounds(&quads) {
                        rect.x += dx;
                        rect.y += dy;
                        boxes.push(rect);
                    }
                }
                Err(CallError::Engine { message, .. }) if not_drawn(&message) => {}
                Err(why) => return Err(Refusal::Withheld(why.to_string())),
            }
        }
        Ok(boxes)
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

    /// Takes in one event from the engine.
    fn take_in(&self, event: &Event) {
        if event.method == "Page.screencastFrame" {
            let aim = event.session_id.as_deref().and_then(|session| {
                let state = lock(&self.state);
                let target = state.page_of_session(session)?;
                state.aim(&target)
            });
            if let Some(aim) = aim {
                self.cast
                    .frame(&event.params, event.session_id.as_deref(), aim);
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
                            if let Some(opener) = opener
                                && let Some(by) = state.page_mut(&opener)
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
                                frames: HashMap::new(),
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
                                if let Some(page) = state.page_mut(&page)
                                    && let Some(frame) = page.frames.get_mut(&target)
                                {
                                    frame.session.clone_from(&session);
                                }
                                follow.push(FollowUp::SetUp(session));
                            }
                            _ => follow.push(FollowUp::Release(session)),
                        }
                    }
                }
                "Target.detachedFromTarget" => {
                    if let Some(session) = text("/sessionId") {
                        if let Some(target) = state.page_of_session(&session)
                            && let Some(page) = state.page_mut(&target)
                        {
                            page.settle.forget_session(&session, now);
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
                        state.frame_seen(session, &params["frame"], true);
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
                "Fetch.requestPaused" => {
                    if let Some(request) = text("/requestId") {
                        let address = text("/request/url").unwrap_or_default();
                        let method = text("/request/method").unwrap_or_default();
                        let frame = text("/frameId");
                        let redirected_from = text("/redirectedRequestId");
                        let why = state.why(&Held {
                            request: &request,
                            address: &address,
                            method: &method,
                            document: text("/resourceType").as_deref() == Some("Document"),
                            frame: frame.as_deref(),
                            redirected_from: redirected_from.as_deref(),
                        });
                        let by = if self.person_drives.load(Ordering::SeqCst) {
                            Driver::Person
                        } else {
                            Driver::Agent
                        };
                        follow.push(FollowUp::Judge {
                            request,
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
                    let swapped = text("/reason").as_deref() == Some("swap");
                    if !swapped
                        && let (Some(session), Some(frame)) = (&event.session_id, text("/frameId"))
                        && let Some(target) = state.page_of_session(session)
                    {
                        state.frame_gone(&target, &frame);
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
            self.follow_up(step);
        }
    }

    /// Judges held request `request` (#2018), and lets it on or fails it as the judge says.
    fn judged(&self, request: &str, outbound: &Outbound) {
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
                self.devtools.call_within(
                    "Fetch.failRequest",
                    json!({ "requestId": request, "errorReason": "BlockedByClient" }),
                    None,
                    JUDGED_WITHIN,
                )
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
            FollowUp::Close(target) => {
                let _ = devtools.call("Target.closeTarget", json!({ "targetId": target }), None);
            }
            FollowUp::SetUp(session) => {
                let on = Some(session.as_str());
                let _ = devtools.call("Page.enable", json!({}), on);
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
                let _ = devtools.call(
                    "Target.setAutoAttach",
                    json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }),
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
            FollowUp::Judge { request, outbound } => {
                // Judged on a thread of its own, so a request is never held up behind another
                // follow-up's calls; where that thread is gone, judged here.
                let unsent = match &*lock(&self.judging) {
                    Some(judging) => judging.send((request, outbound)).err().map(|e| e.0),
                    None => Some((request, outbound)),
                };
                if let Some((request, outbound)) = unsent {
                    self.judged(&request, &outbound);
                }
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

fn stale(written: &Ref) -> Refusal {
    Refusal::StaleRef(written.clone())
}

fn refusal(why: CallError) -> Refusal {
    match why {
        CallError::Engine { message, .. } => Refusal::Engine(message),
        CallError::Ended | CallError::Write(_) => Refusal::Ended,
        CallError::TimedOut => Refusal::Unknown,
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
    let all = quads(answer);
    if all.is_empty() {
        return None;
    }
    let (mut left, mut top, mut right, mut bottom) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in all {
        for i in 0..4 {
            left = left.min(q[2 * i]);
            right = right.max(q[2 * i]);
            top = top.min(q[2 * i + 1]);
            bottom = bottom.max(q[2 * i + 1]);
        }
    }
    Some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

#[cfg(test)]
mod tests;

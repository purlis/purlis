// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One browser's pages, driven over DevTools: what the engine's events say about its pages and
//! frames, and each call made into the engine's own calls.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{Value, json};

use super::blank::{Rect, paint_over};
use super::keys::key_events;
use super::ledger::{Begin, Ledger};
use super::settle::{Activity, Settle, activity};
use super::tree::{FrameAx, NodeKey, Query, Snapshot, View, build};
use super::{
    Answer, Call, CallId, Content, DragTo, Field, Limits, PageId, PageInfo, Ref, Refusal, Settled,
    lock,
};
use crate::WebAddress;
use crate::cdp::{CALL_TIMEOUT, CallError, Connection, Event};

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

/// How long the driver gives the engine to say whether a document has loaded.
const READY_STATE_WITHIN: Duration = Duration::from_secs(2);
/// How often a wait looks again.
const POLL: Duration = Duration::from_millis(50);
/// How often a wait for text reads the page again.
const READ_AGAIN: Duration = Duration::from_millis(250);
/// How many moves a drag makes between its ends.
const DRAG_STEPS: usize = 10;

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
    frames: HashMap<String, Frame>,
}

impl Page {
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
        written
            .as_str()
            .strip_prefix('e')
            .and_then(|n| n.parse::<u64>().ok())
            .is_some_and(|n| n >= 1 && n <= self.dealt)
    }
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
}

impl State {
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

    fn info(&self, target: &str) -> Option<PageInfo> {
        self.page(target).map(|p| PageInfo {
            id: p.id.clone(),
            address: p.url.clone(),
            title: p.title.clone(),
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
}

struct Inner {
    devtools: Arc<dyn DevTools>,
    state: Mutex<State>,
    changed: Condvar,
    limits: Limits,
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
    #[cfg(unix)]
    engine: Option<crate::process::Engine>,
}

impl Browser {
    /// Drives `engine`'s pages within `limits`. The engine's events are the browser's from now.
    #[cfg(unix)]
    pub fn new(mut engine: crate::process::Engine, limits: Limits) -> Result<Self, Refusal> {
        let events = engine.take_events().ok_or(Refusal::Ended)?;
        let devtools: Arc<dyn DevTools> = engine.devtools_shared();
        let mut browser = Self::over(devtools, events, limits)?;
        browser.engine = Some(engine);
        Ok(browser)
    }

    /// Drives the pages `devtools` reaches, reading their events from `events`.
    pub(crate) fn over(
        devtools: Arc<dyn DevTools>,
        events: Receiver<Event>,
        limits: Limits,
    ) -> Result<Self, Refusal> {
        let inner = Arc::new(Inner {
            devtools,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            limits: limits.clone(),
        });
        let reading = inner.clone();
        thread::Builder::new()
            .name("browser-events".to_owned())
            .spawn(move || {
                for event in events {
                    reading.take_in(&event);
                }
                lock(&reading.state).ended = true;
                reading.changed.notify_all();
            })
            .map_err(|e| Refusal::Engine(e.to_string()))?;
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
            #[cfg(unix)]
            engine: None,
        })
    }

    /// Asks the engine to close, waits up to `grace`, then ends it.
    #[cfg(unix)]
    pub fn stop(mut self, grace: Duration) {
        if let Some(engine) = self.engine.take() {
            engine.stop(grace);
        }
    }

    /// Answers `call`. A call whose `id` was answered before answers the same again and acts on
    /// nothing; an `id` given to a different call is refused.
    pub fn call(&self, id: &CallId, call: Call) -> Result<Answer, Refusal> {
        match self.ledger.begin(id, &call) {
            Begin::Fresh(ticket) => {
                let answer = {
                    let _one = lock(&self.acting);
                    self.run(call)
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

    /// Adds the element `target` names to the blanked set: from now on no snapshot, find, wait
    /// or screenshot shows what it holds, while its document lasts. The hook a login, and the
    /// person's typing, fill.
    pub fn blank(&self, target: &Ref) -> Result<(), Refusal> {
        let mut state = lock(&self.inner.state);
        let key = live_ref(&state, target)?.1;
        state.excluded.insert(key);
        Ok(())
    }

    fn run(&self, call: Call) -> Result<Answer, Refusal> {
        match call {
            Call::Open { address } => self.open(&address),
            Call::Navigate { page, address } => {
                let target = self.target(page.as_ref())?;
                let session = self.page_session(&target)?;
                self.acting_on(&target, || self.navigate(&session, &address))
            }
            Call::Snapshot {
                page,
                within,
                from,
                lines,
            } => {
                let target = self.target(page.as_ref())?;
                let lines = lines.unwrap_or(self.inner.limits.snapshot_lines);
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
                let session = self.page_session(&target)?;
                let mut events = Vec::new();
                for chord in &keys {
                    events.extend(key_events(chord).map_err(|e| Refusal::NotAKey(e.to_string()))?);
                }
                self.acting_on(&target, || {
                    for event in events {
                        self.devtools()
                            .call("Input.dispatchKeyEvent", event, Some(&session))
                            .map_err(refusal)?;
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
                    },
                };
                answer.content = Content::Pages(pages);
                Ok(answer)
            }
            Call::SwitchPage { page } => {
                let target = self.target(Some(&page))?;
                lock(&self.inner.state).current = Some(target.clone());
                let _ = self.devtools().call(
                    "Target.activateTarget",
                    json!({ "targetId": target }),
                    None,
                );
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
            .devtools()
            .call("Target.createTarget", json!({ "url": "about:blank" }), None)
            .map_err(refusal)?;
        let target = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| Refusal::Engine("no page was made".into()))?
            .to_owned();
        // The engine says it made the page before its event about the page has been read.
        let deadline = Instant::now() + self.inner.limits.settle_within;
        let mut state = lock(&self.inner.state);
        while state.page(&target).is_none() && !state.ended && Instant::now() < deadline {
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
        self.acting_on(&target, || self.navigate(&session, address))
    }

    /// Sends the page on `session` to `address`; a load that failed is said in the note.
    fn navigate(&self, session: &str, address: &WebAddress) -> Result<Option<String>, Refusal> {
        let went = self
            .devtools()
            .call(
                "Page.navigate",
                json!({ "url": address.as_str() }),
                Some(session),
            )
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
        let note = act()?;
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
                page.title.clone()
            } else {
                snapshot.title().to_owned()
            };
            format!(
                "page {} \"{}\" {}\n{body}",
                page.id,
                title.replace('"', "\\\""),
                page.url
            )
        };
        Ok(self.answer(target, settled, opened, Content::Text(text), None))
    }

    /// `target`'s tree as it is now, every frame in it.
    fn snapshot_of(&self, target: &str) -> Result<Snapshot, Refusal> {
        let session = self.page_session(target)?;
        let (frames, excluded) = {
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
            (frames, state.excluded.clone())
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
        Ok(build(&trees, &excluded))
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
                let _ = self.devtools().call(
                    "DOM.scrollIntoViewIfNeeded",
                    json!({ "backendNodeId": owner }),
                    Some(parent_session),
                );
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
            let _ = self.devtools().call(
                "DOM.scrollIntoViewIfNeeded",
                json!({ "backendNodeId": at.backend }),
                Some(&at.node_session),
            );
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
        self.devtools()
            .call("Input.dispatchMouseEvent", event, Some(session))
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
        let session = self.page_session(&target)?;
        let mut located = Vec::new();
        for field in fields {
            located.push(match &field.into {
                Some(r) => Some((r.clone(), self.locate(r)?)),
                None => None,
            });
        }
        self.acting_on(&target, || {
            for (field, at) in fields.iter().zip(&located) {
                if let Some((r, at)) = at {
                    self.devtools()
                        .call(
                            "DOM.focus",
                            json!({ "backendNodeId": at.backend }),
                            Some(&at.node_session),
                        )
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
                        self.devtools()
                            .call("Input.dispatchKeyEvent", event, Some(&session))
                            .map_err(refusal)?;
                    }
                }
                if !field.text.is_empty() {
                    self.devtools()
                        .call(
                            "Input.insertText",
                            json!({ "text": field.text }),
                            Some(&session),
                        )
                        .map_err(refusal)?;
                }
            }
            Ok(None)
        })
    }

    fn select(&self, target: &Ref, options: &[String]) -> Result<Answer, Refusal> {
        let at = self.locate(target)?;
        let page = at.target.clone();
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
                .devtools()
                .call(
                    "Runtime.callFunctionOn",
                    json!({
                        "objectId": object,
                        "functionDeclaration": SELECT_OPTIONS,
                        "arguments": [{ "value": options }],
                        "returnByValue": true,
                    }),
                    Some(&at.node_session),
                )
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
                self.devtools()
                    .call(
                        "Input.dispatchMouseEvent",
                        json!({
                            "type": "mouseWheel", "x": point.0, "y": point.1,
                            "deltaX": x, "deltaY": y, "modifiers": 0,
                        }),
                        Some(&session),
                    )
                    .map_err(refusal)?;
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
        self.acting_on(&target, || {
            self.devtools()
                .call(
                    "Page.navigateToHistoryEntry",
                    json!({ "entryId": entry }),
                    Some(&session),
                )
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
            thread::sleep(READ_AGAIN.min(deadline.saturating_duration_since(Instant::now())));
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
        // Where each blanked element is, before and after the picture: both are painted.
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
        let scale = if boxes.is_empty() {
            1.0
        } else {
            let metrics = self
                .devtools()
                .call("Page.getLayoutMetrics", json!({}), Some(&session))
                .map_err(|why| Refusal::Withheld(why.to_string()))?;
            let css_width = metrics
                .pointer("/cssVisualViewport/clientWidth")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            match png_width(&png) {
                Some(width) if css_width > 0.0 => f64::from(width) / css_width,
                _ => return Err(Refusal::Withheld("the picture's size is not known".into())),
            }
        };
        let rects: Vec<Rect> = boxes
            .iter()
            .map(|r| Rect {
                x: r.x * scale - 2.0,
                y: r.y * scale - 2.0,
                width: r.width * scale + 4.0,
                height: r.height * scale + 4.0,
            })
            .collect();
        let painted = paint_over(&png, &rects).map_err(|why| Refusal::Withheld(why.to_string()))?;
        Ok(self.answer(&target, settled, opened, Content::Image(painted), None))
    }

    /// The box of every blanked element on `target`'s page, in CSS pixels. An element the engine
    /// says it cannot place is not drawn; any other failure withholds the screenshot.
    fn blanked_boxes(&self, target: &str) -> Result<Vec<Rect>, Refusal> {
        let keys: Vec<NodeKey> = {
            let state = lock(&self.inner.state);
            let page = state.page(target).ok_or(Refusal::NoPage)?;
            state
                .excluded
                .iter()
                .filter(|k| page.frames.contains_key(&k.frame))
                .cloned()
                .collect()
        };
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
    /// Takes in one event from the engine.
    fn take_in(&self, event: &Event) {
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
                            state.pages.push(Page {
                                id,
                                target: target.clone(),
                                session: None,
                                url: url.clone(),
                                title: String::new(),
                                settle: Settle::new(now),
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

/// The live element `written` names: its page and node.
fn live_ref(state: &State, written: &Ref) -> Result<(String, NodeKey), Refusal> {
    let Some((page, key, generation)) = state.refs.by_ref.get(written) else {
        return Err(if state.refs.was_dealt(written) {
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

/// A PNG's width, from its header.
fn png_width(png: &[u8]) -> Option<u32> {
    let header = png.get(16..20)?;
    (png.get(12..16)? == b"IHDR")
        .then(|| u32::from_be_bytes([header[0], header[1], header[2], header[3]]))
}

#[cfg(test)]
mod tests;

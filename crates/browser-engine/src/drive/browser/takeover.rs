// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Taking a browser over (#2014, spec #2008): the driver lease, and the person's own input.
//!
//! **One driver at a time.** The lease is held by the agent or by the person, and every change
//! of holder starts a new epoch. A call of the agent's is made under an epoch
//! ([`Browser::call_at`]) and refused unless the agent holds that lease when it starts; every
//! act it sends a page is sent under the lease's lock, after checking the lease is still the one
//! the call began under, so once [`Browser::take_over`] returns no act of the agent's reaches a
//! page. A call the take-over cuts answers [`Refusal::Interrupted`], whatever it had read: what
//! the page showed while the person drove is never handed to the agent.
//!
//! **The person's input** ([`Browser::person_input`]) is taken only while they drive, and only
//! where it was aimed: at the page and documents a frame showed them. What the person types or
//! pastes stays out of every answer the agent gets:
//!
//! - Before a key that may change a field reaches the page, the element that has the focus is
//!   blanked, so it is in no later snapshot, find or screenshot. A page that cannot say which
//!   element has the focus, or has one focused that its tree does not show, has the key held
//!   back, and the person is told.
//! - After the key, what the field holds is read from the field itself, and the person's runs
//!   of typed characters are kept as they typed them; both are kept out of every later answer
//!   about every page of the browser, a page opened later included, whatever became of the
//!   field. A press of the mouse and the hand-back read every field the person touched again.
//!   A field keeps its latest value: a read drops the field's short earlier values that the new
//!   one contains ([`KEPT_WHOLE_FROM`](super::super::kept::KEPT_WHOLE_FROM)), so what was typed
//!   key by key is kept out whole, not as each short part on the way. What a key that neither
//!   types nor deletes put into the field (a paste), and what the field holds after it, are
//!   kept as they were, like a run.
//!
//! **The hand-back is explicit** ([`Browser::hand_back`]): nothing returns the lease to the
//! agent by itself. It voids every reference dealt before it, so the agent's next act starts
//! from a new snapshot.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::kept::Kept;
use super::super::keys::{command_of, is_modifier};
use super::super::tree::{FrameAx, NodeKey, Snapshot};
use super::{ACTIVE_ELEMENT, FOCUSED_HERE, READ_VALUE};
use super::{Browser, Session, read_trees, refusal};
use crate::cdp::CallError;
use crate::drive::{
    ALT, Aim, CONTROL, Call, Driver, META, Modifiers, MouseButton, MouseKind, PersonInput, Refusal,
    SHIFT, lock,
};

/// Who holds a browser, and the epoch of their lease.
pub(crate) struct Lease {
    pub(super) epoch: u64,
    pub(super) driver: Driver,
}

impl Default for Lease {
    fn default() -> Self {
        Self {
            epoch: 0,
            driver: Driver::Agent,
        }
    }
}

/// What the person typed while driving, still to be kept out of later answers.
#[derive(Default)]
pub(crate) struct Typing {
    /// The pages (by target) the person typed into since each was last read.
    dirty: HashSet<String>,
    /// The runs of characters the person typed on each page (by target), the last one still
    /// growing: a key that types no character, or a press of the mouse, ends a run.
    runs: HashMap<String, Vec<String>>,
    /// Every field the person typed into (its page's target and its element), read again at a
    /// press of the mouse and at the hand-back.
    touched: HashSet<(String, NodeKey)>,
    /// What each field the person typed into held when last read from the field itself.
    latest: HashMap<NodeKey, String>,
    /// The fields the person typed into, in the order they first typed into each.
    order: Vec<NodeKey>,
    /// The mouse buttons the person holds down, as the engine's bits.
    held: i64,
    /// The field the person's last key was blanked and read for, while the page showed the
    /// document number it was blanked under (#2295): a key into it while it keeps the focus
    /// needs no new read of the page.
    focus: Option<Focused>,
    /// The read of a page's trees under way for the person's keys, or come and not yet used:
    /// every key that waits for the page waits on this one read (#3043).
    reading: Option<Arc<Reading>>,
}

/// A read of every frame's tree of a page for the person's keys (#3043): its page's target, the
/// lease's epoch and the page's document number when it began, and its answer once it came.
struct Reading {
    target: String,
    epoch: u64,
    shows: u64,
    /// The answer, and when it came.
    answer: Mutex<Option<(Instant, ReadAnswer)>>,
    came: Condvar,
}

/// What a read of a page's trees for the person's keys came to (#3043).
type ReadAnswer = Result<Arc<Vec<FrameAx>>, Refusal>;

impl Reading {
    /// The read's answer is `answer`: every key waiting on it is told.
    fn answered(&self, answer: ReadAnswer) {
        *lock(&self.answer) = Some((Instant::now(), answer));
        self.came.notify_all();
    }
}

/// Whether input lands on what the person saw (#2638), with what judging it read: the page's
/// trees, or the field the last key went to, still focused, for the key to go on with (#2974).
/// A read of the trees says whether an earlier key began it (#3043).
struct Landing {
    lands: bool,
    read: Option<(Vec<FrameAx>, bool)>,
    focused: Option<Focused>,
}

impl Landing {
    /// It does not land.
    fn not() -> Landing {
        Landing {
            lands: false,
            read: None,
            focused: None,
        }
    }
}

/// A field already blanked: its page's target and element, the page's read it was blanked in,
/// and the page's document number then.
#[derive(Clone)]
struct Focused {
    field: (String, NodeKey),
    read: std::sync::Arc<Snapshot>,
    shows: u64,
}

thread_local! {
    /// Who this thread carries a call out for, the agent or purlis performing a login (#2016),
    /// and the epoch of the lease it was made under, if it is carrying one out.
    static CARRYING: Cell<Option<(Driver, u64)>> = const { Cell::new(None) };
}

/// Marks this thread as carrying out a call of `driver`'s under `epoch` until dropped.
pub(super) struct Carrying;

impl Carrying {
    pub(super) fn under(driver: Driver, epoch: u64) -> Self {
        CARRYING.with(|call| call.set(Some((driver, epoch))));
        Carrying
    }
}

impl Drop for Carrying {
    fn drop(&mut self) {
        CARRYING.with(|call| call.set(None));
    }
}

/// The device metrics that give a page `width` by `height` CSS pixels.
pub(super) fn metrics(width: u16, height: u16) -> Value {
    json!({ "width": width, "height": height, "deviceScaleFactor": 0, "mobile": false })
}

/// The smallest and largest page a size may give, in CSS pixels.
const SIZES: std::ops::RangeInclusive<u16> = 200..=4096;

impl Browser {
    /// The epoch of the lease now held.
    pub fn epoch(&self) -> u64 {
        lock(&self.lease).epoch
    }

    /// Who drives the browser now.
    pub fn driver(&self) -> Driver {
        lock(&self.lease).driver
    }

    /// What a frame of the current page shows now: its page and its document.
    pub fn aim(&self) -> Option<Aim> {
        let state = lock(&self.inner.state);
        let current = state.current.clone()?;
        state.aim(&current)
    }

    /// The size the person set the pages to, if they set one.
    pub fn size(&self) -> Option<(u16, u16)> {
        lock(&self.inner.state).size
    }

    /// The person takes the browser over: a new epoch, held by them. A call of the agent's in
    /// flight is cut and answers [`Refusal::Interrupted`], and so is a login purlis is
    /// performing (#2016); every later one is refused until [`Browser::hand_back`]. Answers the
    /// lease's epoch. Taking over a browser the person already drives changes nothing.
    pub fn take_over(&self) -> u64 {
        let epoch = {
            let mut lease = lock(&self.lease);
            if lease.driver != Driver::Person {
                lease.epoch += 1;
                lease.driver = Driver::Person;
            }
            self.inner.person_drives.store(true, Ordering::SeqCst);
            self.inner.purlis_drives.store(false, Ordering::SeqCst);
            lease.epoch
        };
        // A field blanked in an earlier turn is asked about again from the page itself, and
        // the page read again.
        lock(&self.typing).forget_reads();
        // Whatever waits for the page wakes now, and finds itself cut.
        self.inner.changed.notify_all();
        epoch
    }

    /// The person hands the browser back to the agent: what they typed is kept out of every
    /// later answer, every reference dealt before now is void, and the lease is the agent's
    /// under a new epoch, which it answers. Handing back a browser the agent drives changes
    /// nothing.
    pub fn hand_back(&self) -> u64 {
        if self.driver() != Driver::Person {
            return self.epoch();
        }
        self.keep_typing_out();
        lock(&self.typing).forget_reads();
        // What a login the person cut had filled is emptied before the agent has the page.
        self.clear_cut_logins();
        let mut lease = lock(&self.lease);
        if lease.driver == Driver::Person {
            lease.epoch += 1;
            lease.driver = Driver::Agent;
            self.inner.person_drives.store(false, Ordering::SeqCst);
            lock(&self.inner.state).refs.void();
        }
        lease.epoch
    }

    /// Whether the agent holds the lease of `epoch` now.
    pub(super) fn agent_holds(&self, epoch: u64) -> Result<(), Refusal> {
        let lease = lock(&self.lease);
        match lease.driver {
            Driver::Person => Err(Refusal::PersonDriving),
            Driver::Purlis => Err(Refusal::PurlisDriving),
            Driver::Agent if lease.epoch != epoch => Err(Refusal::Interrupted),
            Driver::Agent => Ok(()),
        }
    }

    /// Carries `call` out as the agent's, under the lease of `epoch`: interrupted, whatever it
    /// came to, if that lease ended before it answered.
    pub(super) fn run_at(&self, epoch: u64, call: Call) -> Result<super::Answer, Refusal> {
        self.acting_epoch.store(epoch, Ordering::SeqCst);
        let _carrying = Carrying::under(Driver::Agent, epoch);
        let answer = self.run(call);
        if self.cut() {
            return Err(Refusal::Interrupted);
        }
        answer
    }

    /// Whether the call this thread carries out (the agent's, or purlis's login) has been cut
    /// by a take-over. Never, on a thread that carries out none.
    pub(super) fn cut(&self) -> bool {
        CARRYING.with(Cell::get).is_some_and(|(driver, epoch)| {
            let lease = lock(&self.lease);
            lease.driver != driver || lease.epoch != epoch
        })
    }

    /// Runs `act`, which acts on a page, under the lease's lock: refused, before it runs, when
    /// it is part of a call (the agent's, or purlis's login) the person has cut.
    pub(super) fn gated<T>(&self, act: impl FnOnce() -> T) -> Result<T, Refusal> {
        let lease = lock(&self.lease);
        if let Some((driver, epoch)) = CARRYING.with(Cell::get)
            && (lease.driver != driver || lease.epoch != epoch)
        {
            return Err(Refusal::Interrupted);
        }
        let done = act();
        drop(lease);
        Ok(done)
    }

    /// Waits `within`, or less if the person takes the browser over meanwhile: then the call
    /// is cut.
    pub(super) fn pause(&self, within: Duration) -> Result<(), Refusal> {
        if self.cut() {
            return Err(Refusal::Interrupted);
        }
        let state = lock(&self.inner.state);
        let _ = self
            .inner
            .changed
            .wait_timeout(state, within)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.cut() {
            return Err(Refusal::Interrupted);
        }
        Ok(())
    }

    /// Sets every page of the browser, and every page it opens later, to `width` by `height`
    /// CSS pixels: the person's, while they drive.
    pub fn size_to(&self, width: u16, height: u16) -> Result<(), Refusal> {
        if self.driver() != Driver::Person {
            return Err(Refusal::NotDriving);
        }
        let width = width.clamp(*SIZES.start(), *SIZES.end());
        let height = height.clamp(*SIZES.start(), *SIZES.end());
        let sessions: Vec<String> = {
            let mut state = lock(&self.inner.state);
            state.size = Some((width, height));
            state
                .sessions
                .iter()
                .filter(|(_, kind)| matches!(kind, Session::Page(_)))
                .map(|(session, _)| session.clone())
                .collect()
        };
        for session in sessions {
            self.devtools()
                .call(
                    "Emulation.setDeviceMetricsOverride",
                    metrics(width, height),
                    Some(&session),
                )
                .map_err(refusal)?;
        }
        Ok(())
    }

    /// One event of the person's, sent to the current page if it still shows what the event
    /// was aimed at ([`Refusal::Missed`] if not). Refused unless the person drives. Before a
    /// key that edits, the focused element is blanked: a page that cannot say which element
    /// has the focus is refused [`Refusal::FocusUnclear`], and the key is not sent.
    pub fn person_input(&self, input: &PersonInput) -> Result<(), Refusal> {
        if self.driver() != Driver::Person {
            return Err(Refusal::NotDriving);
        }
        // What the key reads of the page is read by then, or the key is held back (#3043).
        let by = Instant::now() + self.inner.limits.key_reads_within;
        let target = self.target(None)?;
        let (session, moved_on) = {
            let state = lock(&self.inner.state);
            let now = state.aim(&target).ok_or(Refusal::Missed)?;
            if now.page != input.aim().page || now.document < input.aim().document {
                return Err(Refusal::Missed);
            }
            let session = state
                .page(&target)
                .and_then(|page| page.session.clone())
                .ok_or(Refusal::Missed)?;
            (session, now.document != input.aim().document)
        };
        // Some frame of the page loaded another document since the person saw it: the input
        // goes on only where it lands on what they saw (#2638). What was read of the page for
        // that is what the key's field is found in (#2974).
        let mut read = None;
        // The field the landing was judged by, still focused: the one the key goes on into.
        let mut landed_in = None;
        if moved_on {
            let landing = self.lands_on_what_was_seen(&target, &session, input, by)?;
            if !landing.lands {
                return Err(Refusal::Missed);
            }
            read = landing.read;
            landed_in = landing.focused;
        }
        match input {
            PersonInput::Key {
                down,
                key,
                code,
                key_code,
                modifiers,
                ..
            } => {
                let text = (*down).then(|| typed(key, *modifiers)).flatten();
                let edits = *down && edits(key, *modifiers);
                // A key that may move the focus (Tab, an arrow) leaves a read made before it
                // for no later key (#3043).
                if *down && !edits && !is_modifier(key) {
                    lock(&self.typing).reading = None;
                }
                let field = if edits {
                    // The field the last key went to, still focused, is blanked already: only
                    // the document is asked what has the focus (#2295).
                    match landed_in.or_else(|| self.still_focused(&target)) {
                        Some(focused) => Some((focused.field, focused.read)),
                        None => match self
                            .read_for_a_key(&target, read.take(), by)
                            .map(|(read, kept)| (self.blank_focus_in(&target, Some(read)), kept))?
                        {
                            // A read an earlier key began, which the person may have waited
                            // for, is checked against where the focus is now: where it has
                            // moved since, the key waits for a new read (#3043).
                            (Ok((field, _)), true) if !self.focus_is_on(&field) => {
                                return Err(Refusal::PageUnreadInTime);
                            }
                            (Ok((field, read)), _) => {
                                let read = std::sync::Arc::new(read);
                                let shows = lock(&self.inner.state)
                                    .page(&target)
                                    .map_or(0, |page| page.shows);
                                lock(&self.typing).focus = Some(Focused {
                                    field: field.clone(),
                                    read: std::sync::Arc::clone(&read),
                                    shows,
                                });
                                Some((field, read))
                            }
                            (Err(Refusal::NothingFocused), _) => {
                                // Nothing the tree shows has the focus: unless the document has
                                // nothing focused either, the key waits.
                                if self.focus_unseen(&session, &target) {
                                    return Err(Refusal::FocusUnseen);
                                }
                                None
                            }
                            (Err(why), _) => return Err(why),
                        },
                    }
                } else {
                    None
                };
                // A key that edits without typing or deleting (a paste, an undo) may put in text
                // no run holds: what the field held before it is read, to tell what it put in.
                let before = field
                    .as_ref()
                    .filter(|_| text.is_none() && key != "Backspace" && key != "Delete")
                    .map(|(field, _)| self.field_holds(field))
                    .transpose()?;
                let event = key_event(*down, key, code, *key_code, *modifiers, text.as_deref());
                self.person_sends("Input.dispatchKeyEvent", event, &session)?;
                if *down {
                    lock(&self.typing).typed(&target, key, text.as_deref());
                }
                if edits || text.is_some() {
                    lock(&self.inner.state).kept_on(&target);
                }
                if edits {
                    lock(&self.typing).dirty.insert(target.clone());
                }
                if let Some((field, read)) = field {
                    // The field is read again as the browser is handed back, whatever this read
                    // finds.
                    lock(&self.typing).touched.insert(field.clone());
                    // What the field holds now, with this key in it (or what was pasted), is
                    // kept out from this moment. Read again once if it could not be read: the
                    // key is in already.
                    let after = self
                        .field_holds(&field)
                        .or_else(|_| self.field_holds(&field))
                        .map_err(|_| Refusal::FieldUnreadAfterKey)?;
                    let mut held = Kept::default();
                    if let Some(before) = before {
                        // What the key put in is kept as it was put in, like a run, and so is
                        // what the field holds after it: a paste over a selection may share its
                        // ends with what it replaced.
                        let [first, last] = put_in(&before, &after);
                        held.run(&[first, last, after.clone()]);
                    }
                    let (latest, order) = lock(&self.typing).read(&field.1, &after);
                    // A box of a group (a code typed one character a box) is kept with the
                    // rest of its group too, joined, each box the person typed into as they
                    // last read it, this key in it.
                    for (first, joined) in read.groups_holding(&latest, &order) {
                        held.group(&first, &joined);
                    }
                    held.field(&field.1, &[after]);
                    lock(&self.inner.state).keep_out_everywhere(&held);
                }
                Ok(())
            }
            PersonInput::Mouse {
                kind,
                x,
                y,
                button,
                clicks,
                modifiers,
                delta_x,
                delta_y,
                ..
            } => {
                // A press may send what was typed away (a form's button): it is read first.
                if *kind == MouseKind::Pressed {
                    {
                        let mut typing = lock(&self.typing);
                        typing.forget_reads();
                        typing.end_run(&target);
                    }
                    self.keep_typing_out();
                }
                let bit = button_bit(*button);
                let buttons = {
                    let mut typing = lock(&self.typing);
                    match kind {
                        MouseKind::Pressed => typing.held |= bit,
                        MouseKind::Released => typing.held &= !bit,
                        MouseKind::Moved | MouseKind::Wheel => {}
                    }
                    typing.held
                };
                let mut event = json!({
                    "type": match kind {
                        MouseKind::Moved => "mouseMoved",
                        MouseKind::Pressed => "mousePressed",
                        MouseKind::Released => "mouseReleased",
                        MouseKind::Wheel => "mouseWheel",
                    },
                    "x": x,
                    "y": y,
                    "modifiers": modifiers,
                    "buttons": buttons,
                    "button": match button {
                        MouseButton::None => "none",
                        MouseButton::Left => "left",
                        MouseButton::Middle => "middle",
                        MouseButton::Right => "right",
                    },
                });
                if *clicks > 0 {
                    event["clickCount"] = (*clicks).into();
                }
                if *kind == MouseKind::Wheel {
                    event["deltaX"] = (*delta_x).into();
                    event["deltaY"] = (*delta_y).into();
                }
                self.person_sends("Input.dispatchMouseEvent", event, &session)
            }
        }
    }

    /// Sends one event of the person's, under the lease's lock, while they drive.
    fn person_sends(&self, method: &str, event: Value, session: &str) -> Result<(), Refusal> {
        let lease = lock(&self.lease);
        if lease.driver != Driver::Person {
            return Err(Refusal::NotDriving);
        }
        let sent = self.devtools().call(method, event, Some(session));
        drop(lease);
        sent.map(|_| ()).map_err(|why| match why {
            CallError::Engine { message, .. } => Refusal::Engine(message),
            other => refusal(other),
        })
    }

    /// Reads every page and every field the person typed into again, and keeps what they hold
    /// and each run of characters the person typed out of every later answer about every page.
    fn keep_typing_out(&self) {
        let (dirty, runs, touched) = {
            let mut typing = lock(&self.typing);
            // Each run is kept out once: a run still growing is a run a press of the mouse or
            // the hand-back has just ended.
            let runs: Vec<String> = std::mem::take(&mut typing.runs)
                .into_values()
                .flatten()
                .filter(|run| !run.is_empty())
                .collect();
            let touched: Vec<(String, NodeKey)> = typing.touched.iter().cloned().collect();
            (std::mem::take(&mut typing.dirty), runs, touched)
        };
        let mut held = Kept::default();
        held.run(&runs);
        let mut reads = Vec::new();
        for target in dirty {
            if let Ok(read) = self.snapshot_of(&target) {
                reads.push(read);
            }
            if let Some(page) = lock(&self.inner.state).page(&target) {
                held.take_in(&page.scrubbed);
            }
        }
        // Each field is read last, from the field itself: what it holds now is the latest of
        // its values, whatever its page's tree read before. So are the groups of boxes.
        let mut latest = HashMap::new();
        let mut order = Vec::new();
        for field in &touched {
            // A field that cannot be read now keeps what it was last read to hold.
            let Ok(now) = self.field_holds(field) else {
                continue;
            };
            held.field(&field.1, &[&now]);
            (latest, order) = lock(&self.typing).read(&field.1, &now);
        }
        for read in &reads {
            for (first, joined) in read.groups_holding(&latest, &order) {
                held.group(&first, &joined);
            }
        }
        lock(&self.inner.state).keep_out_everywhere(&held);
    }

    /// What `field` holds now, read from the field itself; empty where it is gone (it, or its
    /// frame). Refused [`Refusal::FieldUnread`] where it is there and cannot be read.
    fn field_holds(&self, (target, key): &(String, NodeKey)) -> Result<String, Refusal> {
        let session = {
            let state = lock(&self.inner.state);
            match state
                .page(target)
                .and_then(|page| page.frames.get(&key.frame))
            {
                Some(frame) => frame.session.clone(),
                None => return Ok(String::new()),
            }
        };
        let on = Some(session.as_str());
        // Read in the world the field was resolved into, by its unique id: an element of any
        // other world is refused by the engine.
        let read = self.in_world(&session, &key.frame, |world| {
            let resolved = self.devtools().call(
                "DOM.resolveNode",
                json!({ "backendNodeId": key.backend, "executionContextId": world.id }),
                on,
            )?;
            let Some(object) = resolved.pointer("/object/objectId").and_then(Value::as_str) else {
                return Ok(Value::Null);
            };
            let read = self.devtools().call(
                "Runtime.callFunctionOn",
                json!({
                    "uniqueContextId": world.unique,
                    "functionDeclaration": READ_VALUE,
                    "arguments": [{ "objectId": object }],
                    "returnByValue": true,
                }),
                on,
            );
            let _ =
                self.devtools()
                    .call("Runtime.releaseObject", json!({ "objectId": object }), on);
            read
        });
        match read {
            Ok(read) => read
                .pointer("/result/value")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(Refusal::FieldUnread),
            Err(CallError::Engine { message, .. }) if message.contains("No node") => {
                Ok(String::new())
            }
            Err(_) => Err(Refusal::FieldUnread),
        }
    }

    /// The field the person's last key went to, where it still has the focus of a document
    /// that has the focus, on `target`'s page as it was when the field was blanked (#2295); asked
    /// of the field's own document in purlis's world, never of the page's tree.
    fn still_focused(&self, target: &str) -> Option<Focused> {
        let focused = lock(&self.typing).focus.clone()?;
        let session = {
            let state = lock(&self.inner.state);
            let page = state.page(target)?;
            // The field's frame, or one around it, loaded another document since: asked anew.
            // Another frame of the page moving on leaves it as it is (#2974).
            if focused.field.0 != target
                || state.moved_at(target, focused.shows, &focused.field.1.frame, false)
            {
                return None;
            }
            page.frames.get(&focused.field.1.frame)?.session.clone()
        };
        let key = &focused.field.1;
        let still = self.focused_in(&session, &key.frame) == Some(key.backend);
        if !still {
            lock(&self.typing).focus = None;
        }
        still.then_some(focused)
    }

    /// The engine's id for the element that has the focus in `frame`'s document (held by
    /// `session`), where that document has the focus; `None` where it has not, or cannot say.
    /// Asked in purlis's own world (#2295).
    pub(super) fn focused_in(&self, session: &str, frame: &str) -> Option<i64> {
        let on = Some(session);
        self.in_world(session, frame, |world| {
            let found = self.devtools().call(
                "Runtime.evaluate",
                json!({ "expression": FOCUSED_HERE, "uniqueContextId": world.unique }),
                on,
            )?;
            let Some(object) = found.pointer("/result/objectId").and_then(Value::as_str) else {
                return Ok(Value::Null);
            };
            let described =
                self.devtools()
                    .call("DOM.describeNode", json!({ "objectId": object }), on);
            let _ =
                self.devtools()
                    .call("Runtime.releaseObject", json!({ "objectId": object }), on);
            described
        })
        .ok()?
        .pointer("/node/backendNodeId")
        .and_then(Value::as_i64)
    }

    /// Whether `input` lands on a document of `target`'s page that has not moved on since the
    /// person saw it (#2638). A key lands in the document that has the focus (the page's own
    /// where nothing does, and the document has nothing focused either); a move in the document
    /// under its point. A press, a release or the wheel never goes on while the page has moved
    /// on; and where a frame of the page went since, nothing under a point is known to be what
    /// they saw.
    fn lands_on_what_was_seen(
        &self,
        target: &str,
        session: &str,
        input: &PersonInput,
        by: Instant,
    ) -> Result<Landing, Refusal> {
        let seen = input.aim().document;
        match input {
            PersonInput::Key { .. } => {
                // The field the last key went to, still focused and as it was: the key lands
                // there, and the page's tree is not read (#2974).
                if let Some(focused) = self.still_focused(target) {
                    let lands = !lock(&self.inner.state).moved_at(
                        target,
                        seen,
                        &focused.field.1.frame,
                        false,
                    );
                    return Ok(Landing {
                        lands,
                        read: None,
                        focused: Some(focused),
                    });
                }
                let (trees, kept) = self.trees_for_a_key(target, by)?;
                let snapshot = self.snapshot_from(target, &trees);
                let reached = match snapshot.focused() {
                    // Nothing the tree shows has the focus: unless the document has nothing
                    // focused either, the focus may be in a frame purlis has not read.
                    [] if self.focus_unseen(session, target) => return Ok(Landing::not()),
                    [] => target.to_owned(),
                    [one] => one.frame.clone(),
                    // As a key's field is found (#2295): the one its document names.
                    many => match self.the_one_focused(target, many) {
                        Some(one) => one.frame,
                        None => return Ok(Landing::not()),
                    },
                };
                let lands = !lock(&self.inner.state).moved_at(target, seen, &reached, false);
                Ok(Landing {
                    lands,
                    read: Some((trees, kept)),
                    focused: None,
                })
            }
            // A press, a release or the wheel waits for a view of the page as it is: what a
            // frame that moved on draws may be moved under the point between the check and the
            // event. Only a move goes on.
            PersonInput::Mouse { kind, .. } if *kind != MouseKind::Moved => Ok(Landing::not()),
            PersonInput::Mouse { x, y, .. } => {
                if lock(&self.inner.state)
                    .page(target)
                    .is_none_or(|page| page.gone > seen)
                {
                    return Ok(Landing::not());
                }
                let Some((reached, owner)) = self.frame_under(session, *x, *y) else {
                    return Ok(Landing::not());
                };
                Ok(Landing {
                    lands: !lock(&self.inner.state).moved_at(target, seen, &reached, owner),
                    read: None,
                    focused: None,
                })
            }
        }
    }

    /// The frame whose document is under (`x`, `y`) of the page `session` holds, and whether
    /// what is there is a frame's element, whose inside may be any frame within it; `None`
    /// where the engine cannot say.
    fn frame_under(&self, session: &str, x: f64, y: f64) -> Option<(String, bool)> {
        let at = self
            .devtools()
            .call(
                "DOM.getNodeForLocation",
                // As the engine routes the event: a box the pointer passes through is not hit.
                json!({ "x": x.round(), "y": y.round(), "includeUserAgentShadowDOM": true, "ignorePointerEventsNone": false }),
                Some(session),
            )
            .ok()?;
        let frame = at.get("frameId")?.as_str()?.to_owned();
        let backend = at.get("backendNodeId")?.as_i64()?;
        let node = self
            .devtools()
            .call(
                "DOM.describeNode",
                json!({ "backendNodeId": backend }),
                Some(session),
            )
            .ok()?;
        match node.pointer("/node/frameId").and_then(Value::as_str) {
            // A frame's element: what is under the point is its document, or one inside it.
            Some(inside) => Some((inside.to_owned(), true)),
            // An element that holds a document and names none: what is under it is not known.
            None if node
                .pointer("/node/nodeName")
                .and_then(Value::as_str)
                .is_some_and(|name| {
                    ["IFRAME", "FRAME", "OBJECT", "EMBED", "FENCEDFRAME"]
                        .contains(&name.to_ascii_uppercase().as_str())
                }) =>
            {
                None
            }
            None => Some((frame, false)),
        }
    }

    /// The tree around the element that has the focus of the current page's own document, for
    /// the person's first key into a field of the page (#2967): the element, the nodes around
    /// it up to the root, its siblings and its children, never the whole page's tree. Only where
    /// the person has typed into no field of the page yet, since a code typed one character a
    /// box is kept out whole from the whole tree (#2144); and only where the element is in the
    /// page's own document, the tree marks it focused, and it holds no frame. `None` otherwise.
    fn first_field_read(&self, target: &str, by: Instant) -> Option<Vec<FrameAx>> {
        if lock(&self.typing)
            .touched
            .iter()
            .any(|(page, _)| page == target)
        {
            return None;
        }
        let (session, name, url) = {
            let state = lock(&self.inner.state);
            let page = state.page(target)?;
            let frame = page.frames.get(target)?;
            (page.session.clone()?, frame.name.clone(), frame.url.clone())
        };
        let backend = self.focused_in(&session, target)?;
        let around = self
            .devtools()
            .call_within(
                "Accessibility.getPartialAXTree",
                json!({ "backendNodeId": backend, "fetchRelatives": true }),
                Some(&session),
                by.saturating_duration_since(Instant::now()),
            )
            .ok()?;
        let nodes = around.get("nodes")?.clone();
        let own = nodes
            .as_array()?
            .iter()
            .find(|node| node["backendDOMNodeId"] == backend)?;
        let role = own
            .pointer("/role/value")
            .and_then(Value::as_str)
            .unwrap_or("");
        let focused = own["properties"].as_array().into_iter().flatten().any(|p| {
            p["name"] == "focused" && p.pointer("/value/value") == Some(&Value::Bool(true))
        });
        if !focused || !self.is_a_field(&session, backend, role) {
            return None;
        }
        Some(vec![FrameAx {
            id: target.to_owned(),
            name,
            url,
            parent: None,
            owner: None,
            nodes,
        }])
    }

    /// Whether the element `backend` of `session`'s document is a field a person types into
    /// (#2967): a text box, search box, combo box or spin button by its role, or an input, a text
    /// area or an editable element by its tag; and never an element that holds a document of its
    /// own (a frame, an object, an embed).
    fn is_a_field(&self, session: &str, backend: i64, role: &str) -> bool {
        let Ok(described) = self.devtools().call(
            "DOM.describeNode",
            json!({ "backendNodeId": backend }),
            Some(session),
        ) else {
            return false;
        };
        let node = &described["node"];
        if !node["frameId"].is_null() || !node["contentDocument"].is_null() {
            return false;
        }
        let name = node["nodeName"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let editable = node["attributes"]
            .as_array()
            .into_iter()
            .flat_map(|pairs| pairs.chunks(2))
            .any(|pair| {
                pair[0] == "contenteditable" && pair.get(1).and_then(Value::as_str) != Some("false")
            });
        ["textbox", "searchbox", "combobox", "spinbutton"].contains(&role)
            || ["INPUT", "TEXTAREA"].contains(&name.as_str())
            || editable
    }

    /// What a key into `target`'s page blanks its field from (#3043): `landed`, the read its
    /// landing was judged by, where there is one; else the tree around the page's first field
    /// (#2967); else every frame's whole tree, read by `by`. With whether an earlier key began
    /// the read (#3043).
    fn read_for_a_key(
        &self,
        target: &str,
        landed: Option<(Vec<FrameAx>, bool)>,
        by: Instant,
    ) -> Result<(Vec<FrameAx>, bool), Refusal> {
        if let Some(landed) = landed {
            return Ok(landed);
        }
        match self.first_field_read(target, by) {
            Some(read) => Ok((read, false)),
            None => self.trees_for_a_key(target, by),
        }
    }

    /// Whether `field` has the focus of its own document now, that document having the focus
    /// of the page, and is no element that holds a document (a frame's): or the element that
    /// has it holds `field`, as a shadow host holds its field (#2972). A field a read of the
    /// whole page found is checked so before its key goes (#3043): a script may have moved the
    /// focus, to another frame or out of one, while the person waited for the read.
    fn focus_is_on(&self, (target, key): &(String, NodeKey)) -> bool {
        let session = lock(&self.inner.state)
            .page(target)
            .and_then(|page| page.frames.get(&key.frame))
            .map(|frame| frame.session.clone());
        let Some(session) = session else {
            return false;
        };
        match self.focused_in(&session, &key.frame) {
            Some(now) if now == key.backend => !self.holds_a_document(&session, now),
            Some(now) => self.holds(&session, now, key.backend),
            None => false,
        }
    }

    /// Whether the element `backend` of `session`'s document holds a document of its own (a
    /// frame, an object, an embed); taken to, where the engine cannot say.
    fn holds_a_document(&self, session: &str, backend: i64) -> bool {
        self.devtools()
            .call(
                "DOM.describeNode",
                json!({ "backendNodeId": backend }),
                Some(session),
            )
            .map_or(true, |described| {
                !described["node"]["frameId"].is_null()
                    || !described["node"]["contentDocument"].is_null()
            })
    }

    /// Every frame's tree of `target`'s page for the person's key, read by `by` (#3043). The
    /// read runs on past the key, within [`Limits::tree_reads_within`](crate::drive::Limits):
    /// a key it outlives is held back, [`Refusal::PageUnreadInTime`]. Every key into the page
    /// waits on the same one read, under way or come and not yet used, where the lease and the
    /// page's document are still as they were and nothing that may move the focus reached the
    /// page since; once used or failed, the next key reads anew. A read that came more than
    /// [`Limits::tree_read_kept_for`](crate::drive::Limits) before is read anew too. A read that
    /// fails in time holds the key back as well: never [`Refusal::Unknown`], since the key was
    /// not sent. Answers whether an earlier key began the read: its field is then checked
    /// against where the focus is now before the key goes.
    fn trees_for_a_key(&self, target: &str, by: Instant) -> Result<(Vec<FrameAx>, bool), Refusal> {
        let epoch = self.epoch();
        let shows = lock(&self.inner.state)
            .page(target)
            .map_or(0, |page| page.shows);
        let (reading, begun) = {
            let mut typing = lock(&self.typing);
            match &typing.reading {
                // Under way, or come and not used yet: every key waits on the one read.
                Some(reading)
                    if reading.target == target
                        && reading.epoch == epoch
                        && reading.shows == shows =>
                {
                    (Arc::clone(reading), false)
                }
                _ => {
                    let reading = Arc::new(Reading {
                        target: target.to_owned(),
                        epoch,
                        shows,
                        answer: Mutex::new(None),
                        came: Condvar::new(),
                    });
                    typing.reading = Some(Arc::clone(&reading));
                    (reading, true)
                }
            }
        };
        if begun {
            self.begin_reading(target, &reading);
        }
        let (came, answer) = {
            let mut answer = lock(&reading.answer);
            loop {
                if let Some((came, answer)) = answer.as_ref() {
                    break (*came, answer.clone());
                }
                let left = by.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(Refusal::PageUnreadInTime);
                }
                answer = reading
                    .came
                    .wait_timeout(answer, left)
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .0;
            }
        };
        // Used, or failed: the next key that needs the page reads it anew.
        {
            let mut typing = lock(&self.typing);
            if typing
                .reading
                .as_ref()
                .is_some_and(|kept| Arc::ptr_eq(kept, &reading))
            {
                typing.reading = None;
            }
        }
        let trees = answer?;
        // A read that came long before the key that would use it is not the page now.
        if came.elapsed() > self.inner.limits.tree_read_kept_for {
            return Err(Refusal::PageUnreadInTime);
        }
        Ok((trees.as_ref().clone(), !begun))
    }

    /// Begins `reading` of every frame's tree of `target`'s page, on a thread of its own, each
    /// frame's within [`Limits::tree_reads_within`](crate::drive::Limits) (#3043). A read past
    /// that limit, or one that cannot begin, holds the keys waiting on it back.
    fn begin_reading(&self, target: &str, reading: &Arc<Reading>) {
        let frames = match self.frames_to_read(target) {
            Ok(frames) => frames,
            Err(why) => return reading.answered(Err(why)),
        };
        let devtools = Arc::clone(&self.inner.devtools);
        let within = self.inner.limits.tree_reads_within;
        let told = Arc::clone(reading);
        let begun = std::thread::Builder::new()
            .name("purlis-tree-read".into())
            .spawn(move || {
                told.answered(
                    read_trees(&*devtools, frames, within)
                        .map(Arc::new)
                        .map_err(|why| match why {
                            CallError::TimedOut => Refusal::PageUnreadInTime,
                            other => refusal(other),
                        }),
                );
            });
        if begun.is_err() {
            reading.answered(Err(Refusal::PageUnreadInTime));
        }
    }

    /// Whether the document of `frame` on `session` has an element focused (or one inside a
    /// frame), though its tree shows none: then a key typed now would go where purlis cannot
    /// blank it.
    fn focus_unseen(&self, session: &str, frame: &str) -> bool {
        self.in_world(session, frame, |world| {
            self.devtools().call(
                "Runtime.evaluate",
                json!({
                    "expression": ACTIVE_ELEMENT,
                    "uniqueContextId": world.unique,
                    "returnByValue": true,
                }),
                Some(session),
            )
        })
        .ok()
        .and_then(|read| {
            read.pointer("/result/value")
                .and_then(Value::as_str)
                .map(|tag| !tag.is_empty())
        })
        // A document that cannot say holds the key back too.
        .unwrap_or(true)
    }
}

impl Typing {
    /// Forgets the field blanked last and any read of a page for the person's keys: the page is
    /// asked anew at the next key.
    fn forget_reads(&mut self) {
        self.focus = None;
        self.reading = None;
    }

    /// The field `key` holds `now`, as read from the field itself: answers what every field
    /// typed into held when last read, and the order they were first typed into.
    fn read(&mut self, key: &NodeKey, now: &str) -> (HashMap<NodeKey, String>, Vec<NodeKey>) {
        self.latest.insert(key.clone(), now.to_owned());
        if !self.order.contains(key) {
            self.order.push(key.clone());
        }
        (self.latest.clone(), self.order.clone())
    }

    /// The person pressed `key` on `target`'s page, typing `text`, if it types any.
    fn typed(&mut self, target: &str, key: &str, text: Option<&str>) {
        let runs = self.runs.entry(target.to_owned()).or_default();
        match text {
            Some(text) if text != "\r" => match runs.last_mut() {
                Some(run) => run.push_str(text),
                None => runs.push(text.to_owned()),
            },
            _ if key == "Backspace" => {
                if let Some(run) = runs.last_mut() {
                    run.pop();
                }
            }
            _ => self.end_run(target),
        }
    }

    /// The run of characters being typed on `target`'s page ends here.
    fn end_run(&mut self, target: &str) {
        let runs = self.runs.entry(target.to_owned()).or_default();
        if runs.last().is_some_and(|run| !run.is_empty()) {
            runs.push(String::new());
        }
    }
}

/// What `after` holds that `before` did not: `after` without the beginning and the end the two
/// share, matched from the beginning first and from the end first (they differ where what was
/// put in begins or ends like what is next to it). Empty when nothing was put in (a selection
/// alone, or a deletion).
fn put_in(before: &str, after: &str) -> [String; 2] {
    let before: Vec<char> = before.chars().collect();
    let after: Vec<char> = after.chars().collect();
    let shared = |a: &mut dyn Iterator<Item = &char>, b: &mut dyn Iterator<Item = &char>| {
        a.zip(b).take_while(|(a, b)| a == b).count()
    };
    let start = shared(&mut before.iter(), &mut after.iter());
    let end = shared(
        &mut before[start..].iter().rev(),
        &mut after[start..].iter().rev(),
    );
    let first: String = after[start..after.len() - end].iter().collect();
    let end = shared(&mut before.iter().rev(), &mut after.iter().rev());
    let start = shared(
        &mut before[..before.len() - end].iter(),
        &mut after[..after.len() - end].iter(),
    );
    let last: String = after[start..after.len() - end].iter().collect();
    [first, last]
}

/// Whether the keys held make a chord rather than a character: Meta, or Control without Alt
/// (Control with Alt is AltGr, which types; Alt alone is Option, which types too).
fn commands(modifiers: Modifiers) -> bool {
    modifiers & META != 0 || (modifiers & CONTROL != 0 && modifiers & ALT == 0)
}

/// The text a key types, if it types any: a key named by one character, or Enter, held with no
/// chord ([`commands`]): with Alt or AltGr, the character the key named (`@`, `€`).
fn typed(key: &str, modifiers: Modifiers) -> Option<String> {
    if commands(modifiers) {
        return None;
    }
    if key == "Enter" {
        return Some("\r".into());
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(c.to_string()),
        _ => None,
    }
}

/// Whether a key pressed may change what a field holds: one that types, a deletion, a paste
/// (Shift+Insert too), or a chord (an undo).
fn edits(key: &str, modifiers: Modifiers) -> bool {
    typed(key, modifiers).is_some()
        || key == "Backspace"
        || key == "Delete"
        || (key == "Insert" && modifiers & SHIFT != 0)
        || commands(modifiers)
}

/// The engine's key event for one key of the person's.
fn key_event(
    down: bool,
    key: &str,
    code: &str,
    key_code: u32,
    modifiers: Modifiers,
    text: Option<&str>,
) -> Value {
    let kind = match (down, text) {
        (false, _) => "keyUp",
        (true, Some(_)) => "keyDown",
        (true, None) => "rawKeyDown",
    };
    let mut event = json!({
        "type": kind,
        "key": key,
        "code": code,
        "windowsVirtualKeyCode": key_code,
        "modifiers": modifiers,
    });
    if let Some(text) = text {
        event["text"] = text.into();
        event["unmodifiedText"] = text.into();
    }
    if down
        && commands(modifiers)
        && let Some(command) = command_of(key)
    {
        event["commands"] = json!([command]);
    }
    event
}

/// The engine's bit for a held mouse button.
fn button_bit(button: MouseButton) -> i64 {
    match button {
        MouseButton::None => 0,
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 4,
    }
}

#[cfg(test)]
mod put_in_tests {
    use super::put_in;

    #[test]
    fn what_was_put_in_is_found_from_either_end() {
        assert_eq!(put_in("ab", "abQb"), ["Qb".to_owned(), "bQ".to_owned()]);
        assert_eq!(put_in("hello", "hello"), [String::new(), String::new()]);
        assert_eq!(
            put_in("", "zq7kp"),
            ["zq7kp".to_owned(), "zq7kp".to_owned()]
        );
    }
}

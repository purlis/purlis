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
use std::time::Duration;

use serde_json::{Value, json};

use super::super::kept::Kept;
use super::super::keys::command_of;
use super::super::tree::NodeKey;
use super::{ACTIVE_ELEMENT, READ_VALUE};
use super::{Browser, Session, refusal};
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
    /// The mouse buttons the person holds down, as the engine's bits.
    held: i64,
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
        let target = self.target(None)?;
        let session = {
            let state = lock(&self.inner.state);
            if state.aim(&target).as_ref() != Some(input.aim()) {
                return Err(Refusal::Missed);
            }
            state
                .page(&target)
                .and_then(|page| page.session.clone())
                .ok_or(Refusal::Missed)?
        };
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
                let field = if edits {
                    match self.blank_what_has_focus(None) {
                        Ok(field) => Some(field),
                        Err(Refusal::NothingFocused) => {
                            // Nothing the tree shows has the focus: unless the document has
                            // nothing focused either, the key waits.
                            if self.focus_unseen(&session) {
                                return Err(Refusal::FocusUnseen);
                            }
                            None
                        }
                        Err(why) => return Err(why),
                    }
                } else {
                    None
                };
                // A key that edits without typing or deleting (a paste, an undo) may put in text
                // no run holds: what the field held before it is read, to tell what it put in.
                let before = field
                    .as_ref()
                    .filter(|_| text.is_none() && key != "Backspace" && key != "Delete")
                    .map(|field| self.field_holds(field));
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
                if let Some(field) = field {
                    // What the field holds now, with this key in it (or what was pasted), is
                    // kept out from this moment.
                    let after = self.field_holds(&field);
                    let mut held = Kept::default();
                    if let Some(before) = before {
                        // What the key put in is kept as it was put in, like a run, and so is
                        // what the field holds after it: a paste over a selection may share its
                        // ends with what it replaced.
                        let [first, last] = put_in(&before, &after);
                        held.run(&[first, last, after.clone()]);
                    }
                    held.field(&field.1, &[after]);
                    lock(&self.inner.state).keep_out_everywhere(&held);
                    lock(&self.typing).touched.insert(field);
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
                    lock(&self.typing).end_run(&target);
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
        for target in dirty {
            let _ = self.snapshot_of(&target);
            if let Some(page) = lock(&self.inner.state).page(&target) {
                held.take_in(&page.scrubbed);
            }
        }
        // Each field is read last, from the field itself: what it holds now is the latest of
        // its values, whatever its page's tree read before.
        for field in &touched {
            held.field(&field.1, &[self.field_holds(field)]);
        }
        lock(&self.inner.state).keep_out_everywhere(&held);
    }

    /// What `field` holds now, read from the field itself; empty where it cannot be read (it is
    /// gone, or its page moved on).
    fn field_holds(&self, (target, key): &(String, NodeKey)) -> String {
        let session = {
            let state = lock(&self.inner.state);
            match state
                .page(target)
                .and_then(|page| page.frames.get(&key.frame))
            {
                Some(frame) => frame.session.clone(),
                None => return String::new(),
            }
        };
        let on = Some(session.as_str());
        let Ok(resolved) = self.devtools().call(
            "DOM.resolveNode",
            json!({ "backendNodeId": key.backend }),
            on,
        ) else {
            return String::new();
        };
        let Some(object) = resolved.pointer("/object/objectId").and_then(Value::as_str) else {
            return String::new();
        };
        let read = self.devtools().call(
            "Runtime.callFunctionOn",
            json!({ "objectId": object, "functionDeclaration": READ_VALUE, "returnByValue": true }),
            on,
        );
        let _ = self
            .devtools()
            .call("Runtime.releaseObject", json!({ "objectId": object }), on);
        read.ok()
            .and_then(|read| {
                read.pointer("/result/value")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }

    /// Whether the document on `session` has an element focused (or one inside a frame), though
    /// its tree shows none: then a key typed now would go where purlis cannot blank it.
    fn focus_unseen(&self, session: &str) -> bool {
        self.devtools()
            .call(
                "Runtime.evaluate",
                json!({ "expression": ACTIVE_ELEMENT, "returnByValue": true }),
                Some(session),
            )
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

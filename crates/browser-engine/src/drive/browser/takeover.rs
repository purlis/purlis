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
//! where it was aimed: at the page and document a frame showed them. Before a key that edits
//! reaches the page, the element that has the focus is blanked, so what the person types is in
//! no later snapshot, find or screenshot; and the page is read again after it, so the field's
//! latest content is kept out of every later answer about its page too. And each run of
//! characters the person typed is kept out of later answers about its page as typed, wherever
//! the page shows it, whatever became of the field.
//!
//! **The hand-back is explicit** ([`Browser::hand_back`]): nothing returns the lease to the
//! agent by itself. It voids every reference dealt before it, so the agent's next act starts
//! from a new snapshot.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::{Value, json};

use super::super::keys::command_of;
use super::{Browser, Session, refusal};
use crate::cdp::CallError;
use crate::drive::{
    Aim, Call, Driver, Modifiers, MouseButton, MouseKind, PersonInput, Refusal, lock,
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
    /// The mouse buttons the person holds down, as the engine's bits.
    held: i64,
}

thread_local! {
    /// The epoch of the agent's call this thread is carrying out, if it is carrying one out.
    static AGENT_CALL: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Marks this thread as carrying out an agent's call under `epoch` until dropped.
struct Carrying;

impl Carrying {
    fn under(epoch: u64) -> Self {
        AGENT_CALL.with(|call| call.set(Some(epoch)));
        Carrying
    }
}

impl Drop for Carrying {
    fn drop(&mut self) {
        AGENT_CALL.with(|call| call.set(None));
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
    /// flight is cut and answers [`Refusal::Interrupted`]; every later one is refused until
    /// [`Browser::hand_back`]. Answers the lease's epoch. Taking over a browser the person
    /// already drives changes nothing.
    pub fn take_over(&self) -> u64 {
        let epoch = {
            let mut lease = lock(&self.lease);
            if lease.driver != Driver::Person {
                lease.epoch += 1;
                lease.driver = Driver::Person;
            }
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
        if self.driver() == Driver::Agent {
            return self.epoch();
        }
        self.keep_typing_out();
        let mut lease = lock(&self.lease);
        if lease.driver == Driver::Person {
            lease.epoch += 1;
            lease.driver = Driver::Agent;
            lock(&self.inner.state).refs.void();
        }
        lease.epoch
    }

    /// Whether the agent holds the lease of `epoch` now.
    pub(super) fn agent_holds(&self, epoch: u64) -> Result<(), Refusal> {
        let lease = lock(&self.lease);
        match lease.driver {
            Driver::Person => Err(Refusal::PersonDriving),
            Driver::Agent if lease.epoch != epoch => Err(Refusal::Interrupted),
            Driver::Agent => Ok(()),
        }
    }

    /// Carries `call` out as the agent's, under the lease of `epoch`: interrupted, whatever it
    /// came to, if that lease ended before it answered.
    pub(super) fn run_at(&self, epoch: u64, call: Call) -> Result<super::Answer, Refusal> {
        self.acting_epoch.store(epoch, Ordering::SeqCst);
        let _carrying = Carrying::under(epoch);
        let answer = self.run(call);
        if self.cut() {
            return Err(Refusal::Interrupted);
        }
        answer
    }

    /// Whether the agent's call this thread carries out has been cut by a take-over. Never, on
    /// a thread that carries out none.
    pub(super) fn cut(&self) -> bool {
        AGENT_CALL.with(Cell::get).is_some_and(|epoch| {
            let lease = lock(&self.lease);
            lease.driver != Driver::Agent || lease.epoch != epoch
        })
    }

    /// Runs `act`, which acts on a page, under the lease's lock: refused, before it runs, when
    /// it is part of an agent's call the person has cut.
    pub(super) fn gated<T>(&self, act: impl FnOnce() -> T) -> Result<T, Refusal> {
        let lease = lock(&self.lease);
        if let Some(epoch) = AGENT_CALL.with(Cell::get)
            && (lease.driver != Driver::Agent || lease.epoch != epoch)
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
                if edits {
                    match self.blank_focused(None) {
                        Ok(()) | Err(Refusal::NothingFocused) => {}
                        Err(why) => return Err(why),
                    }
                }
                let event = key_event(*down, key, code, *key_code, *modifiers, text.as_deref());
                self.person_sends("Input.dispatchKeyEvent", event, &session)?;
                if *down {
                    lock(&self.typing).typed(&target, key, text.as_deref());
                }
                if edits {
                    lock(&self.typing).dirty.insert(target.clone());
                    // Read the page again now, so the field's content with this key in it is
                    // kept out from this moment.
                    let _ = self.snapshot_of(&target);
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

    /// Reads every page the person typed into again, and keeps each run of characters they
    /// typed out of every later answer about its page.
    fn keep_typing_out(&self) {
        let (dirty, runs) = {
            let mut typing = lock(&self.typing);
            // Each run is kept out once: a run still growing is a run a press of the mouse or
            // the hand-back has just ended.
            let runs: Vec<(String, Vec<String>)> = std::mem::take(&mut typing.runs)
                .into_iter()
                .map(|(target, runs)| {
                    let typed = runs.into_iter().filter(|run| !run.is_empty());
                    (target, typed.collect())
                })
                .collect();
            (std::mem::take(&mut typing.dirty), runs)
        };
        for target in dirty {
            let _ = self.snapshot_of(&target);
        }
        let mut state = lock(&self.inner.state);
        for (target, runs) in runs {
            if let Some(page) = state.page_mut(&target) {
                page.keep_out(&runs);
            }
        }
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

/// The text a key types, if it types any: a key named by one character, or Enter, held with
/// no Control, Meta or Alt.
fn typed(key: &str, modifiers: Modifiers) -> Option<String> {
    if modifiers & (1 | 2 | 4) != 0 {
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

/// Whether a key pressed may change what a field holds: one that types, a deletion, or a
/// chord with Control or Meta (a paste, an undo).
fn edits(key: &str, modifiers: Modifiers) -> bool {
    typed(key, modifiers).is_some()
        || key == "Backspace"
        || key == "Delete"
        || modifiers & (2 | 4) != 0
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
        && modifiers & (2 | 4) != 0
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

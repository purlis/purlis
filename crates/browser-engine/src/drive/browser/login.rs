// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A login purlis performs (#2016, spec #2008): the third driver.
//!
//! **purlis takes the lease** ([`Browser::log_in`]), as the person does when they take a
//! browser over: a call of the agent's in flight is cut, and every call is refused until the
//! login has answered. The person may still take the browser over meanwhile, which cuts the
//! login, and the browser stays theirs.
//!
//! **What it fills is kept out before it fills anything.** The values themselves, as the vault
//! gave them, are kept out of every answer about every page of the browser, a page opened
//! later included, for as long as the browser lives, as what the person types is: never what
//! the page's tree reads for the field, which for a password field is bullets. Each field is
//! blanked before its value goes in, so a show-password toggle shows nothing either.
//!
//! **It fills only on the declared origin, in a page of its own**: a page purlis opens itself,
//! which no other page opened, on the origin the engine records for its document, never what
//! the page says. Right before each value goes in and right after, the page must still show the
//! same document with the field focused in it; where it does not, purlis ends the whole browser
//! before the agent has anything of it. A page on any other origin has nothing filled, and the
//! login fails with a sentence. Only the page's own document is searched for the form: a field
//! inside a frame is never filled.
//!
//! **Its scripts run in purlis's own world** (#2138, #2613): the fields are found by scripts
//! run in purlis's world in the page's document, and focused, asked and cleared through that
//! world's own objects, so nothing the page's scripts redefine steers which field gets a value.
//! A world the engine no longer has, or cannot make, fills nothing more: the login fails with a
//! sentence and the browser is handed back, or, where a value already went in, is ended.
//!
//! **The browser holds the login from the first fill**: `bind`, the caller's, holds what the
//! browser reaches to the declared origin before the first value goes in. Where it cannot, no
//! value goes in.
//!
//! **Then it submits, and waits for the password field to go.** A login that does not go
//! through within [`crate::drive::Limits::login_within`] has every field it filled cleared,
//! and fails; one the person cut by taking the browser over has them cleared as they hand it
//! back. Either way the browser is handed back to the agent with a new lease, so every
//! reference dealt before is void. A password field that went is all purlis can tell: it says
//! so, and never that the person is signed in, since a page may ask for a second step. No
//! sentence it answers carries a value.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::kept::Kept;
use super::super::keys::key_events;
use super::super::tree::NodeKey;
use super::Browser;
use super::takeover::Carrying;
use crate::drive::{Answer, Content, Driver, Login, Refusal, lock};

/// How long a login waits for its form to show, and then for the password field to go, unless
/// the browser's limits say otherwise ([`crate::drive::Limits::login_within`]).
pub const LOGIN_WAIT: Duration = Duration::from_secs(30);

/// How often a login looks at its page again while it waits.
const LOOK_AGAIN: Duration = Duration::from_millis(250);

/// The script that finds the page's first shown password field: a constant.
pub(crate) const PASSWORD_FIELD: &str = r#"(() => {
  const shown = (e) => e.getClientRects().length > 0 && getComputedStyle(e).visibility !== 'hidden';
  for (const e of document.querySelectorAll('input[type="password"]')) if (shown(e)) return e;
  return null;
})()"#;

/// A shown field that says it's for a user name: what a login a chat chose is held to where the
/// page shows no password field beside it (#4595).
pub(crate) const SAYS_IT_IS_A_NAME: &str = r#"(() => {
  const shown = (e) => e.getClientRects().length > 0 && getComputedStyle(e).visibility !== 'hidden';
  for (const e of document.querySelectorAll('input[autocomplete~="username"], input[autocomplete~="email"]'))
    if (shown(e)) return e;
  return null;
})()"#;

/// The script that finds the page's first shown user-name field: a constant.
pub(crate) const NAME_FIELD: &str = r#"(() => {
  const shown = (e) => e.getClientRects().length > 0 && getComputedStyle(e).visibility !== 'hidden';
  const named = ['input[autocomplete~="username"]', 'input[autocomplete~="email"]',
    'input[type="email"]', 'input[name*="user" i]', 'input[name*="login" i]',
    'input[name*="email" i]', 'input[id*="user" i]', 'input[id*="email" i]', 'input[type="text"]'];
  for (const one of named) for (const e of document.querySelectorAll(one)) if (shown(e)) return e;
  return null;
})()"#;

/// The function that gives a field the focus and selects what it holds, run on the field: a
/// constant. What is typed next replaces it.
pub(crate) const FOCUS_FIELD: &str = r"function () {
  this.focus();
  if (typeof this.select === 'function') this.select();
}";

/// The function that says whether the field it runs on has the focus in a document that has
/// the focus: a constant.
pub(crate) const FIELD_HAS_FOCUS: &str = r"function () {
  return this.isConnected && this === document.activeElement && document.hasFocus();
}";

/// The function that says how long what the field it runs on holds is, in UTF-16 code units, as
/// a page counts it: a constant.
pub(crate) const FIELD_LENGTH: &str = r"function () {
  return typeof this.value === 'string' ? this.value.length : -1;
}";

/// The function that empties a field purlis filled, given the field as it is in purlis's own
/// world: a constant.
pub(crate) const CLEAR_FIELD: &str = r"function (field) {
  const own = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(field), 'value');
  if (own && own.set) own.set.call(field, ''); else field.value = '';
  field.dispatchEvent(new Event('input', { bubbles: true }));
  field.dispatchEvent(new Event('change', { bubbles: true }));
}";

/// **How a login ended**: who drives the browser now and under which epoch, and what the login
/// came to.
#[derive(Debug)]
pub struct LoginEnded {
    /// The agent, handed the browser back; or the person, who took it over meanwhile.
    pub driver: Driver,
    pub epoch: u64,
    pub outcome: Result<Answer, Refusal>,
}

/// A field of the login's page: the engine's handle on it, and the element it is.
struct Found {
    object: String,
    key: NodeKey,
}

/// **The origin of `address`** as a page's `URL.origin` spells it: the scheme and host in lower
/// case, and the port only where it is not the scheme's own. None for an address of no web
/// scheme or no host.
pub fn origin_of(address: &str) -> Option<String> {
    let (scheme, rest) = address.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let default = match scheme.as_str() {
        "https" => "443",
        "http" => "80",
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let authority = authority.to_ascii_lowercase();
    let (host, port) = if authority.starts_with('[') {
        let end = authority.find(']')? + 1;
        let (host, after) = authority.split_at(end);
        (host, after.strip_prefix(':'))
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority.as_str(), None),
        }
    };
    if host.is_empty() {
        return None;
    }
    Some(
        match port.filter(|port| *port != default && !port.is_empty()) {
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => format!("{scheme}://{host}"),
        },
    )
}

impl Browser {
    /// **purlis performs `login`** (module docs): it takes the lease from the agent, goes to the
    /// declared address, fills only while the page's document is on the declared origin, with
    /// `bind` told just before the first value goes in, submits, waits for the password field to
    /// go, clears what it filled where the login did not go through, and hands the browser back
    /// to the agent. Refused while the person drives, or while another login runs.
    pub fn log_in(
        &self,
        login: &Login,
        bind: &dyn Fn() -> Result<(), String>,
    ) -> Result<LoginEnded, Refusal> {
        let epoch = {
            let mut lease = lock(&self.lease);
            match lease.driver {
                Driver::Person => return Err(Refusal::PersonDriving),
                Driver::Purlis => return Err(Refusal::PurlisDriving),
                Driver::Agent => {}
            }
            lease.epoch += 1;
            lease.driver = Driver::Purlis;
            self.inner
                .purlis_drives
                .store(true, std::sync::atomic::Ordering::SeqCst);
            lease.epoch
        };
        // No read the agent's calls left is kept past a change of hands (#3277).
        lock(&self.reading).forget();
        // A call of the agent's waiting on its page wakes now, and finds itself cut.
        self.inner.changed.notify_all();
        let outcome = {
            let _carrying = Carrying::under(Driver::Purlis, epoch);
            // Once the agent's call in flight has seen it is cut.
            let _one = lock(&self.acting);
            let outcome = self.logging_in(login, bind);
            if self.cut() {
                Err(Refusal::Interrupted)
            } else {
                outcome
            }
        };
        lock(&self.reading).forget();
        let mut lease = lock(&self.lease);
        self.inner
            .purlis_drives
            .store(false, std::sync::atomic::Ordering::SeqCst);
        if lease.driver == Driver::Purlis && lease.epoch == epoch {
            lease.epoch += 1;
            lease.driver = Driver::Agent;
            lock(&self.inner.state).refs.void();
        }
        Ok(LoginEnded {
            driver: lease.driver,
            epoch: lease.epoch,
            outcome,
        })
    }

    /// The login itself, under purlis's lease.
    fn logging_in(
        &self,
        login: &Login,
        bind: &dyn Fn() -> Result<(), String>,
    ) -> Result<Answer, Refusal> {
        let origin = login.origin.as_str();
        // What it fills is kept out before anything else, for the browser's life.
        let mut held = Kept::default();
        let values: Vec<&str> = login
            .user
            .iter()
            .map(|user| user.as_str())
            .chain(std::iter::once(login.password.as_str()))
            .collect();
        held.run(&values);
        lock(&self.inner.state).keep_out_everywhere(&held);
        let target = self.go_to(login)?;
        let within = self.inner.limits.login_within;
        let deadline = Instant::now() + within;
        // Whether the page shows its form: unknown (`None`) while it cannot be read.
        let shows_form = |this: &Self| -> Result<Option<bool>, Refusal> {
            if !this.on_origin(&target, origin) {
                return Ok(Some(false));
            }
            let password = this.shows(&target, PASSWORD_FIELD)?;
            if password == Some(true) || login.user.is_none() {
                return Ok(password);
            }
            Ok(match this.shows(&target, NAME_FIELD)? {
                Some(true) => Some(true),
                name => password.and(name),
            })
        };
        // Opened from kept logins, a page that asks for nothing is signed in already: twice
        // read, so a page between documents is not taken for one (#2021), and never a page
        // that cannot be read.
        if login.only_if_asked && shows_form(self)? == Some(false) {
            self.pause(LOOK_AGAIN)?;
            if shows_form(self)? == Some(false) {
                let shown = lock(&self.inner.state)
                    .current
                    .clone()
                    .unwrap_or_else(|| target.clone());
                let mut answer = self.answer_now(&shown, Content::Nothing);
                answer.note = Some(format!(
                    "{origin} asks for no password: this browser opened from its kept logins, \
                     and purlis filled nothing"
                ));
                return Ok(answer);
            }
        }
        loop {
            let shown = shows_form(self)?;
            if shown == Some(true) {
                break;
            }
            if Instant::now() >= deadline {
                return Err(if shown.is_none() {
                    not_logged_in(format!(
                        "purlis could not read the login page on {origin} within {} seconds, so \
                         nothing was filled",
                        within.as_secs()
                    ))
                } else if self.on_origin(&target, origin) {
                    not_logged_in(format!(
                        "no login form appeared on {origin} within {} seconds, so nothing was \
                         filled",
                        within.as_secs()
                    ))
                } else {
                    not_logged_in(format!(
                        "the login page is not on {origin}, so nothing was filled"
                    ))
                });
            }
            self.pause(LOOK_AGAIN)?;
        }
        // From here on, the browser holds the login.
        bind().map_err(|why| {
            not_logged_in(format!(
                "purlis could not hold this browser to {origin} ({why}), so nothing was filled"
            ))
        })?;
        let mut filled: Vec<NodeKey> = Vec::new();
        let done = self.fill_and_submit(&target, login, &mut filled);
        // Focus is emulated for the fill alone.
        if !lock(&self.inner.state).ended
            && let Ok(session) = self.page_session(&target)
        {
            let _ = self.devtools().call(
                "Emulation.setFocusEmulationEnabled",
                json!({ "enabled": false }),
                Some(&session),
            );
        }
        if done.is_err() {
            if self.cut() {
                // The person took the browser over: what purlis filled is cleared as they hand
                // it back, before the agent has it again.
                lock(&self.to_clear).push((target.clone(), origin.to_owned(), filled));
            } else {
                self.clear(&target, origin, &filled);
            }
        }
        done
    }

    /// Fills the user name where the page asks for one and the password, submits, and waits
    /// for the password field to go. Each field filled is added to `filled`.
    fn fill_and_submit(
        &self,
        target: &str,
        login: &Login,
        filled: &mut Vec<NodeKey>,
    ) -> Result<Answer, Refusal> {
        let origin = login.origin.as_str();
        let within = self.inner.limits.login_within;
        let unread = |why: Refusal| match why {
            Refusal::Unknown => {
                not_logged_in(format!("purlis could not read the login page on {origin}"))
            }
            other => other,
        };
        if let Some(user) = &login.user
            && let Some(name) = self.field(target, NAME_FIELD).map_err(unread)?
        {
            if login.suggested
                && self.shows(target, PASSWORD_FIELD)? != Some(true)
                && self.shows(target, SAYS_IT_IS_A_NAME)? != Some(true)
            {
                return Err(not_logged_in(format!(
                    "the page on {origin} shows no field that says it's for a user name, so \
                     nothing was filled"
                )));
            }
            self.fill(target, origin, &name, user)?;
            filled.push(name.key);
            if self.shows(target, PASSWORD_FIELD)? != Some(true) {
                // The user name first, then the password on a step of its own.
                self.press_enter(target)?;
                self.wait_until(within, 1, || {
                    Ok(self.on_origin(target, origin)
                        && self.shows(target, PASSWORD_FIELD)? == Some(true))
                })
                .map_err(|why| {
                    or_said(
                        why,
                        format!(
                            "no password field appeared on {origin} within {} seconds",
                            within.as_secs()
                        ),
                    )
                })?;
            }
        }
        let Some(password) = self.field(target, PASSWORD_FIELD).map_err(unread)? else {
            return Err(not_logged_in(format!(
                "{origin} shows no password field, so the password was not filled"
            )));
        };
        self.fill(target, origin, &password, &login.password)?;
        filled.push(password.key);
        self.press_enter(target)?;
        // Gone in two reads in a row: a page between documents is read as neither.
        self.wait_until(within, 2, || {
            Ok(
                self.on_origin(target, origin)
                    && self.shows(target, PASSWORD_FIELD)? == Some(false),
            )
        })
        .map_err(|why| {
            or_said(
                why,
                format!("the login did not go through: {origin} still asks for a password"),
            )
        })?;
        let shown = lock(&self.inner.state)
            .current
            .clone()
            .unwrap_or_else(|| target.to_owned());
        let mut answer = self.answer_now(&shown, Content::Nothing);
        answer.note = Some(format!(
            "the password field is gone from {origin} since purlis submitted the login"
        ));
        Ok(answer)
    }

    /// Opens a page of purlis's own at the login's address, which no other page opened, and
    /// answers its target: it becomes the browser's current page.
    fn go_to(&self, login: &Login) -> Result<String, Refusal> {
        self.open(&login.address)?;
        lock(&self.inner.state)
            .current
            .clone()
            .ok_or(Refusal::NoPage)
    }

    /// Whether `target`'s page shows a document on `origin` now, as the engine recorded it.
    fn on_origin(&self, target: &str, origin: &str) -> bool {
        let state = lock(&self.inner.state);
        state
            .page(target)
            .and_then(|page| page.frames.get(target))
            .and_then(|frame| origin_of(&frame.url))
            .is_some_and(|its| its == origin)
    }

    /// Whether `target`'s own document shows the field `script` finds: unknown (`None`) while
    /// the page cannot be read, as between two documents.
    fn shows(&self, target: &str, script: &str) -> Result<Option<bool>, Refusal> {
        match self.field(target, script) {
            Ok(Some(found)) => {
                if let Ok(session) = self.page_session(target) {
                    let _ = self.devtools().call(
                        "Runtime.releaseObject",
                        json!({ "objectId": found.object }),
                        Some(&session),
                    );
                }
                Ok(Some(true))
            }
            Ok(None) => Ok(Some(false)),
            Err(Refusal::Unknown) => Ok(None),
            Err(why) => Err(why),
        }
    }

    /// The field `script` finds in `target`'s own document, run in purlis's own world there, if
    /// it finds one: an object of that world. [`Refusal::Unknown`] where the page cannot be read
    /// now, or the world cannot be had.
    fn field(&self, target: &str, script: &str) -> Result<Option<Found>, Refusal> {
        let session = self.page_session(target)?;
        let on = Some(session.as_str());
        let found = self
            .in_world(&session, target, |world| {
                self.devtools().call(
                    "Runtime.evaluate",
                    json!({
                        "expression": script,
                        "uniqueContextId": world.unique,
                        "returnByValue": false,
                    }),
                    on,
                )
            })
            .map_err(|_| Refusal::Unknown)?;
        if found.get("exceptionDetails").is_some() {
            return Err(Refusal::Unknown);
        }
        let Some(object) = found
            .pointer("/result/objectId")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(None);
        };
        let described = self
            .devtools()
            .call("DOM.describeNode", json!({ "objectId": object }), on)
            .ok()
            .and_then(|node| node.pointer("/node/backendNodeId").and_then(Value::as_i64));
        let Some(backend) = described else {
            let _ =
                self.devtools()
                    .call("Runtime.releaseObject", json!({ "objectId": object }), on);
            return Ok(None);
        };
        Ok(Some(Found {
            object,
            key: NodeKey {
                frame: target.to_owned(),
                backend,
            },
        }))
    }

    /// Fills `field` with `value`, while `target`'s page is on `origin`, the field blanked
    /// before the value goes in. Right before the value goes in, and right after, the page must
    /// still show the document it showed when the fill began, with the field focused in it and
    /// the document focused, and after, the field must hold exactly as much as was typed:
    /// where it does not, the whole browser is ended
    /// ([`Refusal::NotLoggedIn`] says so) before anything reaches the agent.
    fn fill(&self, target: &str, origin: &str, field: &Found, value: &str) -> Result<(), Refusal> {
        if !self.on_origin(target, origin) {
            return Err(not_logged_in(format!(
                "the page left {origin} before the login was filled, so nothing more was filled"
            )));
        }
        let document = self.document_of(target);
        self.blank_node(target, field.key.clone())?;
        let session = self.page_session(target)?;
        let on = Some(session.as_str());
        let could_not = || not_logged_in(format!("purlis could not fill the login on {origin}"));
        // A page purlis drives behaves as one with the focus, whatever the window shows.
        let _ = self.devtools().call(
            "Emulation.setFocusEmulationEnabled",
            json!({ "enabled": true }),
            on,
        );
        self.gated(|| {
            self.devtools().call(
                "Runtime.callFunctionOn",
                json!({ "objectId": field.object, "functionDeclaration": FOCUS_FIELD }),
                on,
            )
        })?
        .map_err(|_| could_not())?;
        if !self.still_filling(target, origin, document, field, on) {
            return Err(self.ended_for_a_changed_page(origin));
        }
        self.gated(|| {
            self.devtools()
                .call("Input.insertText", json!({ "text": value }), on)
        })?
        .map_err(|_| could_not())?;
        if !self.still_filling(target, origin, document, field, on)
            || !self.holds_as_typed(field, value, on)
        {
            return Err(self.ended_for_a_changed_page(origin));
        }
        let _ = self.devtools().call(
            "Runtime.releaseObject",
            json!({ "objectId": field.object }),
            on,
        );
        Ok(())
    }

    /// The number of the document `target`'s page shows, as the engine recorded it.
    fn document_of(&self, target: &str) -> Option<u64> {
        lock(&self.inner.state).page(target).map(|page| page.shows)
    }

    /// Whether `target`'s page still shows `document`, on `origin`, with `field` focused in it
    /// and the document focused: read from the field itself, so a field of a document gone is
    /// never taken for one still there.
    fn still_filling(
        &self,
        target: &str,
        origin: &str,
        document: Option<u64>,
        field: &Found,
        on: Option<&str>,
    ) -> bool {
        if document.is_none()
            || self.document_of(target) != document
            || !self.on_origin(target, origin)
        {
            return false;
        }
        let focused = self
            .devtools()
            .call(
                "Runtime.callFunctionOn",
                json!({
                    "objectId": field.object,
                    "functionDeclaration": FIELD_HAS_FOCUS,
                    "returnByValue": true,
                }),
                on,
            )
            .ok()
            .and_then(|read| read.pointer("/result/value").and_then(Value::as_bool))
            == Some(true);
        focused && self.document_of(target) == document
    }

    /// Whether `field` holds exactly as much as purlis typed into it, read from the field itself.
    fn holds_as_typed(&self, field: &Found, value: &str, on: Option<&str>) -> bool {
        let typed = value.encode_utf16().count() as i64;
        self.devtools()
            .call(
                "Runtime.callFunctionOn",
                json!({
                    "objectId": field.object,
                    "functionDeclaration": FIELD_LENGTH,
                    "returnByValue": true,
                }),
                on,
            )
            .ok()
            .and_then(|read| read.pointer("/result/value").and_then(Value::as_i64))
            == Some(typed)
    }

    /// **Ends the whole browser** because its page changed while purlis filled it (module docs),
    /// and answers what the agent is told.
    fn ended_for_a_changed_page(&self, origin: &str) -> Refusal {
        {
            let mut state = lock(&self.inner.state);
            state.ended = true;
        }
        self.inner.changed.notify_all();
        let _ = self.devtools().call("Browser.close", json!({}), None);
        #[cfg(unix)]
        self.stop(std::time::Duration::ZERO);
        not_logged_in(format!(
            "the page changed while purlis filled the login on {origin}, so purlis ended this \
             browser: open a new one with browser_new and log in again"
        ))
    }

    /// Presses Enter in what has the focus on `target`'s page.
    fn press_enter(&self, target: &str) -> Result<(), Refusal> {
        let session = self.page_session(target)?;
        let events = key_events("Enter").map_err(|e| Refusal::NotAKey(e.to_string()))?;
        for event in events {
            self.input("Input.dispatchKeyEvent", event, &session)?;
        }
        Ok(())
    }

    /// Waits up to `within` for `done` to hold `times` looks in a row, looking again every
    /// [`LOOK_AGAIN`].
    fn wait_until(
        &self,
        within: Duration,
        times: u32,
        done: impl Fn() -> Result<bool, Refusal>,
    ) -> Result<(), Refusal> {
        let deadline = Instant::now() + within;
        let mut held = 0;
        loop {
            if done()? {
                held += 1;
                if held >= times {
                    return Ok(());
                }
            } else {
                held = 0;
            }
            if Instant::now() >= deadline {
                return Err(Refusal::Unknown);
            }
            self.pause(LOOK_AGAIN)?;
        }
    }

    /// Empties what each login the person cut by taking the browser over filled: called as they
    /// hand it back.
    pub(super) fn clear_cut_logins(&self) {
        let cut = std::mem::take(&mut *lock(&self.to_clear));
        for (target, origin, filled) in cut {
            self.clear(&target, &origin, &filled);
        }
    }

    /// Empties each field in `filled` that `target`'s page still shows on `origin`: what a login
    /// that did not go through leaves behind. Each is resolved into purlis's own world and
    /// emptied by a function run in that world by its unique id, so an element of any other
    /// world is refused by the engine; one that cannot be is left.
    fn clear(&self, target: &str, origin: &str, filled: &[NodeKey]) {
        if !self.on_origin(target, origin) {
            return;
        }
        let Ok(session) = self.page_session(target) else {
            return;
        };
        let on = Some(session.as_str());
        for key in filled.iter().filter(|key| key.frame == target) {
            let _ = self.in_world(&session, target, |world| {
                let resolved = self.devtools().call(
                    "DOM.resolveNode",
                    json!({ "backendNodeId": key.backend, "executionContextId": world.id }),
                    on,
                )?;
                let Some(object) = resolved.pointer("/object/objectId").and_then(Value::as_str)
                else {
                    return Ok(Value::Null);
                };
                let cleared = self.gated(|| {
                    self.devtools().call(
                        "Runtime.callFunctionOn",
                        json!({
                            "uniqueContextId": world.unique,
                            "functionDeclaration": CLEAR_FIELD,
                            "arguments": [{ "objectId": object }],
                        }),
                        on,
                    )
                });
                let _ = self.devtools().call(
                    "Runtime.releaseObject",
                    json!({ "objectId": object }),
                    on,
                );
                // A world the engine refuses the element in is made again, once.
                match cleared {
                    Ok(done) => done.map(|_| Value::Null),
                    Err(_) => Ok(Value::Null),
                }
            });
        }
    }
}

fn not_logged_in(why: String) -> Refusal {
    Refusal::NotLoggedIn(why)
}

/// `why`, unless it is a wait that ran out, which is said as `said`.
fn or_said(why: Refusal, said: String) -> Refusal {
    match why {
        Refusal::Unknown => not_logged_in(said),
        other => other,
    }
}

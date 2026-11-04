// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a page holding a secret purlis filled may leave for another site** (#5230; the person's
//! rulings SF-14 and SF-15 on #5096, amending SF-3).
//!
//! While a fill holds the browser, its reach proxy carries the fill's site alone
//! ([`FillHold`]). That stays so for everything the page does, whoever asks: a link, a script,
//! the chat's navigate. Two things are told apart here, by what the engine says of each request
//! as it pauses it, never by what the page says.
//!
//! - **A redirect from the approved site's own server, after a form sent there** (SF-14): a
//!   main-frame document request the engine says was redirected, whose chain began with a form
//!   sent to the fill's site. It is stopped where it is, its address kept, and judged:
//!   - an address that holds the value, plainly or URL-encoded, is refused ([`carries_value`]);
//!   - [`FillHold::may_leave_for`] says whether it goes on by itself (a site this browser was on
//!     before the fill), asks the person once, or is refused (outside the browser's usual reach).
//! - **A form posted straight to another site** (SF-15), or a redirect that would send its body
//!   on: refused unless [`FillHold::approved_for`] says the key is approved for that site too.
//!   The refusal is said in the answer and on a page of purlis's own the person sees, and the
//!   filled page goes with it, so the browser isn't stuck.
//!
//! **Any other load of a page's own, elsewhere, stays where it was** ([`Move::Elsewhere`]): a link,
//! a script, the chat's navigate. Left to the proxy it would end on the engine's own page, whose
//! history entry holds the refused address, and what the page wrote there would go out on a reload
//! once the hold is over. So only a load on the fill's own site can end on that page, which is why
//! that page may end the hold.
//!
//! **Nothing reaches the destination while the page holding the value lives.** A proxy that goes
//! by host and port can't tell the redirect from anything else the page sends there, and a page
//! keeps running until the next document commits. So the page is replaced first: it loads a
//! blank document, while the proxy still carries the fill's site alone, what it sends as it
//! unloads goes nowhere else, the hold ends ([`FillHold::released_then`]), and only then does the
//! browser go to the address the server named. A destination that fails shows the engine's own
//! page, and nothing holds the browser by then.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;

use super::{Held, Inner, Outbound, Paused, State, login};
use crate::drive::{FillHold, Leaving, Why, lock};

/// How long a page leaving for another site waits for the hold to end before it stays blank: the
/// hold ends a moment after the page is gone, and an engine that never says so goes no further.
const THE_HOLD_ENDS_WITHIN: Duration = Duration::from_secs(20);

/// **What a page holding a secret purlis filled filled, and where** (#5230): what the hold of it
/// is judged by.
pub(crate) struct Filled {
    pub(crate) origin: String,
    pub(crate) key: String,
    pub(crate) value: zeroize::Zeroizing<String>,
}

/// What a held page's request is, where it is one this module judges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Move {
    /// A redirect to another site, in a chain that began with a form sent to the fill's site.
    Redirect,
    /// A form sent to another site, or a redirect that sends its body there.
    Form,
    /// Any other load in a page's main frame, on another site.
    Elsewhere,
}

/// **A held page's request stopped to be judged**, or a redirect the person answered about.
pub(super) enum Leave {
    Stopped {
        paused: Paused,
        outbound: Outbound,
        target: String,
        /// Its whole address: what the engine said of it and of its fragment.
        whole: String,
        kind: Move,
    },
    Answered,
}

impl State {
    /// **What `held`, a document request just judged by [`State::why`], is while a page holds a
    /// secret**, with the page it is in; `None` where the proxy's hold alone answers it: anything
    /// on the fill's site, and a frame's load no form began.
    pub(super) fn move_of(
        &self,
        holds: &[super::PageHeld],
        held: &Held<'_>,
    ) -> Option<(String, Move)> {
        if !held.document {
            return None;
        }
        let frame = held.frame?;
        let target = self.page_holding(frame)?;
        let main = frame == target;
        let on = holds.iter().find(|one| one.target == target);
        // Another page's load meets the hold as well: judged by the site that holds the browser.
        let origin = &on.or_else(|| holds.first())?.filled.origin;
        let to = login::origin_of(held.address)?;
        if to == *origin {
            return None;
        }
        let elsewhere = main.then(|| (target.clone(), Move::Elsewhere));
        let hop = self
            .documents
            .iter()
            .rev()
            .find(|hop| hop.id == held.request);
        if on.is_none() || !hop.is_some_and(|hop| hop.form) {
            return elsewhere;
        }
        if !matches!(held.method, "GET" | "HEAD") {
            return Some((target, Move::Form));
        }
        let Some(from) = held.redirected_from else {
            return Some((target, Move::Form));
        };
        let from_there = self
            .documents
            .iter()
            .rev()
            .find(|hop| hop.id == from)
            .and_then(|from| login::origin_of(&from.address))
            .as_ref()
            == Some(origin);
        if main && from_there {
            return Some((target, Move::Redirect));
        }
        elsewhere
    }

    /// **`address` is a site this browser was on**: it showed a document of it in a page, or
    /// answered a page's document with a redirect.
    pub(super) fn was_on(&mut self, address: &str) {
        if self.been_on.len() < MOST_SITES_BEEN_ON
            && let Some(origin) = login::origin_of(address)
        {
            self.been_on.insert(origin);
        }
    }
}

/// The most sites a browser remembers having been on: past them, a new one asks.
const MOST_SITES_BEEN_ON: usize = 4096;

impl Inner {
    /// **Judges what the engine stopped** ([`Leave`]), on a thread of its own: leaving takes as
    /// long as the old page takes to go.
    pub(super) fn leaving(&self, leave: Leave) {
        match leave {
            Leave::Answered => self.leave_where_answered(),
            Leave::Stopped {
                paused,
                outbound,
                target,
                whole,
                kind,
            } => {
                match kind {
                    Move::Redirect => self.redirected(&paused, &outbound, &target, &whole),
                    Move::Form => self.form_sent(&paused, &outbound, &target, &whole),
                    Move::Elsewhere => self.elsewhere(&paused, &outbound, &whole),
                }
                self.done_leaving(&target);
            }
        }
    }

    /// **A redirect from the fill's site's server** (module docs).
    fn redirected(&self, paused: &Paused, outbound: &Outbound, target: &str, whole: &str) {
        let held = self.held_on(target);
        let Some(first) = held.first() else {
            // The page went while this waited: nothing holds it, and it is judged as any is.
            self.judged(paused, outbound);
            return;
        };
        if held.iter().any(|one| carries_value(whole, &one.value)) {
            self.stays(paused);
            self.say(format!(
                "purlis stopped this page leaving {}: its server redirected to an address that \
                 holds the secret purlis filled, and nothing was sent there. The page stays \
                 where it was",
                first.origin
            ));
            return;
        }
        let been_on = first.been_on_at(whole);
        match first.hold.may_leave_for(whole, been_on) {
            Leaving::Go => {
                self.stays(paused);
                self.replace(target, "about:blank", Some(whole));
            }
            Leaving::Ask { site } => {
                self.stays(paused);
                lock(&self.leaving_asked).insert(target.to_owned(), whole.to_owned());
                lock(&self.state).ask(&site);
            }
            Leaving::No { why } => {
                self.stays(paused);
                self.say(why);
            }
        }
    }

    /// **A form sent to another site** (SF-15): sent only where the key is approved for it too,
    /// and the proxy's hold then carries it.
    fn form_sent(&self, paused: &Paused, outbound: &Outbound, target: &str, whole: &str) {
        let held = self.held_on(target);
        let Some(first) = held.first() else {
            self.judged(paused, outbound);
            return;
        };
        if held.iter().all(|one| one.hold.approved_for(whole)) {
            self.judged(paused, outbound);
            return;
        }
        self.stays(paused);
        // A value in the site's own name is not said back.
        let origin = login::origin_of(whole)
            .filter(|to| !held.iter().any(|one| carries_value(to, &one.value)));
        let said = form_refused_said(&first.key, &first.origin, origin.as_deref());
        self.say(said.clone());
        self.replace(target, &said_as_a_page(&said), None);
    }

    /// **Another load of a page's, on another site**: it goes only where every hold's key is
    /// approved there, and where none holds any more it is judged as any is. Otherwise the page
    /// stays where it was. The address is not said: what a filled page sent may be in it.
    fn elsewhere(&self, paused: &Paused, outbound: &Outbound, whole: &str) {
        let (approved, origin) = {
            let holds = lock(&self.fill_holds);
            (
                holds.iter().all(|one| one.hold.approved_for(whole)),
                holds.first().map(|one| one.filled.origin.clone()),
            )
        };
        let Some(origin) = origin.filter(|_| !approved) else {
            self.judged(paused, outbound);
            return;
        };
        self.stays(paused);
        self.say(format!(
            "that page did not load, and this page stays where it was: this browser reaches only \
             {origin} while a page in it holds a secret purlis filled there. Once that page \
             loads another document or is closed, it reaches as before"
        ));
    }

    /// The person answered about a site: a page that waited on it goes on, where it now may. It
    /// waits on, otherwise: an answer may be about another site.
    fn leave_where_answered(&self) {
        let waiting: Vec<(String, String)> = lock(&self.leaving_asked)
            .iter()
            .map(|(target, whole)| (target.clone(), whole.clone()))
            .collect();
        for (target, whole) in waiting {
            let held = self.held_on(&target);
            let Some(first) = held.first() else {
                lock(&self.leaving_asked).remove(&target);
                continue;
            };
            let been_on = first.been_on_at(&whole);
            if first.hold.may_leave_for(&whole, been_on) == Leaving::Go {
                lock(&self.leaving_asked).remove(&target);
                self.replace(&target, "about:blank", Some(&whole));
            }
            self.done_leaving(&target);
        }
    }

    /// **What holds page `target`, as it is now.**
    fn held_on(&self, target: &str) -> Vec<HeldNow> {
        lock(&self.fill_holds)
            .iter()
            .filter(|one| one.target == target)
            .map(|one| HeldNow {
                hold: Arc::clone(&one.hold),
                origin: one.filled.origin.clone(),
                key: one.filled.key.clone(),
                value: one.filled.value.clone(),
                been_on: Arc::clone(&one.been_on),
            })
            .collect()
    }

    /// The request stopped goes nowhere, and its frame stays on the page it showed.
    fn stays(&self, paused: &Paused) {
        let _ = self.devtools.call_within(
            "Fetch.fulfillRequest",
            json!({ "requestId": paused.id, "responseCode": 204 }),
            None,
            super::JUDGED_WITHIN,
        );
    }

    /// Keeps `said` for the next answer of the agent's.
    fn say(&self, said: String) {
        lock(&self.state).leaving_said.push(said);
    }

    /// **Page `target` loads `shown` in place of what holds the value, and once the hold is over
    /// goes on to `then`, where there is one** (module docs). Where the page never goes, or the
    /// hold never ends, it goes no further.
    fn replace(&self, target: &str, shown: &str, then: Option<&str>) {
        let session = lock(&self.state)
            .page(target)
            .and_then(|page| page.session.clone());
        let Some(session) = session else {
            return;
        };
        let (over, heard) = std::sync::mpsc::channel();
        lock(&self.departing).insert(target.to_owned(), over);
        let on = Some(session.as_str());
        let loaded = self
            .devtools
            .call("Page.navigate", json!({ "url": shown }), on);
        if loaded.is_err() {
            lock(&self.departing).remove(target);
            return;
        }
        if heard.recv_timeout(THE_HOLD_ENDS_WITHIN).is_err() {
            lock(&self.departing).remove(target);
            return;
        }
        if let Some(address) = then {
            // purlis's own move, to what the server named: a read of that address, as a page's
            // link is, so it isn't asked about again. The usual reach was judged above.
            lock(&self.state).addressing = Some((target.to_owned(), Why::Reads));
            let _ = self
                .devtools
                .call("Page.navigate", json!({ "url": address }), on);
            lock(&self.state).addressing = None;
        }
    }

    /// Page `target` is done leaving, or stays: an answer waits on it no more.
    fn done_leaving(&self, target: &str) {
        let mut state = lock(&self.state);
        if let Some(page) = state.page_mut(target) {
            page.leaving = false;
        }
        drop(state);
        self.changed.notify_all();
    }

    /// **The holds of page `target` that just went are told so, and whoever waits to leave it
    /// hears once all of them are over** (module docs).
    pub(super) fn released(&self, target: &str, gone: Vec<super::PageHeld>) {
        lock(&self.leaving_asked).remove(target);
        let Some(over) = lock(&self.departing).remove(target) else {
            for one in gone {
                one.hold.released();
            }
            return;
        };
        if gone.is_empty() {
            let _ = over.send(());
            return;
        }
        let left = Arc::new(AtomicUsize::new(gone.len()));
        for one in gone {
            let (left, over) = (Arc::clone(&left), over.clone());
            one.hold.released_then(Box::new(move || {
                if left.fetch_sub(1, Ordering::SeqCst) == 1 {
                    let _ = over.send(());
                }
            }));
        }
    }
}

/// A hold of a page, as it is read to judge what the page does.
struct HeldNow {
    hold: Arc<dyn FillHold>,
    origin: String,
    key: String,
    value: zeroize::Zeroizing<String>,
    been_on: Arc<HashSet<String>>,
}

impl HeldNow {
    fn been_on_at(&self, address: &str) -> bool {
        login::origin_of(address).is_some_and(|origin| self.been_on.contains(&origin))
    }
}

/// **Why a form wasn't sent to another site** (SF-15), and how the person approves the key for
/// it. `to` is left unnamed only where its name would say the value.
fn form_refused_said(key: &str, origin: &str, to: Option<&str>) -> String {
    let site = to.unwrap_or("another site");
    let line = to.map_or(String::new(), |to| format!(" with fill = \"{to}\""));
    format!(
        "purlis did not send this form to {site}: the page holds the secret {key}, which is \
         approved for {origin} and not for {site}. To approve it there, give {key} a secret-fill \
         line{line} in the persona's tools, or allow it when a chat fills it on that site. Then \
         open the form and fill it again"
    )
}

/// `said` as a page of purlis's own, on no site, which the person sees in place of the form.
fn said_as_a_page(said: &str) -> String {
    let mut escaped = String::with_capacity(said.len());
    for c in said.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(c),
        }
    }
    let page = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Form not sent</title>\
         <h1>Form not sent</h1><p>{escaped}</p>"
    );
    let mut address = String::from("data:text/html;charset=utf-8,");
    for byte in page.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                address.push(char::from(byte));
            }
            _ => address.push_str(&format!("%{byte:02X}")),
        }
    }
    address
}

/// **Whether `address` holds `value`**, as written or URL-encoded, however many times, a space as
/// `+` or `%20`, in any case: the engine writes a host in lower case. Reads what the engine sent
/// whole, its fragment too.
pub(super) fn carries_value(address: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let value = value.to_lowercase();
    let holds = |seen: &str| {
        let seen = seen.to_lowercase();
        seen.contains(&value) || seen.replace('+', " ").contains(&value)
    };
    let mut seen = address.to_owned();
    // An address is at most some thousands of characters, and each pass that changes it shortens
    // it: this ends, and a few passes reach what any page would write.
    for _ in 0..8 {
        if holds(&seen) {
            return true;
        }
        let decoded = decoded_once(&seen);
        if decoded == seen {
            return false;
        }
        seen = decoded;
    }
    holds(&seen)
}

/// `text` with each `%XX` read as the byte it names, what doesn't read as UTF-8 replaced.
fn decoded_once(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let pair = (bytes[at] == b'%')
            .then(|| bytes.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(byte) = pair {
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{carries_value, form_refused_said, said_as_a_page};

    #[test]
    fn an_address_holds_a_value_however_it_is_encoded() {
        let value = "s3cret word/é+1";
        for address in [
            "https://app.example/?code=s3cret word/é+1",
            "https://app.example/?code=s3cret%20word%2F%C3%A9%2B1",
            "https://app.example/?code=s3cret+word%2f%c3%a9%2b1",
            "https://app.example/#code=s3cret%2520word%252F%25C3%25A9%252B1",
            "https://S3CRET%20WORD%2F%C3%89%2B1.app.example/",
        ] {
            assert!(carries_value(address, value), "{address}");
        }
        assert!(!carries_value("https://app.example/?code=abc", value));
        assert!(!carries_value("https://app.example/", ""));
    }

    #[test]
    fn an_address_cut_in_an_escape_is_read_to_its_end() {
        assert!(!carries_value("https://app.example/?a=%", "secret"));
        assert!(!carries_value("https://app.example/?a=%é", "secret"));
        assert!(carries_value("https://app.example/?a=%&b=secret", "secret"));
    }

    #[test]
    fn a_form_refused_names_the_site_and_how_to_approve_the_key() {
        let said = form_refused_said("dash-key", "https://a.example", Some("https://b.example"));
        assert!(said.contains("https://b.example"), "{said}");
        assert!(said.contains("fill = \"https://b.example\""), "{said}");
        assert!(said.contains("dash-key"), "{said}");
        let unnamed = form_refused_said("dash-key", "https://a.example", None);
        assert!(unnamed.contains("another site"), "{unnamed}");
    }

    #[test]
    fn a_page_of_purlis_s_own_escapes_what_it_says() {
        let page = said_as_a_page("<script>&\"");
        assert!(page.starts_with("data:text/html;charset=utf-8,"));
        assert!(!page[29..].contains('<'));
        assert!(page.contains("%26lt%3Bscript%26gt%3B"), "{page}");
    }
}

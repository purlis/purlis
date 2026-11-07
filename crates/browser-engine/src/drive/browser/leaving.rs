// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a page holding a secret purlis filled may leave for another site** (#5230; the person's
//! rulings SF-14 and SF-15 on #5096, amending SF-3).
//!
//! While a fill holds the browser, its reach proxy carries the fill's site alone ([`FillHold`]).
//! The driver judges each request as the engine pauses it, by what the engine says of it:
//!
//! - **The redirect the ruling lets go** (SF-14): a main-frame document redirected from the fill's
//!   site, in a chain begun by the form the field was filled in. An address that holds the value
//!   is refused ([`carries_value`]). Otherwise [`FillHold::may_leave_for`] says whether it goes on
//!   by itself, which it does only to the site the chain leading to this sign-in began from, asks
//!   the person once, or is refused. The page stays until it may go: then the page holding the
//!   value goes first, the hold ends, and the browser goes to the address.
//! - **A form sent to another site** (SF-15): sent only where [`FillHold::approved_for`] says the
//!   key is approved for that site too. Refused, it is said in the answer and on a page of purlis's
//!   own the person sees.
//! - **Anything else elsewhere stays refused**, the page where it was, until the hold is over.
//!
//! A load in a browser that has held a value doesn't rest on the engine's own page for a load that
//! failed: purlis's own page stands in for it.
//!
//! **The hold follows related windows** (the person's ruling of 2026-11-05): a fill is refused
//! while a window on its site is related to its page, and any other related window closes when the
//! page is done with the value. The hold ends only once they are gone. Nothing here stops a script
//! on the approved site keeping the value in that site's own storage or workers.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::json;

use super::{Browser, FollowUp, HashSet, Held, Inner, Paused, State, login, site};
use crate::drive::{FillHold, Leaving, Refusal, lock};

/// **The address of purlis's own page**: on a name that is never a site, and answered by the driver.
pub(super) const OWN_PAGE: &str = "http://purlis.invalid/said";

/// How long a page leaving for another site waits for the hold to end before it stays where it is.
const THE_HOLD_ENDS_WITHIN: Duration = Duration::from_secs(20);

/// How long purlis's own page is waited for before what it replaced is forgotten anyway.
const ITS_OWN_PAGE_LOADS_WITHIN: Duration = Duration::from_secs(3);

/// How long a read waits for a page that is changing over.
const A_READ_WAITS: Duration = Duration::from_secs(10);

/// The most a browser says of its refused moves in one answer: a page may try without end.
const MOST_SAID: usize = 8;

/// **What a page holding a secret purlis filled filled, and where** (#5230): what the hold of it
/// is judged by.
pub(crate) struct Filled {
    pub(crate) origin: String,
    pub(crate) key: String,
    pub(crate) value: zeroize::Zeroizing<String>,
    /// Where the form the field is in sends it, read as the value went in.
    pub(crate) form: Option<FormSent>,
}

/// **Where a form sends what it holds**, as its document says, its fragment left off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormSent {
    pub(crate) method: String,
    pub(crate) action: String,
}

impl FormSent {
    /// Whether a chain whose first request was `method` to `head` is this form's. A form sent by
    /// GET writes its own query.
    fn began(&self, method: &str, head: &str) -> bool {
        let head = head.split('#').next().unwrap_or(head);
        if !self.method.eq_ignore_ascii_case(method) {
            return false;
        }
        if method.eq_ignore_ascii_case("GET") {
            let bare = |address: &'_ str| address.split('?').next().unwrap_or("").to_owned();
            return bare(head) == bare(&self.action);
        }
        head == self.action
    }
}

/// **A load chosen by an address**: the origin that address is on, and the address, which each
/// document request of its chain names as its head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChosenLoad {
    pub(super) origin: String,
    pub(super) head: String,
}

/// **A page a login filled, and the windows related to it** (the person's ruling of 2026-11-05):
/// as a fill's [`super::PageHeld`] is, without the hold of the reach, which a login keeps for the
/// browser's life.
pub(super) struct LoginFamily {
    pub(super) target: String,
    /// Every window that could reach it by script, or be reached by one that could, gone or not,
    /// itself among them.
    pub(super) family: HashSet<String>,
    /// Its page has shown another document or closed: the others close, and so does one opened
    /// from any of them since.
    pub(super) left: bool,
}

/// What a request is while a page holds a secret, where it isn't for the hold's own site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Move {
    /// A redirect the ruling may let go, in page `.0`.
    Redirect(String),
    /// A form the page sent to another site, in page `.0`.
    Form(String),
    /// A form a frame in the page sent to another site: only it fails, and the answer says why.
    FrameForm {
        key: String,
        origin: String,
        to: Option<String>,
    },
    /// Any other document of a main frame: its page stays where it is.
    Elsewhere { origin: String },
    /// Anything else: it fails.
    Refused,
}

/// **What the leaving thread is told** (module docs).
pub(super) enum Leave {
    Stopped {
        paused: Paused,
        target: String,
        /// Its whole address: what the engine said of it and of its fragment.
        whole: String,
        form: bool,
    },
    /// The person answered about a site, and these pages wait on one.
    Answered(Vec<String>),
    /// Page `.0` shows the engine's own page for a load that failed, in a browser that held a value.
    Failed(String),
}

impl State {
    /// **What `held`, a request just judged by [`State::why`], is while a page holds a secret or
    /// its hold is ending**; `None` where nothing holds the browser, or it is for the fill's own
    /// site. It is decided here, as it pauses: nothing paused while a hold lives waits to go out
    /// once it is over. An address whose origin can't be read is on no such site, and a frame the
    /// driver doesn't know yet is taken for a page.
    pub(super) fn move_of(
        &self,
        holds: &[super::PageHeld],
        ending: &[String],
        held: &Held<'_>,
    ) -> Option<Move> {
        let origin = holds
            .first()
            .map(|one| &one.filled.origin)
            .or_else(|| ending.first())?;
        let to = login::origin_of(held.address);
        if to.as_ref() == Some(origin) {
            return None;
        }
        if !held.document {
            return Some(Move::Refused);
        }
        let target = held.frame.and_then(|frame| self.page_holding(frame));
        let main = match (&target, held.frame) {
            (Some(target), Some(frame)) => target == frame,
            _ => true,
        };
        let elsewhere = Move::Elsewhere {
            origin: origin.clone(),
        };
        let on: Vec<&super::PageHeld> = holds
            .iter()
            .filter(|one| !one.left && Some(&one.target) == target.as_ref())
            .collect();
        let hop = self
            .documents
            .iter()
            .rev()
            .find(|hop| hop.id == held.request);
        let (Some(first), Some(target), Some(hop)) = (on.first(), &target, hop) else {
            return Some(if main { elsewhere } else { Move::Refused });
        };
        if !hop.form {
            return Some(if main { elsewhere } else { Move::Refused });
        }
        let sends_on = !matches!(held.method, "GET" | "HEAD");
        let Some(from) = held.redirected_from.filter(|_| !sends_on) else {
            // Sent where every hold's key is approved, it is judged as any request is.
            if on.iter().all(|one| one.hold.approved_for(held.address)) {
                return None;
            }
            if !main {
                // A value in the site's own name is not said back.
                let to = to.filter(|to| !on.iter().any(|one| carries_value(to, &one.filled.value)));
                return Some(Move::FrameForm {
                    key: first.filled.key.clone(),
                    origin: origin.clone(),
                    to,
                });
            }
            return Some(Move::Form(target.clone()));
        };
        let from_there = self
            .documents
            .iter()
            .rev()
            .find(|hop| hop.id == from)
            .and_then(|from| login::origin_of(&from.address))
            .as_ref()
            == Some(origin);
        let its_form = on.iter().any(|one| {
            one.filled
                .form
                .as_ref()
                .is_some_and(|form| form.began(&hop.head_method, &hop.head))
        });
        Some(if main && from_there && its_form {
            Move::Redirect(target.clone())
        } else if main {
            elsewhere
        } else {
            Move::Refused
        })
    }

    /// **Page `target`'s main frame shows a document at `address`** (#5230): a chain begins here
    /// only where that document is the end of the load that was chosen. A load refused, stopped or
    /// answered with nothing shows no document, and what shows meanwhile isn't it.
    pub(super) fn load_shown(&mut self, target: &str, address: &str, failed: bool) {
        let Some(load) = self.page(target).and_then(|page| page.chain_begins.clone()) else {
            return;
        };
        let bare = super::without_fragment(address);
        let its_end = self
            .documents
            .iter()
            .any(|hop| hop.head == load.head && hop.address == bare);
        if let Some(page) = self.page_mut(target) {
            if failed {
                page.chain_begins = None;
            } else if its_end {
                page.chain_begins = None;
                page.chain_began = Some(load.origin);
            }
        }
    }

    /// **The windows related to `target`'s**, itself among them: each that opened another, or
    /// was opened by one, where a script in one can reach the other, and so on from there. A window
    /// gone still links those it opened.
    pub(super) fn family_of(&self, target: &str) -> HashSet<String> {
        let mut family = HashSet::from([target.to_owned()]);
        loop {
            let before = family.len();
            for (window, opener) in &self.linked_openers {
                if family.contains(window) || family.contains(opener) {
                    family.insert(window.clone());
                    family.insert(opener.clone());
                }
            }
            if family.len() == before {
                return family;
            }
        }
    }

    /// **Whether a window related to `target`'s is on `origin`'s site, so it could read a field
    /// there.** A window on no site, as a blank one is, takes its site from who opened it, and may
    /// be on this one.
    pub(super) fn related_on_site(&self, target: &str, origin: &str) -> bool {
        let here = site(origin);
        self.family_of(target)
            .iter()
            .filter(|one| *one != target)
            .filter_map(|one| self.page(one))
            .any(|page| {
                let own = page.site();
                own.is_none()
                    || own == here
                    || page.frames.values().any(|frame| site(&frame.url) == here)
            })
    }

    /// Keeps `said` for the next answer of the agent's, once, and only so many.
    pub(super) fn say_of_leaving(&mut self, said: String) {
        if self.leaving_said.len() < MOST_SAID && !self.leaving_said.contains(&said) {
            self.leaving_said.push(said);
        }
    }
}

/// What the agent is told of a load elsewhere that stayed refused.
pub(super) fn elsewhere_said(origin: &str) -> String {
    format!(
        "that did not load, and the page stays where it was: this browser reaches only {origin} \
         while a page in it holds a secret purlis filled there. Once that page loads another \
         document or is closed, it reaches as before"
    )
}

impl Browser {
    /// **A read waits for a page changing over**, so what it reads is what stands afterwards:
    /// refused where that takes too long.
    pub(super) fn reads_once_settled(&self) -> Result<(), Refusal> {
        let until = Instant::now() + A_READ_WAITS;
        let mut state = lock(&self.inner.state);
        while state.pages.iter().any(|page| page.leaving) {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Refusal::Engine(
                    "the page is changing over: read it again in a moment".to_owned(),
                ));
            }
            state = self
                .inner
                .changed
                .wait_timeout(state, left)
                .map_or_else(|e| e.into_inner().0, |waited| waited.0);
        }
        Ok(())
    }
}

impl Inner {
    /// **Judges what the engine stopped** ([`Leave`]), on a thread of its own: leaving takes as
    /// long as the old page takes to go. A hold that has gone by its turn lets nothing go.
    pub(super) fn leaving(&self, leave: Leave) {
        match leave {
            Leave::Answered(targets) => {
                self.leave_where_answered();
                for target in &targets {
                    self.done_leaving(target);
                }
            }
            Leave::Failed(target) => {
                // RED-PROOF: the engine's own page stays.
                if std::hint::black_box(false) {
                    self.show_own(
                        &target,
                        said_as_a_page("Page not loaded", "That page did not load."),
                    );
                }
                self.done_leaving(&target);
            }
            Leave::Stopped {
                paused,
                target,
                whole,
                form,
            } => {
                if form {
                    self.form_sent(&paused, &target, &whole);
                } else {
                    self.redirected(&paused, &target, &whole);
                }
                self.done_leaving(&target);
            }
        }
    }

    /// **A redirect from the fill's site's server** (module docs): the person is asked before the
    /// page goes, so an answer of no keeps it.
    fn redirected(&self, paused: &Paused, target: &str, whole: &str) {
        self.stays(paused);
        let held = self.held_on(target);
        let Some(first) = held.first() else {
            return;
        };
        if held.iter().any(|one| carries_value(whole, &one.value)) {
            self.say(format!(
                "purlis stopped this page leaving {}: its server redirected to an address that \
                 holds the secret purlis filled, and nothing was sent there. The page stays \
                 where it was",
                first.origin
            ));
            return;
        }
        match first.hold.may_leave_for(whole, first.began_at(whole)) {
            Leaving::Go => self.replace(target, "about:blank", Some(whole)),
            Leaving::Ask { site } => {
                lock(&self.leaving_asked).insert(target.to_owned(), whole.to_owned());
                lock(&self.state).ask(&site);
            }
            Leaving::No { why } => self.say(why),
        }
    }

    /// **A form sent to another site, its key not approved there** (SF-15).
    fn form_sent(&self, paused: &Paused, target: &str, whole: &str) {
        self.stays(paused);
        let held = self.held_on(target);
        let Some(first) = held.first() else {
            return;
        };
        // A value in the site's own name is not said back.
        let to = login::origin_of(whole)
            .filter(|to| !held.iter().any(|one| carries_value(to, &one.value)));
        let said = form_refused_said(&first.key, &first.origin, to.as_deref());
        self.say(said.clone());
        // RED-PROOF: the form goes, and its page stays.
        if std::hint::black_box(false) {
            self.show_own(target, said_as_a_page("Form not sent", &said));
        }
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
            if first.hold.may_leave_for(&whole, first.began_at(&whole)) == Leaving::Go {
                lock(&self.leaving_asked).remove(&target);
                self.replace(&target, "about:blank", Some(&whole));
            }
        }
    }

    /// **What holds page `target`, as it is now.**
    fn held_on(&self, target: &str) -> Vec<HeldNow> {
        lock(&self.fill_holds)
            .iter()
            .filter(|one| !one.left && one.target == target)
            .map(|one| HeldNow {
                hold: Arc::clone(&one.hold),
                origin: one.filled.origin.clone(),
                key: one.filled.key.clone(),
                value: one.filled.value.clone(),
                chain_began: one.chain_began.clone(),
            })
            .collect()
    }

    /// The request stopped goes nowhere, and its frame stays on the page it showed.
    pub(super) fn stays(&self, paused: &Paused) {
        let _ = self.devtools.call_within(
            "Fetch.continueRequest",
            json!({ "requestId": paused.id }),
            None,
            super::JUDGED_WITHIN,
        );
    }

    fn say(&self, said: String) {
        lock(&self.state).say_of_leaving(said);
    }

    /// **Page `target` shows `page`, of purlis's own.** A browser with no judge holds no request,
    /// so nothing could answer for that address: it shows a blank page.
    fn show_own(&self, target: &str, page: String) {
        if self.judge.is_none() {
            self.replace(target, "about:blank", None);
            return;
        }
        *lock(&self.own_page) = Some(page);
        self.replace(target, OWN_PAGE, None);
    }

    /// **Page `target` loads `shown`; once the hold is over it goes on to `then`, where there is
    /// one, and otherwise forgets what it showed before.** Where the page never goes or the hold
    /// never ends, it goes no further, and says so.
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
        let within = if then.is_some() {
            THE_HOLD_ENDS_WITHIN
        } else {
            ITS_OWN_PAGE_LOADS_WITHIN
        };
        let went = loaded.is_ok() && heard.recv_timeout(within).is_ok();
        lock(&self.departing).remove(target);
        // What it showed goes from its history, and what the page left in its window goes too. The
        // page it now shows runs nothing of the old one's.
        // RED-PROOF: history kept.
        let cleared = self.devtools.call(
            "Runtime.evaluate",
            json!({ "expression": "window.name = ''" }),
            on,
        );
        let Some(address) = then else {
            return;
        };
        if !went || cleared.is_err() {
            self.say("purlis could not take this page on to the site its server named".to_owned());
            return;
        }
        // A read of the address the server named, as a page's link is: the reach was judged.
        lock(&self.state).offered_by_purlis(address);
        let _ = self
            .devtools
            .call("Page.navigate", json!({ "url": address }), on);
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

    /// **A window opened by `opener`** joins the family of any hold `opener` is in, and closes at
    /// once where that hold's page is done with the value.
    pub(super) fn window_opened(&self, target: &str, opener: &str) -> Vec<FollowUp> {
        let mut closes = false;
        for held in lock(&self.fill_holds).iter_mut() {
            if held.family.contains(opener) {
                held.family.insert(target.to_owned());
                closes |= held.left;
            }
        }
        // A login's page's family likewise (the person's ruling of 2026-11-05).
        for one in lock(&self.login_families).iter_mut() {
            if one.family.contains(opener) {
                one.family.insert(target.to_owned());
                closes |= one.left;
            }
        }
        if closes {
            vec![FollowUp::Close(target.to_owned())]
        } else {
            Vec::new()
        }
    }

    /// **A login is about to fill page `target`** (the person's ruling of 2026-11-05): the windows
    /// related to it are its family from here, and close once the page is done with what it filled.
    /// One already there is kept, a family of the same page twice being one.
    pub(super) fn login_fills(&self, target: &str) {
        let state = lock(&self.state);
        let family = state.family_of(target);
        let mut families = lock(&self.login_families);
        match families
            .iter_mut()
            .find(|one| one.target == target && !one.left)
        {
            Some(one) => one.family.extend(family),
            None => families.push(LoginFamily {
                target: target.to_owned(),
                family,
                left: false,
            }),
        }
    }

    /// **Page `target` is closed, or shows another document in its main frame**: a login's page
    /// that did closes the windows related to it, and a family whose others are all gone is
    /// forgotten. Answers the windows to close.
    fn login_page_left(&self, target: &str) -> Vec<String> {
        let mut state = lock(&self.state);
        let mut families = lock(&self.login_families);
        let mut closes: Vec<String> = Vec::new();
        for one in families.iter_mut() {
            if one.target != target || one.left {
                continue;
            }
            one.left = true;
            for window in &one.family {
                if window != target && state.page(window).is_some() && !closes.contains(window) {
                    closes.push(window.clone());
                }
            }
        }
        families.retain(|one| {
            !one.left
                || one
                    .family
                    .iter()
                    .any(|window| *window != one.target && state.page(window).is_some())
        });
        drop(families);
        if !closes.is_empty() {
            state.say_of_leaving(format!(
                "purlis closed {} other {} related to the page purlis logged in on: {} could have \
                 read what purlis filled there",
                closes.len(),
                if closes.len() == 1 {
                    "window"
                } else {
                    "windows"
                },
                if closes.len() == 1 { "it" } else { "they" },
            ));
        }
        closes
    }

    /// **Page `target` is closed, or shows another document in its main frame** (#5117): a hold of
    /// a secret filled into it closes the windows related to it, and every hold whose page is done
    /// and whose related windows are gone is over, once. Answers what closes them.
    pub(super) fn page_left(&self, target: &str) -> Vec<FollowUp> {
        let mut closes: Vec<String> = self.login_page_left(target);
        let mut over: Vec<super::PageHeld> = Vec::new();
        let its_own_stands =
            {
                let mut state = lock(&self.state);
                let mut holds = lock(&self.fill_holds);
                for held in holds.iter_mut() {
                    if held.target != target || held.left {
                        continue;
                    }
                    held.left = true;
                    for one in &held.family {
                        if one != target && state.page(one).is_some() && !closes.contains(one) {
                            closes.push(one.clone());
                        }
                    }
                }
                for held in std::mem::take(&mut *holds) {
                    let waits = held
                        .family
                        .iter()
                        .any(|one| *one != held.target && state.page(one).is_some());
                    if held.left && !waits {
                        over.push(held);
                    } else {
                        holds.push(held);
                    }
                }
                if !closes.is_empty() {
                    state.say_of_leaving(format!(
                    "purlis closed {} other {} related to the page that held the secret: {} could \
                     have read it. The browser reaches as before once {} gone",
                    closes.len(),
                    if closes.len() == 1 { "window" } else { "windows" },
                    if closes.len() == 1 { "it" } else { "they" },
                    if closes.len() == 1 { "it is" } else { "they are" },
                ));
                }
                holds.iter().any(|held| held.target == target)
            };
        let mut by_page: Vec<(String, Vec<super::PageHeld>)> = Vec::new();
        for held in over {
            match by_page.iter_mut().find(|(page, _)| *page == held.target) {
                Some((_, holds)) => holds.push(held),
                None => by_page.push((held.target.clone(), vec![held])),
            }
        }
        if !its_own_stands && !by_page.iter().any(|(page, _)| page == target) {
            by_page.push((target.to_owned(), Vec::new()));
        }
        for (page, gone) in by_page {
            self.released(&page, gone);
        }
        closes.into_iter().map(FollowUp::Close).collect()
    }

    /// **The holds of page `target` that just went are told so**: each stands as ending until it
    /// is over, and whoever waits to leave the page hears once all of them are.
    pub(super) fn released(&self, target: &str, gone: Vec<super::PageHeld>) {
        lock(&self.leaving_asked).remove(target);
        let over = lock(&self.departing).get(target).cloned();
        if gone.is_empty() {
            if let Some(over) = over {
                let _ = over.send(());
            }
            return;
        }
        let left = Arc::new(AtomicUsize::new(gone.len()));
        for one in gone {
            let origin = one.filled.origin.clone();
            lock(&self.ending).push(origin.clone());
            let ending = Arc::clone(&self.ending);
            let (left, over) = (Arc::clone(&left), over.clone());
            one.hold.released_then(Box::new(move || {
                let mut ending = lock(&ending);
                if let Some(at) = ending.iter().position(|one| *one == origin) {
                    ending.remove(at);
                }
                drop(ending);
                if left.fetch_sub(1, Ordering::SeqCst) == 1
                    && let Some(over) = over
                {
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
    chain_began: Option<String>,
}

impl HeldNow {
    /// Whether `address` is on the origin the chain leading to this page began at.
    fn began_at(&self, address: &str) -> bool {
        login::origin_of(address).is_some_and(|origin| self.chain_began.as_ref() == Some(&origin))
    }
}

/// **Why a form wasn't sent to another site** (SF-15), and how the person approves the key for
/// it. `to` is left unnamed only where its name would say the value.
pub(super) fn form_refused_said(key: &str, origin: &str, to: Option<&str>) -> String {
    let site = to.unwrap_or("another site");
    let how = match to {
        Some(to) if to.starts_with("https://") => format!(
            "To approve it there, give {key} a secret-fill line with fill = \"{to}\" in the \
             persona's tools, or allow it for this chat when a chat fills it on that site. Then \
             open the form and fill it again"
        ),
        Some(_) => "purlis approves a key for another site only over https".to_owned(),
        None => format!(
            "To approve it there, give {key} a secret-fill line for that site in the persona's \
             tools. Then open the form and fill it again"
        ),
    };
    format!(
        "purlis did not send this form to {site}: the page holds the secret {key}, which is \
         approved for {origin} and not for {site}. {how}"
    )
}

/// **A page of purlis's own saying `said`, which the person sees**: the driver answers for its
/// address itself, so nothing is sent anywhere.
fn said_as_a_page(title: &str, said: &str) -> String {
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
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><h1>{title}</h1>\
         <p>{escaped}</p>"
    )
}

/// **Whether `address` holds `value`**, in the spellings a server or a page commonly writes it:
/// as written, URL-encoded any number of times, a space as `+` or `%20`, in base64 or hex, and in
/// any case, as the engine writes a host in lower case. Reads what the engine sent whole, its
/// fragment too. An address encoded past reason counts as holding it.
pub(super) fn carries_value(address: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let lowered = |text: &str| zeroize::Zeroizing::new(text.to_lowercase());
    let spaced = |text: &str| zeroize::Zeroizing::new(text.replace('+', " "));
    let plain = lowered(value);
    let mut forms = vec![spaced(&plain)];
    forms.extend(super::super::scrub::escaped_in_latin1(value).map(|form| spaced(&lowered(&form))));
    let standard = base64::engine::general_purpose::STANDARD_NO_PAD.encode(value);
    let url_safe = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value);
    let hex: String = value.bytes().map(|byte| format!("{byte:02x}")).collect();
    for form in [standard, url_safe, hex] {
        let form = zeroize::Zeroizing::new(form);
        forms.push(spaced(&lowered(&form)));
    }
    let holds = |seen: &str| {
        let seen = spaced(&lowered(seen));
        forms.iter().any(|form| seen.contains(form.as_str()))
    };
    let mut seen = zeroize::Zeroizing::new(address.to_owned());
    for _ in 0..16 {
        if holds(&seen) {
            return true;
        }
        let decoded = zeroize::Zeroizing::new(decoded_once(&seen));
        if *decoded == *seen {
            return false;
        }
        seen = decoded;
    }
    true
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
    use super::{FormSent, carries_value, form_refused_said, said_as_a_page};

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
    fn a_value_with_a_space_and_a_plus_is_found_as_a_form_writes_it() {
        assert!(carries_value("https://app.example/?v=a%2Bb+c", "a+b c"));
    }

    #[test]
    fn a_value_is_found_in_base64_and_hex() {
        let value = "fixture-key-5230";
        for address in [
            "https://app.example/?v=Zml4dHVyZS1rZXktNTIzMA==",
            "https://app.example/?v=Zml4dHVyZS1rZXktNTIzMA",
            "https://app.example/?v=666978747572652d6b65792d35323330",
        ] {
            assert!(carries_value(address, value), "{address}");
        }
    }

    #[test]
    fn an_address_encoded_past_reason_counts_as_holding_it() {
        let mut address = "x".to_owned();
        for _ in 0..20 {
            address = address.replace('%', "%25");
            address.push_str("%41");
        }
        assert!(carries_value(
            &format!("https://app.example/?v={address}"),
            "secret"
        ));
    }

    #[test]
    fn an_address_cut_in_an_escape_is_read_to_its_end() {
        assert!(!carries_value("https://app.example/?a=%", "secret"));
        assert!(!carries_value("https://app.example/?a=%é", "secret"));
        assert!(carries_value("https://app.example/?a=%&b=secret", "secret"));
    }

    #[test]
    fn a_chain_is_the_forms_only_as_the_form_sends_it() {
        let posts = FormSent {
            method: "POST".into(),
            action: "https://a.example/in?next=1".into(),
        };
        assert!(posts.began("POST", "https://a.example/in?next=1"));
        assert!(!posts.began("POST", "https://a.example/in?next=2"));
        assert!(!posts.began("GET", "https://a.example/in?next=1"));
        let gets = FormSent {
            method: "GET".into(),
            action: "https://a.example/in".into(),
        };
        assert!(gets.began("GET", "https://a.example/in?key=1"));
        assert!(!gets.began("GET", "https://a.example/other?key=1"));
    }

    #[test]
    fn a_form_refused_names_the_site_and_how_to_approve_the_key() {
        let said = form_refused_said("dash-key", "https://a.example", Some("https://b.example"));
        assert!(said.contains("https://b.example"), "{said}");
        assert!(said.contains("fill = \"https://b.example\""), "{said}");
        assert!(said.contains("dash-key"), "{said}");
        let plain = form_refused_said("dash-key", "https://a.example", Some("http://b.example"));
        assert!(plain.contains("only over https"), "{plain}");
        let unnamed = form_refused_said("dash-key", "https://a.example", None);
        assert!(unnamed.contains("another site"), "{unnamed}");
    }

    #[test]
    fn a_page_of_purlis_s_own_escapes_what_it_says() {
        let page = said_as_a_page("Form not sent", "<script>&\"");
        assert!(page.contains("&lt;script&gt;&amp;&quot;"), "{page}");
        assert!(!page.contains("<script"), "{page}");
    }
}

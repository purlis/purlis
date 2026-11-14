// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The live ask** (#1666, spec #1661, decisions N-2, N-6, N-10): a chat's connection to a host
//! nothing lists is *held* while the person is asked, and goes on the moment they allow it, so
//! the command that made it carries on with nothing failing and nothing restarting.
//!
//! One [`Asks`] board stands beside each chat's proxy ([`super::egress`]), made when the chat
//! starts and gone when it ends. The proxy asks it of a connection whose decision is
//! [`super::reach::Decision::Ask`] ([`Asks::hold`]); the app answers it from the window alone
//! ([`Asks::allow`], [`Asks::keep_blocked`]), and nothing a connection carries is ever read as
//! an answer.
//!
//! - **Held, then allowed.** A held connection waits up to [`HOLD`]. An Allow at any scope wakes
//!   it, and it is carried as an Allowed host at that scope.
//! - **Held, then timed out.** Nobody answered within [`HOLD`]: the connection is refused as it
//!   was before purlis asked live, and the timeout is told ([`Heard::TimedOut`]). The chat waits
//!   on it no longer, so the app withdraws its ask and writes it as a block (R-290, Q154); a
//!   later connection to it is asked about again. An Allow that comes anyway applies at once,
//!   and answers the hosts that gave up, so the app tells the chat, in fixed words, to retry.
//! - **Given up.** The chat moved on from a held connection ([`Asks::give_up`], R-290, Q240):
//!   it ends now as a timeout does, told the same way, so one path writes its block.
//! - **Grouped.** New hosts one chat reaches within [`GROUP`] of the first are one ask
//!   ([`Heard::Asked`]): told together once the window has passed, each host and port named
//!   whole as the proxy heard it, never a wildcard, so the person answers once.
//! - **Keep blocked** refuses what is held on that host now and what comes after, until an Allow.
//! - **Allow next time** (T1-34, R-291, Q188): once the chat stopped waiting and the refusal is a
//!   block, its Allow next time · once makes a grant with no ask open ([`Asks::allow_next`]). The
//!   chat's next attempt at that target goes on at once, never held or asked about, and uses it
//!   up, as the person's grant; one held already when it lands takes it (Q278). Until then it is
//!   listed ([`Asks::next_attempts`]), and it can be removed ([`Asks::forget_next`]). It is held
//!   in memory alone, so a chat, session host or app restart forgets it; its use and an end
//!   without its attempt are told ([`Heard::UsedNext`], [`Heard::DroppedNext`]), so each is
//!   audited. A wider label is an Allow at its scope ([`Asks::allow`]), which stays until it is
//!   removed.
//! - **Bounded.** At most [`MOST_HELD`] connections are held at once; one more is refused at
//!   once, as before, and not asked about ([`Answer::Busy`]).
//! - **Held open while the person is Quiet** (ADR 0121 decision 13, #6007). While the window
//!   says so ([`Asks::hold_open`]), a held connection past its [`HOLD`] waits on, up to
//!   [`MOST_HELD_OPEN`] in all; once it says no more, one past its hold times out at once, as
//!   any timed-out one does. The program's own connect timeout
//!   and its harness's command timeout still bound it: purlis holds, it cannot make them wait.
//! - **Nobody to ask.** Where no Notice can be raised ([`Asks::asking_while`]: the project's
//!   hooks are not listening yet, or the chat has no number yet), a connection is refused at
//!   once, as before, and not held a minute for an ask nobody sees ([`Answer::NobodyToAsk`]).

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::egress::{allows_on, host_and_port};
use super::hosts::Host;
use super::reach::By;

/// How long a held connection waits for the person: about a minute (N-6).
pub const HOLD: Duration = Duration::from_secs(60);

/// The most a connection is held while held open past its hold (#6007): what keeps a Quiet
/// that lasts all afternoon from holding a connection all afternoon.
pub const MOST_HELD_OPEN: Duration = Duration::from_secs(15 * 60);

/// How close together new hosts from one chat must come to be one ask (N-10).
pub const GROUP: Duration = Duration::from_secs(2);

/// The most connections one chat's board holds at once.
pub const MOST_HELD: usize = 16;

/// The most hosts one board remembers as given up on, kept blocked, or allowed live.
const MOST_KEPT: usize = 64;

/// What a held connection is answered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The person allowed it, at this scope: carried.
    Allowed(By),
    /// The person kept it blocked: refused.
    Refused,
    /// Nobody answered in time, or the chat moved on ([`Asks::give_up`]): refused, and a block
    /// from now on (R-290), not an ask.
    TimedOut,
    /// The board holds all it holds: refused at once, not asked about.
    Busy,
    /// No Notice can be raised now: refused at once, not asked about.
    NobodyToAsk,
}

/// What a board tells the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// Ask the person about these hosts, each `host:port` whole, in the order they came: one
    /// Notice, one decision.
    Asked(Vec<String>),
    /// A connection to `host:port` was held and nobody answered, or the chat moved on from it:
    /// it was refused.
    TimedOut(String),
    /// **The chat's next attempt at `host` used up its Allow next time · once** (T1-34, Q278):
    /// let through as the person's grant, and ended, so the app audits its end.
    UsedNext(String),
    /// **An Allow next time · once of `host` ended without its attempt** (T1-34, Q278): put out
    /// by a newer one past the most a board keeps, or by a Keep blocked that covers it. The app
    /// audits its end.
    DroppedNext(String),
}

/// Who hears what a board tells.
pub type Tell = Arc<dyn Fn(Heard) + Send + Sync + 'static>;

#[derive(Default)]
struct State {
    /// Hosts allowed while the chat runs, each with its scope, as a grant writes them.
    live: Vec<(String, By)>,
    /// Ports of this machine left out of the chat's plan that the person allowed for it while
    /// its confinement runs (#2510): never a grant, and never told as one
    /// ([`Asks::allowed_live`]).
    left_out_allowed: Vec<u16>,
    /// Ports of this machine left out of the chat's plan that the person kept blocked, until an
    /// Allow (#2510).
    left_out_blocked: Vec<u16>,
    /// Hosts kept blocked, until an Allow.
    blocked: Vec<Host>,
    /// Targets allowed for the chat's next attempt alone (T1-34, R-291): each goes as an attempt
    /// it covers goes on, held or not yet. **In memory only** (Q278): a chat, session host or app
    /// restart forgets one not used yet, as the merge hold is forgotten. Its grant, its use, its
    /// removal and its eviction are each audited ([`Heard::UsedNext`], [`Heard::DroppedNext`]).
    next: Vec<Host>,
    /// Targets held and given up on, which an Allow tells the chat to retry.
    gave_up: Vec<String>,
    /// Targets asked about and not answered yet, so each is asked once.
    asked: Vec<String>,
    /// The ask being gathered: its targets, and when it is told.
    gathering: Option<(Vec<String>, Instant)>,
    holding: usize,
    /// Each connection held now (R-290).
    held: Vec<HeldNow>,
    /// The tickets of held connections the chat moved on from ([`Asks::give_up`]).
    cut: Vec<u64>,
    /// The next connection's ticket.
    tickets: u64,
    /// Whether a connection past its hold waits on ([`Asks::hold_open`], #6007).
    held_open: bool,
}

/// A connection a board holds now, by its ticket (R-290).
struct HeldNow {
    ticket: u64,
    /// Its host and port, as [`Heard`] names it.
    target: String,
    named: String,
    port: u16,
    defaults: Vec<u16>,
}

/// **One chat's live asks** (module docs).
pub struct Asks {
    state: Mutex<State>,
    changed: Condvar,
    tell: Tell,
    hold: Duration,
    group: Duration,
    /// The most a connection is held while held open ([`MOST_HELD_OPEN`]).
    most_open: Duration,
    known: Option<Known>,
    askable: Option<Askable>,
}

/// Whether a Notice can be raised now ([`Asks::asking_while`]).
pub type Askable = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

/// Where a host the person allowed already, since the chat started, is found ([`Asks::knowing`]):
/// the scope it is allowed at, or none.
pub type Known = Arc<dyn Fn(&Host) -> Option<By> + Send + Sync + 'static>;

impl std::fmt::Debug for Asks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Asks")
            .field("holding", &self.holding())
            .finish_non_exhaustive()
    }
}

impl Asks {
    /// A board that tells `tell`, holding for [`HOLD`] and grouping over [`GROUP`].
    pub fn new(tell: Tell) -> Self {
        Self::timed(tell, HOLD, GROUP)
    }

    /// A board that holds for `hold` and groups over `group`.
    pub fn timed(tell: Tell, hold: Duration, group: Duration) -> Self {
        Self {
            state: Mutex::default(),
            changed: Condvar::new(),
            tell,
            hold,
            group,
            most_open: MOST_HELD_OPEN,
            known: None,
            askable: None,
        }
    }

    /// This board, holding a connection open at most `most` in all ([`MOST_HELD_OPEN`]).
    #[must_use]
    pub fn held_open_at_most(self, most: Duration) -> Self {
        Self {
            most_open: most,
            ..self
        }
    }

    /// **Whether a held connection past its hold waits on** (ADR 0121 decision 13, #6007): the
    /// window says so while the person is Quiet, and no more when Quiet ends, when one past its
    /// hold times out at once.
    pub fn hold_open(&self, open: bool) {
        self.state().held_open = open;
        self.changed.notify_all();
    }

    /// This board, letting a connection on at once, never held or asked about, where `known`
    /// finds its host allowed already: a person's yes given since the chat started (#1666).
    #[must_use]
    pub fn knowing(self, known: Known) -> Self {
        Self {
            known: Some(known),
            ..self
        }
    }

    /// This board, holding a connection only while `askable` says a Notice can be raised: at
    /// any other time a connection nothing lets on is refused at once ([`Answer::NobodyToAsk`]),
    /// so nothing waits a minute on an ask nobody sees.
    #[must_use]
    pub fn asking_while(self, askable: Askable) -> Self {
        Self {
            askable: Some(askable),
            ..self
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How many connections it holds now.
    pub fn holding(&self) -> usize {
        self.state().holding
    }

    /// Whether the person is being asked about `host`: a connection to it is held, or gave up
    /// and its ask is still unanswered.
    pub fn asks_about(&self, host: &Host) -> bool {
        asks_about_among(&self.state().asked, host)
    }

    /// The hosts and ports the person is being asked about now, each whole, in the order they
    /// came: what the session host tells the window of a board it holds (#1999).
    pub fn asked(&self) -> Vec<String> {
        self.state().asked.clone()
    }

    /// The scope a host allowed live lets `host` on `port` through at, where a host listed
    /// without a port is carried on `defaults`.
    pub fn allows(&self, host: &str, port: u16, defaults: &[u16]) -> Option<By> {
        allowed_in(&self.state(), host, port, defaults)
    }

    /// **Holds a connection to `host` on `port`** until the person answers or [`HOLD`] passes:
    /// what the proxy asks of a connection nothing lists. A host listed without a port is
    /// carried on `defaults`. Answered at once where a live Allow or Keep blocked already
    /// covers it, or where the board is full.
    pub fn hold(&self, host: &str, port: u16, defaults: &[u16]) -> Answer {
        let named = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
        let target = host_and_port(&named, port);
        let parsed = Host::parse(&target).ok();
        // Read before the board is held: it reads files.
        let known = match (&self.known, &parsed) {
            (Some(known), Some(host)) => known(host),
            _ => None,
        };
        let mut state = self.state();
        if let Some(by) = allowed_in(&state, &named, port, defaults) {
            return Answer::Allowed(by);
        }
        // The person's Keep blocked for this chat holds even for a host allowed already: the
        // later answer wins.
        if kept_blocked(&state, parsed.as_ref(), &named, port) {
            return Answer::Refused;
        }
        // Allowed for its next attempt (T1-34, R-291): on at once, never asked about, and used
        // up by this attempt, under the same hold, so two attempts never both take it. It is the
        // person's grant, never one the chat holds (Q278), and its use is told, so it is audited.
        if let Some(used) = take_next(&mut state, parsed.as_ref()) {
            drop(state);
            (self.tell)(Heard::UsedNext(used));
            return Answer::Allowed(By::You);
        }
        // Allowed already, since the chat started: on at once, and carried from now on. Kept
        // under the same hold, so a Keep blocked cannot come between.
        if let (Some(by), Some(host)) = (known, &parsed) {
            allow_in(&mut state, host, by);
            drop(state);
            self.changed.notify_all();
            return Answer::Allowed(by);
        }
        if state.holding >= MOST_HELD {
            return Answer::Busy;
        }
        if self.askable.as_ref().is_some_and(|askable| !askable()) {
            return Answer::NobodyToAsk;
        }
        state.holding += 1;
        let ticket = state.tickets;
        state.tickets += 1;
        state.held.push(HeldNow {
            ticket,
            target: target.clone(),
            named: named.clone(),
            port,
            defaults: defaults.to_vec(),
        });
        let began = Instant::now();
        let deadline = began + self.hold;
        let most = began + self.most_open.max(self.hold);
        // Asked once: a host already asked about, and not answered, waits on that ask.
        if !state.asked.contains(&target) {
            state.asked.push(target.clone());
            keep_within(&mut state.asked);
            match &mut state.gathering {
                Some((targets, _)) => targets.push(target.clone()),
                None => state.gathering = Some((vec![target.clone()], Instant::now() + self.group)),
            }
        }
        // An Allow next time · once that lands while it is held takes it (Q278).
        let mut used = None;
        let answer = loop {
            if let Some(by) = allowed_in(&state, &named, port, defaults) {
                break Answer::Allowed(by);
            }
            if kept_blocked(&state, parsed.as_ref(), &named, port) {
                break Answer::Refused;
            }
            if let Some(taken) = take_next(&mut state, parsed.as_ref()) {
                used = Some(taken);
                break Answer::Allowed(By::You);
            }
            // The chat moved on from it (R-290): refused now, as a timeout is.
            if state.cut.contains(&ticket) {
                break Answer::TimedOut;
            }
            let now = Instant::now();
            // The window has passed: whichever held connection sees it first tells the ask,
            // once, outside the lock.
            if state.gathering.as_ref().is_some_and(|(_, at)| now >= *at) {
                let targets = state
                    .gathering
                    .take()
                    .map(|(targets, _)| targets)
                    .unwrap_or_default();
                drop(state);
                (self.tell)(Heard::Asked(targets));
                state = self.state();
                continue;
            }
            // Past its hold, it waits on only while held open, and never past the most.
            if now >= deadline && (!state.held_open || now >= most) {
                break Answer::TimedOut;
            }
            let ends = if now >= deadline { most } else { deadline };
            let until = state
                .gathering
                .as_ref()
                .map_or(ends, |(_, at)| (*at).min(ends));
            state = self
                .changed
                .wait_timeout(state, until.saturating_duration_since(now))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        };
        state.holding -= 1;
        state.held.retain(|one| one.ticket != ticket);
        state.cut.retain(|one| *one != ticket);
        if state.holding == 0 {
            // Nothing is waiting on an ask not told yet: there is nobody to ask for.
            state.gathering = None;
        }
        // Answered or given up: the next connection to it is asked about again, so one whose
        // Notice was put away is not held a minute for nothing.
        state.asked.retain(|one| *one != target);
        if answer == Answer::TimedOut && !state.gave_up.contains(&target) {
            state.gave_up.push(target.clone());
            keep_within(&mut state.gave_up);
        }
        drop(state);
        self.changed.notify_all();
        if answer == Answer::TimedOut {
            (self.tell)(Heard::TimedOut(target));
        }
        if let Some(used) = used {
            (self.tell)(Heard::UsedNext(used));
        }
        answer
    }

    /// **The chat waits on what is held on `target`, a host and port as [`Heard`] names it, no
    /// longer** (R-290, Q240): it moved on, so each connection held there now ends as a timeout
    /// does ([`Answer::TimedOut`], told as [`Heard::TimedOut`]), and a later connection to it is
    /// asked about again. An Allow or a Keep blocked that reached it first wins. Whether one was
    /// held there.
    pub fn give_up(&self, target: &str) -> bool {
        let mut state = self.state();
        let mut cut = Vec::new();
        for held in &state.held {
            // An answer that reached it first wins, whenever its connection wakes to read it.
            let parsed = Host::parse(&held.target).ok();
            let answered = allowed_in(&state, &held.named, held.port, &held.defaults).is_some()
                || kept_blocked(&state, parsed.as_ref(), &held.named, held.port)
                || next_for(&state, parsed.as_ref()).is_some();
            if held.target == target && !answered {
                cut.push(held.ticket);
            }
        }
        let any = !cut.is_empty();
        state.cut.extend(cut);
        drop(state);
        if any {
            self.changed.notify_all();
        }
        any
    }

    /// **The person allowed `host` at scope `by`**, from the window: every held connection it
    /// covers goes on now, and every later one is carried. Answers the hosts and ports that were
    /// held and gave up before this Allow, once each, which the chat is told to retry.
    pub fn allow(&self, host: &Host, by: By) -> Vec<String> {
        let retry = allow_in(&mut self.state(), host, by);
        self.changed.notify_all();
        retry
    }

    /// **The person allowed the service on `port` of this machine, left out of the chat's
    /// plan, for this chat** (#2510): a connection held on it goes on now, and every later one
    /// is carried while the confinement runs. Only a chat planned on another machine is asked
    /// about one ([`super::egress::Serving::left_out`]). Answers the targets that were held on
    /// it and gave up, which the chat is told to retry.
    pub fn allow_left_out(&self, port: u16) -> Vec<String> {
        let mut state = self.state();
        if !state.left_out_allowed.contains(&port) {
            state.left_out_allowed.push(port);
        }
        state.left_out_blocked.retain(|one| *one != port);
        let covered = |target: &String| super::localport::loopback_port(target) == Some(port);
        let retry: Vec<String> = state
            .gave_up
            .iter()
            .filter(|t| covered(t))
            .cloned()
            .collect();
        state.gave_up.retain(|target| !covered(target));
        state.asked.retain(|target| !covered(target));
        drop(state);
        self.changed.notify_all();
        retry
    }

    /// **The person kept `host` blocked**, from the window: what is held on it is refused now,
    /// and so is every later connection to it, until an Allow. The later answer wins: an
    /// earlier live Allow on this board of a host it covers, or that covers it, ends with it,
    /// as [`Self::allow`] ends a Keep blocked it covers. Whether it ended one.
    pub fn keep_blocked(&self, host: &Host) -> bool {
        let mut state = self.state();
        let before = state.live.len();
        state.live.retain(|(entry, _)| {
            Host::parse(entry).is_ok_and(|allowed| !host.covers(&allowed) && !allowed.covers(host))
        });
        let ended = state.live.len() != before;
        // Allow next time ends with it too: the later answer wins, and each it ends is told, so
        // its end is audited (Q278).
        let (nexts_ended, kept): (Vec<Host>, Vec<Host>) = std::mem::take(&mut state.next)
            .into_iter()
            .partition(|allowed| host.covers(allowed) || allowed.covers(host));
        state.next = kept;
        if !state.blocked.contains(host) {
            state.blocked.push(host.clone());
            if state.blocked.len() > MOST_KEPT {
                state.blocked.remove(0);
            }
        }
        let other = |target: &String| Host::parse(target).is_ok_and(|one| !host.covers(&one));
        state.gave_up.retain(other);
        state.asked.retain(other);
        drop(state);
        self.changed.notify_all();
        for one in nexts_ended {
            (self.tell)(Heard::DroppedNext(one.to_string()));
        }
        ended
    }

    /// The hosts allowed live on this board, as a grant writes each, with the scope each was
    /// allowed at: what a run for the chat started now carries besides what the chat was
    /// compiled with (#1667), decided at that scope (#1708).
    /// A host kept blocked is never among them.
    pub fn allowed_live(&self) -> Vec<(String, By)> {
        let state = self.state();
        state
            .live
            .iter()
            .filter(|(entry, _)| {
                Host::parse(entry).is_ok_and(|allowed| !state.blocked.contains(&allowed))
            })
            .cloned()
            .collect()
    }

    /// **An Allow of `host` at scope `by` was removed**: what it allowed live reaches nothing
    /// again, unless the same host is allowed live at another scope.
    pub fn forget(&self, host: &Host, by: By) {
        let entry = host.to_string();
        self.state()
            .live
            .retain(|(one, kept)| *one != entry || *kept != by);
    }

    /// **Allow next time · once, from a block** (T1-34, R-291, Q188): `host` is let on for the
    /// chat's next attempt at it alone, with no ask open. That attempt goes on at once and uses
    /// it up ([`Self::hold`]), and one held already when it lands takes it (Q278). A Keep
    /// blocked it covers ends, as an Allow ends one: the later answer wins. Made once: allowing
    /// it again while it waits changes nothing. **Held in memory alone** (Q278): a chat, session
    /// host or app restart forgets it. The oldest put out past the most a board keeps is told
    /// ([`Heard::DroppedNext`]), so its end is audited.
    pub fn allow_next(&self, host: &Host) {
        let mut state = self.state();
        state.blocked.retain(|one| !host.covers(one));
        let mut dropped = None;
        if !state.next.contains(host) {
            state.next.push(host.clone());
            if state.next.len() > MOST_KEPT {
                dropped = Some(state.next.remove(0));
            }
        }
        drop(state);
        self.changed.notify_all();
        if let Some(dropped) = dropped {
            (self.tell)(Heard::DroppedNext(dropped.to_string()));
        }
    }

    /// The targets allowed for the chat's next attempt and not used yet, each as a grant
    /// writes it: what the Allowed list shows of them until they are used or removed.
    pub fn next_attempts(&self) -> Vec<String> {
        self.state().next.iter().map(ToString::to_string).collect()
    }

    /// **An Allow next time of `host` was removed** before it was used: the next attempt at it
    /// is held and asked about again.
    pub fn forget_next(&self, host: &Host) {
        self.state().next.retain(|one| one != host);
    }
}

impl Asks {
    /// **The person kept the service on `port` of this machine, left out of the chat's plan,
    /// blocked** (#2510): what is held on it is refused now, and so is every later connection
    /// to it, until an Allow.
    pub fn keep_left_out_blocked(&self, port: u16) {
        let mut state = self.state();
        if !state.left_out_blocked.contains(&port) {
            state.left_out_blocked.push(port);
        }
        state.left_out_allowed.retain(|one| *one != port);
        let other = |target: &String| super::localport::loopback_port(target) != Some(port);
        state.gave_up.retain(other);
        state.asked.retain(other);
        drop(state);
        self.changed.notify_all();
    }

    /// **An Allow of the service on `port` left out of the chat's plan was removed** (#2510): it
    /// reaches nothing again, and is asked about again.
    pub fn forget_left_out(&self, port: u16) {
        self.state().left_out_allowed.retain(|one| *one != port);
    }
}

/// Whether `host` covers one of `asked`, each a host and port a board asks about
/// ([`Asks::asked`]): [`Asks::asks_about`], of a board read elsewhere (#1999).
pub fn asks_about_among(asked: &[String], host: &Host) -> bool {
    asked
        .iter()
        .any(|target| Host::parse(target).is_ok_and(|one| host.covers(&one)))
}

/// **`host` allowed live at scope `by` in `state`** ([`Asks::allow`]): a Keep blocked it covers
/// ends, and the hosts and ports held connections gave up on that it covers are answered, once
/// each, for the chat to retry.
fn allow_in(state: &mut State, host: &Host, by: By) -> Vec<String> {
    let entry = host.to_string();
    if !state
        .live
        .iter()
        .any(|(one, kept)| *one == entry && *kept == by)
    {
        state.live.push((entry, by));
        if state.live.len() > MOST_KEPT {
            state.live.remove(0);
        }
    }
    state.blocked.retain(|one| !host.covers(one));
    let covered = |target: &String| Host::parse(target).is_ok_and(|one| host.covers(&one));
    let retry: Vec<String> = state
        .gave_up
        .iter()
        .filter(|t| covered(t))
        .cloned()
        .collect();
    state.gave_up.retain(|target| !covered(target));
    state.asked.retain(|target| !covered(target));
    retry
}

/// The scope a live Allow in `state` carries `host` on `port` at.
fn allowed_in(state: &State, host: &str, port: u16, defaults: &[u16]) -> Option<By> {
    if state.left_out_allowed.contains(&port) && super::localport::names_loopback(host, port) {
        return Some(By::Chat);
    }
    state
        .live
        .iter()
        .find(|(entry, _)| allows_on(std::slice::from_ref(entry), host, port, defaults))
        .map(|(_, by)| *by)
}

/// Where among `state`'s waiting Allow next time · once one covers `host`, if one does.
fn next_for(state: &State, host: Option<&Host>) -> Option<usize> {
    let host = host?;
    state.next.iter().position(|one| one.covers(host))
}

/// **Takes the Allow next time · once that covers `host`**, if one waits (T1-34, R-291): used
/// up, under the board's hold, so two attempts never both take it. The target as its grant
/// wrote it, which its audit names.
fn take_next(state: &mut State, host: Option<&Host>) -> Option<String> {
    let at = next_for(state, host)?;
    Some(state.next.remove(at).to_string())
}

/// Whether `target`, `host` on `port`, is one kept blocked in `state`: a host, or a service
/// left out of the chat's plan (#2510).
fn kept_blocked(state: &State, target: Option<&Host>, host: &str, port: u16) -> bool {
    target.is_some_and(|target| state.blocked.iter().any(|one| one.covers(target)))
        || (state.left_out_blocked.contains(&port) && super::localport::names_loopback(host, port))
}

/// Lets the oldest go past [`MOST_KEPT`].
fn keep_within(kept: &mut Vec<String>) {
    if kept.len() > MOST_KEPT {
        let over = kept.len() - MOST_KEPT;
        kept.drain(..over);
    }
}

#[cfg(test)]
mod tests;

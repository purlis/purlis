// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whose turn the specimen painter is** (#2614): one specimen is drawn at a time, and **a
//! chat's `theme_preview` goes ahead of Settings › Theme's pictures**, so opening the gallery
//! never makes a chat wait on it; **and the gallery is owed a picture after a chat's turn**
//! (#2751), so chats asking again and again never leave it none.
//!
//! - **A gallery picture** takes the painter only when it is free and no chat is waiting for
//!   it; otherwise it is answered busy at once, and the window asks again later.
//! - **A chat** that finds a gallery picture being drawn waits for it, and that picture gives
//!   way: its painter checks [`Turn::gives_way`] while it waits on the page and stops, answering
//!   busy, so the chat is drawn next. A chat waits [`Turns::chat`]'s `within` at most.
//! - **A chat that finds another chat's specimen being drawn**, or another chat waiting, is
//!   answered busy at once, as before: asks never pile up behind one another, and none is drawn
//!   for a caller that has already stopped waiting.
//! - **The gallery is owed one picture** after any chat's turn during which it was answered busy
//!   or gave way: until it has taken the painter once, or [`OWED`] has passed since that turn
//!   ended, every chat is answered busy at once. The picture it then draws does not give way;
//!   a chat asked meanwhile waits for it as for any picture. So between two chats' turns, the
//!   gallery that asked is drawn one picture whole, however many chats ask.
//!
//! A [`Turn`] holds the painter until it is dropped, however its drawing ends.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// **How often a gallery picture looks for a chat waiting** while its page draws: the most a
/// chat waits on one, past the page's own step.
pub const LOOK: Duration = Duration::from_millis(50);

/// **How long the gallery is owed its picture** after a chat's turn it asked during: one
/// specimen's budget, far longer than the window takes to ask again (`BUSY_WAIT_MS`, a second).
pub const OWED: Duration = super::BUDGET;

/// Who holds the painter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// A chat's `theme_preview`.
    Chat,
    /// One of Settings › Theme's pictures, asked by the window.
    Gallery,
}

#[derive(Debug, Default)]
struct State {
    held: Option<Who>,
    /// Whether the turn held is a gallery picture it was owed, which does not give way.
    held_owed: bool,
    /// How many chats wait for the painter: at most one, since a second is answered busy.
    chats_waiting: usize,
    /// Whether the gallery was answered busy, or gave way, since the last chat's turn ended.
    gallery_asked: bool,
    /// Until when the gallery is owed its picture: chats are answered busy until then, or until
    /// it takes the painter.
    owed_until: Option<Instant>,
}

/// **The painter's turns**: one holder at a time, a chat ahead of the gallery, and the gallery
/// owed a picture after a chat's turn it asked during.
#[derive(Debug)]
pub struct Turns {
    state: Mutex<State>,
    freed: Condvar,
    owed_for: Duration,
}

impl Default for Turns {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            freed: Condvar::new(),
            owed_for: OWED,
        }
    }
}

impl Turns {
    pub fn new() -> Self {
        Self::default()
    }

    /// These turns, the gallery owed its picture for `owed_for` rather than [`OWED`].
    #[cfg(test)]
    fn owing_for(owed_for: Duration) -> Self {
        Self {
            owed_for,
            ..Self::default()
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **A turn for one of the gallery's pictures**: the painter's when it is free and no chat
    /// waits for it; `None`, to answer busy, otherwise. A turn the gallery was owed does not
    /// give way.
    pub fn gallery(&self) -> Option<Turn<'_>> {
        let mut state = self.state();
        if state.held.is_some() || state.chats_waiting > 0 {
            state.gallery_asked = true;
            return None;
        }
        let owed = state
            .owed_until
            .take()
            .is_some_and(|until| Instant::now() < until);
        state.held = Some(Who::Gallery);
        state.held_owed = owed;
        Some(Turn {
            turns: self,
            who: Who::Gallery,
            owed,
        })
    }

    /// **A turn for a chat**: at once when the painter is free; after the gallery picture being
    /// drawn gives way, or ends, waiting `within` at most; `None`, to answer busy, when another
    /// chat holds or waits for the painter, the gallery is owed its picture, or the gallery's
    /// picture did not end in time.
    pub fn chat(&self, within: Duration) -> Option<Turn<'_>> {
        let mut state = self.state();
        if state.held == Some(Who::Chat) || state.chats_waiting > 0 {
            return None;
        }
        if let Some(until) = state.owed_until {
            if Instant::now() < until {
                return None;
            }
            state.owed_until = None;
        }
        if state.held.is_some() {
            // A picture that gives way is one the gallery asked for and is not drawn.
            if !state.held_owed {
                state.gallery_asked = true;
            }
            state.chats_waiting += 1;
            let (waited, _) = self
                .freed
                .wait_timeout_while(state, within, |state| state.held.is_some())
                .unwrap_or_else(PoisonError::into_inner);
            state = waited;
            state.chats_waiting -= 1;
            if state.held.is_some() {
                return None;
            }
        }
        state.held = Some(Who::Chat);
        state.held_owed = false;
        Some(Turn {
            turns: self,
            who: Who::Chat,
            owed: false,
        })
    }

    /// Who holds the painter now.
    pub fn held(&self) -> Option<Who> {
        self.state().held
    }
}

/// **The painter, held**: until this is dropped.
#[derive(Debug)]
pub struct Turn<'a> {
    turns: &'a Turns,
    who: Who,
    owed: bool,
}

impl Turn<'_> {
    /// Who this turn is.
    pub fn who(&self) -> Who {
        self.who
    }

    /// Whether this is a gallery picture it was owed after a chat's turn.
    pub fn owed(&self) -> bool {
        self.owed
    }

    /// **Whether this turn gives way now**: a gallery picture's, while a chat waits for the
    /// painter, unless the gallery was owed it. A chat's never does.
    pub fn gives_way(&self) -> bool {
        self.who == Who::Gallery && !self.owed && self.turns.state().chats_waiting > 0
    }

    /// **What `answer` is sent, waiting until `until` at most**, and looking every [`LOOK`]
    /// whether this turn gives way. [`Waited::GaveWay`] when it did, [`Waited::Over`] past
    /// `until` or when nothing can send any more.
    pub fn wait<T>(&self, answer: &Receiver<T>, until: Instant) -> Result<T, Waited> {
        loop {
            if self.gives_way() {
                return Err(Waited::GaveWay);
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Waited::Over);
            }
            match answer.recv_timeout(left.min(LOOK)) {
                Ok(answered) => return Ok(answered),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(Waited::Over),
            }
        }
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        let mut state = self.turns.state();
        state.held = None;
        state.held_owed = false;
        // A chat's turn the gallery asked during: the gallery is owed its picture next.
        if self.who == Who::Chat && std::mem::take(&mut state.gallery_asked) {
            state.owed_until = Some(Instant::now() + self.turns.owed_for);
        }
        drop(state);
        self.turns.freed.notify_all();
    }
}

/// Why [`Turn::wait`] has no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// A chat waits for the painter, and this gallery picture gave way to it.
    GaveWay,
    /// The time ran out, or nothing can answer any more.
    Over,
}

#[cfg(test)]
mod tests;

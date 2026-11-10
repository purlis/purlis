// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A permission hook's ask that timed out stays answerable** (ADR 0121 decision 15, #6007).
//!
//! A harness kills its permission hook at [`super::HOOK_TIMEOUT`], and its own prompt then asks
//! in the chat's pane. Nothing purlis does can reach that hook again. So the ask is kept here as
//! **lapsed**, and an answer to it is kept as a **pre-answer**: the chat's next request that is
//! the same (the same action, and for an edit the same change) is answered with it at once, by
//! its own hook, and no one is asked again. The app tells the chat the answer on its next turn
//! as well, so it knows to make the request again ([`crate::askoutcome::Answer::AnsweredLate`]).
//! This is the same on every harness whose permission hook purlis holds: Claude Code, Codex and
//! opencode.
//!
//! - **A lapsed ask goes** when it is answered, when its chat asks the same again (the new ask
//!   is the one to answer), when its chat ends ([`Lapsing::forget`]), or after [`LAPSED_FOR`].
//!   At most [`MOST_LAPSED`] are kept per chat, the oldest going first.
//! - **A pre-answer is spent once**, and goes after [`PRE_ANSWERED_FOR`] or when its chat ends.
//! - **Only a request a later one can be the same as lapses**: a command, an edit or a tool. A
//!   question to the person, a form and an unsaid nudge are asked anew or not at all, so they
//!   go when they time out, as before.
//!
//! Pure bookkeeping on times handed in: nothing here reads a clock.

use std::time::{Duration, Instant};

use super::super::asks::{AskId, Raised, Refused};
use super::super::model::{Action, Ask, Choice, Writes};

/// How long a lapsed ask stays listed for the person to answer.
pub const LAPSED_FOR: Duration = Duration::from_secs(30 * 60);

/// How long an answer to a lapsed ask waits for its chat's same request.
pub const PRE_ANSWERED_FOR: Duration = Duration::from_secs(30 * 60);

/// The most lapsed asks one chat keeps, and the most pre-answers it holds.
pub const MOST_LAPSED: usize = super::MOST_OPEN_ASKS;

/// **A hook ask that timed out**, as the window lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lapsed {
    pub raised: Raised,
    /// When it was raised.
    pub at: Instant,
    /// When it timed out.
    pub ended: Instant,
}

/// **The person's answer to a lapsed ask**, as its chat is told it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Late {
    pub id: AskId,
    pub chat: String,
    /// What it asked, as its row said it: the masked one line.
    pub said: String,
    pub choice: Choice,
}

#[derive(Debug)]
struct PreAnswer {
    chat: String,
    action: Action,
    writes: Option<Writes>,
    choice: Choice,
    until: Instant,
}

/// The lapsed asks and pre-answers of one host's hooks.
#[derive(Debug, Default)]
pub struct Lapsing {
    lapsed: Vec<Lapsed>,
    pre: Vec<PreAnswer>,
}

/// Whether a later request can be the same as `ask`, so an answer to it can wait for one.
fn lapses(ask: &Ask) -> bool {
    matches!(
        ask.action,
        Action::Command { .. } | Action::Edit { .. } | Action::Tool { .. }
    ) && !ask.elicits_secret
        && !ask.options.is_empty()
}

/// Whether `ask` from `chat` is the same request as `action` with `writes` from `of`.
fn same(chat: &str, ask: &Ask, of: &str, action: &Action, writes: Option<&Writes>) -> bool {
    chat == of && ask.action == *action && ask.writes.as_ref() == writes
}

impl Lapsing {
    fn forget_old(&mut self, now: Instant) {
        self.lapsed
            .retain(|one| now.saturating_duration_since(one.ended) < LAPSED_FOR);
        self.pre.retain(|one| now < one.until);
    }

    /// Keeps `raised`, raised `at`, as lapsed at `now`, where a later request can be the same.
    pub fn lapse(&mut self, raised: Raised, at: Instant, now: Instant) {
        self.forget_old(now);
        if !lapses(&raised.ask) {
            return;
        }
        let chat = raised.chat.clone();
        self.lapsed.push(Lapsed {
            raised,
            at,
            ended: now,
        });
        while self
            .lapsed
            .iter()
            .filter(|one| one.raised.chat == chat)
            .count()
            > MOST_LAPSED
        {
            let oldest = self
                .lapsed
                .iter()
                .position(|one| one.raised.chat == chat)
                .expect("counted above");
            self.lapsed.remove(oldest);
        }
    }

    /// Every lapsed ask still listed at `now`, oldest first.
    pub fn listed(&mut self, now: Instant) -> Vec<Lapsed> {
        self.forget_old(now);
        self.lapsed.clone()
    }

    /// **The person answered lapsed ask `id` of chat `chat` with `choice`** at `now`: it goes,
    /// and the answer waits for the chat's same request. An id that is no lapsed ask of this
    /// chat's is [`Refused::Unknown`]; an option it did not offer, [`Refused::NotAnOption`].
    /// `may` is the registry's own check of who may choose what, as for a live ask.
    pub fn answer(
        &mut self,
        chat: &str,
        id: &AskId,
        choice: &str,
        now: Instant,
        may: impl FnOnce(&Ask, &Choice) -> Result<(), Refused>,
    ) -> Result<Late, Refused> {
        self.forget_old(now);
        let at = self
            .lapsed
            .iter()
            .position(|one| one.raised.id == *id && one.raised.chat == chat)
            .ok_or(Refused::Unknown)?;
        let ask = &self.lapsed[at].raised.ask;
        let chosen = ask
            .options
            .iter()
            .find(|option| option.id == choice)
            .cloned()
            .ok_or_else(|| Refused::NotAnOption(choice.to_owned()))?;
        may(ask, &chosen)?;
        let lapsed = self.lapsed.remove(at);
        let ask = lapsed.raised.ask;
        self.pre
            .retain(|one| !same(chat, &ask, &one.chat, &one.action, one.writes.as_ref()));
        self.pre.push(PreAnswer {
            chat: chat.to_owned(),
            action: ask.action.clone(),
            writes: ask.writes.clone(),
            choice: chosen.clone(),
            until: now + PRE_ANSWERED_FOR,
        });
        while self.pre.iter().filter(|one| one.chat == chat).count() > MOST_LAPSED {
            let oldest = self
                .pre
                .iter()
                .position(|one| one.chat == chat)
                .expect("counted above");
            self.pre.remove(oldest);
        }
        Ok(Late {
            id: lapsed.raised.id,
            chat: chat.to_owned(),
            said: ask.summary.as_str().to_owned(),
            choice: chosen,
        })
    }

    /// **Chat `chat` asks `ask` now**: the answer waiting for it, spent, where the person
    /// answered the same request after it timed out and it offers that option still. Every
    /// lapsed ask of the chat's that is the same request goes either way: this one is answered,
    /// or is the one to answer.
    pub fn asked(&mut self, chat: &str, ask: &Ask, now: Instant) -> Option<Choice> {
        self.forget_old(now);
        self.lapsed.retain(|one| {
            !same(
                chat,
                ask,
                &one.raised.chat,
                &one.raised.ask.action,
                one.raised.ask.writes.as_ref(),
            )
        });
        let at = self
            .pre
            .iter()
            .position(|one| same(chat, ask, &one.chat, &one.action, one.writes.as_ref()))?;
        let pre = self.pre.remove(at);
        ask.options
            .iter()
            .find(|option| option.id == pre.choice.id && option.kind == pre.choice.kind)
            .cloned()
    }

    /// Chat `chat` ended: nothing of it is kept.
    pub fn forget(&mut self, chat: &str) {
        self.lapsed.retain(|one| one.raised.chat != chat);
        self.pre.retain(|one| one.chat != chat);
    }
}

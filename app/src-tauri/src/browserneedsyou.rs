// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The browser needs you** (#2015, spec #2008): a page in a chat's browser that needs a person
//! (a CAPTCHA, a second factor, a consent page) becomes an Ask under Needs you, whose source is
//! that browser.
//!
//! - **The agent asks through the one ask tool**, `ask_operator`, pointed at its browser by the
//!   name the browser tools gave it. The call reaches the app as an `Ask::Browse` of
//!   [`Browsing::NeedsYou`] on the chat's hook socket and is answered at once ([`answered_first`]):
//!   the Ask is raised, and the chat is told to leave that browser alone until it is told the
//!   person finished. While the Ask waits, every act of the chat's on that browser is refused in
//!   words, so the agent never moves the page the person was asked about.
//! - **Its one answer is Take over**: the window takes the browser over (`take_over_browser`,
//!   the session host's lease) and opens its view, so the person's first click or key reaches
//!   the page. Nothing else answers it.
//! - **Done ends it** ([`NeedsYou::handed_back`]): the person's hand-back, from the view's bar
//!   or its chord, ends the Ask, and the chat is woken with one line of purlis's own that says
//!   the person finished and nothing of what they did ([`finished`], [`tell`]). That line is a
//!   gated send held to the board's answer, as a vault's Allow is (ADR 0064, amended
//!   2026-10-16).
//! - **The Ask ends with its browser**: closed by the chat, ended by a narrowing, by its chat
//!   closing, or by Stop all. Each leaves an Update in the Inbox ([`Ended`]),
//!   and the chat is told nothing more than its next browser call says.
//!
//! **In memory only**, like every Ask a chat raises: a relaunch starts with none. Nothing the
//! chat sends decides who answers: the Ask is the chat whose token the line carried, its
//! browser is looked up among that chat's own, and only the person's hand-back in the window
//! ends it (`ui::WINDOW_ONLY`).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::askoutcome::{self, About, Outcome};
use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_core::state::State;
use purlis_session_protocol::browsing::Browsed;

use crate::chatbrowsers::ChatBrowsers;
use crate::vaultroute::{Refusals, Told};

/// One Ask that waits: the chat, its browser by the name the chat knows it by and by its
/// program in the session host, what the agent said the page needs, and when it was asked, in
/// seconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    pub session: u32,
    pub browser: String,
    pub program: String,
    pub question: String,
    pub since: u64,
}

/// **An Ask that ended with its browser**, as the Inbox keeps it as an Update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    pub session: u32,
    pub browser: String,
    /// When it ended, in seconds since 1970.
    pub at: u64,
    /// What the Update says: purlis's words around the browser's name.
    pub says: String,
}

/// **An Ask about a chat's browser that ended with the browser**, as the window is told it: the
/// Inbox keeps it as an Update.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AskEnded {
    pub plane: crate::planes::PlaneId,
    pub session: u32,
    /// The browser, by the name its chat knows it by.
    pub browser: String,
    /// When it ended, in seconds since 1970.
    pub at: u32,
    /// What the Update says, in purlis's words.
    pub says: String,
}

impl AskEnded {
    /// `ended`, in project `plane`.
    pub fn of(plane: &crate::planes::PlaneId, ended: Ended) -> Self {
        Self {
            plane: plane.clone(),
            session: ended.session,
            browser: ended.browser,
            at: u32::try_from(ended.at).unwrap_or(u32::MAX),
            says: ended.says,
        }
    }
}

/// The event the window hears each [`AskEnded`] on.
pub const ENDED: &str = "browser-ask-ended";

/// Told each [`AskEnded`].
pub type Teller = Arc<dyn Fn(AskEnded) + Send + Sync + 'static>;

/// Told each Ask that ended with its browser.
pub type EndedTeller = Arc<dyn Fn(Ended) + Send + Sync + 'static>;

/// The Asks of one project's chats that wait on the person to take a browser over.
pub struct NeedsYou {
    waiting: Mutex<Vec<Waiting>>,
    ended: EndedTeller,
}

impl Default for NeedsYou {
    fn default() -> Self {
        Self::new(Arc::new(|_| {}))
    }
}

impl std::fmt::Debug for NeedsYou {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeedsYou")
            .field("waiting", &*self.lock())
            .finish_non_exhaustive()
    }
}

impl NeedsYou {
    /// None waiting; `ended` hears each that ends with its browser.
    pub fn new(ended: EndedTeller) -> Self {
        Self {
            waiting: Mutex::new(Vec::new()),
            ended,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Waiting>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every Ask that waits, oldest first.
    pub fn waiting(&self) -> Vec<Waiting> {
        let mut waiting = self.lock().clone();
        waiting.sort_by_key(|one| (one.since, one.session));
        waiting
    }

    /// The Ask that waits on browser `program`, if one does.
    fn on(&self, program: &str) -> Option<Waiting> {
        self.lock()
            .iter()
            .find(|one| one.program == program)
            .cloned()
    }

    /// Raises `one`, unless an Ask waits on its browser already. Whether it was raised.
    fn raise(&self, one: Waiting) -> bool {
        let mut waiting = self.lock();
        if waiting.iter().any(|was| was.program == one.program) {
            return false;
        }
        waiting.push(one);
        true
    }

    /// **The person handed browser `program` back** (Done): the Ask that waited on it ends, and
    /// is answered, so it leaves no Update. None where none waited.
    pub fn handed_back(&self, program: &str) -> Option<Waiting> {
        let mut waiting = self.lock();
        let at = waiting.iter().position(|one| one.program == program)?;
        Some(waiting.remove(at))
    }

    /// Ends every Ask `gone` says is gone, each with an Update.
    fn end_where(&self, gone: impl Fn(&Waiting) -> bool, now: u64) -> usize {
        let ended: Vec<Waiting> = {
            let mut waiting = self.lock();
            let (ended, kept) = waiting.drain(..).partition(&gone);
            *waiting = kept;
            ended
        };
        for one in &ended {
            (self.ended)(Ended {
                session: one.session,
                says: ended_said(&one.browser),
                browser: one.browser.clone(),
                at: now,
            });
        }
        ended.len()
    }

    /// **The Asks whose browser its chat no longer has** end, each with an Update: the chat
    /// closed it, a narrowing ended it, its engine ended under it. How many.
    pub fn settle(&self, browsers: &ChatBrowsers, now: u64) -> usize {
        self.end_where(|one| !browsers.holds(one.session, &one.program), now)
    }

    /// Chat `session` closed: its Asks end with its browsers.
    pub fn chat_closed(&self, session: u32, now: u64) -> usize {
        self.end_where(|one| one.session == session, now)
    }

    /// Stop all ended every browser: every Ask ends.
    pub fn all_ended(&self, now: u64) -> usize {
        self.end_where(|_| true, now)
    }
}

/// What an Update says of an Ask that ended with its browser.
fn ended_said(browser: &str) -> String {
    format!("Its browser {browser} ended, so what it asked you to do there ended too")
}

/// What the chat is told once the person was asked to take its browser over.
fn asked_said(browser: &str) -> String {
    format!(
        "purlis asked the person to take browser {browser} over: it shows under Needs you. Don't \
         use browser {browser} until you are told; you will be told when they hand it back, and \
         never what they did there. Go on with what needs no browser, or end your turn."
    )
}

/// What a chat whose browser's Ask waits already is told.
fn already_said(browser: &str) -> String {
    format!(
        "purlis already asked the person to take browser {browser} over, and asked nothing more: \
         you will be told when they hand it back."
    )
}

/// What an act on a browser whose Ask waits is refused with.
fn waits_said(browser: &str) -> String {
    format!(
        "the person was asked to take browser {browser} over and has not handed it back: don't \
         use browser {browser} until you are told they finished. Go on with what needs no \
         browser."
    )
}

/// An answer in words, for the chat's ask tool, about browser `browser`.
fn told(browser: &str, note: String) -> Answer {
    Answer::Browsed {
        browser: browser.to_owned(),
        browsed: Box::new(Browsed {
            settled: true,
            note: Some(note),
            ..Browsed::default()
        }),
    }
}

/// **What the app answers a browser call with before its browsers do** (#2015): a question
/// about a browser, raised as the person's Ask and answered at once, and an act on a browser
/// whose Ask waits, refused. None for every other call, which [`ChatBrowsers::answer`]
/// answers. `now` is in seconds since 1970.
pub fn answered_first(
    browsers: &ChatBrowsers,
    needs: &NeedsYou,
    browse: &BrowseAsk,
    now: u64,
) -> Option<Answer> {
    match &browse.asked {
        Browsing::NeedsYou { question } => {
            let Some((name, program)) = browsers.which(browse.chat, browse.browser.as_deref())
            else {
                let name = browse.browser.as_deref().unwrap_or_default();
                return Some(Answer::No {
                    why: format!(
                        "this chat has no browser {name}, so the person was asked nothing: name \
                         one the browser tools opened for this chat"
                    ),
                });
            };
            let raised = needs.raise(Waiting {
                session: browse.chat,
                browser: name.clone(),
                program,
                question: question.clone(),
                since: now,
            });
            Some(if raised {
                told(&name, asked_said(&name))
            } else {
                told(&name, already_said(&name))
            })
        }
        Browsing::Act(_) => {
            let (name, program) = browsers.which(browse.chat, browse.browser.as_deref())?;
            needs.on(&program).map(|_| Answer::No {
                why: waits_said(&name),
            })
        }
        // A new browser is another; a close ends the browser, and its Ask with it.
        Browsing::New | Browsing::Close => None,
    }
}

/// **What the chat is told when the person hands its browser back**: that they finished, and
/// nothing of what they did ([`purlis_core::askoutcome`]'s one line).
pub fn finished(back: &Waiting) -> Outcome {
    Outcome {
        about: About::TakeOver {
            browser: back.browser.clone(),
        },
        answer: askoutcome::Answer::Finished,
    }
}

/// **Wakes chat `back.session` with [`finished`]'s line**, by the board's answer (`glance`, as
/// [`crate::vaultroute::tell`] reads it): sent now to a chat that waits and asks nothing, queued
/// for the end of a turn under way, and otherwise handed to its next turn (`next_turn`), so an
/// Enter never lands on a harness's prompt.
pub fn tell(
    refusals: &Refusals,
    back: &Waiting,
    glance: impl FnOnce() -> (State, bool),
    send: impl FnOnce(u32, &str) -> Result<(), String>,
    next_turn: impl FnOnce(Outcome),
) -> Told {
    let outcome = finished(back);
    let told = crate::vaultroute::tell(refusals, back.session, glance, outcome.line(), send);
    if told == Told::NotSent {
        next_turn(outcome);
    }
    told
}

/// **The Asks of project `held` whose browser is gone end**, each with an Update, and the
/// window reads its asks again where any did.
pub fn settled(held: &crate::planes::Held) {
    if held
        .needs_you()
        .settle(held.browsers(), crate::sandboxing::now_secs())
        > 0
    {
        held.chats().asks_may_have_moved();
    }
}

/// **The person handed browser `program` back** (#2015), in whichever project its chat is: the
/// Ask that waited on it ends, and its chat is woken with the line that says they finished
/// ([`tell`]). Nothing where no Ask waited: a take-over of the person's own tells the chat
/// nothing (D-2014-10).
pub fn handed_back(projects: &[Arc<crate::planes::Held>], program: &str) {
    for held in projects {
        let Some(back) = held.needs_you().handed_back(program) else {
            continue;
        };
        let told = tell(
            held.vault_refusals(),
            &back,
            || {
                let glance = held.board().glance(back.session);
                (glance.state, glance.asking)
            },
            |chat, text| {
                held.chats()
                    .sessions()
                    .input(chat, crate::vaultroute::sent_as(text).as_bytes())
            },
            |outcome| held.chats().tell_outcome(back.session, outcome),
        );
        tracing::info!(
            "purlis: the person handed chat {}'s browser {} back; the chat is told ({told:?})",
            back.session,
            back.browser
        );
        held.chats().asks_may_have_moved();
    }
}

/// **Stop all ended every browser** (#2015): every Ask about one ends, with an Update.
pub fn all_ended(projects: &[Arc<crate::planes::Held>]) {
    for held in projects {
        if held.needs_you().all_ended(crate::sandboxing::now_secs()) > 0 {
            held.chats().asks_may_have_moved();
        }
    }
}

#[cfg(test)]
mod tests;

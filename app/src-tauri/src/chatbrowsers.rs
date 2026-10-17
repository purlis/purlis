// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's browsers** (#2013, spec #2008): the window's side of the chat's browser tools.
//!
//! A chat's agent drives a real browser through purlis's `browser_` tools, on every harness
//! (`purlis_core::browsertools`). Each call reaches the app as one `Ask::Browse` on the chat's
//! hook socket, and is answered here:
//!
//! - **The first call asks the person, once** ([`Posture`]): what this chat's browser may reach.
//!   This project's sites and its dev servers (the hosts the chat's sandbox lists, and the
//!   services on this machine the person allowed), also the open web, or Keep off. Nothing
//!   starts until they answer; the call is refused at once, in words that say the person was
//!   asked and the chat will be told, and every later call while they have not answered says
//!   the same and asks nothing more. The Ask is in the asks registry (`crate::asking`), so it
//!   is on the chat's tab and in the Inbox. The answer reaches the chat on its next turn, in
//!   `purlis_core::askoutcome`'s one line.
//! - **Once allowed, the chat's browser is started in the session host** with what it may reach
//!   ([`Driver::start`]), and each act is handed on as it is ([`Driver::act`]); the host's
//!   browser carries it out and enforces the reach. A chat has one browser by default, more on
//!   `browser_new` (each apart, its cookies and storage its own); the project's chats hold at
//!   most [`AT_MOST_PER_PROJECT`] together. Each is named to the chat as `b1`, `b2`, and is
//!   looked up only among the chat's own. A chat's browsers end with it.
//! - **A browser's reach follows its chat's reach, including what the person withdraws**: a
//!   browser is given what its chat reaches as it starts, and when the person narrows the
//!   chat's reach while it runs, its browsers end ([`ChatBrowsers::narrowed`]); the next call
//!   opens one on what the chat reaches then, and says why.
//! - **Keep off** refuses every call of the chat's, in words that say not to retry.
//! - **A site new to the project asks, once allowed** (#2018): each act tells the browser what
//!   its chat reaches now ([`Driver::act`]), so an Allow since it started reaches it at its next
//!   act. Where the browser's judge held something back (an address the agent wrote for a site
//!   new to the project, a form or a body sent there, or typing on a page of it), the act's
//!   answer names the site; the chat is told it was asked about, and the site is asked about
//!   as a host the chat's sandbox refused (`crate::chats::Chats::ask_for_its_browser`): the
//!   one Ask a chat's hosts are asked by, every such host of the chat on one Notice with one
//!   Allow, and the one Granted list.
//!
//! **In memory only**, like the other Asks a chat raises: a relaunch starts with none, and the
//! chat's next call asks again. Nothing a chat sends decides any of it: the chat is the one
//! whose token the line carries, its reach is the app's record of its sandbox, and only the
//! person's press in the window answers the Ask (`ui::WINDOW_ONLY`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};

/// The most browsers the chats of one project hold at once (spec #2008, Limits), until the
/// limits are settings (#2023).
pub const AT_MOST_PER_PROJECT: usize = 3;

/// Why the session host did not carry an act out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The browser is not there any more: stopped, closed, or its engine ended.
    Gone,
    /// The browser's own refusal, in words that say what to do.
    Said(String),
}

/// **Where a chat's browsers run**: the window's session host (`crate::browsers`), behind a
/// seam so what is decided here is tested without one.
pub trait Driver: Send + Sync + 'static {
    /// Starts a browser in the project at `root` reaching `reach`, and answers its program.
    fn start(&self, root: &Path, reach: BrowserReach) -> Result<String, String>;
    /// Carries `act` out on browser `program` under its own id `call_id`, the browser reaching
    /// `reach` from then on (#2018).
    fn act(
        &self,
        program: &str,
        call_id: &str,
        act: Act,
        reach: BrowserReach,
    ) -> Result<Browsed, Refused>;
    /// Ends browser `program`, and forgets it.
    fn close(&self, program: &str) -> Result<(), String>;
    /// Ends browser `program` by its id, with its reach proxy: what is tried when a close did
    /// not end it.
    fn stop(&self, program: &str) -> Result<(), String>;
}

/// How many of a chat's browsers were ended, and how many could not be yet: those are kept, and
/// tried again at every browser call of the project's and every narrowing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ended {
    pub ended: usize,
    pub unended: usize,
}

/// What the person said a chat's browser may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    /// The chat has called no browser tool: nothing asked.
    Unasked,
    /// The person is asked, and has not answered.
    Asked,
    /// Allowed: this project's sites and its dev servers, and with `open_web` the open web too.
    Allowed { open_web: bool },
    /// Kept off.
    Off,
}

/// One Ask that waits: the chat, when it began to wait, in seconds since 1970, and its persona
/// as it was when it asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waits {
    pub session: u32,
    pub since: u64,
    pub as_persona: AsPersona,
}

/// **A chat's persona, as its browser is decided** (#2241): read from the app's own record of
/// the chat and from where no chat writes, at each browser call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AsPersona {
    /// The persona whose grants the chat runs with (`purlis_core::start::runs_with`); none for
    /// a chat on no persona, which is never offered Always.
    pub persona: Option<String>,
    /// What that persona declares its browser reaches, in the project's root `charter.toml`,
    /// which no sandboxed chat writes: only marks an answer of the Ask (ADR 0073), and is never
    /// an answer.
    pub declared: Option<purlis_core::sandbox::persona::Browser>,
    /// **Always for this persona**, as this machine keeps it and policy leaves it in force:
    /// whether it reaches the open web too. None where none is.
    pub always: Option<bool>,
}

/// One chat's browsers, as the app knows them.
#[derive(Debug)]
struct Chat {
    posture: Posture,
    /// When the Ask began, while it waits.
    since: u64,
    /// Each browser by the name the chat knows it by, and its program in the session host,
    /// oldest first.
    browsers: Vec<(String, String)>,
    /// The one a call that names none uses.
    current: Option<String>,
    /// The number the next browser is named by.
    next: u32,
    /// Its browsers were ended because the person narrowed its reach, and it is not told yet.
    narrowed: bool,
    /// Its persona as it was when its Ask was raised.
    as_persona: AsPersona,
    /// The persona whose Always it was allowed on (#2241), where it was: what removing or
    /// changing that Always follows.
    rests_on: Option<String>,
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            posture: Posture::Unasked,
            since: 0,
            browsers: Vec::new(),
            current: None,
            next: 1,
            narrowed: false,
            as_persona: AsPersona::default(),
            rests_on: None,
        }
    }
}

impl Chat {
    fn program(&self, name: &str) -> Option<String> {
        self.browsers
            .iter()
            .find(|(one, _)| one == name)
            .map(|(_, program)| program.clone())
    }

    fn drop_browser(&mut self, name: &str) {
        self.browsers.retain(|(one, _)| one != name);
        if self.current.as_deref() == Some(name) {
            self.current = self.browsers.last().map(|(one, _)| one.clone());
        }
    }
}

/// The browsers of one project's chats, and what the person said each may reach.
#[derive(Debug, Default)]
pub struct ChatBrowsers {
    chats: Mutex<HashMap<u32, Chat>>,
    /// Browsers that were to end and could not be ended yet: no chat uses them, and each is
    /// tried again until it ends. They count toward the project's browsers, and are listed
    /// under the chat they were of, as not ended yet.
    unended: Mutex<Vec<ChatBrowser>>,
    /// Moves each time a browser is added to the chats' or taken from them ([`Self::listing`]).
    listing: AtomicU64,
}

/// **One browser of a chat's, as the window lists it** (#2024): under its chat in the Chats
/// list, live on its chat's tab, and opened in a view tab by its program.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatBrowser {
    /// The chat it is of.
    pub session: u32,
    /// What the chat calls it: `b1`, `b2`.
    pub name: String,
    /// Its program in the session host: what its view is opened by.
    pub program: String,
    /// It was to end and could not be ended yet (D-2013-16): no call of the chat's reaches it,
    /// purlis keeps trying, and Stop all ends it now.
    pub unended: bool,
}

/// Ends `program`: a close, once more, then a stop by its id. Whether it ended.
fn end(driver: &dyn Driver, program: &str) -> bool {
    let closed = driver.close(program).or_else(|_| driver.close(program));
    match closed.or_else(|_| driver.stop(program)) {
        Ok(()) => true,
        Err(why) => {
            tracing::warn!("purlis: a chat's browser was not ended yet, and is tried again: {why}");
            false
        }
    }
}

/// What a chat is told while the person has not answered its Ask.
pub const ASKED: &str = "purlis asked the person what this chat's browser may reach, and opened \
                         nothing. You will be told their answer on your next turn: don't retry \
                         a browser tool until then, and go on with what needs no browser.";

/// What a chat is told of the sites its browser asked the person about (#2018).
fn asked_about(sites: &[String]) -> String {
    format!(
        "{} new to this project, so purlis held back what this browser would have opened, sent \
         or typed there, and asked the person whether it may: you will be told their answer on \
         your next turn. Don't retry it until then; go on with what needs no new site",
        match sites {
            [one] => format!("{one} is a site"),
            many => format!("{} are sites", many.join(", ")),
        }
    )
}

/// What a chat whose browser the person kept off is told.
pub const KEPT_OFF: &str = "The person kept this chat's browser off: don't retry or work around \
                            it.";

const NO_HOST: &str = "This window has no session host to run a browser in, so this chat has \
                       no browser: go on with what needs no browser, and tell the person if the \
                       task needs one.";

/// What a chat whose browsers were ended because the person narrowed its reach is told, with
/// the first answer after.
const NARROWED: &str = "the person withdrew part of what this chat may reach, so purlis ended \
                        its browsers and what they held: this is a new one, reaching what the \
                        chat reaches now";

impl ChatBrowsers {
    fn lock(&self) -> MutexGuard<'_, HashMap<u32, Chat>> {
        self.chats.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Every browser of the project's chats**, by chat, each chat's oldest first, then any of
    /// its that is not ended yet.
    pub fn listed(&self) -> Vec<ChatBrowser> {
        let mut listed: Vec<ChatBrowser> = self
            .lock()
            .iter()
            .flat_map(|(session, chat)| {
                chat.browsers.iter().map(|(name, program)| ChatBrowser {
                    session: *session,
                    name: name.clone(),
                    program: program.clone(),
                    unended: false,
                })
            })
            .collect();
        // And those not ended yet, after the ones its chat uses (D-2013-16).
        listed.extend(
            self.unended
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .cloned(),
        );
        // Stable within a chat: its browsers are kept oldest first.
        listed.sort_by_key(|one| (one.session, one.unended));
        listed
    }

    /// **Where the listing stands**: a number that moves each time a browser is added to the
    /// chats' or taken from them, and on nothing else, so a caller tells the window only when
    /// [`Self::listed`] changed.
    pub fn listing(&self) -> u64 {
        self.listing.load(Ordering::Relaxed)
    }

    fn relisted(&self) {
        self.listing.fetch_add(1, Ordering::Relaxed);
    }

    /// What the person said chat `session`'s browser may reach.
    #[cfg(test)]
    pub fn posture(&self, session: u32) -> Posture {
        self.lock()
            .get(&session)
            .map_or(Posture::Unasked, |chat| chat.posture)
    }

    /// Every Ask that waits, oldest first.
    pub fn waiting(&self) -> Vec<Waits> {
        let mut waits: Vec<Waits> = self
            .lock()
            .iter()
            .filter(|(_, chat)| chat.posture == Posture::Asked)
            .map(|(session, chat)| Waits {
                session: *session,
                since: chat.since,
                as_persona: chat.as_persona.clone(),
            })
            .collect();
        waits.sort_by_key(|one| (one.since, one.session));
        waits
    }

    /// **Chat `session`'s browser `name`, or the one it used last**, by its name and its
    /// program in the session host (#2015). None where the chat has no such browser: another
    /// chat's is never found by its name.
    pub fn which(&self, session: u32, name: Option<&str>) -> Option<(String, String)> {
        let chats = self.lock();
        let chat = chats.get(&session)?;
        let name = name.map(str::to_owned).or_else(|| chat.current.clone())?;
        let program = chat.program(&name)?;
        Some((name, program))
    }

    /// Whether chat `session` still has browser `program` (#2015).
    pub fn holds(&self, session: u32, program: &str) -> bool {
        self.lock()
            .get(&session)
            .is_some_and(|chat| chat.browsers.iter().any(|(_, one)| one == program))
    }

    /// **Allow** on chat `session`'s Ask, with the open web or not. Answers whether it waited.
    pub fn allow(&self, session: u32, open_web: bool) -> bool {
        self.answered(session, Posture::Allowed { open_web })
    }

    /// **Keep off** on chat `session`'s Ask. Answers whether it waited.
    pub fn keep_off(&self, session: u32) -> bool {
        self.answered(session, Posture::Off)
    }

    /// **Always for `persona`**, reaching the open web too with `open_web` (#2241): every chat as
    /// that persona whose Ask waits is answered by it. Answers those chats.
    pub fn allow_always(&self, persona: &str, open_web: bool) -> Vec<u32> {
        let mut answered = Vec::new();
        for (session, chat) in self.lock().iter_mut() {
            if chat.posture == Posture::Asked && chat.as_persona.persona.as_deref() == Some(persona)
            {
                chat.posture = Posture::Allowed { open_web };
                chat.rests_on = Some(persona.to_owned());
                answered.push(*session);
            }
        }
        answered
    }

    /// **The person removed Always for `persona`** (#2241): the browsers of every chat it
    /// allowed end ([`Self::narrowed`]'s way, D-2013-11/16), and each chat's next call asks
    /// again. Answers each such chat with what was ended of its browsers.
    pub fn always_removed(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        persona: &str,
    ) -> Vec<(u32, Ended)> {
        let resting: Vec<u32> = self
            .lock()
            .iter()
            .filter(|(_, chat)| chat.rests_on.as_deref() == Some(persona))
            .map(|(session, _)| *session)
            .collect();
        resting
            .into_iter()
            .map(|session| (session, self.withdraw(driver, session, Posture::Unasked)))
            .collect()
    }

    /// Ends chat `session`'s browsers, as a narrowing does, and sets its answer to `posture`:
    /// unasked again, or a narrower Allow.
    fn withdraw(&self, driver: Option<&Arc<dyn Driver>>, session: u32, posture: Posture) -> Ended {
        if let Some(chat) = self.lock().get_mut(&session) {
            chat.posture = posture;
            if posture == Posture::Unasked {
                chat.rests_on = None;
            }
        }
        self.narrowed(driver, session)
    }

    /// **Follows the Always chat `session` was allowed on, as it is now** (#2241): unchanged,
    /// nothing; widened, the next browser reaches more and what runs runs on; narrowed, its
    /// browsers end and the next reaches less; gone (removed, or held by an administrator's
    /// policy), its browsers end and its next call asks again.
    fn follow_always(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        session: u32,
        now: &AsPersona,
    ) -> Option<Ended> {
        let follows = {
            let mut chats = self.lock();
            let chat = chats.get_mut(&session)?;
            let on = chat.rests_on.as_deref()?;
            let kept = now.always.filter(|_| now.persona.as_deref() == Some(on));
            match (chat.posture, kept) {
                (Posture::Allowed { open_web }, Some(kept)) if kept == open_web => None,
                (Posture::Allowed { open_web: false }, Some(true)) => {
                    chat.posture = Posture::Allowed { open_web: true };
                    None
                }
                (Posture::Allowed { .. }, Some(false)) => {
                    Some(Posture::Allowed { open_web: false })
                }
                _ => Some(Posture::Unasked),
            }
        };
        follows.map(|posture| self.withdraw(driver, session, posture))
    }

    /// **Always for `persona` was answered again, reaching the open web too with `open_web`**
    /// (#2241): each chat it allowed follows it now, as [`Self::follow_always`] says, so a
    /// narrower answer ends their browsers at once (D-2013-11). Answers each chat whose browsers
    /// that ended.
    pub fn always_changed(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        persona: &str,
        open_web: bool,
    ) -> Vec<(u32, Ended)> {
        let resting: Vec<u32> = self
            .lock()
            .iter()
            .filter(|(_, chat)| chat.rests_on.as_deref() == Some(persona))
            .map(|(session, _)| *session)
            .collect();
        let now = AsPersona {
            persona: Some(persona.to_owned()),
            declared: None,
            always: Some(open_web),
        };
        resting
            .into_iter()
            .filter_map(|session| {
                self.follow_always(driver, session, &now)
                    .map(|ended| (session, ended))
            })
            .collect()
    }

    fn answered(&self, session: u32, posture: Posture) -> bool {
        match self.lock().get_mut(&session) {
            Some(chat) if chat.posture == Posture::Asked => {
                chat.posture = posture;
                true
            }
            _ => false,
        }
    }

    /// The browsers that were to end and are not ended yet.
    #[cfg(test)]
    pub fn unended(&self) -> Vec<String> {
        self.unended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|one| one.program.clone())
            .collect()
    }

    /// Ends each of `browsers` ([`end`]), and keeps those that did not end, with those kept
    /// before, which are tried again first. Answers how many of `browsers` ended and did not.
    fn end_all(&self, driver: Option<&Arc<dyn Driver>>, browsers: Vec<ChatBrowser>) -> Ended {
        let before =
            std::mem::take(&mut *self.unended.lock().unwrap_or_else(PoisonError::into_inner));
        let left = |browsers: Vec<ChatBrowser>| -> Vec<ChatBrowser> {
            match driver {
                Some(driver) => browsers
                    .into_iter()
                    .filter(|one| !end(driver.as_ref(), &one.program))
                    .collect(),
                None => browsers,
            }
        };
        let count = browsers.len();
        let had = before.len();
        let mut kept = left(before);
        let still = kept.len();
        let now: Vec<ChatBrowser> = left(browsers)
            .into_iter()
            .map(|one| ChatBrowser {
                unended: true,
                ..one
            })
            .collect();
        let unended = now.len();
        kept.extend(now);
        // One kept before that has ended now, or one that could not be ended: its row moves.
        if still != had || unended > 0 {
            self.relisted();
        }
        self.unended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(kept);
        Ended {
            ended: count - unended,
            unended,
        }
    }

    /// **Stop all stopped every browser** (#2024): the chats' browsers leave them, and their
    /// rows and tabs, as each is ended (a browser Stop all ended answers as gone); one that
    /// could not be is kept and tried again, as any is. Each chat's answer to its Ask stands.
    pub fn all_stopped(&self, driver: Option<&Arc<dyn Driver>>) {
        let browsers: Vec<ChatBrowser> = {
            let mut chats = self.lock();
            chats
                .iter_mut()
                .flat_map(|(session, chat)| {
                    chat.current = None;
                    chat.browsers
                        .drain(..)
                        .map(|(name, program)| ChatBrowser {
                            session: *session,
                            name,
                            program,
                            unended: false,
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        if !browsers.is_empty() {
            self.relisted();
        }
        self.end_all(driver, browsers);
    }

    /// Whether `program` is a browser a chat uses now: one that ends with its chat, and that
    /// nothing else of the window's ends (#2024).
    pub fn is_in_use(&self, program: &str) -> bool {
        self.lock()
            .values()
            .any(|chat| chat.browsers.iter().any(|(_, one)| one == program))
    }

    /// **The person narrowed chat `session`'s reach while it runs**: a port withdrawn, an Allow
    /// removed. Its browsers end, since each was given the chat's reach as it started; one that
    /// cannot be ended yet is kept and tried again until it is, and no call of the chat's
    /// reaches it. The person's answer to its Ask stands, and its next call opens a browser on
    /// what the chat reaches then, and says why.
    pub fn narrowed(&self, driver: Option<&Arc<dyn Driver>>, session: u32) -> Ended {
        let programs: Vec<ChatBrowser> = {
            let mut chats = self.lock();
            let Some(chat) = chats.get_mut(&session) else {
                drop(chats);
                self.end_all(driver, Vec::new());
                return Ended::default();
            };
            let programs: Vec<ChatBrowser> = chat
                .browsers
                .drain(..)
                .map(|(name, program)| ChatBrowser {
                    session,
                    name,
                    program,
                    unended: false,
                })
                .collect();
            chat.current = None;
            chat.narrowed |= !programs.is_empty();
            programs
        };
        if !programs.is_empty() {
            self.relisted();
        }
        self.end_all(driver, programs)
    }

    /// Chat `session` has closed: its browsers end, and nothing of it is kept but a browser that
    /// could not be ended yet, which is tried again until it is.
    pub fn forget(&self, driver: Option<&Arc<dyn Driver>>, session: u32) {
        let gone = self.lock().remove(&session);
        let programs: Vec<ChatBrowser> = gone
            .map(|chat| {
                chat.browsers
                    .into_iter()
                    .map(|(name, program)| ChatBrowser {
                        session,
                        name,
                        program,
                        unended: false,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !programs.is_empty() {
            self.relisted();
        }
        self.end_all(driver, programs);
    }

    /// **Answers one browser call of a chat's**, and whether it raised the chat's Ask.
    ///
    /// `reach` is what the chat's browser may reach, read only as one starts; `now` is in
    /// seconds since 1970. The chat is the one whose token the line carried. With no `driver`
    /// there is no session host, and so no browser.
    pub fn answer(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        root: &Path,
        browse: &BrowseAsk,
        reach: &dyn Fn() -> BrowserReach,
        as_persona: &dyn Fn() -> AsPersona,
        now: u64,
    ) -> (Answer, bool) {
        // A browser that was to end and did not is tried again first.
        self.end_all(driver, Vec::new());
        let persona = as_persona();
        self.follow_always(driver, browse.chat, &persona);
        let (mut answer, raised) = self.answer_now(driver, root, browse, reach, persona, now);
        // What the browser held back for the person says so first (#2018).
        if let Answer::Browsed { browsed, .. } = &mut answer
            && !browsed.asked.is_empty()
        {
            let asked = asked_about(&browsed.asked);
            browsed.note = Some(match browsed.note.take() {
                Some(note) => format!("{asked}. {note}"),
                None => asked,
            });
        }
        // The first answer after its browsers were ended for a narrowed reach says why.
        if let Answer::Browsed { browsed, .. } = &mut answer
            && let Some(chat) = self.lock().get_mut(&browse.chat)
            && std::mem::take(&mut chat.narrowed)
        {
            browsed.note = Some(match browsed.note.take() {
                Some(note) => format!("{NARROWED}. {note}"),
                None => NARROWED.to_owned(),
            });
        }
        (answer, raised)
    }

    fn answer_now(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        root: &Path,
        browse: &BrowseAsk,
        reach: &dyn Fn() -> BrowserReach,
        as_persona: AsPersona,
        now: u64,
    ) -> (Answer, bool) {
        let no = |why: &str| Answer::No {
            why: why.to_owned(),
        };
        let unended = self
            .unended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        let mut chats = self.lock();
        let elsewhere: usize = unended
            + chats
                .iter()
                .filter(|(session, _)| **session != browse.chat)
                .map(|(_, chat)| chat.browsers.len())
                .sum::<usize>();
        let chat = chats.entry(browse.chat).or_default();
        // Always for this persona answers the Ask, raised or not (#2241).
        if matches!(chat.posture, Posture::Unasked | Posture::Asked)
            && let (Some(persona), Some(open_web)) = (&as_persona.persona, as_persona.always)
        {
            chat.posture = Posture::Allowed { open_web };
            chat.rests_on = Some(persona.clone());
        }
        let open_web = match chat.posture {
            Posture::Unasked => {
                chat.posture = Posture::Asked;
                chat.since = now;
                chat.as_persona = as_persona;
                return (no(ASKED), true);
            }
            Posture::Asked => return (no(ASKED), false),
            Posture::Off => return (no(KEPT_OFF), false),
            Posture::Allowed { open_web } => open_web,
        };
        let Some(driver) = driver else {
            return (no(NO_HOST), false);
        };
        let named = browse.browser.clone().or_else(|| chat.current.clone());
        let start = |chat: &mut Chat| -> Result<String, String> {
            if elsewhere + chat.browsers.len() >= AT_MOST_PER_PROJECT {
                return Err(format!(
                    "this project's chats have {AT_MOST_PER_PROJECT} browsers open, the most \
                     it may: close one of this chat's with browser_close first, or go on \
                     without one"
                ));
            }
            let program = driver
                .start(
                    root,
                    BrowserReach {
                        open_web,
                        ..reach()
                    },
                )
                .map_err(|why| {
                    format!(
                        "the browser did not start: {why}. Try once more; if it fails again, \
                         go on with what needs no browser and tell the person what it said"
                    )
                })?;
            let name = format!("b{}", chat.next);
            chat.next += 1;
            chat.browsers.push((name.clone(), program));
            self.relisted();
            chat.current = Some(name.clone());
            Ok(name)
        };
        let act = match &browse.asked {
            Browsing::New => {
                return match start(chat) {
                    Ok(name) => (
                        Answer::Browsed {
                            browser: name,
                            browsed: Box::new(Browsed {
                                settled: true,
                                note: Some(
                                    "a new browser, apart from this chat's others, with no page \
                                     open: the browser tools use it from now"
                                        .to_owned(),
                                ),
                                ..Browsed::default()
                            }),
                        },
                        false,
                    ),
                    Err(why) => (no(&why), false),
                };
            }
            Browsing::Close => {
                let Some(name) = named else {
                    return (no("this chat has no browser open"), false);
                };
                let Some(program) = chat.program(&name) else {
                    return (no(&no_such(&name)), false);
                };
                chat.drop_browser(&name);
                self.relisted();
                drop(chats);
                let closing = ChatBrowser {
                    session: browse.chat,
                    name: name.clone(),
                    program,
                    unended: false,
                };
                if self.end_all(Some(driver), vec![closing]).unended > 0 {
                    return (
                        no(&format!(
                            "purlis could not end browser {name} yet, and keeps trying: this chat \
                             no longer uses it. Go on; the next browser call opens a new one"
                        )),
                        false,
                    );
                }
                return (
                    Answer::Browsed {
                        browser: name,
                        browsed: Box::new(Browsed {
                            settled: true,
                            note: Some("closed, and everything it held is gone".to_owned()),
                            ..Browsed::default()
                        }),
                    },
                    false,
                );
            }
            Browsing::Act(act) => act.clone(),
            // Answered before the browsers are (`crate::browserneedsyou::answered_first`).
            Browsing::NeedsYou { .. } => {
                return (
                    no("purlis could not ask the person about this browser"),
                    false,
                );
            }
        };
        let (name, program) = match named {
            Some(name) => match chat.program(&name) {
                Some(program) => (name, program),
                None => return (no(&no_such(&name)), false),
            },
            None => match start(chat) {
                Ok(name) => {
                    let program = chat.program(&name).unwrap_or_default();
                    (name, program)
                }
                Err(why) => return (no(&why), false),
            },
        };
        // The act may wait on its page a while: never under the lock.
        drop(chats);
        let reach_now = BrowserReach {
            open_web,
            ..reach()
        };
        match driver.act(&program, &browse.call_id, act, reach_now) {
            Ok(browsed) => (
                Answer::Browsed {
                    browser: name,
                    browsed: Box::new(browsed),
                },
                false,
            ),
            Err(Refused::Said(why)) => (no(&why), false),
            Err(Refused::Gone) => {
                if let Some(chat) = self.lock().get_mut(&browse.chat) {
                    chat.drop_browser(&name);
                    self.relisted();
                }
                (
                    no(&format!(
                        "browser {name} has ended (stopped, or its engine closed), and what it \
                         held is gone: the next browser call opens a new one"
                    )),
                    false,
                )
            }
        }
    }
}

/// The event the window hears a chat's browsers ended on, because the person narrowed its
/// reach.
pub const ENDED: &str = "chat-browsers-ended";

/// What a chat's tab says once its browsers ended because the person narrowed its reach.
pub const ENDED_ON_ITS_TAB: &str = "purlis ended this chat's browsers, since you withdrew part \
                                    of what it may reach. Its next browser call opens one on \
                                    what it reaches now.";

/// What a chat's tab says when one of its browsers could not be ended yet as its reach narrowed.
pub const NOT_ENDED_ON_ITS_TAB: &str = "purlis could not end one of this chat's browsers after \
                                        you withdrew part of what it may reach. The chat no \
                                        longer uses it, and purlis keeps trying to end it; Stop \
                                        all in the title bar ends it now.";

/// **A chat whose browsers ended because the person narrowed its reach**, as its tab says so.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowsersEnded {
    pub plane: crate::planes::PlaneId,
    pub session: u32,
    /// What its tab says, in purlis's words.
    pub note: String,
}

/// Told each [`BrowsersEnded`], as the window shows it.
pub type Teller = Arc<dyn Fn(BrowsersEnded) + Send + Sync + 'static>;

/// The event the window hears on when a project's chats' browsers were added to or taken from
/// (#2024): it reads them again ([`chat_browsers`]).
pub const LISTED: &str = "chat-browsers-listed";

/// **A project whose chats' browsers were added to or taken from** (#2024).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowsersListed {
    pub plane: crate::planes::PlaneId,
}

/// Told each project whose chats' browsers were added to or taken from.
pub type ListedTeller = Arc<dyn Fn(crate::planes::PlaneId) + Send + Sync + 'static>;

/// **The person narrowed chat `session`'s reach while it runs, in project `held`**: its
/// browsers end ([`ChatBrowsers::narrowed`]), and where any did, `told` hears it for its tab.
pub fn narrowed(
    held: &crate::planes::Held,
    plane: &crate::planes::PlaneId,
    session: u32,
    told: &dyn Fn(BrowsersEnded),
) {
    let listing = held.browsers().listing();
    let Ended { ended, unended } = held.browsers().narrowed(held.browser_driver(), session);
    held.browsers_listed_since(listing);
    crate::browserneedsyou::settled(held);
    if unended > 0 {
        told(BrowsersEnded {
            plane: plane.clone(),
            session,
            note: NOT_ENDED_ON_ITS_TAB.to_owned(),
        });
    } else if ended > 0 {
        told(BrowsersEnded {
            plane: plane.clone(),
            session,
            note: ENDED_ON_ITS_TAB.to_owned(),
        });
    }
}

/// **Chat `browse.chat`'s browser call, in project `held`** (#2013): answered by
/// [`ChatBrowsers::answer`], with the chat's reach read from the app's record of its sandbox as
/// one of its browsers starts. Raising its Ask tells the registry.
pub fn answered(held: &crate::planes::Held, browse: &BrowseAsk) -> Answer {
    if !held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == browse.chat)
    {
        return Answer::No {
            why: format!(
                "chat {} is not one this purlis window has open, and only such a chat has \
                 browser tools: go on with what needs no browser",
                browse.chat
            ),
        };
    }
    // A browser that needs the person, and the acts on it while it waits (#2015).
    let now = crate::sandboxing::now_secs();
    if let Some(answer) =
        crate::browserneedsyou::answered_first(held.browsers(), held.needs_you(), browse, now)
    {
        if matches!(browse.asked, Browsing::NeedsYou { .. }) {
            held.chats().asks_may_have_moved();
        }
        return answer;
    }
    let reach = || reach_of(held.chats(), browse.chat);
    let as_persona = || as_persona_of(held, browse.chat);
    let listing = held.browsers().listing();
    let (answer, raised) = held.browsers().answer(
        held.browser_driver(),
        held.root(),
        browse,
        &reach,
        &as_persona,
        now,
    );
    held.browsers_listed_since(listing);
    // Each site the browser held back is asked about as the chat's own hosts are (#2018).
    if let Answer::Browsed { browsed, .. } = &answer
        && !browsed.asked.is_empty()
    {
        held.chats()
            .ask_for_its_browser(browse.chat, &browsed.asked);
    }
    if raised {
        tracing::info!(
            "purlis: chat {} asked for a browser; the person is asked what it may reach",
            browse.chat
        );
        held.chats().asks_may_have_moved();
    }
    // A browser that ended takes the person's Ask about it with it (#2015).
    if held.needs_you().settle(held.browsers(), now) > 0 {
        held.chats().asks_may_have_moved();
    }
    answer
}

/// The persona chat `session` runs with, from the app's own record of it, never from what
/// the chat says (`purlis_core::start::runs_with`): none for a chat this window does not have
/// open.
fn persona_of(held: &crate::planes::Held, session: u32) -> Option<String> {
    held.chats()
        .recorded_chat(session)
        .and_then(|chat| purlis_core::start::runs_with(&chat, held.root()))
}

/// **Chat `session`'s persona, as its browser is decided** (#2241): the persona it runs with,
/// what that persona declares in the project's root `charter.toml`, and Always for it as this
/// machine keeps it and policy leaves it in force. Nothing of it is read from a file a chat
/// writes.
fn as_persona_of(held: &crate::planes::Held, session: u32) -> AsPersona {
    use purlis_core::sandbox;
    let Some(persona) = persona_of(held, session) else {
        return AsPersona::default();
    };
    let root = held.root();
    AsPersona {
        declared: sandbox::persona::declared_browser(&sandbox::Plane::read(root), &persona),
        always: sandbox::local::browser_always_here(
            root,
            &persona,
            &sandbox::policy::Locks::of(root),
        ),
        persona: Some(persona),
    }
}

/// **What chat `session`'s browser reaches**: what its sandbox lists, as it was compiled at its
/// start with the hosts allowed it live since, never its floor (a browser takes only what is
/// listed, D-2011-10). A chat started without the sandbox has no such record, so its browser
/// starts with nothing listed, and reaches a public site only where the person let it reach
/// the open web.
fn reach_of(chats: &crate::chats::Chats, session: u32) -> BrowserReach {
    chats
        .confines_now(session)
        .map_or_else(BrowserReach::default, |confines| {
            purlis_core::browsertools::browser_reach(&confines.decides(), false)
        })
}

/// What closing a chat's browser from the window is refused with: it ends with its chat.
pub const ENDS_WITH_ITS_CHAT: &str = "This browser is a chat's, and ends with its chat: closing \
                                      its tab closes only the view.";

/// **Whether the window may end browser `program`** (#2024): never one a chat of `projects`
/// uses, which ends with its chat. Answers the refusal, or nothing.
pub fn closing_refused<'a>(
    projects: impl IntoIterator<Item = &'a ChatBrowsers>,
    program: &str,
) -> Option<String> {
    projects
        .into_iter()
        .any(|browsers| browsers.is_in_use(program))
        .then(|| ENDS_WITH_ITS_CHAT.to_owned())
}

/// **The browsers of project `plane`'s chats** (#2024): what the Chats list draws under each
/// chat and its tab draws live, read again on [`LISTED`]. The window's alone
/// (`ui::WINDOW_ONLY`): a browser is watched only in the person's own window.
#[tauri::command]
#[specta::specta]
pub async fn chat_browsers(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<ChatBrowser>, String> {
    Ok(planes.held(&plane)?.browsers().listed())
}

const WAITS_NO_MORE: &str = "Nothing is asking for a browser for that chat any more.";

/// **This project's sites, or also the open web**, on chat `session`'s browser Ask (#2013): its
/// browser starts at its next browser call, reaching that; the chat is told on its next turn.
/// With `always`, **Always for this persona** (#2241): kept for every chat as the chat's
/// persona in this project on this machine, and answering each of them that waits.
#[tauri::command]
#[specta::specta]
pub async fn allow_chat_browser(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    open_web: bool,
    always: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    let browsers_ended = Arc::clone(planes.browsers_ended());
    crate::off_the_window("allowing a chat's browser", move || {
        if always {
            allowed_always(
                &held,
                &plane,
                session,
                open_web,
                &held.audit_then_record(network.as_ref()),
                crate::sandboxing::now_secs(),
                &*browsers_ended,
            )
        } else {
            allowed(&held, session, open_web)
        }
    })
    .await
}

/// [`allow_chat_browser`]'s work, for this chat.
pub fn allowed(held: &crate::planes::Held, session: u32, open_web: bool) -> Result<(), String> {
    if !held.browsers().allow(session, open_web) {
        return Err(WAITS_NO_MORE.to_owned());
    }
    tell_allowed(
        held,
        session,
        open_web,
        purlis_core::askoutcome::Scope::Chat,
    );
    held.chats().asks_may_have_moved();
    Ok(())
}

fn tell_allowed(
    held: &crate::planes::Held,
    session: u32,
    open_web: bool,
    scope: purlis_core::askoutcome::Scope,
) {
    held.chats().tell_outcome(
        session,
        purlis_core::askoutcome::Outcome {
            about: purlis_core::askoutcome::About::Browser {
                open_web: Some(open_web),
            },
            answer: purlis_core::askoutcome::Answer::Allowed { scope },
        },
    );
}

/// What a Granted row says Always for a persona's browser reaches.
pub fn always_reach(open_web: bool) -> &'static str {
    if open_web {
        "this project's sites, its dev servers and the open web"
    } else {
        "this project's sites and its dev servers"
    }
}

/// What the audit says of Always for `persona`'s browser: the persona, and what it reaches.
pub fn always_said(persona: &str, open_web: bool) -> String {
    format!("{persona}: {}", always_reach(open_web))
}

/// [`allow_chat_browser`]'s work with `always` (#2241): for the persona chat `session` runs
/// with, from the app's own record. Refused while its Ask does not wait, for a chat on no
/// persona, and where an administrator's policy leaves out the scope it is kept at. Audited,
/// then kept in this machine's own file for the project, where no chat writes, and recorded for
/// the Granted list; then every chat as that persona that waits is answered.
pub fn allowed_always(
    held: &crate::planes::Held,
    plane: &crate::planes::PlaneId,
    session: u32,
    open_web: bool,
    audit: crate::sandboxing::Audit<'_>,
    at: u64,
    told: &dyn Fn(BrowsersEnded),
) -> Result<(), String> {
    use purlis_core::sandbox;
    if !held
        .browsers()
        .waiting()
        .iter()
        .any(|one| one.session == session)
    {
        return Err(WAITS_NO_MORE.to_owned());
    }
    let Some(persona) = persona_of(held, session) else {
        return Err(
            "This chat runs as no persona, so there is no persona to allow its \
                    browser for always: allow it for this chat."
                .to_owned(),
        );
    };
    let root = held.root();
    if let Some(why) = sandbox::policy::Locks::of(root).scope_removed(sandbox::grant::Level::You) {
        return Err(why);
    }
    audit(
        Some(session),
        &sandbox::grant::Audited {
            granted: true,
            what: sandbox::local::BROWSER,
            target: &always_said(&persona, open_web),
            level: sandbox::grant::Level::You,
        },
    )?;
    sandbox::local::keep_browser_always(root, &persona, open_web)
        .map_err(|why| format!("purlis could not keep it, so nothing was allowed: {why}"))?;
    if let Err(why) = sandbox::local::record_made(
        root,
        sandbox::local::Made {
            what: sandbox::local::BROWSER.to_owned(),
            target: persona.clone(),
            level: sandbox::grant::Level::You.word().to_owned(),
            at,
            chat: held.chats().recorded_chat(session).map(|chat| chat.name),
        },
    ) {
        // The Allow stands, and is audited; only the Granted list's "when" is lost.
        tracing::warn!("purlis: Always for a persona's browser was kept without when ({why})");
    }
    // The chats it allowed before follow the new answer at once: a narrower one ends their
    // browsers now (D-2013-11), and their tabs say so.
    for (narrowed, Ended { ended, unended }) in
        held.browsers()
            .always_changed(held.browser_driver(), &persona, open_web)
    {
        let note = if unended > 0 {
            NOT_ENDED_ON_ITS_TAB
        } else if ended > 0 {
            ENDED_ON_ITS_TAB
        } else {
            continue;
        };
        told(BrowsersEnded {
            plane: plane.clone(),
            session: narrowed,
            note: note.to_owned(),
        });
    }
    crate::browserneedsyou::settled(held);
    for answered in held.browsers().allow_always(&persona, open_web) {
        tell_allowed(
            held,
            answered,
            open_web,
            purlis_core::askoutcome::Scope::Persona {
                persona: persona.clone(),
            },
        );
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

/// What a chat's tab says once its browsers ended because the person removed Always for its
/// persona (#2241).
pub const ALWAYS_REMOVED_ON_ITS_TAB: &str = "purlis ended this chat's browsers, since you \
                                             removed Always for its persona. Its next browser \
                                             call asks you again.";

/// **The person removed Always for `persona`'s browser, in project `held`** (#2241): the
/// browsers of each chat it allowed end ([`ChatBrowsers::always_removed`]), and where any did,
/// `told` hears it for the chat's tab.
pub fn always_removed(
    held: &crate::planes::Held,
    plane: &crate::planes::PlaneId,
    persona: &str,
    told: &dyn Fn(BrowsersEnded),
) {
    for (session, Ended { ended, unended }) in held
        .browsers()
        .always_removed(held.browser_driver(), persona)
    {
        let note = if unended > 0 {
            NOT_ENDED_ON_ITS_TAB
        } else if ended > 0 {
            ALWAYS_REMOVED_ON_ITS_TAB
        } else {
            continue;
        };
        told(BrowsersEnded {
            plane: plane.clone(),
            session,
            note: note.to_owned(),
        });
    }
    crate::browserneedsyou::settled(held);
}

/// **Keep off** on chat `session`'s browser Ask (#2013): its every browser call is refused,
/// and it is told on its next turn not to retry, with the reason the person typed.
#[tauri::command]
#[specta::specta]
pub async fn keep_chat_browser_off(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    reason: Option<String>,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("keeping a chat's browser off", move || {
        kept_off(&held, session, reason.as_deref())
    })
    .await
}

/// [`keep_chat_browser_off`]'s work.
pub fn kept_off(
    held: &crate::planes::Held,
    session: u32,
    reason: Option<&str>,
) -> Result<(), String> {
    if !held.browsers().keep_off(session) {
        return Err(WAITS_NO_MORE.to_owned());
    }
    held.chats().tell_outcome(
        session,
        crate::askoutcome::kept(
            purlis_core::askoutcome::About::Browser { open_web: None },
            reason,
        ),
    );
    held.chats().asks_may_have_moved();
    Ok(())
}

fn no_such(name: &str) -> String {
    format!("this chat has no browser {name}: leave `browser` out to use the one it used last")
}

#[cfg(test)]
mod tests;

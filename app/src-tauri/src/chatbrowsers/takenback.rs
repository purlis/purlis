// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Browsers survive a crash of the window** (#2025, spec #2008).
//!
//! A browser runs in the session host, which keeps it, with its driver lease, for the grace
//! period after the window's link ends without a quit (ADR 0068 §2). What the window knew of it
//! (whose it is, what its chat calls it, the logins it holds) is in the window's memory, so the
//! window keeps a copy in the host beside it: its **window note** ([`Note`], session protocol
//! 1.39), sent from one place ([`ChatBrowsers::renote`]) as each of those changes, and never read
//! by the host. The host keeps a record of its own beside it: whether a login was ever performed
//! in the browser, which no note can unsay.
//!
//! **A window that comes back takes each back** once the project's put back has settled
//! ([`take_back`]), from what `programs_of` answers:
//!
//! - **a chat's browser goes back to its chat** where that same chat (its identity, ADR 0066) is
//!   open in this window again, under the name it knew it by, with its chat's answer to its Ask.
//!   An answer that rested on its persona's Always follows that Always as it is now, so a
//!   narrowing made while no window was there ends it. One holding a login goes back only where
//!   its chat may log in with that login now, as a hand-over checks; else it is the person's;
//! - **a pinned one whose chat did not come back** is the person's, under Pinned browsers;
//! - **the person's own**, and any browser a view tab of its own showed, comes back in a view
//!   tab ([`ChatBrowsers::take_theirs_back`]);
//! - **anything else is ended**: one with no note, a note that does not read, one being ended
//!   when the window went, one whose engine ended, a chat's whose chat did not come back, and one
//!   the host says was logged in whose note names no login. Nothing runs with no row or tab.
//!
//! The driver lease is the host's: one the person took over stays theirs, and is marked so on
//! its row from the first listing.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError};

use purlis_session_protocol::session::HeldProgram;
use serde::{Deserialize, Serialize};

use super::{
    AsPersona, Chat, ChatBrowser, ChatBrowsers, Driver, HeldLogin, Posture, browser_number,
};

/// **Whose a browser is, as its window note says** (#2025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum Owner {
    /// A chat's: by its number and identity, what it calls the browser, its answer to its
    /// browser Ask, the persona whose Always that answer rests on, and whether it is pinned.
    Chat {
        chat: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        name: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        open_web: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rests_on: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pinned: bool,
    },
    /// Pinned past its chat (#2022): the person's, listed by what its chat was called.
    Kept {
        chat: u32,
        name: String,
        was: String,
    },
    /// The person's own, opened in a view tab with no chat (#2176).
    Theirs,
    /// Being ended: nothing takes it back.
    Ending,
}

/// **A browser's window note** (#2025): whose it is, every login it was signed in with, whether
/// the person took it over, and whether a view tab of its own showed it. Nothing in it is a
/// value, an address or a page's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    #[serde(flatten)]
    pub owner: Owner,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logins: Vec<HeldLogin>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub taken_over: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub viewed: bool,
}

impl Note {
    /// A note of `owner`'s, with nothing else said.
    pub fn of(owner: Owner) -> Note {
        Note {
            owner,
            logins: Vec::new(),
            taken_over: false,
            viewed: false,
        }
    }

    /// The note a window that comes back reads, where it reads.
    pub fn read(note: Option<&purlis_session_protocol::session::ProgramNote>) -> Option<Note> {
        serde_json::from_value(note?.0.clone()).ok()
    }

    /// As the host keeps it.
    pub fn kept(&self) -> Option<purlis_session_protocol::session::ProgramNote> {
        serde_json::to_value(self)
            .ok()
            .map(purlis_session_protocol::session::ProgramNote)
    }
}

/// **What became of a browser a window before this one left** (#2025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakenBack {
    /// Back with its chat, under the name the chat knew it by.
    WithItsChat,
    /// The person's, under Pinned browsers: pinned past a chat that did not come back, or one
    /// its chat may no longer have.
    Kept,
    /// The person's own: a view tab of its own opens.
    Theirs,
    /// This window holds it already: nothing moves.
    HeldHere,
    /// Nothing takes it back: it is ended.
    NotTaken,
}

/// **What a window that comes back knows of its chats now** (#2025): the identity of each chat
/// open in it, by number; each chat's persona as it is now; and whether a chat may log in with
/// a login now, as a hand-over asks.
pub struct Back<'a> {
    pub open: &'a dyn Fn(u32) -> Option<Option<String>>,
    pub as_persona: &'a dyn Fn(u32) -> AsPersona,
    pub may_log_in: &'a dyn Fn(u32, &HeldLogin) -> Result<(), String>,
}

/// What a browser the host kept says of itself, beside its note.
pub struct Left<'a> {
    pub program: &'a str,
    pub note: Option<&'a Note>,
    /// The host says a login was performed in it, or it opened from a kept login.
    pub logged_in: bool,
    /// The host says the person drives it.
    pub person_drives: bool,
}

impl ChatBrowsers {
    /// **Each browser's window note is kept in `driver`** (#2025), the session host the browsers
    /// of the project at `root` run in.
    pub fn noting(self, driver: Option<Arc<dyn Driver>>, root: &Path) -> Self {
        ChatBrowsers {
            noting: driver.map(|driver| (driver, root.to_path_buf())),
            ..self
        }
    }

    /// **Chat `session` is `id`** (ADR 0066), as the app's record of it says: what its browsers'
    /// notes name it by (#2025).
    pub fn identify(&self, session: u32, id: Option<String>) {
        self.lock().entry(session).or_default().id = id;
    }

    /// **The person opened browser `program` in a view tab of its own** (#2176): theirs, noted
    /// so (#2025).
    pub fn opened_by_the_person(&self, program: &str) {
        let mut theirs = self
            .theirs_open
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !theirs.iter().any(|one| one == program) {
            theirs.push(program.to_owned());
        }
    }

    /// **A view tab of its own shows browser `program` now, or no longer** (#2025): one that
    /// did comes back in a view tab after a crash of the window.
    pub fn viewed(&self, program: &str, viewed: bool) {
        let mut shown = self.viewed.lock().unwrap_or_else(PoisonError::into_inner);
        shown.retain(|one| one != program);
        if viewed {
            shown.push(program.to_owned());
        }
    }

    /// **Sends the window note of every browser this window holds whose note changed** (#2025):
    /// the one place notes are sent from, after every change the window makes (each command
    /// tells the window where browsers moved, `Held::browsers_listed_since`). Takes the chats'
    /// lock: never called under it.
    pub fn renote(&self) {
        let Some((driver, root)) = &self.noting else {
            return;
        };
        let notes = self.notes_now();
        let changed: Vec<(String, Note)> = {
            let mut noted = self.noted.lock().unwrap_or_else(PoisonError::into_inner);
            noted.retain(|program, _| notes.iter().any(|(one, _)| one == program));
            let mut changed = Vec::new();
            for (program, note) in notes {
                if noted.get(&program) != Some(&note) {
                    noted.insert(program.clone(), note.clone());
                    changed.push((program, note));
                }
            }
            changed
        };
        for (program, note) in changed {
            driver.note(root, &program, &note);
        }
    }

    /// The note of each browser this window holds, as it is now.
    fn notes_now(&self) -> Vec<(String, Note)> {
        let mut owners: Vec<(String, Owner)> = Vec::new();
        for (session, chat) in self.lock().iter() {
            for (name, program) in &chat.browsers {
                if !chat.ended.contains(program) {
                    owners.push((program.clone(), owner_in(*session, chat, name, program)));
                }
            }
        }
        for kept in self.kept().iter() {
            owners.push((
                kept.program.clone(),
                Owner::Kept {
                    chat: kept.session,
                    name: kept.name.clone(),
                    was: kept.was.clone().unwrap_or_default(),
                },
            ));
        }
        for ending in &self.endings().browsers {
            owners.push((ending.program.clone(), Owner::Ending));
        }
        for program in self
            .theirs_open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            if !owners.iter().any(|(one, _)| one == program) {
                owners.push((program.clone(), Owner::Theirs));
            }
        }
        owners
            .into_iter()
            .map(|(program, owner)| {
                let note = self.with_records(&program, Note::of(owner));
                (program, note)
            })
            .collect()
    }

    /// `note`, with what is recorded of `program` whoever's it is: its logins, the person's
    /// take-over, and whether a view tab of its own shows it.
    fn with_records(&self, program: &str, note: Note) -> Note {
        Note {
            logins: self
                .logins_held
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(program)
                .cloned()
                .unwrap_or_default(),
            taken_over: self
                .person_typed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|one| one == program),
            viewed: self
                .viewed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|one| one == program),
            ..note
        }
    }

    /// Whether this window holds `program` already: a chat's, pinned past its chat, being
    /// ended, or the person's own.
    fn holds_now(&self, program: &str) -> bool {
        self.lock()
            .values()
            .any(|chat| chat.browsers.iter().any(|(_, one)| one == program))
            || self.is_kept(program)
            || self
                .endings()
                .browsers
                .iter()
                .any(|one| one.program == program)
            || self
                .theirs_open
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|one| one == program)
    }

    /// **Takes back browser `left`, left by a window before this one** (#2025), by its note and
    /// what `back` says of the chats open now; `now` is the time it counts as last used, in
    /// seconds since 1970. One this window holds already is left as it is, its logins merged.
    pub fn taken_back(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        left: &Left<'_>,
        back: &Back<'_>,
        now: u64,
    ) -> TakenBack {
        let program = left.program;
        if self.holds_now(program) {
            if let Some(note) = left.note {
                self.merge_logins(program, &note.logins);
            }
            return TakenBack::HeldHere;
        }
        let Some(note) = left.note else {
            return TakenBack::NotTaken;
        };
        // A login the host performed that the note does not name: what it holds is not known.
        if left.logged_in && note.logins.is_empty() {
            return TakenBack::NotTaken;
        }
        let mut follows = None;
        let taken = match &note.owner {
            Owner::Chat {
                chat,
                id,
                name,
                open_web,
                rests_on,
                pinned,
            } => {
                let same_chat = (back.open)(*chat).is_some_and(|now| now == *id);
                let may = note
                    .logins
                    .iter()
                    .all(|login| (back.may_log_in)(*chat, login).is_ok());
                if same_chat && may {
                    self.back_to_its_chat(*chat, program, name, *open_web, rests_on, *pinned, now);
                    follows = rests_on.as_ref().map(|_| *chat);
                    TakenBack::WithItsChat
                } else if *pinned || !note.logins.is_empty() && same_chat {
                    self.keep_for_the_person(*chat, name, program, &format!("chat {chat}"));
                    TakenBack::Kept
                } else {
                    TakenBack::NotTaken
                }
            }
            Owner::Kept { chat, name, was } => {
                self.keep_for_the_person(*chat, name, program, was);
                TakenBack::Kept
            }
            Owner::Theirs => {
                self.opened_by_the_person(program);
                TakenBack::Theirs
            }
            Owner::Ending => TakenBack::NotTaken,
        };
        if taken == TakenBack::NotTaken {
            return taken;
        }
        // What is recorded of it comes back with it.
        self.merge_logins(program, &note.logins);
        if note.taken_over {
            let mut typed = self
                .person_typed
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !typed.iter().any(|one| one == program) {
                typed.push(program.to_owned());
            }
        }
        if left.person_drives {
            let mut driving = self
                .person_drives
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !driving.iter().any(|one| one == program) {
                driving.push(program.to_owned());
            }
        }
        // The person's own, and any a view tab of its own showed, come back in a view tab.
        if taken == TakenBack::Theirs || note.viewed {
            self.viewed(program, true);
            let mut theirs = self
                .theirs_back
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !theirs.iter().any(|one| one == program) {
                theirs.push(program.to_owned());
            }
        }
        self.relisted();
        // An answer resting on Always follows it as it is now (#2241).
        if let Some(chat) = follows {
            let _ = self.follow_always(driver, chat, &(back.as_persona)(chat));
        }
        taken
    }

    #[allow(clippy::too_many_arguments)]
    fn back_to_its_chat(
        &self,
        session: u32,
        program: &str,
        name: &str,
        open_web: bool,
        rests_on: &Option<String>,
        pinned: bool,
        now: u64,
    ) {
        let mut chats = self.lock();
        let chat = chats.entry(session).or_default();
        // It had a browser, so the person allowed it, for this chat or by its persona's Always.
        if matches!(chat.posture, Posture::Unasked | Posture::Asked) {
            chat.posture = Posture::Allowed { open_web };
            chat.rests_on.clone_from(rests_on);
        }
        let name = match browser_number(name) {
            Some(_) if !chat.browsers.iter().any(|(one, _)| one == name) => name.to_owned(),
            _ => chat.next_name(),
        };
        if let Some(number) = browser_number(&name) {
            chat.next = chat.next.max(number + 1);
        }
        chat.browsers.push((name.clone(), program.to_owned()));
        chat.current = Some(name);
        chat.used.insert(program.to_owned(), now);
        if pinned {
            chat.pinned.push(program.to_owned());
        }
    }

    fn keep_for_the_person(&self, session: u32, name: &str, program: &str, was: &str) {
        let mut kept = self.kept();
        if !kept.iter().any(|one| one.program == program) {
            kept.push(ChatBrowser {
                session,
                name: name.to_owned(),
                program: program.to_owned(),
                pinned: true,
                was: Some(was.to_owned()),
                ..ChatBrowser::default()
            });
        }
    }

    /// Every login of `logins` is held by `program` too, beside those recorded already.
    fn merge_logins(&self, program: &str, logins: &[HeldLogin]) {
        if logins.is_empty() {
            return;
        }
        let mut held = self
            .logins_held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let all = held.entry(program.to_owned()).or_default();
        for login in logins {
            if !all.contains(login) {
                all.push(login.clone());
            }
        }
    }

    /// **The browsers a crash of the window left that come back in a view tab** (#2025), each
    /// once: the person's own, and any a view tab of its own showed.
    pub fn take_theirs_back(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .theirs_back
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }
}

/// The owner of chat `session`'s browser `name`, `program`, as its note says it.
fn owner_in(session: u32, chat: &Chat, name: &str, program: &str) -> Owner {
    Owner::Chat {
        chat: session,
        id: chat.id.clone(),
        name: name.to_owned(),
        open_web: matches!(chat.posture, Posture::Allowed { open_web: true }),
        rests_on: chat.rests_on.clone(),
        pinned: chat.pinned.iter().any(|one| one == program),
    }
}

/// **Takes back the browsers project `held` left in its session host** (#2025), once its put
/// back has settled: each by its note ([`ChatBrowsers::taken_back`]); every other is ended. The
/// window is told where they moved.
pub fn take_back(held: &crate::planes::Held) {
    let left = held.chats().browsers_left();
    if left.is_empty() {
        return;
    }
    let open: Vec<(u32, Option<String>)> = held
        .chats()
        .open_now()
        .into_iter()
        .map(|open| (open.session, held.chats().id_of(open.session)))
        .collect();
    let listing = held.browsers().listing();
    let back = Back {
        open: &|session| {
            open.iter()
                .find(|(one, _)| *one == session)
                .map(|(_, id)| id.clone())
        },
        as_persona: &|session| super::as_persona_of(held, session),
        may_log_in: &|session, login| super::may_log_in(held, session, login),
    };
    take_back_from(
        left,
        &back,
        held.browsers(),
        held.browser_driver(),
        held.root(),
    );
    held.browsers_listed_since(listing);
}

/// [`take_back`]'s work: browsers `left` in the project at `root`, taken back into `browsers`
/// and `driver` by what `back` says of the chats open now, or ended.
pub(crate) fn take_back_from(
    left: Vec<HeldProgram>,
    back: &Back<'_>,
    browsers: &ChatBrowsers,
    driver: Option<&Arc<dyn Driver>>,
    root: &Path,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    for browser in left {
        let note = Note::read(browser.note.as_ref());
        // One whose engine ended while no window was there is gone.
        let taken = if browser.exit.is_some() {
            TakenBack::NotTaken
        } else {
            browsers.taken_back(
                driver,
                &Left {
                    program: &browser.program,
                    note: note.as_ref(),
                    logged_in: browser.logged_in,
                    person_drives: browser.driver
                        == Some(purlis_session_protocol::browsing::Driver::Person),
                },
                back,
                now,
            )
        };
        end_or_keep(driver, root, &browser.program, taken);
    }
}

fn end_or_keep(driver: Option<&Arc<dyn Driver>>, root: &Path, program: &str, taken: TakenBack) {
    match taken {
        TakenBack::HeldHere => {}
        TakenBack::NotTaken => {
            tracing::info!(
                "purlis: browser {program} was left in the session host and nothing took it \
                 back, so it is ended"
            );
            if let Some(driver) = driver
                && driver.close(program).is_err()
            {
                let _ = driver.stop(program);
            }
        }
        taken => {
            tracing::info!(
                "purlis: browser {program} was left in the session host and is back ({taken:?})"
            );
            if let Some(driver) = driver {
                driver.taken_back(root, program);
            }
        }
    }
}

/// Where a browser's note is sent from: the project's root, with the session host.
pub(super) type Noting = Option<(Arc<dyn Driver>, PathBuf)>;

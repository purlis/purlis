// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Browsers survive a crash of the window** (#2025, spec #2008).
//!
//! A browser runs in the session host, which keeps it, with its driver lease, for the grace
//! period after the window's link ends without a quit (ADR 0068 §2). What the window knew of it
//! (whose chat it is, what that chat calls it, the logins it holds) is in the window's memory,
//! so the window keeps a copy in the host beside it: its **note** ([`Note`], session protocol
//! 1.42), written as each of those changes ([`ChatBrowsers::note`]) and never read by the host.
//!
//! **A window that comes back takes each back** once the project's put back has settled
//! ([`take_back`]), from what `programs_of` answers:
//!
//! - **a chat's browser goes back to its chat** where that chat is open again, under the name
//!   it knew it by, with its chat's answer to its Ask, so its row, its thumbnail and the chat's
//!   next call find it as they did; its view draws from the next whole frame the host sends;
//! - **a pinned one whose chat did not come back** is the person's, listed under Pinned
//!   browsers, as one whose chat closed is;
//! - **the person's own** comes back in a view tab of its own ([`ChatBrowsers::take_theirs_back`]);
//! - **anything else is ended**: one with no note, a note that does not read, one whose engine
//!   ended, a chat's whose chat did not come back. Nothing runs with no row or tab.
//!
//! The driver lease is the host's: one the person took over stays theirs, and is marked so on
//! its row from the first listing. The logins it holds and the person's take-over come back
//! with it, so a hand-over still checks every login and no quit keeps where it was.

use std::sync::{Arc, PoisonError};

use purlis_session_protocol::session::HeldProgram;
use serde::{Deserialize, Serialize};

use super::{Chat, ChatBrowser, ChatBrowsers, Driver, HeldLogin, Posture};

/// **What the window keeps of a browser in its session host** (#2025): the host keeps it
/// beside the browser and answers it to a window that comes back, which takes the browser back
/// by it. Nothing in it is a value, an address or a page's text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// The chat whose it is, by its number: none for the person's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<u32>,
    /// What that chat calls it: `b1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The chat's answer to its Ask reached the open web too.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open_web: bool,
    /// The person pinned it (#2022): it outlives its chat.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Pinned past its chat, what that chat was called (#2022).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub was: Option<String>,
    /// Every login it was signed in with (#2699).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logins: Vec<HeldLogin>,
    /// The person took it over at some time (#2022): no quit keeps where it was.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub typed: bool,
    /// The person's own, opened in a view tab with no chat (#2176).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub theirs: bool,
}

impl Note {
    /// The note of a browser the person opens, as it starts.
    pub fn theirs() -> Note {
        Note {
            theirs: true,
            ..Note::default()
        }
    }

    /// The note a window that comes back reads, where it reads.
    pub fn read(note: Option<&serde_json::Value>) -> Option<Note> {
        serde_json::from_value(note?.clone()).ok()
    }
}

/// **What became of a browser a window before this one left** (#2025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakenBack {
    /// Back with its chat, under the name the chat knew it by.
    WithItsChat,
    /// Pinned past a chat that did not come back: the person's, under Pinned browsers.
    Kept,
    /// The person's own: a view tab of its own opens.
    Theirs,
    /// Nothing takes it back: it is ended.
    NotTaken,
}

impl ChatBrowsers {
    /// **Each browser's note is kept in `driver`** (#2025), the session host they run in.
    pub fn noting(self, driver: Option<Arc<dyn Driver>>) -> Self {
        ChatBrowsers {
            noting: driver,
            ..self
        }
    }

    /// **Notes browser `program` as it is now** (#2025): its chat's, one pinned past its chat,
    /// or the person's own. Never one being ended. Takes the chats' lock: never called under it.
    pub(super) fn note(&self, program: &str) {
        if self.noting.is_none() {
            return;
        }
        let chats = self.lock();
        if let Some((session, chat)) = chats.iter().find(|(_, chat)| {
            chat.browsers.iter().any(|(_, one)| one == program)
                && !chat.ended.iter().any(|one| one == program)
        }) {
            self.noted_in(*session, chat, program);
            return;
        }
        drop(chats);
        let kept = self
            .kept()
            .iter()
            .find(|one| one.program == program)
            .cloned();
        let note = match kept {
            Some(kept) => Note {
                chat: Some(kept.session),
                name: Some(kept.name),
                pinned: true,
                was: kept.was,
                ..self.records_noted(program)
            },
            None if self
                .endings()
                .browsers
                .iter()
                .any(|one| one.program == program) =>
            {
                return;
            }
            None => Note {
                theirs: true,
                ..self.records_noted(program)
            },
        };
        self.send_note(program, &note);
    }

    /// **Notes browser `program` of chat `session`, `chat`** (#2025), with the chats' lock held
    /// by the caller.
    pub(super) fn noted_in(&self, session: u32, chat: &Chat, program: &str) {
        if self.noting.is_none() {
            return;
        }
        let note = Note {
            chat: Some(session),
            name: chat
                .browsers
                .iter()
                .find(|(_, one)| one == program)
                .map(|(name, _)| name.clone()),
            open_web: matches!(chat.posture, Posture::Allowed { open_web: true }),
            pinned: chat.pinned.iter().any(|one| one == program),
            ..self.records_noted(program)
        };
        self.send_note(program, &note);
    }

    /// What is recorded of `program` whoever's it is: its logins and the person's take-over.
    fn records_noted(&self, program: &str) -> Note {
        Note {
            logins: self
                .logins_held
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(program)
                .cloned()
                .unwrap_or_default(),
            typed: self
                .person_typed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|one| one == program),
            ..Note::default()
        }
    }

    fn send_note(&self, program: &str, note: &Note) {
        if let Some(driver) = &self.noting {
            driver.note(program, note);
        }
    }

    /// **Takes back browser `program`, left by a window before this one** (#2025), by its
    /// `note`: to its chat where `chat_back` says that chat is open in this window, pinned past
    /// it where it was pinned, the person's own as theirs; else nothing takes it. `person_drives`
    /// is the host's lease, and `now` the time it counts as last used, in seconds since 1970.
    pub fn taken_back(
        &self,
        program: &str,
        note: Option<&Note>,
        chat_back: &dyn Fn(u32) -> bool,
        person_drives: bool,
        now: u64,
    ) -> TakenBack {
        let Some(note) = note else {
            return TakenBack::NotTaken;
        };
        let back = match note.chat {
            Some(session) if chat_back(session) => {
                let mut chats = self.lock();
                let chat = chats.entry(session).or_default();
                if !chat.browsers.iter().any(|(_, one)| one == program) {
                    // Its chat's answer to its Ask: it had a browser, so the person allowed it.
                    if matches!(chat.posture, Posture::Unasked | Posture::Asked) {
                        chat.posture = Posture::Allowed {
                            open_web: note.open_web,
                        };
                    }
                    let name = note
                        .name
                        .clone()
                        .filter(|name| !chat.browsers.iter().any(|(one, _)| one == name))
                        .unwrap_or_else(|| format!("b{}", chat.next));
                    let number = name
                        .strip_prefix('b')
                        .and_then(|number| number.parse::<u32>().ok());
                    chat.next = chat.next.max(number.map_or(chat.next, |n| n + 1));
                    chat.browsers.push((name.clone(), program.to_owned()));
                    chat.current = Some(name);
                    chat.used.insert(program.to_owned(), now);
                    if note.pinned {
                        chat.pinned.push(program.to_owned());
                    }
                }
                TakenBack::WithItsChat
            }
            Some(session) if note.pinned || note.was.is_some() => {
                let name = note.name.clone().unwrap_or_default();
                let was = note
                    .was
                    .clone()
                    .unwrap_or_else(|| format!("chat {session}"));
                let mut kept = self.kept();
                if !kept.iter().any(|one| one.program == program) {
                    kept.push(ChatBrowser {
                        session,
                        name,
                        program: program.to_owned(),
                        pinned: true,
                        was: Some(was),
                        ..ChatBrowser::default()
                    });
                }
                TakenBack::Kept
            }
            None if note.theirs => {
                let mut theirs = self
                    .theirs_back
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if !theirs.iter().any(|one| one == program) {
                    theirs.push(program.to_owned());
                }
                TakenBack::Theirs
            }
            _ => return TakenBack::NotTaken,
        };
        // What is recorded of it comes back with it.
        if !note.logins.is_empty() {
            self.logins_held
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(program.to_owned(), note.logins.clone());
        }
        if note.typed {
            let mut typed = self
                .person_typed
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !typed.iter().any(|one| one == program) {
                typed.push(program.to_owned());
            }
        }
        if person_drives {
            let mut driving = self
                .person_drives
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !driving.iter().any(|one| one == program) {
                driving.push(program.to_owned());
            }
        }
        self.relisted();
        back
    }

    /// **The person's own browsers taken back since the window last asked** (#2025), each once:
    /// the window opens a view tab of each.
    pub fn take_theirs_back(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .theirs_back
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }
}

/// **Takes back the browsers project `held` left in its session host** (#2025), once its put
/// back has settled: each to its chat, pinned past it, or as the person's own, by its note
/// ([`ChatBrowsers::taken_back`]); every other is ended. The window is told where they moved.
pub fn take_back(held: &crate::planes::Held) {
    let left = held.chats().browsers_left();
    if left.is_empty() {
        return;
    }
    let open: Vec<u32> = held
        .chats()
        .open_now()
        .into_iter()
        .map(|open| open.session)
        .collect();
    let listing = held.browsers().listing();
    take_back_from(
        left,
        &open,
        held.browsers(),
        held.browser_driver(),
        held.root(),
    );
    held.browsers_listed_since(listing);
}

/// [`take_back`]'s work: browsers `left` in the project at `root`, whose chats `open` are open in
/// this window again, taken back into `browsers` and `driver`, or ended.
pub(crate) fn take_back_from(
    left: Vec<HeldProgram>,
    open: &[u32],
    browsers: &ChatBrowsers,
    driver: Option<&Arc<dyn Driver>>,
    root: &std::path::Path,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    for browser in left {
        // One whose engine ended while no window was there is gone.
        let back = if browser.exit.is_some() {
            TakenBack::NotTaken
        } else {
            browsers.taken_back(
                &browser.program,
                Note::read(browser.note.as_ref()).as_ref(),
                &|session| open.contains(&session),
                browser.driver == Some(purlis_session_protocol::browsing::Driver::Person),
                now,
            )
        };
        match back {
            TakenBack::NotTaken => {
                tracing::info!(
                    "purlis: browser {} was left in the session host and nothing took it back, \
                     so it is ended",
                    browser.program
                );
                if let Some(driver) = driver
                    && driver.close(&browser.program).is_err()
                {
                    let _ = driver.stop(&browser.program);
                }
            }
            back => {
                tracing::info!(
                    "purlis: browser {} was left in the session host and is back ({back:?})",
                    browser.program
                );
                if let Some(driver) = driver {
                    driver.taken_back(root, &browser.program);
                }
            }
        }
    }
}

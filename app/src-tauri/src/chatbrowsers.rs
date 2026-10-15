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
//!   `browser_new` (each apart, its cookies and storage its own), at most
//!   [`AT_MOST_PER_CHAT`]; each is named to the chat as `b1`, `b2`, and is looked up only
//!   among the chat's own. A chat's browsers end with it.
//! - **Keep off** refuses every call of the chat's, in words that say not to retry.
//!
//! **In memory only**, like the other Asks a chat raises: a relaunch starts with none, and the
//! chat's next call asks again. Nothing a chat sends decides any of it: the chat is the one
//! whose token the line carries, its reach is the app's record of its sandbox, and only the
//! person's press in the window answers the Ask (`ui::WINDOW_ONLY`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};

/// The most browsers one chat holds at once: the project's default cap (spec #2008), until the
/// limits are settings (#2023).
pub const AT_MOST_PER_CHAT: usize = 3;

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
    /// Carries `act` out on browser `program` under its own id `call_id`.
    fn act(&self, program: &str, call_id: &str, act: Act) -> Result<Browsed, Refused>;
    /// Ends browser `program`.
    fn close(&self, program: &str);
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

/// One Ask that waits: the chat, and when it began to wait, in seconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waits {
    pub session: u32,
    pub since: u64,
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
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            posture: Posture::Unasked,
            since: 0,
            browsers: Vec::new(),
            current: None,
            next: 1,
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
}

/// What a chat is told while the person has not answered its Ask.
pub const ASKED: &str = "purlis asked the person what this chat's browser may reach, and opened \
                         nothing. You will be told their answer on your next turn: don't retry \
                         a browser tool until then, and go on with what needs no browser.";

/// What a chat whose browser the person kept off is told.
pub const KEPT_OFF: &str = "The person kept this chat's browser off: don't retry or work around \
                            it.";

const NO_HOST: &str = "This window has no session host to run a browser in, so this chat has \
                       no browser.";

impl ChatBrowsers {
    fn lock(&self) -> MutexGuard<'_, HashMap<u32, Chat>> {
        self.chats.lock().unwrap_or_else(PoisonError::into_inner)
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
            })
            .collect();
        waits.sort_by_key(|one| (one.since, one.session));
        waits
    }

    /// **Allow** on chat `session`'s Ask, with the open web or not. Answers whether it waited.
    pub fn allow(&self, session: u32, open_web: bool) -> bool {
        self.answered(session, Posture::Allowed { open_web })
    }

    /// **Keep off** on chat `session`'s Ask. Answers whether it waited.
    pub fn keep_off(&self, session: u32) -> bool {
        self.answered(session, Posture::Off)
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

    /// Chat `session` has closed: its browsers end, and nothing of it is kept.
    pub fn forget(&self, driver: Option<&Arc<dyn Driver>>, session: u32) {
        let gone = self.lock().remove(&session);
        if let (Some(chat), Some(driver)) = (gone, driver) {
            for (_, program) in chat.browsers {
                driver.close(&program);
            }
        }
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
        now: u64,
    ) -> (Answer, bool) {
        let no = |why: &str| Answer::No {
            why: why.to_owned(),
        };
        let mut chats = self.lock();
        let chat = chats.entry(browse.chat).or_default();
        let open_web = match chat.posture {
            Posture::Unasked => {
                chat.posture = Posture::Asked;
                chat.since = now;
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
            if chat.browsers.len() >= AT_MOST_PER_CHAT {
                return Err(format!(
                    "this chat has {AT_MOST_PER_CHAT} browsers open, the most it may: close one \
                     with browser_close first"
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
                .map_err(|why| format!("the browser did not start: {why}"))?;
            let name = format!("b{}", chat.next);
            chat.next += 1;
            chat.browsers.push((name.clone(), program));
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
                drop(chats);
                driver.close(&program);
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
        match driver.act(&program, &browse.call_id, act) {
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
            why: format!("chat {} is not one this app has open", browse.chat),
        };
    }
    let reach = || reach_of(held.chats(), browse.chat);
    let (answer, raised) = held.browsers().answer(
        held.browser_driver(),
        held.root(),
        browse,
        &reach,
        crate::sandboxing::now_secs(),
    );
    if raised {
        tracing::info!(
            "purlis: chat {} asked for a browser; the person is asked what it may reach",
            browse.chat
        );
        held.chats().asks_may_have_moved();
    }
    answer
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

const WAITS_NO_MORE: &str = "Nothing is asking for a browser for that chat any more.";

/// **This project's sites, or also the open web**, on chat `session`'s browser Ask (#2013): its
/// browser starts at its next browser call, reaching that; the chat is told on its next turn.
#[tauri::command]
#[specta::specta]
pub async fn allow_chat_browser(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    open_web: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("allowing a chat's browser", move || {
        allowed(&held, session, open_web)
    })
    .await
}

/// [`allow_chat_browser`]'s work.
pub fn allowed(held: &crate::planes::Held, session: u32, open_web: bool) -> Result<(), String> {
    if !held.browsers().allow(session, open_web) {
        return Err(WAITS_NO_MORE.to_owned());
    }
    held.chats().tell_outcome(
        session,
        purlis_core::askoutcome::Outcome {
            about: purlis_core::askoutcome::About::Browser {
                open_web: Some(open_web),
            },
            answer: purlis_core::askoutcome::Answer::Allowed {
                scope: purlis_core::askoutcome::Scope::Chat,
            },
        },
    );
    held.chats().asks_may_have_moved();
    Ok(())
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
#[path = "chatbrowsers_tests.rs"]
mod tests;

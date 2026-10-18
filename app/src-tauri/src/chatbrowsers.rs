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
//!   most [`purlis_core::browserlimits::Limits::per_project`] together, and the device runs
//!   at most its budget (#2023). Each is named to the chat as `b1`, `b2`, and is
//!   looked up only among the chat's own. A chat's browsers end with it.
//! - **A browser's reach follows its chat's reach, including what the person withdraws**: a
//!   browser is given what its chat reaches as it starts, and when the person narrows the
//!   chat's reach while it runs, its browsers end ([`ChatBrowsers::narrowed`]); the next call
//!   opens one on what the chat reaches then, and says why.
//! - **Keep off** refuses every call of the chat's, in words that say not to retry.
//! - **A login is purlis's to perform** (#2016). `browser_login` names one of the chat's
//!   persona's declared browser logins; the app reads its values for exactly the login the
//!   chat's start armed (`purlis_core::secrets::brokered::login_for`) and hands them to the
//!   session host ([`Driver::log_in`]), whose browser fills them on the login's own origin and
//!   hands the browser back reaching that origin alone. The chat is told the page the login
//!   left, never a value; it chooses which declared login, and nothing else.
//! - **A site new to the project asks** (#2018). Where the browser's judge held something back
//!   (an address the agent wrote for a site new to the project, a form or a body sent there, or
//!   typing on a page of it), the act's answer names the site, the chat is told it was asked
//!   about, and the chat's sites wait as one Ask in the registry, with Allow for this chat,
//!   Allow always on this machine, and Keep blocked. **An Allow is a browser site grant, for the
//!   browser alone** (ruled 2026-10-17): the chat's own commands still need their own Allow for
//!   that host. It is listed apart in the Granted list. Each act tells the browser what it
//!   reaches now ([`Driver::act`]): the chat's reach and its browser sites, so an Allow since it
//!   started reaches it at its next act. A chat started without the sandbox is asked nothing:
//!   until its reach is listed for its browser (#2242), its browser reaches only what it was
//!   given, and the chat is told so.
//!
//! **In memory only**, like the other Asks a chat raises: a relaunch starts with none, and the
//! chat's next call asks again. Nothing a chat sends decides any of it: the chat is the one
//! whose token the line carries, its reach is the app's record of its sandbox, and only the
//! person's press in the window answers the Ask (`ui::WINDOW_ONLY`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_core::sandbox::browser_reach::Site;
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach, LogIn};
use purlis_session_protocol::session::{Layer, Listed};

/// Why the session host did not carry an act out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The browser is not there any more: stopped, closed, or its engine ended.
    Gone,
    /// The browser's own refusal, in words that say what to do.
    Said(String),
}

/// Why the session host started no browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotStarted {
    /// The device runs as many browsers as its budget, `most` (#2023).
    Full { most: usize },
    /// It failed, in the host's words.
    Failed(String),
}

/// **Where a chat's browsers run**: the window's session host (`crate::browsers`), behind a
/// seam so what is decided here is tested without one.
pub trait Driver: Send + Sync + 'static {
    /// Starts a browser in the project at `root` reaching `reach`, and answers its program.
    fn start(&self, root: &Path, reach: BrowserReach) -> Result<String, NotStarted>;
    /// Carries `act` out on browser `program` under its own id `call_id`, the browser reaching
    /// `reach` from then on (#2018).
    fn act(
        &self,
        program: &str,
        call_id: &str,
        act: Act,
        reach: BrowserReach,
    ) -> Result<Browsed, Refused>;
    /// purlis performs `login` in browser `program` (#2016): the page it left once it handed
    /// the browser back, or why it did not go through, in words that carry no value.
    fn log_in(&self, program: &str, login: LogIn) -> Result<Browsed, Refused>;
    /// Ends browser `program`, and forgets it.
    fn close(&self, program: &str) -> Result<(), String>;
    /// Ends browser `program` by its id, with its reach proxy: what is tried when a close did
    /// not end it.
    fn stop(&self, program: &str) -> Result<(), String>;
    /// **The browsers the session host lists with a driver now** (#2023): the person's, from
    /// a take-over, or purlis's while it performs a login. The host's own word, which no idle
    /// stop goes against.
    fn driven(&self) -> Result<Vec<String>, String>;
}

/// How many of a chat's browsers were ended, and how many could not be yet: those are kept, and
/// tried again off every call's path until they end ([`Ending`]).
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
    /// The programs of its browsers whose engine ended by itself (#2385): no longer listed, and
    /// its next call on one says it has gone.
    ended: Vec<String>,
    /// The number the next browser is named by.
    next: u32,
    /// Its browsers were ended because the person narrowed its reach, and it is not told yet.
    narrowed: bool,
    /// Its persona as it was when its Ask was raised.
    as_persona: AsPersona,
    /// The persona whose Always it was allowed on (#2241), where it was: what removing or
    /// changing that Always follows.
    rests_on: Option<String>,
    /// The sites its browser asked about that wait on the person, oldest first, with when the
    /// first began to wait (#2018).
    sites_asked: Vec<Site>,
    sites_since: u64,
    /// The browser sites allowed for this chat alone, while the app holds it (#2018).
    sites: Vec<Site>,
    /// When each of its browsers, by its program, was last used: by the chat's act, or the
    /// person's (#2023), in seconds since 1970.
    used: HashMap<String, u64>,
    /// Its browsers that stopped for going unused, by the names it knew them by, until it is
    /// told (#2023).
    idled: Vec<String>,
    /// How long, in minutes, those had gone unused when they stopped.
    idled_after: u64,
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            posture: Posture::Unasked,
            since: 0,
            browsers: Vec::new(),
            current: None,
            ended: Vec::new(),
            next: 1,
            narrowed: false,
            as_persona: AsPersona::default(),
            rests_on: None,
            sites_asked: Vec::new(),
            sites_since: 0,
            sites: Vec::new(),
            used: HashMap::new(),
            idled: Vec::new(),
            idled_after: 0,
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

    /// How many of its browsers run: one whose engine ended counts no more.
    fn running(&self) -> usize {
        self.browsers
            .iter()
            .filter(|(_, program)| !self.ended.contains(program))
            .count()
    }

    fn drop_browser(&mut self, name: &str) {
        if let Some(program) = self.program(name) {
            self.ended.retain(|one| *one != program);
            self.used.remove(&program);
        }
        self.browsers.retain(|(one, _)| one != name);
        if self.current.as_deref() == Some(name) {
            self.current = self.browsers.last().map(|(one, _)| one.clone());
        }
    }
}

/// The browsers of one project's chats, and what the person said each may reach.
///
/// Two locks: the chats', then [`Ending`]'s, always in that order, and a browser moves from a
/// chat to those ending while both are held, so whoever counts the project's browsers under the
/// chats' lock never misses one that still runs.
pub struct ChatBrowsers {
    chats: Mutex<HashMap<u32, Chat>>,
    /// Browsers that were to end and are not ended yet.
    ending: Arc<Ending>,
    /// Moves each time a browser is added to the chats' or taken from them ([`Self::listing`]).
    listing: Arc<AtomicU64>,
    /// Told when a retry off every call's path ends a browser, so the window reads the listing
    /// again: nothing else of the window's would know it moved.
    told: Option<Arc<dyn Fn() + Send + Sync>>,
    /// The limits in force (#2023).
    limits: LimitsNow,
    /// The browsers the person drives now, by program (#2023): never stopped for going unused.
    person_drives: Mutex<Vec<String>>,
}

impl Default for ChatBrowsers {
    fn default() -> Self {
        ChatBrowsers {
            limits: Arc::new(|| purlis_core::browserlimits::Limits::for_memory(None)),
            person_drives: Mutex::default(),
            chats: Mutex::default(),
            ending: Arc::new(Ending::after(RETRY_FIRST, RETRY_MOST)),
            listing: Arc::default(),
            told: None,
        }
    }
}

/// How long the first retry of a browser that could not be ended waits (#2283).
const RETRY_FIRST: Duration = Duration::from_secs(1);
/// The longest a retry waits: each that ends nothing waits twice as long, up to this.
const RETRY_MOST: Duration = Duration::from_secs(60);

/// **Browsers that were to end and are not ended yet** (D-2013-16, #2283): no chat uses them.
/// Each is tried once as its chat lets go of it, on that path; one that did not end is retried
/// on a timer of its own, off every browser call's path, waiting longer after each try that
/// ends nothing, until it ends. Every one counts toward the project's browsers until it ended,
/// and one whose first try failed is listed under the chat it was of, as not ended yet.
struct Ending {
    state: Mutex<Endings>,
    /// Wakes the retry before its timer: Stop all.
    wake: Condvar,
    first: Duration,
    most: Duration,
}

#[derive(Default)]
struct Endings {
    /// Each browser let go of, until it ended: `unended` once its first try failed.
    browsers: Vec<ChatBrowser>,
    /// Whether the retry runs: one at most.
    retrying: bool,
    /// The retry is to try now, not when its timer says.
    now: bool,
}

impl Ending {
    fn after(first: Duration, most: Duration) -> Self {
        Ending {
            state: Mutex::default(),
            wake: Condvar::new(),
            first,
            most,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Endings> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **The retry**: waits, tries each browser whose first try failed, and lets go of those
    /// that ended, until none is left. Runs on its own thread.
    fn retried(
        &self,
        driver: &dyn Driver,
        listing: &AtomicU64,
        told: Option<&(dyn Fn() + Send + Sync)>,
    ) {
        let mut wait = self.first;
        loop {
            let tried: Vec<String> = {
                let mut endings = self.lock();
                let until = Instant::now() + wait;
                while !endings.now {
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        break;
                    }
                    endings = self
                        .wake
                        .wait_timeout(endings, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
                endings.now = false;
                let tried: Vec<String> = endings
                    .browsers
                    .iter()
                    .filter(|one| one.unended)
                    .map(|one| one.program.clone())
                    .collect();
                if tried.is_empty() {
                    endings.retrying = false;
                    return;
                }
                tried
            };
            // Never under the lock: a try may wait on the session host a while.
            let ended: Vec<String> = tried
                .into_iter()
                .filter(|program| end(driver, program))
                .collect();
            if ended.is_empty() {
                wait = (wait * 2).min(self.most);
                continue;
            }
            self.lock()
                .browsers
                .retain(|one| !ended.contains(&one.program));
            listing.fetch_add(1, Ordering::Relaxed);
            if let Some(told) = told {
                told();
            }
            wait = self.first;
        }
    }
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

/// What a chat is told of its browser `name`, which has ended.
pub fn gone_said(name: &str) -> String {
    format!(
        "browser {name} has ended (stopped, or its engine closed), and what it held is gone: the \
         next browser call opens a new one"
    )
}

/// What a chat is told while the person has not answered its Ask.
pub const ASKED: &str = "purlis asked the person what this chat's browser may reach, and opened \
                         nothing. You will be told their answer on your next turn: don't retry \
                         a browser tool until then, and go on with what needs no browser.";

/// The sites `sites` names, as a chat is told them.
fn sites_said(sites: &[Site]) -> String {
    match sites {
        [one] => format!("{one} is a site"),
        many => format!(
            "{} are sites",
            many.iter()
                .map(Site::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// What a chat is told of the sites its browser asked the person about (#2018).
fn asked_about(sites: &[Site]) -> String {
    format!(
        "{} new to this project, so purlis held back what this browser would have opened, sent \
         or typed there, and asked the person whether it may: you will be told their answer on \
         your next turn. Don't retry it until then; go on with what needs no new site",
        sites_said(sites)
    )
}

/// What a chat started without the sandbox is told of the sites its browser held back for
/// (#2018): nobody is asked, since its browser reaches only what it was given until #2242.
fn not_asked_about(sites: &[Site]) -> String {
    format!(
        "{} new to this project, so purlis held back what this browser would have opened, sent \
         or typed there. This chat started without the sandbox, so its browser reaches only the \
         sites it was given, and the person was not asked: go on with what needs no new site",
        sites_said(sites)
    )
}

/// The most sites one chat's browser waits on the person for at once (#2018).
const MOST_SITES_ASKED: usize = 64;

/// One chat whose browser's sites wait on the person (#2018): the chat, when the first began to
/// wait, and each site, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitesWait {
    pub session: u32,
    pub since: u64,
    pub sites: Vec<Site>,
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
    /// Browsers whose stuck ones are retried first after `first`, then at most every `most`.
    #[cfg(test)]
    pub fn retrying(first: Duration, most: Duration) -> Self {
        ChatBrowsers {
            ending: Arc::new(Ending::after(first, most)),
            ..Self::default()
        }
    }

    /// These browsers, telling `listed` when a retry off every call's path ends one (#2283).
    pub fn telling(self, listed: Arc<dyn Fn() + Send + Sync>) -> Self {
        ChatBrowsers {
            told: Some(listed),
            ..self
        }
    }

    /// **Browser `program` was used** at `now` (#2023): the person took it over or handed it back,
    /// or an Ask raised about it. It counts as used, as the chat's act does.
    pub fn touched(&self, program: &str, now: u64) {
        if let Some(chat) = self
            .lock()
            .values_mut()
            .find(|chat| chat.used.contains_key(program))
        {
            chat.used.insert(program.to_owned(), now);
        }
    }

    /// **The person took browser `program` over, or handed it back** (#2023), at `now`: while
    /// they drive it, it never stops for going unused, and either is a use of it.
    pub fn driven(&self, program: &str, by_the_person: bool, now: u64) {
        {
            let mut driving = self
                .person_drives
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            driving.retain(|one| one != program);
            if by_the_person {
                driving.push(program.to_owned());
            }
        }
        self.touched(program, now);
    }

    /// **Stops each chat's browser unused for the idle time** (#2023), at `now`: one neither
    /// the chat nor the person used for [`Limits::idle_after`], that the person does not drive,
    /// and that no Ask of `waiting` (by program) waits on the person about. It ends as a
    /// narrowing ends one, leaves its chat's rows, and the chat's next call is told why.
    /// Answers each chat with what was ended of its browsers.
    ///
    /// [`Limits::idle_after`]: purlis_core::browserlimits::Limits::idle_after
    pub fn idle(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        now: u64,
        waiting: &[String],
    ) -> Vec<(u32, Ended)> {
        let Some(after) = (self.limits)().idle_after else {
            return Vec::new();
        };
        let minutes = after.as_secs().div_ceil(60);
        let after = after.as_secs();
        // What the host says is driven now, asked before any lock: one it lists with a driver
        // never stops this way. A host that cannot say stops nothing.
        let host_drives = match driver.map(|driver| driver.driven()) {
            Some(Ok(driven)) => driven,
            Some(Err(why)) => {
                tracing::debug!("purlis: no browser stopped for going unused this time: {why}");
                return Vec::new();
            }
            None => Vec::new(),
        };
        let driving = {
            let mut driving = self
                .person_drives
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            // One no chat has any more is forgotten.
            let chats = self.lock();
            driving.retain(|program| {
                chats
                    .values()
                    .any(|chat| chat.browsers.iter().any(|(_, one)| one == program))
            });
            drop(chats);
            driving.clone()
        };
        let mut stopping: Vec<(u32, Vec<ChatBrowser>)> = Vec::new();
        {
            let mut chats = self.lock();
            for (session, chat) in chats.iter_mut() {
                let unused: Vec<(String, String)> = chat
                    .browsers
                    .iter()
                    .filter(|(_, program)| {
                        !chat.ended.contains(program)
                            && !driving.contains(program)
                            && !host_drives.contains(program)
                            && !waiting.contains(program)
                            && chat
                                .used
                                .get(program)
                                .is_some_and(|at| at.saturating_add(after) <= now)
                    })
                    .cloned()
                    .collect();
                if unused.is_empty() {
                    continue;
                }
                let browsers: Vec<ChatBrowser> = unused
                    .into_iter()
                    .map(|(name, program)| {
                        chat.drop_browser(&name);
                        chat.idled.push(name.clone());
                        chat.idled_after = minutes;
                        ChatBrowser {
                            session: *session,
                            name,
                            program,
                            unended: false,
                        }
                    })
                    .collect();
                self.letting_go(&browsers);
                stopping.push((*session, browsers));
            }
        }
        if !stopping.is_empty() {
            self.relisted();
        }
        stopping
            .into_iter()
            .map(|(session, browsers)| (session, self.end_now(driver, &browsers)))
            .collect()
    }

    /// These browsers, held to `limits` (#2023).
    #[cfg(test)]
    pub fn limited(self, limits: purlis_core::browserlimits::Limits) -> Self {
        self.limited_by(Arc::new(move || limits))
    }

    /// These browsers, held to the limits `limits` reads at each call: the person's settings
    /// over purlis's defaults (#2023).
    pub fn limited_by(self, limits: LimitsNow) -> Self {
        ChatBrowsers { limits, ..self }
    }

    fn endings(&self) -> MutexGuard<'_, Endings> {
        self.ending.lock()
    }

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
                chat.browsers
                    .iter()
                    .filter(|(_, program)| !chat.ended.contains(program))
                    .map(|(name, program)| ChatBrowser {
                        session: *session,
                        name: name.clone(),
                        program: program.clone(),
                        unended: false,
                    })
            })
            .collect();
        // And those not ended yet, after the ones its chat uses (D-2013-16).
        listed.extend(
            self.endings()
                .browsers
                .iter()
                .filter(|one| one.unended)
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

    /// Whether chat `session` still has browser `program` (#2015), its engine running.
    pub fn holds(&self, session: u32, program: &str) -> bool {
        self.lock().get(&session).is_some_and(|chat| {
            !chat.ended.iter().any(|one| one == program)
                && chat.browsers.iter().any(|(_, one)| one == program)
        })
    }

    /// Chat `session` lets go of its browser `name`, which has ended: the next call opens a new
    /// one.
    pub fn let_go_of_ended(&self, session: u32, name: &str) {
        if let Some(chat) = self.lock().get_mut(&session) {
            chat.drop_browser(name);
            self.relisted();
        }
    }

    /// **Browser `program`'s engine ended by itself** (#2385), as the session host said: a
    /// crash, or the host stopping it. It leaves its chat's rows at once, and the chat's next
    /// call on it says it has gone. Whether it was a chat's.
    pub fn engine_ended(&self, program: &str) -> bool {
        let mut chats = self.lock();
        let Some(chat) = chats.values_mut().find(|chat| {
            chat.browsers.iter().any(|(_, one)| one == program)
                && !chat.ended.iter().any(|one| one == program)
        }) else {
            return false;
        };
        chat.ended.push(program.to_owned());
        drop(chats);
        self.relisted();
        true
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

    /// Keeps `sites` waiting on the person for chat `session`'s browser (#2018), each once.
    fn asked_about(&self, session: u32, sites: &[Site], now: u64) {
        let mut chats = self.lock();
        let chat = chats.entry(session).or_default();
        if chat.sites_asked.is_empty() {
            chat.sites_since = now;
        }
        for site in sites {
            if !chat.sites_asked.contains(site) && chat.sites_asked.len() < MOST_SITES_ASKED {
                chat.sites_asked.push(site.clone());
            }
        }
    }

    /// **Every chat whose browser's sites wait on the person** (#2018), longest waiting first.
    pub fn sites_waiting(&self) -> Vec<SitesWait> {
        let mut waits: Vec<SitesWait> = self
            .lock()
            .iter()
            .filter(|(_, chat)| !chat.sites_asked.is_empty())
            .map(|(session, chat)| SitesWait {
                session: *session,
                since: chat.sites_since,
                sites: chat.sites_asked.clone(),
            })
            .collect();
        waits.sort_by_key(|one| (one.since, one.session));
        waits
    }

    /// **The person answered chat `session`'s sites `shown`** (#2018): they wait no more, and
    /// with `for_this_chat` its browser reaches them from its next act. Refused whole unless
    /// every one of them waits now: an answer is to what the person was shown.
    pub fn sites_answered(
        &self,
        session: u32,
        shown: &[Site],
        for_this_chat: bool,
    ) -> Result<(), String> {
        let mut chats = self.lock();
        let Some(chat) = chats.get_mut(&session) else {
            return Err(SITES_WAIT_NO_MORE.to_owned());
        };
        if shown.is_empty() || !shown.iter().all(|site| chat.sites_asked.contains(site)) {
            return Err(SITES_WAIT_NO_MORE.to_owned());
        }
        chat.sites_asked.retain(|site| !shown.contains(site));
        if for_this_chat {
            for site in shown {
                if !chat.sites.contains(site) {
                    chat.sites.push(site.clone());
                }
            }
        }
        Ok(())
    }

    /// The browser sites allowed for one chat alone (#2018), each with its chat: what the
    /// Granted list shows of them.
    pub fn chat_sites(&self) -> Vec<(u32, Site)> {
        let mut out: Vec<(u32, Site)> = self
            .lock()
            .iter()
            .flat_map(|(session, chat)| chat.sites.iter().map(|site| (*session, site.clone())))
            .collect();
        out.sort();
        out
    }

    /// **Removes `site` from what chat `session`'s browser was allowed** (#2018): its browsers
    /// end, as a narrowing ends them. Answers whether it was allowed.
    pub fn chat_site_removed(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        session: u32,
        site: &Site,
    ) -> Option<Ended> {
        let was = self.lock().get_mut(&session).is_some_and(|chat| {
            let before = chat.sites.len();
            chat.sites.retain(|one| one != site);
            chat.sites.len() != before
        });
        was.then(|| self.narrowed(driver, session))
    }

    /// **A browser site allowed for every chat here was removed** (#2018): every chat's
    /// browsers end, as a narrowing ends them. Answers each chat whose browsers that ended.
    pub fn sites_removed(&self, driver: Option<&Arc<dyn Driver>>) -> Vec<(u32, Ended)> {
        let browsing: Vec<u32> = self
            .lock()
            .iter()
            .filter(|(_, chat)| !chat.browsers.is_empty())
            .map(|(session, _)| *session)
            .collect();
        browsing
            .into_iter()
            .map(|session| (session, self.narrowed(driver, session)))
            .collect()
    }

    /// The browsers that were to end and are not ended yet.
    #[cfg(test)]
    pub fn unended(&self) -> Vec<String> {
        self.endings()
            .browsers
            .iter()
            .map(|one| one.program.clone())
            .collect()
    }

    /// **Lets go of `browsers`**, which leave their chat: called while the chats' lock is held,
    /// so they count toward the project's browsers with no gap until each has ended.
    fn letting_go(&self, browsers: &[ChatBrowser]) {
        self.endings().browsers.extend(browsers.iter().cloned());
    }

    /// **Ends each of `browsers`**, let go of before ([`Self::letting_go`]): one try each, here;
    /// one that did not end is retried off every call's path until it ends ([`Ending`]).
    /// Answers how many ended and did not.
    fn end_now(&self, driver: Option<&Arc<dyn Driver>>, browsers: &[ChatBrowser]) -> Ended {
        let ended: Vec<&str> = match driver {
            Some(driver) => browsers
                .iter()
                .filter(|one| end(driver.as_ref(), &one.program))
                .map(|one| one.program.as_str())
                .collect(),
            None => Vec::new(),
        };
        let unended = browsers.len() - ended.len();
        {
            let mut endings = self.endings();
            endings
                .browsers
                .retain(|one| !ended.contains(&one.program.as_str()));
            for one in &mut endings.browsers {
                if browsers.iter().any(|let_go| let_go.program == one.program) {
                    one.unended = true;
                }
            }
            if unended > 0
                && let Some(driver) = driver
            {
                self.retry(driver, &mut endings);
            }
        }
        // One that could not be ended is listed as not ended yet: its row moves.
        if unended > 0 {
            self.relisted();
        }
        Ended {
            ended: ended.len(),
            unended,
        }
    }

    /// Starts the retry ([`Ending::retried`]) where it does not run, with `endings` held.
    fn retry(&self, driver: &Arc<dyn Driver>, endings: &mut Endings) {
        if endings.retrying {
            return;
        }
        let ending = Arc::clone(&self.ending);
        let listing = Arc::clone(&self.listing);
        let told = self.told.clone();
        let driver = Arc::clone(driver);
        let spawned = std::thread::Builder::new()
            .name("purlis-browser-retry".into())
            .spawn(move || ending.retried(driver.as_ref(), &listing, told.as_deref()));
        match spawned {
            Ok(_) => endings.retrying = true,
            // Kept, counted and listed; the next browser let go of starts the retry again.
            Err(why) => {
                tracing::warn!("purlis: the retry of a browser not ended did not start: {why}")
            }
        }
    }

    /// **Stop all stopped every browser** (#2024): the chats' browsers leave them, and their
    /// rows and tabs, as each is ended (a browser Stop all ended answers as gone); one that
    /// could not be is kept and tried again, as any is. Each chat's answer to its Ask stands.
    pub fn all_stopped(&self, driver: Option<&Arc<dyn Driver>>) {
        let browsers: Vec<ChatBrowser> = {
            let mut chats = self.lock();
            let browsers: Vec<ChatBrowser> = chats
                .iter_mut()
                .flat_map(|(session, chat)| {
                    chat.current = None;
                    chat.ended.clear();
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
                .collect();
            self.letting_go(&browsers);
            browsers
        };
        if !browsers.is_empty() {
            self.relisted();
        }
        self.end_now(driver, &browsers);
        // One not ended yet is tried again now, off this path.
        let mut endings = self.endings();
        if endings.retrying {
            endings.now = true;
            self.ending.wake.notify_all();
        }
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
            chat.ended.clear();
            chat.narrowed |= !programs.is_empty();
            self.letting_go(&programs);
            programs
        };
        if !programs.is_empty() {
            self.relisted();
        }
        self.end_now(driver, &programs)
    }

    /// Chat `session` has closed: its browsers end, and nothing of it is kept but a browser that
    /// could not be ended yet, which is tried again until it is.
    pub fn forget(&self, driver: Option<&Arc<dyn Driver>>, session: u32) {
        let programs: Vec<ChatBrowser> = {
            let mut chats = self.lock();
            let programs: Vec<ChatBrowser> = chats
                .remove(&session)
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
            self.letting_go(&programs);
            programs
        };
        if !programs.is_empty() {
            self.relisted();
        }
        self.end_now(driver, &programs);
    }

    /// **Answers one browser call of a chat's**, and whether it raised the chat's browser Ask
    /// (what it may reach). The sites its browser held back are kept waiting on the person
    /// ([`Self::sites_waiting`]) where the chat is `confined`.
    ///
    /// `reach` is what the chat's browser may reach, its chat's reach and the browser sites
    /// allowed here, read as a browser starts and at each act; `confined` whether the chat has a
    /// sandbox's record of its reach, without which nobody is asked about a site; `now` is in
    /// seconds since 1970. The chat is the one whose token the line carried. With no `driver`
    /// there is no session host, and so no browser.
    /// A `browser_login` call's login, by its name, is `logins`'s to read (#2016): the
    /// declared page and the vault's values, or why there are none.
    #[allow(clippy::too_many_arguments)]
    pub fn answer(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        root: &Path,
        browse: &BrowseAsk,
        reach: &dyn Fn() -> BrowserReach,
        confined: bool,
        as_persona: &dyn Fn() -> AsPersona,
        logins: &dyn Fn(&str) -> Result<LogIn, String>,
        now: u64,
    ) -> (Answer, bool) {
        let persona = as_persona();
        self.follow_always(driver, browse.chat, &persona);
        let (mut answer, raised) =
            self.answer_now(driver, root, browse, reach, persona, logins, now);
        // What the browser held back for the person says so first (#2018).
        if let Answer::Browsed { browsed, .. } = &mut answer
            && !browsed.asked.is_empty()
        {
            let sites = sites_of(&browsed.asked);
            let said = if confined {
                self.asked_about(browse.chat, &sites, now);
                asked_about(&sites)
            } else {
                not_asked_about(&sites)
            };
            browsed.note = Some(match browsed.note.take() {
                Some(note) => format!("{said}. {note}"),
                None => said,
            });
        }
        // The first answer after a browser of its stopped unused says so (#2023).
        let idled = self
            .lock()
            .get_mut(&browse.chat)
            .map(|chat| std::mem::take(&mut chat.idled))
            .unwrap_or_default();
        if !idled.is_empty() {
            let minutes = self
                .lock()
                .get(&browse.chat)
                .map_or(0, |chat| chat.idled_after);
            let said = idled_said(&idled, minutes);
            match &mut answer {
                Answer::Browsed { browsed, .. } => {
                    browsed.note = Some(match browsed.note.take() {
                        Some(note) => format!("{said}. {note}"),
                        None => said,
                    });
                }
                Answer::No { why } => *why = format!("{said}. {why}"),
                _ => {}
            }
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

    #[allow(clippy::too_many_arguments)]
    fn answer_now(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        root: &Path,
        browse: &BrowseAsk,
        reach: &dyn Fn() -> BrowserReach,
        as_persona: AsPersona,
        logins: &dyn Fn(&str) -> Result<LogIn, String>,
        now: u64,
    ) -> (Answer, bool) {
        let no = |why: &str| Answer::No {
            why: why.to_owned(),
        };
        let mut chats = self.lock();
        // Under the chats' lock, so a browser on its way from a chat to its end is counted
        // once, never missed (#2283).
        let unended = self.endings().browsers.len();
        let elsewhere: usize = unended
            + chats
                .iter()
                .filter(|(session, _)| **session != browse.chat)
                .map(|(_, chat)| chat.running())
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
        let chat_sites = chat.sites.clone();
        let per_project = (self.limits)().per_project;
        let start = |chat: &mut Chat| -> Result<String, String> {
            if elsewhere + chat.running() >= per_project {
                return Err(purlis_core::browserlimits::project_full_said(per_project));
            }
            let program = driver
                .start(
                    root,
                    with_sites(
                        reach(),
                        open_web,
                        &chat.sites,
                        &purlis_core::sandbox::policy::Locks::of(root),
                    ),
                )
                .map_err(|not| match not {
                    NotStarted::Full { most } => purlis_core::browserlimits::device_full_said(most),
                    NotStarted::Failed(why) => format!(
                        "the browser did not start: {why}. Try once more; if it fails again, \
                         go on with what needs no browser and tell the person what it said"
                    ),
                })?;
            let name = format!("b{}", chat.next);
            chat.next += 1;
            chat.used.insert(program.clone(), now);
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
                let closing = [ChatBrowser {
                    session: browse.chat,
                    name: name.clone(),
                    program,
                    unended: false,
                }];
                self.letting_go(&closing);
                drop(chats);
                if self.end_now(Some(driver), &closing).unended > 0 {
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
            Browsing::Act(act) => Doing::Act(act.clone()),
            Browsing::LogIn { login } => Doing::LogIn(login.clone()),
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
        // One whose engine ended is gone, as the host would say (#2385).
        let ended = chat.ended.contains(&program);
        if !ended {
            chat.used.insert(program.clone(), now);
        }
        // The act may wait on its page a while: never under the lock.
        drop(chats);
        let reach_now = with_sites(
            reach(),
            open_web,
            &chat_sites,
            &purlis_core::sandbox::policy::Locks::of(root),
        );
        let acted = match act {
            _ if ended => Err(Refused::Gone),
            Doing::Act(act) => driver.act(&program, &browse.call_id, act, reach_now),
            // The login is read only now, for the browser it goes into, and handed on whole.
            Doing::LogIn(declared) => match logins(&declared) {
                Ok(login) => driver.log_in(&program, login).map(|mut browsed| {
                    browsed.note = Some(logged_in_said(&declared, browsed.note.as_deref()));
                    browsed
                }),
                Err(why) => Err(Refused::Said(why)),
            },
        };
        match acted {
            Ok(browsed) => (
                Answer::Browsed {
                    browser: name,
                    browsed: Box::new(browsed),
                },
                false,
            ),
            Err(Refused::Said(why)) => (no(&why), false),
            Err(Refused::Gone) => {
                self.let_go_of_ended(browse.chat, &name);
                (no(&gone_said(&name)), false)
            }
        }
    }
}

/// What a chat's browser call asks of the browser itself: one of its acts, or a login of
/// purlis's by the login's name (#2016).
enum Doing {
    Act(Act),
    LogIn(String),
}

/// What the chat is told once purlis performed its browser login `name` (#2016): that its
/// password field went, where, and what the browser reaches from now. Never that the person is
/// signed in, which only the page can show (it may ask for a second step), and never a value.
fn logged_in_said(name: &str, note: Option<&str>) -> String {
    let gone = note.unwrap_or("the password field is gone since purlis submitted the login");
    format!(
        "{gone}, with {name}: take a snapshot to see whether the page asks for anything more. \
         This browser now reaches that site alone; open another with browser_new for any other \
         site"
    )
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

/// **Browser `program`'s engine ended by itself** (#2385), in whichever project its chat is:
/// it leaves its chat's rows, and an Ask about it ends with an Update
/// (`crate::browserneedsyou::settled`), at once, before the chat calls again.
pub fn engine_ended(projects: &[Arc<crate::planes::Held>], program: &str) {
    for held in projects {
        let listing = held.browsers().listing();
        if held.browsers().engine_ended(program) {
            held.browsers_listed_since(listing);
            crate::browserneedsyou::settled(held);
        }
    }
}

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
/// [`ChatBrowsers::answer`], with what its browser reaches read at each act from the app's
/// record of its sandbox and the browser sites allowed here ([`reach_of`]). Raising its browser
/// Ask, or keeping sites waiting on the person, tells the registry.
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
    let reach = || reach_of(held, browse.chat);
    let confined = held.chats().confines_now(browse.chat).is_some();
    let as_persona = || as_persona_of(held, browse.chat);
    let logins = |name: &str| login_of(held, browse.chat, name);
    let listing = held.browsers().listing();
    let (answer, raised) = held.browsers().answer(
        held.browser_driver(),
        held.root(),
        browse,
        &reach,
        confined,
        &as_persona,
        &logins,
        now,
    );
    held.browsers_listed_since(listing);
    // The sites the browser held back wait in the registry (#2018).
    if confined
        && let Answer::Browsed { browsed, .. } = &answer
        && !browsed.asked.is_empty()
    {
        held.chats().asks_may_have_moved();
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

/// **Chat `session`'s browser login `name`** (#2016): read for the chat as the app records it,
/// never as the line says, exactly as a declared tool's run is (`purlis_core::secrets::brokered`).
fn login_of(held: &crate::planes::Held, session: u32, name: &str) -> Result<LogIn, String> {
    let open = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == session)
        .ok_or_else(|| format!("chat {session} is not one this app has open"))?;
    let asker = purlis_core::secrets::brokered::Asker {
        root: held.root().to_path_buf(),
        env: purlis_core::secrets::Env::from_process(),
        chat: session,
        persona: open.persona,
        folder: open.cwd,
        confines: held.chats().confines_now(session),
        harness: open.harness,
    };
    purlis_core::secrets::brokered::login_for(&asker, name)
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
/// listed, D-2011-10), and the browser sites you allowed for every chat here (#2018) that
/// policy leaves in force. A host allowed for the chat's commands is a site its browser
/// reaches (D-2018-7); a browser site is never one its commands reach. A chat started without
/// the sandbox has no such record, so its browser starts with nothing listed, and reaches a
/// public site only where the person let it reach the open web. Its own sites, for this chat
/// alone, are added by [`ChatBrowsers::answer`].
fn reach_of(held: &crate::planes::Held, session: u32) -> BrowserReach {
    use purlis_core::sandbox;
    let Some(confines) = held.chats().confines_now(session) else {
        return BrowserReach::default();
    };
    let mut reach = purlis_core::browsertools::browser_reach(&confines.decides(), false);
    let root = held.root();
    let locks = sandbox::policy::Locks::of(root);
    reach.listed.extend(
        sandbox::local::browser_sites_in_force(root, &locks)
            .iter()
            .map(|site| Listed {
                host: site.to_string(),
                layer: Layer::You,
            }),
    );
    reach
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

/// **Allow on chat `session`'s browser sites** (#2018): `sites`, the sites the Ask showed, are
/// browser sites at `level`, this chat or every chat of this project on this machine. A grant
/// for the browser alone (ruled 2026-10-17): the chat's own commands still need their own Allow
/// for each host. Its browser reaches them from its next act; the chat is told on its next turn.
#[tauri::command]
#[specta::specta]
pub async fn allow_browser_sites(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    sites: Vec<String>,
    level: crate::sandboxing::GrantLevel,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window("allowing a browser's sites", move || {
        sites_allowed(
            &held,
            session,
            &sites,
            level.into(),
            &held.audit_then_record(network.as_ref()),
            crate::sandboxing::now_secs(),
        )
    })
    .await
}

/// The sites the window sent, each read as a site; one that is not refuses them all.
fn sites_sent(sites: &[String]) -> Result<Vec<Site>, String> {
    sites.iter().map(|written| Site::parse(written)).collect()
}

/// [`allow_browser_sites`]'s work: judged, then audited, then kept, then answered. Refused
/// whole where a site is no site, the level is not one a browser site is kept at, policy
/// removes that scope or never allows a site's host, or the sites wait no more.
pub fn sites_allowed(
    held: &crate::planes::Held,
    session: u32,
    sites: &[String],
    level: purlis_core::sandbox::grant::Level,
    audit: crate::sandboxing::Audit<'_>,
    at: u64,
) -> Result<(), String> {
    use purlis_core::sandbox::{self, grant::Level};
    let sites = sites_sent(sites)?;
    if !matches!(level, Level::Chat | Level::You) {
        return Err(
            "purlis allows a browser site for this chat, or for every chat of this project on \
             this machine. A browser site for everyone in the project is coming, in #2443."
                .to_owned(),
        );
    }
    let root = held.root();
    let locks = sandbox::policy::Locks::of(root);
    if let Some(why) = locks.scope_removed(level) {
        return Err(why);
    }
    for site in &sites {
        let host = sandbox::hosts::Host::parse(&site.to_string())?;
        if let Some(why) = locks.refuses_grant(&sandbox::grant::What::Host(host), level) {
            return Err(why);
        }
    }
    let waiting = held
        .browsers()
        .sites_waiting()
        .into_iter()
        .find(|one| one.session == session);
    let Some(waiting) = waiting.filter(|one| sites.iter().all(|site| one.sites.contains(site)))
    else {
        return Err(SITES_WAIT_NO_MORE.to_owned());
    };
    if !named_whole(&waiting.sites, &sites) {
        return Err(SITES_NOT_NAMED.to_owned());
    }
    for site in &sites {
        audit(
            Some(session),
            &sandbox::grant::Audited {
                granted: true,
                what: sandbox::local::BROWSER_SITE,
                target: &site.to_string(),
                level,
            },
        )?;
    }
    if level == Level::You {
        for site in &sites {
            sandbox::local::grant_browser_site(root, site).map_err(|why| {
                format!("purlis could not keep {site}, so nothing was allowed: {why}")
            })?;
            if let Err(why) = sandbox::local::record_made(
                root,
                sandbox::local::Made {
                    what: sandbox::local::BROWSER_SITE.to_owned(),
                    target: site.to_string(),
                    level: level.word().to_owned(),
                    at,
                    chat: held.chats().recorded_chat(session).map(|chat| chat.name),
                },
            ) {
                // The Allow stands, and is audited; only the Granted list's "when" is lost.
                tracing::warn!("purlis: a browser site was kept without when ({why})");
            }
        }
    }
    held.browsers()
        .sites_answered(session, &sites, level == Level::Chat)?;
    for site in &sites {
        held.chats().tell_outcome(
            session,
            purlis_core::askoutcome::Outcome {
                about: purlis_core::askoutcome::About::BrowserSite {
                    site: site.to_string(),
                },
                answer: purlis_core::askoutcome::Answer::Allowed {
                    scope: purlis_core::askoutcome::Scope::of(level),
                },
            },
        );
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

/// **Keep blocked on chat `session`'s browser sites** (#2018): nothing is allowed, they wait no
/// more, and the chat is told on its next turn not to retry, with the reason the person typed.
#[tauri::command]
#[specta::specta]
pub async fn keep_browser_sites_blocked(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    sites: Vec<String>,
    reason: Option<String>,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("keeping a browser's sites blocked", move || {
        sites_kept_blocked(&held, session, &sites, reason.as_deref())
    })
    .await
}

/// [`keep_browser_sites_blocked`]'s work.
pub fn sites_kept_blocked(
    held: &crate::planes::Held,
    session: u32,
    sites: &[String],
    reason: Option<&str>,
) -> Result<(), String> {
    let sites = sites_sent(sites)?;
    held.browsers().sites_answered(session, &sites, false)?;
    for site in &sites {
        held.chats().tell_outcome(
            session,
            crate::askoutcome::kept(
                purlis_core::askoutcome::About::BrowserSite {
                    site: site.to_string(),
                },
                reason,
            ),
        );
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

/// **Whether each of `sites` is one the ask of `waiting` names whole** (#2364): the same reading
/// the ask was drawn by ([`crate::asking::sites_named`]), so an Allow takes no site the person
/// could not read.
fn named_whole(waiting: &[Site], sites: &[Site]) -> bool {
    let waits: Vec<String> = waiting.iter().map(ToString::to_string).collect();
    let named = crate::asking::sites_named(&waits)
        .map(|(named, _)| named)
        .unwrap_or_default();
    sites.iter().all(|site| named.contains(&site.to_string()))
}

/// What an Allow of a site its ask did not name whole is refused with (#2364).
const SITES_NOT_NAMED: &str = "Nothing was allowed: the ask did not name each of those sites \
                               whole. Answer the ask as it is shown now.";

/// What an answer to sites that wait no more is refused with (#2018).
const SITES_WAIT_NO_MORE: &str = "Those sites do not wait on you for that chat any more.";

/// Each of `sites`, browser sites allowed for one chat alone, that policy leaves in force
/// (#2364): one a policy came to hold since is kept and reaches nothing.
pub fn chat_sites_in_force<'a>(
    sites: &'a [Site],
    locks: &'a purlis_core::sandbox::policy::Locks,
) -> impl Iterator<Item = &'a Site> + 'a {
    sites.iter().filter(move |site| {
        purlis_core::sandbox::local::browser_site_held_at(
            site,
            purlis_core::sandbox::grant::Level::Chat,
            locks,
        )
        .is_none()
    })
}

/// `reach`, reaching the open web too with `open_web`, and listing `sites`, the browser sites
/// allowed for its chat alone (#2018) that policy leaves in force.
fn with_sites(
    reach: BrowserReach,
    open_web: bool,
    sites: &[Site],
    locks: &purlis_core::sandbox::policy::Locks,
) -> BrowserReach {
    let mut reach = BrowserReach { open_web, ..reach };
    reach
        .listed
        .extend(chat_sites_in_force(sites, locks).map(|site| Listed {
            host: site.to_string(),
            layer: Layer::Chat,
        }));
    reach
}

/// The sites a browser said it asked about (#2018), each as the session host wrote it. One that
/// does not read as a site is a fault of the host's, said in the log and asked about as nothing.
fn sites_of(asked: &[String]) -> Vec<Site> {
    asked
        .iter()
        .filter_map(|written| match Site::parse(written) {
            Ok(site) => Some(site),
            Err(why) => {
                tracing::warn!("purlis: a browser asked about what is no site: {why}");
                None
            }
        })
        .collect()
}

/// What a chat is told of its browsers `names`, stopped for going unused for `after` (#2023).
fn idled_said(names: &[String], minutes: u64) -> String {
    let (which, held) = match names {
        [one] => (format!("browser {one} was"), "what it held is"),
        many => (
            format!("browsers {} were", many.join(", ")),
            "what they held is",
        ),
    };
    format!(
        "{which} stopped after {} unused ({} sets how long), and {held} gone; a browser call \
         that names no browser opens a new one",
        purlis_core::counts::counted(minutes, "minute", "minutes"),
        purlis_core::browserlimits::SETTING,
    )
}

/// The limits in force, read at each call (#2023).
pub type LimitsNow = Arc<dyn Fn() -> purlis_core::browserlimits::Limits + Send + Sync>;

/// How often each project looks for its chats' browsers gone unused (#2023).
pub const IDLE_LOOKED_AT_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// **Stops project `held`'s chats' browsers gone unused** (#2023), at `now`: an Ask about one
/// ends with it, with an Update, and the window's rows move.
pub fn idle_swept(held: &crate::planes::Held, now: u64) {
    let waiting: Vec<String> = held
        .needs_you()
        .waiting()
        .into_iter()
        .map(|one| one.program)
        .collect();
    let listing = held.browsers().listing();
    let stopped = held.browsers().idle(held.browser_driver(), now, &waiting);
    held.browsers_listed_since(listing);
    if !stopped.is_empty() {
        tracing::info!(
            "purlis: {} chats' browsers stopped after going unused",
            stopped.len()
        );
        crate::browserneedsyou::settled(held);
    }
}

/// **The person took browser `program` over (`driving` true) or handed it back (false)**
/// (#2023), in whichever project its chat is: while they drive it, it never stops for going
/// unused, and either is a use of it. Their input is not counted one by one: a browser they
/// drive is never stopped this way.
pub fn person_drove(projects: &[Arc<crate::planes::Held>], program: &str, driving: bool) {
    let now = crate::sandboxing::now_secs();
    for held in projects {
        held.browsers().driven(program, driving, now);
    }
}

fn no_such(name: &str) -> String {
    format!("this chat has no browser {name}: leave `browser` out to use the one it used last")
}

#[cfg(test)]
mod tests;

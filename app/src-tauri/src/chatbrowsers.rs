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
//!   chat's start armed, or, for a chat started without the sandbox, the login its persona
//!   declares and this machine allowed, read at the call (#2560,
//!   `purlis_core::secrets::brokered::login_for`), and hands them to the
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
//!   started reaches it at its next act.
//! - **A chat started without the sandbox gets a browser like a sandboxed one** (#2242): with
//!   no record of its sandbox, its browser lists what a sandboxed chat of its persona would list
//!   here, read at each call, never the floor's reach (D-2011-10); its sites ask as any chat's
//!   do, and its posture Ask says what each answer gives it.
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
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach, KeptLogin, LogIn};
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
    /// The browser engine cannot make its sandbox on this machine (#2835).
    NoSandbox,
    /// The person cancelled the engine's download the start waited on (#2207).
    Cancelled,
    /// It failed, in the host's words.
    Failed(String),
}

/// **Where a chat's browsers run**: the window's session host (`crate::browsers`), behind a
/// seam so what is decided here is tested without one.
pub trait Driver: Send + Sync + 'static {
    /// Starts a browser in the project at `root` reaching `reach`, and answers its program.
    /// Opened from the kept login `kept` where one is named (#2021).
    fn start(
        &self,
        root: &Path,
        reach: BrowserReach,
        kept: Option<KeptLogin>,
    ) -> Result<String, NotStarted>;
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
    /// The chat was started without the sandbox (#2242): its browser lists what a sandboxed
    /// chat of its persona would, and its Ask says so.
    pub unsandboxed: bool,
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
    /// It was started without the sandbox (#2242), as the app recorded it at its last call.
    unsandboxed: bool,
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
    /// Its browsers the person pinned, by program (#2022): they outlive it.
    pinned: Vec<String>,
    /// The browsers the person handed it, by the names it knows them by, until it is told
    /// (#2022).
    handed: Vec<String>,
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
            unsandboxed: false,
            rests_on: None,
            sites_asked: Vec::new(),
            sites_since: 0,
            sites: Vec::new(),
            used: HashMap::new(),
            idled: Vec::new(),
            pinned: Vec::new(),
            handed: Vec::new(),
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
            self.pinned.retain(|one| *one != program);
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
    /// The browsers purlis performs a login in now, by program, and how many logins it
    /// performs in each (#2300): two at once in one browser each hold the mark until it ends.
    logging_in: Mutex<HashMap<String, usize>>,
    /// The pinned browsers whose chat has closed (#2022), each listed by what its chat was
    /// called: the workspace's, until the person stops it or hands it to a chat, Stop all, or a
    /// quit. Never counted toward the project's chats' browsers: they are the person's now.
    kept: Mutex<Vec<ChatBrowser>>,
    /// Where each browser's page was at its last act, by program (#2022): what a quit keeps for
    /// the next start. Addresses only.
    addresses: Mutex<HashMap<String, String>>,
    /// The addresses the last quit kept, once read at this start (#2022), until the person puts
    /// them away.
    at_quit: Mutex<Option<Vec<String>>>,
    /// The browsers signed in with a login purlis performed, by program, and with which (#2022):
    /// a hand-over goes only to a chat that may log in with it now, and no quit keeps where
    /// they were.
    logins_held: Mutex<HashMap<String, Vec<HeldLogin>>>,
    /// The browsers the person took over, by program (#2022): what they typed may be in a page,
    /// so no quit keeps where they were.
    person_typed: Mutex<Vec<String>>,
    /// What each browser was last given to reach, by program (#2022): what a hand-over that
    /// fails gives back.
    reaches: Mutex<HashMap<String, BrowserReach>>,
    /// The sites each browser the person opened asked about (#2176), by its program, until
    /// they answer or its next address: what their Allow may name.
    theirs: Mutex<HashMap<String, Vec<Site>>>,
}

/// **A login a browser holds** (#2022): the persona whose login purlis performed in it, or
/// whose kept store it was opened from (#2021), the login's name, and its identity: the
/// persona, the project and the login as declared ([`purlis_core::browserlogins::identity_of`]).
/// A browser holding one goes only to a chat that may log in with that same login now, the
/// identity compared whole (#2699; spec #2008: a kept login needs the Allow of the login that
/// made it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldLogin {
    pub persona: Option<String>,
    pub login: String,
    pub identity: String,
}

impl HeldLogin {
    /// **Whether a chat running as `persona`, whose own login of this name is allowed now as
    /// `allowed` (its identity, or why not), may be handed a browser holding this login**
    /// (#2699): the same persona, and the same login as declared, compared whole.
    pub fn may_go_to(
        &self,
        persona: Option<&str>,
        allowed: Result<String, String>,
    ) -> Result<(), String> {
        if persona != self.persona.as_deref() {
            return Err(format!(
                "{} is {}'s login, and that chat runs as {}",
                self.login,
                self.persona.as_deref().unwrap_or("no persona"),
                persona.unwrap_or("no persona")
            ));
        }
        // Why it is not allowed may name the login's vault or declaration: the person is told
        // only that it is not (D-2699-3).
        let Ok(allowed) = allowed else {
            return Err(format!("that chat may not log in with {} now", self.login));
        };
        if allowed != self.identity {
            return Err(format!(
                "that chat's {} is declared otherwise than the one this browser signed in with",
                self.login
            ));
        }
        Ok(())
    }
}

/// **What a hand-over is** (#2022): browser `program`, to chat `session`, at `now`, the chat
/// reaching `reach` (its sandbox's record and the browser sites allowed here).
pub struct HandOver<'a> {
    pub program: &'a str,
    pub session: u32,
    pub now: u64,
    pub reach: BrowserReach,
}

impl Default for ChatBrowsers {
    fn default() -> Self {
        ChatBrowsers {
            limits: Arc::new(|| purlis_core::browserlimits::Limits::for_memory(None)),
            person_drives: Mutex::default(),
            logging_in: Mutex::default(),
            kept: Mutex::default(),
            addresses: Mutex::default(),
            at_quit: Mutex::default(),
            logins_held: Mutex::default(),
            person_typed: Mutex::default(),
            reaches: Mutex::default(),
            chats: Mutex::default(),
            ending: Arc::new(Ending::after(RETRY_FIRST, RETRY_MOST)),
            listing: Arc::default(),
            told: None,
            theirs: Mutex::default(),
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
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
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
    /// The person pinned it (#2022): it outlives its chat.
    pub pinned: bool,
    /// Where its chat has closed, what that chat was called: it is pinned, no chat uses it, and
    /// it runs until the person stops it, Stop all, or the app quits (#2022).
    pub was: Option<String>,
    /// The login purlis performed in it, where it holds one (#2022).
    pub login: Option<String>,
    /// Who drives it, where the agent does not (#2300): the person, who took it over, or
    /// purlis, while it performs a login.
    pub driver: Option<BrowserDriver>,
}

/// **Who drives a browser, where its chat's agent does not** (#2300), as its row and thumbnail
/// mark it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BrowserDriver {
    /// The person took it over.
    Person,
    /// purlis performs a login in it (#2016).
    Purlis,
}

/// **Why the window ends a chat's browser** (#2745), for the line each end leaves in its log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndWhy {
    /// The chat closed it (`browser_close`).
    ClosedByItsChat,
    /// Its chat ended, and it was not pinned.
    ChatEnded,
    /// It went unused this many minutes.
    Unused { minutes: u64 },
    /// Its chat's reach was narrowed, or a browser site it reached was removed.
    ReachNarrowed,
    /// The person stopped every browser.
    StopAll,
    /// It could not be given the chat it was handed to's reach.
    NotHandedOver,
}

impl std::fmt::Display for EndWhy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndWhy::ClosedByItsChat => write!(f, "its chat closed it"),
            EndWhy::ChatEnded => write!(f, "its chat ended"),
            EndWhy::Unused { minutes } => write!(f, "it went unused for {minutes} minutes"),
            EndWhy::ReachNarrowed => write!(f, "what its chat may reach was narrowed"),
            EndWhy::StopAll => write!(f, "the person stopped every browser"),
            EndWhy::NotHandedOver => {
                write!(
                    f,
                    "it could not be given the reach of the chat it was handed to"
                )
            }
        }
    }
}

/// The window's line for ending `browser` because of `why` (#2745).
pub fn ending_said(browser: &ChatBrowser, why: EndWhy) -> String {
    format!(
        "purlis: chat {}'s browser {} ({}) is ended by the window: {why}",
        browser.session, browser.name, browser.program
    )
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
        let moved = {
            let mut driving = self
                .person_drives
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let was = driving.iter().any(|one| one == program);
            driving.retain(|one| one != program);
            if by_the_person {
                driving.push(program.to_owned());
            }
            was != by_the_person
        };
        // Its row and thumbnail say who drives it (#2300).
        if moved {
            self.relisted();
        }
        if by_the_person {
            let mut typed = self
                .person_typed
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !typed.iter().any(|one| one == program) {
                typed.push(program.to_owned());
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
                            && !chat.pinned.contains(program)
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
                            ..ChatBrowser::default()
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
            .map(|(session, browsers)| {
                (
                    session,
                    self.end_now(driver, &browsers, EndWhy::Unused { minutes }),
                )
            })
            .collect()
    }

    /// **Where each open browser's page is** (#2022), as its last act answered, in the
    /// listing's order: the chats' browsers, then those pinned past their chat. One that has
    /// answered no act yet, or is not ended yet, is left out.
    pub fn open_addresses(&self) -> Vec<String> {
        let addresses = self
            .addresses
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let typed = self
            .person_typed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        // None of a browser that holds a login or what the person typed.
        self.listed()
            .into_iter()
            .filter(|one| !one.unended && one.login.is_none() && !typed.contains(&one.program))
            .filter_map(|one| addresses.get(&one.program).cloned())
            .collect()
    }

    /// **The addresses the last quit kept for the project at `root`** (#2022): read once, the
    /// first time asked, and the file taken away, so they are offered at this start alone; then
    /// kept here, so a window drawn again offers them still, until the person puts them away.
    pub fn at_quit(&self, root: &Path) -> Vec<String> {
        self.at_quit
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| purlis_core::browsersatquit::take(root))
            .clone()
    }

    /// The person put the addresses the last quit kept away (#2022).
    pub fn put_away_at_quit(&self) {
        *self.at_quit.lock().unwrap_or_else(PoisonError::into_inner) = Some(Vec::new());
    }

    /// **Forgets what was recorded of each browser that has ended** (#2022): its login, the
    /// person's take-over, its address and its reach. Kept only for a browser a chat uses, one
    /// pinned past its chat, and one purlis is still ending.
    fn prune_records(&self) {
        let mut alive: Vec<String> = self
            .lock()
            .values()
            .flat_map(|chat| chat.browsers.iter().map(|(_, program)| program.clone()))
            .collect();
        alive.extend(self.kept().iter().map(|one| one.program.clone()));
        alive.extend(
            self.endings()
                .browsers
                .iter()
                .map(|one| one.program.clone()),
        );
        let keep = |program: &String| alive.contains(program);
        self.logins_held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|program, _| keep(program));
        self.person_typed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|program| keep(program));
        self.addresses
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|program, _| keep(program));
        self.reaches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|program, _| keep(program));
    }

    /// **`program` holds `login` from now** (#2699), beside every login it held before: a later
    /// login never takes an earlier one's place, and the same one is kept once.
    fn signed_in_with(&self, program: &str, login: HeldLogin) {
        let mut held = self
            .logins_held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let logins = held.entry(program.to_owned()).or_default();
        if !logins.contains(&login) {
            logins.push(login);
        }
    }

    /// Whether a login, the person's take-over and an address are recorded of `program`.
    #[cfg(test)]
    pub fn records_of(&self, program: &str) -> (bool, bool, bool) {
        (
            self.logins_held
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains_key(program),
            self.person_typed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|one| one == program),
            self.addresses
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains_key(program),
        )
    }

    fn kept(&self) -> MutexGuard<'_, Vec<ChatBrowser>> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **The person pins chat `session`'s browser `program`, or unpins it** (#2022): pinned, it
    /// outlives the chat, and runs until the person stops it, Stop all, or a quit; it never stops
    /// for going unused. Refused where the chat has no such browser.
    pub fn pin(&self, session: u32, program: &str, pinned: bool) -> Result<(), String> {
        let mut chats = self.lock();
        let Some(chat) = chats.get_mut(&session).filter(|chat| {
            chat.browsers.iter().any(|(_, one)| one == program)
                && !chat.ended.iter().any(|one| one == program)
        }) else {
            return Err(NO_LONGER_THE_CHATS.to_owned());
        };
        let was = chat.pinned.iter().any(|one| one == program);
        if was == pinned {
            return Ok(());
        }
        chat.pinned.retain(|one| one != program);
        if pinned {
            chat.pinned.push(program.to_owned());
        }
        drop(chats);
        self.relisted();
        Ok(())
    }

    /// Whether `program` is a pinned browser whose chat has closed (#2022): no chat uses it, and
    /// closing the last tab that shows it leaves it running.
    pub fn is_kept(&self, program: &str) -> bool {
        self.kept().iter().any(|one| one.program == program)
    }

    /// **The person hands browser `program` to chat `session`** (#2022), at `now`: a pinned one
    /// whose chat closed, or one the person opened with no chat. The chat's browser tools use it
    /// from its next call, which says the person handed it over; it ends with the chat unless
    /// it is pinned. It is given the chat's reach at once, with a read of its pages, and one that
    /// cannot take it is not handed over. Refused for a browser another chat uses or purlis is
    /// still ending, past the project's limit, and for one signed in with a login unless
    /// `may_log_in` says the chat may log in with that login now: it then stays with no chat.
    /// Answers what the chat calls it.
    pub fn hand_over(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        root: &Path,
        over: HandOver<'_>,
        may_log_in: &dyn Fn(&HeldLogin) -> Result<(), String>,
    ) -> Result<String, String> {
        let HandOver {
            program,
            session,
            now,
            reach,
        } = over;
        let Some(driver) = driver else {
            return Err(NO_HOST.to_owned());
        };
        if self
            .endings()
            .browsers
            .iter()
            .any(|one| one.program == program)
        {
            return Err(
                "purlis is still ending that browser, so it is not handed over.".to_owned(),
            );
        }
        // A browser signed in with logins goes only to a chat that may log in with every one
        // of them now (#2699): each it was ever signed in with may still hold its session.
        let held = self
            .logins_held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(program)
            .cloned()
            .unwrap_or_default();
        for held in &held {
            if let Err(why) = may_log_in(held) {
                return Err(format!(
                    "This browser is signed in with {}, so it goes only to a chat that may log \
                     in with {} now, and it stays with no chat: {why}.",
                    held.login, held.login
                ));
            }
        }
        let mut chats = self.lock();
        let has = |chat: &Chat| chat.browsers.iter().any(|(_, one)| one == program);
        if chats.get(&session).is_some_and(has) {
            return Err("That chat has this browser already.".to_owned());
        }
        if chats.values().any(has) {
            return Err(
                "This browser is another chat's, and ends with it: only a browser no chat uses is \
                 handed over."
                    .to_owned(),
            );
        }
        let per_project = (self.limits)().per_project;
        let running =
            self.endings().browsers.len() + chats.values().map(Chat::running).sum::<usize>();
        if running >= per_project {
            return Err(purlis_core::browserlimits::project_full_told(per_project));
        }
        let kept = {
            let mut kept = self.kept();
            let at = kept.iter().position(|one| one.program == program);
            at.map(|at| kept.remove(at))
        };
        let chat = chats.entry(session).or_default();
        let name = format!("b{}", chat.next);
        chat.next += 1;
        chat.browsers.push((name.clone(), program.to_owned()));
        chat.used.insert(program.to_owned(), now);
        let was_current = chat.current.replace(name.clone());
        if kept.is_some() {
            chat.pinned.push(program.to_owned());
        }
        let open_web = matches!(chat.posture, Posture::Allowed { open_web: true });
        let reach_now = with_sites(
            reach,
            open_web,
            &chat.sites,
            &purlis_core::sandbox::policy::Locks::of(root),
        );
        drop(chats);
        // The new chat's reach, given at once, with a read of its pages: never the old chat's
        // until the new one first acts. A browser that cannot take it is not handed over.
        // What it reached before, given back if the hand-over fails: one the person opened
        // reached nothing listed.
        let before = self
            .reaches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(program)
            .cloned()
            .unwrap_or_default();
        if let Err(not) = driver.act(
            program,
            &format!("hand-over:{program}"),
            Act::Pages,
            reach_now.clone(),
        ) {
            let mut chats = self.lock();
            if let Some(chat) = chats.get_mut(&session) {
                chat.drop_browser(&name);
                chat.current = was_current;
            }
            drop(chats);
            let why = match not {
                Refused::Gone => "it has ended".to_owned(),
                Refused::Said(why) => why,
            };
            // The host may have taken the chat's reach before the read failed: it is given back
            // what it had, and a browser that cannot take that back is ended, never left with
            // the chat's reach.
            let given_back = driver
                .act(program, &format!("hand-back:{program}"), Act::Pages, before)
                .is_ok();
            if given_back {
                if let Some(kept) = kept {
                    self.kept().push(kept);
                }
                return Err(format!(
                    "purlis could not give it that chat's reach, so it was not handed over: {why}"
                ));
            }
            let ending = [kept.unwrap_or_else(|| ChatBrowser {
                session,
                name,
                program: program.to_owned(),
                ..ChatBrowser::default()
            })];
            self.letting_go(&ending);
            self.end_now(Some(driver), &ending, EndWhy::NotHandedOver);
            self.relisted();
            return Err(format!(
                "purlis could not give it that chat's reach, so it was not handed over, and could \
                 not give it back the reach it had, so it ended it: {why}"
            ));
        }
        self.reaches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(program.to_owned(), reach_now);
        if let Some(chat) = self.lock().get_mut(&session) {
            chat.handed.push(name.clone());
        }
        self.relisted();
        Ok(name)
    }

    /// **The person stopped browser `program`** from the window (#2022): a pinned one whose
    /// chat closed leaves the listing. Whether it was one.
    pub fn stopped_by_the_person(&self, program: &str) -> bool {
        let was = {
            let mut kept = self.kept();
            let before = kept.len();
            kept.retain(|one| one.program != program);
            kept.len() != before
        };
        if was {
            self.relisted();
            self.prune_records();
        }
        was
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
                        pinned: chat.pinned.contains(program),
                        ..ChatBrowser::default()
                    })
            })
            .collect();
        // And those pinned past their chat (#2022).
        listed.extend(self.kept().iter().cloned());
        // And those not ended yet, after the ones its chat uses (D-2013-16).
        listed.extend(
            self.endings()
                .browsers
                .iter()
                .filter(|one| one.unended)
                .cloned(),
        );
        // Stable within a chat: its browsers are kept oldest first.
        listed.sort_by_key(|one| (one.session, one.was.is_some(), one.unended));
        // With the login each holds (#2022).
        let held = self
            .logins_held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for one in &mut listed {
            // The last it was signed in with.
            one.login = held
                .get(&one.program)
                .and_then(|held| held.last())
                .map(|held| held.login.clone());
        }
        drop(held);
        // And who drives each, where the agent does not (#2300).
        let person = self
            .person_drives
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let purlis = self
            .logging_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for one in &mut listed {
            one.driver = if person.contains(&one.program) {
                Some(BrowserDriver::Person)
            } else if purlis.contains_key(&one.program) {
                Some(BrowserDriver::Purlis)
            } else {
                None
            };
        }
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

    /// **purlis performs a login in `program` now, or has handed it back** (#2300): its row and
    /// thumbnail say so, and the window reads the listing again at once, as a login may take a
    /// while.
    fn purlis_drives(&self, program: &str, now: bool) {
        {
            let mut logging_in = self
                .logging_in
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let logins = logging_in.entry(program.to_owned()).or_default();
            if now {
                *logins += 1;
            } else {
                *logins = logins.saturating_sub(1);
            }
            if *logins == 0 {
                logging_in.remove(program);
            }
        }
        self.relisted();
        if now && let Some(told) = &self.told {
            told();
        }
    }

    /// What the person said chat `session`'s browser may reach.
    #[cfg(test)]
    pub fn posture(&self, session: u32) -> Posture {
        self.lock()
            .get(&session)
            .map_or(Posture::Unasked, |chat| chat.posture)
    }

    /// **The person's browser `program` asked about `sites`** (#2176), new to the project, as
    /// its answer to their address named them: what waits on them for it is these alone.
    pub fn theirs_asked(&self, program: &str, sites: &[String]) {
        let sites: Vec<Site> = sites_of(sites).into_iter().take(MOST_SITES_ASKED).collect();
        let mut theirs = self.theirs.lock().unwrap_or_else(PoisonError::into_inner);
        if sites.is_empty() {
            theirs.remove(program);
        } else {
            theirs.insert(program.to_owned(), sites);
        }
    }

    /// The sites the person's browser `program` waits on them for (#2176).
    pub fn theirs_waiting(&self, program: &str) -> Vec<Site> {
        self.theirs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(program)
            .cloned()
            .unwrap_or_default()
    }

    /// The person answered `sites` of their browser `program` (#2176): they wait no more.
    fn theirs_answered(&self, program: &str, sites: &[Site]) {
        let mut theirs = self.theirs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(waiting) = theirs.get_mut(program) {
            waiting.retain(|site| !sites.contains(site));
            if waiting.is_empty() {
                theirs.remove(program);
            }
        }
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
                unsandboxed: chat.unsandboxed,
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

    /// **Who drives browser `program` now, where its agent does not** (#2300): the person,
    /// who took it over, or purlis, while it performs a login.
    pub fn driver_of(&self, program: &str) -> Option<BrowserDriver> {
        if self
            .person_drives
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|one| one == program)
        {
            return Some(BrowserDriver::Person);
        }
        self.logging_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(program)
            .then_some(BrowserDriver::Purlis)
    }

    /// The chat whose browser `program` is, and what that chat calls it (#2017).
    pub fn named(&self, program: &str) -> Option<(u32, String)> {
        self.lock().iter().find_map(|(session, chat)| {
            chat.browsers
                .iter()
                .find(|(_, one)| one == program)
                .map(|(name, _)| (*session, name.clone()))
        })
    }

    /// The chat whose browser `program` is, where a chat has it (#2293).
    pub fn chat_of(&self, program: &str) -> Option<u32> {
        self.lock()
            .iter()
            .find(|(_, chat)| chat.browsers.iter().any(|(_, one)| one == program))
            .map(|(session, _)| *session)
    }

    /// Chat `session` lets go of its browser `name`, which has ended: the next call opens a new
    /// one.
    pub fn let_go_of_ended(&self, session: u32, name: &str) {
        if let Some(chat) = self.lock().get_mut(&session) {
            chat.drop_browser(name);
            self.relisted();
        }
        self.prune_records();
    }

    /// **Browser `program`'s engine ended by itself** (#2385), as the session host said: a
    /// crash, or the host stopping it. It leaves its chat's rows at once, and the chat's next
    /// call on it says it has gone. Whether it was a chat's.
    pub fn engine_ended(&self, program: &str) -> bool {
        if self.stopped_by_the_person(program) {
            return true;
        }
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
    fn end_now(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        browsers: &[ChatBrowser],
        why: EndWhy,
    ) -> Ended {
        for browser in browsers {
            tracing::info!("{}", ending_said(browser, why));
        }
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
        self.prune_records();
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
                    chat.pinned.clear();
                    chat.browsers
                        .drain(..)
                        .map(|(name, program)| ChatBrowser {
                            session: *session,
                            name,
                            program,
                            unended: false,
                            ..ChatBrowser::default()
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let mut browsers = browsers;
            // And every pinned one past its chat (#2022): Stop all stops every browser.
            browsers.extend(self.kept().drain(..).map(|one| ChatBrowser {
                unended: false,
                ..one
            }));
            self.letting_go(&browsers);
            browsers
        };
        if !browsers.is_empty() {
            self.relisted();
        }
        self.end_now(driver, &browsers, EndWhy::StopAll);
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
                    ..ChatBrowser::default()
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
        self.end_now(driver, &programs, EndWhy::ReachNarrowed)
    }

    /// Chat `session` has closed: its browsers end, and nothing of it is kept but a browser that
    /// could not be ended yet, which is tried again until it is.
    pub fn forget(&self, driver: Option<&Arc<dyn Driver>>, session: u32, was: &str) {
        let (programs, kept) = {
            let mut chats = self.lock();
            let (kept, programs): (Vec<ChatBrowser>, Vec<ChatBrowser>) = chats
                .remove(&session)
                .map(|chat| {
                    chat.browsers
                        .into_iter()
                        .map(|(name, program)| ChatBrowser {
                            session,
                            // One whose engine ended is let go of as any is: nothing to keep.
                            pinned: chat.pinned.contains(&program)
                                && !chat.ended.contains(&program),
                            name,
                            program,
                            ..ChatBrowser::default()
                        })
                        .partition(|one| one.pinned)
                })
                .unwrap_or_default();
            // A pinned one runs on, the workspace's, listed by what its chat was called (#2022).
            let kept: Vec<ChatBrowser> = kept
                .into_iter()
                .map(|one| ChatBrowser {
                    was: Some(was.to_owned()),
                    ..one
                })
                .collect();
            self.kept().extend(kept.iter().cloned());
            self.letting_go(&programs);
            (programs, kept)
        };
        if !kept.is_empty() {
            self.relisted();
        }
        if !programs.is_empty() {
            self.relisted();
        }
        self.end_now(driver, &programs, EndWhy::ChatEnded);
    }

    /// **Answers one browser call of a chat's**, and whether it raised the chat's browser Ask
    /// (what it may reach). The sites its browser held back are kept waiting on the person
    /// ([`Self::sites_waiting`]), whether or not the chat started with the sandbox (#2242).
    ///
    /// `reach` is what the chat's browser may reach, its chat's reach and the browser sites
    /// allowed here, read as a browser starts and at each act; `unsandboxed` whether the app
    /// started the chat without the sandbox; `now` is in seconds since 1970. The chat is the one
    /// whose token the line carried. With no `driver` there is no session host, and so no
    /// browser.
    /// A `browser_login` call's login, by its name, is `logins`'s to read (#2016): the
    /// declared page and the vault's values, with the kept login it keeps, and whether a kept
    /// store of it is there to open (#2021); or why there are none. Read only once the chat may
    /// have a browser, and never under the browsers' lock.
    #[allow(clippy::too_many_arguments)]
    pub fn answer(
        &self,
        driver: Option<&Arc<dyn Driver>>,
        root: &Path,
        browse: &BrowseAsk,
        reach: &dyn Fn() -> BrowserReach,
        unsandboxed: bool,
        as_persona: &dyn Fn() -> AsPersona,
        logins: &dyn Fn(&str) -> ReadLogin,
        now: u64,
    ) -> (Answer, bool) {
        self.lock().entry(browse.chat).or_default().unsandboxed = unsandboxed;
        let persona = as_persona();
        self.follow_always(driver, browse.chat, &persona);
        let read = match &browse.asked {
            Browsing::LogIn { login }
                if self
                    .lock()
                    .get(&browse.chat)
                    .is_some_and(|chat| matches!(chat.posture, Posture::Allowed { .. }))
                    || persona.always.is_some() =>
            {
                Some(logins(login))
            }
            _ => None,
        };
        let (mut answer, raised) = self.answer_now(driver, root, browse, reach, persona, read, now);
        // What the browser held back for the person says so first (#2018).
        if let Answer::Browsed { browsed, .. } = &mut answer
            && !browsed.asked.is_empty()
        {
            let sites = sites_of(&browsed.asked);
            self.asked_about(browse.chat, &sites, now);
            let said = asked_about(&sites);
            browsed.note = Some(match browsed.note.take() {
                Some(note) => format!("{said}. {note}"),
                None => said,
            });
        }
        // The first answer that reaches a browser the person handed it says so (#2022).
        if let Answer::Browsed { browsed, .. } = &mut answer
            && let Some(chat) = self.lock().get_mut(&browse.chat)
            && !chat.handed.is_empty()
        {
            let said = handed_said(&std::mem::take(&mut chat.handed));
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
        read: Option<ReadLogin>,
        now: u64,
    ) -> (Answer, bool) {
        let no = |why: &str| Answer::No {
            why: why.to_owned(),
        };
        let persona_now = as_persona.persona.clone();
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
        let start = |chat: &mut Chat, kept: Option<KeptLogin>| -> Result<String, String> {
            let kept_as = kept.as_ref().map(|kept| HeldLogin {
                persona: Some(kept.persona.clone()),
                login: kept.login.clone(),
                identity: kept.identity.clone(),
            });
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
                    kept,
                )
                .map_err(|not| match not {
                    NotStarted::Full { most } => purlis_core::browserlimits::device_full_said(most),
                    NotStarted::NoSandbox => {
                        crate::enginesandbox::refused();
                        crate::enginesandbox::NO_BROWSER_HERE.to_owned()
                    }
                    NotStarted::Cancelled => {
                        crate::enginedownload::CANCELLED_FOR_THE_CHAT.to_owned()
                    }
                    NotStarted::Failed(why) => format!(
                        "the browser did not start: {why}. Try once more; if it fails again, \
                         go on with what needs no browser and tell the person what it said"
                    ),
                })?;
            // Opened from a kept store, it holds that login from its first moment (#2699).
            if let Some(kept) = kept_as {
                self.signed_in_with(&program, kept);
            }
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
                return match start(chat, None) {
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
                    ..ChatBrowser::default()
                }];
                self.letting_go(&closing);
                drop(chats);
                if self
                    .end_now(Some(driver), &closing, EndWhy::ClosedByItsChat)
                    .unended
                    > 0
                {
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
            Browsing::LogIn { login } => match read {
                Some(Ok(read)) => Doing::LogIn(login.clone(), read),
                Some(Err(why)) => return (no(&why), false),
                None => {
                    return (
                        no("purlis could not read this login: call browser_login again"),
                        false,
                    );
                }
            },
            // Answered before the browsers are (`crate::browserneedsyou::answered_first`).
            Browsing::NeedsYou { .. } => {
                return (
                    no("purlis could not ask the person about this browser"),
                    false,
                );
            }
        };
        // A login whose kept store is there opens in a browser of its own, from that store
        // (#2021), already held to the login's site: the chat's other browsers stay as they are.
        let named = match &act {
            Doing::LogIn(_, read) if read.opens => match start(chat, read.login.keep.clone()) {
                Ok(name) => Some(name),
                Err(why) => return (no(&why), false),
            },
            _ => named,
        };
        let (name, program) = match named {
            Some(name) => match chat.program(&name) {
                Some(program) => (name, program),
                None => return (no(&no_such(&name)), false),
            },
            None => match start(chat, None) {
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
            Doing::Act(act) => {
                // What it reaches from now, for a hand-over that fails to give back (#2022).
                self.reaches
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(program.clone(), reach_now.clone());
                driver.act(&program, &browse.call_id, act, reach_now)
            }
            // The login was read before the lock, for the browser it goes into, and is handed on
            // whole.
            Doing::LogIn(
                declared,
                LoginRead {
                    login, identity, ..
                },
            ) => {
                // Held from the moment it is tried, whatever its answer (#2022): a login that
                // timed out, or that the person finished by hand, may have signed it in.
                self.signed_in_with(
                    &program,
                    HeldLogin {
                        persona: persona_now.clone(),
                        login: declared.clone(),
                        identity,
                    },
                );
                // Its row says purlis drives it while it logs in (#2300).
                self.purlis_drives(&program, true);
                let logged_in = driver.log_in(&program, login).map(|mut browsed| {
                    browsed.note = Some(logged_in_said(&declared, browsed.note.as_deref()));
                    browsed
                });
                self.purlis_drives(&program, false);
                logged_in
            }
        };
        match acted {
            Ok(browsed) => {
                // Where its page is now, for a quit to keep (#2022).
                if let Some(page) = &browsed.page {
                    self.addresses
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(program.clone(), page.address.clone());
                }
                (
                    Answer::Browsed {
                        browser: name,
                        browsed: Box::new(browsed),
                    },
                    false,
                )
            }
            Err(Refused::Said(why)) => (no(&why), false),
            Err(Refused::Gone) => {
                self.let_go_of_ended(browse.chat, &name);
                (no(&gone_said(&name)), false)
            }
        }
    }
}

/// A browser login as read for a chat (#2016): the login with the kept login it keeps, and
/// whether a kept store of it is there to open (#2021); or why there is none.
pub type ReadLogin = Result<LoginRead, String>;

/// A browser login as read for a chat (#2016): the login, with the kept login it keeps;
/// whether a kept store of it is there to open (#2021); and its identity (#2699).
#[derive(Debug, Clone)]
pub struct LoginRead {
    pub login: LogIn,
    pub opens: bool,
    pub identity: String,
}

/// What a chat's browser call asks of the browser itself: one of its acts, or a login of
/// purlis's by the login's name (#2016).
enum Doing {
    Act(Act),
    /// A login by its name, as read.
    LogIn(String, LoginRead),
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
    let unsandboxed = held.chats().started_unsandboxed(browse.chat);
    let as_persona = || as_persona_of(held, browse.chat);
    let logins = |name: &str| login_of(held, browse.chat, name);
    let listing = held.browsers().listing();
    let (answer, raised) = held.browsers().answer(
        held.browser_driver(),
        held.root(),
        browse,
        &reach,
        unsandboxed,
        &as_persona,
        &logins,
        now,
    );
    held.browsers_listed_since(listing);
    // The sites the browser held back wait in the registry (#2018).
    if let Answer::Browsed { browsed, .. } = &answer
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
    if let Some(happened) = timeline_of(held.browsers(), browse, &answer, now_ms()) {
        keep_in_timeline(held, browse.chat, happened);
    }
    answer
}

/// What a timeline's moment is dated by: now, in milliseconds since 1970.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// **The moment of chat `browse.chat`'s timeline its call's `answer` makes** (#2017): an act of
/// the agent's, with the screenshot it was answered unless the person or purlis drives that
/// browser now, or the login purlis performed. None for a call no browser answered.
pub fn timeline_of(
    browsers: &ChatBrowsers,
    browse: &BrowseAsk,
    answer: &Answer,
    at: u64,
) -> Option<purlis_core::browsertimeline::Happened> {
    use purlis_core::browsertimeline::{self as timeline, By};
    let Answer::Browsed { browser, browsed } = answer else {
        return None;
    };
    match &browse.asked {
        Browsing::Act(act) => {
            let driven_by = browsers
                .which(browse.chat, Some(browser))
                .and_then(|(_, program)| browsers.driver_of(&program))
                .map(|driver| match driver {
                    BrowserDriver::Person => By::Person,
                    BrowserDriver::Purlis => By::Purlis,
                });
            timeline::acted(browser, act, browsed, at, driven_by)
        }
        Browsing::LogIn { login } => Some(timeline::signed_in(browser, login, browsed, at)),
        Browsing::New | Browsing::Close | Browsing::NeedsYou { .. } => None,
    }
}

/// Keeps `happened` in chat `session`'s browser timeline (#2017), by the chat's id, and tells
/// the window. A chat with no id keeps none; a write that fails is warned of, and never holds
/// up the chat's call.
fn keep_in_timeline(
    held: &crate::planes::Held,
    session: u32,
    happened: purlis_core::browsertimeline::Happened,
) {
    let Some(chat) = held.chats().chat_at(session).and_then(|chat| chat.id) else {
        return;
    };
    match purlis_core::browsertimeline::record(held.root(), &chat, happened) {
        Ok(()) => held.browser_timeline_moved(&chat),
        Err(why) => tracing::warn!("purlis: a browser moment was not kept in its timeline: {why}"),
    }
}

/// **Chat `chat` closed** (#2017): its browser timeline goes, with its screenshots.
pub fn timeline_closed(held: &crate::planes::Held, chat: &str) {
    match purlis_core::browsertimeline::remove(held.root(), chat) {
        Ok(()) => held.browser_timeline_moved(chat),
        Err(why) => tracing::warn!("purlis: a closed chat's browser timeline stays on disk: {why}"),
    }
}

/// Told each chat whose browser timeline moved, by its project and its id (#2017).
pub type TimelineTeller = Arc<dyn Fn(crate::planes::PlaneId, String) + Send + Sync + 'static>;

/// The event a chat's browser timeline is said to have moved on (#2017).
pub const TIMELINE_MOVED: &str = "browser-timeline-moved";

/// **A chat whose browser timeline moved** (#2017): a tab of it reads it again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TimelineMoved {
    pub plane: crate::planes::PlaneId,
    /// The chat's id.
    pub chat: String,
}

/// One moment of a chat's browser timeline, as the window draws it (#2017).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TimelineMoment {
    /// When, in seconds since 1970.
    pub at: u32,
    /// Which of the chat's browsers (`b1`).
    pub browser: String,
    /// Who drove it: `agent`, `person` or `purlis`.
    pub by: String,
    /// What was done, in words that hold nothing typed.
    pub did: String,
    /// The page it left: its scheme, host, port and path.
    pub page: Option<String>,
    /// Its screenshot's number, to read with [`chat_browser_timeline_shot`].
    pub shot: Option<u32>,
    /// Why a screenshot it had is not kept: `bound`, `too_large` or `driven`.
    pub unkept: Option<String>,
}

impl From<purlis_core::browsertimeline::Moment> for TimelineMoment {
    fn from(moment: purlis_core::browsertimeline::Moment) -> Self {
        let word = |value: serde_json::Result<serde_json::Value>| {
            value
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        TimelineMoment {
            at: u32::try_from(moment.at / 1000).unwrap_or(u32::MAX),
            browser: moment.browser,
            by: word(serde_json::to_value(moment.by)),
            did: moment.did,
            page: moment.page,
            shot: moment.shot.and_then(|shot| u32::try_from(shot).ok()),
            unkept: moment.unkept.map(|why| word(serde_json::to_value(why))),
        }
    }
}

/// **Chat `chat`'s browser timeline** (#2017), by its id, oldest first: what its agent did in
/// its browsers, and where the person or purlis drove. The window's alone (`ui::WINDOW_ONLY`):
/// it holds screenshots of the chat's pages.
#[tauri::command]
#[specta::specta]
pub async fn chat_browser_timeline(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    chat: String,
) -> Result<Vec<TimelineMoment>, String> {
    let held = planes.held(&plane)?;
    let moments = purlis_core::browsertimeline::read(held.root(), &chat);
    Ok(moments.into_iter().map(TimelineMoment::from).collect())
}

/// **Screenshot `shot` of chat `chat`'s browser timeline** (#2017), PNG in base64: only one a
/// moment of the timeline lists. The window's alone (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn chat_browser_timeline_shot(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    chat: String,
    shot: u32,
) -> Result<String, String> {
    use base64::Engine as _;
    let held = planes.held(&plane)?;
    match purlis_core::browsertimeline::shot_of(held.root(), &chat, u64::from(shot)) {
        Some(png) => Ok(base64::engine::general_purpose::STANDARD.encode(png)),
        None => Err("That screenshot is no longer kept.".to_owned()),
    }
}

/// **Chat `session`'s browser login `name`** (#2016): read for the chat as the app records it,
/// never as the line says, exactly as a declared tool's run is (`purlis_core::secrets::brokered`).
fn login_of(held: &crate::planes::Held, session: u32, name: &str) -> ReadLogin {
    // The persona the chat was started as, as every brokered read of a vault takes it, and
    // whether it runs with no sandbox at all, from the same run of it (#2560).
    let (asker, unconfined) = login_asker_of(held, session, name).map_err(|why| {
        why.unwrap_or_else(|| format!("chat {session} is not one this app has open"))
    })?;
    let persona = asker.persona.clone().unwrap_or_default();
    let (mut login, identity) =
        purlis_core::secrets::brokered::login_and_identity_for(&asker, name, unconfined)?;
    // Kept under the chat's profile on this device, bound to the login as declared and allowed
    // (#2021), and opened from it where a store of it is there.
    let kept = held.chats().profile_of(session).map(|profile| KeptLogin {
        profile,
        persona,
        login: name.to_owned(),
        identity: identity.clone(),
        address: login.address.clone(),
    });
    let store = kept.as_ref().and_then(|kept| {
        purlis_core::browserlogins::Store::new(
            &kept.profile,
            &kept.persona,
            &kept.login,
            &kept.identity,
        )
        .ok()
    });
    let opens = match (&store, purlis_core::datahome::root()) {
        (Some(store), Some(data)) => store.exists(&data),
        _ => false,
    };
    // A login whose names cannot name a store keeps nothing.
    login.keep = kept.filter(|_| store.is_some());
    Ok(LoginRead {
        login,
        opens,
        identity,
    })
}

/// **Who asks for chat `session`'s browser login `name`, and whether it runs with no sandbox at
/// all** ([`crate::chats::Chats::login_asker`]). A chat its host composed in a sandbox there is
/// refused in words (#2692): its tools are what the runner armed, which this window holds no
/// record of, and a restart would not give it one. `Err(None)` for a chat this app does not
/// have open.
fn login_asker_of(
    held: &crate::planes::Held,
    session: u32,
    name: &str,
) -> Result<(purlis_core::secrets::brokered::Asker, bool), Option<String>> {
    if held.chats().sandboxed_by_its_host(session) {
        return Err(Some(in_its_hosts_sandbox(name)));
    }
    held.chats().login_asker(held.root(), session).ok_or(None)
}

/// What a browser login of a chat its runner composed in its sandbox is refused with (#2692).
pub(crate) fn in_its_hosts_sandbox(name: &str) -> String {
    format!(
        "this chat runs in its runner's sandbox, whose tools the runner armed there, so purlis \
         performs no browser login {name} for it from this window, and nothing was filled"
    )
}

/// The persona whose grants chat `session` runs with, as its start read them, from the app's
/// own record of it and never from what the chat says ([`crate::chats::Chats::persona_of`],
/// #2691): what its browser reaches and its Asks are about. None for a chat this window does not
/// have open, or one that runs with no persona's grants.
fn persona_of(held: &crate::planes::Held, session: u32) -> Option<String> {
    held.chats()
        .persona_of(session)
        .and_then(|persona| persona.grants)
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
/// listed, D-2011-10), the browser sites you allowed for every chat here (#2018) that policy
/// leaves in force, and the project's browser sites for everyone in it that the person here was
/// shown (#2443). A host allowed for the chat's commands is a site its browser
/// reaches (D-2018-7); a browser site is never one its commands reach. **A chat started without
/// the sandbox** has no such record (#2242): its browser lists what a sandboxed chat of its
/// persona would list here, read at each call ([`listed_reach`]), and nothing more. A
/// sandboxed chat reattached under a sandbox whose compile was not recorded (D-2006-11) lists
/// nothing. Its own
/// sites, for this chat alone, are added by [`ChatBrowsers::answer`].
fn reach_of(held: &crate::planes::Held, session: u32) -> BrowserReach {
    let Some(confines) = held.chats().confines_now(session) else {
        // Held by a sandbox whose compile was not recorded (D-2006-11): nothing is listed.
        if !held.chats().started_unsandboxed(session) {
            return BrowserReach::default();
        }
        return listed_reach(held, persona_of(held, session).as_deref());
    };
    with_browser_sites(
        held.root(),
        purlis_core::browsertools::browser_reach(&confines.decides(), false),
    )
}

/// **What a browser the person opens from the window reaches** (#2176): it has no chat, so no
/// chat's sandbox was compiled for it. It lists what the project's sandbox settings list for a
/// chat on no persona, read now (`purlis_core::sandbox::browser_listing`), never the floor's
/// reach, and the browser sites allowed here, as a chat's browser does. A site new to the
/// project asks the person as an address the agent writes would (#2018).
pub fn person_reach(held: &crate::planes::Held) -> BrowserReach {
    listed_reach(held, None)
}

/// **What the project lists for a browser of a chat as `persona`, or as none**, where no chat's
/// sandbox was compiled for it (#2176, #2242): the project's sandbox settings compiled now as a
/// chat's start compiles them, never the floor's reach, and the browser sites allowed here. A
/// project whose file cannot be read lists nothing.
fn listed_reach(held: &crate::planes::Held, persona: Option<&str>) -> BrowserReach {
    let root = held.root();
    let listed = purlis_core::sandbox::browser_listing::listed_for(
        root,
        &purlis_core::sandbox::Machine::this(),
        persona,
    )
    .map(|reach| purlis_core::browsertools::browser_reach(&reach, false))
    .unwrap_or_default();
    with_browser_sites(root, listed)
}

/// `reach`, listing the browser sites you allowed for every chat here (#2018) that policy
/// leaves in force.
fn with_browser_sites(root: &Path, mut reach: BrowserReach) -> BrowserReach {
    use purlis_core::sandbox;
    let locks = sandbox::policy::Locks::of(root);
    reach.listed.extend(
        sandbox::local::browser_sites_in_force(root, &locks)
            .iter()
            .map(|site| Listed {
                host: site.to_string(),
                layer: Layer::You,
            }),
    );
    with_project_sites(reach, &sandbox::browser_sites::in_force(root, &locks))
}

/// `reach`, listing `sites`, the project's browser sites for everyone in it in force here
/// (#2443), each at the project's layer.
fn with_project_sites(
    mut reach: BrowserReach,
    sites: &[purlis_core::sandbox::hosts::Host],
) -> BrowserReach {
    reach.listed.extend(sites.iter().map(|site| Listed {
        host: site.to_string(),
        layer: Layer::Project,
    }));
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
/// browser sites at `level`: this chat, every chat of this project on this machine, or everyone
/// in the project, committed (#2443). A grant for the browser alone (ruled 2026-10-17): the
/// chat's own commands still need their own Allow for each host. Its browser reaches them from
/// its next act; the chat is told on its next turn.
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
    use purlis_core::sandbox::grant::Level;
    let sites = sites_sent(sites)?;
    if !matches!(level, Level::Chat | Level::You | Level::Project) {
        return Err(
            "purlis allows a browser site for this chat, for every chat of this project on this \
             machine, or for everyone in the project."
                .to_owned(),
        );
    }
    sites_grantable(held.root(), &sites, level)?;
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
    sites_kept(
        held.root(),
        &sites,
        level,
        audit,
        at,
        Some(session),
        held.chats().recorded_chat(session).map(|chat| chat.name),
    )?;
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

/// Whether `sites` may be kept as browser sites at `level` in the project at `root`: refused
/// where policy removes that scope or never allows a site's host, and for everyone in the
/// project where the project's sandbox is not on (#2443).
fn sites_grantable(
    root: &Path,
    sites: &[Site],
    level: purlis_core::sandbox::grant::Level,
) -> Result<(), String> {
    use purlis_core::sandbox;
    let locks = sandbox::policy::Locks::of(root);
    // A committed browser site reaches a browser only where the project's sandbox is on (#2443).
    if level == sandbox::grant::Level::Project && !everyone_counts(root, &locks) {
        return Err(EVERYONE_COUNTS_NOT.to_owned());
    }
    if let Some(why) = locks.scope_removed(level) {
        return Err(why);
    }
    for site in sites {
        let host = sandbox::hosts::Host::parse(&site.to_string())?;
        if let Some(why) = locks.refuses_grant(&sandbox::grant::What::Host(host), level) {
            return Err(why);
        }
    }
    Ok(())
}

/// **Audits, then keeps, `sites` as browser sites at `level`** in the project at `root`, for
/// chat `session` where a chat's Ask allowed them, or for a browser the person opened: committed
/// for everyone in the project (#2443), or kept on this machine at your level with when and for
/// which chat (`chat`); a chat's own level is its browsers' alone and kept by
/// [`ChatBrowsers::sites_answered`].
fn sites_kept(
    root: &Path,
    sites: &[Site],
    level: purlis_core::sandbox::grant::Level,
    audit: crate::sandboxing::Audit<'_>,
    at: u64,
    session: Option<u32>,
    chat: Option<String>,
) -> Result<(), String> {
    use purlis_core::sandbox::{self, grant::Level};
    for site in sites {
        audit(
            session,
            &sandbox::grant::Audited {
                granted: true,
                what: sandbox::local::BROWSER_SITE,
                target: &site.to_string(),
                level,
            },
        )?;
    }
    if level == Level::Project {
        // Committed for everyone in the project (#2443): the browsers' alone, never a host of
        // the chats' own commands, and seen here as this window wrote it.
        for site in sites {
            let host = sandbox::hosts::Host::parse(&site.to_string())?;
            sandbox::browser_sites::grant(root, &host).map_err(|why| {
                format!("purlis could not keep {site}, so nothing was allowed: {why}")
            })?;
        }
    }
    if level == Level::You {
        for site in sites {
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
                    chat: chat.clone(),
                },
            ) {
                // The Allow stands, and is audited; only the Granted list's "when" is lost.
                tracing::warn!("purlis: a browser site was kept without when ({why})");
            }
        }
    }
    Ok(())
}

/// **Allow on the sites a browser the person opened asked about** (#2176): `sites`, the sites
/// its view showed, are browser sites at `level`: every browser of this project on this
/// machine, or everyone in the project, committed (#2443). A grant for browsers alone, as a
/// chat's site Allow is: no chat's own commands reach them. Refused whole unless each waits on
/// the person for that browser now.
#[tauri::command]
#[specta::specta]
pub async fn allow_person_browser_sites(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    program: String,
    sites: Vec<String>,
    level: crate::sandboxing::GrantLevel,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window("allowing a browser's sites", move || {
        person_sites_allowed(
            &held,
            &program,
            &sites,
            level.into(),
            &held.audit_then_record(network.as_ref()),
            crate::sandboxing::now_secs(),
        )
    })
    .await
}

/// **The person's browser `program` held back `asked`** (#2176): the sites its ask names whole
/// wait on the person for it, and only those, with the answers it offers: Always allow in this
/// project on this machine, and Allow for everyone in this project where the project's sandbox
/// is in force here (committed browser sites count only there, #2443), each unless an
/// administrator's policy leaves its scope out; and Keep blocked.
pub fn theirs_asked(
    held: &crate::planes::Held,
    program: &str,
    asked: &[String],
) -> (Vec<String>, Vec<crate::asking::Offered>) {
    let locks = purlis_core::sandbox::policy::Locks::of(held.root());
    let (named, _, answers) = crate::asking::browser_site_answers(
        asked,
        &locks,
        false,
        everyone_counts(held.root(), &locks),
    );
    // Only what may be allowed waits: a site not named whole is offered Keep blocked alone.
    let waiting: &[String] = if answers.iter().any(|one| one.allows) {
        &named
    } else {
        &[]
    };
    held.browsers().theirs_asked(program, waiting);
    (named, answers)
}

/// What an Allow for everyone in the project is refused with where the project's sandbox is not
/// in force (#2443): it would reach no browser.
const EVERYONE_COUNTS_NOT: &str = "Nothing was allowed: a browser site for everyone in the \
                                   project reaches browsers only where the project's sandbox is \
                                   on, and it is not on here. Allow it on this machine instead.";

/// Whether a browser site for everyone in the project reaches anything here: only where the
/// project's sandbox is in force (#2443).
pub fn everyone_counts(root: &Path, locks: &purlis_core::sandbox::policy::Locks) -> bool {
    purlis_core::sandbox::Plane::read(root)
        .in_force(locks)
        .is_some()
}

/// [`allow_person_browser_sites`]'s work: judged, then audited, then kept.
pub fn person_sites_allowed(
    held: &crate::planes::Held,
    program: &str,
    sites: &[String],
    level: purlis_core::sandbox::grant::Level,
    audit: crate::sandboxing::Audit<'_>,
    at: u64,
) -> Result<(), String> {
    use purlis_core::sandbox::grant::Level;
    let sites = sites_sent(sites)?;
    if !matches!(level, Level::You | Level::Project) {
        return Err(
            "purlis allows a site for your own browser for every browser of this project on \
             this machine, or for everyone in the project."
                .to_owned(),
        );
    }
    sites_grantable(held.root(), &sites, level)?;
    let waiting = held.browsers().theirs_waiting(program);
    if sites.is_empty() || !sites.iter().all(|site| waiting.contains(site)) {
        return Err(SITES_WAIT_NO_MORE_THEIRS.to_owned());
    }
    if !named_whole(&waiting, &sites) {
        return Err(SITES_NOT_NAMED.to_owned());
    }
    sites_kept(held.root(), &sites, level, audit, at, None, None)?;
    held.browsers().theirs_answered(program, &sites);
    Ok(())
}

/// What an answer to a person's browser's sites that wait no more is refused with (#2176).
const SITES_WAIT_NO_MORE_THEIRS: &str = "Those sites do not wait on you for that browser any \
                                         more: go to the address again.";

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

/// What a chat is told of the browsers `names` the person handed it (#2022).
fn handed_said(names: &[String]) -> String {
    let which = match names {
        [one] => format!("browser {one}"),
        many => format!("browsers {}", many.join(", ")),
    };
    format!(
        "the person handed this chat {which}, on the page they left it at: the browser tools use \
         the last one handed over unless a call names another"
    )
}

/// **Keeps where project `held`'s browsers are, as purlis quits** (#2022): their addresses
/// alone, for the next start to offer once. A keep that fails is said in the log; the quit goes
/// on.
pub fn kept_at_quit(held: &crate::planes::Held) {
    let addresses = held.browsers().open_addresses();
    if let Err(why) = purlis_core::browsersatquit::keep(held.root(), &addresses) {
        tracing::warn!("purlis: the project's browsers at quit were not kept ({why})");
    }
}

/// **The addresses project `plane`'s browsers were at when purlis last quit** (#2022): offered
/// at this start, each a button, nothing loaded by itself. The window's alone
/// (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn browsers_at_quit(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<String>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the browsers at the last quit", move || {
        Ok(held.browsers().at_quit(held.root()))
    })
    .await
}

/// **The person put away the browsers of the last quit** (#2022): they are offered no more.
#[tauri::command]
#[specta::specta]
pub async fn put_away_browsers_at_quit(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<(), String> {
    planes.held(&plane)?.browsers().put_away_at_quit();
    Ok(())
}

/// What pinning a browser the chat no longer has is refused with (#2022).
const NO_LONGER_THE_CHATS: &str = "That browser is no longer this chat's: it ended, or its chat \
                                   closed.";

/// **The person pins chat `session`'s browser `program`, or unpins it** (#2022): pinned, it
/// outlives its chat. The window's alone (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn pin_chat_browser(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    program: String,
    pinned: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("pinning a chat's browser", move || {
        let listing = held.browsers().listing();
        let pinned = held.browsers().pin(session, &program, pinned);
        held.browsers_listed_since(listing);
        pinned
    })
    .await
}

/// **The person hands browser `program` to chat `session` of project `held`** (#2022): a pinned
/// one whose chat closed, or, with `opened_here`, one the person opened in this project that no
/// chat uses. Refused for a chat this window does not have open. Answers what the chat calls it.
pub fn handed(
    held: &crate::planes::Held,
    program: &str,
    session: u32,
    opened_here: bool,
) -> Result<String, String> {
    if !held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == session)
    {
        return Err("That chat is not open in this window any more.".to_owned());
    }
    if !opened_here && !held.browsers().is_kept(program) {
        return Err(
            "That browser is not one of this project's that no chat uses, so it is not handed \
             over."
                .to_owned(),
        );
    }
    // The same login, the same persona's, and allowed for that chat now: the Allow that made
    // it (spec #2008).
    // The identity compared whole (#2699): persona, project and declaration.
    let may_log_in = |login: &HeldLogin| -> Result<(), String> {
        let (asker, unconfined) = login_asker_of(held, session, &login.login).map_err(|why| {
            why.unwrap_or_else(|| "that chat is not open in this window".to_owned())
        })?;
        login.may_go_to(
            asker.persona.as_deref(),
            purlis_core::secrets::brokered::login_allowed(&asker, &login.login, unconfined),
        )
    };
    let listing = held.browsers().listing();
    let name = held.browsers().hand_over(
        held.browser_driver(),
        held.root(),
        HandOver {
            program,
            session,
            now: crate::sandboxing::now_secs(),
            reach: reach_of(held, session),
        },
        &may_log_in,
    );
    held.browsers_listed_since(listing);
    if name.is_ok() {
        tracing::info!("purlis: the person handed a browser to chat {session}");
        let handed = purlis_core::eventlog::browsing::Said::HandedOver {
            browser: program.to_owned(),
        };
        if let Err(why) = held
            .hooks()
            .record_browser(held.root(), Some(session), handed)
        {
            tracing::warn!("purlis: a hand-over was not written to the event log ({why})");
        }
    }
    name
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

/// **Writes what the window says of a browser in the event log** (#2293), under the chat whose
/// browser it is, in whichever of `projects` has it; for a browser no chat uses, with no chat,
/// in the project at `root` it was started in, and nowhere where that project is not held. A
/// write that fails is warned of, and never holds up what the person did.
pub fn say(
    projects: &[Arc<crate::planes::Held>],
    root: Option<&Path>,
    said: purlis_core::eventlog::browsing::Said,
) {
    let of_a_chat = projects.iter().find_map(|held| {
        held.browsers()
            .chat_of(said.browser())
            .map(|session| (held, Some(session)))
    });
    let started_in = || {
        projects
            .iter()
            .find(|held| root.is_some_and(|root| held.root() == root))
            .map(|held| (held, None))
    };
    // Never another project's log: one whose project is not held is written nowhere.
    let Some((held, number)) = of_a_chat.or_else(started_in) else {
        tracing::warn!(
            "purlis: {} was not written to the event log: the browser's project is not held",
            said.kind()
        );
        return;
    };
    let kind = said.kind();
    if let Err(why) = held.hooks().record_browser(held.root(), number, said) {
        tracing::warn!("purlis: {kind} was not written to the event log ({why})");
    }
}

/// **The person took browser `program` over (`driving` true) or handed it back (false)**
/// (#2023), in whichever project its chat is: while they drive it, it never stops for going
/// unused, and either is a use of it. Their input is not counted one by one: a browser they
/// drive is never stopped this way.
pub fn person_drove(projects: &[Arc<crate::planes::Held>], program: &str, driving: bool) {
    let now = crate::sandboxing::now_secs();
    for held in projects {
        let listing = held.browsers().listing();
        let was = held.browsers().driver_of(program) == Some(BrowserDriver::Person);
        held.browsers().driven(program, driving, now);
        // Its row and thumbnail say who drives it (#2300).
        held.browsers_listed_since(listing);
        // Its chat's timeline says so where it changed hands, with no screenshot (#2017).
        if was != driving
            && let Some((session, name)) = held.browsers().named(program)
        {
            let at = now_ms();
            let happened = if driving {
                purlis_core::browsertimeline::took_over(&name, at)
            } else {
                purlis_core::browsertimeline::handed_back(&name, at)
            };
            keep_in_timeline(held, session, happened);
        }
    }
}

fn no_such(name: &str) -> String {
    format!("this chat has no browser {name}: leave `browser` out to use the one it used last")
}

#[cfg(test)]
mod tests;

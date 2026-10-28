// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **System notifications, for asks only** (#1694, spec #1688, I-7; #1989, Q66).
//!
//! What the person is interrupted for is what blocks work: an **ask** (a permission prompt, a
//! dispatch grant, a sandbox host, a prompt in a harness's terminal, a chat whose turn ended
//! on them). An **update** — a task that failed, a refused commit, a report — is information,
//! and never raises a notification: the registry does not list it (`asking::only_updates`, the
//! registry's own split), so every item it lists is an ask.
//!
//! **What is asked is the asks registry's** (`asking::every_ask`), read again whenever one of
//! its sources may have moved ([`poke`]): a chat moving on the board, its hooks' asks, a
//! dispatch held, a block heard — the events the window reads its own list again on. An ask
//! is notified once, the first time it is read ([`Rules`]); it holds no store of its own past
//! the keys it has seen, so it agrees with the Inbox by construction. A chat that waits on its
//! background agents is not in the queue (#1626), so it is no ask and sends nothing. A chat's
//! own wait is keyed by the chat, so the board's edge into the queue renews it ([`moved`]).
//!
//! **A notification says who asks and what kind of ask it is** (`steward 12 › #3046 drill`,
//! "Asks your permission"), never the ask's words ([`kind_said`]). **Several chats waiting
//! together are one** (#1989): "3 chats need you", naming them (`steward 3, devops 4 and
//! builder 5`). A name is the chat's, so it is data: plain text, capped ([`plain`]).
//!
//! **A project's asks a few seconds apart share one notification** ([`TOGETHER`]): the first
//! is sent at once, and each that follows within the window of the one before rides it,
//! whichever chat asks. Grouping is per project, because a click brings forward the one window
//! holding it. A new ask past the window sends one that counts every chat still
//! waiting that the person is not looking at, the longest waiting first.
//!
//! **A chat that rides another's notification is not hidden by it**: when the window closes
//! with other chats on it, one notification more names every chat still waiting unseen
//! ([`Rules::next_due`]), so a trickle of asks names each of them.
//!
//! **Nothing is sent in the person's quiet hours** ([`QuietHours`], Settings › You ›
//! Notifications, kept in the layout file): an ask asked in them waits in the Inbox, and when
//! they end one notification names what still waits unseen, as the system's own focus modes
//! sum up what they held. Whether purlis may show notifications at all is the system's own
//! notification settings' to say.
//!
//! **Nothing is sent while the person is looking** ([`Looking`]): the window holding the
//! ask's project is on screen, focused and has that project in front, and it shows either the
//! chat itself (`Chats::looks_at`, #1486, #1489) or that project's Inbox ([`InboxOpen`], which
//! the window says). Every question unanswered reads as "not looking", so a notification is
//! sent rather than held back: one not needed costs a glance, one needed and not sent costs a
//! chat sitting unanswered.
//!
//! **A click opens the Inbox at that group.** The desktop tells the app nothing of a click but
//! that it is brought forward: the notification plugin reports a click on mobile only, and
//! shows a desktop notification and lets go of it. So the window holding the project is sent
//! [`NOTIFIED`] with the chat, and lands there the next time it comes to the front
//! (`askNotices.ts`). A click with every window hidden (the close button hides the main
//! window, ADR 0025) brings none forward by itself: macOS says it as a reopen, and the window
//! holding the project the latest notification was about is shown ([`reopened`]), where the
//! focus it gets lands it.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::asking::{AskSource, Shown};
use crate::planes::{PlaneId, Planes, Showing};

/// The event the window holding a project is sent when a notification about one of its chats
/// went out: [`Landing`].
pub const NOTIFIED: &str = "asks-notified";

/// How close together one chat's asks are to share one notification: each within this of the
/// one before rides the first.
pub const TOGETHER: Duration = Duration::from_secs(5);

/// The most characters of a chat's name, or of a title, a notification says.
pub const MOST_NAME: usize = 60;

/// The most chats a notification names; the rest it counts.
const MOST_NAMED: usize = 3;

/// How long a burst of moves is let settle before the registry is read, as the window does.
const SETTLE: Duration = Duration::from_millis(150);

/// One notification to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub plane: PlaneId,
    /// The chat a click lands on: the first of [`Self::sessions`].
    pub session: u32,
    /// Every chat it counts, the longest waiting first.
    pub sessions: Vec<u32>,
    /// Who is asking, as its chain (I-9).
    pub title: String,
    /// What kind of ask it is ([`kind_said`]), and how many more ride with it.
    pub body: String,
}

/// Where a click on a notification lands, as the window is sent it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Landing {
    pub plane: PlaneId,
    /// The first chat it counted.
    pub session: u32,
    /// Every chat it counted, in order: a window lands on the first still waiting.
    pub sessions: Vec<u32>,
}

/// One read of a project's asks, as [`Rules::heard`] takes it.
pub struct Heard<'a> {
    pub plane: &'a PlaneId,
    /// The registry's whole list, as it was derived.
    pub asks: &'a [Shown],
    /// Whether the person is looking at a chat's asks now ([`Looking`]).
    pub looking: &'a dyn Fn(u32) -> bool,
    /// Whether it is the person's quiet hours now: asked only when there is something to send.
    pub quiet: &'a dyn Fn() -> bool,
    pub now: Instant,
}

/// **What has been told, and when each chat was last**: the whole of the notifier's memory.
#[derive(Default)]
pub struct Rules {
    /// The asks of each project read last, by key, with the chat that asks and whether the
    /// ask is the chat's own wait ([`own_wait`]): an ask in here was already considered.
    known: HashMap<PlaneId, HashMap<String, (u32, bool)>>,
    /// Each project's notification still riding ([`TOGETHER`]).
    covered: HashMap<PlaneId, Covered>,
    /// The projects with asks that came in quiet hours: when they end, one notification names
    /// what still waits.
    held: HashSet<PlaneId>,
}

/// **A notification sent, and what has ridden it since.**
struct Covered {
    /// When an ask last rode it, or it was sent.
    at: Instant,
    /// The chats it named.
    told: Vec<u32>,
    /// The chats it did not name whose asks rode it: those still waiting are named in one
    /// notification more when the window closes, so a trickle of asks hides none of them.
    rode: Vec<u32>,
}

impl Rules {
    /// The notification one read of a project's asks sends, if any.
    ///
    /// - An ask new since the last read sends one, counting every chat waiting that the person
    ///   is not looking at, unless the project's asks are riding one sent within
    ///   [`TOGETHER`]. An ask the person was looking at is not sent later.
    /// - A window closed with other chats on it ([`Self::next_due`]) sends one more, while any
    ///   of them still waits unseen.
    /// - In quiet hours nothing is sent; at the first read after them, one names what still
    ///   waits unseen ([`Self::held_in_quiet_hours`]).
    pub fn heard(&mut self, heard: &Heard<'_>) -> Vec<Notice> {
        let plane = heard.plane;
        let known = self.known.entry(plane.clone()).or_default();
        let fresh: Vec<&Shown> = heard
            .asks
            .iter()
            .filter(|ask| !known.contains_key(&ask.ask))
            .collect();
        *known = heard
            .asks
            .iter()
            .map(|ask| (ask.ask.clone(), (ask.session, own_wait(ask))))
            .collect();
        let waiting = || {
            let mut chats: Vec<u32> = Vec::new();
            for ask in heard.asks {
                if !chats.contains(&ask.session) && !(heard.looking)(ask.session) {
                    chats.push(ask.session);
                }
            }
            chats
        };
        // Why a notification would be sent now: the chats it is for.
        let mut owed: Vec<u32> = Vec::new();
        match self.covered.get_mut(plane) {
            Some(covered) if heard.now.saturating_duration_since(covered.at) < TOGETHER => {
                if !fresh.is_empty() {
                    covered.at = heard.now;
                    for ask in &fresh {
                        let chat = ask.session;
                        if !covered.told.contains(&chat) && !covered.rode.contains(&chat) {
                            covered.rode.push(chat);
                        }
                    }
                }
                return Vec::new();
            }
            Some(_) => {
                if let Some(closed) = self.covered.remove(plane) {
                    owed.extend(closed.rode);
                }
            }
            None => {}
        }
        owed.extend(fresh.iter().map(|ask| ask.session));
        if self.held.contains(plane) {
            if (heard.quiet)() {
                return Vec::new();
            }
            self.held.remove(plane);
            owed.extend(heard.asks.iter().map(|ask| ask.session));
        }
        if owed.is_empty() {
            return Vec::new();
        }
        let chats = waiting();
        if !owed.iter().any(|chat| chats.contains(chat)) {
            return Vec::new();
        }
        if (heard.quiet)() {
            self.held.insert(plane.clone());
            return Vec::new();
        }
        self.covered.insert(
            plane.clone(),
            Covered {
                at: heard.now,
                told: chats.clone(),
                rode: Vec::new(),
            },
        );
        vec![notice(plane, &chats, heard.asks, &fresh)]
    }

    /// **When the notifier should read a project again by itself**: the earliest time a
    /// riding window closes with other chats on it.
    pub fn next_due(&self) -> Option<Instant> {
        self.covered
            .values()
            .filter(|covered| !covered.rode.is_empty())
            .map(|covered| covered.at + TOGETHER)
            .min()
    }

    /// The projects due a read by itself now ([`Self::next_due`]).
    fn due(&self, now: Instant) -> Vec<PlaneId> {
        self.covered
            .iter()
            .filter(|(_, covered)| !covered.rode.is_empty() && covered.at + TOGETHER <= now)
            .map(|(plane, _)| plane.clone())
            .collect()
    }

    /// **The projects with asks held back by quiet hours**, read again by the notifier while
    /// there are any, so the first read after them sends.
    pub fn held_in_quiet_hours(&self) -> Vec<PlaneId> {
        self.held.iter().cloned().collect()
    }

    /// **Chat `session` of `plane` waits on the person anew**, as the board's own edge says
    /// (`Moved::interrupts`): its own wait, a reply or a prompt in its terminal, is keyed by
    /// the chat and not by the wait, so a turn that ended on the person again between two
    /// reads would read as the same ask. It is fresh at the next read. A decision it holds,
    /// keyed by itself, is untouched.
    pub fn renew(&mut self, plane: &PlaneId, session: u32) {
        if let Some(known) = self.known.get_mut(plane) {
            known.retain(|_, (asker, own)| !(*own && *asker == session));
        }
    }

    /// A project let go of, or no longer held: nothing of it is remembered.
    pub fn forget(&mut self, plane: &PlaneId) {
        self.known.remove(plane);
        self.covered.remove(plane);
        self.held.remove(plane);
    }
}

/// **What a notification says an ask is**: its kind, never its words. A command line, a host
/// or a path is the window's to show; a notification is drawn where the window is not, on a
/// locked screen and in the system's own list of them, and kept there.
fn kind_said(ask: &Shown) -> &'static str {
    match ask.source {
        AskSource::Permission => "Asks your permission",
        AskSource::Dispatch => "Asks to hand a task on",
        AskSource::SandboxHost => "Asks to reach a host",
        AskSource::SandboxWrite => "Asks to write in a folder",
        AskSource::SandboxLocal => "Asks to reach a service on this machine",
        AskSource::WorkspaceRemoval => "Asks you to remove a workspace",
        AskSource::ProposedChange => "Proposes a change for you to apply",
        AskSource::RunnerWrite => "Asks, from a pool's runner, to change this project",
        AskSource::Terminal => "Waiting in its terminal",
        AskSource::Question => "Waiting on your reply",
        AskSource::SignIn => "Asks you to sign a profile in",
        AskSource::SandboxOnly => "Asks you to make a sandbox-only profile",
        AskSource::OldHarnessPlugins => "Old harness plugin settings, ignored now",
        AskSource::Browser => "Asks what its browser may reach",
        AskSource::BrowserNeedsYou => "Needs you in its browser",
        AskSource::BrowserSite => "Asks to let its browser reach a new site",
        AskSource::PoolLogin => "Asks to log its browser in on a pool's runner",
        AskSource::PoolTool => "Asks to send a tool its vault values for a run on a pool's runner",
    }
}

/// Whether `ask` is a chat's own wait, keyed by the chat: a reply it waits on, or a prompt in
/// its terminal.
fn own_wait(ask: &Shown) -> bool {
    matches!(ask.source, AskSource::Question | AskSource::Terminal)
}

/// The notification for `chats` waiting, the first first, of a project whose list is `asks`
/// and whose new asks are `fresh`. One chat: who asks and what kind of ask, its new asks first
/// (#1694). Several: how many, and their names.
fn notice(plane: &PlaneId, chats: &[u32], asks: &[Shown], fresh: &[&Shown]) -> Notice {
    let session = chats[0];
    let (title, body) = if let [only] = chats {
        let new: Vec<&Shown> = fresh
            .iter()
            .copied()
            .filter(|ask| ask.session == *only)
            .collect();
        let mine: Vec<&Shown> = if new.is_empty() {
            asks.iter().filter(|ask| ask.session == *only).collect()
        } else {
            new
        };
        let first = mine[0];
        let title = if first.chain.is_empty() {
            format!("chat {only}")
        } else {
            capped(
                &first
                    .chain
                    .iter()
                    .map(|name| plain(name))
                    .collect::<Vec<_>>()
                    .join(" › "),
                MOST_NAME,
            )
        };
        let kind = kind_said(first);
        let body = match mine.len() {
            1 => kind.to_owned(),
            many => format!("{kind}, and {} more", many - 1),
        };
        (title, body)
    } else {
        let names: Vec<String> = chats
            .iter()
            .take(MOST_NAMED)
            .map(|chat| name_of(*chat, asks))
            .collect();
        let body = match (chats.len() - names.len(), names.split_last()) {
            (0, Some((last, rest))) => format!("{} and {last}", rest.join(", ")),
            (more, _) => format!("{} and {more} more", names.join(", ")),
        };
        (format!("{} chats need you", chats.len()), body)
    };
    Notice {
        plane: plane.clone(),
        session,
        sessions: chats.to_vec(),
        title,
        body,
    }
}

/// Chat `session`'s name, as its first ask in `asks` gives it (the session, I-9), or its
/// number.
fn name_of(session: u32, asks: &[Shown]) -> String {
    asks.iter()
        .find(|ask| ask.session == session)
        .and_then(|ask| ask.chain.first())
        .map(|name| plain(name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("chat {session}"))
}

/// **A name a chat chose, as plain text** for a notification: every control and direction
/// mark dropped, each run of white space one space, and at most [`MOST_NAME`] characters.
pub fn plain(name: &str) -> String {
    plain_within(name, MOST_NAME)
}

/// [`plain`], at most `most` characters: for words a host or a runner wrote, which the window
/// shows as text (#2846).
pub fn plain_within(name: &str, most: usize) -> String {
    let mut out = String::new();
    let mut gap = false;
    for one in name.chars() {
        if one.is_whitespace() {
            gap = !out.is_empty();
            continue;
        }
        // The marks that turn the direction of the text after them: purlis's generated
        // Bidi_Control table (SK-49).
        if one.is_control() || purlis_unicode::bidi_control(one) {
            continue;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        out.push(one);
    }
    capped(&out, most)
}

/// `text` at most `most` characters, ending in `…` where it was cut.
fn capped(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(most.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// **The person's quiet hours** (#1989): from a time of day until another, in minutes since
/// midnight, local time. A span that ends before it starts runs across midnight; one that ends
/// where it starts covers nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHours {
    pub from: u16,
    pub to: u16,
}

impl QuietHours {
    /// The quiet hours a layout document keeps, under `notifications.quiet` (`{"on": true,
    /// "from": "22:00", "to": "07:00"}`, `app/src/notificationPrefs.ts`), while they are on.
    /// Anything else is none: the window says what it put right.
    pub fn of_layout(document: &serde_json::Value) -> Option<Self> {
        let quiet = document.get("notifications")?.get("quiet")?;
        if quiet.get("on")?.as_bool() != Some(true) {
            return None;
        }
        let at = |key: &str| quiet.get(key)?.as_str().and_then(minute_of);
        Some(Self {
            from: at("from")?,
            to: at("to")?,
        })
    }

    /// Whether `minute` of the day is in them.
    pub fn covers(self, minute: u16) -> bool {
        match self.from.cmp(&self.to) {
            std::cmp::Ordering::Less => (self.from..self.to).contains(&minute),
            std::cmp::Ordering::Greater => minute >= self.from || minute < self.to,
            std::cmp::Ordering::Equal => false,
        }
    }
}

/// The minute of the day `text` names, written `HH:MM` on a 24-hour clock.
pub fn minute_of(text: &str) -> Option<u16> {
    let (hours, minutes) = text.split_once(':')?;
    let two = |part: &str| {
        (part.len() == 2 && part.bytes().all(|one| one.is_ascii_digit()))
            .then(|| part.parse::<u16>().ok())
            .flatten()
    };
    let (hours, minutes) = (two(hours)?, two(minutes)?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// Whether it is the person's quiet hours on this machine now, as the layout file says. A file
/// that cannot be read says none.
fn quiet_now() -> bool {
    use chrono::Timelike;
    let Some(root) = purlis_core::machine::config_root() else {
        return false;
    };
    let Some(document) = purlis_core::windowprefs::read_layout(&root).document else {
        return false;
    };
    let Some(hours) = QuietHours::of_layout(&document) else {
        return false;
    };
    let now = chrono::Local::now();
    let minute = u16::try_from(now.hour() * 60 + now.minute()).unwrap_or_default();
    hours.covers(minute)
}

/// **Whether the person is looking at a chat's asks**: all of the window's questions, each
/// answered by the window holding the chat's project.
#[derive(Debug, Clone, Copy)]
pub struct Looking {
    /// That window is on screen, has the keyboard and has the project in front.
    pub window_in_front: bool,
    /// It has the project's Inbox open.
    pub inbox_open: bool,
    /// It has the chat on screen in the tab in front.
    pub chat_on_screen: bool,
}

impl Looking {
    pub fn is_looking(self) -> bool {
        self.window_in_front && (self.inbox_open || self.chat_on_screen)
    }
}

/// **Which project's Inbox each window has open**, as the window says it (`inbox_shown`).
/// A window that never said has none open.
#[derive(Default)]
pub struct InboxOpen {
    open: Mutex<HashSet<(String, PlaneId)>>,
}

impl InboxOpen {
    fn open(&self) -> MutexGuard<'_, HashSet<(String, PlaneId)>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `window` says whether it has `plane`'s Inbox open.
    pub fn shown(&self, window: &str, plane: &PlaneId, open: bool) {
        let key = (window.to_owned(), plane.clone());
        if open {
            self.open().insert(key);
        } else {
            self.open().remove(&key);
        }
    }

    pub fn is_open(&self, window: &str, plane: &PlaneId) -> bool {
        self.open().contains(&(window.to_owned(), plane.clone()))
    }

    /// A window gone has no Inbox open.
    pub fn forget_window(&self, window: &str) {
        self.open().retain(|(label, _)| label != window);
    }
}

/// **The project the latest notification was about**: where a reopen with every window
/// hidden goes ([`brought_back`]).
#[derive(Default)]
pub struct LatestSent {
    plane: Mutex<Option<PlaneId>>,
}

impl LatestSent {
    fn held(&self) -> MutexGuard<'_, Option<PlaneId>> {
        self.plane.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A notification about one of `plane`'s chats went out.
    pub fn sent(&self, plane: &PlaneId) {
        *self.held() = Some(plane.clone());
    }

    /// The project the latest notification was about, if one went out.
    #[cfg(any(target_os = "macos", test))]
    pub fn plane(&self) -> Option<PlaneId> {
        self.held().clone()
    }
}

/// **The window a reopen with every window hidden brings back**: the one holding the project
/// the latest notification was about (`holder`), where its landing waits; else the main
/// window, as a click on the dock icon would.
#[cfg(any(target_os = "macos", test))]
pub fn brought_back(
    latest: Option<&PlaneId>,
    holder: impl Fn(&PlaneId) -> Option<String>,
) -> String {
    latest
        .and_then(holder)
        .unwrap_or_else(|| crate::windows::MAIN.to_owned())
}

/// **The notifier**: where the registry's sources say a project's asks may have moved, and
/// the one thread that reads them and sends.
pub struct Notifier {
    poked: Mutex<mpsc::Sender<Poke>>,
    inbox: InboxOpen,
    latest: LatestSent,
}

impl Notifier {
    /// Starts the thread that reads and sends, for `app`.
    pub fn start(app: &tauri::AppHandle) -> Self {
        let (poked, heard) = mpsc::channel();
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("purlis-ask-notifications".to_owned())
            .spawn(move || listen(&app, &heard));
        Self {
            poked: Mutex::new(poked),
            inbox: InboxOpen::default(),
            latest: LatestSent::default(),
        }
    }
}

/// What the notifier is told: a project whose asks may have moved, and the chat that waits on
/// the person anew, where the board said one does.
type Poke = (PlaneId, Option<u32>);

/// `plane`'s asks may have moved: they are read again once the burst settles.
pub fn poke(app: &tauri::AppHandle, plane: &PlaneId) {
    tell(app, (plane.clone(), None));
}

/// Chat `moved` moved: its project's asks are read again, and a chat that came to wait on the
/// person by its own move (`Moved::interrupts`) waits anew ([`Rules::renew`]).
pub fn moved(app: &tauri::AppHandle, moved: &crate::hooks::Moved) {
    tell(
        app,
        (
            moved.plane.clone(),
            moved.interrupts().then_some(moved.session),
        ),
    );
}

fn tell(app: &tauri::AppHandle, poke: Poke) {
    if let Some(notifier) = app.try_state::<Notifier>() {
        let poked = notifier
            .poked
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _ = poked.send(poke);
    }
}

/// A window gone: it has no Inbox open.
pub fn window_gone<R: tauri::Runtime>(app: &impl Manager<R>, window: &str) {
    if let Some(notifier) = app.try_state::<Notifier>() {
        notifier.inbox.forget_window(window);
    }
}

/// **The app was reopened** (macOS: the dock icon, or a notification clicked) with
/// `has_visible_windows` or not. With every window hidden, nothing comes forward by itself, so
/// the window [`brought_back`] says is shown and given the focus; its landing, if a
/// notification left one, opens the Inbox at that chat's group (`askNotices.ts`).
#[cfg(target_os = "macos")]
pub fn reopened(app: &tauri::AppHandle, has_visible_windows: bool) {
    if has_visible_windows {
        return;
    }
    let latest = app
        .try_state::<Notifier>()
        .and_then(|notifier| notifier.latest.plane());
    let showing = app.try_state::<Showing>();
    let label = brought_back(latest.as_ref(), |plane| {
        showing.as_ref().and_then(|showing| showing.holder(plane))
    });
    crate::windows::raise(app, &label);
}

/// The window says whether it has `plane`'s Inbox open (#1694): while it is, and the window
/// is focused with the project in front, no notification is sent about that project's asks.
#[tauri::command]
#[specta::specta]
pub fn inbox_shown(
    window: tauri::Window,
    notifier: tauri::State<'_, Notifier>,
    plane: PlaneId,
    open: bool,
) {
    notifier.inbox.shown(window.label(), &plane, open);
}

/// The notifier's thread: each poke, then whatever follows it while it settles, read once.
fn listen(app: &tauri::AppHandle, heard: &mpsc::Receiver<Poke>) {
    let mut rules = Rules::default();
    loop {
        // Woken by a poke, or by itself: when a riding window closes with other chats on it,
        // and every so often while quiet hours hold asks back.
        let wake = woken_at(&rules, Instant::now());
        let first = match wake {
            None => match heard.recv() {
                Ok(poke) => Some(poke),
                Err(_) => return,
            },
            Some(at) => match heard.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(poke) => Some(poke),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        let mut pokes: Vec<Poke> = first.into_iter().collect();
        if !pokes.is_empty() {
            let until = Instant::now() + SETTLE;
            loop {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                match heard.recv_timeout(left) {
                    Ok(poke) => pokes.push(poke),
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
        let mut planes: Vec<PlaneId> = Vec::new();
        for (plane, anew) in pokes {
            if let Some(session) = anew {
                rules.renew(&plane, session);
            }
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
        let now = Instant::now();
        let quiet_held = woken_for_quiet(&rules);
        for plane in rules.due(now).into_iter().chain(quiet_held) {
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
        for plane in planes {
            read(app, &mut rules, &plane);
        }
    }
}

/// How often the notifier looks whether quiet hours have ended, while they hold asks back.
const QUIET_LOOK: Duration = Duration::from_secs(60);

/// When the notifier wakes by itself next, if it does.
fn woken_at(rules: &Rules, now: Instant) -> Option<Instant> {
    let quiet = (!rules.held.is_empty()).then(|| now + QUIET_LOOK);
    match (rules.next_due(), quiet) {
        (Some(one), Some(other)) => Some(one.min(other)),
        (one, other) => one.or(other),
    }
}

/// The projects quiet hours held asks back in, once they are over.
fn woken_for_quiet(rules: &Rules) -> Vec<PlaneId> {
    if rules.held.is_empty() || quiet_now() {
        return Vec::new();
    }
    rules.held_in_quiet_hours()
}

/// Reads `plane`'s asks and sends what [`Rules`] says to.
fn read(app: &tauri::AppHandle, rules: &mut Rules, plane: &PlaneId) {
    let Some(held) = app
        .try_state::<Planes>()
        .and_then(|planes| planes.held(plane).ok())
    else {
        rules.forget(plane);
        return;
    };
    let asking = crate::asking::every_ask(&held);
    let sent = rules.heard(&Heard {
        plane,
        // The registry's own split (#1693, #1694): an update is not listed, so never notifies.
        asks: &asking.asks,
        looking: &|session| looking(app, plane, session, &held).is_looking(),
        quiet: &quiet_now,
        now: Instant::now(),
    });
    for notice in sent {
        send(app, notice);
    }
}

/// What the window holding `plane` answers about chat `session` ([`Looking`]).
///
/// **The window asked is the one holding the chat's project** (charter#126): with a project
/// split into a window of its own, asking the main window would hold a notification back
/// because the person was looking at a different window. **And that window must have the
/// project in front** (#111): every project numbers its chats from one, so "is chat 3 on
/// screen" has as many answers as there are projects open, and only the window knows which one
/// it draws. A task shown inside its session's tab is on screen; a session's own chat hidden
/// behind a task is not (`Chats::looked_at`).
fn looking(
    app: &tauri::AppHandle,
    plane: &PlaneId,
    session: u32,
    held: &crate::planes::Held,
) -> Looking {
    let showing = app.try_state::<Showing>();
    let window = showing
        .as_ref()
        .and_then(|showing| showing.holder(plane))
        .and_then(|label| app.get_webview_window(&label));
    let Some(window) = window else {
        return Looking {
            window_in_front: false,
            inbox_open: false,
            chat_on_screen: false,
        };
    };
    let window_in_front = window.is_visible().unwrap_or(false)
        && window.is_focused().unwrap_or(false)
        && showing.is_some_and(|showing| showing.is_showing(window.label(), plane));
    Looking {
        window_in_front,
        inbox_open: app
            .try_state::<Notifier>()
            .is_some_and(|notifier| notifier.inbox.is_open(window.label(), plane)),
        chat_on_screen: held.chats().looks_at(session),
    }
}

/// Sends one notification, and tells the window holding its project where a click lands.
fn send(app: &tauri::AppHandle, notice: Notice) {
    use tauri_plugin_notification::NotificationExt;
    // Best effort, always. A desktop that refuses notifications, or a person who turned them
    // off, is not a reason for anything else here to stop working.
    let _ = app
        .notification()
        .builder()
        .title(notice.title)
        .body(notice.body)
        .show();
    if let Some(notifier) = app.try_state::<Notifier>() {
        notifier.latest.sent(&notice.plane);
    }
    let landing = Landing {
        plane: notice.plane,
        session: notice.session,
        sessions: notice.sessions,
    };
    crate::windows::emit_for_plane(app, &landing.plane.clone(), NOTIFIED, &landing);
}

#[cfg(test)]
#[path = "asknotify_tests.rs"]
mod tests;

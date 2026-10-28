// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Devices view** (#2052, spec #2041 story 54): this machine and every runner it keeps, each
//! with its chats and browsers, its version, how it is and when it last answered. It replaces the
//! Runners view ADR 0089 §12 described (V50), which was never drawn (ADR 0089 as amended
//! 2026-10-27).
//!
//! - **Where each chat and browser is** is where the window started it and keeps it
//!   ([`placed`]): a chat's pool as the router kept it (`pool_of`), so a chat moved since is
//!   placed where it runs now; a browser's as its chat runs it (#2050). A pool reaches its
//!   runner that takes new chats; `<pool>/<runner>` names that runner, a draining one too
//!   (#2865). A chat or browser on a pool this machine no longer keeps is said apart, never put
//!   on another device.
//! - **Each runner is asked how it is on a link of its own** ([`device_runner`]), as the Pools
//!   settings ask theirs: whether its container runs, and only then whether it answers with the
//!   key pinned for it, which version it runs and what it holds. **A suspended runner is never
//!   woken to be asked** (#2051): its container is read, and nothing links to it. A runner asked
//!   again while it is still being asked is not asked twice ([`Joined`]).
//! - **A runner is believed only about its own pool** ([`not_open_here`]): it may say it holds a
//!   chat or browser the window placed on a runner of the same pool, which puts it on that
//!   runner's row (a draining runner's, #2865). What it says of anything this machine runs, or a
//!   runner of another pool runs, moves nothing. What it holds that the window has not open is
//!   counted apart, so nothing it runs goes unsaid.
//! - **Text from elsewhere is shown safely** ([`shown_safely`]): what Docker says of a container
//!   and why a link is down reach the window on one line, without a character a reader does not
//!   see, capped, and with no address's user or password. A runner's own words for refusing to
//!   list what it holds are never shown ([`list_refused`]).
//! - **When a runner last answered** is what this window heard ([`Contacts`]): kept in memory for
//!   as long as the app runs, so a runner not reached since it started says so rather than
//!   showing a time nobody saw.
//!
//! The window's alone (`ui::WINDOW_ONLY`): the view links to runners on the person's machine.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use crate::dockerpools::{PoolLimitsShown, PoolLink};
use crate::planes::PlaneId;

/// One runner this machine keeps, as the view lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptRunner {
    pub pool: String,
    pub runner: String,
    pub draining: bool,
}

/// One chat the window has open, and the pool it runs on, as the router keeps it: `None` for
/// this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub plane: PlaneId,
    pub session: u32,
    pub name: String,
    pub on: Option<String>,
}

/// One browser of a chat the window has open, and the pool its chat runs it on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPlaced {
    pub plane: PlaneId,
    /// The chat it is of.
    pub session: u32,
    /// Its program in the session host that runs it.
    pub program: String,
    /// What the chat calls it: `b1`.
    pub name: String,
    /// What its chat is called, or was where it outlived its chat.
    pub chat: String,
    pub on: Option<String>,
}

/// A chat on a device, as its row names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatOnADevice {
    pub plane: PlaneId,
    pub session: u32,
    pub name: String,
}

/// A browser on a device, as its row names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowserOnADevice {
    pub plane: PlaneId,
    /// The chat it is of.
    pub session: u32,
    /// Its program in the session host that runs it: what a runner names it by.
    pub program: String,
    /// What the chat calls it: `b1`.
    pub name: String,
    /// What its chat is called.
    pub chat: String,
}

/// What runs on one device, of what the window has open.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OnADevice {
    pub chats: Vec<ChatOnADevice>,
    pub browsers: Vec<BrowserOnADevice>,
}

/// One runner, as its row lists it before it is asked how it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerOnTheList {
    pub pool: String,
    pub runner: String,
    /// It was replaced by a runner of this machine's version: it keeps its chats until they
    /// end, then purlis removes it (#2749).
    pub draining: bool,
    pub on: OnADevice,
}

/// A chat on a pool this machine no longer keeps.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatOnAPoolNotKept {
    pub plane: PlaneId,
    pub session: u32,
    pub name: String,
    /// The pool its chat was started on, as the router kept it.
    pub pool: String,
}

/// A browser on a pool this machine no longer keeps.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowserOnAPoolNotKept {
    pub plane: PlaneId,
    /// The chat it is of.
    pub session: u32,
    /// What the chat calls it: `b1`.
    pub name: String,
    /// What its chat is called.
    pub chat: String,
    /// The pool its chat runs it on.
    pub pool: String,
}

/// Where everything the window has open runs: this machine, each runner, and what is on a pool
/// no longer kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Placement {
    pub here: OnADevice,
    pub runners: Vec<RunnerOnTheList>,
    pub unkept: Vec<ChatOnAPoolNotKept>,
    pub unkept_browsers: Vec<BrowserOnAPoolNotKept>,
}

/// One pool's limits, which each of its runners that runs requests (#2052, story 55).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolRequests {
    pub pool: String,
    /// What one runner of it is held to.
    pub limits: PoolLimitsShown,
    /// Its CPUs in thousandths, to be added up.
    pub millicpus: u32,
}

/// **What the Devices view draws first** (#2052): this machine's version, where everything the
/// window has open runs, and each pool's limits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DevicesShown {
    /// This machine's version.
    pub version: String,
    /// Where everything the window has open runs.
    pub placement: Placement,
    pub pools: Vec<PoolRequests>,
    /// Whether this machine's Docker pools are shown to be the person's own (D-2050-4): their
    /// engine is a local socket on this machine. Only then does the use table name them as the
    /// person's.
    pub owned: bool,
}

/// **Where each chat and browser the window has open is** (#2052): on this machine, on a runner
/// `kept` lists, or on a pool no longer kept. Every kept runner is listed, in `kept`'s order.
pub fn placed(kept: &[KeptRunner], chats: Vec<Placed>, browsers: Vec<BrowserPlaced>) -> Placement {
    let mut runners: Vec<RunnerOnTheList> = kept
        .iter()
        .map(|one| RunnerOnTheList {
            pool: one.pool.clone(),
            runner: one.runner.clone(),
            draining: one.draining,
            on: OnADevice::default(),
        })
        .collect();
    // A pool reaches its runner that takes new chats; `<pool>/<runner>` that runner itself.
    let at = |on: &str| -> Option<usize> {
        match on.split_once('/') {
            Some((pool, runner)) => kept
                .iter()
                .position(|one| one.pool == pool && one.runner == runner),
            None => kept.iter().position(|one| one.pool == on && !one.draining),
        }
    };
    let mut placement = Placement::default();
    for chat in chats {
        let shown = ChatOnADevice {
            plane: chat.plane,
            session: chat.session,
            name: chat.name,
        };
        match chat.on {
            None => placement.here.chats.push(shown),
            Some(pool) => match at(&pool) {
                Some(index) => runners[index].on.chats.push(shown),
                None => placement.unkept.push(ChatOnAPoolNotKept {
                    plane: shown.plane,
                    session: shown.session,
                    name: shown.name,
                    pool,
                }),
            },
        }
    }
    for browser in browsers {
        let shown = BrowserOnADevice {
            plane: browser.plane,
            session: browser.session,
            program: browser.program,
            name: browser.name,
            chat: browser.chat,
        };
        match browser.on {
            None => placement.here.browsers.push(shown),
            Some(pool) => match at(&pool) {
                Some(index) => runners[index].on.browsers.push(shown),
                None => placement.unkept_browsers.push(BrowserOnAPoolNotKept {
                    plane: shown.plane,
                    session: shown.session,
                    name: shown.name,
                    chat: shown.chat,
                    pool,
                }),
            },
        }
    }
    placement.runners = runners;
    placement
}

/// The most characters of a text from elsewhere the window is shown ([`shown_safely`]).
pub const MOST_SHOWN: usize = 200;

/// **A text from elsewhere, as the window may show it** (#2052): what Docker says of a container,
/// or why a link is down. One line (a line end or a tab is a space), with no character a reader
/// would not see there (`purlis_unicode`'s one rule: a reordering or a format character is
/// dropped), no address's user or password (`ssh://me:pw@host` is `ssh://…@host`), and at most
/// [`MOST_SHOWN`] characters, then `…`.
pub fn shown_safely(text: &str) -> String {
    // Read no further than a cap can use: a megabyte of text costs no more than its start.
    let start: String = text.chars().take(MOST_SHOWN * 8).collect();
    let cut = text.chars().nth(MOST_SHOWN * 8).is_some();
    let one_line: String = start
        .chars()
        // Every blank parts two words, as a space does.
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .collect();
    let seen = purlis_unicode::unhidden(&one_line, purlis_unicode::Breaks::Marked);
    let hidden = without_userinfo(&seen);
    let mut words: Vec<&str> = hidden.split(' ').filter(|word| !word.is_empty()).collect();
    // A word the cut ends part way through is not shown where it may be an address: what of it
    // would be hidden may lie past the cut.
    if cut && words.last().is_some_and(|word| word.contains(':')) {
        words.pop();
    }
    let mut out = String::new();
    for word in words {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.chars().count() > MOST_SHOWN {
        let mut capped: String = out.chars().take(MOST_SHOWN).collect();
        capped.push('…');
        return capped;
    }
    out
}

/// `text` with the user and password before a host written `…`, word by word: in
/// `scheme://user[:password]@host`, the whole of what stands between `://` and the word's last
/// `@` (a password may hold a `/`), and in `user@host` without a scheme (as ssh writes it), what
/// stands before the `@`, a password holding any character. An image named by its digest
/// (`name@sha256:…`) keeps its name.
fn without_userinfo(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let Some(at) = word.rfind('@') else {
                return word.to_owned();
            };
            if let Some(scheme) = word.find("://").filter(|scheme| scheme + 3 <= at) {
                return format!("{}…{}", &word[..scheme + 3], &word[at..]);
            }
            let (before, after) = word.split_at(at);
            let lower = after.to_ascii_lowercase();
            let digest = ["@sha256:", "@sha384:", "@sha512:"]
                .iter()
                .any(|algorithm| lower.starts_with(algorithm));
            // A name and a password (`name:…@`) is a user's, whatever the password holds: one at
            // the word's start, or one after a path's last `/`. Anything else before its `@`
            // names a path or a registry.
            let whole = before.split_once(':').is_some_and(|(name, rest)| {
                // `registry:5000/…` is a host and its port, not a name and a password.
                let port = rest.split_once('/').is_some_and(|(port, _)| {
                    !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
                });
                !name.is_empty() && !name.contains('/') && !port
            });
            let after_a_path = before.rsplit_once('/').filter(|(_, last)| {
                last.split_once(':')
                    .is_some_and(|(name, _)| !name.is_empty())
            });
            if !whole
                && after_a_path.is_none()
                && (digest || before.is_empty() || before.contains('/'))
            {
                return word.to_owned();
            }
            // Keep what opens the word (a quote, a bracket, a path): only the name itself goes.
            let opens = if whole {
                before
                    .char_indices()
                    .find(|(_, ch)| ch.is_alphanumeric())
                    .map_or(before.len(), |(at, _)| at)
            } else {
                after_a_path.map_or_else(
                    || {
                        before
                            .char_indices()
                            .find(|(_, ch)| ch.is_alphanumeric())
                            .map_or(before.len(), |(at, _)| at)
                    },
                    |(path, _)| path.len() + 1,
                )
            };
            format!("{}…{after}", &before[..opens])
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// **Why a runner did not list what it holds, in purlis's words** (#2052): a refusal says so,
/// with its code where the code is a plain word; anything else the runner answered is said to be
/// unread. The runner's own sentence is never shown: it would read as purlis's.
pub fn list_refused(error: &purlis_session_protocol::session::CallError) -> String {
    use purlis_session_protocol::session::CallError;
    match error {
        CallError::Refused(refusal) => {
            let plain = !refusal.code.is_empty()
                && refusal.code.len() <= 64
                && refusal
                    .code
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_');
            if plain {
                format!(
                    "the runner refused to list what it holds ({})",
                    refusal.code
                )
            } else {
                "the runner refused to list what it holds".to_owned()
            }
        }
        CallError::Unexpected(_) => "the runner answered something purlis does not read".to_owned(),
        // Said on this machine, of its own link.
        CallError::Closed(_) => shown_safely(&error.to_string()),
    }
}

/// Whether a runner's container runs (#2051).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunnerStanding {
    Running,
    /// Its container stopped, its home kept: it starts again as a chat next needs it.
    Suspended,
    /// Its container could not be read, and why, as Docker said it ([`shown_safely`]).
    Unknown {
        why: String,
    },
    /// It was not answered within the time an ask of it waits ([`WAITS_AT_MOST`]): how it is
    /// is not known.
    Unanswered,
}

/// One program a runner holds, as its own list says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedProgram {
    /// Its id in the runner's host.
    pub id: String,
    /// The project it is of, by its stable id.
    pub project: String,
    /// Its number in that project, for a chat.
    pub number: Option<u32>,
    pub ended: bool,
    pub browser: bool,
}

/// What one runner answered, before it is set against what the window has open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub standing: RunnerStanding,
    /// What the link said; none where nothing linked to it.
    pub link: Option<PoolLink>,
    pub version: Option<String>,
    /// When it last answered, in seconds since 1970.
    pub contacted: Option<u64>,
    /// What it holds that has not ended.
    pub holds: Vec<ListedProgram>,
}

/// **What one runner answers** (#2052): its container read first (`running`); only a running one
/// is linked to (`list`), and asked its version (`version`). `last` is when it last answered
/// before, and `now` the time now, in seconds since 1970. What Docker and the link say is shown
/// safely ([`shown_safely`]).
pub fn answered(
    running: Result<bool, String>,
    list: impl FnOnce() -> Result<Vec<ListedProgram>, PoolLink>,
    version: impl FnOnce() -> Option<String>,
    last: Option<u64>,
    now: u64,
) -> Answered {
    let standing = match running {
        Ok(true) => RunnerStanding::Running,
        Ok(false) => RunnerStanding::Suspended,
        Err(why) => RunnerStanding::Unknown {
            why: shown_safely(&why),
        },
    };
    if standing != RunnerStanding::Running {
        return Answered {
            standing,
            link: None,
            version: None,
            contacted: last,
            holds: Vec::new(),
        };
    }
    match list() {
        Ok(listed) => Answered {
            standing,
            link: Some(PoolLink::Linked),
            version: version(),
            contacted: Some(now),
            holds: listed.into_iter().filter(|one| !one.ended).collect(),
        },
        Err(link) => Answered {
            standing,
            link: Some(match link {
                PoolLink::Down { why } => PoolLink::Down {
                    why: shown_safely(&why),
                },
                other => other,
            }),
            version: None,
            contacted: last,
            holds: Vec::new(),
        },
    }
}

/// An open chat or browser, where the window has it and the pool it placed it on (`None` for
/// this machine).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenChat {
    pub plane: PlaneId,
    pub session: u32,
    /// The pool, without a runner's name after it.
    pub pool: Option<String>,
}

/// The chats and browsers the window has open, by what a runner's list names them by: a chat by
/// its project's stable id and its number, a browser by its program's id.
#[derive(Debug, Clone, Default)]
pub struct OpenHere {
    pub chats: BTreeMap<(String, u32), OpenChat>,
    /// Each browser, with its chat as the window has it.
    pub browsers: BTreeMap<String, OpenChat>,
}

/// A chat the window has open, by its plane and its session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatHeld {
    pub plane: PlaneId,
    pub session: u32,
}

/// What a runner holds, set against what the window has open: the chats and browsers of its own
/// pool it holds, which its row lists by what the runner says rather than where the window
/// started them, and how many it holds the window has not open.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NotOpenHere {
    pub open: Vec<ChatHeld>,
    /// The browsers, by their programs.
    pub browsers_open: Vec<String>,
    pub chats: u32,
    pub browsers: u32,
}

/// **What of `holds`, the list of a runner of `pool`, the window has open, and how much it has
/// not** (#2052). A runner is believed only about a chat or browser the window placed on its own
/// pool; one this machine runs, or a runner of another pool does, is neither moved nor counted.
pub fn not_open_here(holds: &[ListedProgram], open: &OpenHere, pool: &str) -> NotOpenHere {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let ours = |chat: &OpenChat| chat.pool.as_deref() == Some(pool);
    let chat = |one: &ListedProgram| {
        one.number
            .and_then(|number| open.chats.get(&(one.project.clone(), number)))
    };
    let (mut shown, mut chats, mut browsers_open, mut browsers) = (Vec::new(), 0, Vec::new(), 0);
    for one in holds {
        match (
            one.browser,
            one.browser.then(|| open.browsers.get(&one.id)).flatten(),
            chat(one),
        ) {
            (true, Some(held), _) if ours(held) => browsers_open.push(one.id.clone()),
            (true, Some(_), _) => {}
            (true, None, _) => browsers += 1,
            (false, _, Some(held)) if ours(held) => shown.push(ChatHeld {
                plane: held.plane.clone(),
                session: held.session,
            }),
            (false, _, Some(_)) => {}
            (false, _, None) => chats += 1,
        }
    }
    NotOpenHere {
        open: shown,
        browsers_open,
        chats: count(chats),
        browsers: count(browsers),
    }
}

/// **When each runner last answered this window**, by `<pool>/<runner>`, for as long as the app
/// runs.
#[derive(Debug, Default)]
pub struct Contacts(Mutex<HashMap<String, u64>>);

impl Contacts {
    /// `runner` answered at `at`, in seconds since 1970.
    pub fn answered(&self, runner: &str, at: u64) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(runner.to_owned(), at);
    }

    /// When `runner` last answered, if it has since the app started.
    pub fn last(&self, runner: &str) -> Option<u64> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(runner)
            .copied()
    }
}

/// Where one ask in flight stands.
enum Standing<T> {
    Asking,
    Answered(T),
    /// It ended without an answer: it panicked.
    Failed,
}

/// One ask in flight, which a second ask of the same runner waits on.
struct Slot<T> {
    standing: Mutex<Standing<T>>,
    ready: Condvar,
    waiting: AtomicUsize,
}

/// **Each runner asked once at a time** (#2052): an ask of a runner while it is still being
/// asked waits for that ask's answer rather than making another link. A Refresh, or the view
/// opened again, so never leaves more than one link open to one runner. The ask in flight is
/// joined, not cancelled: a `docker exec` that is part way through its handshake is not stopped
/// cleanly from here, and its answer is what the second ask wants anyway.
///
/// **Nothing waits for ever.** A second ask waits at most the time it is given, then answers what
/// `late` says; and the first always leaves its answer and its place, whatever becomes of it: one
/// that panics is said to have failed to everyone waiting on it ([`Done`]), and the next ask asks
/// again.
pub struct Joined<T> {
    asking: Mutex<HashMap<String, Arc<Slot<T>>>>,
}

impl<T> Default for Joined<T> {
    fn default() -> Self {
        Joined {
            asking: Mutex::default(),
        }
    }
}

/// **The first ask's guard**: when it is dropped, answered or not, the ask's place is cleared and
/// everyone waiting on it is told. One dropped without an answer (the ask panicked) is said to
/// have failed.
struct Done<'a, T> {
    joined: &'a Joined<T>,
    key: &'a str,
    slot: Arc<Slot<T>>,
}

impl<T> Drop for Done<'_, T> {
    fn drop(&mut self) {
        {
            let mut standing = self
                .slot
                .standing
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if matches!(*standing, Standing::Asking) {
                *standing = Standing::Failed;
            }
        }
        self.joined
            .asking
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(self.key);
        self.slot.ready.notify_all();
    }
}

impl<T: Clone> Joined<T> {
    /// What `ask` answers for `key`, asked only when no ask of `key` is in flight. Else what that
    /// one answers, waited for at most `within`; and what `late` answers where it gave none in
    /// that time, or failed.
    pub fn once(
        &self,
        key: &str,
        within: std::time::Duration,
        ask: impl FnOnce() -> T,
        late: impl FnOnce() -> T,
    ) -> T {
        let (slot, first) = {
            let mut asking = self.asking.lock().unwrap_or_else(PoisonError::into_inner);
            match asking.get(key) {
                Some(slot) => {
                    slot.waiting.fetch_add(1, Ordering::SeqCst);
                    (Arc::clone(slot), false)
                }
                None => {
                    let slot = Arc::new(Slot {
                        standing: Mutex::new(Standing::Asking),
                        ready: Condvar::new(),
                        waiting: AtomicUsize::new(0),
                    });
                    asking.insert(key.to_owned(), Arc::clone(&slot));
                    (slot, true)
                }
            }
        };
        if first {
            let done = Done {
                joined: self,
                key,
                slot: Arc::clone(&slot),
            };
            let answer = ask();
            *slot.standing.lock().unwrap_or_else(PoisonError::into_inner) =
                Standing::Answered(answer.clone());
            drop(done);
            return answer;
        }
        let deadline = std::time::Instant::now() + within;
        let mut standing = slot.standing.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            match &*standing {
                Standing::Answered(answer) => return answer.clone(),
                Standing::Failed => {
                    drop(standing);
                    return late();
                }
                Standing::Asking => {}
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                drop(standing);
                return late();
            }
            standing = slot
                .ready
                .wait_timeout(standing, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Whether another ask of `key` waits on the one in flight.
    #[cfg(test)]
    pub fn is_waited_on(&self, key: &str) -> bool {
        self.asking
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .is_some_and(|slot| slot.waiting.load(Ordering::SeqCst) > 0)
    }

    /// Whether an ask of `key` is in flight.
    #[cfg(test)]
    pub fn is_asking(&self, key: &str) -> bool {
        self.asking
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(key)
    }
}

/// The contacts this window keeps.
fn contacts() -> &'static Contacts {
    static CONTACTS: std::sync::OnceLock<Contacts> = std::sync::OnceLock::new();
    CONTACTS.get_or_init(Contacts::default)
}

/// The asks of runners in flight.
fn asking() -> &'static Joined<Result<RunnerAsked, String>> {
    static ASKING: std::sync::OnceLock<Joined<Result<RunnerAsked, String>>> =
        std::sync::OnceLock::new();
    ASKING.get_or_init(Joined::default)
}

/// **How one runner is**, as its row says it once asked (#2052).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerAsked {
    /// Whether its container runs. A suspended runner is told by this alone: nothing links to
    /// it, so its `link` is none.
    pub standing: RunnerStanding,
    /// What a link to it said; none where nothing linked to it (one suspended, or whose
    /// container could not be read).
    pub link: Option<PoolLink>,
    /// The version it runs, where it answered.
    pub version: Option<String>,
    /// When it last answered this window, in seconds since 1970 (good to 2106); none if it has
    /// not since the app started.
    pub contacted: Option<u32>,
    /// What it holds, set against what the window has open.
    pub holds: NotOpenHere,
}

/// **Everything the window has open**, in one walk of the projects it holds: each chat and
/// browser with the pool it runs on, and the same by what a runner names them by.
fn what_is_open(planes: &crate::planes::Planes) -> (Vec<Placed>, Vec<BrowserPlaced>, OpenHere) {
    let (mut chats, mut browsers, mut open) = (Vec::new(), Vec::new(), OpenHere::default());
    let pool_part = |on: &Option<String>| {
        on.as_deref()
            .map(|on| on.split_once('/').map_or(on, |(pool, _)| pool).to_owned())
    };
    for held in planes.every_held() {
        let plane = held.plane_id().clone();
        let project = purlis_core::scaffold::planefile::project_id(held.root());
        let now = held.chats().open_now();
        let names: BTreeMap<u32, String> = now
            .iter()
            .filter_map(|one| Some((one.session, held.chats().shown_name(one.session)?)))
            .collect();
        let mut pools: BTreeMap<u32, Option<String>> = BTreeMap::new();
        for one in now.iter().filter(|one| one.harness.is_some()) {
            let on = held.chats().pool_of(one.session);
            pools.insert(one.session, pool_part(&on));
            if let Some(project) = &project {
                open.chats.insert(
                    (project.clone(), one.session),
                    OpenChat {
                        plane: plane.clone(),
                        session: one.session,
                        pool: pool_part(&on),
                    },
                );
            }
            chats.push(Placed {
                plane: plane.clone(),
                session: one.session,
                name: names.get(&one.session).cloned().unwrap_or_default(),
                on,
            });
        }
        for browser in held.browsers().listed() {
            open.browsers.insert(
                browser.program.clone(),
                OpenChat {
                    plane: plane.clone(),
                    session: browser.session,
                    pool: pool_part(&browser.pool),
                },
            );
            browsers.push(BrowserPlaced {
                plane: plane.clone(),
                session: browser.session,
                chat: browser
                    .was
                    .clone()
                    .or_else(|| names.get(&browser.session).cloned())
                    .unwrap_or_default(),
                program: browser.program,
                name: browser.name,
                on: browser.pool,
            });
        }
    }
    chats.sort_by(|a, b| (a.plane.as_str(), a.session).cmp(&(b.plane.as_str(), b.session)));
    (chats, browsers, open)
}

/// **Every device and what runs on it** (#2052): this machine and every runner it keeps, each
/// with the chats and browsers the window has open there, and each pool's limits.
#[tauri::command]
#[specta::specta]
pub async fn devices(
    planes: tauri::State<'_, crate::planes::Planes>,
) -> Result<DevicesShown, String> {
    let (chats, browsers, _) = what_is_open(&planes);
    crate::off_the_window("reading this machine's devices", move || {
        let (kept, pools, owned) = kept_here()?;
        Ok(DevicesShown {
            version: crate::runnerlinks::desktop_version().to_owned(),
            placement: placed(&kept, chats, browsers),
            pools,
            owned,
        })
    })
    .await
}

#[cfg(unix)]
fn kept_here() -> Result<(Vec<KeptRunner>, Vec<PoolRequests>, bool), String> {
    let Some(config) = purlis_core::machine::config_root() else {
        return Ok((Vec::new(), Vec::new(), false));
    };
    let kept = purlis_core::runners::read(&config)
        .map_err(|why| why.to_string())?
        .iter()
        .map(|one| KeptRunner {
            pool: one.pool().to_owned(),
            runner: one.name().to_owned(),
            draining: one.draining,
        })
        .collect();
    let pools = purlis_core::runners::pools(&config)
        .map_err(|why| why.to_string())?
        .iter()
        .map(|pool| PoolRequests {
            pool: pool.name().to_owned(),
            limits: crate::dockerpools::shown(pool.limits()),
            millicpus: pool.limits().millicpus(),
        })
        .collect();
    // As the pool logins decide it (D-2050-4): a local socket on this machine.
    let owned =
        purlis_core::poollogins::owned(purlis_core::dockerpool::Docker::engine_on_this_machine())
            .is_ok();
    Ok((kept, pools, owned))
}

/// No Docker pool is kept but on macOS and Linux: this machine is the only device.
#[cfg(not(unix))]
fn kept_here() -> Result<(Vec<KeptRunner>, Vec<PoolRequests>, bool), String> {
    Ok((Vec::new(), Vec::new(), false))
}

/// **How the runner `runner` of pool `pool` is** (#2052): its container read, and only a running
/// one linked to, asked its version and what it holds. Asked once at a time ([`Joined`]).
#[tauri::command]
#[specta::specta]
pub async fn device_runner(
    planes: tauri::State<'_, crate::planes::Planes>,
    pool: String,
    runner: String,
) -> Result<RunnerAsked, String> {
    let (_, _, open) = what_is_open(&planes);
    #[cfg(unix)]
    {
        crate::off_the_window("asking a runner how it is", move || {
            let name = format!("{pool}/{runner}");
            asking().once(
                &name,
                WAITS_AT_MOST,
                || ask_runner(&pool, &runner, &name, &open),
                || Ok(unanswered(contacts().last(&name))),
            )
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (open, pool, runner);
        Err("purlis keeps Docker pools on macOS and Linux only.".to_owned())
    }
}

/// How long an ask of a runner waits on another ask of it already in flight.
pub const WAITS_AT_MOST: std::time::Duration = std::time::Duration::from_secs(60);

/// How long each `docker` run an ask makes may take (an inspect, a version, the check before a
/// link): a wedged engine answers nothing, and the ask goes on without it.
pub const EACH_DOCKER_RUN_WITHIN: std::time::Duration = std::time::Duration::from_secs(20);

/// What a runner not answered in time is said to be: not known, last answered when it last was.
pub fn unanswered(last: Option<u64>) -> RunnerAsked {
    RunnerAsked {
        standing: RunnerStanding::Unanswered,
        link: None,
        version: None,
        contacted: last.and_then(|at| u32::try_from(at).ok()),
        holds: NotOpenHere::default(),
    }
}

/// [`device_runner`], asked.
#[cfg(unix)]
fn ask_runner(
    pool: &str,
    runner: &str,
    name: &str,
    open: &OpenHere,
) -> Result<RunnerAsked, String> {
    let config = purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine.".to_owned())?;
    let here = crate::runnerlinks::Machine::here(&config)?;
    // Every `docker` run this ask makes is held to a time: a wedged engine holds up nothing.
    let machine = crate::runnerlinks::Machine {
        docker: here.docker.clone().within(EACH_DOCKER_RUN_WITHIN),
        ..here
    };
    let kept = purlis_core::runners::read(&config)
        .map_err(|why| why.to_string())?
        .into_iter()
        .find(|one| one.pool() == pool && one.name() == runner)
        .ok_or_else(|| format!("No runner {runner} of pool {pool} is on this machine."))?;
    let asked = answered(
        machine.docker.running(&kept.made).map_err(|why| why.0),
        || {
            // As it is: a runner is never woken to be asked.
            let to = crate::runnerlinks::ToARunner::of_kept(&machine, &kept)
                .map_err(|why| PoolLink::Down { why })?;
            crate::runnerlinks::listed(&to)
        },
        || crate::runnerlinks::version_of(&machine, &kept).ok(),
        contacts().last(name),
        crate::sandboxing::now_secs(),
    );
    if let Some(at) = asked.contacted {
        contacts().answered(name, at);
    }
    Ok(RunnerAsked {
        holds: not_open_here(&asked.holds, open, pool),
        standing: asked.standing,
        link: asked.link,
        version: asked.version,
        contacted: asked.contacted.and_then(|at| u32::try_from(at).ok()),
    })
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Asks that reach nobody, and needs-you** (M75 slice 19, #7028; spec §6.8 and §8.3 to §8.5,
//! Q314 points 1 to 5, 13 to 15).
//!
//! An ask outcome reaches the person only if the chat is attended **and** a window can be reached
//! ([`route`]). The app closed, a runner away, a pool and headless `purlis serve` have no window,
//! and a task or a runner has nobody at it; the runner's browser-login hold is this rule (Q314.2),
//! not a rule of its own. Otherwise the chat gets a refusal ([`refusal`]) and one item in
//! needs-you ([`Record::raise`]).
//!
//! - **One item per rule, resource, workspace and request digest** ([`Key`]), with a count of the
//!   chats that asked and the last time. The digest is in the key, so an Allow grants only what the
//!   item showed (review round 2, S2-SF4): [`Grant::text`] carries it.
//! - **The wait is capped below the harness's hook timeout** ([`cap`], through
//!   [`Deadline::below_hook_timeout`]). An ask unanswered at the cap is refused and raised
//!   ([`wait`], [`Why::NoAnswer`]); the harness's own prompt never decides it.
//! - **An answer from needs-you** ([`Record::allow`], [`Record::deny`]) leaves the item and is
//!   kept for each chat that asked, to be told at its next call ([`Record::take_told`]); nothing
//!   is run again. An Allow becomes a grant for next time ([`Grant`], sealed by slice 20's store),
//!   except a **Once** grant, which is deferred ([`Deferred`], [`Late`]): bound to the chat and to
//!   the request's digest, spent once, within [`LATE_FOR_SECS`] (spec §8.3, proposed).
//! - **Only the person allows.** [`Record::allow`] takes a [`Press`], made only by the window's
//!   command layer, so a headless machine, `purlis inbox` included, can [`Record::deny`] and
//!   [`Record::dismiss`] and cannot allow: it has no press to pass (Q314.5).
//! - **Its caps are limits** ([`Caps`]): `asks.needs-you-max-items` (the least recently raised go
//!   first), `asks.needs-you-max-days` (past it from the first raise an item goes) and
//!   `asks.needs-you-quiet-days` (an item nobody raised again for that long goes).
//!
//! # Where and how
//!
//! `<data>/needs-you/items.json`, one JSON object for the machine, rewritten whole under a lock on
//! its folder, `0600` ([`Store`]). It holds items and the notes chats are still to be told, and no
//! grant: a grant is the sealed store's. A file this build cannot read reads as nothing and is
//! written over. Deferred Once grants are in memory only ([`Late`]), as a lapsed ask's pre-answer
//! is: a restart drops them and the chat asks again.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::askwait::{self, Ended};
use crate::dispatchunattended::Attendance;
use crate::harness::hooked::HOOK_TIMEOUT;
use crate::harness::model::Deadline;
use crate::secrets::answers::Press;

/// The folder under the data home.
pub const DIR: &str = "needs-you";

/// The file in it.
pub const FILE: &str = "items.json";

/// **How long a deferred Once grant waits for its chat's same request**: half an hour (spec §8.3).
pub const LATE_FOR_SECS: u64 = 30 * 60;

/// The most bytes the file is read to.
pub const MOST_BYTES: u64 = 4 * 1024 * 1024;

/// The most notes kept for chats not yet told; the oldest go first.
pub const MOST_TOLD: usize = 500;

/// The most characters of what an item says.
const MOST_SAID: usize = 400;

const DAY: u64 = 24 * 60 * 60;

// ---- where an ask goes --------------------------------------------------------------------

/// **Whether the person can be reached for an ask**: the chat's attendance, and whether a window
/// is open to show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    attended: bool,
    window: bool,
}

impl Reach {
    /// The chat's `attendance` (the fact the app holds, [`crate::attendance::of_held`]: no fact is
    /// unattended) and whether a `window` can be reached.
    pub fn of(attendance: Attendance, window: bool) -> Self {
        Self {
            attended: attendance == Attendance::Attended,
            window,
        }
    }
}

/// **Where an ask goes.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The person is asked, in the window.
    Ask,
    /// The chat is refused and the ask raised in needs-you.
    Refuse(Why),
}

/// **Why an ask was refused and raised in needs-you.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// Nobody is at the chat: a task, a runner, or a chat whose prompts are off.
    Unattended,
    /// Nobody can be shown it: the app is closed, the runner is away, a pool or headless `serve`.
    NoWindow,
    /// It was asked, and was not answered before the cap.
    NoAnswer,
}

/// **Where an ask at `reach` goes** (spec §6.8): to the person only if the chat is attended and a
/// window can be reached.
pub fn route(reach: Reach) -> Route {
    if !reach.attended {
        Route::Refuse(Why::Unattended)
    } else if !reach.window {
        Route::Refuse(Why::NoWindow)
    } else {
        Route::Ask
    }
}

/// **What a refused chat is told**: what happened and what to do, naming the request.
pub fn refusal(why: Why, said: &str) -> String {
    let cause = match why {
        Why::Unattended => "Nobody is at this chat to answer, so purlis did not ask.",
        Why::NoWindow => "No purlis window could be reached to ask in, so purlis did not ask.",
        Why::NoAnswer => "Nobody answered in time, so purlis refused it.",
    };
    format!(
        "Refused: {said}. {cause} It is in needs-you for the person to decide. Do not retry \
         it now; if they allow it, you are told at your next call and can ask again."
    )
}

// ---- the wait's cap -----------------------------------------------------------------------

/// **How long an ask waits for the person**: below the harness's hook timeout by the margin
/// ([`Deadline::below_hook_timeout`]), so the refusal lands before the harness stops waiting.
pub fn cap() -> Duration {
    Deadline::below_hook_timeout(HOOK_TIMEOUT)
        .and_then(Deadline::duration)
        .expect("the hook timeout leaves room below it")
}

/// **How a wait on the person ended.**
#[derive(Debug, PartialEq, Eq)]
pub enum Settled<H> {
    /// The person answered.
    Answered(H),
    /// It is refused, and raised in needs-you for `Why`.
    Refused(Why),
    /// What waited went away: nothing is raised.
    Gone,
}

/// [`Settled`], from how [`askwait::waited`] ended: an unanswered wait is refused, never left to
/// the harness's pane.
pub fn settle<H>(ended: Ended<H>) -> Settled<H> {
    match ended {
        Ended::Heard(answer) => Settled::Answered(answer),
        Ended::NotAnswered => Settled::Refused(Why::NoAnswer),
        Ended::Closed | Ended::AskerGone => Settled::Gone,
    }
}

/// **Waits for an answer at most `at_most`**, looking whether the asker is `gone`; `last` is the
/// answer that came in the last moment.
pub fn wait<H>(
    heard: &mpsc::Receiver<H>,
    at_most: Duration,
    gone: &dyn Fn() -> bool,
    last: &dyn Fn() -> Option<H>,
) -> Settled<H> {
    settle(askwait::waited(heard, at_most, gone, last))
}

/// [`wait`], for the cap ([`cap`]): what an ask on a hook waits.
pub fn wait_for_the_person<H>(
    heard: &mpsc::Receiver<H>,
    gone: &dyn Fn() -> bool,
    last: &dyn Fn() -> Option<H>,
) -> Settled<H> {
    wait(heard, cap(), gone, last)
}

// ---- the caps -----------------------------------------------------------------------------

/// **The needs-you caps**, slice 12's `asks.*` limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// `asks.needs-you-max-items`.
    pub max_items: usize,
    /// `asks.needs-you-max-days`: an item this old, from its first raise, goes.
    pub max_days: u64,
    /// `asks.needs-you-quiet-days`: an item nobody raised again for this long goes.
    pub quiet_days: u64,
}

impl Caps {
    /// The shipped limits.
    pub fn shipped() -> Self {
        Self::from_limits(&|_| None)
    }

    /// The caps as `limit` reads them (the value in force for a key, `None` for no word of it:
    /// the shipped one then).
    pub fn from_limits(limit: &dyn Fn(&str) -> Option<i64>) -> Self {
        let number = |key: &str| -> u64 {
            let shipped = super::limits::spec(key).and_then(|spec| match spec.shipped {
                super::limits::Shipped::Number(n) => Some(n),
                _ => None,
            });
            let said = limit(key)
                .or(shipped)
                .expect("a needs-you limit ships a number");
            u64::try_from(said).unwrap_or(0)
        };
        Self {
            max_items: usize::try_from(number("asks.needs-you-max-items")).unwrap_or(usize::MAX),
            max_days: number("asks.needs-you-max-days"),
            quiet_days: number("asks.needs-you-quiet-days"),
        }
    }
}

// ---- items --------------------------------------------------------------------------------

/// **The digest of a request**, as the host made it (its line, its change), in the item's key.
pub fn digest_of(action: &str, resource: &str, request: &str) -> String {
    use sha2::Digest as _;
    let text = format!("{action}\u{0}{resource}\u{0}{request}");
    crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
}

/// **What one needs-you item is for**: a rule, a resource, a workspace and a request digest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Key {
    /// The `@id` of the rule that asks.
    pub rule: String,
    /// The resource, as the host names it.
    pub resource: String,
    /// The workspace the chat works in.
    pub workspace: String,
    /// The digest of the request ([`digest_of`]).
    pub digest: String,
}

impl Key {
    /// A key.
    pub fn new(rule: &str, resource: &str, workspace: &str, digest: &str) -> Self {
        Self {
            rule: rule.to_owned(),
            resource: resource.to_owned(),
            workspace: workspace.to_owned(),
            digest: digest.to_owned(),
        }
    }

    /// **The id `purlis inbox` names it by**: the start of a digest of the whole key.
    pub fn id(&self) -> String {
        use sha2::Digest as _;
        let text = format!(
            "{}\u{0}{}\u{0}{}\u{0}{}",
            self.rule, self.resource, self.workspace, self.digest
        );
        crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))[..12].to_owned()
    }
}

/// **One thing the person has yet to decide.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    /// What it is for.
    pub key: Key,
    /// The action, a verb of the schema.
    pub action: String,
    /// What it asks, in one masked line.
    pub said: String,
    /// Why it is here (the cause of the last raise).
    pub why: Why,
    /// The chats that asked, each once.
    pub chats: Vec<String>,
    /// When it was first raised, seconds since 1970.
    pub first: u64,
    /// When it was last raised.
    pub last: u64,
}

impl Item {
    /// How many chats asked.
    pub fn chat_count(&self) -> usize {
        self.chats.len()
    }
}

/// **What raises an item.**
#[derive(Debug, Clone)]
pub struct Raise<'a> {
    /// What it is for.
    pub key: Key,
    /// The action.
    pub action: &'a str,
    /// What it asks, in one line (cut where long).
    pub said: &'a str,
    /// Why the ask did not reach the person.
    pub why: Why,
    /// The chat that asked.
    pub chat: &'a str,
}

// ---- the person's answers -----------------------------------------------------------------

/// **How far an Allow reaches**: Once, This chat, This workspace, Everywhere (spec §8.6; the
/// timed grant is slice 21's).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Once, for a chat: deferred, bound to the chat and the request.
    Once(String),
    /// This chat.
    Chat(String),
    /// This workspace.
    Workspace,
    /// Everywhere.
    Everywhere,
}

impl Scope {
    fn word(&self) -> &'static str {
        match self {
            Scope::Once(_) => "once",
            Scope::Chat(_) => "this chat",
            Scope::Workspace => "this workspace",
            Scope::Everywhere => "everywhere",
        }
    }

    fn chat(&self) -> Option<&str> {
        match self {
            Scope::Once(chat) | Scope::Chat(chat) => Some(chat),
            Scope::Workspace | Scope::Everywhere => None,
        }
    }
}

/// **What the person answered.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Allowed, to this reach.
    Allowed(Scope),
    /// Denied.
    Denied,
}

/// **A grant an Allow becomes**, for slice 20's sealed store to keep: it covers one rule, one
/// action, one resource and the request digest it was given under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// What it covers.
    pub key: Key,
    /// The action it covers.
    pub action: String,
    /// How far it reaches.
    pub scope: Scope,
}

impl Grant {
    /// **The grant as one line of the sealed store** ([`crate::secrets::answers::GRANT`]).
    pub fn text(&self) -> String {
        let mut text = format!(
            "needs-you {} rule={} action={} resource={} workspace={} digest={}",
            self.scope.word(),
            self.key.rule,
            self.action,
            self.key.resource,
            self.key.workspace,
            self.key.digest
        );
        if let Some(chat) = self.scope.chat() {
            text.push_str(&format!(" chat={chat}"));
        }
        text
    }
}

/// **What an Allow made.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowed {
    /// The grant for the sealed store; for a Once grant it is only the record of what was given.
    pub grant: Grant,
    /// For a Once grant, what the host holds in memory to serve the chat's same request once.
    pub deferred: Option<Deferred>,
}

/// **A Once grant, deferred**: bound to a chat and a request digest, spent once, until a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deferred {
    chat: String,
    digest: String,
    until: u64,
}

/// **The deferred Once grants a host holds**, in memory.
#[derive(Debug, Default)]
pub struct Late {
    held: Vec<Deferred>,
}

impl Late {
    /// Holds `grant` until its time.
    pub fn hold(&mut self, grant: Deferred) {
        self.held.push(grant);
    }

    /// **Whether `chat`'s request with `digest`, at `now`, is served by a grant**, which is spent:
    /// one grant serves one request of the chat it was given to, within [`LATE_FOR_SECS`].
    pub fn spend(&mut self, chat: &str, digest: &str, now: u64) -> bool {
        self.held.retain(|one| one.until >= now);
        match self
            .held
            .iter()
            .position(|one| one.chat == chat && one.digest == digest)
        {
            Some(at) => {
                self.held.remove(at);
                true
            }
            None => false,
        }
    }
}

/// **What a chat is told at its next call**: the person answered what it had waited on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Told {
    /// The chat.
    pub chat: String,
    /// What it had asked.
    pub said: String,
    /// What the person answered.
    pub answer: Answer,
    /// When.
    pub at: u64,
}

impl Told {
    /// **The words the chat reads.** Nothing was run for it.
    pub fn words(&self) -> String {
        match &self.answer {
            Answer::Allowed(scope) => format!(
                "The person allowed {} ({}). Nothing was run for you: ask again.",
                self.said,
                scope.word()
            ),
            Answer::Denied => format!(
                "The person denied {}. Nothing was run; do not ask for it again.",
                self.said
            ),
        }
    }
}

// ---- the record ---------------------------------------------------------------------------

/// **Needs-you as kept**: the items, and the notes chats are still to be told.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    #[serde(default)]
    items: Vec<Item>,
    #[serde(default)]
    told: Vec<Told>,
}

fn cut(text: &str) -> String {
    text.chars().take(MOST_SAID).collect()
}

impl Record {
    /// The items, the least recently raised first.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The notes not yet told.
    pub fn told(&self) -> &[Told] {
        &self.told
    }

    /// The only item whose id starts with `prefix`; none where several or none do.
    pub fn find(&self, prefix: &str) -> Option<&Item> {
        let mut found = self
            .items
            .iter()
            .filter(|one| one.key.id().starts_with(prefix));
        let one = found.next()?;
        found.next().is_none().then_some(one)
    }

    /// **Raises an item at `now`**: a new one, or the chat added to the one with the same key and
    /// its last time moved. Then the caps are held.
    pub fn raise(&mut self, raise: Raise<'_>, now: u64, caps: Caps) {
        match self.items.iter_mut().find(|one| one.key == raise.key) {
            Some(one) => {
                if !one.chats.iter().any(|chat| chat == raise.chat) {
                    one.chats.push(raise.chat.to_owned());
                }
                one.last = one.last.max(now);
                one.why = raise.why;
            }
            None => self.items.push(Item {
                key: raise.key,
                action: raise.action.to_owned(),
                said: cut(raise.said),
                why: raise.why,
                chats: vec![raise.chat.to_owned()],
                first: now,
                last: now,
            }),
        }
        self.hold_caps(now, caps);
    }

    fn hold_caps(&mut self, now: u64, caps: Caps) {
        self.items.retain(|one| {
            now.saturating_sub(one.first) <= caps.max_days.saturating_mul(DAY)
                && now.saturating_sub(one.last) <= caps.quiet_days.saturating_mul(DAY)
        });
        self.items.sort_by_key(|one| one.last);
        let past = self.items.len().saturating_sub(caps.max_items);
        self.items.drain(..past);
    }

    fn take(&mut self, key: &Key) -> Option<Item> {
        let at = self.items.iter().position(|one| &one.key == key)?;
        Some(self.items.remove(at))
    }

    fn tell(&mut self, item: &Item, answer: &Answer, only: Option<&str>, now: u64) {
        for chat in item
            .chats
            .iter()
            .filter(|chat| only.is_none_or(|one| one == *chat))
        {
            self.told.push(Told {
                chat: chat.clone(),
                said: item.said.clone(),
                answer: answer.clone(),
                at: now,
            });
        }
        let past = self.told.len().saturating_sub(MOST_TOLD);
        self.told.drain(..past);
    }

    /// **Allows the item `key`, on the person's press** (`_press`): the item leaves needs-you,
    /// each chat that asked is to be told, and the answer is a grant for next time. A This-chat or
    /// Once scope must name a chat that asked, or nothing changes and none is answered.
    pub fn allow(&mut self, _press: &Press, key: &Key, scope: Scope, now: u64) -> Option<Allowed> {
        let item = self.items.iter().find(|one| &one.key == key)?;
        if let Some(chat) = scope.chat()
            && !item.chats.iter().any(|asked| asked == chat)
        {
            return None;
        }
        let item = self.take(key)?;
        let deferred = match &scope {
            Scope::Once(chat) => Some(Deferred {
                chat: chat.clone(),
                digest: item.key.digest.clone(),
                until: now.saturating_add(LATE_FOR_SECS),
            }),
            _ => None,
        };
        self.tell(&item, &Answer::Allowed(scope.clone()), None, now);
        Some(Allowed {
            grant: Grant {
                key: item.key,
                action: item.action,
                scope,
            },
            deferred,
        })
    }

    /// **Denies the item `key`**: it leaves needs-you and each chat that asked is to be told.
    /// Needs no press: a denial loosens nothing.
    pub fn deny(&mut self, key: &Key, now: u64) -> Option<Item> {
        let item = self.take(key)?;
        self.tell(&item, &Answer::Denied, None, now);
        Some(item)
    }

    /// **Dismisses the item `key`**: it leaves needs-you and nobody is told. The same ask later
    /// is a new item.
    pub fn dismiss(&mut self, key: &Key) -> Option<Item> {
        self.take(key)
    }

    /// **The notes `chat` is to be told at its next call**, which are then forgotten.
    pub fn take_told(&mut self, chat: &str) -> Vec<Told> {
        let (mine, rest) = std::mem::take(&mut self.told)
            .into_iter()
            .partition(|note| note.chat == chat);
        self.told = rest;
        mine
    }
}

// ---- the store ----------------------------------------------------------------------------

/// **The record on this machine**, under a data home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// The record under the data home `data`.
    pub fn in_data(data: &Path) -> Self {
        Self {
            dir: data.join(DIR),
        }
    }

    /// The record under this machine's data home, where there is one purlis may write in.
    pub fn here() -> Option<Self> {
        let data = crate::datahome::root()?;
        crate::datahome::refusal(&data)
            .is_none()
            .then(|| Self::in_data(&data))
    }

    /// Its file.
    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    /// The record as kept; nothing where it is missing, cannot be read, or is too large.
    pub fn read(&self) -> Record {
        use std::io::Read as _;
        let Ok(file) = std::fs::File::open(self.file()) else {
            return Record::default();
        };
        let mut text = Vec::new();
        if file.take(MOST_BYTES + 1).read_to_end(&mut text).is_err()
            || text.len() as u64 > MOST_BYTES
        {
            return Record::default();
        }
        serde_json::from_slice(&text).unwrap_or_default()
    }

    /// Raises an item ([`Record::raise`]).
    pub fn raise(&self, raise: Raise<'_>, now: u64, caps: Caps) -> io::Result<()> {
        self.change(|record| record.raise(raise, now, caps))
    }

    /// Denies the item whose id starts with `id` ([`Record::deny`]); none where no one item does.
    pub fn deny(&self, id: &str, now: u64) -> io::Result<Option<Item>> {
        let mut gone = None;
        self.change(|record| {
            if let Some(key) = record.find(id).map(|one| one.key.clone()) {
                gone = record.deny(&key, now);
            }
        })?;
        Ok(gone)
    }

    /// Dismisses the item whose id starts with `id` ([`Record::dismiss`]).
    pub fn dismiss(&self, id: &str) -> io::Result<Option<Item>> {
        let mut gone = None;
        self.change(|record| {
            if let Some(key) = record.find(id).map(|one| one.key.clone()) {
                gone = record.dismiss(&key);
            }
        })?;
        Ok(gone)
    }

    /// The notes `chat` is to be told, taken ([`Record::take_told`]).
    pub fn take_told(&self, chat: &str) -> io::Result<Vec<Told>> {
        let mut mine = Vec::new();
        self.change(|record| mine = record.take_told(chat))?;
        Ok(mine)
    }

    /// Reads the record, lets `change` edit it and writes it back under the lock on its folder.
    pub fn change(&self, change: impl FnOnce(&mut Record)) -> io::Result<()> {
        crate::rewrite::create_dir_all(&self.dir)?;
        let _held = crate::rewrite::Lock::bounded(&self.dir)?;
        let mut record = self.read();
        change(&mut record);
        let text = serde_json::to_string(&record).map_err(io::Error::other)?;
        crate::rewrite::replace(
            &self.dir,
            &self.file(),
            text.as_bytes(),
            crate::rewrite::Mode::Private,
        )
    }
}

#[cfg(test)]
#[path = "needsyou_tests.rs"]
mod tests;

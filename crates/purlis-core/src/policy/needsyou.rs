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
//!   chats that asked and the last time. The digest is in the key, and the item keeps the request
//!   it is the digest of, so an Allow recomputes it and grants only what the item showed (review
//!   round 2, S2-SF4): a file whose digest no longer matches its request grants nothing.
//!   [`Grant::text`] is the sealed store's own grant ([`crate::policy::grants`]), carrying the
//!   request's narrowing context, so an inbox grant serves the same ask a window grant does. The
//!   digest catches damage, not a forger who writes a consistent
//!   item with a benign `said`: the defence against that is the sandbox, which denies a chat
//!   reading and writing the folder. So what the person is shown to Allow is made from the
//!   item's action, resource and request, never from `said`.
//! - **The wait is capped below the harness's hook timeout, less what the hook already ran**
//!   ([`cap`], through [`Deadline::below_hook_timeout`], as `harness/hooked.rs` does for a hook
//!   that says `ran_for`, #1063). An ask unanswered at the cap, or with no time left at all, is
//!   refused and raised ([`wait`], [`Why::NoAnswer`]); the harness's own prompt never decides it.
//! - **An answer from needs-you** ([`Record::allow`], [`Record::deny`]) leaves the item and is
//!   kept for each chat that asked, to be told at its next call ([`Record::take_told`]); nothing
//!   is run again. An Allow becomes a grant for next time ([`Grant`], sealed by slice 20's store),
//!   except a **Once** grant, which is deferred ([`Deferred`], [`Late`]): bound to the chat and to
//!   the request's digest, spent once, within [`LATE_FOR_SECS`] (spec §8.3, proposed).
//! - **Only the person allows.** [`Record::allow`] takes a [`Press`], made only by the window's
//!   command layer, so a headless machine, `purlis inbox` included, can [`Record::deny`] and
//!   [`Record::dismiss`] and cannot allow: it has no press to pass (Q314.5).
//! - **Its caps are limits** ([`Caps`]): `asks.needs-you-max-items` (the chat holding the most
//!   items loses its least recently raised first, so one chat cannot evict the others; the
//!   file's 4 MiB is held the same way, by bytes, and one item is at most [`MOST_ITEM_BYTES`]),
//!   `asks.needs-you-max-days` (past it from the first raise an item goes) and
//!   `asks.needs-you-quiet-days` (an item nobody raised again for that long goes). They are held
//!   on every read as well as every write, and every field is bounded ([`MOST_FIELD`] and
//!   its kin).
//!
//! # Where and how
//!
//! `<data>/needs-you/items.json`, one JSON object for the machine, rewritten whole under a lock on
//! its folder, `0600` ([`Store`]). It holds items and the notes chats are still to be told, and no
//! grant: a grant is the sealed store's. A chat's sandbox is denied reading and writing
//! the folder (`sandbox::app_state_held`, Integrity), so a chat can neither forge an item nor
//! read another chat's requests. A file this build
//! cannot parse reads as nothing and is written over; any other failure to read it (permissions,
//! a device error) refuses the change instead. Deferred Once grants are in memory only ([`Late`]), as a lapsed ask's pre-answer
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

/// The most characters of a rule id, an action, a chat id or a workspace; of a resource or a
/// request, [`MOST_RESOURCE`] and [`MOST_REQUEST`]. A raise past one is not kept.
pub const MOST_FIELD: usize = 128;

/// The most bytes of a resource, as the host names it.
pub const MOST_RESOURCE: usize = 1024;

/// The most bytes of a request, the digest's preimage.
pub const MOST_REQUEST: usize = 16 * 1024;

/// **The most bytes one item takes in the file**, as serialised. A control character is six bytes
/// in JSON (`\u0001`), so the field bounds alone allow about 100 KiB; an item past this is not
/// kept, so the file's cap is shared among many items and not taken by one.
pub const MOST_ITEM_BYTES: usize = 64 * 1024;

/// The most chats one item names; a chat past it is not added.
pub const MOST_CHATS: usize = 100;

/// The fewest characters of an id [`Record::find`] takes: the empty prefix must not stand for
/// the only item.
pub const MIN_PREFIX: usize = 4;

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
/// `kept` is [`Record::raise`]'s answer: only an item that is in needs-you is said to be.
pub fn refusal(why: Why, said: &str, kept: bool) -> String {
    let cause = match why {
        Why::Unattended => "Nobody is at this chat to answer, so purlis did not ask.",
        Why::NoWindow => "No purlis window could be reached to ask in, so purlis did not ask.",
        Why::NoAnswer => "Nobody answered in time, so purlis refused it.",
    };
    if kept {
        format!(
            "Refused: {said}. {cause} It is in needs-you for the person to decide. Do not retry \
             it now; if they allow it, you are told at your next call and can ask again."
        )
    } else {
        format!(
            "Refused: {said}. {cause} It could not be kept in needs-you, so the person has not \
             been told of it. Do not retry it; ask the person to allow it another way."
        )
    }
}

// ---- the wait's cap -----------------------------------------------------------------------

/// **How long an ask waits for the person**: below the harness's hook timeout, less what the hook
/// had already run (`ran_for`, as `harness/hooked.rs` takes it), by the margin
/// ([`Deadline::below_hook_timeout`]), so the refusal lands before the harness stops waiting.
/// `None` when no time is left: the ask is then refused at once, never left to the pane.
pub fn cap(ran_for: Duration) -> Option<Duration> {
    Deadline::below_hook_timeout(HOOK_TIMEOUT.saturating_sub(ran_for)).and_then(Deadline::duration)
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

/// [`wait`], for the cap ([`cap`]): what an ask on a hook that had run `ran_for` waits. With no
/// time left it does not wait: it is refused at once ([`Why::NoAnswer`]), as an ask unanswered at
/// the cap is.
pub fn wait_for_the_person<H>(
    heard: &mpsc::Receiver<H>,
    ran_for: Duration,
    gone: &dyn Fn() -> bool,
    last: &dyn Fn() -> Option<H>,
) -> Settled<H> {
    match cap(ran_for) {
        Some(at_most) => wait(heard, at_most, gone, last),
        None => Settled::Refused(Why::NoAnswer),
    }
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

    /// **No cap at all**: nothing is held back and nothing goes. For a command that has no word of
    /// the limits in force (`purlis inbox`), which must neither hide an item the window shows nor
    /// evict one by a cap the window would not apply.
    pub fn none() -> Self {
        Self {
            max_items: usize::MAX,
            max_days: u64::MAX,
            quiet_days: u64::MAX,
        }
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
        // A limit of 0 (or a negative one) must not empty needs-you on every read: each has a
        // floor, so an item is held for at least a day and the box for at least one item.
        Self {
            max_items: usize::try_from(number("asks.needs-you-max-items"))
                .unwrap_or(usize::MAX)
                .max(1),
            max_days: number("asks.needs-you-max-days").max(1),
            quiet_days: number("asks.needs-you-quiet-days").max(1),
        }
    }
}

// ---- items --------------------------------------------------------------------------------

/// **A digest of `parts`, each length-prefixed**, so no value can run into the next one.
fn digest_parts(parts: &[&str]) -> String {
    use sha2::Digest as _;
    let mut hash = sha2::Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    crate::extension::hex(&hash.finalize())
}

/// **The digest of a request**, as the host made it (its line, its change), in the item's key.
/// Each part is length-prefixed, so `("a", "bc")` and `("ab", "c")` are not the same request.
pub fn digest_of(action: &str, resource: &str, request: &str) -> String {
    digest_parts(&[action, resource, request])
}

/// **What binds an item's digest to what a sealed grant is matched on besides it** (review round 2,
/// F4): the resource's kind and the narrowing context, with the digest of the request, each part
/// length-prefixed and the narrowing's fields in name order. The digest catches damage, not a
/// forger who writes a consistent item (the sandbox is the defence against that).
pub fn bound_of(
    digest: &str,
    resource_kind: &str,
    narrowing: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let mut fields: Vec<(&String, String)> = narrowing
        .iter()
        .map(|(name, value)| (name, value.to_string()))
        .collect();
    fields.sort();
    let mut parts: Vec<String> = vec![digest.to_owned(), resource_kind.to_owned()];
    for (name, value) in fields {
        parts.push(name.clone());
        parts.push(value);
    }
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    digest_parts(&parts)
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
        digest_parts(&[&self.rule, &self.resource, &self.workspace, &self.digest])[..12].to_owned()
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
    /// The request the key's digest is the digest of ([`digest_of`]): an Allow recomputes it, so
    /// what is granted is what was raised. Empty (and so never allowed) where a file lacks it.
    #[serde(default)]
    pub request: String,
    /// The kind of the resource (`Branch`), which a sealed grant is matched on. Empty where a file
    /// lacks it: the grant made from it then matches nothing.
    #[serde(default)]
    pub resource_kind: String,
    /// The narrowing context the request carried (`force`, `port`), which a sealed grant covers.
    #[serde(default)]
    pub narrowing: serde_json::Map<String, serde_json::Value>,
    /// What binds the key's digest to the resource's kind and the narrowing ([`bound_of`]): an
    /// Allow recomputes it, so a file whose kind or narrowing was edited grants nothing. Empty
    /// (and so never allowed) where a file lacks it.
    #[serde(default)]
    pub bound: String,
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

/// **What a sealed grant is matched on, besides what a [`Key`] holds**: the resource's kind and the
/// narrowing context the request carried ([`crate::policy::grants::narrowing_of`]).
#[derive(Debug, Clone, Default)]
pub struct Narrowed {
    /// The resource's kind, `Branch`.
    pub resource_kind: String,
    /// The narrowing context.
    pub narrowing: serde_json::Map<String, serde_json::Value>,
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
    /// The request itself, which the key's digest is the digest of ([`digest_of`]).
    pub request: &'a str,
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

/// **A grant an Allow becomes**, for the sealed store: it covers one rule, one action, one
/// resource and the narrowing context the request carried (M75-20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// What it covers.
    pub key: Key,
    /// The action it covers.
    pub action: String,
    /// The kind of the resource.
    pub resource_kind: String,
    /// The narrowing context it was given under.
    pub narrowing: serde_json::Map<String, serde_json::Value>,
    /// How far it reaches.
    pub scope: Scope,
}

impl Grant {
    /// **The grant as the sealed store keeps it** ([`crate::policy::grants::Grant`]), the one form
    /// [`crate::policy::grants::apply`] reads, so a grant given in the inbox serves the same ask a
    /// grant given in the window does. This workspace is the item's workspace; Once and This chat
    /// are the chat's.
    pub fn sealed(&self) -> crate::policy::grants::Grant {
        use crate::policy::grants::{Grant as Sealed, Scope as Reach};
        let (scope, chat) = match &self.scope {
            Scope::Once(chat) => (Reach::Once, Some(chat.clone())),
            Scope::Chat(chat) => (Reach::Chat, Some(chat.clone())),
            Scope::Workspace => (Reach::Workspace, None),
            Scope::Everywhere => (Reach::Everywhere, None),
        };
        Sealed {
            rule: self.key.rule.clone(),
            action: self.action.clone(),
            resource_kind: self.resource_kind.clone(),
            resource: self.key.resource.clone(),
            narrowing: self.narrowing.clone(),
            scope,
            chat,
            workspace: matches!(self.scope, Scope::Workspace)
                .then(|| self.key.workspace.clone()),
        }
    }

    /// **The grant as one line of the sealed store** ([`crate::secrets::answers::GRANT`]): one
    /// JSON object, so a value holding a space, an `=` or a quote cannot pass for another field.
    /// What the person was shown is what it covers: the item's digest was checked at the Allow
    /// ([`Record::allow`]), and the grant carries the request's own narrowing.
    pub fn text(&self) -> String {
        self.sealed().text()
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

impl Allowed {
    /// **The line the sealed store is to keep for this Allow, if any** (review round 2, F9). A
    /// Once Allow is the deferred grant's alone: sealing it too would let one Allow serve twice.
    pub fn to_seal(&self) -> Option<String> {
        Some(self.grant.text())
    }
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

/// Whether `raise` is within every bound and its digest is that of its request.
fn fits(raise: &Raise<'_>) -> bool {
    let key = &raise.key;
    let within = |text: &str, most: usize| text.chars().count() <= most;
    within(&key.rule, MOST_FIELD)
        && within(&key.workspace, MOST_FIELD)
        && within(raise.action, MOST_FIELD)
        && within(raise.chat, MOST_FIELD)
        && key.resource.len() <= MOST_RESOURCE
        && raise.request.len() <= MOST_REQUEST
        && key.digest == digest_of(raise.action, &key.resource, raise.request)
}

/// The bytes `item` takes in the file.
fn size_of(item: &Item) -> usize {
    serde_json::to_vec(item).map_or(usize::MAX, |text| text.len())
}

/// The chat an item counts against when the items are over the cap: the first that asked.
fn owner(item: &Item) -> &str {
    item.chats.first().map_or("", String::as_str)
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

    /// The only item whose id starts with `prefix` (at least [`MIN_PREFIX`] characters); none
    /// where several or none do.
    pub fn find(&self, prefix: &str) -> Option<&Item> {
        if prefix.chars().count() < MIN_PREFIX {
            return None;
        }
        let mut found = self
            .items
            .iter()
            .filter(|one| one.key.id().starts_with(prefix));
        let one = found.next()?;
        found.next().is_none().then_some(one)
    }

    /// **Raises an item at `now`**: a new one, or the chat added to the one with the same key and
    /// its last time moved. Then the caps are held. `true` when the item is in needs-you after
    /// that: not where a field is past its bound or the digest is not the request's, nor where
    /// the caps let it go at once (a re-raise past the most days), and the chat is not to be told
    /// it is there.
    pub fn raise(&mut self, raise: Raise<'_>, now: u64, caps: Caps) -> bool {
        self.raise_narrowed(raise, Narrowed::default(), now, caps)
    }

    /// [`Record::raise`], with what a sealed grant is matched on besides the rule, the action and
    /// the resource's id: the resource's kind and the narrowing context (M75-20). An item raised
    /// without them makes a grant that matches nothing.
    pub fn raise_narrowed(
        &mut self,
        raise: Raise<'_>,
        narrowed: Narrowed,
        now: u64,
        caps: Caps,
    ) -> bool {
        if !fits(&raise) || narrowed.resource_kind.chars().count() > MOST_FIELD {
            return false;
        }
        // What a reader sees is marked, so a line end or a bidirectional control cannot forge a
        // line. A rule or workspace that needed it is no real one, and its item is its own.
        let mut key = raise.key;
        key.rule = crate::shown::one_line(&key.rule, MOST_FIELD);
        key.workspace = crate::shown::one_line(&key.workspace, MOST_FIELD);
        let said = crate::shown::one_line(raise.said, MOST_SAID);
        match self.items.iter_mut().find(|one| one.key == key) {
            Some(one) => {
                if !one.chats.iter().any(|chat| chat == raise.chat) {
                    if one.chats.len() >= MOST_CHATS {
                        // Past the most chats: this chat is not named, so it is not to be told
                        // the item is there for it.
                        return false;
                    }
                    one.chats.push(raise.chat.to_owned());
                }
                one.last = one.last.max(now);
                one.why = raise.why;
            }
            None => {
                let item = Item {
                    key: key.clone(),
                    action: raise.action.to_owned(),
                    said,
                    request: raise.request.to_owned(),
                    bound: bound_of(&key.digest, &narrowed.resource_kind, &narrowed.narrowing),
                    resource_kind: narrowed.resource_kind,
                    narrowing: narrowed.narrowing,
                    why: raise.why,
                    chats: vec![raise.chat.to_owned()],
                    first: now,
                    last: now,
                };
                if size_of(&item) > MOST_ITEM_BYTES {
                    return false;
                }
                self.items.push(item);
            }
        }
        self.hold_caps(now, caps);
        self.items.iter().any(|one| one.key == key)
    }

    /// **Holds the caps at `now`**: what is too old or too quiet goes, then, while there are too
    /// many, the least recently raised item of the chat holding the most (so one chat cannot
    /// push the others out). Each [`Store`] read and write does it.
    pub fn hold_caps(&mut self, now: u64, caps: Caps) {
        self.items.retain(|one| {
            now.saturating_sub(one.first) <= caps.max_days.saturating_mul(DAY)
                && now.saturating_sub(one.last) <= caps.quiet_days.saturating_mul(DAY)
        });
        self.items.sort_by_key(|one| one.last);
        let mut held: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for one in &self.items {
            *held.entry(owner(one).to_owned()).or_default() += 1;
        }
        while self.items.len() > caps.max_items {
            // The heaviest chat; of equals, the one whose item is the least recent.
            let heaviest = self
                .items
                .iter()
                .map(|one| (held[owner(one)], owner(one)))
                .fold(None::<(usize, &str)>, |best, now| match best {
                    Some(best) if best.0 >= now.0 => Some(best),
                    _ => Some(now),
                })
                .map(|(_, chat)| chat.to_owned());
            let Some(chat) = heaviest else { break };
            if let Some(at) = self.items.iter().position(|one| owner(one) == chat) {
                self.items.remove(at);
                if let Some(count) = held.get_mut(&chat) {
                    *count -= 1;
                }
            }
        }
    }

    /// **Drops the least recently raised item of the owner holding the most bytes**, as
    /// [`Record::hold_caps`] does by count, so a chat that floods needs-you loses its own items
    /// and never another chat's. `false` when there is no item left to drop.
    fn shed_heaviest_owner(&mut self) -> bool {
        let mut bytes: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for one in &self.items {
            *bytes.entry(owner(one).to_owned()).or_default() += size_of(one);
        }
        // Of equal weights the first in name order, so the choice is the same on every run.
        let Some((chat, _)) =
            bytes
                .into_iter()
                .fold(None::<(String, usize)>, |best, now| match best {
                    Some(best) if best.1 >= now.1 => Some(best),
                    _ => Some(now),
                })
        else {
            return false;
        };
        let least_recent = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, one)| owner(one) == chat)
            .min_by_key(|(_, one)| one.last)
            .map(|(at, _)| at);
        match least_recent {
            Some(at) => {
                self.items.remove(at);
                true
            }
            None => false,
        }
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
    /// the chats it concerns are to be told (every chat that asked, or for a Once or This-chat
    /// scope only that chat), and the answer is a grant for next time. A This-chat or Once scope
    /// must name a chat that asked, and the item's digest must be the digest of the request it
    /// keeps: otherwise nothing changes and none is answered, so what was shown is what is granted.
    pub fn allow(&mut self, _press: &Press, key: &Key, scope: Scope, now: u64) -> Option<Allowed> {
        let item = self.items.iter().find(|one| &one.key == key)?;
        if digest_of(&item.action, &item.key.resource, &item.request) != item.key.digest {
            return None;
        }
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
        self.tell(&item, &Answer::Allowed(scope.clone()), scope.chat(), now);
        Some(Allowed {
            grant: Grant {
                key: item.key,
                action: item.action,
                resource_kind: item.resource_kind,
                narrowing: item.narrowing,
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

    /// The record as kept at `now`, its caps held ([`Record::hold_caps`]). Nothing where the file
    /// is missing, too large or not something this build parses; any other failure to read it
    /// (permissions, a device error) is an error, and a change is then refused rather than
    /// written over what could not be read.
    pub fn read(&self, now: u64, caps: Caps) -> io::Result<Record> {
        use std::io::Read as _;
        let file = match std::fs::File::open(self.file()) {
            Ok(file) => file,
            Err(why) if why.kind() == io::ErrorKind::NotFound => return Ok(Record::default()),
            Err(why) => return Err(why),
        };
        let mut text = Vec::new();
        file.take(MOST_BYTES + 1).read_to_end(&mut text)?;
        if text.len() as u64 > MOST_BYTES {
            return Ok(Record::default());
        }
        let mut record: Record = serde_json::from_slice(&text).unwrap_or_default();
        record.hold_caps(now, caps);
        Ok(record)
    }

    /// Raises an item ([`Record::raise`]); `true` when it is in needs-you after the caps.
    pub fn raise(&self, raise: Raise<'_>, now: u64, caps: Caps) -> io::Result<bool> {
        let mut kept = false;
        self.change(now, caps, |record| kept = record.raise(raise, now, caps))?;
        Ok(kept)
    }

    /// Denies the item whose id starts with `id` ([`Record::deny`]); none where no one item does.
    pub fn deny(&self, id: &str, now: u64, caps: Caps) -> io::Result<Option<Item>> {
        let mut gone = None;
        self.change(now, caps, |record| {
            if let Some(key) = record.find(id).map(|one| one.key.clone()) {
                gone = record.deny(&key, now);
            }
        })?;
        Ok(gone)
    }

    /// Dismisses the item whose id starts with `id` ([`Record::dismiss`]).
    pub fn dismiss(&self, id: &str, now: u64, caps: Caps) -> io::Result<Option<Item>> {
        let mut gone = None;
        self.change(now, caps, |record| {
            if let Some(key) = record.find(id).map(|one| one.key.clone()) {
                gone = record.dismiss(&key);
            }
        })?;
        Ok(gone)
    }

    /// The notes `chat` is to be told, taken ([`Record::take_told`]).
    pub fn take_told(&self, chat: &str, now: u64, caps: Caps) -> io::Result<Vec<Told>> {
        let mut mine = Vec::new();
        self.change(now, caps, |record| mine = record.take_told(chat))?;
        Ok(mine)
    }

    /// Reads the record, lets `change` edit it and writes it back under the lock on its folder.
    /// A file that cannot be read for a reason other than being absent is not written over; one
    /// that would pass [`MOST_BYTES`] is written without items first, the least recently raised of the
    /// owner holding the most bytes each time, so one chat's flood never evicts another's item.
    pub fn change(&self, now: u64, caps: Caps, change: impl FnOnce(&mut Record)) -> io::Result<()> {
        crate::rewrite::create_dir_all(&self.dir)?;
        let _held = crate::rewrite::Lock::bounded(&self.dir)?;
        let mut record = self.read(now, caps)?;
        change(&mut record);
        record.hold_caps(now, caps);
        let text = loop {
            let text = serde_json::to_vec(&record).map_err(io::Error::other)?;
            if text.len() as u64 <= MOST_BYTES {
                break text;
            }
            if record.shed_heaviest_owner() {
                continue;
            } else if !record.told.is_empty() {
                record.told.remove(0);
            } else {
                return Err(io::Error::other("needs-you cannot be made small enough"));
            }
        };
        crate::rewrite::replace(
            &self.dir,
            &self.file(),
            &text,
            crate::rewrite::Mode::Private,
        )
    }
}

#[cfg(test)]
#[path = "needsyou_tests.rs"]
mod tests;

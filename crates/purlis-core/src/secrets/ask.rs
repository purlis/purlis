// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A missing secret is a Secret ask, never a dead end** (ADR 0109, spec #5054; #5068, #5071).
//!
//! A brokered `purlis secret exec` whose vault holds no value under a key it hands over no
//! longer fails with "no secret". The app raises a Secret ask, which the person answers in the
//! window, and the command waits ([`Asks::hold`]):
//!
//! - **The person fills it or declines it, in the window alone.** A fill is a new value, written
//!   through the vault's own provider, and the waiting command reads it as it reads every other
//!   value. A decline is said to the chat as one, `declined <vault>/<key>`, distinct from a key
//!   the vault does not hold. Only an [`Answerer`] admitted as the window answers
//!   ([`Asks::answer`]): `charter inbox` is refused, and a chat, a relayed link or any other
//!   client is never an answerer at all (V16, V75, ADR 0080 §5.3).
//! - **Or the person picks an entry they keep** ([`Answer::Link`], #5073): the asked key becomes
//!   a Linked entry reading it ([`super::links`]), never a copy, and the waiting command reads the
//!   entry's value through it. The chat hears filled and the key's name, never the entry: what a
//!   chat learns must not tell it where a value lives.
//! - **The command waits until just before its harness would kill it** ([`holds_for`]): three
//!   quarters of the harness's own command timeout. A quick answer is invisible to the chat. A
//!   slow one ends the command saying the ask is still answerable in Asks (✋) in the purlis
//!   window ([`crate::askwait::still_answerable_said`]); the ask stays open, and the chat is
//!   nudged when it is answered ([`Answered::nudge`]).
//! - **One ask per vault and key** in a project, whichever chats miss it: a second chat joins
//!   the open ask, which says how many wait ([`Open::waiting`]), and one fill settles them all.
//! - **Every read purlis makes for a chat asks alike** (#5074): a brokered run's, through
//!   [`Asks::hold`] for each key its run misses, and a browser login's and a profile's
//!   credential at a chat's start, each through [`Asks::read_asking`].
//!   A start has no chat number yet ([`Need::chat`] is `None`): the app's waits on nothing and
//!   is refused, telling the person to fill the ask and start it again. It is counted among the
//!   waiting, but is never nudged, keeps no decline, and is traced under
//!   [`crate::trace::NO_SESSION`].
//! - **A decline settles the chats waiting then, and only them.** Each is not asked about that
//!   key again for as long as it lives; another chat that misses it later is asked afresh.
//!
//! **A chat on a pool's runner asks here too, and is sent a value only as it always was** (#5077,
//! ADR 0109): the desktop's window raises its ask, and the fill is stored on the desktop. What
//! reaches the runner goes by the one route a runner chat's tool is ever sent a value by, its
//! run's own Allow (ADR 0078 §7, `secret.forwarded`); nothing here sends anything anywhere. A
//! runner that cannot receive one is told so at once, `declined <vault>/<key>: runner can't
//! receive secrets`, and the ask stays for the person to fill for this machine's chats
//! ([`Asks::cannot_receive`], [`Open::cannot_receive`]).
//!
//! **No value is held here past its write.** The window's answer carries it straight to the
//! provider; [`Open`], what the chat hears, the nudge and the trace name the vault and the key,
//! never a value. Each ask's life is recorded in the chat's trace as `secret.ask`, metadata only:
//! raised, joined, filled, declined.
//!
//! **An open ask outlives a restart of the app** (#5079, #5054's story 59), as a task's open
//! question does: [`Asks::kept`] writes each one, by its names and the chats that wait on it, to
//! `<state>/app/`[`KEPT_IN`] whenever that changes, masked as the open questions are
//! ([`crate::dispatchrecord::masked`]), and reads them back as the app starts. A restart ends
//! every command that waited, so each chat is nudged when the ask is answered, unless it ran its
//! command again and waits once more; a chat the restart did not bring back waits on nothing
//! ([`Asks::only_these_came_back`]). What a chat declined is not kept: it lives as long as the
//! app's own run, so after a restart that chat may be asked about that key once more.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use serde_json::Value;

use super::{Ctx, Zeroizing, cmd};
use crate::harness::Harness;
use crate::harness::asks::{Answerer, HumanScope};

pub mod run;

/// What the trace records an ask's life under.
pub const TRACED_AS: &str = "secret.ask";

/// The file in `<state>/app/` the open asks are kept in across a restart ([`Asks::kept`]).
pub const KEPT_IN: &str = "secret-asks.json";

/// The version of [`KEPT_IN`]'s shape this purlis writes.
const KEPT_VERSION: u32 = 1;

/// **What a brokered run that missed a key asks for**: the app's record of the chat (its number
/// and the persona it was started as), and the vault and key its run reads. Never a word the
/// chat's line carried but the vault and key it asked for, which the run was authorised for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    /// The chat that asks; `None` for a chat's start, which has no number yet (#5074).
    pub chat: Option<u32>,
    pub persona: String,
    pub vault: String,
    pub key: String,
    /// The declared tool the run is of, where it is one: what the person reads the key is for.
    pub tool: Option<String>,
    /// **The chat's own words for what it needs the key for** (#5072), one line and bounded
    /// ([`purpose_said`]): shown as the chat's, never as purlis's. None for a broker miss.
    pub purpose: Option<String>,
    /// **What the persona's definition says the key is for**, where it declares the key as a
    /// Secret need ([`super::needs`], #5076). None for a key it does not declare, which is shown
    /// with a quiet "not declared by this persona" tag (#5072).
    pub declared: Option<String>,
    /// **The persona names no vault** (`vault: none`, or nothing at all, #5076): the ask is for
    /// a vault named after it, made at the first fill, and Remember sets the persona's `vault`
    /// (#5240).
    pub names_no_vault: bool,
    /// Why the key is asked for though the vault has it (#5075): the chat says its value doesn't
    /// work, or it is a link whose entry is gone. None for a key the vault doesn't hold.
    pub reason: Option<Reason>,
    /// **Why the run that missed it is offered no Allow** (ADR 0112, #5243): the app's own words
    /// on what policy locks, said on the notice, where a Fill is still offered.
    pub no_allow: Option<String>,
}

/// The longest purpose a Secret ask carries, in characters: about a line of the ask.
pub const MOST_PURPOSE_CHARS: usize = 200;

/// **A chat's purpose as an ask carries it** (#5072, #5078): one line, at most
/// [`MOST_PURPOSE_CHARS`], and masked as a report's text is
/// ([`crate::dispatchrecord::masked`]): each vault value this process knows and each
/// credential-shaped stretch, so an ask cannot carry a secret into the window, the trace, a
/// proposed need or the audit.
pub fn purpose_said(purpose: &str) -> String {
    crate::dispatchrecord::masked(&crate::shown::one_line(purpose.trim(), MOST_PURPOSE_CHARS))
}

/// **Why a Secret ask is raised for a key the vault has** (#5075). Shown to the person alone:
/// the entry a link reads is never told to a chat. Kept across a restart as [`KEPT_IN`] keeps
/// the ask, by names only, as the links index already names them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    /// **The chat says the value doesn't work**, in its own words (`said`, one line, masked).
    /// `reads` is the entry the key reads where it is a Linked entry: a new value goes there, and
    /// reaches every key linked to it.
    Replace {
        said: String,
        reads: Option<super::links::Target>,
    },
    /// **The key is a Linked entry whose entry is gone** (`was`): a value or another entry
    /// mends it.
    BrokenLink { was: super::links::Target },
}

impl Reason {
    /// A replace of `key` in `vault`, with the chat's words `said`: one line and masked, as a
    /// report's text is, and naming the entry the key reads where it is a link.
    pub fn replace(ctx: &Ctx, vault: &str, key: &str, said: &str) -> Self {
        Self::Replace {
            // One line: what the chat wrapped is folded, never shown escaped.
            said: crate::dispatchrecord::masked(&crate::shown::one_line(
                &said.split_whitespace().collect::<Vec<_>>().join(" "),
                MOST_REASON_CHARS,
            )),
            reads: super::links::of(ctx, vault, key).ok().flatten(),
        }
    }

    /// The broken link `key` in `vault` is, naming the entry that is gone; none where it is no
    /// link.
    pub fn broken(ctx: &Ctx, vault: &str, key: &str) -> Option<Self> {
        super::links::of(ctx, vault, key)
            .ok()
            .flatten()
            .map(|was| Self::BrokenLink { was })
    }

    /// What the person reads of it, for key `key`: the chat's words marked as the chat's, or the
    /// entry that is gone.
    pub fn said(&self, key: &str) -> String {
        match self {
            Self::Replace { said, .. } => {
                format!("the chat says the current value doesn't work: {said}")
            }
            Self::BrokenLink { was } => format!(
                "{key} was linked to {} in vault {}, which is gone",
                was.key, was.vault
            ),
        }
    }
}

/// The longest reason a chat gives for a replace, in characters: about a line of the ask.
pub const MOST_REASON_CHARS: usize = 200;

/// **An open Secret ask, as the window shows it**: names only, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// Its number in the project: what an answer names.
    pub id: u32,
    pub persona: String,
    pub vault: String,
    pub key: String,
    pub tool: Option<String>,
    /// The chat's own words for what it needs the key for, where one said ([`Need::purpose`]).
    pub purpose: Option<String>,
    /// What the persona's definition says the key is for, where it declares it
    /// ([`Need::declared`]).
    pub declared: Option<String>,
    /// The persona names no vault ([`Need::names_no_vault`]).
    pub names_no_vault: bool,
    /// Why it is asked for though the vault has it (#5075).
    pub reason: Option<Reason>,
    /// Why the run that missed it is offered no Allow ([`Need::no_allow`]).
    pub no_allow: Option<String>,
    /// Each chat that needs it, the first to ask first: those holding a command, and those
    /// whose command stopped waiting and will be nudged.
    pub chats: Vec<u32>,
    /// Each chat on a runner that cannot receive the value, which asked for it and was told
    /// it was declined: a fill is not for it, and it is not nudged (#5077).
    pub cannot_receive: Vec<u32>,
    /// How many chats' starts asked for it, which have no number yet (#5074).
    pub starts: usize,
    /// When it was raised, in seconds since 1970.
    pub since: u64,
}

impl Open {
    /// How many chats wait on it.
    pub fn waiting(&self) -> usize {
        self.chats.len() + self.starts
    }
}

/// What a command that missed a key comes to ([`Asks::hold`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// The person filled it: read the vault again. Under the vault and key it was filled as,
    /// which the person may have chosen in place of the asked ones (#5072).
    Filled { vault: String, key: String },
    /// The person declined it, now or earlier in this chat. `line` is what the chat is told.
    Declined { line: String },
    /// Nobody answered before the hold ran out: the ask stays open and the chat is nudged when
    /// it is answered. `line` is what the chat is told.
    StillOpen { line: String },
    /// Policy refuses asks for a key the persona does not declare (#5078): nothing was asked,
    /// and `line` is what the chat is told.
    Refused { line: String },
    /// The command's caller hung up while it waited ([`Asks::hold_while`]): it stopped waiting,
    /// and the ask went where nobody else needs it. `line` is what the chat would be told.
    CallerGone { line: String },
}

/// **Why [`Asks::read_asking`] read nothing.** Its `Display` is what the asker is told.
#[derive(Debug)]
pub enum Unread {
    /// The vault refused the read for another reason than a missing key, or the key was still
    /// missing once filled: as it refused it.
    Vault(super::VaultError),
    /// The person declined, now or earlier in this chat, or did not answer in time: what
    /// [`Held`] says.
    Asked(String),
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vault(e) => e.fmt(f),
            Self::Asked(line) => f.write_str(line),
        }
    }
}

/// What a chat whose ask policy refused hears (#5078).
fn refused_said(vault: &str, key: &str) -> String {
    format!(
        "declined {}: policy turns off asks for a secret your persona does not declare",
        vault_and_key(vault, key)
    )
}

/// Why a decline policy made is said to be one (#5078).
pub const DECLINED_BY_POLICY: &str = "policy";

/// Told each step of an ask's life, for the event log (#5078): the chat, or none for an ask
/// only a chat's start waits on (#5074), and the step.
pub type Telling =
    std::sync::Arc<dyn Fn(Option<u32>, crate::eventlog::secretasks::Step) + Send + Sync>;

/// **Told that the Run grants moved** (#5800) some way the window did not make: an Allow once
/// taken by its run or dropped unused, or a chat that held one closed.
pub type RunGrantsMoved = std::sync::Arc<dyn Fn() + Send + Sync>;

/// The person's answer to a Secret ask.
#[derive(Debug)]
pub enum Answer {
    /// A new value, written into the vault under the asked key. Wiped as it is dropped.
    Fill(Zeroizing<String>),
    /// **An entry the person keeps already** (#5073): the asked key becomes a Linked entry
    /// reading it ([`super::links::link`]), so the value is never copied and a rotation of the
    /// entry reaches the key. The chat hears filled and the key's name, never the entry.
    Link(super::links::Target),
    /// Not now.
    Decline,
}

/// What an answer did: the chats to nudge, whose command stopped waiting before it, and what
/// each is told ([`crate::askoutcome::Outcome::line`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub vault: String,
    pub key: String,
    pub filled: bool,
    /// The persona the ask was raised for.
    pub persona: String,
    /// Whether the persona declares the asked key as a Secret need: Remember adds one it does
    /// not (#5240).
    pub declared: bool,
    /// The persona names no vault: Remember sets its `vault` to [`Self::vault`] (#5240).
    pub names_no_vault: bool,
    /// The vault was made by this fill, on the default provider (#5076).
    pub made_vault: bool,
    /// The first chat that asked, where one did: named on anything held from this answer (#5240).
    pub asker: Option<u32>,
    pub nudge: Vec<u32>,
    pub told: crate::askoutcome::Outcome,
}

/// How a waiter is settled.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Settled {
    /// Filled, as this vault and key.
    Filled {
        vault: String,
        key: String,
    },
    Declined,
}

/// **Where the person stores a fill** (#5072): the asked vault and key, or another vault or
/// key name they chose before filling. The chat hears the name it ends up under.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Named {
    /// Another vault of the project's, where the person chose one.
    pub vault: Option<String>,
    /// Another key name, where the person renamed it.
    pub key: Option<String>,
}

impl Named {
    /// As the ask asked.
    pub fn as_asked() -> Self {
        Self::default()
    }
}

#[derive(Debug)]
struct Waiter {
    /// None for a chat's start (#5074).
    chat: Option<u32>,
    /// Where its command hears the answer; none once it stopped waiting, and is nudged instead.
    tell: Option<mpsc::Sender<Settled>>,
    /// Which of the ask's waiters it is, to stop waiting by.
    n: u64,
}

#[derive(Debug)]
struct Asked {
    id: u32,
    persona: String,
    tool: Option<String>,
    purpose: Option<String>,
    declared: Option<String>,
    names_no_vault: bool,
    reason: Option<Reason>,
    no_allow: Option<String>,
    since: u64,
    waiters: Vec<Waiter>,
    /// The runner chats that cannot receive the value ([`Open::cannot_receive`]).
    cannot_receive: Vec<u32>,
    /// An answer is being written: a second is refused, as any ask's second answer is.
    answering: bool,
}

impl Asked {
    /// Whether anything still needs it: a chat waiting, a runner chat that asked, or an answer
    /// being written.
    fn needed(&self) -> bool {
        !self.waiters.is_empty() || !self.cannot_receive.is_empty() || self.answering
    }
}

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    next_waiter: u64,
    /// Each open ask, by its vault and key.
    open: HashMap<(String, String), Asked>,
    /// Each chat's declines, by vault and key, for the chat's life.
    declined: HashSet<(u32, String, String)>,
    /// The asks to allow a program, and the Run grants given ([`run`], ADR 0112).
    runs: run::Runs,
}

/// **The Secret asks one project holds open** (module docs).
pub struct Asks {
    root: PathBuf,
    inner: Mutex<Inner>,
    /// Where they are kept across a restart; nowhere for [`Asks::new`]'s.
    kept_at: Option<PathBuf>,
    /// Told each step, where the app records them ([`Self::telling`]).
    told: Mutex<Option<Telling>>,
    /// Told that the Run grants moved ([`Self::telling_run_grants_moved`]).
    grants_moved: Mutex<Option<RunGrantsMoved>>,
}

impl std::fmt::Debug for Asks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Asks")
            .field("root", &self.root)
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

/// **The open asks as [`KEPT_IN`] holds them**: names and chats, never a value.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Kept {
    v: u32,
    /// The last number an ask was given, so a number is never given twice.
    asked: u32,
    asks: Vec<KeptAsk>,
    /// The open asks to allow a program ([`run`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    runs: Vec<run::OpenRun>,
}

/// One open ask as it is kept: each name masked as an open question is.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct KeptAsk {
    id: u32,
    persona: String,
    vault: String,
    key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    /// What the chat said it is for, as its ask shows it: masked already (#5072).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    purpose: Option<String>,
    /// What the persona's definition says the key is for; none where it does not declare it
    /// (#5076).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    declared: Option<String>,
    /// Whether the chat named no vault, so the persona's own was asked (#5072).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    names_no_vault: bool,
    since: u64,
    /// The chats that wait on it, by the app's number, which a restart keeps.
    chats: Vec<u32>,
    /// The runner chats that cannot receive it, by number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cannot_receive: Vec<u32>,
    /// How many chats' starts wait on it, which have no number (#5074).
    #[serde(default, skip_serializing_if = "is_zero")]
    starts: usize,
    /// Why it is asked though the vault has the key: a replace or a broken link (#5075).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<Reason>,
    /// Why the run that missed it is offered no Allow (#5243).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    no_allow: Option<String>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl KeptAsk {
    /// `asked`, as it is written: none where a name of it reads as a secret, since masked it
    /// would name another key, and its answer would be written under that one.
    fn of(vault: &str, key: &str, asked: &Asked) -> Option<Self> {
        let kept = |name: &str| {
            let masked = crate::dispatchrecord::masked(name);
            (masked == name).then_some(masked)
        };
        Some(Self {
            id: asked.id,
            persona: kept(&asked.persona)?,
            vault: kept(vault)?,
            key: kept(key)?,
            tool: match &asked.tool {
                Some(tool) => Some(kept(tool)?),
                None => None,
            },
            // Words, not names: masked as they are, since nothing is written under them.
            purpose: asked.purpose.as_deref().map(crate::dispatchrecord::masked),
            declared: asked.declared.as_deref().map(crate::dispatchrecord::masked),
            names_no_vault: asked.names_no_vault,
            since: asked.since,
            chats: chats_of(asked),
            cannot_receive: asked.cannot_receive.clone(),
            starts: starts_of(asked),
            reason: asked.reason.clone(),
            no_allow: asked.no_allow.clone(),
        })
    }

    /// Whether every name reads as a name: one line, not empty.
    fn reads(&self) -> bool {
        let name = |name: &str| !name.is_empty() && !name.chars().any(char::is_control);
        [&self.persona, &self.vault, &self.key]
            .into_iter()
            .chain(&self.tool)
            .all(|one| name(one))
            && !(self.chats.is_empty() && self.cannot_receive.is_empty() && self.starts == 0)
    }
}

/// How long a command waits on the person at most, by the harness the app started its chat on
/// ([`holds_for`]): three quarters of the harness's own command timeout, so the command ends
/// saying why before the harness kills it.
fn command_timeout(harness: Option<Harness>) -> Duration {
    match harness {
        // Claude Code's Bash tool and opencode's bash tool give a command two minutes unless the
        // model asks for more.
        Some(Harness::ClaudeCode | Harness::Opencode) => Duration::from_secs(120),
        // Codex's shell tool gives one ten seconds unless the model asks for more. A chat whose
        // harness is not known is held to the shortest.
        Some(Harness::Codex) | None => Duration::from_secs(10),
    }
}

/// **How long a brokered command of a chat on `harness` waits on a Secret ask**: three quarters
/// of that harness's default command timeout. A model that gave its command longer only sees it
/// end sooner than it had to, saying why; one that gave it less was told the timeout it set.
pub fn holds_for(harness: Option<Harness>) -> Duration {
    command_timeout(harness) * 3 / 4
}

fn vault_and_key(vault: &str, key: &str) -> String {
    format!("{vault}/{key}")
}

/// What a chat whose ask was declined hears.
pub(crate) fn declined_said(vault: &str, key: &str) -> String {
    format!("declined {}", vault_and_key(vault, key))
}

/// What a chat on a runner that cannot receive a value hears (#5077).
fn cannot_receive_said(vault: &str, key: &str) -> String {
    format!(
        "{}: runner can't receive secrets",
        declined_said(vault, key)
    )
}

/// What a chat whose command stopped waiting hears: the ask stays open where it was asked.
fn still_open_said(root: &Path, vault: &str, key: &str) -> String {
    format!(
        "waiting on you for {}; {}; retry once answered",
        vault_and_key(vault, key),
        crate::askwait::still_answerable_said(root)
    )
}

/// What a command whose caller hung up while it waited is answered: nobody reads it.
fn caller_gone_said(vault: &str, key: &str) -> String {
    format!(
        "the call ended while the person was asked for {}, so it stopped waiting",
        vault_and_key(vault, key)
    )
}

/// What a chat whose runner could not be confirmed as the person's own just now hears.
fn unconfirmed_said(vault: &str, key: &str) -> String {
    format!(
        "waiting on you for {}, and the runner was unreachable just now; retry once answered",
        vault_and_key(vault, key)
    )
}

/// What a command is told as it starts to wait. Never the entry a link reads.
fn asked_said(
    root: &Path,
    vault: &str,
    key: &str,
    reason: Option<&Reason>,
    within: Duration,
) -> String {
    let why = match reason {
        None => "which the vault doesn't hold",
        Some(Reason::Replace { .. }) => "whose value you said doesn't work",
        Some(Reason::BrokenLink { .. }) => "a linked entry whose entry is gone",
    };
    format!(
        "purlis asked the person for {}, {why}; {}",
        vault_and_key(vault, key),
        crate::askwait::waiting_said(root, within)
    )
}

/// How a chat whose command is not held on its ask stands to it (#5077).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unheld {
    /// It waits on it, and is nudged on its answer.
    Waits,
    /// Its runner cannot receive a value: it was told it was declined.
    CannotReceive,
}

/// Whether policy's lock refuses asking for a key its persona does not declare (#5078).
/// **What a run's notice could not ask for `key` of `vault`** (#5242): policy turns off asks for a
/// key its persona does not declare, which [`Asks::hold`] refuses with these words.
pub(crate) fn refused_by_policy(
    root: &Path,
    vault: &str,
    key: &str,
    declared: Option<&String>,
) -> Option<String> {
    declined_by_policy(root, declared).then(|| refused_said(vault, key))
}

fn declined_by_policy(root: &Path, declared: Option<&String>) -> bool {
    declared.is_none() && crate::sandbox::policy::Locks::of(root).forbids_undeclared_secret_asks()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

impl Asks {
    /// The Secret asks of the project at `root`, where their trace is kept.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            inner: Mutex::new(Inner::default()),
            kept_at: None,
            told: Mutex::new(None),
            grants_moved: Mutex::new(None),
        }
    }

    /// **Tells `told` each step of every ask's life from now on** (#5078): what the app writes
    /// to the event log, the audit's and telemetry's source. Counts and closed words alone
    /// ([`crate::eventlog::secretasks::Step`]).
    pub fn telling(&self, told: Telling) {
        *self.told.lock().unwrap_or_else(PoisonError::into_inner) = Some(told);
    }

    /// **Tells `moved` each time the Run grants move** some way the window did not make (#5800):
    /// an Allow once taken by its run or dropped unused, or a chat that held a grant closed. Told
    /// off the asks' lock, so it may read them again at once.
    pub fn telling_run_grants_moved(&self, moved: RunGrantsMoved) {
        *self
            .grants_moved
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(moved);
    }

    /// Tells that the Run grants moved, where anything listens. Never under the asks' lock.
    fn run_grants_moved(&self) {
        let moved = self
            .grants_moved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(moved) = moved {
            moved();
        }
    }

    /// Tells the step of chat `chat`'s, where anything listens.
    fn tell(&self, chat: Option<u32>, step: crate::eventlog::secretasks::Step) {
        let told = self
            .told
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(told) = told {
            told(chat, step);
        }
    }

    /// **The Secret asks of the project at `root`, kept across a restart** (module docs): those
    /// a run before this one left open, each chat that waited on one to be nudged once it is
    /// answered. A file that does not read starts with none, and an entry that does not read is
    /// left out, each said in the log.
    pub fn kept(root: &Path) -> Self {
        let path = crate::names::state(root).join("app").join(KEPT_IN);
        let mut inner = Inner::default();
        let kept = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Kept>(&bytes).unwrap_or_else(|why| {
                tracing::warn!(
                    "purlis: {} does not read as the open secret asks ({why}); none is open",
                    path.display()
                );
                Kept::default()
            }),
            Err(_) => Kept::default(),
        };
        inner.next = kept.asked;
        for one in kept.runs {
            inner.runs.keep(one);
        }
        inner.next = inner.next.max(inner.runs.highest());
        for one in kept.asks {
            let at = (one.vault.clone(), one.key.clone());
            if !one.reads() {
                tracing::warn!(
                    "purlis: an open secret ask kept in {} does not read; it is left out",
                    path.display()
                );
                continue;
            }
            if inner.open.values().any(|asked| asked.id == one.id) {
                tracing::warn!(
                    "purlis: {} keeps a second open secret ask numbered {}; only the first is \
                     open",
                    path.display(),
                    one.id
                );
                continue;
            }
            if inner.open.contains_key(&at) {
                tracing::warn!(
                    "purlis: {} keeps a second open secret ask for one vault and key; only the \
                     first is open",
                    path.display()
                );
                continue;
            }
            inner.next = inner.next.max(one.id);
            let mut waiters = Vec::new();
            // A start kept waits on nothing either: its person was told to start it again.
            let starts = std::iter::repeat_n(None, one.starts);
            for chat in one.chats.into_iter().map(Some).chain(starts) {
                inner.next_waiter += 1;
                waiters.push(Waiter {
                    chat,
                    tell: None,
                    n: inner.next_waiter,
                });
            }
            inner.open.insert(
                at,
                Asked {
                    id: one.id,
                    persona: one.persona,
                    tool: one.tool,
                    purpose: one.purpose,
                    declared: one.declared,
                    names_no_vault: one.names_no_vault,
                    reason: one.reason,
                    no_allow: one.no_allow,
                    since: one.since,
                    waiters,
                    cannot_receive: one.cannot_receive,
                    answering: false,
                },
            );
        }
        Self {
            root: root.to_path_buf(),
            inner: Mutex::new(inner),
            kept_at: Some(path),
            told: Mutex::new(None),
            grants_moved: Mutex::new(None),
        }
    }

    /// Writes the asks open in `inner` where [`Asks::kept`] keeps them, under its lock, so two
    /// writes land in the order their changes were made.
    fn save(&self, inner: &Inner) {
        let Some(path) = &self.kept_at else { return };
        let mut asks: Vec<KeptAsk> = Vec::new();
        for ((vault, key), asked) in &inner.open {
            match KeptAsk::of(vault, key, asked) {
                Some(one) => asks.push(one),
                None => tracing::info!(
                    "purlis: an open secret ask whose names read as a secret is not kept across \
                     a restart"
                ),
            }
        }
        asks.sort_by_key(|one| one.id);
        let kept = Kept {
            v: KEPT_VERSION,
            asked: inner.next,
            asks,
            runs: inner.runs.kept(),
        };
        let written = serde_json::to_vec_pretty(&kept)
            .map_err(std::io::Error::other)
            .and_then(|mut bytes| {
                bytes.push(b'\n');
                let dir = path.parent().unwrap_or(Path::new("."));
                std::fs::create_dir_all(dir)?;
                crate::rewrite::replace(dir, path, &bytes, crate::rewrite::Mode::Private)
            });
        if let Err(why) = written {
            tracing::warn!(
                "purlis: the open secret asks were not written ({why}); they are kept in memory \
                 until the next write"
            );
        }
    }

    /// **A launch brought back the chats `back`, by number** (#5079): a chat waiting on a kept
    /// ask that is not among them waits on nothing, and an ask nobody waits on goes. A chat's
    /// start, which has no number, is no chat a launch brings back, and stays (#5074).
    pub fn only_these_came_back(&self, back: &[u32]) {
        let mut inner = self.inner();
        inner.open.retain(|_, asked| {
            asked
                .waiters
                .retain(|waiter| waiter.chat.is_none_or(|chat| back.contains(&chat)));
            asked.cannot_receive.retain(|chat| back.contains(chat));
            asked.needed()
        });
        let before = inner.runs.given_count();
        inner.runs.retain_chats(|chat| back.contains(&chat));
        let moved = inner.runs.given_count() != before;
        self.save(&inner);
        drop(inner);
        // The Allowed list may have read them before the launch settled (#5876's review).
        if moved {
            self.run_grants_moved();
        }
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records `what` of the ask for `vault`/`key` in chat `chat`'s trace, a start's under
    /// [`crate::trace::NO_SESSION`]: names, never a value.
    fn trace(&self, chat: Option<u32>, what: &str, vault: &str, key: &str, waiting: usize) {
        let session = chat.map_or_else(|| crate::trace::NO_SESSION.to_owned(), |c| c.to_string());
        crate::trace::record_values(
            &self.root,
            &session,
            TRACED_AS,
            &[
                ("what", Value::from(what)),
                ("vault", Value::from(vault)),
                ("key", Value::from(key)),
                ("chat", Value::from(chat)),
                ("waiting", Value::from(waiting)),
            ],
            chrono::Local::now().naive_local(),
        );
    }

    /// **Asks the person for `need` and holds its command at most `within`** (module docs):
    /// raised, or joined where its vault and key are asked already; or declined at once, with
    /// nothing asked, where this chat declined it before. `say` tells the command, as it starts
    /// to wait, what it waits on.
    pub fn hold(&self, need: Need, within: Duration, say: &mut dyn FnMut(&str)) -> Held {
        self.hold_while(need, within, &|| false, say)
    }

    /// **[`Asks::hold`], for a command whose caller may hang up**: it looks every
    /// [`crate::askwait::LOOKS_AGAIN`] whether its caller is `gone`, and where it is, it stops
    /// waiting at once and is no chat to nudge, and an ask nobody else needs goes from the
    /// person's window ([`Held::CallerGone`]).
    pub fn hold_while(
        &self,
        need: Need,
        within: Duration,
        gone: &dyn Fn() -> bool,
        say: &mut dyn FnMut(&str),
    ) -> Held {
        let Need {
            chat,
            persona,
            vault,
            key,
            tool,
            purpose,
            declared,
            names_no_vault,
            reason,
            no_allow,
        } = need;
        let said_why = reason.clone();
        let (answered, n) = {
            let mut inner = self.inner();
            if let Some(chat) = chat
                && inner.declined.contains(&(chat, vault.clone(), key.clone()))
            {
                return Held::Declined {
                    line: declined_said(&vault, &key),
                };
            }
            // Policy's lock on undeclared keys (#5078): refused here, never shown to the person.
            if declined_by_policy(&self.root, declared.as_ref()) {
                drop(inner);
                self.trace(chat, "refused", &vault, &key, 0);
                self.tell(
                    chat,
                    crate::eventlog::secretasks::Step::Refused {
                        by: crate::eventlog::secretasks::By::Policy,
                        declared: false,
                    },
                );
                return Held::Refused {
                    line: refused_said(&vault, &key),
                };
            }
            inner.next_waiter += 1;
            let n = inner.next_waiter;
            let (tell, answered) = mpsc::channel();
            let waiter = Waiter {
                chat,
                tell: Some(tell),
                n,
            };
            let at = (vault.clone(), key.clone());
            let is_declared = declared.is_some();
            let mut raised_id = 0;
            let joined = if let Some(asked) = inner.open.get_mut(&at) {
                // A chat that tries again waits again: a command of its that stopped waiting is
                // no longer one to nudge, since this one hears the answer.
                asked
                    .waiters
                    .retain(|one| one.chat != chat || one.tell.is_some());
                asked.waiters.push(waiter);
                // A chat that says what for, joining a broker miss that could not, says it.
                if asked.purpose.is_none() {
                    asked.purpose = purpose;
                }
                // The reason is the raiser's: a chat joining never turns an ask the person may be
                // reading into another kind (a replace may delete a value).
                drop(reason);
                Some((asked.id, waiting_on(asked)))
            } else {
                inner.next += 1;
                let id = inner.next;
                raised_id = id;
                inner.open.insert(
                    at,
                    Asked {
                        id,
                        persona,
                        tool,
                        purpose,
                        declared,
                        names_no_vault,
                        reason,
                        no_allow,
                        since: now_secs(),
                        waiters: vec![waiter],
                        cannot_receive: Vec::new(),
                        answering: false,
                    },
                );
                None
            };
            self.save(&inner);
            use crate::eventlog::secretasks::Step;
            // Told before the lock goes, so no answer to it is told first: the event log's
            // order is the ask's.
            self.tell(
                chat,
                match joined {
                    Some((ask, waiting)) => Step::Joined {
                        ask,
                        waiting: u32::try_from(waiting).unwrap_or(u32::MAX),
                    },
                    None => Step::Raised {
                        ask: raised_id,
                        declared: is_declared,
                    },
                },
            );
            // Traced before the lock goes too, for the same reason (#5220): an answer's trace
            // is written only once the ask is taken, under the lock.
            match joined {
                Some((_, waiting)) => self.trace(chat, "joined", &vault, &key, waiting),
                None => self.trace(chat, "raised", &vault, &key, 1),
            }
            drop(inner);
            (answered, n)
        };
        say(&asked_said(
            &self.root,
            &vault,
            &key,
            said_why.as_ref(),
            within,
        ));
        let settled_as = |settled: Settled| match settled {
            Settled::Filled { vault, key } => Held::Filled { vault, key },
            Settled::Declined => Held::Declined {
                line: declined_said(&vault, &key),
            },
        };
        let at = (vault.clone(), key.clone());
        // What the last look found, so `settle` knows which way the wait ends.
        let hung_up = std::cell::Cell::new(false);
        let looked = || {
            let now = gone();
            hung_up.set(now);
            now
        };
        // Stops waiting, unless it was answered in that very moment, which wins. Its caller gone,
        // it leaves in the same hold of the lock, so no answer finds it as one to nudge.
        let settle = || {
            let mut inner = self.inner();
            if hung_up.get() {
                let waiting = left(&mut inner, &at, n);
                if waiting.is_some() {
                    self.save(&inner);
                }
                drop(inner);
                if let Some(waiting) = waiting {
                    self.trace(chat, "caller gone", &vault, &key, waiting);
                    return None;
                }
            } else {
                let still = inner
                    .open
                    .get_mut(&at)
                    .and_then(|asked| asked.waiters.iter_mut().find(|w| w.n == n))
                    .map(|waiter| waiter.tell = None)
                    .is_some();
                drop(inner);
                if still {
                    return None;
                }
            }
            answered.try_recv().ok()
        };
        match crate::askwait::waited(&answered, within, &looked, &settle) {
            crate::askwait::Ended::Heard(settled) => settled_as(settled),
            // Its chat closed meanwhile, or the hold ran out: the ask stays open, and the chat
            // is nudged when it is answered.
            crate::askwait::Ended::Closed | crate::askwait::Ended::NotAnswered => Held::StillOpen {
                line: still_open_said(&self.root, &vault, &key),
            },
            crate::askwait::Ended::AskerGone => Held::CallerGone {
                line: caller_gone_said(&vault, &key),
            },
        }
    }

    /// **A chat on a runner that cannot receive a value misses `need`'s key** (#5077): the
    /// person is asked for it all the same, raised or joined as any ask is, so they may fill it
    /// for this machine's chats; the chat waits on nothing and is told at once that it was
    /// declined, and why. Answers what it is told: refused instead, with nothing asked, where
    /// policy forbids asking for a key its persona does not declare, as [`Asks::hold`] is.
    pub fn cannot_receive(&self, need: Need) -> String {
        let (vault, key) = (need.vault.clone(), need.key.clone());
        match self.unheld(need, Unheld::CannotReceive) {
            Ok(()) => cannot_receive_said(&vault, &key),
            Err(line) => line,
        }
    }

    /// **A chat on a runner that could not be confirmed as the person's own just now misses
    /// `need`'s key** (#5077): asked as any ask is, raised or joined, and the chat waits on it
    /// as one whose command stopped waiting does: nudged on its answer, and nothing declined. It
    /// is not held, since no value could go to its runner now. Answers what it is told: that it
    /// waits; or that it was declined or refused (`Err`), with nothing asked, as by
    /// [`Asks::hold`].
    pub fn unconfirmed(&self, need: Need) -> Result<String, String> {
        let (vault, key) = (need.vault.clone(), need.key.clone());
        self.unheld(need, Unheld::Waits)?;
        Ok(unconfirmed_said(&vault, &key))
    }

    /// Raises or joins `need`'s ask for a chat whose command is not held on it (#5077), as
    /// `how` says, saved, traced and told as [`Asks::hold`]'s are. Refused, what the chat is
    /// told: a decline it made before (a chat that waits), or policy's lock on an undeclared
    /// key.
    fn unheld(&self, need: Need, how: Unheld) -> Result<(), String> {
        use crate::eventlog::secretasks::{By, Step};
        let Need {
            chat,
            persona,
            vault,
            key,
            tool,
            purpose,
            declared,
            names_no_vault,
            reason,
            no_allow,
        } = need;
        let mut inner = self.inner();
        if how == Unheld::Waits
            && let Some(chat) = chat
            && inner.declined.contains(&(chat, vault.clone(), key.clone()))
        {
            return Err(declined_said(&vault, &key));
        }
        if declined_by_policy(&self.root, declared.as_ref()) {
            drop(inner);
            self.trace(chat, "refused", &vault, &key, 0);
            self.tell(
                chat,
                Step::Refused {
                    by: By::Policy,
                    declared: false,
                },
            );
            return Err(refused_said(&vault, &key));
        }
        inner.next_waiter += 1;
        let n = inner.next_waiter;
        // A runner chat always has its number; a start never reaches a runner.
        let (waiter, cannot) = match how {
            Unheld::Waits => (
                Some(Waiter {
                    chat,
                    tell: None,
                    n,
                }),
                Vec::new(),
            ),
            Unheld::CannotReceive => (None, chat.into_iter().collect::<Vec<u32>>()),
        };
        let at = (vault.clone(), key.clone());
        let is_declared = declared.is_some();
        let joined = if let Some(asked) = inner.open.get_mut(&at) {
            if let Some(waiter) = waiter
                && !asked.waiters.iter().any(|one| one.chat == waiter.chat)
            {
                asked.waiters.push(waiter);
            }
            for chat in cannot {
                if !asked.cannot_receive.contains(&chat) {
                    asked.cannot_receive.push(chat);
                }
            }
            if asked.purpose.is_none() {
                asked.purpose = purpose;
            }
            Ok((asked.id, waiting_on(asked)))
        } else {
            inner.next += 1;
            let id = inner.next;
            inner.open.insert(
                at,
                Asked {
                    id,
                    persona,
                    tool,
                    purpose,
                    declared,
                    names_no_vault,
                    reason,
                    no_allow,
                    since: now_secs(),
                    waiters: waiter.into_iter().collect(),
                    cannot_receive: cannot,
                    answering: false,
                },
            );
            Err(id)
        };
        self.save(&inner);
        // Told before the lock goes, as [`Asks::hold`]'s are.
        self.tell(
            chat,
            match joined {
                Ok((ask, waiting)) => Step::Joined {
                    ask,
                    waiting: u32::try_from(waiting).unwrap_or(u32::MAX),
                },
                Err(ask) => Step::Raised {
                    ask,
                    declared: is_declared,
                },
            },
        );
        // And traced before it goes, as [`Asks::hold`]'s are (#5220).
        let said = match how {
            Unheld::Waits => "runner unconfirmed",
            Unheld::CannotReceive => "cannot receive",
        };
        match joined {
            Ok((_, waiting)) => {
                self.trace(chat, &format!("joined, {said}"), &vault, &key, waiting);
            }
            Err(_) => self.trace(chat, &format!("raised, {said}"), &vault, &key, 1),
        }
        drop(inner);
        Ok(())
    }

    /// **Reads `need`'s key with `read`, asking the person where the vault holds none** (#5074):
    /// what a browser login's and a profile's credential read with, so each asks as a brokered
    /// run does. `read` is the read of exactly that key, and is read once more once the person
    /// fills it; a key still missing then is refused as the vault refuses it, never asked
    /// twice. Any other refusal of the vault's asks nothing. `within` and `say` are
    /// [`Asks::hold`]'s.
    pub fn read_asking<T>(
        &self,
        need: Need,
        within: Duration,
        say: &mut dyn FnMut(&str),
        read: &mut dyn FnMut() -> Result<T, super::VaultError>,
    ) -> Result<T, Unread> {
        match read() {
            // A key the vault doesn't hold, or a link whose entry is gone (#5075).
            Err(e) if matches!(e.kind, super::Kind::NotFound | super::Kind::BrokenLink) => {}
            got => return got.map_err(Unread::Vault),
        }
        match self.hold(need, within, say) {
            Held::Filled { .. } => read().map_err(Unread::Vault),
            Held::Declined { line }
            | Held::StillOpen { line }
            | Held::Refused { line }
            | Held::CallerGone { line } => Err(Unread::Asked(line)),
        }
    }

    /// **Every Secret ask open now**, longest waiting first.
    pub fn open(&self) -> Vec<Open> {
        let inner = self.inner();
        let mut open: Vec<Open> = inner
            .open
            .iter()
            .map(|((vault, key), asked)| Open {
                id: asked.id,
                persona: asked.persona.clone(),
                vault: vault.clone(),
                key: key.clone(),
                tool: asked.tool.clone(),
                purpose: asked.purpose.clone(),
                declared: asked.declared.clone(),
                names_no_vault: asked.names_no_vault,
                reason: asked.reason.clone(),
                no_allow: asked.no_allow.clone(),
                chats: chats_of(asked),
                cannot_receive: asked.cannot_receive.clone(),
                starts: starts_of(asked),
                since: asked.since,
            })
            .collect();
        open.sort_by_key(|one| (one.since, one.id));
        open
    }

    /// **The person answers Secret ask `id`**, from `by`, in the project `ctx` is of: a fill is
    /// written through the vault's provider and then settles every chat waiting on the ask; a
    /// decline settles them all as declined, and each is not asked about that key again.
    /// Answered from the window alone: an answer from anywhere else is refused and leaves the
    /// ask open, and so does a fill the provider refuses.
    pub fn answer(
        &self,
        ctx: &Ctx,
        id: u32,
        answer: Answer,
        by: Answerer,
    ) -> Result<Answered, String> {
        self.answer_as(ctx, id, answer, Named::as_asked(), by)
    }

    /// Whether the person may link asked key `key` of vault `vault` to entry `to`, judged as the
    /// press comes and before anything is written. Where `into_too`, refused where `vault` is one
    /// the asking persona may not use, as [`fillable`] refuses a Fill (a vault the project does
    /// not have yet is left to the link's own write, as before). Always refused for an entry the
    /// run's picker would not list (D-5246-1, amending D-5080-11): one of a vault the persona may
    /// not use, or one no program is handed ([`super::brokered::NoProgram`]). A link is for good
    /// and every run reads through it, while the notice of a run names only the persona's vault.
    fn linkable(
        &self,
        ctx: &Ctx,
        vault: &str,
        key: &str,
        into_too: bool,
        to: &super::links::Target,
    ) -> Result<(), String> {
        let persona = self
            .inner()
            .open
            .get(&(vault.to_owned(), key.to_owned()))
            .map(|asked| asked.persona.clone())
            .unwrap_or_default();
        if into_too {
            let doc = super::registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
            if super::registry::vault_in(&doc, vault).is_ok() {
                super::brokered::authorise(ctx, Some(&persona), vault).map_err(|why| {
                    format!(
                        "{why} Allow it for {persona} first, or link the key into {persona}'s \
                         own vault."
                    )
                })?;
            }
        }
        if super::brokered::authorise(ctx, Some(&persona), &to.vault).is_err() {
            return Err(format!(
                "{persona} can't use vault {}, so its entries can't fill this ask. Pick another \
                 entry, or type a value.",
                to.vault
            ));
        }
        if let Some(why) = super::brokered::no_program_is_handed(ctx, &to.vault, &[&to.key]) {
            return Err(format!("{why}. Pick another entry, or type a value."));
        }
        // #5745: nor one another persona of the link's vault may not use, which its tools,
        // declared ones too, would read through the link.
        if let Some(why) = super::brokered::read_through_by(ctx, vault, &to.vault) {
            return Err(why);
        }
        Ok(())
    }

    /// **[`Self::answer`], storing a fill as `named`** (#5072): in another vault or under
    /// another key name, which the person chose before filling. Every chat that waits hears the
    /// name it is stored under. A vault the project does not have is made for the persona
    /// ([`MADE_ON`], #5076). Refused, leaving the ask open, for a key name no Secret ask takes,
    /// a vault the persona may not use, or a key the vault holds a value under already.
    pub fn answer_as(
        &self,
        ctx: &Ctx,
        id: u32,
        answer: Answer,
        named: Named,
        by: Answerer,
    ) -> Result<Answered, String> {
        if by.scope() != HumanScope::LocalUi {
            return Err("a secret ask is answered in purlis's window only".to_owned());
        }
        if matches!(&answer, Answer::Fill(value) if value.is_empty()) {
            return Err(
                "an empty value would read as a present one: type the value, or choose Not now"
                    .to_owned(),
            );
        }
        let (vault, key, reason) = {
            let mut inner = self.inner();
            let Some(((vault, key), asked)) = inner
                .open
                .iter_mut()
                .find(|(_, asked)| asked.id == id && !asked.answering)
            else {
                return Err(format!("secret ask {id} is no longer open"));
            };
            asked.answering = true;
            (vault.clone(), key.clone(), asked.reason.clone())
        };
        let still_open = || {
            if let Some(asked) = self.inner().open.get_mut(&(vault.clone(), key.clone())) {
                asked.answering = false;
            }
        };
        let into_vault = named.vault.clone().unwrap_or_else(|| vault.clone());
        let mut made_vault = false;
        let into_key = named.key.clone().unwrap_or_else(|| key.clone());
        let filled = match &answer {
            // A replace or a broken link is filled where it is asked (#5075): the key the person
            // was shown, never another name or vault, and past the "never over a value" rule,
            // which is what a replace is for.
            Answer::Fill(value) if reason.is_some() => {
                if into_vault != vault || into_key != key {
                    still_open();
                    return Err(format!(
                        "{} is asked to be replaced or mended where it is: fill it under that \
                         name, in that vault",
                        vault_and_key(&vault, &key)
                    ));
                }
                if let Err(e) = filled_with(ctx, &vault, &key, reason.as_ref(), value) {
                    still_open();
                    return Err(e.message.to_string());
                }
                true
            }
            Answer::Fill(value) => {
                if !super::needs::is_key(&into_key) {
                    still_open();
                    return Err(not_a_key(&into_key));
                }
                let persona = self
                    .inner()
                    .open
                    .get(&(vault.clone(), key.clone()))
                    .map(|asked| asked.persona.clone())
                    .unwrap_or_default();
                made_vault = match made_if_missing(ctx, &into_vault, &persona) {
                    Ok(made) => made,
                    Err(why) => {
                        still_open();
                        return Err(why);
                    }
                };
                // Only into a vault the persona may use, and never over a value already there:
                // an ask is for a key nobody holds, wherever the chat or the person named.
                if !made_vault && let Err(why) = fillable(ctx, &persona, &into_vault, &into_key) {
                    still_open();
                    return Err(why);
                }
                let written = cmd::provider(ctx, &into_vault)
                    .and_then(|v| cmd::set_value(ctx, &v, &into_key, value));
                if let Err(e) = written {
                    still_open();
                    return Err(e.message.to_string());
                }
                true
            }
            Answer::Link(to) => {
                // A link is made at the asked key: the window offers no other name for one.
                if into_vault != vault || into_key != key {
                    still_open();
                    return Err(format!(
                        "an entry is linked under the asked name, {}",
                        vault_and_key(&vault, &key)
                    ));
                }
                // Only into a vault the persona may use, as a Fill is (#5246's review): a link
                // planted in another persona's vault is read by that persona's tools. A replace
                // was judged so as it was asked. And only to an entry the run's picker would
                // list (D-5246-1), whatever the window listed.
                if let Err(why) = self.linkable(ctx, &vault, &key, reason.is_none(), to) {
                    still_open();
                    return Err(why);
                }
                if let Err(e) = linked_to(ctx, &vault, &key, reason.as_ref(), to) {
                    still_open();
                    return Err(e.message.to_string());
                }
                true
            }
            Answer::Decline => false,
        };
        let linked = matches!(answer, Answer::Link(_));
        // A decline stores nothing, so the chat hears the names it asked for.
        let (vault, key, asked_at) = if filled {
            (into_vault, into_key, (vault, key))
        } else {
            (vault.clone(), key.clone(), (vault, key))
        };
        drop(answer);
        let mut inner = self.inner();
        let Some(asked) = inner.open.remove(&asked_at) else {
            return Err(format!("secret ask {id} is no longer open"));
        };
        let chats = chats_of(&asked);
        let asked_cannot_receive = asked.cannot_receive.clone();
        let started = asked.waiters.iter().any(|waiter| waiter.chat.is_none());
        let waiting = waiting_on(&asked);
        let settled = if filled {
            Settled::Filled {
                vault: vault.clone(),
                key: key.clone(),
            }
        } else {
            Settled::Declined
        };
        let mut nudge = Vec::new();
        for waiter in asked.waiters {
            let told = waiter
                .tell
                .is_some_and(|tell| tell.send(settled.clone()).is_ok());
            // A start that stopped waiting has nobody to nudge: its refusal told the person to
            // start it again.
            if let Some(chat) = waiter.chat
                && !told
                && !nudge.contains(&chat)
            {
                nudge.push(chat);
            }
        }
        if !filled {
            for &chat in &chats {
                inner.declined.insert((chat, vault.clone(), key.clone()));
            }
        }
        self.save(&inner);
        // Traced and told before the lock goes (#5220): a chat that asks again the moment the
        // ask is taken raises another, and a trace names no ask, so its answer is written
        // before anything else can be. A link is traced as one, by the asked names alone:
        // never the entry it reads.
        let what = match (filled, linked) {
            (true, true) => "linked",
            (true, false) => "filled",
            (false, _) => "declined",
        };
        for &chat in chats.iter().chain(&asked_cannot_receive) {
            self.trace(Some(chat), what, &vault, &key, waiting);
        }
        if started {
            self.trace(None, what, &vault, &key, waiting);
        }
        // One step for the ask, under the first chat to ask, or under none where only a chat's
        // start waits on it (#5078, #5074).
        {
            use crate::eventlog::secretasks::{How, Step};
            let waiting = u32::try_from(waiting).unwrap_or(u32::MAX);
            self.tell(
                chats.first().copied(),
                if filled {
                    Step::Filled {
                        ask: asked.id,
                        how: How::Value,
                        waiting,
                        renamed: (&vault, &key) != (&asked_at.0, &asked_at.1),
                        made_vault,
                    }
                } else {
                    Step::Declined {
                        ask: asked.id,
                        waiting,
                    }
                },
            );
        }
        drop(inner);
        let told = crate::askoutcome::Outcome {
            about: crate::askoutcome::About::Secret {
                vault: vault.clone(),
                key: key.clone(),
            },
            answer: if filled {
                crate::askoutcome::Answer::Filled
            } else {
                crate::askoutcome::Answer::Declined
            },
        };
        Ok(Answered {
            vault,
            key,
            filled,
            persona: asked.persona,
            declared: asked.declared.is_some(),
            names_no_vault: asked.names_no_vault,
            made_vault,
            asker: chats.first().copied(),
            nudge,
            told,
        })
    }

    /// **`key` of `vault` was filled by a run's notice** (#5242): an open Secret ask for it is
    /// settled as filled, its waiters told and the chats that stopped waiting nudged by the
    /// caller's one line, as the notice's own are.
    pub(super) fn settle_filled(&self, vault: &str, key: &str) -> Vec<u32> {
        let mut inner = self.inner();
        let Some(asked) = inner.open.remove(&(vault.to_owned(), key.to_owned())) else {
            return Vec::new();
        };
        let mut nudge = Vec::new();
        for waiter in asked.waiters {
            let told = waiter.tell.is_some_and(|tell| {
                tell.send(Settled::Filled {
                    vault: vault.to_owned(),
                    key: key.to_owned(),
                })
                .is_ok()
            });
            if let Some(chat) = waiter.chat
                && !told
                && !nudge.contains(&chat)
            {
                nudge.push(chat);
            }
        }
        self.save(&inner);
        nudge
    }

    /// **Chat `chat` has closed**: it waits on nothing, and what it declined goes with it. An ask
    /// nobody waits on any more goes.
    pub fn forget(&self, chat: u32) {
        let mut inner = self.inner();
        inner.declined.retain(|(of, _, _)| *of != chat);
        inner.open.retain(|_, asked| {
            asked.waiters.retain(|waiter| waiter.chat != Some(chat));
            asked.cannot_receive.retain(|one| *one != chat);
            asked.needed()
        });
        let held_grants = Self::forget_runs(&mut inner, chat);
        self.save(&inner);
        drop(inner);
        if held_grants {
            self.run_grants_moved();
        }
    }
}

/// **The waiter `n` on the ask for `at` left with its caller**: it is no chat to nudge, and an
/// ask nobody needs any more goes, as one does when its chat closes. How many still wait on it,
/// where the waiter was still there.
fn left(inner: &mut Inner, at: &(String, String), n: u64) -> Option<usize> {
    let asked = inner.open.get_mut(at)?;
    let before = asked.waiters.len();
    asked.waiters.retain(|waiter| waiter.n != n);
    if asked.waiters.len() == before {
        return None;
    }
    let waiting = waiting_on(asked);
    if !asked.needed() {
        inner.open.remove(at);
    }
    Some(waiting)
}

/// What a brokered command is told where the person stored its key under another vault or
/// name than it reads (#5072).
pub fn filled_elsewhere(vault: &str, key: &str) -> String {
    format!(
        "filled as {}, which this command does not read: run it again naming that key",
        vault_and_key(vault, key)
    )
}

// ----------------------------------------------------------------------------------------
// A chat asks: the `secret_ask` line (#5072)

/// **A chat asks for a credential it lacks** (#5072, ADR 0109): the `secret_ask` line on the
/// hook socket, sent by the `secret_ask` tool and `purlis secret ask` alike
/// ([`ask_the_window`]). Neither reads a vault. **It names no persona**: which persona's vault
/// is the app's record of the chat whose token the line carries, and whether the persona may
/// use a vault the line names is the broker's, as for a brokered run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecretAsk {
    /// The chat asking, from [`crate::hookwire::CHAT_ENV`]: the chat whose token the line must
    /// carry.
    pub chat: u32,
    /// The key it needs, as it will hand it to `purlis secret exec`.
    pub key: String,
    /// What for, in the chat's own words: shown to the person as the chat's.
    pub purpose: String,
    /// The vault, where the chat names one: its persona's otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault: Option<String>,
    /// **The chat says the value it has doesn't work** (#5075), and why, in its own words: the
    /// person is asked to replace it even though the vault holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace: Option<String>,
    /// **The program the chat will run with it** (#5244), where it says: a reviewed one is offered
    /// Allow for this chat on the same notice; any other gets Fill only, and its run asks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
}

/// **What a chat hears of its Secret ask** (#5072): never a value, and never where it lives
/// but the vault and key it is stored under.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Said {
    /// Filled: by the person now, or before the chat asked. Use this vault and key.
    Filled { vault: String, key: String },
    /// Declined, and why: the person said Not now, now or earlier in this chat.
    Declined {
        vault: String,
        key: String,
        reason: String,
    },
    /// Nobody answered before the chat's harness would end the wait: the ask stays open, and
    /// the chat is nudged when it is answered.
    Pending { vault: String, key: String },
    /// Filled, and the program the ask named allowed with it for this chat (#5244).
    FilledAndAllowed {
        vault: String,
        key: String,
        program: String,
    },
    /// Filled, and the program the ask named not allowed (#5244).
    FilledNotAllowed {
        vault: String,
        key: String,
        program: String,
    },
    /// The person did not allow the program the ask named with that vault in this chat, earlier
    /// or now, and nothing was filled (#5244).
    NotAllowed { vault: String, program: String },
}

/// Why a decline the person made is said to be one.
pub const DECLINED_BY_THE_PERSON: &str = "the person";

impl Said {
    /// What the command prints of it, and the status it ends with: 0 filled, 1 declined, and
    /// [`super::brokered::WAITING_ON_THE_PERSON`] still open, answerable in the project at `root`.
    pub fn line(&self, root: &Path) -> (String, i32) {
        match self {
            // It promises only what works (#5244): a program no tool of the persona's is asked
            // about as it runs.
            Self::Filled { vault, key } => (
                format!(
                    "filled {}: run your command with `purlis secret exec --vault {vault}` handing \
                     {key}; if its program is no tool of your persona's, the person is asked to \
                     allow it as it runs",
                    vault_and_key(vault, key)
                ),
                0,
            ),
            Self::FilledAndAllowed {
                vault,
                key,
                program,
            } => (
                format!(
                    "filled {}, and the person allowed {program} with it for this chat: run your \
                     command with `purlis secret exec --vault {vault}` handing {key}",
                    vault_and_key(vault, key)
                ),
                0,
            ),
            Self::FilledNotAllowed {
                vault,
                key,
                program,
            } => (
                format!(
                    "filled {}, but the person did not allow {program} with it in this chat: don't \
                     run {program} with it or work around that",
                    vault_and_key(vault, key)
                ),
                1,
            ),
            Self::NotAllowed { vault, program } => (
                format!(
                    "the person did not allow {program} with vault {vault} in this chat, so \
                     nothing was asked: don't ask again or work around it; go on without it"
                ),
                1,
            ),
            Self::Declined { vault, key, reason } => (
                format!(
                    "declined {} by {reason}: don't ask for it again in this chat; go on without \
                     it",
                    vault_and_key(vault, key)
                ),
                1,
            ),
            Self::Pending { vault, key } => (
                still_open_said(root, vault, key),
                super::brokered::WAITING_ON_THE_PERSON,
            ),
        }
    }
}

/// The asking chat, by the app's own record of it: never anything the line says.
#[derive(Debug, Clone, Copy)]
pub struct Chat<'a> {
    pub chat: u32,
    /// The persona the app started it as.
    pub persona: Option<&'a str>,
    pub harness: Option<Harness>,
    /// **The chat as its brokered runs are served** (#5244): what a program its ask names is
    /// resolved and judged by, as a run's is. None where the app has no sandbox record of it, and
    /// a program named is then asked about as it runs.
    pub asker: Option<&'a super::brokered::Asker>,
}

/// **The app's answer to a chat's `secret_ask` line** (#5072): the persona's vault unless the
/// line names one; a key the vault holds already answers filled at once, with nothing asked;
/// otherwise a Secret ask is raised or joined, and held to the chat's harness's timeout margin
/// ([`holds_for`]). `moved` is told once the ask is listed. The refusal is a sentence.
pub fn raised(
    ctx: &Ctx,
    asks: &Asks,
    chat: Chat<'_>,
    line: &SecretAsk,
    moved: &dyn Fn(),
) -> Result<Said, String> {
    raised_within(ctx, asks, chat, line, holds_for(chat.harness), moved)
}

/// [`raised`], held at most `within`.
pub fn raised_within(
    ctx: &Ctx,
    asks: &Asks,
    chat: Chat<'_>,
    line: &SecretAsk,
    within: Duration,
    moved: &dyn Fn(),
) -> Result<Said, String> {
    if line.chat != chat.chat {
        return Err(format!("this line is not chat {}'s", chat.chat));
    }
    if !super::needs::is_key(&line.key) {
        return Err(not_a_key(&line.key));
    }
    let purpose = purpose_said(&line.purpose);
    if purpose.is_empty() {
        return Err("say what the key is for: the person reads it before filling it".to_owned());
    }
    let Some(persona) = chat.persona.filter(|persona| !persona.is_empty()) else {
        return Err(
            "this chat runs as no persona, so it has no vault to ask into: start the chat as \
             the persona whose work needs the key"
                .to_owned(),
        );
    };
    let (vault, names_no_vault) = match line.vault.as_deref().filter(|vault| !vault.is_empty()) {
        Some(vault) => (vault.to_owned(), false),
        None => match persona_vault(ctx, persona)? {
            Some(vault) => (vault, false),
            // A persona that names no vault is asked into one named after it (#5076).
            None => (persona.to_owned(), true),
        },
    };
    // A replace (#5075) asks the person even for a key the vault holds, and only in a vault the
    // persona may use: the person may overwrite what is there.
    let reason = match line.replace.as_deref() {
        None => None,
        Some(why) => {
            let reason = Reason::replace(ctx, &vault, &line.key, why);
            if matches!(&reason, Reason::Replace { said, .. } if said.is_empty()) {
                return Err(
                    "say why the value doesn't work: the person reads it before replacing it"
                        .to_owned(),
                );
            }
            super::brokered::authorise(ctx, Some(persona), &vault).map_err(|why| {
                format!("{why} A replace is asked only in a vault {persona} may use.")
            })?;
            Some(reason)
        }
    };
    if reason.is_none() && holds_already(ctx, persona, &vault, &line.key) {
        return Ok(Said::Filled {
            vault,
            key: line.key.clone(),
        });
    }
    // A program named ahead of its run (#5244): one notice with its Allow, where it may have one.
    let mut no_allow = None;
    if let (Some(program), Some(asker), None) = (
        line.program
            .as_deref()
            .filter(|program| !program.is_empty()),
        chat.asker,
        reason.as_ref(),
    ) {
        use super::brokered::Ahead;
        let declared = super::needs::declared(&ctx.root, persona, &line.key).map(|n| n.purpose);
        match super::brokered::asked_ahead(asker, asks, &vault, &line.key, program, declared) {
            Ahead::Declared => {}
            Ahead::FillOnly(why) => no_allow = Some(why),
            Ahead::Refused { program } => return Ok(Said::NotAllowed { vault, program }),
            Ahead::Offer(need) => {
                let program = need.bound.program_name();
                let key = line.key.clone();
                return Ok(
                    match asks.allowed_or_held(*need, within, &mut |_: &str| moved()) {
                        run::RunHeld::Allowed | run::RunHeld::Filled => Said::FilledAndAllowed {
                            vault,
                            key,
                            program,
                        },
                        run::RunHeld::Refused { .. }
                            if holds_already(ctx, persona, &vault, &key) =>
                        {
                            Said::FilledNotAllowed {
                                vault,
                                key,
                                program,
                            }
                        }
                        run::RunHeld::Refused { .. } => Said::NotAllowed { vault, program },
                        run::RunHeld::StillOpen { .. } => Said::Pending { vault, key },
                    },
                );
            }
        }
    }
    let need = Need {
        chat: Some(chat.chat),
        persona: persona.to_owned(),
        vault: vault.clone(),
        key: line.key.clone(),
        tool: None,
        purpose: Some(purpose),
        declared: super::needs::declared(&ctx.root, persona, &line.key).map(|need| need.purpose),
        names_no_vault,
        reason,
        no_allow,
    };
    Ok(match asks.hold(need, within, &mut |_: &str| moved()) {
        Held::Filled { vault, key } => Said::Filled { vault, key },
        Held::Declined { .. } => Said::Declined {
            vault,
            key: line.key.clone(),
            reason: DECLINED_BY_THE_PERSON.to_owned(),
        },
        Held::Refused { .. } => Said::Declined {
            vault,
            key: line.key.clone(),
            reason: DECLINED_BY_POLICY.to_owned(),
        },
        // A `secret_ask` line's hold never looks for a caller gone.
        Held::StillOpen { .. } | Held::CallerGone { .. } => Said::Pending {
            vault,
            key: line.key.clone(),
        },
    })
}

/// What a key name no Secret ask takes is refused with.
fn not_a_key(key: &str) -> String {
    format!(
        "{} is not a key name a secret ask takes: use letters, digits, _, - and ., starting with \
         a letter or _",
        crate::shown::one_line(key, 40)
    )
}

/// The vault persona `persona` uses, as its definition or the registry names it; none where
/// it names none (`vault: none`, or nothing at all); or why the registry could not be read.
fn persona_vault(ctx: &Ctx, persona: &str) -> Result<Option<String>, String> {
    let state = crate::plane::state_dir(&ctx.root);
    match crate::personaverbs::vault(&ctx.root, &state, persona) {
        crate::personas::Vault::Named(vault) | crate::personas::Vault::Registered(vault) => {
            Ok(Some(vault))
        }
        crate::personas::Vault::RegistryUnreadable(why) => Err(why),
        crate::personas::Vault::DeclaredNone | crate::personas::Vault::Unnamed => Ok(None),
    }
}

/// **Whether a fill of `key` may go into vault `vault` for `persona`**: one the persona may
/// use ([`super::brokered::authorise`]: the registry's tag, or the person's Allow), which holds
/// no value under that key yet. Read by key names alone. The refusal says what to do.
fn fillable(ctx: &Ctx, persona: &str, vault: &str, key: &str) -> Result<(), String> {
    super::brokered::authorise(ctx, Some(persona), vault).map_err(|why| {
        format!("{why} Allow it for {persona} first, or fill the key into {persona}'s own vault.")
    })?;
    let held = cmd::provider(ctx, vault)
        .and_then(|v| cmd::keys(ctx, &v))
        .map_err(|e| e.message.to_string())?;
    if held.iter().any(|one| one == key) {
        return Err(format!(
            "{} holds a value already, and a fill never replaces one: choose another name",
            vault_and_key(vault, key)
        ));
    }
    Ok(())
}

/// The provider a vault made by a first fill is kept by: the system's credential store, as a
/// new vault in the window is (#5076).
pub const MADE_ON: &str = "keyring";

/// **Makes vault `vault` where the project does not have it** (#5076): on [`MADE_ON`], tagged
/// for `persona` so its chats may use it, as `purlis vault add` makes one. Whether it was made;
/// the refusal is the command's own sentence.
fn made_if_missing(ctx: &Ctx, vault: &str, persona: &str) -> Result<bool, String> {
    let doc = super::registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    if super::registry::vault_in(&doc, vault).is_ok() {
        return Ok(false);
    }
    let req = super::vaultcmd::AddRequest {
        name: vault.to_owned(),
        provider: MADE_ON.to_owned(),
        persona: Some(persona.to_owned()).filter(|persona| !persona.is_empty()),
        ..Default::default()
    };
    let mut said = AddSaid::default();
    if super::vaultcmd::add(ctx, &req, &mut said) != 0 {
        return Err(said.0.join(" "));
    }
    Ok(true)
}

/// What `purlis vault add` said, kept: its refusal is the fill's.
#[derive(Default)]
struct AddSaid(Vec<String>);

impl super::cmd::Io for AddSaid {
    fn say(&mut self, line: super::cmd::Say) {
        if let super::cmd::Say::Err(text) | super::cmd::Say::Info(text) = line {
            self.0.push(text.to_string());
        }
    }
    fn out(&mut self, _bytes: &[u8]) {}
    fn err(&mut self, _bytes: &[u8]) {}
    fn stdout_is_terminal(&self) -> bool {
        false
    }
    fn stdin_is_terminal(&self) -> bool {
        false
    }
    fn read_stdin(&mut self) -> String {
        String::new()
    }
    fn read_hidden(&mut self, _prompt: &str) -> String {
        String::new()
    }
}

/// **Whether vault `vault` holds `key` already, for a chat as `persona`**: only where the
/// persona may use the vault, so a chat learns nothing of a vault's keys it could not list
/// itself. Read by the key names alone, never a value.
fn holds_already(ctx: &Ctx, persona: &str, vault: &str, key: &str) -> bool {
    super::brokered::authorise(ctx, Some(persona), vault).is_ok()
        && cmd::provider(ctx, vault)
            .and_then(|v| cmd::keys(ctx, &v))
            .is_ok_and(|keys| keys.iter().any(|one| one == key))
}

/// How much longer than the app holds an ask its asker waits for the answer: the app answers
/// pending at its margin, and the line comes back after it.
const ASKER_WAITS_PAST_THE_HOLD: Duration = Duration::from_secs(15);

/// **How long the chat whose environment is `env` waits for the app's answer to its Secret
/// ask**: its harness's command timeout, and [`ASKER_WAITS_PAST_THE_HOLD`] more, so the app's
/// pending, sent at [`holds_for`] that harness, always arrives first.
fn asker_waits(env: &dyn Fn(&str) -> Option<String>) -> Duration {
    command_timeout(asker_harness(env)) + ASKER_WAITS_PAST_THE_HOLD
}

/// The harness of the chat a command runs in, by its start's environment `env`.
fn asker_harness(env: &dyn Fn(&str) -> Option<String>) -> Option<Harness> {
    // A start names the harness by the registry's name (`claude-code`), read as the app reads
    // it: by the kind's word alone it was no harness, and held to the shortest wait (#5192).
    crate::hookwire::chat_harness(env).and_then(|name| Harness::of_registry_or_kind(&name))
}

/// **How long the app holds a Secret ask of the chat a command runs in** ([`holds_for`]), by
/// its start's environment `env`: what `purlis secret ask` says it waits for (#6008).
fn asker_holds(env: &dyn Fn(&str) -> Option<String>) -> Duration {
    holds_for(asker_harness(env))
}

/// **What `purlis secret ask` and the `secret_ask` tool both send** (#5072): the
/// `secret_ask` line to the app that started this chat, and its answer. Refused, with a
/// sentence, in a chat purlis did not start: only its window asks the person.
#[cfg(unix)]
pub fn ask_the_window(
    ctx: &Ctx,
    key: &str,
    purpose: &str,
    vault: Option<&str>,
    replace: Option<&str>,
    program: Option<&str>,
    waits: &mut dyn FnMut(&str),
) -> Result<Said, String> {
    use crate::hookwire::{Ask, Asking, ChatToken};
    let env = |name: &str| ctx.env.get(name);
    let not_here = || {
        "a secret ask is answered in purlis's window, and this chat was not started by purlis: \
         ask the person to add the key to the vault themselves"
            .to_owned()
    };
    let socket = env(crate::hookwire::SOCKET_ENV)
        .filter(|socket| !socket.is_empty())
        .ok_or_else(not_here)?;
    let chat = [crate::hookwire::CHAT_ENV, crate::active::SESSION_ID_ENV]
        .into_iter()
        .filter_map(env)
        .find_map(|n| n.trim().parse::<u32>().ok().filter(|n| *n > 0))
        .ok_or_else(not_here)?;
    let mut asking =
        Asking::on(Path::new(&socket), ChatToken::read(&env)).map_err(|_| not_here())?;
    let line = Ask::Secret(Box::new(SecretAsk {
        chat,
        key: key.to_owned(),
        purpose: purpose.to_owned(),
        vault: vault.map(str::to_owned),
        replace: replace.map(str::to_owned),
        program: program.map(str::to_owned),
    }));
    ask_the_window_over(ctx, &mut asking, &line, waits)
}

/// [`ask_the_window`], sending `line` over a connection to the app already made. `waits` is told
/// where the person answers it once the app is reached, before the wait for their answer.
#[cfg(unix)]
pub fn ask_the_window_over(
    ctx: &Ctx,
    asking: &mut crate::hookwire::Asking,
    line: &crate::hookwire::Ask,
    waits: &mut dyn FnMut(&str),
) -> Result<Said, String> {
    use crate::hookwire::{Answer, NOTHING_ANSWERS, UNKNOWN_LINE};
    let env = |name: &str| ctx.env.get(name);
    // Said once the app is reached, so nothing is said of a wait that never was (#6008).
    waits(&crate::askwait::waiting_said(&ctx.root, asker_holds(&env)));
    match asking.ask(line, asker_waits(&env)) {
        Ok(Answer::Secret(said)) => Ok(said),
        Ok(Answer::No { why }) if why == NOTHING_ANSWERS || why == UNKNOWN_LINE => {
            Err(super::brokered::OLDER_APP.to_owned())
        }
        Ok(Answer::No { why }) => Err(why),
        Ok(_) => Err(super::brokered::OLDER_APP.to_owned()),
        Err(e) => Err(match e.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                super::brokered::NOT_IN_TIME.to_owned()
            }
            _ => super::brokered::OLDER_APP.to_owned(),
        }),
    }
}

/// Where there is no unix socket there is no window to ask.
#[cfg(not(unix))]
pub fn ask_the_window(
    _ctx: &Ctx,
    _key: &str,
    _purpose: &str,
    _vault: Option<&str>,
    _replace: Option<&str>,
    _program: Option<&str>,
    _waits: &mut dyn FnMut(&str),
) -> Result<Said, String> {
    Err(
        "a secret ask is answered in purlis's window, which a chat on this system has no \
         socket to reach: ask the person to add the key to the vault themselves"
            .to_owned(),
    )
}

/// **A value written for an ask** (#5075): into the vault under the key; or, for a replace of a
/// Linked entry, into the entry it reads, so the rotation reaches every key linked to it. A
/// broken link is given the value in its place (a write to a link replaces it).
pub(super) fn filled_with(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    reason: Option<&Reason>,
    value: &str,
) -> Result<(), super::VaultError> {
    use crate::sandbox::tools::mint;
    if let Some(Reason::Replace { reads, .. }) = reason {
        // Where the person was shown it goes, or nowhere: a link re-pointed or made since the
        // ask was raised is refused, never followed.
        if super::links::of(ctx, vault, key)? != *reads {
            return Err(super::VaultError::new(format!(
                "{key} in vault {vault} changed since the chat asked: look at it again in the \
                 vault, then fill it there"
            )));
        }
        let (into_vault, into_key) = reads
            .as_ref()
            .map_or((vault, key), |to| (to.vault.as_str(), to.key.as_str()));
        // A GitHub App's key is never replaced through an ask.
        if let mint::Raw::Refused(why) = mint::raw(&ctx.root, into_vault, into_key) {
            return Err(super::VaultError::new(why));
        }
        let into = cmd::provider(ctx, into_vault)?;
        return cmd::set_value(ctx, &into, into_key, value);
    }
    let v = cmd::provider(ctx, vault)?;
    cmd::set_value(ctx, &v, key, value)
}

/// **A link made for an ask** (#5073, #5075): the key comes to read `to`, re-pointing the link it
/// is. Over a value of its own only where the ask is a replace, the person's explicit choice
/// ([`super::links::link_over`]); otherwise refused, as every other link over a value is.
fn linked_to(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    reason: Option<&Reason>,
    to: &super::links::Target,
) -> Result<(), super::VaultError> {
    let v = cmd::provider(ctx, vault)?;
    if let Some(Reason::Replace { reads, .. }) = reason {
        if super::links::of(ctx, vault, key)? != *reads {
            return Err(super::VaultError::new(format!(
                "{key} in vault {vault} changed since the chat asked: look at it again in the \
                 vault, then link it there"
            )));
        }
        super::links::link_over(ctx, &v, key, &to.vault, &to.key)
    } else {
        super::links::link(ctx, &v, key, &to.vault, &to.key)
    }
}

/// Each chat waiting on `asked`, once, the first to ask first.
fn chats_of(asked: &Asked) -> Vec<u32> {
    let mut chats: Vec<u32> = Vec::new();
    for chat in asked.waiters.iter().filter_map(|waiter| waiter.chat) {
        if !chats.contains(&chat) {
            chats.push(chat);
        }
    }
    chats
}

/// How many chats' starts asked for `asked`: those holding on it, and one that stopped waiting,
/// whose person was told to start it again once it is answered. A start that asks again takes
/// the place of one that stopped ([`Asks::hold`]).
fn starts_of(asked: &Asked) -> usize {
    asked
        .waiters
        .iter()
        .filter(|waiter| waiter.chat.is_none())
        .count()
}

/// How many wait on `asked`: each chat once, and each start holding on it.
fn waiting_on(asked: &Asked) -> usize {
    chats_of(asked).len() + starts_of(asked)
}

#[cfg(test)]
mod tests;

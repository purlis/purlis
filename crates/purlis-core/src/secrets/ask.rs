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
//! - **The command waits until just before its harness would kill it** ([`holds_for`]): three
//!   quarters of the harness's own command timeout. A quick answer is invisible to the chat. A
//!   slow one ends the command with `waiting on you for <vault>/<key>; retry once answered`; the
//!   ask stays open, and the chat is nudged when it is answered ([`Answered::nudge`]).
//! - **One ask per vault and key** in a project, whichever chats miss it: a second chat joins
//!   the open ask, which says how many wait ([`Open::waiting`]), and one fill settles them all.
//! - **A decline settles the chats waiting then, and only them.** Each is not asked about that
//!   key again for as long as it lives; another chat that misses it later is asked afresh.
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
    pub chat: u32,
    pub persona: String,
    pub vault: String,
    pub key: String,
    /// The declared tool the run is of, where it is one: what the person reads the key is for.
    pub tool: Option<String>,
}

/// **An open Secret ask, as the window shows it**: names only, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// Its number in the project: what an answer names.
    pub id: u32,
    pub persona: String,
    pub vault: String,
    pub key: String,
    pub tool: Option<String>,
    /// Each chat that needs it, the first to ask first: those holding a command, and those
    /// whose command stopped waiting and will be nudged.
    pub chats: Vec<u32>,
    /// When it was raised, in seconds since 1970.
    pub since: u64,
}

impl Open {
    /// How many chats wait on it.
    pub fn waiting(&self) -> usize {
        self.chats.len()
    }
}

/// What a command that missed a key comes to ([`Asks::hold`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// The person filled it: read the vault again.
    Filled,
    /// The person declined it, now or earlier in this chat. `line` is what the chat is told.
    Declined { line: String },
    /// Nobody answered before the hold ran out: the ask stays open and the chat is nudged when
    /// it is answered. `line` is what the chat is told.
    StillOpen { line: String },
}

/// The person's answer to a Secret ask.
#[derive(Debug)]
pub enum Answer {
    /// A new value, written into the vault under the asked key. Wiped as it is dropped.
    Fill(Zeroizing<String>),
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
    pub nudge: Vec<u32>,
    pub told: crate::askoutcome::Outcome,
}

/// How a waiter is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Settled {
    Filled,
    Declined,
}

#[derive(Debug)]
struct Waiter {
    chat: u32,
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
    since: u64,
    waiters: Vec<Waiter>,
    /// An answer is being written: a second is refused, as any ask's second answer is.
    answering: bool,
}

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    next_waiter: u64,
    /// Each open ask, by its vault and key.
    open: HashMap<(String, String), Asked>,
    /// Each chat's declines, by vault and key, for the chat's life.
    declined: HashSet<(u32, String, String)>,
}

/// **The Secret asks one project holds open** (module docs).
#[derive(Debug)]
pub struct Asks {
    root: PathBuf,
    inner: Mutex<Inner>,
    /// Where they are kept across a restart; nowhere for [`Asks::new`]'s.
    kept_at: Option<PathBuf>,
}

/// **The open asks as [`KEPT_IN`] holds them**: names and chats, never a value.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Kept {
    v: u32,
    /// The last number an ask was given, so a number is never given twice.
    asked: u32,
    asks: Vec<KeptAsk>,
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
    since: u64,
    /// The chats that wait on it, by the app's number, which a restart keeps.
    chats: Vec<u32>,
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
            since: asked.since,
            chats: chats_of(asked),
        })
    }

    /// Whether every name reads as a name: one line, not empty.
    fn reads(&self) -> bool {
        let name = |name: &str| !name.is_empty() && !name.chars().any(char::is_control);
        [&self.persona, &self.vault, &self.key]
            .into_iter()
            .chain(&self.tool)
            .all(|one| name(one))
            && !self.chats.is_empty()
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
fn declined_said(vault: &str, key: &str) -> String {
    format!("declined {}", vault_and_key(vault, key))
}

/// What a chat whose command stopped waiting hears.
fn still_open_said(vault: &str, key: &str) -> String {
    format!(
        "waiting on you for {}; retry once answered",
        vault_and_key(vault, key)
    )
}

/// What a command is told as it starts to wait.
fn asked_said(vault: &str, key: &str, within: Duration) -> String {
    format!(
        "purlis asked the person in its window for {}, which the vault doesn't hold: this \
         command waits for their answer for up to {} seconds",
        vault_and_key(vault, key),
        within.as_secs()
    )
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
        for one in kept.asks {
            let at = (one.vault.clone(), one.key.clone());
            if !one.reads() {
                tracing::warn!(
                    "purlis: an open secret ask kept in {} does not read; it is left out",
                    path.display()
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
            for chat in one.chats {
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
                    since: one.since,
                    waiters,
                    answering: false,
                },
            );
        }
        Self {
            root: root.to_path_buf(),
            inner: Mutex::new(inner),
            kept_at: Some(path),
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
    /// ask that is not among them waits on nothing, and an ask nobody waits on goes.
    pub fn only_these_came_back(&self, back: &[u32]) {
        let mut inner = self.inner();
        inner.open.retain(|_, asked| {
            asked.waiters.retain(|waiter| back.contains(&waiter.chat));
            !asked.waiters.is_empty() || asked.answering
        });
        self.save(&inner);
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records `what` of the ask for `vault`/`key` in chat `chat`'s trace: names, never a value.
    fn trace(&self, chat: u32, what: &str, vault: &str, key: &str, waiting: usize) {
        crate::trace::record_values(
            &self.root,
            &chat.to_string(),
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
        let Need {
            chat,
            persona,
            vault,
            key,
            tool,
        } = need;
        let (answered, n) = {
            let mut inner = self.inner();
            if inner.declined.contains(&(chat, vault.clone(), key.clone())) {
                return Held::Declined {
                    line: declined_said(&vault, &key),
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
            let joined = if let Some(asked) = inner.open.get_mut(&at) {
                // A chat that tries again waits again: a command of its that stopped waiting is
                // no longer one to nudge, since this one hears the answer.
                asked
                    .waiters
                    .retain(|one| one.chat != chat || one.tell.is_some());
                asked.waiters.push(waiter);
                Some(chats_of(asked).len())
            } else {
                inner.next += 1;
                let id = inner.next;
                inner.open.insert(
                    at,
                    Asked {
                        id,
                        persona,
                        tool,
                        since: now_secs(),
                        waiters: vec![waiter],
                        answering: false,
                    },
                );
                None
            };
            self.save(&inner);
            drop(inner);
            match joined {
                Some(waiting) => self.trace(chat, "joined", &vault, &key, waiting),
                None => self.trace(chat, "raised", &vault, &key, 1),
            }
            (answered, n)
        };
        say(&asked_said(&vault, &key, within));
        let settled_as = |settled: Settled| match settled {
            Settled::Filled => Held::Filled,
            Settled::Declined => Held::Declined {
                line: declined_said(&vault, &key),
            },
        };
        match answered.recv_timeout(within) {
            Ok(settled) => settled_as(settled),
            // Its chat closed meanwhile.
            Err(mpsc::RecvTimeoutError::Disconnected) => Held::StillOpen {
                line: still_open_said(&vault, &key),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Stops waiting, unless it was answered as the hold ran out.
                let mut inner = self.inner();
                let still = inner
                    .open
                    .get_mut(&(vault.clone(), key.clone()))
                    .and_then(|asked| asked.waiters.iter_mut().find(|w| w.n == n))
                    .map(|waiter| waiter.tell = None)
                    .is_some();
                drop(inner);
                if !still && let Ok(settled) = answered.try_recv() {
                    return settled_as(settled);
                }
                Held::StillOpen {
                    line: still_open_said(&vault, &key),
                }
            }
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
                chats: chats_of(asked),
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
        if by.scope() != HumanScope::LocalUi {
            return Err("a secret ask is answered in purlis's window only".to_owned());
        }
        if matches!(&answer, Answer::Fill(value) if value.is_empty()) {
            return Err(
                "an empty value would read as a present one: type the value, or choose Not now"
                    .to_owned(),
            );
        }
        let (vault, key) = {
            let mut inner = self.inner();
            let Some(((vault, key), asked)) = inner
                .open
                .iter_mut()
                .find(|(_, asked)| asked.id == id && !asked.answering)
            else {
                return Err(format!("secret ask {id} is no longer open"));
            };
            asked.answering = true;
            (vault.clone(), key.clone())
        };
        let filled = match &answer {
            Answer::Fill(value) => {
                let written =
                    cmd::provider(ctx, &vault).and_then(|v| cmd::set_value(ctx, &v, &key, value));
                if let Err(e) = written {
                    if let Some(asked) = self.inner().open.get_mut(&(vault.clone(), key.clone())) {
                        asked.answering = false;
                    }
                    return Err(e.message.to_string());
                }
                true
            }
            Answer::Decline => false,
        };
        drop(answer);
        let mut inner = self.inner();
        let Some(asked) = inner.open.remove(&(vault.clone(), key.clone())) else {
            return Err(format!("secret ask {id} is no longer open"));
        };
        let chats = chats_of(&asked);
        let settled = if filled {
            Settled::Filled
        } else {
            Settled::Declined
        };
        let mut nudge = Vec::new();
        for waiter in asked.waiters {
            let told = waiter.tell.is_some_and(|tell| tell.send(settled).is_ok());
            if !told && !nudge.contains(&waiter.chat) {
                nudge.push(waiter.chat);
            }
        }
        if !filled {
            for &chat in &chats {
                inner.declined.insert((chat, vault.clone(), key.clone()));
            }
        }
        self.save(&inner);
        drop(inner);
        let what = if filled { "filled" } else { "declined" };
        for &chat in &chats {
            self.trace(chat, what, &vault, &key, chats.len());
        }
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
            nudge,
            told,
        })
    }

    /// **Chat `chat` has closed**: it waits on nothing, and what it declined goes with it. An ask
    /// nobody waits on any more goes.
    pub fn forget(&self, chat: u32) {
        let mut inner = self.inner();
        inner.declined.retain(|(of, _, _)| *of != chat);
        inner.open.retain(|_, asked| {
            asked.waiters.retain(|waiter| waiter.chat != chat);
            !asked.waiters.is_empty() || asked.answering
        });
        self.save(&inner);
    }
}

/// Each chat waiting on `asked`, once, the first to ask first.
fn chats_of(asked: &Asked) -> Vec<u32> {
    let mut chats: Vec<u32> = Vec::new();
    for waiter in &asked.waiters {
        if !chats.contains(&waiter.chat) {
            chats.push(waiter.chat);
        }
    }
    chats
}

#[cfg(test)]
mod tests;

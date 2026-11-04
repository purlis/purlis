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
//! **No value is held here past its write.** The window's answer carries it straight to the
//! provider; [`Open`], what the chat hears, the nudge and the trace name the vault and the key,
//! never a value. Each ask's life is recorded in the chat's trace as `secret.ask`, metadata only:
//! raised, joined, filled, declined.
//!
//! In memory only, as every other ask the app holds: an ask dies with the app (keeping open
//! asks across a restart is #5054's story 59, not this one).

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
    /// The person filled it: read the vault again.
    Filled,
    /// The person declined it, now or earlier in this chat. `line` is what the chat is told.
    Declined { line: String },
    /// Nobody answered before the hold ran out: the ask stays open and the chat is nudged when
    /// it is answered. `line` is what the chat is told.
    StillOpen { line: String },
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
        let Need {
            chat,
            persona,
            vault,
            key,
            tool,
        } = need;
        let (answered, n) = {
            let mut inner = self.inner();
            if let Some(chat) = chat
                && inner.declined.contains(&(chat, vault.clone(), key.clone()))
            {
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
                Some(waiting_on(asked))
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
            Err(e) if e.kind == super::Kind::NotFound => {}
            got => return got.map_err(Unread::Vault),
        }
        match self.hold(need, within, say) {
            Held::Filled => read().map_err(Unread::Vault),
            Held::Declined { line } | Held::StillOpen { line } => Err(Unread::Asked(line)),
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
        let started = asked.waiters.iter().any(|waiter| waiter.chat.is_none());
        let waiting = waiting_on(&asked);
        let settled = if filled {
            Settled::Filled
        } else {
            Settled::Declined
        };
        let mut nudge = Vec::new();
        for waiter in asked.waiters {
            let told = waiter.tell.is_some_and(|tell| tell.send(settled).is_ok());
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
        drop(inner);
        let what = if filled { "filled" } else { "declined" };
        for &chat in &chats {
            self.trace(Some(chat), what, &vault, &key, waiting);
        }
        if started {
            self.trace(None, what, &vault, &key, waiting);
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
            asked.waiters.retain(|waiter| waiter.chat != Some(chat));
            !asked.waiters.is_empty() || asked.answering
        });
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

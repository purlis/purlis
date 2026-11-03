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
    /// **The chat's own words for what it needs the key for** (#5072), one line and bounded
    /// ([`purpose_said`]): shown as the chat's, never as purlis's. None for a broker miss.
    pub purpose: Option<String>,
    /// **What the persona's definition says the key is for**, where it declares the key as a
    /// Secret need ([`super::needs`], #5076). None for a key it does not declare, which is shown
    /// with a quiet "not declared by this persona" tag (#5072).
    pub declared: Option<String>,
    /// **The persona names no vault** (`vault: none`, or nothing at all, #5076): the ask is for
    /// a vault named after it, made at the first fill, and the fill offers a proposed change
    /// setting the persona's `vault`.
    pub names_no_vault: bool,
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

/// Told each step of an ask's life, for the event log (#5078): the chat and the step.
pub type Telling = std::sync::Arc<dyn Fn(u32, crate::eventlog::secretasks::Step) + Send + Sync>;

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
    /// The persona the ask was raised for.
    pub persona: String,
    /// Whether the persona declares the asked key as a Secret need: a fill of one it does not
    /// is offered to the person as a proposed change adding it (#5076).
    pub declared: bool,
    /// The chat's words for what the key is for, where one said.
    pub purpose: Option<String>,
    /// The persona names no vault: a fill is offered as a proposed change setting its
    /// `vault` to [`Self::vault`] (#5076).
    pub names_no_vault: bool,
    /// The vault was made by this fill, on the default provider (#5076).
    pub made_vault: bool,
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
pub struct Asks {
    root: PathBuf,
    inner: Mutex<Inner>,
    /// Told each step, where the app records them ([`Self::telling`]).
    told: Mutex<Option<Telling>>,
}

impl std::fmt::Debug for Asks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Asks")
            .field("root", &self.root)
            .field("inner", &self.inner)
            .finish_non_exhaustive()
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
            told: Mutex::new(None),
        }
    }

    /// **Tells `told` each step of every ask's life from now on** (#5078): what the app writes
    /// to the event log, the audit's and telemetry's source. Counts and closed words alone
    /// ([`crate::eventlog::secretasks::Step`]).
    pub fn telling(&self, told: Telling) {
        *self.told.lock().unwrap_or_else(PoisonError::into_inner) = Some(told);
    }

    /// Tells the step of chat `chat`'s, where anything listens.
    fn tell(&self, chat: u32, step: crate::eventlog::secretasks::Step) {
        let told = self
            .told
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(told) = told {
            told(chat, step);
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
            purpose,
            declared,
            names_no_vault,
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
            // Policy's lock on undeclared keys (#5078): refused here, never shown to the person.
            if declared.is_none()
                && crate::sandbox::policy::Locks::of(&self.root).forbids_undeclared_secret_asks()
            {
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
                        since: now_secs(),
                        waiters: vec![waiter],
                        answering: false,
                    },
                );
                None
            };
            drop(inner);
            use crate::eventlog::secretasks::Step;
            match joined {
                Some((ask, waiting)) => {
                    self.trace(chat, "joined", &vault, &key, waiting);
                    self.tell(
                        chat,
                        Step::Joined {
                            ask,
                            waiting: u32::try_from(waiting).unwrap_or(u32::MAX),
                        },
                    );
                }
                None => {
                    self.trace(chat, "raised", &vault, &key, 1);
                    self.tell(
                        chat,
                        Step::Raised {
                            ask: raised_id,
                            declared: is_declared,
                        },
                    );
                }
            }
            (answered, n)
        };
        say(&asked_said(&vault, &key, within));
        let settled_as = |settled: Settled| match settled {
            Settled::Filled { vault, key } => Held::Filled { vault, key },
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
            Held::Filled { .. } => read().map_err(Unread::Vault),
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
                purpose: asked.purpose.clone(),
                declared: asked.declared.clone(),
                names_no_vault: asked.names_no_vault,
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
        self.answer_as(ctx, id, answer, Named::as_asked(), by)
    }

    /// **[`Self::answer`], storing a fill as `named`** (#5072): in another vault of the
    /// project's or under another key name, which the person chose before filling. Every chat
    /// that waits hears the name it is stored under. Refused, leaving the ask open, for a key
    /// name no Secret ask takes or a vault the project does not have.
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
        let still_open = || {
            if let Some(asked) = self.inner().open.get_mut(&(vault.clone(), key.clone())) {
                asked.answering = false;
            }
        };
        let into_vault = named.vault.clone().unwrap_or_else(|| vault.clone());
        let mut made_vault = false;
        let into_key = named.key.clone().unwrap_or_else(|| key.clone());
        let filled = match &answer {
            Answer::Fill(value) => {
                if !super::needs::is_key(&into_key) {
                    still_open();
                    return Err(format!(
                        "{} is not a key name purlis stores: use letters, digits, _, - and ., \
                         starting with a letter or _",
                        crate::shown::one_line(&into_key, 40)
                    ));
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
                let written = cmd::provider(ctx, &into_vault)
                    .and_then(|v| cmd::set_value(ctx, &v, &into_key, value));
                if let Err(e) = written {
                    still_open();
                    return Err(e.message.to_string());
                }
                true
            }
            Answer::Decline => false,
        };
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
        drop(inner);
        let what = if filled { "filled" } else { "declined" };
        for &chat in &chats {
            self.trace(Some(chat), what, &vault, &key, waiting);
        }
        if started {
            self.trace(None, what, &vault, &key, waiting);
        }
        // One step for the ask, under the first chat to ask (#5078).
        if let Some(&first) = chats.first() {
            use crate::eventlog::secretasks::{How, Step};
            let waiting = u32::try_from(chats.len()).unwrap_or(u32::MAX);
            self.tell(
                first,
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
            purpose: asked.purpose,
            names_no_vault: asked.names_no_vault,
            made_vault,
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
}

/// Why a decline the person made is said to be one.
pub const DECLINED_BY_THE_PERSON: &str = "the person";

impl Said {
    /// What the command prints of it, and the status it ends with: 0 filled, 1 declined, and
    /// [`super::brokered::WAITING_ON_THE_PERSON`] still open.
    pub fn line(&self) -> (String, i32) {
        match self {
            Self::Filled { vault, key } => (
                format!(
                    "filled {}: run your command with `purlis secret exec {vault}` handing {key}",
                    vault_and_key(vault, key)
                ),
                0,
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
                still_open_said(vault, key),
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
        return Err(format!(
            "{} is not a key name a secret ask takes: use letters, digits, _, - and ., starting \
             with a letter or _",
            crate::shown::one_line(&line.key, 40)
        ));
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
    if holds_already(ctx, persona, &vault, &line.key) {
        return Ok(Said::Filled {
            vault,
            key: line.key.clone(),
        });
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
        Held::StillOpen { .. } => Said::Pending {
            vault,
            key: line.key.clone(),
        },
    })
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
    let mut kept = Kept::default();
    if super::vaultcmd::add(ctx, &req, &mut kept) != 0 {
        return Err(kept.0.join(" "));
    }
    Ok(true)
}

/// What `purlis vault add` said, kept: its refusal is the fill's.
#[derive(Default)]
struct Kept(Vec<String>);

impl super::cmd::Io for Kept {
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

/// **What `purlis secret ask` and the `secret_ask` tool both send** (#5072): the
/// `secret_ask` line to the app that started this chat, and its answer. Refused, with a
/// sentence, in a chat purlis did not start: only its window asks the person.
#[cfg(unix)]
pub fn ask_the_window(
    ctx: &Ctx,
    key: &str,
    purpose: &str,
    vault: Option<&str>,
) -> Result<Said, String> {
    use crate::hookwire::{Answer, Ask, Asking, ChatToken, NOTHING_ANSWERS, UNKNOWN_LINE};
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
    let harness = env(crate::hookwire::HARNESS_ENV).and_then(|word| Harness::of_kind(&word));
    let mut asking =
        Asking::on(Path::new(&socket), ChatToken::read(&env)).map_err(|_| not_here())?;
    let line = Ask::Secret(Box::new(SecretAsk {
        chat,
        key: key.to_owned(),
        purpose: purpose.to_owned(),
        vault: vault.map(str::to_owned),
    }));
    match asking.ask(&line, command_timeout(harness) + ASKER_WAITS_PAST_THE_HOLD) {
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
) -> Result<Said, String> {
    Err(
        "a secret ask is answered in purlis's window, which this platform does not reach yet"
            .to_owned(),
    )
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

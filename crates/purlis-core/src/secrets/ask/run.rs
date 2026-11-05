// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Run grant, and the ask that raises one** (ADR 0111, spec #5235, #5239): the person's
//! allowance, given in the window, for one program a persona does not declare to receive the keys
//! its command names, from one vault.
//!
//! A brokered run of an undeclared program checks here for a grant ([`Asks::allowed_or_held`]).
//! With none, it raises an ask only the person answers, and waits as a run missing a key waits:
//!
//! - **What a grant binds is [`Bound`]**: the chat, the program as the app resolved it where no
//!   chat writes, the vault, the variables and keys, and the exact arguments. Allow once is good
//!   for one run of exactly that, until used or the chat ends, so a slow answer is not wasted.
//!   Nothing of it is written: grants live in this process and end with the chat.
//! - **One ask for one command of one chat.** The same command run again joins the open ask, and a
//!   changed one asks again, up to [`MOST_OPEN_PER_CHAT`] open at once, so a chat cannot bury the
//!   person in notices.
//! - **Allow for this chat (#5241) binds all of that but the arguments**: the same program and
//!   the same variables and keys, with any arguments, for the chat's life. A key added asks again.
//!   It is listed with Allow once's not yet used ([`Asks::given_runs`]), and taken back from the
//!   next run ([`Asks::revoke_run`]): a run already going is not stopped, since it holds its
//!   values by then.
//! - **Don't allow is remembered for the chat, the program and the vault**, as a decline is: that
//!   chat is told so at once from then on, whatever the arguments, and nothing is asked. What the
//!   chat was given of them goes with it.
//! - **Only the window answers** ([`Asks::answer_run`]), as for every Secret ask.
//!
//! An open ask outlives a restart of the app as a Secret ask does, since a command that stopped
//! waiting was told to wait for the answer. Its command passed the mask before it was raised
//! ([`crate::secrets::brokered`]), so it is written as it is. A grant does not: it is asked again.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;

use super::{Asks, Inner, now_secs};
use crate::askoutcome::{About, Answer, Outcome, Scope};
use crate::harness::asks::{Answerer, HumanScope};

/// **The most asks to allow a program one chat holds open at once**: past it the chat hears it
/// waits on the person, and nothing more reaches them.
pub const MOST_OPEN_PER_CHAT: usize = 3;

/// **What a Run grant binds**: all the app's record or resolved by it, but the arguments, which
/// are the chat's and are matched exactly. `env` and `file` are `VAR=key` as `secret exec` spells
/// them, sorted, so a grant does not hang on their order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bound {
    pub chat: u32,
    /// The program as it runs: found by the app and with its links resolved, where no chat
    /// writes ([`crate::sandbox::tools::for_run_grant`]).
    pub program: PathBuf,
    pub vault: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

impl Bound {
    /// Whether it hands over the same values to the same program from the same vault in the same
    /// chat, whatever the arguments.
    fn same_but_args(&self, other: &Self) -> bool {
        (
            &self.chat,
            &self.program,
            &self.vault,
            &self.env,
            &self.file,
        ) == (
            &other.chat,
            &other.program,
            &other.vault,
            &other.env,
            &other.file,
        )
    }

    /// The program's file name: how the chat and its nudge name it.
    pub fn program_name(&self) -> String {
        self.program
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }
}

/// **What the app can say of where a run's values could go**, read from the chat's own sandbox.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "reach", content = "hosts", rename_all = "snake_case")]
pub enum Hosts {
    /// The hosts its proxy carries.
    Listed(Vec<String>),
    /// The chat's sandbox is at the floor: no proxy holds the run to any host.
    Every,
}

/// **What a run of an undeclared program asks of the person** (module docs). Every field is the
/// app's but `command`, which is the chat's words made inert
/// ([`crate::secrets::brokered::command_shown`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunNeed {
    pub bound: Bound,
    pub persona: String,
    pub hosts: Hosts,
    /// T-4's sentence, where the program is a general-purpose one
    /// ([`crate::sandbox::tools::caution`]).
    pub caution: Option<String>,
    pub command: String,
}

/// **An open ask to allow a program, as the window shows it**: names only, never a value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OpenRun {
    pub id: u32,
    pub persona: String,
    pub bound: Bound,
    pub hosts: Hosts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caution: Option<String>,
    pub command: String,
    /// When it was raised, in seconds since 1970.
    pub since: u64,
}

impl OpenRun {
    /// Each key its command hands over, once, in order.
    pub fn keys(&self) -> Vec<String> {
        keys_of(&self.bound)
    }
}

/// Each key `bound` hands over, once, sorted.
fn keys_of(bound: &Bound) -> Vec<String> {
    let mut keys: Vec<String> = bound
        .env
        .iter()
        .chain(&bound.file)
        .filter_map(|spec| spec.split_once('=').map(|(_, key)| key.to_owned()))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// **How the person answered.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunAnswer {
    /// One run of exactly this command.
    Once,
    /// This program with these variables and keys and any arguments, until the chat ends.
    ForChat,
    /// Don't allow: this chat is not asked about this program and vault again.
    Refuse,
}

/// The audit's kind for a Run grant the person gave.
pub const GRANT_AUDITED: &str = "trust.run_grant.grant";
/// The audit's kind for a Run grant taken back.
pub const REVOKE_AUDITED: &str = "trust.run_grant.revoke";
/// The audit's kind for a Don't allow.
pub const REFUSE_AUDITED: &str = "trust.run_grant.refuse";

/// **A Run grant given, taken back or not allowed, as the audit records it** (#5241, ADR 0075 §4):
/// who, the level, the program, the keys, the vault and the chat. **Never a value, and never the
/// command's arguments**, which are the chat's words and may hold anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Audited<'a> {
    kind: &'static str,
    level: &'static str,
    bound: &'a Bound,
}

impl<'a> Audited<'a> {
    /// The person's `answer` on an ask for `bound`.
    pub fn granted(bound: &'a Bound, answer: RunAnswer) -> Self {
        let (kind, level) = match answer {
            RunAnswer::Once => (GRANT_AUDITED, "once"),
            RunAnswer::ForChat => (GRANT_AUDITED, "chat"),
            RunAnswer::Refuse => (REFUSE_AUDITED, "chat"),
        };
        Self { kind, level, bound }
    }

    /// The person took back `bound`'s grant, which was good `once` or for the chat.
    pub fn revoked(bound: &'a Bound, once: bool) -> Self {
        Self {
            kind: REVOKE_AUDITED,
            level: if once { "once" } else { "chat" },
            bound,
        }
    }

    /// The event's kind.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// The event's body.
    pub fn body(&self) -> Value {
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "local-ui",
            "level": self.level,
            "program": self.bound.program.display().to_string(),
            "keys": keys_of(self.bound),
            "vault": self.bound.vault,
            "chat": self.bound.chat,
        })
    }
}

/// **A Run grant the chat has not used up** ([`Asks::given_runs`]), for the Granted list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GivenRun {
    pub id: u32,
    pub bound: Bound,
    /// Good for one run of exactly its arguments; otherwise for any, for the chat's life.
    pub once: bool,
    pub at: u64,
}

/// What an answer did: the chats to nudge, whose command stopped waiting before it, and what each
/// is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAnswered {
    pub nudge: Vec<u32>,
    pub told: Outcome,
}

/// What a run of an undeclared program comes to ([`Asks::allowed_or_held`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunHeld {
    /// A grant covers it, or the person just gave one: it runs.
    Allowed,
    /// It was not allowed: nothing ran, and `line` says so.
    Refused { line: String },
    /// Nobody answered before the hold ran out, or the chat has as many open as it may: the
    /// ask stays open and the chat is nudged when it is answered. `line` is what it is told.
    StillOpen { line: String },
}

/// How a run that waits is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunSettled {
    Allowed,
    /// Allow once went to another run of the same command.
    TakenByAnother,
    Refused,
}

#[derive(Debug)]
struct RunWaiter {
    /// Where its command hears the answer; none once it stopped waiting, and is nudged instead.
    tell: Option<mpsc::Sender<RunSettled>>,
    n: u64,
}

#[derive(Debug)]
pub(super) struct RunAsked {
    open: OpenRun,
    waiters: Vec<RunWaiter>,
}

/// One grant the person gave.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Given {
    /// What Revoke is sent by: numbered with the asks, so never one of theirs.
    id: u32,
    bound: Bound,
    /// Good for one run of exactly its arguments; otherwise for any, for the chat's life.
    once: bool,
    at: u64,
}

/// The Run grants half of [`Inner`].
#[derive(Debug, Default)]
pub(super) struct Runs {
    open: Vec<RunAsked>,
    given: Vec<Given>,
    /// What each chat was not allowed: by chat, program and vault, for the chat's life.
    refused: std::collections::HashSet<(u32, PathBuf, String)>,
}

impl Runs {
    /// Takes what covers `bound`, where a grant does: Allow once goes as it is taken.
    fn take(&mut self, bound: &Bound) -> bool {
        let Some(at) = self.given.iter().position(|given| {
            if given.once {
                given.bound == *bound
            } else {
                given.bound.same_but_args(bound)
            }
        }) else {
            return false;
        };
        if self.given[at].once {
            self.given.remove(at);
        }
        true
    }

    /// What the app keeps of its open asks across a restart.
    pub(super) fn kept(&self) -> Vec<OpenRun> {
        self.open.iter().map(|asked| asked.open.clone()).collect()
    }

    /// Takes back `open`, from what an earlier run of the app kept: none of its commands waits
    /// now, so each chat is nudged.
    pub(super) fn keep(&mut self, open: OpenRun) {
        self.open.push(RunAsked {
            open,
            waiters: Vec::new(),
        });
    }

    /// Every open ask of a chat in `back`, or none of `gone`'s, stays.
    pub(super) fn retain_chats(&mut self, keep: impl Fn(u32) -> bool) {
        self.open.retain(|asked| keep(asked.open.bound.chat));
        self.given.retain(|given| keep(given.bound.chat));
        self.refused.retain(|(chat, _, _)| keep(*chat));
    }

    /// The highest number any kept ask holds.
    pub(super) fn highest(&self) -> u32 {
        self.open
            .iter()
            .map(|asked| asked.open.id)
            .max()
            .unwrap_or(0)
    }
}

/// What a chat hears as its command starts to wait.
fn asked_said(bound: &Bound, within: Duration) -> String {
    format!(
        "purlis asked the person in its window to allow {} with vault {}: this command waits for \
         their answer for up to {} seconds",
        bound.program_name(),
        bound.vault,
        within.as_secs()
    )
}

/// What a chat hears whose command stopped waiting.
fn still_open_said(bound: &Bound) -> String {
    format!(
        "waiting on the person to allow {} with vault {}; you will be told their answer, so run \
         this command again only then",
        bound.program_name(),
        bound.vault
    )
}

/// What a chat hears that has as many asks open as it may.
fn too_many_said(bound: &Bound) -> String {
    format!(
        "this chat already waits on the person for {MOST_OPEN_PER_CHAT} commands, so purlis did \
         not ask about {} too; you will be told their answers, so run it again only then",
        bound.program_name()
    )
}

/// What a chat hears of a program the person did not allow.
fn refused_said(bound: &Bound) -> String {
    format!(
        "the person did not allow {} with vault {} in this chat, so nothing ran: don't run it \
         again or work around it; go on without it",
        bound.program_name(),
        bound.vault
    )
}

/// What a second run of one command hears where Allow once went to the first.
fn taken_said(bound: &Bound) -> String {
    format!(
        "the person allowed {} with vault {} for one run, and another run of this command took \
         it, so this one did not run",
        bound.program_name(),
        bound.vault
    )
}

impl Asks {
    /// **Whether `need`'s command may run**, asking the person where no grant covers it, and
    /// holding the command at most `within` (module docs). `say` tells the command, as it starts
    /// to wait, what it waits on.
    pub fn allowed_or_held(
        &self,
        need: RunNeed,
        within: Duration,
        say: &mut dyn FnMut(&str),
    ) -> RunHeld {
        let RunNeed {
            bound,
            persona,
            hosts,
            caution,
            command,
        } = need;
        let (answered, n, id) = {
            let mut inner = self.inner();
            // A grant comes first: a Don't allow given later takes it away ([`Self::answer_run`]).
            if inner.runs.take(&bound) {
                return RunHeld::Allowed;
            }
            if inner.runs.refused.contains(&(
                bound.chat,
                bound.program.clone(),
                bound.vault.clone(),
            )) {
                return RunHeld::Refused {
                    line: refused_said(&bound),
                };
            }
            inner.next_waiter += 1;
            let n = inner.next_waiter;
            let (tell, answered) = mpsc::channel();
            let waiter = RunWaiter {
                tell: Some(tell),
                n,
            };
            let id = if let Some(asked) = inner
                .runs
                .open
                .iter_mut()
                .find(|asked| asked.open.bound == bound)
            {
                asked.waiters.push(waiter);
                asked.open.id
            } else {
                let open_here = inner
                    .runs
                    .open
                    .iter()
                    .filter(|asked| asked.open.bound.chat == bound.chat)
                    .count();
                if open_here >= MOST_OPEN_PER_CHAT {
                    return RunHeld::StillOpen {
                        line: too_many_said(&bound),
                    };
                }
                inner.next += 1;
                let id = inner.next;
                inner.runs.open.push(RunAsked {
                    open: OpenRun {
                        id,
                        persona,
                        bound: bound.clone(),
                        hosts,
                        caution,
                        command,
                        since: now_secs(),
                    },
                    waiters: vec![waiter],
                });
                self.save(&inner);
                id
            };
            (answered, n, id)
        };
        say(&asked_said(&bound, within));
        let settled_as = |settled: RunSettled| match settled {
            RunSettled::Allowed => RunHeld::Allowed,
            RunSettled::TakenByAnother => RunHeld::Refused {
                line: taken_said(&bound),
            },
            RunSettled::Refused => RunHeld::Refused {
                line: refused_said(&bound),
            },
        };
        match answered.recv_timeout(within) {
            Ok(settled) => settled_as(settled),
            Err(mpsc::RecvTimeoutError::Disconnected) => RunHeld::StillOpen {
                line: still_open_said(&bound),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut inner = self.inner();
                let still = inner
                    .runs
                    .open
                    .iter_mut()
                    .find(|asked| asked.open.id == id)
                    .and_then(|asked| asked.waiters.iter_mut().find(|waiter| waiter.n == n))
                    .map(|waiter| waiter.tell = None)
                    .is_some();
                drop(inner);
                // Answered as the hold ran out: that answer stands.
                if !still && let Ok(settled) = answered.try_recv() {
                    return settled_as(settled);
                }
                RunHeld::StillOpen {
                    line: still_open_said(&bound),
                }
            }
        }
    }

    /// **Whether a grant covers `bound`**, with nothing asked: Allow once goes as it is taken.
    pub fn granted(&self, bound: &Bound) -> bool {
        self.inner().runs.take(bound)
    }

    /// **Every Run grant given and not used up**, oldest first, for the Granted list.
    pub fn given_runs(&self) -> Vec<GivenRun> {
        let inner = self.inner();
        let mut given: Vec<GivenRun> = inner
            .runs
            .given
            .iter()
            .map(|one| GivenRun {
                id: one.id,
                bound: one.bound.clone(),
                once: one.once,
                at: one.at,
            })
            .collect();
        given.sort_by_key(|one| (one.at, one.id));
        given
    }

    /// **Takes back Run grant `id`**, from `by`: refused from anywhere but the window. It applies
    /// from the chat's next run; one going holds its values already and is not stopped.
    pub fn revoke_run(&self, id: u32, by: Answerer) -> Result<GivenRun, String> {
        if by.scope() != HumanScope::LocalUi {
            return Err("a run grant is taken back in purlis's window only".to_owned());
        }
        let mut inner = self.inner();
        let Some(at) = inner.runs.given.iter().position(|one| one.id == id) else {
            return Err("purlis did not remove it: it is no longer there.".to_owned());
        };
        let gone = inner.runs.given.remove(at);
        drop(inner);
        Ok(GivenRun {
            id: gone.id,
            bound: gone.bound,
            once: gone.once,
            at: gone.at,
        })
    }

    /// **Every ask to allow a program open now**, longest waiting first.
    pub fn open_runs(&self) -> Vec<OpenRun> {
        let inner = self.inner();
        let mut open: Vec<OpenRun> = inner.runs.kept();
        open.sort_by_key(|one| (one.since, one.id));
        open
    }

    /// **The person answers the ask to allow a program `id`**, from `by`: refused from anywhere
    /// but the window, leaving it open. A run that holds on it goes on, or is told; one whose
    /// command stopped waiting is nudged. Allow once is given to the first run holding on it, or
    /// kept for the chat's next run of exactly that command.
    pub fn answer_run(
        &self,
        id: u32,
        answer: RunAnswer,
        by: Answerer,
    ) -> Result<RunAnswered, String> {
        if by.scope() != HumanScope::LocalUi {
            return Err("a run grant is given in purlis's window only".to_owned());
        }
        let mut inner = self.inner();
        let Some(at) = inner.runs.open.iter().position(|asked| asked.open.id == id) else {
            return Err(format!("secret ask {id} is no longer open"));
        };
        let asked = inner.runs.open.remove(at);
        let bound = asked.open.bound.clone();
        let mut held = false;
        let reach = |settled: RunSettled, waiter: RunWaiter| {
            waiter.tell.is_some_and(|tell| tell.send(settled).is_ok())
        };
        let answer_word = match answer {
            RunAnswer::Once => {
                for waiter in asked.waiters {
                    if held {
                        reach(RunSettled::TakenByAnother, waiter);
                    } else {
                        held = reach(RunSettled::Allowed, waiter);
                    }
                }
                if !held {
                    inner.next += 1;
                    let id = inner.next;
                    inner.runs.given.push(Given {
                        id,
                        bound: bound.clone(),
                        once: true,
                        at: now_secs(),
                    });
                }
                Answer::Allowed { scope: Scope::Once }
            }
            RunAnswer::ForChat => {
                // Every run holding on this ask goes on, and every other this grant covers.
                let mut covered = asked.waiters;
                let mut at = 0;
                while at < inner.runs.open.len() {
                    if inner.runs.open[at].open.bound.same_but_args(&bound) {
                        covered.extend(inner.runs.open.remove(at).waiters);
                    } else {
                        at += 1;
                    }
                }
                for waiter in covered {
                    held |= reach(RunSettled::Allowed, waiter);
                }
                inner.next += 1;
                let id = inner.next;
                inner.runs.given.push(Given {
                    id,
                    bound: bound.clone(),
                    once: false,
                    at: now_secs(),
                });
                Answer::Allowed { scope: Scope::Chat }
            }
            RunAnswer::Refuse => {
                for waiter in asked.waiters {
                    held |= reach(RunSettled::Refused, waiter);
                }
                inner
                    .runs
                    .refused
                    .insert((bound.chat, bound.program.clone(), bound.vault.clone()));
                inner.runs.given.retain(|one| {
                    one.bound.chat != bound.chat
                        || one.bound.program != bound.program
                        || one.bound.vault != bound.vault
                });
                Answer::KeptBlocked { reason: None }
            }
        };
        self.save(&inner);
        drop(inner);
        self.trace_run(&bound, id, answer);
        Ok(RunAnswered {
            nudge: if held { Vec::new() } else { vec![bound.chat] },
            told: Outcome {
                about: About::Run {
                    program: bound.program_name(),
                    vault: bound.vault,
                },
                answer: answer_word,
            },
        })
    }

    /// Records the answer in the chat's trace: names, never a value.
    fn trace_run(&self, bound: &Bound, id: u32, answer: RunAnswer) {
        crate::trace::record_values(
            &self.root,
            &bound.chat.to_string(),
            super::TRACED_AS,
            &[
                (
                    "what",
                    Value::from(match answer {
                        RunAnswer::Once => "allowed once",
                        RunAnswer::ForChat => "allowed for this chat",
                        RunAnswer::Refuse => "not allowed",
                    }),
                ),
                ("ask", Value::from(id)),
                ("vault", Value::from(bound.vault.as_str())),
                ("program", Value::from(bound.program.display().to_string())),
                ("chat", Value::from(bound.chat)),
            ],
            chrono::Local::now().naive_local(),
        );
    }

    /// Chat `chat` has closed: what it was given, what it was not, and what it asked go with it.
    pub(super) fn forget_runs(inner: &mut Inner, chat: u32) {
        inner.runs.retain_chats(|one| one != chat);
    }
}

#[cfg(test)]
mod tests;

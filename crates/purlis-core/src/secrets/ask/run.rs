// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Run grant, and the ask that raises one** (ADR 0112, spec #5235, #5239): the person's
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
//! - **Wider than one command only for a reviewed program** (the person's ruling on ADR 0112):
//!   Allow for this chat and Always allow are refused here, as well as not offered, for a program
//!   off [`crate::sandbox::tools::reviewed_for_any_arguments`]'s list.
//! - **Allow for this chat (#5241) binds all of that but the arguments**: the same program and
//!   the same variables and keys, with any arguments, for the chat's life. A key added asks again.
//!   It is listed with Allow once's not yet used ([`Asks::given_runs`]), and taken back from the
//!   next run ([`Asks::revoke_run`]): a run already going is not stopped, since it holds its
//!   values by then.
//! - **The vault comes with the grant, for its scope alone** (the operator's ruling on ADR 0112).
//!   A run covered by one may read a vault its persona may not otherwise use. Nothing is written
//!   for that, so another chat, a differing command and a revoked grant all stand as before. Only
//!   Always allow gives the persona the vault, as an Allow of its own.
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
    /// The name it was asked for by, which a program of several names goes by.
    #[serde(default)]
    pub name: String,
    /// The program as it runs: found by the app and with its links resolved, where no chat
    /// writes ([`crate::sandbox::tools::for_run_grant`]).
    pub program: PathBuf,
    pub vault: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file: Vec<String>,
    /// Where it starts. Part of what Allow once names, and shown; a grant for the chat is for any
    /// folder, as it is for any arguments, since the chat's folders are all one to it.
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

impl Bound {
    /// **Whether a grant of any arguments may be given for it** (the person's ruling on ADR 0112):
    /// by the name it was asked by and the program it resolved to, both on the reviewed list.
    pub fn reviewed_for_any_arguments(&self) -> bool {
        use crate::sandbox::tools::reviewed_for_any_arguments as reviewed;
        reviewed(&self.name) && reviewed(&self.program_name())
    }

    /// Whether it hands over the same values to the same program from the same vault in the same
    /// chat, whatever the arguments.
    fn same_but_args(&self, other: &Self) -> bool {
        (
            &self.chat,
            &self.name,
            &self.program,
            &self.vault,
            &self.env,
            &self.file,
        ) == (
            &other.chat,
            &other.name,
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

/// **A key the run misses, as its notice's value field** (#5242): the vault holds no value under
/// it, or it is a link whose entry is gone. Names only, never a value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fill {
    pub key: String,
    /// What the persona's definition says the key is for, where it declares it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<String>,
    /// Why it is asked though the vault has it: a link whose entry is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<super::Reason>,
}

/// **What a run's one notice offers** (#5242, ADR 0112): an Allow for the program, or, where it has
/// none, the vault's own Allow, or only its value fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Offer {
    /// Allow once, for this chat or always, and Don't allow: a program no tool declares.
    #[default]
    Program,
    /// **No program Allow, and a vault the persona may not use** (a declared tool, or a lock):
    /// the vault's line has its own Allow, as a refused vault's notice has (#5160), which gives
    /// the persona the vault; and Keep blocked. `no_allow` says why, where policy locks the run.
    Vault {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        no_allow: Option<String>,
    },
    /// **Its value fields alone**: Fill and Not now. `no_allow` says why, where policy locks it.
    Fill {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        no_allow: Option<String>,
    },
}

impl Offer {
    /// Why the notice offers no Allow for the program, where policy is why.
    pub fn no_allow(&self) -> Option<&str> {
        match self {
            Self::Program => None,
            Self::Vault { no_allow } | Self::Fill { no_allow } => no_allow.as_deref(),
        }
    }
}

/// **What a run asks of the person** (module docs): one notice for every gate it hit. Every field
/// is the app's but `command`, which is the chat's words made inert
/// ([`crate::secrets::brokered::command_shown`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunNeed {
    /// **Asked ahead of its run** (#5244, ADR 0112 amended 2026-11-06): a chat's `secret_ask`
    /// naming a reviewed program, with no command yet. Only Allow for this chat is offered, and
    /// its grant binds the chat, program, vault and exactly [`Self::fills`]' keys; the first run
    /// it covers fixes the variables they are handed by.
    pub ahead: bool,
    /// The keys the run misses, one value field each (#5242).
    pub fills: Vec<Fill>,
    /// What the notice offers (#5242).
    pub offer: Offer,
    /// **The vault is not one the persona may use** (ADR 0112): an Allow opens it for that grant's
    /// scope alone, and only Always allow gives it to the persona.
    pub not_its_vault: bool,
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
    /// Asked ahead of its run ([`RunNeed::ahead`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ahead: bool,
    /// The keys the run misses ([`RunNeed::fills`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fills: Vec<Fill>,
    /// What the notice offers ([`RunNeed::offer`]).
    #[serde(default)]
    pub offer: Offer,
    /// The vault is not one the persona may use ([`RunNeed::not_its_vault`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub not_its_vault: bool,
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
    /// Each key its command hands over, once, sorted: asked ahead of its run, with no variables
    /// yet, its fields' keys, which its grant would cover (#5244).
    pub fn keys(&self) -> Vec<String> {
        if !self.ahead {
            return keys_of(&self.bound);
        }
        fill_keys(&self.fills)
    }
}

/// Each key `fills` names, once, sorted.
fn fill_keys(fills: &[Fill]) -> Vec<String> {
    let mut keys: Vec<String> = fills.iter().map(|fill| fill.key.clone()).collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Each key `bound` hands over, once, sorted.
pub fn keys_of(bound: &Bound) -> Vec<String> {
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
    /// As [`Self::ForChat`], the caller having declared it as the persona's tool (#5245).
    Always,
    /// Don't allow: this chat is not asked about this program and vault again. On a notice with
    /// no program Allow, Keep blocked or Not now.
    Refuse,
    /// The vault's own Allow, on a notice with no program Allow (#5242): the caller has given the
    /// persona the vault, as a refused vault's Allow does.
    AllowVault,
    /// Fill, on a notice of value fields alone (#5242).
    Fill,
}

impl RunAnswer {
    /// Whether a notice offering `offer` offers it.
    pub fn is_offered_by(self, offer: &Offer) -> bool {
        match offer {
            Offer::Program => matches!(
                self,
                Self::Once | Self::ForChat | Self::Always | Self::Refuse
            ),
            Offer::Vault { .. } => matches!(self, Self::AllowVault | Self::Refuse),
            Offer::Fill { .. } => matches!(self, Self::Fill | Self::Refuse),
        }
    }

    /// Whether the run goes on, so every field must be filled first.
    pub fn goes_on(self) -> bool {
        self != Self::Refuse
    }
}

/// What a chat is told of an Allow at `scope`, where values were stored or not.
fn allowed(filled: bool, scope: Scope) -> Answer {
    if filled {
        Answer::FilledAndAllowed { scope }
    } else {
        Answer::Allowed { scope }
    }
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
    /// The keys it covers where its bound has no variables yet ([`Self::handing`]).
    keys: Option<&'a [String]>,
}

impl<'a> Audited<'a> {
    /// The person's `answer` on an ask for `bound`, where it is a Run grant's: none for the vault's
    /// own Allow, audited as a vault's, or a Fill, which grants nothing.
    pub fn granted(bound: &'a Bound, answer: RunAnswer) -> Option<Self> {
        let (kind, level) = match answer {
            RunAnswer::Once => (GRANT_AUDITED, "once"),
            RunAnswer::ForChat => (GRANT_AUDITED, "chat"),
            RunAnswer::Always => (GRANT_AUDITED, "project"),
            RunAnswer::Refuse => (REFUSE_AUDITED, "chat"),
            RunAnswer::AllowVault | RunAnswer::Fill => return None,
        };
        Some(Self {
            kind,
            level,
            bound,
            keys: None,
        })
    }

    /// The same, naming `keys` as what it covers: a grant asked ahead of its run (#5244), whose
    /// bound has no variables until its first run fixes them ([`OpenRun::keys`]).
    #[must_use]
    pub fn handing(self, keys: &'a [String]) -> Self {
        Self {
            keys: Some(keys),
            ..self
        }
    }

    /// The person took back `bound`'s grant, which was good `once` or for the chat.
    pub fn revoked(bound: &'a Bound, once: bool) -> Self {
        Self {
            kind: REVOKE_AUDITED,
            level: if once { "once" } else { "chat" },
            bound,
            keys: None,
        }
    }

    /// An Always allow audited as given whose line could not be written, so it gave nothing.
    pub fn revoked_always(bound: &'a Bound) -> Self {
        Self {
            kind: REVOKE_AUDITED,
            level: "project",
            bound,
            keys: None,
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
            "keys": self.keys.map_or_else(|| keys_of(self.bound), <[String]>::to_vec),
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
    /// **Given ahead of its run** (#5244), the keys it covers before a run binds them: what its
    /// audit named, and so what its taking back names too (#5617).
    pub ahead_keys: Option<Vec<String>>,
}

/// What Keep blocked did to the asks (#5617): the asks open on the vault, for the window to answer
/// Don't allow, and the grants it took back, for the window to audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptBlocked {
    pub open: Vec<u32>,
    pub taken_back: Vec<GivenRun>,
}

/// What an answer did: the chats to nudge, whose command stopped waiting before it, and what each
/// is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAnswered {
    pub nudge: Vec<u32>,
    pub told: Outcome,
}

/// What [`Asks::fill_run`] stored: the keys, and each chat that stopped waiting on a Secret ask
/// for one of them, with what it is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFilled {
    pub stored: Vec<String>,
    pub nudge: Vec<(u32, Outcome)>,
}

/// What a run of an undeclared program comes to ([`Asks::allowed_or_held`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunHeld {
    /// A grant covers it, or the person just gave one: it runs, once [`Asks::granted`] still
    /// says so as it starts.
    Allowed,
    /// The person filled its values, on a notice that offered no program Allow: it goes on where
    /// nothing else stops it.
    Filled,
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
    /// Its values are stored, and nothing was to be allowed.
    Filled,
    /// Its values are stored, and the person did not allow the program.
    FilledNotAllowed,
    /// Not now: nothing stored, nothing allowed.
    Declined,
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
    /// The keys of its fields the person stored a value under ([`Asks::fill_run`]).
    stored: Vec<String>,
}

/// One grant the person gave.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Given {
    /// What Revoke is sent by: numbered with the asks, so never one of theirs.
    id: u32,
    bound: Bound,
    /// **Given ahead of its run** (#5244): the keys it covers, by any variables, until the first
    /// run it covers fixes them into [`Self::bound`]. None once fixed, or for any other grant.
    ahead_keys: Option<Vec<String>>,
    /// Good for one run of exactly its arguments; otherwise for any, for the chat's life.
    once: bool,
    at: u64,
}

impl Given {
    /// Whether it covers `bound`: Allow once exactly; a grant ahead its keys, by any variables;
    /// any other whatever the arguments.
    fn covers(&self, bound: &Bound) -> bool {
        if let Some(keys) = &self.ahead_keys {
            // The same program, vault and chat, handing exactly those keys, by any variables.
            (
                &self.bound.chat,
                &self.bound.name,
                &self.bound.program,
                &self.bound.vault,
            ) == (&bound.chat, &bound.name, &bound.program, &bound.vault)
                && keys_of(bound) == *keys
        } else if self.once {
            self.bound == *bound
        } else {
            self.bound.same_but_args(bound)
        }
    }
}

/// The Run grants half of [`Inner`].
#[derive(Debug, Default)]
pub(super) struct Runs {
    open: Vec<RunAsked>,
    given: Vec<Given>,
    /// What each chat was not allowed: by chat, program and vault, for the chat's life.
    refused: std::collections::HashSet<(u32, PathBuf, String)>,
    /// The vaults the person kept blocked for each chat (#5424), for the chat's life: no ask
    /// that would open one opens, whatever the gate read before.
    kept_blocked: std::collections::HashSet<(u32, String)>,
}

impl Runs {
    /// Where the grant that covers `bound` is held, if one does.
    fn covers(&self, bound: &Bound) -> Option<usize> {
        self.given.iter().position(|given| given.covers(bound))
    }

    /// Takes what covers `bound`, where a grant does: Allow once goes as it is taken. Whether it
    /// was an Allow once, which the Allowed list then no longer lists (#5800).
    fn take(&mut self, bound: &Bound) -> Option<bool> {
        let at = self.covers(bound)?;
        let once = self.given[at].once;
        if once {
            self.given.remove(at);
        } else if self.given[at].ahead_keys.take().is_some() {
            // The first run fixes how the values are handed: from now on, only those variables.
            self.given[at].bound.env.clone_from(&bound.env);
            self.given[at].bound.file.clone_from(&bound.file);
        }
        Some(once)
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
            stored: Vec::new(),
        });
    }

    /// How many grants are held.
    pub(super) fn given_count(&self) -> usize {
        self.given.len()
    }

    /// Every open ask of a chat in `back`, or none of `gone`'s, stays.
    pub(super) fn retain_chats(&mut self, keep: impl Fn(u32) -> bool) {
        self.open.retain(|asked| keep(asked.open.bound.chat));
        self.given.retain(|given| keep(given.bound.chat));
        self.refused.retain(|(chat, _, _)| keep(*chat));
        self.kept_blocked.retain(|(chat, _)| keep(*chat));
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

/// What a chat hears as its command starts to wait: what it was asked about, but which keys a
/// vault its persona may not use lacks, which it is not told.
fn asked_said(
    bound: &Bound,
    offer: &Offer,
    keys: &[String],
    not_its_vault: bool,
    within: Duration,
) -> String {
    let fill = match (keys, not_its_vault) {
        ([], _) => String::new(),
        (_, true) => " and to fill what it lacks".to_owned(),
        (keys, false) => format!(" and to fill {}", keys.join(", ")),
    };
    let what = match offer {
        Offer::Program => format!(
            "to allow {} with vault {}{fill}",
            bound.program_name(),
            bound.vault
        ),
        Offer::Vault { .. } => format!("to let it use vault {}{fill}", bound.vault),
        Offer::Fill { .. } => format!("to fill {} in vault {}", keys.join(", "), bound.vault),
    };
    format!(
        "purlis asked the person in its window {what}: this command waits for their answer for \
         up to {} seconds",
        within.as_secs()
    )
}

/// What a chat hears whose values the person stored and whose program they did not allow.
fn filled_not_allowed_said(bound: &Bound) -> String {
    format!(
        "the person filled what this command lacked in vault {}, but did not allow {} in this \
         chat, so nothing ran: don't run it again or work around it",
        bound.vault,
        bound.program_name()
    )
}

/// What a chat hears where the person chose Not now.
fn declined_all_said(bound: &Bound, keys: &[String]) -> String {
    match keys {
        [key] => super::declined_said(&bound.vault, key),
        _ => format!(
            "the person declined to fill {} in vault {} for this chat, so nothing ran: don't ask \
             for them again in this chat; go on without them",
            keys.join(", "),
            bound.vault
        ),
    }
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
fn kept_blocked_said(bound: &Bound) -> String {
    format!(
        "the person kept vault {} blocked for this chat, so {} did not run: don't try it again",
        bound.vault,
        bound.program_name()
    )
}

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
            ahead,
            fills,
            offer,
            not_its_vault,
            bound,
            persona,
            hosts,
            caution,
            command,
        } = need;
        let keys: Vec<String> = fills.iter().map(|fill| fill.key.clone()).collect();
        let (answered, n, id) = {
            let mut inner = self.inner();
            // A grant comes first: a Don't allow given later takes it away ([`Self::answer_run`]).
            // It is used only as the run starts ([`Self::granted`]), so a run that stops short of
            // that, for a full slot or a value it waits on, leaves Allow once for its retry.
            if offer == Offer::Program && inner.runs.covers(&bound).is_some() {
                return RunHeld::Allowed;
            }
            // Not now on a key is a decline, as a Secret ask's is: not asked again in this chat.
            if let Some(key) = keys.iter().find(|key| {
                inner
                    .declined
                    .contains(&(bound.chat, bound.vault.clone(), (*key).clone()))
            }) {
                return RunHeld::Refused {
                    line: super::declined_said(&bound.vault, key),
                };
            }
            // Kept blocked since the gate read it (#5424): the run hears what the gate says.
            if not_its_vault
                && inner
                    .runs
                    .kept_blocked
                    .contains(&(bound.chat, bound.vault.clone()))
            {
                return RunHeld::Refused {
                    line: kept_blocked_said(&bound),
                };
            }
            if offer == Offer::Program
                && inner.runs.refused.contains(&(
                    bound.chat,
                    bound.program.clone(),
                    bound.vault.clone(),
                ))
            {
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
            let id = if let Some(asked) = inner.runs.open.iter_mut().find(|asked| {
                // Ahead of its run a bound names no variables: its fields say which keys.
                asked.open.bound == bound
                    && asked.open.offer == offer
                    && asked.open.ahead == ahead
                    && (!ahead || fill_keys(&asked.open.fills) == fill_keys(&fills))
            }) {
                asked.waiters.push(waiter);
                let id = asked.open.id;
                // Traced before the lock goes, as a Secret ask's is (#5220, #5509): an answer is
                // traced only once the ask is taken, under the lock.
                self.trace_run_said(&bound, id, "joined");
                id
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
                        ahead,
                        fills,
                        offer: offer.clone(),
                        not_its_vault,
                        id,
                        persona,
                        bound: bound.clone(),
                        hosts,
                        caution,
                        command,
                        since: now_secs(),
                    },
                    waiters: vec![waiter],
                    stored: Vec::new(),
                });
                self.save(&inner);
                self.trace_run_said(&bound, id, "raised");
                id
            };
            (answered, n, id)
        };
        say(&asked_said(&bound, &offer, &keys, not_its_vault, within));
        let settled_as = |settled: RunSettled| match settled {
            RunSettled::Allowed => RunHeld::Allowed,
            RunSettled::Filled => RunHeld::Filled,
            RunSettled::FilledNotAllowed => RunHeld::Refused {
                line: filled_not_allowed_said(&bound),
            },
            RunSettled::Declined => RunHeld::Refused {
                line: declined_all_said(&bound, &keys),
            },
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

    /// **The person kept `vault` blocked for chat `chat`** (#5424): from now on, for the chat's
    /// life, an ask that only the vault's own Allow would answer is refused as it would open,
    /// under the lock it opens under, so a run the gate let past just before cannot raise one.
    /// A grant the chat was given on the vault goes with it, so a run the gate let past is not
    /// covered either. The asks open on it already are returned, for the window to answer Don't
    /// allow.
    pub fn keep_vault_blocked(&self, chat: u32, vault: &str) -> Vec<u32> {
        self.keep_vault_blocked_taking(chat, vault).open
    }

    /// [`Self::keep_vault_blocked`], answering also the grants it took back (#5617), for the
    /// window to audit each as taken back, as a grant removed from the Allowed list is.
    pub fn keep_vault_blocked_taking(&self, chat: u32, vault: &str) -> KeptBlocked {
        let mut inner = self.inner();
        inner.runs.kept_blocked.insert((chat, vault.to_owned()));
        let (taken, kept): (Vec<Given>, Vec<Given>) = std::mem::take(&mut inner.runs.given)
            .into_iter()
            .partition(|given| given.bound.chat == chat && given.bound.vault == vault);
        inner.runs.given = kept;
        let open = inner
            .runs
            .open
            .iter()
            .filter(|asked| {
                asked.open.not_its_vault
                    && asked.open.bound.chat == chat
                    && asked.open.bound.vault == vault
            })
            .map(|asked| asked.open.id)
            .collect();
        KeptBlocked {
            open,
            taken_back: taken
                .into_iter()
                .map(|given| GivenRun {
                    id: given.id,
                    bound: given.bound,
                    once: given.once,
                    at: given.at,
                    ahead_keys: given.ahead_keys,
                })
                .collect(),
        }
    }

    /// **Records that policy refused a run of `program`** (ADR 0112, #5243): closed words in the
    /// event log, and the names in the chat's own trace.
    pub fn refused_run_by_policy(&self, chat: u32, program: &str, vault: &str) {
        crate::trace::record_values(
            &self.root,
            &chat.to_string(),
            super::TRACED_AS,
            &[
                ("what", Value::from("run grant refused by policy")),
                ("vault", Value::from(vault)),
                ("program", Value::from(program)),
                ("chat", Value::from(chat)),
            ],
            chrono::Local::now().naive_local(),
        );
        self.tell(
            Some(chat),
            crate::eventlog::secretasks::Step::Refused {
                by: crate::eventlog::secretasks::By::RunGrantPolicy,
                declared: false,
            },
        );
    }

    /// **Drops an Allow once given for exactly `bound`**, unused: a check after the Allow
    /// refused its run, so no retry may use it (the person's ruling, #5242). A run that only
    /// waited keeps it.
    pub fn drop_once(&self, bound: &Bound) {
        let dropped = {
            let mut inner = self.inner();
            let before = inner.runs.given.len();
            inner
                .runs
                .given
                .retain(|given| !(given.once && given.bound == *bound));
            inner.runs.given.len() != before
        };
        if dropped {
            self.run_grants_moved();
        }
    }

    /// **Whether the person did not allow `bound`'s program with its vault in its chat**, which
    /// is not asked about again (#5244).
    pub fn refused_run(&self, bound: &Bound) -> bool {
        self.inner().runs.refused.contains(&(
            bound.chat,
            bound.program.clone(),
            bound.vault.clone(),
        ))
    }

    /// **Whether a grant covers `bound`**, with nothing asked and nothing used up.
    pub fn covers(&self, bound: &Bound) -> bool {
        self.inner().runs.covers(bound).is_some()
    }

    /// **Whether a grant for the chat's life covers `bound`**, whatever its arguments.
    pub fn covers_for_the_chat(&self, bound: &Bound) -> bool {
        self.inner()
            .runs
            .given
            .iter()
            .any(|given| !given.once && given.covers(bound))
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
                ahead_keys: one.ahead_keys.clone(),
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
            ahead_keys: gone.ahead_keys,
        })
    }

    /// **Every ask to allow a program open now**, longest waiting first.
    pub fn open_runs(&self) -> Vec<OpenRun> {
        let inner = self.inner();
        let mut open: Vec<OpenRun> = inner.runs.kept();
        open.sort_by_key(|one| (one.since, one.id));
        open
    }

    /// **The person answers run ask `id`**, from `by`: refused from anywhere but the window, leaving
    /// it open, and refused for an answer its notice does not offer. A run that holds on it goes
    /// on, or is told; one whose command stopped waiting is nudged. Allow once is kept for the
    /// chat's next run of exactly that command, and the first run holding on it is told to take it
    /// ([`Self::granted`]). The values its fields ask for are stored first ([`Self::fill_run`]):
    /// whatever the person answers, what they stored stays, and the chat is told so.
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
            return Err("This ask has already been answered, or its chat has closed.".to_owned());
        };
        if !answer.is_offered_by(&inner.runs.open[at].open.offer) {
            return Err("That answer isn't offered on this notice.".to_owned());
        }
        // **Kept blocked for its chat** (#5617): nothing that opens the vault is given, under the
        // lock Keep blocked marks it under, however far the press got before the mark. The ask
        // stays open for Keep blocked to answer.
        let bound = &inner.runs.open[at].open.bound;
        if answer.goes_on()
            && inner.runs.open[at].open.not_its_vault
            && inner
                .runs
                .kept_blocked
                .contains(&(bound.chat, bound.vault.clone()))
        {
            return Err(format!(
                "You kept vault {} blocked for this chat, so nothing was allowed.",
                bound.vault
            ));
        }
        // Ahead of its run there is no command to bind: no Allow once, and no Always allow.
        if inner.runs.open[at].open.ahead
            && !matches!(answer, RunAnswer::ForChat | RunAnswer::Refuse)
        {
            return Err(
                "Ahead of its run, a program is allowed for this chat or not at all: there is no \
                 command yet to allow once."
                    .to_owned(),
            );
        }
        if matches!(answer, RunAnswer::ForChat | RunAnswer::Always)
            && !inner.runs.open[at].open.bound.reviewed_for_any_arguments()
        {
            return Err(format!(
                "{} is allowed one exact command at a time: purlis has not reviewed it for any \
                 arguments. Press Allow once, or declare it as a tool.",
                inner.runs.open[at].open.bound.program_name()
            ));
        }
        // Every field filled before anything is allowed or filled: the run reads them all.
        if answer.goes_on() {
            let asked = &inner.runs.open[at];
            if let Some(fill) = asked
                .open
                .fills
                .iter()
                .find(|fill| !asked.stored.contains(&fill.key))
            {
                return Err(format!(
                    "Type a value for {}, or choose {}.",
                    fill.key,
                    match asked.open.offer {
                        Offer::Program => "Don't allow",
                        Offer::Vault { .. } => "Keep blocked",
                        Offer::Fill { .. } => "Not now",
                    }
                ));
            }
        }
        let asked = inner.runs.open.remove(at);
        let bound = asked.open.bound.clone();
        let filled = !asked.stored.is_empty();
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
                // Kept until a run takes it as it starts: the run told it may still stop short.
                inner.next += 1;
                let id = inner.next;
                inner.runs.given.push(Given {
                    id,
                    bound: bound.clone(),
                    ahead_keys: None,
                    once: true,
                    at: now_secs(),
                });
                allowed(filled, Scope::Once)
            }
            RunAnswer::ForChat | RunAnswer::Always => {
                let ahead_keys: Option<Vec<String>> = asked.open.ahead.then(|| {
                    asked
                        .open
                        .fills
                        .iter()
                        .map(|fill| fill.key.clone())
                        .collect()
                });
                // Every run holding on this ask goes on, and every other this grant covers.
                let mut covered = asked.waiters;
                let mut at = 0;
                while at < inner.runs.open.len() {
                    let other = &inner.runs.open[at];
                    if other.open.offer == Offer::Program
                        && other.open.fills.is_empty()
                        && other.open.bound.same_but_args(&bound)
                    {
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
                    ahead_keys,
                    once: false,
                    at: now_secs(),
                });
                // Always allow too: this chat is what it promises the chat, since each machine
                // and every other chat stands on its own Allow.
                allowed(filled, Scope::Chat)
            }
            RunAnswer::AllowVault => {
                for waiter in asked.waiters {
                    held |= reach(RunSettled::Allowed, waiter);
                }
                allowed(
                    filled,
                    Scope::Persona {
                        persona: asked.open.persona.clone(),
                    },
                )
            }
            RunAnswer::Fill => {
                for waiter in asked.waiters {
                    held |= reach(RunSettled::Filled, waiter);
                }
                Answer::Filled
            }
            RunAnswer::Refuse => match asked.open.offer {
                Offer::Program => {
                    let settled = if filled {
                        RunSettled::FilledNotAllowed
                    } else {
                        RunSettled::Refused
                    };
                    // Not asked again of this program and vault: its other commands open now
                    // close too.
                    let mut refused = asked.waiters;
                    let mut at = 0;
                    while at < inner.runs.open.len() {
                        let one = &inner.runs.open[at].open;
                        if one.offer == Offer::Program
                            && one.bound.chat == bound.chat
                            && one.bound.program == bound.program
                            && one.bound.vault == bound.vault
                        {
                            refused.extend(inner.runs.open.remove(at).waiters);
                        } else {
                            at += 1;
                        }
                    }
                    for waiter in refused {
                        held |= reach(settled, waiter);
                    }
                    inner.runs.refused.insert((
                        bound.chat,
                        bound.program.clone(),
                        bound.vault.clone(),
                    ));
                    inner.runs.given.retain(|one| {
                        one.bound.chat != bound.chat
                            || one.bound.program != bound.program
                            || one.bound.vault != bound.vault
                    });
                    if filled {
                        Answer::FilledNotAllowed
                    } else {
                        Answer::KeptBlocked { reason: None }
                    }
                }
                Offer::Vault { .. } => {
                    for waiter in asked.waiters {
                        held |= reach(RunSettled::Refused, waiter);
                    }
                    Answer::KeptBlocked { reason: None }
                }
                // Not now, as on a Secret ask: this chat is not asked for these keys again.
                Offer::Fill { .. } => {
                    for waiter in asked.waiters {
                        held |= reach(RunSettled::Declined, waiter);
                    }
                    for fill in &asked.open.fills {
                        inner
                            .declined
                            .insert((bound.chat, bound.vault.clone(), fill.key.clone()));
                    }
                    Answer::Declined
                }
            },
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

    /// **Stores what the person typed in run ask `id`'s value fields** (#5242), from `by`, in the
    /// project `ctx` is of: each through the vault's provider, as a Secret ask's fill is, and
    /// never over a value that came to be there meanwhile. Every chat waiting on a Secret ask for
    /// the same key is settled as filled. Answers the keys it stored. No value is held past its
    /// write, and none is in what it answers.
    pub fn fill_run(
        &self,
        ctx: &super::Ctx,
        id: u32,
        values: Vec<(String, super::Zeroizing<String>)>,
        by: Answerer,
    ) -> Result<RunFilled, String> {
        self.fill_run_with(ctx, id, values, Vec::new(), by)
    }

    /// **[`Self::fill_run`], with a field filled by an entry the person keeps** (#5080): each of
    /// `links` makes its field's key a Linked entry reading the entry, never a copy, as a Secret
    /// ask's link does ([`super::Answer::Link`]), and under the same rule as a value: only where
    /// the vault holds nothing under the key, or a link whose entry is gone. A field is given a
    /// value or an entry, never both. Links are written first, then values; one that fails stops
    /// there, leaving what was written before it, which a press again finds filled. The caller
    /// judges every entry before this is called.
    pub fn fill_run_with(
        &self,
        ctx: &super::Ctx,
        id: u32,
        values: Vec<(String, super::Zeroizing<String>)>,
        links: Vec<(String, super::super::links::Target)>,
        by: Answerer,
    ) -> Result<RunFilled, String> {
        if by.scope() != HumanScope::LocalUi {
            return Err("a secret ask is answered in purlis's window only".to_owned());
        }
        let (vault, fills) = {
            let inner = self.inner();
            let Some(asked) = inner.runs.open.iter().find(|asked| asked.open.id == id) else {
                return Err(
                    "This ask has already been answered, or its chat has closed.".to_owned(),
                );
            };
            (asked.open.bound.vault.clone(), asked.open.fills.clone())
        };
        for (key, _) in &links {
            if !fills.iter().any(|fill| fill.key == *key) {
                return Err(format!("{key} is not a value this notice asks for"));
            }
            if values
                .iter()
                .any(|(typed, value)| typed == key && !value.is_empty())
            {
                return Err(format!(
                    "{key} has a value typed and an entry picked: keep one"
                ));
            }
        }
        let mut stored = Vec::new();
        for (key, to) in links {
            let Some(fill) = fills.iter().find(|fill| fill.key == key) else {
                return Err(format!("{key} is not a value this notice asks for"));
            };
            let v = super::cmd::provider(ctx, &vault).map_err(|e| e.message.to_string())?;
            match super::cmd::get_value(ctx, &v, &key) {
                // Filled meanwhile, elsewhere: what is there stays.
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind,
                        crate::secrets::Kind::NotFound | crate::secrets::Kind::BrokenLink
                    ) =>
                {
                    super::linked_to(ctx, &vault, &key, fill.reason.as_ref(), &to)
                        .map_err(|e| e.message.to_string())?;
                }
                Err(e) => return Err(e.message.to_string()),
            }
            stored.push(key);
        }
        for (key, value) in values {
            let Some(fill) = fills.iter().find(|fill| fill.key == key) else {
                return Err(format!("{key} is not a value this notice asks for"));
            };
            if value.is_empty() {
                continue;
            }
            let v = super::cmd::provider(ctx, &vault).map_err(|e| e.message.to_string())?;
            let now = super::cmd::get_value(ctx, &v, &key);
            match now {
                // Filled meanwhile, elsewhere: what is there stays.
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind,
                        crate::secrets::Kind::NotFound | crate::secrets::Kind::BrokenLink
                    ) =>
                {
                    super::filled_with(ctx, &vault, &key, fill.reason.as_ref(), &value)
                        .map_err(|e| e.message.to_string())?;
                }
                Err(e) => return Err(e.message.to_string()),
            }
            stored.push(key);
        }
        let mut inner = self.inner();
        if let Some(asked) = inner.runs.open.iter_mut().find(|asked| asked.open.id == id) {
            for key in &stored {
                if !asked.stored.contains(key) {
                    asked.stored.push(key.clone());
                }
            }
        }
        drop(inner);
        let mut nudge = Vec::new();
        for key in &stored {
            for chat in self.settle_filled(&vault, key) {
                nudge.push((
                    chat,
                    Outcome {
                        about: About::Secret {
                            vault: vault.clone(),
                            key: key.clone(),
                        },
                        answer: Answer::Filled,
                    },
                ));
            }
        }
        Ok(RunFilled { stored, nudge })
    }

    /// **Takes what covers `bound`, as its run starts** (#5242): Allow once goes as it is taken, so
    /// it is used by the run it was for and no earlier.
    pub fn granted(&self, bound: &Bound) -> bool {
        let taken = self.inner().runs.take(bound);
        if taken == Some(true) {
            self.run_grants_moved();
        }
        taken.is_some()
    }

    /// Records the answer in the chat's trace: names, never a value.
    fn trace_run(&self, bound: &Bound, id: u32, answer: RunAnswer) {
        self.trace_run_said(
            bound,
            id,
            match answer {
                RunAnswer::Once => "allowed once",
                RunAnswer::ForChat => "allowed for this chat",
                RunAnswer::Always => "always allowed",
                RunAnswer::Refuse => "not allowed",
                RunAnswer::AllowVault => "vault allowed",
                RunAnswer::Fill => "filled",
            },
        );
    }

    /// Records `what` of Run grant ask `id` in the chat's trace (#5509): raised, joined or its
    /// answer, by names, never a value.
    fn trace_run_said(&self, bound: &Bound, id: u32, what: &str) {
        crate::trace::record_values(
            &self.root,
            &bound.chat.to_string(),
            super::TRACED_AS,
            &[
                ("what", Value::from(what)),
                ("ask", Value::from(id)),
                ("vault", Value::from(bound.vault.as_str())),
                ("program", Value::from(bound.program.display().to_string())),
                ("chat", Value::from(bound.chat)),
            ],
            chrono::Local::now().naive_local(),
        );
    }

    /// Chat `chat` has closed: what it was given, what it was not, and what it asked go with it.
    /// Whether it was given any grant, which the Allowed list then no longer lists (#5800).
    pub(super) fn forget_runs(inner: &mut Inner, chat: u32) -> bool {
        let held = inner
            .runs
            .given
            .iter()
            .any(|given| given.bound.chat == chat);
        inner.runs.retain_chats(|one| one != chat);
        held
    }
}

#[cfg(test)]
mod tests;

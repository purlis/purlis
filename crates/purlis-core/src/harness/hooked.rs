// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A level-2 ask: a harness's permission hook, held open on the hook channel until the operator
//! answers it from the window, and answered back on that same hook (HP-6, research track 04
//! §5.2).
//!
//! Claude Code runs `charter hook permissionrequest` when it would ask for a permission. The
//! hook hands its payload to the host on the hook channel and waits; the host raises it in the
//! shared [`Asks`] and holds the connection. The operator's answer goes back on that connection
//! as the option they chose, and the hook prints the harness's own decision for it
//! ([`decision`]). Nothing is typed into the chat's terminal, so the pane need not have focus.
//!
//! **What decides, when nobody answers.** The ask carries a deadline below the hook's timeout
//! ([`HOOK_TIMEOUT`], [`super::model::Deadline::below_hook_timeout`]). Past it, and whenever the
//! hook goes away first, the hook prints nothing and the harness's own prompt decides in the
//! pane. charter never allows or denies on its own.
//!
//! **Only a human answers** ([`Answerer`], V16, V75). On the hook channel a chat can raise an
//! ask and never answer one: the host reads no answer there, and the hook believes a reply
//! only from a listener that is its own ancestor ([`crate::hookwire::permission`]).
//!
//! **An answer is bound to its chat.** The registry is keyed by ask and chat together
//! ([`Asks::answer`]), so an answer naming another chat, or one replayed after the first, is
//! refused and never reaches a hook.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::asks::{Answerer, Applied, AskId, Asks, Raised, Refused};
use super::model::{Action, Ask, Choice, ChoiceKind, ChoiceScope, Writes};

/// The word after `charter hook` that a permission hook runs.
pub const WORD: &str = "permissionrequest";

/// How long the harness waits on charter's permission hook before it gives up on it: Claude
/// Code's own default for a hook. The ask's deadline sits [`super::model::HOOK_MARGIN`] below it.
pub const HOOK_TIMEOUT: Duration = Duration::from_secs(60);

/// The most open hook asks one chat may hold at once, as at level 3 ([`crate::acp`]): one past
/// it is not raised, and the harness's own prompt decides it.
pub const MOST_OPEN_ASKS: usize = crate::acp::MOST_OPEN_ASKS;

/// The most a permission hook's payload may be. A larger one is not raised and not cut short:
/// a command shown for approval without its tail is not the command that runs.
pub const MOST_ASK_BYTES: usize = crate::acp::MOST_ASK_BYTES;

/// The harness whose permission hook an ask came on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Claude Code's `PermissionRequest`.
    ClaudeCode,
    /// Codex's `PermissionRequest`, armed on the chat's own session flags (#1691).
    Codex,
    /// opencode's `permission.asked`, handed on by purlis's shim, which replies on opencode's
    /// own client with what the hook prints (#1691).
    Opencode,
}

impl Source {
    /// The source of a permission hook run in a chat the app started as `harness` (#1703):
    /// what the host reads an ask by, whatever the chat's environment says.
    pub fn of(harness: crate::harness::Harness) -> Self {
        use crate::harness::Harness;
        match harness {
            Harness::ClaudeCode => Self::ClaudeCode,
            Harness::Codex => Self::Codex,
            Harness::Opencode => Self::Opencode,
        }
    }

    /// The source of a permission hook run in a chat of `harness`, the registry name the app
    /// put into the chat's environment ([`crate::hookwire::HARNESS_ENV`]): Claude Code where
    /// it names none, as a chat started before the variable was set does. A name this does not
    /// know is no source, and the hook then decides nothing: a decision printed in another
    /// harness's words is one its harness may misread.
    ///
    /// What it picks is only how the ask is read and how the hook prints its answer: a hook
    /// that names another harness than its own shows the person a different ask, which they
    /// answer or not, and prints its harness a decision it cannot read.
    pub fn of_harness(harness: Option<&str>) -> Option<Self> {
        crate::harness::Harness::of_chat(harness).map(Self::of)
    }
}

/// Who asked a permission hook's ask (#1644): the chat itself, one of its helpers by agent id,
/// or unread, where the payload names an agent in a shape this does not read. An answer in the
/// window clears only the prompt of the one who asked ([`crate::state::Board::answered_by`]);
/// an unread asker's answer clears every prompt the chat waits on, as before.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asker {
    Chat,
    Helper(String),
    Unread,
}

impl Asker {
    /// Who asked the ask `payload` is: the helper its `agent_id` names, or the chat itself
    /// where it names none, as a harness's hooks say a helper's call and the chat's own.
    pub fn of(payload: &Value) -> Self {
        match payload.get("agent_id") {
            None | Some(Value::Null) => Self::Chat,
            Some(Value::String(agent)) if !agent.is_empty() => Self::Helper(agent.clone()),
            Some(_) => Self::Unread,
        }
    }
}

/// The ask a permission hook's `payload` is, as the window shows it and the host holds it.
///
/// **"From now on" is never offered** (V28c): a Claude Code suggestion saved to a settings file
/// would write into the worktree's harness settings, which a chat can then read as standing
/// permission. Allow and deny for this call, and what lasts only for the session, are kept.
///
/// **Nothing that lets the call run is offered on a display that hides part of it** (HP-6
/// review). The window shows an ask's one-line summary, which collapses newlines, cuts at
/// [`super::model::SUMMARY_WIDTH`] and masks credential shapes and the vault values this process
/// knows (#1047); a command whose harmless head
/// is all that shows is not the command that runs. Unless the summary IS the action, word for
/// word ([`shown_in_full`]), the allowing options are dropped and the window offers deny, or
/// the chat's own pane, which shows it whole. An edit is allowed from here only where the
/// window shows its whole change beside its path ([`super::model::Writes::Whole`], #1146), and
/// then only this once: a rule or a directory for the session would let later edits run that
/// nobody was shown. An edit whose source names only its path is never allowed here. A shell call that carries anything beyond its
/// command, description and timeout (`dangerouslyDisableSandbox`, `run_in_background`, a field
/// a later harness adds) runs differently from what the line says, so it is not allowed from
/// here either.
///
/// **No option that switches the session's permission mode is ever offered** (D-88o): only
/// allow and deny for this call, and rules for this session, come from the window. A mode
/// switch (`bypassPermissions`, `acceptEdits`, any `setMode`) stays in the chat's own pane.
///
/// **An opencode command or edit is never allowed from here** (#1691): its ask names the
/// commands opencode matched (`patterns`), not the line that runs, and an edit only its path.
///
/// **A Claude Code question is answered, never allowed** (#1703): its options are its answers,
/// offered only where the window shows the question and every answer whole
/// ([`super::asked::question`]), and the hook sends the chosen one back as the tool's own.
pub fn ask(source: Source, payload: &Value) -> Ask {
    ask_within(source, payload, HOOK_TIMEOUT)
}

/// [`ask`], for a hook the harness gives up on `left` from now: its deadline sits
/// [`super::model::HOOK_MARGIN`] below that, and with no room left the ask has no deadline and
/// no options, so only the pane can answer it.
fn ask_within(source: Source, payload: &Value, left: Duration) -> Ask {
    let mut ask = match source {
        Source::ClaudeCode => super::asked::claude_permission_request(payload, left),
        Source::Codex => super::asked::codex_permission_request(payload, left),
        Source::Opencode => super::asked::opencode_permission_hooked(payload, left),
    };
    // An option whose own words would draw otherwise (a rule's text, a directory's path) says
    // something other than what it grants, so it is not offered either.
    ask.options.retain(|option| {
        option.scope != ChoiceScope::Always
            && !switches_the_mode(source, payload, option)
            && !purlis_unicode::hides(&option.label, purlis_unicode::Breaks::Marked)
            // Nor one whose words carry a vault value this process knows (#1047): it is not
            // shown, and a label with a mask in it does not say what it grants.
            && !crate::secrets::seen::holds(&option.label)
    });
    // An edit is allowed this once from here, on the change shown, and never by a rule or a
    // directory that would let later edits run that nobody was shown (#1146).
    if matches!(ask.action, Action::Edit { .. }) {
        ask.options
            .retain(|option| option.kind != ChoiceKind::Allow || option.id == "allow");
    }
    let opencode_line = source == Source::Opencode && !matches!(ask.action, Action::Tool { .. });
    if !shown_in_full(&ask) || !plain_shell_input(&ask, payload) || opencode_line {
        ask.options
            .retain(|option| option.kind != ChoiceKind::Allow);
    }
    ask
}

/// Whether choosing `option` would switch the session's permission mode: a Claude Code
/// suggestion of type `setMode`.
fn switches_the_mode(source: Source, payload: &Value, option: &Choice) -> bool {
    match source {
        Source::ClaudeCode => option
            .id
            .strip_prefix("suggestion:")
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| payload["permission_suggestions"].get(n))
            .is_some_and(|entry| entry["type"].as_str() == Some("setMode")),
        // Neither offers a mode: Codex's hook answers allow or deny, opencode's once, always
        // for the patterns it names, or reject.
        Source::Codex | Source::Opencode => false,
    }
}

/// The keys a shell call's input may carry and still run as its line says.
const PLAIN_SHELL_KEYS: [&str; 3] = ["command", "description", "timeout"];

/// Whether a command ask's tool input holds nothing beyond [`PLAIN_SHELL_KEYS`]. Any other
/// action has its whole input on the line already, so this is about commands alone.
fn plain_shell_input(ask: &Ask, payload: &Value) -> bool {
    if !matches!(ask.action, Action::Command { .. }) {
        return true;
    }
    payload["tool_input"].as_object().is_some_and(|input| {
        input
            .keys()
            .all(|key| PLAIN_SHELL_KEYS.contains(&key.as_str()))
    })
}

/// Whether `ask`'s summary says everything its action would run, exactly, and draws it as it
/// is: the command line or the tool's whole input, with nothing collapsed, cut or masked, and
/// no character a window draws as nothing or draws elsewhere (a control, a zero-width or
/// format character, a bidirectional override: purlis's one rule, [`purlis_unicode::hidden`]). For an edit, its path, and only where its
/// whole change is shown beside it (#1146); never for an ask that says nothing.
pub fn shown_in_full(ask: &Ask) -> bool {
    let whole = match &ask.action {
        Action::Command { .. } | Action::Tool { .. } => ask.action.said(),
        Action::Question { question } if !question.is_empty() => ask.action.said(),
        Action::Edit { .. } if matches!(ask.writes, Some(Writes::Whole { .. })) => {
            ask.action.said()
        }
        Action::Edit { .. } | Action::Elicit { .. } | Action::Question { .. } | Action::Unsaid => {
            return false;
        }
    };
    ask.summary.as_str() == whole && !purlis_unicode::hides(&whole, purlis_unicode::Breaks::Marked)
}

/// The most combining marks one character carries in ordinary text: as the window's own cap on
/// a line (`MOST_MARKS` in `app/src/combiningMarks.ts`). Past it they draw over the rows around.
pub const MOST_MARKS: usize = 4;

/// Whether `text` holds a run of more than [`MOST_MARKS`] marks that draw over something
/// (non-spacing and enclosing): what a window draws over the rows around it.
pub fn stacks_marks(text: &str) -> bool {
    use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

    let mut run = 0;
    text.chars().any(|c| {
        let mark = matches!(
            c.general_category(),
            GeneralCategory::NonspacingMark | GeneralCategory::EnclosingMark
        );
        run = if mark { run + 1 } else { 0 };
        run > MOST_MARKS
    })
}

/// What the hook prints for the option `chosen` of `payload`'s ask: the harness's own
/// decision, as one line of JSON. `None` for an option the ask does not offer, and the hook then
/// prints nothing, so the harness's prompt decides.
///
/// For opencode it is `{"reply": <word>}`, opencode's own reply word, which purlis's shim hands
/// opencode's client (#1691).
pub fn decision(source: Source, payload: &Value, chosen: &str) -> Option<String> {
    let choice = ask(source, payload)
        .options
        .into_iter()
        .find(|option| option.id == chosen)?;
    let decision = match source {
        Source::ClaudeCode => claude_decision(payload, &choice)?,
        Source::Codex => codex_decision(&choice),
        Source::Opencode => return Some(serde_json::json!({ "reply": choice.id }).to_string()),
    };
    Some(
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": decision,
            }
        })
        .to_string(),
    )
}

/// Codex's `decision` for `choice`: `allow` or `deny` and nothing else, since codex-cli
/// 0.147.0's schema fails closed on `updatedPermissions` and on `interrupt` (#1691).
fn codex_decision(choice: &Choice) -> Value {
    match choice.kind {
        ChoiceKind::Allow => serde_json::json!({ "behavior": "allow" }),
        ChoiceKind::Reject | ChoiceKind::Cancel => serde_json::json!({
            "behavior": "deny",
            "message": "The operator denied this in purlis's window.",
        }),
    }
}

/// Claude Code's `decision` for `choice`: `allow` or `deny`, and for a suggestion, the
/// suggestion itself as the permission update it asks for.
fn claude_decision(payload: &Value, choice: &Choice) -> Option<Value> {
    let behavior = match choice.kind {
        ChoiceKind::Allow => "allow",
        ChoiceKind::Reject | ChoiceKind::Cancel => "deny",
    };
    let mut decision = serde_json::json!({ "behavior": behavior });
    if let Some(input) = super::asked::question::answered(payload, choice) {
        decision["updatedInput"] = input;
    }
    if let Some(n) = choice.id.strip_prefix("suggestion:") {
        let n: usize = n.parse().ok()?;
        let entry = payload["permission_suggestions"].get(n)?.clone();
        decision["updatedPermissions"] = Value::Array(vec![entry]);
    }
    if behavior == "deny" {
        decision["message"] = Value::from("The operator denied this in purlis's window.");
    }
    Some(decision)
}

/// Why a permission hook's ask was not raised. The hook then decides nothing, and the harness's
/// own prompt asks in the pane.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotRaised {
    #[error("this chat already has {MOST_OPEN_ASKS} asks waiting")]
    TooMany,
    #[error("this ask is larger than {MOST_ASK_BYTES} bytes")]
    TooBig,
    /// The hook named another harness than the one the app started the chat as, or the app
    /// started no harness there (#1703): its ask would be read, and its answer printed, in
    /// terms its harness may misread.
    #[error("the hook named another harness than the one this chat was started as")]
    NotItsHarness,
    /// The hook had run so long already that no answer could reach it before its harness gives
    /// up on it (#1063).
    #[error("this hook has too little time left for an answer to reach it")]
    TooLate,
}

/// The harness the app started chat `chat` as, by the source its permission hook reads in, or
/// none where the app started no harness there (#1703).
pub type Recorded = Box<dyn Fn(&str) -> Option<Source> + Send + Sync + 'static>;

/// A raised hook ask, and where its answer arrives: the option the operator chose.
#[derive(Debug)]
pub struct Held {
    pub raised: Raised,
    pub answered: mpsc::Receiver<Choice>,
}

/// The hook asks a host holds open, over the [`Asks`] it shares with every other source.
#[derive(Debug)]
pub struct HookAsks {
    asks: Arc<Asks>,
    /// Each open hook ask, the chat that raised it, and where its answer goes.
    waiting: Mutex<HashMap<AskId, Waiter>>,
    /// The app's own record of each chat's harness, once it is wired ([`Self::reading_by`]).
    recorded: Mutex<Option<Record>>,
}

/// [`Recorded`], held: said only as whether it is wired.
struct Record(Recorded);

impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Record(..)")
    }
}

#[derive(Debug)]
struct Waiter {
    chat: String,
    /// Who asked it (#1644), read from the payload as it was raised.
    asker: Asker,
    tell: mpsc::Sender<Choice>,
}

impl HookAsks {
    pub fn new(asks: Arc<Asks>) -> Self {
        Self {
            asks,
            waiting: Mutex::new(HashMap::new()),
            recorded: Mutex::new(None),
        }
    }

    /// Reads each ask from now on by the harness the app recorded for its chat (#1703),
    /// rather than by the one the chat's hook names: one that names another, or a chat with no
    /// harness of record, is not raised, and the harness's own prompt asks in the pane. Before
    /// this is wired, an ask is read by the hook's word, as nothing else is known yet.
    pub fn reading_by(&self, recorded: Recorded) {
        *self
            .recorded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Record(recorded));
    }

    /// Whether `chat`'s hook may be read as `source`: the harness of record, where one is wired.
    fn its_harness(&self, chat: &str, source: Source) -> bool {
        self.recorded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_none_or(|Record(recorded)| recorded(chat) == Some(source))
    }

    fn waiting(&self) -> std::sync::MutexGuard<'_, HashMap<AskId, Waiter>> {
        self.waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Raises the ask `payload` is, from chat `chat`'s permission hook, at `now`, read as
    /// `source`: only where that is the harness the app recorded for the chat
    /// ([`Self::reading_by`]). For a hook that says nothing of how long it has run: its
    /// deadline is counted from now ([`Self::raise_after`]).
    pub fn raise(
        &self,
        chat: &str,
        source: Source,
        payload: &Value,
        now: Instant,
    ) -> Result<Held, NotRaised> {
        self.raise_after(chat, source, payload, Duration::ZERO, now)
    }

    /// [`Self::raise`], for a hook that had already run `ran_for` when its ask was raised
    /// (#1063): the harness's timeout counts from the hook's start, so the ask's deadline does
    /// too, and an ask raised late in its hook's life expires before the hook times out. A hook
    /// with no room left below its timeout raises nothing ([`NotRaised::TooLate`]).
    ///
    /// The hook says `ran_for` itself, from the chat's side, so it can only shorten its own
    /// ask's deadline, never lengthen it past the hook's timeout: a hook that says too much
    /// leaves its ask to the pane, as a timeout does.
    pub fn raise_after(
        &self,
        chat: &str,
        source: Source,
        payload: &Value,
        ran_for: Duration,
        now: Instant,
    ) -> Result<Held, NotRaised> {
        if payload.to_string().len() > MOST_ASK_BYTES {
            return Err(NotRaised::TooBig);
        }
        if !self.its_harness(chat, source) {
            return Err(NotRaised::NotItsHarness);
        }
        let mut waiting = self.waiting();
        let open = waiting.values().filter(|w| w.chat == chat).count();
        if open >= MOST_OPEN_ASKS {
            return Err(NotRaised::TooMany);
        }
        let left = HOOK_TIMEOUT.saturating_sub(ran_for);
        let asked = ask_within(source, payload, left);
        if asked.deadline == super::model::Deadline::None {
            return Err(NotRaised::TooLate);
        }
        let raised = self.asks.raise(chat, asked, now).raised;
        let (tell, answered) = mpsc::channel();
        waiting.insert(
            raised.id.clone(),
            Waiter {
                chat: chat.to_owned(),
                asker: Asker::of(payload),
                tell,
            },
        );
        Ok(Held { raised, answered })
    }

    /// Answers hook ask `id` of chat `chat` with `choice`, from `by`, at `now`, and sends the
    /// option to the hook that waits on it. The answer and the send happen under one hold, so
    /// a hook that goes away is withdrawn either before the answer (which is then refused) or
    /// after it (and was told).
    ///
    /// An open ask from another source is [`Refused::Unknown`] here: its answer goes back on its
    /// own channel, never on a hook.
    pub fn answer(
        &self,
        chat: &str,
        id: &AskId,
        choice: &str,
        by: Answerer,
        now: Instant,
    ) -> Result<Applied, Refused> {
        let mut waiting = self.waiting();
        if !waiting.contains_key(id) && self.asks.chat_of(id).is_some() {
            return Err(Refused::Unknown);
        }
        let applied = self.asks.answer(chat, id, choice, by, now)?;
        if let Some(waiter) = waiting.remove(id) {
            let _ = waiter.tell.send(applied.choice.clone());
        }
        Ok(applied)
    }

    /// Who asked hook ask `id` (#1644), while it is held; none once it is answered, withdrawn
    /// or timed out, or for an ask from another source.
    pub fn asker_of(&self, id: &AskId) -> Option<Asker> {
        self.waiting().get(id).map(|waiter| waiter.asker.clone())
    }

    /// Withdraws hook ask `id`: its hook went away, or the harness answered it in the pane.
    pub fn withdraw(&self, id: &AskId) -> bool {
        let mut waiting = self.waiting();
        waiting.remove(id);
        self.asks.withdraw(id)
    }

    /// Stops holding every ask past its deadline at `now`, and says which, so each hook can
    /// decide nothing and let the harness's own prompt ask.
    pub fn time_out(&self, now: Instant) -> Vec<AskId> {
        let mut waiting = self.waiting();
        let late = self.asks.expire(now);
        for id in &late {
            waiting.remove(id);
        }
        late
    }

    /// Every ask still waiting at `now`, oldest first.
    pub fn pending(&self, now: Instant) -> Vec<Raised> {
        self.asks.pending(now)
    }

    /// [`Self::pending`], each with when it was raised (#1700).
    pub fn pending_since(&self, now: Instant) -> Vec<(Raised, Instant)> {
        self.asks.pending_since(now)
    }
}

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod question_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod vault_value_tests;

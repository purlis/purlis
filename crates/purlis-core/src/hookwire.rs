// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How a hook process reaches the app: one line on a unix socket the app owns.
//!
//! A hook runs inside the chat's own process tree, so everything it needs is already in its
//! environment — the socket to write to and the chat it belongs to. It looks nothing up and
//! reads no plane, which is what keeps it inside the spec's 50 ms. The one file it may open is
//! its chat's spool beside the socket, when the app does not take its line ([`spool`], FD-30).
//!
//! **It can never break a turn.** Every failure here is silent and fast: no app listening, a
//! socket that has gone, a payload that will not parse. A harness whose turn fails because
//! charter wanted to draw a spinner is worse than a spinner that is wrong.

// Only the unix half reads and writes a socket; `off_unix` names its own.
#[cfg(unix)]
use std::io;

use crate::state::{Detail, Ending, Event, Started};

// The channel is a unix socket. Windows has `AF_UNIX` but Rust's standard library does not
// expose it, and a socket file's `0600` — which is what keeps anything else on the machine
// out of the channel — has no mode bit to set there either. So Windows gets a second answer,
// and until it is written that answer is a REFUSAL and not a quiet no-op: `bind` returns
// `Unsupported`, `send` returns `Unsupported`, and an app that cannot open its hook channel
// says so instead of running with a channel nothing is listening on (charter-app#95).
//
// This used to be a `compile_error!`, which stopped the whole crate at expansion and so hid
// every OTHER thing Windows has to say about the core. What is below says the same thing at
// the same volume and lets the rest of the build be measured (M4).

/// The socket a hook writes to, in the environment of every session the app starts.
pub const SOCKET_ENV: &str = "PURLIS_HOOK_SOCKET";

/// How a hook's stderr begins when the app did not take its line: the hook says so there, and
/// the opencode shim shows that sentence in the chat's window (ruling V73c).
pub const NOT_TAKEN: &str = "purlis: the app did not take this";

/// Which chat the hook is running inside, in the same environment.
///
/// The app's own number for the chat, set at the `exec`, exactly as the Python charter sets
/// `$CHARTER_SESSION_ID` (`charter/hooks.py:_chat_id`). A hook already knows which chat it is
/// in; nothing has to be worked out from a payload.
pub const CHAT_ENV: &str = "PURLIS_CHAT";

/// The chat's own token, in the same environment: what the hook channel checks every line
/// against before it believes the chat number the line names ([`ChatTokens`]).
///
/// The app mints one per chat at the `exec` and keeps it in memory only. It is never passed on
/// to a chat another chat starts ([`NOT_INHERITED`]): a chat the app starts gets its own.
pub const TOKEN_ENV: &str = "PURLIS_CHAT_TOKEN";

/// The chat's own id (its ULID, ADR 0066), in the environment of a chat the app started and
/// of no other: what `purlis statusline` keeps the harness's figure of the session's cost under
/// ([`crate::usage::record_spend`], #1457). Set by the app at the launch, after anything a
/// profile set, so neither a profile nor a chat's own shell names it for the harness. Never
/// inherited ([`NOT_INHERITED`]): an app started from a chat's shell does not hand it on.
pub const CHAT_ID_ENV: &str = "PURLIS_CHAT_ULID";

/// The chat's prompt key (#3369), in the environment of a chat purlis started and of no other:
/// the secret its hooks key the digests of its prompts by ([`crate::skillspan::PromptSalt`]).
/// Minted where the chat is composed, by the window or by the session host, and set there, after
/// anything a profile set. Never on a command line, and never inherited ([`NOT_INHERITED`]): a
/// chat another chat's shell starts gets its own.
pub const PROMPT_KEY_ENV: &str = "PURLIS_PROMPT_KEY";

/// Set to `1` in the environment of a chat the app actually started under a sandbox, and in no
/// other (#1338, #1345). Read through [`crate::sandbox::chat_is_sandboxed`]. The app sets it at
/// the launch, so a chat's own shell cannot claim it for the harness the hooks run under.
/// Never inherited ([`NOT_INHERITED`]), and nor is the chat's folder
/// ([`crate::sandboxblock::CHAT_DIR_ENV`]): an app started from a chat's shell does not hand
/// either on to the chats it starts.
pub const SANDBOXED_ENV: &str = "PURLIS_SANDBOXED";

/// Set to `1` on a chat purlis wraps whole although its harness has a sandbox of its own: a
/// Claude Code chat on a runner (#4083). Read for how a refusal is worded alone
/// ([`crate::sandbox::wrapped_whole_in`]).
pub const WRAPPED_ENV: &str = "PURLIS_CHAT_WRAPPED";

/// Where Claude Code puts the conversation a hook is running in.
///
/// ADR 0024, C7, measured again on claude 2.1.276: it always equals the payload's
/// `session_id`. It is read only as a fallback, and it is what keeps the check against a
/// nested harness standing when a payload cannot be read — a pipe can be slow, an
/// environment cannot. A harness that sets no such variable simply has no fallback.
pub const CLAUDE_CONVERSATION_ENV: &str = "CLAUDE_CODE_SESSION_ID";

/// Where Claude Code puts the pid of the harness a hook is running under.
///
/// It is what tells `/clear` from a nested harness, and the Python charter's
/// `_record_harness_report` turns on it: a new conversation id from the SAME pid is `/clear`
/// (ADR 0024, C6) and the chat follows it; a new id from a DIFFERENT pid is a `claude` run
/// inside the chat (C5) and is ignored. Without it the two are the same event.
pub const CLAUDE_PID_ENV: &str = "CLAUDE_PID";

/// Everything a session must NOT inherit from whatever started charter.
///
/// charter may itself be launched from inside a harness session — an operator running the app
/// from a chat, or a scenario test — and then every chat it starts inherits that harness's
/// identity. A hook would report the LAUNCHER's conversation and process, so the chat's own
/// report would look like somebody else's: either refused as a nested harness, or, worse,
/// taken as one chat's when it is another's.
///
/// A scenario test found this: a fake harness with no identity of its own reported the pid of
/// the Claude Code session that had started the app.
///
/// Only the harness the app starts may set these, and it sets them for its own hooks.
///
/// **Nor where the launcher's chat was** (SI-1): `$CHARTER_WORKSPACE` and
/// `$CHARTER_PLANE_ROOT_SESSION` say where the app started THIS chat, and it sets them itself
/// (`start::ready`). An app launched from a chat pinned to `alpha` would otherwise hand
/// `alpha` to every chat it starts at the plane root, where the pin outranks the root. They are
/// removed before the chat's own are set, so a chat in a workspace still gets its own.
///
/// **Nor the launcher's chat token** ([`TOKEN_ENV`]): each chat gets its own at the `exec`,
/// and one the app inherited is never another chat's, whoever lists it.
///
/// **Nor the host's relaunch marker** ([`crate::noterminal::RELAUNCHED_ENV`]): it names the
/// app's own parent, and an app started from a chat's shell leaves that shell's terminal.
pub const NOT_INHERITED: &[&str] = &[
    CLAUDE_CONVERSATION_ENV,
    CLAUDE_PID_ENV,
    "CLAUDECODE",
    crate::active::WORKSPACE_ENV,
    crate::active::PLANE_ROOT_ENV,
    TOKEN_ENV,
    crate::noterminal::RELAUNCHED_ENV,
    SANDBOXED_ENV,
    WRAPPED_ENV,
    crate::sandboxblock::CHAT_DIR_ENV,
    CHAT_ID_ENV,
    PROMPT_KEY_ENV,
    crate::sandbox::tools::POOL_IMAGE_VAR,
];

/// Which conversation a report is of, and how well that is known.
///
/// **"I could not tell" and "someone is lying" are different evidence, and merging them was a
/// defect.** An earlier version answered `Option<String>` for both, so the chat's own harness
/// having a slow payload looked exactly like a nested one contradicting its environment — and
/// the honest harness's report was dropped, leaving the chat `running` for ever. A review
/// found it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conversation {
    /// The payload and the environment agree, or only one of them exists.
    Named(String),
    /// They disagree. Something is reporting a conversation that is not its own — a harness
    /// nested in the chat's shell is the case ADR 0024's C5 names.
    Contradicted,
    /// A payload that parsed and named no conversation charter recognises.
    ///
    /// **A harness speaking another dialect.** opencode puts it under `sessionID`, not
    /// `session_id` (ADR 0024, O1), and charter has not measured every harness there is. In a
    /// chat that has already adopted one, a report in a dialect charter cannot read is by
    /// definition not that chat's own — a review reproduced a nested `opencode` marking the
    /// outer Claude chat as waiting, mid-turn, because it inherits `CLAUDE_PID` and sets
    /// neither variable of its own.
    Foreign,
    /// Nothing could be read at all: no JSON, because charter's own deadline passed or the
    /// pipe was cut. Not evidence of anything, and never treated as such.
    #[default]
    Unknown,
}

/// What one hook tells the app.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Report {
    /// The app's number for the chat the hook ran in.
    pub chat: u32,
    /// The event the harness fired.
    pub event: Event,
    /// Which conversation the harness says this is.
    ///
    /// The app checks it against the conversation it started the chat under, so a harness
    /// nested INSIDE the chat cannot move the chat's state (ADR 0024, C5).
    #[serde(default)]
    pub conversation: Conversation,
    /// The pid of the harness that fired this, where the harness names one.
    ///
    /// The whole of what tells `/clear` (C6) from a nested harness (C5): both report an id
    /// the chat has not seen, and only the pid says which happened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The sub-agent this came from, where the payload's `agent_id` names one on a harness
    /// where that field was measured to mean a sub-agent (ADR 0066, a child run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// What the harness said about this event beyond its name: what a `SessionStart` was for,
    /// and what a `SessionEnd` was for. Each is meaningless on the other's event and ignored
    /// there.
    #[serde(default)]
    pub detail: Detail,
}

impl Report {
    /// The report a hook makes, read from its environment and its payload.
    ///
    /// `payload` is the harness's JSON on stdin. It is read for one field and never required:
    /// the EVENT comes from argv, which charter wrote itself, so a payload that will not parse
    /// still reports the event — it just cannot say which conversation fired it.
    pub fn read(event: Event, payload: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        // No chat means this harness was not started by the app — the operator's own
        // `claude` in a terminal, with hooks pointed here. There is no chat to move, and
        // inventing one would move somebody else's.
        let chat: u32 = env(CHAT_ENV)?.parse().ok()?;
        let read = serde_json::from_str::<serde_json::Value>(payload).ok();
        let field = |name: &str| {
            read.as_ref()
                .and_then(|payload| payload.get(name))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let said = field("session_id");
        let source = field("source");
        // `/clear` fires `SessionEnd(reason=clear)` and then `SessionStart(source=clear)`
        // from the same process — measured on claude 2.1.276. Without this the first of the
        // two would end the chat and the second could never be heard.
        let reason = field("reason");
        let speaker = Speaker::of(said, read.is_some(), env);
        Some(Self {
            chat,
            event,
            conversation: speaker.conversation,
            pid: speaker.pid,
            agent: sub_agent(field("agent_id").as_deref(), env),
            detail: Detail {
                started: Started::of(source.as_deref()),
                ending: Ending::of(reason.as_deref()),
                // One bit of the prompt, and only on the event that carries one: whether the
                // person typed `/smart-close` (#1332). The prompt never leaves the hook.
                smart_close: event == Event::UserPromptSubmit
                    && field("prompt").is_some_and(|prompt| {
                        crate::state::smart_close_typed(
                            &prompt,
                            chat_harness(env)
                                .as_deref()
                                .and_then(crate::harness::Harness::of_registry_or_kind),
                        )
                    }),
                unattended: crate::floorguard::unattended(field("permission_mode").as_deref()),
                // The mode as the chat's own harness names it (#3787): read by that harness's
                // adapter alone, so a harness whose word is not read says nothing.
                mode: chat_harness(env)
                    .as_deref()
                    .and_then(crate::harness::Harness::of_registry_or_kind)
                    .and_then(|harness| {
                        harness
                            .adapter()
                            .reported_mode(field("permission_mode").as_deref())
                    }),
                // Only a `Stop` of the chat's own can end its turn, so only there is it asked.
                helpers_at_work: event == Event::Stop
                    && chat_harness(env).as_deref() == Some(BACKGROUND_TASKS_HARNESS)
                    && helpers_at_work(read.as_ref()),
                // What it waits on of its own, read on the chat's own wall clock into seconds
                // from now, which the app stamps on its own (#3878).
                own_work: if event == Event::Stop
                    && chat_harness(env).as_deref() == Some(BACKGROUND_TASKS_HARNESS)
                {
                    crate::harness::claude::ownwork::read(
                        read.as_ref(),
                        chrono::Local::now().naive_local(),
                    )
                } else {
                    crate::ownwork::Said::default()
                },
                // Only a `Notification` is a nudge (#1626).
                idle: event == Event::Notification
                    && field("notification_type").as_deref() == Some(IDLE_NUDGE),
                // Which prompt it shows, or that it asks nothing (#1691).
                notified: if event == Event::Notification {
                    notified(field("notification_type").as_deref())
                } else {
                    crate::state::Notified::Unsaid
                },
                // The model the session runs on, only where a `SessionStart` names it (#1021):
                // what a commit's `Assisted-by` then names.
                model: (event == Event::SessionStart)
                    .then(|| field("model"))
                    .flatten()
                    .and_then(|model| crate::state::Model::new(&model)),
                // What the prompt holds, as digests salted by the chat and never its words: how
                // a skill Use skill… typed (#2775) or a curation action's prompt (#2783) is
                // known to be in it, marked only as far as the chat's harness needs (#3029).
                prompt_marks: match event {
                    Event::UserPromptSubmit => field("prompt")
                        .map(|prompt| prompt_marks(&prompt, env))
                        .unwrap_or_default(),
                    _ => crate::skillspan::PromptMarks::default(),
                },
            },
        })
    }

    /// The report as it may be kept on disk: the same report without its prompt's marks, which
    /// are digests of what the person typed and matter only to a host that hears it live.
    pub(crate) fn at_rest(&self) -> Report {
        let mut kept = self.clone();
        kept.detail.prompt_marks = crate::skillspan::PromptMarks::default();
        kept
    }
}

/// **What a hook marks of the prompt the person sent** (SK-38, #3029), under the chat's prompt
/// key ([`crate::skillspan::PromptSalt`], #3369): every stretch of its lines where the chat's harness
/// confirms a typed skill by the prompt (Codex, opencode), and only its lead where the harness
/// names the skill a prompt started (Claude Code), as a curation action's prompt is known by its
/// lead on every harness; a first line of six words or fewer is its own lead, and so is
/// digested whole ([`crate::skillspan::PromptMarks::lead_of`]). Nothing outside a chat that has
/// a prompt key, or on a harness purlis does not know.
fn prompt_marks(
    prompt: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> crate::skillspan::PromptMarks {
    use crate::skillspan::{Confirmed, PromptMarks, PromptSalt};
    let harness = crate::harness::Harness::of_chat(chat_harness(env).as_deref());
    match (
        PromptSalt::read(env),
        harness.map(crate::harness::Harness::typed_start),
    ) {
        (Some(salt), Some(Confirmed::ByPrompt)) => PromptMarks::of(prompt, &salt),
        (Some(salt), Some(Confirmed::ByName)) => PromptMarks::lead_of(prompt, &salt),
        _ => PromptMarks::default(),
    }
}

/// **Which harness run a line says it came from** (#1601): the pid and the conversation a
/// [`Report`] carries, read the same way, for a line that is not a report. The chat's board
/// judges it against the run it adopted (`state::Board::tool_said_by`), so a harness nested in
/// the chat, which holds its token and sits in its process tree, moves nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Speaker {
    /// The pid of the harness, where it names one ([`CLAUDE_PID_ENV`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Which conversation the harness says this is ([`Report::conversation`]'s rule).
    #[serde(default)]
    pub conversation: Conversation,
}

impl Speaker {
    /// Whether it names no run at all: no pid, and no conversation read.
    pub fn is_no_one(&self) -> bool {
        *self == Self::default()
    }

    /// The run a hook ran under, read from its environment and its payload as
    /// [`Report::read`] reads them.
    pub fn read(payload: &str, env: &dyn Fn(&str) -> Option<String>) -> Self {
        let read = serde_json::from_str::<serde_json::Value>(payload).ok();
        let said = read
            .as_ref()
            .and_then(|payload| payload.get("session_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        Self::of(said, read.is_some(), env)
    }

    fn of(said: Option<String>, parsed: bool, env: &dyn Fn(&str) -> Option<String>) -> Self {
        Self {
            pid: env(CLAUDE_PID_ENV)
                .and_then(|pid| pid.parse().ok())
                .filter(|pid| *pid > 0),
            conversation: conversation(said, parsed, env),
        }
    }
}

/// The harness whose `Stop` input was read to say what it has in flight: Claude Code, by the
/// registry name its chats carry in [`HARNESS_ENV`]. Another harness's `background_tasks`, if
/// it ever sent one, is not read as Claude Code's (#1626).
const BACKGROUND_TASKS_HARNESS: &str = "claude-code";

/// The kinds of background work, by Claude Code's label for them, that end by themselves and
/// wake the chat when they do: an agent, and a workflow of agents ([`helpers_at_work`]).
const HELPER_KINDS: [&str; 2] = ["subagent", "workflow"];

/// The statuses of work still in flight: running, or pending ([`helpers_at_work`]). Read from
/// the code that builds the list in Claude Code 2.1.296, which keeps only an entry with one of
/// these two and drops one taken out of the background; any other word is no helper at work.
const IN_FLIGHT: [&str; 2] = ["running", "pending"];

/// The `notification_type` of the nudge a harness sends when a chat sits idle at its prompt, as
/// opposed to a permission or a question: Claude Code's word, which since 2.1.288 it holds back
/// while background agents still run.
const IDLE_NUDGE: &str = "idle_prompt";

/// **What a `Notification`'s `notification_type` says it is** (#1691): the prompt each kind
/// leaves in the harness's terminal, as Claude Code's hooks reference lists them (2.1.296), a
/// notice that the form it showed was answered, or a notice that asks nothing. `question` is purlis's own word, which its opencode shim sends
/// for opencode's question. Any other word, and none, is a prompt of no kind: a notice a later
/// version adds is shown as a wait rather than hidden.
fn notified(kind: Option<&str>) -> crate::state::Notified {
    use crate::harness::model::Prompt;
    use crate::state::Notified;
    Notified::Asks(match kind {
        Some("permission_prompt") => Prompt::Permission,
        Some("elicitation_dialog") => Prompt::Form,
        Some("elicitation_url_dialog") => Prompt::Link,
        Some("agent_needs_input" | "question") => Prompt::Question,
        Some("quota_auto_resume_stale") => Prompt::Continue,
        Some("elicitation_complete" | "elicitation_response") => return Notified::Answered,
        Some(
            "auth_success"
            | "agent_completed"
            | "quota_auto_resume_fired"
            | "quota_auto_resume_disabled",
        ) => return Notified::Informs,
        _ => return Notified::Unsaid,
    })
}

/// **Whether a `Stop` payload says helpers of the chat's own are still at work in the
/// background**, and will wake it when they finish (#1626).
///
/// Claude Code's `Stop` input carries `background_tasks`, the in-flight background work of the
/// session, which its own schema says is there to tell "session is done" from "session is
/// paused waiting for background work to wake it" (read from the hook input schema of
/// 2.1.296; an empty array when nothing is in flight, absent from older versions). Each entry
/// has a `type`, Claude Code's label for the kind of work, and a `status`.
///
/// **Only agents count ([`HELPER_KINDS`]), and only while in flight ([`IN_FLIGHT`]).** Anything
/// not read as one of these is no helper at work.
///
/// **What else a chat waits on of its own is [`crate::harness::claude::ownwork::read`]**
/// (#3878, superseding D-1644-1): its monitors, its background commands and its scheduled
/// wake-ups make its turn's end **waiting**, a state of its own, where this says only whether
/// helpers are at work. Kept for an app older than that reading, which reads this bit alone.
pub fn helpers_at_work(payload: Option<&serde_json::Value>) -> bool {
    payload
        .and_then(|payload| payload.get("background_tasks"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tasks| {
            tasks.iter().any(|task| {
                let is_one_of = |key: &str, words: &[&str]| {
                    task.get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|word| words.contains(&word))
                };
                is_one_of("type", &HELPER_KINDS) && is_one_of("status", &IN_FLIGHT)
            })
        })
}

/// The sub-agent a payload's `agent_id` names, only on a harness where that field was measured
/// to mean one ([`crate::handoffguard::Caller::from_a_subagent`]): elsewhere it is no agent.
pub fn sub_agent(agent_id: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let harness = chat_harness(env);
    let caller = crate::handoffguard::Caller {
        agent_id,
        harness: harness.as_deref(),
        permission_mode: None,
    };
    caller
        .from_a_subagent()
        .then(|| agent_id.unwrap_or_default().to_owned())
}

/// The harness a chat runs, as charter put it into the chat's environment.
pub const HARNESS_ENV: &str = "PURLIS_HARNESS";

/// **The harness the chat's environment `env` names in [`HARNESS_ENV`]**, by the registry's
/// name a chat's start puts there ([`crate::harness::Harness::of_registry_or_kind`]): under either
/// spelling of the variable ([`crate::envvar::lookup`]), and none where it is unset or empty.
/// The one reader of it, so an empty one means the same to every hook and to the MCP server.
pub fn chat_harness(env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    crate::envvar::lookup(HARNESS_ENV, env).filter(|name| !name.is_empty())
}

/// Which conversation this report is of, and how well that is known.
///
/// **The payload and the environment must AGREE, and this used to be an `or`.** That was
/// backwards, and a review proved it: the environment holds the OUTER chat's id — a harness
/// nested in the chat's own shell inherits it — so falling back to it when a payload is slow
/// does not lose the id, it substitutes the wrong one, and the nested harness's report is
/// then taken as the outer chat's. The very case ADR 0024's C5 exists to refuse.
///
/// The Python charter has always required the two to be equal (`charter/hooks.py`), and ADR
/// 0024's consequences say so outright: "A Claude Code report counts only when
/// `CLAUDE_CODE_SESSION_ID` equals its payload's id."
///
/// A harness that sets no such variable — Codex — has nothing to disagree with, so its
/// payload stands alone.
fn conversation(
    said: Option<String>,
    parsed: bool,
    env: &dyn Fn(&str) -> Option<String>,
) -> Conversation {
    // A payload charter READ but could not find a conversation in is a harness speaking a
    // dialect charter has not measured — not the same thing as a payload it could not read at
    // all, and the difference is what keeps a nested one out.
    if said.is_none() && parsed {
        return Conversation::Foreign;
    }
    match (said, env(CLAUDE_CONVERSATION_ENV)) {
        // Claude Code, and the two agree: this is its own hook (ADR 0024, C7).
        (Some(said), Some(here)) if said == here => Conversation::Named(said),
        // Claude Code, and they disagree. Something is reporting a conversation that is not
        // the one its own process is in.
        (Some(_), Some(_)) => Conversation::Contradicted,
        // The environment names one and the payload could not be read at all. That is not a
        // contradiction — it is charter failing to read, and the report must not be punished
        // for it. The pid is what identifies the harness in this case.
        (None, Some(_)) => Conversation::Unknown,
        // No such variable at all: a harness that is not Claude Code. Its payload is all
        // there is, and it is not contradicted by anything.
        (Some(said), None) => Conversation::Named(said),
        (None, None) => Conversation::Unknown,
    }
}

// ----------------------------------------------------------------------------------------
// each chat's token: what the channel checks a line's chat number against
// ----------------------------------------------------------------------------------------

/// One chat's token, as a hook or a `charter` command inside that chat carries it.
///
/// **Its `Debug` never shows the value**, so a line that prints one by mistake prints nothing
/// of it. The value leaves this type in two places only: into the chat's environment at the
/// `exec` ([`ChatToken::expose`]) and onto the wire, beside the line it vouches for.
#[derive(Clone, PartialEq, Eq)]
pub struct ChatToken(String);

impl ChatToken {
    /// The token in `env`'s [`TOKEN_ENV`], where there is one.
    pub fn read(env: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        env(TOKEN_ENV).filter(|value| !value.is_empty()).map(Self)
    }

    /// The token in this process's own environment, where there is one.
    pub fn from_env() -> Option<Self> {
        Self::read(&crate::envvar::var)
    }

    /// The value, for the one caller that must write it down: the chat's environment.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ChatToken {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Debug for ChatToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChatToken(..)")
    }
}

/// How many random bytes a chat's token is: 256 bits from the operating system's generator.
const A_TOKEN_IS: usize = 32;

/// The token each chat the app started was given, by the chat's number. In memory only.
///
/// **The hook channel reads no line without it.** Every line names a chat, and the listener
/// hands a line on only when it carries that chat's token ([`ChatTokens::admits`]); anything
/// else is dropped without a word on the connection, as a line that will not parse is.
///
/// **And from inside the chat** (D-1407-6). A token is in the chat's environment, and on macOS a
/// process of the same user can read another's arguments and environment, so a token alone
/// does not say who sent a line. Each chat's program is recorded as it starts, by pid and by
/// when it started ([`ChatTokens::bind`]), and forgotten when it ends
/// ([`ChatTokens::program_ended`]). A live line is read only from a process that is that
/// program or runs inside it: in its session, or below it ([`descends`]). A chat whose program
/// is not recorded has no line read. A line its hooks spool while the app is away is checked by
/// its token's key alone: the spool is not bound to the program yet.
#[derive(Debug, Default)]
pub struct ChatTokens {
    held: std::sync::Mutex<std::collections::HashMap<u32, ChatToken>>,
    /// Each chat's program, once it has started.
    roots: std::sync::Mutex<std::collections::HashMap<u32, Program>>,
    /// The spool directory each token's key is recorded in as it is issued (FD-30), where
    /// this host has one.
    spool: Option<std::path::PathBuf>,
    /// The connections whose asks are being answered right now, by the number the listener
    /// dealt each: what an answer that takes minutes asks whether its asker is still there
    /// ([`Self::asker_gone`]).
    #[cfg(unix)]
    answering: std::sync::Mutex<std::collections::HashMap<u64, std::os::unix::net::UnixStream>>,
    /// Each chat whose current token's spool key was not recorded as it was issued (#1426),
    /// with that token: recorded at the next chance (`Self::record_unrecorded`). Only a
    /// chat's current token is here, and only while the chat runs, so no key is recorded
    /// after its token was replaced or its chat let it go.
    #[cfg(unix)]
    unrecorded: std::sync::Mutex<std::collections::HashMap<u32, ChatToken>>,
    /// Each chat's armed key (#3822), minted with its token and written into the spool for its
    /// hook alone ([`armed::arm`]): what a report's mode is believed on ([`Self::proves`]).
    #[cfg(unix)]
    armed: std::sync::Mutex<std::collections::HashMap<u32, armed::ArmedKey>>,
}

/// A chat's program: its pid, and when the process with that pid started, so a later process
/// given the same number is not taken for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub pid: u32,
    pub started: String,
}

impl Program {
    /// The process `pid` is now, or `None` where when it started cannot be read: such a program
    /// is never recorded, so its chat has no line read. A program that has already exited is
    /// one, whether or not it has been reaped (on macOS a zombie answers nothing).
    pub fn of(pid: u32) -> Option<Self> {
        #[cfg(unix)]
        {
            Some(Self {
                pid,
                started: purlis_same_user::Parents::started(pid)?,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            None
        }
    }
}

/// What the channel makes of a line's token and the process that sent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// The chat's token, from inside the chat.
    Admitted,
    /// No token, a wrong one, or a chat with none: dropped unread, and answered only with
    /// "this line does not carry chat N's token" (#1333), as a line that will not parse is
    /// answered that the app could not read it. Nothing the line asks is acted on.
    NoToken,
    /// The chat's token, from a process outside the chat: refused, and told so
    /// ([`OUTSIDE_THE_CHAT`]).
    Outside,
    /// The chat's token, for a chat whose program purlis could not confirm as it started:
    /// refused, and told so ([`UNCONFIRMED_PROGRAM`]).
    Unbound,
}

impl Admission {
    /// The sentence a refused line is answered with, where it is answered.
    pub fn refusal(self) -> Option<&'static str> {
        match self {
            Self::Outside => Some(OUTSIDE_THE_CHAT),
            Self::Unbound => Some(UNCONFIRMED_PROGRAM),
            Self::Admitted | Self::NoToken => None,
        }
    }
}

/// What a line for a chat whose program purlis could not confirm is answered with: the chat's
/// own start went wrong, not where the line came from.
pub const UNCONFIRMED_PROGRAM: &str = "purlis couldn't confirm this chat's program when it \
                                       started, so it can't read its lines; restart the chat.";

/// What a line that carried its chat's token from outside the chat is answered with (D-1407-9):
/// a block that says why, never a silence.
pub const OUTSIDE_THE_CHAT: &str = "This came from a process outside the chat (for example tmux, \
                                    nohup after its shell ended, or docker exec); run it from \
                                    the chat's own shell.";

impl ChatTokens {
    /// Tokens whose spool keys are recorded in `dir` as each is issued ([`spool::remember`]),
    /// so a line a chat spools can be checked by whichever host drains it.
    pub fn spooling_into(dir: std::path::PathBuf) -> Self {
        Self {
            held: std::sync::Mutex::default(),
            roots: std::sync::Mutex::default(),
            spool: Some(dir),
            #[cfg(unix)]
            answering: std::sync::Mutex::default(),
            #[cfg(unix)]
            unrecorded: std::sync::Mutex::default(),
            #[cfg(unix)]
            armed: std::sync::Mutex::default(),
        }
    }

    /// Whether the asker on `connection` has gone: it closed its end while its ask was being
    /// answered. What an answer that waits for minutes looks at, so a command that was killed
    /// does not keep a thread of the app's waiting for it (#1441). A connection that is not
    /// being answered here has not gone: a caller that asks about one it made up waits on.
    ///
    /// **An asker says nothing while it is answered**, so anything readable on its connection
    /// is its end of file. A byte it did send out of turn is read as the same: it is not a
    /// purlis this app answers.
    pub fn asker_gone(&self, connection: u64) -> bool {
        #[cfg(unix)]
        {
            use std::io::Read;
            let answering = self
                .answering
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(mut stream) = answering.get(&connection) else {
                return false;
            };
            // A look, not a wait: the shortest timeout a socket takes.
            if stream
                .set_read_timeout(Some(std::time::Duration::from_millis(1)))
                .is_err()
            {
                return true;
            }
            let mut one = [0u8; 1];
            let gone = match stream.read(&mut one) {
                Ok(_) => true,
                Err(why) => !matches!(
                    why.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ),
            };
            let _ = stream.set_read_timeout(Some(A_REPORT_TAKES_AT_MOST));
            gone
        }
        #[cfg(not(unix))]
        {
            let _ = connection;
            false
        }
    }

    /// `stream` is the connection numbered `connection`, whose ask is about to be answered.
    #[cfg(unix)]
    fn answering(&self, connection: u64, stream: std::os::unix::net::UnixStream) {
        self.answering
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(connection, stream);
    }

    /// The ask on `connection` has its answer.
    #[cfg(unix)]
    fn answered(&self, connection: u64) {
        self.answering
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&connection);
    }

    /// A fresh token for `chat`, replacing any it had: 32 bytes from the operating system's
    /// generator, written as hex.
    ///
    /// Its spool key is on disk before it returns, where these tokens spool, and stays there
    /// until the chat ends ([`spool::end_chat`], [`spool::forget_all_but`]). A key that cannot
    /// be recorded is said in the log and the token is issued all the same: the chat runs, and
    /// its key is recorded at the next chance (`Self::record_unrecorded`, #1426). Until then
    /// a line it spools reads as `no-key` at the drain rather than as the chat's. Any other
    /// chat's key still waiting is recorded with this one.
    pub fn issue(&self, chat: u32) -> std::io::Result<ChatToken> {
        let mut bytes = [0u8; A_TOKEN_IS];
        getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
        let token = ChatToken(bytes.iter().map(|byte| format!("{byte:02x}")).collect());
        // Held until the token is the chat's, so a chat let go meanwhile is let go after.
        #[cfg(unix)]
        let mut unrecorded = self.waiting();
        #[cfg(unix)]
        if let Some(dir) = &self.spool {
            // The token this replaces is never recorded now: the drain keeps a chat's newest.
            unrecorded.remove(&chat);
            let others: Vec<(u32, ChatToken)> = unrecorded
                .iter()
                .map(|(chat, token)| (*chat, token.clone()))
                .collect();
            let all: Vec<(u32, &ChatToken)> = std::iter::once((chat, &token))
                .chain(others.iter().map(|(chat, token)| (*chat, token)))
                .collect();
            match spool::remember_all(dir, &all) {
                Ok(()) => unrecorded.clear(),
                Err(why) => {
                    tracing::warn!(
                        "purlis: chat {chat}'s spool key was not recorded ({why}); a line it \
                         spools cannot be checked until it is"
                    );
                    if worth_another_try(&why) {
                        unrecorded.insert(chat, token.clone());
                    }
                }
            }
        }
        self.held().insert(chat, token.clone());
        self.rooted().remove(&chat);
        #[cfg(unix)]
        self.arm(chat);
        Ok(token)
    }

    /// Mints chat `chat`'s armed key and writes it where its hook reads it (#3822), replacing
    /// any it had. A key that cannot be minted or written is said in the log and none is held:
    /// the chat runs, and every mode it reports reads as asking (fail closed).
    #[cfg(unix)]
    fn arm(&self, chat: u32) {
        // The key this start replaces proves nothing from here on, and the file is written with
        // no lock held, so no other chat's line waits on it.
        self.keys().remove(&chat);
        let key = match armed::ArmedKey::new() {
            Ok(key) => key,
            Err(why) => {
                tracing::warn!(
                    "purlis: chat {chat} has no armed key ({why}); its modes read as asking"
                );
                return;
            }
        };
        if let Some(dir) = &self.spool
            && let Err(why) = armed::arm(dir, chat, &key)
        {
            tracing::warn!(
                "purlis: chat {chat}'s armed key was not written ({why}); its modes read as asking"
            );
            return;
        }
        self.keys().insert(chat, key);
    }

    /// Chat `chat` has ended or been let go: its armed key is forgotten and removed.
    fn disarm(&self, chat: u32) {
        #[cfg(unix)]
        {
            self.keys().remove(&chat);
            if let Some(dir) = &self.spool {
                armed::disarm(dir, chat);
            }
        }
        #[cfg(not(unix))]
        let _ = chat;
    }

    /// **Whether `said` is the key chat `chat`'s hook was armed with** (#3822), compared in
    /// constant time: what a report's mode is believed on. A chat holding no key proves
    /// nothing, nor does a line that carries none.
    pub fn proves(&self, chat: u32, said: Option<&str>) -> bool {
        #[cfg(unix)]
        {
            self.keys().get(&chat).is_some_and(|key| key.matches(said))
        }
        #[cfg(not(unix))]
        {
            let _ = (chat, said);
            false
        }
    }

    /// Chat `chat`'s armed key, for a test that stands in for its hook.
    #[cfg(all(unix, any(test, feature = "test-support")))]
    pub fn armed_key(&self, chat: u32) -> Option<armed::ArmedKey> {
        self.keys().get(&chat).cloned()
    }

    #[cfg(unix)]
    fn keys(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, armed::ArmedKey>> {
        self.armed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Records the spool key of each chat whose key was not recorded as its token was issued
    /// (#1426), if the lock on the keys is free now: a line heard from a chat asks this, and
    /// the next token issued records them with its own. Never waits, so a line is never held
    /// up by it; a lock still held leaves them for the next chance. Only a chat's current token is recorded,
    /// and a key already recorded is left as it is, what was drained under it included.
    #[cfg(unix)]
    pub(crate) fn record_unrecorded(&self) {
        let Some(dir) = &self.spool else { return };
        // One that is busy with them (a token being issued) records them itself.
        let mut unrecorded = match self.unrecorded.try_lock() {
            Ok(unrecorded) => unrecorded,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return,
        };
        if unrecorded.is_empty() {
            return;
        }
        let all: Vec<(u32, &ChatToken)> = unrecorded
            .iter()
            .map(|(chat, token)| (*chat, token))
            .collect();
        match spool::remember_now(dir, &all) {
            Ok(()) => {
                let chats: Vec<u32> = unrecorded.keys().copied().collect();
                tracing::info!("purlis: the spool keys of chats {chats:?} are recorded now");
                unrecorded.clear();
            }
            Err(why) if !worth_another_try(&why) => {
                tracing::warn!("purlis: spool keys still not recorded ({why}); not tried again");
                unrecorded.clear();
            }
            Err(_) => {}
        }
    }

    /// Chat `chat`'s key is not to be recorded any more: its token is gone.
    fn unrecorded_forgotten(&self, chat: u32) {
        #[cfg(unix)]
        self.waiting().remove(&chat);
        #[cfg(not(unix))]
        let _ = chat;
    }

    #[cfg(unix)]
    fn waiting(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, ChatToken>> {
        self.unrecorded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Records `program` as chat `chat`'s program: its lines are read from it and from what runs
    /// inside it, and from nothing else.
    pub fn bind(&self, chat: u32, program: Program) {
        self.rooted().insert(chat, program);
    }

    /// The program chat `chat`'s lines are read from now, where one is recorded: what a caller
    /// that waited compares with what it read before, to know the chat it serves is still the
    /// one that asked (qw381).
    pub fn program_of(&self, chat: u32) -> Option<Program> {
        self.rooted().get(&chat).cloned()
    }

    /// Chat `chat`'s program `pid` has ended: its token and its program are forgotten, so a
    /// later process given the same number speaks for nothing. A program that is not the one
    /// recorded (an end heard after the chat started again) changes nothing, and answers false.
    pub fn program_ended(&self, chat: u32, pid: u32) -> bool {
        let mut roots = self.rooted();
        let another = roots.get(&chat).is_some_and(|program| program.pid != pid);
        if roots.get(&chat).is_some_and(|program| program.pid == pid) {
            roots.remove(&chat);
            drop(roots);
            self.unrecorded_forgotten(chat);
            self.held().remove(&chat);
            self.disarm(chat);
        }
        !another
    }

    /// [`Self::issue`], with this process recorded as the chat's program: for a test that
    /// stands in for a chat's hooks itself, or starts them as its children.
    #[cfg(any(test, feature = "test-support"))]
    pub fn issue_to_this_process(&self, chat: u32) -> std::io::Result<ChatToken> {
        let token = self.issue(chat)?;
        let me = std::process::id();
        let program = Program::of(me).unwrap_or(Program {
            pid: me,
            started: String::new(),
        });
        self.bind(chat, program);
        Ok(token)
    }

    /// Forgets `chat`'s token: a line for it is dropped from now on.
    /// **Whether chat `chat`'s token is held**: it has started and not been forgotten (#2050).
    pub fn holds(&self, chat: u32) -> bool {
        self.held().contains_key(&chat)
    }

    pub fn forget(&self, chat: u32) {
        self.unrecorded_forgotten(chat);
        self.held().remove(&chat);
        self.rooted().remove(&chat);
        self.disarm(chat);
    }

    fn rooted(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, Program>> {
        self.roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Whether a line for `chat` carrying `token` from process `peer` is read
    /// ([`Self::admission`] is [`Admission::Admitted`]).
    pub fn admits(&self, chat: u32, token: Option<&str>, peer: Option<u32>) -> bool {
        self.admission(chat, token, peer) == Admission::Admitted
    }

    /// What a line for `chat` carrying `token` from process `peer` is.
    ///
    /// The token is compared in constant time, so how long a wrong token takes to refuse says
    /// nothing about how much of it was right. Then `peer` must be the chat's recorded program,
    /// still the process that started then, or run inside it ([`descends`]). No peer, and a
    /// chat with no program recorded, are [`Admission::Outside`].
    pub fn admission(&self, chat: u32, token: Option<&str>, peer: Option<u32>) -> Admission {
        self.admission_by(chat, token, peer, &|peer, program| descends(*peer, program))
    }

    /// [`Self::admission`], asking `inside` whether `peer` runs inside a chat's program.
    pub(crate) fn admission_by<P>(
        &self,
        chat: u32,
        token: Option<&str>,
        peer: Option<P>,
        inside: &dyn Fn(&P, &Program) -> bool,
    ) -> Admission {
        use subtle::ConstantTimeEq;

        let Some(token) = token else {
            return Admission::NoToken;
        };
        let carried = self
            .held()
            .get(&chat)
            .is_some_and(|held| bool::from(held.0.as_bytes().ct_eq(token.as_bytes())));
        if !carried {
            return Admission::NoToken;
        }
        let Some(program) = self.rooted().get(&chat).cloned() else {
            return Admission::Unbound;
        };
        if peer.is_some_and(|peer| inside(&peer, &program)) {
            Admission::Admitted
        } else {
            Admission::Outside
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, ChatToken>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Whether a spool key refused `why` may be recorded later: not where the spool is one nothing
/// is spooled in, nor where `keys.json` is a newer build's (#1426).
#[cfg(unix)]
fn worth_another_try(why: &std::io::Error) -> bool {
    !matches!(
        why.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidData
    )
}

/// A process as the channel sees it (D-1407-10): itself, the leader of its session and its
/// ancestors, each with when it started, read from the kernel in this process. A line is
/// judged by this once its token has checked, so a connection with no token, or one that never
/// writes, costs no reading at all. Every purlis sender stays until the app has let its
/// connection go, so it is still running when it is read; a process given a sender's number
/// since is told apart by when it started, since it is below no chat's program.
///
/// **A sender the channel cannot name is refused**, failing closed: on macOS that includes one
/// that closed its connection before the app could name it (`LOCAL_PEERPID` answers only while
/// the connection is open), even if it is still running (operator ruling 2026-10-17, #2465).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub pid: u32,
    started: String,
    /// Its session's leader, then its parent and up: each with when it started, where known.
    around: Vec<(u32, Option<String>)>,
}

impl Seen {
    /// Process `pid` now, or `None` where it cannot be read (it has gone, or the kernel would
    /// not say), which no chat admits.
    pub fn of(pid: u32) -> Option<Self> {
        Self::all(&[pid]).and_then(|mut seen| seen.pop())
    }

    /// Each of `pids` now, or `None` where any cannot be read.
    ///
    /// Read in this process, a pid at a time: `/proc` on Linux, and `proc_pidinfo` on macOS
    /// (D-1407-8, `purlis_same_user::Parents`), where it was one `/bin/ps` run under the fork
    /// lock before. No program is started, so a burst of lines no longer queues on that lock.
    pub fn all(pids: &[u32]) -> Option<Vec<Self>> {
        #[cfg(unix)]
        {
            use purlis_same_user::Parents;
            pids.iter()
                .map(|&pid| {
                    let started = Parents::started(pid)?;
                    let mut around = Vec::new();
                    if let Ok(session) = purlis_same_user::session_of(pid) {
                        around.push((session, Parents::started(session)));
                    }
                    for up in Parents::chain(pid) {
                        around.push((up, Parents::started(up)));
                    }
                    Some(Self {
                        pid,
                        started,
                        around,
                    })
                })
                .collect()
        }
        #[cfg(not(unix))]
        {
            let _ = pids;
            None
        }
    }

    /// Whether it runs inside `program`: it is that process, or its session's leader or one of
    /// its ancestors is, each by pid AND by when it started.
    pub fn inside(&self, program: &Program) -> bool {
        std::iter::once((self.pid, Some(&self.started)))
            .chain(
                self.around
                    .iter()
                    .map(|(pid, started)| (*pid, started.as_ref())),
            )
            .any(|(pid, started)| {
                pid == program.pid && started.is_some_and(|started| *started == program.started)
            })
    }
}

/// Whether process `peer` is `program`, or runs inside it, now ([`Seen::of`] and
/// [`Seen::inside`]): every doubt is no.
pub fn descends(peer: u32, program: &Program) -> bool {
    Seen::of(peer).is_some_and(|seen| seen.inside(program))
}

/// The line `what` is on the wire: its JSON with `token` beside it, and a newline.
pub(crate) fn line_with(
    token: Option<&ChatToken>,
    what: &impl serde::Serialize,
) -> std::io::Result<Vec<u8>> {
    line_armed(token, None, what)
}

/// [`line_with`], with the chat's armed key beside the token where the hook holds one (#3822).
pub(crate) fn line_armed(
    token: Option<&ChatToken>,
    armed: Option<&str>,
    what: &impl serde::Serialize,
) -> std::io::Result<Vec<u8>> {
    let mut value = serde_json::to_value(what).map_err(std::io::Error::other)?;
    if let (Some(token), Some(fields)) = (token, value.as_object_mut()) {
        fields.insert(TOKEN.to_owned(), serde_json::Value::String(token.0.clone()));
    }
    if let (Some(armed), Some(fields)) = (armed, value.as_object_mut()) {
        fields.insert(
            ARMED.to_owned(),
            serde_json::Value::String(armed.to_owned()),
        );
    }
    let mut line = serde_json::to_vec(&value).map_err(std::io::Error::other)?;
    line.push(b'\n');
    Ok(line)
}

/// The field a line's token travels in.
const TOKEN: &str = "token";

/// The field a report's armed key travels in (#3822), on the socket alone: never spooled.
const ARMED: &str = "armed";

/// [`read_delivered`] for a test that looks at the line and its token alone.
#[cfg(test)]
fn read_line(line: &str) -> Option<(Line, Option<String>)> {
    read_delivered(line).map(|read| (read.line, read.token))
}

/// One line off the socket as [`read_delivered`] reads it: the line, and what it carried
/// beside it, each taken off it.
struct ReadLine {
    line: Line,
    /// The chat's token.
    token: Option<String>,
    /// Its [`Delivery`].
    delivery: Option<Delivery>,
    /// The chat's armed key, which only a report carries ([`armed`]).
    armed: Option<String>,
}

/// A line off the socket, read, and the token it carried taken off it — so nothing past the
/// listener ever holds one — with the [`Delivery`] it carried taken off it too.
///
/// **Only a line a hook delivers carries one** ([`deliver`]: a report, a tool call or a refused
/// commit), and only in its own shape. Any other kind that carries the field, or one of those
/// three whose id is not one a hook makes, is not read: nothing past the listener holds a
/// field that kind never has. The same of an armed key (#3822), which a report alone carries,
/// as a string.
fn read_delivered(line: &str) -> Option<ReadLine> {
    let mut value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let fields = value.as_object_mut()?;
    let token = match fields.remove(TOKEN) {
        Some(serde_json::Value::String(token)) => Some(token),
        _ => None,
    };
    let delivery = fields.remove(DELIVERY);
    let armed = fields.remove(ARMED);
    let line: Line = serde_json::from_value(value).ok()?;
    let armed = match armed {
        None => None,
        Some(serde_json::Value::String(armed)) if matches!(line, Line::Report(_)) => Some(armed),
        Some(_) => return None,
    };
    let delivery = match delivery {
        None => None,
        Some(said) => {
            if !matches!(line, Line::Report(_) | Line::Tool(_) | Line::Refused(_)) {
                return None;
            }
            Some(said.as_str().and_then(Delivery::parse)?)
        }
    };
    Some(ReadLine {
        line,
        token,
        delivery,
        armed,
    })
}

/// The field a delivered line's [`Delivery`] travels in, on the socket and in the spool.
const DELIVERY: &str = "delivery";

/// **One delivery of a hook's line** (#952): an id the hook makes for the line as it delivers
/// it ([`deliver`]), sent with it to the host and kept with it in the chat's spool when the host
/// does not take it in time. A host that took the line late, after the hook had stopped
/// waiting and spooled it, has recorded it once already; the id is how the drain knows the
/// spooled copy is that line, and not another that reads the same (two `Stop` reports of one
/// conversation are byte for byte alike). 16 random bytes, in hex.
///
/// **It proves nothing about who sent the line**: the token does that. A chat's hook chooses
/// it, so it is never trusted alone: a copy is taken for a line recorded already only where it
/// is the same chat's, under the same id, and says the same thing ([`Deliveries`]). A line that
/// carries another line's id and says something else is recorded as its own, so no line can
/// keep another from being recorded.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) struct Delivery(String);

/// How many bytes a [`Delivery`] is made of.
const A_DELIVERY_IS: usize = 16;

impl Delivery {
    /// A new id, or none where the system has no randomness to give: the line is then delivered
    /// without one, as a hook before #952 delivered it, and may be recorded twice.
    #[cfg(unix)]
    pub(crate) fn new() -> Option<Self> {
        let mut bytes = [0u8; A_DELIVERY_IS];
        getrandom::fill(&mut bytes).ok()?;
        Some(Self(
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        ))
    }

    /// The id `said` is, where it is one a hook makes: lowercase hex of [`A_DELIVERY_IS`] bytes.
    fn parse(said: &str) -> Option<Self> {
        (said.len() == A_DELIVERY_IS * 2
            && said
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(said.to_owned()))
    }
}

/// `line` as JSON with `delivery` beside its own fields: what a hook sends, and what its spool
/// keeps, for a line it delivers ([`deliver`]).
#[cfg(unix)]
pub(crate) fn with_delivery(
    line: &impl serde::Serialize,
    delivery: Option<&Delivery>,
) -> io::Result<serde_json::Value> {
    let mut value = serde_json::to_value(line).map_err(io::Error::other)?;
    if let (Some(delivery), Some(fields)) = (delivery, value.as_object_mut()) {
        fields.insert(
            DELIVERY.to_owned(),
            serde_json::Value::String(delivery.0.clone()),
        );
    }
    Ok(value)
}

/// The most deliveries [`Deliveries`] remembers, of every chat together. One is a hook line the
/// host heard live, and a copy of one that is spooled is drained within seconds while the host
/// runs, so this is minutes of the busiest chats; past it the oldest go, and a spooled copy of
/// one of those is recorded a second time, which is the side the spool errs on.
#[cfg(unix)]
const A_HOST_REMEMBERS_AT_MOST: usize = 4096;

/// How long a drain of one chat waits, in all, and a line heard live waits, for a copy of the
/// same line the other is recording ([`Deliveries::begin`]): a line is recorded in the time an
/// event takes to be made durable. Past it the line is recorded again.
#[cfg(unix)]
pub(crate) const A_LIVE_LINE_IS_WAITED_FOR_AT_MOST: std::time::Duration =
    std::time::Duration::from_secs(1);

/// **The lines this process recorded, by chat and [`Delivery`]** (#952), heard live or drained
/// from the spool, whichever came first: each path asks before it records a line
/// ([`Deliveries::begin`]) and says after ([`Deliveries::finish`]), under one lock, so a line
/// the host took after its hook had stopped waiting, and the hook spooled as well, is recorded
/// once in either order.
///
/// **A copy is the line only where it says the same**: what is kept beside each id is the
/// line's own content ([`content_of`]), and a line under a known id that says something else is
/// recorded as its own and leaves what is kept alone. The id is the chat's hook's to choose, so
/// it may name another line's; it never keeps that line from being recorded.
///
/// One for the process ([`deliveries`]): an id is random, so the hosts of several projects, and
/// a project closed and opened again, share it without one's lines reading as another's. A
/// host that starts again remembers nothing: a line it recorded before it stopped, and whose
/// copy its next start drains, is recorded twice, as before #952.
#[cfg(unix)]
pub(crate) struct Deliveries {
    remembered: std::sync::Mutex<Remembered>,
    settled: std::sync::Condvar,
}

#[cfg(unix)]
#[derive(Default)]
struct Remembered {
    lines: std::collections::HashMap<(u32, Delivery), (Recording, Vec<u8>)>,
    /// The keys of `lines`, oldest first: the order they are let go in.
    order: std::collections::VecDeque<(u32, Delivery)>,
}

/// Where a line is, as [`Deliveries`] remembers it.
#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Recording {
    /// One path is recording it.
    Now,
    /// It was recorded: another copy of it is not.
    Done,
    /// The path that took it could not record it: the next copy is the line.
    Failed,
}

/// The process's [`Deliveries`].
#[cfg(unix)]
pub(crate) fn deliveries() -> &'static Deliveries {
    static ONE: std::sync::OnceLock<Deliveries> = std::sync::OnceLock::new();
    ONE.get_or_init(|| Deliveries {
        remembered: std::sync::Mutex::default(),
        settled: std::sync::Condvar::new(),
    })
}

/// What [`Deliveries`] keeps of a line to tell a copy of it: the line itself, as JSON, without
/// its token or id. A live line and its spooled copy are the one value the hook sent both ways,
/// but for a tool line's argument digest, which the spooled copy leaves out, so a tool line is
/// told by [`ToolCall::at_rest`].
#[cfg(unix)]
pub(crate) fn content_of(line: &impl serde::Serialize) -> Vec<u8> {
    serde_json::to_vec(line).unwrap_or_default()
}

#[cfg(unix)]
impl Deliveries {
    fn held(&self) -> std::sync::MutexGuard<'_, Remembered> {
        self.remembered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// **Whether chat `chat`'s line `delivery`, saying `content`, is to be recorded now**:
    /// `false` where it was recorded already, live or from the spool. Where the other path is
    /// recording the same line, this waits for it, at most `within`, and records it too past
    /// that: the side it errs on is a line twice, never none. A `true` is followed by
    /// [`Self::finish`].
    pub(crate) fn begin(
        &self,
        chat: u32,
        delivery: &Delivery,
        content: &[u8],
        within: std::time::Duration,
    ) -> bool {
        let at = (chat, delivery.clone());
        let held = self.held();
        let (mut held, _) = self
            .settled
            .wait_timeout_while(held, within, |held| {
                held.lines
                    .get(&at)
                    .is_some_and(|(now, kept)| *now == Recording::Now && kept == content)
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match held.lines.get_mut(&at) {
            None => {
                held.lines
                    .insert(at.clone(), (Recording::Now, content.to_vec()));
                held.order.push_back(at);
                while held.order.len() > A_HOST_REMEMBERS_AT_MOST {
                    if let Some(oldest) = held.order.pop_front() {
                        held.lines.remove(&oldest);
                    }
                }
                true
            }
            // Another line under this id: its own, recorded as such.
            Some((_, kept)) if kept.as_slice() != content => true,
            Some((Recording::Done, _)) => false,
            Some((state, _)) => {
                *state = Recording::Now;
                true
            }
        }
    }

    /// Chat `chat`'s line `delivery`, saying `content`, which [`Self::begin`] let through, was
    /// recorded, or (`recorded` false) could not be: its next copy is the line then. A line
    /// recorded once stays so.
    pub(crate) fn finish(&self, chat: u32, delivery: &Delivery, content: &[u8], recorded: bool) {
        let mut held = self.held();
        if let Some((state, kept)) = held.lines.get_mut(&(chat, delivery.clone()))
            && kept.as_slice() == content
        {
            if recorded {
                *state = Recording::Done;
            } else if *state != Recording::Done {
                *state = Recording::Failed;
            }
        }
        drop(held);
        self.settled.notify_all();
    }
}

#[cfg(unix)]
std::thread_local! {
    /// Set while a line heard live is handed to its hearer after its spooled copy was recorded
    /// already ([`recorded_already`]).
    static RECORDED_ALREADY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **Whether the line this thread's hearer is given was recorded already** (#952), from its
/// spooled copy, which the drain took before the host heard the line live: the hearer moves
/// what the line moves and records nothing. Only while the listener hands that line on.
pub fn recorded_already() -> bool {
    #[cfg(unix)]
    {
        RECORDED_ALREADY.with(std::cell::Cell::get)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// [`recorded_already`] said for as long as this is held.
#[cfg(unix)]
struct RecordedAlready;

#[cfg(unix)]
impl RecordedAlready {
    fn held() -> Self {
        RECORDED_ALREADY.with(|it| it.set(true));
        Self
    }
}

#[cfg(unix)]
impl Drop for RecordedAlready {
    fn drop(&mut self) {
        RECORDED_ALREADY.with(|it| it.set(false));
    }
}

// ----------------------------------------------------------------------------------------
// the other direction: a chat ASKING the app to open a chat (charter-app#204)
// ----------------------------------------------------------------------------------------

/// What a chat asks the app for, when reporting is not what it wants.
///
/// **This is the one verb on this socket that makes something happen in the world**, and it is
/// why the two lines below are a handshake rather than a single message. A report moves a
/// chat that already exists; an open makes one, whose first message is a brief another chat
/// wrote and nobody approved (#1444): what may start is the app's decision, never a word of
/// the request. See [`OpenChat`] for what the ticket does and does not protect against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ask {
    /// Mint a ticket for the chat this asker is running inside. The app answers with
    /// [`Answer::Ticket`], or with [`Answer::No`] when it will not.
    Ticket { chat: u32 },
    /// Spend a ticket: open the chat this describes. Boxed because it carries the whole
    /// first message, which is the brief, and the other variant is two words — clippy's
    /// `large_enum_variant` is right about it.
    Open(Box<OpenChat>),
    /// Spend a ticket: hand a report back to the chat that opened this one (charter-app#259).
    /// Boxed for `Open`'s reason.
    Report(Box<ReportBack>),
    /// Write this chat's session record: a brokered write (ADR 0067 §2, #1332). The app
    /// answers [`Answer::Recorded`], or [`Answer::No`] with the refusal. Boxed: it carries the
    /// record's whole body.
    SessionRecord(Box<RecordAsk>),
    /// Write one of the project's own files for this chat: a brokered write (ADR 0067 §2,
    /// #1333), as `purlis persona remember`, `purlis workspace remember` and `purlis workspace
    /// todo` hand it over in a chat the app started. The app answers [`Answer::Written`], or
    /// [`Answer::No`] with the refusal. Boxed: it carries the memory's whole text.
    Write(Box<WriteAsk>),
    /// Clone a repo, or cut a worktree, for this chat: a brokered git action (ADR 0067 §2,
    /// #1335). The app answers [`Answer::Said`], or [`Answer::No`] with the refusal. Boxed for
    /// `Open`'s reason.
    Git(Box<GitAsk>),
    /// Commit in the branch folder this chat stands in: a brokered git action (ADR 0067 §2,
    /// #1055). The app answers [`Answer::Said`], or [`Answer::No`] with the refusal. Boxed: it
    /// carries the whole message.
    Commit(Box<CommitAsk>),
    /// List the project's vaults for this chat (#1430): `purlis vault list` from a sandboxed
    /// chat, whose sandbox denies it every provider's own files. The app answers
    /// [`Answer::Vaults`] from the registry alone: names, tags, and whether the persona it
    /// recorded for the chat may use each. **It names no persona**: that is the app's record of
    /// the chat whose token the line carries. No ticket, for a record's reason: it changes
    /// nothing.
    Vaults { chat: u32 },
    /// List the key names of one vault for this chat (qw385): `purlis secret list <vault>`
    /// from a sandboxed chat, whose sandbox denies it the vault's storage. The app authorises
    /// the vault as it does a brokered `secret exec` (the persona it recorded for the chat, and
    /// your grant), then answers [`Answer::Keys`] with the names alone, or [`Answer::No`] with
    /// the refusal. **Never a value.** No ticket, for [`Self::Vaults`]'s reason.
    Keys { chat: u32, vault: String },
    /// Where this chat is working: who asked for it, its sibling tasks and the other chats
    /// running as its persona (#1450). The app answers [`Answer::Working`] from its own record
    /// of its open chats, or [`Answer::No`] with the refusal.
    WhereWorking(WhereWorking),
    /// Spend a ticket: dispatch a task to a persona (#1436). The app answers
    /// [`Answer::Dispatched`], [`Answer::NeedsGrant`], or [`Answer::No`] with the refusal.
    /// Boxed for `Open`'s reason.
    Dispatch(Box<DispatchAsk>),
    /// Ask after a task this chat dispatched (#1441): wait for its report, list this chat's
    /// tasks, or cancel one. The app answers [`Answer::Task`], or [`Answer::No`] with the
    /// refusal. No ticket: see [`crate::dispatched::Asked`].
    Task(Box<crate::dispatched::Asked>),
    /// Ask the person, in the window, whether to remove a workspace (qw356): what `purlis
    /// workspace remove <name>` hands over from a chat whose sandbox holds the removal. **The
    /// app removes nothing for it.** It holds the question on the asking chat's tab, and
    /// answers [`Answer::No`] with the sentence that says the person has been asked, or why
    /// they were not. No ticket, for a record's reason: nothing is made or removed by the ask.
    Removal(RemovalAsk),
    /// Show the person a change to one of purlis's own configuration files, which only they
    /// apply, in the window (#1975, [`crate::proposed`]): what a Claude Code chat's edit of such
    /// a file and `purlis propose` hand over. **The app writes nothing for it**: it holds the
    /// proposal for the person and answers [`Answer::No`] with the sentence that says they have
    /// it, or why they do not. No ticket, for [`Self::Removal`]'s reason. Boxed: it carries the
    /// file's whole new content.
    Propose(Box<ProposeAsk>),
    /// **A proposed value for a pool's limit** (#4284, the person's ruling of 2026-11-01 on
    /// PC-1): what `purlis pool limit` hands over in a chat. The same ask as [`Self::Propose`],
    /// held for the person, who alone applies it. **It carries a value and no file**, so nothing
    /// else in the pool's file can change. No ticket, for [`Self::Removal`]'s reason.
    ProposeLimit(Box<ProposeLimitAsk>),
    /// Drive one of this chat's browsers (#2013): what a chat's browser tools hand over. The app
    /// decides whether the chat may have a browser at all (the one Ask that sets its posture),
    /// starts its browser in the session host with what it may reach, and hands the act on. It
    /// answers [`Answer::Browsed`], or [`Answer::No`] with why, in words that say what to do.
    /// Asked over the framed stream, so a screenshot comes back as an image. Boxed: it may carry
    /// the text the agent types.
    Browse(Box<BrowseAsk>),
    /// Draw the theme specimen for this chat (#2330, [`crate::themepreview`]): what the
    /// `theme_preview` tool hands over. The app draws a fixed page of sample content in a hidden
    /// window of its own, in each theme and appearance asked, and answers
    /// [`Answer::Previewed`], or [`Answer::No`] with why. Asked over the framed stream, so each
    /// picture comes back as an image. No ticket, for a record's reason: nothing is made or
    /// moved, and the picture is of the specimen's own content, never the window's.
    ThemePreview(ThemePreviewAsk),
    /// Start or save this chat's theme draft (#2332, [`crate::theme::drafts`]): what `purlis
    /// theme draft` hands over. **A brokered write**: no chat writes the drafts folder, and the app
    /// writes this one in the drafts folder of the chat whose token the line carries, by its own
    /// record of that chat, never a chat a line names. It answers [`Answer::Drafted`] with what
    /// the window draws of the draft, or [`Answer::No`] with the refusal. No ticket, for a record's
    /// reason. Boxed: it carries the draft's theme files.
    ThemeDraft(Box<ThemeDraftAsk>),
    /// **Which skills this prompt is pointed at** (SK-15c, ADR 0102 §3): what a chat's prompt
    /// hook hands over where its start armed it with [`crate::skills::PROMPT_ROUTER_ENV`],
    /// because the router its persona or workspace chose reads prompts. The app asks that router
    /// within its 100 ms, about the skills the chat's list leaves out by its own record of the
    /// chat whose token the line carries, writes the routing record, and answers
    /// [`Answer::Pointed`]. **The prompt goes no further than that router**: never written,
    /// never in a debug dump ([`PromptAsk`]). No ticket, for a record's reason.
    Prompt(Box<PromptAsk>),
}

/// **A chat's prompt, for its router** ([`Ask::Prompt`], SK-15c).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// The prompt the person sent, as its hook read it.
    pub prompt: String,
}

/// Its chat, and never its prompt: a debug dump of a line that holds one says nothing of it, as
/// a prompt's marks say nothing (SK-38).
impl std::fmt::Debug for PromptAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptAsk")
            .field("chat", &self.chat)
            .finish_non_exhaustive()
    }
}

/// **A chat's theme draft, started or saved** ([`Ask::ThemeDraft`], #2332).
///
/// **It names a theme folder and its files, and no chat**: whose draft it is, and where that
/// chat works, are the app's record of the chat whose token the line carries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThemeDraftAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// The theme folder's name.
    pub folder: String,
    /// The draft's theme files to save, by their path in the folder (`theme.json`, and icon
    /// sets under `icons/`); `None` starts the draft and saves nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<std::collections::BTreeMap<String, String>>,
}

/// **The theme specimens a chat asks for** ([`Ask::ThemePreview`], #2330): one, or two side by
/// side, each a theme by its id, an appearance (high contrast included, #2709) and option values
/// (#2536). Nothing else travels: no workspace, no path, nothing of the window's. Where the chat
/// works, and so what an id names, is the app's own record of the chat, and the app holds the
/// line to the tool's bounds again ([`crate::themepreview::checked`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThemePreviewAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// What to draw, in the order the pictures come back; at most
    /// [`crate::themepreview::AT_MOST`].
    pub specimens: Vec<crate::themepreview::Specimen>,
}

/// **An act a chat asks of one of its browsers** ([`Ask::Browse`], #2013).
///
/// **It names the chat's own browser and nothing else, and that is the guard**: a browser is
/// named as the app named it to this chat (`b1`), looked up among the browsers the app started
/// for the chat whose token the line carries, so no line reaches another chat's browser. What
/// the browser may reach is the app's and the person's, never the line's.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrowseAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// Which of the chat's browsers, by the name the app gave it; none for the one it used last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    /// The act's own id: a retry with the same id answers again and acts on nothing.
    pub call_id: String,
    pub asked: Browsing,
    /// **It waited for the window** (#2050): a runner's host kept it while its desktop was away
    /// and hands it on now. A login that waited is asked of the person afresh, never performed
    /// on an answer they gave before. Set by the host alone: a hook line's own is cleared.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub waited: bool,
}

/// What a chat asks of its browsers ([`BrowseAsk`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Browsing {
    /// One act of a browser's, carried to it as it is.
    Act(purlis_session_protocol::browsing::Act),
    /// A new browser for this chat, apart from its others (cookies and storage its own), which
    /// becomes the one it uses.
    New,
    /// End the browser named, or the one it used last.
    Close,
    /// **The browser named needs the person** (#2015): `ask_operator` pointed at it. The app
    /// raises an Ask under Needs you whose source is the browser and answers at once; the chat
    /// is told when the person hands the browser back, and never what they did in it.
    NeedsYou { question: String },
    /// **Log the browser named in with one of its persona's declared browser logins** (#2016),
    /// by the login's name: the one thing the chat chooses. The app checks the chat's start
    /// armed that login, reads its values from the vault itself and hands them to the session
    /// host, whose browser fills them only on the login's own origin. The chat is answered
    /// with the page the login left, never a value.
    LogIn { login: String },
}

/// **A proposed change** a chat hands the app ([`Ask::Propose`], #1975): the file, by its path
/// in the project, the base the change was worked out against, and the change. Nothing else travels: whose it is is the app's record of the chat whose token the
/// line carries, and what the person sees as the file now is the app's own read of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProposeAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// The file, relative to the project root, as [`crate::proposed::proposable`] took it.
    pub path: String,
    /// The SHA-256 of the file as the change was worked out against it, in hex; none where it
    /// was not there. Checked at Apply; never trusted to show what the file holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// What it changes: the tool's own edits, which the app makes to the file as it reads it,
    /// or the file's whole new content.
    pub change: crate::proposed::Change,
}

/// **A value a chat proposes for a pool's limit** ([`Ask::ProposeLimit`]). Judged by the app
/// against the list of limits, and again at Apply.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProposeLimitAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    pub pool: String,
    /// The limit's key ([`crate::poollimits::of_key`]).
    pub key: String,
    /// The value; `None` returns it to purlis's default.
    pub value: Option<u64>,
}

/// Something a chat asks the person to remove ([`crate::removalask`], qw356).
///
/// **It carries a name and nothing else, and that is the guard.** The name is looked up in
/// the project by the app; what the question says, what a removal deletes and whether anything
/// is removed are the app's and the person's. There is no field for a reason, for `--force`,
/// or for another project.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemovalAsk {
    /// The chat asking, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// What it asks the person to remove.
    pub removing: Removing,
}

/// What a chat may ask the person to remove: one arm for each removal a chat's sandbox holds
/// and the window can make. A workspace today; a persona's and a branch folder's removal are
/// refused the same way and would each be an arm here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "what")]
pub enum Removing {
    /// A workspace of this project, by the name the command was given.
    Workspace { name: String },
}

/// A chat asking where it is working ([`crate::awareness`], #1450).
///
/// **It names no chat but itself, and that is the guard**, as [`RecordAsk`]'s: the picture is
/// drawn from the app's record of the chat whose token the line carries, so a line can only
/// ever learn what that chat may be told. No ticket, for a record's reason: nothing is made
/// or moved by the answer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WhereWorking {
    /// The chat asking, from [`CHAT_ENV`].
    pub chat: u32,
    /// Why it asks: the command, the start briefing, or a turn beginning.
    #[serde(default)]
    pub tell: crate::awareness::Tell,
}

/// A task a chat asks the app to dispatch, as `purlis dispatch` and the `dispatch` chat tool
/// hand it over (#1434, #1436).
///
/// **It says nothing about the chat that asks but its number, and that is the guard.** The
/// asking chat's persona, profile, folder, name and lineage, the stamp the new chat opens with
/// and the grants in force are all the app's own record of the chat whose token the line
/// carries ([`crate::dispatchdecision`]). A request cannot say who asks, where from, or that a
/// grant exists: there is no field to say it in. What it does say is what an agent chooses:
/// which persona, what the task is called, which of the project's profiles to start it on,
/// whether it works in another workspace or a worktree of its own, and the brief. A profile is a name, only ever looked up among the profiles the project
/// offers on this machine and has approved ([`crate::personaprofile::for_dispatch`]).
///
/// The ticket is [`OpenChat`]'s, minted and spent the same way, so one run of the command
/// starts at most one chat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DispatchAsk {
    /// The chat that asks, from [`CHAT_ENV`]: the chat whose token the line must carry.
    pub chat: u32,
    /// The persona the new chat runs as. `None` is the asking chat's own, whatever the app's
    /// record says that is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// The task's name, which the persona chat is called and listed under. Held to
    /// `reopen::label` by the command and again by the app.
    pub name: String,
    /// The brief, verbatim. The app puts its own stamp in front of it.
    pub brief: String,
    /// The profile the asking chat asks for the new chat to start on (`--profile`, #1445).
    /// `None` is the persona's own, else the asking chat's. One the project does not offer
    /// here, or has not approved, is a refusal and never another profile in its place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Where the new chat is to work (`--in`, #1453): `worktree`, `worktree:<repo>` (#3903),
    /// or `workspace:<name>`.
    /// `None` is the persona's own default, else the asking chat's folder. **A word, never a
    /// place**: which repo a worktree is cut from, and what its folder and branch are called,
    /// are the app's ([`crate::dispatchplace`]), and a workspace is looked up among the
    /// project's own. There is no field for a folder or a branch.
    #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// **A long-running task** (`--long`, #3905): its notes are its heartbeat, and purlis never
    /// asks it for a report nor makes one from its last message. It ends when it reports
    /// `done` or `failed`, or is cancelled. Absent, and `false`, for a task that is one job
    /// with one answer.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub long: bool,
    /// See [`OpenChat`]. Minted by the app, spent once, never written down.
    pub ticket: String,
}

/// What a task's report says beside its text, as `purlis dispatch report` hands it over
/// (#1436). Where the persona chat's session record is, the report's fourth part, is not here:
/// the app says it, from its own record of what it wrote for that chat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskReport {
    pub outcome: crate::handback::Outcome,
    /// What changed, in the persona chat's words: files, commits, a branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed: Option<String>,
}

/// A write a chat asks the app to make for it ([`crate::brokered`]).
///
/// **It names no place, and that is the guard**, as [`RecordAsk`]'s: the workspace and the
/// persona it is written to are the app's record of the chat whose token the line carries.
/// No ticket, for a record's reason: what it writes is the chat's own.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WriteAsk {
    /// The chat asking, from [`CHAT_ENV`].
    pub chat: u32,
    /// What to write.
    #[serde(flatten)]
    pub write: crate::brokered::Write,
}

/// A git action a chat asks the app to take for it, as `purlis clone` and `purlis worktree add`
/// hand it over: **a brokered write** (ADR 0067 §2, #1335). A sandboxed chat may not write a
/// clone's `.git/config`, its hooks or the editor settings a checkout carries; the app writes
/// them, with the same core code the terminal's commands run ([`crate::gitbroker`]).
///
/// **What it names is checked, never trusted.** The workspace must be one the chat already
/// writes its ordinary files in, the clone's host one the chat's sandbox may reach, and who
/// the piece log credits is the app's record of the chat whose token the line carries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GitAsk {
    /// The chat that asks, from [`CHAT_ENV`].
    pub chat: u32,
    /// The workspace, as the command resolved it.
    pub workspace: String,
    pub work: GitWork,
    /// Whether the asker reads [`Answer::Alive`] lines while it waits (#4080), as a clone's
    /// does, whose wait follows the clone's liveness and has no total bound. Absent, as from an
    /// older command, reads as no, and nothing but the answer is sent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub alive: bool,
}

/// A commit a sandboxed chat asks the app to make in the branch folder it stands in, as
/// `purlis worktree commit` hands it over: **a brokered git action** (ADR 0067 §2, #1055). A
/// linked worktree keeps its index, its HEAD and its objects in its clone's `.git`, which the
/// chat may not write; the app stages and commits ([`crate::gitbroker::commit`]).
///
/// **It names the message and what to stage, and nothing else, and that is the guard**: no
/// repository, folder, branch, git directory, author, date or option. The folder is where the
/// app recorded the chat whose token the line carries as standing, or, for a chat recorded above
/// branch folders, the one branch folder below that which holds every path named (#1476); the
/// branch is the one that folder is on. A line that carries any other field is not read. No ticket, for a
/// record's reason: the commit is the chat's own work on its own branch.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitAsk {
    /// The chat that asks, from [`CHAT_ENV`].
    pub chat: u32,
    /// The commit message, as written.
    pub message: String,
    /// What to stage for it.
    pub stage: Stage,
    /// The branch folder the command was run for (#3927), when it ran inside one: its paths
    /// are then relative to that folder's top, as git takes them there, and not to where the
    /// chat stands. Held to the app's record all the same: the folder must be the one the chat
    /// stands in, or one below it, as a path naming it from above would have to be. Left out
    /// when the command ran in no branch folder, so an app older than it reads the line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<FolderName>,
}

/// A branch folder by its names (#3927): `workspaces/<workspace>/.worktrees/<repo>/<piece>`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderName {
    pub workspace: String,
    pub repo: String,
    pub piece: String,
}

/// What a [`CommitAsk`] stages before it commits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Every change to a file git already tracks in the folder (`git add -u`).
    Tracked,
    /// These paths, each relative to where the chat stands and inside its folder, or, for an
    /// ask that names its [`CommitAsk::folder`], to that folder's top (#3927).
    Paths(Vec<String>),
}

/// What a [`GitAsk`] asks for: one of the commands, with its arguments.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitWork {
    /// `purlis clone <repo>…`.
    Clone { repos: Vec<String> },
    /// `purlis workspace create <name> <repo>… [--vision]`: the workspace the ask names, which
    /// must not be there yet, made as a chat's plain create makes one, and the repos cloned
    /// into it (qw387). The one git ask that names a workspace the chat does not work in, and
    /// only one this ask makes.
    Create {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        vision: Option<String>,
        repos: Vec<String>,
    },
    /// `purlis worktree add <repo> <piece> [--branch]`.
    WorktreeAdd {
        repo: String,
        piece: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
    },
    /// `purlis worktree add <repo> <piece> [--base <branch>] [--branch]` (qw387): cut from
    /// `base`, a branch of the clone by name, or `HEAD` for the clone's HEAD; with no base, from
    /// the branch the remote names its default where the clone has one. A kind of its own, so
    /// an app that does not know it answers nothing rather than cutting from the clone's HEAD.
    WorktreeCut {
        repo: String,
        piece: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<String>,
    },
    /// `purlis worktree update [--base <branch>] [--continue | --abort]` (qw387): the branch
    /// folder `<repo>/<piece>` brought up to its base, which the chat stands in or above.
    WorktreeUpdate {
        repo: String,
        piece: String,
        step: UpdateStep,
    },
    /// `purlis worktree update [--continue | --abort]` standing in a workspace's clone
    /// (#3831): the clone `<repo>` itself fetched, and its branch fast-forwarded or rebased
    /// onto the remote branch it follows. A kind of its own, so an app that does not know it
    /// answers nothing rather than reading it as a branch folder's.
    CloneUpdate { repo: String, step: UpdateStep },
    /// `purlis worktree update` standing at the project root (#3831, ruled 2026-10-29): the
    /// window's own Update run for the chat, only on a clean tree, and refused where it would
    /// write a file the chat's sandbox denies. The ask's workspace is empty and unread.
    RootUpdate {},
    /// `purlis worktree push [--force-with-lease]` (qw387): the branch of folder
    /// `<repo>/<piece>` pushed to its remote, under its own name and nothing else.
    WorktreePush {
        repo: String,
        piece: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        lease: bool,
    },
    /// `purlis worktree remove <repo> <piece> [--delete-branch]` (#3879): the branch folder
    /// `<repo>/<piece>` of the ask's workspace removed, only where nothing would be lost: no
    /// uncommitted change and no commit no other ref reaches. **It has no word for `--force`**,
    /// which discards work and stays the person's, from the window. A kind of its own, so an
    /// app that does not know it answers nothing and removes nothing.
    WorktreeRemove {
        repo: String,
        piece: String,
        /// Also delete the folder's branch, as `git branch -d` does: only where it is merged.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        delete_branch: bool,
    },
    /// **The guard's question before plain git writes a branch folder** (#3931): may this chat
    /// `act` in the folder `<repo>/<piece>` of the ask's workspace? The app runs no git: it
    /// judges the folder's claim against the chats it has, and takes the folder for the chat
    /// where no live chat holds it, as a brokered write does ([`crate::piececlaim::take`]). It
    /// answers an empty `Said` with exit 0 where the chat may write, and a `Said` with exit
    /// [`CLAIM_HELD`] and the refusal as its one line where another live chat holds the folder:
    /// the one answer the asking guard refuses on. Any `No` (a chat the app does not have open,
    /// a name that cannot be a branch folder, a claim it could not judge) lets the git run, as
    /// no answer does. Asked with [`GitAsk::alive`] off: the guard's wait is bounded, and a
    /// liveness line would only spend it. A kind of its own, so an app that does not know it
    /// answers nothing.
    WorktreeClaim {
        repo: String,
        piece: String,
        act: ClaimAct,
    },
}

/// The exit status a [`GitWork::WorktreeClaim`] is answered with where another live chat holds
/// the folder: the one answer the guard on a chat's own git refuses on (#3931, the review of
/// #4129). Not 1, which any failure could say.
pub const CLAIM_HELD: u8 = 3;

/// What plain git would do in a branch folder, as a [`GitWork::WorktreeClaim`] names it.
///
/// **A wire enum of its own, not [`crate::piececlaim::Act`]**: that one also names a removal
/// and a dispatch's warning, which no claim ask may carry, and its words are prose for a
/// refusal. This holds the three a chat's git can ask about, in the stable snake-case spelling
/// the socket's lines keep across app and command-line versions; [`ClaimAct::act`] and
/// [`ClaimAct::of`] map between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimAct {
    /// A commit: `git commit`, `merge`, `cherry-pick`, `revert`, `am`.
    Commit,
    /// The branch rebased or pulled.
    Update,
    /// The branch pushed.
    Push,
}

impl ClaimAct {
    /// The act a branch folder's claim judges and its refusal names.
    pub fn act(self) -> crate::piececlaim::Act {
        match self {
            Self::Commit => crate::piececlaim::Act::Commit,
            Self::Update => crate::piececlaim::Act::Update,
            Self::Push => crate::piececlaim::Act::Push,
        }
    }

    /// The wire's word for `act`, or `None` for an act no plain git asks about.
    pub fn of(act: crate::piececlaim::Act) -> Option<Self> {
        match act {
            crate::piececlaim::Act::Commit => Some(Self::Commit),
            crate::piececlaim::Act::Update => Some(Self::Update),
            crate::piececlaim::Act::Push => Some(Self::Push),
            crate::piececlaim::Act::Remove | crate::piececlaim::Act::Write => None,
        }
    }
}

/// What a [`GitWork::WorktreeUpdate`] does.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStep {
    /// Fetch, bring the base up to its remote where it may be, and rebase onto it: the base
    /// named, or the one recorded when the piece was cut.
    Start {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<String>,
    },
    /// Stage the files a stopped rebase named, once resolved, and go on.
    Continue,
    /// Put the branch back where it was before the rebase began.
    Abort,
}

/// A session record a chat asks the app to write for it, as `purlis session record` and the
/// MCP server's `session_record` tool both hand it over: **a brokered write** (ADR 0067 §2,
/// #1332). The chat's sandbox may forbid the write; the app makes it, with the same core code
/// the terminal's command uses ([`crate::sessionrecord::record`]).
///
/// **It names no place, and that is the guard.** Where the record goes, the persona and every
/// other fact in its frontmatter are the app's record of the chat whose token the line carries,
/// never the request's. The request carries only what the model writes (the title and the
/// body) and the pieces it says it worked in, which git is asked about before any is recorded.
///
/// No ticket: a record is the chat's own to write, with or without a smart close, and a line
/// can only ever name the chat whose token it carries. What only a person grants, the close of
/// the tab, is the smart-close pass the app holds, never anything on this line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordAsk {
    /// The chat whose record it is, from [`CHAT_ENV`].
    pub chat: u32,
    pub title: String,
    pub body: String,
    /// The pieces named as `<repo>/<piece>`, as `--piece` takes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<String>,
    /// The directory the asker runs in, so the piece it stands in is recorded as the terminal's
    /// command records it. Only ever looked up among the pieces git reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<std::path::PathBuf>,
}

/// A report a dispatched chat sends back, as `purlis dispatch report` hands it over.
///
/// **It names no recipient, and that is the guard.** The chat it goes to is the one the app
/// recorded as this chat's asker when it started it (`reopen::HandedFrom`), so nothing a chat
/// can say sends a report anywhere else. The ticket
/// is [`OpenChat`]'s, spent the same way, so no single line on the socket reports anything and
/// no line that reported can report again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReportBack {
    /// The chat that is reporting — the app's own number for it, from [`CHAT_ENV`].
    pub chat: u32,
    /// The report, already through `handoff::report_summary`; the app asks again.
    pub summary: String,
    /// See [`OpenChat`]. Minted by the app, spent once, never written down.
    pub ticket: String,
    /// A task's outcome and what changed (#1436). **Every report this purlis sends carries
    /// it** (#1471): `purlis handoff report`, which sent a summary alone, is now a refusal that
    /// names `purlis dispatch report`. Absent only from an older line, which the app refuses
    /// with that command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskReport>,
}

/// A handoff the app is asked to open, as `charter handoff` hands it over.
///
/// Every field here has already been through the command's own refusals
/// ([`crate::handoff`]): the workspace is one this plane has (or one this call is creating),
/// the persona is one it defines, and `message` is the stamped first message a chat may be
/// started on. The app asks the questions only it can answer — is this a chat I started, is
/// its harness one that takes a first message, is the workspace's directory there — and
/// refuses rather than guessing. **And it decides the handoff, as it decides any dispatch**
/// (#1444): a handoff is a dispatch in handoff mode, so the app answers [`Answer::Opened`],
/// [`Answer::NeedsGrant`] where the person is being asked for the pair, or [`Answer::No`].
///
/// # The ticket, and exactly what it is worth
///
/// `ticket` is a value the app minted moments earlier, on this same connection, in answer to
/// [`Ask::Ticket`] for this same chat. It is spent here and can never be spent again.
///
/// **What that buys.** The socket carries no ambient "open a chat" verb: no single line on it
/// opens anything, and no line that opened something can open a second thing. The ticket
/// never touches a readable surface — not the environment, not argv (which is where the brief
/// itself travels, and which `charter handoff`'s credential refusal already calls readable by
/// any local process), not a file, not the transcript — so nothing that can read those can
/// produce one. And because a ticket belongs to the one connection it was minted on, and the app
/// forgets it the moment it is spent or the deadline passes, one `charter handoff`
/// opens at most one chat.
///
/// **What it does not buy, plainly.** It does not say which process in the chat asked. A
/// process already inside the chat's own process tree has the socket path and the chat number
/// in its environment, so it can run this same two-line exchange — or simply exec
/// `charter handoff` itself. charter cannot tell that process from the chat's own turn: a helper
/// sub-agent is refused by the tool hook where the hook can tell one made the call, and that is
/// advice it meets, not a boundary. So **what consents to a handoff is never the request**: it
/// is the app's own decision, from its own record of the asking chat and the dispatch grants
/// the person made (`dispatchdecision`), and a grant for a pair covers whatever in that chat's
/// tree asks in its name. ADR 0024 concedes exactly this for *moving* a chat, and this is the
/// same concession for opening one.
///
/// **Why that is acceptable, and it is the whole of the justification.** Opening a chat with
/// an arbitrary first message and the operator's authority is *already* within reach of any
/// process in the tree, with no app involved: `claude -p "…"`, or
/// `charter claude --workspace x "<msg>"`, starts a harness headless, in the background, where
/// nobody sees it. What the app opens instead is a tab in the target workspace's strip, in the
/// window the operator is looking at, whose first message is stamped with the chat it came from
/// (`⟨handoff from chat N · …⟩`). That is strictly more visible than what the tree could
/// already do. **Visibility is the control here, not the ticket**, which is why the app never
/// opens a handed-off chat anywhere but on a strip, and why the stamp is not optional.
/// (charter-app#204, where the operator ruled on this with the premise corrected: an earlier
/// framing called opening a chat "a larger primitive than moving a tab", and that does not
/// survive counting the harness itself.)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OpenChat {
    /// The chat that is handing off — the app's own number for it, from [`CHAT_ENV`].
    pub chat: u32,
    /// The workspace the new chat belongs to for life.
    pub workspace: String,
    /// `Some(vision)` when this handoff is creating that workspace, which is the only shape
    /// `--create` can reach this in: `--create` without `--vision` is refused long before.
    pub create_vision: Option<String>,
    /// The persona the operator named, or none for the plane's own answer.
    pub persona: Option<String>,
    /// The whole of what the new chat is sent: the stamp line, a blank line, the brief.
    pub message: String,
    /// See the note above. Minted by the app, spent once, never written down.
    pub ticket: String,
    /// The short task name `--name` gave, which the new chat is called instead of its default
    /// (charter-app#258). Held to `reopen::label` by the command and again by the app.
    /// Absent from a `charter` older than it, which reads as no name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// **Retired** (#1471): the `report` an older command line set when `--report` asked for an
    /// answer (charter-app#259). Work that needs an answer is a task, and `purlis handoff
    /// --report` has sent a dispatch since #1515, so this purlis never writes it. It is still
    /// read, so that an open that sets it is refused, naming `purlis dispatch`, and is never
    /// opened as a handoff nobody waits on. Absent reads as no.
    #[serde(default, rename = "report", skip_serializing)]
    pub older_report: bool,
}

/// What the app answers an [`Ask`] with. One line, on the same connection.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// A ticket to spend on the next line, and on no other connection.
    Ticket { ticket: String },
    /// The chat is open, under this number on the app's board. `note` is one sentence the app
    /// has to say about how it opened it: that the chat runs on the asking chat's profile
    /// because its persona's own is not offered on this machine (#1445, D-1445-8). None when
    /// there is nothing to say, and from an app that never says it.
    ///
    /// **No `row`** (#1457): an older app wrote the handoff's row in the project's dispatch log
    /// and said here what became of it (#1421). A handoff writes no row now; the key an older
    /// app still sends is read past, and an older command that hears none writes the row it
    /// always wrote where it may, which a log already holds and nothing counts.
    Opened {
        chat: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// The report was handed back: to the chat named `to`, or — where that chat is gone —
    /// kept for its workspace, `kept_for`, for the next chat that starts there.
    Reported {
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kept_for: Option<String>,
    },
    /// A task's report was handed to the chat named `to`, **and the task is finished**
    /// (#1485): purlis ends the reporting chat's program once this turn is over. Said only
    /// where the app will: never for a handoff's chat, a task that came out blocked, or one
    /// the person started from a tab, which answer [`Self::Reported`]. Where that chat has
    /// gone, the report is kept for its workspace, `kept_for`, and the task still ends
    /// (#1510).
    Finished {
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kept_for: Option<String>,
    },
    /// The app will not, and this is the sentence saying why. The asker prints the command
    /// to run in a terminal and says this underneath it: a refusal the operator cannot see
    /// is a handoff that vanished.
    No { why: String },
    /// What one of the chat's browsers answered ([`Ask::Browse`], #2013): the browser, by the
    /// name the app gave it, and what it said. On the framed stream its screenshot travels as an
    /// image after the answer ([`framed::Reply`]), never inside it. Boxed: it carries a page's
    /// lines.
    Browsed {
        browser: String,
        browsed: Box<purlis_session_protocol::browsing::Browsed>,
    },
    /// The theme specimens drawn ([`Ask::ThemePreview`], #2330), in the order asked. On the
    /// framed stream each picture travels as an image after the answer ([`framed::Reply`]),
    /// never inside it.
    Previewed {
        painted: Vec<crate::themepreview::Painted>,
    },
    /// The session record is written (#1332): where, plane-relative, and whether the chat's
    /// tab now closes, which it does only under a smart-close pass. `warnings` is what did not
    /// follow it ([`crate::sessionrecord::Recorded::warnings`]).
    Recorded {
        record: String,
        closes: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        warnings: Vec<String>,
    },
    /// A brokered write is made (#1333): the workspace or persona it went to, and the file,
    /// project-relative.
    Written { to: String, path: String },
    /// A brokered git action ran (#1335): every line the command said, in order, and its exit
    /// status. The asker prints them as its own run would have.
    Said {
        lines: Vec<crate::repocmd::Say>,
        code: u8,
    },
    /// Not an answer: the ask is still being worked on, and shows life (#4080). Sent only to an
    /// asker that said it reads it ([`GitAsk::alive`]), before its one answer, and read past.
    Alive {},
    /// A chat's theme draft is started or saved ([`Ask::ThemeDraft`], #2332): every line the
    /// app said, its exit status, and whether the draft was **made** by this ask, a copy of its
    /// kept folder or a new theme, rather than there already. The command makes its working copy
    /// again from a draft made now, so the edits of one kept or reverted never come back.
    Drafted {
        lines: Vec<crate::repocmd::Say>,
        code: u8,
        made: bool,
    },
    /// The project's vaults, for the chat that asked (#1430): the persona the app started it
    /// as, and each vault with whether that persona may use it. No value and no key name.
    Vaults {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        persona: Option<String>,
        vaults: Vec<crate::secrets::brokered::Listed>,
        /// Whether a policy forbids the person's Allow for a vault here, so the listing does
        /// not name it as a way forward.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        allow_locked: bool,
    },
    /// The key names of the vault a chat asked to list (qw385), sorted. No value.
    Keys { keys: Vec<String> },
    /// Where the chat is working (#1450): names, tasks and states from the app's own record,
    /// and what changed since the chat was last told. Boxed: it is the largest answer.
    Working(Box<crate::awareness::Working>),
    /// What a chat's router points its prompt at (SK-15c): the text its hook adds to the turn's
    /// context, in purlis's own words, or none.
    Pointed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pointer: Option<String>,
    },
    /// A task was dispatched (#1436): the persona chat is running, under this number on the
    /// app's board and this name, as `persona` (none for a chat on no persona). `note` is
    /// [`Self::Opened`]'s: something the asking chat should be told about how it was started,
    /// today that it runs on the asking chat's profile because its persona's own is not
    /// offered on this machine (#1445, D-1445-8).
    ///
    /// `works` says where the new chat works, where that is not the asking chat's folder
    /// (#1453): another workspace, or a worktree of its own with the branch purlis cut for
    /// it. The app's words, said after "It works".
    Dispatched {
        chat: u32,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        persona: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        works: Option<String>,
    },
    /// Nothing has started **yet**: the asking chat's persona has no dispatch grant for `to`,
    /// and the person is being asked for one on the asking chat's tab (#1434, #1437). `from`
    /// is the asking chat's persona, by the app's record. The dispatch is held by the app:
    /// where the person allows it, it starts then, and where they keep it blocked, the asking
    /// chat is told on its next turn.
    ///
    /// `waiting` is set where the person is already being asked about this pair for an
    /// earlier task of this chat, by that task's name: **this** ask was not held, because the
    /// person saw one brief and only that one starts.
    ///
    /// `held_by` is set where no grant is the question: the asking chat runs on that pool, so
    /// the person approves this ask in the window, whatever the persona (#4148). An older
    /// purlis on either end ignores or lacks it, and says the grant's words.
    NeedsGrant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        waiting: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        held_by: Option<String>,
    },
    /// Nothing has started **yet**: every check let the dispatch through and its grant stands,
    /// and this machine is short on memory (#1467). The app holds it and decides it again once
    /// memory frees, against the limits as they are then; where it is still short after
    /// [`crate::dispatchdecision::MEMORY_WAIT`], nothing starts and the asking chat is told on
    /// its next turn. `to` is the persona it is for, none for the asking chat's own.
    WaitingOnMemory {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
    /// What became of an ask after a dispatched task (#1441).
    Task(Box<crate::dispatched::Answered>),
}

/// How long a ticket lives unspent.
///
/// `charter handoff` spends its ticket on the very next line, milliseconds after the mint, so
/// this bounds only a ticket that was minted and abandoned: a `charter` killed between the two
/// lines, or something that minted with no intention of spending. An abandoned ticket holds up
/// nothing but its own connection; it counts towards [`MOST_LIVE_TICKETS_A_CHAT`] until it
/// expires. Seconds, not minutes, for that reason.
pub const A_TICKET_LIVES: std::time::Duration = std::time::Duration::from_secs(10);

/// The most tickets one chat may have live at once.
///
/// A chat that fans out runs several commands in one step, each with a connection and a ticket
/// of its own, minted before any is spent. This is room for the most tasks one chat may have
/// running at the highest setting anyone has asked for, and still a bound: mints that are
/// never spent cannot grow the map for longer than [`A_TICKET_LIVES`].
pub const MOST_LIVE_TICKETS_A_CHAT: usize = 16;

/// The tickets an app has minted and not yet seen spent: one per connection, and at most
/// [`MOST_LIVE_TICKETS_A_CHAT`] per chat.
///
/// This is the whole of the ticket's mechanism, kept in the core so that it is plain Rust
/// with tests of its own, and so the app holds a value rather than a policy. The argument for
/// what it is worth is on [`OpenChat`].
#[derive(Debug, Default)]
pub struct Tickets {
    /// By the chat a ticket was minted for and the connection it was minted on.
    live: std::sync::Mutex<std::collections::HashMap<(u32, u64), Live>>,
}

#[derive(Debug)]
struct Live {
    ticket: String,
    until: std::time::Instant,
}

impl Tickets {
    /// A ticket for `chat`, bound to `connection`, or why not.
    ///
    /// **One live ticket per connection, and each run of a command is a connection.** A chat
    /// that dispatches six tasks in one step runs six commands, and each mints and spends its
    /// own (#1441): a ticket is the property of the run that asked for it, so no run can spend,
    /// replace or cancel another's. A second mint on a connection that already holds a live
    /// ticket is refused rather than replacing it, which is what keeps one run of a command to
    /// one thing started.
    ///
    /// The value is two v4 UUIDs, 244 random bits from the operating system's generator: the
    /// same source charter already trusts for the session ids it mints (`harness::SessionId`).
    pub fn mint(
        &self,
        chat: u32,
        connection: u64,
        now: std::time::Instant,
    ) -> Result<String, String> {
        let mut live = self.held();
        live.retain(|_, one| one.until > now);
        if live.contains_key(&(chat, connection)) {
            return Err(format!(
                "this connection already holds a ticket for chat {chat}; spend it first"
            ));
        }
        if live.keys().filter(|(of, _)| *of == chat).count() >= MOST_LIVE_TICKETS_A_CHAT {
            return Err(format!(
                "chat {chat} has {MOST_LIVE_TICKETS_A_CHAT} requests to the app under way at \
                 once; try again in a few seconds"
            ));
        }
        let ticket = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        live.insert(
            (chat, connection),
            Live {
                ticket: ticket.clone(),
                until: now + A_TICKET_LIVES,
            },
        );
        Ok(ticket)
    }

    /// Spends `ticket` for `chat` on `connection`, or says why it will not open anything.
    ///
    /// **The ticket this connection holds for the chat is gone after this whatever the
    /// answer.** A wrong guess spends the real ticket too, so a guesser gets one try per mint.
    /// A try on any other connection finds nothing to spend and costs nobody anything: a ticket
    /// never spends off the connection it was minted on. The comparison runs over every byte
    /// whatever it finds, so how long a wrong ticket takes to refuse says nothing about how
    /// much of it was right.
    pub fn spend(
        &self,
        chat: u32,
        connection: u64,
        ticket: &str,
        now: std::time::Instant,
    ) -> Result<(), String> {
        let Some(live) = self.held().remove(&(chat, connection)) else {
            return Err(NO_TICKET.to_owned());
        };
        let same = live.ticket.len() == ticket.len()
            && live
                .ticket
                .bytes()
                .zip(ticket.bytes())
                .fold(0u8, |differs, (a, b)| differs | (a ^ b))
                == 0;
        if !same || live.until <= now {
            return Err(NO_TICKET.to_owned());
        }
        Ok(())
    }

    fn held(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<(u32, u64), Live>> {
        self.live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What every spend that opens nothing says. One sentence for every reason, so the refusal
/// does not tell a guesser which part it got wrong.
pub const NO_TICKET: &str =
    "that open was not preceded by a ticket this app minted for this chat on this connection";

/// One line read off the socket, whichever kind it is.
///
/// **Untagged, and the report comes first.** A [`Report`] requires both `chat` and `event`,
/// and no [`Ask`] carries an `event`, so the two can never be mistaken for each other.
///
/// Every kind is read with the chat's token taken off it first ([`read_delivered`]), so none of
/// them carries one past the listener.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum Line {
    Report(Report),
    Ask(Ask),
    /// Last, so it can never shadow the two above: it requires `started_by_hand`, which
    /// neither carries, and a report or an ask never reads as one.
    ByHand(StartedByHand),
    /// After every other kind, for the same reason: it requires `session_saved`, which none of
    /// them carries, so no line an older `charter` writes ever reads as one.
    Saved(SessionSaved),
    /// After every other kind: it requires `commit_refused`, which none of them carries.
    Refused(CommitRefused),
    /// After every other kind: it requires `tool_hook`, which none of them carries.
    Tool(ToolCall),
    /// After every other kind: it requires `permission_request`, which none of them carries.
    Permission(permission::PermissionAsked),
    /// After every other kind: it requires `touching`, which none of them carries.
    Touching(Touching),
    /// After every other kind: it requires `sandbox_blocked`, which none of them carries.
    Blocked(SandboxBlocked),
    /// After every other kind: it requires `doing`, which none of them carries.
    Doing(Doing),
    /// After every other kind: it requires `turn_output`, which none of them carries.
    Output(TurnOutput),
    /// After every other kind: it requires `secret_exec`, which none of them carries. Held on
    /// its connection for as long as the command runs (#1407).
    SecretExec(crate::secrets::brokered::Ask),
    /// After every other kind: it requires `frames`, which none of them carries. The
    /// connection carries frames from the app's acceptance on ([`framed`], #1996).
    Frames(framed::Hello),
    /// After every other kind: it requires `serve`, which none of them carries. The connection
    /// is parked as a slot of the chat's door from the app's acceptance on ([`serve`], #3316).
    Serve(serve::ServeAsk),
}

impl Ask {
    /// The chat this ask says it comes from, which its token has to be the token of.
    pub fn chat(&self) -> u32 {
        match self {
            Self::Ticket { chat } => *chat,
            Self::Open(open) => open.chat,
            Self::Report(back) => back.chat,
            Self::SessionRecord(record) => record.chat,
            Self::Write(write) => write.chat,
            Self::Git(git) => git.chat,
            Self::Commit(commit) => commit.chat,
            Self::Vaults { chat } | Self::Keys { chat, .. } => *chat,
            Self::WhereWorking(asks) => asks.chat,
            Self::Dispatch(dispatch) => dispatch.chat,
            Self::Task(asked) => asked.chat,
            Self::Removal(removal) => removal.chat,
            Self::Propose(proposed) => proposed.chat,
            Self::ProposeLimit(proposed) => proposed.chat,
            Self::Browse(browse) => browse.chat,
            Self::ThemePreview(asks) => asks.chat,
            Self::ThemeDraft(draft) => draft.chat,
            Self::Prompt(asked) => asked.chat,
        }
    }
}

impl Line {
    /// The chat this line says it comes from, which its token has to be the token of.
    fn chat(&self) -> u32 {
        match self {
            Self::Report(report) => report.chat,
            Self::Ask(ask) => ask.chat(),
            Self::ByHand(notice) => notice.chat,
            Self::Saved(saved) => saved.chat,
            Self::Refused(refused) => refused.chat,
            Self::Tool(call) => call.chat,
            Self::Permission(asked) => asked.chat,
            Self::Touching(touching) => touching.chat,
            Self::Blocked(blocked) => blocked.chat,
            Self::Doing(doing) => doing.chat,
            Self::Output(output) => output.chat,
            Self::SecretExec(ask) => ask.chat,
            Self::Frames(hello) => hello.chat,
            Self::Serve(ask) => ask.chat,
        }
    }
}

/// A harness the operator started by hand, in a shell tab's own shell (SI-5).
///
/// **Neither a report nor an ask.** It moves no chat — nothing about a chat's state is
/// learned from it — and it asks the app to do nothing: the window draws a banner on the tab
/// it came from, and whatever happens next is the operator's click. What sends it is `charter
/// shell-guard`, which the shims on a shell tab's `PATH` run in front of the real harness, so
/// the whole of the detection is which command was started. Nothing reads what the harness
/// then prints.
///
/// Anything that can write the socket can send one, which is the same account that can already
/// move a chat's state; the most it buys is a banner the operator can dismiss.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StartedByHand {
    /// The app's number for the shell tab's chat, from [`CHAT_ENV`].
    pub chat: u32,
    /// The harness, by the word the plane calls it (`claude`, `codex`, `opencode`).
    pub started_by_hand: String,
    /// Where the shell was standing when it started it — where a chat opened in its place
    /// would start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<std::path::PathBuf>,
}

/// What hears a [`StartedByHand`].
pub type Noticed = Box<dyn Fn(StartedByHand) + Send + Sync + 'static>;

/// A chat wrote its session record: the end of a Smart close (SI-8, ADR 0064).
///
/// **Neither a report nor an ask, and it moves no chat.** `charter session record` sends it
/// once the record, the index and the workspace's pointer are on disk, so the app hears that
/// the one thing a Smart close waits for is done — from the command that did it, never from
/// reading what the harness printed. What the app does with it is the app's: close the tab it
/// was closing, or nothing, for a chat it was not closing. A line from a chat charter did not
/// ask to close therefore closes nothing.
///
/// Anything that can write the socket can send one, which is the account that can already
/// move a chat's state; the most it buys is the close of a tab the operator already asked to
/// close.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionSaved {
    /// The app's number for the chat that wrote it, from [`CHAT_ENV`].
    pub chat: u32,
    /// The record, as a path the app can open: absolute.
    pub session_saved: std::path::PathBuf,
}

/// What hears a [`SessionSaved`].
pub type Saved = Box<dyn Fn(SessionSaved) + Send + Sync + 'static>;

/// An agent's commit was refused before it was made: charter's `pre-commit` or
/// `pre-merge-commit` found a secret or personal data in what it adds (SQ-16, ADR 0074,
/// [`crate::diffscan`]).
///
/// **Neither a report nor an ask.** It does not move the chat's state — the turn goes on, and
/// the agent has git's refusal in front of it — but it is a needs-you item of its own: the
/// operator hears that a commit was stopped, and what for, masked.
///
/// **The guard is the chat's token**, as for every line: the listener hands this on only when
/// the token beside it is the one [`ChatTokens`] issued for the chat `Line::chat()` names — here
/// `chat`. So a line naming a chat is believed only from something holding that chat's token,
/// which is what the chat's own process tree holds (ADR 0068 §5, the `chat` scope). What that
/// buys a process in the chat is an item in its own chat's queue, which the operator can
/// ignore.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommitRefused {
    /// The app's number for the chat whose commit it was, from [`CHAT_ENV`].
    pub chat: u32,
    /// What the queue item says: the repository and the first finding, masked. Never a value.
    pub commit_refused: String,
}

/// What hears a [`CommitRefused`].
///
/// It answers once the line is recorded durably, or with the error that kept it from being:
/// only an `Ok` is told to the hook as taken, so a line the host could not record is spooled
/// (FD-30).
pub type Refused = Box<dyn Fn(CommitRefused) -> std::io::Result<()> + Send + Sync + 'static>;

/// A tool hook ran: what the host's event log records of it (FD-9, #649).
///
/// **Neither a report nor an ask, and it moves no chat.** Every tool hook `charter hook`
/// answers sends one, after it has answered, so the host has one event per hook call. It never
/// carries the tool's arguments: `args` is their SHA-256 ([`crate::eventlog::args_hash`]), which
/// the host keys again before anything is written, and which a spooled line leaves out (#977). A line that cannot be sent changes nothing
/// about the answer the harness was given.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// The app's number for the chat the hook ran in, from [`CHAT_ENV`].
    pub chat: u32,
    /// The hook's word: `pretooluse`, `pretooluse-read`, `posttooluse`, ….
    pub tool_hook: String,
    /// The tool, by the harness's name for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// The harness's id for this one call, which pairs its pre hook with its post hook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<String>,
    /// SHA-256 of the arguments, in hex. Never the arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// What charter's hook answered.
    #[serde(default)]
    pub decision: Decision,
    /// The guard rule that refused, for a `deny`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// How long charter's hook took, in milliseconds.
    #[serde(default)]
    pub hook_ms: u64,
    /// The sub-agent that made the call ([`Report::agent`]'s rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// When the hook began, in milliseconds since 1970 by the hook's own clock: what pairs a
    /// call's pre and post hooks into a duration whichever the host hears first. 0 is unknown.
    #[serde(default)]
    pub at_ms: u64,
    /// The skill run this call starts, where the hook found it starts one (SK-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<Box<SkillStarted>>,
}

/// `at` in milliseconds since 1970, the clock every line's `at_ms` is on; 0 for a time before it.
pub fn ms_since_1970(at: std::time::SystemTime) -> u64 {
    at.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|since| u64::try_from(since.as_millis()).ok())
        .unwrap_or_default()
}

/// The word a [`ToolCall`] line carries in [`ToolCall::tool_hook`] when purlis's own MCP server,
/// not a hook, says the chat loaded a skill with `skill_load` (SK-13, #2776): a line with the
/// [`SkillStarted`] the load opens, sent the way a tool hook sends its line, so the host opens the
/// run through the one [`crate::skillspan::Tracker`] every start goes through. It is counted as
/// the tool call it is, recorded as `hook.skill-load`.
pub const SKILL_LOAD: &str = "skill-load";

/// **A skill run a tool call starts** (SK-3, #2766), as the hook found it from the harness's
/// own words ([`crate::harness::HarnessAdapter::skill_start`]): the Skill tool's skill, or the
/// skill whose `SKILL.md` a briefed chat read; or the skill purlis's MCP server loaded
/// ([`SKILL_LOAD`]). The host opens the run under the id the hook
/// minted, where its turn has not started that skill already ([`crate::skillspan`]).
///
/// It comes from a chat's side of the channel, so the host reads it as a claim: a name is
/// contained before it is recorded, and an id that is not a ULID or a start it does not know
/// opens no run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillStarted {
    /// The skill, by the name the harness started it by.
    pub skill: String,
    /// The run's id: a ULID the hook minted.
    pub run: String,
    pub started_by: purlis_skills::runs::StartedBy,
    /// The workspace the chat worked in, or none at the plane root (SK-13): with [`Self::level`],
    /// which copy of the skill the run is of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The copy of the skill, by its level, where the start knew it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "purlis_skills::runs::level_or_none"
    )]
    pub level: Option<purlis_skills::runs::Level>,
}

impl ToolCall {
    /// The line as it may be kept on disk: the same call without its argument digest (#977).
    /// The digest is unkeyed, so anyone who reads it can match it against a guess at the
    /// arguments; only the host keys it ([`crate::eventlog::ArgsKey`]) before anything is
    /// written, and a spooled line, which no host has keyed yet, leaves it out.
    pub(crate) fn at_rest(&self) -> ToolCall {
        ToolCall {
            args: None,
            ..self.clone()
        }
    }
}

/// What a tool hook answered the harness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// It allowed the call.
    Allow,
    /// It asked the operator.
    Ask,
    /// It refused the call.
    Deny,
    /// It said nothing either way, so the harness decides as it would have.
    #[default]
    None,
}

impl Decision {
    /// The word an event records.
    pub fn word(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
            Self::None => "none",
        }
    }
}

/// What hears a [`ToolCall`].
///
/// It answers as a [`Refused`] does: `Ok` once the call is recorded durably.
pub type Tooled = Box<dyn Fn(ToolCall) -> std::io::Result<()> + Send + Sync + 'static>;

/// A file tool of a chat touched a path (FM-6, #1109): what the window marks in its tree.
///
/// **Never recorded, anywhere** (D-86a). It is a line of its own so that nothing that records
/// a line ever holds it: it is not a [`ToolCall`], whose line the event log keeps and the spool
/// writes to disk, and it is never spooled ([`touch`] sends it once, or not at all). The host
/// hands it to the window in memory and forgets it. What the event log keeps of the same call is
/// the digest of its arguments (ADR 0066).
///
/// The path is the chat's own word, so the host believes none of it until
/// [`crate::touching::confine`] has put it inside the chat's folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Touching {
    /// The app's number for the chat whose tool it was, from [`CHAT_ENV`].
    pub chat: u32,
    /// The path the tool was given, as the harness spelled it.
    pub touching: String,
    /// Whether the tool was one that writes the file ([`crate::touching::writes`]): what a
    /// task's Changes tab keeps (#1511). Absent from a line an older hook sent, and read as a
    /// read then: a path only read is never listed as a change.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrote: bool,
}

/// What hears a [`Touching`]. Nothing is answered: the hook does not wait for it.
pub type Touched = Box<dyn Fn(Touching) + Send + Sync + 'static>;

/// What a chat's tool hook says the chat is doing (#1493): what the window says in one line
/// under a working chat's name.
///
/// **Never recorded, anywhere**, as a [`Touching`] is not: a line of its own, sent once and
/// never spooled ([`tell_doing`]), which the host keeps in memory until the chat's next one or
/// the end of its turn. It holds a kind from a fixed list and at most one short name
/// ([`crate::doing`]): never a command's arguments, a path, or anything a tool came back with.
///
/// The name is the chat's own word, so the host believes none of it until
/// [`crate::doing::Said::neutral`] has passed it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Doing {
    /// The app's number for the chat whose tool it was, from [`CHAT_ENV`].
    pub chat: u32,
    /// What the hook said.
    pub doing: crate::doing::Said,
    /// The sub-agent whose tool it was ([`Report::agent`]'s rule). The chat's line is of the
    /// chat's own work, so the host says nothing of a helper's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The harness run the hook ran under (#1601), judged against the run the chat adopted
    /// before the line may say the chat got past its prompt. A line without one is no one's,
    /// and is written without it.
    #[serde(default, skip_serializing_if = "Speaker::is_no_one")]
    pub speaker: Speaker,
}

/// What hears a [`Doing`]. Nothing is answered: the hook does not wait for it.
pub type DoingHeard = Box<dyn Fn(Doing) + Send + Sync + 'static>;

/// What a chat's turn ended saying (#1981): the harness's own last message of the turn, as its
/// `Stop` hook is handed it. What a task that ended its turns without a report is reported
/// from, as **a report purlis made from its last message**.
///
/// **The line itself is never recorded**, as a [`Doing`] is not: a line of its own, sent once
/// and never spooled ([`tell_turn_output`]), which the host keeps in memory, for a task alone,
/// until its next turn begins or its chat closes. The text is the chat's own words: the host
/// masks what looks like a credential before it keeps any of it, and hands it on only quoted
/// as data. **A report purlis makes from it is recorded, by design**: that masked text is the
/// report, left for the chat that asked and kept on the task's dispatch record as any report is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TurnOutput {
    /// The app's number for the chat the hook ran in, from [`CHAT_ENV`].
    pub chat: u32,
    /// What it said.
    pub turn_output: Output,
    /// The harness run the hook ran under (#1601), judged against the run the chat adopted: a
    /// harness nested in the chat says nothing for it.
    #[serde(default, skip_serializing_if = "Speaker::is_no_one")]
    pub speaker: Speaker,
}

/// One thing a chat's turn put out ([`TurnOutput`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "is", rename_all = "snake_case")]
pub enum Output {
    /// The last message of a turn that has ended.
    Final {
        /// The message, cut to [`Output::MOST_BYTES`] by the hook.
        text: String,
    },
    /// A shell command the turn ran, as its tool's hook heard it come back (#1981): what
    /// `purlis dispatch read` lists of a task.
    Ran {
        /// The command line, cut to [`Output::MOST_COMMAND_BYTES`] by the hook.
        command: String,
        /// Its exit status, where the harness said one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit: Option<i32>,
        /// Whether its harness said it failed, with or without a status.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        failed: bool,
    },
}

impl Output {
    /// The most bytes of a message the hook sends: a report's text is far shorter, and what is
    /// kept is cut again to it.
    pub const MOST_BYTES: usize = 16 * 1024;

    /// The most bytes of a command line the hook sends.
    pub const MOST_COMMAND_BYTES: usize = 1024;

    /// The command a shell tool's post hook `word` heard come back, from its `payload`, or none
    /// for any other hook or tool (#1981). `harness` is the chat's harness, by the word the
    /// project calls it ([`HARNESS_ENV`]).
    ///
    /// **The status is the harness's word, where it says one**: opencode's (purlis's plugin
    /// sends `exit_code`); Claude Code's, whose `PostToolUse` fires for a command that
    /// succeeded and `PostToolUseFailure` for one that did not, with "Exit code N" in its
    /// error. A command left running in the background has no status yet.
    pub fn of_ran(word: &str, payload: &serde_json::Value, harness: Option<&str>) -> Option<Self> {
        if !word.starts_with("posttooluse") {
            return None;
        }
        if !crate::doing::is_a_command(payload["tool_name"].as_str()?) {
            return None;
        }
        let input = &payload["tool_input"];
        let command = match &input["command"] {
            serde_json::Value::String(line) => line.trim().to_owned(),
            serde_json::Value::Array(words) => words
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            _ => return None,
        };
        if command.is_empty() {
            return None;
        }
        let mut end = command.len().min(Self::MOST_COMMAND_BYTES);
        while !command.is_char_boundary(end) {
            end -= 1;
        }
        let failure = word.starts_with("posttoolusefailure");
        let background = input["run_in_background"].as_bool() == Some(true);
        let exit = payload["exit_code"]
            .as_i64()
            .and_then(|code| i32::try_from(code).ok())
            .or_else(|| {
                failure
                    .then(|| payload["error"].as_str().and_then(exit_code_in))
                    .flatten()
            })
            .or_else(|| {
                (!failure && !background && harness == Some(BACKGROUND_TASKS_HARNESS)).then_some(0)
            });
        Some(Self::Ran {
            command: command[..end].to_owned(),
            exit,
            failed: failure || exit.is_some_and(|code| code != 0),
        })
    }

    /// The last message `text` of a turn, cut on a character to [`Self::MOST_BYTES`], or none
    /// for one with nothing in it.
    pub fn last_message(text: &str) -> Option<Self> {
        Self::last_message_within(text, Self::MOST_BYTES)
    }

    /// [`Self::last_message`], cut to `most` bytes, and so that its line as JSON writes it is one
    /// the app reads: a text of what JSON escapes is longer on the line (SK-55).
    pub fn last_message_within(text: &str, most: usize) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let most_written = A_LINE_IS_AT_MOST as usize - LINE_BESIDES_ITS_MESSAGE;
        let (mut end, mut written) = (0, 0);
        for (at, ch) in text.char_indices() {
            written += match ch {
                '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
                ch if u32::from(ch) < 0x20 => 6,
                ch => ch.len_utf8(),
            };
            if at + ch.len_utf8() > most || written > most_written {
                break;
            }
            end = at + ch.len_utf8();
        }
        Some(Self::Final {
            text: text[..end].to_owned(),
        })
    }

    /// The last message a `Stop` hook's `payload` hands it, where the harness hands one: Claude
    /// Code's and Codex's `last_assistant_message`, which purlis's opencode plugin sends too.
    pub fn of_stop(payload: &str) -> Option<Self> {
        Self::of_stop_within(payload, Self::MOST_BYTES)
    }

    /// [`Self::of_stop`], cut to `most` bytes: a judge's answer has to come back whole (SK-55).
    pub fn of_stop_within(payload: &str, most: usize) -> Option<Self> {
        let read = serde_json::from_str::<serde_json::Value>(payload).ok()?;
        Self::last_message_within(read.get("last_assistant_message")?.as_str()?, most)
    }
}

/// What a turn's output line holds beside its message: its keys, its chat and who said it.
const LINE_BESIDES_ITS_MESSAGE: usize = 1024;

/// The status in a Claude Code `PostToolUseFailure` error: "Exit code N" at its start.
fn exit_code_in(error: &str) -> Option<i32> {
    let rest = error.trim_start().strip_prefix("Exit code ")?;
    let digits: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().ok()
}

/// What hears a [`TurnOutput`]. Nothing is answered: the hook does not wait for it.
pub type OutputHeard = Box<dyn Fn(TurnOutput) + Send + Sync + 'static>;

/// A chat's sandbox blocked an operation (#1338): what the window shows as a notice on the
/// chat's tab, and what `purlis doctor` counts.
///
/// **Neither a report nor an ask, and it moves no chat.** A tool hook that runs outside the
/// sandbox read the block in the harness's own report of the tool's result and sorted it
/// ([`crate::sandboxblock::detect`]); this carries only that sort: an operation, a kind, and
/// whether it was purlis's own; and, for a block a person could grant (#1342), the host or
/// path it would name ([`Self::target`]). No argument or output is on the line.
///
/// Sent once, and never spooled ([`tell_blocked`]): a block the app did not take is one nobody
/// was shown, and the count is of what the app heard. Anything holding the chat's token can send
/// one, `ours` included, which buys a notice on its own chat's tab and one more in its project's
/// count; the app takes a handful a minute per chat ([`crate::sandboxblock::Throttle`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SandboxBlocked {
    /// The app's number for the chat that was blocked, from [`CHAT_ENV`].
    pub chat: u32,
    /// The block, sorted.
    pub sandbox_blocked: crate::sandboxblock::Block,
    /// The harness, by the word the project calls it ([`HARNESS_ENV`]), when the chat says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// What a grant would name, for the Notice's Allow (#1342,
    /// [`crate::sandboxblock::detect_with_targets`]): a host, or a whole path; for a refused
    /// lookup, the host it was of, shown and never offered (#1663). Held by the app in memory
    /// for the Notice and checked again before anything is granted. A host is kept with the
    /// block in the network record once checked as a grant checks one
    /// ([`crate::sandboxblock::record::Entry`]); a path never is, and neither is counted or
    /// reported. A chat can send any, which buys only a Notice asking the person on its own tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Whether the block was found in what a tool that ran nothing read back
    /// ([`crate::sandboxblock::read_back`], #1884): the app then shows a quiet line nobody has
    /// to answer. A chat can send either; what `true` buys is a quieter Notice on its own tab.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub read_back: bool,
    /// **A port the command's own printed words name** for a refused service on this Mac whose
    /// [`Self::target`] is none (qw384, [`crate::sandboxblock::printed_port`]): only ever
    /// pre-filled in the Notice's port field, said to come from the command's output for the
    /// person to check, and judged again as a grant before anything is allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested: Option<String>,
}

/// What hears a brokered `secret exec` ([`crate::secrets::brokered`]): the ask, and its
/// connection, which it writes every frame on and reads the asker's stdin from. It returns once
/// the command has ended; the connection ends with it.
pub type SecretExecuting = Box<
    dyn Fn(
            crate::secrets::brokered::Ask,
            Box<dyn std::io::BufRead + Send>,
            Box<dyn std::io::Write + Send>,
        ) + Send
        + Sync
        + 'static,
>;

/// **What hears a `serve` line** (#3316): the ask and its connection, once the chat's token
/// has checked. It parks the connection as a slot of the chat's door, or refuses it
/// ([`serve::parked`], [`serve::refused`]); it returns at once either way.
pub type Serves = Box<dyn Fn(serve::ServeAsk, serve::Slot) + Send + Sync + 'static>;

/// What hears a [`SandboxBlocked`]. Nothing is answered: the hook does not wait for it.
pub type Blocked = Box<dyn Fn(SandboxBlocked) + Send + Sync + 'static>;

/// What hears a [`Report`]. It answers as a [`Refused`] does.
pub type Reported = Box<dyn Fn(Report) -> std::io::Result<()> + Send + Sync + 'static>;

/// Who hears each kind of line on the hook channel ([`Listener::hear`]): one field per kind, so
/// a kind added later is a field every caller is made to answer.
pub struct Hearing {
    /// Every [`Report`].
    pub each: Reported,
    /// Every [`Ask`], whose [`Answer`] is written back on the connection it came on.
    pub answer: Answerer,
    /// Every [`StartedByHand`].
    pub noticed: Noticed,
    /// Every [`SessionSaved`].
    pub saved: Saved,
    /// Every [`CommitRefused`].
    pub refused: Refused,
    /// Every [`ToolCall`].
    pub tool: Tooled,
    /// Every [`PermissionAsked`]: held on its connection until it is answered ([`permission`]).
    pub permission: Permitting,
    /// Every [`Touching`].
    pub touching: Touched,
    /// Every [`SandboxBlocked`].
    pub blocked: Blocked,
    /// Every [`Doing`].
    pub doing: DoingHeard,
    /// Every [`TurnOutput`].
    pub output: OutputHeard,
    /// Every brokered `secret exec`: held on its connection while the command runs.
    pub secret_exec: SecretExecuting,
    /// Every `serve`: its connection parked as a slot of the chat's door, or refused (#3316).
    pub serve: Serves,
}

/// Whether an app is accepting on the hook socket at `path` right now (#95): what the status
/// line asks before it says it runs in the app, stated here once with the channel's shape.
///
/// A `connect` succeeds only while an app accepts, and a socket file left behind by one that
/// has gone refuses with `ECONNREFUSED`. Neither can hang, so no deadline is armed. Nothing
/// is written: the connection is closed unused, which the app reads as a line that never came.
#[cfg(unix)]
pub fn listening(path: &std::path::Path) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

/// Sends one report to the socket at `path`. Answers whether it was written: what the app made
/// of it, taken or refused, is not read.
///
/// One connection is one report, written, and then **held until the app lets it go**
/// (D-1407-10), as [`tell`] and [`deliver`] hold theirs: the app judges a line by the process
/// that sent it, read once the line has arrived. On macOS the kernel names that process
/// (`LOCAL_PEERPID`) only while the connection is still open, so a sender that closed the
/// moment it wrote was refused unread whenever the app accepted after the close (#2458). The
/// wait is bounded by [`A_REPORT_TAKES_AT_MOST`] for an app that has stopped reading.
#[cfg(unix)]
pub fn send(path: &std::path::Path, token: Option<&ChatToken>, report: &Report) -> io::Result<()> {
    use std::io::{Read, Write};

    // A path that is not there, and a socket file whose app has gone, both refuse at once
    // — `ENOENT` and `ECONNREFUSED`. Neither can hang, so no timeout is armed for them.
    let mut socket = std::os::unix::net::UnixStream::connect(path)?;
    socket.write_all(&line_with(token, report)?)?;
    socket.flush()?;
    // An app that has handled the line and closed already makes these fail on macOS
    // (`ENOTCONN`): the line was written, which is all this answers.
    let _ = socket.shutdown(std::net::Shutdown::Write);
    if socket
        .set_read_timeout(Some(A_REPORT_TAKES_AT_MOST))
        .is_ok()
    {
        let _ = socket.read_to_end(&mut Vec::new());
    }
    Ok(())
}

/// Tells the app at `path` a harness was started by hand. Answers whether the app took it.
///
/// [`send`]'s shape — one connection, one line, closed — with a deadline on the write as well:
/// what calls it is a harness the operator is waiting to see start, and it is started whether
/// or not this got through.
#[cfg(unix)]
pub fn tell(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    notice: &StartedByHand,
) -> io::Result<()> {
    one_line_with_a_deadline(path, token, notice)
}

/// Tells the app at `path` which file a chat's tool touched (FM-6). Answers whether it was
/// written.
///
/// [`tell`]'s shape and deadline, and **never spooled**: a marker the app did not take is a
/// marker nobody needed, and a spool is a file the path would then be in (D-86a).
#[cfg(unix)]
pub fn touch(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    touching: &Touching,
) -> io::Result<()> {
    one_line_with_a_deadline(path, token, touching)
}

/// Tells the app at `path` what its chat's tool hook says the chat is doing (#1493). Answers
/// whether it was written.
///
/// [`tell`]'s shape and deadline, and **never spooled**, as a touch is not: a line the app did
/// not take is one nobody was shown, and the next tool says the next thing.
#[cfg(unix)]
pub fn tell_doing(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    doing: &Doing,
) -> io::Result<()> {
    one_line_with_a_deadline(path, token, doing)
}

/// Tells the app at `path` what its chat's turn put out (#1981). Answers whether it was written.
///
/// [`tell`]'s shape and deadline, and **never spooled**, as a [`Doing`] is not: a spool is a
/// file the chat's words would then be in.
#[cfg(unix)]
pub fn tell_turn_output(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    output: &TurnOutput,
) -> io::Result<()> {
    one_line_with_a_deadline(path, token, output)
}

/// Tells the app at `path` its chat's sandbox blocked an operation (#1338). Answers whether it
/// was written.
///
/// [`tell`]'s shape and deadline, and never spooled ([`SandboxBlocked`]).
#[cfg(unix)]
pub fn tell_blocked(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    blocked: &SandboxBlocked,
) -> io::Result<()> {
    one_line_with_a_deadline(path, token, blocked)
}

/// Tells the app at `path` a chat's session record is saved. Answers whether the app took it.
///
/// [`tell`]'s shape and deadline: the record is on disk whether or not the app hears this, and
/// the command that sends it must not hang on an app that stopped reading.
#[cfg(unix)]
pub fn tell_saved(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    saved: &SessionSaved,
) -> io::Result<()> {
    one_line_with_a_deadline(path, token, saved)
}

/// A line a hook delivers, which its chat's spool keeps when the host does not take it
/// (FD-30): a [`Report`], a [`ToolCall`] or a [`CommitRefused`]. [`deliver`] serves all three.
///
/// **Sealed.** The drain reads these three kinds back and no other ([`spool::Spooled`]), so
/// nothing outside this module can make a fourth kind a hook would spool.
pub trait SpoolLine: serde::Serialize + sealed::Sealed + Sized {
    /// The number of the chat whose spool the line goes to.
    fn chat(&self) -> u32;

    /// The line as its spool keeps it: without what only a live host may hear. The line itself
    /// where nothing is.
    fn kept(&self) -> Option<Self> {
        None
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Report {}
    impl Sealed for super::ToolCall {}
    impl Sealed for super::CommitRefused {}
}

impl SpoolLine for Report {
    fn chat(&self) -> u32 {
        self.chat
    }

    fn kept(&self) -> Option<Self> {
        Some(self.at_rest())
    }
}

impl SpoolLine for ToolCall {
    fn chat(&self) -> u32 {
        self.chat
    }
}

impl SpoolLine for CommitRefused {
    fn chat(&self) -> u32 {
        self.chat
    }
}

/// Where a hook's line went ([`deliver`]: [`deliver_report`], [`deliver_tool`],
/// [`deliver_refused`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivered {
    /// The host said it took the line, once its hearer had recorded it.
    Taken,
    /// The host did not take it in time, and it is in the chat's spool, durably, under this
    /// number (FD-30).
    Spooled(u64),
}

/// The host's receipt for a hook's line, `{"taken": true}` once the line is recorded, or
/// `{"taken": false, "refused": "…"}`. Hooks of every build read these words, so they never
/// change; the type's name is the host's own.
#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize)]
struct Receipt {
    /// Whether the host recorded the line.
    taken: bool,
    /// Why it was refused, where it was (D-1407-9): the hook says this and spools nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refused: Option<String>,
}

/// Delivers a chat's report: taken by the host at `path`, or spooled beside it ([`deliver`]).
///
/// **With the chat's armed key beside it, where this process can read one** (#3822,
/// [`armed::ArmedKey::read`]): the chat's armed hook can, so the mode it reports is believed.
#[cfg(unix)]
pub fn deliver_report(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    report: &Report,
) -> io::Result<Delivered> {
    deliver_armed(
        path,
        token,
        armed::ArmedKey::read(path, report.chat).as_ref(),
        report,
    )
}

/// Delivers a tool call's line: taken by the host at `path`, or spooled beside it
/// ([`deliver`]).
#[cfg(unix)]
pub fn deliver_tool(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    call: &ToolCall,
) -> io::Result<Delivered> {
    deliver(path, token, call)
}

/// Delivers a refused commit's line: taken by the host at `path`, or spooled beside it
/// ([`deliver`]).
#[cfg(unix)]
pub fn deliver_refused(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    refused: &CommitRefused,
) -> io::Result<Delivered> {
    deliver(path, token, refused)
}

/// The line to the host, and an answer from it that it took the line, within
/// [`A_NOTICE_TAKES_AT_MOST`]; otherwise the line in its chat's spool ([`spool::append`]). The
/// one delivery every [`SpoolLine`] kind takes.
///
/// **When this returns `Ok` the line is recorded or durable** (ADR 0075 §7, FD-30): a hook
/// answers its harness after this, so a host that is down, slow or gone costs the line its
/// moment, never the line. A host that took it after the wait ran out may record it as well
/// as the drain: the line carries a [`Delivery`] to the host and into the spool, so the drain
/// knows a copy of a line the host recorded live, and the line is recorded once (#952).
///
/// Without a token nothing can be spooled, since a spool line is checked under the token's key,
/// and the error says the line is lost.
#[cfg(unix)]
pub fn deliver<L: SpoolLine>(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    line: &L,
) -> io::Result<Delivered> {
    deliver_armed(path, token, None, line)
}

/// [`deliver`], with `armed` beside the token on the socket ([`line_armed`]): on the socket
/// alone, never into the spool, whose lines prove no mode (#3822).
#[cfg(unix)]
pub fn deliver_armed<L: SpoolLine>(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    armed: Option<&armed::ArmedKey>,
    line: &L,
) -> io::Result<Delivered> {
    use std::io::{BufRead, Read, Write};

    let delivery = Delivery::new();
    let delivered = with_delivery(line, delivery.as_ref())?;
    let bytes = line_armed(token, armed.map(armed::ArmedKey::expose), &delivered)?;
    let to = path.to_path_buf();
    let answered = within(A_NOTICE_TAKES_AT_MOST, move || {
        let socket = std::os::unix::net::UnixStream::connect(&to)?;
        socket.set_write_timeout(Some(A_NOTICE_TAKES_AT_MOST))?;
        socket.set_read_timeout(Some(A_NOTICE_TAKES_AT_MOST))?;
        (&socket).write_all(&bytes)?;
        let mut said = String::new();
        std::io::BufReader::new(&socket)
            .take(256)
            .read_line(&mut said)?;
        match serde_json::from_str::<Receipt>(&said) {
            Ok(Receipt { taken: true, .. }) => Ok(Ok(())),
            Ok(Receipt {
                refused: Some(why), ..
            }) => Ok(Err(why)),
            _ => Err(io::Error::other("the app did not say it took the line")),
        }
    });
    let why = match answered {
        Ok(Ok(())) => return Ok(Delivered::Taken),
        // Refused, not missed: the app is there and said no, so nothing is spooled to be read
        // as if it had been away.
        Ok(Err(refused)) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, refused)),
        Err(why) => why,
    };
    let Some(token) = token else {
        return Err(io::Error::new(
            why.kind(),
            format!("{why}, and with no chat token it cannot be spooled, so it is lost"),
        ));
    };
    let kept = match line.kept() {
        Some(kept) => with_delivery(&kept, delivery.as_ref())?,
        None => delivered,
    };
    spool::append(&spool::dir_for(path), line.chat(), token, &kept)
        .map(Delivered::Spooled)
        .map_err(|spooled| not_spooled(&why, spooled))
}

/// What a hook says of a line the host did not take (`why`) and its spool did not keep
/// (`spooled`). A spool the hook stopped waiting for ([`io::ErrorKind::TimedOut`]) may still
/// finish the write, and the next open of the project then records the line, so that one is
/// said to be perhaps lost; every other is lost.
#[cfg(unix)]
fn not_spooled(why: &io::Error, spooled: io::Error) -> io::Error {
    let so = if spooled.kind() == io::ErrorKind::TimedOut {
        "so it may be lost"
    } else {
        "so it is lost"
    };
    io::Error::new(
        spooled.kind(),
        format!("{why}, and it could not be spooled ({spooled}), {so}"),
    )
}

/// `act`, or an error once `deadline` has passed without it finishing.
#[cfg(unix)]
///
/// On a thread of its own, as `charter hook`'s payload read is: a `connect` to a socket whose
/// backlog is full blocks, on Linux, for as long as the app does not accept, and an app that
/// is frozen never does. A guard's verdict must never wait on that, so the act is let go and
/// left to finish or fail on its own; the process is on its way out anyway.
#[cfg(unix)]
fn within<T: Send + 'static>(
    deadline: std::time::Duration,
    act: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("charter-hook-notice".into())
        .spawn(move || {
            let _ = done.send(act());
        })?;
    finished.recv_timeout(deadline).unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the app did not take the line in time",
        ))
    })
}

/// One connection, one line, and the app's letting go waited for: at most
/// [`A_NOTICE_TAKES_AT_MOST`] spent writing it, and [`A_NOTICE_IS_SEEN_WITHIN`] in all.
#[cfg(unix)]
fn one_line_with_a_deadline(
    path: &std::path::Path,
    token: Option<&ChatToken>,
    line: &impl serde::Serialize,
) -> io::Result<()> {
    use std::io::Write;

    let bytes = line_with(token, line)?;
    let path = path.to_path_buf();
    // The connect as well as the write is bounded: see [`within`].
    within(A_NOTICE_IS_SEEN_WITHIN, move || {
        let mut socket = std::os::unix::net::UnixStream::connect(&path)?;
        socket.set_write_timeout(Some(A_NOTICE_TAKES_AT_MOST))?;
        socket.write_all(&bytes)?;
        socket.flush()?;
        // And then stays until the app has let the connection go (D-1407-10): it judges a line
        // by the process that sent it, read once the line has arrived, and one that exited the
        // moment it wrote could be gone by then. An app that says nothing closes once the line
        // is handled; one that answers is read to its end. A frozen app costs a notice up to
        // [`A_NOTICE_IS_SEEN_WITHIN`] where it once cost [`A_NOTICE_TAKES_AT_MOST`].
        socket.shutdown(std::net::Shutdown::Write)?;
        socket.set_read_timeout(Some(A_NOTICE_IS_SEEN_WITHIN))?;
        let mut rest = Vec::new();
        let _ = std::io::Read::read_to_end(&mut socket, &mut rest);
        Ok(())
    })
}

/// How long [`tell`] and [`tell_saved`] may spend writing their line: a line is under 4 KiB and the app reads it on
/// a thread of its own, so this is only a bound on an app that has stopped reading.
#[cfg(unix)]
const A_NOTICE_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_millis(250);

/// How long a notice's sender stays, all told, for the app to look at who sent it and let the
/// connection go (D-1407-10). Spent in full only on an app that has stopped reading.
#[cfg(unix)]
const A_NOTICE_IS_SEEN_WITHIN: std::time::Duration = std::time::Duration::from_millis(500);

/// The longest a tool hook can spend telling the app, after it has decided and before it
/// answers its harness, with an app that has stopped reading: its tool call's line
/// ([`deliver_tool`], and the spool it falls back to), then the two lines nothing records
/// ([`touch`], [`tell_doing`]). Whatever decides a tool call adds this to its own budget and
/// holds the sum inside every harness's timeout for the hook (#1493).
#[cfg(unix)]
pub const A_TOOL_HOOK_TELLS_WITHIN: std::time::Duration = std::time::Duration::from_millis(
    (A_NOTICE_TAKES_AT_MOST.as_millis()
        + spool::A_LINE_IS_SPOOLED_WITHIN.as_millis()
        + 2 * A_NOTICE_IS_SEEN_WITHIN.as_millis()) as u64,
);

/// One conversation with the app: lines written, and each answered on the same connection.
///
/// **The same connection is the point.** A ticket is bound to the connection it was minted
/// on ([`OpenChat`]), so the mint and the spend have to travel together, and a line copied
/// onto a connection of its own spends nothing.
#[cfg(unix)]
pub struct Asking {
    stream: std::io::BufReader<std::os::unix::net::UnixStream>,
    token: Option<ChatToken>,
}

#[cfg(unix)]
impl Asking {
    /// Connects to the app listening at `path`.
    ///
    /// A path that is not there, and a socket whose app has gone, both refuse at once —
    /// `ENOENT` and `ECONNREFUSED` — so connecting cannot hang. What can is an app that
    /// takes the line and never answers, which is what [`Asking::ask`]'s deadline is for.
    ///
    /// `token` is the asking chat's own, carried on every line this writes.
    pub fn on(path: &std::path::Path, token: Option<ChatToken>) -> io::Result<Self> {
        let stream = std::os::unix::net::UnixStream::connect(path)?;
        Ok(Self {
            stream: std::io::BufReader::new(stream),
            token,
        })
    }

    /// Writes `ask` and reads the app's answer, waiting at most `within` for each half, and
    /// for each [`Answer::Alive`] line that comes before the answer.
    ///
    /// Every way this fails is an `Err` and never a wait: a handoff whose app did not answer
    /// prints the command to run in a terminal, which is a handoff the operator can still
    /// carry out, where a hang is a Bash tool call that never ends.
    pub fn ask(&mut self, ask: &Ask, within: std::time::Duration) -> io::Result<Answer> {
        use std::io::{BufRead, Read, Write};

        let line = line_with(self.token.as_ref(), ask)?;
        let socket = self.stream.get_mut();
        socket.set_write_timeout(Some(within))?;
        socket.set_read_timeout(Some(within))?;
        socket.write_all(&line)?;
        socket.flush()?;
        // A line saying the ask is still alive is read past, and the wait starts again (#4080):
        // `within` bounds each line, so only a silence that long ends it.
        loop {
            let mut said = String::new();
            // The same cap the app holds a report to, turned round: an answer is a ticket or a
            // sentence, and something that writes for ever without a newline is not the app.
            (&mut self.stream)
                .take(A_REPORT_IS_AT_MOST)
                .read_line(&mut said)?;
            if said.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the app closed the connection without answering",
                ));
            }
            match serde_json::from_str(&said).map_err(io::Error::other)? {
                Answer::Alive {} => {}
                answer => return Ok(answer),
            }
        }
    }
}

/// Where there is no unix socket, [`Asking`], [`send`], [`Listener`] and [`Reading`] are the
/// refusals in `off_unix`, kept in a file of their own because no unix build compiles them.
#[cfg(not(unix))]
mod off_unix;

#[cfg(unix)]
pub mod spool;

#[cfg(unix)]
pub mod armed;

pub mod permission;

pub mod alive;

pub mod framed;

pub mod relay;

pub mod serve;
#[cfg(unix)]
pub use framed::Framed;
#[cfg(not(unix))]
pub use off_unix::{
    Asking, Framed, Listener, Reading, deliver, deliver_refused, deliver_report, listening, send,
    tell, tell_saved,
};
pub use permission::{PermissionAsked, Permitting, ask_permission};

/// What the app answers an ask with, told which connection it came on.
///
/// The connection is a number the listener deals, one per connection and never twice. It is
/// how a ticket is bound to the connection that minted it without this module knowing what a
/// ticket is.
pub type Answerer = Box<dyn Fn(u64, Ask) -> Answer + Send + Sync + 'static>;

/// The socket the app listens on for hook reports.
#[cfg(unix)]
pub struct Listener {
    socket: std::os::unix::net::UnixListener,
    path: std::path::PathBuf,
    /// Which file at `path` is this listener's: the one `bind` made.
    file: BoundFile,
    /// Two ends of a pair that wakes the reading thread without the path. The thread waits on
    /// `woken` beside the socket; [`Reading`]'s drop writes to and closes `waking`.
    woken: std::os::unix::net::UnixStream,
    waking: std::os::unix::net::UnixStream,
    tokens: std::sync::Arc<ChatTokens>,
    /// The only uid a connection is read from: this process's own, as it binds (FD-6).
    owner: purlis_same_user::Uid,
    /// How many connections are served at once from one process, and from all (#991).
    serves: (usize, usize),
}

#[cfg(unix)]
impl Listener {
    /// Binds a fresh socket at `socket`, replacing one left behind by a process that is gone.
    ///
    /// `within` is where containment begins: every component from it down to the socket must
    /// not be a link (`contain::no_link_on_the_way`, shared with the record `reopen` keeps in
    /// the same directory). It is named by the caller and never worked out here — a walk from
    /// the root of the filesystem would refuse every path on macOS, where `/tmp` and `/var`
    /// are themselves links.
    ///
    /// **This is the one caller of that walk that cannot close the window after it** (charter
    /// ADR 0028). `reopen` opens the record through `contain::open_no_link`, which puts
    /// `O_NOFOLLOW` on the open so the kernel answers the last component at the instant it is
    /// opened; `bind` takes a path and there is no portable `bindat`, so the walk here, the
    /// `remove_file` of a stale socket below, the `bind` itself and the `set_permissions`
    /// after it are four path calls with three windows between them. The socket carries
    /// nothing secret and executes nothing it is handed, which is why that is accepted and
    /// written down rather than worked around; the fix is the same `openat`-beneath-a-
    /// descriptor rewrite ADR 0028 puts at M3.
    pub fn bind(within: &std::path::Path, socket: &std::path::Path) -> io::Result<Self> {
        // The same walk `reopen` uses for the record in this very directory
        // (charter-app#28), and the same one, not a second copy of it: two containment gates
        // drift, which is the failure this repo has found five times.
        crate::contain::no_link_on_the_way(within, socket)?;
        let path = socket;
        if let Some(parent) = path.parent() {
            private_directory(parent)?;
        }
        // `bind` refuses an address already in use, and a socket file outlives the process
        // that made it — so an app that was killed would stop the next one from listening at
        // all. Removing it first is the standard answer, and the single-instance plugin is
        // what makes it safe: there is no second live app whose socket this could be. On
        // Linux the app also holds a per-user lock (`instance.rs` in charter-app), because a
        // launch without a session bus has no single-instance name to hold.
        // Held from the stale file's removal to the new file's identity being read, and by a
        // stopping listener from its check to its removal: in this process a stop can never
        // take a file bound between the two (`SOCKET_FILES`).
        let files = SOCKET_FILES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        let socket = std::os::unix::net::UnixListener::bind(path)?;
        // Whoever can write here can move a chat's state and raise a notification. Nothing
        // secret travels over it and nothing it carries is executed, so this is not a
        // secret's lock — it is the difference between "the operator" and "anything running
        // on the machine", and it is one call.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        // Which file is this listener's, read the moment after it was made: a listener that
        // stops removes the file at its path only while it is still this one.
        let file = BoundFile::of(&std::fs::symlink_metadata(path)?);
        drop(files);
        // The reading thread waits on the socket and on this pair at once, and is woken
        // through the pair. Waking it by connecting to the path reached whatever listened
        // there by then, which need not be this listener.
        socket.set_nonblocking(true)?;
        let (woken, waking) = std::os::unix::net::UnixStream::pair()?;
        // Each token's spool key is recorded beside the socket as it is issued (FD-30), so
        // whichever host drains a chat's spool can check its lines: only where the sandbox's
        // integrity denial covers the directory (V63). Anywhere else nothing spools.
        let spool = spool::dir_for(path);
        let tokens = if spool::covered(&spool) {
            private_directory(&spool)?;
            ChatTokens::spooling_into(spool)
        } else {
            tracing::warn!(
                "purlis: the hook channel at {} is outside the directory a sandboxed chat is \
                 denied, so its hooks spool nothing and a line it does not take is lost",
                path.display()
            );
            ChatTokens::default()
        };
        Ok(Self {
            socket,
            path: path.to_path_buf(),
            file,
            woken,
            waking,
            tokens: std::sync::Arc::new(tokens),
            owner: purlis_same_user::Uid::effective(),
            serves: (A_PEER_IS_SERVED_AT_MOST, THE_CHANNEL_SERVES_AT_MOST),
        })
    }

    /// The same listener, reading only connections from `uid`. For the tests, which have no
    /// second user to connect as: told it belongs to another uid, the test's own connection
    /// is the other user's.
    #[cfg(test)]
    fn owned_by(mut self, uid: purlis_same_user::Uid) -> Self {
        self.owner = uid;
        self
    }

    /// Hears `serve` lines alone, each handed to `serve`; every other line is heard as
    /// [`Self::each`] hears it. For the tests of a server dialling in (#3316).
    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn serving_only(self, serve: Serves) -> Reading {
        let mut hearing = Hearing {
            each: Box::new(|_| Ok(())),
            answer: Box::new(|_, _| Answer::No {
                why: NOTHING_ANSWERS.to_owned(),
            }),
            noticed: Box::new(|_| {}),
            saved: Box::new(|_| {}),
            refused: Box::new(|_| Ok(())),
            tool: Box::new(|_| Ok(())),
            permission: Box::new(|_| None),
            blocked: Box::new(|_| {}),
            doing: Box::new(|_| {}),
            output: Box::new(|_| {}),
            touching: Box::new(|_| {}),
            secret_exec: Box::new(|_, _, writer| {
                crate::secrets::brokered::not_answered(writer);
            }),
            serve: serve::refusing(),
        };
        hearing.serve = serve;
        self.hear(hearing)
    }

    /// The same listener, serving at most `per_peer` connections from one process and
    /// `at_most` from all at once. For the tests, whose every connection is one process's.
    #[cfg(test)]
    fn serving_at_most(mut self, per_peer: usize, at_most: usize) -> Self {
        self.serves = (per_peer, at_most);
        self
    }

    /// The path a hook writes to.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// The chats' tokens this listener checks every line against. It starts with none, so
    /// it reads no line until the app issues a chat its token ([`ChatTokens::issue`]).
    pub fn tokens(&self) -> std::sync::Arc<ChatTokens> {
        std::sync::Arc::clone(&self.tokens)
    }

    /// Hands every report to `each`, on a thread of its own, until the listener is dropped.
    ///
    /// One connection is one report. A caller that cannot keep up does not block a harness:
    /// the hook has already written its line and gone.
    pub fn each(self, each: Box<dyn Fn(Report) + Send + Sync + 'static>) -> Reading {
        self.each_answering(
            each,
            Box::new(|_, _| Answer::No {
                why: NOTHING_ANSWERS.to_owned(),
            }),
        )
    }

    /// [`Listener::each`], and every [`Ask`] handed to `answer`, whose [`Answer`] is written
    /// back on the connection the ask came on.
    pub fn each_answering(
        self,
        each: Box<dyn Fn(Report) + Send + Sync + 'static>,
        answer: Answerer,
    ) -> Reading {
        self.each_answering_and_noticing(each, answer, Box::new(|_| {}))
    }

    /// [`Listener::each_answering`], and every [`StartedByHand`] handed to `noticed`.
    ///
    /// A [`SessionSaved`] is heard and dropped: an app that does not close a chat on one has
    /// nothing to do with it.
    pub fn each_answering_and_noticing(
        self,
        each: Box<dyn Fn(Report) + Send + Sync + 'static>,
        answer: Answerer,
        noticed: Noticed,
    ) -> Reading {
        self.each_answering_noticing_and_saving(each, answer, noticed, Box::new(|_| {}))
    }

    /// [`Listener::each_answering_and_noticing`], and every [`SessionSaved`] handed to `saved`.
    ///
    /// A [`CommitRefused`] is heard and dropped.
    pub fn each_answering_noticing_and_saving(
        self,
        each: Box<dyn Fn(Report) + Send + Sync + 'static>,
        answer: Answerer,
        noticed: Noticed,
        saved: Saved,
    ) -> Reading {
        self.hear(Hearing {
            each: Box::new(move |report| {
                each(report);
                Ok(())
            }),
            answer,
            noticed,
            saved,
            refused: Box::new(|_| Ok(())),
            tool: Box::new(|_| Ok(())),
            permission: Box::new(|_| None),
            blocked: Box::new(|_| {}),
            doing: Box::new(|_| {}),
            output: Box::new(|_| {}),
            touching: Box::new(|_| {}),
            secret_exec: Box::new(|_, _, writer| {
                crate::secrets::brokered::not_answered(writer);
            }),
            serve: serve::refusing(),
        })
    }

    /// Hands every line to whoever `hearing` names for its kind, on a thread of its own, until
    /// the listener is dropped. The other `each…` methods are this with the kinds they leave
    /// out dropped.
    pub fn hear(self, hearing: Hearing) -> Reading {
        let stopping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let Self {
            socket,
            path,
            file,
            woken,
            waking,
            tokens,
            owner,
            serves: (per_peer, at_most),
        } = self;
        let stopped = std::sync::Arc::clone(&stopping);
        let hearing = std::sync::Arc::new(hearing);
        let turns = std::sync::Arc::new(InTurn::new());
        let reading = std::thread::spawn(move || {
            let serving = Serving::new(per_peer, at_most);
            let _closing = Closing(std::sync::Arc::clone(&serving));
            let mut dealt: u64 = 0;
            loop {
                let woke = next_connection(&socket, &woken);
                if stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                // One bad connection is one lost report, never the end of the channel: an
                // app that stopped listening because something wrote nonsense would leave
                // every chat frozen at the state it last had, and never say so. The two
                // reachable ways in — a line that will not read, and one that will not parse
                // — have a test; `accept` failing does not, because nothing a test can do
                // makes it fail. It is written the same way for the same reason.
                let connection = match woke {
                    Woke::Connection(connection) => connection,
                    Woke::Nothing => continue,
                    Woke::Stop => return,
                };
                // **Only this user's connections are read** (FD-6, ADR 0068 §5). The socket is
                // `0600` in a `0700` directory, which keeps every other user out already; this
                // still holds if either is ever wrong. A peer of another uid, or one the socket
                // will not name, is closed unread, before the chat's token is even looked at.
                // Its pid too, which a line is checked against (D-1407-6).
                let peer = purlis_same_user::peer_process_of(&connection);
                let pid = peer.as_ref().ok().map(|(_, pid)| *pid);
                if let Err(refused) = purlis_same_user::admit_peer(peer.map(|(uid, _)| uid), owner)
                {
                    if let Some(also) = PEER_REFUSALS.say() {
                        tracing::warn!(
                            "purlis: the hook channel refused a connection: {refused}{also}"
                        );
                    }
                    continue;
                }
                // **Each connection is served on a thread of its own, from a bounded pool
                // (#991).** This used to be one thread for all of them, and an independent
                // review reproduced the consequence: anything that connects and does not
                // write — `nc -U` on the socket, a hook stopped in a debugger, a child that
                // inherited the descriptor — held the single reader inside `read_line`
                // forever. Every other session's report queued behind it, the sidebar went
                // quiet with nothing to say why, and quitting deadlocked in this thread's
                // `join`. Then it was a thread per connection with no bound, so a process
                // that connected in a loop cost a thread each.
                //
                // Now one process is served only its share at once, and the channel only so
                // many in all. A connection past either is answered and closed here, unread
                // and without a turn, so it neither waits nor holds up the ones after it.
                let slot = match serving.admit(pid) {
                    Ok(slot) => slot,
                    Err(busy) => {
                        if let Some(also) = BUSY_REFUSALS.say() {
                            tracing::warn!(
                                "purlis: the hook channel turned a connection away: {busy}{also}"
                            );
                        }
                        turn_away(connection, busy);
                        continue;
                    }
                };
                let hearing = std::sync::Arc::clone(&hearing);
                let tokens = std::sync::Arc::clone(&tokens);
                let serving_turns = std::sync::Arc::clone(&turns);
                dealt += 1;
                let this = dealt;
                let started = serving.run(
                    slot,
                    Box::new(move || {
                        serve(connection, pid, this, &tokens, &hearing, &serving_turns);
                    }),
                );
                // A thread that will not start costs this one report. Refusing the rest of
                // the channel over it would cost every report after it too, and so would a
                // turn left waiting for a connection nobody serves.
                if started.is_err() {
                    turns.done(this);
                }
            }
        });
        Reading {
            stopping,
            path,
            file,
            waking: Some(waking),
            reading: Some(reading),
        }
    }
}

/// What woke a listener's reading thread.
#[cfg(unix)]
enum Woke {
    /// A connection, to read.
    Connection(std::os::unix::net::UnixStream),
    /// Its [`Reading`] was dropped.
    Stop,
    /// Neither: a connection that went before it was taken, or a wait that was interrupted.
    Nothing,
}

/// Waits for a connection on `socket` or a word on `woken`, whichever comes first.
///
/// The socket is non-blocking (`Listener::bind` set it), so an `accept` after the wait never
/// blocks: a connection that is gone by then is `Nothing`, and the wait begins again.
#[cfg(unix)]
fn next_connection(
    socket: &std::os::unix::net::UnixListener,
    woken: &std::os::unix::net::UnixStream,
) -> Woke {
    use rustix::event::{PollFd, PollFlags, poll};
    let mut waits = [
        PollFd::new(socket, PollFlags::IN),
        PollFd::new(woken, PollFlags::IN),
    ];
    match poll(&mut waits, None) {
        Ok(_) => {}
        Err(rustix::io::Errno::INTR) => return Woke::Nothing,
        // Nothing this thread passes makes `poll` fail, and memory short for a moment is not
        // the end of the channel: wait a little, so a failure that lasts is not a busy loop.
        Err(_) => {
            std::thread::sleep(std::time::Duration::from_millis(10));
            return Woke::Nothing;
        }
    }
    // Any word on the pair is the stop: a byte, the other end closed, or an error on it.
    if !waits[1].revents().is_empty() {
        return Woke::Stop;
    }
    match socket.accept() {
        // macOS hands an accepted socket its listener's `O_NONBLOCK`, and Linux does not. A
        // connection is read blocking, as it always was, on both.
        Ok((connection, _)) => match connection.set_nonblocking(false) {
            Ok(()) => Woke::Connection(connection),
            Err(_) => Woke::Nothing,
        },
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Woke::Nothing,
        // Out of descriptors, most likely: the socket stays readable and `poll` answers at
        // once, so wait a little rather than spin, as for a failed `poll`.
        Err(_) => {
            std::thread::sleep(std::time::Duration::from_millis(10));
            Woke::Nothing
        }
    }
}

/// Held across a bind's removal of the file at its path, the bind and the read of what it made,
/// and across a stopping listener's check of its file and its removal. Without it, a project
/// opened again on another thread could bind its file between a stop's check and its removal,
/// and lose it. Another process is kept out by the single-instance lock, not by this.
#[cfg(unix)]
static SOCKET_FILES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Which file a listener made: its device and inode, read just after `bind`.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundFile {
    dev: u64,
    ino: u64,
}

#[cfg(unix)]
impl BoundFile {
    fn of(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
}

/// What is at a listener's path when it stops.
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
enum AtItsPath {
    /// The file it made: it is the listener's to remove.
    Its,
    /// Nothing: the file was removed while it listened.
    Removed,
    /// Another file: another listener was bound at the path since.
    Another,
    /// What is there could not be read, so it is left as it is.
    Unread,
}

/// Is the file at `path` still the one `bound` names?
///
/// Read without following a link, so a link put at the path is `Another`. The caller holds
/// [`SOCKET_FILES`] from this read to its removal, so no listener in this process binds the
/// path in between. An inode is dealt again once its file is gone, so a file removed and made
/// again by something else in the same instant could read as `Its`; nothing in purlis does.
#[cfg(unix)]
fn what_is_at(path: &std::path::Path, bound: BoundFile) -> AtItsPath {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if BoundFile::of(&metadata) == bound => AtItsPath::Its,
        Ok(_) => AtItsPath::Another,
        Err(err) if err.kind() == io::ErrorKind::NotFound => AtItsPath::Removed,
        Err(_) => AtItsPath::Unread,
    }
}

/// Makes `directory`, and makes sure nobody else on the machine may enter it.
///
/// **The directory, not only the socket, and the mode is set as it is CREATED.** Between a
/// `bind` and a `chmod` there is a window where the socket sits at whatever the umask
/// allowed, and the socket's own path is predictable — on Linux the fallback lives in `/tmp`,
/// which everyone can write to. A local user who got there first would own the channel: they
/// could read every report and inject their own, moving chats and raising notifications.
///
/// A review found the earlier version, which created the directory at the umask and then
/// DISCARDED the result of tightening it. Every step here is checked, and a path that is
/// already something else — a symlink, a file, a directory somebody else owns — is refused
/// rather than used.
///
/// The checks are `purlis_same_user::private_directory`'s, the same ones the credentials of
/// `charterd`'s client scopes sit behind (FD-6), so the two cannot drift: the directory is
/// checked and tightened through one descriptor opened without following a link, and one
/// another uid owns is refused before anything about it is changed.
#[cfg(unix)]
fn private_directory(directory: &std::path::Path) -> io::Result<()> {
    purlis_same_user::private_directory(directory)
}

/// The hook channel's refusals of a peer that is not this user, said in the log at most once
/// per [`A_REFUSAL_IS_SAID_EVERY`].
///
/// A refusal is something to know about, and a client that connects in a loop must not be
/// able to fill the log with them, so the ones in between are counted and the next line that is
/// said gives the count. **One limiter per kind of refusal**, so one kind cannot spend the
/// interval and hide the other: a throwaway line without a token does not keep a peer of
/// another uid out of the log, nor the reverse.
#[cfg(unix)]
static PEER_REFUSALS: RateLimited = RateLimited::new();

/// The hook channel's refusals of a line that did not carry its chat's token, limited apart
/// from [`PEER_REFUSALS`].
#[cfg(unix)]
static TOKEN_REFUSALS: RateLimited = RateLimited::new();

/// How often a refusal on the hook channel is said in the log at most.
#[cfg(unix)]
const A_REFUSAL_IS_SAID_EVERY: std::time::Duration = std::time::Duration::from_secs(10);

#[cfg(unix)]
struct RateLimited {
    state: std::sync::Mutex<(Option<std::time::Instant>, u64)>,
}

#[cfg(unix)]
impl RateLimited {
    const fn new() -> Self {
        RateLimited {
            state: std::sync::Mutex::new((None, 0)),
        }
    }

    /// `Some` with what to add to the line when this one is to be said (how many were not
    /// since the last), and `None` when it is one of the ones counted instead.
    fn say(&self) -> Option<String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = std::time::Instant::now();
        let (last, held) = &mut *state;
        if last.is_some_and(|last| now.duration_since(last) < A_REFUSAL_IS_SAID_EVERY) {
            *held += 1;
            return None;
        }
        *last = Some(now);
        let also = match std::mem::take(held) {
            0 => String::new(),
            n => format!(" ({n} more refused since the last line)"),
        };
        Some(also)
    }
}

/// How long one connection has to say its piece.
///
/// A hook writes its line and closes at once; this is only a bound on something that does
/// not. Short, because the thread holding it is doing nothing else, and generous next to the
/// 1.8 ms the whole hook call was measured at.
///
/// `cfg(unix)` because its one reader is: off unix there is no connection to bound.
#[cfg(unix)]
const A_REPORT_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(2);

/// The most one report may be.
///
/// A report is a chat number, an event word, a uuid and a pid — well under 200 bytes. The cap
/// is what stops a client that writes without ever sending a newline from growing a `String`
/// in the app's memory until there is none left: the deadline above bounds how LONG one may
/// write, and two seconds of writing is gigabytes.
///
/// `cfg(unix)` for the same reason as the deadline above it.
#[cfg(unix)]
const A_REPORT_IS_AT_MOST: u64 = 64 * 1024;

/// The most lines one connection may carry.
///
/// A hook's is one report; a handoff's is a mint and a spend. Anything past that is not
/// charter, and bounding it is what keeps one connection from holding a thread in a loop.
#[cfg(unix)]
const A_CONNECTION_SAYS_AT_MOST: usize = 4;

/// The most one line may be once asks share the socket with reports.
///
/// [`A_REPORT_IS_AT_MOST`] was sized for a report, and an open carries a whole first message:
/// up to [`crate::handoff::FIRST_MESSAGE_MAX_BYTES`] bytes, which JSON can grow sixfold where
/// the brief holds control characters (`\u001b`). This is that, with room, and it is still
/// a bound: the reason the cap exists is a client that never sends a newline.
#[cfg(unix)]
pub(crate) const A_LINE_IS_AT_MOST: u64 = 6 * crate::handoff::FIRST_MESSAGE_MAX_BYTES as u64 + 4096;

/// How long a brokered `secret exec`'s frame may take to write before its asker counts as gone.
#[cfg(unix)]
const A_FRAME_WRITE_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(60);

/// What an app with no answerer says to an ask, so an asker never waits on a silence.
pub const NOTHING_ANSWERS: &str = "this app does not open chats on request";

/// What the app answers a line it cannot read (#1333): one from a newer purlis, whose kind it
/// does not know. An asker can tell an older app from a refusal by it.
pub const UNKNOWN_LINE: &str =
    "the app could not read this line: it is from a purlis this app does not know";

/// Reads one connection to its end: each report handed to `each`, each ask answered on it.
///
/// **A report still costs exactly what it did.** A hook writes its one line and closes, so
/// the read after it sees the end at once; nothing here waits on a hook for a second line.
#[cfg(unix)]
fn serve(
    connection: std::os::unix::net::UnixStream,
    peer: Option<u32>,
    this: u64,
    tokens: &ChatTokens,
    hearing: &Hearing,
    turns: &InTurn,
) {
    let mut turn = Turn {
        turns,
        this,
        done: false,
    };
    use std::io::{BufRead, Read, Write};

    // Both directions: a client that connects and neither writes nor closes must not hold
    // this thread past the deadline.
    let _ = connection.set_read_timeout(Some(A_REPORT_TAKES_AT_MOST));
    let _ = connection.set_write_timeout(Some(A_REPORT_TAKES_AT_MOST));
    let Ok(mut writer) = connection.try_clone() else {
        return;
    };
    let deadline = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mut reader = std::io::BufReader::new(Deadlined {
        socket: connection,
        until: std::sync::Arc::clone(&deadline),
    });
    for _ in 0..A_CONNECTION_SAYS_AT_MOST {
        let mut line = String::new();
        // One deadline for the whole line, not one per read (D-1407-6): a sender dripping a
        // byte at a time would otherwise hold the line open while the pid it connected from is
        // given to another process.
        *deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(std::time::Instant::now() + A_REPORT_TAKES_AT_MOST);
        // The cap is per line: `take` on the reader would make it per connection, and a
        // brief is most of what an open carries.
        match (&mut reader).take(A_LINE_IS_AT_MOST).read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        *deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        // A line the app will not read ends this connection, never the channel, and is
        // answered with the reason first (#1333): an asker can then tell an app that refused
        // from one that never took the ask. A hook reads anything but "taken" as not taken,
        // and spools the line, as it did when nothing was written back.
        let refuse = |writer: &mut std::os::unix::net::UnixStream, why: String| {
            if let Ok(mut said) = serde_json::to_vec(&Answer::No { why }) {
                said.push(b'\n');
                let _ = writer.write_all(&said);
            }
        };
        if !line.ends_with('\n') && line.len() as u64 >= A_LINE_IS_AT_MOST {
            refuse(
                &mut writer,
                format!("the line was longer than the app reads ({A_LINE_IS_AT_MOST} bytes)"),
            );
            return;
        }
        let Some(ReadLine {
            line,
            token,
            delivery,
            armed,
        }) = read_delivered(&line)
        else {
            refuse(&mut writer, UNKNOWN_LINE.to_owned());
            return;
        };
        // Every line is checked against the token of the chat it names, and one that does not
        // carry it ends the connection unread, said so and never acted on. The chat's number
        // is said in the app's log (#647); the token is not.
        let chat = line.chat();
        match admission_of(tokens, chat, token.as_deref(), peer, &writer) {
            // A chat heard from is running: a key of its, or another's, that a busy lock kept
            // out as its token was issued is recorded now, if the lock is free (#1426).
            Admission::Admitted => tokens.record_unrecorded(),
            // **Answered and let go before it is logged** (#1870): the sender waits on this
            // connection's end, within a hook's budget, and the log's first event in a process
            // pays for building its redaction. A refusal's sender never waits on the log.
            Admission::NoToken => {
                refuse(
                    &mut writer,
                    format!("this line does not carry chat {chat}'s token, so nothing was done"),
                );
                let _ = writer.shutdown(std::net::Shutdown::Both);
                if let Some(also) = TOKEN_REFUSALS.say() {
                    tracing::warn!(
                        "purlis: a line on the hook channel for chat {chat} did not carry that \
                         chat's token, so it was dropped{also}"
                    );
                }
                return;
            }
            // The chat's own token from outside it, or for a chat whose program was never
            // confirmed: refused, and told why (D-1407-9). A hook spools nothing it is told this.
            refused @ (Admission::Outside | Admission::Unbound) => {
                if let Some(why) = refused.refusal() {
                    refuse_with(&line, why, &mut writer);
                }
                let _ = writer.shutdown(std::net::Shutdown::Both);
                tracing::warn!(
                    "purlis: a line for chat {chat} carried its token and was refused ({refused:?})"
                );
                return;
            }
        }
        // The first line waits for the connections that arrived before this one (FD-9).
        if !turn.done {
            turns.wait(this);
        }
        let first = !turn.done;
        // A report, a tool call and a refused commit are answered once their hearer has
        // returned, which in the app is once the line is recorded (FD-30): the hook answers its
        // harness only after this, or after it has spooled the line.
        let mut recorded = None;
        // A line its hook may also spool, if this is slower than it waits (#952): recorded by
        // whichever of this and the drain takes it first, and by the other not again. Its
        // hearer runs either way, for what the line moves.
        let content = match &line {
            // Each without what its spooled copy leaves out: a report its prompt's marks, a
            // call its digest (#977).
            Line::Report(report) => Some(content_of(&report.at_rest())),
            Line::Tool(call) => Some(content_of(&call.at_rest())),
            Line::Refused(refused) => Some(content_of(refused)),
            _ => None,
        };
        let begun = match (&delivery, &content) {
            (Some(delivery), Some(content)) => {
                Some(deliveries().begin(chat, delivery, content, A_LIVE_LINE_IS_WAITED_FOR_AT_MOST))
            }
            _ => None,
        };
        let already = (begun == Some(false)).then(RecordedAlready::held);
        match line {
            Line::Report(mut report) => {
                // A mode is believed only from the chat's armed hook (#3822): one any other
                // line names reads as asking, after the line's copy was told by what it said.
                if !tokens.proves(chat, armed.as_deref()) {
                    report.mode_unproven();
                }
                recorded = Some((hearing.each)(report));
            }
            Line::Ask(ask) => {
                // An ask is answered for as long as opening a chat takes, and nothing about
                // the order of hook calls hangs on it: the turn is let go before it is.
                turn.finish();
                // Known by its number while it is answered, so an answer that waits can ask
                // whether its asker is still there.
                if let Ok(asker) = writer.try_clone() {
                    tokens.answering(this, asker);
                }
                // An asker that reads [`Answer::Alive`] is told while its ask shows life (#4080).
                let pulse = match &ask {
                    Ask::Git(git) if git.alive => alive::on(&writer),
                    _ => None,
                };
                let answer = alive::within(pulse.as_ref(), || (hearing.answer)(this, ask));
                if let Some(pulse) = &pulse {
                    pulse.end();
                }
                let answer = framed::on_a_line(answer);
                tokens.answered(this);
                let Ok(mut said) = serde_json::to_vec(&answer) else {
                    return;
                };
                said.push(b'\n');
                if writer.write_all(&said).is_err() {
                    return;
                }
            }
            Line::ByHand(notice) => (hearing.noticed)(notice),
            Line::Saved(record) => (hearing.saved)(record),
            Line::Refused(refused) => recorded = Some((hearing.refused)(refused)),
            Line::Tool(call) => recorded = Some((hearing.tool)(call)),
            Line::Permission(asked) => {
                // Held for as long as the operator takes, so the turn is let go first, as an
                // ask's is, and the connection ends with its one reply.
                turn.finish();
                permission::hold(&mut reader, &mut writer, (hearing.permission)(asked));
                return;
            }
            Line::Touching(touching) => (hearing.touching)(touching),
            Line::Blocked(blocked) => (hearing.blocked)(blocked),
            Line::Doing(doing) => (hearing.doing)(doing),
            Line::Output(output) => (hearing.output)(output),
            Line::SecretExec(ask) => {
                // Held for as long as the command runs, so the turn is let go first, as an
                // ask's is. The asker's stdin may be quiet for as long as the child is, so no
                // read deadline; a write that cannot land in a minute is an asker gone.
                turn.finish();
                let _ = writer.set_read_timeout(None);
                let _ = writer.set_write_timeout(Some(A_FRAME_WRITE_TAKES_AT_MOST));
                (hearing.secret_exec)(ask, Box::new(reader), Box::new(writer));
                return;
            }
            Line::Serve(ask) => {
                // Parked for as long as the chat's server keeps it (#3316): the turn is let go
                // first, and the connection is handed on whole. A sender that wrote past its
                // line is not the shim, which waits for its answer, and is refused.
                turn.finish();
                drop(writer);
                if !reader.buffer().is_empty() {
                    serve::refused(reader.into_inner().socket, serve::NOT_ONE_LINE);
                    return;
                }
                let slot = reader.into_inner().socket;
                let _ = slot.set_read_timeout(None);
                let _ = slot.set_write_timeout(Some(A_REPORT_TAKES_AT_MOST));
                (hearing.serve)(ask, slot);
                return;
            }
            Line::Frames(hello) => {
                // Held for as long as its asks take, as one ask is: the turn is let go first.
                turn.finish();
                serve_frames(
                    Greeting {
                        hello,
                        token: token.as_deref(),
                        peer,
                        this,
                    },
                    tokens,
                    hearing,
                    reader,
                    writer,
                    &deadline,
                );
                return;
            }
        }
        // Told as taken only once it is recorded durably: a line the hearer could not record
        // gets no answer, so its hook spools it (FD-30). An older hook has closed its end
        // already, and what it was not waiting for is lost with nothing to say.
        drop(already);
        if let (Some(true), Some(delivery), Some(content), Some(recorded)) =
            (begun, &delivery, &content, &recorded)
        {
            deliveries().finish(chat, delivery, content, recorded.is_ok());
        }
        if let Some(recorded) = recorded {
            if let Err(why) = recorded {
                tracing::warn!(
                    "purlis: a line on the hook channel for chat {chat} was not recorded \
                     ({why}), so its hook was not told it was taken"
                );
            } else if let Ok(mut said) = serde_json::to_vec(&Receipt {
                taken: true,
                refused: None,
            }) {
                said.push(b'\n');
                let _ = writer.write_all(&said);
            }
        }
        if first {
            turn.finish();
        }
    }
}

/// Whether a line or a framed ask naming `chat` and carrying `token` is admitted
/// (D-1407-6, D-1407-10): the token must be the chat's, and the process that connected (`peer`)
/// and the one the socket names now must each run inside the chat, so one finished by a process
/// the first handed the connection to is judged by that process too. The processes are read
/// from the kernel only once the token has checked. The chat's number is said in the app's log
/// (#647); the token is not.
#[cfg(unix)]
fn admission_of(
    tokens: &ChatTokens,
    chat: u32,
    token: Option<&str>,
    peer: Option<u32>,
    connection: &std::os::unix::net::UnixStream,
) -> Admission {
    let now = purlis_same_user::peer_process_of(connection)
        .ok()
        .map(|(_, pid)| pid);
    // A connection handed on mid-line names its new holder on macOS.
    let senders: Vec<u32> = match (peer, now) {
        (Some(peer), Some(now)) if peer != now => vec![peer, now],
        (Some(pid), _) | (None, Some(pid)) => vec![pid],
        (None, None) => Vec::new(),
    };
    tokens.admission_by(
        chat,
        token,
        (!senders.is_empty()).then_some(senders),
        &|senders, program| {
            Seen::all(senders).is_some_and(|seen| seen.iter().all(|one| one.inside(program)))
        },
    )
}

/// How long one framed ask may take to arrive, from when it begins to be read: an ask is at
/// most a line, so this bounds only a sender dripping.
#[cfg(unix)]
const A_FRAMED_ASK_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(10);

/// The hello a framed connection was admitted on: its chat and token, the process that
/// connected, and the connection's number.
#[cfg(unix)]
struct Greeting<'a> {
    hello: framed::Hello,
    token: Option<&'a str>,
    peer: Option<u32>,
    this: u64,
}

/// A connection whose hello was admitted (#1996): accepted on one line, then served as a framed
/// stream for the hello's chat ([`framed::serve`]). Each ask is admitted again as a line is
/// ([`admission_of`], with the hello's token and the process the socket names then), and
/// answered as a line's is, by `hearing.answer` told this connection's number.
#[cfg(unix)]
fn serve_frames(
    greeting: Greeting<'_>,
    tokens: &ChatTokens,
    hearing: &Hearing,
    mut reader: std::io::BufReader<Deadlined>,
    mut writer: std::os::unix::net::UnixStream,
    deadline: &std::sync::Mutex<Option<std::time::Instant>>,
) {
    use std::io::Write;

    let Greeting {
        hello,
        token,
        peer,
        this,
    } = greeting;
    fn say(writer: &mut std::os::unix::net::UnixStream, line: &impl serde::Serialize) -> bool {
        serde_json::to_vec(line).is_ok_and(|mut said| {
            said.push(b'\n');
            writer.write_all(&said).is_ok()
        })
    }
    if hello.frames != framed::VERSION {
        say(
            &mut writer,
            &Answer::No {
                why: format!(
                    "the app speaks frames version {}, and this asked for {}",
                    framed::VERSION,
                    hello.frames
                ),
            },
        );
        return;
    }
    if !say(&mut writer, &framed::Accepted::now()) {
        return;
    }
    // An answer may be a frame at its bound, and an asker is given as long to take it as a
    // brokered command's output is.
    let _ = writer.set_write_timeout(Some(A_FRAME_WRITE_TAKES_AT_MOST));
    let Ok(connection) = writer.try_clone() else {
        return;
    };
    let chat = hello.chat;
    let mut arm = || {
        *deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(std::time::Instant::now() + A_FRAMED_ASK_TAKES_AT_MOST);
    };
    let mut admit = || {
        *deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        match admission_of(tokens, chat, token, peer, &connection) {
            Admission::Admitted => Ok(()),
            Admission::NoToken => Err(format!(
                "this ask does not carry chat {chat}'s token, so nothing was done"
            )),
            refused => {
                tracing::warn!(
                    "purlis: a framed ask for chat {chat} carried its token and was refused \
                     ({refused:?})"
                );
                Err(refused
                    .refusal()
                    .unwrap_or("this ask was refused, so nothing was done")
                    .to_owned())
            }
        }
    };
    let _ = framed::serve(
        &mut reader,
        &mut writer,
        framed::Serving {
            chat,
            asks: A_CONNECTION_SAYS_AT_MOST,
            an_ask_is_at_most: u32::try_from(A_LINE_IS_AT_MOST).unwrap_or(u32::MAX),
            arm: &mut arm,
            admit: &mut admit,
        },
        &mut |ask| {
            if let Ok(asker) = connection.try_clone() {
                tokens.answering(this, asker);
            }
            let answer = (hearing.answer)(this, ask);
            tokens.answered(this);
            framed::Reply::from(answer)
        },
    );
}

/// A connection read with one deadline for a whole line: each read is given what is left of it,
/// and none once it has passed. With no deadline set, a read waits as the socket's own timeout
/// says.
#[cfg(unix)]
pub(crate) struct Deadlined {
    socket: std::os::unix::net::UnixStream,
    until: std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>,
}

#[cfg(unix)]
impl Deadlined {
    /// The connection itself.
    pub(crate) fn socket(&self) -> &std::os::unix::net::UnixStream {
        &self.socket
    }
}

#[cfg(unix)]
impl std::io::Read for Deadlined {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let until = *self
            .until
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(until) = until {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the line took longer than a line may",
                ));
            }
            self.socket.set_read_timeout(Some(left))?;
        }
        self.socket.read(buf)
    }
}

/// The answer `why` to a line that carried its chat's token and was refused (D-1407-9): from a
/// process outside the chat, or for a chat whose program was never confirmed. In the shape its sender waits for: a report, a tool call or a refused commit is
/// told it was refused (so its hook prints why and spools nothing), an ask is answered no, a
/// permission ask is given no option, and a brokered `secret exec` is refused. A line nothing
/// waits on gets nothing.
#[cfg(unix)]
fn refuse_with(line: &Line, why: &str, writer: &mut std::os::unix::net::UnixStream) {
    use std::io::Write;
    let said = match line {
        Line::Report(_) | Line::Tool(_) | Line::Refused(_) | Line::Serve(_) => {
            serde_json::to_vec(&Receipt {
                taken: false,
                refused: Some(why.to_owned()),
            })
        }
        Line::Ask(_) | Line::Frames(_) => serde_json::to_vec(&Answer::No {
            why: why.to_owned(),
        }),
        Line::Permission(_) => serde_json::to_vec(&permission::Chosen { chosen: None }),
        Line::SecretExec(_) => serde_json::to_vec(&crate::secrets::brokered::Frame::Refused {
            why: why.to_owned(),
            code: 1,
        }),
        Line::ByHand(_)
        | Line::Saved(_)
        | Line::Touching(_)
        | Line::Blocked(_)
        | Line::Doing(_)
        | Line::Output(_) => return,
    };
    if let Ok(mut said) = said {
        said.push(b'\n');
        let _ = writer.write_all(&said);
    }
}

/// The order connections arrived in, which their first lines are handed on in (FD-9).
///
/// **A thread per connection reads lines in whatever order the threads are scheduled**, and
/// the host's event log wants the order the hooks ran in: a tool call's post hook connects only
/// after its pre hook has exited, so it should be recorded after it. Each connection is dealt
/// a number as it is accepted, and hands its first line to its hearer only once every earlier
/// connection's hearer has returned, which in the app is once that line is recorded.
///
/// **Best effort: ordered unless a recording takes longer than [`A_TURN_IS_WAITED_AT_MOST`].**
/// A connection that says nothing, or a hearer that is slow, holds the ones after it for that
/// long at most, so the channel is never held hostage by one; past it, lines can be recorded
/// out of order, and the event log's pairing of a tool call's two ends does not depend on the
/// order (`eventlog::Recorder::tool`). An ask lets its turn go before it is answered.
#[cfg(unix)]
struct InTurn {
    /// The lowest number not yet done, and the numbers above it that are.
    state: std::sync::Mutex<(u64, std::collections::BTreeSet<u64>)>,
    turned: std::sync::Condvar,
    /// How long a line waits for its turn: [`A_TURN_IS_WAITED_AT_MOST`] on the channel.
    at_most: std::time::Duration,
}

#[cfg(unix)]
impl InTurn {
    fn new() -> Self {
        Self::waiting_at_most(A_TURN_IS_WAITED_AT_MOST)
    }

    fn waiting_at_most(at_most: std::time::Duration) -> Self {
        Self {
            state: std::sync::Mutex::new((1, std::collections::BTreeSet::new())),
            turned: std::sync::Condvar::new(),
            at_most,
        }
    }

    /// Waits until every connection before `this` is done, or the wait has gone on too long.
    fn wait(&self, this: u64) {
        let until = std::time::Instant::now() + self.at_most;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while state.0 < this {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return;
            }
            state = self
                .turned
                .wait_timeout(state, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// Marks `this` done: its first line is handed on, or it never had one.
    fn done(&self, this: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1.insert(this);
        loop {
            let next = state.0;
            if !state.1.remove(&next) {
                break;
            }
            state.0 += 1;
        }
        drop(state);
        self.turned.notify_all();
    }
}

/// Marks a connection done however its serving ends.
#[cfg(unix)]
struct Turn<'a> {
    turns: &'a InTurn,
    this: u64,
    done: bool,
}

#[cfg(unix)]
impl Turn<'_> {
    fn finish(&mut self) {
        if !self.done {
            self.done = true;
            self.turns.done(self.this);
        }
    }
}

#[cfg(unix)]
impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.finish();
    }
}

/// How long a line waits for the lines whose connections arrived before it.
#[cfg(unix)]
const A_TURN_IS_WAITED_AT_MOST: std::time::Duration = std::time::Duration::from_millis(50);

/// How many connections from one process the hook channel serves at once (#991).
///
/// A hook is a process of its own that opens one connection, and a handoff's mint and spend
/// share one, so a process with more than a few open at once is a long-lived one asking
/// several things in parallel, or something that is not purlis. Past this, a process's next
/// connection is refused at once, so one process cannot take every place in the pool.
#[cfg(unix)]
const A_PEER_IS_SERVED_AT_MOST: usize = 32;

/// How many connections the hook channel serves at once from every process together, and so
/// how many threads serve them at most (#991).
///
/// Fifty live chats is the product's scale. Each may hold an ask or a permission prompt open
/// on the operator and a brokered command on its connection while its hooks write, so this is
/// room for that with margin, and still a bound: a connection past it is refused at once.
#[cfg(unix)]
const THE_CHANNEL_SERVES_AT_MOST: usize = 128;

/// How long a serving thread with nothing to serve waits for the next connection before it
/// ends. A quiet channel holds no threads; a busy one reuses the ones it has.
#[cfg(unix)]
const A_SERVER_IDLES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(30);

/// The hook channel's refusals of a connection past [`Serving`]'s bounds, limited apart from
/// the other refusals.
#[cfg(unix)]
static BUSY_REFUSALS: RateLimited = RateLimited::new();

/// Why [`Serving`] turned a connection away.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Busy {
    /// Its process already has as many connections served as one may.
    Peer,
    /// The channel serves as many as it serves at once.
    Channel,
}

#[cfg(unix)]
impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Busy::Peer => write!(
                f,
                "the app is already serving as many connections from this process as it \
                 serves from one ({A_PEER_IS_SERVED_AT_MOST}), so this one was not read; \
                 try again once one of them ends"
            ),
            Busy::Channel => write!(
                f,
                "the app is already serving as many connections as it serves at once \
                 ({THE_CHANNEL_SERVES_AT_MOST}), so this one was not read; try again in a \
                 moment"
            ),
        }
    }
}

/// The pool that serves the hook channel's connections (#991): at most `at_most` at once, of
/// which at most `per_peer` from one process, on at most `at_most` threads that are reused
/// from one connection to the next.
///
/// **It replaced a thread per connection.** That kept a silent client from freezing the
/// channel, but nothing bounded how many a client could open: a process that connected in a
/// loop cost a thread each, and every other chat's hooks waited behind it. A connection is
/// admitted before it is read, by the process the socket names, and one past either bound is
/// answered and closed at once by the accepting thread, never queued.
#[cfg(unix)]
struct Serving {
    state: std::sync::Mutex<Served>,
    work: std::sync::Condvar,
    per_peer: usize,
    at_most: usize,
}

#[cfg(unix)]
#[derive(Default)]
struct Served {
    /// Admitted connections no thread has taken yet.
    waiting: std::collections::VecDeque<Box<dyn FnOnce() + Send>>,
    /// Threads waiting for a connection, and threads alive.
    idle: usize,
    threads: usize,
    /// Admitted connections not yet done, in all and by the process they came from. A peer the
    /// socket would not name shares one count, which only refuses more.
    admitted: usize,
    by_peer: std::collections::HashMap<Option<u32>, usize>,
    /// The channel has stopped: a thread with nothing left to serve ends.
    closed: bool,
}

/// A place in [`Serving`], given back however its connection's serving ends.
#[cfg(unix)]
struct Slot {
    serving: std::sync::Arc<Serving>,
    peer: Option<u32>,
}

#[cfg(unix)]
impl Drop for Slot {
    fn drop(&mut self) {
        let mut state = self.serving.lock();
        state.admitted = state.admitted.saturating_sub(1);
        if let Some(held) = state.by_peer.get_mut(&self.peer) {
            *held = held.saturating_sub(1);
            if *held == 0 {
                state.by_peer.remove(&self.peer);
            }
        }
    }
}

#[cfg(unix)]
impl Serving {
    fn new(per_peer: usize, at_most: usize) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            state: std::sync::Mutex::new(Served::default()),
            work: std::sync::Condvar::new(),
            per_peer,
            at_most,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Served> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A place for one more connection from `peer`, or why there is none. Never waits.
    fn admit(self: &std::sync::Arc<Self>, peer: Option<u32>) -> Result<Slot, Busy> {
        let mut state = self.lock();
        let held = state.by_peer.get(&peer).copied().unwrap_or(0);
        if held >= self.per_peer {
            return Err(Busy::Peer);
        }
        if state.admitted >= self.at_most {
            return Err(Busy::Channel);
        }
        state.admitted += 1;
        *state.by_peer.entry(peer).or_insert(0) += 1;
        Ok(Slot {
            serving: std::sync::Arc::clone(self),
            peer,
        })
    }

    /// Serves `job` in `slot`'s place on a thread of the pool, starting one if none is free.
    /// An error is a thread that would not start: `job` is dropped unrun.
    fn run(
        self: &std::sync::Arc<Self>,
        slot: Slot,
        job: Box<dyn FnOnce() + Send>,
    ) -> io::Result<()> {
        let mut state = self.lock();
        state.waiting.push_back(Box::new(move || {
            let _slot = slot;
            job();
        }));
        // Every admitted connection has a thread, so none waits behind a held one: the
        // admitted count is bounded by `at_most`, and so is this.
        if state.waiting.len() > state.idle && state.threads < self.at_most {
            let pool = std::sync::Arc::clone(self);
            let started = std::thread::Builder::new()
                .name("purlis-hook-serving".into())
                .spawn(move || pool.serve_until_idle());
            match started {
                Ok(_) => state.threads += 1,
                Err(err) if state.idle == 0 => {
                    // Nothing would take it before a held connection ends, which may be
                    // never: it is dropped, closing its connection, rather than left waiting.
                    // Dropped once the lock is let go, since its place gives itself back.
                    let unserved = state.waiting.pop_back();
                    drop(state);
                    drop(unserved);
                    return Err(err);
                }
                Err(_) => {}
            }
        }
        drop(state);
        self.work.notify_one();
        Ok(())
    }

    /// One thread of the pool: serves what is waiting until nothing has come for a while, or
    /// the channel has stopped.
    fn serve_until_idle(&self) {
        loop {
            let job = {
                let mut state = self.lock();
                loop {
                    if let Some(job) = state.waiting.pop_front() {
                        break job;
                    }
                    if state.closed {
                        state.threads -= 1;
                        return;
                    }
                    state.idle += 1;
                    let (again, waited) = self
                        .work
                        .wait_timeout(state, A_SERVER_IDLES_AT_MOST)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state = again;
                    state.idle -= 1;
                    if waited.timed_out() && state.waiting.is_empty() {
                        state.threads -= 1;
                        return;
                    }
                }
            };
            // A connection whose serving panics costs that connection, never the thread
            // serving the ones after it; its place is given back as the job unwinds.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        }
    }

    /// The channel has stopped: idle threads end now, and busy ones once they are done.
    fn close(&self) {
        self.lock().closed = true;
        self.work.notify_all();
    }
}

/// Closes a [`Serving`] when the thread that accepts for it returns, however it does.
#[cfg(unix)]
struct Closing(std::sync::Arc<Serving>);

#[cfg(unix)]
impl Drop for Closing {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Answers a connection [`Serving`] turned away, unread, and closes it.
///
/// Answered as an ask the app will not take is, so an asker shows why, and a hook reads it as
/// not taken and spools its line for the next drain. Written without waiting: the socket's
/// buffer is empty, and a write that would block is given up, since the close says enough.
#[cfg(unix)]
fn turn_away(connection: std::os::unix::net::UnixStream, busy: Busy) {
    use std::io::Write;
    if let Ok(mut said) = serde_json::to_vec(&Answer::No {
        why: busy.to_string(),
    }) {
        said.push(b'\n');
        if connection.set_nonblocking(true).is_ok() {
            let _ = (&connection).write_all(&said);
        }
    }
}

/// A listener being read on its own thread. Dropping it stops the reading.
#[cfg(unix)]
pub struct Reading {
    stopping: std::sync::Arc<std::sync::atomic::AtomicBool>,
    path: std::path::PathBuf,
    file: BoundFile,
    waking: Option<std::os::unix::net::UnixStream>,
    reading: Option<std::thread::JoinHandle<()>>,
}

/// How long a dropped [`Reading`] waits for its thread to be woken and go.
#[cfg(unix)]
const A_READING_STOPS_WITHIN: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(unix)]
impl Drop for Reading {
    fn drop(&mut self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // The thread waits on its socket and on the other end of this pair. A byte, and the
        // end closed, wake it whatever is at the path now: another listener bound there, or
        // the file removed.
        if let Some(waking) = self.waking.take() {
            use std::io::Write;
            let _ = (&waking).write_all(&[0]);
        }
        if let Some(reading) = self.reading.take() {
            // **The wait is still bounded.** Nothing at the path can keep the thread now, so
            // this is only so that nothing that closes a project can be held for good by it.
            let began = std::time::Instant::now();
            while !reading.is_finished() && began.elapsed() < A_READING_STOPS_WITHIN {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            if reading.is_finished() {
                let _ = reading.join();
            } else {
                tracing::warn!(
                    "purlis: the hook channel at {} did not stop within {} seconds of being \
                     closed. Its thread is left",
                    self.path.display(),
                    A_READING_STOPS_WITHIN.as_secs()
                );
            }
        }
        // The file goes only while it is the one this listener made. Removing whatever is at
        // the path took a newer listener's file, bound there in the same instant.
        let _files = SOCKET_FILES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match what_is_at(&self.path, self.file) {
            AtItsPath::Its => {
                let _ = std::fs::remove_file(&self.path);
            }
            AtItsPath::Removed => tracing::warn!(
                "purlis: the hook channel at {} had been removed while the app listened on \
                 it, so no hook reached the app after that",
                self.path.display()
            ),
            AtItsPath::Another => tracing::warn!(
                "purlis: the hook channel at {} was left in place when it closed: another \
                 listener was bound at its path since",
                self.path.display()
            ),
            AtItsPath::Unread => tracing::warn!(
                "purlis: the hook channel at {} was left in place when it closed: what is \
                 at its path could not be read",
                self.path.display()
            ),
        }
    }
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, unix))]
#[path = "hookwire/delivery_tests.rs"]
mod delivery_tests;

#[cfg(test)]
#[path = "hookwire/prompt_marks_tests.rs"]
mod prompt_marks_tests;

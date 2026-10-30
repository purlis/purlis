// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The tools a chat is offered through charter's MCP server (HP-7, Q15): the project's todos,
//! memory, session records and change status, and a question for the operator. Skills stay for
//! prose guidance; these carry the actions, the same way on every harness.
//!
//! This module is the tools themselves, with no protocol in it: [`TOOLS`] says what each takes
//! and [`call`] does it. `charter mcp` (the CLI) serves them over stdio with `rmcp`, and each
//! harness adapter hands a chat that server for the chat alone ([`crate::harness::HarnessAdapter`],
//! FD-13): Claude Code on `--mcp-config`, Codex on `-c mcp_servers.charter=…`, opencode in the
//! config its shim already rides in, an ACP agent in `session/new`. Under every one the server
//! is handed the chat's hook socket and token, as the chat's hooks are ([`CONNECTION_ENV`]), so
//! a tool the app answers works the same under each.
//!
//! **The boundary is the tool set, because the server runs where the harness runs its MCP
//! servers.** Codex and Claude Code start an MCP server outside the sandbox they put a command
//! in, as they do charter's hooks, so what a tool may do is decided here and nowhere else:
//!
//! - **The chat's own place, and nothing it names.** Every call acts on the workspace (or the
//!   project root) a `charter` command run in the chat would act on with no `-w`, resolved by
//!   the server at each call ([`crate::active`]). No tool takes a workspace, a project, a
//!   path or a directory, and a test holds that. A workspace that is not there is refused,
//!   never created.
//! - **Names, never paths.** A todo is closed by its slug and a record read by its file name,
//!   each checked by the store that owns it, which also refuses a link out of the project.
//! - **No credential, and no human power.** No tool reads a vault, the environment, the audit
//!   or a forge: `change_status` is the record half of `charter change show`, because the forge
//!   half signs in as the person (ADR 0077). `ask_operator` asks through the harness, which
//!   shows the question to the person; nothing here answers it.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::active::Place;

/// The server's name in every harness's config, and so the prefix a harness gives its tools
/// (`mcp__purlis__todo_add` in Claude Code, [`crate::names::MCP_TOOL_PREFIX`]). It is the
/// plugin's name ([`crate::names::PLUGIN_NAME`]), renamed with it (#1266).
pub const SERVER: &str = crate::names::PLUGIN_NAME.write;

/// The `charter` subcommand that serves the tools over stdio.
pub const SUBCOMMAND: &str = "mcp";

/// The one tool [`call`] does not answer: only the server, which holds the connection to the
/// harness, can ask the person through it.
pub const ASK_OPERATOR: &str = "ask_operator";

/// The tool that writes this chat's session record, which [`call`] does not answer either: the
/// server hands it to the app that started the chat (a brokered write, ADR 0067 §2, #1332), as
/// `purlis session record` run in the chat does, and only where no app takes it writes it
/// itself. One operation behind both, in the CLI that serves the tools.
pub const SESSION_RECORD: &str = "session_record";

/// The tool that writes one memory of this chat's persona, or of shared memory, which [`call`]
/// does not answer either: like [`SESSION_RECORD`] it is a brokered write the server hands to
/// the app (#1333), the one operation `purlis persona remember` run in the chat performs.
pub const PERSONA_REMEMBER: &str = "persona_remember";

/// The tool that sets the `## Vision` of this chat's workspace, which [`call`] does not answer
/// either: a brokered write the server hands to the app (#1384), the one operation `purlis
/// workspace vision "<text>"` run in the chat performs.
pub const WORKSPACE_VISION: &str = "workspace_vision";

/// The tool that adds one entry to the decisions or the glossary section of this chat's
/// workspace's `workspace.md`, handed to the app as [`WORKSPACE_VISION`] is (#1384).
pub const WORKSPACE_SECTION: &str = "workspace_section";

/// The tool that says where this chat is working: who asked for it, its sibling tasks and the
/// other chats running as its persona (#1450). [`call`] does not answer it: only the app that
/// started the chat knows, so the server asks it over the chat's hook socket, as
/// `purlis persona where` run in the chat does.
pub const PERSONA_WHERE: &str = "persona_where";
/// The tool that dispatches a task to a persona (#1436), which [`call`] does not answer either:
/// the server hands it to the app that started the chat, the one operation `purlis dispatch`
/// run in the chat performs. Only the app starts a chat, so with no app it starts nothing.
pub const DISPATCH: &str = "dispatch";

/// The tool that sends a task's one report back to the chat that dispatched it (#1436): the
/// one operation `purlis dispatch report` performs, handed to the app as [`DISPATCH`] is.
pub const DISPATCH_REPORT: &str = "dispatch_report";

/// The tool that lists the tasks this chat dispatched (#1441): the one operation `purlis
/// dispatch list` performs, answered by the app as [`DISPATCH`] is.
pub const DISPATCH_LIST: &str = "dispatch_list";

/// The tool that reads what a task this chat dispatched said last and the commands it ran
/// (#1981, #2084): the one operation `purlis dispatch read` performs, answered by the app as
/// [`DISPATCH`] is.
pub const DISPATCH_READ: &str = "dispatch_read";

/// The tool that waits for the report of a task this chat dispatched (#3786): the one
/// operation `purlis dispatch wait` performs.
pub const DISPATCH_WAIT: &str = "dispatch_wait";

/// The tool that sends a follow-up to a task this chat dispatched (#3786): `purlis dispatch
/// tell`.
pub const DISPATCH_TELL: &str = "dispatch_tell";

/// The tool that answers the question a task this chat dispatched asked it (#3786): `purlis
/// dispatch answer`.
pub const DISPATCH_ANSWER: &str = "dispatch_answer";

/// The tool that cancels a task this chat dispatched (#3786): `purlis dispatch cancel`.
pub const DISPATCH_CANCEL: &str = "dispatch_cancel";

/// The tool that closes a finished task this chat dispatched (#3786): `purlis dispatch close`.
pub const DISPATCH_CLOSE: &str = "dispatch_close";

/// The tool a task sends its asking chat a progress note with (#3786): `purlis dispatch note`.
pub const DISPATCH_NOTE: &str = "dispatch_note";

/// The tool a task asks its asking chat a question with, and waits for the answer (#3786):
/// `purlis dispatch ask`.
pub const DISPATCH_ASK: &str = "dispatch_ask";

/// The tool that brings the clone or branch folder this chat stands in up to its remote
/// (#3831): the one operation `purlis worktree update` run there performs, which [`call`] does
/// not answer either. The server hands it to the app that started the chat, which runs it
/// outside the chat's sandbox after judging it; with no app, it runs nothing.
pub const WORKTREE_UPDATE: &str = "worktree_update";

/// What [`WORKTREE_UPDATE`] is asked to do: `start` (the default), `continue` or `abort`, and
/// for a branch folder's start, the `base` branch.
pub fn worktree_update_args(
    args: &Map<String, Value>,
) -> Result<crate::hookwire::UpdateStep, String> {
    use crate::hookwire::UpdateStep;
    let base = match args.get("base") {
        None | Some(Value::Null) => None,
        Some(Value::String(base)) if !base.trim().is_empty() => Some(base.trim().to_owned()),
        Some(_) => return Err("base is the name of a branch".to_owned()),
    };
    let step = match args.get("step") {
        None | Some(Value::Null) => "start",
        Some(Value::String(step)) => step.as_str(),
        Some(_) => return Err("step is start, continue or abort".to_owned()),
    };
    match (step, base) {
        ("start", base) => Ok(UpdateStep::Start { base }),
        ("continue" | "abort", Some(_)) => {
            Err("base is taken only when an update starts".to_owned())
        }
        ("continue", None) => Ok(UpdateStep::Continue),
        ("abort", None) => Ok(UpdateStep::Abort),
        _ => Err("step is start, continue or abort".to_owned()),
    }
}

/// The tools a Claude Code chat runs without asking (V79, #1050, amending SI-8e in ADR 0064):
/// the five that only read, [`PERSONA_WHERE`] (V98a, #1450), which reads the app's own
/// record and answers names and states a chat is told at its start anyway, and
/// [`DISPATCH_LIST`] (#1463), which lists the tasks the asking chat dispatched itself, or the
/// person started from its tab, and no other chat's: the same answer `purlis dispatch list`
/// gives, whose command line is already allowed ([`crate::harness::claude::DISPATCH_TASK_ALLOW`]);
/// and [`DISPATCH_READ`] (#2084), which reads one of those tasks alone, the same answer
/// `purlis dispatch read` gives, also allowed by its command line; and
/// [`crate::themepreview::TOOL`] (#2330), which returns only pictures of the theme specimen's
/// sample content (ADR 0064, amended 2026-10-17); and the four skill tools
/// ([`crate::skilltools`], SK-13), which read the chat's own skill set and its runs, or load one
/// of its skills (ADR 0063 as amended 2026-10-20): `skill_load` writes nothing but the start of
/// the run it opens, in the plane's skill log; and [`DISPATCH_WAIT`] (#3786), which waits on
/// one of the chat's own tasks and returns its report, as `purlis dispatch wait` does.
/// Named one by one, never derived from [`Tool::read_only`]: `ask_operator` is marked
/// read-only too and still asks, and a tool added later is asked about until someone rules it
/// in here. Writes are never in this list.
pub const PRE_ALLOWED: [&str; 14] = [
    "todo_list",
    "memory_search",
    "session_record_list",
    "session_record_read",
    "change_status",
    PERSONA_WHERE,
    DISPATCH_LIST,
    DISPATCH_READ,
    DISPATCH_WAIT,
    crate::themepreview::TOOL,
    crate::skilltools::SKILL_LIST,
    crate::skilltools::SKILL_RUNS,
    crate::skilltools::SKILL_FIND,
    crate::skilltools::SKILL_LOAD,
];

/// The tools a Claude Code chat runs without asking that **write only the chat's own scope**
/// (#3829, the operator's ruling of 2026-10-29, amending ADR 0064): its workspace's todos and
/// memory, its persona's memory (`--shared` included, as ruled), and its workspace's
/// `workspace.md`; and [`ASK_OPERATOR`], whose consent is the person's own answer, so the
/// harness's prompt in front of it asked the person twice. **The person is asked only when
/// something new reaches someone new**, and none of these reaches anyone new.
///
/// Apart from [`PRE_ALLOWED`] on purpose: a grade's chat is served the reads alone (SK-16), and
/// is handed none of these. Named one by one, so a tool added later is asked about until it is
/// ruled in here; an operator's or a project's `ask` or `deny` for any of them still wins.
pub const OWN_SCOPE_TOOLS: [&str; 8] = [
    "todo_add",
    "todo_done",
    "memory_add",
    PERSONA_REMEMBER,
    WORKSPACE_SECTION,
    WORKSPACE_VISION,
    ASK_OPERATOR,
    // The update the app makes for the chat in its own clone, branch folder or root (#3831).
    WORKTREE_UPDATE,
];

/// The variables the server reads to find the chat's place, as a `charter` command in the chat
/// reads them. A harness that hands an MCP server only the variables it is told to (Codex, an
/// ACP agent) is told these, and [`CONNECTION_ENV`].
///
/// Under their purlis names only: the server is this product's own binary, which reads either.
pub const SCOPE_ENV: [&str; 5] = [
    "PURLIS_ROOT",
    crate::active::WORKSPACE_ENV,
    crate::active::PLANE_ROOT_ENV,
    crate::active::PERSONA_ENV,
    crate::active::SESSION_ID_ENV,
];

/// The variables that connect the server to the app that started the chat, exactly as they
/// connect the chat's hooks (#1996): the hook socket, the chat's number, the chat's own token
/// and its harness. The token is the one the hooks carry, minted at the chat's `exec`; nothing
/// is minted for the server, and it is checked as every line of the chat's is.
///
/// **They reach the server the way they reach the hooks: in its environment, inherited from the
/// harness.** Claude Code and opencode hand a stdio server their whole environment; Codex hands
/// on the variables named in `env_vars` ([`codex_flag`]), which names them and holds no value;
/// an ACP agent is handed them in `session/new` over its own stdio ([`acp_server`]), in an
/// environment it already holds them in. None of them is on a command line or in a file.
pub const CONNECTION_ENV: [&str; 4] = [
    crate::hookwire::SOCKET_ENV,
    crate::hookwire::CHAT_ENV,
    crate::hookwire::TOKEN_ENV,
    crate::hookwire::HARNESS_ENV,
];

/// Every variable the server is handed by a harness that hands it only those it is told to.
fn handed_env() -> impl Iterator<Item = &'static str> {
    SCOPE_ENV.into_iter().chain(CONNECTION_ENV)
}

/// How long Codex waits on one of these tools, in seconds. Its own default is 60, and
/// `ask_operator` waits on a person; an hour is a question left for lunch, not one forgotten.
pub const CODEX_TOOL_TIMEOUT_SECS: i64 = 3600;

/// The server as Claude Code's `--mcp-config` takes it: JSON, on the argument, for that session
/// alone. Claude Code hands a stdio server its own environment and directory (measured on
/// 2.1.288), so the chat's place reaches it with nothing named.
pub fn claude_code_config(binary: &Path) -> String {
    claude_code_config_as(binary, false)
}

/// The argument a grade's chat starts the server with (SK-16, the operator's ruling of
/// 2026-10-26): it lists and answers the tools that only read ([`PRE_ALLOWED`]), and refuses
/// every other.
pub const GRADED_ARG: &str = "--graded";

/// [`claude_code_config`], for a grade's chat with `graded`: the server started with
/// [`GRADED_ARG`].
pub fn claude_code_config_as(binary: &Path, graded: bool) -> String {
    let args: Vec<&str> = if graded {
        vec![SUBCOMMAND, GRADED_ARG]
    } else {
        vec![SUBCOMMAND]
    };
    json!({
        "mcpServers": {
            SERVER: {
                "type": "stdio",
                "command": binary.display().to_string(),
                "args": args,
            }
        }
    })
    .to_string()
}

/// The server as Codex's `-c` takes it: `mcp_servers.charter=<inline table>`, beside the
/// operator's own servers (measured on codex-cli 0.147.0 with `codex mcp get`). Codex hands a
/// stdio server only a few variables of its own, so the ones that say where the chat works
/// and how it reaches the app are named in `env_vars` ([`SCOPE_ENV`], [`CONNECTION_ENV`]), by
/// name only: Codex reads each value from its own environment.
pub fn codex_flag(binary: &Path) -> String {
    let table: toml::Table = [
        (
            "command".to_owned(),
            toml::Value::from(binary.display().to_string()),
        ),
        (
            "args".to_owned(),
            toml::Value::Array(vec![toml::Value::from(SUBCOMMAND)]),
        ),
        (
            "env_vars".to_owned(),
            toml::Value::Array(handed_env().map(toml::Value::from).collect()),
        ),
        (
            "tool_timeout_sec".to_owned(),
            toml::Value::from(CODEX_TOOL_TIMEOUT_SECS),
        ),
    ]
    .into_iter()
    .collect();
    format!("mcp_servers.{SERVER}={}", toml::Value::Table(table))
}

/// The server as ACP's `session/new` takes it in `mcpServers` (ADR 0080 §1): a stdio server, run
/// by the agent, handed the [`SCOPE_ENV`] and [`CONNECTION_ENV`] variables `chat_env` sets, each
/// once with its last value, and nothing else of the chat's environment.
pub fn acp_server(
    binary: &Path,
    chat_env: &[(std::ffi::OsString, std::ffi::OsString)],
) -> agent_client_protocol::schema::v1::McpServer {
    use agent_client_protocol::schema::v1::{EnvVariable, McpServer, McpServerStdio};
    // Each name once, in `SCOPE_ENV`'s then `CONNECTION_ENV`'s order, with its last value: the
    // chat's own comes after what the app passed on, and an agent may read a repeated name
    // either way. A value that is not UTF-8 cannot be said in JSON, and is left out rather than
    // mangled.
    let env = handed_env()
        .filter_map(|wanted| {
            let value = chat_env
                .iter()
                .rev()
                // Under either of its names (V93k); the server is handed the purlis one.
                .find(|(name, _)| {
                    name.to_str()
                        .is_some_and(|name| crate::envvar::same(name, wanted))
                })?
                .1
                .to_str()?;
            Some(EnvVariable::new(wanted, value))
        })
        .collect();
    McpServer::Stdio(
        McpServerStdio::new(SERVER, binary)
            .args(vec![SUBCOMMAND.to_owned()])
            .env(env),
    )
}

/// The server as opencode's config names a local one. opencode hands it its own environment
/// and directory (measured on 1.18.33).
pub fn opencode_entry(binary: &Path) -> Value {
    json!({
        "type": "local",
        "command": [binary.display().to_string(), SUBCOMMAND],
        "enabled": true,
    })
}

/// One tool: its name, what it is for, and the JSON Schema of what it takes.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
    /// Whether it changes nothing, for a harness that shows read-only tools differently.
    pub read_only: bool,
}

pub(crate) fn no_arguments() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

pub(crate) fn one_string(key: &str, description: &str) -> Value {
    json!({
        "type": "object",
        "properties": { key: { "type": "string", "description": description } },
        "required": [key],
        "additionalProperties": false,
    })
}

/// The dispatch tools that write, which a Claude Code chat runs without its harness asking
/// (V98b, amending ADR 0064): consent to a dispatch is purlis's own dispatch grant, asked of
/// the person by the app, never the harness's prompt. **And what a chat does with its own
/// tasks is its own** (#3786, ruled 2026-10-29): a follow-up, an answer, a cancel and a close
/// of a task it dispatched, and a task's note, question and report to the chat that asked,
/// each reaching only what the app's record lets its caller reach. Named apart from
/// [`PRE_ALLOWED`], which is the tools that only read.
pub const DISPATCH_TOOLS: [&str; 8] = [
    DISPATCH,
    DISPATCH_REPORT,
    DISPATCH_TELL,
    DISPATCH_ANSWER,
    DISPATCH_CANCEL,
    DISPATCH_CLOSE,
    DISPATCH_NOTE,
    DISPATCH_ASK,
];

/// A task's chat, by its number in `dispatch_list`: the one argument a tool about one task
/// takes.
fn a_task() -> Value {
    json!({
        "type": "integer",
        "description": "The task's chat, by its number in dispatch_list.",
    })
}

/// How long a tool that waits waits: 100 seconds when left out, 540 at most.
fn wait_seconds() -> Value {
    json!({
        "type": "integer",
        "description": "How long to wait, in seconds: 100 when left out, 540 at most.",
    })
}

/// Every tool, in the order a harness lists them.
pub static TOOLS: [Tool; 26] = [
    Tool {
        name: "todo_list",
        description: "List the open todos of the workspace this chat works in, oldest first, \
                      each with the slug todo_done closes it by. A done todo is not listed: \
                      `purlis ws todo --done` lists those.",
        schema: no_arguments,
        read_only: true,
    },
    Tool {
        name: "todo_add",
        description: "Record a todo in the workspace this chat works in. Refused when an open \
                      todo is already about the same work.",
        schema: || one_string("text", "What is to be done. The first line is its title."),
        read_only: false,
    },
    Tool {
        name: "todo_done",
        description: "Mark one of this workspace's todos done by its slug (todo_list shows \
                      it). Its closing is written to the workspace's memory, and the todo stays \
                      as done, so it can be reopened (`purlis ws todo reopen <slug>`).",
        schema: || one_string("slug", "The todo's slug, as todo_list shows it."),
        read_only: false,
    },
    Tool {
        name: "memory_search",
        description: "Search the memory of the workspace this chat works in. With no query, \
                      list every memory.",
        schema: || {
            json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "Words to look for." } },
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
    Tool {
        name: "memory_add",
        description: "Record one durable fact in the memory of the workspace this chat works \
                      in. Never a secret: memory is read by every later chat here.",
        schema: || one_string("text", "The fact. The first line is its title."),
        read_only: false,
    },
    Tool {
        name: PERSONA_REMEMBER,
        description: "Record one durable fact in the memory of the persona this chat runs as, \
                      or with shared in the memory every persona reads. Never a secret: \
                      persona and shared memory are published with the project.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The fact. The first line is its title.",
                    },
                    "title": {
                        "type": "string",
                        "description": "A short title, when the first line is not one.",
                    },
                    "shared": {
                        "type": "boolean",
                        "description": "Write it to shared memory, which every persona reads, \
                                        instead of this chat's persona's own.",
                    },
                },
                "required": ["text"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: PERSONA_WHERE,
        description: "Where this chat is working, from purlis's own record: the chat that \
                      asked for it, the other tasks that chat asked for, and every other chat \
                      running as the same persona in this project, each with its workspace, \
                      its name, its state and when it started. Names and states only, never \
                      another chat's content.",
        schema: no_arguments,
        read_only: true,
    },
    Tool {
        name: "session_record_list",
        description: "List the session records of where this chat works (its workspace, or \
                      the project root), newest first.",
        schema: no_arguments,
        read_only: true,
    },
    Tool {
        name: "session_record_read",
        description: "Read one session record of where this chat works, by its file name \
                      (session_record_list shows it).",
        schema: || {
            one_string(
                "file",
                "The record's file name, YYYYMMDD-HHMMSS-<title>.md. Never a path.",
            )
        },
        read_only: true,
    },
    Tool {
        name: SESSION_RECORD,
        description: "Write this chat's session record: a summary of this session, never the \
                      transcript, that the next chat reads to pick up where this one stopped. \
                      purlis writes it where this chat works and says whether the tab now \
                      closes, which it does when the person started a smart close. The body is \
                      exactly the sections ## Goal, ## Done, ## Decisions, ## Open and \
                      ## How to resume, in that order. Never a secret.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "The record's title: one line, at most 120 characters.",
                    },
                    "body": {
                        "type": "string",
                        "description": "The five sections, each a `## ` heading with at least \
                                        one line under it.",
                    },
                    "pieces": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Pieces this session worked in that this chat does not \
                                        stand in, each as <repo>/<piece>; purlis reads their \
                                        branches from git.",
                    },
                },
                "required": ["title", "body"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: WORKTREE_UPDATE,
        description: "Bring the clone, branch folder or project root this chat stands in up \
                      to date, as `purlis worktree update` there does: the app fetches, then \
                      fast-forwards or rebases the folder's branch onto its remote (a clone), \
                      its base (a branch folder), or runs the window's Update (the root). \
                      Refused with uncommitted changes, or where it would overwrite a file that \
                      is not committed. A conflict stops it and names the files: resolve them, \
                      then call it with step continue, or abort.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "step": {
                        "type": "string",
                        "enum": ["start", "continue", "abort"],
                        "description": "start (the default), or continue or abort a rebase \
                                        it stopped.",
                    },
                    "base": {
                        "type": "string",
                        "description": "A branch folder only, at start: the branch to rebase \
                                        onto (default: the one it was cut from).",
                    },
                },
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: WORKSPACE_VISION,
        description: "Set the vision of the workspace this chat works in: one or a few lines \
                      saying what the work is for, which every later chat here reads first. \
                      It replaces the vision there was. Use it when the goal has changed. \
                      Never a secret.",
        schema: || one_string("text", "The vision. No line of it starts with `## `."),
        read_only: false,
    },
    Tool {
        name: WORKSPACE_SECTION,
        description: "Add one entry to the workspace.md of the workspace this chat works in, \
                      keeping what is there: a decision every later chat should act on, to \
                      decisions (## Context & decisions), or a term this work coined, to \
                      glossary (## Glossary). Never a secret.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "section": {
                        "type": "string",
                        "enum": crate::brokered::Section::ALL.map(crate::brokered::Section::word),
                        "description": "decisions or glossary.",
                    },
                    "text": {
                        "type": "string",
                        "description": "The entry: the decision and why, or `term` — what it \
                                        means. It is added as one list item.",
                    },
                },
                "required": ["section", "text"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: "change_status",
        description: "The cross-repo changes of this workspace: with no change named, one line \
                      each; with one, its record (why, members, branches and what each \
                      needs). The forge is not asked: run `purlis change show` for that.",
        schema: || {
            json!({
                "type": "object",
                "properties": { "change": { "type": "string", "description": "The change's slug." } },
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
    Tool {
        name: DISPATCH,
        description: "Dispatch a task to a persona. A chat of its own is started for it, \
                      which the person can see, open and stop, and which reports back to this \
                      chat on its next turn. With no persona named it runs as this chat's own \
                      persona, which needs no grant; another persona needs a dispatch grant \
                      that only the person gives, and where there is none the person is asked \
                      and the task starts when they allow it. The brief is a request to that \
                      chat, never a secret.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "A short name for the task, which the new chat is \
                                        called and listed under. At most 64 characters.",
                    },
                    "brief": {
                        "type": "string",
                        "description": "What to do and what to report back, as the new chat's \
                                        first message.",
                    },
                    "to": {
                        "type": "string",
                        "description": "The persona the new chat runs as. Leave it out for \
                                        this chat's own persona.",
                    },
                    "profile": {
                        "type": "string",
                        "description": "One of the project's harness profiles to start it \
                                        on. Leave it out for the persona's own, else this \
                                        chat's.",
                    },
                    "in": {
                        "type": "string",
                        "description": "Where it works: `workspace:<name>` for another \
                                        workspace of the project, or `worktree` for a new \
                                        worktree of the repo this chat works in, on a new \
                                        branch purlis names, which only the person merges. \
                                        From a workspace's own folder, `worktree` is cut from \
                                        its one clone, and `worktree:<repo>` names which \
                                        clone where it holds several. A sandboxed chat \
                                        commits there with `purlis worktree commit`. Leave it \
                                        out for this chat's folder.",
                    },
                    "wait": {
                        "type": "boolean",
                        "description": "Wait for the task's report and answer with it, \
                                        instead of carrying on. A wait that runs out says the \
                                        task is still running.",
                    },
                    "long": {
                        "type": "boolean",
                        "description": "A long-running task, which works through many \
                                        pieces and sends dispatch_note as it goes: its notes \
                                        are its heartbeat, purlis never asks it for a report, \
                                        and it ends when it reports done or failed, or is \
                                        cancelled. Leave it out for one job with one answer.",
                    },
                    "wait_seconds": {
                        "type": "integer",
                        "description": "With wait, how long to wait: 100 seconds when left \
                                        out, 540 at most.",
                    },
                },
                "required": ["name", "brief"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_LIST,
        description: "List the tasks this chat dispatched: each one's chat number, name, \
                      persona, where it works, its state and how long ago it started. A task \
                      ends at its report, and stays listed as finished until its row is cleared \
                      or this chat closes.",
        schema: || json!({"type": "object", "properties": {}, "additionalProperties": false}),
        read_only: true,
    },
    Tool {
        name: DISPATCH_READ,
        description: "Read what a task this chat dispatched said last and the commands it ran, \
                      with their exit codes, as its harness handed them to purlis: quoted as \
                      data, credential shapes masked. Only a task this chat dispatched can be \
                      read; the whole conversation is the person's, in the task's tab.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "chat": {
                        "type": "integer",
                        "description": "The task's chat, by its number in dispatch_list.",
                    },
                },
                "required": ["chat"],
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
    Tool {
        name: DISPATCH_WAIT,
        description: "Wait for the report of a task this chat dispatched, and answer with it. \
                      A wait that runs out says the task is still running; its report reaches \
                      this chat when it lands all the same. A task that asked this chat a \
                      question answers the wait with the question.",
        schema: || {
            json!({
                "type": "object",
                "properties": { "chat": a_task(), "wait_seconds": wait_seconds() },
                "required": ["chat"],
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
    Tool {
        name: DISPATCH_TELL,
        description: "Send a follow-up to a task this chat dispatched that is still working, \
                      quoted as data from this chat, or with `all` to every task of this chat \
                      still working. A task at rest is started on it at once; one mid-turn \
                      reads it when that turn ends. A task that reported while its program \
                      still runs is told too. Refused, with the task's state, once its program \
                      has ended. Each follow-up is an instruction with a number, which a task \
                      acknowledges with dispatch_note's `ack`; dispatch_list shows which tasks \
                      have. Where the app does not answer, it is queued and delivered when it \
                      does.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "chat": a_task(),
                    "all": {
                        "type": "boolean",
                        "description": "Send it to every task of this chat still working, \
                                        instead of the one `chat` names.",
                    },
                    "text": {
                        "type": "string",
                        "description": "What to add or correct, in a few lines.",
                    },
                },
                "required": ["text"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_ANSWER,
        description: "Answer the question a task this chat dispatched asked it. Only a question \
                      that task asked this chat can be answered: one it put to the person is \
                      the person's.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "chat": a_task(),
                    "text": {
                        "type": "string",
                        "description": "The answer, in a few lines.",
                    },
                },
                "required": ["chat", "text"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_CANCEL,
        description: "Cancel a task this chat dispatched: its turn is ended and it is asked for \
                      one short report of what it did, which arrives with the outcome \
                      `cancelled`. A task that reported while its program still runs can be \
                      cancelled too. Refused for a task the person has typed in: that one is \
                      theirs.",
        schema: || {
            json!({
                "type": "object",
                "properties": { "chat": a_task() },
                "required": ["chat"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_CLOSE,
        description: "Close a finished task this chat dispatched: its finished row goes, and a \
                      task kept open after its report has its program ended. Refused for a \
                      task that has not reported, one in a turn, and one the person typed in.",
        schema: || {
            json!({
                "type": "object",
                "properties": { "chat": a_task() },
                "required": ["chat"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_REPORT,
        description: "Send this task's one report back to the chat that dispatched it: how it \
                      ended, what you did and found, and what changed. Write your session \
                      record first; purlis adds its path. Only a chat a dispatch started has \
                      a report to send. A turn that ends without one is asked for it once; if \
                      the next turn ends without one too, purlis sends your last message in \
                      its place, marked as a report purlis made from its last message.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "outcome": {
                        "type": "string",
                        "enum": ["done", "blocked", "failed", "cancelled"],
                        "description": "How the task ended.",
                    },
                    "text": {
                        "type": "string",
                        "description": "What you did and what you found, in a few lines.",
                    },
                    "changed": {
                        "type": "string",
                        "description": "What changed: files, commits, a branch.",
                    },
                },
                "required": ["outcome", "text"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_NOTE,
        description: "From a dispatched task: send the chat that asked a progress note. It \
                      reads it on its next turn, and nothing waits on it. With `ack`, it \
                      acknowledges that instruction (a follow-up's number) of the asking \
                      chat's, and `text` may be left out.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Where the task has got to, in a few lines.",
                    },
                    "ack": {
                        "type": "integer",
                        "description": "The number of an instruction of the asking chat's \
                                        this task has taken in.",
                    },
                },
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: DISPATCH_ASK,
        description: "From a dispatched task: ask the chat that asked a question, and wait for \
                      its answer. A question only the person can answer is not asked this \
                      way: ask them in this chat. If the wait runs out, end the turn: the \
                      answer is handed to the next one.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question, in a few lines.",
                    },
                    "wait_seconds": wait_seconds(),
                },
                "required": ["question"],
                "additionalProperties": false,
            })
        },
        read_only: false,
    },
    Tool {
        name: ASK_OPERATOR,
        description: "Ask the operator a question and wait for the answer, through this \
                      harness's own prompt. Never ask for a password, token or other secret: \
                      a credential is used through `purlis secret`, never typed in. When a page \
                      in one of your browsers needs a person (a CAPTCHA, a second factor, a \
                      consent page), name that browser in `browser`: the person is asked to \
                      take it over, and this returns at once. Leave that browser alone; you are \
                      told when they hand it back, and never what they did there.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question, as the operator will read it: for a \
                                        browser, what the page needs of them.",
                    },
                    "browser": {
                        "type": "string",
                        "description": "One of your browsers, by the name the browser tools \
                                        gave it (b1), when it is that browser that needs the \
                                        person.",
                    },
                },
                "required": ["question"],
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
];

/// What `tool` does with `args` for a chat whose place is `place` in the project at `root`:
/// the text the chat is answered with, or the sentence saying why nothing was done.
///
/// Every store is held open for the call ([`held::Store`]) and used through its descriptor
/// only, so nothing a chat swaps in while the tool runs can move a read or a write out of its
/// workspace (V74).
pub fn call(
    root: &Path,
    place: &Place,
    tool: &str,
    args: &Map<String, Value>,
    now: chrono::NaiveDateTime,
) -> Result<String, String> {
    let Some(spec) = TOOLS.iter().find(|t| t.name == tool) else {
        return Err(format!("purlis has no tool {}", crate::shown::short(tool)));
    };
    if tool == ASK_OPERATOR {
        return Err(format!(
            "{ASK_OPERATOR} is asked through the harness by purlis's MCP server, not here"
        ));
    }
    if [
        SESSION_RECORD,
        PERSONA_REMEMBER,
        WORKSPACE_VISION,
        WORKSPACE_SECTION,
        DISPATCH,
        DISPATCH_LIST,
        DISPATCH_READ,
        DISPATCH_WAIT,
        DISPATCH_TELL,
        DISPATCH_ANSWER,
        DISPATCH_CANCEL,
        DISPATCH_CLOSE,
        DISPATCH_REPORT,
        DISPATCH_NOTE,
        DISPATCH_ASK,
        WORKTREE_UPDATE,
    ]
    .contains(&tool)
    {
        return Err(format!(
            "{tool} is handed to the app by purlis's MCP server, not written here"
        ));
    }
    if tool == PERSONA_WHERE {
        return Err(format!(
            "{tool} is answered by the app through purlis's MCP server, not here"
        ));
    }
    // FR-24, at every write and not only when the server started: the server lives as long
    // as the chat, and the project can be moved to a newer format under it.
    if !spec.read_only
        && let crate::compat::Compat::ReadOnly(why) = crate::compat::read(root)
    {
        return Err(format!("nothing was written: {why}"));
    }
    run(root, place, tool, args, now)
}

#[cfg(not(unix))]
fn run(
    _root: &Path,
    _place: &Place,
    _tool: &str,
    _args: &Map<String, Value>,
    _now: chrono::NaiveDateTime,
) -> Result<String, String> {
    // Fail closed: the stores are held through descriptors that refuse links, which only the
    // unix build does yet.
    Err("purlis's MCP tools are not available on this platform yet".to_owned())
}

#[cfg(unix)]
fn run(
    root: &Path,
    place: &Place,
    tool: &str,
    args: &Map<String, Value>,
    now: chrono::NaiveDateTime,
) -> Result<String, String> {
    match tool {
        "todo_list" => {
            let ws = workspace_name(place)?;
            let Some(todos) = held::Store::open(root, place, "todos", false)? else {
                return Ok(format!("No open todos in '{ws}'."));
            };
            todo_list(&todos, ws, now)
        }
        "todo_add" => {
            let ws = workspace_name(place)?;
            let text = string(args, "text")?;
            if text.trim().is_empty() {
                return Err(crate::workspaces::RecordRefused::Empty.to_string());
            }
            let todos = must_open(root, place, "todos")?;
            let stored = stored_texts(&todos)?;
            if let Some(dup) =
                crate::memstore::duplicate_in(text, stored.iter().map(String::as_str))
            {
                return Err(crate::workspaces::RecordRefused::AlreadyListed(dup).to_string());
            }
            let header = crate::workspaces::TODOS_HEADER.replace("{name}", ws);
            let file = write_entry(&todos, &header, text, now)?;
            Ok(format!("Todo recorded in '{ws}': {}", stem(&file)))
        }
        "todo_done" => {
            let ws = workspace_name(place)?;
            let slug = string(args, "slug")?;
            if !crate::contain::segment_ok(slug.strip_suffix(".md").unwrap_or(slug)) {
                return Err(format!(
                    "{} is not a todo's slug: todo_list shows them",
                    crate::shown::short(slug)
                ));
            }
            let Some(todos) = held::Store::open(root, place, "todos", false)? else {
                return Err(format!("no such todo: {slug}"));
            };
            let file = resolve(&todos, slug)?;
            let text = todos
                .read(&file)?
                .ok_or_else(|| format!("no such todo: {slug}"))?;
            let title = crate::workspaces::parse_entry(&stem(&file), &text).title;
            // The trace first, then the todo: a failure leaves the todo open rather than closed
            // with nothing recorded, as `Workspace::close_todo` does.
            let memory = must_open(root, place, "memory")?;
            let header = crate::workspaces::ws_memory_header(root, ws);
            write_entry(&memory, &header, &format!("Closed todo: {title}"), now)?;
            file_as_done(&todos, &file)?;
            Ok(format!(
                "Closed todo {} in '{ws}'; its memory has the trace. It stays as done: \
                 purlis ws todo reopen {} opens it again.",
                stem(&file),
                stem(&file)
            ))
        }
        "memory_search" => {
            let ws = workspace_name(place)?;
            let found = match held::Store::open(root, place, "memory", false)? {
                Some(memory) => memories(&memory)?,
                None => Vec::new(),
            };
            memory_search(ws, found, args)
        }
        "memory_add" => {
            let ws = workspace_name(place)?;
            let text = string(args, "text")?;
            let memory = must_open(root, place, "memory")?;
            let header = crate::workspaces::ws_memory_header(root, ws);
            let file = write_entry(&memory, &header, text, now)?;
            Ok(format!("Remembered in '{ws}': {}", stem(&file)))
        }
        "session_record_list" => {
            let mut listed = Vec::new();
            if let Some(sessions) =
                held::Store::open(root, place, crate::sessionrecord::DIR, false)?
            {
                let mut names: Vec<String> = sessions
                    .files()?
                    .into_iter()
                    .filter(|name| crate::sessionrecord::is_record_name(name))
                    .collect();
                names.sort_unstable_by(|a, b| b.cmp(a));
                for name in names {
                    if let Some(text) = sessions.read(&name)? {
                        listed.push(crate::sessionrecord::listed(place, name, &text));
                    }
                }
            }
            if listed.is_empty() {
                return Ok(format!("No session records at {}.", place.said()));
            }
            Ok(listed
                .iter()
                .map(|r| format!("{} · {} · {}", r.when, r.file, crate::shown::line(&r.title)))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "session_record_read" => {
            let file = string(args, "file")?;
            if !crate::sessionrecord::is_record_name(file) {
                return Err(format!(
                    "{} is not a session record's file name (YYYYMMDD-HHMMSS-<title>.md)",
                    crate::shown::short(file)
                ));
            }
            held::Store::open(root, place, crate::sessionrecord::DIR, false)?
                .map(|sessions| sessions.read(file))
                .transpose()?
                .flatten()
                .ok_or_else(|| format!("no session record {file} at {}", place.said()))
        }
        "change_status" => {
            let ws = workspace_name(place)?;
            let changes = held::Store::open(root, place, crate::change::store::DIRNAME, false)?;
            match optional_string(args, "change")? {
                None => change_list(changes.as_ref(), ws),
                Some(slug) => change_record(changes.as_ref(), slug),
            }
        }
        other => Err(format!("purlis has no tool {}", crate::shown::short(other))),
    }
}

/// The store, held, and made when it is not there yet.
#[cfg(unix)]
fn must_open(root: &Path, place: &Place, store: &str) -> Result<held::Store, String> {
    held::Store::open(root, place, store, true)?
        .ok_or_else(|| format!("the chat's {store} could not be made"))
}

/// What a `session_record` call hands over: its title, its body and the pieces it names, or
/// why it hands over nothing.
pub fn record_args(args: &Map<String, Value>) -> Result<(String, String, Vec<String>), String> {
    let title = string(args, "title")?.to_owned();
    let body = string(args, "body")?.to_owned();
    let pieces = match args.get("pieces") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(pieces)) => pieces
            .iter()
            .map(|piece| {
                piece
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "`pieces` is a list of strings".to_owned())
            })
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("`pieces` is a list of strings".to_owned()),
    };
    Ok((title, body, pieces))
}

/// What a `persona_remember` call hands over: its text, its title and whether it is shared,
/// or why it hands over nothing.
pub fn persona_remember_args(
    args: &Map<String, Value>,
) -> Result<(String, Option<String>, bool), String> {
    let shared = match args.get("shared") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(shared)) => *shared,
        Some(_) => return Err("`shared` is true or false".to_owned()),
    };
    Ok((
        string(args, "text")?.to_owned(),
        optional_string(args, "title")?.map(str::to_owned),
        shared,
    ))
}
/// What a `workspace_vision` or `workspace_section` call hands the app: the brokered write, or
/// why it hands over nothing.
pub fn workspace_write(
    tool: &str,
    args: &Map<String, Value>,
) -> Result<crate::brokered::Write, String> {
    let text = string(args, "text")?.to_owned();
    match tool {
        WORKSPACE_VISION => Ok(crate::brokered::Write::WorkspaceVision { text }),
        WORKSPACE_SECTION => {
            let word = string(args, "section")?;
            let section = crate::brokered::Section::named(word).ok_or_else(|| {
                format!(
                    "{} is not a section a chat adds to: decisions or glossary",
                    crate::shown::short(word)
                )
            })?;
            Ok(crate::brokered::Write::WorkspaceSection { section, text })
        }
        other => Err(format!(
            "{} is not a workspace.md tool",
            crate::shown::short(other)
        )),
    }
}

/// How long the `dispatch` tool waits for the task's report, in seconds, where its arguments
/// say to wait: `wait_seconds`, or [`crate::dispatched::WAITS_BY_DEFAULT`].
pub fn dispatch_waits(args: &Map<String, Value>) -> Result<Option<u32>, String> {
    let wait = match args.get("wait") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(wait)) => *wait,
        Some(_) => return Err("`wait` is true or false".to_owned()),
    };
    let seconds = match args.get("wait_seconds") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => Some(
            n.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("`wait_seconds` is a whole number of seconds")?,
        ),
        Some(_) => return Err("`wait_seconds` is a whole number of seconds".to_owned()),
    };
    Ok(wait.then(|| seconds.unwrap_or(crate::dispatched::WAITS_BY_DEFAULT)))
}

/// The `dispatch` tool's arguments: the persona where one is named, the task's name and the
/// brief.
pub fn dispatch_args(
    args: &Map<String, Value>,
) -> Result<(Option<String>, String, String), String> {
    let to = match args.get("to") {
        None | Some(Value::Null) => None,
        Some(Value::String(to)) if !to.trim().is_empty() => Some(to.trim().to_owned()),
        Some(Value::String(_)) => None,
        Some(_) => return Err("`to` is a persona's name".to_owned()),
    };
    Ok((
        to,
        string(args, "name")?.to_owned(),
        string(args, "brief")?.to_owned(),
    ))
}

/// The `dispatch` tool's `profile`: the name of the project's profile the new chat starts on,
/// where one is asked for. A name and nothing more: the app looks it up among the profiles
/// the project offers.
pub fn dispatch_profile(args: &Map<String, Value>) -> Result<Option<String>, String> {
    match args.get("profile") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(profile)) if !profile.trim().is_empty() => {
            Ok(Some(profile.trim().to_owned()))
        }
        Some(Value::String(_)) => Ok(None),
        Some(_) => Err("`profile` is a profile's name".to_owned()),
    }
}

/// The `dispatch` tool's `in`: where the new chat works, where that is asked for (#1453). One
/// of three words and nothing more: the app reads it ([`crate::dispatchplace::asked`]).
pub fn dispatch_place(args: &Map<String, Value>) -> Result<Option<String>, String> {
    match args.get("in") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(place)) if !place.trim().is_empty() => Ok(Some(place.trim().to_owned())),
        Some(Value::String(_)) => Ok(None),
        Some(_) => Err("`in` is `worktree`, `worktree:<repo>` or `workspace:<name>`".to_owned()),
    }
}

/// The `dispatch` tool's `long` (#3905): whether the task is a long-running one, `false` where
/// it is left out.
pub fn dispatch_long(args: &Map<String, Value>) -> Result<bool, String> {
    match args.get("long") {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(long)) => Ok(*long),
        Some(_) => Err("`long` is true or false".to_owned()),
    }
}

/// The `dispatch_read` tool's argument (#2084): the task's chat, by its number.
pub fn dispatch_read_args(args: &Map<String, Value>) -> Result<u32, String> {
    dispatch_task(args, DISPATCH_READ)
}

/// The task a dispatch tool about one task names (#3786): its chat, by its number.
pub fn dispatch_task(args: &Map<String, Value>, tool: &str) -> Result<u32, String> {
    args.get("chat")
        .and_then(Value::as_u64)
        .and_then(|chat| u32::try_from(chat).ok())
        .ok_or_else(|| {
            format!("{tool} needs `chat`: the task's chat, by its number in dispatch_list")
        })
}

/// The text a dispatch tool sends, under `key`, as it was written (#3786): the app and the
/// command hold it to their own bounds.
pub fn dispatch_text(args: &Map<String, Value>, key: &str, tool: &str) -> Result<String, String> {
    match args.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        _ => Err(format!("{tool} needs `{key}`, as text")),
    }
}

/// A yes-or-no field of a dispatch tool's, `false` where it is left out (#3904).
pub fn dispatch_flag(args: &Map<String, Value>, key: &str, tool: &str) -> Result<bool, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(_) => Err(format!("{tool}'s `{key}` is true or false")),
    }
}

/// The instruction `dispatch_note` acknowledges, where it names one (#3904).
pub fn dispatch_ack(args: &Map<String, Value>) -> Result<Option<u32>, String> {
    match args.get("ack") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| "`ack` is an instruction's number".to_owned()),
        Some(_) => Err("`ack` is an instruction's number".to_owned()),
    }
}

/// How long a dispatch tool that waits is asked to wait, in seconds, where it says (#3786).
pub fn dispatch_wait_seconds(args: &Map<String, Value>) -> Result<Option<u32>, String> {
    match args.get("wait_seconds") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| "`wait_seconds` is a whole number of seconds".to_owned()),
        Some(_) => Err("`wait_seconds` is a whole number of seconds".to_owned()),
    }
}

/// The `dispatch_report` tool's arguments: the outcome, the text, and what changed.
pub fn dispatch_report_args(
    args: &Map<String, Value>,
) -> Result<(String, String, Option<String>), String> {
    let changed = match args.get("changed") {
        None | Some(Value::Null) => None,
        Some(Value::String(changed)) => Some(changed.clone()),
        Some(_) => return Err("`changed` is text".to_owned()),
    };
    Ok((
        string(args, "outcome")?.to_owned(),
        string(args, "text")?.to_owned(),
        changed,
    ))
}

/// The question an `ask_operator` call puts, trimmed, or why it puts none.
pub fn question(args: &Map<String, Value>) -> Result<String, String> {
    let asked = string(args, "question")?.trim();
    if asked.is_empty() {
        return Err("a question for the operator needs some words".to_owned());
    }
    Ok(asked.to_owned())
}

/// The message the harness shows the operator for `question`: who is asking, so a chat cannot
/// phrase its question as if charter itself asked.
pub fn asked(question: &str) -> String {
    format!("This chat asks: {question}")
}

/// What the answer's field says beneath the question, in the harness's own form.
pub const NO_SECRET_HERE: &str = "Don't type a password or secret here.";

/// The `requestState` an `ask_operator` call is retried with at protocol 2026-07-28: bound to
/// the question, so an answer is only ever read back for the question it answers.
pub fn question_state(question: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(question.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("{ASK_OPERATOR}:{hex}")
}

/// The chat's workspace, where it is one.
#[cfg_attr(not(unix), allow(dead_code))]
fn workspace_name(place: &Place) -> Result<&str, String> {
    match place {
        Place::Workspace(name) => Ok(name),
        Place::PlaneRoot => Err(
            "this chat works at the project root, and todos, memory and changes belong to a \
             workspace"
                .to_owned(),
        ),
    }
}

fn string<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    optional_string(args, key)?.ok_or_else(|| format!("`{key}` is required"))
}

fn optional_string<'a>(args: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(format!("`{key}` is a string")),
    }
}

/// A file name without its `.md`.
#[cfg_attr(not(unix), allow(dead_code))]
fn stem(file: &str) -> String {
    file.strip_suffix(".md").unwrap_or(file).to_owned()
}

/// The entries of a memory or todo store: every `*.md` but the index, sorted by name.
#[cfg(unix)]
fn entry_names(store: &held::Store) -> Result<Vec<String>, String> {
    Ok(store
        .files()?
        .into_iter()
        .filter(|name| name.ends_with(".md") && name != crate::memstore::INDEX)
        .collect())
}

/// The whole text of every entry of a store.
#[cfg(unix)]
fn stored_texts(store: &held::Store) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for name in entry_names(store)? {
        if let Some(text) = store.read(&name)? {
            out.push(text);
        }
    }
    Ok(out)
}

/// Every entry of a memory store as `charter recall` reads it.
#[cfg(unix)]
fn memories(store: &held::Store) -> Result<Vec<crate::memstore::Found>, String> {
    let mut out = Vec::new();
    for name in entry_names(store)? {
        if let Some(text) = store.read(&name)? {
            let path = std::path::PathBuf::from(&name);
            let title = crate::memstore::title_in(&path, &text);
            out.push(crate::memstore::Found { path, title, text });
        }
    }
    Ok(out)
}

/// Write one entry and its index line, as `memstore::write` does for a workspace's memory and
/// todos: a timestamp-prefixed file named for its title, numbered when the name is taken, and
/// the index made with `header` when it is not there. The file name it took.
#[cfg(unix)]
fn write_entry(
    store: &held::Store,
    header: &str,
    text: &str,
    now: chrono::NaiveDateTime,
) -> Result<String, String> {
    let text = crate::memstore::py_strip(text);
    if text.is_empty() {
        return Err("empty memory".to_owned());
    }
    let title = crate::memstore::title_of(text);
    let header = if header.ends_with('\n') {
        header.to_owned()
    } else {
        format!("{header}\n")
    };
    // The index first, as `ensure_index` makes it with the store's own header.
    store.append(crate::memstore::INDEX, &header, b"")?;
    // Then the one write of a workspace's memory and todos, which `charter` commands use too:
    // the file and its index line together, or neither (#1058).
    crate::memstore::write_in(
        store,
        &crate::memstore::NoGates,
        text,
        &title,
        true,
        "persistent",
        true,
        now,
    )
    .map_err(|e| e.to_string())
}

/// The entry `ident` names: its exact name, or the one entry whose name ends `-<ident>.md`.
#[cfg(unix)]
fn resolve(store: &held::Store, ident: &str) -> Result<String, String> {
    let names = entry_names(store)?;
    let exact = crate::memstore::md_name(ident);
    if names.contains(&exact) {
        return Ok(exact);
    }
    let suffix = format!("-{exact}");
    let tails: Vec<&String> = names.iter().filter(|n| n.ends_with(&suffix)).collect();
    match tails.as_slice() {
        [] => Err(format!("no such todo: {ident}")),
        [one] => Ok((*one).clone()),
        many => Err(format!(
            "'{ident}' is the end of more than one todo's name — {}; name one in full",
            many.iter().map(|n| stem(n)).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// Move `file` into the store's `archive/` and drop its index line, through the held store: a
/// done todo stays there until it is reopened or deleted (ADR 0106), as
/// `Workspace::close_todo` keeps it. A name already taken there is numbered `-2`, then `-2-3`,
/// as `memstore::archive_one` numbers it.
#[cfg(unix)]
fn file_as_done(store: &held::Store, file: &str) -> Result<(), String> {
    let archive = store.sub(crate::memstore::ARCHIVE, true)?.ok_or_else(|| {
        format!(
            "the chat's todos/{} could not be made",
            crate::memstore::ARCHIVE
        )
    })?;
    // Under the store's lock, as `memstore::archive_one` moves one, and never onto a file: a
    // name taken, or taken between the look and the move, is numbered.
    let _held = store.lock()?;
    let mut dest = file.to_owned();
    let mut n = 2;
    while archive.entry(&dest)?.is_some() || !store.rename_into_new(file, &archive, &dest)? {
        dest = format!("{}-{n}.md", stem(&dest));
        n += 1;
    }
    // As `memstore::archive_one`: the file has moved, so a failure to drop its line is drift
    // the next `optimize` names, never a failure of the close.
    crate::memstore::rewrite_index(store, file, None);
    Ok(())
}

#[cfg(unix)]
fn todo_list(store: &held::Store, ws: &str, now: chrono::NaiveDateTime) -> Result<String, String> {
    let mut open = Vec::new();
    for name in entry_names(store)? {
        if let Some(text) = store.read(&name)? {
            open.push(crate::workspaces::parse_entry(&stem(&name), &text));
        }
    }
    if open.is_empty() {
        return Ok(format!("No open todos in '{ws}'."));
    }
    let today = now.date();
    Ok(open
        .iter()
        .map(|todo| {
            let age = chrono::NaiveDate::parse_from_str(
                todo.stamp.split_whitespace().next().unwrap_or_default(),
                "%Y-%m-%d",
            )
            .map_or(0, |d| (today - d).num_days());
            format!(
                "{}  {age}d  {}",
                todo.slug,
                crate::personas::one_line(&todo.title)
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

#[cfg_attr(not(unix), allow(dead_code))]
fn memory_search(
    ws: &str,
    found: Vec<crate::memstore::Found>,
    args: &Map<String, Value>,
) -> Result<String, String> {
    let query = optional_string(args, "query")?
        .map(str::trim)
        .filter(|q| !q.is_empty());
    let found: Vec<crate::memstore::Found> = match query {
        Some(q) => crate::memstore::rank(found, q, 8)
            .into_iter()
            .map(|(found, _)| found)
            .collect(),
        None => found,
    };
    if found.is_empty() {
        return Ok(match query {
            Some(q) => format!("No memories in '{ws}' match {}.", crate::shown::short(q)),
            None => format!("'{ws}' has no memories yet."),
        });
    }
    Ok(found
        .iter()
        .map(|f| format!("## {}\n{}", crate::shown::line(&f.title), f.text.trim()))
        .collect::<Vec<_>>()
        .join("\n\n"))
}

/// One line per change, as `charter change list` draws them.
#[cfg(unix)]
fn change_list(changes: Option<&held::Store>, ws: &str) -> Result<String, String> {
    let line = crate::shown::line;
    let mut out = Vec::new();
    if let Some(changes) = changes {
        for name in changes.files()? {
            let Some(slug) = name.strip_suffix(".json") else {
                continue;
            };
            let Some(text) = changes.read(&name)? else {
                continue;
            };
            match crate::change::Record::parse(&text, slug) {
                Ok(record) => out.push(format!(
                    "{}  {} member(s)  ·  {}",
                    line(&record.change),
                    record.members.len(),
                    line(&record.why)
                )),
                Err(why) => out.push(format!(
                    "{}: {}",
                    crate::shown::short(slug),
                    line(&why.to_string())
                )),
            }
        }
    }
    if out.is_empty() {
        return Ok(format!(
            "No changes in workspace '{ws}'. Create one: purlis change create <slug> --why \"…\""
        ));
    }
    Ok(out.join("\n"))
}

/// One change's record: why, members, branches and what each needs. Never the forge.
#[cfg(unix)]
fn change_record(changes: Option<&held::Store>, slug: &str) -> Result<String, String> {
    if !crate::change::name_ok(slug) {
        return Err(format!(
            "{} is not a change name",
            crate::shown::short(slug)
        ));
    }
    let text = changes
        .map(|changes| changes.read(&format!("{slug}.json")))
        .transpose()?
        .flatten()
        .ok_or_else(|| format!("no change {slug}"))?;
    let record = crate::change::Record::parse(&text, slug).map_err(|e| e.to_string())?;
    let line = crate::shown::line;
    let mut out = vec![
        format!(
            "{} · {} member(s)",
            line(&record.change),
            record.members.len()
        ),
        format!("why: {}", line(&record.why)),
        format!("created {} by {}", line(&record.created), line(&record.by)),
    ];
    for m in &record.members {
        let mut row = format!("member {} on {}", line(&m.repo), line(&m.branch));
        if !m.needs.is_empty() {
            let needs: Vec<String> = m.needs.iter().map(|n| line(n)).collect();
            row.push_str(&format!(", needs {}", needs.join(", ")));
        }
        out.push(row);
    }
    for e in &record.excluded {
        out.push(format!("excluded {}: {}", line(&e.repo), line(&e.why)));
    }
    Ok(out.join("\n"))
}

#[cfg(unix)]
use crate::held;

#[cfg(test)]
mod tests;

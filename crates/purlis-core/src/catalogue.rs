// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one tool catalogue** (ADR 0122 §7, #5951): every tool purlis's MCP server can list, and
//! the read-only ones among them that are also **queries**, the API a page, the CLI and an
//! agent read through alike.
//!
//! A query is an entry of [`CATALOGUE`] naming an MCP tool that only reads. It is
//! `<source>.<name>` (`purlis.todo_list`), permanent once released: a rename is a new id
//! declaring what it [`Query::replaces`]. Its parameters are the tool's own JSON Schema, and its
//! result is described in the same subset MCP tools use. Every answer is one [`Answer`]:
//! `{ data | error: { kind }, at, cursor? }`. `purlis queries` prints [`listing`].
//!
//! A read-only tool that is no query says why in [`NOT_QUERIES`]: a page has no chat, no
//! person to ask and no browser, so a tool about one of those is not a page's to call. A test
//! holds that every read-only tool is in exactly one of the two lists.
//!
//! The protocol's key, its version and the released ids are pinned in
//! `tests/fixtures/queries/v1.json`, so none of them moves except on purpose.

use serde_json::{Map, Value, json};

use crate::chattools::Tool;

/// The source of the core's queries, before the dot.
pub const SOURCE: &str = "purlis";

/// The key a page declares the API version it was written against under (`api: 1`).
pub const PROTOCOL_KEY: &str = "api";

/// The API version this build answers. purlis keeps answering every version it has shipped.
pub const API: u32 = 1;

/// **Every tool purlis's MCP server can list**, before a chat's place narrows it, in the order a
/// harness lists them.
pub fn every_tool() -> impl Iterator<Item = &'static Tool> {
    crate::chattools::TOOLS
        .iter()
        .chain(crate::browsertools::TOOLS.iter())
        .chain(crate::themepreview::TOOLS.iter())
        .chain(crate::skilltools::TOOLS.iter())
        .chain(crate::artifact::proposal::TOOLS.iter())
        .chain(crate::artifact::tools::TOOLS.iter())
}

/// What a query reads: one workspace's own, or the whole plane's, which is a grant of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    Plane,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Workspace => "workspace",
            Scope::Plane => "plane",
        }
    }
}

/// One query: a read-only MCP tool, by its name, with what a page needs to rely on it.
pub struct Query {
    /// The MCP tool's name, which is the id after `purlis.`.
    pub name: &'static str,
    /// The purlis release the id first answers in.
    pub since: &'static str,
    /// The id this one renamed, which still answers while it is deprecated.
    pub replaces: Option<&'static str>,
    /// Why the id is deprecated and what to use instead; it answers for one more release.
    pub deprecated: Option<&'static str>,
    pub scope: Scope,
    /// The parameters that widen what it reads, each with what it then reads, which a page's
    /// approval names (ADR 0122 §7, #5969).
    pub widens: &'static [(&'static str, &'static str)],
    /// The JSON Schema of the answer's `data`.
    pub result: fn() -> Value,
}

impl Query {
    /// `purlis.<name>`.
    pub fn id(&self) -> String {
        format!("{SOURCE}.{}", self.name)
    }

    /// The MCP tool it answers from.
    ///
    /// # Panics
    ///
    /// When no tool has its name, which a test refuses.
    pub fn tool(&self) -> &'static Tool {
        every_tool()
            .find(|t| t.name == self.name)
            .unwrap_or_else(|| panic!("no tool {}", self.name))
    }

    /// The JSON Schema of what it takes: its tool's.
    pub fn parameters(&self) -> Value {
        (self.tool().schema)()
    }

    /// The query as `purlis queries` lists it.
    pub fn listed(&self) -> Value {
        let mut out = Map::new();
        out.insert("id".into(), json!(self.id()));
        out.insert("description".into(), json!(self.tool().description));
        out.insert("scope".into(), json!(self.scope.as_str()));
        out.insert("since".into(), json!(self.since));
        if let Some(old) = self.replaces {
            out.insert("replaces".into(), json!(old));
        }
        if let Some(why) = self.deprecated {
            out.insert("deprecated".into(), json!(why));
        }
        out.insert("parameters".into(), self.parameters());
        if !self.widens.is_empty() {
            let widens: Map<String, Value> = self
                .widens
                .iter()
                .map(|(name, what)| ((*name).to_owned(), json!(what)))
                .collect();
            out.insert("widens".into(), Value::Object(widens));
        }
        out.insert("result".into(), (self.result)());
        Value::Object(out)
    }
}

/// The release the first queries answer in.
const FIRST: &str = "0.5.1";

/// A JSON Schema object whose every property is required and no other is allowed.
fn object(properties: Value) -> Value {
    let required: Vec<String> = properties
        .as_object()
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn list_of(item: Value) -> Value {
    json!({ "type": "array", "items": item })
}

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

/// What `purlis.todo_list` answers: the workspace's open todos, or its done ones.
fn todos() -> Value {
    object(json!({
        "done": { "type": "boolean", "description": "Whether these are the done todos." },
        "todos": list_of(object(json!({
            "slug": string("What todo_done closes it by, or todo_reopen opens it again by."),
            "title": string("Its title."),
            "opened": string("When it was recorded, as its file says it (YYYY-MM-DD HH:MM), or empty."),
        }))),
    }))
}

/// What `purlis.memory_search` answers: the memories found, best or newest first.
fn memories() -> Value {
    object(json!({
        "memories": list_of(object(json!({
            "file": string("Its file name in the workspace's memory."),
            "title": string("Its title."),
            "text": string("Its whole text, as written."),
        }))),
    }))
}

/// What `purlis.session_record_list` answers: the workspace's session records, newest first.
fn session_records() -> Value {
    object(json!({
        "records": list_of(object(json!({
            "file": string("Its file name, which session_record_read reads it by."),
            "title": string("Its title, or its file name when it has none."),
            "when": string("When it was written, YYYY-MM-DD HH:MM."),
        }))),
    }))
}

/// What `purlis.session_record_read` answers: one record, or null when it is not there.
fn session_record() -> Value {
    json!({
        "type": ["object", "null"],
        "properties": {
            "file": string("Its file name."),
            "text": string("Its whole text, as written."),
        },
        "required": ["file", "text"],
        "additionalProperties": false,
    })
}

/// What `purlis.change_status` answers: the workspace's cross-repo changes, or the one named.
fn changes() -> Value {
    object(json!({
        "changes": list_of(object(json!({
            "change": string("Its name."),
            "why": string("Why it is being made."),
            "created": string("When it was created."),
            "by": string("Who created it."),
            "members": list_of(object(json!({
                "repo": string("The repo."),
                "branch": string("Its branch there."),
                "needs": list_of(string("A repo whose member lands first.")),
            }))),
            "excluded": list_of(object(json!({
                "repo": string("The repo left out."),
                "why": string("Why."),
            }))),
        }))),
        "unreadable": list_of(object(json!({
            "change": string("The name of a record that could not be read."),
            "why": string("Why."),
        }))),
    }))
}

/// What `purlis.chats` answers: the workspace's chats, a tree by their parents (#6235).
fn chats() -> Value {
    let nullable =
        |description: &str| json!({ "type": ["string", "null"], "description": description });
    object(json!({
        "history": { "type": "boolean", "description": "Whether ended dispatches are listed too." },
        "chats": list_of(object(json!({
            "id": string("Its id, or chat-<n> for one given none yet: what a parent names it by."),
            "name": string("Its name, as its tab shows it."),
            "persona": nullable("The persona it runs as, or null."),
            "harness": nullable("Its harness, by the project's word for it, or null for a shell."),
            "state": json!({
                "type": "string",
                "enum": chats::Mark::ALL.iter().map(|m| m.key()).collect::<Vec<_>>(),
                "description": "Its Chat state, as the window's mark keys it.",
            }),
            "state_said": string("Its Chat state, as the window names it: Waiting on you, Failed, …"),
            "cause": string("The fact that decided its state, such as needs-you-queue or turn-ended."),
            "started": nullable("When it started, RFC 3339, or null where it is not known."),
            "last_activity": nullable("When it last moved, RFC 3339, or null where it is not known."),
            "parent": json!({
                "type": ["object", "null"],
                "description": "The chat it came from, or null for one the person opened.",
                "properties": {
                    "id": string("That chat's id."),
                    "relation": json!({
                        "type": "string",
                        "enum": ["dispatched", "handed-off-from"],
                        "description": "dispatched: a task that owes it a report; handed-off-from: the work moved here.",
                    }),
                },
                "required": ["id", "relation"],
                "additionalProperties": false,
            }),
            "session_record": nullable("Its newest session record, the file name session_record_read takes, or null."),
            "open": { "type": "boolean", "description": "Whether it is open now; false only with history." },
        }))),
    }))
}

/// Every query, in the order the MCP server lists its tools. A new one is added at the end of
/// its own line, and to the fixture.
pub static CATALOGUE: [Query; 6] = [
    Query {
        name: "todo_list",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        widens: &[("done", "the done todos, not only the open ones")],
        result: todos,
    },
    Query {
        name: "memory_search",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        widens: &[],
        result: memories,
    },
    Query {
        name: "session_record_list",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        widens: &[],
        result: session_records,
    },
    Query {
        name: "session_record_read",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        widens: &[],
        result: session_record,
    },
    Query {
        name: "change_status",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        widens: &[],
        result: changes,
    },
    // Answered by the app, which alone holds the live chats (#6235); `answer::ask` has none.
    Query {
        name: crate::chattools::CHATS,
        since: "0.5.2",
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        widens: &[(
            "history",
            "the workspace's ended tasks and handoffs, not only its open chats",
        )],
        result: chats,
    },
];

/// **The query a page or a caller names by `id`**, and the warning the bar shows for it: an id
/// that is deprecated, or one a newer id replaces, still answers (ADR 0122 §7).
pub fn find(id: &str) -> Option<(&'static Query, Option<String>)> {
    if let Some(query) = CATALOGUE.iter().find(|q| q.id() == id) {
        let warning = query
            .deprecated
            .map(|why| format!("Calls {id}, which is deprecated: {why}"));
        return Some((query, warning));
    }
    CATALOGUE
        .iter()
        .find(|q| q.replaces == Some(id))
        .map(|query| {
            let warning = format!("Calls {id}, which {} replaces", query.id());
            (query, Some(warning))
        })
}

/// The read-only tools that are no query, each with why.
pub const NOT_QUERIES: [(&str, &str); 20] = [
    (
        crate::chattools::QUERIES,
        "it lists the catalogue for a page's author, and a page declares what it calls",
    ),
    (
        crate::chattools::PERSONA_WHERE,
        "it is about the chat that asks, and a page is no chat",
    ),
    (
        crate::chattools::DISPATCH_LIST,
        "it lists the asking chat's own tasks, and a page is no chat",
    ),
    (
        crate::chattools::DISPATCH_READ,
        "it reads one of the asking chat's own tasks, and a page is no chat",
    ),
    (
        crate::chattools::DISPATCH_WAIT,
        "it waits on one of the asking chat's own tasks, and a page is no chat",
    ),
    (
        crate::chattools::ASK_OPERATOR,
        "it asks the person, which a page never does",
    ),
    ("browser_snapshot", "it reads the asking chat's own browser"),
    ("browser_find", "it reads the asking chat's own browser"),
    (
        "browser_wait_for_text",
        "it waits on the asking chat's own browser",
    ),
    (
        "browser_pages",
        "it lists the asking chat's own browser's pages",
    ),
    (
        "browser_screenshot",
        "it pictures the asking chat's own browser",
    ),
    (
        "browser_downloads",
        "it lists the asking chat's own browser's downloads",
    ),
    (
        crate::themepreview::TOOL,
        "the app draws its picture for a chat",
    ),
    (
        crate::skilltools::SKILL_LIST,
        "it lists the asking chat's own skill set",
    ),
    (
        crate::skilltools::SKILL_RUNS,
        "it lists the runs of the asking chat's own skills",
    ),
    (
        crate::skilltools::SKILL_FIND,
        "it searches the asking chat's own skill set",
    ),
    (
        crate::skilltools::SKILL_LOAD,
        "it loads a skill into the asking chat and opens its run",
    ),
    (
        crate::artifact::tools::LIST,
        "it lists the pins for the author of a page, and a page reads its own data, not its pin",
    ),
    (
        crate::artifact::tools::CHECK,
        "it checks a page for its author before it is approved, and a page does not check itself",
    ),
    (
        crate::artifact::tools::APPROVE_STATUS,
        "it says what this machine approved, which is the person's and never a page's to read",
    ),
];

/// Why a query gave no data. Exactly these six (ADR 0122 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The query is an extension's, and that extension is not installed here.
    NeedsExtension,
    /// The page did not declare it, or its approval does not cover it.
    NotApproved,
    Timeout,
    /// The answer is over its size cap.
    TooLarge,
    /// purlis refused what it was asked, such as a parameter out of its type.
    Refused,
    Failed,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 6] = [
        ErrorKind::NeedsExtension,
        ErrorKind::NotApproved,
        ErrorKind::Timeout,
        ErrorKind::TooLarge,
        ErrorKind::Refused,
        ErrorKind::Failed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::NeedsExtension => "needs-extension",
            ErrorKind::NotApproved => "not-approved",
            ErrorKind::Timeout => "timeout",
            ErrorKind::TooLarge => "too-large",
            ErrorKind::Refused => "refused",
            ErrorKind::Failed => "failed",
        }
    }
}

/// **Every answer a query gives**: its data or why there is none, when it was answered, and
/// where the next page of it starts when there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub outcome: Result<Value, ErrorKind>,
    pub at: chrono::DateTime<chrono::Utc>,
    pub cursor: Option<String>,
}

impl Answer {
    pub fn data(data: Value, at: chrono::DateTime<chrono::Utc>) -> Self {
        Answer {
            outcome: Ok(data),
            at,
            cursor: None,
        }
    }

    pub fn error(kind: ErrorKind, at: chrono::DateTime<chrono::Utc>) -> Self {
        Answer {
            outcome: Err(kind),
            at,
            cursor: None,
        }
    }

    #[must_use]
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// `{ data | error: { kind }, at, cursor? }`.
    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        match &self.outcome {
            Ok(data) => out.insert("data".into(), data.clone()),
            Err(kind) => out.insert("error".into(), json!({ "kind": kind.as_str() })),
        };
        out.insert(
            "at".into(),
            json!(self.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        );
        if let Some(cursor) = &self.cursor {
            out.insert("cursor".into(), json!(cursor));
        }
        Value::Object(out)
    }
}

/// The catalogue as `purlis queries --json` prints it: the API version under its key, and every
/// query.
pub fn listing() -> Value {
    json!({
        PROTOCOL_KEY: API,
        "queries": CATALOGUE.iter().map(Query::listed).collect::<Vec<_>>(),
    })
}

pub mod answer;
pub mod chats;

#[cfg(test)]
mod tests;

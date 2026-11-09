// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one tool catalogue** (ADR 0120 §7, #5951): every tool purlis's MCP server can list, and
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
        out.insert("result".into(), (self.result)());
        Value::Object(out)
    }
}

/// The release the first queries answer in.
const FIRST: &str = "0.5.1";

/// An answer that is the text the tool gives a chat.
fn text() -> Value {
    json!({ "type": "string", "description": "The tool's answer, as a chat reads it." })
}

/// Every query, in the order the MCP server lists its tools. A new one is added at the end of
/// its own line, and to the fixture.
pub static CATALOGUE: [Query; 5] = [
    Query {
        name: "todo_list",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        result: text,
    },
    Query {
        name: "memory_search",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        result: text,
    },
    Query {
        name: "session_record_list",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        result: text,
    },
    Query {
        name: "session_record_read",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        result: text,
    },
    Query {
        name: "change_status",
        since: FIRST,
        replaces: None,
        deprecated: None,
        scope: Scope::Workspace,
        result: text,
    },
];

/// The read-only tools that are no query, each with why.
pub const NOT_QUERIES: [(&str, &str); 16] = [
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
];

/// Why a query gave no data. Exactly these six (ADR 0120 §7).
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

#[cfg(test)]
mod tests;

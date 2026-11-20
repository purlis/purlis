// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The old ways a chat was handed a forge token, which the person's sign-in replaces** (ADR 0128,
//! "Migration"). With the `forge-wiring` feature on, every chat reaches its forge through the
//! app's proxy as the person, and holds no token, so two kinds of tool line are no longer read:
//!
//! - `mint = "github-app"`: a token an organisation's GitHub App mints for the chat ([`mint`]).
//! - a vault value handed in a forge's own token variable (`GH_TOKEN`, `GITLAB_TOKEN`, …): the
//!   per-chat GitLab token route of #7230 and any like it.
//!
//! The lines **keep parsing** here: [`kind_of`] reads a tool's table and says which of the two it
//! is, so a project written for an older build loads, the tool is left out with a sentence that
//! says what to do ([`sentence`]), and `purlis doctor` counts the lines ([`found`]). Nothing here
//! rewrites the project file or touches a vault item or a keyring entry: a person's own secret is
//! deleted only on their press.

use std::path::Path;

use super::mint;

/// The variables a forge's own CLI reads a token from, and a CI job is handed one in. A tool that
/// hands a vault value in one of these is the vault-token route.
const TOKEN_VARS: [&str; 7] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GITLAB_TOKEN",
    "GITLAB_ACCESS_TOKEN",
    "CI_JOB_TOKEN",
];

/// The prefix of the variables a token connection is read from (ADR 0128 D9.2): never handed to a
/// chat either.
const CONNECTION_PREFIX: &str = "PURLIS_FORGE_TOKEN";

/// Which old route a tool line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `mint = "github-app"`: a token minted from an organisation's GitHub App.
    GithubApp,
    /// A vault value handed in a forge token variable.
    VaultToken,
}

/// **Whether `var` is a variable a forge token is handed in**, in any case.
pub fn token_var(var: &str) -> bool {
    let upper = var.trim().to_ascii_uppercase();
    TOKEN_VARS.contains(&upper.as_str()) || upper.starts_with(CONNECTION_PREFIX)
}

/// **Which old route a tool's table is**, or `None` for a tool of any other kind. It reads the
/// table whole and as written, so a line with a fault of its own is still known.
pub fn kind_of(tool: &toml::map::Map<String, toml::Value>) -> Option<Kind> {
    if tool.contains_key(mint::KEY) || tool.contains_key(mint::APP_KEY) {
        return Some(Kind::GithubApp);
    }
    let as_var = tool
        .get("as")
        .and_then(toml::Value::as_str)
        .and_then(|text| text.split_whitespace().next())
        .is_some_and(token_var);
    let values_var = tool
        .get(super::VALUES_KEY)
        .and_then(toml::Value::as_table)
        .is_some_and(|values| values.keys().any(|var| token_var(var)));
    (as_var || values_var).then_some(Kind::VaultToken)
}

/// **What a tool of `kind` says when it is not run**, as the clause after its own name, which says
/// what happened and what to do. The step is the sign-in; the line is the person's to delete.
pub fn sentence(kind: Kind) -> &'static str {
    match kind {
        Kind::GithubApp => {
            "asks for a GitHub App token that purlis no longer mints, so it runs nothing: chats \
             use your sign-in. Sign in under Settings › Forges, then delete this line"
        }
        Kind::VaultToken => {
            "hands a forge token from a vault to a chat, which purlis no longer does, so it runs \
             nothing: chats use your sign-in. Sign in under Settings › Forges, then delete this \
             line (the vault item stays until you delete it)"
        }
    }
}

/// One old line found in a project file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub persona: String,
    pub tool: String,
    pub kind: Kind,
}

/// **Every old line of the project file `cfg`**, by persona then tool.
pub fn found_in(cfg: &toml::Table) -> Vec<Found> {
    let personas = cfg
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("personas"))
        .and_then(toml::Value::as_table);
    let mut out = Vec::new();
    for (persona, grants) in personas.into_iter().flatten() {
        let tools = grants.get(super::KEY).and_then(toml::Value::as_table);
        for (tool, table) in tools.into_iter().flatten() {
            if let Some(kind) = table.as_table().and_then(kind_of) {
                out.push(Found {
                    persona: persona.clone(),
                    tool: tool.clone(),
                    kind,
                });
            }
        }
    }
    out
}

/// **Every old line of the project at `root`**, read from its committed file now: none where
/// there is no file or it does not read.
pub fn found(root: &Path) -> Vec<Found> {
    std::fs::read_to_string(crate::names::manifest(root))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .map(|cfg| found_in(&cfg))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;

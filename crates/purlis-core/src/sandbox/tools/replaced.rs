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

/// The prefixes a forge's own tokens carry: GitHub's (`ghp_` classic, `gho_` OAuth, `ghu_` user,
/// `ghs_` installation, `github_pat_` fine-grained) and GitLab's (`glpat-` personal or project,
/// `glcbt-` CI build, `glrt-` runner).
const TOKEN_PREFIXES: [&str; 8] = [
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "github_pat_",
    "glpat-",
    "glcbt-",
    "glrt-",
];

/// **Whether `value` is a forge token**, by its documented prefix.
pub fn forge_token_value(value: &str) -> bool {
    let value = value.trim_start();
    TOKEN_PREFIXES
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// **Whether a vault item called `key` is one purlis recorded as a forge token**: what Agent
/// identity keeps under its reserved prefix ([`mint::RESERVED_PREFIX`]) and what GitLab access
/// keeps for a host ([`crate::forge::gitlabaccess::key_of`]).
pub fn forge_item_name(key: &str) -> bool {
    let own = crate::forge::gitlabaccess::key_of("");
    mint::reserved(key)
        || key
            .get(..own.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(&own))
}

/// What a resolved tool whose program is `gh` or `glab` says when it is left out, as the clause
/// after its name.
pub const FORGE_PROGRAM_SAID: &str = "runs gh or glab with a value from a vault, which purlis no \
     longer does: chats use your sign-in. Sign in under Settings › Forges";

/// What Agent identity's and GitLab access's setup answers with once the sign-in replaces them:
/// neither writes a line or stores a key any more.
pub const SETUP_REFUSED: &str = "purlis no longer sets up a token for a chat: chats use your \
     sign-in. Sign in under Settings › Forges";

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
    /// A tool whose program is `gh` or `glab`, whatever variable its values name: a chat's forge
    /// CLI takes no value from a vault.
    ForgeProgram,
}

/// **Whether a tool called `name` is a forge CLI's**: its name is `gh` or `glab`, as a path's last
/// part, in any case.
pub fn forge_name(name: &str) -> bool {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    ["gh", "glab"]
        .iter()
        .any(|program| last.eq_ignore_ascii_case(program))
}

/// **Which old route the tool `name` with table `tool` is**: [`kind_of`], or a forge CLI by its
/// name.
pub fn kind_named(name: &str, tool: &toml::map::Map<String, toml::Value>) -> Option<Kind> {
    kind_of(tool).or_else(|| forge_name(name).then_some(Kind::ForgeProgram))
}

/// **Whether `var` is a variable a forge token is handed in**, in any case.
pub fn token_var(var: &str) -> bool {
    let upper = var.trim().to_ascii_uppercase();
    TOKEN_VARS.contains(&upper.as_str())
        || crate::forge::cli::EVERY_TOKEN_ENV.contains(&upper.as_str())
        || upper.starts_with(CONNECTION_PREFIX)
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
             use your sign-in. Sign in under Settings › Forges. While the App's key is in a vault, \
             keep this line, since it is what stops purlis handing that key to a chat; remove \
             the key from the vault on your press, then delete the line"
        }
        Kind::ForgeProgram => {
            "runs gh or glab with a value from a vault, which purlis no longer does, so it runs \
             nothing: chats use your sign-in. Sign in under Settings › Forges, then delete this \
             line (the vault item stays until you delete it)"
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
            if let Some(kind) = table.as_table().and_then(|table| kind_named(tool, table)) {
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

/// **What the window says once about an old project** (ADR 0128, "Migration"): its sentence, and
/// a cause key that names this project's old lines, so a Notice carrying it is dismissed once and
/// stays dismissed until the lines change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    pub cause: String,
    pub text: String,
}

/// **The one Notice an old project's lines raise**, from what [`found`] reads at `root`: `None`
/// where it has none. Nothing is written, so the way out is the Notice's own Dismiss.
pub fn told(root: &Path) -> Option<Told> {
    told_of(&found(root))
}

fn told_of(found: &[Found]) -> Option<Told> {
    if found.is_empty() {
        return None;
    }
    // FNV-1a over each line's persona and tool, so the key is the same on every run and moves
    // only when the lines do.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for one in found {
        for byte in one
            .persona
            .bytes()
            .chain([0])
            .chain(one.tool.bytes())
            .chain([0])
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    let lines = if found.len() == 1 {
        "1 tool line".to_owned()
    } else {
        format!("{} tool lines", found.len())
    };
    Some(Told {
        cause: format!("forge-sign-in-replaces:{hash:016x}"),
        text: format!(
            "This project has {lines} that hand a chat a forge token, which purlis no longer \
             does: chats use your sign-in. Sign in under Settings › Forges, then delete the \
             lines. Keep a `mint` line while its GitHub App's key is in a vault: it stops purlis \
             handing that key to a chat. Nothing was deleted for you."
        ),
    })
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A GitHub App's token** (#2053, SD-7a; the person's ruling of 2026-10-29, E1 to E3): a
//! persona's tool whose program is handed a token the organisation's own GitHub App mints for
//! the chat, and never the App's key.
//!
//! ```toml
//! [sandbox.personas.dev]
//! tools = { gh = { vault = "forge", mint = "github-app", app = "Iv23liExample",
//!                  key = "app-key" } }
//! ```
//!
//! `mint` makes the tool this kind, and `"github-app"` is the one value it takes. `app` is the
//! App's client id, which is public and is the JWT's issuer. `key` is the vault's key holding the
//! App's private key. The program is handed the minted token in [`VAR`], as `env`.
//!
//! **The key is never handed over raw** (E2): a key a `mint` line names, in that line's vault,
//! is [`marked`], on any persona and whether or not the line is otherwise taken, and every read of
//! a vault value but the mint's refuses it ([`raw`], [`crate::secrets::cmd::get_value`]).
//! The mint itself is [`crate::forge::apptoken`].

use std::path::Path;

/// The key that makes a tool one whose token is minted.
pub const KEY: &str = "mint";

/// The key of the App's client id.
pub const APP_KEY: &str = "app";

/// The one kind of mint read: a GitHub App's installation token.
pub const GITHUB_APP: &str = "github-app";

/// The variable the minted token is handed in, as `env`: the one `gh` reads.
pub const VAR: &str = "GH_TOKEN";

/// **The vault keys that hold a GitHub App's key before any line names it** (#4485): Agent
/// identity keeps what GitHub hands it under this prefix, so the key is marked from the moment
/// it is stored, with no window between the vault and the tool line. Stateless, so it holds with
/// the project file unreadable and over a restart.
pub const RESERVED_PREFIX: &str = "agent-identity-github-";

/// **Whether `key` is under [`RESERVED_PREFIX`]**, in any case, since some stores fold case.
pub fn reserved(key: &str) -> bool {
    key.get(..RESERVED_PREFIX.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(RESERVED_PREFIX))
}

/// What a write or delete under [`RESERVED_PREFIX`] is refused with, but Agent identity's own.
pub fn reserved_write_said(key: &str) -> String {
    format!("'{key}' is a name purlis keeps for Agent identity, so only that tab changes it")
}

/// The longest client id read.
const MOST_APP: usize = 64;

/// **A GitHub App a tool's token is minted from**: its client id. Its key is the tool's one
/// value ([`super::Tool::values`]), handed in [`VAR`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mint {
    pub app: String,
}

/// **A minting tool's App**, as its table writes it, or why the tool is not one. `text` reads a
/// key's trimmed text. Refused: a kind other than [`GITHUB_APP`], no client id or one that is not
/// letters, digits, `.`, `_` and `-` (at most [`MOST_APP`]), and any of `as`, `values`, `browser`
/// or `user`, since the token's hand-over is the mint's own.
pub fn of(
    tool: &toml::map::Map<String, toml::Value>,
    text: &dyn Fn(&str) -> Option<String>,
) -> Result<Mint, String> {
    if let Some(other) = [
        "as",
        super::VALUES_KEY,
        super::login::KEY,
        super::login::USER_KEY,
    ]
    .into_iter()
    .find(|key| tool.contains_key(*key))
    {
        return Err(format!(
            "a GitHub App's token is handed in {VAR} by purlis, so the tool takes no {other} — \
             write vault, mint = \"{GITHUB_APP}\", app and key"
        ));
    }
    match text(KEY).as_deref() {
        Some(GITHUB_APP) => {}
        Some(other) => {
            return Err(format!(
                "mint = {other:?} is not a kind purlis mints — mint = \"{GITHUB_APP}\""
            ));
        }
        None => return Err(format!("mint is not a kind — mint = \"{GITHUB_APP}\"")),
    }
    let Some(app) = text(APP_KEY) else {
        return Err("app names no GitHub App — app = \"<the App's client id>\"".to_owned());
    };
    let named = app.len() <= MOST_APP
        && app
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !named {
        return Err(format!(
            "app = {app:?} is not a GitHub App's client id — letters, digits, '.', '_' and '-'"
        ));
    }
    Ok(Mint { app })
}

/// **Every vault key a `mint` line of the project file `cfg` names**, as `(vault, key)`: each
/// tool of each persona whose table holds `mint`, whatever else it holds or lacks, so a line with
/// a fault still marks its key.
pub fn marked_in(cfg: &toml::Table) -> Vec<(String, String)> {
    let personas = cfg
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("personas"))
        .and_then(toml::Value::as_table);
    let mut out: Vec<(String, String)> = personas
        .into_iter()
        .flat_map(|personas| personas.values())
        .filter_map(|persona| persona.get(super::KEY)?.as_table())
        .flat_map(|tools| tools.values())
        .filter_map(toml::Value::as_table)
        .filter(|tool| tool.contains_key(KEY))
        .filter_map(|tool| {
            let text = |key: &str| Some(tool.get(key)?.as_str()?.trim().to_owned());
            Some((text("vault")?, text("key")?))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// **Every vault key a `mint` line of the project at `root` names** ([`marked_in`]), read from
/// its committed file now: none where there is no file, and `None` where there is one that
/// cannot be read or does not parse, so nothing is known to be unmarked.
pub fn marked(root: &Path) -> Option<Vec<(String, String)>> {
    match std::fs::read_to_string(crate::names::manifest(root)) {
        Ok(text) => text.parse::<toml::Table>().ok().map(|cfg| marked_in(&cfg)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

/// **How a raw read of key `key` of vault `vault` is taken** in the project at `root` (E2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Raw {
    /// No `mint` line names it.
    Taken,
    /// A `mint` line names it: why it is never handed over.
    Refused(String),
    /// The project file cannot be read, so no line is known: the value is handed over only if it
    /// is no private key ([`a_private_key`]).
    NotIfAKey,
}

/// [`Raw`] for key `key` of vault `vault` in the project at `root`.
pub fn raw(root: &Path, vault: &str, key: &str) -> Raw {
    if reserved(key) {
        return Raw::Refused(refused_said(vault, key));
    }
    match marked(root) {
        None => Raw::NotIfAKey,
        Some(marks) if marks.iter().any(|(v, k)| v == vault && k == key) => {
            Raw::Refused(refused_said(vault, key))
        }
        Some(_) => Raw::Taken,
    }
}

/// Whether `value` holds a PEM private key, of any kind.
pub fn a_private_key(value: &str) -> bool {
    value.lines().any(|line| {
        let line = line.trim();
        line.starts_with("-----BEGIN ") && line.ends_with("PRIVATE KEY-----")
    })
}

/// What a raw read of a private key is refused with while the project file cannot be read.
pub fn unreadable_said() -> String {
    "the project's settings file cannot be read, so purlis cannot tell whether this private key \
     is one a persona's tool mints GitHub App tokens with, and hands it over to nothing until \
     the file reads again"
        .to_owned()
}

/// What a raw read of a marked key is refused with.
pub fn refused_said(vault: &str, key: &str) -> String {
    format!(
        "'{key}' in vault '{vault}' is a GitHub App's key that a persona's tool mints tokens \
         with, so purlis never hands it over: a chat's gh is handed a token minted from it"
    )
}

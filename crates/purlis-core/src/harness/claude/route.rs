// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Claude Code's pin of a key chat's route** (T1a-15; D6640-R1 §4, D6640-R3 §5, design §3.7).
//!
//! A proxied chat's route rides in the one `--settings` value the adapter already writes, merged
//! into its one `env` object beside the skill listing limit (#4610): never a second `env`, never a
//! second `--settings`. A `--settings` `env` outranks the person's own settings files and the
//! launch environment (measured on 2.1.325 in T1a-02, `work/any-model/spikes/1a-startup.md`), so
//! what it names is what the chat runs on. It carries **every variable the adapter sets but the
//! token** (R-232, R-289):
//!
//! - `ANTHROPIC_BASE_URL`, the run's listener on `127.0.0.1`;
//! - **one model slot, the background model's**: `ANTHROPIC_DEFAULT_HAIKU_MODEL`, the provider's
//!   `[models] background` (R-293). It is purlis's: written here, it outranks a value the person
//!   exported for the same variable, and Claude Code's `/model` shows a pinned Haiku row for it
//!   (C-8). Every other slot is left to the person and Claude Code: `ANTHROPIC_MODEL` is never set
//!   in 1a, since it outranks the person's own `model` setting, and nor is any other
//!   `ANTHROPIC_DEFAULT_*_MODEL` or `ANTHROPIC_SMALL_FAST_MODEL` (spec V3);
//! - an explicit empty value for every member of the [`STRIP_FLOOR`] but the base URL, since an
//!   omitted key keeps a lower file's value, the seven provider switches among them
//!   ([`BLANKED_SWITCHES`]);
//! - `NO_PROXY` and `no_proxy`, both the person's two lists merged, each host once, with
//!   `127.0.0.1` added once (not `localhost`, which doesn't cover it), so a proxy variable never
//!   sends a model call away from the listener, and neither spelling outranks the other.
//!
//! **The token is not here.** It is `ANTHROPIC_AUTH_TOKEN` in the process environment alone
//! (R-066, R-256), never in argv, `--settings` or a kept file, and it ranks above `apiKeyHelper`.
//! `ANTHROPIC_API_KEY` is blanked, so Claude Code never falls back to a saved claude.ai login.
//!
//! **Only a proxied chat is pinned** (R-252): a chat with no route, a person's own cloud route
//! among them, has none of this written, and its environment is untouched.
//!
//! **Discovery stays off** (R-301): [`DISCOVERY`] is on no list here, and nothing sets it.

use std::collections::BTreeSet;

use crate::keyroute::KeyRoute;

/// The token's variable: in the process environment alone, never in `--settings` (R-066).
pub const TOKEN: &str = "ANTHROPIC_AUTH_TOKEN";

/// The base URL's variable, set to the listener rather than blanked (H-17).
pub const BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// The one model slot purlis sets in 1a, the background model's (spec V3).
pub const BACKGROUND_SLOT: &str = "ANTHROPIC_DEFAULT_HAIKU_MODEL";

/// Claude Code's model discovery behind a custom base URL, which purlis never turns on (R-301).
pub const DISCOVERY: &str = "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY";

/// The loopback the listener is on, which the bypass names.
pub const LOOPBACK: &str = "127.0.0.1";

/// The proxy bypass variables, both cases, which Claude Code reads alike (T1a-02).
pub const BYPASS: [&str; 2] = ["NO_PROXY", "no_proxy"];

/// **The strip floor** (R-248): each variable a proxied Claude Code chat carries empty in its
/// `--settings` `env`, but the base URL, which it sets. Declarations can only add to it. The
/// `ANTHROPIC_GOOGLE_CLOUD_*` members are the five T1a-02 listed from the pinned binary (spec
/// C-40), which [`google_cloud_names_not_in_the_floor`] holds the pin to. The token's own
/// variable is never in it.
pub const STRIP_FLOOR: &[&str] = &[
    BASE_URL,
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_CUSTOM_HEADERS",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "ANTHROPIC_PROFILE",
    "ANTHROPIC_BEDROCK_BASE_URL",
    "ANTHROPIC_VERTEX_BASE_URL",
    "ANTHROPIC_FOUNDRY_BASE_URL",
    "ANTHROPIC_FOUNDRY_API_KEY",
    "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
    "ANTHROPIC_FOUNDRY_RESOURCE",
    "ANTHROPIC_AWS_BASE_URL",
    "ANTHROPIC_BEDROCK_MANTLE_BASE_URL",
    "AWS_BEARER_TOKEN_BEDROCK",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_USE_MANTLE",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
    "CLAUDE_CODE_USE_GATEWAY",
    "ANTHROPIC_FEDERATION_RULE_ID",
    "ANTHROPIC_ORGANIZATION_ID",
    "ANTHROPIC_IDENTITY_TOKEN_FILE",
    "ANTHROPIC_BETAS",
    "CLAUDE_CODE_EXTRA_BODY",
    "ANTHROPIC_AWS_API_KEY",
    "ANTHROPIC_AWS_WORKSPACE_ID",
    "ANTHROPIC_GOOGLE_CLOUD_AUTH",
    "ANTHROPIC_GOOGLE_CLOUD_BASE_URL",
    "ANTHROPIC_GOOGLE_CLOUD_LOCATION",
    "ANTHROPIC_GOOGLE_CLOUD_PROJECT",
    "ANTHROPIC_GOOGLE_CLOUD_WORKSPACE_ID",
    "AWS_ENDPOINT_URL_BEDROCK_RUNTIME",
];

/// **What a key chat's process is started without** (T1a-15 fix round): the variables of the
/// floor (R-248, through "the additions T1a-02 found", `spikes/1a-startup.md`) that Claude Code
/// reads from its launch environment alone, so a key chat's harness never carries them, whoever
/// passed them (`hoststart::started_without`). An unproxied chat keeps the person's value.
///
/// **Found at build: `ANTHROPIC_UNIX_SOCKET` is not settable through `--settings` `env`.** Run on
/// 2.1.300 with a key chat's exact `--settings` and the run token, a socket path in the launch
/// environment sent every model call and the token to the socket, around the listener, and
/// `"ANTHROPIC_UNIX_SOCKET": ""` in `--settings` `env` changed nothing: Claude Code reads it
/// from the launch environment only. So it is not in [`STRIP_FLOOR`], whose members are blanked
/// in `--settings` `env`; it is kept out of the process instead, and an empty value written there
/// would prove nothing about it.
pub const LAUNCH_ONLY: [&str; 1] = ["ANTHROPIC_UNIX_SOCKET"];

/// **The provider switches a proxied chat blanks** (R-248, R-252, X8), each with why: every
/// `CLAUDE_CODE_USE_` name in the pinned binary that routes a model call is here, or in
/// [`KNOWN_NOT_A_ROUTE`] ([`switches_in_neither_list`]).
pub const BLANKED_SWITCHES: [(&str, &str); 7] = [
    (
        "CLAUDE_CODE_USE_BEDROCK",
        "sends model calls to Amazon Bedrock instead of the base URL",
    ),
    (
        "CLAUDE_CODE_USE_VERTEX",
        "sends model calls to Google Vertex AI instead of the base URL",
    ),
    (
        "CLAUDE_CODE_USE_FOUNDRY",
        "sends model calls to Microsoft Foundry instead of the base URL",
    ),
    (
        "CLAUDE_CODE_USE_MANTLE",
        "sends model calls to Bedrock's Mantle endpoint instead of the base URL",
    ),
    (
        "CLAUDE_CODE_USE_ANTHROPIC_AWS",
        "sends model calls to Claude Platform on AWS instead of the base URL",
    ),
    (
        "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
        "sends model calls to Claude Models on Google Cloud instead of the base URL",
    ),
    (
        "CLAUDE_CODE_USE_GATEWAY",
        "beside a base URL and a token, runs the session in Claude apps gateway mode, with its \
         own default model (2.1.325, X8)",
    ),
];

/// **The `CLAUDE_CODE_USE_` names in the pinned binary that route no model call** (X8), each
/// with why, as read from its strings at 2.1.300. A name in neither list is a decision someone
/// has to make, so the guard fails on it.
pub const KNOWN_NOT_A_ROUTE: &[(&str, &str)] = &[
    (
        "CLAUDE_CODE_USE_CCR_V2",
        "the transport of a remote session Claude Code's own server spawns; it moves no model call",
    ),
    (
        "CLAUDE_CODE_USE_COWORK_PLUGINS",
        "where plugins are read from; a source of plugins, not of model calls",
    ),
    (
        "CLAUDE_CODE_USE_NATIVE_FILE_SEARCH",
        "which file search the tools use; it moves no model call",
    ),
    (
        "CLAUDE_CODE_USE_POWERSHELL_TOOL",
        "turns on the PowerShell tool; it moves no model call",
    ),
];

/// The prefix of the provider switches.
pub const SWITCH_PREFIX: &str = "CLAUDE_CODE_USE_";

/// The prefix of the Claude Models on Google Cloud variables (spec C-40).
pub const GOOGLE_CLOUD_PREFIX: &str = "ANTHROPIC_GOOGLE_CLOUD_";

/// **What a proxied chat's `--settings` `env` carries** for `route`, besides what the adapter
/// writes there for every chat: the route's variables, never the token.
pub fn settings_env(route: &KeyRoute) -> serde_json::Map<String, serde_json::Value> {
    let mut env: serde_json::Map<String, serde_json::Value> = STRIP_FLOOR
        .iter()
        .map(|name| ((*name).to_owned(), "".into()))
        .collect();
    env.insert(BASE_URL.to_owned(), route.base_url().into());
    if let Some(model) = route.background_model() {
        env.insert(BACKGROUND_SLOT.to_owned(), model.into());
    }
    let bypass = with_loopback(route.bypass());
    for name in BYPASS {
        env.insert(name.to_owned(), bypass.clone().into());
    }
    env
}

/// **The one bypass both spellings are written with**: every host of the person's `theirs`
/// (`NO_PROXY`, then `no_proxy`), each once, then [`LOOPBACK`] where neither names it. Both
/// spellings carry the same list, so one purlis writes never outranks a list the person keeps
/// in the other.
fn with_loopback(theirs: [Option<&str>; 2]) -> String {
    let mut hosts: Vec<&str> = Vec::new();
    for host in theirs
        .into_iter()
        .flatten()
        .flat_map(|list| list.split(','))
    {
        let host = host.trim();
        if !host.is_empty() && !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    if !hosts.contains(&LOOPBACK) {
        hosts.push(LOOPBACK);
    }
    hosts.join(",")
}

/// Each environment variable name in `binary` that starts with `prefix`: the prefix, then
/// capitals, digits and `_`, standing alone (no name character before it).
pub fn names_in(binary: &[u8], prefix: &str) -> BTreeSet<String> {
    let named = |byte: u8| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_';
    memchr::memmem::find_iter(binary, prefix.as_bytes())
        .filter(|&at| {
            at == 0 || {
                let before = binary[at - 1];
                !(named(before) || before.is_ascii_lowercase())
            }
        })
        .filter_map(|at| {
            let rest = &binary[at + prefix.len()..];
            let tail = rest.iter().take_while(|byte| named(**byte)).count();
            (tail > 0).then(|| {
                String::from_utf8_lossy(&binary[at..at + prefix.len() + tail]).into_owned()
            })
        })
        .collect()
}

/// The `ANTHROPIC_GOOGLE_CLOUD_` names in `binary` the floor doesn't list.
pub fn google_cloud_names_not_in_the_floor(binary: &[u8]) -> Vec<String> {
    names_in(binary, GOOGLE_CLOUD_PREFIX)
        .into_iter()
        .filter(|name| !STRIP_FLOOR.contains(&name.as_str()))
        .collect()
}

/// The `CLAUDE_CODE_USE_` names in `binary` in neither [`BLANKED_SWITCHES`] nor
/// [`KNOWN_NOT_A_ROUTE`].
pub fn switches_in_neither_list(binary: &[u8]) -> Vec<String> {
    names_in(binary, SWITCH_PREFIX)
        .into_iter()
        .filter(|name| {
            !BLANKED_SWITCHES
                .iter()
                .chain(KNOWN_NOT_A_ROUTE)
                .any(|(listed, _)| listed == name)
        })
        .collect()
}

#[cfg(test)]
mod tests;

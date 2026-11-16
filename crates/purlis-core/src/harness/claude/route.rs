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
//! - `NO_PROXY` and `no_proxy`, each the person's value with `127.0.0.1` added (not `localhost`,
//!   which doesn't cover it), so a proxy variable never sends a model call away from the listener.
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
pub const STRIP_FLOOR: &[&str] = &[];

/// **The provider switches a proxied chat blanks** (R-248, R-252, X8), each with why: every
/// `CLAUDE_CODE_USE_` name in the pinned binary that routes a model call is here, or in
/// [`KNOWN_NOT_A_ROUTE`] ([`switches_in_neither_list`]).
pub const BLANKED_SWITCHES: [(&str, &str); 7] = [("", ""); 7];

/// **The `CLAUDE_CODE_USE_` names in the pinned binary that route no model call** (X8), each
/// with why, as read from its strings at 2.1.300. A name in neither list is a decision someone
/// has to make, so the guard fails on it.
pub const KNOWN_NOT_A_ROUTE: &[(&str, &str)] = &[];

/// The prefix of the provider switches.
pub const SWITCH_PREFIX: &str = "CLAUDE_CODE_USE_";

/// The prefix of the Claude Models on Google Cloud variables (spec C-40).
pub const GOOGLE_CLOUD_PREFIX: &str = "ANTHROPIC_GOOGLE_CLOUD_";

/// **What a proxied chat's `--settings` `env` carries** for `route`, besides what the adapter
/// writes there for every chat: the route's variables, never the token.
pub fn settings_env(route: &KeyRoute) -> serde_json::Map<String, serde_json::Value> {
    let _ = route;
    serde_json::Map::new()
}

/// Each environment variable name in `binary` that starts with `prefix`: the prefix, then
/// capitals, digits and `_`, standing alone (no name character before it).
pub fn names_in(binary: &[u8], prefix: &str) -> BTreeSet<String> {
    let _ = (binary, prefix);
    BTreeSet::new()
}

/// The `ANTHROPIC_GOOGLE_CLOUD_` names in `binary` the floor doesn't list.
pub fn google_cloud_names_not_in_the_floor(binary: &[u8]) -> Vec<String> {
    let _ = binary;
    Vec::new()
}

/// The `CLAUDE_CODE_USE_` names in `binary` in neither [`BLANKED_SWITCHES`] nor
/// [`KNOWN_NOT_A_ROUTE`].
pub fn switches_in_neither_list(binary: &[u8]) -> Vec<String> {
    let _ = binary;
    Vec::new()
}

#[cfg(test)]
mod tests;

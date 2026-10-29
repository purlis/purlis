// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A profile's settings, as they are sent to a runner: with no sign-in in them** (#3448,
//! D-3448-8, the steward's ruling of 2026-10-29).
//!
//! A settings file is sent by its name only where an import would copy it
//! ([`super::made::ALLOWED`]), but a sign-in can sit inside one: Claude Code's `settings.json`
//! applies its `env` block to the harness's own process, where a subscription's token or a key is
//! often set, and its helpers run a program that prints one. ADR 0095 §3 has no sign-in travel
//! between devices, and a vault's values belong in a vault, so every harness's settings file is
//! sent with **every environment block removed, and every setting that names or runs a
//! credential source** ([`without_credentials`]), whatever the variables in it are called. The
//! runner signs in its own way.
//!
//! A settings file that does not parse is refused in words: the harness could not read it either,
//! and purlis cannot say what is in it.

/// The settings a harness's settings file may name a credential source by, or carry one in,
/// removed wherever they are in it: its environment blocks, the helpers that print a key or a
/// cloud's credentials, the headers a request is signed with, and the tokens themselves.
fn dropped(kind: &str) -> &'static [&'static str] {
    match kind {
        // https://code.claude.com/docs/en/settings: `env`, and the helpers that print a key, a
        // cloud's credentials or the telemetry's headers.
        "claude" => &[
            "env",
            "apiKeyHelper",
            "awsAuthRefresh",
            "awsCredentialExport",
            "gcpAuthRefresh",
            "otelHeadersHelper",
            "headers",
        ],
        // Codex's `config.toml`: a server's `env`, a provider's and a server's bearer tokens, the
        // variables that name one, a provider's `env_key` and its command-run `auth`, and the
        // headers a request carries. `shell_environment_policy.set` is removed apart
        // ([`codex_without`]).
        "codex" => &[
            "env",
            "env_key",
            "auth",
            "bearer_token",
            "bearer_token_env_var",
            "experimental_bearer_token",
            "http_headers",
            "env_http_headers",
        ],
        // opencode's `opencode.json`: a server's `environment`, a provider's `apiKey`, and the
        // headers either sends.
        "opencode" => &["env", "environment", "apiKey", "headers"],
        _ => &[],
    }
}

/// **`bytes`, the settings file `path` of a `kind` profile, as it is sent to a runner**: a
/// harness's own settings file with every setting [`dropped`] names removed wherever it is in it,
/// any other file as it is. `Err`, in words, for a settings file that does not parse.
pub fn without_credentials(kind: &str, path: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let unreadable = || {
        format!(
            "its settings file {} is not one its harness can read, so no settings were sent",
            crate::shown::short(path)
        )
    };
    match (kind, path) {
        ("claude", "settings.json") | ("opencode", "opencode.json" | "tui.json") => {
            let text = std::str::from_utf8(bytes).map_err(|_| unreadable())?;
            json_without(kind, text).ok_or_else(unreadable)
        }
        ("opencode", "opencode.jsonc") => {
            let text = std::str::from_utf8(bytes).map_err(|_| unreadable())?;
            json_without(kind, &plain_json(text)).ok_or_else(unreadable)
        }
        ("codex", "config.toml") => {
            let text = std::str::from_utf8(bytes).map_err(|_| unreadable())?;
            codex_without(text).ok_or_else(unreadable)
        }
        _ => Ok(bytes.to_vec()),
    }
}

/// `text`, a JSON settings file of `kind`'s, with every setting [`dropped`] names removed at any
/// depth, written again. `None` where it is not JSON.
fn json_without(kind: &str, text: &str) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_str(text).ok()?;
    drop_json(&mut value, dropped(kind));
    let mut written = serde_json::to_vec_pretty(&value).ok()?;
    written.push(b'\n');
    Some(written)
}

fn drop_json(value: &mut serde_json::Value, names: &[&str]) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|key, _| !names.contains(&key.as_str()));
            for inner in map.values_mut() {
                drop_json(inner, names);
            }
        }
        serde_json::Value::Array(items) => {
            for inner in items {
                drop_json(inner, names);
            }
        }
        _ => {}
    }
}

/// `text`, JSON with comments and trailing commas (opencode's `opencode.jsonc`), as plain JSON:
/// each comment and each comma before a closing bracket left out, strings kept as they are. What
/// is not JSON still is not, and is refused by the parse after.
fn plain_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                while let Some(s) = chars.next() {
                    out.push(s);
                    match s {
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                out.push(escaped);
                            }
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for s in chars.by_ref() {
                    if s == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for s in chars.by_ref() {
                    if last == '*' && s == '/' {
                        break;
                    }
                    last = s;
                }
                out.push(' ');
            }
            ',' => {
                // A comma whose next mark, past blanks, closes an object or an array is left out.
                let next = chars.clone().find(|s| !s.is_whitespace());
                if !matches!(next, Some('}' | ']')) {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// `text`, a Codex `config.toml`, with every setting [`dropped`] names removed at any depth, and
/// the variables `[shell_environment_policy]` sets, written again. `None` where it is not TOML.
fn codex_without(text: &str) -> Option<Vec<u8>> {
    let mut doc: toml_edit::DocumentMut = text.parse().ok()?;
    let names = dropped("codex");
    drop_toml(doc.as_table_mut(), names);
    if let Some(policy) = doc
        .get_mut("shell_environment_policy")
        .and_then(|p| p.as_table_like_mut())
    {
        policy.remove("set");
    }
    Some(doc.to_string().into_bytes())
}

fn drop_toml(table: &mut dyn toml_edit::TableLike, names: &[&str]) {
    let keys: Vec<String> = table.iter().map(|(k, _)| k.to_owned()).collect();
    for key in keys {
        if names.contains(&key.as_str()) {
            table.remove(&key);
            continue;
        }
        if let Some(item) = table.get_mut(&key) {
            drop_toml_item(item, names);
        }
    }
}

fn drop_toml_item(item: &mut toml_edit::Item, names: &[&str]) {
    match item {
        toml_edit::Item::Table(table) => drop_toml(table, names),
        toml_edit::Item::ArrayOfTables(tables) => {
            for table in tables.iter_mut() {
                drop_toml(table, names);
            }
        }
        toml_edit::Item::Value(toml_edit::Value::InlineTable(table)) => drop_toml(table, names),
        toml_edit::Item::Value(toml_edit::Value::Array(items)) => {
            for value in items.iter_mut() {
                if let toml_edit::Value::InlineTable(table) = value {
                    drop_toml(table, names);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;

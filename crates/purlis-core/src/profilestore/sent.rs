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
//!
//! **The rest of what is sent is filtered the same way** (#3866, ADR 0095's 2026-11-01 amendment):
//! an agent's or a skill's frontmatter loses every setting any harness's list names, a URL in any
//! text file loses its user information, and a hook, status line, file suggester or server whose
//! command shows a credential is removed whole. [`filtered`] says what went, by name and never by
//! value. A command is only read as written: what a script it runs holds, purlis cannot see.

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
            "headersHelper",
        ],
        // Codex's `config.toml`: a server's `env`, a provider's and a server's bearer tokens, the
        // variables that name one, a provider's `env_key` and its command-run `auth`, and the
        // headers a request carries, the telemetry exporters' among them.
        // `shell_environment_policy.set` is removed apart ([`codex_without`]).
        "codex" => &[
            "env",
            "headers",
            "env_key",
            "auth",
            "bearer_token",
            "bearer_token_env_var",
            "experimental_bearer_token",
            "http_headers",
            "env_http_headers",
        ],
        // opencode's `opencode.json`: a server's `environment` and its OAuth `clientSecret`, a
        // provider's `apiKey`, the credentials its SDK takes (Bedrock's `accessKeyId`,
        // `secretAccessKey` and `sessionToken`, Vertex's `googleAuthOptions.credentials`), and
        // the headers either sends.
        "opencode" => &[
            "env",
            "environment",
            "apiKey",
            "headers",
            "clientSecret",
            "accessKeyId",
            "secretAccessKey",
            "sessionToken",
            "credentials",
        ],
        _ => &[],
    }
}

/// Whether `path` is a `kind` harness's own settings file, which [`without_credentials`] parses
/// and writes again.
pub fn rewritten(kind: &str, path: &str) -> bool {
    matches!(
        (kind, path),
        ("claude", "settings.json")
            | ("opencode", "opencode.json" | "opencode.jsonc" | "tui.json")
            | ("codex", "config.toml")
    )
}

/// [`filtered`]'s bytes: what a runner checks a file with before it keeps it.
pub fn without_credentials(kind: &str, path: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    filtered(kind, path, bytes).map(|file| file.bytes)
}

/// [`filtered`]'s bytes, with what was removed said in the log, as what an import leaves out is:
/// what a desktop sends.
pub fn sent_without_credentials(kind: &str, path: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let file = filtered(kind, path, bytes)?;
    for what in &file.removed {
        tracing::info!(
            "purlis: a setting is not sent to a runner: {}: {what}",
            crate::shown::short(path)
        );
    }
    Ok(file.bytes)
}

/// A settings file as it is sent, and what was removed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    pub bytes: Vec<u8>,
    /// What went, by where it was and never by its value, each once.
    pub removed: Vec<String>,
}

/// **`bytes`, the settings file `path` of a `kind` profile, as it is sent to a runner**: a
/// harness's own settings file with every setting [`dropped`] names removed wherever it is in it,
/// and any command that shows a credential; a Markdown file with the same taken out of its
/// frontmatter; and any text with no URL's user information. A file with nothing to remove comes
/// back byte for byte, and filtering twice changes nothing, since a runner filters what it was
/// sent again and the two must come to one digest. `Err`, in words, for a settings file that does
/// not parse.
pub fn filtered(kind: &str, path: &str, bytes: &[u8]) -> Result<Filtered, String> {
    let mut removed = Vec::new();
    let bytes = filtered_saying(kind, path, bytes, &mut removed)?;
    let mut seen = std::collections::HashSet::new();
    removed.retain(|what| seen.insert(what.clone()));
    Ok(Filtered { bytes, removed })
}

fn filtered_saying(
    kind: &str,
    path: &str,
    bytes: &[u8],
    said: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    let unreadable = || {
        format!(
            "its settings file {} is not one its harness can read, so no settings were sent",
            crate::shown::short(path)
        )
    };
    match (kind, path) {
        ("claude", "settings.json") => {
            let text = std::str::from_utf8(bytes).map_err(|_| unreadable())?;
            json_without(kind, text, said).ok_or_else(unreadable)
        }
        // opencode reads each of its settings files as JSON with comments.
        ("opencode", "opencode.json" | "opencode.jsonc" | "tui.json") => {
            let text = std::str::from_utf8(bytes).map_err(|_| unreadable())?;
            json_without(kind, &plain_json(text), said).ok_or_else(unreadable)
        }
        ("codex", "config.toml") => {
            let text = std::str::from_utf8(bytes).map_err(|_| unreadable())?;
            codex_without(text, said).ok_or_else(unreadable)
        }
        _ => Ok(match std::str::from_utf8(bytes) {
            Ok(text) if is_markdown(path) => {
                let text = frontmatter_without(text, said);
                no_url_users(&text, said).into_owned().into_bytes()
            }
            Ok(text) => no_url_users(text, said).into_owned().into_bytes(),
            // No text to read a URL in, and rewriting one could only break it.
            Err(_) => bytes.to_vec(),
        }),
    }
}

fn is_markdown(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// What a frontmatter may not carry: whatever any harness's settings file may not, since a
/// frontmatter can declare a server or a hook for any of them.
fn dropped_from_frontmatter() -> Vec<&'static str> {
    crate::profiles::KINDS
        .iter()
        .flat_map(|kind| dropped(kind.word).iter().copied())
        .collect()
}

/// `text`, with its frontmatter as it may be sent. One with nothing to remove and no comment is
/// left byte for byte, and any other written again, one plain `key: value` a line, with no comment,
/// since a commented-out key is a key copied all the same. One purlis cannot read keeps only its
/// `name` and `description` lines, so what it may declare goes and the skill or agent still loads.
fn frontmatter_without(text: &str, said: &mut Vec<String>) -> String {
    let Some((span, read)) = purlis_skills::handed::frontmatter_value(text) else {
        return text.to_owned();
    };
    let block = &text[span.clone()];
    let written = match read {
        Ok(mut value) => {
            let names = dropped_from_frontmatter();
            let before = value.clone();
            clean_json(&mut value, &names, "frontmatter", said);
            let commented = block
                .lines()
                .any(|line| line.trim_start().starts_with('#') || line.contains(" #"));
            if value == before && !commented {
                return text.to_owned();
            }
            frontmatter_written(&value)
        }
        Err(_) => {
            let kept: String = block
                .split_inclusive('\n')
                .filter(|line| line.starts_with("name:") || line.starts_with("description:"))
                .collect();
            if kept != block {
                said.push("frontmatter, but for its name and description: not readable".to_owned());
            }
            kept
        }
    };
    format!("{}{written}{}", &text[..span.start], &text[span.end..])
}

/// A frontmatter's values, one key a line, each value in JSON's notation, which is YAML's too.
fn frontmatter_written(value: &serde_json::Value) -> String {
    let mut out = String::new();
    if let serde_json::Value::Object(map) = value {
        for (key, value) in map {
            let plain = !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if plain {
                out.push_str(key);
            } else {
                out.push_str(&serde_json::Value::String(key.clone()).to_string());
            }
            out.push_str(": ");
            out.push_str(&value.to_string());
            out.push('\n');
        }
    }
    out
}

/// `text` with each URL's user information removed: `https://me:pw@host/x` as `https://host/x`.
fn no_url_users<'a>(text: &'a str, said: &mut Vec<String>) -> std::borrow::Cow<'a, str> {
    let mut out = String::new();
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = text[from..].find("://") {
        let at = from + found;
        let authority = at + 3;
        from = authority;
        let scheme_ok = text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        if !scheme_ok {
            continue;
        }
        let end = text[authority..]
            .find(|c: char| c.is_whitespace() || "/?#\"'<>\\`".contains(c))
            .map_or(text.len(), |n| authority + n);
        if let Some(user_end) = text[authority..end].rfind('@') {
            out.push_str(&text[copied..authority]);
            copied = authority + user_end + 1;
            from = copied;
        }
    }
    if copied == 0 {
        return std::borrow::Cow::Borrowed(text);
    }
    out.push_str(&text[copied..]);
    said.push("a URL's user information".to_owned());
    std::borrow::Cow::Owned(out)
}

/// Whether a command, as `words`, shows a credential where it is written: an assignment or a token
/// the plane's own save would refuse, a header that signs a request, or an option that takes a
/// secret, each with a value and not a variable's name.
fn shows_a_credential(words: &[String]) -> bool {
    let whole = words.join(" ");
    if crate::secretshape::secret_kind(&whole).is_some() {
        return true;
    }
    let literal = |value: &str| !value.is_empty() && !value.starts_with('$');
    words.iter().enumerate().any(|(at, word)| {
        let lower = word.to_ascii_lowercase();
        if let Some(value) = ["authorization:", "x-api-key:"]
            .iter()
            .find_map(|header| lower.strip_prefix(header))
        {
            return !value.contains('$');
        }
        if !word.starts_with('-') {
            return false;
        }
        let (name, given) = match lower.split_once('=') {
            Some((name, given)) => (name, Some(given)),
            None => (lower.as_str(), None),
        };
        let value = given.or_else(|| words.get(at + 1).map(String::as_str));
        let user = matches!(name, "-u" | "--user");
        let secret = [
            "token", "password", "passwd", "secret", "api-key", "api_key", "apikey",
        ]
        .iter()
        .any(|one| name.contains(one))
            && !name.ends_with("-file")
            && !name.ends_with("-env");
        value.is_some_and(|value| {
            (secret && literal(value)) || (user && literal(value) && value.contains(':'))
        })
    })
}

/// A command's words, however it was written: one string to split as a shell would, or a list.
fn words_of<'a>(written: impl Iterator<Item = (bool, &'a str)>) -> Vec<String> {
    let mut words = Vec::new();
    for (whole, text) in written {
        if whole {
            words.push(text.to_owned());
        } else {
            match crate::shellseg::posix_split(text) {
                Ok(split) => words.extend(split),
                Err(_) => words.extend(text.split_whitespace().map(str::to_owned)),
            }
        }
    }
    words
}

/// Whether the JSON object `value` runs a command that [`shows_a_credential`].
fn json_runs_a_credential(value: &serde_json::Value) -> bool {
    use serde_json::Value;
    let Value::Object(map) = value else {
        return false;
    };
    let mut written = Vec::new();
    for key in ["command", "args"] {
        match map.get(key) {
            Some(Value::String(text)) => written.push((false, text.as_str())),
            Some(Value::Array(items)) => {
                written.extend(items.iter().filter_map(Value::as_str).map(|t| (true, t)));
            }
            _ => {}
        }
    }
    !written.is_empty() && shows_a_credential(&words_of(written.into_iter()))
}

/// `value` with every setting `names` lists removed at any depth, each object that runs a command
/// showing a credential removed whole, and each URL without its user information. `at` is where it
/// is, for what is said.
fn clean_json(value: &mut serde_json::Value, names: &[&str], at: &str, said: &mut Vec<String>) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            map.retain(|key, inner| {
                let where_ = format!("{at}.{key}");
                if names.contains(&key.as_str()) {
                    said.push(where_);
                    return false;
                }
                if json_runs_a_credential(inner) {
                    said.push(format!("{where_}: a command with a credential in it"));
                    return false;
                }
                true
            });
            for (key, inner) in map.iter_mut() {
                clean_json(inner, names, &format!("{at}.{key}"), said);
            }
        }
        Value::Array(items) => {
            let before = items.len();
            items.retain(|inner| !json_runs_a_credential(inner));
            if items.len() != before {
                said.push(format!("{at}: a command with a credential in it"));
            }
            for inner in items {
                clean_json(inner, names, at, said);
            }
        }
        Value::String(text) => {
            let mut found = Vec::new();
            let clean = no_url_users(text, &mut found).into_owned();
            if !found.is_empty() {
                said.push(format!("{at}: a URL's user information"));
                *text = clean;
            }
        }
        _ => {}
    }
}

/// [`clean_json`]'s command and URL walk, for the plain values of a Codex `config.toml`. Its
/// `notify` is a command too, as a bare list.
fn clean_toml(table: &mut toml::Table, at: &str, said: &mut Vec<String>) {
    use toml::Value;
    let runs = |value: &Value| {
        let mut written = Vec::new();
        if let Value::Table(inner) = value {
            for key in ["command", "args"] {
                match inner.get(key) {
                    Some(Value::String(text)) => written.push((false, text.as_str())),
                    Some(Value::Array(items)) => {
                        written.extend(items.iter().filter_map(Value::as_str).map(|t| (true, t)));
                    }
                    _ => {}
                }
            }
        }
        !written.is_empty() && shows_a_credential(&words_of(written.into_iter()))
    };
    table.retain(|key, inner| {
        let list_command = key == "notify"
            && inner.as_array().is_some_and(|items| {
                shows_a_credential(&words_of(
                    items.iter().filter_map(Value::as_str).map(|t| (true, t)),
                ))
            });
        if list_command || runs(&*inner) {
            said.push(format!("{at}.{key}: a command with a credential in it"));
            return false;
        }
        true
    });
    for (key, inner) in table.iter_mut() {
        clean_toml_value(inner, &format!("{at}.{key}"), said);
    }
}

fn clean_toml_value(value: &mut toml::Value, at: &str, said: &mut Vec<String>) {
    match value {
        toml::Value::Table(inner) => clean_toml(inner, at, said),
        toml::Value::Array(items) => {
            for inner in items {
                clean_toml_value(inner, at, said);
            }
        }
        toml::Value::String(text) => {
            let mut found = Vec::new();
            let clean = no_url_users(text, &mut found).into_owned();
            if !found.is_empty() {
                said.push(format!("{at}: a URL's user information"));
                *text = clean;
            }
        }
        _ => {}
    }
}

/// `text`, a JSON settings file of `kind`'s, with every setting [`dropped`] names removed at any
/// depth, written again. `None` where it is not JSON.
fn json_without(kind: &str, text: &str, said: &mut Vec<String>) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_str(text).ok()?;
    clean_json(&mut value, dropped(kind), "settings", said);
    let mut written = serde_json::to_vec_pretty(&value).ok()?;
    written.push(b'\n');
    Some(written)
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
                if !closes_next(chars.clone()) {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Whether the next mark in `rest`, past blanks and comments, closes an object or an array.
fn closes_next(mut rest: std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    while let Some(c) = rest.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if rest.peek() == Some(&'/') => {
                if !rest.by_ref().any(|s| s == '\n') {
                    return false;
                }
            }
            '/' if rest.peek() == Some(&'*') => {
                rest.next();
                let mut last = ' ';
                let mut ended = false;
                for s in rest.by_ref() {
                    if last == '*' && s == '/' {
                        ended = true;
                        break;
                    }
                    last = s;
                }
                if !ended {
                    return false;
                }
            }
            '}' | ']' => return true,
            _ => return false,
        }
    }
    false
}

/// `text`, a Codex `config.toml`, with every setting [`dropped`] names removed at any depth, and
/// the variables `[shell_environment_policy]` sets, written again with no comment. `None` where
/// it is not TOML.
fn codex_without(text: &str, said: &mut Vec<String>) -> Option<Vec<u8>> {
    let mut doc: toml_edit::DocumentMut = text.parse().ok()?;
    let names = dropped("codex");
    drop_toml(doc.as_table_mut(), names);
    if let Some(policy) = doc
        .get_mut("shell_environment_policy")
        .and_then(|p| p.as_table_like_mut())
    {
        policy.remove("set");
    }
    // Written again through plain values, so no comment travels: a commented-out key is a key
    // copied all the same.
    let mut plain: toml::Table = toml::from_str(&doc.to_string()).ok()?;
    clean_toml(&mut plain, "settings", said);
    toml::to_string(&plain).ok().map(String::into_bytes)
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

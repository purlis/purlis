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
/// and always writes again. A Markdown file is written again only where something goes.
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

/// [`filtered`], with what was removed said in the log, as what an import leaves out is: what a
/// desktop sends.
pub fn sent_without_credentials(kind: &str, path: &str, bytes: &[u8]) -> Result<Filtered, String> {
    let file = filtered(kind, path, bytes)?;
    for what in &file.removed {
        tracing::info!(
            "purlis: a setting is not sent to a runner: {}: {what}",
            crate::shown::short(path)
        );
    }
    Ok(file)
}

/// A settings file as it is sent, and what was removed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    pub bytes: Vec<u8>,
    /// What went, by where it was and never by its value, each once.
    pub removed: Vec<String>,
    /// The variables among what went that only choose an endpoint or a provider
    /// ([`chooses_an_endpoint`]), by name, so a runner chat can say it may use another (#3867).
    pub variables: Vec<String>,
}

/// Whether the variable `name` only chooses where a harness's requests go or who serves them.
fn chooses_an_endpoint(name: &str) -> bool {
    name.ends_with("_BASE_URL")
        || name.ends_with("_ENDPOINT")
        || matches!(
            name,
            "CLAUDE_CODE_USE_BEDROCK"
                | "CLAUDE_CODE_USE_VERTEX"
                | "CLAUDE_CODE_USE_FOUNDRY"
                | "ANTHROPIC_MODEL"
                | "ANTHROPIC_VERTEX_PROJECT_ID"
                | "CLOUD_ML_REGION"
                | "AWS_REGION"
                | "AWS_PROFILE"
        )
}

/// **`bytes`, the settings file `path` of a `kind` profile, as it is sent to a runner**: a
/// harness's own settings file with every setting [`dropped`] names removed wherever it is in it,
/// and any command that shows a credential; a Markdown file with the same taken out of its
/// frontmatter; and any text with no URL's user information. A file with nothing to remove comes
/// back byte for byte, and filtering twice changes nothing, since a runner filters what it was
/// sent again and the two must come to one digest. `Err`, in words, for a settings file that does
/// not parse.
pub fn filtered(kind: &str, path: &str, bytes: &[u8]) -> Result<Filtered, String> {
    let mut said = Said::default();
    let bytes = filtered_saying(kind, path, bytes, &mut said)?;
    Ok(Filtered {
        bytes,
        removed: said.items,
        variables: said.variables,
    })
}

fn filtered_saying(
    kind: &str,
    path: &str,
    bytes: &[u8],
    said: &mut Said,
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
                let text = frontmatter_without(text, path.contains('/'), said);
                no_url_users(&text, said).into_owned().into_bytes()
            }
            Ok(text) => no_url_users(text, said).into_owned().into_bytes(),
            // A harness can read Markdown past a byte that is not UTF-8, frontmatter and all, so
            // it is filtered all the same, as the text it reads.
            Err(_) if is_markdown(path) => {
                let text = String::from_utf8_lossy(bytes);
                let text = frontmatter_without(&text, path.contains('/'), said);
                said.push("a character that is not UTF-8".to_owned());
                no_url_users(&text, said).into_owned().into_bytes()
            }
            // No text to read a URL in, and rewriting one could only break it.
            Err(_) => bytes.to_vec(),
        }),
    }
}

fn is_markdown(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// The most things one file's [`Said`] holds: a hostile file can repeat a removal for as long as
/// its size allows, and what is said of it need not grow with it.
const MOST_SAID: usize = 256;

/// What was removed from a file, each once, up to [`MOST_SAID`].
#[derive(Default)]
struct Said {
    items: Vec<String>,
    seen: std::collections::HashSet<String>,
    variables: Vec<String>,
}

impl Said {
    fn push(&mut self, what: String) {
        if self.items.len() < MOST_SAID && self.seen.insert(what.clone()) {
            self.items.push(what);
        }
    }

    /// Notes each of the `names` a removed environment block set that chooses an endpoint.
    fn variables<'a>(&mut self, names: impl Iterator<Item = &'a str>) {
        for name in names {
            let new = !self.variables.iter().any(|one| one == name);
            if chooses_an_endpoint(name) && new && self.variables.len() < MOST_SAID {
                self.variables.push(name.to_owned());
            }
        }
    }
}

/// `at`, one name further in. A name that is long, or could hold a value, is not repeated: what is
/// said names where a thing was and never what it held.
fn further(at: &str, name: &str) -> String {
    let plain = name.len() <= 64
        && !name.contains("://")
        && !name.contains('@')
        && crate::secretshape::token_kind(name).is_none();
    format!("{at}.{}", if plain { name } else { "(a name)" })
}

/// What a frontmatter may not carry: whatever any harness's settings file may not, since a
/// frontmatter can declare a server or a hook for any of them.
fn dropped_from_frontmatter() -> Vec<&'static str> {
    crate::profiles::KINDS
        .iter()
        .flat_map(|kind| dropped(kind.word).iter().copied())
        .collect()
}

/// `text`, with its frontmatter as it may be sent. One with nothing to remove and no `#` is left
/// byte for byte. Any other is written again, one plain `key: value` a line, so no comment travels:
/// a commented-out key is a key copied all the same. One purlis cannot read keeps only the plain
/// values of its `name` and `description`, so what it may declare goes and the skill or agent still
/// loads; so does one that would not read back as it was written, so filtering it again changes
/// nothing. Rules around text that holds no keys are no frontmatter, and neither is what a harness's
/// own instructions file (`in_folder` false) holds that purlis cannot read: both stay.
fn frontmatter_without(text: &str, in_folder: bool, said: &mut Said) -> String {
    let Some((span, read)) = purlis_skills::handed::frontmatter_value(text) else {
        return text.to_owned();
    };
    let block = &text[span.clone()];
    let names = dropped_from_frontmatter();
    let filtered_written = match read {
        Ok(value) if !value.is_object() => return text.to_owned(),
        Ok(mut value) => {
            let before = value.clone();
            if json_runs_a_credential(&value)
                && let serde_json::Value::Object(map) = &mut value
            {
                map.remove("command");
                map.remove("args");
                said.push("frontmatter: a command with a credential in it".to_owned());
            }
            clean_json(&mut value, &names, "frontmatter", said);
            if value == before && !block.contains('#') {
                return text.to_owned();
            }
            let written = frontmatter_written(&value);
            reads_back(&written, &value).then_some(written)
        }
        Err(_) if !in_folder => return text.to_owned(),
        Err(_) => None,
    };
    let written = filtered_written.unwrap_or_else(|| {
        let mut value = serde_json::Value::Object(plain_name_and_description(block));
        clean_json(&mut value, &names, "frontmatter", said);
        said.push("frontmatter, but for its name and description: not readable".to_owned());
        frontmatter_written(&value)
    });
    format!("{}{written}{}", &text[..span.start], &text[span.end..])
}

/// Whether `written`, a frontmatter as [`frontmatter_written`] wrote it, reads as `value` again.
fn reads_back(written: &str, value: &serde_json::Value) -> bool {
    let whole = format!("---\n{written}---\n");
    matches!(
        purlis_skills::handed::frontmatter_value(&whole),
        Some((_, Ok(read))) if read == *value
    )
}

/// The `name` and `description` of a frontmatter that is not YAML, each read off its own line as
/// plain text up to any comment. One that opens anything but plain text (a mapping, a list, a block,
/// a reference) is left out, since what it holds is not plain.
fn plain_name_and_description(block: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut kept = serde_json::Map::new();
    for line in block.split(['\n', '\r']) {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !matches!(key, "name" | "description") {
            continue;
        }
        let value = value
            .find(" #")
            .or_else(|| value.find("\t#"))
            .map_or(value, |comment| &value[..comment])
            .trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        let plain =
            !value.is_empty() && !value.starts_with(['{', '[', '|', '>', '&', '*', '!', '#']);
        if plain {
            kept.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
        }
    }
    kept
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
/// Whether any was comes back with it.
fn without_url_users(text: &str) -> (std::borrow::Cow<'_, str>, bool) {
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
            .find(|c: char| c.is_whitespace() || "/?#\"'<>\\`,;)]|".contains(c))
            .map_or(text.len(), |n| authority + n);
        if let Some(user_end) = text[authority..end].rfind('@') {
            out.push_str(&text[copied..authority]);
            copied = authority + user_end + 1;
            from = copied;
        }
    }
    if copied == 0 {
        return (std::borrow::Cow::Borrowed(text), false);
    }
    out.push_str(&text[copied..]);
    (std::borrow::Cow::Owned(out), true)
}

/// [`without_url_users`], saying so where any went.
fn no_url_users<'a>(text: &'a str, said: &mut Said) -> std::borrow::Cow<'a, str> {
    let (clean, any) = without_url_users(text);
    if any {
        said.push("a URL's user information".to_owned());
    }
    clean
}

/// A string setting at `at`, as it may be sent: no URL's user information, and empty where it is
/// a token by its shape alone.
fn clean_string(text: &mut String, at: &str, said: &mut Said) {
    if crate::secretshape::token_kind(text).is_some() {
        text.clear();
        said.push(format!("{at}: a value shaped like a token"));
        return;
    }
    let (clean, any) = without_url_users(text);
    if any {
        let clean = clean.into_owned();
        said.push(format!("{at}: a URL's user information"));
        *text = clean;
    }
}

/// Whether `name`, an option's or a variable's, ends in a word that holds a credential:
/// `GH_TOKEN`, `--api-key`, and not `--max-tokens`, `--token-file`, `--secret-id` or `--no-password`.
fn names_a_credential(name: &str) -> bool {
    let words: Vec<&str> = name
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .collect();
    let [.., before, last] = words.as_slice() else {
        return words
            .last()
            .is_some_and(|last| CREDENTIAL_WORDS.contains(last));
    };
    words[0] != "no"
        && (CREDENTIAL_WORDS.contains(last)
            || (*last == "key" && matches!(*before, "api" | "secret" | "access")))
}

/// The last word of a name that holds a credential.
const CREDENTIAL_WORDS: [&str; 7] = [
    "token", "password", "passwd", "pwd", "secret", "apikey", "bearer",
];

/// Whether `value` is written out, and not read from a variable, another command or a harness's
/// own reference (opencode's `{env:KEY}`).
fn written_out(value: &str) -> bool {
    let value = value.trim_start_matches(['"', '\'']);
    !value.is_empty()
        && !value.starts_with(['$', '`'])
        && !value.starts_with("{env:")
        && !value.starts_with("{file:")
}

/// How deep a command is read through commands quoted inside it (`sh -c '…'`).
const MOST_QUOTED: usize = 3;

/// Whether a header's name at `found` in `lower` is one a request is sent with: after `-H` or
/// `--header`, in this word or as the word `before` it. A word that only mentions one
/// (`echo "Cookie: none"`) is not.
fn is_a_sent_header(lower: &str, found: usize, before: Option<&str>) -> bool {
    let ahead = lower[..found].trim_end_matches([' ', '"', '\'', '=']);
    if ahead.is_empty() {
        return before.is_some_and(|word| matches!(word, "-H" | "--header"));
    }
    ahead.ends_with("-h") || ahead.ends_with("--header")
}

/// Whether `word`, between the words `before` and `next`, shows a credential. `takes_a_user` is
/// whether the command is one whose `-u` is a user and password (`curl`, and not Docker's `-u
/// 1000:1000`).
fn word_shows_a_credential(
    word: &str,
    before: Option<&str>,
    next: Option<&str>,
    takes_a_user: bool,
) -> bool {
    let lower = word.to_ascii_lowercase();
    for header in ["authorization:", "x-api-key:", "cookie:"] {
        if let Some(found) = lower.find(header)
            && is_a_sent_header(&lower, found, before)
        {
            let mut value = lower[found + header.len()..].split_whitespace();
            let first = value.next().unwrap_or_default();
            let given = if matches!(first, "bearer" | "basic" | "token") {
                value.next().unwrap_or_default()
            } else {
                first
            };
            if written_out(given) {
                return true;
            }
        }
    }
    let (name, given) = match lower.split_once('=') {
        Some((name, given)) => (name, Some(given)),
        None => (lower.as_str(), None),
    };
    // An assignment: `GH_TOKEN=x`.
    if !word.starts_with('-') {
        let assignment =
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        return assignment && names_a_credential(name) && given.is_some_and(written_out);
    }
    // The word after an option is its value only where it is no option itself.
    let next = next.filter(|word| !word.starts_with('-'));
    // A user and password: `-u me:pw`, `-ume:pw`, `--user me:pw`, `--proxy-user me:pw`.
    let user = |value: &str| {
        value
            .split_once(':')
            .is_some_and(|(_, password)| written_out(password))
    };
    let says_a_user = match name {
        "--proxy-user" => given.or(next).is_some_and(user),
        "-u" | "--user" => takes_a_user && given.or(next).is_some_and(user),
        _ => {
            takes_a_user
                && word
                    .strip_prefix("-u")
                    .is_some_and(|glued| !glued.starts_with('-') && user(glued))
        }
    };
    if says_a_user {
        return true;
    }
    // An option that takes a secret: `--token x`, `--api-key=x`. `--secret id=…,src=…` names a
    // file, as Docker's does.
    names_a_credential(name)
        && given
            .or(next)
            .is_some_and(|value| written_out(value) && !value.starts_with("id="))
}

/// Whether a command, as `words`, shows a credential where it is written: a token by its shape, an
/// assignment, a header or an option holding one, each with a value and not a variable's name. A
/// word holding a command quoted whole is read too, and each part of one a separator is glued
/// into (`cd a&&GH_TOKEN=x y`).
fn shows_a_credential(words: &[String], depth: usize) -> bool {
    if crate::secretshape::token_kind(&words.join(" ")).is_some() {
        return true;
    }
    let takes_a_user = words.iter().any(|word| {
        let program = word.rsplit('/').next().unwrap_or(word);
        matches!(program, "curl" | "wget")
    });
    words.iter().enumerate().any(|(at, word)| {
        let before = at
            .checked_sub(1)
            .and_then(|n| words.get(n))
            .map(String::as_str);
        let next = words.get(at + 1).map(String::as_str);
        word_shows_a_credential(word, before, next, takes_a_user)
            || (word.contains([';', '&', '|'])
                && word
                    .split([';', '&', '|'])
                    .any(|part| word_shows_a_credential(part, None, None, false)))
            || (depth < MOST_QUOTED
                && word.contains(char::is_whitespace)
                && shows_a_credential(&words_of([(false, word.as_str())].into_iter()), depth + 1))
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
    !written.is_empty() && shows_a_credential(&words_of(written.into_iter()), 0)
}

/// `value` with every setting `names` lists removed at any depth, each object that runs a command
/// showing a credential removed whole, and each string and name as [`clean_string`] leaves it. `at`
/// is where it is, for what is said.
fn clean_json(value: &mut serde_json::Value, names: &[&str], at: &str, said: &mut Said) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let odd: Vec<String> = map.keys().filter(|k| k.contains("://")).cloned().collect();
            for key in odd {
                if let (clean, true) = without_url_users(&key) {
                    let clean = clean.into_owned();
                    if let Some(inner) = map.remove(&key) {
                        map.insert(clean, inner);
                        said.push(format!("{at}: a URL's user information"));
                    }
                }
            }
            map.retain(|key, inner| {
                if names.contains(&key.as_str()) || crate::secretshape::token_kind(key).is_some() {
                    if let Value::Object(block) = &*inner {
                        said.variables(block.keys().map(String::as_str));
                    }
                    said.push(further(at, key));
                    return false;
                }
                if json_runs_a_credential(inner) {
                    said.push(format!(
                        "{}: a command with a credential in it",
                        further(at, key)
                    ));
                    return false;
                }
                true
            });
            for (key, inner) in map.iter_mut() {
                clean_json(inner, names, &further(at, key), said);
            }
            // Read again as it will be sent, so filtering it again finds nothing more.
            map.retain(|key, inner| {
                let keep = !json_runs_a_credential(inner);
                if !keep {
                    said.push(format!(
                        "{}: a command with a credential in it",
                        further(at, key)
                    ));
                }
                keep
            });
        }
        Value::Array(items) => {
            let before = items.len();
            items.retain(|inner| !json_runs_a_credential(inner));
            for inner in items.iter_mut() {
                clean_json(inner, names, at, said);
            }
            items.retain(|inner| !json_runs_a_credential(inner));
            if items.len() != before {
                said.push(format!("{at}: a command with a credential in it"));
            }
        }
        Value::String(text) => clean_string(text, at, said),
        _ => {}
    }
}

/// Whether the TOML table `value` runs a command that [`shows_a_credential`].
fn toml_runs_a_credential(value: &toml::Value) -> bool {
    use toml::Value;
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
    !written.is_empty() && shows_a_credential(&words_of(written.into_iter()), 0)
}

/// [`clean_json`]'s command and string walk, for the plain values of a Codex `config.toml`. Its
/// `notify` is a command too, as a bare list.
fn clean_toml(table: &mut toml::Table, at: &str, said: &mut Said) {
    let odd: Vec<String> = table
        .keys()
        .filter(|k| k.contains("://"))
        .cloned()
        .collect();
    for key in odd {
        if let (clean, true) = without_url_users(&key) {
            let clean = clean.into_owned();
            if let Some(inner) = table.remove(&key) {
                table.insert(clean, inner);
                said.push(format!("{at}: a URL's user information"));
            }
        }
    }
    retain_toml_commands(table, at, said);
    for (key, inner) in table.iter_mut() {
        clean_toml_value(inner, &further(at, key), said);
    }
    // Read again as it will be sent, so filtering it again finds nothing more.
    retain_toml_commands(table, at, said);
}

fn retain_toml_commands(table: &mut toml::Table, at: &str, said: &mut Said) {
    use toml::Value;
    table.retain(|key, inner| {
        let list_command = key == "notify"
            && inner.as_array().is_some_and(|items| {
                shows_a_credential(
                    &words_of(items.iter().filter_map(Value::as_str).map(|t| (true, t))),
                    0,
                )
            });
        if crate::secretshape::token_kind(key).is_some() {
            said.push(further(at, key));
            return false;
        }
        if list_command || toml_runs_a_credential(inner) {
            said.push(format!(
                "{}: a command with a credential in it",
                further(at, key)
            ));
            return false;
        }
        true
    });
}

fn clean_toml_value(value: &mut toml::Value, at: &str, said: &mut Said) {
    match value {
        toml::Value::Table(inner) => clean_toml(inner, at, said),
        toml::Value::Array(items) => {
            let before = items.len();
            items.retain(|inner| !toml_runs_a_credential(inner));
            for inner in items.iter_mut() {
                clean_toml_value(inner, at, said);
            }
            items.retain(|inner| !toml_runs_a_credential(inner));
            if items.len() != before {
                said.push(format!("{at}: a command with a credential in it"));
            }
        }
        toml::Value::String(text) => clean_string(text, at, said),
        _ => {}
    }
}

/// `text`, a JSON settings file of `kind`'s, with every setting [`dropped`] names removed at any
/// depth, written again. `None` where it is not JSON.
fn json_without(kind: &str, text: &str, said: &mut Said) -> Option<Vec<u8>> {
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
fn codex_without(text: &str, said: &mut Said) -> Option<Vec<u8>> {
    let mut doc: toml_edit::DocumentMut = text.parse().ok()?;
    let names = dropped("codex");
    drop_toml(doc.as_table_mut(), names, "settings", said);
    if let Some(policy) = doc
        .get_mut("shell_environment_policy")
        .and_then(|p| p.as_table_like_mut())
        && let Some(set) = policy.remove("set")
    {
        if let Some(block) = set.as_table_like() {
            said.variables(block.iter().map(|(name, _)| name));
        }
        said.push("settings.shell_environment_policy.set".to_owned());
    }
    // Written again through plain values, so no comment travels: a commented-out key is a key
    // copied all the same.
    let mut plain: toml::Table = toml::from_str(&doc.to_string()).ok()?;
    clean_toml(&mut plain, "settings", said);
    toml::to_string(&plain).ok().map(String::into_bytes)
}

fn drop_toml(table: &mut dyn toml_edit::TableLike, names: &[&str], at: &str, said: &mut Said) {
    let keys: Vec<String> = table.iter().map(|(k, _)| k.to_owned()).collect();
    for key in keys {
        if names.contains(&key.as_str()) {
            let gone = table.remove(&key);
            if let Some(block) = gone.as_ref().and_then(toml_edit::Item::as_table_like) {
                said.variables(block.iter().map(|(name, _)| name));
            }
            said.push(further(at, &key));
            continue;
        }
        if let Some(item) = table.get_mut(&key) {
            drop_toml_item(item, names, &further(at, &key), said);
        }
    }
}

fn drop_toml_item(item: &mut toml_edit::Item, names: &[&str], at: &str, said: &mut Said) {
    match item {
        toml_edit::Item::Table(table) => drop_toml(table, names, at, said),
        toml_edit::Item::ArrayOfTables(tables) => {
            for table in tables.iter_mut() {
                drop_toml(table, names, at, said);
            }
        }
        toml_edit::Item::Value(toml_edit::Value::InlineTable(table)) => {
            drop_toml(table, names, at, said);
        }
        toml_edit::Item::Value(toml_edit::Value::Array(items)) => {
            for value in items.iter_mut() {
                if let toml_edit::Value::InlineTable(table) = value {
                    drop_toml(table, names, at, said);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;

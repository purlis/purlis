// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Model provider declarations: a model provider as data (design §3.4, T1a-06, #6749).
//!
//! A **model provider declaration** says how to reach one model provider: the wire formats it
//! serves and where, how it takes a key, and where its models are listed. This module is the
//! one reader for every declaration, whichever tier it comes from (built-in, a project's or
//! yours), so a later tier adds its source and never a second reader.
//!
//! **An unknown field is refused, naming it**, so a field a later version adds is never read as
//! nothing by this one. In particular there is no `hosts` field: the hosts a declaration may
//! reach are derived from its `[[endpoint]] base`s and its `[models] list` ([`Declaration::hosts`]).
//!
//! **A `base` is an origin and a plain path**: `https` (or `http` to a loopback, private or
//! Tailscale host, for `local` only), no userinfo, query, fragment, dot segment, percent escape
//! or reserved character, and never `{model}`. A built-in's allowlist is matched against the
//! full upstream path, the base's path joined with the request's ([`Base::allows`]).
//!
//! **The digest is an exclusion list** (spec C-23): it covers every field but
//! `[[models.table]]` and `[models] background`, so a price or background-model edit to a
//! built-in leaves its digest, and a runner on another build, alone.
//!
//! Every refusal is one sentence the window can show, naming the file, the field and why.
//!
//! Not read in this version: `serves`, `translates`, `[[endpoint]] headers`, template variables,
//! `[models] ids`, the alias map, local probes and the provider's links (1b and later). Each is
//! refused as an unknown field until its version reads it.

use std::collections::BTreeSet;
use std::net::IpAddr;

use sha2::Digest as _;

use crate::shown;

/// Where a declaration comes from: its tier. An `id` is a word within its tier, and the same
/// word in two tiers is refused when the second is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum From {
    /// Shipped with purlis.
    BuiltIn,
    /// Committed in a project, named by the project's identity.
    Project { project: String },
    /// The person's own, kept on this machine.
    Yours,
}

impl From {
    /// The one word for this tier, as `profiles.json`'s `from` writes it.
    pub fn word(&self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::Project { .. } => "project",
            Self::Yours => "yours",
        }
    }
}

/// What a provider is, by what it changes (design §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Vendor,
    Router,
    Cloud,
    Local,
}

/// Whether the provider takes a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Required,
    Optional,
    None,
}

/// A wire format an endpoint serves. The words for Responses, Gemini and Bedrock InvokeModel
/// join when the spec fixes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `anthropic-messages`.
    AnthropicMessages,
    /// `openai-chat`.
    OpenaiChat,
}

impl Format {
    /// Every wire format purlis reads, in the order a refusal lists them.
    pub const ALL: [Format; 2] = [Self::AnthropicMessages, Self::OpenaiChat];

    /// The word a declaration names it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic-messages",
            Self::OpenaiChat => "openai-chat",
        }
    }

    /// The format `word` names, where it is one purlis reads.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.word() == word)
    }
}

/// How an endpoint takes its credential. The named-header form is 1b's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    XApiKey,
    Bearer,
    Sigv4,
    GoogleOauth,
    Entra,
}

impl Auth {
    fn word(self) -> &'static str {
        match self {
            Self::XApiKey => "x-api-key",
            Self::Bearer => "bearer",
            Self::Sigv4 => "sigv4",
            Self::GoogleOauth => "google-oauth",
            Self::Entra => "entra",
        }
    }
}

/// A checked `base` (or list URL): an origin and a plain path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    origin: String,
    path: String,
}

impl Base {
    /// `scheme://host[:port]`, the host lower-cased and a default port left out.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The path, with no trailing slash: empty for a bare origin, else `/seg/seg`.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The path upstream sees for `request_path` (which starts with `/`): the base's path
    /// joined with it.
    pub fn upstream_path(&self, request_path: &str) -> String {
        format!("{}{request_path}", self.path)
    }

    /// Whether `request_path`, joined to this base, is on `allowlist`. The joined path is what
    /// reaches upstream, so it alone is matched (R-086).
    pub fn allows(&self, request_path: &str, allowlist: &[&str]) -> bool {
        let joined = self.upstream_path(request_path);
        allowlist.iter().any(|allowed| *allowed == joined)
    }
}

/// One `[[endpoint]]`: a wire format and where it is served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub format: Format,
    pub base: Base,
    pub auth: Auth,
}

/// A model's prices, in US dollars per million tokens. A price left out is unknown.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub cache_write_1h: Option<f64>,
}

/// One `[[models.table]]` entry (spec C-15). What each field means is its consumer's; this
/// reader parses and type-checks them, and a field left out is unknown, never guessed.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRow {
    pub id: String,
    pub context: Option<u64>,
    pub max_output: Option<u64>,
    pub tools: Option<bool>,
    pub price: Option<Price>,
    /// Tokens: a call whose input plus cache reads and writes is at or over it reads *estimated*.
    pub tier_above: Option<u64>,
    pub source: Option<String>,
    pub as_of: Option<String>,
}

/// The `[models]` table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Models {
    /// A cheaper model for Claude Code's `haiku` slot.
    pub background: Option<String>,
    /// Where the provider lists its models.
    pub list: Option<Base>,
    pub table: Vec<ModelRow>,
}

/// A model provider declaration, read and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub from: From,
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub key: Key,
    pub endpoints: Vec<Endpoint>,
    pub models: Models,
    digest: String,
}

impl Declaration {
    /// The digest over every field but `[[models.table]]` and `[models] background`, as
    /// lower-case hex SHA-256.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The origins this declaration may reach: its endpoints' and its list's, and no other.
    pub fn hosts(&self) -> BTreeSet<String> {
        self.endpoints
            .iter()
            .map(|endpoint| &endpoint.base)
            .chain(self.models.list.as_ref())
            .map(|base| base.origin.clone())
            .collect()
    }
}

/// A reference to a project's declaration: the project's identity, the declaration's id and
/// the digest approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReference {
    pub project: String,
    pub id: String,
    pub digest: String,
}

impl ProjectReference {
    /// The declaration this names among `held`, offered only inside its own project and only
    /// while its digest is the one referenced.
    pub fn resolve<'a>(
        &self,
        in_project: &str,
        held: &'a [Declaration],
    ) -> Option<&'a Declaration> {
        if in_project != self.project {
            return None;
        }
        held.iter().find(|declaration| {
            matches!(&declaration.from, From::Project { project } if *project == self.project)
                && declaration.id == self.id
                && declaration.digest == self.digest
        })
    }
}

/// The most characters of an `id`.
const MOST_ID: usize = 64;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    id: String,
    name: String,
    kind: String,
    key: Option<String>,
    #[serde(default)]
    endpoint: Vec<RawEndpoint>,
    #[serde(default)]
    models: RawModels,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEndpoint {
    format: String,
    base: String,
    auth: Option<String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawModels {
    background: Option<String>,
    list: Option<String>,
    #[serde(default)]
    table: Vec<ModelRow>,
}

/// `text` as a declaration from `from`, read beside the declarations already `known` and
/// named `file` in every refusal — or the one sentence saying why it is not one. The first
/// failure wins.
pub fn parse(
    text: &str,
    from: &From,
    known: &[Declaration],
    file: &str,
) -> Result<Declaration, String> {
    let raw: Raw = toml::from_str(text).map_err(|e| {
        let at = e
            .span()
            .and_then(|span| {
                let line = text[..span.start].matches('\n').count();
                text.lines().nth(line).map(|words| (line + 1, words.trim()))
            })
            .map(|(number, words)| format!(", at line {number}: {}", shown::short(words)))
            .unwrap_or_default();
        format!(
            "{file} is not a model provider declaration purlis reads ({}{at}). Fix the file.",
            shown::short(e.message())
        )
    })?;
    let refused = |field: &str, why: String| format!("{file}'s `{field}` {why}");

    let id = raw.id.trim();
    if !is_word(id) {
        return Err(refused(
            "id",
            format!(
                "is {}, which is not a word: lower-case letters, digits and `-`, starting with a \
                 letter, at most {MOST_ID} of them.",
                shown::short(&raw.id)
            ),
        ));
    }
    if let Some(other) = known.iter().find(|other| {
        other.id == id && std::mem::discriminant(&other.from) != std::mem::discriminant(from)
    }) {
        return Err(refused(
            "id",
            format!(
                "is {id}, which a {} declaration already has. Give it an id of its own.",
                other.from.word()
            ),
        ));
    }

    let name = raw.name.trim();
    if name.is_empty() {
        return Err(refused(
            "name",
            "is empty. Give it the name the window shows.".to_owned(),
        ));
    }
    if *from != From::BuiltIn {
        let folded = confusable_fold(name);
        if let Some(built_in) = known
            .iter()
            .find(|other| other.from == From::BuiltIn && confusable_fold(&other.name) == folded)
        {
            return Err(refused(
                "name",
                format!(
                    "is {}, which reads as {}, a provider purlis ships. Give it a name of its own.",
                    shown::short(name),
                    shown::short(&built_in.name)
                ),
            ));
        }
    }

    let kind = match raw.kind.as_str() {
        "vendor" => Kind::Vendor,
        "router" => Kind::Router,
        "cloud" => Kind::Cloud,
        "local" => Kind::Local,
        other => {
            return Err(refused(
                "kind",
                format!(
                    "is {}, which is not one of `vendor`, `router`, `cloud` or `local`.",
                    shown::short(other)
                ),
            ));
        }
    };
    if matches!(from, From::Project { .. }) && matches!(kind, Kind::Cloud | Kind::Local) {
        return Err(refused(
            "kind",
            format!("is {}, which a project's declaration may not be.", raw.kind),
        ));
    }

    let key = match (raw.key.as_deref(), kind) {
        (Some(_), Kind::Cloud) => {
            return Err(refused(
                "key",
                "is set on a cloud declaration, whose credentials come from the profile's source. \
                 Remove it."
                    .to_owned(),
            ));
        }
        (None, Kind::Local | Kind::Cloud) => Key::None,
        (None, Kind::Vendor | Kind::Router) | (Some("required"), _) => Key::Required,
        (Some("optional"), _) => Key::Optional,
        (Some("none"), _) => Key::None,
        (Some(other), _) => {
            return Err(refused(
                "key",
                format!(
                    "is {}, which is not one of `required`, `optional` or `none`.",
                    shown::short(other)
                ),
            ));
        }
    };

    if raw.endpoint.is_empty() {
        return Err(format!(
            "{file} has no `[[endpoint]]`, so it says nowhere to send a model call. Add one per \
             wire format it serves."
        ));
    }
    let endpoints = raw
        .endpoint
        .iter()
        .map(|endpoint| {
            let format = match endpoint.format.as_str() {
                "anthropic-messages" => Format::AnthropicMessages,
                "openai-chat" => Format::OpenaiChat,
                other => {
                    return Err(refused(
                        "[[endpoint]] format",
                        format!(
                            "is {}, which is not a wire format purlis reads: `anthropic-messages` \
                             or `openai-chat`.",
                            shown::short(other)
                        ),
                    ));
                }
            };
            let base = checked_base(&endpoint.base, kind).map_err(|why| refused("[[endpoint]] base", why))?;
            let auth = match endpoint.auth.as_deref() {
                None | Some("bearer") => Auth::Bearer,
                Some("x-api-key") => Auth::XApiKey,
                Some(mode @ ("sigv4" | "google-oauth" | "entra")) if *from != From::BuiltIn => {
                    return Err(refused(
                        "[[endpoint]] auth",
                        format!("is {mode}, a cloud mode, which only a declaration purlis ships may use."),
                    ));
                }
                Some("sigv4") => Auth::Sigv4,
                Some("google-oauth") => Auth::GoogleOauth,
                Some("entra") => Auth::Entra,
                Some(other) => {
                    return Err(refused(
                        "[[endpoint]] auth",
                        format!(
                            "is {}, which is not one of `x-api-key` or `bearer`.",
                            shown::short(other)
                        ),
                    ));
                }
            };
            Ok(Endpoint { format, base, auth })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let list = raw
        .models
        .list
        .as_deref()
        .map(|list| checked_base(list, kind).map_err(|why| refused("[models] list", why)))
        .transpose()?;
    let models = Models {
        background: raw.models.background,
        list,
        table: raw.models.table,
    };

    let mut declaration = Declaration {
        from: from.clone(),
        id: id.to_owned(),
        name: name.to_owned(),
        kind,
        key,
        endpoints,
        models,
        digest: String::new(),
    };
    declaration.digest = digest_of(&declaration);
    Ok(declaration)
}

/// The digest: every field but `[[models.table]]` and `[models] background` (spec C-23, H-6).
/// A field this reader adds joins here unless the spec names it excluded.
fn digest_of(declaration: &Declaration) -> String {
    let kind = match declaration.kind {
        Kind::Vendor => "vendor",
        Kind::Router => "router",
        Kind::Cloud => "cloud",
        Kind::Local => "local",
    };
    let key = match declaration.key {
        Key::Required => "required",
        Key::Optional => "optional",
        Key::None => "none",
    };
    let endpoints: Vec<serde_json::Value> = declaration
        .endpoints
        .iter()
        .map(|endpoint| {
            serde_json::json!({
                "format": endpoint.format.word(),
                "base": format!("{}{}", endpoint.base.origin, endpoint.base.path),
                "auth": endpoint.auth.word(),
            })
        })
        .collect();
    let covered = serde_json::json!({
        "id": declaration.id,
        "name": declaration.name,
        "kind": kind,
        "key": key,
        "endpoint": endpoints,
        "models": {
            "list": declaration.models.list.as_ref().map(|list| format!("{}{}", list.origin, list.path)),
        },
    });
    let bytes = serde_json::to_vec(&covered).unwrap_or_default();
    sha2::Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `id` is a word: a lower-case letter, then lower-case letters, digits and `-`.
fn is_word(id: &str) -> bool {
    id.len() <= MOST_ID
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `name` folded for case and the confusables a name is most likely forged with. The full
/// confusable set is 1b's; this holds the Cyrillic and Greek letters drawn as Latin ones.
fn confusable_fold(name: &str) -> String {
    crate::casefold::folded(name.trim())
        .chars()
        .map(|c| match c {
            'а' | 'α' => 'a',
            'с' | 'ϲ' => 'c',
            'ԁ' => 'd',
            'е' | 'ε' => 'e',
            'һ' => 'h',
            'і' | 'ι' => 'i',
            'ј' => 'j',
            'κ' => 'k',
            'ӏ' => 'l',
            'о' | 'ο' => 'o',
            'р' | 'ρ' => 'p',
            'ԛ' => 'q',
            'ѕ' => 's',
            'τ' => 't',
            'υ' => 'u',
            'ν' => 'v',
            'ԝ' => 'w',
            'х' | 'χ' => 'x',
            'у' => 'y',
            other => other,
        })
        .collect()
}

/// `text` as a [`Base`] for a declaration of `kind`, or why not: the rest of a sentence that
/// starts with the field's name.
fn checked_base(text: &str, kind: Kind) -> Result<Base, String> {
    let shown = shown::short(text);
    let refuse = |why: &str| format!("is {shown}, {why}");
    if text.contains("{model}") {
        return Err(refuse(
            "which takes `{model}`, and a base never does: the model travels in the request.",
        ));
    }
    if text.contains(['{', '}']) {
        return Err(refuse(
            "a template, which this version of purlis doesn't read.",
        ));
    }
    if text.contains('?') {
        return Err(refuse(
            "which has a query, and a base is an origin and a plain path.",
        ));
    }
    if text.contains('#') {
        return Err(refuse(
            "which has a fragment, and a base is an origin and a plain path.",
        ));
    }
    let (scheme, rest) = if let Some(rest) = text.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = text.strip_prefix("http://") {
        ("http", rest)
    } else {
        return Err(refuse("which is not an `https` address."));
    };
    let (authority, path) = rest.find('/').map_or((rest, ""), |at| rest.split_at(at));
    if authority.contains('@') {
        return Err(refuse("which carries a user name, and a base never does."));
    }
    let (host, port) =
        split_host(authority).ok_or_else(|| refuse("whose host is not one purlis reads."))?;
    if scheme == "http" && !(kind == Kind::Local && is_near(&host)) {
        return Err(refuse(
            "which is not `https`: only a local server on a loopback, private or Tailscale address \
             may use `http`.",
        ));
    }
    let default_port = if scheme == "https" { 443 } else { 80 };
    let origin = match port {
        Some(port) if port != default_port => format!("{scheme}://{host}:{port}"),
        _ => format!("{scheme}://{host}"),
    };
    let path = path.strip_suffix('/').unwrap_or(path);
    let plain = path.split('/').skip(1).all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
    });
    if !plain {
        return Err(refuse(
            "whose path is not plain: each segment letters, digits, `-`, `.`, `_` or `~`, with no \
             `.` or `..` segment and no escape.",
        ));
    }
    Ok(Base {
        origin,
        path: path.to_owned(),
    })
}

/// `authority`'s host, lower-cased, and its port, or nothing when either is malformed.
fn split_host(authority: &str) -> Option<(String, Option<u16>)> {
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (inside, after) = rest.split_once(']')?;
        inside.parse::<std::net::Ipv6Addr>().ok()?;
        (
            format!("[{inside}]"),
            after
                .strip_prefix(':')
                .map(str::to_owned)
                .or_else(|| after.is_empty().then(String::new)),
        )
    } else {
        let (host, port) = authority
            .split_once(':')
            .map_or((authority, None), |(h, p)| (h, Some(p)));
        let valid = !host.is_empty()
            && !host.starts_with(['.', '-'])
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
        if !valid {
            return None;
        }
        (
            host.to_ascii_lowercase(),
            port.map(str::to_owned).or_else(|| Some(String::new())),
        )
    };
    let port = match port?.as_str() {
        "" => None,
        digits => Some(digits.parse::<u16>().ok().filter(|port| *port != 0)?),
    };
    Some((host, port))
}

/// Whether `host` is loopback, a private range or a Tailscale address: where a local server's
/// plain `http` stays on this machine or its own network.
fn is_near(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".ts.net") {
        return true;
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let [a, b, ..] = ip.octets();
            ip.is_loopback() || ip.is_private() || (a == 100 && (64..128).contains(&b))
        }
        Ok(IpAddr::V6(ip)) => ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests;

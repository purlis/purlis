// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A request's line as the policy reads it: the method, the exact host and port, and the target
//! split into segments **decoded once**.

use super::policy::Refusal;
use super::repo::normalise_host;

/// The request line and `Host`, as the endpoint read them. The host and port are what the chat
/// asked for: they are an input to the policy (which matches them exactly against its repos), never
/// a statement of who is asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHead {
    pub method: String,
    pub host: String,
    pub port: u16,
    /// Path and query, as sent: `/owner/repo.git/info/refs?service=git-upload-pack`.
    pub target: String,
}

impl RequestHead {
    pub fn new(method: &str, host: &str, port: u16, target: &str) -> Self {
        Self {
            method: method.to_string(),
            host: host.to_string(),
            port,
            target: target.to_string(),
        }
    }
}

/// A target that passed every check: its path as segments (each decoded once) and its query.
#[derive(Debug)]
pub(crate) struct Target {
    pub(crate) segments: Vec<String>,
    pub(crate) query: Vec<(String, String)>,
}

impl Target {
    pub(crate) fn has_query_key(&self, key: &str) -> bool {
        self.query.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
    }

    pub(crate) fn query_value(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

/// The methods a forge answers that this proxy knows. Anything else is not on the list.
pub(crate) fn method(head: &RequestHead) -> Option<&'static str> {
    ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"]
        .into_iter()
        .find(|m| head.method.eq_ignore_ascii_case(m))
}

pub(crate) fn is_read(method: &str) -> bool {
    method == "GET" || method == "HEAD"
}

/// The host, normalised, or a refusal.
pub(crate) fn host(head: &RequestHead) -> Result<String, Refusal> {
    normalise_host(&head.host).map_err(|_| Refusal::MalformedRequest)
}

/// Read the target the way a forge's router does, and refuse what could be read two ways:
/// percent-encoding is decoded **once**, `.` and `..` segments, empty segments (`//`), a trailing
/// slash, backslashes, `;`, control characters, a fragment and anything outside ASCII are refused.
/// A decoded `/` is kept inside its segment (GitLab names a project `group%2Fproj`); the policy
/// accepts it where that is meant and nowhere else.
pub(crate) fn parse_target(target: &str) -> Result<Target, Refusal> {
    if !target.is_ascii() || target.bytes().any(|b| b.is_ascii_control() || b == b'\\') {
        return Err(Refusal::MalformedRequest);
    }
    if target.contains('#') {
        return Err(Refusal::MalformedRequest);
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    };
    let rest = path.strip_prefix('/').ok_or(Refusal::MalformedRequest)?;
    let mut segments = Vec::new();
    if !rest.is_empty() {
        for raw in rest.split('/') {
            if raw.is_empty() {
                return Err(Refusal::MalformedRequest);
            }
            let decoded = percent_encoding::percent_decode_str(raw)
                .decode_utf8()
                .map_err(|_| Refusal::MalformedRequest)?;
            let bad = decoded == "."
                || decoded == ".."
                || decoded.is_empty()
                || decoded
                    .bytes()
                    .any(|b| b.is_ascii_control() || b == b'\\' || b == b';');
            if bad {
                return Err(Refusal::MalformedRequest);
            }
            segments.push(decoded.into_owned());
        }
    }
    let query = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    Ok(Target { segments, query })
}

/// A `CONNECT` host as the policy reads it.
pub(crate) fn normalise_connect_host(host: &str) -> Result<String, Refusal> {
    normalise_host(host).map_err(|_| Refusal::MalformedRequest)
}

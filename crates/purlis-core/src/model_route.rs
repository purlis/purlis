// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's route** (design §3.3, T1a-08, #6751): what a run's key proxy is bound to at its
//! start and keeps to its end, and the in-memory checks it makes of each request against it.
//!
//! A [`Route`] is one profile, one declaration by its digest, and the endpoint whose `format` is
//! the harness's (R-077). It is a value taken when the run starts: editing the declaration
//! afterwards changes nothing for that run, and a new route is a new run (R-479). Nothing here
//! reads a file, a vault or the network.
//!
//! **The upstream URL** (R-084) is the endpoint's origin, then its base path, then the request's
//! path once normalised. An absolute-form or authority-form request target, `CONNECT`, a dot
//! segment (written or escaped) and an encoded slash (`%2F`, `%5C`) are refused.
//!
//! **The allowlist** (R-087, R-089) is core's own table of method and path for each wire format,
//! matched against the joined upstream path ([`Base::allows`], R-086), a query allowed on a listed
//! path. Messages lists `POST /v1/messages` alone in 1a, and every other format lists nothing until
//! its phase. Claude Code's startup `HEAD /api/hello` is answered absent, a 404 with no body
//! (spec C-19); everything else is refused before any key is swapped in.

use crate::model_provider::{Auth, Base, Declaration, Format};

/// Messages' allowlist: method and full upstream path (spec C-19). `count_tokens` is not on it:
/// it gets R-117's 400 and Claude Code falls back to a character estimate.
const MESSAGES: &[(&str, &str)] = &[("POST", "/v1/messages")];

/// The request Claude Code makes at startup to see its base URL answers (spec C-19): answered
/// absent, never sent upstream.
const HELLO: (&str, &str) = ("HEAD", "/api/hello");

/// Core's allowlist for `format`: each method and full upstream path a request may have.
pub fn allowlist(format: Format) -> &'static [(&'static str, &'static str)] {
    match format {
        Format::AnthropicMessages => MESSAGES,
        // Chat Completions' list is 1b's: until then nothing on it is sent.
        Format::OpenaiChat => &[],
    }
}

/// **The route a run is bound to**: taken once at its start, kept to its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The profile the run was started on.
    pub profile: String,
    /// The declaration's id.
    pub declaration: String,
    /// The declaration's digest when the run started.
    pub digest: String,
    /// The harness's wire format, and the endpoint's.
    pub format: Format,
    /// The endpoint's base: its origin and path.
    pub base: Base,
    /// How the endpoint takes its credential.
    pub auth: Auth,
}

/// Why a request is refused before anything is sent upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `CONNECT`.
    Connect,
    /// A request target that is not a path: absolute form, authority form or `*`.
    NotAPath,
    /// A `.` or `..` segment, written or escaped.
    DotSegment,
    /// An escaped `/` or `\` (`%2F`, `%5C`).
    EncodedSlash,
    /// A path that is not well formed: a bad escape, a backslash or a fragment.
    Malformed,
    /// A method and path the format's allowlist does not list.
    NotListed,
}

/// What a request is answered with, judged in memory against the run's route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Sent upstream, to this path and query (the base's path joined with the request's).
    Forward { path_and_query: String },
    /// Answered 404 with no body, never sent upstream: Claude Code's `HEAD /api/hello`.
    Absent,
    /// Refused, never sent upstream.
    Refused(Refusal),
}

impl Route {
    /// The route of a run on `profile` through `declaration`, for a harness speaking `format`:
    /// the declaration's endpoint of that format, or nothing when it has none.
    pub fn bind(profile: &str, declaration: &Declaration, format: Format) -> Option<Route> {
        let endpoint = declaration
            .endpoints
            .iter()
            .find(|endpoint| endpoint.format == format)?;
        Some(Route {
            profile: profile.to_owned(),
            declaration: declaration.id.clone(),
            digest: declaration.digest().to_owned(),
            format,
            base: endpoint.base.clone(),
            auth: endpoint.auth,
        })
    }

    /// The upstream URL for `target`, a request's target as it came: the origin, the base's
    /// path, then the target's path once normalised, then its query (R-084).
    pub fn upstream_url(&self, method: &str, target: &str) -> Result<String, Refusal> {
        let (path, query) = normalised(method, target)?;
        Ok(format!(
            "{}{}",
            self.base.origin(),
            joined(&self.base.upstream_path(&path), query)
        ))
    }

    /// What `method` on `target` is answered with (R-084, R-087, R-089).
    pub fn judge(&self, method: &str, target: &str) -> Verdict {
        let (path, query) = match normalised(method, target) {
            Ok(parts) => parts,
            Err(why) => return Verdict::Refused(why),
        };
        if (method, path.as_str()) == HELLO {
            return Verdict::Absent;
        }
        let listed: Vec<&str> = allowlist(self.format)
            .iter()
            .filter(|(listed, _)| *listed == method)
            .map(|(_, path)| *path)
            .collect();
        if !self.base.allows(&path, &listed) {
            return Verdict::Refused(Refusal::NotListed);
        }
        Verdict::Forward {
            path_and_query: joined(&self.base.upstream_path(&path), query),
        }
    }
}

/// `path`, then `query` after a `?` when there is one.
fn joined(path: &str, query: Option<&str>) -> String {
    match query {
        Some(query) => format!("{path}?{query}"),
        None => path.to_owned(),
    }
}

/// `target`'s path once normalised, and its query as it came; or why it is refused. The path's
/// escaped unreserved characters are decoded once and every other escape kept as written; an
/// escaped `/` or `\`, a `.` or `..` segment (written or escaped), a bad escape, a backslash and
/// a fragment are refused, as are `CONNECT` and every target that is not a path.
fn normalised<'a>(method: &str, target: &'a str) -> Result<(String, Option<&'a str>), Refusal> {
    if method.eq_ignore_ascii_case("CONNECT") {
        return Err(Refusal::Connect);
    }
    if !target.starts_with('/') {
        return Err(Refusal::NotAPath);
    }
    if target.contains(['#', '\\']) {
        return Err(Refusal::Malformed);
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    let mut decoded = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(at) = rest.find('%') {
        decoded.push_str(&rest[..at]);
        let byte = rest
            .get(at + 1..at + 3)
            .filter(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            .ok_or(Refusal::Malformed)?;
        match byte {
            b'/' | b'\\' => return Err(Refusal::EncodedSlash),
            b if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') => {
                decoded.push(char::from(b));
            }
            _ => decoded.push_str(&rest[at..at + 3]),
        }
        rest = &rest[at + 3..];
    }
    decoded.push_str(rest);
    if decoded.split('/').any(|segment| segment == "." || segment == "..") {
        return Err(Refusal::DotSegment);
    }
    Ok((decoded, query))
}

#[cfg(test)]
mod tests;

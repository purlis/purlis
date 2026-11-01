// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the browser sign-ins share (ADR 0077 §1 and §3): a forge's OAuth endpoints, one form
//! posted to one of them, and the grant its answer holds.
//!
//! The device flow ([`super::device`]), the PKCE code exchange and a refresh
//! ([`super::refresh`]) each post a small form to a forge's OAuth endpoint and read a JSON
//! answer. This module is that one request and the two things read from it: a [`Granted`] token
//! pair, or the forge's `error` word.
//!
//! **A public client, so no secret is ever sent.** Every form here carries purlis's public
//! client id ([`super::registration`]) and never a client secret: purlis has none.
//!
//! **What an answer holds is never printed.** An [`Answer`] has no `Debug` that shows its body,
//! every token read from one is a [`SecretString`] registered with
//! [`crate::applog::keep_out`] as it is taken, and the body's own copy is wiped then. A sentence
//! built from a forge's words goes through [`said_safely`], which takes out every value kept
//! out and keeps only a short, printable stretch.
//!
//! **Where a form may go.** Over HTTPS only, to no URL with user information, following no
//! redirect, as the native transport's requests go ([`super::http`]). A test build alone may
//! post to a stand-in on this machine's loopback.

use std::sync::Arc;
use std::time::{Duration, Instant};

use secrecy::SecretString;
use serde_json::{Map, Value};
use zeroize::Zeroize;

use super::Kind;
use super::registration::{Flow, Registration};

/// A forge's OAuth endpoints for one registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// Where a device flow asks for its codes; `None` for a registration that signs in by
    /// PKCE.
    pub device_code: Option<String>,
    /// Where a code is exchanged, a device flow is polled, and a token is refreshed.
    pub token: String,
    /// Where a token is revoked without a secret (RFC 7009), on a forge that lets a public
    /// client do it: GitLab. `None` on GitHub, whose revocation needs the app's client secret,
    /// which purlis does not have (ADR 0077 §3).
    pub revoke: Option<String>,
    /// The authority a page the person is sent to must be on.
    pub host: String,
}

impl Endpoints {
    /// The endpoints of `registration`, on its own host.
    pub fn of(registration: &Registration) -> Endpoints {
        let host = registration.host;
        match registration.kind {
            Kind::GitHub => Endpoints {
                device_code: (registration.flow == Flow::Device)
                    .then(|| format!("https://{host}/login/device/code")),
                token: format!("https://{host}/login/oauth/access_token"),
                revoke: None,
                host: host.to_string(),
            },
            Kind::GitLab => Endpoints {
                device_code: None,
                token: format!("https://{host}/oauth/token"),
                revoke: Some(format!("https://{host}/oauth/revoke")),
                host: host.to_string(),
            },
        }
    }

    /// `kind`'s endpoints served by a stand-in at `base`, on this machine's loopback. Only a
    /// test build has it.
    #[cfg(any(test, feature = "fenced"))]
    pub fn at(kind: Kind, base: &str) -> Endpoints {
        let base = base.trim_end_matches('/');
        let host = base
            .split_once("://")
            .map_or(base, |(_, rest)| rest)
            .to_string();
        match kind {
            Kind::GitHub => Endpoints {
                device_code: Some(format!("{base}/login/device/code")),
                token: format!("{base}/login/oauth/access_token"),
                revoke: None,
                host,
            },
            Kind::GitLab => Endpoints {
                device_code: None,
                token: format!("{base}/oauth/token"),
                revoke: Some(format!("{base}/oauth/revoke")),
                host,
            },
        }
    }
}

/// A forge's answer to a posted form: its status, and its JSON object.
pub struct Answer {
    pub status: u16,
    body: Map<String, Value>,
}

impl std::fmt::Debug for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Answer")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        wipe(&mut self.body);
    }
}

/// Wipes every string `body` still holds.
fn wipe(body: &mut Map<String, Value>) {
    for (_, value) in body.iter_mut() {
        if let Value::String(text) = value {
            text.zeroize();
        }
    }
}

impl Answer {
    /// The answer whose body is the JSON object in `text`; an empty object when `text` is no
    /// JSON object, which reads as an answer that says nothing.
    pub fn of(status: u16, text: &str) -> Answer {
        let body = match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        Answer { status, body }
    }

    /// The text at `key`, when it is a string that is not empty.
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.body.get(key)
    }

    /// A key's text, where it is a string that is not empty.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.body
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    /// The whole seconds at `key`: a number, or a string of digits.
    pub fn seconds(&self, key: &str) -> Option<Duration> {
        let value = self.body.get(key)?;
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
            .map(Duration::from_secs)
    }

    /// The text at `key` taken out as a secret: registered with [`crate::applog::keep_out`],
    /// and the body's own copy wiped.
    pub fn secret(&mut self, key: &str) -> Option<SecretString> {
        let Some(Value::String(mut text)) = self.body.remove(key) else {
            return None;
        };
        let held = (!text.is_empty()).then(|| SecretString::from(text.as_str()));
        text.zeroize();
        if let Some(held) = &held {
            crate::applog::keep_out(held);
        }
        held
    }

    /// The forge's OAuth `error` word (RFC 6749 §5.2), when the answer is a refusal.
    pub fn error(&self) -> Option<&str> {
        self.text("error")
    }

    /// The token pair this answer grants, or `None` when it grants none.
    pub fn granted(mut self) -> Option<Granted> {
        if self.error().is_some() {
            return None;
        }
        let scopes = self.text("scope").map(|listed| {
            listed
                .split([',', ' '])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        });
        let expires_in = self.seconds("expires_in");
        let refresh = self.secret("refresh_token");
        let access = self.secret("access_token")?;
        Some(Granted {
            access,
            refresh,
            expires_in,
            scopes,
        })
    }
}

/// What a forge granted: the access token, and the refresh token and lifetime where the
/// registration's tokens expire.
pub struct Granted {
    pub access: SecretString,
    pub refresh: Option<SecretString>,
    /// How long the access token lasts from when it was granted.
    pub expires_in: Option<Duration>,
    /// The scopes the forge says the token has, when it says.
    pub scopes: Option<Vec<String>>,
}

impl std::fmt::Debug for Granted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Granted")
            .field("scopes", &self.scopes)
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

/// The most of a forge's own words a sentence repeats.
const MOST_SAID: usize = 200;

/// A forge's own words, safe to put in a sentence the person reads: every value this process
/// keeps out taken out, control characters dropped, and cut short.
pub fn said_safely(said: &str) -> String {
    let said = crate::applog::without_values_kept_out(said);
    said.chars()
        .filter(|c| !c.is_control())
        .take(MOST_SAID)
        .collect()
}

/// Posts a form to a forge's OAuth endpoint. The seam a test answers from a stand-in.
pub trait Endpoint: Send + Sync {
    /// `form` posted to `url`, and the forge's answer whatever its status. `Err` only when
    /// nothing answered, in words that hold none of `form`'s values.
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<Answer, String>;
}

/// The most an OAuth endpoint's answer may be: a few hundred bytes in practice.
const MOST_BODY: u64 = 64 * 1024;

/// The production [`Endpoint`]: the form sent over HTTPS, each request listed in the network
/// log by its host and path and nothing else.
pub struct Https {
    agent: ureq::Agent,
}

impl Https {
    pub fn new() -> Https {
        let agent = ureq::Agent::config_builder()
            // A refusal is an answer: the flow reads its `error`.
            .http_status_as_error(false)
            // A redirect could take a code or a refresh token to another host.
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("purlis/", env!("CARGO_PKG_VERSION")))
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .new_agent();
        Https { agent }
    }

    /// The production endpoint, as the flows take it.
    pub fn shared() -> Arc<dyn Endpoint> {
        Arc::new(Https::new())
    }
}

impl Default for Https {
    fn default() -> Self {
        Https::new()
    }
}

/// `form` as an `application/x-www-form-urlencoded` body, wiped when it goes.
fn encoded(form: &[(&str, &str)]) -> zeroize::Zeroizing<String> {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let mut body = zeroize::Zeroizing::new(String::new());
    for (name, value) in form {
        if !body.is_empty() {
            body.push('&');
        }
        body.extend(utf8_percent_encode(name, NON_ALPHANUMERIC));
        body.push('=');
        body.extend(utf8_percent_encode(value, NON_ALPHANUMERIC));
    }
    body
}

impl Endpoint for Https {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<Answer, String> {
        // HTTPS only, no user information; plain HTTP to loopback in a test build alone.
        let (_, authority) = super::http::checked(url)?;
        let path = url
            .parse::<http::Uri>()
            .map(|uri| uri.path().to_string())
            .unwrap_or_default();
        let body = encoded(form);
        let request = http::Request::builder()
            .method(http::Method::POST)
            .uri(url)
            .header(http::header::ACCEPT, "application/json")
            .header(
                http::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body.as_bytes().to_vec())
            .map_err(|e| e.to_string())?;
        let started = Instant::now();
        let response = self.agent.run(request);
        // The network log: host, method, path and status; never the form or the answer.
        crate::netlog::record(crate::netlog::Call {
            feature: crate::netlog::Feature::Forge,
            via: crate::netlog::Via::Https,
            host: &authority,
            method: "POST",
            path: &path,
            status: response.as_ref().ok().map(|r| r.status().as_u16()),
            answered: response.is_ok(),
            took: started.elapsed(),
        });
        let response = response.map_err(|e| said_safely(&format!("{authority}: {e}")))?;
        let status = response.status().as_u16();
        let mut bytes = response
            .into_body()
            .into_with_config()
            .limit(MOST_BODY)
            .read_to_vec()
            .map_err(|e| said_safely(&format!("reading {authority}'s answer: {e}")))?;
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        let answer = Answer::of(status, &text);
        text.zeroize();
        bytes.zeroize();
        Ok(answer)
    }
}

#[cfg(test)]
mod tests;

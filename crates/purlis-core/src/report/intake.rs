// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Report intake adapter** (#5689, spec #5683, ADR 0118): the one outward adapter that
//! sends a report to purlis's private Report intake, Sentry SaaS in the EU (ADR 0083, ruling O9).
//!
//! It speaks Sentry's envelope format (develop.sentry.dev/sdk/data-model/envelopes/): one
//! envelope header line, one item header line, and the item, POSTed to the project's
//! `/api/<project>/envelope/` with the DSN's public key in `X-Sentry-Auth`. The DSN is public by
//! design (ADR 0118): it lets anyone send to the intake and no one read it. It is built into the
//! binary from `PURLIS_REPORT_INTAKE_DSN` at build time ([`built_in`]); a build without one has no
//! intake, and sends nothing.
//!
//! Two halves, so the adapter is tested whole against a recording: [`Sentry`] makes the request
//! from the envelope's bytes, and a [`Post`] carries it. [`Https`] is purlis's own HTTPS client,
//! and lists every request in the network log as the intake's ([`crate::netlog::Feature::ReportIntake`]);
//! [`Recorded`] answers from recorded exchanges, as the forge's recorded transport does.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// The DSN this build sends to, if it was built with one (#5687 wires it into release builds).
///
/// A fenced build (a scenario test's) reads `PURLIS_REPORT_INTAKE_DSN` at run time first, so a
/// test sends to a recording of its own (`tools/crash-relaunch.mjs`). No shipped build is fenced.
pub fn built_in() -> Option<Dsn> {
    #[cfg(feature = "fenced")]
    if let Some(said) = crate::envvar::var_os("PURLIS_REPORT_INTAKE_DSN") {
        return Dsn::parse(&said.to_string_lossy()).ok();
    }
    option_env!("PURLIS_REPORT_INTAKE_DSN").and_then(|said| Dsn::parse(said).ok())
}

/// A Sentry DSN: `https://<public key>@<host>/<project>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dsn {
    said: String,
    scheme: String,
    key: String,
    /// The host, with its port when it has one.
    host: String,
    project: String,
}

impl Dsn {
    /// The DSN `said` names, or why it is not one.
    pub fn parse(said: &str) -> Result<Dsn, String> {
        let not = || format!("{said:?} is not a Sentry DSN (https://<key>@<host>/<project>)");
        let (scheme, rest) = said.split_once("://").ok_or_else(not)?;
        if !matches!(scheme, "https" | "http") {
            return Err(not());
        }
        let (key, rest) = rest.split_once('@').ok_or_else(not)?;
        let (host, project) = rest.split_once('/').ok_or_else(not)?;
        let project = project.trim_end_matches('/');
        let plain = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric());
        if !plain(key) || host.is_empty() || host.contains(['/', '@']) || !plain(project) {
            return Err(not());
        }
        Ok(Dsn {
            said: said.to_string(),
            scheme: scheme.to_string(),
            key: key.to_string(),
            host: host.to_string(),
            project: project.to_string(),
        })
    }

    /// The DSN as it was written.
    pub fn as_str(&self) -> &str {
        &self.said
    }

    /// Where an envelope is POSTed.
    pub fn envelope_url(&self) -> String {
        format!(
            "{}://{}/api/{}/envelope/",
            self.scheme, self.host, self.project
        )
    }

    /// The `X-Sentry-Auth` header's value: the public key, and who is sending.
    pub fn auth(&self) -> String {
        format!(
            "Sentry sentry_version=7, sentry_key={}, sentry_client=purlis/{}",
            self.key,
            env!("CARGO_PKG_VERSION")
        )
    }
}

/// One request, as a [`Post`] is handed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// What came back: the HTTP status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
}

/// Carries a request to the intake and back.
pub trait Post: Send + Sync {
    /// The intake's answer, or why none came.
    fn post(&self, request: &Request) -> Result<Answer, String>;
}

/// Why the intake did not take a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// No answer came: this machine is offline, or the intake cannot be reached. The report
    /// waits on this machine and is sent when the intake answers again (#5693, Q24).
    NoAnswer(String),
    /// The intake answered, and did not take it.
    Refused(String),
}

/// Where reports go: the Report intake, as a [`super::Reports`] holds it.
pub trait Intake: Send + Sync {
    /// The DSN an envelope names in its header.
    fn dsn(&self) -> &str;

    /// Sends `envelope`'s bytes, and answers with the address it was kept at, or why not.
    fn send(&self, envelope: &[u8], event_id: &str) -> Result<String, Failed>;
}

/// The intake at a Sentry DSN, over `post`.
pub struct Sentry<P> {
    dsn: Dsn,
    post: P,
}

impl<P: Post> Sentry<P> {
    pub fn new(dsn: Dsn, post: P) -> Sentry<P> {
        Sentry { dsn, post }
    }
}

impl<P: Post> Intake for Sentry<P> {
    fn dsn(&self) -> &str {
        self.dsn.as_str()
    }

    fn send(&self, envelope: &[u8], event_id: &str) -> Result<String, Failed> {
        let answer = self
            .post
            .post(&Request {
                url: self.dsn.envelope_url(),
                headers: vec![
                    (
                        "content-type".to_string(),
                        "application/x-sentry-envelope".to_string(),
                    ),
                    ("x-sentry-auth".to_string(), self.dsn.auth()),
                ],
                body: envelope.to_vec(),
            })
            .map_err(Failed::NoAnswer)?;
        if (200..300).contains(&answer.status) {
            Ok(format!("purlis's report service, event {event_id}"))
        } else {
            Err(Failed::Refused(format!(
                "purlis's report service answered {}",
                answer.status
            )))
        }
    }
}

/// purlis's own HTTPS client, every request listed in the network log.
pub struct Https {
    agent: ureq::Agent,
}

impl Default for Https {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            // A refusal is an answer: the adapter reads it.
            .http_status_as_error(false)
            .max_redirects(0)
            // A crash report is sent from a panicking process on its way out.
            .timeout_global(Some(Duration::from_secs(5)))
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
}

impl Post for Https {
    fn post(&self, request: &Request) -> Result<Answer, String> {
        let started = Instant::now();
        let mut sent = self.agent.post(&request.url);
        for (name, value) in &request.headers {
            sent = sent.header(name, value);
        }
        let said = sent.send(&request.body[..]);
        let status = said.as_ref().ok().map(|answer| answer.status().as_u16());
        if let Ok(uri) = request.url.parse::<http::Uri>()
            && let Some(host) = uri.authority().map(|a| a.as_str().to_string())
        {
            crate::netlog::record(crate::netlog::Call {
                feature: crate::netlog::Feature::ReportIntake,
                via: crate::netlog::Via::Https,
                host: &host,
                method: "POST",
                path: uri.path(),
                status,
                answered: status.is_some(),
                took: started.elapsed(),
            });
        }
        match said {
            Ok(answer) => Ok(Answer {
                status: answer.status().as_u16(),
            }),
            Err(err) => Err(format!("purlis's report service did not answer: {err}")),
        }
    }
}

/// One recorded request (its URL's path and headers, never its body, which carries a new event
/// id each time) and the status it was answered with.
#[derive(Debug, Clone, Deserialize)]
pub struct Exchange {
    pub path: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    pub status: u16,
}

#[derive(Debug, Deserialize)]
struct File {
    #[allow(dead_code)]
    source: String,
    exchanges: Vec<Exchange>,
}

/// A [`Post`] that answers from recorded exchanges, each once and in order, and keeps every body
/// it was handed. A request that matches none is refused, naming it.
#[derive(Debug)]
pub struct Recorded {
    exchanges: Mutex<Vec<(Exchange, bool)>>,
    received: Mutex<Vec<Vec<u8>>>,
}

impl Recorded {
    /// A recording read from its JSON text.
    pub fn parse(text: &str) -> Result<Recorded, String> {
        let file: File = serde_json::from_str(text).map_err(|e| format!("a recording: {e}"))?;
        Ok(Recorded {
            exchanges: Mutex::new(file.exchanges.into_iter().map(|e| (e, false)).collect()),
            received: Mutex::new(Vec::new()),
        })
    }

    /// Every body the intake was sent and answered, oldest first.
    pub fn received(&self) -> Vec<Vec<u8>> {
        self.received
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The recorded requests nobody sent.
    pub fn unspent(&self) -> Vec<String> {
        self.exchanges
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(_, spent)| !spent)
            .map(|(e, _)| e.path.clone())
            .collect()
    }
}

impl Post for Recorded {
    fn post(&self, request: &Request) -> Result<Answer, String> {
        let path = request
            .url
            .parse::<http::Uri>()
            .map(|uri| uri.path().to_string())
            .unwrap_or_default();
        let mut exchanges = self.exchanges.lock().unwrap_or_else(|e| e.into_inner());
        let Some((exchange, spent)) = exchanges.iter_mut().find(|(e, spent)| {
            !spent
                && e.path == path
                && e.headers
                    .iter()
                    .all(|wanted| request.headers.contains(wanted))
        }) else {
            return Err(format!(
                "nothing recorded for POST {path} with {:?}",
                request.headers
            ));
        };
        *spent = true;
        self.received
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(request.body.clone());
        Ok(Answer {
            status: exchange.status,
        })
    }
}

impl<P: Post> Post for std::sync::Arc<P> {
    fn post(&self, request: &Request) -> Result<Answer, String> {
        self.as_ref().post(request)
    }
}

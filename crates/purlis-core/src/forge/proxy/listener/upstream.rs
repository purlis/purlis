// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How the listener speaks to the forge (ADR 0128 D3a): **TLS with the system's roots, the chain and
//! the host name checked strictly, no fallback**, and nothing of the chat's environment.
//!
//! - the roots are the operating system's ([`RootCerts::PlatformVerifier`]), so a GHES behind a
//!   company authority verifies as the browser does; **the per-chat authority is never one of
//!   them** (it is the listener's own, for the chat's side of the connection);
//! - `https` only, and **no proxy**: the chat's `HTTPS_PROXY` is the listener itself, and the
//!   upstream client reads no environment (`proxy(None)`);
//! - **no redirect is followed** here: the listener decides each one;
//! - **no idle connection is kept**: one request per upstream connection, so a connection's state
//!   after one request is never another request's.
//!
//! [`Upstream`] is a trait so that a test can stand a fake forge in the place of a real one with
//! roots and addresses of its own; the shipped one is [`StrictUpstream`], and takes neither.
//!
//! [`RootCerts::PlatformVerifier`]: ureq::tls::RootCerts::PlatformVerifier

use std::io::Read;
use std::time::Duration;

use ureq::config::AutoHeaderValue;

/// One request to a forge: the address the proxy built, the headers it decided and the body.
#[derive(Debug, Clone)]
pub struct UpstreamRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// The forge's answer, with its body still to be read.
pub struct UpstreamResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

impl std::fmt::Debug for UpstreamResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpstreamResponse")
            .field("status", &self.status)
            .field("headers", &self.headers.len())
            .finish()
    }
}

/// The forge could not be reached, or did not answer as HTTP.
#[derive(Debug)]
pub struct UpstreamError(pub String);

impl std::fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UpstreamError {}

/// Sends a request to a forge.
pub trait Upstream: Send + Sync {
    fn send(&self, request: UpstreamRequest) -> Result<UpstreamResponse, UpstreamError>;
}

/// The upstream the app uses: strict TLS against the system's roots.
pub struct StrictUpstream {
    agent: ureq::Agent,
}

impl StrictUpstream {
    #[must_use]
    pub fn system_roots() -> Self {
        let agent = ureq::Agent::config_builder()
            // The chat's `HTTPS_PROXY` is this listener: the upstream reads no environment.
            .proxy(None)
            .https_only(true)
            // A refusal is an answer: it is passed on.
            .http_status_as_error(false)
            .max_redirects(0)
            .max_idle_connections(0)
            .max_idle_connections_per_host(0)
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(120)))
            // A clone streams for as long as it takes; each read is bounded, not the whole.
            .timeout_per_read(Some(Duration::from_secs(120)))
            .timeout_per_write(Some(Duration::from_secs(120)))
            // The listener frames what it sends and reads what it gets: nothing is added to or
            // decoded in a request or an answer.
            .accept_encoding(AutoHeaderValue::None)
            .accept(AutoHeaderValue::None)
            .user_agent(AutoHeaderValue::None)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .new_agent();
        Self { agent }
    }

    /// An upstream over `agent`, which a test builds with roots and addresses of its own. Not
    /// for the app: [`StrictUpstream::system_roots`] is.
    #[must_use]
    pub fn over(agent: ureq::Agent) -> Self {
        Self { agent }
    }
}

impl Upstream for StrictUpstream {
    fn send(&self, request: UpstreamRequest) -> Result<UpstreamResponse, UpstreamError> {
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(request.url.as_str());
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        // An empty body is none: a read carries no `Content-Length` of its own making.
        let response = if request.body.is_empty() {
            let request = builder
                .body(())
                .map_err(|e| UpstreamError(format!("the request could not be made: {e}")))?;
            self.agent.run(request)
        } else {
            let request = builder
                .body(request.body)
                .map_err(|e| UpstreamError(format!("the request could not be made: {e}")))?;
            self.agent.run(request)
        }
        .map_err(|e| UpstreamError(e.to_string()))?;
        let (parts, body) = response.into_parts();
        let headers = parts
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        Ok(UpstreamResponse {
            status: parts.status.as_u16(),
            headers,
            body: Box::new(body.into_reader()),
        })
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The proxy's host against curl's and Node's** (T1.1-47): for one URL, the host the egress
//! proxy judges and the host the client meant must be the same host, or the proxy must refuse the
//! request. A proxy that reads another host than the client asked for is judging a request
//! nobody made.
//!
//! An input is a URL, and, after its first line, what a client sent the proxy for it:
//!
//! - **a URL alone** is read by the `url` crate, which implements the WHATWG URL Standard that
//!   Node's `URL` and `http` implement. The target sends the proxy the head Node sends for it
//!   (absolute form and `Host` for `http:`, `CONNECT host:port` for `https:`, as
//!   `clients/record.mjs` recorded), and also the URL as written, as a hand-made client may.
//! - **a URL and a head** is a head curl or Node really sent through a proxy, recorded by
//!   `clients/record.mjs` (`seeds/differential_host_parse_against_curl_and_node/`, and in CI
//!   fresh from the runner's curl and Node). The proxy must read the host the client named on
//!   its request line, and that host must be the URL's.
//!
//! Each disagreement this target found is fixed in `egress::Request::parse` or written here as
//! why it is not one (`docs/fuzzing.md`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use purlis_core::sandbox::fuzzing::{Head, host_header_agrees, proxy_request_head};
use url::{Host, Url};

/// A host as a resolver takes it: lower case, without IPv6 brackets or a final dot.
fn plain(host: &str) -> String {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

/// The host `url` names, and the port a client connects to for it.
fn meant(url: &Url) -> Option<(String, u16)> {
    let host = match url.host()? {
        Host::Domain(name) => name.to_owned(),
        Host::Ipv4(ip) => ip.to_string(),
        Host::Ipv6(ip) => ip.to_string(),
    };
    Some((plain(&host), url.port_or_known_default()?))
}

/// The head Node sends a proxy for `url` (`clients/record.mjs` recorded its shape).
fn as_node_sends(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    let authority = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    match url.scheme() {
        "http" => {
            let mut target = format!("http://{authority}{}", url.path());
            if let Some(query) = url.query() {
                target.push('?');
                target.push_str(query);
            }
            Some(format!(
                "GET {target} HTTP/1.1\r\nHost: {authority}\r\nproxy-connection: close\r\n\
                 Connection: close\r\n\r\n"
            ))
        }
        "https" => {
            let port = url.port_or_known_default()?;
            Some(format!(
                "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\
                 Connection: keep-alive\r\n\r\n"
            ))
        }
        _ => None,
    }
}

/// The proxy's reading of `head`, where it carries it past its `Host` check.
fn carried(head: &str) -> Option<Head> {
    proxy_request_head(head).filter(host_header_agrees)
}

/// Fails where the proxy carries `head` to another host than `wanted`.
fn same_or_refused(head: &str, wanted: &(String, u16), url: &str) {
    if let Some(read) = carried(head) {
        assert_eq!(
            (plain(&read.host), read.port),
            *wanted,
            "the proxy read another host than the client meant for {url:?}: {head:?}"
        );
    }
}

fuzz_target!(|text: &str| {
    let (written, sent) = match text.split_once('\n') {
        Some((url, head)) => (url, Some(head)),
        None => (text, None),
    };
    let Ok(url) = Url::parse(written) else {
        // A URL no client parses, the proxy may read as it likes: no client sends it.
        return;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return;
    }
    let Some(wanted) = meant(&url) else {
        return;
    };
    match sent {
        Some(head) => {
            // What a real client sent: carried to the host on its request line, which is the
            // URL's.
            same_or_refused(head, &wanted, written);
        }
        None => {
            if let Some(head) = as_node_sends(&url) {
                same_or_refused(&head, &wanted, written);
            }
            // The URL as written, sent by a client that copies it whole.
            if let Some(rest) = written.strip_prefix("http://") {
                let authority = rest.split('/').next().unwrap_or(rest);
                let head = format!(
                    "GET http://{rest} HTTP/1.1\r\nHost: {authority}\r\n\r\n"
                );
                same_or_refused(&head, &wanted, written);
            }
            if let Some(rest) = written.strip_prefix("https://") {
                let authority = rest.split('/').next().unwrap_or(rest);
                let head = format!("CONNECT {authority} HTTP/1.1\r\n\r\n");
                same_or_refused(&head, &wanted, written);
            }
        }
    }
});

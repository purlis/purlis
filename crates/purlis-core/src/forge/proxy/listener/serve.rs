// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One request, from the chat's client to the forge and back.
//!
//! The listener reads a request ([`http`]), builds the [`RequestHead`] the proxy's core decides
//! (the host is the one the chat asked for: `github.localhost`'s alias on the unix socket, the
//! `CONNECT` target inside TLS), and asks [`ForgeProxy::handle`]. A refusal is answered `403`; an
//! allowed request goes to the [`Upstream`] with the address and headers **the core built**, never
//! the chat's, and the answer is read ([`scrub`]) before it reaches the chat.
//!
//! **Redirects are never followed with a credential.** On `gh`'s plain socket the client cannot
//! reach a storage host itself, so a redirect to a download host (a release asset, an archive) is
//! followed **once, anonymously**, and its answer streamed; a redirect to the recorded host is
//! written again for the chat; any other is answered `502`. Inside TLS the chat follows a redirect
//! to a download host itself (it is tunnelled), and one to a recorded host is its own request.
//!
//! **Bodies.** A response streams in a bounded window. A request body is held (a JSON body up to
//! [`Limits::body`](super::Limits::body), a push up to [`Limits::push`](super::Limits::push)): the
//! core decides on a push's ref commands before one byte of it reaches the forge, and a chunked
//! body is sent on with the length it turned out to have.

use std::io::{self, Read, Write};

use url::Url;

use super::super::{
    BodyNeed, Class, Forwarded, Inbound, Outcome, Refusal, RequestHead, is_download_host,
};
use super::Wiring;
use super::http::{
    BodyError, Framed, Framing, Head, HeadError, finish_chunks, framing_of, message, write_answer,
    write_chunk, write_head,
};
use super::lfs;
use super::scrub::{
    ChatSide, Scanner, advertised_branches, is_dropped_header, is_text, push_fully_accepted,
};
use super::upstream::{UpstreamRequest, UpstreamResponse};
use crate::forge::proxy::{framing_is_unambiguous, header_names_are_plain, normal};

/// Where a request arrived.
pub(crate) enum Route<'a> {
    /// Plain HTTP on the chat's unix socket: `gh`, naming the forge by `Host`.
    Plain,
    /// Inside TLS the proxy terminated for the `CONNECT` target.
    Tls { host: &'a str, port: u16 },
}

/// Serve one request on `stream` and close it. Nothing here panics on what a client sends, and an
/// I/O error only ends the connection.
pub(crate) fn serve_one<S: Read + Write>(wiring: &Wiring, stream: S, route: &Route<'_>) {
    let mut framed = Framed::new(stream);
    let _ = serve(wiring, &mut framed, route);
}

fn serve<S: Read + Write>(w: &Wiring, framed: &mut Framed<S>, route: &Route<'_>) -> io::Result<()> {
    let head = match framed.read_head(w.limits.head) {
        Ok(head) => head,
        Err(HeadError::Closed | HeadError::Io) => return Ok(()),
        Err(HeadError::TooLarge) => {
            return write_answer(
                &mut framed.stream,
                400,
                &message("the request is too large"),
            );
        }
        Err(HeadError::Malformed) => {
            return write_answer(
                &mut framed.stream,
                400,
                &message("not a request purlis reads"),
            );
        }
    };
    let Head {
        method,
        target,
        mut headers,
    } = head;

    // A storage address the listener handed the chat in an LFS batch: fetched here, for it.
    if let Route::Tls { host, port } = route
        && method.eq_ignore_ascii_case("GET")
        && let Some(token) = target.strip_prefix(lfs::PATH_PREFIX)
    {
        let asked = RequestHead::new("GET", host, *port, &target);
        return serve_storage(w, &mut framed.stream, &asked, token, &headers);
    }

    // Where it is for.
    let (rh, alias) = match route {
        Route::Tls { host, port } => (RequestHead::new(&method, host, *port, &target), None),
        Route::Plain => {
            let (host, port) = plain_host(&headers);
            match w.proxy.github_alias(&w.chat, &host) {
                Some(Ok(alias)) => {
                    let value = if alias.port == 443 {
                        alias.host.clone()
                    } else {
                        format!("{}:{}", alias.host, alias.port)
                    };
                    headers.retain(|(n, _)| normal(n) != "host");
                    headers.push(("Host".to_string(), value));
                    let mapped = alias.target(&target);
                    (
                        RequestHead::new(&method, &alias.host, alias.port, &mapped),
                        Some(alias),
                    )
                }
                Some(Err(refusal)) => {
                    let asked = RequestHead::new(&method, &host, port.unwrap_or(80), &target);
                    w.proxy.note_refused(&w.chat, &asked, refusal);
                    return refused(&mut framed.stream, refusal);
                }
                None => (
                    RequestHead::new(&method, &host, port.unwrap_or(80), &target),
                    None,
                ),
            }
        }
    };

    // Its body, if the head is one the core will read at all.
    let clean = framing_is_unambiguous(&headers) && header_names_are_plain(&headers);
    let need = w.proxy.body_need(&rh);
    let limit = match need {
        BodyNeed::PushCommands => w.limits.push,
        _ => w.limits.body,
    };
    let mut body = Vec::new();
    if clean {
        let framing = framing_of(&headers);
        if framing != Framing::None && expects_continue(&headers) {
            framed.stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
            framed.stream.flush()?;
        }
        match framed.read_body(framing, limit) {
            Ok(read) => body = read,
            Err(BodyError::TooLarge) => {
                return write_answer(
                    &mut framed.stream,
                    413,
                    &message("the request body is larger than purlis carries"),
                );
            }
            Err(BodyError::Malformed) => {
                return write_answer(&mut framed.stream, 400, &message("not a body purlis reads"));
            }
            Err(BodyError::Io) => return Ok(()),
        }
    }
    headers.retain(|(n, _)| normal(n) != "expect");
    if let Some(observer) = &w.observe {
        observer.request(&rh, &body);
    }

    let outcome = w.proxy.handle(
        &w.chat,
        &Inbound {
            head: &rh,
            headers: &headers,
            body: &body,
        },
    );
    match outcome {
        Outcome::Refused(refusal) => refused(&mut framed.stream, refusal),
        Outcome::Forward(forwarded) => {
            let hosts = w.proxy.terminable_hosts(&w.chat);
            let side = ChatSide {
                hosts: &hosts,
                alias: alias.as_ref(),
            };
            forward(w, &mut framed.stream, &rh, forwarded, body, &side)
        }
    }
}

fn refused<W: Write>(out: &mut W, refusal: Refusal) -> io::Result<()> {
    // A request with no credential at all is **challenged**, not refused outright: `git` sends its
    // first request with none and asks its credential helper (which answers the sentinel) only when
    // it is told to by a `401`. A wrong credential, or any other refusal, stays `403`.
    if refusal == Refusal::NoSentinel {
        let body = message("purlis needs this chat's credential for this request: NoSentinel");
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
            (
                "WWW-Authenticate".to_string(),
                "Basic realm=\"purlis\", charset=\"UTF-8\"".to_string(),
            ),
        ];
        write_head(out, 401, &headers)?;
        out.write_all(body.as_bytes())?;
        return out.flush();
    }
    write_answer(
        out,
        403,
        &message(&format!("purlis refused this request: {refusal:?}")),
    )
}

/// The host and port a plain request names in its `Host`. A request with none, or two, names
/// nothing, and the core refuses the empty host it is given.
fn plain_host(headers: &[(String, String)]) -> (String, Option<u16>) {
    let mut values = headers
        .iter()
        .filter(|(n, _)| normal(n) == "host")
        .map(|(_, v)| v.trim());
    let (Some(value), None) = (values.next(), values.next()) else {
        return (String::new(), None);
    };
    match value.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => (host.to_string(), Some(port)),
            Err(_) => (String::new(), None),
        },
        None => (value.to_string(), None),
    }
}

fn expects_continue(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .any(|(n, v)| normal(n) == "expect" && v.eq_ignore_ascii_case("100-continue"))
}

fn header<'a>(headers: &'a [(String, String)], wanted: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| normal(n) == wanted)
        .map(|(_, v)| v.as_str())
}

/// Headers of the request that go to the forge: those the core built, less what belongs to the
/// connection or to a protocol the listener does not carry on.
fn upstream_headers(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(n, _)| {
            !matches!(
                normal(n).as_str(),
                "host"
                    | "connection"
                    | "keep-alive"
                    | "proxy-connection"
                    | "te"
                    | "trailer"
                    | "upgrade"
                    | "expect"
                    | "accept-encoding"
                    | "content-length"
                    | "transfer-encoding"
            )
        })
        .cloned()
        .collect()
}

fn forward<W: Write>(
    w: &Wiring,
    out: &mut W,
    rh: &RequestHead,
    forwarded: Forwarded,
    chat_body: Vec<u8>,
    side: &ChatSide<'_>,
) -> io::Result<()> {
    let body = forwarded.body.clone().unwrap_or(chat_body);
    let request = UpstreamRequest {
        method: rh.method.to_ascii_uppercase(),
        url: forwarded.upstream.clone(),
        headers: upstream_headers(&forwarded.headers),
        body,
    };
    match w.upstream.send(request) {
        Ok(response) => answer(w, out, rh, &forwarded, response, side, false),
        Err(e) => write_answer(
            out,
            502,
            &message(&format!("purlis could not reach the forge: {e}")),
        ),
    }
}

/// What the chat is sent of the forge's answer.
fn answer<W: Write>(
    w: &Wiring,
    out: &mut W,
    rh: &RequestHead,
    forwarded: &Forwarded,
    mut response: UpstreamResponse,
    side: &ChatSide<'_>,
    followed: bool,
) -> io::Result<()> {
    let needles: Vec<Vec<u8>> = forwarded.leak_needles().map(<[u8]>::to_vec).collect();
    let content_type = header(&response.headers, "content-type").map(str::to_string);
    let mut scanner = Scanner::new(needles, is_text(content_type.as_deref()));
    for (_, value) in &response.headers {
        if scanner.holds(value.as_bytes()) {
            return write_answer(
                out,
                502,
                &message("purlis did not pass the forge's answer on"),
            );
        }
    }
    let base = Url::parse(&forwarded.upstream).ok();
    let status = response.status;

    // A redirect.
    if matches!(status, 301 | 302 | 303 | 307 | 308)
        && let (Some(location), Some(base)) = (header(&response.headers, "location"), &base)
    {
        return redirect(
            w, out, rh, forwarded, &response, location, base, side, followed,
        );
    }
    if let Some(encoding) = header(&response.headers, "content-encoding")
        && !encoding.eq_ignore_ascii_case("identity")
    {
        return write_answer(out, 502, &message("the forge's answer is encoded"));
    }

    let headers = chat_headers(&response.headers, base.as_ref(), side);
    let no_body = rh.method.eq_ignore_ascii_case("HEAD") || matches!(status, 204 | 304);
    if no_body {
        write_head(
            out,
            status,
            &with_length(headers, header(&response.headers, "content-length")),
        )?;
        return out.flush();
    }

    // What the core is told of what the forge said: a push's report and its advertisement.
    let buffered_limit = match forwarded.class {
        Class::GitPush => Some(1 << 20),
        Class::LfsBatch if status == 200 => Some(4 << 20),
        Class::GitRefs if forwarded.upstream.contains("service=git-receive-pack") => Some(64 << 20),
        _ => None,
    };
    if let Some(limit) = buffered_limit {
        let mut raw = Vec::new();
        let read = response
            .body
            .by_ref()
            .take(limit as u64 + 1)
            .read_to_end(&mut raw);
        if read.is_err() || raw.len() > limit {
            return write_answer(
                out,
                502,
                &message("the forge's answer is too large to read"),
            );
        }
        // An LFS batch's answer names storage on other hosts, with credentials of their own: the
        // chat is given addresses on the host it reached, and the rest stays here.
        let for_chat = if forwarded.class == Class::LfsBatch {
            match lfs::rewrite_batch(&raw, &rh.host, side.hosts, &w.storage) {
                Ok(rewritten) => rewritten,
                Err(sentence) => return write_answer(out, 502, &message(sentence)),
            }
        } else {
            raw.clone()
        };
        let Ok(mut safe) = scanner.feed(&for_chat) else {
            return write_answer(
                out,
                502,
                &message("purlis did not pass the forge's answer on"),
            );
        };
        let Ok(rest) = scanner.finish() else {
            return write_answer(
                out,
                502,
                &message("purlis did not pass the forge's answer on"),
            );
        };
        safe.extend(rest);
        if status == 200 {
            match forwarded.class {
                Class::GitPush if push_fully_accepted(&raw) => {
                    w.proxy.confirm_push(&w.chat, forwarded);
                }
                Class::GitRefs => {
                    w.proxy.note_advertised(
                        &w.chat,
                        &forwarded.host,
                        &forwarded.repo,
                        advertised_branches(&raw),
                    );
                }
                _ => {}
            }
        }
        write_head(
            out,
            status,
            &with_length(headers, Some(&safe.len().to_string())),
        )?;
        out.write_all(&safe)?;
        return out.flush();
    }

    // Everything else streams. The first read is scanned before anything is sent, so an answer
    // that holds the credential in its first bytes is refused whole.
    let known = header(&response.headers, "content-length").map(str::to_string);
    let redacts = is_text(content_type.as_deref());
    let mut buf = vec![0u8; 32 * 1024];
    let first = read_some(&mut response.body, &mut buf)?;
    let mut ready = match scanner.feed(&buf[..first]) {
        Ok(ready) => ready,
        Err(_) => {
            return write_answer(
                out,
                502,
                &message("purlis did not pass the forge's answer on"),
            );
        }
    };
    let by_length = known.as_deref().filter(|_| !redacts);
    let mut out_headers = headers;
    match by_length {
        Some(length) => out_headers.push(("Content-Length".to_string(), length.to_string())),
        None => out_headers.push(("Transfer-Encoding".to_string(), "chunked".to_string())),
    }
    write_head(out, status, &out_headers)?;
    let chunked = by_length.is_none();
    emit(out, &ready, chunked)?;
    if first == 0 {
        let rest = scanner.finish().map_err(|_| leak())?;
        emit(out, &rest, chunked)?;
    } else {
        loop {
            let n = read_some(&mut response.body, &mut buf)?;
            if n == 0 {
                let rest = scanner.finish().map_err(|_| leak())?;
                emit(out, &rest, chunked)?;
                break;
            }
            ready = scanner.feed(&buf[..n]).map_err(|_| leak())?;
            emit(out, &ready, chunked)?;
        }
    }
    if chunked {
        finish_chunks(out)
    } else {
        out.flush()
    }
}

fn leak() -> io::Error {
    io::Error::other("the forge's answer held the credential: not sent")
}

fn read_some<R: Read + ?Sized>(from: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match from.read(buf) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

fn emit<W: Write>(out: &mut W, bytes: &[u8], chunked: bool) -> io::Result<()> {
    if chunked {
        write_chunk(out, bytes)
    } else {
        out.write_all(bytes)
    }
}

fn with_length(mut headers: Vec<(String, String)>, length: Option<&str>) -> Vec<(String, String)> {
    if let Some(length) = length {
        headers.push(("Content-Length".to_string(), length.to_string()));
    }
    headers
}

/// The forge's headers as the chat is sent them.
fn chat_headers(
    sent: &[(String, String)],
    base: Option<&Url>,
    side: &ChatSide<'_>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, value) in sent {
        if is_dropped_header(name) {
            continue;
        }
        match (normal(name).as_str(), base) {
            ("link", Some(base)) => {
                if let Some(link) = side.link(value, base) {
                    out.push((name.clone(), link));
                }
            }
            ("location" | "content-location", Some(base)) => {
                if let Some(address) = side.address(value, base) {
                    out.push((name.clone(), address));
                }
            }
            ("link" | "location" | "content-location", None) => {}
            _ => out.push((name.clone(), value.clone())),
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn redirect<W: Write>(
    w: &Wiring,
    out: &mut W,
    rh: &RequestHead,
    forwarded: &Forwarded,
    response: &UpstreamResponse,
    location: &str,
    base: &Url,
    side: &ChatSide<'_>,
    followed: bool,
) -> io::Result<()> {
    let refuse = |out: &mut W| {
        write_answer(
            out,
            502,
            &message("the forge redirected to a host purlis does not carry"),
        )
    };
    let Ok(target) = base.join(location.trim()) else {
        return refuse(out);
    };
    let host = target
        .host_str()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let storage = target.scheme() == "https"
        && target.username().is_empty()
        && target.password().is_none()
        && target.port().is_none_or(|p| p == 443)
        && is_download_host(&host);

    // `gh` on its socket cannot follow a redirect to a storage host: it is followed here, once,
    // anonymously, and what it answers is streamed. A second redirect is not followed.
    if storage && side.alias.is_some() && rh.method.eq_ignore_ascii_case("GET") && !followed {
        let kept = ["accept", "range", "if-none-match", "if-modified-since"];
        let request = UpstreamRequest {
            method: "GET".to_string(),
            url: target.to_string(),
            headers: forwarded
                .headers
                .iter()
                .filter(|(n, _)| kept.contains(&normal(n).as_str()))
                .cloned()
                .collect(),
            body: Vec::new(),
        };
        // The credential is gone with the headers: nothing of the request but what is kept.
        let anonymous = Forwarded::anonymous_follow(forwarded, target.as_str());
        return match w.upstream.send(request) {
            Ok(next) => answer(w, out, rh, &anonymous, next, side, true),
            Err(e) => write_answer(
                out,
                502,
                &message(&format!("purlis could not reach the forge: {e}")),
            ),
        };
    }
    // Inside TLS the chat follows a redirect to a storage host itself (a tunnel, never terminated).
    let address = if storage && side.alias.is_none() {
        Some(target.to_string())
    } else {
        side.address(location, base)
    };
    let Some(address) = address else {
        return refuse(out);
    };
    let mut headers = chat_headers(&response.headers, Some(base), side);
    headers.retain(|(n, _)| normal(n) != "location");
    headers.push(("Location".to_string(), address));
    headers.push(("Content-Length".to_string(), "0".to_string()));
    write_head(out, response.status, &headers)?;
    out.flush()
}

/// Fetch the object behind a token the listener gave the chat, with the headers the forge gave
/// with it, and none of the person's. A token that was never given, has expired or was given on
/// another host is refused.
fn serve_storage<W: Write>(
    w: &Wiring,
    out: &mut W,
    asked: &RequestHead,
    token: &str,
    headers: &[(String, String)],
) -> io::Result<()> {
    let Some((url, mut sent)) = w.storage.lookup(token, &asked.host) else {
        w.proxy.note_refused(&w.chat, asked, Refusal::NotOnList);
        return refused(out, Refusal::NotOnList);
    };
    w.proxy.note_allowed(&w.chat, asked);
    let kept = ["accept", "range", "if-none-match", "if-modified-since"];
    sent.extend(
        headers
            .iter()
            .filter(|(n, _)| kept.contains(&normal(n).as_str()))
            .cloned(),
    );
    let forwarded = Forwarded::for_storage(&asked.host, &url);
    let hosts = w.proxy.terminable_hosts(&w.chat);
    let side = ChatSide {
        hosts: &hosts,
        alias: None,
    };
    let request = UpstreamRequest {
        method: "GET".to_string(),
        url,
        headers: sent,
        body: Vec::new(),
    };
    match w.upstream.send(request) {
        Ok(response) => answer(w, out, asked, &forwarded, response, &side, true),
        Err(e) => write_answer(
            out,
            502,
            &message(&format!("purlis could not reach the storage: {e}")),
        ),
    }
}

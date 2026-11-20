// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What of the forge's answer may reach the chat (ADR 0128 D3a, D3c, D3g).
//!
//! The chat holds no credential, and the answer must not hand it one back:
//!
//! - **the real credential**, as the forge was sent it and in the forms a forge or a proxy gives
//!   it back ([`Forwarded::leak_needles`](super::super::Forwarded::leak_needles)), is looked for in
//!   every header and every byte of the body; one found ends the answer (it is never sent);
//! - **a credentialed clone URL** (`https://user:secret@host/…`) in a text answer is cut down to
//!   the URL without its userinfo;
//! - **`Location` and `Link`** are written again for the chat: an address on a host the chat may
//!   reach and with no userinfo and no credential in its query, in the form the chat's own client
//!   can follow (for `gh` on its unix socket, the `github.localhost` name over plain HTTP), and
//!   dropped otherwise;
//! - **`Set-Cookie`** and every hop-by-hop header are dropped.

use regex::bytes::Regex;
use url::Url;

use super::super::GithubAlias;

/// A needle was found: the answer is not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Leak;

/// Reads a body as it streams past, holding back the tail that a needle or a URL may still be
/// completed in, so that **nothing of one is ever sent**.
pub(crate) struct Scanner {
    needles: Vec<Vec<u8>>,
    /// Text answers have a credentialed URL cut down; binary ones (a pack, an archive) are only
    /// looked through for the needles.
    redact: bool,
    carry: Vec<u8>,
    /// Set while the rest of a userinfo that outgrew [`URL_CAP`] is being dropped.
    skipping: bool,
    hold: usize,
}

/// A URL's userinfo may be this long and still be cut down when it straddles two reads.
const URL_WINDOW: usize = 2048;
/// A URL that is still open after this many bytes is not read further: what follows its `://` up
/// to the byte that ends a userinfo is dropped, so a hold never grows with the answer.
const URL_CAP: usize = 4096;
/// The longest scheme held back when a read ends inside one.
const SCHEME_MAX: usize = 32;

fn userinfo() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        // `scheme://user:secret@`: a pair, so `git@host` and `ssh://git@host/…` are left alone.
        Regex::new(r#"(?-u)((?i:[a-z][a-z0-9+.\-]*))://[^/\s"'<>@?#:]+:[^/\s"'<>@?#]+@"#)
            .expect("a fixed pattern")
    })
}

fn token_only() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        // A lone userinfo over HTTP is the credential (a token, or a user name that is one); over
        // ssh it is the account's name, and is left alone.
        Regex::new(r#"(?-u)((?i:https?|wss?|ftps?))://[^/\s"'<>@?#:]+@"#).expect("a fixed pattern")
    })
}

fn redact_urls(ready: &[u8]) -> Vec<u8> {
    let pairs = userinfo().replace_all(ready, &b"$1://"[..]);
    token_only().replace_all(&pairs, &b"$1://"[..]).into_owned()
}

impl Scanner {
    pub(crate) fn new(needles: Vec<Vec<u8>>, redact: bool) -> Self {
        let longest = needles.iter().map(Vec::len).max().unwrap_or(0);
        Self {
            needles,
            redact,
            carry: Vec::new(),
            skipping: false,
            hold: if redact {
                URL_WINDOW.max(longest)
            } else {
                longest.saturating_sub(1)
            },
        }
    }

    fn leaks(&self, data: &[u8]) -> bool {
        self.needles
            .iter()
            .any(|needle| !needle.is_empty() && memchr::memmem::find(data, needle).is_some())
    }

    /// Whether `bytes` holds a needle.
    pub(crate) fn holds(&self, bytes: &[u8]) -> bool {
        self.leaks(bytes)
    }

    /// The next read of the body: what is safe to send now.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Result<Vec<u8>, Leak> {
        self.advance(chunk, false)
    }

    /// The end of the body: what was held back.
    pub(crate) fn finish(&mut self) -> Result<Vec<u8>, Leak> {
        self.advance(&[], true)
    }

    fn advance(&mut self, chunk: &[u8], last: bool) -> Result<Vec<u8>, Leak> {
        let mut data = std::mem::take(&mut self.carry);
        data.extend_from_slice(chunk);
        if self.leaks(&data) {
            return Err(Leak);
        }
        if !self.redact {
            let end = if last {
                data.len()
            } else {
                data.len().saturating_sub(self.hold)
            };
            self.carry = data.split_off(end);
            return Ok(data);
        }
        let mut out = Vec::new();
        loop {
            if self.skipping {
                let (n, done) = skip_userinfo(&data);
                data.drain(..n);
                self.skipping = !done;
            }
            let mut end = if last {
                data.len()
            } else {
                data.len().saturating_sub(self.hold)
            };
            if !last && let Some(open) = open_url(&data, end) {
                // Too long to wait for: keep its scheme, drop what follows up to the end of a
                // userinfo, and read on from there.
                if let Some(scheme) = open.scheme.filter(|_| data.len() - open.start > URL_CAP) {
                    out.extend(redact_urls(&data[..scheme + 3]));
                    data.drain(..scheme + 3);
                    self.skipping = true;
                    continue;
                }
                end = open.start;
            }
            out.extend(redact_urls(&data[..end]));
            self.carry = data.split_off(end);
            return Ok(out);
        }
    }
}

/// An open URL: where it starts, and where its `://` is when the read is already past it.
struct OpenUrl {
    start: usize,
    scheme: Option<usize>,
}

/// A byte that ends a userinfo.
fn ends_userinfo(b: u8) -> bool {
    b.is_ascii_whitespace() || b"/\"'<>?#".contains(&b)
}

/// How much of `data` is the rest of a userinfo being dropped, and whether it ended in it.
fn skip_userinfo(data: &[u8]) -> (usize, bool) {
    for (i, b) in data.iter().enumerate() {
        if *b == b'@' {
            return (i + 1, true);
        }
        if ends_userinfo(*b) {
            return (i, true);
        }
    }
    (data.len(), false)
}

/// The URL that is still open at `end`, if there is one: what is held back has to start there, so
/// that what follows can complete it. Open means the cut falls inside the scheme, between it and
/// its `://`, or after the `://` and before a byte that ends a userinfo.
fn open_url(data: &[u8], end: usize) -> Option<OpenUrl> {
    let head = &data[..end];
    if let Some(scheme) = memchr::memmem::rfind(head, b"://")
        && !data[scheme + 3..end].iter().copied().any(ends_userinfo)
    {
        return Some(OpenUrl {
            start: scheme_start(data, scheme),
            scheme: Some(scheme),
        });
    }
    let colon = if head.ends_with(b":/") {
        2
    } else {
        usize::from(head.ends_with(b":"))
    };
    let start = scheme_start(data, end - colon);
    (start < end).then_some(OpenUrl {
        start,
        scheme: None,
    })
}

/// Where the scheme that ends at `end` starts (at most [`SCHEME_MAX`] bytes back).
fn scheme_start(data: &[u8], end: usize) -> usize {
    let mut start = end;
    while start > 0
        && end - start < SCHEME_MAX
        && (data[start - 1].is_ascii_alphanumeric() || b"+.-".contains(&data[start - 1]))
    {
        start -= 1;
    }
    start
}

/// How the chat reaches hosts: which it may reach, and, on `gh`'s plain socket, the name it knows
/// the one GitHub host by.
pub(crate) struct ChatSide<'a> {
    /// The hosts the chat's per-chat CA vouches for: its recorded hosts and their API hosts.
    pub(crate) hosts: &'a [String],
    /// Set on the plain unix route.
    pub(crate) alias: Option<&'a GithubAlias>,
}

impl ChatSide<'_> {
    /// An address from an answer, as the chat should see it, or `None` when it must not reach the
    /// chat. `base` is the address the answer came from, which a relative one is read against.
    pub(crate) fn address(&self, raw: &str, base: &Url) -> Option<String> {
        let url = base.join(raw.trim()).ok()?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        let host = url.host_str()?.to_ascii_lowercase();
        if !self.hosts.contains(&host) {
            return None;
        }
        if url.query_pairs().any(|(k, _)| is_credential_key(&k)) {
            return None;
        }
        let tail = |path: &str| match url.query() {
            Some(query) => format!("{path}?{query}"),
            None => path.to_string(),
        };
        match self.alias {
            None => {
                let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
                Some(format!("https://{host}{port}{}", tail(url.path())))
            }
            Some(alias) => {
                if host != alias.host || url.port().is_some_and(|p| p != alias.port) {
                    return None;
                }
                let path = if alias.enterprise {
                    if url.path() == "/api/graphql" {
                        "/graphql".to_string()
                    } else {
                        url.path().strip_prefix("/api/v3")?.to_string()
                    }
                } else {
                    url.path().to_string()
                };
                let path = if path.is_empty() {
                    "/".to_string()
                } else {
                    path
                };
                Some(format!("http://api.github.localhost{}", tail(&path)))
            }
        }
    }

    /// A `Link` header with each address written for the chat; one that may not reach it is
    /// dropped, and `None` is no address left.
    pub(crate) fn link(&self, value: &str, base: &Url) -> Option<String> {
        let mut kept = Vec::new();
        for entry in split_links(value) {
            let Some((target, params)) = entry
                .trim()
                .strip_prefix('<')
                .and_then(|rest| rest.split_once('>'))
            else {
                continue;
            };
            if let Some(address) = self.address(target, base) {
                kept.push(format!("<{address}>{params}"));
            }
        }
        (!kept.is_empty()).then(|| kept.join(", "))
    }
}

/// The comma-separated entries of a `Link`, a comma inside `<…>` being part of the address.
fn split_links(value: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let (mut start, mut inside) = (0, false);
    for (i, c) in value.char_indices() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            ',' if !inside => {
                entries.push(&value[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    entries.push(&value[start..]);
    entries
}

fn is_credential_key(key: &str) -> bool {
    const NAMES: &[&str] = &[
        "access_token",
        "private_token",
        "job_token",
        "deploy_token",
        "oauth_token",
        "auth_token",
        "token",
        "password",
        "client_secret",
    ];
    let key = key.to_ascii_lowercase().replace('-', "_");
    NAMES.contains(&key.as_str())
}

/// Headers an answer is not sent to the chat with: those that belong to the upstream connection,
/// that frame the body (the listener frames it itself) or that set a cookie.
pub(crate) fn is_dropped_header(name: &str) -> bool {
    matches!(
        super::super::normal(name).as_str(),
        "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "set-cookie"
            | "set-cookie2"
            | "alt-svc"
            | "content-length"
    )
}

/// Whether an answer of this `Content-Type` is text a credentialed URL may be in.
pub(crate) fn is_text(content_type: Option<&str>) -> bool {
    let Some(ct) = content_type else {
        return false;
    };
    let ct = ct.to_ascii_lowercase();
    ct.starts_with("text/")
        || ct.contains("json")
        || ct.contains("xml")
        || ct.contains("javascript")
        || ct.contains("yaml")
        || ct.contains("x-www-form-urlencoded")
}

// ---- git's own words -------------------------------------------------------------------------

/// The pkt-lines of `data`: `Some(payload)` for a line and `None` for a flush, or `None` when it
/// is not pkt-lines to the end.
fn pkt_lines(mut data: &[u8]) -> Option<Vec<Option<&[u8]>>> {
    let mut lines = Vec::new();
    while !data.is_empty() {
        let length = std::str::from_utf8(data.get(..4)?).ok()?;
        let length = usize::from_str_radix(length, 16).ok()?;
        if length == 0 {
            lines.push(None);
            data = &data[4..];
            continue;
        }
        if length < 4 || data.len() < length {
            return None;
        }
        lines.push(Some(&data[4..length]));
        data = &data[length..];
    }
    Some(lines)
}

/// Whether a push's report says **every** ref was updated: `unpack ok`, an `ok` for each ref and
/// no `ng`. Anything the forge's report does not say plainly is not an acceptance.
pub(crate) fn push_fully_accepted(body: &[u8]) -> bool {
    let Some(outer) = pkt_lines(body) else {
        return false;
    };
    // With side-band the report is the data of channel 1, itself pkt-lines.
    let banded: Vec<u8> = outer
        .iter()
        .flatten()
        .filter(|line| line.first() == Some(&1))
        .flat_map(|line| line[1..].iter().copied())
        .collect();
    let inner_owned;
    let inner: Vec<&[u8]> = if banded.is_empty() {
        outer.iter().flatten().copied().collect()
    } else {
        inner_owned = pkt_lines(&banded);
        match &inner_owned {
            Some(lines) => lines.iter().flatten().copied().collect(),
            None => return false,
        }
    };
    let (mut unpack, mut ok, mut ng) = (false, 0, 0);
    for line in inner {
        let line = String::from_utf8_lossy(line);
        let line = line.trim_end();
        if line == "unpack ok" {
            unpack = true;
        } else if line.starts_with("ok ") {
            ok += 1;
        } else if line.starts_with("ng ") || line.starts_with("unpack ") {
            ng += 1;
        }
    }
    unpack && ok > 0 && ng == 0
}

/// The branches (short names) a push's ref advertisement lists.
pub(crate) fn advertised_branches(body: &[u8]) -> Vec<String> {
    let Some(lines) = pkt_lines(body) else {
        return Vec::new();
    };
    lines
        .into_iter()
        .flatten()
        .filter_map(|line| {
            let line = std::str::from_utf8(line).ok()?;
            let line = line.split('\0').next()?.trim_end();
            let (_, name) = line.split_once(' ')?;
            name.strip_prefix("refs/heads/").map(str::to_string)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A credentialed URL, built here so no source line is one.
    fn credentialed(scheme: &str, user: &str, pw: &str) -> String {
        [scheme, "://", user, ":", pw, "@host.example/x"].concat()
    }

    fn run(scanner: &mut Scanner, parts: &[&[u8]]) -> String {
        let mut out = Vec::new();
        for part in parts {
            out.extend(scanner.feed(part).expect("no needle"));
        }
        out.extend(scanner.finish().expect("no needle"));
        String::from_utf8(out).expect("text")
    }

    #[test]
    fn a_credentialed_url_is_cut_down_at_every_offset_the_hold_boundary_can_fall() {
        let url = credentialed("https", "someone", "hunter2hunter2");
        for k in 0..=url.len() {
            // The first read's cut (its length less the hold) falls `k` bytes into the URL.
            let mut first = b"pad ".to_vec();
            first.extend_from_slice(url.as_bytes());
            first.resize(4 + k + URL_WINDOW, b'z');
            let mut scanner = Scanner::new(Vec::new(), true);
            let text = run(&mut scanner, &[&first, b" more text"]);
            assert!(!text.contains("someone"), "offset {k}: {text:.80}");
            assert!(!text.contains("hunter2"), "offset {k}: {text:.80}");
            assert!(text.contains("https://host.example/x"), "offset {k}");
        }
    }

    #[test]
    fn a_url_split_across_two_reads_at_every_byte_is_cut_down() {
        let url = credentialed("https", "someone", "hunter2hunter2");
        let body = format!("see {url} and {url}");
        for k in 0..=body.len() {
            let (a, b) = body.as_bytes().split_at(k);
            let mut scanner = Scanner::new(Vec::new(), true);
            let text = run(&mut scanner, &[a, b]);
            assert!(!text.contains("hunter2"), "split {k}: {text}");
            assert!(!text.contains("someone"), "split {k}: {text}");
        }
    }

    #[test]
    fn a_token_only_userinfo_over_http_is_cut_and_an_ssh_user_is_not() {
        let token = ["https://", "tok123", "@host.example/r"].concat();
        let ssh = ["ssh://", "git", "@host.example/r"].concat();
        let kept = ["ssh://git", "@host.example/r"].concat();
        let mut scanner = Scanner::new(Vec::new(), true);
        let text = run(&mut scanner, &[format!("{token} {ssh}").as_bytes()]);
        assert!(!text.contains("tok123"), "{text}");
        assert!(text.contains(&kept), "{text}");
    }

    #[test]
    fn a_userinfo_longer_than_the_cap_is_dropped_and_the_hold_stays_bounded() {
        let long = "q".repeat(URL_CAP * 3);
        let body = ["https://", &long, ":pw", "@host.example/x tail"].concat();
        let mut scanner = Scanner::new(Vec::new(), true);
        let mut out = Vec::new();
        for piece in body.as_bytes().chunks(1000) {
            out.extend(scanner.feed(piece).expect("no needle"));
            assert!(scanner.carry.len() <= URL_CAP + 1000);
        }
        out.extend(scanner.finish().expect("no needle"));
        let text = String::from_utf8(out).expect("text");
        assert!(!text.contains("qqqq"), "{}", text.len());
        assert!(text.ends_with("host.example/x tail"), "{text}");
    }
}

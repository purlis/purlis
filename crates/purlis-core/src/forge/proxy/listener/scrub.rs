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
    hold: usize,
}

/// A URL's userinfo may be this long and still be cut down when it straddles two reads.
const URL_WINDOW: usize = 2048;

fn userinfo() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        // `scheme://user:secret@`: a pair, so `git@host` and `ssh://git@host/…` are left alone.
        Regex::new(r#"(?-u)((?i:[a-z][a-z0-9+.\-]*))://[^/\s"'<>@?#:]+:[^/\s"'<>@?#]+@"#)
            .expect("a fixed pattern")
    })
}

impl Scanner {
    pub(crate) fn new(needles: Vec<Vec<u8>>, redact: bool) -> Self {
        let longest = needles.iter().map(Vec::len).max().unwrap_or(0);
        Self {
            needles,
            redact,
            carry: Vec::new(),
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
        let mut end = if last {
            data.len()
        } else {
            data.len().saturating_sub(self.hold)
        };
        if self.redact && !last {
            end = open_url_start(&data, end);
        }
        let ready = &data[..end];
        let out = if self.redact {
            userinfo().replace_all(ready, &b"$1://"[..]).into_owned()
        } else {
            ready.to_vec()
        };
        self.carry = data[end..].to_vec();
        Ok(out)
    }
}

/// The index `end` moved back to the start of a URL that is still open there: one that started
/// before `end` and has not yet met a byte that ends a userinfo, so what follows may complete it.
fn open_url_start(data: &[u8], end: usize) -> usize {
    let Some(scheme) = memchr::memmem::rfind(&data[..end], b"://") else {
        return end;
    };
    let ended = data[scheme + 3..end]
        .iter()
        .any(|b| b.is_ascii_whitespace() || b"/\"'<>?#".contains(b));
    if ended {
        return end;
    }
    let mut start = scheme;
    while start > 0
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

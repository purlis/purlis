// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The fake forge** (T1.1-50, #6897): a local origin that answers the part of a GitHub and a
//! GitLab API that a merge needs, and records every request it gets, the credential presented
//! included.
//!
//! ## The API, all of it
//!
//! One server speaks both forges: a path under `/api/v4/` is GitLab's, any other is GitHub's,
//! under `/api/v3/` (a GitHub Enterprise Server's root, which purlis's forge client builds) or
//! at the root (what `gh` asks for on `github.localhost`). Every answer is JSON and ends the
//! connection. A request with no credential is `401`, never recorded as a success.
//!
//! | Request | Answer |
//! |---|---|
//! | GitHub `GET /user` | `200`, the account a token names |
//! | GitHub `GET /repos/{owner}/{repo}/pulls/{n}` | `200`, its head SHA, state and `merged` |
//! | GitHub `PUT /repos/{owner}/{repo}/pulls/{n}/merge` | merged `200`; head moved `409`; a [`Refusal`] `405`; merged already `405` |
//! | GitLab `GET /user` | `200` |
//! | GitLab `GET /projects/{id}/merge_requests/{iid}` | `200`, its `sha`, `state` and `merge_status` |
//! | GitLab `PUT /projects/{id}/merge_requests/{iid}/merge` | merged `200`; `sha` moved `409`; a [`Refusal`] `405`; merged already `405` |
//! | anything else | `404`, naming the request, so a cell that needs more fails on it |
//!
//! `{id}` is a GitLab project's path (`group%2Fproject`, as `glab` sends it) or the number
//! [`FakeForge::add_merge_request`] gave. A pull request or merge request exists only once a
//! test adds it ([`FakeForge::add_pull_request`]); asking after another is `404`.
//!
//! ## What a cell asserts on
//!
//! - [`FakeForge::requests`]: every request, in order, with its headers, body, the TLS name
//!   asked for and the answer's status.
//! - [`Request::credential`]: the token presented, and the header it came in.
//! - [`FakeForge::tokens_seen`]: every distinct token, so a cell can say the one it handed was
//!   used and the person's own never was.
//! - [`FakeForge::merged`]: the SHA a merge produced, or `None` while nothing merged.
//!
//! It listens twice on this machine's loopback, over one state: plain HTTP
//! ([`FakeForge::http_addr`]) and, when a [`TestCa`] is given, HTTPS ([`FakeForge::https_addr`]).
//! The resolver seam maps the names a cell's client uses to those addresses.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rustls::ServerConnection;
use serde_json::{Value, json};

use super::TestCa;
use super::origin::{PATIENCE, parse, read_head, tls_for};

/// The most a request's body may take.
const BODY_AT_MOST: usize = 1024 * 1024;

/// The header every answer carries: purlis's own refusals never do.
pub const FORGE_HEADER: &str = "x-purlis-fake-forge";

/// The SHA a successful merge answers with.
pub const MERGE_SHA: &str = "feedfacefeedfacefeedfacefeedfacefeedface";

/// **Which forge's API a request or an item belongs to.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Forge {
    GitHub,
    GitLab,
}

/// **Why a merge is refused when a test asks for it** ([`FakeForge::refuse_merge`]): the two
/// refusals a merge cell needs. Both are `405`, as each forge answers them, with its own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A ruleset (GitHub) or a protected-branch rule (GitLab) the change does not satisfy.
    Ruleset,
    /// The branch is merged through a queue (GitHub's merge queue, GitLab's merge train).
    Queue,
}

/// **The credential a request presented**, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    /// The header it came in, lower case: `authorization`, `private-token`, `job-token`.
    pub header: String,
    /// The token itself. For `Basic`, the password half.
    pub token: String,
}

/// **One request the fake forge got.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// The path, without its query.
    pub path: String,
    pub query: String,
    pub host: Option<String>,
    /// The TLS server name the client sent, for an HTTPS request.
    pub sni: Option<String>,
    /// Every header, in order, names lower case.
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// The status it was answered with.
    pub status: u16,
}

impl Request {
    /// The credential it presented, if any.
    pub fn credential(&self) -> Option<Credential> {
        credential_of(&self.headers)
    }
}

fn credential_of(headers: &[(String, String)]) -> Option<Credential> {
    let get = |name: &str| {
        headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    };
    for name in ["private-token", "job-token"] {
        if let Some(value) = get(name).filter(|value| !value.is_empty()) {
            return Some(Credential {
                header: name.to_owned(),
                token: value.to_owned(),
            });
        }
    }
    let value = get("authorization")?;
    let (scheme, rest) = value.split_once(' ')?;
    let token = if scheme.eq_ignore_ascii_case("basic") {
        let decoded = decode_base64(rest.trim())?;
        decoded
            .split_once(':')
            .map(|(_, password)| password.to_owned())?
    } else {
        rest.trim().to_owned()
    };
    (!token.is_empty()).then(|| Credential {
        header: "authorization".to_owned(),
        token,
    })
}

/// Base64, standard alphabet, no dependency: for a `Basic` credential's `user:password`.
fn decode_base64(text: &str) -> Option<String> {
    let mut bits = 0u32;
    let mut have = 0;
    let mut out = Vec::new();
    for byte in text.bytes().take_while(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        have += 6;
        if have >= 8 {
            have -= 8;
            out.push(u8::try_from((bits >> have) & 0xff).ok()?);
        }
    }
    String::from_utf8(out).ok()
}

/// One pull request or merge request.
#[derive(Debug, Clone)]
struct Change {
    number: u64,
    head: String,
    merged: Option<String>,
    refusal: Option<Refusal>,
}

#[derive(Default)]
struct State {
    requests: Vec<Request>,
    /// By forge and the lower-case `owner/repo` or project path.
    changes: HashMap<(Forge, String), Vec<Change>>,
    /// A GitLab project's number, to its path.
    project_ids: HashMap<u64, String>,
}

/// What a request is answered with.
#[derive(Debug)]
struct Reply {
    status: u16,
    body: Value,
}

fn reply(status: u16, body: Value) -> Reply {
    Reply { status, body }
}

impl State {
    fn change(&mut self, forge: Forge, project: &str, number: u64) -> Option<&mut Change> {
        self.changes
            .get_mut(&(forge, project.to_ascii_lowercase()))?
            .iter_mut()
            .find(|change| change.number == number)
    }

    fn answer(&mut self, request: &Request) -> Reply {
        let gitlab = request.path.starts_with("/api/v4/");
        let path = request
            .path
            .strip_prefix("/api/v4")
            .or_else(|| request.path.strip_prefix("/api/v3"))
            .unwrap_or(&request.path);
        let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        let unknown = || {
            reply(
                404,
                json!({"message": format!(
                    "the fake forge has no answer for {} {}",
                    request.method, request.path
                )}),
            )
        };
        let Some(credential) = request.credential() else {
            return if gitlab {
                reply(401, json!({"message": "401 Unauthorized"}))
            } else {
                reply(
                    401,
                    json!({"message": "Requires authentication", "status": "401"}),
                )
            };
        };
        match (gitlab, request.method.as_str(), segments.as_slice()) {
            (false, "GET", ["user"]) => reply(
                200,
                json!({"login": "fake-forge-user", "id": 1, "token_prefix": prefix(&credential.token)}),
            ),
            (true, "GET", ["user"]) => reply(
                200,
                json!({"username": "fake-forge-user", "id": 1, "state": "active"}),
            ),
            (false, "GET", ["repos", owner, repo, "pulls", number]) => {
                let project = format!("{owner}/{repo}");
                let Some(change) = number
                    .parse()
                    .ok()
                    .and_then(|n| self.change(Forge::GitHub, &project, n))
                else {
                    return reply(404, json!({"message": "Not Found", "status": "404"}));
                };
                reply(200, github_pull(&project, change))
            }
            (false, "PUT", ["repos", owner, repo, "pulls", number, "merge"]) => {
                let project = format!("{owner}/{repo}");
                let sha = body_field(&request.body, "sha");
                let Some(change) = number
                    .parse()
                    .ok()
                    .and_then(|n| self.change(Forge::GitHub, &project, n))
                else {
                    return reply(404, json!({"message": "Not Found", "status": "404"}));
                };
                merge_github(change, sha.as_deref())
            }
            (true, "GET", ["projects", id, "merge_requests", iid]) => {
                let Some(project) = self.project_of(id) else {
                    return reply(404, json!({"message": "404 Project Not Found"}));
                };
                let Some(change) = iid
                    .parse()
                    .ok()
                    .and_then(|n| self.change(Forge::GitLab, &project, n))
                else {
                    return reply(404, json!({"message": "404 Not found"}));
                };
                reply(200, gitlab_request(&project, change))
            }
            (true, "PUT", ["projects", id, "merge_requests", iid, "merge"]) => {
                let Some(project) = self.project_of(id) else {
                    return reply(404, json!({"message": "404 Project Not Found"}));
                };
                let sha =
                    body_field(&request.body, "sha").or_else(|| query_field(&request.query, "sha"));
                let Some(change) = iid
                    .parse()
                    .ok()
                    .and_then(|n| self.change(Forge::GitLab, &project, n))
                else {
                    return reply(404, json!({"message": "404 Not found"}));
                };
                merge_gitlab(&project, change, sha.as_deref())
            }
            _ => unknown(),
        }
    }

    /// A GitLab project's path from `{id}`: percent-encoded path or a number.
    fn project_of(&self, id: &str) -> Option<String> {
        if let Ok(number) = id.parse::<u64>() {
            return self.project_ids.get(&number).cloned();
        }
        let path = percent_decoded(id).to_ascii_lowercase();
        self.changes
            .contains_key(&(Forge::GitLab, path.clone()))
            .then_some(path)
    }
}

/// The first characters of a token, never all of it, for an answer a person may read.
fn prefix(token: &str) -> String {
    token.chars().take(4).collect()
}

fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let decoded = (bytes[at] == b'%')
            .then(|| text.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(byte) = decoded {
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A field of a JSON or form-encoded body.
fn body_field(body: &str, name: &str) -> Option<String> {
    if let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(body) {
        return fields.get(name).and_then(Value::as_str).map(str::to_owned);
    }
    query_field(body, name)
}

fn query_field(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| percent_decoded(value))
    })
}

fn github_pull(project: &str, change: &Change) -> Value {
    json!({
        "number": change.number,
        "state": if change.merged.is_some() { "closed" } else { "open" },
        "merged": change.merged.is_some(),
        "mergeable": change.merged.is_none(),
        "html_url": format!("https://forge.test/{project}/pull/{}", change.number),
        "head": {"sha": change.head, "ref": "feature"},
        "base": {"ref": "main"},
    })
}

fn gitlab_request(project: &str, change: &Change) -> Value {
    json!({
        "iid": change.number,
        "state": if change.merged.is_some() { "merged" } else { "opened" },
        "merge_status": "can_be_merged",
        "sha": change.head,
        "web_url": format!("https://gitlab.test/{project}/-/merge_requests/{}", change.number),
        "source_branch": "feature",
        "target_branch": "main",
    })
}

fn merge_github(change: &mut Change, sha: Option<&str>) -> Reply {
    if change.merged.is_some() {
        return reply(
            405,
            json!({"message": "Pull Request is not mergeable", "status": "405"}),
        );
    }
    match change.refusal {
        Some(Refusal::Ruleset) => {
            return reply(
                405,
                json!({
                    "message": "Repository rule violations found\n\nRequired status check \"ci\" is expected.",
                    "status": "405",
                }),
            );
        }
        Some(Refusal::Queue) => {
            return reply(
                405,
                json!({"message": "Changes must be made through the merge queue", "status": "405"}),
            );
        }
        None => {}
    }
    if sha.is_some_and(|asked| asked != change.head) {
        return reply(
            409,
            json!({"message": "Head branch was modified. Review and try the merge again.", "status": "409"}),
        );
    }
    change.merged = Some(MERGE_SHA.to_owned());
    reply(
        200,
        json!({"sha": MERGE_SHA, "merged": true, "message": "Pull Request successfully merged"}),
    )
}

fn merge_gitlab(project: &str, change: &mut Change, sha: Option<&str>) -> Reply {
    if change.merged.is_some() {
        return reply(405, json!({"message": "405 Method Not Allowed"}));
    }
    match change.refusal {
        Some(Refusal::Ruleset) => {
            return reply(
                405,
                json!({"message": "405 Method Not Allowed: the branch is protected, and this change does not satisfy its rules"}),
            );
        }
        Some(Refusal::Queue) => {
            return reply(
                405,
                json!({"message": "405 Method Not Allowed: this branch is merged through a merge train"}),
            );
        }
        None => {}
    }
    if let Some(asked) = sha.filter(|asked| *asked != change.head) {
        return reply(
            409,
            json!({"message": format!("SHA does not match HEAD of source branch: {}", change.head), "asked": asked}),
        );
    }
    change.merged = Some(MERGE_SHA.to_owned());
    let mut shown = gitlab_request(project, change);
    shown["merge_commit_sha"] = json!(MERGE_SHA);
    reply(200, shown)
}

/// **A fake forge, serving until dropped** ([module docs](self)).
pub struct FakeForge {
    http: SocketAddr,
    https: Option<SocketAddr>,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for FakeForge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeForge")
            .field("http", &self.http)
            .field("https", &self.https)
            .finish_non_exhaustive()
    }
}

impl FakeForge {
    /// **Starts it** on the loopback: plain HTTP always, and HTTPS under a certificate from `ca`
    /// for `names` (each a `.test` name) where a CA is given.
    pub fn start(https: Option<(&TestCa, &[&str])>) -> io::Result<Self> {
        let state: Arc<Mutex<State>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let loopback = IpAddr::from([127, 0, 0, 1]);
        let http = listen(loopback, None, &state, &stop)?;
        let https = match https {
            Some((ca, names)) => {
                let config = tls_for(&super::Scheme::Https { ca, names })?;
                Some(listen(loopback, config, &state, &stop)?)
            }
            None => None,
        };
        Ok(Self {
            http,
            https,
            state,
            stop,
        })
    }

    /// Where it speaks plain HTTP.
    pub fn http_addr(&self) -> SocketAddr {
        self.http
    }

    /// Where it speaks HTTPS, when it was started with a CA.
    pub fn https_addr(&self) -> Option<SocketAddr> {
        self.https
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **A GitHub pull request** `number` of `owner/repo`, its head at `head`.
    pub fn add_pull_request(&self, repo: &str, number: u64, head: &str) -> &Self {
        self.add(Forge::GitHub, repo, number, head);
        self
    }

    /// **A GitLab merge request** `iid` of the project at `path`, which `project_id` also names.
    pub fn add_merge_request(&self, path: &str, project_id: u64, iid: u64, head: &str) -> &Self {
        self.state()
            .project_ids
            .insert(project_id, path.to_ascii_lowercase());
        self.add(Forge::GitLab, path, iid, head);
        self
    }

    fn add(&self, forge: Forge, project: &str, number: u64, head: &str) {
        self.state()
            .changes
            .entry((forge, project.to_ascii_lowercase()))
            .or_default()
            .push(Change {
                number,
                head: head.to_owned(),
                merged: None,
                refusal: None,
            });
    }

    /// **Refuses that change's merge** from now on, as `why`.
    ///
    /// # Panics
    /// If no such change was added: a test that refuses nothing would pass for nothing.
    pub fn refuse_merge(&self, forge: Forge, project: &str, number: u64, why: Refusal) -> &Self {
        let mut state = self.state();
        let change = state
            .change(forge, project, number)
            .unwrap_or_else(|| panic!("no {forge:?} change {project}#{number} to refuse"));
        change.refusal = Some(why);
        drop(state);
        self
    }

    /// **Moves that change's head** to `head`, as a push to its branch would.
    ///
    /// # Panics
    /// If no such change was added.
    pub fn move_head(&self, forge: Forge, project: &str, number: u64, head: &str) -> &Self {
        let mut state = self.state();
        let change = state
            .change(forge, project, number)
            .unwrap_or_else(|| panic!("no {forge:?} change {project}#{number} to move"));
        change.head = head.to_owned();
        drop(state);
        self
    }

    /// The SHA that change was merged as, or `None` while it is not merged.
    pub fn merged(&self, forge: Forge, project: &str, number: u64) -> Option<String> {
        self.state()
            .change(forge, project, number)
            .and_then(|change| change.merged.clone())
    }

    /// Every request it got so far, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.state().requests.clone()
    }

    /// **Every distinct token presented**, in the order first seen.
    pub fn tokens_seen(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for request in self.state().requests.iter() {
            if let Some(credential) = request.credential()
                && !seen.contains(&credential.token)
            {
                seen.push(credential.token);
            }
        }
        seen
    }
}

impl Drop for FakeForge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes each accept, which then sees the stop.
        for addr in std::iter::once(self.http).chain(self.https) {
            let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
        }
    }
}

fn listen(
    at: IpAddr,
    tls: Option<Arc<rustls::ServerConfig>>,
    state: &Arc<Mutex<State>>,
    stop: &Arc<AtomicBool>,
) -> io::Result<SocketAddr> {
    let listener = TcpListener::bind((at, 0))?;
    let addr = listener.local_addr()?;
    let (state, stop) = (Arc::clone(state), Arc::clone(stop));
    std::thread::Builder::new()
        .name("scenario-forge".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(stream) = stream else { continue };
                let (tls, state) = (tls.clone(), Arc::clone(&state));
                let _ = std::thread::Builder::new()
                    .name("scenario-forge-conn".into())
                    .spawn(move || {
                        let _ = serve(stream, tls, &state);
                    });
            }
        })?;
    Ok(addr)
}

fn serve(
    stream: TcpStream,
    tls: Option<Arc<rustls::ServerConfig>>,
    state: &Mutex<State>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(PATIENCE))?;
    stream.set_write_timeout(Some(PATIENCE))?;
    match tls {
        None => answer(stream, None, state),
        Some(config) => {
            let mut conn = ServerConnection::new(config).map_err(io::Error::other)?;
            let mut stream = stream;
            while conn.is_handshaking() {
                conn.complete_io(&mut stream)?;
            }
            let sni = conn.server_name().map(str::to_owned);
            answer(rustls::StreamOwned::new(conn, stream), sni, state)
        }
    }
}

fn answer(
    mut stream: impl Read + Write,
    sni: Option<String>,
    state: &Mutex<State>,
) -> io::Result<()> {
    let head = read_head(&mut stream)?;
    let Some(seen) = parse(&head, sni) else {
        return stream.write_all(b"HTTP/1.1 400 Bad Request\r\nconnection: close\r\n\r\n");
    };
    let wanted = seen
        .headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(BODY_AT_MOST);
    let mut body = vec![0u8; wanted];
    stream.read_exact(&mut body)?;
    let (path, query) = seen
        .path
        .split_once('?')
        .map_or((seen.path.as_str(), ""), |(path, query)| (path, query));
    let mut request = Request {
        method: seen.method.clone(),
        path: path.to_owned(),
        query: query.to_owned(),
        host: seen.host.clone(),
        sni: seen.sni.clone(),
        headers: seen.headers.clone(),
        body: String::from_utf8_lossy(&body).into_owned(),
        status: 0,
    };
    let answered = {
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        let answered = state.answer(&request);
        request.status = answered.status;
        state.requests.push(request);
        answered
    };
    let text = answered.body.to_string();
    let reply = format!(
        "HTTP/1.1 {} {}\r\n{FORGE_HEADER}: 1\r\ncontent-type: application/json\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{text}",
        answered.status,
        reason(answered.status),
        text.len()
    );
    stream.write_all(reply.as_bytes())?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        _ => "Status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Request {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        Request {
            method: method.to_owned(),
            path: path.to_owned(),
            query: query.to_owned(),
            host: None,
            sni: None,
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            body: body.to_owned(),
            status: 0,
        }
    }

    fn with_change(forge: Forge, project: &str, id: u64, head: &str) -> State {
        let mut state = State::default();
        state.project_ids.insert(id, project.to_owned());
        state.changes.insert(
            (forge, project.to_owned()),
            vec![Change {
                number: 7,
                head: head.to_owned(),
                merged: None,
                refusal: None,
            }],
        );
        state
    }

    const BEARER: &[(&str, &str)] = &[("authorization", "Bearer minted-token")];

    #[test]
    fn a_request_with_no_credential_is_refused_401_on_both_forges() {
        let mut state = with_change(Forge::GitHub, "o/r", 1, "abc");
        assert_eq!(
            state.answer(&asked("GET", "/api/v3/user", &[], "")).status,
            401
        );
        assert_eq!(
            state.answer(&asked("GET", "/api/v4/user", &[], "")).status,
            401
        );
    }

    #[test]
    fn the_credential_is_read_from_each_header_a_client_sends_it_in() {
        for (headers, header, token) in [
            (vec![("authorization", "Bearer a")], "authorization", "a"),
            (vec![("authorization", "token b")], "authorization", "b"),
            (vec![("private-token", "c")], "private-token", "c"),
            (vec![("job-token", "d")], "job-token", "d"),
            // x-access-token:e
            (
                vec![("authorization", "Basic eC1hY2Nlc3MtdG9rZW46ZQ==")],
                "authorization",
                "e",
            ),
        ] {
            let got = credential_of(
                &headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect::<Vec<_>>(),
            )
            .expect("a credential");
            assert_eq!((got.header.as_str(), got.token.as_str()), (header, token));
        }
        assert_eq!(credential_of(&[]), None);
    }

    #[test]
    fn a_github_merge_with_the_head_it_was_shown_merges_once() {
        let mut state = with_change(Forge::GitHub, "o/r", 1, "abc");
        let merge = |state: &mut State| {
            state.answer(&asked(
                "PUT",
                "/api/v3/repos/O/r/pulls/7/merge",
                BEARER,
                r#"{"sha":"abc"}"#,
            ))
        };
        let first = merge(&mut state);
        assert_eq!(
            (first.status, first.body["merged"].clone()),
            (200, json!(true))
        );
        assert_eq!(
            merge(&mut state).status,
            405,
            "a merged change is not merged again"
        );
    }

    #[test]
    fn a_github_merge_names_a_moved_head_409_and_leaves_it_unmerged() {
        let mut state = with_change(Forge::GitHub, "o/r", 1, "new");
        let got = state.answer(&asked(
            "PUT",
            "/repos/o/r/pulls/7/merge",
            BEARER,
            r#"{"sha":"old"}"#,
        ));
        assert_eq!(got.status, 409);
        assert!(
            state
                .change(Forge::GitHub, "o/r", 7)
                .expect("it")
                .merged
                .is_none()
        );
    }

    #[test]
    fn the_fake_forge_returns_a_ruleset_refusal_on_request() {
        for forge in [Forge::GitHub, Forge::GitLab] {
            let mut state = with_change(forge, "o/r", 5, "abc");
            state.change(forge, "o/r", 7).expect("it").refusal = Some(Refusal::Ruleset);
            let path = match forge {
                Forge::GitHub => "/api/v3/repos/o/r/pulls/7/merge",
                Forge::GitLab => "/api/v4/projects/o%2Fr/merge_requests/7/merge",
            };
            let got = state.answer(&asked("PUT", path, BEARER, r#"{"sha":"abc"}"#));
            assert_eq!(got.status, 405, "{forge:?}");
            assert!(
                got.body["message"].to_string().contains("rule"),
                "{forge:?}: {:?}",
                got.body
            );
            assert!(
                state.change(forge, "o/r", 7).expect("it").merged.is_none(),
                "{forge:?}"
            );
        }
    }

    #[test]
    fn a_queue_refusal_says_the_queue() {
        let mut state = with_change(Forge::GitHub, "o/r", 1, "abc");
        state.change(Forge::GitHub, "o/r", 7).expect("it").refusal = Some(Refusal::Queue);
        let got = state.answer(&asked("PUT", "/repos/o/r/pulls/7/merge", BEARER, "{}"));
        assert_eq!(got.status, 405);
        assert!(got.body["message"].to_string().contains("merge queue"));
    }

    #[test]
    fn a_gitlab_merge_is_found_by_path_or_number_and_checks_its_sha() {
        let mut state = with_change(Forge::GitLab, "g/p", 42, "abc");
        for id in ["g%2Fp", "42"] {
            let got = state.answer(&asked(
                "GET",
                &format!("/api/v4/projects/{id}/merge_requests/7"),
                &[("private-token", "t")],
                "",
            ));
            assert_eq!(
                (got.status, got.body["sha"].clone()),
                (200, json!("abc")),
                "{id}"
            );
        }
        let moved = state.answer(&asked(
            "PUT",
            "/api/v4/projects/g%2Fp/merge_requests/7/merge",
            &[("private-token", "t")],
            r#"{"sha":"old"}"#,
        ));
        assert_eq!(moved.status, 409);
        let merged = state.answer(&asked(
            "PUT",
            "/api/v4/projects/42/merge_requests/7/merge?sha=abc",
            &[("private-token", "t")],
            "",
        ));
        assert_eq!(
            (merged.status, merged.body["state"].clone()),
            (200, json!("merged"))
        );
    }

    #[test]
    fn a_request_it_has_no_answer_for_is_404_and_names_itself() {
        let mut state = State::default();
        let got = state.answer(&asked("POST", "/api/graphql", BEARER, "{}"));
        assert_eq!(got.status, 404);
        assert!(
            got.body["message"]
                .to_string()
                .contains("POST /api/graphql")
        );
    }
}

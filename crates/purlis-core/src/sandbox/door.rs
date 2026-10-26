// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's front door** (#2019): how a server a sandboxed chat runs is reached.
//!
//! A chat allowed to serve on port N never listens on a TCP port of this machine. Its server
//! listens on a unix socket in a folder purlis made for the chat alone (`0700`), and purlis
//! listens on `127.0.0.1:N` and `[::1]:N` in its place: the door. A browser cannot reach a unix
//! socket, so the person's browser, the chat's watched browser and the chat's own commands all
//! reach the server through the door, which holds every request to this:
//!
//! - **Its `Host` is this door's own**: exactly `localhost:N`, `127.0.0.1:N` or `[::1]:N`, or
//!   it is refused with `421`. A page that rebinds a name of its own to this machine is
//!   refused here.
//! - **An `Origin` is this door's own, where one is sent**: one of the three origins above,
//!   or it is refused with `403`. A page of another site never has the server act for it.
//! - **No cookie of the browser's reaches the server, and no cookie of the server's reaches the
//!   browser.** A browser sends a site's cookies to every port of that host, so the cookies of
//!   the person's other servers on this machine would reach the chat's. The door strips every
//!   `Cookie` and keeps every `Set-Cookie` in a jar of its own instead, one for each browser
//!   (told apart by a cookie of the door's own, `purlis-door-N`, which it never forwards), and
//!   hands that jar's cookies back to the server alone ([`Jar`]). So a dev app's own sign-in
//!   still works, and nothing of it is stored in a browser.
//! - **Read strictly, one request a connection.** The head is read whole within
//!   [`HEAD_WITHIN`] and at most [`HEAD_MOST`] bytes, every line as the chat's proxy reads one
//!   (`super::egress`), and a request two parsers could read two ways is refused with `400`. A
//!   body is sent on by its `Content-Length` alone; `Transfer-Encoding` is refused with `501`,
//!   as the chat's proxy refuses it. The server is asked to close after its answer, and so is
//!   the browser, so no second request on the connection is ever carried unread.
//! - **An upgrade (hot reload's WebSocket) is carried** once its request passed, and the server
//!   answered `101`: then both ways, as they are.
//! - **Streamed, never buffered**: a server's answer, an event stream included, goes on as it
//!   comes.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// How long a request's head may take to arrive whole.
pub const HEAD_WITHIN: Duration = Duration::from_secs(30);

/// The longest head the door reads, a request's or a server's answer's.
pub const HEAD_MOST: usize = 32 * 1024;

/// How many connections one door carries at once; one more is closed at once.
pub const AT_ONCE: usize = 256;

/// The most browsers a door keeps a jar for, and cookies in one jar: the oldest goes first.
pub const JARS_MOST: usize = 64;
pub const COOKIES_MOST: usize = 128;

/// The name of the cookie a door tells one browser from another by, on port `port`.
pub fn token_cookie(port: u16) -> String {
    format!("purlis-door-{port}")
}

/// What a request's head is answered with, where the door does not carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// `400`: not a request the door reads one way only.
    Malformed,
    /// `421`: its `Host` is not this door's.
    Host,
    /// `403`: it came from a page of another site.
    Origin,
    /// `501`: a body sent in chunks, which the door does not carry.
    Chunked,
}

impl Refused {
    /// The status line and the one sentence the door answers with.
    pub fn answer(&self) -> (&'static str, &'static str) {
        match self {
            Self::Malformed => (
                "400 Bad Request",
                "purlis's door reads this request no one way.",
            ),
            Self::Host => (
                "421 Misdirected Request",
                "purlis's door carries a request only to this server, by 127.0.0.1, ::1 or \
                 localhost and its port.",
            ),
            Self::Origin => (
                "403 Forbidden",
                "purlis's door carries no request a page of another site sent to this server.",
            ),
            Self::Chunked => (
                "501 Not Implemented",
                "purlis's door carries a body by its Content-Length alone.",
            ),
        }
    }
}

/// A request the door carries, as it sends it on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    /// The head sent to the server: the browser's own, without its cookies and with the jar's,
    /// asking the server to close after its answer (but for an upgrade).
    pub head: String,
    /// How many bytes of body follow it.
    pub body: u64,
    /// Whether it asks to upgrade the connection (a WebSocket).
    pub upgrade: bool,
    /// The browser's jar token, where it sent one.
    pub token: Option<String>,
}

/// **The request in `head`** (which ends in its blank line) **as the door on `port` carries
/// it, with `jar`'s cookies for it**, or why it is refused. Read as strictly as the chat's proxy
/// reads one (`super::egress::Request::parse`).
pub fn judge(
    head: &str,
    port: u16,
    jar: &dyn Fn(Option<&str>) -> Vec<String>,
) -> Result<Carried, Refused> {
    let lines: Vec<&str> = head
        .strip_suffix("\r\n\r\n")
        .ok_or(Refused::Malformed)?
        .split("\r\n")
        .collect();
    if lines.iter().any(|line| line.contains(['\r', '\n'])) {
        return Err(Refused::Malformed);
    }
    let parts: Vec<&str> = lines[0].split(' ').collect();
    let [method, target, version] = parts[..] else {
        return Err(Refused::Malformed);
    };
    if version != "HTTP/1.1"
        || method.is_empty()
        || !method.bytes().all(|b| b.is_ascii_uppercase())
        || method == "CONNECT"
        || !(target.starts_with('/') || (method == "OPTIONS" && target == "*"))
        || target.bytes().any(|b| b <= b' ' || b == 0x7f)
    {
        return Err(Refused::Malformed);
    }
    let mut headers: Vec<(&str, &str)> = Vec::new();
    for line in &lines[1..] {
        let (name, value) = line.split_once(':').ok_or(Refused::Malformed)?;
        if name.is_empty() || !name.bytes().all(is_token) {
            return Err(Refused::Malformed);
        }
        let value = value.trim_matches([' ', '\t']);
        if value.bytes().any(|b| (b < 0x20 && b != b'\t') || b == 0x7f) {
            return Err(Refused::Malformed);
        }
        headers.push((name, value));
    }
    let named = |wanted: &str| -> Vec<&str> {
        headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(wanted))
            .map(|(_, value)| *value)
            .collect()
    };
    if !named("transfer-encoding").is_empty() {
        return Err(Refused::Chunked);
    }
    let [host] = named("host")[..] else {
        return Err(if named("host").is_empty() {
            Refused::Host
        } else {
            Refused::Malformed
        });
    };
    if !own_hosts(port).iter().any(|own| own == host) {
        return Err(Refused::Host);
    }
    match named("origin")[..] {
        [] => {}
        [origin]
            if own_hosts(port)
                .iter()
                .any(|own| origin == format!("http://{own}")) => {}
        _ => return Err(Refused::Origin),
    }
    let body = match named("content-length")[..] {
        [] => 0,
        [length] if !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()) => {
            length.parse().map_err(|_| Refused::Malformed)?
        }
        _ => return Err(Refused::Malformed),
    };
    let token_name = token_cookie(port);
    let token = named("cookie")
        .iter()
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == token_name)
        .map(|(_, value)| value.to_owned())
        .filter(|value| is_a_token(value));
    let upgrade = !named("upgrade").is_empty()
        && named("connection")
            .iter()
            .flat_map(|value| value.split(','))
            .any(|option| option.trim().eq_ignore_ascii_case("upgrade"));
    let mut out = format!("{method} {target} {version}\r\n");
    for (name, value) in &headers {
        let dropped = [
            "cookie",
            "connection",
            "keep-alive",
            "proxy-connection",
            "te",
            "trailer",
        ]
        .iter()
        .any(|it| name.eq_ignore_ascii_case(it))
            || (!upgrade && name.eq_ignore_ascii_case("upgrade"));
        if !dropped {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    let kept = jar(token.as_deref());
    if !kept.is_empty() {
        out.push_str(&format!("Cookie: {}\r\n", kept.join("; ")));
    }
    out.push_str(if upgrade {
        "Connection: Upgrade\r\n\r\n"
    } else {
        "Connection: close\r\n\r\n"
    });
    Ok(Carried {
        head: out,
        body,
        upgrade,
        token,
    })
}

/// The three `Host`s a door on `port` answers to.
fn own_hosts(port: u16) -> [String; 3] {
    [
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ]
}

/// Whether `byte` may be in a header's name (RFC 9110's token).
fn is_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

/// Whether `value` is a jar token a door made: 32 lower-case hex.
fn is_a_token(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// **A server's answer's head, as the door hands it to the browser**: its `Set-Cookie`s taken
/// out and handed to `keep`, asking the browser to close after it (but for a `101`), and the
/// door's own jar cookie set where the browser has none (`new_token`). `None` for a head the
/// door does not read one way.
pub fn answered(
    head: &str,
    port: u16,
    new_token: Option<&str>,
    keep: &mut dyn FnMut(&str),
) -> Option<String> {
    let lines: Vec<&str> = head.strip_suffix("\r\n\r\n")?.split("\r\n").collect();
    let status = lines.first()?;
    let switching = status.starts_with("HTTP/1.1 101 ") || *status == "HTTP/1.1 101";
    if !status.starts_with("HTTP/1.") || lines.iter().any(|line| line.contains(['\r', '\n'])) {
        return None;
    }
    let mut out = format!("{status}\r\n");
    for line in &lines[1..] {
        let (name, value) = line.split_once(':')?;
        if name.is_empty() || !name.bytes().all(is_token) {
            return None;
        }
        let value = value.trim_matches([' ', '\t']);
        if name.eq_ignore_ascii_case("set-cookie") {
            keep(value);
            continue;
        }
        if !switching
            && ["connection", "keep-alive"]
                .iter()
                .any(|it| name.eq_ignore_ascii_case(it))
        {
            continue;
        }
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(token) = new_token {
        out.push_str(&format!(
            "Set-Cookie: {}={token}; Path=/; HttpOnly; SameSite=Strict\r\n",
            token_cookie(port)
        ));
    }
    if !switching {
        out.push_str("Connection: close\r\n");
    }
    out.push_str("\r\n");
    Some(out)
}

/// **The cookies a door keeps for the chat's server, one jar for each browser** (module docs):
/// by name, each with its value; a cookie the server sets again replaces it, and one it sets
/// with `Max-Age=0` or less, or an empty value, is taken away. At most [`JARS_MOST`] jars and
/// [`COOKIES_MOST`] cookies in each, the oldest let go first.
#[derive(Debug, Default)]
pub struct Jar {
    jars: Vec<(String, Vec<(String, String)>)>,
}

impl Jar {
    /// The cookies jar `token` holds, as `name=value`.
    pub fn of(&self, token: Option<&str>) -> Vec<String> {
        let Some(token) = token else {
            return Vec::new();
        };
        self.jars
            .iter()
            .find(|(held, _)| held == token)
            .map(|(_, cookies)| {
                cookies
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Keeps `set`, a `Set-Cookie`'s value, in jar `token`.
    pub fn keep(&mut self, token: &str, set: &str) {
        let mut parts = set.split(';');
        let Some((name, value)) = parts.next().and_then(|pair| pair.trim().split_once('=')) else {
            return;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() || !name.bytes().all(is_token) || value.contains([';', ',']) {
            return;
        }
        let gone = value.is_empty()
            || parts.any(|attribute| {
                attribute.trim().split_once('=').is_some_and(|(key, age)| {
                    key.trim().eq_ignore_ascii_case("max-age")
                        && age.trim().parse::<i64>().is_ok_and(|age| age <= 0)
                })
            });
        let at = match self.jars.iter().position(|(held, _)| held == token) {
            Some(at) => at,
            None => {
                if self.jars.len() >= JARS_MOST {
                    self.jars.remove(0);
                }
                self.jars.push((token.to_owned(), Vec::new()));
                self.jars.len() - 1
            }
        };
        let cookies = &mut self.jars[at].1;
        cookies.retain(|(held, _)| held != name);
        if !gone {
            if cookies.len() >= COOKIES_MOST {
                cookies.remove(0);
            }
            cookies.push((name.to_owned(), value.to_owned()));
        }
    }
}

/// A new jar token: 32 lower-case hex of the system's randomness.
fn new_token() -> Option<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The folder of purlis's data home that chats' servers listen in, one folder a chat.
pub const FOLDER: &str = "serve";

/// **The folder chat `id`'s servers listen in** (#2019): `<data>/serve/<id>`, the same each time
/// the chat starts, so a chat whose proxy outlived the window is served again where it was.
/// None where there is no data home, or `id` is not one word of letters, digits, `-` and `_`.
pub fn folder_for(id: &str) -> Option<PathBuf> {
    let one_word = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    one_word.then(|| crate::datahome::root().map(|data| data.join(FOLDER).join(id)))?
}

/// The variable a chat that serves is handed its servers' folder in, which `purlis listen`
/// reads (#2019).
pub const FOLDER_ENV: &str = "PURLIS_SERVE_DIR";

/// What a chat is told of `ports` it was allowed to serve on that it does not serve on this
/// run: something on this Mac listens on each already.
pub fn unserved_said(ports: &[u16]) -> Option<String> {
    let (last, rest) = ports.split_last()?;
    let ports = match rest {
        [] => format!("port {last}"),
        _ => format!(
            "ports {} and {last}",
            rest.iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    Some(format!(
        "This chat was allowed to serve on {ports}, but something on this Mac already listens \
         there, so it serves on none of them this run. Stop what listens there, then restart \
         this chat."
    ))
}

/// **Whether anything listens on `port` of this machine now**, by `127.0.0.1` or `::1`: what
/// a door, or a chat's server, may never take from it.
pub fn in_use(port: u16) -> bool {
    let wait = Duration::from_millis(300);
    [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ]
    .iter()
    .any(|at| TcpStream::connect_timeout(at, wait).is_ok())
}

/// **A chat's door on one port, listening while this is held**; dropped, it stops taking
/// connections and lets its port go.
#[derive(Debug)]
pub struct Door {
    port: u16,
    socket: PathBuf,
    stop: Arc<AtomicBool>,
}

impl Door {
    /// **The door on `port` to the server on `socket`**: listening on `127.0.0.1` and, where
    /// this machine has it, `::1`. Refused where anything listens on the port already
    /// ([`in_use`]).
    pub fn open(port: u16, socket: &Path) -> io::Result<Door> {
        if port == 0 || in_use(port) {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("something on this Mac already listens on port {port}"),
            ));
        }
        let mut listeners = vec![TcpListener::bind((Ipv4Addr::LOCALHOST, port))?];
        if let Ok(v6) = TcpListener::bind((Ipv6Addr::LOCALHOST, port)) {
            listeners.push(v6);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let jar = Arc::new(Mutex::new(Jar::default()));
        let busy = Arc::new(AtomicUsize::new(0));
        for listener in listeners {
            let (stop, jar, busy) = (Arc::clone(&stop), Arc::clone(&jar), Arc::clone(&busy));
            let socket = socket.to_path_buf();
            std::thread::Builder::new()
                .name(format!("purlis-door-{port}"))
                .spawn(move || {
                    for stream in listener.incoming() {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        let Ok(stream) = stream else {
                            continue;
                        };
                        if busy.fetch_add(1, Ordering::SeqCst) >= AT_ONCE {
                            busy.fetch_sub(1, Ordering::SeqCst);
                            continue;
                        }
                        let (held_jar, held_busy, socket) =
                            (Arc::clone(&jar), Arc::clone(&busy), socket.clone());
                        let spawned = std::thread::Builder::new()
                            .name(format!("purlis-door-{port}-in"))
                            .spawn(move || {
                                carry(stream, port, &socket, &held_jar);
                                held_busy.fetch_sub(1, Ordering::SeqCst);
                            });
                        if spawned.is_err() {
                            busy.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                })?;
        }
        Ok(Door {
            port,
            socket: socket.to_path_buf(),
            stop,
        })
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The server's socket it carries to.
    pub fn socket(&self) -> &Path {
        &self.socket
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes each accept, which then sees the stop and lets its listener go.
        for at in [
            SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            SocketAddr::from((Ipv6Addr::LOCALHOST, self.port)),
        ] {
            let _ = TcpStream::connect_timeout(&at, Duration::from_millis(200));
        }
    }
}

/// Carries one connection, `client`, to the server on `socket`, as the module says.
fn carry(mut client: TcpStream, port: u16, socket: &Path, jar: &Mutex<Jar>) {
    let Some((head, mut rest)) = read_head(&mut client) else {
        return;
    };
    let carried = {
        let held = jar.lock().unwrap_or_else(PoisonError::into_inner);
        judge(&head, port, &|token| held.of(token))
    };
    let carried = match carried {
        Ok(carried) => carried,
        Err(refused) => {
            let (status, said) = refused.answer();
            let _ = write!(
                client,
                "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{said}",
                said.len()
            );
            return;
        }
    };
    let Ok(mut server) = UnixStream::connect(socket) else {
        let said = "The chat's server is not running.";
        let _ = write!(
            client,
            "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: \
             {}\r\nConnection: close\r\n\r\n{said}",
            said.len()
        );
        return;
    };
    let _ = client.set_read_timeout(None);
    if server.write_all(carried.head.as_bytes()).is_err() {
        return;
    }
    // The body by its length alone: whatever the browser sent after it is never carried.
    let mut left = carried.body;
    let from_rest = usize::try_from(left).unwrap_or(usize::MAX).min(rest.len());
    if server.write_all(&rest[..from_rest]).is_err() {
        return;
    }
    left -= from_rest as u64;
    rest.drain(..from_rest);
    if left > 0 && io::copy(&mut (&client).take(left), &mut server).is_err() {
        return;
    }
    // The server's answer: its head read, its cookies kept, then the rest as it comes.
    let Some((answer, after)) = read_answer(&mut server) else {
        return;
    };
    let fresh = match &carried.token {
        Some(_) => None,
        None => new_token(),
    };
    let token = carried.token.clone().or_else(|| fresh.clone());
    let head = {
        let mut held = jar.lock().unwrap_or_else(PoisonError::into_inner);
        answered(&answer, port, fresh.as_deref(), &mut |set| {
            if let Some(token) = &token {
                held.keep(token, set);
            }
        })
    };
    let Some(head) = head else {
        return;
    };
    if client.write_all(head.as_bytes()).is_err() || client.write_all(&after).is_err() {
        return;
    }
    let switched = carried.upgrade && answer.starts_with("HTTP/1.1 101");
    if switched {
        // Both ways, as they are: what the browser sent after its head goes first.
        if !rest.is_empty() && server.write_all(&rest).is_err() {
            return;
        }
        let (Ok(mut client_in), Ok(mut server_out)) = (client.try_clone(), server.try_clone())
        else {
            return;
        };
        let up = std::thread::spawn(move || {
            let _ = io::copy(&mut client_in, &mut server_out);
            let _ = server_out.shutdown(Shutdown::Write);
        });
        let _ = io::copy(&mut server, &mut client);
        let _ = client.shutdown(Shutdown::Both);
        let _ = up.join();
    } else {
        let _ = io::copy(&mut server, &mut client);
        let _ = client.shutdown(Shutdown::Both);
    }
}

/// A request's head and what came after it, read whole within [`HEAD_WITHIN`].
fn read_head(client: &mut TcpStream) -> Option<(String, Vec<u8>)> {
    let deadline = std::time::Instant::now() + HEAD_WITHIN;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let left = deadline.checked_duration_since(std::time::Instant::now())?;
        if left.is_zero() || client.set_read_timeout(Some(left)).is_err() {
            return None;
        }
        let n = client.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            return String::from_utf8(buf).ok().map(|head| (head, rest));
        }
        if buf.len() > HEAD_MOST {
            return None;
        }
    }
}

/// A server's answer's head and what came after it.
fn read_answer(server: &mut UnixStream) -> Option<(String, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = server.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            return String::from_utf8(buf).ok().map(|head| (head, rest));
        }
        if buf.len() > HEAD_MOST {
            return None;
        }
    }
}

#[cfg(test)]
mod tests;

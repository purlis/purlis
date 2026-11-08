// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The ssh route's proxy command** (#3536): what a chat's ssh runs as its `ProxyCommand`, so
//! that every ssh connection leaves through the chat's SOCKS port ([`super::tunnel::SshRoute`]).
//!
//! It is the `purlis` binary itself, started as `purlis _ssh-proxy <socks port> <host> <port>`:
//! one SOCKS5 `CONNECT` (RFC 1928, no authentication, the host sent as a name so the proxy
//! judges and resolves it) to the loopback port, then ssh's stdin carried to the connection and
//! the connection to its stdout. The route ran `/usr/bin/nc -X 5 -x` before, which is OpenBSD
//! netcat's spelling: Fedora's and RHEL's `nc` is ncat, which refuses it, and a minimal system
//! has no `nc` at all. purlis is on every machine its chats run on, and speaks the same SOCKS
//! its own proxy serves, so the route depends on no program the system may or may not have.
//!
//! - **Never around the route.** It connects to the loopback interface at the port it is
//!   given and nowhere else; what may be reached is the proxy's to decide.
//! - **Fails closed, and says why.** A refusal, or a port nobody listens on, ends it with a
//!   line on stderr, which ssh shows the chat, and nothing carried.
//! - **Hidden.** Not a command anyone types: a route is its one caller, and the binary answers
//!   it before its command line is parsed ([`proxy_if_asked`]).

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

/// The hidden first word the binary answers as the route's proxy command.
pub const COMMAND: &str = "_ssh-proxy";

/// How long the greeting may take before the route is said not to answer. The `CONNECT`'s own
/// reply has no limit: the proxy may hold it while the person is asked about the host.
const GREETING: Duration = Duration::from_secs(10);

/// The exit code where nothing was carried: ssh's own for a connection that failed.
const UNCARRIED: i32 = 255;

/// **Answers `purlis _ssh-proxy <socks port> <host> <port>`**, with the code to exit with, or
/// `None` where the binary was started as anything else.
pub fn proxy_if_asked() -> Option<i32> {
    let mut words = std::env::args_os().skip(1);
    if words.next()? != COMMAND {
        return None;
    }
    let words: Vec<String> = words.map(|w| w.to_string_lossy().into_owned()).collect();
    let [socks, host, port] = words.as_slice() else {
        say("the ssh route's proxy takes a SOCKS port, a host and a port, so nothing was carried.");
        return Some(UNCARRIED);
    };
    let (Ok(socks), Ok(port)) = (socks.parse::<u16>(), port.parse::<u16>()) else {
        say(
            "the ssh route's proxy was handed a port that is not a number, so nothing was carried.",
        );
        return Some(UNCARRIED);
    };
    match carry(socks, host, port, io::stdin(), &mut io::stdout().lock()) {
        Ok(()) => Some(0),
        Err(why) => {
            say(&why.to_string());
            Some(UNCARRIED)
        }
    }
}

/// `line` on stderr, which ssh shows the chat: the proxy's one way to say why.
fn say(line: &str) {
    let _ = writeln!(io::stderr(), "purlis: {line}");
}

/// **Carries `from` to `host`:`port` through the SOCKS port `socks` on the loopback interface,
/// and what comes back to `to`**, until the far end closes. An error says, in a line for the
/// chat, why nothing was carried.
pub fn carry(
    socks: u16,
    host: &str,
    port: u16,
    mut from: impl Read + Send + 'static,
    to: &mut impl Write,
) -> io::Result<()> {
    let target = format!("{host}:{port}");
    let mut stream = connected(socks, host, port).map_err(|why| {
        io::Error::new(
            why.kind(),
            format!("ssh to {target} was not carried: {why}"),
        )
    })?;
    stream.set_read_timeout(None)?;
    let mut upstream = stream.try_clone()?;
    // What ssh writes, until it closes its end; then the connection is told no more comes.
    std::thread::spawn(move || {
        let _ = io::copy(&mut from, &mut upstream);
        let _ = upstream.shutdown(Shutdown::Write);
    });
    copy_flushing(&mut stream, to)?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

/// Copies `from` to `to`, flushing after each read, since ssh waits on each answer whole.
fn copy_flushing(from: &mut impl Read, to: &mut impl Write) -> io::Result<()> {
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        to.write_all(&buf[..n])?;
        to.flush()?;
    }
}

/// A connection the SOCKS port at `socks` has carried to `host`:`port`, or why not.
fn connected(socks: u16, host: &str, port: u16) -> io::Result<TcpStream> {
    let at = SocketAddr::from((Ipv4Addr::LOCALHOST, socks));
    let mut stream = TcpStream::connect_timeout(&at, GREETING).map_err(|why| {
        io::Error::new(
            why.kind(),
            format!("this chat's route, its SOCKS port {socks}, did not answer ({why})"),
        )
    })?;
    stream.set_read_timeout(Some(GREETING))?;
    stream.set_write_timeout(Some(GREETING))?;
    let not_socks = || {
        io::Error::other(format!(
            "this chat's route, its SOCKS port {socks}, did not answer as SOCKS5 does"
        ))
    };
    stream.write_all(&[VERSION, 1, NO_AUTH])?;
    let mut chosen = [0u8; 2];
    stream.read_exact(&mut chosen).map_err(|_| not_socks())?;
    if chosen != [VERSION, NO_AUTH] {
        return Err(not_socks());
    }
    stream.write_all(&request(host, port)?)?;
    // The proxy may hold the reply while the person is asked about this host.
    stream.set_read_timeout(None)?;
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).map_err(|_| not_socks())?;
    if head[0] != VERSION {
        return Err(not_socks());
    }
    if head[1] != SUCCEEDED {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            refused(head[1]),
        ));
    }
    // The bound address that follows is the proxy's to say and nothing of ours: read past it.
    let rest = match head[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).map_err(|_| not_socks())?;
            usize::from(len[0])
        }
        _ => return Err(not_socks()),
    } + 2;
    let mut bound = vec![0u8; rest];
    stream.read_exact(&mut bound).map_err(|_| not_socks())?;
    Ok(stream)
}

/// The `CONNECT` for `host`:`port`: an address as one, anything else as a name, which the
/// proxy judges and resolves itself.
fn request(host: &str, port: u16) -> io::Result<Vec<u8>> {
    let mut asked = vec![VERSION, CONNECT, 0];
    let bare = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(v4) = bare.parse::<Ipv4Addr>() {
        asked.push(1);
        asked.extend(v4.octets());
    } else if let Ok(v6) = bare.parse::<Ipv6Addr>() {
        asked.push(4);
        asked.extend(v6.octets());
    } else {
        let name = u8::try_from(host.len())
            .ok()
            .filter(|len| *len > 0)
            .ok_or_else(|| io::Error::other("its host name is empty or longer than 255 bytes"))?;
        asked.push(3);
        asked.push(name);
        asked.extend(host.as_bytes());
    }
    asked.extend(port.to_be_bytes());
    Ok(asked)
}

/// Why the proxy refused, by SOCKS's reply code.
fn refused(code: u8) -> String {
    match code {
        2 => "this chat may not reach it. Its sandbox lists the hosts and ports it may reach."
            .to_owned(),
        4 => "the host could not be reached.".to_owned(),
        5 => "the host refused the connection.".to_owned(),
        other => format!("this chat's route refused it (SOCKS reply {other})."),
    }
}

/// SOCKS's version byte.
const VERSION: u8 = 5;
/// The one method asked for: none.
const NO_AUTH: u8 = 0;
/// SOCKS's `CONNECT` command.
const CONNECT: u8 = 1;
/// SOCKS's reply that carried it.
const SUCCEEDED: u8 = 0;

/// **The `ProxyCommand` a route writes for its SOCKS port `socks`**: `program`, purlis's own
/// binary, run as [`COMMAND`] where one was found. Where none was, a command that carries
/// nothing and says why: ssh fails, as it would with no route, and the chat is told.
pub fn proxy_command(program: Option<&Path>, socks: u16) -> String {
    match program {
        // `%` is a token ssh expands in a ProxyCommand, so one in the path is doubled.
        Some(program) => format!(
            "{} {COMMAND} {socks} %h %p",
            sh_quoted(&program.display().to_string()).replace('%', "%%")
        ),
        None => format!(
            "/bin/sh -c {}",
            sh_quoted(&format!(
                "echo {} >&2; exit {UNCARRIED}",
                sh_quoted(NONE_FOUND)
            ))
        ),
    }
}

/// What a chat's ssh says where no purlis binary was found for its route.
const NONE_FOUND: &str = "purlis: ssh was not carried: no purlis command line was found beside the app to carry it through the route of this chat. Reinstall purlis, or set PURLIS_BINARY to its command line.";

/// `text` in single quotes, as a POSIX shell reads one word.
fn sh_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

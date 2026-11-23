// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat does on the machine it runs on, said on its own output so a test sees it in the
//! chat's pane wherever the chat runs (#3288): `--serve`, `--tunnel` and `--listen-at`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// The longest a service or a listener stays, so one its test never ended ends by itself.
const AT_MOST: Duration = Duration::from_secs(300);

/// `--serve`: a service on a port of this machine's loopback, which answers nothing. Says its
/// port, then holds every connection it takes until it is ended.
pub fn serve() -> Result<(), String> {
    let service =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|err| format!("cannot serve: {err}"))?;
    let port = service
        .local_addr()
        .map_err(|err| format!("cannot read its port: {err}"))?
        .port();
    say(&format!("serving on port {port}"))?;
    let ends = std::time::Instant::now() + AT_MOST;
    service
        .set_nonblocking(true)
        .map_err(|err| format!("cannot serve: {err}"))?;
    let mut held = Vec::new();
    while std::time::Instant::now() < ends {
        match service.accept() {
            Ok((stream, _)) => held.push(stream),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(format!("cannot take a connection: {err}")),
        }
    }
    Ok(())
}

/// `--tunnel <target> --through <proxy>`: asks the HTTP proxy at `proxy` (`host:port`, or its
/// URL) for a tunnel to `target`, and says the status line it answers.
pub fn tunnel(target: &str, proxy: &str) -> Result<(), String> {
    let proxy = proxy.trim_start_matches("http://").trim_end_matches('/');
    let mut stream = TcpStream::connect(proxy)
        .map_err(|err| format!("cannot reach the proxy {proxy}: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|err| format!("cannot wait: {err}"))?;
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .map_err(|err| format!("cannot ask the proxy: {err}"))?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|err| format!("the proxy answered nothing: {err}"))?;
    say(&format!("tunnel to {target}: {}", line.trim_end()))
}

/// `--listen-at <path>`: takes the socket's path over with a listener of its own, says so, and
/// ends once its one caller has gone.
#[cfg(unix)]
pub fn listen_at(path: &std::path::Path) -> Result<(), String> {
    let _ = std::fs::remove_file(path);
    let listener = std::os::unix::net::UnixListener::bind(path)
        .map_err(|err| format!("cannot listen at {}: {err}", path.display()))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("cannot listen: {err}"))?;
    say("listening beside the chat.")?;
    let ends = std::time::Instant::now() + AT_MOST;
    let mut caller = loop {
        match listener.accept() {
            Ok((caller, _)) => break caller,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= ends {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(format!("cannot take a caller: {err}")),
        }
    };
    caller
        .set_nonblocking(false)
        .and_then(|()| caller.set_read_timeout(Some(AT_MOST)))
        .map_err(|err| format!("cannot wait: {err}"))?;
    let mut sent = Vec::new();
    let _ = caller.read_to_end(&mut sent);
    Ok(())
}

fn say(line: &str) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .map_err(|err| format!("cannot write output: {err}"))
}

/// `dial <variable>`: connects to the host and port the variable's value names (a database URL
/// or a bare `host:port`) and says `DIALED`, or why it could not. A tool of a persona in a
/// scenario: the value a vault hands `purlis secret exec` is pointed at a tunnel, and this is the
/// client that dials it.
pub fn dial(variable: &str) -> Result<(), String> {
    use std::net::ToSocketAddrs as _;
    let value = std::env::var(variable).map_err(|_| format!("{variable} is not set"))?;
    let target = dialled(&value).ok_or_else(|| format!("{variable} names no host and port"))?;
    let address = target
        .to_socket_addrs()
        .map_err(|err| format!("cannot resolve {target}: {err}"))?
        .next()
        .ok_or_else(|| format!("{target} has no address"))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|err| format!("cannot reach {target}: {err}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {target}\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.read_to_end(&mut Vec::new());
    say("DIALED")
}

/// The `host:port` a value names: the authority of a URL (after any `user:password@`), or the
/// value itself.
fn dialled(value: &str) -> Option<String> {
    let rest = value.split_once("://").map_or(value, |(_, rest)| rest);
    let authority = rest.split(['/', '?']).next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, at)| at);
    let (host, port) = authority.rsplit_once(':')?;
    (!host.is_empty() && port.parse::<u16>().is_ok()).then(|| authority.to_owned())
}

#[cfg(test)]
mod tests {
    use super::dialled;

    #[test]
    fn a_database_urls_authority_is_what_it_dials() {
        assert_eq!(
            dialled("postgres://app:pw@127.0.0.1:41234/orders?sslmode=disable").as_deref(),
            Some("127.0.0.1:41234")
        );
    }

    #[test]
    fn a_bare_host_and_port_is_dialled_as_it_is() {
        assert_eq!(dialled("db.test:5432").as_deref(), Some("db.test:5432"));
    }

    #[test]
    fn a_value_with_no_port_dials_nothing() {
        assert_eq!(dialled("postgres://app@db.test/orders"), None);
    }
}

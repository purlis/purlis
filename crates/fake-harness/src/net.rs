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

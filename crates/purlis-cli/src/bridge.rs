// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis bridge` (ADR 0078 §2): **a pipe, not a host.** What a connector runs on a runner: its
//! standard input and output joined to the runner's host's link socket, as the same OS user.
//! The link's handshake ends in the host, so the bridge holds no key and decides nothing.
//!
//! `purlis bridge --pair` pairs the runner (ADR 0078 §3, #2044): it reads the desktop's pin on
//! its standard input, one line of [`purlis_core::runnerlink::to_line`], pins it as the
//! runner's one desktop, makes the runner's link key at the first pairing, and writes the
//! runner's own pin on its standard output for the desktop to keep. The connector's own login
//! vouches for this one exchange; after it, the keys decide.

use std::io::{BufRead, Read, Write};
use std::time::{Duration, Instant};

/// How long the bridge waits for the host's link socket, for a host that is still starting.
const HOST_WITHIN: Duration = Duration::from_secs(60);

/// The longest pin line the pairing reads.
const MOST_PIN: u64 = 4096;

pub fn run(pair: bool) -> Result<u8, String> {
    let config = purlis_core::machine::config_root()
        .ok_or("there is no home folder, so this runner keeps no link key")?;
    let folder = purlis_core::machine::charterd(&config);
    if pair {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .take(MOST_PIN)
            .read_line(&mut line)
            .map_err(|why| format!("the desktop's pin could not be read: {why}"))?;
        let desktop = purlis_core::runnerlink::from_line(&line)?;
        let device = purlis_core::machine::device_id(&config)
            .map_err(|why| format!("this runner has no device id: {why}"))?;
        let ours = purlis_core::runnerlink::pair(&config, &device, &desktop)?;
        let mut out = std::io::stdout().lock();
        writeln!(out, "{}", purlis_core::runnerlink::to_line(&ours))
            .and_then(|()| out.flush())
            .map_err(|why| format!("the runner's pin could not be written: {why}"))?;
        return Ok(0);
    }
    let link = purlis_host::Place::at(purlis_core::hostsocket::here(), folder).link();
    let stream = connect_within(&link, HOST_WITHIN)?;
    pipe(stream)
}

/// The host's link socket, waited for while a host that is starting makes it.
fn connect_within(
    link: &std::path::Path,
    within: Duration,
) -> Result<std::os::unix::net::UnixStream, String> {
    let deadline = Instant::now() + within;
    loop {
        match std::os::unix::net::UnixStream::connect(link) {
            Ok(stream) => return Ok(stream),
            Err(why) if Instant::now() >= deadline => {
                return Err(format!(
                    "no session host answered at {} within {}s ({why}); start it with \
                     `purlis serve`",
                    link.display(),
                    within.as_secs()
                ));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Standard input to the socket and the socket to standard output, until either side ends.
fn pipe(stream: std::os::unix::net::UnixStream) -> Result<u8, String> {
    let mut to_host = stream
        .try_clone()
        .map_err(|why| format!("the link socket could not be shared: {why}"))?;
    let mut from_host = stream;
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut to_host);
        let _ = to_host.shutdown(std::net::Shutdown::Write);
    });
    let mut out = std::io::stdout().lock();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        match from_host.read(&mut buffer) {
            Ok(0) | Err(_) => return Ok(0),
            Ok(read) => {
                if out
                    .write_all(&buffer[..read])
                    .and_then(|()| out.flush())
                    .is_err()
                {
                    return Ok(0);
                }
            }
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis bridge` (ADR 0078 §2): **a pipe, not a host.** What a connector runs on a runner: its
//! standard input and output joined to the runner's host's link socket, as the same OS user.
//! The link's handshake ends in the host, so the bridge holds no key and decides nothing.
//!
//! `purlis bridge --pair` pairs the runner, once (ADR 0078 §3, as ruled 2026-10-17, #2044): it
//! reads the desktop's device id and public link key on its standard input, one line of
//! [`purlis_core::runnerlink::to_line`], keeps them as the runner's one peer, mints the
//! runner's device id and makes its link key, and writes the runner's own device id and public
//! key on its standard output for the desktop to keep. For a Docker pool's runner the
//! connector's own login vouches for this one exchange; after it, the keys decide.
//!
//! A cluster runner's key is made earlier, as its pod starts (#2056): `purlis bridge --make-key
//! <file> --publish <file> --home <dir>`, the pod's first step, makes the runner's home on its
//! pod's volume (#2057), publishes the key a kept home already keeps, or hands the private half
//! of a new one to the pairing and
//! writes the public half where the cluster shows it, and the cluster operator vouches for that
//! key. `purlis bridge --pair --key-made-at-start <file>` then pairs with it, and the desktop
//! accepts the runner only when its answer is the vouched key. Pairing and making the key are
//! the bridge's only acts that are not a pipe.

use std::io::{BufRead, Read, Write};
use std::time::{Duration, Instant};

/// How long the bridge waits for the host's link socket, for a host that is still starting.
const HOST_WITHIN: Duration = Duration::from_secs(60);

/// What the bridge is asked to do.
pub enum Asked {
    /// Be a pipe to the host's link socket.
    Pipe,
    /// Pair the runner, with a new key, or with the key its pod made as it started.
    Pair {
        key_made_at_start: Option<std::path::PathBuf>,
    },
    /// Make a cluster runner's link key as its pod starts (#2056), with its home on its pod's
    /// volume (#2057).
    MakeKey {
        handoff: std::path::PathBuf,
        published: std::path::PathBuf,
        home: Option<std::path::PathBuf>,
    },
}

pub fn run(asked: Asked) -> Result<u8, String> {
    if let Asked::MakeKey {
        handoff,
        published,
        home,
    } = &asked
    {
        match home {
            // The image's own HOME is this step's: the runner's is set to `home` (#2057).
            Some(home) => purlis_core::runnerlink::make_at_start_in(
                handoff,
                published,
                home,
                std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .as_deref(),
                &purlis_core::machine::config_root_under(home),
            )?,
            None => purlis_core::runnerlink::make_at_start(handoff, published)?,
        }
        return Ok(0);
    }
    let config = purlis_core::machine::config_root()
        .ok_or("there is no home folder, so this runner keeps no link key")?;
    let folder = purlis_core::machine::charterd(&config);
    if let Asked::Pair { key_made_at_start } = &asked {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .take(purlis_core::runnerlink::MOST_LINE)
            .read_line(&mut line)
            .map_err(|why| format!("the desktop's device id and key could not be read: {why}"))?;
        let desktop = purlis_core::runnerlink::from_line(&line)?;
        let ours = match key_made_at_start {
            Some(handoff) => {
                purlis_core::runnerlink::pair_with_key_made_at_start(&config, &desktop, handoff)?
            }
            None => purlis_core::runnerlink::pair(&config, &desktop)?,
        };
        let mut out = std::io::stdout().lock();
        writeln!(out, "{}", purlis_core::runnerlink::to_line(&ours))
            .and_then(|()| out.flush())
            .map_err(|why| format!("the runner's device id and key could not be written: {why}"))?;
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

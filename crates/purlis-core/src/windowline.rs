// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A line from `purlis` in the person's own terminal to their running window** (#4141).
//!
//! `purlis profile remove` should reach the pools' runners as Remove in the window does, but
//! only the window holds what a runner's link needs, and the command reads none of it. The hook
//! socket is no road for this: every line on it carries a chat's token, and this command is
//! the person's, in no chat.
//!
//! **What keeps this safe.** The socket sits beside the session host's, in the folder only this
//! user may enter and every chat's sandbox is denied ([`crate::hostsocket`]), and each side
//! closes a peer of another user unread. It serves one thing: [`answer`] refuses a name still
//! among this machine's profiles as it is asked, so a line asks what the next tidy would do
//! anyway. It is private to one build and promises nothing across versions: where the words do
//! not read, the command says what it says with no window. What the window says is marked before
//! a terminal shows it.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long the command waits on the window, which asks each runner in turn over its link.
const THE_WINDOW_ANSWERS_WITHIN: Duration = Duration::from_secs(60);
/// The most of one line either side reads: a name, or a note that names pools.
const LONGEST_LINE: usize = 64 * 1024;
/// How long the window waits for a peer's whole line, and to write its answer.
const A_PEER_HAS: Duration = Duration::from_secs(2);

/// What the command asks of the window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asked {
    /// The profile `name` was just removed on this machine: each pool's runner forgets it.
    ForgetOffThePools { name: String },
}

/// What the window says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Said {
    /// The runners were asked; `note` names the pools whose runner keeps it (before 1.52).
    Asked { note: Option<String> },
    /// Nothing was asked, for this reason.
    Refused { why: String },
}

/// How an ask of the window went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// Nothing listens: no window is running here.
    NoWindow,
    /// It was heard, and nothing readable came back in time.
    NoAnswer,
    Said(Said),
}

/// **The window's answer to `asked`**: `forget` runs only for a name this machine, with its
/// config home at `config`, no longer has among its profiles, so no line takes a live profile's
/// sign-in off a runner.
pub fn answer(
    config: &Path,
    asked: Asked,
    forget: impl FnOnce(&str) -> Result<Option<String>, String>,
) -> Said {
    let Asked::ForgetOffThePools { name } = asked;
    if !crate::profiles::name_ok(&name) {
        return Said::Refused {
            why: "that is not a profile's name".to_owned(),
        };
    }
    match crate::profilestore::list(config) {
        Err(why) => Said::Refused { why },
        Ok(records) if records.iter().any(|record| record.name == name) => Said::Refused {
            why: format!(
                "'{}' is still a profile on this machine",
                crate::shown::short(&name)
            ),
        },
        Ok(_) => match forget(&name) {
            Ok(note) => Said::Asked { note },
            Err(why) => Said::Refused { why },
        },
    }
}

/// **What `purlis profile remove` does of the pools once the profile is gone**: first no drain
/// carries its sign-in, then `ask` has the window ask the runners. The lines that say how it
/// went, none where this machine has no Docker pool.
pub fn reach_the_pools(config: &Path, name: &str, ask: impl FnOnce(&Asked) -> Answered) -> String {
    let Ok(pools) = crate::runners::pools(config) else {
        return "this machine's pools could not be read, so its runners were not asked to forget \
                it, and a drain may carry its sign-in\n"
            .to_owned();
    };
    let mut out = String::new();
    for pool in &pools {
        if crate::runners::hold_from_carrying(config, pool.name(), name).is_err() {
            out.push_str(&format!(
                "pool {}'s runners could not be marked, so a drain may carry its sign-in\n",
                pool.name()
            ));
        }
    }
    if pools.is_empty() {
        return out;
    }
    let at_the_next_tidy = "each Docker pool's runner forgets it at its next tidy: a chat's start \
                            there, or its pool's profiles being shown. A runner of a purlis \
                            before 1.57 does not tidy, and one before 1.52 keeps it until it is \
                            made again.";
    let asked = Asked::ForgetOffThePools {
        name: name.to_owned(),
    };
    match ask(&asked) {
        Answered::NoWindow => {
            out.push_str(&format!(
                "no purlis window is running, so {at_the_next_tidy}\n"
            ));
        }
        Answered::NoAnswer => {
            out.push_str(&format!(
                "the purlis window did not answer in time; where it had not asked yet, \
                 {at_the_next_tidy}\n"
            ));
        }
        Answered::Said(Said::Refused { why }) => {
            out.push_str(&format!(
                "the purlis window did not ask the runners ({}), so {at_the_next_tidy}\n",
                shown(&why)
            ));
        }
        Answered::Said(Said::Asked { note }) => {
            out.push_str(
                "each Docker pool's runner was asked to forget it, its sign-in there with it; \
                 one that is away, or that could not, is asked again at its next tidy\n",
            );
            if let Some(note) = note {
                out.push_str(&format!("{}\n", shown(&note)));
            }
        }
    }
    out
}

/// What the window said, as a terminal may show it.
fn shown(said: &str) -> String {
    crate::shown::one_line(said, 1000)
}

/// Where the window listens: beside the session host of this config home, in its private folder.
pub fn socket() -> PathBuf {
    beside(&crate::hostsocket::here())
}

/// The window's socket beside the host's socket `host`.
fn beside(host: &Path) -> PathBuf {
    let stem = host.file_stem().map_or_else(
        || "purlisd".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    host.with_file_name(format!("{stem}-window.sock"))
}

/// Asks the running window.
pub fn ask(asked: &Asked) -> Answered {
    ask_on(&socket(), asked, THE_WINDOW_ANSWERS_WITHIN)
}

/// One line read within `within` all told, at most [`LONGEST_LINE`] of it.
fn line_of(mut stream: &UnixStream, within: Duration) -> Option<String> {
    let until = Instant::now() + within;
    let mut line = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let left = until
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())?;
        stream.set_read_timeout(Some(left)).ok()?;
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        line.extend_from_slice(&chunk[..read]);
        if line.contains(&b'\n') {
            return String::from_utf8(line).ok();
        }
        if line.len() > LONGEST_LINE {
            return None;
        }
    }
}

/// One value written as one line.
fn write_line(mut stream: &UnixStream, value: &impl serde::Serialize) -> Option<()> {
    let mut line = serde_json::to_string(value).ok()?;
    line.push('\n');
    stream.write_all(line.as_bytes()).ok()
}

/// Whether the peer on `stream` is this user.
fn this_user(stream: &UnixStream) -> bool {
    let them = purlis_same_user::peer_process_of(stream).map(|(uid, _)| uid);
    purlis_same_user::admit_peer(them, purlis_same_user::Uid::effective()).is_ok()
}

/// [`ask`], of the window listening on `socket`, waiting `within`. What another user listens on
/// is no window, and is told nothing.
pub fn ask_on(socket: &Path, asked: &Asked, within: Duration) -> Answered {
    let Ok(stream) = UnixStream::connect(socket) else {
        return Answered::NoWindow;
    };
    if !this_user(&stream) {
        return Answered::NoWindow;
    }
    let answered = || -> Option<Said> {
        stream.set_write_timeout(Some(A_PEER_HAS)).ok()?;
        write_line(&stream, asked)?;
        serde_json::from_str(&line_of(&stream, within)?).ok()
    };
    answered().map_or(Answered::NoAnswer, Answered::Said)
}

/// **Serves `answer` on `socket`**, one peer at a time on a thread for the app's life: a removal
/// is rare, and a peer has [`A_PEER_HAS`] for its whole line. A socket another window answers on is left alone, and said as the error; one
/// nothing answers on is what a last window left.
pub fn serve_on(
    socket: &Path,
    answer: impl Fn(Asked) -> Said + Send + 'static,
) -> std::io::Result<()> {
    crate::hostsocket::make_the_folder(socket)?;
    match UnixStream::connect(socket) {
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "another purlis window listens there",
            ));
        }
        Err(why)
            if !matches!(
                why.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Err(why);
        }
        Err(_) => {}
    }
    match std::fs::remove_file(socket) {
        Err(why) if why.kind() != std::io::ErrorKind::NotFound => return Err(why),
        _ => {}
    }
    let listener = UnixListener::bind(socket)?;
    std::thread::Builder::new()
        .name("window-line".to_owned())
        .spawn(move || {
            for peer in listener.incoming() {
                let Ok(peer) = peer else {
                    // Out of descriptors, say: not worth a spinning thread.
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                };
                if this_user(&peer) {
                    let _ = serve_one(&peer, &answer);
                }
            }
        })?;
    Ok(())
}

/// One peer's line, answered.
fn serve_one(peer: &UnixStream, answer: &impl Fn(Asked) -> Said) -> Option<()> {
    peer.set_write_timeout(Some(A_PEER_HAS)).ok()?;
    let said = match serde_json::from_str(&line_of(peer, A_PEER_HAS)?) {
        Ok(asked) => answer(asked),
        Err(_) => Said::Refused {
            why: "this window does not read that; update purlis".to_owned(),
        },
    };
    write_line(peer, &said)
}

#[cfg(test)]
mod tests;

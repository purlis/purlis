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
//! user may enter and every chat's sandbox is denied ([`crate::hostsocket`]), and a peer of
//! another user is closed unread. It serves one thing, which destroys nothing of a profile this
//! machine still has: [`answer`] refuses a name still among its profiles, so all a line can ask
//! is what the next tidy would do anyway. It is private to one build and promises nothing
//! across versions: where the words do not read, the command says what it says with no window.

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::time::Duration;

/// How long the command waits on the window, which asks each runner in turn over its link.
#[cfg(unix)]
const THE_WINDOW_ANSWERS_WITHIN: Duration = Duration::from_secs(60);

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
pub fn answer(config: &Path, asked: Asked, forget: impl FnOnce(&str) -> Option<String>) -> Said {
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
        Ok(_) => Said::Asked {
            note: forget(&name),
        },
    }
}

/// **What `purlis profile remove` does of the pools once the profile is gone**: first no drain
/// carries its sign-in, then `ask` has the window ask the runners. The lines that say how it
/// went, none where this machine has no Docker pool.
pub fn reach_the_pools(config: &Path, name: &str, ask: impl FnOnce(&Asked) -> Answered) -> String {
    let Ok(pools) = crate::runners::pools(config) else {
        return String::new();
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
    let docker = pools.iter().any(|pool| {
        matches!(
            crate::runners::cluster_runner_for(config, pool.name()),
            Ok(None)
        )
    });
    if !docker {
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
                "the purlis window did not ask the runners ({why}), so {at_the_next_tidy}\n"
            ));
        }
        Answered::Said(Said::Asked { note }) => {
            out.push_str(
                "each Docker pool's runner was asked to forget it, its sign-in there with it; \
                 one that is away, or that could not, is asked again at its next tidy\n",
            );
            if let Some(note) = note {
                out.push_str(&format!("{note}\n"));
            }
        }
    }
    out
}

/// Where the window listens: beside the session host of this config home, in its private folder.
#[cfg(unix)]
pub fn socket() -> PathBuf {
    beside(&crate::hostsocket::here())
}

/// The window's socket beside the host's socket `host`.
#[cfg(unix)]
fn beside(host: &Path) -> PathBuf {
    let stem = host.file_stem().map_or_else(
        || "purlisd".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    host.with_file_name(format!("{stem}-window.sock"))
}

/// Asks the running window.
#[cfg(unix)]
pub fn ask(asked: &Asked) -> Answered {
    ask_on(&socket(), asked, THE_WINDOW_ANSWERS_WITHIN)
}

/// No window listens where there are no unix sockets, so far.
#[cfg(not(unix))]
pub fn ask(_: &Asked) -> Answered {
    Answered::NoWindow
}

#[cfg(unix)]
pub use on_unix::{ask_on, serve_on};

#[cfg(unix)]
mod on_unix {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::time::Duration;

    use super::{Answered, Asked, Said};

    /// The most of one line either side reads: a name, or a note that names pools.
    const LONGEST_LINE: u64 = 64 * 1024;
    /// How long the window waits for a peer's line, and to write its answer.
    const A_PEER_HAS: Duration = Duration::from_secs(2);

    /// One line read, at most [`LONGEST_LINE`] of it.
    fn line_of(stream: &UnixStream) -> Option<String> {
        let mut line = String::new();
        BufReader::new(stream.take(LONGEST_LINE))
            .read_line(&mut line)
            .ok()?;
        line.ends_with('\n').then_some(line)
    }

    /// One value written as one line.
    fn write_line(mut stream: &UnixStream, value: &impl serde::Serialize) -> Option<()> {
        let mut line = serde_json::to_string(value).ok()?;
        line.push('\n');
        stream.write_all(line.as_bytes()).ok()
    }

    /// [`super::ask`], of the window listening on `socket`, waiting `within`.
    pub fn ask_on(socket: &Path, asked: &Asked, within: Duration) -> Answered {
        let Ok(stream) = UnixStream::connect(socket) else {
            return Answered::NoWindow;
        };
        let answered = || -> Option<Said> {
            stream.set_read_timeout(Some(within)).ok()?;
            stream.set_write_timeout(Some(A_PEER_HAS)).ok()?;
            write_line(&stream, asked)?;
            serde_json::from_str(&line_of(&stream)?).ok()
        };
        answered().map_or(Answered::NoAnswer, Answered::Said)
    }

    /// **Serves `answer` on `socket`**, one peer at a time on a thread for the app's life: a
    /// removal is rare, and a peer that says nothing is closed after [`A_PEER_HAS`]. A socket
    /// another window answers on is left alone, and said as the error; one nothing answers on
    /// is what a last window left.
    pub fn serve_on(
        socket: &Path,
        answer: impl Fn(Asked) -> Said + Send + 'static,
    ) -> std::io::Result<()> {
        crate::hostsocket::make_the_folder(socket)?;
        if UnixStream::connect(socket).is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "another purlis window listens there",
            ));
        }
        match std::fs::remove_file(socket) {
            Err(why) if why.kind() != std::io::ErrorKind::NotFound => return Err(why),
            _ => {}
        }
        let listener = UnixListener::bind(socket)?;
        let owner = purlis_same_user::Uid::effective();
        std::thread::Builder::new()
            .name("window-line".to_owned())
            .spawn(move || {
                for peer in listener.incoming() {
                    let Ok(peer) = peer else {
                        // Out of descriptors, say: not worth a spinning thread.
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    };
                    let them = purlis_same_user::peer_process_of(&peer).map(|(uid, _)| uid);
                    if purlis_same_user::admit_peer(them, owner).is_ok() {
                        let _ = serve_one(&peer, &answer);
                    }
                }
            })?;
        Ok(())
    }

    /// One peer's line, answered.
    fn serve_one(peer: &UnixStream, answer: &impl Fn(Asked) -> Said) -> Option<()> {
        peer.set_read_timeout(Some(A_PEER_HAS)).ok()?;
        peer.set_write_timeout(Some(A_PEER_HAS)).ok()?;
        let said = match serde_json::from_str(&line_of(peer)?) {
            Ok(asked) => answer(asked),
            Err(_) => Said::Refused {
                why: "this window does not read that; update purlis".to_owned(),
            },
        };
        write_line(peer, &said)
    }
}

#[cfg(test)]
mod tests;

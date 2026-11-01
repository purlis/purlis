// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A line from `purlis` in the person's own terminal to their running window** (#4141).
//!
//! `purlis profile remove` should reach the pools' runners as Remove in the window does, and
//! `purlis profile sign-in --pool` should keep a profile on a runner first as the strip does
//! (#4112), but only the window holds what a runner's link needs, and the command reads none of
//! it. The hook socket is no road for this: every line on it carries a chat's token, and this
//! command is the person's, in no chat.
//!
//! **What keeps this safe.** The socket sits beside the session host's, in the folder only this
//! user may enter and every chat's sandbox is denied ([`crate::hostsocket`]), and each side
//! closes a peer of another user unread. It serves two things. [`answer`] refuses a forget of a
//! name still among this machine's profiles as it is asked, so such a line asks what the next
//! tidy would do anyway. It refuses a keep of anything but one of this machine's profiles, and the
//! window holds it to the gate its own strip passes, so such a line asks what Sign in on the
//! profile's line would do. It is private to one build and promises nothing across versions: where the words do
//! not read, the command says what it says with no window. What the window says is marked before
//! a terminal shows it.
//!
//! **A pool sign-in started from the command is confirmed in the window** ([`approved`]): the
//! command asks before it reaches a runner or its engine, shows a code the dialog shows too, and
//! goes on only on the person's answer there. With no window it refuses. A keep from the command
//! then needs that yes as well.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long the command waits on the window, which asks each runner in turn over its link.
const THE_WINDOW_ANSWERS_WITHIN: Duration = Duration::from_secs(60);
/// How long the command waits on the window keeping a profile: a runner may fetch its harness.
const THE_WINDOW_KEEPS_WITHIN: Duration = Duration::from_secs(180);
/// How long the person has to answer in the window: then it is taken back, as a no.
pub const THE_PERSON_HAS: Duration = Duration::from_secs(120);
/// How much longer than that the command waits, to hear the window say so.
const THE_WINDOW_SAYS_WITHIN: Duration = Duration::from_secs(10);
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
    /// This machine's profile `name` is about to be signed in on pool `pool`: its runner keeps it
    /// first, and the pool names it (#4112).
    KeepOnAPool { name: String, pool: String },
    /// This machine's profile `name` is about to be signed in on pool `pool`, by the command: the
    /// person answers in the window. `code` is what the command's terminal shows, for the person
    /// to compare; `config` is the config home the command goes on to read.
    ApproveSignIn {
        name: String,
        pool: String,
        code: String,
        config: PathBuf,
    },
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
/// sign-in off a runner; `keep` only for one it has, so no line puts anything else on one;
/// `approve`, given the name, the pool and the code, only for a profile it has on a pool it has
/// that may be signed in on, and a command reading these same files, so the person is asked only
/// of what the window's own files name.
pub fn answer(
    config: &Path,
    asked: Asked,
    forget: impl FnOnce(&str) -> Result<Option<String>, String>,
    keep: impl FnOnce(&str, &str) -> Result<Option<String>, String>,
    approve: impl FnOnce(&str, &str, &str) -> Result<Option<String>, String>,
) -> Said {
    let name = match &asked {
        Asked::ForgetOffThePools { name }
        | Asked::KeepOnAPool { name, .. }
        | Asked::ApproveSignIn { name, .. } => name,
    };
    if !crate::profiles::name_ok(name) {
        return Said::Refused {
            why: "that is not a profile's name".to_owned(),
        };
    }
    let has_it = match crate::profilestore::list(config) {
        Ok(records) => records.iter().any(|record| &record.name == name),
        Err(why) => return Said::Refused { why },
    };
    let said = match &asked {
        Asked::ForgetOffThePools { .. } if has_it => {
            return Said::Refused {
                why: format!(
                    "'{}' is still a profile on this machine",
                    crate::shown::short(name)
                ),
            };
        }
        Asked::ForgetOffThePools { name } => forget(name),
        Asked::KeepOnAPool { .. } | Asked::ApproveSignIn { .. } if !has_it => {
            return Said::Refused {
                why: format!(
                    "this machine has no profile named '{}'",
                    crate::shown::short(name)
                ),
            };
        }
        Asked::KeepOnAPool { name, pool } => keep(name, pool),
        Asked::ApproveSignIn {
            name,
            pool,
            code,
            config: theirs,
        } => {
            if let Err(why) = may_ask_the_person(config, pool, code, theirs) {
                return Said::Refused { why };
            }
            approve(name, pool, code)
        }
    };
    match said {
        Ok(note) => Said::Asked { note },
        Err(why) => Said::Refused { why },
    }
}

/// Why the window would not ask the person of a sign-in on `pool`, if it would not.
fn may_ask_the_person(config: &Path, pool: &str, code: &str, theirs: &Path) -> Result<(), String> {
    if !a_code(code) {
        return Err("that is not a code this purlis made".to_owned());
    }
    let same = |one: &Path, other: &Path| match (one.canonicalize(), other.canonicalize()) {
        (Ok(one), Ok(other)) => one == other,
        _ => false,
    };
    if !same(config, theirs) {
        return Err("that command reads another config home than this window".to_owned());
    }
    let pools = crate::runners::pool_names(config).map_err(|why| why.to_string())?;
    if !pools.iter().any(|one| one == pool) {
        return Err(format!(
            "this machine has no pool named '{}'",
            crate::shown::short(pool)
        ));
    }
    if crate::runnertaint::pool_ran_unsandboxed(config, pool) {
        return Err(crate::runnertaint::signs_nothing_in_on(pool));
    }
    Ok(())
}

/// How many letters a sign-in's code has.
const CODE_LETTERS: usize = 6;

/// Whether `code` reads as [`a_new_code`] writes one.
fn a_code(code: &str) -> bool {
    code.len() == CODE_LETTERS && code.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// **A code for one sign-in's question**: the random end of a new ULID.
pub fn a_new_code() -> String {
    let ulid = ulid::Ulid::generate().to_string();
    ulid[ulid.len() - CODE_LETTERS..].to_owned()
}

/// **Whether the person approved, in their window, signing `name` in on pool `pool`** from this
/// command, whose config home is `config`: `ask` puts the question, with `code`, to the running
/// window, and only its yes goes on. With no window running, no answer in time, or a no, this says
/// so and nothing was started.
pub fn approved(
    config: &Path,
    pool: &str,
    name: &str,
    code: &str,
    ask: impl FnOnce(&Asked) -> Answered,
) -> Result<(), String> {
    let asked = Asked::ApproveSignIn {
        name: name.to_owned(),
        pool: pool.to_owned(),
        code: code.to_owned(),
        config: config.to_path_buf(),
    };
    match ask(&asked) {
        Answered::Said(Said::Asked { .. }) => Ok(()),
        Answered::NoWindow => Err(format!(
            "signing '{name}' in on pool {pool} from here is confirmed in the purlis window, and \
             none is running, so nothing was started. Open purlis and run this again, or sign it \
             in from its row in Settings › Pools."
        )),
        Answered::NoAnswer => Err(format!(
            "the purlis window did not answer in time, so '{name}' was not signed in on pool \
             {pool} and nothing was started. Run this again."
        )),
        Answered::Said(Said::Refused { why }) => Err(format!(
            "the purlis window did not confirm signing '{name}' in on pool {pool}: {}. Nothing \
             was started.",
            shown(&why)
        )),
    }
}

/// **How a runner stands with `name`, kept there first where it was not** (#4112): `standing` asks
/// the runner, `None` where it keeps no such profile; then `ask` has the window keep it on pool
/// `pool`, as the strip does before a sign-in, and the runner is asked again. Only the window
/// can keep it, so with none running this says so and nothing was sent.
pub fn kept_first<S>(
    pool: &str,
    name: &str,
    standing: impl Fn() -> Result<Option<S>, String>,
    ask: impl FnOnce(&Asked) -> Answered,
) -> Result<S, String> {
    if let Some(stands) = standing()? {
        return Ok(stands);
    }
    let not_kept = format!("pool {pool}'s runner does not keep '{name}'");
    let asked = Asked::KeepOnAPool {
        name: name.to_owned(),
        pool: pool.to_owned(),
    };
    match ask(&asked) {
        Answered::NoWindow => Err(format!(
            "{not_kept}, and only the purlis window can put it there: none is running. Open \
             purlis and run this again, or sign it in from its row in Settings › Pools."
        )),
        Answered::NoAnswer => Err(format!(
            "{not_kept}, and the purlis window did not say in time whether it put it there, so \
             nothing was started. Run this again."
        )),
        Answered::Said(Said::Refused { why }) => Err(format!(
            "{not_kept}, and the purlis window did not put it there: {}. Nothing was started.",
            shown(&why)
        )),
        Answered::Said(Said::Asked { .. }) => standing()?.ok_or_else(|| {
            format!("{not_kept}, even after the purlis window asked it to, so nothing was started.")
        }),
    }
}

/// **What `purlis profile remove` does of the pools once the profile is gone**: first no drain
/// carries its sign-in, then `ask` has the window ask the runners. The lines that say how it
/// went, none where this machine has no pool.
pub fn reach_the_pools(config: &Path, name: &str, ask: impl FnOnce(&Asked) -> Answered) -> String {
    let Ok(pools) = crate::runners::pool_names(config) else {
        return "this machine's pools could not be read, so its runners were not asked to forget \
                it, and a drain may carry its sign-in\n"
            .to_owned();
    };
    let mut out = String::new();
    for pool in &pools {
        if crate::runners::hold_from_carrying(config, pool, name).is_err() {
            out.push_str(&format!(
                "pool {pool}'s runners could not be marked, so a drain may carry its sign-in\n"
            ));
        }
    }
    if pools.is_empty() {
        return out;
    }
    let at_the_next_tidy = "each pool's runner forgets it at its next tidy: a chat's start \
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
                "each pool's runner shown to be yours was asked to forget it, its sign-in there with \
                 it; one that is away, or that could not, is asked again at its next tidy\n",
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
pub fn beside(host: &Path) -> PathBuf {
    let stem = host.file_stem().map_or_else(
        || "purlisd".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    host.with_file_name(format!("{stem}-window.sock"))
}

/// Asks the running window.
pub fn ask(asked: &Asked) -> Answered {
    let within = match asked {
        Asked::ForgetOffThePools { .. } => THE_WINDOW_ANSWERS_WITHIN,
        Asked::KeepOnAPool { .. } => THE_WINDOW_KEEPS_WITHIN,
        Asked::ApproveSignIn { .. } => THE_PERSON_HAS + THE_WINDOW_SAYS_WITHIN,
    };
    ask_on(&socket(), asked, within)
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

/// **Serves `answer` on `socket`** for the app's life, each peer on a thread of its own, since a
/// keep can wait minutes on a runner and holds no other up. A peer has [`A_PEER_HAS`] for its whole
/// line. A socket another window answers on is left alone, and said as the error; one
/// nothing answers on is what a last window left.
pub fn serve_on(
    socket: &Path,
    answer: impl Fn(Asked) -> Said + Send + Sync + 'static,
) -> std::io::Result<()> {
    serve_watching(socket, move |asked, _| answer(asked))
}

/// [`serve_on`], `answer` also handed what says whether its peer has hung up, for what waits long.
pub fn serve_watching(
    socket: &Path,
    answer: impl Fn(Asked, &dyn Fn() -> bool) -> Said + Send + Sync + 'static,
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
    let answer = std::sync::Arc::new(answer);
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
                    let answer = std::sync::Arc::clone(&answer);
                    // Out of threads, the peer hears nothing, which it says as no answer.
                    let _ = std::thread::Builder::new()
                        .spawn(move || serve_one(&peer, answer.as_ref()));
                }
            }
        })?;
    Ok(())
}

/// Whether the peer closed its end: it sends nothing more after its line, so a read ends at once
/// where it did, and waits out a moment where it did not.
fn hung_up(mut peer: &UnixStream) -> bool {
    if peer
        .set_read_timeout(Some(Duration::from_millis(1)))
        .is_err()
    {
        return false;
    }
    matches!(peer.read(&mut [0u8; 16]), Ok(0))
}

/// One peer's line, answered.
fn serve_one(peer: &UnixStream, answer: &impl Fn(Asked, &dyn Fn() -> bool) -> Said) -> Option<()> {
    peer.set_write_timeout(Some(A_PEER_HAS)).ok()?;
    let said = match serde_json::from_str(&line_of(peer, A_PEER_HAS)?) {
        Ok(asked) => answer(asked, &|| hung_up(peer)),
        Err(_) => Said::Refused {
            why: "this window does not read that; update purlis".to_owned(),
        },
    };
    write_line(peer, &said)
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat's terminal starts with, wherever it runs: in the window's process, or in the
//! session host (#1998). One function, so a chat in the host starts with the environment and
//! `PATH` the same code gives it in the window (spec #1974, user story 16).
//!
//! The chat's environment is [`crate::chatenv::compose`]'s, from the environment of whatever
//! starts it: the window's own, or the session host's, which the window started with its own.
//! Then what the starter adds for the chat itself: its number, and where its hooks report with
//! its token. Every product variable goes under both its names (V93k).

use std::ffi::OsString;
use std::path::Path;

use crate::engine::Size;
use crate::session::Spec;

/// A chat's terminal, as its start composed it.
pub struct Terminal<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    /// Where it starts, or the starter's own working directory.
    pub cwd: Option<&'a str>,
    pub size: Size,
    /// What its environment is made of.
    pub starting: crate::chatenv::Starting<'a>,
    /// The chat's number: the key of its per-chat state (`active::SESSION_ID_ENV`).
    pub number: u32,
    /// Where its hooks report, when the starter listens on a hook socket.
    pub reporting: Option<Reporting<'a>>,
}

/// Where a chat's hooks report, and the token each of its lines carries.
pub struct Reporting<'a> {
    pub socket: &'a Path,
    pub token: &'a str,
}

/// The spec a chat's terminal is started from, with `app` the starter's own environment.
pub fn spec(app: impl IntoIterator<Item = (OsString, OsString)>, chat: &Terminal<'_>) -> Spec {
    let mut spec = Spec::new(chat.program, chat.size).args(chat.args);
    spec.cwd = chat.cwd.map(Into::into);
    // **A chat starts from an empty environment plus a keep-list** (`crate::chatenv`).
    spec.env_clear = true;
    spec.env = crate::chatenv::compose(app, &chat.starting);
    // The chat is what purlis's per-session state is keyed on, whether or not there is a socket
    // to report to (charter-app#63).
    spec.env.push((
        crate::active::SESSION_ID_ENV.into(),
        chat.number.to_string().into(),
    ));
    if let Some(reporting) = &chat.reporting {
        spec.env.push((
            crate::hookwire::SOCKET_ENV.into(),
            reporting.socket.as_os_str().to_owned(),
        ));
        spec.env.push((
            crate::hookwire::CHAT_ENV.into(),
            chat.number.to_string().into(),
        ));
        spec.env
            .push((crate::hookwire::TOKEN_ENV.into(), reporting.token.into()));
    }
    // What the starter added, under both names too (V93k), as `compose` gave the rest.
    spec.env = crate::envvar::twinned(std::mem::take(&mut spec.env));
    spec
}

/// A chat's start as the window composed it and handed it to the session host, by its words
/// (the session protocol's `launch`, #1998): [`launched`] turns it into the spec the window
/// would have started.
pub struct Launched<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    pub cwd: Option<&'a str>,
    pub set: &'a [(String, String)],
    pub strip: &'a [String],
    pub pass: &'a [String],
    pub declared: &'a [String],
    /// The harness, by the word the plane calls it ([`crate::harness::Harness::name`]).
    pub harness: Option<&'a str>,
    /// The folder of purlis's git hooks.
    pub git_hooks: Option<&'a str>,
    pub reporting: Option<Reporting<'a>>,
}

/// The spec for a chat the window composed, numbered `number` and started at `size`, with `app`
/// the starter's own environment: [`spec`], as the window starts it. A harness this build does
/// not know is refused in a sentence.
pub fn launched(
    app: impl IntoIterator<Item = (OsString, OsString)>,
    launch: &Launched<'_>,
    number: u32,
    size: Size,
) -> Result<Spec, String> {
    let harness =
        match launch.harness {
            Some(word) => Some(crate::harness::Harness::of_kind(word).ok_or_else(|| {
                format!("this build of purlis does not start the harness {word:?}")
            })?),
            None => None,
        };
    let git_hooks = launch.git_hooks.map(crate::githooks::GitHooks::at);
    Ok(spec(
        app,
        &Terminal {
            program: launch.program,
            args: launch.args,
            cwd: launch.cwd,
            size,
            starting: crate::chatenv::Starting {
                harness,
                operator: launch.pass,
                strip: launch.strip,
                set: launch.set,
                git_hooks: git_hooks.as_ref(),
                declared: launch.declared,
            },
            number,
            reporting: launch.reporting.as_ref().map(|reporting| Reporting {
                socket: reporting.socket,
                token: reporting.token,
            }),
        },
    ))
}

/// **A start's `launch`, as the session protocol carries it, started by its words** (#2174):
/// [`launched`] of the launch's parts, numbered `number` and started at `size`, with `app` the
/// starter's own environment. The one adapter from the protocol's launch to the core's
/// composition, which every binary that hosts uses.
pub fn from_launch(
    app: impl IntoIterator<Item = (OsString, OsString)>,
    launch: &purlis_session_protocol::session::Launch,
    number: u32,
    size: Size,
) -> Result<Spec, String> {
    launched(
        app,
        &Launched {
            program: &launch.program,
            args: &launch.args,
            cwd: launch.cwd.as_deref(),
            set: &launch.set,
            strip: &launch.strip,
            pass: &launch.pass,
            declared: &launch.declared,
            harness: launch.harness.as_deref(),
            git_hooks: launch.git_hooks.as_deref(),
            reporting: launch.reporting.as_ref().map(|reporting| Reporting {
                socket: Path::new(&reporting.socket),
                token: &reporting.token,
            }),
        },
        number,
        size,
    )
}

/// [`from_launch`] against the environment this process was started with: what the host the
/// window started, or `purlis serve`, starts a chat with, as the window would have (#1998).
pub fn from_launch_here(
    launch: &purlis_session_protocol::session::Launch,
    number: u32,
    size: Size,
) -> Result<Spec, String> {
    from_launch(std::env::vars_os(), launch, number, size)
}

#[cfg(test)]
mod tests;

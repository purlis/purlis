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
    /// The chat's prompt key, which its hooks key the digests of its prompts by (#3369): set in
    /// its environment alone, never on its line.
    pub prompt_key: Option<&'a str>,
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
    if let Some(key) = chat.prompt_key {
        spec.env
            .push((crate::hookwire::PROMPT_KEY_ENV.into(), key.into()));
    }
    // What the starter added, under both names too (V93k), as `compose` gave the rest.
    spec.env = crate::envvar::twinned(std::mem::take(&mut spec.env));
    // The folder of links to the plane's skills the chat is handed: noted while its program
    // runs, so it is never tidied away from under it (SK-24). Its program never sees the name.
    spec.holds.extend(crate::skills::take_hold(&mut spec.env));
    // **purlis's own wrap on Linux starts with nothing of the chat's** (#2460): the chat's
    // environment is carried for its program, after the terminal's type its terminal would
    // have given that program, and the wrap's own environment is only what carries it.
    if wraps(chat.args) {
        let mut env = vec![(OsString::from("TERM"), OsString::from(crate::session::TERM))];
        env.append(&mut spec.env);
        spec.env = purlis_linux_sandbox::carried::carried(env);
    }
    spec
}

/// **`spec`, with what the chat's profile `handed` it set last** in its program's environment
/// ([`crate::profilesignin::handed_onto`]), carried with the rest where the chat runs on
/// purlis's own wrap. Applied where the program is spawned, after everything else is settled.
pub fn with_handed(mut spec: Spec, handed: &crate::profilesignin::Handed) -> Spec {
    if handed.is_empty() {
        return spec;
    }
    let env = std::mem::take(&mut spec.env);
    // Decided as [`spec`] decided it, by the chat's line. A wrap whose environment could not be
    // read back is handed its variables beside it, and refuses to start (fail closed).
    let words: Option<Vec<String>> = spec
        .args
        .iter()
        .map(|word| word.to_str().map(str::to_owned))
        .collect();
    let carried = words
        .filter(|words| wraps(words))
        .and_then(|_| purlis_linux_sandbox::carried::read(env.iter().cloned()).ok());
    spec.env = match carried {
        Some(carried) => purlis_linux_sandbox::carried::carried(crate::profilesignin::handed_onto(
            carried, handed,
        )),
        None => crate::profilesignin::handed_onto(env, handed),
    };
    spec
}

/// Whether a chat's arguments, after its program, are purlis's own wrap's line
/// ([`purlis_linux_sandbox::line`]).
fn wraps(args: &[String]) -> bool {
    args.split_first().is_some_and(|(command, rest)| {
        command == purlis_linux_sandbox::COMMAND && purlis_linux_sandbox::read_line(rest).is_some()
    })
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
    /// The chat's prompt key (#3369).
    pub prompt_key: Option<&'a str>,
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
            prompt_key: launch.prompt_key,
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
            prompt_key: launch
                .prompt_key
                .as_ref()
                .map(purlis_session_protocol::session::PromptKey::expose),
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A brokered `secret exec` (#1407, ADR 0067 §5 class 1): **a sandboxed chat never reads a
//! vault; the app resolves the secret and hands it to the command it runs.**
//!
//! Inside a sandboxed chat the app started, `purlis secret exec <vault> … -- <cmd>` reads
//! nothing. It sends one line over the chat's hook socket ([`Ask`], carrying the chat's token)
//! and prints what comes back ([`forwarded`]). The app then ([`serve`]):
//!
//! 1. **authorises** the vault against the persona the app recorded for the chat, never one the
//!    request names: a vault the registry tags with that persona, or one the person allowed for
//!    it on this machine ([`authorise`], D-1430-1). A chat on no persona gets no vault;
//! 2. **resolves** each value with the providers' own code, in the app's process and
//!    environment, never the chat's, so nothing the chat sets moves where a value is read from,
//!    and a provider's program is never one in the chat's folder or anywhere its sandbox lets
//!    it write ([`Ctx::chat`]);
//! 3. **runs** the command outside the chat's read rules but inside a sandbox built from what
//!    the chat's own sandbox was compiled to when it started ([`crate::sandbox::Confines`]): the
//!    same denials, its harness's own among them, the chat's folder and a temp directory of its
//!    own as the only places it writes, and the network only through an egress proxy carrying
//!    the chat's hosts. A `--file`/`--dotenv` credential is a 0600 file in the vaults directory,
//!    which every chat is denied, and the child's profile gives back a read of that one file
//!    alone. It is removed when the child ends. The child leads a process group of its own:
//!    everything it started that stayed in that group is killed when it ends, when the asker
//!    goes, and when the app lets go. A process that leaves the group (`setsid`, a daemon) is
//!    not;
//! 4. **streams** stdout and stderr back as they come, redacted across chunk boundaries
//!    ([`Redactor`]), then the exit status. A child's stdin is the asker's when that is not a
//!    terminal;
//! 5. **tells the asking chat's tab** each host the run's proxy refused because the chat's
//!    hosts do not list it, as the proxy refused it (`host:port`), as it happens ([`Told`]):
//!    the same sandbox block a chat's own command raises, so its Notice offers Allow. A client
//!    reports the proxy's `403` in its own words (kubectl's "Forbidden"), which name no host,
//!    so the chat could not raise it itself. The host comes from the proxy's record
//!    ([`crate::sandbox::egress::Refusals`]), never from what the command printed. Before the
//!    exit status, purlis says the same in its own words on stderr ([`refused_note`]), so a chat
//!    that sees only "Forbidden" can tell the person where to allow it, masked as every frame
//!    is. A refused host that carries one of the run's values is never named, told or offered:
//!    purlis says only that one was withheld ([`WITHHELD_NOTE`]). A grant reaches the next run
//!    once the chat is started again with it, which its Allow owes it.
//!
//! Every run is recorded as `secret-exec` with `brokered`, the chat and its persona; every
//! refusal is logged with the chat's number and the vault's name, never a value.
//!
//! Where no app takes the line (no socket, an app older than the line, or one with nothing that
//! answers it: [`crate::hookwire::NOTHING_ANSWERS`]) the command runs in this process as it
//! always has, **except for a vault whose values come from the keyring**, which is refused
//! instead ([`refused_in_this_chat`], #1638): read here, the Keychain would ask the person for
//! the chat. `secret get` and `cp` are refused it the same way, since the app runs only `exec`.
//!
//! **Only a declared tool** (D-PE-ruling-2, the operator's ruling of 2026-10-12). Between the
//! authorisation and the run, the command must be a tool the chat's persona declares
//! (`[sandbox.personas.<p>] tools`), armed as the chat started (allowed on this machine with the
//! persona's hosts, D-PE-ruling-1, its program found where no chat writes), asked for with
//! exactly that tool's vault and hand-over ([`crate::sandbox::tools::declared`]), or a program the
//! person allowed under a Run grant ([`run_granted`], ADR 0112). Anything else
//! is refused with a sentence naming the tool to declare. The run is of the tool's program by
//! its absolute path, from a `PATH` without the folders the chat may write, and without the
//! chat's variables that make a program load code ([`crate::sandbox::tools::loads_code`]): it
//! starts from only the few of the chat's variables a run needs, with a home of its own in the
//! run's temp directory ([`crate::sandbox::tools::a_run_starts_from`]), and keeps no core dump
//! (#2047). On
//! Linux, purlis's own wrap that holds it starts with none of these: the run's environment is
//! carried past it to the tool alone ([`purlis_linux_sandbox::carried`]). A
//! credentialed MCP server's line passes only as this machine approved it, wrapped by purlis
//! from the approved entry. The person's own terminal and a chat the app did not start
//! sandboxed never come here.
//!
//! **What this guarantees, and what it does not.** A chat no longer runs an arbitrary program
//! with its persona's vault: a shell or an interpreter is never a tool. A tool's arguments are
//! still the chat's to choose, so a tool that can be told to run something else, or to send its
//! credential somewhere, can hand over the value of its own key; masking catches only a value's
//! own literal bytes. Brokering keeps out of the chat what a chat could otherwise reach: the
//! vault's storage and its provider's session (the op config, the keychain, the plain file),
//! every vault the chat's persona may not use (D-1430-1), every key no tool of its persona is
//! handed, and the credential file itself, which only the child may read.

use std::ffi::OsString;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine as _;
use serde_json::Value;

use super::cmd::{self, Io, Say};
use super::exec::{self, Request, Stopped};
use super::{Ctx, Env, registry};
use crate::sandbox::{Confines, Denial};
use crate::tui::Markup;

/// The line a chat writes: which chat (checked against its token), and what it wants run.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Ask {
    /// The app's number for the chat, from [`crate::hookwire::CHAT_ENV`].
    pub chat: u32,
    /// What is to be run. The field's name is what tells this line from every other kind.
    pub secret_exec: Wanted,
    /// **The declared tool the chat's own host runs it as** (#2047): set by a session host that
    /// composed the chat in a sandbox and found the command to be one of the tools its start
    /// armed there, so the window may send that host the values for that one run. Set by the
    /// host alone: a hook line's own is cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs_as: Option<String>,
    /// **It waited for the window** (#2047): the host held it while its desktop was away and
    /// hands it on now, so the window asks the person about it afresh. Set by the host alone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub waited: bool,
    /// **This run's own number** (#2047, [`a_run_number`]): made by the host that composed the
    /// chat, for each run it hands on, and echoed in the values the window sends for it
    /// ([`Handed`]), which the host runs only on a match. Set by the host alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// What a chat asks to have run: `secret exec`'s own arguments, where it runs, and the
/// environment the child starts from. **No persona, no place and no policy**: those are the
/// app's record of the chat whose token the line carries.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Wanted {
    pub vault: String,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub file: Vec<String>,
    #[serde(default)]
    pub dotenv: Vec<String>,
    /// The command, its program first.
    pub command: Vec<String>,
    /// The directory the asker runs in, which the child starts in when it is a directory.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// The asker's environment, which the child's starts from, as the operating system gives
    /// it: a variable that is not UTF-8 is handed on too ([`asked_environment`]).
    #[serde(flatten, with = "asked_environment")]
    pub environment: Vec<(OsString, OsString)>,
    /// Whether the asker sends its stdin ([`Input`]). The child's is empty otherwise.
    #[serde(default)]
    pub stdin: bool,
    /// Where this run is a persona's MCP server's start, the server and whether this is its
    /// last try ([`super::mcpstart`]): what the app tells the person of when it fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<super::mcpstart::McpStart>,
}

/// **How [`Wanted::environment`] is written on the line** (#1407 review). Each variable whose
/// name and value are UTF-8 is a pair of strings under `environment`, the shape an app of an
/// earlier build reads; each other one is a pair of base64 strings of its bytes under
/// `environment_bytes`, which such an app skips, as it skipped the variable before. Off unix
/// no bytes are read from an `OsString`, so such a variable is not handed on there.
mod asked_environment {
    use std::ffi::OsString;

    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Default, Serialize, Deserialize)]
    struct Line {
        #[serde(default)]
        environment: Vec<(String, String)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        environment_bytes: Vec<(String, String)>,
    }

    #[cfg(unix)]
    fn bytes_of(text: &OsString) -> Option<String> {
        use std::os::unix::ffi::OsStrExt;
        Some(base64::engine::general_purpose::STANDARD.encode(text.as_bytes()))
    }

    #[cfg(not(unix))]
    fn bytes_of(_text: &OsString) -> Option<String> {
        None
    }

    #[cfg(unix)]
    fn from_bytes(text: &str) -> Option<OsString> {
        use std::os::unix::ffi::OsStringExt;
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .ok()
            .map(OsString::from_vec)
    }

    #[cfg(not(unix))]
    fn from_bytes(_text: &str) -> Option<OsString> {
        None
    }

    pub fn serialize<S: Serializer>(
        vars: &[(OsString, OsString)],
        to: S,
    ) -> Result<S::Ok, S::Error> {
        let mut line = Line::default();
        for (name, value) in vars {
            match (name.to_str(), value.to_str()) {
                (Some(name), Some(value)) => {
                    line.environment.push((name.to_owned(), value.to_owned()));
                }
                _ => {
                    if let (Some(name), Some(value)) = (bytes_of(name), bytes_of(value)) {
                        line.environment_bytes.push((name, value));
                    }
                }
            }
        }
        line.serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        from: D,
    ) -> Result<Vec<(OsString, OsString)>, D::Error> {
        let line = Line::deserialize(from)?;
        let mut vars: Vec<(OsString, OsString)> = line
            .environment
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        // A pair that is not base64 is not the asker's, and is not handed on.
        vars.extend(
            line.environment_bytes
                .iter()
                .filter_map(|(name, value)| Some((from_bytes(name)?, from_bytes(value)?))),
        );
        Ok(vars)
    }
}

/// What the app writes back, one JSON line each.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frame {
    /// The child is running: from now on the asker's stdin is wanted, when it sends one.
    Started,
    /// A line in purlis's voice, for stderr.
    Note(String),
    /// Redacted bytes of the child's stdout, base64.
    Stdout(String),
    /// Redacted bytes of the child's stderr, base64.
    Stderr(String),
    /// The child ended with this status. The last frame.
    Exit(i32),
    /// Nothing was run, with the status and the sentence. The last frame.
    Refused { why: String, code: i32 },
}

/// What the asker writes after its ask, once the child has [`Frame::Started`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    /// Bytes of the asker's stdin, base64.
    Stdin(String),
    /// The asker's stdin has ended.
    StdinClosed,
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(text: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .unwrap_or_default()
}

/// The most one frame may be, either way: a chunk of output or of stdin, base64, with room.
const A_FRAME_IS_AT_MOST: u64 = 256 * 1024;

/// The most bytes of output one frame carries: base64 makes them a third longer, and the rest
/// of the frame is a few bytes, so a frame of this many is well under [`A_FRAME_IS_AT_MOST`].
/// A frame past the cap is never read whole: its reader takes the cap and stops there.
const A_FRAMES_BYTES_ARE_AT_MOST: usize = 128 * 1024;

const _: () =
    assert!(A_FRAMES_BYTES_ARE_AT_MOST.div_ceil(3) * 4 + 64 < A_FRAME_IS_AT_MOST as usize);

/// The longest value of the asker's environment handed on to the child.
const A_VARIABLE_IS_AT_MOST: usize = 4 * 1024;

/// How long a child's pipes are read for after it has ended.
const PIPES_AFTER_EXIT: Duration = Duration::from_secs(2);

/// How much of a pipe is read at a time.
const A_CHUNK: usize = 16 * 1024;

/// How long `secret exec` waits on the app: it says what the app waits on after a few seconds
/// without a frame, and stops past the longest wait the app itself holds (an ask in the window
/// and a GitHub App's token mint are each held for 2 minutes), counted again from each frame.
#[cfg(unix)]
const OF_THE_APP: Patience = Patience {
    slow_after: Duration::from_secs(3),
    deadline: Duration::from_secs(180),
};

// ----------------------------------------------------------------------------------------
// the asker: `purlis secret exec` inside a chat

/// The app's number for the chat this runs in, as `purlis session record` reads it: the hook
/// variable, else the session id the app sets to the same number.
fn chat_number(env: &dyn Fn(&str) -> Option<String>) -> Option<u32> {
    [crate::hookwire::CHAT_ENV, crate::active::SESSION_ID_ENV]
        .into_iter()
        .filter_map(env)
        .find_map(|n| n.trim().parse::<u32>().ok().filter(|n| *n > 0))
}

/// What would read a vault in a chat, for [`refused_in_this_chat`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// `secret exec`, which the app did not take.
    Exec,
    /// `secret get` or `cp`, which the app does not run for a chat.
    GetOrCp,
    /// Any other read of the keyring (`vault verify`, `secret rename`, a listing that runs the
    /// provider): the backstop where the keyring itself is read.
    Other,
}

/// What `reader` on `vault` is refused with in this process, or `None` where it may read the
/// vault itself (#1638): **a chat the app started sandboxed never reads the keyring through the
/// command.**
///
/// Only for a vault whose values come from the keyring ([`super::keyring::holds_values_of`]).
/// Read here, in the chat, each such read makes the Keychain ask the person (V90a), and an
/// "Always Allow" given to the command would let the chat read it silently from then on; the
/// Secret Service on Linux answers without asking at all. So the only way a sandboxed chat
/// reaches one is `secret exec` through the app: where the app does not take it, or for
/// anything else, nothing is read. Any other vault is left to its own provider and to the
/// chat's sandbox, as before. Resolving every chat's reads in the app is the rest of V16b
/// (#1180).
///
/// **Where the mark is gone, where the process runs decides** ([`super::setup::sandboxed_chat`]):
/// below a chat's program whose record does not say it ran without the sandbox, it is a
/// sandboxed chat all the same, and where this project's record or its ancestry cannot be read,
/// it is refused with why. The person's terminal, an unsandboxed chat or shell tab, and the app
/// read as before.
///
/// **A courtesy, not the boundary.** The mark and the records are what purlis can see, and a
/// chat's process can leave its chat's ancestry; what holds the item is the store's own rule
/// (V90a) and the sandbox. This keeps purlis itself from raising a question on the chat's
/// behalf, and says what to do.
pub fn refused_in_this_chat(ctx: &Ctx, vault: &registry::Vault, reader: Reader) -> Option<Markup> {
    refused_where(ctx, vault, reader, &|| super::setup::sandboxed_chat(ctx))
}

/// [`refused_in_this_chat`], with where this process runs asked of `located` only when the
/// environment carries no mark and the vault is the keyring's.
pub(crate) fn refused_where(
    ctx: &Ctx,
    vault: &registry::Vault,
    reader: Reader,
    located: &dyn Fn() -> super::setup::Sandboxed,
) -> Option<Markup> {
    let env = |name: &str| ctx.env.get(name);
    if !super::keyring::holds_values_of(ctx, vault) {
        return None;
    }
    if !crate::sandbox::chat_is_sandboxed_in(&env) {
        match located() {
            super::setup::Sandboxed::No => return None,
            super::setup::Sandboxed::Yes => {}
            super::setup::Sandboxed::Unsure(doubt) => {
                return Some(Markup::lines([
                    Markup::from(format!(
                        "vault '{}' is read from {}, and purlis cannot tell whether this runs \
                         inside a sandboxed chat: {}. purlis does not read {} where it cannot \
                         tell.",
                        vault.name,
                        super::keyring::STORE_NAME,
                        crate::personas::one_line(&doubt),
                        super::keyring::STORE_NAME,
                    )),
                    Markup::from(match reader {
                        Reader::Exec => "  Run this again once that is cleared.",
                        Reader::GetOrCp | Reader::Other => {
                            "  In a chat, hand the value to a command with `purlis secret exec`, \
                             which the app runs. Anywhere else, run this again once that is \
                             cleared."
                        }
                    }),
                ]));
            }
        }
    }
    let (why, next) = match reader {
        // The app runs no command for a chat on this system yet ([`NO_WRAP`]).
        _ if !runs_a_chats_command_here() => (
            "purlis cannot yet run a command in a sandbox on this system, so the app runs none \
             for a chat here",
            "Run it in a terminal outside the chat.".to_owned(),
        ),
        Reader::Exec => (
            "the app that started this chat did not take this `secret exec`, so nothing was run",
            "Run it again while the app that started this chat is open.".to_owned(),
        ),
        Reader::GetOrCp | Reader::Other => (
            "the app runs only `secret exec` for a chat",
            format!(
                "Hand the value to a command with `purlis secret exec {} --env NAME=key -- \
                 <command>`: the app runs that for the chat.",
                vault.name
            ),
        ),
    };
    Some(Markup::lines([
        Markup::from(format!(
            "vault '{}' is read from {}, which a sandboxed chat reaches only through the app, \
             and {why}. purlis does not read {} from inside a sandboxed chat.",
            vault.name,
            super::keyring::STORE_NAME,
            super::keyring::STORE_NAME,
        )),
        Markup::from(format!("  {next}")),
    ]))
}

/// Runs `req` through the app that started this chat, and answers its status, or `None` when
/// no app takes it and it is to run here.
///
/// Only inside a chat the app started sandboxed ([`crate::sandbox::chat_is_sandboxed_in`]): an
/// unsandboxed chat reads its vault as the person does. A chat that claims to be sandboxed when
/// it is not gets the stricter run. One that hides the mark but still names the app's socket
/// and its number is sent there all the same where it runs below a sandboxed chat's program, or
/// where purlis cannot tell ([`super::setup::sandboxed_chat`]), since the app judges by its own
/// record; one that hides all of it gets the vault's own refusal ([`refused_in_this_chat`]).
///
/// A persona's MCP server's start ([`super::mcpstart::server_in`]) is tried once more where the
/// first run failed before the server spoke ([`started_server`]).
#[cfg(unix)]
pub fn forwarded(ctx: &Ctx, req: &Request, command: &[String], io: &mut dyn Io) -> Option<i32> {
    forwarded_where(ctx, req, command, io, &|| super::setup::sandboxed_chat(ctx))
}

/// [`forwarded`], with where this process runs asked of `located` only when the environment
/// names the app's socket and a chat but carries no mark.
#[cfg(unix)]
pub(crate) fn forwarded_where(
    ctx: &Ctx,
    req: &Request,
    command: &[String],
    io: &mut dyn Io,
    located: &dyn Fn() -> super::setup::Sandboxed,
) -> Option<i32> {
    let env = |name: &str| ctx.env.get(name);
    let socket = env(crate::hookwire::SOCKET_ENV).filter(|s| !s.is_empty())?;
    let chat = chat_number(&env)?;
    if !crate::sandbox::chat_is_sandboxed_in(&env) && located() == super::setup::Sandboxed::No {
        return None;
    }
    let token = crate::hookwire::ChatToken::read(&env);
    let connect = || std::os::unix::net::UnixStream::connect(&socket).ok();
    if let Some(server) = super::mcpstart::server_in(&env) {
        return started_server(ctx, req, command, chat, token, &server, &connect, io);
    }
    converse(ctx, req, command, chat, token, connect()?, io)
}

/// How long the asker waits on the app before it says what the app waits on, and before it stops.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Patience {
    pub slow_after: Duration,
    pub deadline: Duration,
}

/// [`forwarded`]'s exchange with the app, on a connection already made.
#[cfg(unix)]
fn converse(
    ctx: &Ctx,
    req: &Request,
    command: &[String],
    chat: u32,
    token: Option<crate::hookwire::ChatToken>,
    stream: std::os::unix::net::UnixStream,
    io: &mut dyn Io,
) -> Option<i32> {
    converse_within(ctx, req, command, chat, token, stream, io, OF_THE_APP)
}

/// [`converse`], waiting on the app with `patience`.
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn converse_within(
    ctx: &Ctx,
    req: &Request,
    command: &[String],
    chat: u32,
    token: Option<crate::hookwire::ChatToken>,
    stream: std::os::unix::net::UnixStream,
    io: &mut dyn Io,
    patience: Patience,
) -> Option<i32> {
    let feed = super::mcpstart::Feed::new(stdin_of(io), false);
    match exchange(
        ctx,
        req,
        command,
        chat,
        token.as_ref(),
        stream,
        io,
        &feed,
        None,
        patience,
    ) {
        Outcome::NotTaken => None,
        Outcome::Ended { code, .. } => Some(code),
    }
}

/// **A persona's MCP server's start, tried once more where the first failed** (qw381): a run
/// refused, not taken, or ended before the server wrote a byte on stdout is asked for again
/// after [`super::mcpstart::RETRY_AFTER`], handed the stdin the first was. The second ask says
/// it is the last, so the app tells the person when it fails too. `connect` makes each
/// connection to the app.
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn started_server(
    ctx: &Ctx,
    req: &Request,
    command: &[String],
    chat: u32,
    token: Option<crate::hookwire::ChatToken>,
    server: &str,
    connect: &dyn Fn() -> Option<std::os::unix::net::UnixStream>,
    io: &mut dyn Io,
) -> Option<i32> {
    use super::mcpstart::{AGAIN, McpStart, RETRY_AFTER};
    let feed = super::mcpstart::Feed::new(stdin_of(io), true);
    // Once a run has started, the stdin is the feed's: nothing may then run here instead.
    let given_over = |io: &mut dyn Io| {
        io.say(Say::Err(GIVEN_OVER.into()));
        Some(1)
    };
    for last in [false, true] {
        let start = McpStart {
            server: server.to_owned(),
            last,
        };
        let outcome = match connect() {
            Some(stream) => exchange(
                ctx,
                req,
                command,
                chat,
                token.as_ref(),
                stream,
                io,
                &feed,
                Some(start),
                OF_THE_APP,
            ),
            None => Outcome::NotTaken,
        };
        match outcome {
            // Not taken: no app runs it, now or on a second ask. It runs here, unless a run
            // already took the stdin.
            Outcome::NotTaken if feed.started() => return given_over(io),
            Outcome::NotTaken => return None,
            // Tried again only where asking again can change it: the server ended before it
            // spoke, or the chat was not open yet. A refusal of the vault or of the command is
            // the same every time.
            Outcome::Ended {
                code,
                spoke,
                refused,
            } if spoke || last || !feed.replayable() || (refused && code != NOT_YET) => {
                return Some(code);
            }
            Outcome::Ended { .. } => {}
        }
        io.say(Say::Info(AGAIN.into()));
        std::thread::sleep(RETRY_AFTER);
    }
    None
}

/// What a server's start says where its second ask found no app to run it after its first run
/// had started: the stdin was handed to that run, so nothing runs here.
pub const GIVEN_OVER: &str = "the app that started this chat stopped answering after the MCP \
                              server's first run, so it was not started again.";

/// The asker's stdin, where it is not a terminal and the caller can hand it on as a stream.
#[cfg(unix)]
fn stdin_of(io: &mut dyn Io) -> Option<Box<dyn Read + Send>> {
    if io.stdin_is_terminal() {
        None
    } else {
        io.stdin_stream()
    }
}

/// What one exchange with the app came to.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// No app took it: nothing ran.
    NotTaken,
    /// It ended with `code`; `spoke` where the child wrote on stdout, `refused` where nothing
    /// ran.
    Ended {
        code: i32,
        spoke: bool,
        refused: bool,
    },
}

/// One ask on `stream`, and what comes back printed through `io`. The child is given `feed`
/// once it has started.
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn exchange(
    ctx: &Ctx,
    req: &Request,
    command: &[String],
    chat: u32,
    token: Option<&crate::hookwire::ChatToken>,
    stream: std::os::unix::net::UnixStream,
    io: &mut dyn Io,
    feed: &Arc<super::mcpstart::Feed>,
    mcp: Option<super::mcpstart::McpStart>,
    patience: Patience,
) -> Outcome {
    let ask = Ask {
        chat,
        secret_exec: Wanted {
            vault: req.vault.clone(),
            env: req.env.clone(),
            file: req.file.clone(),
            dotenv: req.dotenv.clone(),
            command: command.to_vec(),
            cwd: std::env::current_dir().ok(),
            environment: ctx
                .env
                .vars()
                .iter()
                // The line has a cap (`hookwire`'s), and an environment is a few kilobytes: a
                // variable past this is not handed on, so the line is never cut.
                .filter(|(_, v)| v.len() <= A_VARIABLE_IS_AT_MOST)
                .cloned()
                .collect(),
            stdin: feed.has_stdin(),
            mcp,
        },
        ..Ask::default()
    };
    let Ok(line) = crate::hookwire::line_with(token, &ask) else {
        return Outcome::NotTaken;
    };
    if (&stream).write_all(&line).is_err() {
        return Outcome::NotTaken;
    }
    let mut reader = std::io::BufReader::new(&stream);
    let mut started = false;
    let mut spoke = false;
    let mut waiting = Waiting::new(patience);
    loop {
        let mut said = String::new();
        let read = loop {
            // Until the child has started, the app is waited on with a deadline; after, the
            // command runs as long as it runs.
            let _ = stream.set_read_timeout((!started).then(|| waiting.next_timeout()));
            match (&mut reader).take(A_FRAME_IS_AT_MOST).read_line(&mut said) {
                Err(e) if !started && waited_out(&e) => match waiting.timed_out() {
                    Waited::Slow => io.say(Say::Info(
                        waiting_on(&waits_on(ctx, &req.vault), command, patience).into(),
                    )),
                    Waited::Again => {}
                    Waited::Out(last) => {
                        io.say(Say::Err(stopped_waiting(
                            last.as_deref(),
                            &waits_on(ctx, &req.vault),
                            patience,
                        )));
                        return Outcome::Ended {
                            code: 1,
                            spoke,
                            refused: true,
                        };
                    }
                },
                read => break read,
            }
        };
        let frame = match read {
            Ok(n) if n > 0 => serde_json::from_str::<Frame>(&said).ok(),
            _ => None,
        };
        let Some(frame) = frame else {
            // An app older than this line drops it unread: nothing ran, so it runs here.
            if !started {
                return Outcome::NotTaken;
            }
            io.say(Say::Err(
                "the app stopped answering while the command ran, so how it ended is not known."
                    .into(),
            ));
            return Outcome::Ended {
                code: 1,
                spoke,
                refused: false,
            };
        };
        if !started {
            waiting.framed(&frame);
        }
        match frame {
            Frame::Started => {
                started = true;
                if let Ok(writer) = stream.try_clone() {
                    feed.attach(Box::new(writer));
                }
            }
            Frame::Note(note) => io.say(Say::Info(note.into())),
            Frame::Stdout(bytes) => {
                if !spoke {
                    spoke = true;
                    feed.spoke();
                }
                io.out(&unb64(&bytes));
            }
            Frame::Stderr(bytes) => io.err(&unb64(&bytes)),
            Frame::Exit(code) => {
                return Outcome::Ended {
                    code,
                    spoke,
                    refused: false,
                };
            }
            Frame::Refused { why, .. } if why == crate::hookwire::NOTHING_ANSWERS && !started => {
                return Outcome::NotTaken;
            }
            Frame::Refused { why, code } => {
                io.say(Say::Err(why.into()));
                return Outcome::Ended {
                    code,
                    spoke,
                    refused: true,
                };
            }
        }
    }
}

/// **The asker's wait on the app, before the child starts** (R-356, Q232): never silent, never
/// for ever. With no frame for [`Patience::slow_after`] it says once what the app waits on; with
/// none for [`Patience::deadline`], counted again from each frame, it stops.
#[cfg(unix)]
struct Waiting {
    patience: Patience,
    since: std::time::Instant,
    framed: bool,
    slow_said: bool,
    /// The last note the app sent, which says what it waits on from then.
    last: Option<String>,
}

/// What a read that timed out comes to.
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
enum Waited {
    /// The app is slow: say what it waits on, and wait on.
    Slow,
    /// Wait on.
    Again,
    /// Past the deadline, with the last note the app sent.
    Out(Option<String>),
}

#[cfg(unix)]
impl Waiting {
    fn new(patience: Patience) -> Self {
        Self {
            patience,
            since: std::time::Instant::now(),
            framed: false,
            slow_said: false,
            last: None,
        }
    }

    /// How long the next read may wait. Never zero, which a socket refuses.
    fn next_timeout(&self) -> Duration {
        let until = if self.framed || self.slow_said {
            self.patience.deadline
        } else {
            self.patience.slow_after
        };
        until
            .saturating_sub(self.since.elapsed())
            .max(Duration::from_millis(1))
    }

    fn timed_out(&mut self) -> Waited {
        let waited = self.since.elapsed();
        if waited >= self.patience.deadline {
            Waited::Out(self.last.clone())
        } else if !self.framed && !self.slow_said && waited >= self.patience.slow_after {
            self.slow_said = true;
            Waited::Slow
        } else {
            Waited::Again
        }
    }

    fn framed(&mut self, frame: &Frame) {
        self.since = std::time::Instant::now();
        self.framed = true;
        if let Frame::Note(note) = frame {
            self.last = Some(note.clone());
        }
    }
}

/// Whether `e` is a read that ran out of time rather than a connection that failed.
#[cfg(unix)]
fn waited_out(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

/// What the app does for a `secret exec` on a vault before the child starts, as far as the
/// asker can tell from the project: the slow part of it.
#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// A GitHub App's token is minted for a tool the persona declares on the vault.
    TokenMint(String),
    /// The vault is read from the system keyring.
    Keyring(String),
    /// The vault is read from 1Password, over the network.
    Network(String),
    /// The vault is read through its provider.
    Provider { vault: String, provider: String },
    /// The project does not say.
    Unknown(String),
}

/// [`Step`] for `vault`, read from the project's registry and the persona's declared tools.
#[cfg(unix)]
fn waits_on(ctx: &Ctx, vault: &str) -> Step {
    let named = vault.to_owned();
    let mints = ctx
        .env
        .get(crate::active::PERSONA_ENV)
        .is_some_and(|persona| {
            declared_now(&ctx.root, &persona)
                .iter()
                .any(|tool| tool.vault == vault && tool.mint.is_some())
        });
    if mints {
        return Step::TokenMint(named);
    }
    match registry::vault(ctx, vault) {
        Ok(v) if super::keyring::holds_values_of(ctx, &v) => Step::Keyring(named),
        Ok(v) if v.provider == "1password" => Step::Network(named),
        Ok(v) => Step::Provider {
            vault: named,
            provider: v.provider,
        },
        Err(_) => Step::Unknown(named),
    }
}

/// `step` as what the app does: "reads vault 'x' from the system keyring".
#[cfg(unix)]
fn doing(step: &Step) -> String {
    match step {
        Step::TokenMint(vault) => {
            format!("mints a GitHub App's token for vault '{vault}', over the network")
        }
        Step::Keyring(vault) => {
            format!("reads vault '{vault}' from {}", super::keyring::STORE_NAME)
        }
        Step::Network(vault) => format!("reads vault '{vault}' from 1Password, over the network"),
        Step::Provider { vault, provider } => {
            format!("reads vault '{vault}' through its provider '{provider}'")
        }
        Step::Unknown(vault) => format!(
            "reads vault '{vault}' (from {}, a GitHub App's token mint or the network)",
            super::keyring::STORE_NAME
        ),
    }
}

/// A wait in words: "3 minutes", "90 seconds".
#[cfg(unix)]
fn spoken(wait: Duration) -> String {
    let secs = wait.as_secs();
    match secs {
        0 => format!("{} ms", wait.as_millis()),
        60 => "1 minute".to_owned(),
        s if s > 60 && s % 60 == 0 => format!("{} minutes", s / 60),
        1 => "1 second".to_owned(),
        s => format!("{s} seconds"),
    }
}

/// The line said once the app is slow: what it waits on, and when this stops waiting.
#[cfg(unix)]
fn waiting_on(step: &Step, command: &[String], patience: Patience) -> String {
    format!(
        "Waiting on the app that started this chat, which {} before it runs `{}`. This stops \
         waiting after {}.",
        doing(step),
        command_shown(command),
        spoken(patience.deadline),
    )
}

/// The error said at the deadline: what the app was waiting on, by its last note or by `step`.
#[cfg(unix)]
fn stopped_waiting(last: Option<&str>, step: &Step, patience: Patience) -> Markup {
    let waited = spoken(patience.deadline);
    let what = match last {
        Some(note) => format!(
            "the app that started this chat said nothing more for {waited} after: {}",
            crate::personas::one_line(note)
        ),
        None => format!(
            "the app that started this chat did not answer for {waited} while it {}",
            doing(step)
        ),
    };
    Markup::lines([
        Markup::from(format!(
            "{what}. purlis stopped waiting before the command started, so nothing ran."
        )),
        Markup::from(
            "  Look in the purlis window for what holds it, such as a keychain prompt or an ask, \
             then run this again.",
        ),
    ])
}

/// Where there is no unix socket there is no app to ask, and the command runs here.
#[cfg(not(unix))]
pub fn forwarded(_ctx: &Ctx, _req: &Request, _command: &[String], _io: &mut dyn Io) -> Option<i32> {
    None
}

// ----------------------------------------------------------------------------------------
// the app: authorise, resolve, run

/// **The longest command a person is asked to allow**, as it is shown ([`command_shown`]): what
/// runs past this is not offered, so no host is pushed out of what they read.
pub const MOST_COMMAND_SHOWN: usize = 2000;

/// **A chat's command as the person reads it** (ADR 0112): each word a shell word, every byte
/// outside printable ASCII spelled out ([`crate::shown::ascii_word`]), so no control character,
/// direction override or lookalike letter changes what it seems to say. Inert, and told apart
/// from the app's facts wherever it is drawn.
pub fn command_shown(words: &[String]) -> String {
    words
        .iter()
        .map(|word| crate::shown::ascii_word(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The plain name of the program `word` names.
fn command_name(word: &str) -> String {
    Path::new(word).file_name().map_or_else(
        || word.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// **Why policy allows no Run grant for `wanted`'s command**, where it does not (#5243): its lock
/// on them, or on a persona's tools, or, strictest wins, its lock on asks for undeclared keys
/// where the command names a key the persona does not declare.
fn run_grant_locked(
    asker: &Asker,
    wanted: &Wanted,
    command: &[String],
    locks: &crate::sandbox::policy::Locks,
) -> Option<String> {
    let persona = asker.persona.as_deref().unwrap_or_default();
    let named = command_name(&command[0]);
    locks.refuses_run_grant(persona, &named).or_else(|| {
        // Policy's `allow-scopes` may leave no scope a Run grant can be given at.
        use crate::sandbox::grant::Level;
        let reviewed = crate::sandbox::tools::reviewed_for_any_arguments(&named);
        if !locks.keeps_scope(Level::Once)
            && !(reviewed && (locks.keeps_scope(Level::Chat) || locks.keeps_scope(Level::Project)))
        {
            return Some(format!(
                "Policy removes every scope a program could be allowed at with a vault's keys, so \
                 {named} needs to be declared as a tool of {persona}'s. {}",
                locks.locked_by()
            ));
        }
        let undeclared = locks.forbids_undeclared_secret_asks()
            && keys_named(wanted)
                .any(|key| super::needs::declared(&asker.root, persona, key).is_none());
        undeclared.then(|| {
            format!(
                "Policy turns off asks for a secret {persona} does not declare, so {named} needs \
                 to be declared as a tool of {persona}'s. {}",
                locks.locked_by()
            )
        })
    })
}

/// Each key `wanted` hands over by `--env` or `--file`, as `VAR=key` spells it.
fn keys_named(wanted: &Wanted) -> impl Iterator<Item = &str> {
    wanted
        .env
        .iter()
        .chain(&wanted.file)
        .filter_map(|spec| spec.split_once('=').map(|(_, key)| key))
}

/// Each key `wanted` hands over: [`keys_named`]'s and each `--dotenv` entry's
/// (`ENVVAR=NAME:key`), for a judge that must see them all (#5795).
fn keys_handed(wanted: &Wanted) -> impl Iterator<Item = &str> {
    keys_named(wanted).chain(wanted.dotenv.iter().filter_map(|spec| {
        spec.split_once('=')
            .and_then(|(_, entry)| entry.split_once(':'))
            .map(|(_, key)| key)
    }))
}

/// **Each key `wanted` hands over that `vault` lacks** (#5242), once, in order: none under it, or
/// a link whose entry is gone, which the run's one notice asks a value for. Told by the
/// provider's kind of failure, never its sentence; a vault that can't be read lacks nothing here,
/// and its run says why as it reads.
fn fills_missing(asker: &Asker, ctx: &Ctx, wanted: &Wanted) -> Vec<super::ask::run::Fill> {
    let persona = asker.persona.as_deref().unwrap_or_default();
    let Ok(v) = cmd::provider(ctx, &wanted.vault) else {
        return Vec::new();
    };
    let mut fills: Vec<super::ask::run::Fill> = Vec::new();
    for key in keys_named(wanted) {
        if fills.iter().any(|fill| fill.key == key) {
            continue;
        }
        // A link into a vault the persona may not use is neither read nor asked for: its run is
        // refused (#5795).
        if read_out_of_reach(ctx, persona, &wanted.vault, &[key]).is_some() {
            continue;
        }
        let lacks = cmd::get_value(ctx, &v, key).map(zeroize::Zeroizing::new);
        if lacks
            .as_ref()
            .is_err_and(|e| matches!(e.kind, super::Kind::NotFound | super::Kind::BrokenLink))
        {
            fills.push(super::ask::run::Fill {
                key: key.to_owned(),
                declared: super::needs::declared(&ctx.root, persona, key).map(|need| need.purpose),
                reason: super::ask::Reason::broken(ctx, &wanted.vault, key),
            });
        }
    }
    fills
}

/// **Why a run's notice may not ask for one of `fills`**, where policy turns off asks for a key the
/// persona does not declare (#5078): said as a Secret ask's refusal is, and nothing is asked.
fn fills_refused(asker: &Asker, vault: &str, fills: &[super::ask::run::Fill]) -> Option<String> {
    fills.iter().find_map(|fill| {
        super::ask::refused_by_policy(&asker.root, vault, &fill.key, fill.declared.as_ref())
    })
}

/// **What a `secret_ask` naming a program may offer, ahead of its run** (#5244; ADR 0112, amended
/// 2026-11-06 by the person's ruling).
#[derive(Debug)]
pub enum Ahead {
    /// A tool the persona declares, or a chat purlis holds to no sandbox: its run needs no Allow,
    /// so the ask is a Fill, as before.
    Declared,
    /// Fill only, saying why no Allow is offered ahead: the program isn't reviewed for any
    /// arguments (its run asks on the exact command), runs what it is told, is locked by policy,
    /// or is no program a Run grant may be.
    FillOnly(String),
    /// The person did not allow it with that vault in this chat: refused at once, nothing asked.
    Refused { program: String },
    /// One notice: the key's field, and Allow for this chat bound to the chat, the program, the
    /// vault and exactly that key.
    Offer(Box<super::ask::run::RunNeed>),
}

/// **What `asker`'s `secret_ask` for `key` of `vault`, naming `program`, may offer** (#5244). The
/// program is resolved and judged as a run's is ([`run_granted`]): where no chat writes, never
/// one that runs what it is told, under policy, with no key a browser or a minter holds. Only a
/// program reviewed for any arguments gets an Allow ahead, and only Allow for this chat, since
/// there is no command yet to allow once. Every check a run makes is made again as it runs.
pub fn asked_ahead(
    asker: &Asker,
    asks: &super::ask::Asks,
    vault: &str,
    key: &str,
    program: &str,
    declared: Option<String>,
) -> Ahead {
    use crate::sandbox::tools;
    let Some(confines) = &asker.confines else {
        return Ahead::Declared;
    };
    // The chat's words, which the window would show in purlis's: only a plain program name or
    // path is named, and anything else is never repeated (#5244).
    let plain = |name: &str| {
        name.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    };
    let named = command_name(program);
    if program.len() > 256
        || !plain(&named)
        || !program
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | '/'))
    {
        return Ahead::FillOnly(
            "the program named isn't a plain program name, so no Allow is offered ahead: its run \
             asks for the Allow on the command it runs"
                .to_owned(),
        );
    }
    let persona = asker.persona.as_deref().unwrap_or_default();
    // Judged as a run handing that key by a variable named for it would be.
    let wanted = Wanted {
        vault: vault.to_owned(),
        env: vec![format!("{key}={key}")],
        command: vec![program.to_owned()],
        ..Wanted::default()
    };
    let (written_lines, _) = tools::declared_and_allowed_here(&asker.root, persona);
    if tools::is_declared(&confines.tools, &written_lines, program) {
        return Ahead::Declared;
    }
    let plane = crate::sandbox::Plane::read(&asker.root);
    if plane.unreadable() {
        return Ahead::FillOnly(format!(
            "purlis couldn't read the project's settings, so {named} gets no Allow ahead of its run"
        ));
    }
    let locks = crate::sandbox::policy::Locks::of(&asker.root);
    if let Some(locked) = run_grant_locked(asker, &wanted, &wanted.command, &locks) {
        return Ahead::FillOnly(locked);
    }
    let asked = tools::Asked {
        vault,
        env: &wanted.env,
        file: &wanted.file,
        dotenv: &wanted.dotenv,
        program,
    };
    let writes = chat_writes(asker, confines);
    let ready = match grantable(asker, persona, &asked, &written_lines, &locks, &writes) {
        Ok(ready) => ready,
        Err(why) => {
            return Ahead::FillOnly(format!("{named} gets no Allow: {why}"));
        }
    };
    let mut bound = bound_of(asker, &wanted, &wanted.command, &ready);
    bound.env = Vec::new();
    bound.cwd = PathBuf::new();
    if asks.refused_run(&bound) {
        return Ahead::Refused { program: named };
    }
    use crate::sandbox::grant::Level;
    if !bound.reviewed_for_any_arguments() || !locks.keeps_scope(Level::Chat) {
        return Ahead::FillOnly(format!(
            "purlis allows {named} one exact command at a time, so no Allow is offered ahead: \
             its run asks for the Allow on the command it runs"
        ));
    }
    // Not into a vault the persona may not use: that is asked about as the run asks.
    if authorise(&reading_for(asker), asker.persona.as_deref(), vault).is_err() {
        return Ahead::FillOnly(format!(
            "vault {vault} isn't one {persona} may use, so no Allow is offered ahead: its run asks"
        ));
    }
    let need = need_of(
        asker,
        confines,
        &wanted.command,
        &ready,
        bound,
        vec![super::ask::run::Fill {
            key: key.to_owned(),
            declared,
            reason: None,
        }],
        super::ask::run::Offer::Program,
        false,
    );
    Ahead::Offer(Box::new(super::ask::run::RunNeed {
        ahead: true,
        ..need
    }))
}

/// **What a run's one notice binds and shows** (#5242): `ready`'s program, as the app resolved it,
/// with `wanted`'s vault and hand-over and `command`'s arguments, where it starts.
fn bound_of(
    asker: &Asker,
    wanted: &Wanted,
    command: &[String],
    ready: &crate::sandbox::tools::Ready,
) -> super::ask::run::Bound {
    let folder = asker.folder.as_deref().filter(|folder| folder.is_dir());
    let start_in = wanted
        .cwd
        .as_deref()
        .filter(|cwd| cwd.is_absolute() && cwd.is_dir())
        .or(folder);
    let mut env = wanted.env.clone();
    let mut file = wanted.file.clone();
    env.sort();
    file.sort();
    super::ask::run::Bound {
        chat: asker.chat,
        name: command_name(&command[0]),
        program: ready.program.clone(),
        vault: wanted.vault.clone(),
        env,
        file,
        cwd: start_in.map(Path::to_path_buf).unwrap_or_default(),
        args: command[1..].to_vec(),
    }
}

/// **A run's one notice** (#5242): what it binds and shows, with its fields and what it offers.
#[allow(clippy::too_many_arguments)]
fn need_of(
    asker: &Asker,
    confines: &Confines,
    command: &[String],
    ready: &crate::sandbox::tools::Ready,
    bound: super::ask::run::Bound,
    fills: Vec<super::ask::run::Fill>,
    offer: super::ask::run::Offer,
    not_its_vault: bool,
) -> super::ask::run::RunNeed {
    use super::ask::run::{Hosts, RunNeed};
    RunNeed {
        ahead: false,
        fills,
        offer,
        bound,
        not_its_vault,
        persona: asker.persona.clone().unwrap_or_default(),
        hosts: if confines.level == crate::sandbox::Level::Floor {
            Hosts::Every
        } else {
            Hosts::Listed(confines.decides().hosts())
        },
        caution: crate::sandbox::tools::caution_here(&ready.tool),
        command: command_shown(command),
    }
}

/// **Holds a run on its one notice** (#5242), telling the app's gate it took the run, so no vault
/// ask is raised beside it. What it came to, or the chat's refusal.
fn held_on(
    missing: &Missing<'_>,
    need: super::ask::run::RunNeed,
    say: &mut dyn FnMut(&str),
) -> Result<super::ask::run::RunHeld, (i32, String)> {
    use super::ask::run::RunHeld;
    if let Some(gate) = missing.run_gate {
        gate.took_it
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    match missing
        .asks
        .allowed_or_held(need, missing.within, &mut |line: &str| {
            (missing.moved)();
            say(line);
        }) {
        RunHeld::Refused { line } => Err((1, line)),
        RunHeld::StillOpen { line } => Err((WAITING_ON_THE_PERSON, line)),
        held => Ok(held),
    }
}

/// **[`crate::sandbox::tools::for_run_grant`], and no key a browser is handed** (#5265): a key any
/// persona's browser login or secret-fill line holds, or a `mint` line marks, is offered to no
/// program, through a link the person made either way and in any case, since a provider may read
/// a key in any case ([`crate::secretfill::a_browsers`]). A settings file that can't be read
/// offers nothing.
fn grantable(
    asker: &Asker,
    persona: &str,
    asked: &crate::sandbox::tools::Asked<'_>,
    listed: &[crate::sandbox::tools::Tool],
    locks: &crate::sandbox::policy::Locks,
    writes: &[PathBuf],
) -> Result<crate::sandbox::tools::Ready, String> {
    use crate::sandbox::tools;
    let ready = tools::for_run_grant(&asker.root, persona, asked, listed, locks, writes)?;
    let keys: Vec<&str> = ready
        .tool
        .values
        .iter()
        .map(|value| value.key.as_str())
        .collect();
    match handed_to_no_granted_program(&reading_for(asker), Some(persona), asked.vault, &keys) {
        Some(why) => Err(why),
        None => Ok(ready),
    }
}

/// **Why no program may be handed one of `keys` of `vault`** (#5265), if one is a key that a
/// browser login or secret-fill line of any persona holds, or a `mint` line marks, through a
/// link the person made either way and in any case ([`crate::secretfill::a_browsers`]); or the
/// project's settings file can't be read to tell. Judged before a person is asked, again as they
/// answer and once they have, since a line may come in with a pull while they read, and after a
/// Fill, which may link the key to an entry the person keeps.
pub fn no_program_is_handed(ctx: &Ctx, vault: &str, keys: &[&str]) -> Option<String> {
    NoProgram::read(ctx).refuses(vault, keys)
}

/// **Why no program the person allowed may be handed one of `keys` of `vault`** (ADR 0112): what
/// [`no_program_is_handed`] refuses, and a key that is a link into a vault `persona` may not use
/// (D-5246-1, the person's ruling amending D-5080-11). A Run grant's notice names only the
/// persona's vault, so a value read through such a link would go where the person never read;
/// however the link came to be, before the picker and the press were narrowed too: the judge
/// every read of a link is held to ([`read_out_of_reach`], #5795).
fn handed_to_no_granted_program(
    ctx: &Ctx,
    persona: Option<&str>,
    vault: &str,
    keys: &[&str],
) -> Option<String> {
    no_program_is_handed(ctx, vault, keys)
        .or_else(|| read_out_of_reach(ctx, persona.unwrap_or_default(), vault, keys))
}

/// **Why `persona` may read none of `keys` of `vault`** (#5795, the person's ruling: judge a link
/// when it is read): one is a link into a vault `persona` may not use, which is whatever
/// [`authorise`] would not open for it. A link to another entry of the same vault reaches no new
/// vault and is not judged, as the press does not judge it ([`read_through_by`]). Asked before a value is read for
/// a declared tool's run, a run under a Run grant, a browser fill, a login (declared or granted)
/// and a profile's sign-in, and again once it is read ([`read_judged`]). A fill or a granted login
/// counts as a reader of its vault, so this covers links made before #5745's press check and a
/// persona that came to use the link's vault later, by a tag, an Allow, Always allow or a lifted
/// lock. Fails closed: a link that can't be read refuses, and so does this machine's grants file
/// where it can't be read, which counts as a refusal here and not as no grants. What the chat
/// hears names the key and its own vault, never the other vault (D-5246-8), nor anything of the
/// grants file, which holds other vaults' names: that goes to the log.
pub fn read_out_of_reach(ctx: &Ctx, persona: &str, vault: &str, keys: &[&str]) -> Option<String> {
    for &key in keys {
        let to = match super::links::of(ctx, vault, key) {
            Ok(None) => continue,
            // Another entry of the same vault: no new vault is reached, as the press judges it
            // too ([`read_through_by`]).
            Ok(Some(to)) if to.vault == vault => continue,
            Ok(Some(to)) => to,
            Err(_) => {
                return Some(format!(
                    "purlis could not read where {key} in vault {vault} leads, so it read none \
                     of it"
                ));
            }
        };
        if !crate::sandbox::policy::Locks::of(&ctx.root).forbids_vault_grants()
            && let Err(e) = crate::sandbox::local::try_granted_vaults(&ctx.root)
        {
            tracing::warn!(
                "purlis: read nothing through {key} in vault {vault}: this machine's vault \
                 grants at {} did not read ({e})",
                crate::sandbox::local::path(&ctx.root).display()
            );
            return Some(format!(
                "{key} in vault {vault} is a link, and purlis could not read which vaults this \
                 machine allows each persona, so it read none of it. The person can find why in \
                 purlis's log."
            ));
        }
        if authorise(ctx, Some(persona), &to.vault).is_err() {
            return Some(if persona.is_empty() {
                format!(
                    "{key} in vault {vault} reads an entry of another vault, and this chat runs \
                     as no persona, so purlis read none of it"
                )
            } else {
                format!(
                    "{key} in vault {vault} reads an entry of a vault {persona} may not use, so \
                     purlis read none of it. The person can link it to an entry {persona} may \
                     use, or fill it."
                )
            });
        }
    }
    None
}

/// **`read`, judged by [`read_out_of_reach`] before and after it**: a link written between the
/// first judgement and the read is seen by the second, and what was read is then dropped unused.
/// `refused` turns the judge's sentence into `read`'s error.
pub fn read_judged<T, E>(
    ctx: &Ctx,
    persona: &str,
    vault: &str,
    keys: &[&str],
    read: impl FnOnce() -> Result<T, E>,
    refused: impl Fn(String) -> E,
) -> Result<T, E> {
    if let Some(why) = read_out_of_reach(ctx, persona, vault, keys) {
        return Err(refused(why));
    }
    let got = read();
    if let Some(why) = read_out_of_reach(ctx, persona, vault, keys) {
        drop(got);
        return Err(refused(why));
    }
    got
}

/// **[`no_program_is_handed`] for many keys, the project's file and the links index read once**
/// (#5582): what a picker of many entries judges each with. Each key is judged from what was
/// read, failing closed as the one-off judge does; a line that comes after the read is seen by
/// the next one, so a judge is read again for each judgement that gives anything.
pub struct NoProgram(crate::secretfill::HeldByLines);

impl NoProgram {
    /// Reads what judges every key.
    pub fn read(ctx: &Ctx) -> Self {
        NoProgram(crate::secretfill::HeldByLines::read(ctx))
    }

    /// Why no program may be handed one of `keys` of `vault`, judged from what was read.
    pub fn refuses(&self, vault: &str, keys: &[&str]) -> Option<String> {
        use crate::sandbox::tools::mint;
        if self.0.unreadable() {
            return Some(mint::unreadable_said());
        }
        for &key in keys {
            if self.0.mints(vault, key) {
                return Some(mint::refused_said(vault, key));
            }
            if self.0.browsers(vault, key) {
                return Some(format!(
                    "{key} in vault {vault} is one purlis fills in a browser itself, on the one \
                     site a persona declares, so no program is handed it"
                ));
            }
        }
        None
    }
}

/// **Whether `wanted`'s command runs under a Run grant** (ADR 0112, #5239): the program its words
/// name, as the app resolves it where no chat writes, once the person allowed it. Asked only for
/// a program that is no declared tool, and refused with no notice for one no tool may be
/// ([`crate::sandbox::tools::for_run_grant`]), and for any `why`, which is what the chat was to
/// hear without this. `say` tells the command, as it starts to wait, what it waits on.
#[allow(clippy::too_many_arguments)]
fn run_granted(
    asker: &Asker,
    confines: &Confines,
    wanted: &Wanted,
    command: &[String],
    missing: &Missing<'_>,
    not_its_vault: bool,
    why: String,
    say: &mut dyn FnMut(&str),
) -> Result<(crate::sandbox::tools::Ready, super::ask::run::Bound), (i32, String)> {
    use crate::sandbox::tools;
    // The person kept this vault blocked for this chat: nothing here is to open it, whatever was
    // given before, and they are not asked again.
    if not_its_vault && missing.run_gate.is_some_and(|gate| gate.kept_blocked) {
        return Err((1, why));
    }
    let persona = asker.persona.as_deref().unwrap_or_default();
    // A server's start is its harness's, and nobody is there to retry it.
    if wanted.mcp.is_some() {
        return Err((1, why));
    }
    let plane = crate::sandbox::Plane::read(&asker.root);
    // A manifest nobody can read may declare this program, so nobody is asked about it.
    if plane.unreadable() {
        return Err((1, why));
    }
    let locks = crate::sandbox::policy::Locks::of(&asker.root);
    // Every tool line written for this persona, in force or not: a teammate's is no ask.
    let (written_lines, _) = tools::declared_and_allowed_here(&asker.root, persona);
    let listed = written_lines.as_slice();
    let asked = tools::Asked {
        vault: &wanted.vault,
        env: &wanted.env,
        file: &wanted.file,
        dotenv: &wanted.dotenv,
        program: &command[0],
    };
    let writes = chat_writes(asker, confines);
    // A tool's own program, or one a teammate declared, is held as it always was: refused in the
    // words it always was, with nothing asked. Only a grant the person gave is another matter,
    // since Always allow declares a program as it grants it.
    let is_a_tool = tools::is_declared(&confines.tools, listed, &command[0]);
    let locked = run_grant_locked(asker, wanted, command, &locks);
    if is_a_tool && (locked.is_some() || tools::limits_its_arguments(listed, &command[0])) {
        return Err((1, why));
    }
    // A flag that names a command is not offered (#5799), by the name the chat gave and by the
    // program a link leads to.
    let names_a_command =
        |name: &str| tools::command_flags::names_a_command(name, &command[1..]).map_or(Ok(()), Err);
    // Policy first (#5243): whatever was given before, nothing runs under a Run grant while it
    // locks them, and nobody is offered an Allow. A key the command misses can still be stored,
    // whichever lock holds, for any program that could have been allowed without it: one notice
    // with its fields, and, for a vault the persona may not use, that vault's own Allow (#5242).
    if let Some(locked) = locked {
        use super::ask::run::Offer;
        missing
            .asks
            .refused_run_by_policy(asker.chat, &command_name(&command[0]), &wanted.vault);
        let unlocked = crate::sandbox::policy::Locks::none();
        let Ok(ready) = grantable(asker, persona, &asked, listed, &unlocked, &writes) else {
            return Err((1, locked));
        };
        // Nor a key for a command whose flag names a command, which no Allow could follow
        // (#5876's second review).
        if names_a_command(&command_name(&command[0])).is_err()
            || names_a_command(&command_name(&ready.program.to_string_lossy())).is_err()
        {
            return Err((1, locked));
        }
        if not_its_vault && missing.run_gate.is_some_and(|gate| gate.kept_blocked) {
            return Err((1, locked));
        }
        let fills = fills_missing(asker, &reading_for(asker), wanted);
        if fills.is_empty() {
            return Err((1, locked));
        }
        if let Some(refused) = fills_refused(asker, &wanted.vault, &fills) {
            return Err((1, refused));
        }
        let offer = if not_its_vault {
            Offer::Vault {
                no_allow: Some(locked.clone()),
            }
        } else {
            Offer::Fill {
                no_allow: Some(locked.clone()),
            }
        };
        let bound = bound_of(asker, wanted, command, &ready);
        let need = need_of(
            asker,
            confines,
            command,
            &ready,
            bound,
            fills,
            offer,
            not_its_vault,
        );
        held_on(missing, need, say)?;
        // Stored, or the vault allowed; the lock still holds the run.
        return Err((1, locked));
    }
    let refused = |said: String| {
        if is_a_tool {
            (1, why.clone())
        } else {
            (1, said)
        }
    };
    names_a_command(&command_name(&command[0])).map_err(refused)?;
    let ready = grantable(asker, persona, &asked, listed, &locks, &writes)
        .map_err(|refused_as| refused(format!("{why} ({refused_as})")))?;
    names_a_command(&command_name(&ready.program.to_string_lossy())).map_err(refused)?;
    // A command its run would refuse is not offered.
    tools::allows(&ready, &command[1..]).map_err(refused)?;
    let bound = bound_of(asker, wanted, command, &ready);
    if bound.cwd.as_os_str().is_empty() {
        // No folder to judge the repository it reads by: as before, nothing more to judge.
    } else {
        tools::reads_held_repository(&ready, &command[1..], &bound.cwd, &asker.root, &writes)
            .map_err(refused)?;
    }
    // A grant for the chat lasts as long as policy leaves a scope it can stand at.
    use crate::sandbox::grant::Level;
    if !locks.keeps_scope(Level::Chat)
        && !locks.keeps_scope(Level::Project)
        && missing.asks.covers_for_the_chat(&bound)
    {
        return Err((
            1,
            format!(
                "Policy removes every scope a program could be allowed at for a chat, so {} needs \
                 to be declared as a tool of {persona}'s. {}",
                bound.name,
                locks.locked_by()
            ),
        ));
    }
    let covered = missing.asks.covers(&bound);
    if is_a_tool && !covered {
        return Err((1, why));
    }
    // What it lacks, gathered before anything is asked, so one notice asks all of it (#5242).
    let fills = fills_missing(asker, &reading_for(asker), wanted);
    if let Some(refused) = fills_refused(asker, &wanted.vault, &fills) {
        return Err((1, refused));
    }
    use super::ask::run::Offer;
    let offer = if covered {
        // A grant covers it: a key it lacks is the one gate left, asked of the person as a
        // Secret ask, as before, where it is one of a vault its persona uses.
        if fills.is_empty() || (fills.len() == 1 && !not_its_vault) {
            if let Some(gate) = missing.run_gate {
                gate.took_it
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            return Ok((ready, bound));
        }
        if not_its_vault {
            // Only an Allow stores in a vault its persona may not use: the vault's own.
            if missing.run_gate.is_some_and(|gate| gate.kept_blocked) {
                return Err((1, why));
            }
            Offer::Vault { no_allow: None }
        } else {
            Offer::Fill { no_allow: None }
        }
    } else {
        Offer::Program
    };
    let shown = command_shown(command);
    // What the person reads must be what runs, so these go unshown. A grant for any arguments was
    // given without them, and shows nothing.
    if !covered {
        if shown.chars().count() > MOST_COMMAND_SHOWN {
            return Err((
                1,
                format!(
                    "{why} (this command is too long to show the person whole, so purlis did not \
                     ask: shorten it)"
                ),
            ));
        }
        // A stretch the mask would hide could hide a host.
        if crate::dispatchrecord::masked(&shown) != shown {
            return Err((
                1,
                "this command holds what reads as a credential, so purlis did not show it to the \
                 person or run it: hand a credential by --file or --env"
                    .to_owned(),
            ));
        }
    }
    let need = need_of(
        asker,
        confines,
        command,
        &ready,
        bound.clone(),
        fills,
        offer,
        not_its_vault,
    );
    held_on(missing, need, say)?;
    // Allowed, or its values filled under a grant. Policy may have come to lock them while it
    // waited, and a line may have come to make a key a browser's: refused so, its Allow once
    // goes unused, and no retry may use it (the person's ruling, #5242).
    let refused_after = |why: String| {
        missing.asks.drop_once(&bound);
        Err((1, why))
    };
    if let Some(locked) = run_grant_locked(
        asker,
        wanted,
        command,
        &crate::sandbox::policy::Locks::of(&asker.root),
    ) {
        return refused_after(locked);
    }
    let keys: Vec<&str> = ready
        .tool
        .values
        .iter()
        .map(|value| value.key.as_str())
        .collect();
    match handed_to_no_granted_program(
        &reading_for(asker),
        asker.persona.as_deref(),
        &wanted.vault,
        &keys,
    ) {
        Some(why) => refused_after(why),
        None => Ok((ready, bound)),
    }
}

/// How long the app waits for a chat that is still starting to be recorded as open, before a
/// brokered run it asked for is refused (qw381).
///
/// A chat's harness may start a persona's MCP server as soon as its program runs, and that
/// server asks for its run at once; the app records the chat as open only once its start is
/// done, which takes longer while several chats start together. Only a line that carried the
/// chat's current token, from inside its program, is waited for.
pub const A_START_IS_WAITED_FOR: Duration = Duration::from_secs(10);

/// How often a starting chat's record is looked for.
const A_START_IS_LOOKED_AT_EVERY: Duration = Duration::from_millis(50);

/// **The asking chat's record, once the app has it** (qw381): `look` now, and again until it
/// answers or `at_most` has passed.
pub fn once_recorded<T>(mut look: impl FnMut() -> Option<T>, at_most: Duration) -> Option<T> {
    let until = std::time::Instant::now() + at_most;
    loop {
        if let Some(found) = look() {
            return Some(found);
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(A_START_IS_LOOKED_AT_EVERY);
    }
}

/// The chat that asked, as the app recorded it: never as the line says.
#[derive(Debug, Clone)]
pub struct Asker {
    /// The project the chat runs in.
    pub root: PathBuf,
    /// The app's own environment, which every value is resolved in.
    pub env: Env,
    pub chat: u32,
    /// The persona the app started the chat as.
    pub persona: Option<String>,
    /// The chat's folder, which its sandbox lets it write.
    pub folder: Option<PathBuf>,
    /// What the chat's sandbox was compiled to when it started, or `None` for a chat the app
    /// did not start sandboxed.
    pub confines: Option<Confines>,
    /// The harness the app started the chat on, where it knows one (#1353): what a block of the
    /// run names as the chat's, never a word the line carried.
    pub harness: Option<crate::harness::Harness>,
    /// The count of chat ends ([`crate::forge::apptoken::ends_so_far`]) read before the chat was
    /// found open, where the caller looked it up: a chat that ends from then on keeps no GitHub
    /// App token minted for it (#3808). `None` counts from when the run is served.
    pub ends_seen: Option<u64>,
}

/// Why nothing is run where purlis has no sandbox to run it in: neither macOS nor Linux. The
/// chat's `purlis secret exec` prints it, so it names no issue (#1422).
pub const NO_WRAP: &str = "purlis cannot yet run a command in a sandbox on this system, so it \
                           runs none for a sandboxed chat here: run `purlis secret exec` in a \
                           terminal outside the chat";

/// Why nothing is run on Linux for a chat whose start held no program of purlis's own to wrap a
/// run in ([`Confines::wrap`], #2342).
pub const NO_WRAP_RECORDED: &str = "purlis did not find its own program in a place this chat \
                                    cannot write when the chat started, so it runs no command \
                                    for the chat: install purlis outside the project and outside \
                                    what a chat may write, then start the chat again";

/// A vault a chat asked for that is registered and is not one its persona may use (#1430):
/// what the Notice on the chat's tab names. Every field is the app's own record or the
/// registry's, never the line's but the vault's name, which is one the registry holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotTagged {
    pub vault: String,
    /// The persona the app started the chat as.
    pub persona: String,
    /// The persona the registry tags the vault for, where it tags one.
    pub tagged_for: Option<String>,
}

/// Whether you let `persona`'s chats use `vault` on this machine (#1430): a grant in
/// [`crate::sandbox::local`], which no chat writes, and which an administrator's policy can
/// take away ([`crate::sandbox::policy::Locks::forbids_vault_grants`]). Read on every call, so
/// an Allow or a Revoke reaches the next run with no restart.
fn allowed_here(ctx: &Ctx, persona: &str, vault: &str) -> bool {
    !crate::sandbox::policy::Locks::of(&ctx.root).forbids_vault_grants()
        && crate::sandbox::local::granted_vaults(&ctx.root)
            .iter()
            .any(|one| one.vault == vault && one.persona == persona)
}

/// **The vaults a chat started as `persona` is handed**, by name and sorted: the ones the
/// registry tags for it and the ones you let it use on this machine, which is what
/// [`authorise`] opens. Never the `vault:` line of the persona's own file. An error where the
/// registry does not read, so "which is not known" is never said as "none". Names only: no
/// vault is opened, and nothing inside one is read.
pub fn vaults_of(ctx: &Ctx, persona: &str) -> Result<Vec<String>, String> {
    let doc = registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    let mut out = registry::vaults_for_persona(&doc, persona);
    if !crate::sandbox::policy::Locks::of(&ctx.root).forbids_vault_grants() {
        for one in crate::sandbox::local::granted_vaults(&ctx.root) {
            let held = registry::vaults(&doc).contains_key(&one.vault);
            if one.persona == persona && held && !out.contains(&one.vault) {
                out.push(one.vault);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// **The personas that may use `vault`** (#5745), by name and sorted: the one the registry tags
/// it for, and each you let use it on this machine, which is whom [`authorise`] opens it for. A
/// grant that policy's lock holds dormant is not counted, as `authorise` does not open it: lifting
/// the lock is a later grant (#5795). No personas (an empty list) for a vault the registry
/// does not hold. An error where the registry, or the vault's own entry in it, does not read, so
/// "which is not known" is never said as "none".
pub fn personas_of(ctx: &Ctx, vault: &str) -> Result<Vec<String>, String> {
    let doc = registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    if !registry::vaults(&doc).contains_key(vault) {
        return Ok(Vec::new());
    }
    let registered = registry::vault_in(&doc, vault).map_err(|e| e.message.to_string())?;
    let mut out: Vec<String> = registered.persona.into_iter().collect();
    if !crate::sandbox::policy::Locks::of(&ctx.root).forbids_vault_grants() {
        for one in crate::sandbox::local::granted_vaults(&ctx.root) {
            if one.vault == vault {
                out.push(one.persona);
            }
        }
    }
    out.retain(|one| !one.is_empty());
    out.sort();
    out.dedup();
    Ok(out)
}

/// **Why a link in `vault` may not read an entry of `to`** (#5745, the person's ruling of
/// 2026-11-08: refuse such links): some persona that may use `vault` may not use `to`, and would
/// read `to`'s entry through the link, by a declared tool if by nothing else. Failing closed
/// where the registry does not read. Said for the person, who sees both vaults' names.
pub fn read_through_by(ctx: &Ctx, vault: &str, to: &str) -> Option<String> {
    if vault == to {
        return None;
    }
    let personas = match personas_of(ctx, vault) {
        Ok(personas) => personas,
        Err(why) => {
            return Some(format!(
                "purlis could not read which personas may use vault {vault}, so nothing is \
                 linked there ({why}). Check it with `purlis vault verify {vault}`, or type a \
                 value."
            ));
        }
    };
    personas
        .iter()
        .find(|persona| authorise(ctx, Some(persona), to).is_err())
        .map(|persona| {
            format!(
                "{persona} may use vault {vault} and can't use vault {to}, so it would read \
                 {to}'s entry through the link. Pick an entry every persona of {vault} may use, \
                 or type a value."
            )
        })
}

/// `vault` as [`NotTagged`] for a chat started as `persona`, or `None` where there is nothing
/// to allow: the chat runs as no persona, the vault is not registered, or the persona may use
/// it already.
pub fn not_tagged(ctx: &Ctx, persona: Option<&str>, vault: &str) -> Option<NotTagged> {
    let persona = persona.filter(|p| !p.is_empty())?;
    let doc = registry::load_registry(ctx).ok()?;
    let tagged_for = registry::vault_in(&doc, vault).ok()?.persona;
    if tagged_for.as_deref() == Some(persona) || allowed_here(ctx, persona, vault) {
        return None;
    }
    Some(NotTagged {
        vault: vault.to_owned(),
        persona: persona.to_owned(),
        tagged_for: tagged_for.filter(|p| !p.is_empty()),
    })
}

/// What every refusal of a vault says of the chat's own persona (ADR 0090 as amended, #1435).
pub const FIXED: &str = "A chat's persona is fixed for its life, so nothing run in this chat \
                         changes which vaults it may use.";

/// The persona `not`'s vault is tagged for, where a chat can be dispatched to it: one the
/// project defines. The registry's tag is any text its shared, committed half holds, so a tag
/// that names no persona offers no chat, and the name is one line wherever it is shown.
pub fn dispatchable(ctx: &Ctx, not: &NotTagged) -> Option<String> {
    let theirs = not.tagged_for.as_deref()?;
    crate::personas::name_refusal(&ctx.root, theirs)
        .is_none()
        .then(|| crate::personas::one_line(theirs))
}

/// **The ways forward a refused vault names** (#1430), for the chat that reads the refusal: the
/// operator's Allow on the chat's tab, where policy leaves it open, and a dispatch to the
/// persona the vault is tagged for, where the project defines that persona
/// ([`dispatchable`]). Never a change of the chat's own persona, which is fixed for its life.
pub fn routes(ctx: &Ctx, not: &NotTagged) -> String {
    let dispatch = dispatchable(ctx, not).map(|theirs| {
        format!(
            "to have '{theirs}', the persona the vault is tagged for, do the work, dispatch to \
             it: `purlis dispatch --to {theirs}`"
        )
    });
    let said = match (
        crate::sandbox::policy::Locks::of(&ctx.root).vault_grants_refused(),
        dispatch,
    ) {
        (None, Some(dispatch)) => format!("Two ways forward: {ALLOW}; or, {dispatch}."),
        (None, None) => format!("The way forward: {ALLOW}."),
        (Some(locked), Some(dispatch)) => format!("{locked} The way forward: {dispatch}."),
        (Some(locked), None) => locked,
    };
    format!("{said} {FIXED}")
}

/// The first way forward: the operator's Allow, which the next run reads.
const ALLOW: &str = "ask the operator to press Allow on this chat's vault notice, then run \
                     the command again (no restart is needed)";

/// Whether `persona` may use `vault`: the vault registry tags it with that persona
/// (`purlis vault add <vault> --persona <persona>`), or you allowed it for that persona on this
/// machine ([`allowed_here`], D-1430-1). A chat on no persona uses no vault.
///
/// **The registry and your own grant, and nothing a chat writes.** D-1407-1 made the registry
/// the only thing that opens a vault to a chat; D-1430-1 widens it by one thing, the person's
/// own grant on this machine. A persona's own `vault:` line is what `purlis persona secret`
/// reads first, but it lives in the persona's file, which a chat can write (a chat at the
/// project root writes `personas/`, and a chat edits its own persona's charter), so it cannot
/// be what lets a chat at a vault: a chat would grant itself any vault by naming it there. The
/// registry is denied to every chat's writes (ADR 0067 §5 class 1), and so is the file your
/// grant is kept in (the integrity class).
///
/// **A vault the registry does not hold is refused in the registry's own words**, with no way
/// forward named: there is nothing to allow and nobody to dispatch to, and no Notice is raised
/// for it ([`not_tagged`]).
pub fn authorise(ctx: &Ctx, persona: Option<&str>, vault: &str) -> Result<(), String> {
    let Some(persona) = persona.filter(|p| !p.is_empty()) else {
        return Err(format!(
            "this chat runs as no persona, so purlis hands it no vault. Start the chat as the \
             persona that vault '{vault}' is tagged with."
        ));
    };
    let doc = registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    let registered = registry::vault_in(&doc, vault).map_err(|e| e.message.to_string())?;
    let tagged = registry::vaults_for_persona(&doc, persona);
    if tagged.iter().any(|t| t == vault) || allowed_here(ctx, persona, vault) {
        return Ok(());
    }
    let not = NotTagged {
        vault: vault.to_owned(),
        persona: persona.to_owned(),
        tagged_for: registered.persona.filter(|p| !p.is_empty()),
    };
    let routes = routes(ctx, &not);
    if cmd::persona_vault(ctx, persona).is_ok_and(|declared| declared == vault) {
        return Err(format!(
            "persona '{persona}' names vault '{vault}' in its own file, and a chat can edit that \
             file, so purlis does not open the vault for a chat on that alone. {routes}"
        ));
    }
    let theirs = if tagged.is_empty() {
        "none is tagged with it".to_owned()
    } else {
        format!(
            "it may use {}",
            tagged
                .iter()
                .map(|a| format!("'{a}'"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(format!(
        "vault '{vault}' is not one persona '{persona}' may use, so purlis did not open it \
         ({theirs}). {routes}"
    ))
}

/// One vault as `purlis vault list` shows it to a sandboxed chat (#1430): its name, the
/// provider's id, the persona the registry tags it for, and whether the chat's persona may use
/// it. **No value, no key name, and nothing of a provider's session.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Listed {
    pub name: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// Whether the chat that asked may use it, as [`authorise`] would answer.
    pub usable: bool,
}

/// Every vault the project registers, for a chat started as `persona`: read from the registry
/// alone, in the app, so no provider is asked and no session is opened. The registry, the
/// policy and this machine's grants are each read once.
pub fn listed(ctx: &Ctx, persona: Option<&str>) -> Result<Vec<Listed>, String> {
    let doc = registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    let persona = persona.filter(|p| !p.is_empty());
    let granted = if crate::sandbox::policy::Locks::of(&ctx.root).forbids_vault_grants() {
        Vec::new()
    } else {
        crate::sandbox::local::granted_vaults(&ctx.root)
    };
    let mut names: Vec<String> = registry::vaults(&doc).keys().cloned().collect();
    names.sort();
    Ok(names
        .into_iter()
        .filter_map(|name| {
            let vault = registry::vault_in(&doc, &name).ok()?;
            let tag = vault.persona.filter(|p| !p.is_empty());
            let usable = persona.is_some_and(|persona| {
                tag.as_deref() == Some(persona)
                    || granted
                        .iter()
                        .any(|one| one.vault == name && one.persona == persona)
            });
            Some(Listed {
                usable,
                provider: vault.provider,
                persona: tag,
                name,
            })
        })
        .collect())
}

/// What the app answers `purlis vault list` from a chat: the persona it started the chat as,
/// every vault, and whether an administrator's policy forbids the person's Allow here, so the
/// listing names only a way forward that is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub persona: Option<String>,
    pub vaults: Vec<Listed>,
    pub allow_locked: bool,
}

/// How long `purlis vault list` waits for the app's answer: the registry alone is read.
#[cfg(unix)]
const A_LISTING_TAKES_AT_MOST: Duration = if cfg!(test) {
    Duration::from_secs(1)
} else {
    Duration::from_secs(5)
};

/// How long `purlis secret list <vault>` waits for the app's answer: a provider may be asked
/// (`op`, the keyring's index), which takes longer than the registry alone.
#[cfg(unix)]
pub(crate) const A_KEY_LISTING_TAKES_AT_MOST: Duration = if cfg!(test) {
    Duration::from_secs(1)
} else {
    Duration::from_secs(30)
};

/// What a sandboxed chat says where the app took its ask and did not answer in time.
pub const NOT_IN_TIME: &str = "the app that started this chat did not answer in time, so \
                               nothing was read. Try again; this chat's sandbox does not let it \
                               read the vault itself.";

/// What a sandboxed chat says where the app is older than the ask: it answers that it cannot
/// read the line ([`crate::hookwire::UNKNOWN_LINE`]), or, older still, closes the connection
/// without an answer.
pub const OLDER_APP: &str = "the app that started this chat did not answer this: it is older \
                             than this purlis command. Update the purlis app and start the chat \
                             again; this chat's sandbox does not let it read the vault itself.";

/// **Asks the app that started this sandboxed chat**, for a listing (qw385, review of #1967).
/// `None` only where there is no app to ask: the chat is not sandboxed, it names no socket or
/// no chat, or nothing listens there (connecting refuses at once). **Once the ask is sent the
/// app's answer is the only one**: a deadline passed, a dropped connection or an answer of
/// another kind is an error naming it, never a fallback to reading the vault in the chat,
/// whose sandbox denies that.
#[cfg(unix)]
fn ask_the_app(
    ctx: &Ctx,
    ask: impl FnOnce(u32) -> crate::hookwire::Ask,
    within: Duration,
) -> Option<Result<crate::hookwire::Answer, String>> {
    use crate::hookwire::{Answer, Asking, ChatToken, NOTHING_ANSWERS, UNKNOWN_LINE};
    let env = |name: &str| ctx.env.get(name);
    if !crate::sandbox::chat_is_sandboxed_in(&env) {
        return None;
    }
    let socket = env(crate::hookwire::SOCKET_ENV).filter(|s| !s.is_empty())?;
    let chat = chat_number(&env)?;
    let mut asking = Asking::on(Path::new(&socket), ChatToken::read(&env)).ok()?;
    Some(match asking.ask(&ask(chat), within) {
        // An app older than the ask: one that reads no ask, or not this one (#1333).
        Ok(Answer::No { why }) if why == NOTHING_ANSWERS || why == UNKNOWN_LINE => {
            Err(OLDER_APP.to_owned())
        }
        Ok(answer) => Ok(answer),
        Err(e) => Err(match e.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => NOT_IN_TIME,
            _ => OLDER_APP,
        }
        .to_owned()),
    })
}

/// The app's answer to `purlis vault list` from a sandboxed chat it started (#1430), or `None`
/// where there is no app to ask ([`ask_the_app`]) and the listing is made here as it always
/// was.
#[cfg(unix)]
pub fn listing_from_the_app(ctx: &Ctx) -> Option<Result<Listing, String>> {
    use crate::hookwire::{Answer, Ask};
    Some(
        match ask_the_app(ctx, |chat| Ask::Vaults { chat }, A_LISTING_TAKES_AT_MOST)? {
            Ok(Answer::Vaults {
                persona,
                vaults,
                allow_locked,
            }) => Ok(Listing {
                persona,
                vaults,
                allow_locked,
            }),
            Ok(Answer::No { why }) => Err(why),
            Ok(_) => Err(OLDER_APP.to_owned()),
            Err(why) => Err(why),
        },
    )
}

/// Where there is no unix socket there is no app to ask.
#[cfg(not(unix))]
pub fn listing_from_the_app(_ctx: &Ctx) -> Option<Result<Listing, String>> {
    None
}

/// **The key names of `vault`, for a chat started as `persona`** (qw385): what the app answers
/// `purlis secret list <vault>` from a sandboxed chat, whose own sandbox denies it the vault's
/// storage. Authorised exactly as a brokered `secret exec` is ([`authorise`]: the registry's tag
/// or your grant on this machine), then read with the provider's own code in the app's
/// process. **Names only, sorted: never a value.** The refusal is [`authorise`]'s sentence,
/// which names the ways forward.
pub fn keys_for(ctx: &Ctx, persona: Option<&str>, vault: &str) -> Result<Vec<String>, String> {
    authorise(ctx, persona, vault)?;
    let v = cmd::provider(ctx, vault).map_err(|e| e.message.to_string())?;
    let mut keys = cmd::keys(ctx, &v).map_err(|e| e.message.to_string())?;
    keys.sort();
    Ok(keys)
}

/// The app's answer to `purlis secret list <vault>` from a sandboxed chat it started
/// (qw385): the key names, or its refusal. `None` only where there is no app to ask
/// ([`ask_the_app`]), and the vault is read here as it always was.
#[cfg(unix)]
pub fn keys_from_the_app(ctx: &Ctx, vault: &str) -> Option<Result<Vec<String>, String>> {
    use crate::hookwire::{Answer, Ask};
    let asked = |chat| Ask::Keys {
        chat,
        vault: vault.to_owned(),
    };
    Some(
        match ask_the_app(ctx, asked, A_KEY_LISTING_TAKES_AT_MOST)? {
            Ok(Answer::Keys { keys }) => Ok(keys),
            Ok(Answer::No { why }) => Err(why),
            Ok(_) => Err(OLDER_APP.to_owned()),
            Err(why) => Err(why),
        },
    )
}

/// Where there is no unix socket there is no app to ask.
#[cfg(not(unix))]
pub fn keys_from_the_app(_ctx: &Ctx, _vault: &str) -> Option<Result<Vec<String>, String>> {
    None
}

/// Whether `persona` declares a tool that is handed a value of `vault` (qw385), as the
/// project's own sandbox settings hold it now: what the line an Allow sends says runs. Names
/// nothing a chat wrote into that line; it only chooses between two fixed sentences.
pub fn declares_a_tool_with(root: &Path, persona: &str, vault: &str) -> bool {
    declared_now(root, persona)
        .iter()
        .any(|tool| tool.vault == vault)
}

/// **The browser login `persona` declares with `vault`**, where one is (#4595).
pub fn login_with(root: &Path, persona: &str, vault: &str) -> Option<String> {
    declared_now(root, persona)
        .into_iter()
        .find(|tool| tool.vault == vault && tool.login.is_some())
        .map(|tool| tool.name)
}

/// **What reading a value directly is refused with in a sandboxed chat** (qw385): `secret
/// get` and `secret cp` (and their `persona secret` forms), for every provider. A sandboxed
/// chat never reads a vault, and no grant changes that; the route that works is a tool its
/// persona declares, handed the value by `secret exec`, which the app runs. `None` outside a
/// sandboxed chat.
pub fn value_refused_in_this_chat(ctx: &Ctx, vault: &str) -> Option<Markup> {
    let env = |name: &str| ctx.env.get(name);
    if !crate::sandbox::chat_is_sandboxed_in(&env) {
        return None;
    }
    // The app runs no command for a chat on this system yet ([`NO_WRAP`]).
    let to_use = if runs_a_chats_command_here() {
        format!(
            "To use a value, hand it to a tool this chat's persona declares: purlis secret exec \
             {vault} --env NAME=key -- <tool> …; the app runs that for the chat."
        )
    } else {
        "To use a value, run the command in a terminal outside the chat: purlis cannot yet run \
         a command in a sandbox on this system."
            .to_owned()
    };
    Some(Markup::lines([
        Markup::from(format!(
            "a sandboxed chat does not read a vault's values, so purlis did not read '{vault}', \
             and no Allow changes that."
        )),
        Markup::from(format!(
            "  To check which keys it holds: purlis secret list {vault}"
        )),
        Markup::from(format!("  {to_use}")),
    ]))
}

/// How a run is wrapped: in the profile built from the chat's policy, applied by this system's
/// own mechanism as the chat's own is ([`crate::sandbox::wrapper`]); or, for a test of
/// everything around it on a machine that cannot apply a second sandbox, not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wrap {
    /// macOS: Seatbelt, through `sandbox-exec`.
    Seatbelt,
    /// Linux: purlis's own wrap (#1040, #2342), `purlis _sandbox`, started with the program
    /// the chat's own start checked and held ([`Confines::wrap`]).
    Purlis,
    /// No sandbox purlis can apply on this system: nothing runs ([`NO_WRAP`]).
    Nothing,
    #[cfg(test)]
    Unwrapped,
    /// Unwrapped, with the proxy refusing each of these `host:port`, split by spaces, as the
    /// run starts.
    #[cfg(test)]
    Refusing(&'static str),
}

/// Why a Claude Code chat's `secret exec --env` runs nothing on Linux ([`env_refused`]).
pub const ENV_REFUSED: &str = "on Linux a Claude Code chat under its own sandbox hands a value \
                               to a command by file only, so nothing was run: run it again with `--file NAME=key` in \
                               place of `--env NAME=key`, and the command reads the value from \
                               the file NAME names. A persona's tool that takes its value as \
                               `env` is declared with `as = \"NAME file\"` instead";

/// **Why a run handed `--env` values is refused before anything is read**: a Claude Code chat's
/// on Linux (`wrap` is purlis's own there) on Claude Code's own sandbox, which hands a value by
/// `--file` alone. A Claude Code chat purlis wraps whole (`whole`, as on a runner), every other
/// harness and every other system, as before.
fn env_refused(
    harness: Option<crate::harness::Harness>,
    whole: bool,
    wrap: Wrap,
    wanted: &Wanted,
) -> Option<&'static str> {
    (harness == Some(crate::harness::Harness::ClaudeCode)
        && !whole
        && wrap == Wrap::Purlis
        && !wanted.env.is_empty())
    .then_some(ENV_REFUSED)
}

/// **Whether the app runs a sandboxed chat's `secret exec` on this system**: where it has a
/// sandbox to run the command in, macOS's Seatbelt or purlis's own wrap on Linux (#2342). What
/// every sentence that names the route asks.
pub const fn runs_a_chats_command_here() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux"))
}

impl Wrap {
    /// This system's.
    fn here() -> Self {
        if !runs_a_chats_command_here() {
            Self::Nothing
        } else if cfg!(target_os = "macos") {
            Self::Seatbelt
        } else {
            Self::Purlis
        }
    }
}

/// How the run's sandbox is applied, once its checks have passed.
enum Applying {
    Seatbelt,
    /// By purlis's own program, as [`wrap_for`] checked it.
    Purlis(PathBuf),
    #[cfg(test)]
    Unwrapped,
}

/// What is told each sandbox block of a brokered run, for the asking chat's Notice.
pub type Told = Arc<dyn Fn(RunBlocked) + Send + Sync + 'static>;

/// **A sandbox block of a brokered run** (#1353): a host the run's proxy refused, as
/// [`crate::hookwire::SandboxBlocked`] carries a chat's own, with the harness the app recorded
/// for the asking chat; and the vault the run was given, so the Notice can say the host was
/// refused while a command ran with it, for the person to judge whether that vault belongs at
/// that host before allowing it.
///
/// Every field is the app's record or the proxy's, never the line's. The vault is the name the
/// run resolved and was authorised to open, never a value; and a host that carries one of the
/// run's values is never told at all ([`WITHHELD_NOTE`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunBlocked {
    pub block: crate::hookwire::SandboxBlocked,
    /// The vault's name, as the registry holds it.
    pub vault: String,
}

/// Answers `wanted` for `asker` on the connection `reader`/`writer`: every frame written there,
/// the last an [`Frame::Exit`] or a [`Frame::Refused`]. Returns once the child has ended.
/// `told` hears each host the run's proxy refused, as the asking chat's block.
pub fn serve(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    told: Told,
) {
    serve_in(asker, wanted, reader, writer, Wrap::here(), told);
}

/// The status a brokered command ends with when it stopped waiting on a Secret ask that is
/// still open (#5071): `EX_TEMPFAIL`, a failure to try again.
pub const WAITING_ON_THE_PERSON: i32 = 75;

/// **What a run that misses a key asks** (#5068, ADR 0109): the project's Secret asks, and how
/// long its command waits on one ([`super::ask::holds_for`]).
#[derive(Clone, Copy)]
pub struct Missing<'a> {
    pub asks: &'a super::ask::Asks,
    pub within: Duration,
    /// Told once an ask is raised or joined, so the window lists it.
    pub moved: &'a (dyn Fn() + Sync),
    /// What the app and the Run grant gate tell each other about this run, where the app raises
    /// a vault's own ask ([`RunGate`]).
    pub run_gate: Option<&'a RunGate>,
}

/// **The Run grant gate and the vault's own ask, for one run** (ADR 0112): one run raises one ask.
#[derive(Debug, Default)]
pub struct RunGate {
    /// Set when the gate takes the run: a grant covers it, the person's Don't allow stands, or it
    /// goes to be asked about, which a chat with too many asks open tries again. Whichever, the
    /// app raises no vault ask beside it.
    pub took_it: std::sync::atomic::AtomicBool,
    /// Whether the person kept this vault blocked for this chat: the gate offers nothing that
    /// would open it, so a later command can't bring the question back.
    pub kept_blocked: bool,
}

impl Missing<'_> {
    /// [`super::ask::Asks::read_asking`] for `need`, holding at most [`Self::within`] and telling [`Self::moved`] once it asks (#5074).
    pub fn read_asking<T>(
        &self,
        need: super::ask::Need,
        read: &mut dyn FnMut() -> Result<T, super::VaultError>,
    ) -> Result<T, super::ask::Unread> {
        self.asks
            .read_asking(need, self.within, &mut |_| (self.moved)(), read)
    }
}

/// [`serve`], telling `reached` each connection the run made (#1667): through its proxy, and
/// through a tunnel to a database host its values point at ([`crate::sandbox::tunnel`]). The
/// network record keeps them under the asking chat. A host that carries one of the run's values
/// is counted and never named. A key its vault holds no value under is asked of the person
/// through `missing` ([`Missing`]), and the run waits on the answer.
pub fn serve_recording(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    told: Told,
    reached: crate::sandbox::egress::Reached,
    missing: Missing<'_>,
) {
    serve_recorded(
        asker,
        wanted,
        None,
        reader,
        writer,
        Wrap::here(),
        told,
        Some(reached),
        Some(&missing),
    );
}

/// [`serve`] in `wrap`, asking `missing` for a key its vault holds no value under (#5068).
#[cfg(test)]
pub(crate) fn serve_asking(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    wrap: Wrap,
    missing: Missing<'_>,
) {
    serve_recorded(
        asker,
        wanted,
        None,
        reader,
        writer,
        wrap,
        Arc::new(|_| {}),
        None,
        Some(&missing),
    );
}

/// [`serve`] in `wrap`, telling nothing of what its proxy refused.
#[cfg(test)]
pub(crate) fn serve_wrapped(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    wrap: Wrap,
) {
    serve_in(asker, wanted, reader, writer, wrap, Arc::new(|_| {}));
}

/// Where `wanted` running `command` is exactly the `secret exec` line of a credentialed MCP
/// server of `persona` this machine approved ([`crate::personaverbs::chatstart::approved_servers`]):
/// its vault, its `--env` and `--file` hand-overs in order, no `--dotenv`, and its command. The
/// variables its approved entry sets (`env`) and their approved values, which its run starts
/// with (#3441).
fn an_approved_server(
    root: &Path,
    persona: &str,
    wanted: &Wanted,
    command: &[String],
) -> Option<Vec<(String, String)>> {
    let state = crate::personaverbs::state_dir(root);
    crate::personaverbs::chatstart::approved_servers(root, &state, persona)
        .into_iter()
        .find(|server| approved_line_is(&server.words, wanted, command))
        .map(|server| server.env)
}

/// Whether `words`, an approved server's `secret exec` words, ask for what `wanted` running
/// `command` does.
fn approved_line_is(words: &[String], wanted: &Wanted, command: &[String]) -> bool {
    let Some(at) = words.iter().position(|word| word == "--") else {
        return false;
    };
    let (flags, theirs) = (&words[..at], &words[at + 1..]);
    let mut env = Vec::new();
    let mut file = Vec::new();
    let mut rest = flags.iter().skip(3);
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--env" => env.extend(rest.next().cloned()),
            "--file" => file.extend(rest.next().cloned()),
            "--exec" | "--stream" => {}
            _ => return false,
        }
    }
    flags.get(2) == Some(&wanted.vault)
        && env == wanted.env
        && file == wanted.file
        && wanted.dotenv.is_empty()
        && theirs == command
}

/// **Every folder the chat may write**, as a brokered run judges a program's place: its grants,
/// its own folder, and the cache homes its sandbox widens to (#1337). A program found in one,
/// or a `PATH` entry naming one, is never one a run takes.
pub(crate) fn chat_writes(asker: &Asker, confines: &Confines) -> Vec<PathBuf> {
    let mut writable = confines.writable.clone();
    writable.extend(asker.folder.iter().cloned());
    if let Some(caches) = &confines.widened.caches {
        writable.extend(caches.trees.iter().chain(caches.bare.iter()).cloned());
    }
    writable
}

/// **The login chat `asker`'s browser login `name` fills** (#2016): the declared page, and the
/// values from its vault, for purlis to fill in the chat's browser itself, so none reaches the
/// chat. Exactly what a declared tool's run is held to (D-PE-ruling-2): a tool the chat's start
/// armed, allowed on this machine with its persona's hosts, whose vault the registry, or your
/// grant on this machine, lets that persona use. A chat the app started without the sandbox,
/// `unconfined` (no sandbox compiled for it and none holding it), is held to the same, its
/// persona's declarations and this machine's Allow read at the call (#2560); one a sandbox
/// still holds with no record of its start is offered a restart. Refused, with a sentence and
/// no value, for anything else.
pub fn login_for(
    asker: &Asker,
    name: &str,
    unconfined: bool,
) -> Result<purlis_session_protocol::browsing::LogIn, String> {
    login_and_identity_for(asker, name, unconfined, None).map(|(login, _)| login)
}

/// **[`login_for`], and the login's identity** (#2021): a digest of the persona, the project
/// and the login as declared and allowed, which its kept store is bound to
/// ([`crate::browserlogins::identity_of`]), so a login of the same name of another persona or
/// project, or declared otherwise since, never opens it. A value its vault holds none of is
/// asked of the person through `missing`, where the app asks, and the login waits on it (#5074).
pub fn login_and_identity_for(
    asker: &Asker,
    name: &str,
    unconfined: bool,
    missing: Option<&Missing<'_>>,
) -> Result<(purlis_session_protocol::browsing::LogIn, String), String> {
    let (ready, ctx) = login_ready(asker, name, unconfined)?;
    read_login(asker, name, &ready, &ctx, missing)
}

/// **The login the person allowed from its notice, read for chat `asker`** (#4595): `tool`, as the
/// app kept it from their press, with no start's record and no vault tag asked of it, since that
/// press confirmed this vault and these keys for this one login. Nothing a chat says reaches it.
/// The read is every other login's, so a key a `mint` line names is refused here too, and a
/// value its vault holds none of is asked of the person through `missing` (#5074).
pub fn granted_login(
    asker: &Asker,
    tool: &crate::sandbox::tools::Tool,
    missing: Option<&Missing<'_>>,
) -> Result<(purlis_session_protocol::browsing::LogIn, String), String> {
    let (ready, ctx) = granted_ready(asker, tool);
    let (login, identity) = read_login(asker, &tool.name, &ready, &ctx, missing)?;
    Ok((
        purlis_session_protocol::browsing::LogIn {
            suggested: true,
            ..login
        },
        identity,
    ))
}

/// [`granted_login`]'s identity, no value read.
pub fn granted_login_identity(
    asker: &Asker,
    tool: &crate::sandbox::tools::Tool,
) -> Result<String, String> {
    let (ready, _) = granted_ready(asker, tool);
    identity_of_ready(asker, &tool.name, &ready)
}

/// The tool and the context a login the person allowed is read in.
fn granted_ready(
    asker: &Asker,
    tool: &crate::sandbox::tools::Tool,
) -> (crate::sandbox::tools::Ready, Ctx) {
    let ready = crate::sandbox::tools::Ready {
        tool: tool.clone(),
        program: PathBuf::new(),
    };
    let ctx = match &asker.confines {
        Some(confines) => {
            Ctx::new(&asker.root, asker.env.clone()).chat(confines, asker.folder.as_deref())
        }
        None => Ctx::new(&asker.root, asker.env.clone()),
    };
    (ready, ctx)
}

/// **The value of `key` in `vault`, for a secret fill the person allowed** (#5115): read for
/// chat `asker` as a login the person allowed from its notice is ([`granted_login`]), since that
/// press confirmed this vault and this key for one exact site. Refused, with a sentence and no
/// value, for a private key, whoever asked: no private key is filled on a page; for a value that
/// holds an authenticator's address, whatever the person's mark says (#5237); and for a value
/// taken as a 2FA seed by its look, unless the person marked this value as no seed
/// ([`super::no_seed`]): a seed goes into no page, its code does ([`fill_code`]). `None` where
/// the vault holds no value under `key` (#5116).
pub fn fill_value(
    asker: &Asker,
    vault: &str,
    key: &str,
) -> Result<Option<purlis_session_protocol::browsing::Secret>, String> {
    let Some(value) = read_for_fill(asker, vault, key)? else {
        return Ok(None);
    };
    goes_into_a_page(asker, &value, vault, key)?;
    Ok(Some(value))
}

/// **The rules for a value that goes into a page**, a fill's and a login's alike (#5272): no private
/// key, no authenticator's address whatever the person's mark says, and nothing taken as a 2FA seed
/// by its look unless the person marked this value as no seed. Why not, in words that hold none of
/// it.
fn goes_into_a_page(
    asker: &Asker,
    value: &purlis_session_protocol::browsing::Secret,
    vault: &str,
    key: &str,
) -> Result<(), String> {
    if crate::sandbox::tools::mint::a_private_key(value.expose()) {
        return Err(format!(
            "{key} in vault '{vault}' is a private key, so purlis filled nothing"
        ));
    }
    // The next step, said only where it would work.
    let code = if super::totp::Seed::parse(value.expose()).is_ok() {
        "Call browser_fill_secret with `code` to fill the code it makes"
    } else {
        "purlis can't make a code from this value either"
    };
    if super::totp::holds_an_address(value.expose()) {
        return Err(format!(
            "{key} in vault '{vault}' holds an otpauth address, which carries a 2FA seed, so \
             purlis filled nothing: a seed never goes into a page, and no mark changes that. {code}"
        ));
    }
    if super::totp::looks_like_a_seed(value.expose()) {
        // The key is named once, up front: what the person has to do comes last, and has to
        // survive what their view of the browser keeps of a long key's name.
        let place = "in the window, open the vault's tab and open the menu on this key's row, or \
                     on the row of the entry it reads if it is linked, and choose";
        let then = match super::no_seed::of(&reading_for(asker), vault, key, value.expose()) {
            super::no_seed::Mark::Holds => return Ok(()),
            super::no_seed::Mark::None => format!(
                "If its value is no seed, ask the person to mark it: {place} {}",
                super::no_seed::MARK_WORDS
            ),
            super::no_seed::Mark::ValueChanged => format!(
                "The person marked this key as no 2FA seed, and its value has changed since. If \
                 this value is no seed either, ask them to mark it again: {place} {}, then {}",
                super::no_seed::UNMARK_WORDS,
                super::no_seed::MARK_WORDS
            ),
        };
        return Err(format!(
            "{key} in vault '{vault}' looks like a 2FA seed, so purlis filled nothing: a seed \
             never goes into a page. {code}. A chat can't make this mark. {then}"
        ));
    }
    Ok(())
}

/// **The 2FA code the seed under `key` in `vault` makes now, for a fill the person allowed**
/// (#5118, ruling SF-4, decision SF-11): the seed is read as [`fill_value`] reads a value, the
/// code worked out from it here ([`super::totp`]), and only the code leaves. Refused, in words
/// that hold nothing of the value, where the key holds no TOTP seed. `None` where the vault holds
/// no value under `key`, as for [`fill_value`].
pub fn fill_code(
    asker: &Asker,
    vault: &str,
    key: &str,
) -> Result<Option<purlis_session_protocol::browsing::Secret>, String> {
    let Some(value) = read_for_fill(asker, vault, key)? else {
        return Ok(None);
    };
    let seed = super::totp::Seed::parse(value.expose())
        .map_err(|why| format!("{key} in vault '{vault}' {why}, so purlis filled nothing"))?;
    drop(value);
    Ok(Some(purlis_session_protocol::browsing::Secret::new(
        seed.code_now(),
    )))
}

/// What a vault is read through for chat `asker`: the app's record of the chat.
fn reading_for(asker: &Asker) -> Ctx {
    match &asker.confines {
        Some(confines) => {
            Ctx::new(&asker.root, asker.env.clone()).chat(confines, asker.folder.as_deref())
        }
        None => Ctx::new(&asker.root, asker.env.clone()),
    }
}

/// `key` of `vault`, read for chat `asker`'s fill: the value whole, `None` where the vault holds
/// none under `key`, or why not, in words that hold none of it.
fn read_for_fill(
    asker: &Asker,
    vault: &str,
    key: &str,
) -> Result<Option<purlis_session_protocol::browsing::Secret>, String> {
    let ctx = reading_for(asker);
    let persona = asker.persona.as_deref().unwrap_or_default();
    let provider = cmd::provider(&ctx, vault).map_err(|e| e.message.to_string())?;
    if let Some(why) = read_out_of_reach(&ctx, persona, vault, &[key]) {
        return Err(why);
    }
    let got = cmd::get_value(&ctx, &provider, key).map(zeroize::Zeroizing::new);
    // And again once read: a link written meanwhile is seen, and what was read goes unused.
    if let Some(why) = read_out_of_reach(&ctx, persona, vault, &[key]) {
        return Err(why);
    }
    match got {
        // Moved out whole: what is left behind is wiped as it is dropped.
        Ok(mut value) => Ok(Some(purlis_session_protocol::browsing::Secret::new(
            std::mem::take(&mut *value),
        ))),
        // A key the vault holds no value under is the person's to add (#5116, #5054): told by
        // the provider's kind of failure, never by its sentence.
        Err(e) if e.kind == super::Kind::NotFound => Ok(None),
        Err(e) => Err(format!(
            "vault '{vault}' did not give {key}, so nothing was filled ({})",
            e.message
        )),
    }
}

/// `ready`'s values, read from its vault, and its identity. A key the vault holds no value
/// under is asked of the person through `missing`, as a brokered run's is (#5074, ADR 0109):
/// the read waits on the ask, and a decline, or no answer in time, fills nothing and says so.
fn read_login(
    asker: &Asker,
    name: &str,
    ready: &crate::sandbox::tools::Ready,
    ctx: &Ctx,
    missing: Option<&Missing<'_>>,
) -> Result<(purlis_session_protocol::browsing::LogIn, String), String> {
    use purlis_session_protocol::browsing::{LogIn, Secret};
    let (Some(login), Some((user_key, pass_key))) = (&ready.tool.login, ready.tool.login_keys())
    else {
        return Err(format!("{name} is not a browser login"));
    };
    let vault = &ready.tool.vault;
    let persona = asker.persona.as_deref().unwrap_or_default();
    let keys: Vec<&str> = user_key.into_iter().chain([pass_key]).collect();
    if let Some(why) = read_out_of_reach(ctx, persona, vault, &keys) {
        return Err(why);
    }
    let v = cmd::provider(ctx, vault).map_err(|e| e.message.to_string())?;
    let refused = |e: &super::VaultError| {
        format!(
            "vault '{vault}' did not give {name}'s login, so nothing was filled ({})",
            e.message
        )
    };
    let read_once = |key: &str| {
        let mut once = || cmd::get_value(ctx, &v, key);
        let Some(missing) = missing else {
            return once().map(Secret::new).map_err(|e| refused(&e));
        };
        let need = super::ask::Need {
            chat: Some(asker.chat),
            persona: asker.persona.clone().unwrap_or_default(),
            vault: vault.clone(),
            key: key.to_owned(),
            tool: Some(name.to_owned()),
            purpose: None,
            declared: super::needs::declared(
                &ctx.root,
                asker.persona.as_deref().unwrap_or_default(),
                key,
            )
            .map(|need| need.purpose),
            names_no_vault: false,
            reason: super::ask::Reason::broken(ctx, vault, key),
            no_allow: None,
        };
        missing
            .read_asking(need, &mut once)
            .map(Secret::new)
            .map_err(|unread| match unread {
                super::ask::Unread::Vault(e) => refused(&e),
                super::ask::Unread::Asked(line) => format!("{line}, so nothing was filled"),
            })
    };
    // Judged again once read: a link written meanwhile, a Fill's among them, is seen (#5795).
    let read = |key: &str| {
        let got = read_once(key);
        match read_out_of_reach(ctx, persona, vault, &[key]) {
            Some(why) => Err(why),
            None => got,
        }
    };
    let identity = identity_of_ready(asker, name, ready)?;
    let user = user_key.map(read).transpose()?;
    let password = read(pass_key)?;
    // A login's values follow a fill's rules (#5272), whoever declared it: the `mint` mark is by a
    // name in a vault, and one item may have two, and Always may have saved the line that names it.
    goes_into_a_page(asker, &password, vault, pass_key)?;
    if let (Some(user), Some(user_key)) = (&user, user_key) {
        goes_into_a_page(asker, user, vault, user_key)?;
    }
    Ok((
        LogIn {
            address: login.url.clone(),
            user,
            password,
            keep: None,
            identity: Some(identity.clone()),
            suggested: false,
            form: login.form.clone(),
            start: None,
        },
        identity,
    ))
}

/// **Whether chat `asker` may log in with its browser login `name` now** (#2022): every check
/// [`login_for`] makes, its vault's grant included, and no value read. Answers the login's
/// identity as declared and allowed now (#2699), the one [`login_and_identity_for`] answers: a
/// browser signed in with a login is handed only to a chat whose login has the same.
pub fn login_allowed(asker: &Asker, name: &str, unconfined: bool) -> Result<String, String> {
    let (ready, _) = login_ready(asker, name, unconfined)?;
    identity_of_ready(asker, name, &ready)
}

/// The identity of login `name`, declared and allowed for `asker` as `ready` (#2021): its
/// persona, its project (the plane's id, else its root) and its declaration.
fn identity_of_ready(
    asker: &Asker,
    name: &str,
    ready: &crate::sandbox::tools::Ready,
) -> Result<String, String> {
    let (Some(login), Some((user_key, pass_key))) = (&ready.tool.login, ready.tool.login_keys())
    else {
        return Err(format!("{name} is not a browser login"));
    };
    let project = crate::scaffold::planefile::id_to_hold_by(&asker.root)
        .unwrap_or_else(|_| asker.root.display().to_string());
    Ok(crate::browserlogins::identity_of(
        asker.persona.as_deref().unwrap_or_default(),
        &project,
        &crate::browserlogins::Declared {
            vault: &ready.tool.vault,
            user_key,
            password_key: pass_key,
            page: &login.url,
            form: login.form.as_deref(),
        },
    ))
}

/// [`login_for`]'s checks, short of reading a value: the login declared and allowed for the
/// chat, and its vault one the persona may use. Answers the tool and the context its values are
/// read in.
fn login_ready(
    asker: &Asker,
    name: &str,
    unconfined: bool,
) -> Result<(crate::sandbox::tools::Ready, Ctx), String> {
    let persona = asker.persona.as_deref().unwrap_or_default();
    let not_a_login = || {
        format!(
            "{name} is {persona}'s tool, not a browser login: run it as a command; \
             browser_login takes a tool declared with browser = \"https://…\""
        )
    };
    // A sandboxed chat's tools are what its start armed; one started without the sandbox,
    // which no start armed, is held to the same declarations and Allow, read now (#2560).
    let (armed, listed, then) = match &asker.confines {
        Some(confines) => (
            confines
                .tools
                .iter()
                .map(|ready| ready.tool.clone())
                .collect(),
            declared_now(&asker.root, persona),
            "Start the chat again once it is",
        ),
        // A chat a sandbox still holds whose start's record the app no longer keeps (a sandbox
        // that changed while no window was there, D-2006-11) is offered a restart, as it is
        // for the rest of its sandbox.
        None if !unconfined => {
            return Err(format!(
                "this chat's sandbox changed since it started, so purlis performs no browser \
                 login for it until it starts again: restart the chat, then call browser_login \
                 with {name} again"
            ));
        }
        None if asker.persona.as_deref().is_some_and(|p| !p.is_empty()) => {
            let (declared, allowed) =
                crate::sandbox::tools::declared_and_allowed_here(&asker.root, persona);
            (allowed, declared, "Call browser_login again once it is")
        }
        None => (Vec::new(), Vec::new(), ""),
    };
    let Some(tool) = armed.into_iter().find(|tool| tool.name == name) else {
        return Err(match listed.iter().find(|tool| tool.name == name) {
            Some(tool) if tool.login.is_some() => format!(
                "{name} is {persona}'s browser login, and it is not allowed on this machine for \
                 this chat: it logs in only once {persona}'s hosts and tools are allowed here \
                 (the project's Notice, or Settings › Sandbox) and policy allows persona tools. \
                 {then} ({})",
                tool.whole()
            ),
            Some(_) => not_a_login(),
            None if persona.is_empty() => format!(
                "this chat runs as no persona, so it has no browser login {name}, and nothing \
                 was filled"
            ),
            None => format!(
                "{persona} declares no browser login {name}, so nothing was filled. Call \
                 browser_login again with site, vault and key (and user_key if the page asks for \
                 a user name), and the person is asked"
            ),
        });
    };
    let ready = crate::sandbox::tools::Ready {
        tool,
        program: PathBuf::new(),
    };
    if ready.tool.login.is_none() || ready.tool.login_keys().is_none() {
        return Err(not_a_login());
    }
    let ctx = match &asker.confines {
        Some(confines) => {
            Ctx::new(&asker.root, asker.env.clone()).chat(confines, asker.folder.as_deref())
        }
        None => Ctx::new(&asker.root, asker.env.clone()),
    };
    authorise(&ctx, asker.persona.as_deref(), &ready.tool.vault)?;
    Ok((ready, ctx))
}

/// What a persona's browser login run as a command is refused with (#2016): purlis performs
/// the login in the chat's own browser, through the browser tools.
pub fn login_by_the_browser_tool(name: &str) -> String {
    format!(
        "{name} is a browser login, and purlis performs it in this chat's own browser: call the \
         browser_login tool with login = \"{name}\". Nothing ran, and no session file is written"
    )
}

/// `persona`'s tools as the project at `root` lists them now, under its policy: what a refusal
/// of an undeclared command names as declared but not armed in the chat.
fn declared_now(root: &Path, persona: &str) -> Vec<crate::sandbox::tools::Tool> {
    crate::sandbox::Plane::read(root)
        .in_force(&crate::sandbox::policy::Locks::of(root))
        .and_then(|policy| {
            policy
                .personas
                .get(persona)
                .map(|grants| grants.tools.clone())
        })
        .unwrap_or_default()
}

/// The block a host the run's proxy refused is, for the Notice of the chat `asker` is: a
/// connection to a host, the chat's own, naming `target` (`host:port`) whole, with the chat's
/// harness and the run's `vault` (#1353).
fn refused_block(asker: &Asker, vault: &str, target: String) -> RunBlocked {
    use crate::sandboxblock::{Block, Kind, Operation};
    RunBlocked {
        block: crate::hookwire::SandboxBlocked {
            chat: asker.chat,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::Host,
                ours: false,
            },
            harness: asker.harness.map(|harness| harness.name().to_owned()),
            target: Some(target),
            read_back: false,
            suggested: None,
        },
        vault: vault.to_owned(),
    }
}

/// What purlis says on the command's stderr of a host its proxy refused (`host:port`): that the
/// sandbox refused it, and where the person allows it. Not in the words a chat's own block hook
/// reads (`crate::sandboxblock`), so one refusal raises one Notice, from the proxy's record.
pub fn refused_note(target: &str) -> String {
    format!(
        "purlis's sandbox refused this command a connection to {target}, which this chat's \
         hosts do not list. It is the sandbox, not the server. The person can allow it in the \
         Inbox, or on this chat's pane when it is on screen (Allow for this chat, or Always \
         allow); the chat is then started again with it, and the command can be run again."
    )
}

/// What a run says of the hosts its proxy refused past the few it keeps and names (#1353): how
/// many, so a ninth is never hidden behind the eight. Never their names: they are not kept.
pub fn more_refused_note(more: usize) -> Option<String> {
    use crate::sandbox::egress::{MORE_COUNTED_AT_MOST, REFUSALS_KEPT};
    let hosts = match more {
        0 => return None,
        1 => "1 more host".to_owned(),
        n if n >= MORE_COUNTED_AT_MOST => format!("{n} or more hosts"),
        n => format!("{n} more hosts"),
    };
    Some(format!(
        "purlis's sandbox also refused this command connections to {hosts} that this chat's \
         hosts do not list. purlis keeps only the first {REFUSALS_KEPT} it refuses, so these \
         are neither named nor offered to the person."
    ))
}

/// Whether `target`, a host the run's proxy refused, carries any of the run's `values`, as the
/// mask would find it or in another case (a host is the same host in any case).
fn carries_a_value(target: &str, values: &[String]) -> bool {
    let target = target.to_lowercase();
    values
        .iter()
        .filter(|value| !value.is_empty())
        .any(|value| target.contains(&value.to_lowercase()))
}

/// What purlis says instead of naming a refused host that carries one of the run's values:
/// nothing of the host, and no Notice offers it.
pub const WITHHELD_NOTE: &str = "purlis's sandbox refused this command a connection to a host \
                                 that carries a value from the vault, so purlis does not name \
                                 it and offers no way to allow it.";

/// Writes `frame` on `writer` as one line.
fn send(writer: &mut dyn Write, frame: &Frame) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(frame).map_err(std::io::Error::other)?;
    line.push(b'\n');
    writer.write_all(&line)?;
    writer.flush()
}

/// Refuses the ask on `writer` with `why`: nothing was run.
pub fn refused(mut writer: Box<dyn Write + Send>, why: String) {
    let _ = send(&mut *writer, &Frame::Refused { why, code: 1 });
}

/// The status of a refusal that may pass if asked again shortly: the chat is not one the app
/// has open yet (qw381). `EX_TEMPFAIL`, as `sysexits.h` names it. Every other refusal is the
/// same however often it is asked, and a persona's MCP server's start is not tried again for it.
pub const NOT_YET: i32 = 75;

/// [`refused`], for a refusal that may pass if asked again shortly ([`NOT_YET`]).
pub fn refused_for_now(mut writer: Box<dyn Write + Send>, why: String) {
    let _ = send(&mut *writer, &Frame::Refused { why, code: NOT_YET });
}

/// What an app with nothing that runs a brokered command answers, so the asker runs it itself.
pub fn not_answered(mut writer: Box<dyn Write + Send>) {
    let _ = send(
        &mut *writer,
        &Frame::Refused {
            why: crate::hookwire::NOTHING_ANSWERS.to_owned(),
            code: 1,
        },
    );
}

/// The variables of the asker's environment a child never starts with: the chat's hook channel,
/// its token and its prompt key (the child cannot reach the socket, and needs none of them), and
/// the ones the wrap sets itself. Each under its old prefix too ([`not_handed_on`]).
const NOT_HANDED_ON: &[&str] = &[
    crate::hookwire::SOCKET_ENV,
    crate::hookwire::TOKEN_ENV,
    crate::hookwire::PROMPT_KEY_ENV,
];

/// Whether `key` is one of [`NOT_HANDED_ON`], under the purlis prefix or an old one the window
/// still reads ([`crate::names::ENV_PREFIX`]).
fn not_handed_on(key: &str) -> bool {
    let prefix = &crate::names::ENV_PREFIX;
    NOT_HANDED_ON.iter().any(|name| {
        *name == key
            || name.strip_prefix(prefix.write).is_some_and(|rest| {
                prefix
                    .reads
                    .iter()
                    .any(|old| key.strip_prefix(old) == Some(rest))
            })
    })
}

pub(crate) fn serve_in(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    wrap: Wrap,
    told: Told,
) {
    serve_recorded(asker, wanted, None, reader, writer, wrap, told, None, None);
}

/// **A runner chat's declared tool, run by the runner's own host on the values its desktop
/// sent** (#2047, ADR 0078 §7): exactly as [`serve`] runs one for a chat of this machine, held
/// to what the chat's own sandbox was compiled to here and to the tools its start armed here,
/// but with no vault read: each value comes from `handed`, which the desktop's window read and
/// sent for this one run after its own gate. Nothing of `handed` is kept: a `--file` value is a
/// file removed as the run ends, and the values go with this call.
pub fn serve_handed(
    asker: &Asker,
    wanted: Wanted,
    handed: Handed,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    told: Told,
) {
    serve_recorded(
        asker,
        wanted,
        Some(&handed),
        reader,
        writer,
        Wrap::here(),
        told,
        None,
        None,
    );
}

/// **The value a minting tool's run is handed for `key`** (#2053): the token minted from the
/// tool's one key, and nothing for any other key.
fn minted_for(
    tool: &crate::sandbox::tools::Tool,
    key: &str,
    token: &zeroize::Zeroizing<String>,
) -> Result<zeroize::Zeroizing<String>, super::VaultError> {
    if tool.values.iter().any(|value| value.key == key) {
        super::seen::remember(token);
        Ok(token.clone())
    } else {
        Err(super::VaultError::new(format!(
            "{} is handed a GitHub App's token alone, and no value of key {key}",
            tool.name
        )))
    }
}

/// [`serve_handed`] in `wrap`, telling nothing of what its proxy refused.
#[cfg(test)]
pub(crate) fn serve_handed_wrapped(
    asker: &Asker,
    wanted: Wanted,
    handed: Handed,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    wrap: Wrap,
) {
    serve_recorded(
        asker,
        wanted,
        Some(&handed),
        reader,
        writer,
        wrap,
        Arc::new(|_| {}),
        None,
        None,
    );
}

/// **Which of the tools chat `asker`'s start armed here `wanted` runs as** (#2047): by its name,
/// where the command is exactly one of them, with that tool's vault and hand-over
/// ([`crate::sandbox::tools::declared`]); refused in the words a run here would be refused in
/// otherwise. What a runner's host says to its desktop before any value is sent for the run, so
/// nothing is asked for a command its own sandbox would never run.
pub fn armed_as(asker: &Asker, wanted: &Wanted) -> Result<String, String> {
    let Some(confines) = &asker.confines else {
        return Err(
            "this chat was not started in its sandbox here, so no vault value is sent \
                    for its commands"
                .to_owned(),
        );
    };
    let persona = asker.persona.as_deref().unwrap_or_default();
    let program = wanted
        .command
        .iter()
        .find(|word| *word != "--")
        .cloned()
        .unwrap_or_default();
    let listed = declared_now(&asker.root, persona);
    // A tool this runner's image has no program for says so, before anything is asked (#4354).
    if let Some(why) =
        crate::sandbox::tools::absent_from_the_image(&confines.tools, &listed, persona, &program)
    {
        return Err(why);
    }
    let ready = crate::sandbox::tools::declared(
        &confines.tools,
        &listed,
        persona,
        &crate::sandbox::tools::Asked {
            vault: &wanted.vault,
            env: &wanted.env,
            file: &wanted.file,
            dotenv: &wanted.dotenv,
            program: &program,
        },
    )?;
    if ready.tool.login.is_some() {
        return Err(login_by_the_browser_tool(&ready.tool.name));
    }
    // The run's own arguments (#3421), judged here so no value is sent for a run its host
    // would refuse.
    let args: Vec<String> = wanted
        .command
        .iter()
        .skip_while(|word| *word == "--")
        .skip(1)
        .cloned()
        .collect();
    crate::sandbox::tools::allows(ready, &args)?;
    // And the repository it runs in (#3868), on the host it runs on.
    if let Some(folder) = wanted
        .cwd
        .clone()
        .filter(|cwd| cwd.is_absolute() && cwd.is_dir())
        .or_else(|| asker.folder.clone())
    {
        crate::sandbox::tools::reads_held_repository(
            ready,
            &args,
            &folder,
            &asker.root,
            &chat_writes(asker, confines),
        )?;
    }
    Ok(ready.tool.name.clone())
}

/// **The values a runner's desktop sent for one `secret exec`** (#2047): each key of the
/// tool's vault the run hands over, with its value, read by the desktop's window after its gate
/// and sent down the link for that one run, **bound to that run**: the chat, the tool its host
/// said it runs, and the run's own number its host made ([`Ask::run`]), which its host checks
/// before it runs anything on them. Each value is wiped from memory as it is dropped
/// ([`zeroize::Zeroizing`]); its `Debug` names the keys alone, and it is never written anywhere.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Handed {
    pub chat: u32,
    pub tool: String,
    pub run: String,
    values: Vec<(String, zeroize::Zeroizing<String>)>,
    /// A token this runner's own host had from the cluster's operator (#4332). Never read off the
    /// link, so nothing a desktop sends is recorded as the cluster's.
    from_the_cluster: bool,
}

/// [`Handed`] as it crosses the link.
#[derive(serde::Serialize, serde::Deserialize)]
struct HandedLine<V> {
    chat: u32,
    tool: String,
    run: String,
    values: Vec<(String, V)>,
}

impl serde::Serialize for Handed {
    fn serialize<S: serde::Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        HandedLine {
            chat: self.chat,
            tool: self.tool.clone(),
            run: self.run.clone(),
            values: self
                .values
                .iter()
                .map(|(key, value)| (key.clone(), value.as_str()))
                .collect(),
        }
        .serialize(to)
    }
}

impl<'de> serde::Deserialize<'de> for Handed {
    fn deserialize<D: serde::Deserializer<'de>>(from: D) -> Result<Self, D::Error> {
        let line = HandedLine::<String>::deserialize(from)?;
        Ok(Handed {
            chat: line.chat,
            tool: line.tool,
            run: line.run,
            from_the_cluster: false,
            values: line
                .values
                .into_iter()
                .map(|(key, value)| (key, zeroize::Zeroizing::new(value)))
                .collect(),
        })
    }
}

impl std::fmt::Debug for Handed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handed")
            .field("chat", &self.chat)
            .field("tool", &self.tool)
            .field("keys", &self.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl Handed {
    /// The values `values`, each a key and its value, for run `run` of chat `chat`'s tool `tool`.
    /// Each value is taken as it is, never copied: one read into a [`zeroize::Zeroizing`] stays
    /// the one copy (#3420).
    pub fn of<V: Into<zeroize::Zeroizing<String>>>(
        chat: u32,
        tool: &str,
        run: &str,
        values: impl IntoIterator<Item = (String, V)>,
    ) -> Handed {
        Handed {
            chat,
            tool: tool.to_owned(),
            run: run.to_owned(),
            values: values
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect(),
            from_the_cluster: false,
        }
    }

    /// [`Handed::of`], for a token the cluster's operator minted for this host (#4332): its record
    /// says so, and names no request, since no desktop sent it.
    pub fn from_the_cluster<V: Into<zeroize::Zeroizing<String>>>(
        chat: u32,
        tool: &str,
        run: &str,
        values: impl IntoIterator<Item = (String, V)>,
    ) -> Handed {
        Handed {
            from_the_cluster: true,
            ..Handed::of(chat, tool, run, values)
        }
    }

    /// The keys it carries a value for.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.iter().map(|(key, _)| key.as_str())
    }

    /// Whether these are the values for run `run` of chat `chat`'s tool `tool`, and no other.
    pub fn is_for(&self, chat: u32, tool: &str, run: &str) -> bool {
        !run.is_empty() && self.chat == chat && self.tool == tool && self.run == run
    }

    /// The value sent for `key`, remembered as read ([`super::seen`]) so no store of a chat's
    /// words on this host keeps it; refused where the desktop sent none. Its copy is wiped as it
    /// is dropped (#3420).
    fn value(&self, key: &str) -> Result<zeroize::Zeroizing<String>, super::VaultError> {
        let (_, value) = self
            .values
            .iter()
            .find(|(one, _)| one == key)
            .ok_or_else(|| {
                super::VaultError::new(format!(
                    "the person's desktop sent no value for key {key}, so nothing ran"
                ))
            })?;
        super::seen::remember(value);
        Ok(value.clone())
    }
}

/// **Whether a brokered run may start with `name` set**, from the chat's environment or from an
/// approved server's entry (#3441): never purlis's own hook channel, a variable that makes a
/// program load code it names ([`crate::sandbox::tools::loads_code`]), or one that makes it write
/// out its memory ([`DUMPS_MEMORY`]).
pub fn may_hand_on(name: &str) -> bool {
    !not_handed_on(name)
        && !crate::sandbox::tools::loads_code(name)
        && !DUMPS_MEMORY.contains(&name)
}

/// Variables of the chat's that make a program write out its memory as it crashes, or change
/// how it does: a run starts without them, so its values are never written that way (#2047).
const DUMPS_MEMORY: &[&str] = &["GOTRACEBACK", "GODEBUG"];

/// **No process of this one, a run's child among them, writes a core file** (#2047): its soft
/// limit set to nothing, the hard one kept. A core file of a run would hold the run's values.
/// Inherited by every child from here on, and by this process itself, which holds values too.
#[cfg(unix)]
fn no_core_files() {
    use nix::sys::resource::{Resource, getrlimit, setrlimit};
    let hard = getrlimit(Resource::RLIMIT_CORE).map_or(0, |(_, hard)| hard);
    if let Err(why) = setrlimit(Resource::RLIMIT_CORE, 0, hard) {
        tracing::warn!("purlis: core files could not be turned off for a brokered run ({why})");
    }
}

#[cfg(not(unix))]
fn no_core_files() {}

/// **A fresh number for one run of a chat's `secret exec`** (#2047): 128 random bits, in hex,
/// made by the host that hands the line on and checked against what comes back for it, so values
/// sent for one run never reach another, whatever numbers the lines took. `None` where no random
/// bytes could be had: then no values are taken for the run.
pub fn a_run_number() -> Option<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// **The event a desktop records each run's values it sent to a runner under** (#3406, ADR 0078
/// §10, its registry row in ADR 0075 as amended there): on the desktop's own record, where the
/// runner records the run itself as `secret-exec`.
pub const FORWARDED: &str = "secret.forwarded";

/// **What the desktop's record of values it sent names beside the run's own** ([`Forwarded`]).
pub struct Sent<'a> {
    /// The persona whose tool it is.
    pub persona: &'a str,
    /// The vault the values were read from.
    pub vault: &'a str,
    /// The pool whose runner they went to.
    pub pool: &'a str,
    /// That runner's device id.
    pub runner: &'a str,
}

/// **The request a run's values answer, as both chains name it** (#3406): the first 8 bytes of
/// the SHA-256 of the run's number, in hex. The desktop's [`FORWARDED`] and the runner's
/// `secret-exec` say the same, so each is found from the other; the number itself is never
/// written.
pub fn request_of(run: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(run.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// **What the desktop records of values it sends a runner for one run** (#3406), taken from
/// them before they go: the chat, the tool, the keys and the request ([`request_of`]); never a
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forwarded {
    chat: u32,
    tool: String,
    keys: Vec<String>,
    request: String,
}

impl Forwarded {
    /// What `handed` is recorded by.
    pub fn of(handed: &Handed) -> Forwarded {
        let mut keys: Vec<String> = handed.keys().map(str::to_owned).collect();
        keys.sort();
        keys.dedup();
        Forwarded {
            chat: handed.chat,
            tool: handed.tool.clone(),
            keys,
            request: request_of(&handed.run),
        }
    }

    /// **Records on this desktop that the values were sent** (#3406): [`FORWARDED`] under the
    /// chat's own session in project `root`, naming the vault, the keys, the runner's device id,
    /// its pool, the chat, its persona, the tool and the request. Kept by the 30-day sweep, as
    /// every hand-out is, until it is audited (V71).
    pub fn record(&self, root: &Path, sent: &Sent<'_>) {
        let chat = self.chat.to_string();
        let ctx = Ctx::new(root, Env::of(&[(crate::active::SESSION_ID_ENV, &chat)]));
        cmd::trace_secret_use(
            &ctx,
            // [`FORWARDED`], written as a literal: the sweep's test finds every hand-out's
            // event by the literal each call names.
            "secret.forwarded",
            &[],
            &[
                ("vault", Value::String(sent.vault.to_owned())),
                (
                    "key_names",
                    Value::Array(self.keys.iter().cloned().map(Value::String).collect()),
                ),
                ("runner", Value::String(sent.runner.to_owned())),
                ("pool", Value::String(sent.pool.to_owned())),
                ("chat", Value::from(self.chat)),
                ("persona", Value::String(sent.persona.to_owned())),
                ("tool", Value::String(self.tool.clone())),
                ("request", Value::String(self.request.clone())),
            ],
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn serve_recorded(
    asker: &Asker,
    wanted: Wanted,
    handed: Option<&Handed>,
    reader: Box<dyn BufRead + Send>,
    mut writer: Box<dyn Write + Send>,
    wrap: Wrap,
    told: Told,
    reached: Option<crate::sandbox::egress::Reached>,
    missing: Option<&Missing<'_>>,
) {
    // Counted as soon as the run is served: a chat that ends from here on keeps no GitHub App
    // token minted for it (#3808).
    let ends_seen = asker
        .ends_seen
        .unwrap_or_else(crate::forge::apptoken::ends_so_far);
    // A runner's journal names the program, never a vault (#2838): the refusal's words name
    // vaults, so a service logs neither.
    let program = wanted
        .command
        .iter()
        .find(|word| *word != "--")
        .cloned()
        .unwrap_or_default();
    let refuse = |writer: &mut dyn Write, code: i32, why: String| {
        if crate::applog::is_a_service() {
            tracing::warn!(
                "purlis: chat {} was refused `secret exec` of `{program}`",
                asker.chat
            );
        } else {
            tracing::warn!(
                "purlis: chat {} was refused `secret exec` on vault '{}': {why}",
                asker.chat,
                wanted.vault
            );
        }
        let _ = send(writer, &Frame::Refused { why, code });
    };
    let ctx = Ctx::new(&asker.root, asker.env.clone());
    let mut command = wanted.command.clone();
    if command.first().is_some_and(|c| c == "--") {
        command.remove(0);
    }
    if command.is_empty() {
        return refuse(&mut *writer, 2, exec::NO_COMMAND.to_owned());
    }
    let Some(confines) = &asker.confines else {
        // A chat the app did not start sandboxed reads its vault as the person does: the line
        // claimed a sandbox the chat does not have, and the command runs where it was asked.
        tracing::info!(
            "purlis: chat {} asked for a brokered `secret exec` and was not started sandboxed, \
             so it runs the command itself",
            asker.chat
        );
        return not_answered(writer);
    };
    // A Claude Code chat on Linux on its own sandbox hands a value by file alone: refused before
    // anything is read.
    if let Some(why) = env_refused(asker.harness, confines.whole, wrap, &wanted) {
        return refuse(&mut *writer, 1, why.to_owned());
    }
    // What this chat may write is the app's record of it, and no provider's program is run
    // from there (#1516).
    let ctx = ctx.chat(confines, asker.folder.as_deref());
    // Values its desktop sent were authorised there, where the vault is (#2047): a runner holds
    // no vault to authorise against. A vault the persona may not use may yet be opened by a Run
    // grant, for that grant's scope alone: judged once the command is, below.
    let mut vault_refused = if handed.is_none() {
        authorise(&ctx, asker.persona.as_deref(), &wanted.vault).err()
    } else {
        None
    };
    // A sandboxed chat runs only its persona's declared tools with a vault (D-PE-ruling-2, the
    // operator's ruling of 2026-10-12): exactly a tool its start armed, allowed on this machine,
    // with that tool's vault and hand-over. The run is of the tool's own program, by the
    // absolute path the app found where no chat writes, never one the chat's `PATH` names.
    let persona = asker.persona.as_deref().unwrap_or_default();
    let listed = declared_now(&asker.root, persona);
    let mut declared_as: Option<String> = None;
    // A tool whose token a GitHub App mints (#2053): handed the token, never the key.
    let mut minting: Option<crate::sandbox::tools::Tool> = None;
    // The repository a declared tool's run reads (#3868), judged with its arguments.
    let mut its_repository: Option<crate::sandbox::tools::Ready> = None;
    // A forge tool's host, told to its run (#3869).
    let mut forge_env: Vec<(String, String)> = Vec::new();
    // The variables an approved server's run starts with beside
    // [`crate::sandbox::tools::a_run_starts_from`]'s: those its approved entry sets, with the
    // values the person approved (#3441).
    let mut its_entry_sets: Vec<(String, String)> = Vec::new();
    let declared = crate::sandbox::tools::declared(
        &confines.tools,
        &listed,
        persona,
        &crate::sandbox::tools::Asked {
            vault: &wanted.vault,
            env: &wanted.env,
            file: &wanted.file,
            dotenv: &wanted.dotenv,
            program: &command[0],
        },
    );
    // Only a run a person can allow gets past a vault its persona may not use: never a server's,
    // and never where policy locks a vault to its persona (ADR 0112).
    let may_ask_for_the_vault = missing.is_some()
        && wanted.mcp.is_none()
        && not_tagged(&ctx, asker.persona.as_deref(), &wanted.vault).is_some()
        && !crate::sandbox::policy::Locks::of(&asker.root).forbids_vault_grants()
        && an_approved_server(&asker.root, persona, &wanted, &command).is_none();
    if let Some(why) = &vault_refused
        && !may_ask_for_the_vault
    {
        return refuse(&mut *writer, 1, why.clone());
    }
    // Every key it hands over that is a link into a vault the persona may not use refuses the
    // run before anything is read or asked (#5795): no notice for one key, then a refusal for
    // another. Each read is judged again as it happens, below.
    if handed.is_none()
        && let Some(why) = read_out_of_reach(
            &ctx,
            persona,
            &wanted.vault,
            &keys_handed(&wanted).collect::<Vec<_>>(),
        )
    {
        return refuse(&mut *writer, 1, why);
    }
    // A declared tool's run on a vault its persona may not use, that lacks a key too: one notice
    // with its fields and the vault's own Allow (#5242). Lacking none, the app's refused vault's
    // notice asks, as before (#5160).
    if let (Some(why), Ok(ready), Some(missing)) = (&vault_refused, &declared, missing) {
        use super::ask::run::{Offer, RunHeld};
        if missing.run_gate.is_some_and(|gate| gate.kept_blocked) {
            return refuse(&mut *writer, 1, why.clone());
        }
        let fills = fills_missing(asker, &reading_for(asker), &wanted);
        if fills.is_empty() {
            return refuse(&mut *writer, 1, why.clone());
        }
        if let Some(refused) = fills_refused(asker, &wanted.vault, &fills) {
            return refuse(&mut *writer, 1, refused);
        }
        let bound = bound_of(asker, &wanted, &command, ready);
        let need = need_of(
            asker,
            confines,
            &command,
            ready,
            bound,
            fills,
            Offer::Vault { no_allow: None },
            true,
        );
        let mut say = |line: &str| {
            let _ = send(&mut *writer, &Frame::Note(line.to_owned()));
        };
        match held_on(missing, need, &mut say) {
            Ok(RunHeld::Allowed) => {}
            Ok(_) => return refuse(&mut *writer, 1, why.clone()),
            Err((code, why)) => return refuse(&mut *writer, code, why),
        }
        // The person gave the persona the vault: it is read as any of its own now.
        match authorise(&ctx, asker.persona.as_deref(), &wanted.vault) {
            Ok(()) => vault_refused = None,
            Err(why) => return refuse(&mut *writer, 1, why),
        }
    }
    // Never past it otherwise: a declared tool reads only a vault its persona may use.
    if let (Some(why), Ok(_)) = (&vault_refused, &declared) {
        return refuse(&mut *writer, 1, why.clone());
    }
    let armed_as_a_tool = declared.is_ok();
    // What a Run grant this run goes under binds: taken as the run starts, never sooner (#5242).
    let mut grant_bound: Option<super::ask::run::Bound> = None;
    let asks_of_grants = missing.map(|missing| missing.asks);
    let its_tool = match declared {
        // A browser login is purlis's to perform in the chat's own browser (#2016): run as a
        // command, it runs nothing.
        Ok(ready) if ready.tool.login.is_some() => {
            return refuse(&mut *writer, 2, login_by_the_browser_tool(&ready.tool.name));
        }
        // Any other tool, its own program.
        Ok(ready) => Some(ready.clone()),
        // A persona's credentialed MCP server this machine approved, started by the chat's
        // harness, asks for its line as approved: the person's consent, as an Allow is.
        Err(why) => match an_approved_server(&asker.root, persona, &wanted, &command) {
            Some(sets) => {
                its_entry_sets = sets;
                None
            }
            // No tool: the person may allow this program, here and now (ADR 0112, #5239), where
            // somebody is there to be asked. A runner's values were sent for a tool it declared.
            None => match missing.filter(|_| handed.is_none()) {
                None => return refuse(&mut *writer, 1, why),
                Some(missing) => {
                    let mut say = |line: &str| {
                        let _ = send(&mut *writer, &Frame::Note(line.to_owned()));
                    };
                    let not_its_vault = vault_refused.is_some();
                    let why = vault_refused.clone().unwrap_or(why);
                    match run_granted(
                        asker,
                        confines,
                        &wanted,
                        &command,
                        missing,
                        not_its_vault,
                        why,
                        &mut say,
                    ) {
                        Ok((ready, bound)) => {
                            grant_bound = Some(bound);
                            Some(ready)
                        }
                        Err((code, why)) => return refuse(&mut *writer, code, why),
                    }
                }
            },
        },
    };
    if let Some(ready) = &its_tool {
        match crate::sandbox::tools::run_words(ready, &command[1..]) {
            Ok(words) => {
                declared_as = ready.argv0().map(str::to_owned);
                minting = ready.tool.mint.is_some().then(|| ready.tool.clone());
                command = words;
                its_repository = Some(ready.clone());
                forge_env = crate::sandbox::tools::forge_env(ready);
            }
            Err(why) => return refuse(&mut *writer, 1, why),
        }
    }
    // A person allowed what they read, and a chat's stdin is not in that: a run under a grant has
    // none. A program told to read its settings from there reads nothing.
    let under_a_grant = its_tool.is_some() && !armed_as_a_tool;
    // What policy is asked about again, should the run wait on a key: only its keys are read.
    let locked_since = under_a_grant.then(|| Wanted {
        env: wanted.env.clone(),
        file: wanted.file.clone(),
        ..Wanted::default()
    });
    // A vault opened by a Run grant is read, never filled: storing in it is its persona's.
    let missing = missing.filter(|_| vault_refused.is_none());
    // At most a few at once per chat: each holds a proxy, a temp directory and a process group.
    let Some(_slot) = Slot::take(&asker.root, asker.chat) else {
        return refuse(
            &mut *writer,
            1,
            format!(
                "this chat already has {RUNS_AT_ONCE} commands running through purlis with a \
                 vault; wait for one to end, then run this again"
            ),
        );
    };
    let Some(folder) = asker.folder.clone().filter(|f| f.is_dir()) else {
        return refuse(
            &mut *writer,
            1,
            "this chat has no folder purlis knows of, so there is no sandbox to run the command \
             in"
            .to_owned(),
        );
    };
    // On Linux the run's sandbox is purlis's own program, as the chat's start held it, checked
    // again now: never one the chat may write, nor one replaced since.
    let applying = match wrap {
        Wrap::Nothing => return refuse(&mut *writer, 1, NO_WRAP.to_owned()),
        Wrap::Seatbelt => Applying::Seatbelt,
        Wrap::Purlis => match wrap_for(asker, confines, &folder) {
            Ok(program) => Applying::Purlis(program),
            Err(why) => return refuse(&mut *writer, 1, why),
        },
        #[cfg(test)]
        Wrap::Unwrapped | Wrap::Refusing(_) => Applying::Unwrapped,
    };
    // A declared tool reads only the person's repository configuration where it runs, and its
    // git is held to the repository judged (#3868).
    let mut held_to: Vec<(String, String)> = Vec::new();
    if let Some(ready) = &its_repository {
        let start_in = wanted
            .cwd
            .clone()
            .filter(|cwd| cwd.is_absolute() && cwd.is_dir())
            .unwrap_or_else(|| folder.clone());
        match crate::sandbox::tools::reads_held_repository(
            ready,
            &command[1..],
            &start_in,
            &asker.root,
            &chat_writes(asker, confines),
        ) {
            Ok(env) => held_to = env,
            Err(why) => return refuse(&mut *writer, 1, why),
        }
    }
    // A declared tool's run, or one a grant covers, that lacks more than one key: one notice for
    // all of them (#5242). One key alone is asked as before, as the run reads.
    if let (Some(missing), Some(ready), None, None) = (missing, &its_tool, handed, &minting) {
        use super::ask::run::{Offer, RunHeld};
        let fills = fills_missing(asker, &ctx, &wanted);
        if fills.len() > 1 {
            if let Some(refused) = fills_refused(asker, &wanted.vault, &fills) {
                return refuse(&mut *writer, 1, refused);
            }
            let bound = grant_bound
                .clone()
                .unwrap_or_else(|| bound_of(asker, &wanted, &command, ready));
            let need = need_of(
                asker,
                confines,
                &command,
                ready,
                bound,
                fills,
                Offer::Fill { no_allow: None },
                false,
            );
            let mut say = |line: &str| {
                let _ = send(&mut *writer, &Frame::Note(line.to_owned()));
            };
            match held_on(missing, need, &mut say) {
                Ok(RunHeld::Filled | RunHeld::Allowed) => {}
                Ok(_) => return refuse(&mut *writer, 1, "nothing was filled".to_owned()),
                Err((code, why)) => return refuse(&mut *writer, code, why),
            }
            if under_a_grant
                && let Some(why) = handed_to_no_granted_program(
                    &ctx,
                    asker.persona.as_deref(),
                    &wanted.vault,
                    &keys_named(&wanted).collect::<Vec<_>>(),
                )
            {
                return refuse(&mut *writer, 1, why);
            }
        }
    }
    let v = match handed {
        Some(_) => None,
        None => match cmd::provider(&ctx, &wanted.vault) {
            Ok(v) => Some(v),
            Err(e) => return refuse(&mut *writer, 1, e.message.to_string()),
        },
    };
    // What runs beside the child for as long as it does: the egress proxy carrying the chat's
    // hosts, telling the chat's tab each host it refuses, and a temp directory of its own.
    // A host that carries one of the run's values is never told: no Notice names it and no
    // grant is made from it. The values are known once the run is prepared, which is after its
    // proxy starts; until then nothing is told.
    let values: Arc<std::sync::OnceLock<zeroize::Zeroizing<Vec<String>>>> = Arc::default();
    let refusals = crate::sandbox::egress::Refusals::telling({
        let asker = asker.clone();
        let vault = wanted.vault.clone();
        let values = Arc::clone(&values);
        Arc::new(move |host: &str, port: u16| {
            let target = crate::sandbox::egress::host_and_port(host, port);
            if values
                .get()
                .is_some_and(|values| !carries_a_value(&target, values))
            {
                told(refused_block(&asker, &vault, target));
            }
        })
    });
    // Each connection the run makes, told under the chat; a host that carries one of the
    // run's values counted with no name, as the record never keeps a value.
    let reached = reached.map(|reached| -> crate::sandbox::egress::Reached {
        let values = Arc::clone(&values);
        Arc::new(move |target: Option<&str>, by: &'static str, times: u64| {
            let named = target.filter(|target| {
                values
                    .get()
                    .is_some_and(|values| !carries_a_value(target, values))
            });
            reached(named, by, times);
        })
    });
    let beside = match Beside::start(wrap, confines, refusals.clone(), reached.clone()) {
        Ok(beside) => beside,
        Err(e) => {
            return refuse(
                &mut *writer,
                1,
                format!("purlis could not start the sandbox the command runs in ({e})"),
            );
        }
    };
    // The asker's environment, less the hook channel and what makes a program load code
    // ([`crate::sandbox::tools::loads_code`]), then the wrap's own (its proxy and its
    // temp directory), then every other vault's identity variables taken out. A run starts
    // from only the few of the chat's variables it needs, with a home of its own in the run's
    // temp directory (#2047), and an approved server's with those its approved entry sets too,
    // at the values the person approved, never the chat's (#3441).
    // A name is judged as it reads, with any byte that is not UTF-8 replaced, so one is never
    // let through for not spelling the name it is judged by.
    // An approved value is held to the same names a chat's is ([`may_hand_on`]).
    // Under a grant, where it connects and whom it trusts is what the command says, which the
    // person read ([`crate::sandbox::tools::a_granted_run_starts_from`]).
    let starts_from = if under_a_grant {
        crate::sandbox::tools::a_granted_run_starts_from
    } else {
        crate::sandbox::tools::a_run_starts_from
    };
    let mut base: Vec<(OsString, OsString)> = wanted
        .environment
        .into_iter()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            may_hand_on(&k) && starts_from(&k)
        })
        .collect();
    for (k, v) in its_entry_sets.into_iter().filter(|(k, _)| may_hand_on(k)) {
        exec::set_var(&mut base, &k, v);
    }
    // What the tool runs by a bare name is never from a folder the chat may write
    // (D-PE-ruling-2): the project, the chat's folder, its grants and the temporary folders
    // are taken off the `PATH` it starts from.
    if let Some(path) = base
        .iter()
        .rev()
        .find(|(k, _)| k == "PATH")
        .and_then(|(_, v)| v.to_str())
        .map(str::to_owned)
    {
        exec::set_var(
            &mut base,
            "PATH",
            crate::sandbox::tools::path_outside(&path, &asker.root, &chat_writes(asker, confines)),
        );
    }
    // At the floor (#2071) the run's proxy carries every host, as its chat's does.
    let floor = confines.level == crate::sandbox::Level::Floor;
    for (k, v) in crate::sandbox::seatbelt::env(&beside.proxy_url(), beside.tmp()) {
        exec::set_var(&mut base, &k, v);
    }
    for (k, v) in crate::sandbox::tools::home_of_its_own(beside.tmp()) {
        exec::set_var(&mut base, &k, v);
    }
    // Its git held to the repository judged above (#3868).
    for (k, v) in held_to {
        exec::set_var(&mut base, &k, v);
    }
    for (k, v) in forge_env {
        exec::set_var(&mut base, &k, v);
    }
    // ssh, and git over ssh, through the run's own SOCKS port (#1667): the chat's route names
    // the chat's port, which the run's sandbox does not reach.
    if let Some(route) = beside.ssh_route() {
        for (k, v) in route.env() {
            exec::set_var(&mut base, &k, v);
        }
        let path = base
            .iter()
            .rev()
            .find(|(k, _)| k == "PATH")
            .and_then(|(_, v)| v.to_str())
            .unwrap_or("")
            .to_owned();
        exec::set_var(&mut base, "PATH", route.on_path(&path));
    }
    let base = match exec::child_env_of(&ctx, &wanted.vault, &base) {
        Ok(base) => base,
        Err(e) => return refuse(&mut *writer, 1, e.message.to_string()),
    };
    let within = match private_dir(&ctx.vaults_dir()) {
        Ok(within) => within,
        Err(e) => {
            return refuse(
                &mut *writer,
                1,
                format!("purlis could not make the folder a credential file is kept in ({e})"),
            );
        }
    };
    let req = Request {
        vault: wanted.vault.clone(),
        env: wanted.env.clone(),
        file: wanted.file.clone(),
        dotenv: wanted.dotenv.clone(),
        stream: false,
        exec: false,
        command: command.clone(),
        argv0: None,
    };
    let prepared = match (&v, handed, &minting) {
        // A GitHub App's token, minted here from its key, which goes to nothing else (#2053, E2):
        // the tool's one key is answered with the token.
        (Some(_), _, Some(tool)) => {
            // Only the clones its workspace had when the chat started, as they were then, and
            // whose git directory is in none of its grants, name the chat's repos.
            let mut grants = confines.writable.clone();
            if let Some(caches) = &confines.widened.caches {
                grants.extend(caches.trees.iter().chain(caches.bare.iter()).cloned());
            }
            // And only those purlis cloned or the person saved from the window.
            let config_root = crate::forge::apptoken::record_root();
            // The App's key is read to mint, through a link too: judged as any read (#5795).
            let minted_from: Vec<&str> = tool.values.iter().map(|one| one.key.as_str()).collect();
            if let Some(why) = read_out_of_reach(&ctx, persona, &tool.vault, &minted_from) {
                return refuse(&mut *writer, 1, why);
            }
            let token = match crate::forge::apptoken::token_from_vault(
                &ctx,
                &crate::forge::apptoken::For {
                    root: &asker.root,
                    chat: asker.chat,
                    folder: asker.folder.as_deref(),
                    here: wanted.cwd.as_deref(),
                    writes: &grants,
                    started: Some(&confines.clones),
                    on_a_runner: false,
                    config_root: config_root.as_deref(),
                    ends_seen: Some(ends_seen),
                },
                tool,
            ) {
                Ok(token) => token,
                Err(why) => return refuse(&mut *writer, 1, why),
            };
            if let Some(why) = read_out_of_reach(&ctx, persona, &tool.vault, &minted_from) {
                return refuse(&mut *writer, 1, why);
            }
            exec::prepare_from(
                &|key| minted_for(tool, key, &token),
                &req,
                base,
                Some(&within),
                &|| None,
            )
        }
        // A key the vault holds no value under, or a link whose entry is gone (#5075), is
        // asked of the person, where the app asks (#5068, ADR 0109): the run waits, and reads
        // the vault again once it is filled. Told by the provider's kind of failure, never by
        // its sentence. Each key is asked once a run: one still missing after its fill is
        // refused as it was before.
        (Some(v), _, None) => {
            let mut filled: Vec<String> = Vec::new();
            loop {
                let missed: std::sync::Mutex<Option<String>> = std::sync::Mutex::default();
                let read = |key: &str| {
                    // A link into a vault the persona may not use is read for no run (#5795),
                    // and asked of nobody: `Kind::Other` raises no Secret ask.
                    if let Some(why) = read_out_of_reach(&ctx, persona, &wanted.vault, &[key]) {
                        return Err(super::VaultError::new(why));
                    }
                    let got = cmd::get_value(&ctx, v, key).map(zeroize::Zeroizing::new);
                    // And again once read: a link written meanwhile is seen (#5795).
                    if let Some(why) = read_out_of_reach(&ctx, persona, &wanted.vault, &[key]) {
                        return Err(super::VaultError::new(why));
                    }
                    // A Linked entry whose entry is gone is asked for too (#5075), naming
                    // the gone entry to the person alone.
                    let asks = got.as_ref().is_err_and(|e| {
                        matches!(e.kind, super::Kind::NotFound | super::Kind::BrokenLink)
                    });
                    if asks {
                        *missed
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) =
                            Some(key.to_owned());
                    }
                    got
                };
                let got = exec::prepare_from(&read, &req, base.clone(), Some(&within), &|| None);
                let missed = missed
                    .into_inner()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let (Err(Stopped::Said(..)), Some(key), Some(missing)) = (&got, missed, missing)
                else {
                    break got;
                };
                if filled.contains(&key) {
                    break got;
                }
                let need = super::ask::Need {
                    chat: Some(asker.chat),
                    persona: persona.to_owned(),
                    vault: wanted.vault.clone(),
                    key: key.clone(),
                    tool: its_repository.as_ref().map(|ready| ready.tool.name.clone()),
                    purpose: None,
                    declared: super::needs::declared(&ctx.root, persona, &key)
                        .map(|need| need.purpose),
                    names_no_vault: false,
                    reason: super::ask::Reason::broken(&ctx, &wanted.vault, &key),
                    no_allow: None,
                };
                let held = missing.asks.hold(need, missing.within, &mut |line: &str| {
                    (missing.moved)();
                    let _ = send(&mut *writer, &Frame::Note(line.to_owned()));
                });
                match held {
                    super::ask::Held::Filled { vault, key: stored }
                        if vault == wanted.vault && stored == key =>
                    {
                        // Policy may have come to lock them while it waited, and the value that
                        // was stored stays stored.
                        if let Some(locked) = locked_since.as_ref().and_then(|keys| {
                            run_grant_locked(
                                asker,
                                keys,
                                &command,
                                &crate::sandbox::policy::Locks::of(&asker.root),
                            )
                        }) {
                            // Refused after its Allow, its Allow once goes unused (#5242).
                            if let Some(bound) = &grant_bound {
                                missing.asks.drop_once(bound);
                            }
                            return refuse(&mut *writer, 1, locked);
                        }
                        // The person may have filled it with an entry they keep, as a link: one
                        // a browser login fills is no granted program's either.
                        if under_a_grant
                            && let Some(why) = handed_to_no_granted_program(
                                &ctx,
                                asker.persona.as_deref(),
                                &wanted.vault,
                                &[&key],
                            )
                        {
                            if let Some(bound) = &grant_bound {
                                missing.asks.drop_once(bound);
                            }
                            return refuse(&mut *writer, 1, why);
                        }
                        filled.push(key);
                    }
                    // Stored where this run does not read it: said, so the chat runs it again
                    // naming the key it is stored under (#5072).
                    super::ask::Held::Filled { vault, key: stored } => {
                        return refuse(
                            &mut *writer,
                            1,
                            super::ask::filled_elsewhere(&vault, &stored),
                        );
                    }
                    super::ask::Held::Declined { line }
                    | super::ask::Held::Refused { line }
                    | super::ask::Held::CallerGone { line } => {
                        return refuse(&mut *writer, 1, line);
                    }
                    super::ask::Held::StillOpen { line } => {
                        return refuse(&mut *writer, WAITING_ON_THE_PERSON, line);
                    }
                }
            }
        }
        (None, Some(handed), _) => {
            exec::prepare_from(&|key| handed.value(key), &req, base, Some(&within), &|| {
                None
            })
        }
        (None, None, _) => return refuse(&mut *writer, 1, "no vault to read".to_owned()),
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(Stopped::Said(code, why)) => return refuse(&mut *writer, code, why),
        Err(Stopped::Signal(code)) => return refuse(&mut *writer, code, "stopped".to_owned()),
    };
    // The grant it goes under is used now, as it starts: Allow once goes with this run, and a
    // grant taken back meanwhile stops it (#5242).
    if let (Some(bound), Some(asks)) = (&grant_bound, asks_of_grants)
        && !asks.granted(bound)
    {
        return refuse(
            &mut *writer,
            1,
            format!(
                "the person's Allow for {} with vault {} was used by another run of this command, \
                 or taken back, so this one did not run",
                bound.program_name(),
                bound.vault
            ),
        );
    }
    let _ = values.set(zeroize::Zeroizing::new(prepared.secret_values.clone()));
    let mut prepared = prepared;
    let given: Vec<PathBuf> = Vec::new();
    // A value that points at a host the chat may reach goes through a tunnel to exactly it, and
    // one the chat may not reach is refused as the proxy refuses one (#1667).
    // No tunnel at the floor: nothing keeps the run from the host itself.
    let opened = if floor {
        Ok((Vec::new(), Vec::new()))
    } else {
        tunnelled(
            &mut prepared,
            &req.env,
            &command[0],
            confines,
            &refusals,
            reached,
        )
    };
    let (tunnels, notes) = match opened {
        Ok(opened) => opened,
        Err(e) => {
            return refuse(
                &mut *writer,
                1,
                format!("purlis could not open the tunnel the command reaches its host by ({e})"),
            );
        }
    };
    #[cfg(test)]
    if let Wrap::Refusing(targets) = wrap {
        for target in targets.split_whitespace() {
            let (host, port) = target.rsplit_once(':').expect("host:port");
            refusals.heard(host, port.parse().expect("a port"));
        }
    }
    let program = match on_path(&command[0], &prepared.env) {
        Some(program) => program,
        None => {
            return refuse(
                &mut *writer,
                127,
                format!("command not found: {}", command[0]),
            );
        }
    };
    // A tool declared under a name its program's file does not have runs under that name
    // (#1977), the one the chat's start armed and never one the chat asks for. A run's wrap
    // (`sandbox-exec`, purlis's own on Linux) starts a program under the path it is given, so
    // the run is of a link of that name, the app's own for this run beside the credential files
    // no chat reads, to the program as resolved: what runs is that program still.
    let name_link = match declared_as
        .as_deref()
        .map(|name| named_link(&within, name, &program))
        .transpose()
    {
        Ok(name_link) => name_link,
        Err(e) => {
            return refuse(
                &mut *writer,
                1,
                format!(
                    "purlis could not start {} under its tool's name ({e})",
                    command[0]
                ),
            );
        }
    };
    let run_as = name_link
        .as_ref()
        .map_or_else(|| program.clone(), |(_, link)| link.clone());
    // The run's credential files and its named link: given back to the run alone.
    let readable: Vec<PathBuf> = prepared
        .files
        .iter()
        .cloned()
        .chain(name_link.iter().map(|(_, link)| link.clone()))
        .collect();
    let profile = || {
        let mut ports = beside.proxy_ports();
        ports.extend(tunnels.iter().map(crate::sandbox::tunnel::Tunnel::port));
        profile_on(confines, &readable, &given, &folder, beside.tmp(), &ports)
    };
    let mut child = match &applying {
        Applying::Seatbelt => {
            let profile = match profile() {
                Ok(profile) => profile,
                Err(why) => return refuse(&mut *writer, 1, why.to_owned()),
            };
            let mut child = std::process::Command::new(crate::sandbox::backend::SANDBOX_EXEC);
            child.arg("-p").arg(profile).arg("--").arg(&run_as);
            child
        }
        // The same profile, read by purlis's own wrap, as a wrapped chat's line starts
        // ([`crate::sandbox::wrapper::line`]).
        Applying::Purlis(purlis) => {
            let profile = match profile() {
                Ok(profile) => profile,
                Err(why) => return refuse(&mut *writer, 1, why.to_owned()),
            };
            let Some(named) = run_as.to_str() else {
                return refuse(
                    &mut *writer,
                    1,
                    format!(
                        "the program's path is not text, so purlis's sandbox cannot start it: {}",
                        run_as.display()
                    ),
                );
            };
            let mut child = std::process::Command::new(purlis);
            child.args(purlis_linux_sandbox::line(
                &profile,
                named.to_owned(),
                Vec::new(),
            ));
            child
        }
        #[cfg(test)]
        Applying::Unwrapped => std::process::Command::new(&run_as),
    };
    let start_in = wanted
        .cwd
        .clone()
        .filter(|cwd| cwd.is_absolute() && cwd.is_dir())
        .unwrap_or_else(|| folder.clone());
    // A process group of its own, so everything it starts is ended with it.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut child, 0);
    // purlis's own wrap starts with nothing but the run's environment carried for the command
    // (#2460), and gives the command exactly that; `sandbox-exec` hands on its own.
    // A copy of the run's values: wiped once the child has its own (#3420).
    let start_with = exec::WipedEnv(match &applying {
        Applying::Purlis(_) => purlis_linux_sandbox::carried::carried(prepared.env.iter().cloned()),
        _ => prepared.env.clone(),
    });
    child
        .args(&command[1..])
        .current_dir(start_in)
        .env_clear()
        .envs(start_with.0.iter().map(|(k, v)| (k, v)))
        .stdin(if wanted.stdin && !under_a_grant {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Recorded under the chat's own session, as the record of a run in the chat would be.
    let chat = asker.chat.to_string();
    let ctx_traced = Ctx {
        root: ctx.root.clone(),
        state: ctx.state.clone(),
        env: Env::of(&[(crate::active::SESSION_ID_ENV, &chat)]),
        chat_writes: Vec::new(),
        kept: super::identity::Kept::default(),
    };
    exec::record(
        &ctx_traced,
        &wanted.vault,
        &prepared,
        &command[0],
        "brokered",
        &[
            ("chat", Value::from(asker.chat)),
            (
                "persona",
                Value::String(asker.persona.clone().unwrap_or_default()),
            ),
            // Values its desktop sent (#3406): said so, with the request its desktop's
            // `secret.forwarded` names. A token from the cluster's operator (#4332) is said so,
            // and no desktop's record answers it.
            (
                "from",
                handed.map_or(Value::Null, |handed| {
                    Value::String(
                        if handed.from_the_cluster {
                            "cluster"
                        } else {
                            "desktop"
                        }
                        .to_owned(),
                    )
                }),
            ),
            (
                "request",
                handed
                    .filter(|handed| !handed.from_the_cluster)
                    .map_or(Value::Null, |handed| Value::String(request_of(&handed.run))),
            ),
        ],
    );
    if crate::applog::is_a_service() {
        // A runner's journal names the program, never the vault (#2838).
        tracing::info!(
            "purlis: chat {} (persona {}) ran `{}` with a vault through its host",
            asker.chat,
            asker.persona.as_deref().unwrap_or("none"),
            command[0]
        );
    } else {
        tracing::info!(
            "purlis: chat {} (persona {}) ran `{}` with vault '{}' through the app",
            asker.chat,
            asker.persona.as_deref().unwrap_or("none"),
            command[0],
            wanted.vault
        );
    }

    // No core file of the run, which would hold its values, in the chat's folder or anywhere.
    no_core_files();
    let mut spawned = match crate::forklock::spawn(&mut child) {
        Ok(spawned) => spawned,
        Err(e) => return refuse(&mut *writer, 1, format!("cannot run {}: {e}", command[0])),
    };
    let group = Live::hold(spawned.id());
    if v.as_ref()
        .is_some_and(|v| cmd_said_note(&ctx, v, &mut *writer).is_err())
        || notes
            .iter()
            .any(|note| send(&mut *writer, &Frame::Note(note.clone())).is_err())
        || send(&mut *writer, &Frame::Started).is_err()
    {
        group.kill();
        let _ = spawned.wait();
        return;
    }
    let code = run(
        spawned,
        &group,
        reader,
        &mut *writer,
        &prepared.secret_values,
    );
    let (withheld, named): (Vec<String>, Vec<String>) = refusals
        .refused()
        .into_iter()
        .partition(|target| carries_a_value(target, &prepared.secret_values));
    for target in named {
        let note = super::redact_str(&refused_note(&target), &prepared.secret_values);
        let _ = send(&mut *writer, &Frame::Note(note));
    }
    if !withheld.is_empty() {
        let _ = send(&mut *writer, &Frame::Note(WITHHELD_NOTE.to_owned()));
    }
    if let Some(note) = more_refused_note(refusals.more()) {
        let _ = send(&mut *writer, &Frame::Note(note));
    }
    let _ = send(&mut *writer, &Frame::Exit(code));
    drop(tunnels);
    drop(prepared);
    drop(beside);
}

/// Points each `--env` value of `prepared` that names a host and port at a tunnel to it, where
/// the chat may reach that exact host and port ([`crate::sandbox::tunnel::route`]), and each
/// value a `--file` or `--dotenv` file holds the same way, in the file (#1708): the value is
/// swapped and masked as the value is. A host the chat may not reach is told to `refusals`, as
/// the proxy tells one, so the chat's Notice offers Allow; one on this machine, or one whose
/// client checks the certificate against a name the tunnel cannot keep for `program` (#1708),
/// is said in a note naming the variable alone. Answers the tunnels, which live as long as the
/// run, and the notes.
fn tunnelled(
    prepared: &mut exec::Prepared,
    specs: &[String],
    program: &str,
    confines: &Confines,
    refusals: &crate::sandbox::egress::Refusals,
    reached: Option<crate::sandbox::egress::Reached>,
) -> std::io::Result<(Vec<crate::sandbox::tunnel::Tunnel>, Vec<String>)> {
    use crate::sandbox::tunnel::{Client, Route, route};
    let client = Client::of(program, env_var(&prepared.env, PGSSLMODE).flatten());
    let mut opening = Opening {
        reach: confines.decides(),
        own: crate::sandbox::hosts::own_addresses(),
        reached,
        refusals,
        tunnels: Vec::new(),
        notes: Vec::new(),
    };
    for name in specs
        .iter()
        .filter_map(|spec| spec.split_once('=').map(|(n, _)| n))
    {
        let Some(value) = prepared
            .env
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.to_str())
            .map(|value| zeroize::Zeroizing::new(value.to_owned()))
        else {
            continue;
        };
        // A host alone in its variable, its port beside it in another (#1708).
        let pair = host_pair(name, &value, &prepared.env);
        let routed = zeroize::Zeroizing::new(
            pair.as_ref()
                .map_or_else(|| value.to_string(), |pair| pair.target.clone()),
        );
        let mut routed = route(&routed, &client, &opening.reach, &opening.own);
        if pair
            .as_ref()
            .is_some_and(|pair| pair.by_name && !client.reads_hostaddr)
        {
            // libpq's `PGHOSTADDR` keeps the name; a driver that is not libpq reads none.
            if let Route::Through(pointed, _) | Route::Refused(pointed) = routed {
                routed = Route::ByName(pointed);
            }
        }
        let Some((pointed, at)) = opening.open(name, routed)? else {
            continue;
        };
        if let Some(pair) = &pair {
            // Nothing secret is added: loopback and the tunnel's port.
            let place = match pair.addr_var {
                Some(addr) if pair.by_name => addr,
                _ => name,
            };
            exec::set_var(&mut prepared.env, place, crate::sandbox::tunnel::LOOPBACK);
            exec::set_var(&mut prepared.env, pair.port_var, at.to_string());
        } else {
            let swapped = pointed.at(at);
            exec::set_var(&mut prepared.env, name, swapped.clone());
            prepared.secret_values.push(swapped);
        }
    }
    // A value a credential file holds, pointed in the file itself, every other line as it was.
    for handed in &mut prepared.handed {
        let mut moved = false;
        for (entry, value) in &mut handed.entries {
            let name = match entry {
                Some(entry) => format!("{entry} in {}", handed.envvar),
                None => handed.envvar.clone(),
            };
            let routed = route(value, &client, &opening.reach, &opening.own);
            if let Some((pointed, at)) = opening.open(&name, routed)? {
                // The value it was is wiped as the file is pointed (#3420).
                drop(zeroize::Zeroizing::new(std::mem::replace(
                    value,
                    pointed.at(at),
                )));
                prepared.secret_values.push(value.clone());
                let escaped = super::dotenv::escaped(value);
                if escaped != *value {
                    prepared.secret_values.push(escaped);
                }
                moved = true;
            }
        }
        if moved {
            let body = zeroize::Zeroizing::new(handed_body(&handed.entries)?);
            std::fs::write(&handed.path, body.as_bytes())?;
        }
    }
    Ok((opening.tunnels, opening.notes))
}

/// What a credential file holds: a `--file`'s one value, or a `--dotenv` file's lines, as
/// [`exec::prepare`] writes them.
fn handed_body(entries: &[(Option<String>, String)]) -> std::io::Result<String> {
    if let [(None, value)] = entries {
        return Ok(value.clone());
    }
    let lines = entries
        .iter()
        .map(|(name, value)| {
            super::dotenv::line(name.as_deref().unwrap_or_default(), value)
                .map_err(|e| std::io::Error::other(e.0))
        })
        .collect::<std::io::Result<Vec<String>>>()?;
    Ok(format!("{}\n", lines.join("\n")))
}

/// The tunnels one brokered run opens, and what it says of the values it does not point.
struct Opening<'a> {
    reach: crate::sandbox::reach::Reach,
    own: Vec<std::net::IpAddr>,
    reached: Option<crate::sandbox::egress::Reached>,
    refusals: &'a crate::sandbox::egress::Refusals,
    tunnels: Vec<crate::sandbox::tunnel::Tunnel>,
    notes: Vec<String>,
}

impl Opening<'_> {
    /// **The tunnel value `name` is pointed at**, as `routed` decides: where it points and the
    /// tunnel's port, one tunnel to each target. None where it is not pointed: told to
    /// `refusals` where a person could allow it, said in a note where it is handed as it is.
    fn open(
        &mut self,
        name: &str,
        routed: crate::sandbox::tunnel::Route,
    ) -> std::io::Result<Option<(crate::sandbox::tunnel::Pointed, u16)>> {
        use crate::sandbox::tunnel::{Route, TUNNELS_AT_MOST, Tunnel};
        match routed {
            Route::Through(pointed, decision) => {
                let at = match self.tunnels.iter().find(|t| t.target() == pointed.target()) {
                    Some(open) => open.port(),
                    None if self.tunnels.len() >= TUNNELS_AT_MOST => {
                        self.notes.push(format!(
                            "purlis opens at most {TUNNELS_AT_MOST} tunnels for one command, so \
                             {name} was handed as it is."
                        ));
                        return Ok(None);
                    }
                    None => {
                        let tunnel = Tunnel::open(
                            &pointed,
                            &decision,
                            self.reach.clone(),
                            self.reached.clone(),
                        )?;
                        let port = tunnel.port();
                        self.tunnels.push(tunnel);
                        port
                    }
                };
                // A runner's journal names no secret's variable (#2838).
                if crate::applog::is_a_service() {
                    tracing::info!(
                        "purlis: a variable of a brokered `secret exec` reaches its host \
                         through a tunnel on port {at}"
                    );
                } else {
                    tracing::info!(
                        "purlis: {name} of a brokered `secret exec` reaches its host through a \
                         tunnel on port {at}"
                    );
                }
                Ok(Some((pointed, at)))
            }
            Route::Refused(pointed) => {
                self.refusals.heard(pointed.host(), pointed.port());
                Ok(None)
            }
            Route::Local(_) => {
                self.notes.push(format!(
                    "{name} points at this machine, a link-local address or a cloud metadata \
                     service, which purlis's sandbox never reaches, so it was handed as it is."
                ));
                Ok(None)
            }
            Route::ByName(_) => {
                self.notes.push(format!(
                    "{name} checks the server's certificate against its host's name, which this \
                     client cannot keep through purlis's tunnel, so it was handed as it is."
                ));
                Ok(None)
            }
            Route::Untouched => Ok(None),
        }
    }
}

/// The last `wanted` in `env`: None where it is not set, `Some(None)` where it is not text.
fn env_var(env: &[(OsString, OsString)], wanted: &str) -> Option<Option<String>> {
    env.iter()
        .rev()
        .find(|(k, _)| k == wanted)
        .map(|(_, v)| v.to_str().map(str::to_owned))
}

/// libpq's variables for where it connects: the host, the address it connects to instead of
/// looking the host up, the port, and the certificate check.
const PGHOST: &str = "PGHOST";
const PGHOSTADDR: &str = "PGHOSTADDR";
const PGPORT: &str = "PGPORT";
const PGSSLMODE: &str = "PGSSLMODE";

/// The host variables read beside a port variable (#1708): the host's, the port's, the client's
/// own port where none is set, and the address it connects to instead of looking the host up,
/// where it has one. MySQL's client reads `MYSQL_HOST` and `MYSQL_TCP_PORT`; `REDIS_HOST` and
/// `REDIS_PORT` are the pair applications and images commonly read.
const HOST_PAIRS: &[(&str, &str, u16, Option<&str>)] = &[
    (PGHOST, PGPORT, 5432, Some(PGHOSTADDR)),
    ("MYSQL_HOST", "MYSQL_TCP_PORT", 3306, None),
    ("REDIS_HOST", "REDIS_PORT", 6379, None),
];

/// A host handed alone in its variable, with its port beside it in another (#1708).
struct HostPair {
    /// `host:port`, as a value pointed at one place spells it.
    target: String,
    /// The variable its port is read from, and pointed at the tunnel's.
    port_var: &'static str,
    /// The variable the client connects to instead of looking the host up, if it has one.
    addr_var: Option<&'static str>,
    /// The certificate is checked by name (`PGSSLMODE=verify-full`): the host's variable stays,
    /// and the client connects to `addr_var` instead.
    by_name: bool,
}

/// **A host variable as one place** (#1708): where `name` is one of [`HOST_PAIRS`] and `value`
/// a host alone, the host with its port from the pair's port variable in `env` (the client's own
/// where none is set). None for any other variable, a value that already names its port, a port
/// that is not one, or where libpq's `PGHOSTADDR` is set already, which could be read two ways.
fn host_pair(name: &str, value: &str, env: &[(OsString, OsString)]) -> Option<HostPair> {
    let &(_, port_var, default, addr_var) = HOST_PAIRS.iter().find(|(host, ..)| *host == name)?;
    if crate::sandbox::tunnel::pointed(value).is_some()
        || addr_var.is_some_and(|addr| env_var(env, addr).is_some())
    {
        return None;
    }
    let port = match env_var(env, port_var) {
        None => default.to_string(),
        Some(Some(port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => port,
        Some(_) => return None,
    };
    let host = if value.contains(':') {
        format!("[{value}]")
    } else {
        value.to_owned()
    };
    Some(HostPair {
        target: format!("{host}:{port}"),
        port_var,
        addr_var,
        by_name: addr_var.is_some()
            && env_var(env, PGSSLMODE).flatten().as_deref() == Some("verify-full"),
    })
}

/// What runs beside a brokered child: the chat's egress proxy and a temp directory of its own,
/// or, for a test that runs it unwrapped, a temp directory and a proxy nothing listens on.
enum Beside {
    /// The confinement, and its temp directory.
    Confined(Box<crate::sandbox::Confinement>, PathBuf),
    #[cfg(test)]
    Pretend(tempfile::TempDir),
}

impl Beside {
    fn start(
        wrap: Wrap,
        confines: &Confines,
        refusals: crate::sandbox::egress::Refusals,
        reached: Option<crate::sandbox::egress::Reached>,
    ) -> std::io::Result<Self> {
        match wrap {
            Wrap::Seatbelt | Wrap::Purlis | Wrap::Nothing => {
                crate::sandbox::Confinement::serving(crate::sandbox::egress::Serving {
                    refusals,
                    reached,
                    ..crate::sandbox::egress::Serving::of(confines.decides())
                })
                .and_then(|confinement| {
                    let tmp = confinement
                        .tmp()
                        .map(Path::to_path_buf)
                        .ok_or_else(|| std::io::Error::other("its temp directory was not made"))?;
                    Ok(Self::Confined(Box::new(confinement), tmp))
                })
            }
            #[cfg(test)]
            Wrap::Unwrapped => tempfile::tempdir().map(Self::Pretend),
            #[cfg(test)]
            Wrap::Refusing(_) => {
                drop(refusals);
                tempfile::tempdir().map(Self::Pretend)
            }
        }
    }

    fn tmp(&self) -> &Path {
        match self {
            Self::Confined(_, tmp) => tmp,
            #[cfg(test)]
            Self::Pretend(tmp) => tmp.path(),
        }
    }

    /// Its proxy's ports, HTTP then SOCKS5: what the child may connect to besides its tunnels.
    fn proxy_ports(&self) -> Vec<u16> {
        match self {
            Self::Confined(confinement, _) => confinement.proxy_ports().to_vec(),
            #[cfg(test)]
            Self::Pretend(_) => vec![9],
        }
    }

    /// Its ssh route through its own SOCKS port (#1667), where it has one.
    fn ssh_route(&self) -> Option<&crate::sandbox::tunnel::SshRoute> {
        match self {
            Self::Confined(confinement, _) => confinement.ssh_route(),
            #[cfg(test)]
            Self::Pretend(_) => None,
        }
    }

    fn proxy_url(&self) -> String {
        match self {
            Self::Confined(confinement, _) => confinement.proxy_url(),
            #[cfg(test)]
            Self::Pretend(_) => "http://127.0.0.1:9".to_owned(),
        }
    }
}

/// The vault's one note, where it is due, as a frame.
fn cmd_said_note(ctx: &Ctx, v: &registry::Vault, writer: &mut dyn Write) -> std::io::Result<()> {
    match cmd::held_note(ctx, v) {
        Some(note) => send(writer, &Frame::Note(note)),
        None => Ok(()),
    }
}

/// The folder under the vaults directory a brokered run keeps its credential files in.
pub const EXEC_DIR: &str = "exec";

/// How the folder of a run's [`named_link`] is named in the exec folder: `as-<random>`.
const NAMED_PREFIX: &str = "as-";

/// **A link named `name` to `program`**, in a folder of its own made in `within`, the app's
/// private folder no chat reads or writes (#1977): the folder, removed when it is dropped, and
/// the link. A name that is not one file's name is refused.
fn named_link(
    within: &Path,
    name: &str,
    program: &Path,
) -> std::io::Result<(tempfile::TempDir, PathBuf)> {
    if Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
        return Err(std::io::Error::other(format!(
            "{name} is not a file's name"
        )));
    }
    let dir = tempfile::Builder::new()
        .prefix(NAMED_PREFIX)
        .tempdir_in(within)?;
    let link = dir.path().join(name);
    #[cfg(unix)]
    std::os::unix::fs::symlink(program, &link)?;
    // A run is wrapped only on macOS: this is never reached elsewhere.
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(program, &link)?;
    Ok((dir, link))
}

/// The `exec` folder under `vaults` (the project's vaults directory), made `0700` if it is not
/// there. Refused unless `vaults` is a directory and no link, and `exec` is, as the kernel names
/// it, exactly `exec` inside it: a link anywhere on the way could put a credential file where a
/// chat reads.
fn private_dir(vaults: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(vaults)?;
    let not_a_folder = |what: &Path| {
        std::io::Error::other(format!("{} is not a folder of its own", what.display()))
    };
    if !std::fs::symlink_metadata(vaults)?.is_dir() {
        return Err(not_a_folder(vaults));
    }
    let dir = vaults.join(EXEC_DIR);
    match std::fs::create_dir(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    if !std::fs::symlink_metadata(&dir)?.is_dir()
        || dir.canonicalize()? != vaults.canonicalize()?.join(EXEC_DIR)
    {
        return Err(not_a_folder(&dir));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

/// Removes every credential file a brokered run left in the project at `root`, and every
/// folder of a link it ran a tool under its declared name by ([`named_link`]): one the app was
/// stopped before it could remove (a crash, a kill). Run when a project opens, when no run of
/// its can be live. Only the files [`super::exec`] names and the folders [`named_link`] makes,
/// and nothing behind a link: a folder's links are removed, never followed.
pub fn sweep(ctx: &Ctx) {
    let dir = ctx.vaults_dir().join(EXEC_DIR);
    let ours = std::fs::symlink_metadata(ctx.vaults_dir()).is_ok_and(|m| m.is_dir())
        && std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir());
    if !ours {
        return;
    }
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if name.starts_with("charter-secret-") && kind.is_file() {
            let _ = std::fs::remove_file(entry.path());
        } else if name.starts_with(NAMED_PREFIX) && kind.is_dir() {
            // `remove_dir_all` removes the links inside and follows none of them.
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The most brokered runs one chat may have at once.
pub const RUNS_AT_ONCE: usize = 4;

/// One chat's place among its brokered runs, given back when dropped.
struct Slot {
    key: (PathBuf, u32),
}

static SLOTS: std::sync::Mutex<Vec<(PathBuf, u32)>> = std::sync::Mutex::new(Vec::new());

impl Slot {
    /// A place for chat `chat` of the project at `root`, or `None` when it has
    /// [`RUNS_AT_ONCE`] already.
    fn take(root: &Path, chat: u32) -> Option<Self> {
        let key = (root.to_path_buf(), chat);
        let mut held = SLOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.iter().filter(|k| **k == key).count() >= RUNS_AT_ONCE {
            return None;
        }
        held.push(key.clone());
        Some(Self { key })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut held = SLOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(at) = held.iter().position(|k| *k == self.key) {
            held.remove(at);
        }
    }
}

/// A brokered child's process group while it may be running, which [`stop_every_run`] kills.
struct Live {
    group: u32,
}

static LIVE: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

impl Live {
    fn hold(group: u32) -> Self {
        LIVE.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(group);
        Self { group }
    }

    /// Kills the whole group: the child and everything it started that stayed in it.
    fn kill(&self) {
        kill_group(self.group);
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        // Not killed here: the group was killed as the child ended (`run`), and a number killed
        // long after could by then be another group's.
        let mut live = LIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(at) = live.iter().position(|g| *g == self.group) {
            live.remove(at);
        }
    }
}

fn kill_group(group: u32) {
    #[cfg(unix)]
    if let Some(pid) = i32::try_from(group)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    #[cfg(not(unix))]
    let _ = group;
}

/// Kills every brokered run this process has going, as the app lets go of its projects: each
/// run's process group, so everything it started that stayed in that group.
pub fn stop_every_run() {
    let live: Vec<u32> = LIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    for group in live {
        kill_group(group);
    }
}

/// `program` as a path: itself when it names one, else the first executable of that name on
/// the child's own `PATH`.
fn on_path(program: &str, env: &[(OsString, OsString)]) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }
    let path = env.iter().rev().find(|(k, _)| k == "PATH")?.1.clone();
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// **purlis's own program a run for the chat is wrapped in on Linux** (#2342): the one the
/// chat's start checked and held ([`Confines::wrap`]), checked again as
/// [`crate::sandbox::program::checked_wrap_at`] checks it against every folder the chat may write
/// ([`chat_writes`]), on a kernel with the Landlock the wrap needs. Or why nothing runs.
/// At the floor it is also one the start held unwritten ([`Confines::held`]).
fn wrap_for(asker: &Asker, confines: &Confines, folder: &Path) -> Result<PathBuf, String> {
    let Some(recorded) = &confines.wrap else {
        return Err(NO_WRAP_RECORDED.to_owned());
    };
    let mut writable = chat_writes(asker, confines);
    // As the chat's start checked it: at the full sandbox the temporary folders count as the
    // chat's too, and at the floor what the start held is what keeps it unwritten.
    if confines.level == crate::sandbox::Level::Full {
        writable.extend(crate::sandbox::program::temp_roots(&[]));
    }
    let program = crate::sandbox::program::checked_wrap_at(
        recorded,
        &asker.root,
        crate::sandbox::program::Chat {
            cwd: folder,
            writable: &writable,
            env: &[],
        },
    )
    .map_err(|refused| match refused {
        crate::sandbox::NotStarted::ProgramWritable(path) => format!(
            "purlis's own program lives where this chat can write: {}, so it runs no command \
             for the chat. Keep purlis outside the project and outside what a chat may write, \
             then start the chat again",
            path.display()
        ),
        _ => "purlis's own program was replaced since this chat started, so it runs no command \
              for the chat. Restart purlis, then start the chat again"
            .to_owned(),
    })?;
    if confines.level == crate::sandbox::Level::Floor
        && !confines.held.iter().any(|held| program.starts_with(held))
    {
        return Err(format!(
            "purlis's own program, {}, is not one this chat's start kept it from writing, so it \
             runs no command for the chat. Start the chat again",
            program.display()
        ));
    }
    #[cfg(target_os = "linux")]
    if purlis_linux_sandbox::abi() < purlis_linux_sandbox::MIN_ABI {
        return Err(format!(
            "{}, so purlis runs no command for the chat on this kernel",
            purlis_linux_sandbox::NEEDS_LANDLOCK
        ));
    }
    Ok(program)
}

/// The child's Seatbelt profile: the chat's own ([`crate::sandbox::seatbelt::profile`]) for
/// its folder, a temp directory of the child's own, and `ports` on loopback (its proxy's two,
/// carrying the chat's hosts, and each tunnel's, #1667), with a read of each credential file in
/// `files`, and a read and write of each folder in `dirs` (a browser login's own), given back
/// after every denial, and nothing else of the vaults class.
pub(crate) fn profile_on(
    confines: &Confines,
    files: &[PathBuf],
    dirs: &[PathBuf],
    folder: &Path,
    tmp: &Path,
    ports: &[u16],
) -> Result<String, &'static str> {
    use crate::sandbox::seatbelt;
    let denied: &[Denial] = &confines.denied;
    // The chat's own widenings (#1337), so the run gets its certificate check and its cache
    // grants.
    let mut own = seatbelt::Own::default();
    seatbelt::widen(&mut own, &confines.widened)?;
    // The floor (#2071): every folder written and the network open, but what the chat's own
    // sandbox denies and holds.
    let mut profile = if confines.level == crate::sandbox::Level::Floor {
        seatbelt::floor_profile(denied, &confines.held, &own, ports, None)?
    } else {
        seatbelt::profile(denied, &own, folder, tmp, ports, None)?
    };
    // Last of all, after the vaults every profile denies wherever they are (#2071): the run's
    // own credential files and folders, which purlis made in the vaults folder for it alone.
    // Each under both names the kernel may give it, as the vaults are denied under both: across
    // a macOS firmlink, `/private/var/…` is also `/System/Volumes/Data/private/var/…` (#2489).
    // A file is given back by its own name, never followed: a run's named link is read and
    // started by the link's name, and what it points at is given nothing (#2617).
    for file in files {
        for name in seatbelt::kernel_strings_itself(file)? {
            profile.push_str(&format!("(allow file-read* (literal {name}))\n"));
        }
    }
    for dir in dirs {
        for name in seatbelt::kernel_strings(dir)? {
            profile.push_str(&format!(
                "(allow file-read* file-write* (subpath {name}))\n"
            ));
        }
    }
    Ok(profile)
}

/// What [`run`] hears from the child's pipes and the asker.
enum Heard {
    Out(Vec<u8>),
    Err(Vec<u8>),
    OutDone,
    ErrDone,
}

/// Streams the child's output to `writer` as it comes, redacted, feeding it the asker's stdin,
/// and answers its status. A connection that goes away kills the child: nobody is left to see
/// what it does.
fn run(
    mut child: std::process::Child,
    group: &Live,
    reader: Box<dyn BufRead + Send>,
    writer: &mut dyn Write,
    secrets: &[String],
) -> i32 {
    let (tx, rx) = std::sync::mpsc::channel::<Heard>();
    let gone = Arc::new(AtomicBool::new(false));
    for (pipe, out) in [
        (
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
            true,
        ),
        (
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
            false,
        ),
    ] {
        let tx = tx.clone();
        let Some(mut pipe) = pipe else {
            let _ = tx.send(if out { Heard::OutDone } else { Heard::ErrDone });
            continue;
        };
        std::thread::spawn(move || {
            let mut buf = vec![0u8; A_CHUNK];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = tx.send(if out { Heard::OutDone } else { Heard::ErrDone });
                        return;
                    }
                    Ok(n) => {
                        let chunk = buf[..n].to_vec();
                        let _ = tx.send(if out {
                            Heard::Out(chunk)
                        } else {
                            Heard::Err(chunk)
                        });
                    }
                }
            }
        });
    }
    drop(tx);
    listen(reader, child.stdin.take(), Arc::clone(&gone));

    let mut out = Redactor::new(secrets);
    let mut err = Redactor::new(secrets);
    let (mut out_done, mut err_done) = (false, false);
    let mut status: Option<i32> = None;
    // Once the child has ended, how long its pipes are read for: whatever held them was in its
    // group and is killed with it, so they close at once, and this bounds one that does not.
    let mut until: Option<std::time::Instant> = None;
    loop {
        if gone.load(Ordering::SeqCst) && status.is_none() {
            group.kill();
        }
        if status.is_none()
            && let Ok(Some(ended)) = child.try_wait()
        {
            status = Some(exec::exit_status(&ended));
            group.kill();
            until = Some(std::time::Instant::now() + PIPES_AFTER_EXIT);
        }
        let past = until.is_some_and(|until| std::time::Instant::now() > until);
        if (out_done && err_done) || past {
            if let Some(code) = status {
                if past {
                    let _ = frame_of(&out.finish(), Frame::Stdout, writer);
                    let _ = frame_of(&err.finish(), Frame::Stderr, writer);
                }
                return code;
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        let sent = match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Heard::Out(chunk)) => frame_of(&out.push(&chunk), Frame::Stdout, writer),
            Ok(Heard::Err(chunk)) => frame_of(&err.push(&chunk), Frame::Stderr, writer),
            Ok(Heard::OutDone) => {
                out_done = true;
                frame_of(&out.finish(), Frame::Stdout, writer)
            }
            Ok(Heard::ErrDone) => {
                err_done = true;
                frame_of(&err.finish(), Frame::Stderr, writer)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(()),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                out_done = true;
                err_done = true;
                Ok(())
            }
        };
        if sent.is_err() {
            gone.store(true, Ordering::SeqCst);
        }
    }
}

/// `bytes`, already redacted, as frames of `kind`, each of at most [`A_FRAMES_BYTES_ARE_AT_MOST`]
/// so no frame passes [`A_FRAME_IS_AT_MOST`] (#1407): the redactor holds back as much as the
/// longest value, and a value can be longer than one frame holds.
fn frame_of(
    bytes: &[u8],
    kind: fn(String) -> Frame,
    writer: &mut dyn Write,
) -> std::io::Result<()> {
    bytes
        .chunks(A_FRAMES_BYTES_ARE_AT_MOST)
        .try_for_each(|piece| send(writer, &kind(b64(piece))))
}

/// Reads the asker's [`Input`] on a thread of its own, into `stdin` where the child has one,
/// and marks the asker `gone` when its connection ends.
fn listen(
    mut reader: Box<dyn BufRead + Send>,
    mut stdin: Option<std::process::ChildStdin>,
    gone: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        loop {
            let mut line = String::new();
            match (&mut reader).take(A_FRAME_IS_AT_MOST).read_line(&mut line) {
                Ok(0) | Err(_) => {
                    gone.store(true, Ordering::SeqCst);
                    return;
                }
                Ok(_) => {}
            }
            match serde_json::from_str::<Input>(&line) {
                Ok(Input::Stdin(bytes)) => {
                    if let Some(pipe) = stdin.as_mut()
                        && pipe.write_all(&unb64(&bytes)).is_err()
                    {
                        stdin = None;
                    }
                }
                // The child's stdin ends; the connection stays, and its end is the asker's.
                Ok(Input::StdinClosed) => stdin = None,
                Err(_) => {
                    gone.store(true, Ordering::SeqCst);
                    return;
                }
            }
        }
    });
}

/// [`super::redact`] over a stream: a value split across two chunks is still masked.
///
/// It holds back the bytes that could still be the start of a value, at most one byte short of
/// the longest, and never emits part of an occurrence that a later chunk could complete.
/// What it holds, the values and the output it holds back, is wiped as it is dropped (#3420).
pub struct Redactor {
    secrets: zeroize::Zeroizing<Vec<String>>,
    longest: usize,
    held: zeroize::Zeroizing<Vec<u8>>,
}

impl Redactor {
    pub fn new(secrets: &[String]) -> Self {
        let secrets: zeroize::Zeroizing<Vec<String>> =
            zeroize::Zeroizing::new(secrets.iter().filter(|s| !s.is_empty()).cloned().collect());
        let longest = secrets.iter().map(String::len).max().unwrap_or(0);
        Self {
            secrets,
            longest,
            held: zeroize::Zeroizing::new(Vec::new()),
        }
    }

    /// `chunk` added; what is safe to show now, redacted.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.held.extend_from_slice(chunk);
        let mut cut = self
            .held
            .len()
            .saturating_sub(self.longest.saturating_sub(1));
        // No occurrence is cut in two: one that begins before the cut and ends after it is
        // held back whole, and so is any that then straddles the new cut.
        loop {
            let mut moved = false;
            for secret in self.secrets.iter() {
                let needle = secret.as_bytes();
                let first = cut.saturating_sub(needle.len() - 1);
                let straddling = (first..cut)
                    .find(|&at| at + needle.len() > cut && self.held[at..].starts_with(needle));
                if let Some(at) = straddling {
                    cut = at;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        let shown = super::redact(&self.held[..cut], &self.secrets);
        self.held.drain(..cut);
        shown
    }

    /// Everything still held, redacted: the stream has ended.
    pub fn finish(&mut self) -> Vec<u8> {
        let shown = super::redact(&self.held, &self.secrets);
        self.held.clear();
        shown
    }
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(test)]
mod fill_tests;

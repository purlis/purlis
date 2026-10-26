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
//! exactly that tool's vault and hand-over ([`crate::sandbox::tools::declared`]). Anything else
//! is refused with a sentence naming the tool to declare. The run is of the tool's program by
//! its absolute path, from a `PATH` without the folders the chat may write, and without the
//! chat's variables that make a program load code ([`crate::sandbox::tools::loads_code`]). On
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

/// The line a chat writes: which chat (checked against its token), and what it wants run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ask {
    /// The app's number for the chat, from [`crate::hookwire::CHAT_ENV`].
    pub chat: u32,
    /// What is to be run. The field's name is what tells this line from every other kind.
    pub secret_exec: Wanted,
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
/// **A courtesy, not the boundary.** The mark it reads is the chat's environment, which the
/// chat can change; what holds the item is the store's own rule (V90a) and the sandbox. This
/// keeps purlis itself from raising a question on the chat's behalf, and says what to do.
pub fn refused_in_this_chat(ctx: &Ctx, vault: &registry::Vault, reader: Reader) -> Option<String> {
    let env = |name: &str| ctx.env.get(name);
    if !crate::sandbox::chat_is_sandboxed_in(&env) || !super::keyring::holds_values_of(ctx, vault) {
        return None;
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
    Some(format!(
        "vault '{}' is read from {}, which a sandboxed chat reaches only through the app, and \
         {why}. purlis does not read {} from inside a sandboxed chat.\n  {next}",
        vault.name,
        super::keyring::STORE_NAME,
        super::keyring::STORE_NAME,
    ))
}

/// Runs `req` through the app that started this chat, and answers its status, or `None` when
/// no app takes it and it is to run here.
///
/// Only inside a chat the app started sandboxed ([`crate::sandbox::chat_is_sandboxed_in`]): an
/// unsandboxed chat reads its vault as the person does. A chat that claims to be sandboxed when
/// it is not gets the stricter run, and one that hides it gets the vault's own refusal.
///
/// A persona's MCP server's start ([`super::mcpstart::server_in`]) is tried once more where the
/// first run failed before the server spoke ([`started_server`]).
#[cfg(unix)]
pub fn forwarded(ctx: &Ctx, req: &Request, command: &[String], io: &mut dyn Io) -> Option<i32> {
    let env = |name: &str| ctx.env.get(name);
    if !crate::sandbox::chat_is_sandboxed_in(&env) {
        return None;
    }
    let socket = env(crate::hookwire::SOCKET_ENV).filter(|s| !s.is_empty())?;
    let chat = chat_number(&env)?;
    let token = crate::hookwire::ChatToken::read(&env);
    let connect = || std::os::unix::net::UnixStream::connect(&socket).ok();
    if let Some(server) = super::mcpstart::server_in(&env) {
        return started_server(ctx, req, command, chat, token, &server, &connect, io);
    }
    converse(ctx, req, command, chat, token, connect()?, io)
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
    loop {
        let mut said = String::new();
        let read = (&mut reader).take(A_FRAME_IS_AT_MOST).read_line(&mut said);
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

/// Where there is no unix socket there is no app to ask, and the command runs here.
#[cfg(not(unix))]
pub fn forwarded(_ctx: &Ctx, _req: &Request, _command: &[String], _io: &mut dyn Io) -> Option<i32> {
    None
}

// ----------------------------------------------------------------------------------------
// the app: authorise, resolve, run

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
    let doc = registry::load_registry(ctx).map_err(|e| e.message)?;
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
const ALLOW: &str = "ask the operator to press Allow in the notice on this chat's tab, then run \
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
    let doc = registry::load_registry(ctx).map_err(|e| e.message)?;
    let registered = registry::vault_in(&doc, vault).map_err(|e| e.message)?;
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
    let doc = registry::load_registry(ctx).map_err(|e| e.message)?;
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
    let v = cmd::provider(ctx, vault).map_err(|e| e.message)?;
    let mut keys = cmd::keys(ctx, &v).map_err(|e| e.message)?;
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

/// **What reading a value directly is refused with in a sandboxed chat** (qw385): `secret
/// get` and `secret cp` (and their `persona secret` forms), for every provider. A sandboxed
/// chat never reads a vault, and no grant changes that; the route that works is a tool its
/// persona declares, handed the value by `secret exec`, which the app runs. `None` outside a
/// sandboxed chat.
pub fn value_refused_in_this_chat(ctx: &Ctx, vault: &str) -> Option<String> {
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
    Some(format!(
        "a sandboxed chat does not read a vault's values, so purlis did not read '{vault}', and \
         no Allow changes that.\n  To check which keys it holds: purlis secret list {vault}\n  \
         {to_use}"
    ))
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

/// [`serve`], telling `reached` each connection the run made (#1667): through its proxy, and
/// through a tunnel to a database host its values point at ([`crate::sandbox::tunnel`]). The
/// network record keeps them under the asking chat. A host that carries one of the run's values
/// is counted and never named.
pub fn serve_recording(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    told: Told,
    reached: crate::sandbox::egress::Reached,
) {
    serve_recorded(
        asker,
        wanted,
        reader,
        writer,
        Wrap::here(),
        told,
        Some(reached),
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

/// Whether `wanted` running `command` is exactly the `secret exec` line of a credentialed MCP
/// server of `persona` this machine approved ([`crate::personaverbs::chatstart::approved_lines`]):
/// its vault, its `--env` and `--file` hand-overs in order, no `--dotenv`, and its command.
fn an_approved_server(root: &Path, persona: &str, wanted: &Wanted, command: &[String]) -> bool {
    let state = crate::personaverbs::state_dir(root);
    crate::personaverbs::chatstart::approved_lines(root, &state, persona)
        .iter()
        .any(|words| approved_line_is(words, wanted, command))
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
    login_and_identity_for(asker, name, unconfined).map(|(login, _)| login)
}

/// **[`login_for`], and the login's identity** (#2021): a digest of the persona, the project
/// and the login as declared and allowed, which its kept store is bound to
/// ([`crate::browserlogins::identity_of`]), so a login of the same name of another persona or
/// project, or declared otherwise since, never opens it.
pub fn login_and_identity_for(
    asker: &Asker,
    name: &str,
    unconfined: bool,
) -> Result<(purlis_session_protocol::browsing::LogIn, String), String> {
    use purlis_session_protocol::browsing::{LogIn, Secret};
    let (ready, ctx) = login_ready(asker, name, unconfined)?;
    let (Some(login), Some((user_key, pass_key))) = (&ready.tool.login, ready.tool.login_keys())
    else {
        return Err(format!("{name} is not a browser login"));
    };
    let vault = &ready.tool.vault;
    let v = cmd::provider(&ctx, vault).map_err(|e| e.message)?;
    let read = |key: &str| {
        cmd::get_value(&ctx, &v, key).map(Secret::new).map_err(|e| {
            format!(
                "vault '{vault}' did not give {name}'s login, so nothing was filled ({})",
                e.message
            )
        })
    };
    let identity = identity_of_ready(asker, name, &ready)?;
    Ok((
        LogIn {
            address: login.url.clone(),
            user: user_key.map(read).transpose()?,
            password: read(pass_key)?,
            keep: None,
            identity: Some(identity.clone()),
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
                "{persona} declares no browser login {name}, so nothing was filled. A login is \
                 declared in the project's settings, [sandbox.personas.{persona}] tools = {{ \
                 {name} = {{ vault = \"<vault>\", browser = \"https://…\", key = \"<key>\" }} \
                 }}, and allowed on this machine"
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
         hosts do not list. It is the sandbox, not the server. The person can allow it on this \
         chat's tab (Allow for this chat, or Always allow); the chat is then started again \
         with it, and the command can be run again."
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
         are neither named nor offered on the chat's tab."
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

/// The variables of the asker's environment a child never starts with: the chat's hook channel
/// and its token (the child cannot reach the socket, and needs neither), and the ones the wrap
/// sets itself. Each under its old prefix too ([`not_handed_on`]).
const NOT_HANDED_ON: &[&str] = &[crate::hookwire::SOCKET_ENV, crate::hookwire::TOKEN_ENV];

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
    serve_recorded(asker, wanted, reader, writer, wrap, told, None);
}

pub(crate) fn serve_recorded(
    asker: &Asker,
    wanted: Wanted,
    reader: Box<dyn BufRead + Send>,
    mut writer: Box<dyn Write + Send>,
    wrap: Wrap,
    told: Told,
    reached: Option<crate::sandbox::egress::Reached>,
) {
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
    // What this chat may write is the app's record of it, and no provider's program is run
    // from there (#1516).
    let ctx = ctx.chat(confines, asker.folder.as_deref());
    if let Err(why) = authorise(&ctx, asker.persona.as_deref(), &wanted.vault) {
        return refuse(&mut *writer, 1, why);
    }
    // A sandboxed chat runs only its persona's declared tools with a vault (D-PE-ruling-2, the
    // operator's ruling of 2026-10-12): exactly a tool its start armed, allowed on this machine,
    // with that tool's vault and hand-over. The run is of the tool's own program, by the
    // absolute path the app found where no chat writes, never one the chat's `PATH` names.
    let persona = asker.persona.as_deref().unwrap_or_default();
    let listed = declared_now(&asker.root, persona);
    let mut declared_as: Option<String> = None;
    match crate::sandbox::tools::declared(
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
    ) {
        // A browser login is purlis's to perform in the chat's own browser (#2016): run as a
        // command, it runs nothing.
        Ok(ready) if ready.tool.login.is_some() => {
            return refuse(&mut *writer, 2, login_by_the_browser_tool(&ready.tool.name));
        }
        // Any other tool, its own program.
        Ok(ready) => match crate::sandbox::tools::run_words(ready, &command[1..]) {
            Ok(words) => {
                declared_as = ready.argv0().map(str::to_owned);
                command = words;
            }
            Err(why) => return refuse(&mut *writer, 1, why),
        },
        // A persona's credentialed MCP server this machine approved, started by the chat's
        // harness, asks for its line as approved: the person's consent, as an Allow is.
        Err(_) if an_approved_server(&asker.root, persona, &wanted, &command) => {}
        Err(why) => return refuse(&mut *writer, 1, why),
    }
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
    let v = match cmd::provider(&ctx, &wanted.vault) {
        Ok(v) => v,
        Err(e) => return refuse(&mut *writer, 1, e.message),
    };
    // What runs beside the child for as long as it does: the egress proxy carrying the chat's
    // hosts, telling the chat's tab each host it refuses, and a temp directory of its own.
    // A host that carries one of the run's values is never told: no Notice names it and no
    // grant is made from it. The values are known once the run is prepared, which is after its
    // proxy starts; until then nothing is told.
    let values: Arc<std::sync::OnceLock<Vec<String>>> = Arc::default();
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
    // temp directory), then every other vault's identity variables taken out.
    // A name is judged as it reads, with any byte that is not UTF-8 replaced, so one is never
    // let through for not spelling the name it is judged by.
    let mut base: Vec<(OsString, OsString)> = wanted
        .environment
        .into_iter()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            !not_handed_on(&k) && !crate::sandbox::tools::loads_code(&k)
        })
        .collect();
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
        Err(e) => return refuse(&mut *writer, 1, e.message),
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
    let prepared = match exec::prepare(&ctx, &v, &req, base, Some(&within), &|| None) {
        Ok(prepared) => prepared,
        Err(Stopped::Said(code, why)) => return refuse(&mut *writer, code, why),
        Err(Stopped::Signal(code)) => return refuse(&mut *writer, code, "stopped".to_owned()),
    };
    let _ = values.set(prepared.secret_values.clone());
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
    let start_with = match &applying {
        Applying::Purlis(_) => purlis_linux_sandbox::carried::carried(prepared.env.iter().cloned()),
        _ => prepared.env.clone(),
    };
    child
        .args(&command[1..])
        .current_dir(start_in)
        .env_clear()
        .envs(start_with)
        .stdin(if wanted.stdin {
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

    let mut spawned = match crate::forklock::spawn(&mut child) {
        Ok(spawned) => spawned,
        Err(e) => return refuse(&mut *writer, 1, format!("cannot run {}: {e}", command[0])),
    };
    let group = Live::hold(spawned.id());
    if cmd_said_note(&ctx, &v, &mut *writer).is_err()
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
            .map(str::to_owned)
        else {
            continue;
        };
        // A host alone in its variable, its port beside it in another (#1708).
        let pair = host_pair(name, &value, &prepared.env);
        let routed = pair.as_ref().map_or(value, |pair| pair.target.clone());
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
                *value = pointed.at(at);
                prepared.secret_values.push(value.clone());
                let escaped = super::dotenv::escaped(value);
                if escaped != *value {
                    prepared.secret_values.push(escaped);
                }
                moved = true;
            }
        }
        if moved {
            std::fs::write(&handed.path, handed_body(&handed.entries)?)?;
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
pub struct Redactor {
    secrets: Vec<String>,
    longest: usize,
    held: Vec<u8>,
}

impl Redactor {
    pub fn new(secrets: &[String]) -> Self {
        let secrets: Vec<String> = secrets.iter().filter(|s| !s.is_empty()).cloned().collect();
        let longest = secrets.iter().map(String::len).max().unwrap_or(0);
        Self {
            secrets,
            longest,
            held: Vec::new(),
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
            for secret in &self.secrets {
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

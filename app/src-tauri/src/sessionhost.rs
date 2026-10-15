// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host the window starts, and is the first client of (ADR 0068 §1, #1997).
//!
//! **The window's own binary runs the host.** The app starts itself again with
//! [`purlis_host::SERVE_FOR_THE_WINDOW`] (`main.rs` answers it before anything of the app
//! starts), so the host is the same program, with the same Keychain identity, and nothing
//! prompts (ADR 0068 §1, amended 2026-10-14). One host serves every project open in the window.
//!
//! **It lives as long as the window.** The window holds the other end of the host's standard
//! input; the host stops when it closes, so a quit, a crash and a kill all end it. At a quit the
//! window closes it and waits a moment for the host to go ([`SessionHost::end`]).
//!
//! **Its folder is made first, the host is started off the first paint.** The socket's folder
//! is made this user's alone on the window's own start, before any chat can start, so every
//! chat's sandbox denies a folder that is there; the host then starts on a thread of its own,
//! and nothing the window draws waits for it. Once it says it is ready, the window connects as `local-ui` with the
//! credential the host minted, and holds the link.
//!
//! **Chats run in it behind the start-time switch** (#1998, `crate::hostedsessions`): the link
//! the window holds is what every project's chats run over when the switch is on, and with no
//! host of the window's own, those chats are refused with the reason ([`Connection`]). Wherever
//! chats run, the host takes every process below the window as inside a chat and refuses it a
//! person's scope (`purlis_host::Window`). What this host says goes to the window's log.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::{link, session};

use crate::hostedsessions::{Connection, HostLink};

/// How long the window waits for its host to say it is ready.
const READY_WITHIN: Duration = Duration::from_secs(20);

/// How long a quit waits for the host to end once its standard input is closed, before it is
/// killed.
const ENDS_WITHIN: Duration = Duration::from_secs(2);

/// The exit code a host gives when another host of this user already serves on this device
/// (`purlis_host::HostError::AlreadyServing`).
const ANOTHER_SERVES: i32 = 3;

/// Where this user's session host listens and keeps its credentials on this device: the place
/// the window's host takes and `purlis serve` takes, so each finds the other.
pub fn place() -> Option<purlis_host::Place> {
    let config = purlis_core::machine::config_root()?;
    Some(purlis_host::Place::at(
        purlis_core::hostsocket::here(),
        purlis_core::machine::charterd(&config),
    ))
}

/// The command that starts `binary`, the window's own, as its session host: the host's
/// standard input is the window's to hold, and what it says is read for the window's log.
pub fn command(binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .arg(purlis_host::SERVE_FOR_THE_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// What a host that did not become ready said by its exit, for the window's log.
pub fn why_not_ready(code: Option<i32>) -> &'static str {
    match code {
        Some(ANOTHER_SERVES) => {
            "another session host already serves this user on this device; this window \
             leaves it serving and does not connect to it"
        }
        _ => "the session host ended before it was ready",
    }
}

/// A chat the window composed, as the window would start it: the core's composition, against
/// the environment the window started this host with (#1998). The host's [`purlis_host::Compose`]
/// for the window's binary.
pub fn launched(
    launch: &session::Launch,
    number: u32,
    size: purlis_core::engine::Size,
) -> Result<purlis_core::session::Spec, String> {
    purlis_core::chatspec::launched(
        std::env::vars_os(),
        &purlis_core::chatspec::Launched {
            program: &launch.program,
            args: &launch.args,
            cwd: launch.cwd.as_deref(),
            set: &launch.set,
            strip: &launch.strip,
            pass: &launch.pass,
            declared: &launch.declared,
            harness: launch.harness.as_deref(),
            git_hooks: launch.git_hooks.as_deref(),
            reporting: launch.reporting.as_ref().map(|reporting| {
                purlis_core::chatspec::Reporting {
                    socket: Path::new(&reporting.socket),
                    token: &reporting.token,
                }
            }),
        },
        number,
        size,
    )
}

/// What hears each held project's hooks in the window's session host (#2000): the host's
/// [`purlis_host::Hear`] for the window's binary.
///
/// **The project's hook channel, as the window bound it before.** At the same place
/// ([`crate::hooks::socket_for`]), with the same listener and every one of its checks: the
/// token, the process that sent it, each framed ask again, the spool's keys beside it. Only who
/// listens changes, and the host is each chat's ancestor, so a hook admits it. Each line it
/// admits is handed on to the window ([`purlis_core::hookwire::relay`]).
pub struct HookChannels;

impl purlis_host::Hear for HookChannels {
    fn open(
        &self,
        _project: &str,
        path: &Path,
        hand_on: purlis_host::HandOn,
    ) -> Result<Option<Arc<dyn purlis_host::Channel>>, String> {
        use purlis_core::hookwire::relay::Relaying;
        let at = crate::hooks::socket_for(Some(path));
        let listener = purlis_core::hookwire::Listener::bind(&at.within, &at.socket)
            .map_err(|why| format!("its hook socket at {} ({why})", at.socket.display()))?;
        let socket = listener.path().to_path_buf();
        let tokens = listener.tokens();
        let relaying = Relaying::new(Box::new(move |id, heard| {
            serde_json::to_value(heard).is_ok_and(|line| hand_on(id, line))
        }));
        let reading = listener.hear(relaying.hearing(Arc::clone(&tokens)));
        Ok(Some(Arc::new(HookChannel {
            socket,
            tokens,
            relaying,
            reading: Mutex::new(Some(reading)),
        })))
    }
}

/// One held project's hook channel in the host.
struct HookChannel {
    socket: PathBuf,
    tokens: Arc<purlis_core::hookwire::ChatTokens>,
    relaying: Arc<purlis_core::hookwire::relay::Relaying>,
    reading: Mutex<Option<purlis_core::hookwire::Reading>>,
}

impl purlis_host::Channel for HookChannel {
    fn report_to(&self, number: u32) -> Result<session::Reporting, String> {
        let token = self.tokens.issue(number).map_err(|why| why.to_string())?;
        Ok(session::Reporting {
            socket: self.socket.display().to_string(),
            token: token.expose().to_owned(),
        })
    }

    fn bind(&self, number: u32, pid: u32) {
        match purlis_core::hookwire::Program::of(pid) {
            Some(program) => self.tokens.bind(number, program),
            None => purlis_host_says(&format!(
                "chat {number}'s program (pid {pid}) could not be confirmed as it started, so \
                 none of its lines will be read until it is restarted"
            )),
        }
    }

    fn ended(&self, number: u32, pid: u32) {
        self.tokens.program_ended(number, pid);
    }

    fn forget(&self, number: u32) {
        self.tokens.forget(number);
    }

    fn replied(&self, id: u64, reply: serde_json::Value) {
        match serde_json::from_value(reply) {
            Ok(replied) => self.relaying.replied(id, replied),
            Err(why) => {
                purlis_host_says(&format!("a reply the window sent could not be read: {why}"))
            }
        }
    }
}

/// The channel stops: every line still waiting on the window is let go of, then the socket.
impl Drop for HookChannel {
    fn drop(&mut self) {
        self.relaying.let_go();
        drop(
            self.reading
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
    }
}

/// One line in the host's log, which the window keeps.
fn purlis_host_says(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "purlisd: {line}");
}

/// The window's session host, while it runs.
#[derive(Default)]
pub struct SessionHost {
    held: Arc<Mutex<Option<Held>>>,
    /// The link to it, for the chats that run in it.
    connection: Arc<Connection>,
}

struct Held {
    child: Child,
    /// The host's standard input: closing it ends the host.
    stdin: Option<ChildStdin>,
}

impl SessionHost {
    /// Makes the host's socket folder this user's alone now, on the calling thread, so that
    /// every chat's sandbox, compiled from here on, denies a folder that is there; then starts
    /// the host on a thread of its own and connects to it once it is ready. Nothing waits for
    /// the host; what goes wrong is said in the log.
    pub fn start(&self) {
        let connection = &self.connection;
        let Some(place) = place() else {
            let why = "there is no home folder to keep its credentials in";
            tracing::warn!("purlis: no session host: {why}");
            connection.gone(why);
            return;
        };
        if let Err(why) = purlis_core::hostsocket::make_the_folder(&place.socket) {
            let why = format!(
                "its folder {} could not be made this user's alone: {why}",
                place.socket.parent().unwrap_or(&place.socket).display()
            );
            tracing::warn!("purlis: no session host: {why}");
            connection.gone(why);
            return;
        }
        let held = Arc::clone(&self.held);
        let connection = Arc::clone(connection);
        let spawned = std::thread::Builder::new()
            .name("session host".into())
            .spawn({
                let connection = Arc::clone(&connection);
                move || start_here(&held, place, &connection)
            });
        if let Err(why) = spawned {
            tracing::warn!("purlis: the session host was not started: {why}");
            connection.gone(format!("it was not started: {why}"));
        }
    }

    /// The link to the host, for the chats that run in it: ready once the host is, or gone with
    /// the reason there is none.
    pub fn connection(&self) -> Arc<Connection> {
        Arc::clone(&self.connection)
    }

    /// Ends the host: closes its standard input, waits [`ENDS_WITHIN`] for it to go, and kills
    /// it if it has not. For the app's exit.
    pub fn end(&self) {
        end_held(&self.held);
    }

    /// What ends the host from elsewhere, as [`Self::end`] does: a quit that the host did not
    /// answer in time ends it before the save at quit (#2002). Ending it twice ends it once.
    pub fn ender(&self) -> Arc<dyn Fn() + Send + Sync> {
        let held = Arc::clone(&self.held);
        Arc::new(move || end_held(&held))
    }
}

/// Ends the host `held` holds, if it still does: closes its standard input, waits
/// [`ENDS_WITHIN`] for it to go, and kills it if it has not.
fn end_held(held: &Mutex<Option<Held>>) {
    let Some(mut held) = held.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    drop(held.stdin.take());
    let deadline = Instant::now() + ENDS_WITHIN;
    while Instant::now() < deadline {
        if matches!(held.child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = held.child.kill();
    let _ = held.child.wait();
}

/// Starts the host and connects to it, on the calling thread.
fn start_here(held: &Mutex<Option<Held>>, place: purlis_host::Place, connection: &Arc<Connection>) {
    let binary = match std::env::current_exe() {
        Ok(binary) => binary,
        Err(why) => {
            tracing::warn!("purlis: no session host: this program's path is unknown: {why}");
            connection.gone(format!("this program's path is unknown: {why}"));
            return;
        }
    };
    let mut child = match purlis_core::forklock::spawn(&mut command(&binary)) {
        Ok(child) => child,
        Err(why) => {
            tracing::warn!("purlis: the session host did not start: {why}");
            connection.gone(format!("it did not start: {why}"));
            return;
        }
    };
    if let Some(said) = child.stderr.take() {
        log_what_it_says(said);
    }
    let stdout = child.stdout.take();
    let stdin = child.stdin.take();
    let pid = child.id();
    *held.lock().unwrap_or_else(|e| e.into_inner()) = Some(Held { child, stdin });

    if !stdout.is_some_and(said_ready) {
        let code = wait_for_its_end(held, Instant::now() + ENDS_WITHIN);
        tracing::warn!("purlis: {} (process {pid})", why_not_ready(code));
        connection.gone(why_not_ready(code));
        return;
    }
    tracing::info!(
        "purlis: the session host serves at {}",
        place.socket.display()
    );
    connect(place.socket, place.credentials, Arc::clone(connection));
}

/// The host's exit code, once it has ended by `deadline`.
fn wait_for_its_end(held: &Mutex<Option<Held>>, deadline: Instant) -> Option<i32> {
    while Instant::now() < deadline {
        let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(Ok(Some(status))) = held.as_mut().map(|held| held.child.try_wait()) {
            *held = None;
            return status.code();
        }
        drop(held);
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Whether the host's first line says it is ready, within [`READY_WITHIN`]. A host that ends
/// first, or says anything else, is not.
fn said_ready(stdout: std::process::ChildStdout) -> bool {
    let (told, heard) = std::sync::mpsc::channel();
    let reading = std::thread::Builder::new()
        .name("session host ready".into())
        .spawn(move || {
            let mut line = String::new();
            let read = BufReader::new(stdout).read_line(&mut line);
            let _ = told.send(read.is_ok() && line.trim_end() == purlis_host::READY);
        });
    reading.is_ok() && heard.recv_timeout(READY_WITHIN).unwrap_or(false)
}

/// Each line the host says on its standard error, into the window's log.
fn log_what_it_says(said: std::process::ChildStderr) {
    let _ = std::thread::Builder::new()
        .name("session host log".into())
        .spawn(move || {
            for line in BufReader::new(said).lines().map_while(Result::ok) {
                tracing::info!("{line}");
            }
        });
}

/// Connect to the host as `local-ui`, the window's scope, and hold the link for as long as the
/// window runs: the chats that run in the host go over it ([`HostLink`]).
fn connect(socket: PathBuf, credentials: PathBuf, connection: Arc<Connection>) {
    tauri::async_runtime::spawn(async move {
        let credential = match Credential::read(&credentials, Scope::LocalUi) {
            Ok(credential) => credential,
            Err(why) => {
                tracing::warn!("purlis: the session host's credential could not be read: {why}");
                connection.gone(format!("its credential could not be read: {why}"));
                return;
            }
        };
        let linked = match tokio::net::UnixStream::connect(&socket).await {
            Ok(stream) => {
                link::connect(stream, session::speaks(), Scope::LocalUi, &credential).await
            }
            Err(why) => {
                tracing::warn!("purlis: the session host's socket did not answer: {why}");
                connection.gone(format!("its socket did not answer: {why}"));
                return;
            }
        };
        let link = match linked {
            Ok(link) => link,
            Err(why) => {
                tracing::warn!("purlis: the session host refused the window: {why}");
                connection.gone(format!("it refused the window: {why}"));
                return;
            }
        };
        let ended = {
            let connection = Arc::clone(&connection);
            move || {
                tracing::info!("purlis: the link to the session host ended");
                connection.gone("the link to it ended");
            }
        };
        match HostLink::over(link, tokio::runtime::Handle::current(), ended).await {
            Ok(link) => connection.ready(Arc::new(link)),
            Err(why) => {
                tracing::warn!("purlis: {why}");
                connection.gone(why);
            }
        }
    });
}

#[cfg(test)]
mod tests;

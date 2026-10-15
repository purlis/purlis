// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Starting and ending an engine: one process group per browser, led by its keeper, with
//! DevTools on an inherited pipe.
//!
//! **The group.** [`EngineGroup::start`] starts the keeper ([`crate::keeper`]) as the leader
//! of a new process group, its stdin the lifeline. The engine is then started into that group.
//! While the keeper has not been reaped, the group's id cannot be dealt again, so ending the
//! group with `killpg` can never reach a process that is not the engine's.
//!
//! **The pipe.** The engine is started with `posix_spawn`, whose file actions put the read end
//! of one pipe at descriptor 3 and the write end of another at descriptor 4, which is where
//! `--remote-debugging-pipe` looks. The engine inherits those two and its standard streams and
//! nothing else, whatever the host holds open: on macOS `POSIX_SPAWN_CLOEXEC_DEFAULT` closes
//! every descriptor the file actions do not place, and on Linux the file actions close each
//! descriptor the host has open (`/proc/self/fd`). No port is opened: nothing else on the
//! machine can reach DevTools.
//!
//! **The end.** [`Engine::stop`] asks the engine to close, gives it a moment, then kills the
//! group. Dropping an [`Engine`] kills the group at once. When the host itself ends, however it
//! ends, the engine's DevTools pipe and the keeper's lifeline both close, and the keeper kills
//! the group within [`crate::keeper::GRACE`].

#![cfg(unix)]

use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io::{self, PipeReader, PipeWriter, Read};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::spawn::{PosixSpawnAttr, PosixSpawnFileActions, PosixSpawnFlags, posix_spawn};
use nix::sys::signal::{SigSet, Signal, killpg};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use serde_json::{Value, json};

use crate::WebAddress;
use crate::cdp::{CallError, Connection, Event};
use crate::launch::Launch;

/// How long a starting engine has to answer over its pipe: a cold start on a busy machine.
pub const FIRST_ANSWER: Duration = Duration::from_secs(60);

/// How the keeper is run: a program and the arguments that put it in its keeper's part.
#[derive(Debug, Clone)]
pub struct Keeper {
    /// The executable.
    pub program: PathBuf,
    /// Its arguments, such as the mode word of a binary that runs the keeper among other
    /// things.
    pub args: Vec<OsString>,
}

impl Keeper {
    /// A keeper that is a program of its own, run with no arguments (`purlis-engine-keeper`).
    pub fn program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }
}

/// One engine's process group: its keeper, and the lifeline whose end ends the group.
pub struct EngineGroup {
    keeper: Child,
    /// Held, never written: the kernel closes it when this process ends, however it ends.
    lifeline: Option<PipeWriter>,
    ended: bool,
}

impl EngineGroup {
    /// Starts `keeper` as the leader of a new process group.
    pub fn start(keeper: &Keeper) -> io::Result<Self> {
        let (lifeline_read, lifeline) = io::pipe()?;
        let keeper = Command::new(&keeper.program)
            .args(&keeper.args)
            .env_clear()
            .stdin(Stdio::from(lifeline_read))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        let mut group = Self {
            keeper,
            lifeline: Some(lifeline),
            ended: false,
        };
        // Nothing joins the group until its keeper says it guards it.
        let mut said = vec![0_u8; crate::keeper::READY.len()];
        let ready = group
            .keeper
            .stdout
            .take()
            .map(|mut out| out.read_exact(&mut said));
        if !matches!(ready, Some(Ok(()))) || said != crate::keeper::READY {
            return Err(io::Error::other("the engine's keeper did not start"));
        }
        Ok(group)
    }

    /// The process group's id, which is the keeper's pid.
    pub fn id(&self) -> i32 {
        i32::try_from(self.keeper.id()).expect("a pid fits in an i32")
    }

    /// Makes `command` start in this group.
    pub fn join(&self, command: &mut Command) {
        command.process_group(self.id());
    }

    /// Kills every process in the group, and reaps the keeper.
    pub fn end(mut self) {
        self.kill_now();
    }

    fn kill_now(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;
        // The keeper is not reaped yet, so the group's id still names this group alone.
        let _ = killpg(Pid::from_raw(self.id()), Signal::SIGKILL);
        let _ = self.keeper.wait();
        self.lifeline.take();
    }
}

impl Drop for EngineGroup {
    fn drop(&mut self) {
        self.kill_now();
    }
}

/// Why an engine did not start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// The data folder could not be made, or its preferences written.
    #[error("the browser's data folder could not be prepared: {0}")]
    DataFolder(#[source] io::Error),
    /// The keeper, the pipes or the engine itself could not be started.
    #[error("the engine could not be started: {0}")]
    Spawn(#[source] io::Error),
    /// The engine started and did not answer over its pipe.
    #[error("the engine started but did not answer over its pipe: {0}")]
    NoAnswer(#[source] CallError),
}

/// A running engine: its process group, and DevTools over its pipe.
pub struct Engine {
    pid: Pid,
    reaped: bool,
    connection: Option<Connection>,
    events: Option<Receiver<Event>>,
    group: EngineGroup,
}

impl Engine {
    /// Starts the engine `launch` describes, in a new process group led by `keeper`, and waits
    /// until it answers over its pipe.
    pub fn start(launch: &Launch, keeper: &Keeper) -> Result<Self, StartError> {
        launch.prepare_data_dir().map_err(StartError::DataFolder)?;
        let group = EngineGroup::start(keeper).map_err(StartError::Spawn)?;
        let (engine_reads, host_writes) = io::pipe().map_err(StartError::Spawn)?;
        let (host_reads, engine_writes) = io::pipe().map_err(StartError::Spawn)?;
        let log = File::create(launch.data_dir().join("engine.log")).map_err(StartError::Spawn)?;
        let null = File::open("/dev/null").map_err(StartError::Spawn)?;
        let environment = launch.environment(std::env::vars_os());
        let pid = spawn_engine(
            launch,
            &environment,
            group.id(),
            &engine_reads,
            &engine_writes,
            &null,
            &log,
        )
        .map_err(StartError::Spawn)?;
        // The engine holds its own copies now; the host's must go, or the engine would never
        // read the end of its pipe when the host ends.
        drop((engine_reads, engine_writes));
        let (connection, events) = Connection::new(host_reads, host_writes);
        let engine = Self {
            pid,
            reaped: false,
            connection: Some(connection),
            events: Some(events),
            group,
        };
        engine
            .devtools()
            .call_within("Browser.getVersion", json!({}), None, FIRST_ANSWER)
            .map_err(StartError::NoAnswer)?;
        Ok(engine)
    }

    /// DevTools over the engine's pipe, for this crate only: what reaches the engine from
    /// outside goes through typed calls, and every one that opens a page takes a
    /// [`WebAddress`].
    pub(crate) fn devtools(&self) -> &Connection {
        self.connection
            .as_ref()
            .expect("the connection lives as long as the engine")
    }

    /// The engine's product and version, as DevTools reports them (`Browser.getVersion`).
    pub fn version(&self) -> Result<Value, CallError> {
        self.devtools().call("Browser.getVersion", json!({}), None)
    }

    /// The engine's events, once: the broker takes them to read on its own thread.
    pub fn take_events(&mut self) -> Option<Receiver<Event>> {
        self.events.take()
    }

    /// The engine's own pid.
    pub fn pid(&self) -> u32 {
        self.pid.as_raw().unsigned_abs()
    }

    /// The process group every process of this engine is in.
    pub fn process_group(&self) -> i32 {
        self.group.id()
    }

    /// Opens a page at `address` and returns its target id. Only an http or https address can
    /// be given: [`WebAddress`] refuses every other scheme before the engine is asked.
    pub fn open_page(&self, address: &WebAddress) -> Result<String, CallError> {
        let created = self.devtools().call(
            "Target.createTarget",
            json!({ "url": address.as_str() }),
            None,
        )?;
        Ok(created
            .get("targetId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    /// Whether the engine's own process has ended.
    pub fn has_ended(&mut self) -> bool {
        self.forget_reaped();
        self.reaped
    }

    /// Asks the engine to close, waits up to `grace` for it, then ends its whole group.
    pub fn stop(mut self, grace: Duration) {
        let _ = self
            .devtools()
            .call_within("Browser.close", json!({}), None, grace);
        let started = Instant::now();
        while !self.has_ended() && started.elapsed() < grace {
            std::thread::sleep(Duration::from_millis(20));
        }
        self.end_now();
    }

    fn forget_reaped(&mut self) {
        if self.reaped {
            return;
        }
        match waitpid(self.pid, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) | Err(Errno::EINTR) => {}
            Ok(_) | Err(_) => self.reaped = true,
        }
    }

    fn end_now(&mut self) {
        // Closing the pipe first lets an engine that is still reading see its end.
        self.connection.take();
        self.group.kill_now();
        if !self.reaped {
            let _ = waitpid(self.pid, None);
            self.reaped = true;
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.end_now();
    }
}

/// `posix_spawn`s the engine into `group`, with `devtools_in` at descriptor 3, `devtools_out`
/// at 4, stdin from `null`, and stdout and stderr to `log`.
fn spawn_engine(
    launch: &Launch,
    environment: &[(OsString, OsString)],
    group: i32,
    devtools_in: &PipeReader,
    devtools_out: &PipeWriter,
    null: &File,
    log: &File,
) -> io::Result<Pid> {
    let (fd_in, fd_out) = (devtools_in.as_raw_fd(), devtools_out.as_raw_fd());
    // Each source goes to a number above every descriptor involved before it goes to 3 or 4,
    // so no dup2 overwrites a source still to be moved, and none is a dup2 onto itself (which
    // would leave it close-on-exec).
    let open_now = open_descriptors()?;
    let above = [fd_in, fd_out, null.as_raw_fd(), log.as_raw_fd(), 4]
        .into_iter()
        .chain(open_now.iter().copied())
        .max()
        .unwrap_or(4)
        + 1;
    let (spare_in, spare_out) = (above, above + 1);
    let mut actions = PosixSpawnFileActions::init().map_err(io::Error::from)?;
    actions.add_dup2(fd_in, spare_in).map_err(io::Error::from)?;
    actions
        .add_dup2(fd_out, spare_out)
        .map_err(io::Error::from)?;
    actions
        .add_dup2(null.as_raw_fd(), 0)
        .map_err(io::Error::from)?;
    actions
        .add_dup2(log.as_raw_fd(), 1)
        .map_err(io::Error::from)?;
    actions
        .add_dup2(log.as_raw_fd(), 2)
        .map_err(io::Error::from)?;
    // Everything else the host holds goes (Linux; macOS closes it by the spawn's flag below).
    for fd in open_now.iter().copied().filter(|&fd| fd > 2) {
        actions.add_close(fd).map_err(io::Error::from)?;
    }
    actions.add_dup2(spare_in, 3).map_err(io::Error::from)?;
    actions.add_dup2(spare_out, 4).map_err(io::Error::from)?;
    actions.add_close(spare_in).map_err(io::Error::from)?;
    actions.add_close(spare_out).map_err(io::Error::from)?;

    let mut attr = PosixSpawnAttr::init().map_err(io::Error::from)?;
    attr.set_pgroup(Pid::from_raw(group))
        .map_err(io::Error::from)?;
    // A Rust host ignores SIGPIPE, and an ignored signal survives exec; the engine gets every
    // signal back at its default, and none blocked, as `Command` would give it.
    let mut defaults = SigSet::empty();
    defaults.add(Signal::SIGPIPE);
    attr.set_sigdefault(&defaults).map_err(io::Error::from)?;
    attr.set_sigmask(&SigSet::empty())
        .map_err(io::Error::from)?;
    attr.set_flags(
        PosixSpawnFlags::POSIX_SPAWN_SETPGROUP
            | PosixSpawnFlags::POSIX_SPAWN_SETSIGDEF
            | PosixSpawnFlags::POSIX_SPAWN_SETSIGMASK
            | close_the_rest(),
    )
    .map_err(io::Error::from)?;

    let program = c_string(launch.program().as_os_str())?;
    let mut args = vec![program.clone()];
    for arg in launch.arguments() {
        args.push(c_string(&arg)?);
    }
    let mut env = Vec::with_capacity(environment.len());
    for (name, value) in environment {
        let mut pair = name.clone();
        pair.push("=");
        pair.push(value);
        env.push(c_string(&pair)?);
    }
    posix_spawn(program.as_c_str(), &actions, &attr, &args, &env).map_err(io::Error::from)
}

/// macOS's own way to start a program with only the descriptors its file actions place.
#[cfg(target_os = "macos")]
fn close_the_rest() -> PosixSpawnFlags {
    PosixSpawnFlags::from_bits_retain(nix::libc::POSIX_SPAWN_CLOEXEC_DEFAULT)
}

#[cfg(not(target_os = "macos"))]
fn close_the_rest() -> PosixSpawnFlags {
    PosixSpawnFlags::empty()
}

/// The descriptors this process holds open, for the file actions to close in the engine. On
/// macOS the spawn's flag closes them, so none are listed. A close of one that is no longer open
/// by the time of the spawn is passed over by the C library.
#[cfg(target_os = "macos")]
fn open_descriptors() -> io::Result<Vec<i32>> {
    Ok(Vec::new())
}

#[cfg(not(target_os = "macos"))]
fn open_descriptors() -> io::Result<Vec<i32>> {
    let mut open = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd")? {
        if let Some(fd) = entry?.file_name().to_str().and_then(|n| n.parse().ok()) {
            open.push(fd);
        }
    }
    Ok(open)
}

fn c_string(text: &OsStr) -> io::Result<CString> {
    CString::new(text.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a NUL byte in an argument"))
}

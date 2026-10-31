// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A login held to what drives it** (#4043, spec #4023): the runner's side of a sign-in the
//! window drives. Ending a `docker exec` client ends nothing it started in the container, and a
//! harness's login does not stop when its input closes, so a cancelled sign-in would run on
//! there. Here the runner's own `purlis` stands between: the login runs under it, what comes in
//! on its input is passed on, and **when its input closes, the login and everything it started
//! are ended**, whether the window cancelled, tried again, timed out or went away.
//!
//! [`until_input_ends`] runs the login with pipes, for the driven strip; [`in_own_terminal`]
//! runs it in a terminal made here, so the bytes that draw it cross a plain pipe and the same
//! end holds, which a terminal `docker exec` makes for it could not give.
//!
//! **A login that waits for the browser says so here too** ([`until_answered_or_input_ends`]):
//! what it prints is read as the desktop reads it, and the `state` of its page is recorded before
//! that page goes any further ([`super::waiting`]), for as long as it runs. Only such a login's
//! browser answer is handed to it on this machine.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process_group};

use super::Login;
use super::driven::{self, Driven, Event, Stream};
use super::waiting::Waiting;
use crate::engine::{AlacrittyEngine, Size};
use crate::session::{Exit, Session, Spec};

/// How often a running login is looked at.
const LOOK: Duration = Duration::from_millis(50);

/// How long a login has to end when it is asked before it is made to.
const ENDS_WITHIN: Duration = Duration::from_secs(2);

/// **Runs `login` with `input` passed to its own, until it ends or `input` closes**: how it ended
/// by itself, or `None` where it was ended. Its output is this process's own.
pub fn until_input_ends(
    login: &Login,
    input: impl Read + Send + 'static,
) -> Result<Option<i32>, String> {
    run_until_input_ends(login, input, None)
}

/// **[`until_input_ends`], of a login that waits for the browser's answer**, read by `driven` as
/// its desktop reads it: what it prints goes on to `out` and `err`, and from before its page does
/// the login is recorded in `kept_in` as waiting with that page's `state`, until it ends. One that
/// cannot be recorded is ended, with its page gone nowhere.
pub fn until_answered_or_input_ends(
    login: &Login,
    driven: Driven,
    kept_in: &Path,
    input: impl Read + Send + 'static,
    out: impl Write + Send + 'static,
    err: impl Write + Send + 'static,
) -> Result<Option<i32>, String> {
    run_until_input_ends(
        login,
        input,
        Some(Watched {
            reading: Arc::new(Mutex::new(Reading {
                driven,
                kept_in: kept_in.to_path_buf(),
                waiting: None,
            })),
            out: Box::new(out),
            err: Box::new(err),
        }),
    )
}

/// What a login's output goes through, where it is read here.
struct Watched {
    reading: Arc<Mutex<Reading>>,
    out: Box<dyn Write + Send>,
    err: Box<dyn Write + Send>,
}

struct Reading {
    driven: Driven,
    kept_in: PathBuf,
    waiting: Option<Waiting>,
}

/// Takes the login's record away as it ends, whichever way, whatever still reads its output.
struct RecordEnds(Arc<Mutex<Reading>>);

impl Drop for RecordEnds {
    fn drop(&mut self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiting = None;
    }
}

fn run_until_input_ends(
    login: &Login,
    input: impl Read + Send + 'static,
    watched: Option<Watched>,
) -> Result<Option<i32>, String> {
    let mut command = Command::new(&login.program);
    command
        .args(&login.args)
        .envs(login.env.iter().map(|(n, v)| (n, v)))
        .stdin(Stdio::piped())
        // A group of its own, so what it started ends with it.
        .process_group(0);
    if watched.is_some() {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    for name in &login.cleared {
        command.env_remove(name);
    }
    let mut child = crate::forklock::spawn(&mut command)
        .map_err(|e| format!("{} could not be run: {e}", login.program))?;
    let (not_recorded, refused) = mpsc::channel();
    let mut readers = Vec::new();
    let _record_ends = watched.map(|watched| {
        if let Some(from) = child.stdout.take() {
            readers.push(passing_on(
                Stream::Out,
                from,
                watched.out,
                Arc::clone(&watched.reading),
                not_recorded.clone(),
            ));
        }
        if let Some(from) = child.stderr.take() {
            readers.push(passing_on(
                Stream::Err,
                from,
                watched.err,
                Arc::clone(&watched.reading),
                not_recorded.clone(),
            ));
        }
        RecordEnds(watched.reading)
    });
    let mut to_login = child.stdin.take();
    let closed = passing(input, move |bytes| {
        if let Some(to) = to_login.as_mut()
            && to.write_all(bytes).and_then(|()| to.flush()).is_err()
        {
            to_login = None;
        }
    });
    loop {
        match child.try_wait() {
            // What it started ends with it.
            Ok(Some(status)) => {
                end_group_of(child.id());
                // Its last words are still on their way. What it started and let go may hold
                // them open, so this ends too.
                purlis_terminal::process::wait_until(Instant::now() + ENDS_WITHIN, || {
                    readers.iter().all(JoinHandle::is_finished)
                });
                return Ok(status.code());
            }
            Ok(None) => {}
            Err(e) => {
                end_group(&mut child);
                return Err(format!("the sign-in could not be watched: {e}"));
            }
        }
        if let Ok(why) = refused.try_recv() {
            end_group(&mut child);
            return Err(why);
        }
        match closed.recv_timeout(LOOK) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                end_group(&mut child);
                return Ok(None);
            }
        }
    }
}

/// **Runs `login` in a terminal of `size` made here, with `input` typed into it and what draws
/// it written to `output`, until it ends or `input` closes**: how it ended by itself, or `None`
/// where it was ended.
pub fn in_own_terminal(
    login: &Login,
    size: Size,
    input: impl Read + Send + 'static,
    mut output: impl Write,
) -> Result<Option<i32>, String> {
    let mut spec = Spec::new(login.program.clone(), size).args(login.args.iter());
    spec.env = login
        .env
        .iter()
        .map(|(n, v)| (n.into(), v.into()))
        .collect();
    spec.env_without = login.cleared.iter().map(Into::into).collect();
    let session = Arc::new(
        Session::spawn(spec, Box::new(AlacrittyEngine::new(size, 0)))
            .map_err(|e| format!("{} could not be run: {e}", login.program))?,
    );
    let view = session.attach();
    let closed = passing(input, {
        let session = Arc::clone(&session);
        move |bytes| {
            let _ = session.write(bytes);
        }
    });
    loop {
        match view.output.recv_timeout(LOOK) {
            Ok(bytes) => {
                if output
                    .write_all(&bytes)
                    .and_then(|()| output.flush())
                    .is_err()
                {
                    session.stopper().stop();
                    return Ok(None);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let ended = session.wait(ENDS_WITHIN);
                session.stopper().stop();
                return Ok(match ended {
                    Ok(Some(Exit::Code(code))) => i32::try_from(code).ok(),
                    _ => None,
                });
            }
        }
        // A program it started may hold the terminal open after it ended: it is looked at.
        if let Ok(Some(exit)) = session.wait(Duration::ZERO) {
            while let Ok(bytes) = view.output.try_recv() {
                let _ = output.write_all(&bytes);
            }
            let _ = output.flush();
            session.stopper().stop();
            return Ok(match exit {
                Exit::Code(code) => i32::try_from(code).ok(),
                Exit::Signal(_) => None,
            });
        }
        if !matches!(closed.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            session.stopper().stop();
            return Ok(None);
        }
    }
}

/// Reads what the login prints on `from`, its output `stream`, and passes it on to `to`. A page
/// is recorded as waiting before the line that shows it is passed on: where it cannot be, `refused`
/// hears why, and nothing more is.
fn passing_on(
    stream: Stream,
    mut from: impl Read + Send + 'static,
    mut to: Box<dyn Write + Send>,
    reading: Arc<Mutex<Reading>>,
    refused: Sender<String>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        let mut carried = Vec::new();
        while let Ok(read) = from.read(&mut buffer) {
            if read == 0 {
                return;
            }
            let text = driven::whole_text(&mut carried, &buffer[..read]);
            let recorded = reading
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .read(stream, &text);
            if let Err(why) = recorded {
                let _ = refused.send(why);
                return;
            }
            // Whoever reads it may have gone: its input ends too, and meanwhile the login never
            // waits on a full pipe.
            let _ = to.write_all(&buffer[..read]).and_then(|()| to.flush());
        }
    })
}

impl Reading {
    /// `text`, read as the desktop reads it: the page of a login that waits is recorded.
    fn read(&mut self, stream: Stream, text: &str) -> Result<(), String> {
        for event in self.driven.read_from(stream, text) {
            if let Event::Approve { state, .. } = event {
                self.waiting = Some(Waiting::record(&self.kept_in, &state).map_err(|why| {
                    format!("purlis could not note this sign-in on this runner: {why}")
                })?);
            }
        }
        Ok(())
    }
}

/// Reads `input` on a thread of its own, handing each piece to `pass`: what it answers says
/// `input` closed.
fn passing(
    mut input: impl Read + Send + 'static,
    mut pass: impl FnMut(&[u8]) + Send + 'static,
) -> Receiver<()> {
    let (closed, says) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(read) = input.read(&mut buffer) {
            if read == 0 {
                break;
            }
            pass(&buffer[..read]);
        }
        let _ = closed.send(());
    });
    says
}

/// How long [`Input::closed_by_now`] gives what reads it to see its end.
const SEEN_WITHIN: Duration = Duration::from_millis(50);

/// **What drives a held login, watched from before it starts** (#4043): a runner may fetch its
/// harness first, and whoever drives it may cancel meanwhile. What comes in waits for the login.
pub struct Input {
    pieces: Receiver<Vec<u8>>,
    left: Vec<u8>,
    ended: Receiver<()>,
}

impl Input {
    /// `input`, read from now on.
    pub fn of(mut input: impl Read + Send + 'static) -> Self {
        let (piece, pieces) = mpsc::channel();
        let (end, ended) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _end = end;
            let mut buffer = [0u8; 4096];
            while let Ok(read) = input.read(&mut buffer) {
                if read == 0 || piece.send(buffer[..read].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            pieces,
            left: Vec::new(),
            ended,
        }
    }

    /// **Whether it closed by now**, so nothing should start.
    pub fn closed_by_now(&self) -> bool {
        matches!(
            self.ended.recv_timeout(SEEN_WITHIN),
            Err(RecvTimeoutError::Disconnected)
        )
    }
}

impl Read for Input {
    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        if self.left.is_empty() {
            match self.pieces.recv() {
                Ok(piece) => self.left = piece,
                Err(_) => return Ok(0),
            }
        }
        let read = into.len().min(self.left.len());
        into[..read].copy_from_slice(&self.left[..read]);
        self.left.drain(..read);
        Ok(read)
    }
}

/// Ends what is left of the group the login led, once it has ended and been reaped.
fn end_group_of(leader: u32) {
    if let Some(leader) = i32::try_from(leader).ok().and_then(Pid::from_raw) {
        let _ = kill_process_group(leader, Signal::KILL);
    }
}

/// Asks the login's group to end, makes it after [`ENDS_WITHIN`], and reaps the login.
fn end_group(child: &mut Child) {
    let Some(leader) = i32::try_from(child.id()).ok().and_then(Pid::from_raw) else {
        return;
    };
    let _ = kill_process_group(leader, Signal::TERM);
    let gone = purlis_terminal::process::wait_until(Instant::now() + ENDS_WITHIN, || {
        // Reaped as it goes, or it counts as still in its group.
        let _ = child.try_wait();
        purlis_terminal::process::group_still_runs(leader) == Some(false)
    });
    if !gone {
        let _ = kill_process_group(leader, Signal::KILL);
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests;

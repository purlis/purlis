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

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process_group};

use super::Login;
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
    let mut command = Command::new(&login.program);
    command
        .args(&login.args)
        .envs(login.env.iter().map(|(n, v)| (n, v)))
        .stdin(Stdio::piped())
        // A group of its own, so what it started ends with it.
        .process_group(0);
    for name in &login.cleared {
        command.env_remove(name);
    }
    let mut child = crate::forklock::spawn(&mut command)
        .map_err(|e| format!("{} could not be run: {e}", login.program))?;
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
                return Ok(status.code());
            }
            Ok(None) => {}
            Err(e) => {
                end_group(&mut child);
                return Err(format!("the sign-in could not be watched: {e}"));
            }
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
            // A program it started may hold the terminal open after it ended: it is looked at.
            Err(RecvTimeoutError::Timeout) => {
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
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Ok(match session.wait(ENDS_WITHIN) {
                    Ok(Some(Exit::Code(code))) => i32::try_from(code).ok(),
                    _ => None,
                });
            }
        }
        if !matches!(closed.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            session.stopper().stop();
            return Ok(None);
        }
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

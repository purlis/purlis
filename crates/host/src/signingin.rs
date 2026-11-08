// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The runner's own sign-in, run over its desktop's link** (session protocol 1.79).
//!
//! The desktop a runner is paired with asks it to sign one of that desktop's profiles in
//! (`start_sign_in`): the runner runs its own `purlis profile sign-in` with pipes, as a
//! `kubectl exec -i` into its pod ran it before, and holds it to the link it was asked on. The
//! desktop reads what it printed (`sign_in_next`), types to it (`sign_in_input`) and ends it
//! (`end_sign_in`). A link proves the runner by its key, so such a sign-in reaches the runner the
//! desktop paired with, where a `kubectl exec` reaches the pod of a name.
//!
//! What is run is the runner's binary's to say ([`SigningIn`]): this module only runs it, keeps
//! what it prints until it is read, and ends it with its link.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_session_protocol::hostlink::Pinned;
use purlis_session_protocol::session;

/// **The runner's own sign-in of the desktop's profile**, as its binary runs it: the command,
/// with its words, for the desktop a link proved. Refused, in words, for a profile the runner does
/// not keep for that desktop. Its standard input, output and error output are set here.
pub type SigningIn = Arc<
    dyn Fn(&Pinned, &session::SignInToStart) -> Result<std::process::Command, String> + Send + Sync,
>;

/// How long a read of what a sign-in printed waits for something, where it printed nothing yet.
const NEXT_WAITS: Duration = Duration::from_millis(800);
/// How often that wait looks.
const LOOKS_EVERY: Duration = Duration::from_millis(25);
/// How long a sign-in whose input ended has to end before it is stopped.
const ENDS_WITHIN: Duration = Duration::from_secs(5);
/// The most a sign-in may have printed and not been read: past it, it is stopped. A login prints
/// a page and a few lines; a terminal's redraws, read a few times a second, stay far below.
const MOST_UNREAD: usize = 4 * 1024 * 1024;
/// The most sign-ins one link runs at once.
const MOST_AT_ONCE: usize = 4;

/// What one sign-in printed and not yet read, on each output, and whether each has ended.
#[derive(Default)]
struct Printed {
    out: Vec<u8>,
    err: Vec<u8>,
    outputs_ended: u8,
    too_much: bool,
}

/// **One sign-in running for a link.**
struct Running {
    child: Mutex<Child>,
    input: Mutex<Option<ChildStdin>>,
    printed: Arc<Mutex<Printed>>,
}

impl Running {
    /// Its input ends, and it is stopped where it has not ended within [`ENDS_WITHIN`].
    fn end(self: &Arc<Self>) {
        drop(
            self.input
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
        let running = Arc::clone(self);
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + ENDS_WITHIN;
            loop {
                let mut child = running.child.lock().unwrap_or_else(PoisonError::into_inner);
                match child.try_wait() {
                    Ok(Some(_)) | Err(_) => return,
                    Ok(None) if std::time::Instant::now() >= until => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                    Ok(None) => {}
                }
                drop(child);
                std::thread::sleep(LOOKS_EVERY * 4);
            }
        });
    }

    /// How it ended, once it has and all it printed was read.
    fn ended(&self) -> Option<session::SignInEnded> {
        let printed = self.printed.lock().unwrap_or_else(PoisonError::into_inner);
        if printed.outputs_ended < 2 || !printed.out.is_empty() || !printed.err.is_empty() {
            return None;
        }
        drop(printed);
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        match child.try_wait() {
            Ok(Some(status)) => Some(session::SignInEnded {
                code: status.code(),
            }),
            Ok(None) => None,
            Err(_) => Some(session::SignInEnded { code: None }),
        }
    }
}

/// **The sign-ins one link runs**, by their number on it: all ended as the link ends.
#[derive(Default)]
pub(crate) struct SignIns {
    running: Mutex<HashMap<u32, Arc<Running>>>,
    last: AtomicU32,
}

fn not_started(why: impl Into<String>) -> session::Refusal {
    session::Refusal {
        code: session::NOT_STARTED.into(),
        why: why.into(),
    }
}

impl SignIns {
    /// `start_sign_in`: runs what `signing_in` says for `desktop`'s `start`, its outputs read
    /// into memory until they are asked for.
    pub(crate) async fn start(
        &self,
        signing_in: &SigningIn,
        desktop: &Pinned,
        start: session::SignInToStart,
    ) -> Result<session::Answer, session::Refusal> {
        if self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
            >= MOST_AT_ONCE
        {
            return Err(not_started(
                "this runner runs as many sign-ins for this desktop as it may: end one first",
            ));
        }
        let (signing_in, desktop) = (Arc::clone(signing_in), desktop.clone());
        let started = tokio::task::spawn_blocking(move || {
            let mut command = signing_in(&desktop, &start)?;
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
                .spawn()
                .map_err(|why| format!("the sign-in could not be started: {why}"))
        })
        .await
        .unwrap_or_else(|why| Err(format!("the sign-in did not start: {why}")))
        .map_err(not_started)?;
        let mut child = started;
        let printed = Arc::new(Mutex::new(Printed::default()));
        let outputs: [Option<Box<dyn Read + Send>>; 2] = [
            child
                .stdout
                .take()
                .map(|out| Box::new(out) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|err| Box::new(err) as Box<dyn Read + Send>),
        ];
        let running = Arc::new(Running {
            input: Mutex::new(child.stdin.take()),
            child: Mutex::new(child),
            printed: Arc::clone(&printed),
        });
        for (at, output) in outputs.into_iter().enumerate() {
            let printed = Arc::clone(&printed);
            let Some(mut output) = output else {
                printed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .outputs_ended += 1;
                continue;
            };
            let running = Arc::clone(&running);
            std::thread::spawn(move || {
                let mut buffer = [0u8; 4096];
                loop {
                    let read = match output.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => read,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    };
                    let mut held = printed.lock().unwrap_or_else(PoisonError::into_inner);
                    if held.out.len() + held.err.len() + read > MOST_UNREAD {
                        held.too_much = true;
                        drop(held);
                        running.end();
                        break;
                    }
                    let to = if at == 0 {
                        &mut held.out
                    } else {
                        &mut held.err
                    };
                    to.extend_from_slice(&buffer[..read]);
                }
                printed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .outputs_ended += 1;
            });
        }
        let sign_in = self.last.fetch_add(1, Ordering::SeqCst) + 1;
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(sign_in, running);
        Ok(session::Answer::SignInStarted { sign_in })
    }

    fn running(&self, sign_in: u32) -> Result<Arc<Running>, session::Refusal> {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&sign_in)
            .cloned()
            .ok_or_else(|| not_started("this link runs no such sign-in"))
    }

    /// `sign_in_next`: what it printed since it was last asked, waiting a little for something
    /// where it printed nothing yet; how it ended, once all it printed was read, after which it is
    /// let go.
    pub(crate) async fn next(&self, sign_in: u32) -> Result<session::Answer, session::Refusal> {
        let running = self.running(sign_in)?;
        let until = tokio::time::Instant::now() + NEXT_WAITS;
        loop {
            let (out, err, too_much) = {
                let mut printed = running
                    .printed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                (
                    std::mem::take(&mut printed.out),
                    std::mem::take(&mut printed.err),
                    printed.too_much,
                )
            };
            if too_much {
                self.let_go(sign_in);
                return Err(not_started(
                    "the sign-in printed more than a sign-in does unread, so it was stopped",
                ));
            }
            let ended = running.ended();
            if ended.is_some() {
                self.let_go(sign_in);
            }
            if !out.is_empty()
                || !err.is_empty()
                || ended.is_some()
                || tokio::time::Instant::now() >= until
            {
                return Ok(session::Answer::SignInPrinted { out, err, ended });
            }
            tokio::time::sleep(LOOKS_EVERY).await;
        }
    }

    /// `sign_in_input`: `bytes`, typed to it as they are.
    pub(crate) async fn input(
        &self,
        sign_in: u32,
        bytes: Vec<u8>,
    ) -> Result<session::Answer, session::Refusal> {
        let running = self.running(sign_in)?;
        tokio::task::spawn_blocking(move || {
            let mut input = running.input.lock().unwrap_or_else(PoisonError::into_inner);
            let input = input
                .as_mut()
                .ok_or_else(|| "the sign-in takes no more input".to_owned())?;
            input
                .write_all(&bytes)
                .and_then(|()| input.flush())
                .map_err(|why| format!("what was typed did not reach the sign-in: {why}"))
        })
        .await
        .unwrap_or_else(|why| Err(format!("what was typed did not reach the sign-in: {why}")))
        .map(|()| session::Answer::Done)
        .map_err(not_started)
    }

    /// `end_sign_in`: its input ends, and it is stopped where it goes on. Done for one that ended
    /// already.
    pub(crate) fn end(&self, sign_in: u32) -> session::Answer {
        self.let_go(sign_in);
        session::Answer::Done
    }

    /// Ends sign-in `sign_in`, if this link runs it, and forgets it.
    fn let_go(&self, sign_in: u32) {
        let running = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&sign_in);
        if let Some(running) = running {
            running.end();
        }
    }
}

/// Every sign-in a link ran ends with it.
impl Drop for SignIns {
    fn drop(&mut self) {
        let running: Vec<_> = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
            .map(|(_, running)| running)
            .collect();
        for running in running {
            running.end();
        }
    }
}

#[cfg(test)]
mod tests;

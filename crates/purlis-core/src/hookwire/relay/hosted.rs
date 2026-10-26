// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A `secret exec` of a chat this host composed, run here on the values its desktop sends**
//! (#2047, ADR 0078 §7, spec #2041 stories 14 and 21).
//!
//! A runner holds no vault. When a chat it composed in a sandbox runs one of its persona's
//! declared tools through `purlis secret exec`, the runner's host first finds which tool its own
//! start armed the command as ([`brokered::armed_as`]), and refuses, as a run here would, what
//! is none. It hands the line on to the window with that tool's name ([`brokered::Ask::runs_as`]),
//! and the window, after its own gate, sends the values for that one run
//! ([`Replied::Handed`]). This host then runs the tool itself, held to its own record of the
//! chat's sandbox ([`brokered::serve_handed`]), and the values go with the run.
//!
//! **With the desktop away the command waits** for a window to hear this host again, as a
//! browser login does ([`super::A_LOGIN_WAITS_AT_MOST`], within the same bounds of how many
//! wait), told so on its stderr, and is handed on marked as one that waited, so the window asks
//! the person afresh. The chat may carry on: the command holds only itself. It is let go of, with
//! nothing run, once it has waited too long, once its asker hangs up, or once its chat is gone.
//! Nothing is kept while it waits but the line itself, which holds no value.
//!
//! The asker writes nothing before its run has started, so its connection is read from the
//! start: lines that come are kept for the run, and its end says the asker hung up.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use super::{
    A_LOGIN_WAITS_AT_MOST, Heard, Lines, MOST_LOGINS_WAITING, MOST_LOGINS_WAITING_FOR_A_CHAT,
    Relaying, Replied, lock,
};
use crate::secrets::brokered::{self, Asker};

/// What runs a `secret exec` on the values the window sent ([`brokered::serve_handed`]).
pub type RunsHanded = Arc<
    dyn Fn(
            &Asker,
            brokered::Wanted,
            brokered::Handed,
            Box<dyn BufRead + Send>,
            Box<dyn Write + Send>,
            brokered::Told,
        ) + Send
        + Sync
        + 'static,
>;

/// How long a `secret exec` waits for the window while its desktop is away (#2047): as long as a
/// browser login waits.
pub const AN_EXEC_WAITS_AT_MOST: Duration = A_LOGIN_WAITS_AT_MOST;

/// How often a `secret exec` that waits for the window tries to hand itself on.
const AN_EXEC_LOOKS_FOR_A_WINDOW_EVERY: Duration = Duration::from_millis(500);

/// How many lines of the asker's input are kept before its run reads them.
const LINES_KEPT: usize = 64;

/// What the asker of `tool` is told on its stderr while it waits for the person's desktop.
pub fn exec_waits_said(tool: &str) -> String {
    format!(
        "the person's desktop is not connected, so purlis cannot send {tool} its vault value \
         yet: this command waits for them, at most 15 minutes, and runs once they allow it. Run \
         it in the background to carry on with what needs no vault meanwhile"
    )
}

/// What the asker of `tool` is told where the desktop was not back in time.
fn waited_too_long_said(tool: &str) -> String {
    format!(
        "the person's desktop was not back within 15 minutes, so {tool} did not run and was \
         sent no value: run it again once they are back"
    )
}

/// What the asker of `tool` is told where too many commands wait already.
fn too_many_wait_said(tool: &str) -> String {
    format!(
        "the person's desktop is not connected, and too many commands wait for it already, so \
         {tool} did not run: run it again once they are back"
    )
}

/// What the asker of `tool` is told where the window went before it sent the values.
fn window_went_said(tool: &str) -> String {
    format!(
        "the person's desktop went before it answered, so {tool} did not run and was sent no \
         value: run it again"
    )
}

/// The asker's input, read from the start into a channel the run reads later: `ended` once the
/// asker's end of the connection has.
struct Input {
    lines: mpsc::Receiver<String>,
    ended: Arc<AtomicBool>,
}

impl Input {
    fn read(reader: Box<dyn BufRead + Send>) -> Option<Input> {
        let (to, lines) = mpsc::sync_channel(LINES_KEPT);
        let ended = Arc::new(AtomicBool::new(false));
        let ending = Arc::clone(&ended);
        std::thread::Builder::new()
            .name("purlisd-secret-input".into())
            .spawn(move || {
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if to.send(line).is_err() {
                        break;
                    }
                }
                ending.store(true, Ordering::SeqCst);
            })
            .ok()?;
        Some(Input { lines, ended })
    }

    /// Whether the asker hung up: its connection ended with nothing more to read.
    fn gone(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }
}

impl Relaying {
    /// Runs each `secret exec` handed its values with `runs` in place of the brokered run: for
    /// a test of what reaches the run.
    #[cfg(test)]
    pub(super) fn running_handed_with(self: Arc<Self>, runs: RunsHanded) -> Arc<Self> {
        let mut relaying = Arc::try_unwrap(self).unwrap_or_else(|_| panic!("held elsewhere"));
        relaying.runs_handed = runs;
        Arc::new(relaying)
    }

    /// **Chat `asker`'s `secret exec` `ask`, the chat composed here** (module docs).
    pub(super) fn secret_exec_composed_here(
        self: &Arc<Self>,
        asker: Asker,
        mut ask: brokered::Ask,
        reader: Box<dyn BufRead + Send>,
        mut writer: Box<dyn Write + Send>,
    ) {
        let tool = match brokered::armed_as(&asker, &ask.secret_exec) {
            Ok(tool) => tool,
            Err(why) => return brokered::refused(writer, why),
        };
        ask.runs_as = Some(tool.clone());
        let Some(input) = Input::read(reader) else {
            return brokered::not_answered(writer);
        };
        let exchanged = match self.exchange(&Heard::SecretExec(ask.clone())) {
            Some(exchanged) => exchanged,
            None => match self.waits_for_a_window(&mut ask, &tool, &input, &mut writer) {
                Ok(exchanged) => exchanged,
                Err(Some(why)) => return brokered::refused(writer, why),
                Err(None) => return,
            },
        };
        let (id, replied) = exchanged;
        loop {
            match replied.recv() {
                Ok(Replied::Handed(handed)) => {
                    self.done(id);
                    // An asker that hung up while the person was asked is run nothing.
                    if input.gone() {
                        return;
                    }
                    let relaying = Arc::clone(self);
                    let told: brokered::Told = Arc::new(move |run: brokered::RunBlocked| {
                        relaying.tell(&Heard::Blocked(run.block));
                    });
                    let reader = Box::new(std::io::BufReader::new(Lines {
                        lines: input.lines,
                        pending: Vec::new(),
                        at: 0,
                    }));
                    return (self.runs_handed)(
                        &asker,
                        ask.secret_exec,
                        handed,
                        reader,
                        writer,
                        told,
                    );
                }
                // The window's own words: a refusal, said as it said it.
                Ok(Replied::Wrote { text }) => {
                    if writer
                        .write_all(text.as_bytes())
                        .and_then(|()| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Replied::Ended) => break,
                Ok(_) => {}
                Err(_) => {
                    brokered::refused(writer, window_went_said(&tool));
                    break;
                }
            }
        }
        self.done(id);
    }

    /// **Holds `ask` until a window hears this host again** (#2047), at most
    /// [`AN_EXEC_WAITS_AT_MOST`], telling its asker so on stderr; then hands it on marked as one
    /// that waited. `Err` with what to tell the asker, or with nothing where it hung up or its
    /// chat is gone.
    fn waits_for_a_window(
        &self,
        ask: &mut brokered::Ask,
        tool: &str,
        input: &Input,
        writer: &mut Box<dyn Write + Send>,
    ) -> Result<(u64, mpsc::Receiver<Replied>), Option<String>> {
        let chat = ask.chat;
        if !self.an_exec_may_wait(chat) {
            return Err(Some(too_many_wait_said(tool)));
        }
        let _waiting = ExecWaits {
            relaying: self,
            chat,
        };
        let note = brokered::Frame::Note(exec_waits_said(tool));
        if serde_json::to_string(&note)
            .ok()
            .and_then(|line| {
                writeln!(writer, "{line}")
                    .and_then(|()| writer.flush())
                    .ok()
            })
            .is_none()
        {
            return Err(None);
        }
        ask.waited = true;
        let since = Instant::now();
        loop {
            std::thread::sleep(AN_EXEC_LOOKS_FOR_A_WINDOW_EVERY);
            if input.gone() || !self.still_there(chat) {
                return Err(None);
            }
            if since.elapsed() > AN_EXEC_WAITS_AT_MOST {
                return Err(Some(waited_too_long_said(tool)));
            }
            if let Some(exchanged) = self.exchange(&Heard::SecretExec(ask.clone())) {
                return Ok(exchanged);
            }
        }
    }

    /// Whether one more of chat `chat`'s `secret exec`s may wait for the window: counted with
    /// the browser logins that wait, within the same bounds.
    fn an_exec_may_wait(&self, chat: u32) -> bool {
        let (logins, logins_of_its) = {
            let waiting = lock(&self.logins_waiting);
            let its = waiting.iter().filter(|one| one.browse.chat == chat).count();
            (waiting.len(), its)
        };
        let mut execs = lock(&self.execs_waiting);
        let all: usize = execs.values().sum();
        let its = execs.get(&chat).copied().unwrap_or_default();
        if logins + all >= MOST_LOGINS_WAITING
            || logins_of_its + its >= MOST_LOGINS_WAITING_FOR_A_CHAT
        {
            return false;
        }
        *execs.entry(chat).or_default() += 1;
        true
    }
}

/// One `secret exec` of `chat`'s counted as waiting, until this is dropped.
struct ExecWaits<'a> {
    relaying: &'a Relaying,
    chat: u32,
}

impl Drop for ExecWaits<'_> {
    fn drop(&mut self) {
        let mut execs = lock(&self.relaying.execs_waiting);
        if let Some(count) = execs.get_mut(&self.chat) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                execs.remove(&self.chat);
            }
        }
    }
}

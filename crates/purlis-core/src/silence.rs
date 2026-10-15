// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Silence, not heartbeats** (#1981, D-6): purlis watches each task's last activity, and after
//! a quiet stretch says so to the chat that asked and on the task's tab. The chat that asked
//! never polls, and nothing here ends a task.
//!
//! # What counts as activity
//!
//! **What the task's harness tells purlis's hooks**: a turn beginning or ending, a tool starting
//! or coming back, a notification, the last message a turn ended on. Never the bytes its
//! terminal draws: a harness redraws a spinner for as long as it runs, whether or not anything
//! happens, so a pane that prints is no sign of work.
//!
//! # When a note is made
//!
//! Only while the task's turn is running ([`Watch::look`]): a task whose turn has ended is the
//! report rule's (`crate::dispatched::Ledger::turn_ended_owing`), and one showing the person a
//! prompt is the person's.
//!
//! - **One command for a long time**: a shell command began and has not come back for the
//!   one-command limit (`one-command-note-minutes`, 30 by default). The note names its program
//!   where the hook named one from purlis's fixed list (`crate::doing::PROGRAMS`), so a chat
//!   cannot plant words in it. While a command is in flight, only this limit applies: a command
//!   that runs long is quiet by nature.
//! - **Quiet**: nothing heard for the quiet limit (`quiet-note-minutes`, 15 by default).
//!
//! **One note a quiet stretch.** Any activity starts a new stretch; a stretch that goes on is
//! not noted again. Plain Rust with no clock of its own: the app passes the time.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A note on a task gone quiet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// Nothing heard from it for `minutes`.
    Quiet { minutes: u64 },
    /// One command has run for `minutes`, `program` where the hook named one.
    OneCommand {
        minutes: u64,
        program: Option<String>,
    },
}

impl Note {
    /// The note in purlis's words, about task `name` (chat `chat`).
    pub fn said(&self, name: &str, chat: u32) -> String {
        let name = crate::personas::one_line(name);
        match self {
            Self::Quiet { minutes } => format!(
                "task '{name}' (chat {chat}) has shown no activity for {}",
                minutes_said(*minutes)
            ),
            Self::OneCommand {
                minutes,
                program: Some(program),
            } => format!(
                "task '{name}' (chat {chat}) is running one command for {}: `{program}`",
                minutes_said(*minutes)
            ),
            Self::OneCommand {
                minutes,
                program: None,
            } => format!(
                "task '{name}' (chat {chat}) is running one command for {}",
                minutes_said(*minutes)
            ),
        }
    }
}

fn minutes_said(minutes: u64) -> String {
    if minutes == 1 {
        "1 minute".to_owned()
    } else {
        format!("{minutes} minutes")
    }
}

/// What purlis has heard of one task.
#[derive(Debug, Clone, Default)]
struct Heard {
    /// When it was last heard from.
    last: Option<Instant>,
    /// A command in flight: when it began, and its program where the hook named one.
    command: Option<(Instant, Option<String>)>,
    /// This quiet stretch has been noted.
    noted: bool,
}

/// What purlis has heard of each task of one project, in memory.
#[derive(Debug, Default)]
pub struct Watch {
    tasks: HashMap<u32, Heard>,
}

impl Watch {
    /// Task `task` was heard from at `now`: a new stretch begins.
    pub fn heard(&mut self, task: u32, now: Instant) {
        let heard = self.tasks.entry(task).or_default();
        heard.last = Some(now);
        heard.noted = false;
    }

    /// Task `task`'s turn ended at `now`: no command of that turn is in flight any more.
    pub fn turn_ended(&mut self, task: u32, now: Instant) {
        self.heard(task, now);
        if let Some(heard) = self.tasks.get_mut(&task) {
            heard.command = None;
        }
    }

    /// Task `task` began a shell command at `now`, running `program` where the hook named one.
    pub fn command_began(&mut self, task: u32, program: Option<String>, now: Instant) {
        self.heard(task, now);
        if let Some(heard) = self.tasks.get_mut(&task) {
            heard.command = Some((now, program));
        }
    }

    /// A shell command of task `task`'s came back at `now`.
    pub fn command_ended(&mut self, task: u32, now: Instant) {
        self.heard(task, now);
        if let Some(heard) = self.tasks.get_mut(&task) {
            heard.command = None;
        }
    }

    /// Task `task` is gone: nothing is kept of it.
    pub fn forget(&mut self, task: u32) {
        self.tasks.remove(&task);
    }

    /// **The note task `task` is owed at `now`, if any**: `running` says its turn is running
    /// and showing the person no prompt; `quiet` and `one_command` are the limits in force.
    /// Noted once a stretch. A task purlis has heard nothing from is not watched: its start is
    /// what the app's own start rules are about.
    pub fn look(
        &mut self,
        task: u32,
        now: Instant,
        running: bool,
        quiet: Duration,
        one_command: Duration,
    ) -> Option<Note> {
        let heard = self.tasks.get_mut(&task)?;
        if !running || heard.noted {
            return None;
        }
        let note = match &heard.command {
            Some((since, program)) => {
                let ran = now.saturating_duration_since(*since);
                (ran >= one_command).then(|| Note::OneCommand {
                    minutes: ran.as_secs() / 60,
                    program: program.clone(),
                })
            }
            None => {
                let still = now.saturating_duration_since(heard.last?);
                (still >= quiet).then_some(Note::Quiet {
                    minutes: still.as_secs() / 60,
                })
            }
        }?;
        heard.noted = true;
        Some(note)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TASK: u32 = 9;
    const QUIET: Duration = Duration::from_secs(15 * 60);
    const ONE_COMMAND: Duration = Duration::from_secs(30 * 60);

    fn minutes(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    #[test]
    fn a_running_task_heard_from_nothing_for_the_quiet_limit_is_noted_once() {
        let start = Instant::now();
        let mut watch = Watch::default();
        watch.heard(TASK, start);
        assert_eq!(
            watch.look(TASK, start + minutes(14), true, QUIET, ONE_COMMAND),
            None
        );
        assert_eq!(
            watch.look(TASK, start + minutes(16), true, QUIET, ONE_COMMAND),
            Some(Note::Quiet { minutes: 16 })
        );
        // Once a stretch.
        assert_eq!(
            watch.look(TASK, start + minutes(40), true, QUIET, ONE_COMMAND),
            None
        );
        // Heard from again: a new stretch, noted again when it too goes quiet.
        watch.heard(TASK, start + minutes(41));
        assert_eq!(
            watch.look(TASK, start + minutes(57), true, QUIET, ONE_COMMAND),
            Some(Note::Quiet { minutes: 16 })
        );
    }

    #[test]
    fn one_command_in_flight_is_noted_at_its_own_limit_by_its_program() {
        let start = Instant::now();
        let mut watch = Watch::default();
        watch.command_began(TASK, Some("cargo".to_owned()), start);
        // Quiet past the quiet limit, but one command runs: only its own limit applies.
        assert_eq!(
            watch.look(TASK, start + minutes(20), true, QUIET, ONE_COMMAND),
            None
        );
        assert_eq!(
            watch.look(TASK, start + minutes(31), true, QUIET, ONE_COMMAND),
            Some(Note::OneCommand {
                minutes: 31,
                program: Some("cargo".to_owned())
            })
        );
        // It came back: the next stretch is a quiet one again.
        watch.command_ended(TASK, start + minutes(32));
        assert_eq!(
            watch.look(TASK, start + minutes(48), true, QUIET, ONE_COMMAND),
            Some(Note::Quiet { minutes: 16 })
        );
    }

    #[test]
    fn a_task_at_rest_unheard_or_gone_is_never_noted() {
        let start = Instant::now();
        let mut watch = Watch::default();
        assert_eq!(
            watch.look(TASK, start + minutes(60), true, QUIET, ONE_COMMAND),
            None,
            "nothing heard yet"
        );
        watch.command_began(TASK, None, start);
        watch.turn_ended(TASK, start + minutes(1));
        assert_eq!(
            watch.look(TASK, start + minutes(60), false, QUIET, ONE_COMMAND),
            None,
            "its turn has ended"
        );
        // Its turn ended: the command of that turn is no longer in flight.
        assert_eq!(
            watch.look(TASK, start + minutes(60), true, QUIET, ONE_COMMAND),
            Some(Note::Quiet { minutes: 59 })
        );
        watch.forget(TASK);
        assert_eq!(
            watch.look(TASK, start + minutes(90), true, QUIET, ONE_COMMAND),
            None
        );
    }

    #[test]
    fn a_note_is_said_in_purlis_s_words_with_the_program_where_one_was_named() {
        assert_eq!(
            Note::Quiet { minutes: 15 }.said("check prod", 9),
            "task 'check prod' (chat 9) has shown no activity for 15 minutes"
        );
        assert_eq!(
            Note::OneCommand {
                minutes: 30,
                program: Some("cargo".to_owned())
            }
            .said("check prod", 9),
            "task 'check prod' (chat 9) is running one command for 30 minutes: `cargo`"
        );
        assert_eq!(
            Note::OneCommand {
                minutes: 1,
                program: None
            }
            .said("check prod", 9),
            "task 'check prod' (chat 9) is running one command for 1 minute"
        );
    }
}

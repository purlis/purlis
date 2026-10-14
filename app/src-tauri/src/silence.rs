// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Silence, not heartbeats** (#1981, D-6): the app's half of watching each task's last
//! activity. The rules are `purlis_core::silence`'s; this feeds them what the app hears of each
//! task, and does what they answer.
//!
//! - **What it hears** ([`heard`], [`doing`]): a report of the task's own harness (a turn
//!   beginning or ending, a notification), a tool line of its own (a command beginning or coming
//!   back), and what its turn put out. Never its terminal's bytes.
//! - **The look** ([`look`]): on the clock that holds tasks to their time limit
//!   (`crate::overlimit`), every open task whose turn is running and showing the person no
//!   prompt is looked at, held to `quiet-note-minutes` and `one-command-note-minutes` as in
//!   force for its dispatch (`purlis_core::dispatchlimits`).
//! - **A note goes two ways**: to the chat that asked, in purlis's own words, handed to its next
//!   turn and typed a line where it may be (`dispatchtalk::Talk::quiet_word`); and to the task's
//!   tab, a Notice the person can put away ([`EVENT`], [`task_quiet`]). **It ends nothing.** A
//!   task heard from again takes its Notice down.
//!
//! In memory only: nothing here is written anywhere, and a closed chat's goes with it.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::dispatchtalk::QuietWord;
use purlis_core::reopen::Mode;
use purlis_core::silence::Watch;

use crate::planes::{Held, PlaneId, Planes};

/// The event the window is sent when a task's Notice comes or goes.
pub const EVENT: &str = "task-quiet";

/// A task's quiet note, as its tab's Notice shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TaskQuiet {
    pub plane: PlaneId,
    pub session: u32,
    /// The note in purlis's words; none where the task was heard from again.
    pub said: Option<String>,
}

/// Told each Notice that comes or goes. The event carries its project.
pub type Teller = std::sync::Arc<dyn Fn(TaskQuiet) + Send + Sync + 'static>;

/// What one project holds: the watch, and each task's note for its tab.
#[derive(Debug, Default)]
pub struct Quiet {
    watch: Mutex<Watch>,
    notes: Mutex<HashMap<u32, String>>,
}

impl Quiet {
    fn watch(&self) -> MutexGuard<'_, Watch> {
        self.watch.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn notes(&self) -> MutexGuard<'_, HashMap<u32, String>> {
        self.notes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `session`'s note for its tab, where it has one.
    pub fn note_of(&self, session: u32) -> Option<String> {
        self.notes().get(&session).cloned()
    }

    /// Chat `session` has closed: nothing is kept of it.
    pub fn forget(&self, session: u32) {
        self.watch().forget(session);
        self.notes().remove(&session);
    }
}

/// Whether chat `chat` is a task.
fn is_a_task(held: &Held, chat: u32) -> bool {
    held.chats()
        .handed_from(chat)
        .is_some_and(|from| from.mode == Mode::Task)
}

/// Task `chat` was heard from: a report of its own harness's (`ended` says its turn ended), or
/// what its turn put out. Its Notice, where it had one, comes down.
pub fn heard(held: &Held, chat: u32, ended: bool) {
    if !is_a_task(held, chat) {
        return;
    }
    let now = Instant::now();
    let quiet = held.tasks().quiet();
    if ended {
        quiet.watch().turn_ended(chat, now);
    } else {
        quiet.watch().heard(chat, now);
    }
    let had = quiet.notes().remove(&chat).is_some();
    if had {
        held.tell_task_quiet(chat, None);
    }
}

/// A tool line of task `chat`'s own said `said`: a shell command beginning or coming back is
/// kept as one, anything else is activity.
pub fn doing(held: &Held, chat: u32, said: &purlis_core::doing::Said) {
    use purlis_core::doing::{Kind, Said};
    if !is_a_task(held, chat) {
        return;
    }
    let now = Instant::now();
    {
        let quiet = held.tasks().quiet();
        let mut watch = quiet.watch();
        match said {
            Said::Began {
                kind: Kind::Command,
                name,
            } => watch.command_began(chat, name.clone(), now),
            Said::Ended {
                kind: Some(Kind::Command),
            } => watch.command_ended(chat, now),
            _ => watch.heard(chat, now),
        }
    }
    let had = held.tasks().quiet().notes().remove(&chat).is_some();
    if had {
        held.tell_task_quiet(chat, None);
    }
}

/// **Looks at each open task of `held`** at `now`, and notes each that has gone quiet: to the
/// chat that asked, and on its tab.
pub fn look(held: &Held, now: Instant) {
    let tasks: Vec<(u32, u32, String)> = held
        .chats()
        .open_now()
        .iter()
        .filter_map(|open| {
            let from = held.chats().handed_from(open.session)?;
            (from.mode == Mode::Task).then(|| {
                let name = held
                    .chats()
                    .shown_name(open.session)
                    .unwrap_or_else(|| open.name.clone());
                (open.session, from.chat, name)
            })
        })
        .collect();
    for (task, asker, name) in tasks {
        let seen = crate::dispatched::seen_now(held, task);
        let running = seen.running && !seen.prompt_showing();
        let limits = crate::dispatched::limits_between(held, asker, task);
        let note = held.tasks().quiet().watch().look(
            task,
            now,
            running,
            minutes(limits.quiet_note_minutes),
            minutes(limits.one_command_note_minutes),
        );
        let Some(note) = note else {
            continue;
        };
        let said = note.said(&name, task);
        tracing::info!("purlis: {said}");
        held.tasks().quiet().notes().insert(task, said.clone());
        held.tell_task_quiet(task, Some(said.clone()));
        {
            let mut ledger = held.tasks().ledger();
            ledger
                .talk
                .quiet_word(asker, QuietWord { chat: task, said });
            ledger.landed(asker, purlis_core::dispatched::Landed::Quiet(task));
        }
        crate::dispatched::told_or_settled(held, asker);
    }
}

fn minutes(n: u32) -> Duration {
    Duration::from_secs(u64::from(n) * 60)
}

/// Task `session`'s quiet note for its tab, where it has one.
#[tauri::command]
#[specta::specta]
pub fn task_quiet(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    Ok(held.tasks().quiet().note_of(session))
}

/// Puts task `session`'s quiet note away: the task goes on, and a later stretch is noted again.
#[tauri::command]
#[specta::specta]
pub fn put_away_task_quiet(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    held.tasks().quiet().notes().remove(&session);
    Ok(())
}

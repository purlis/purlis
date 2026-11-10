// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a project's chats did, sent to its window as one event about every
//! [`INTERVAL`] rather than one event for each (#4746, part of #4724).
//!
//! A chat that moves on the board, says what it is doing, touches a file or counts a skill call
//! tells the window. Each of those was an event of its own, and each event is a hop onto the
//! window's main thread: with many busy chats the hops grew with the number of chats, up to a
//! few a second for each. Here every telling is queued in the order it came, and once an
//! interval has passed since the first of them, everything queued for a project goes to its
//! window as one [`ChatEvents`]. So a window gets at most one batch an interval, however many
//! chats are busy, and nothing is dropped, merged or reordered: the window applies the batch in
//! the order it was told.
//!
//! **Idle costs nothing.** The thread that sends the batches waits on its queue and wakes only
//! when something is told; it ends when the [`Batches`] that feeds it is dropped.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::doing::ChatDoing;
use crate::hooks::{ChatTouching, Moved};
use crate::planes::PlaneId;
use crate::skillrun::ChatSkillRun;

/// The event the window is sent: a [`ChatEvents`].
pub const EVENT: &str = "chat-events";

/// How long the first telling waits for others to join it before the batch is sent.
pub const INTERVAL: Duration = Duration::from_millis(100);

/// The variable that names another interval, in milliseconds, as the `e2e` build reads it: the
/// nightly scale run's load bench measures the chat events reaching the window with `0`, each
/// telling sent as it comes, beside the batched default (#4883, `app/e2e/specs/*.scale.e2e.ts`).
#[cfg(feature = "e2e")]
pub const INTERVAL_VARIABLE: &str = "PURLIS_E2E_CHAT_EVENTS_MS";

/// The interval the app batches with: [`INTERVAL`], or in the `e2e` build alone the one
/// [`INTERVAL_VARIABLE`] names (`var` reads the environment). **Every other build reads
/// nothing**: the variable is never looked up, so the window anyone is given batches at
/// [`INTERVAL`] whatever its environment holds.
pub fn interval(var: impl FnOnce(&str) -> Option<String>) -> Duration {
    #[cfg(feature = "e2e")]
    {
        var(INTERVAL_VARIABLE)
            .and_then(|ms| ms.trim().parse().ok())
            .map_or(INTERVAL, Duration::from_millis)
    }
    #[cfg(not(feature = "e2e"))]
    {
        let _ = var;
        INTERVAL
    }
}

/// One thing a chat told the window: what each of the four events used to carry on its own.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(tag = "kind", content = "told", rename_all = "kebab-case")]
pub enum ChatEvent {
    /// The chat moved on the board (`chat-moved` before).
    Moved(Moved),
    /// What the chat is doing changed (`chat-doing` before, #1493).
    Doing(ChatDoing),
    /// The chat's tool touched a file (`chat-touching` before, FM-6).
    Touching(ChatTouching),
    /// The chat's open skill run changed (`chat-skill-run` before, SK-3).
    SkillRun(ChatSkillRun),
    /// The chat's Stuck? or work left behind changed (#6014).
    Noticed(crate::stuck::ChatNoticed),
}

impl ChatEvent {
    /// The project whose chat told it.
    fn plane(&self) -> &PlaneId {
        match self {
            Self::Moved(told) => &told.plane,
            Self::Doing(told) => &told.plane,
            Self::Touching(told) => &told.plane,
            Self::SkillRun(told) => &told.plane,
            Self::Noticed(told) => &told.plane,
        }
    }
}

/// What `chat-events` carries: everything one project's chats told within an interval, in the
/// order they told it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ChatEvents {
    pub plane: PlaneId,
    pub events: Vec<ChatEvent>,
}

/// Where tellings are queued. Cloned for each teller; the thread sending the batches ends when
/// the last clone is dropped.
#[derive(Clone)]
pub struct Batches {
    queue: Sender<ChatEvent>,
}

impl Batches {
    /// Starts the thread that sends `send` one [`ChatEvents`] per project with something told,
    /// `interval` after the first telling it has not sent yet.
    pub fn start(interval: Duration, send: impl Fn(ChatEvents) + Send + 'static) -> Self {
        let (queue, told) = mpsc::channel();
        std::thread::Builder::new()
            .name("chat-events".into())
            .spawn(move || sending(&told, interval, &send))
            .expect("the chat events thread starts");
        Self { queue }
    }

    /// Queues `event` for the next batch of its project.
    pub fn tell(&self, event: ChatEvent) {
        // The thread is gone only while the app is going away, when nothing is listening.
        let _ = self.queue.send(event);
    }
}

fn sending(told: &Receiver<ChatEvent>, interval: Duration, send: &impl Fn(ChatEvents)) {
    while let Ok(first) = told.recv() {
        std::thread::sleep(interval);
        for batch in by_plane(std::iter::once(first).chain(told.try_iter())) {
            send(batch);
        }
    }
}

/// `events` as one batch per project, the projects in the order each first told something.
fn by_plane(events: impl Iterator<Item = ChatEvent>) -> Vec<ChatEvents> {
    let mut batches: Vec<ChatEvents> = Vec::new();
    for event in events {
        match batches.iter_mut().find(|one| &one.plane == event.plane()) {
            Some(batch) => batch.events.push(event),
            None => batches.push(ChatEvents {
                plane: event.plane().clone(),
                events: vec![event],
            }),
        }
    }
    batches
}

#[cfg(test)]
mod tests;

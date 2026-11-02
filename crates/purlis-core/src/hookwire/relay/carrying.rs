// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's host carries what its chats queued to the window itself** (#4804).
//!
//! A chat's `dispatch ask`, `tell`, `note` and `answer` are queued first beside the hook socket
//! the chat was given ([`crate::dispatchqueue`]), then the app is asked to deliver them
//! ([`What::Deliver`]). On a pool's runner that queue is on the runner's disk, where the app
//! does not read. So a host told to carry ([`Relaying::carrying_queues_beside`], `purlis serve`
//! alone) answers a chat's `Deliver` itself: it sends each message the chat has waiting on to
//! the window, oldest first, as [`What::Carried`], and answers the `Deliver` with what the window
//! answered of the message it names.
//!
//! **Once, in order.** A message is removed, its floor raised, only once the window has answered
//! it; one the window holds back ([`Answered::Queued`] `delivered: false`), or that nobody heard
//! or answered, stays, and so does every message after it. One whose answer was lost with the
//! link is carried again, and the app answers a message it delivered under that id as delivered,
//! without delivering it again. Each chat's queue is carried by one drain at a time.
//!
//! **What waited goes when the window is back**, as a browser login that waited does: a drain
//! that found nobody there marks the queues as waiting, and the next line a window hears of
//! this host sets off a drain of every chat's queue, on a thread of its own.
//!
//! **Only what was queued while this host carries is carried.** What a chat queued before it
//! started carrying is retired, removed and never delivered: a host before it handed the
//! `Deliver` on to a window that could not read the queue, which told the chat plainly that
//! nothing was sent (#4560's refusal), and a chat of a host before this one ended with it.
//! Delivering either now would send what its chat was told was not sent.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::{Answer, Ask, Heard, Relaying, Replied, lock};
use crate::dispatched::{Answered, Asked, What};
use crate::dispatchqueue::{self, Found};

/// How long the window is waited on to answer one carried message; past it the message stays
/// queued, and is carried again.
const A_CARRIED_MESSAGE_IS_ANSWERED_WITHIN: Duration = Duration::from_secs(30);

/// What a host that carries its chats' queues keeps.
#[derive(Default)]
pub(super) struct Carrying {
    /// The hook socket its chats' queues are beside: none, and it carries nothing.
    socket: Mutex<Option<PathBuf>>,
    /// One drain at a time, so a chat's messages go in the order it queued them.
    draining: Mutex<()>,
}

/// How one carried message came out.
enum Came {
    /// The window answered it: it is delivered, or the app found it undeliverable.
    Answered(Answer),
    /// The window holds it back: it stays, with every message after it.
    HeldBack(Answer),
    /// Nobody heard it, or no answer came: it stays, and goes once the window is back.
    Waits,
}

impl Relaying {
    /// **Carries its chats' queued messages to the window itself** (#4804): those queued beside
    /// the hook socket `socket`. What a runner's host does, where the window cannot read them.
    /// What is queued there already, from before, is retired, never carried.
    pub fn carrying_queues_beside(&self, socket: &Path) {
        let _one = self
            .carrying
            .draining
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        retire_what_is_queued(socket);
        *lock(&self.carrying.socket) = Some(socket.to_path_buf());
    }

    /// Where the queues it carries are, where it carries them.
    fn carries(&self) -> Option<PathBuf> {
        lock(&self.carrying.socket).clone()
    }

    /// **Chat `chat`'s ask, answered here where it is a `Deliver` this host carries**: what the
    /// window answered of the message it names, or that the message waits.
    pub(super) fn carried(&self, connection: u64, ask: &Ask) -> Option<Answer> {
        let Ask::Task(asked) = ask else {
            return None;
        };
        let What::Deliver { id } = &asked.what else {
            return None;
        };
        let socket = self.carries()?;
        Some(self.carry(&socket, connection, asked.chat, Some(id)))
    }

    /// Carries chat `chat`'s queue, oldest first, until one stays: what the window answered of
    /// message `fresh`, or where it was not carried, whether it was delivered before.
    fn carry(&self, socket: &Path, connection: u64, chat: u32, fresh: Option<&str>) -> Answer {
        let _one = self
            .carrying
            .draining
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut answered = None;
        let mut stayed = false;
        for found in dispatchqueue::waiting(socket, chat) {
            let message = match found {
                Found::Message(message) => message,
                Found::Delivered(path) => {
                    dispatchqueue::drop_unchecked(&path);
                    continue;
                }
                Found::Unchecked(path) => {
                    tracing::warn!(
                        "purlisd: a message in chat {chat}'s queue does not check, so it is \
                         removed and not carried ({})",
                        path.display()
                    );
                    dispatchqueue::drop_unchecked(&path);
                    continue;
                }
            };
            let is_fresh = fresh == Some(message.id.as_str());
            match self.carry_one(connection, chat, &message) {
                Came::Answered(answer) => {
                    dispatchqueue::delivered(socket, chat, &message);
                    if is_fresh {
                        answered = Some(answer);
                    }
                }
                Came::HeldBack(answer) => {
                    // Tried again with the next line the window hears, as a drain there is.
                    stayed = true;
                    self.carry_waits.store(true, Ordering::SeqCst);
                    if is_fresh {
                        answered = Some(answer);
                    }
                    break;
                }
                Came::Waits => {
                    stayed = true;
                    self.carry_waits.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
        answered.unwrap_or_else(|| {
            let id = fresh.unwrap_or_default().to_owned();
            // Not reached, it waits behind one that stayed; not found, a drain before took it.
            Answer::Task(Box::new(Answered::Queued {
                delivered: !stayed,
                id,
            }))
        })
    }

    /// Sends one message on, and waits for the window's answer to it.
    fn carry_one(&self, connection: u64, chat: u32, message: &dispatchqueue::Waiting) -> Came {
        let ask = Ask::Task(Box::new(Asked {
            chat,
            what: What::Carried {
                id: message.id.clone(),
                message: message.what.clone(),
            },
        }));
        let Some((id, replied)) = self.exchange(&Heard::Ask {
            connection,
            ask,
            alive: false,
        }) else {
            return Came::Waits;
        };
        let reply = replied.recv_timeout(A_CARRIED_MESSAGE_IS_ANSWERED_WITHIN);
        self.done(id);
        match reply {
            Ok(Replied::Answer(answer)) => match &answer {
                Answer::Task(answered)
                    if matches!(
                        **answered,
                        Answered::Queued {
                            delivered: false,
                            ..
                        }
                    ) =>
                {
                    Came::HeldBack(answer)
                }
                _ => Came::Answered(answer),
            },
            _ => Came::Waits,
        }
    }

    /// **Carries every chat's queue once the window hears this host again**, where a drain
    /// found nobody there, or none has looked since this host started: on a thread of its own.
    pub(super) fn carry_what_waited(self: &Arc<Self>) {
        if self.carries().is_none() || !self.carry_waits.swap(false, Ordering::SeqCst) {
            return;
        }
        let relaying = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("purlisd-carry-queued".into())
            .spawn(move || {
                let Some(socket) = relaying.carries() else {
                    return;
                };
                for chat in dispatchqueue::chats(&socket) {
                    if dispatchqueue::holds(&socket, chat) {
                        relaying.carry(&socket, 0, chat, None);
                    }
                }
            });
        if let Err(why) = spawned {
            self.carry_waits.store(true, Ordering::SeqCst);
            tracing::warn!("purlisd: what chats queued was not carried: {why}");
        }
    }
}

/// **Removes every message queued beside `socket`**, none delivered: what was queued before a
/// host carried them ([`Relaying::carrying_queues_beside`]).
fn retire_what_is_queued(socket: &Path) {
    for chat in dispatchqueue::chats(socket) {
        for found in dispatchqueue::waiting(socket, chat) {
            match found {
                Found::Message(message) => {
                    tracing::warn!(
                        "purlisd: chat {chat} queued a message before this host carried its \
                         messages, and its sender was told it was not sent: it is removed, and \
                         not carried"
                    );
                    dispatchqueue::delivered(socket, chat, &message);
                }
                Found::Delivered(path) | Found::Unchecked(path) => {
                    dispatchqueue::drop_unchecked(&path);
                }
            }
        }
    }
}

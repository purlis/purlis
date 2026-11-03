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
//! without delivering it again. Each chat's queue is carried by one drain at a time; a `Deliver`
//! whose message a drain before it carried is told what the window answered there, kept for it.
//!
//! **What waited goes when the window is back**, as a browser login that waited does: a drain
//! that found nobody there marks the queues as waiting, and the next line a window hears of
//! this host sets off a drain of every chat's queue, on a thread of its own.
//!
//! **Only what a host told its chat waits is carried.** Each `Deliver` a host answers, it first
//! writes down ([`dispatchqueue::TOLD_WAITS`]), and a host starting to carry retires every
//! message queued beside its socket that no host wrote down, removed and never delivered: it
//! was queued while no host carried, and a host then handed its `Deliver` on to a window that
//! could not read the queue, which told the chat plainly that nothing was sent (#4560's
//! refusal). Delivering it now would send what its chat was told was not sent. So what waits
//! after `purlis serve` restarts is still carried (#4895), and what a build from before hosts
//! carried queued in between is not, whatever a host before it wrote down (#4941): that build
//! writes nothing down. The retire looks at every message in the folder, past any a sender is
//! still writing.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::{Answer, Ask, Heard, Relaying, Replied, lock};
use crate::dispatched::{Answered, Asked, What};
use crate::dispatchqueue::{self, Found};

/// How long the window is waited on to answer one carried message; past it the message stays
/// queued, and is carried again.
#[cfg(not(test))]
const A_CARRIED_MESSAGE_IS_ANSWERED_WITHIN: Duration = Duration::from_secs(30);
#[cfg(test)]
const A_CARRIED_MESSAGE_IS_ANSWERED_WITHIN: Duration = Duration::from_millis(500);

/// How many of the window's answers to messages a drain carried, ahead of their own `Deliver`,
/// are kept for that `Deliver` to be told: the newest, as a chat waits on its own at once.
const ANSWERS_KEPT: usize = 64;

/// What a host that carries its chats' queues keeps.
#[derive(Default)]
pub(super) struct Carrying {
    /// The hook socket its chats' queues are beside: none, and it carries nothing.
    socket: Mutex<Option<PathBuf>>,
    /// One drain at a time, so a chat's messages go in the order it queued them.
    draining: Mutex<()>,
    /// What the window answered of each message a drain carried that was not its own `Deliver`'s
    /// (chat, id, answer), newest last: what that `Deliver`, finding it gone, is told.
    answered: Mutex<VecDeque<(u32, String, Answer)>>,
    /// How many drains of every chat's queue run on a thread of their own: one at most.
    drains: std::sync::atomic::AtomicUsize,
    /// The ids of the messages it told each chat wait, as written down beside the queues
    /// ([`dispatchqueue::TOLD_WAITS`]), newest last.
    told: Mutex<Told>,
}

/// The ids of the messages a host told each chat wait, newest last.
type Told = BTreeMap<u32, VecDeque<String>>;

/// What [`dispatchqueue::TOLD_WAITS`] holds.
#[derive(serde::Serialize, serde::Deserialize)]
struct ToldFile {
    v: u32,
    chats: Told,
}

/// The version of [`ToldFile`].
const TOLD_VERSION: u32 = 1;

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
    /// What is queued there already that a host before this one told its chat waits goes once
    /// the window hears this host; the rest was refused, and is retired, never carried.
    pub fn carrying_queues_beside(&self, socket: &Path) {
        let _one = self
            .carrying
            .draining
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut told = lock(&self.carrying.told);
        *told = retire_what_no_host_told_waits(socket, &told_waits(socket));
        write_told_waits(socket, &told);
        if !told.is_empty() {
            self.carry_waits.store(true, Ordering::SeqCst);
        }
        *lock(&self.carrying.socket) = Some(socket.to_path_buf());
    }

    /// **Writes down that chat `chat` is told its message `id` waits**, before it is told so or
    /// carried: a host started since carries it, where it is still queued then.
    fn telling_it_waits(&self, socket: &Path, chat: u32, id: &str) {
        let mut told = lock(&self.carrying.told);
        let ids = told.entry(chat).or_default();
        // Past the most a chat may queue, the oldest is delivered: carried in order, every one
        // after a message still queued is still queued.
        if ids.len() >= dispatchqueue::MOST_QUEUED {
            ids.pop_front();
        }
        ids.push_back(id.to_owned());
        write_told_waits(socket, &told);
    }

    /// How many drains of every chat's queue run on threads of their own now.
    #[cfg(test)]
    pub(super) fn drains_running(&self) -> usize {
        self.carrying.drains.load(Ordering::SeqCst)
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
        self.telling_it_waits(&socket, asked.chat, id);
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
                    } else {
                        self.keep_answer(chat, message.id, answer);
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
        if answered.is_none() {
            // Carried by a drain before this one: told what the window answered it there.
            answered = fresh.and_then(|fresh| self.answer_kept(chat, fresh));
        }
        answered.unwrap_or_else(|| {
            let id = fresh.unwrap_or_default().to_owned();
            // Not reached, it waits behind one that stayed; not found and no answer of the
            // window's kept for it, a drain before took it.
            Answer::Task(Box::new(Answered::Queued {
                delivered: !stayed,
                id,
            }))
        })
    }

    /// Keeps what the window answered of chat `chat`'s message `id`, carried ahead of its own
    /// `Deliver`, for that `Deliver`.
    fn keep_answer(&self, chat: u32, id: String, answer: Answer) {
        let mut answered = lock(&self.carrying.answered);
        if answered.len() == ANSWERS_KEPT {
            answered.pop_front();
        }
        answered.push_back((chat, id, answer));
    }

    /// What the window answered of chat `chat`'s message `id`, where a drain carried it ahead of
    /// its own `Deliver`: told once.
    fn answer_kept(&self, chat: u32, id: &str) -> Option<Answer> {
        let mut answered = lock(&self.carrying.answered);
        let at = answered
            .iter()
            .position(|(of, kept, _)| *of == chat && kept == id)?;
        answered.remove(at).map(|(_, _, answer)| answer)
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
        if self.carries().is_none() {
            return;
        }
        // One drain at a time (#4892): one that runs carries what waits, or marks it waiting
        // again for the line after it ends. Each waits up to a while on an app that never
        // answers a carried message, one from before #4804, so a drain a line set off while one
        // ran would only queue behind it, one more thread for every line.
        let drains = &self.carrying.drains;
        if drains
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        if !self.carry_waits.swap(false, Ordering::SeqCst) {
            drains.fetch_sub(1, Ordering::SeqCst);
            return;
        }
        let relaying = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("purlisd-carry-queued".into())
            .spawn(move || {
                let _gone = Gone(&relaying.carrying.drains);
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
            drains.fetch_sub(1, Ordering::SeqCst);
            self.carry_waits.store(true, Ordering::SeqCst);
            tracing::warn!("purlisd: what chats queued was not carried: {why}");
        }
    }
}

/// Counts a drain's thread gone, however it ends.
struct Gone<'a>(&'a std::sync::atomic::AtomicUsize);

impl Drop for Gone<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// **Removes every message queued beside `socket` that no host told its chat waits** (`told`),
/// none delivered, past any message a sender is still writing: what was queued while no host
/// carried. What it keeps, as [`Told`].
fn retire_what_no_host_told_waits(socket: &Path, told: &Told) -> Told {
    let mut kept = Told::new();
    for chat in dispatchqueue::chats(socket) {
        let waits = told.get(&chat);
        for found in dispatchqueue::every_queued(socket, chat) {
            match found {
                Found::Message(message) if waits.is_some_and(|ids| ids.contains(&message.id)) => {
                    kept.entry(chat).or_default().push_back(message.id);
                }
                Found::Message(message) => {
                    tracing::warn!(
                        "purlisd: chat {chat} queued a message while no host carried its \
                         messages, and its sender was told it was not sent: it is removed, and \
                         not carried"
                    );
                    dispatchqueue::retired(&message);
                }
                Found::Delivered(path) | Found::Unchecked(path) => {
                    dispatchqueue::drop_unchecked(&path);
                }
            }
        }
    }
    kept
}

/// What hosts before this one told each chat waits ([`dispatchqueue::TOLD_WAITS`]): nothing,
/// where it is not there or does not read, and so every message queued is retired.
fn told_waits(socket: &Path) -> Told {
    std::fs::read(dispatchqueue::told_waits_for(socket))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ToldFile>(&bytes).ok())
        .filter(|file| file.v == TOLD_VERSION)
        .map(|file| file.chats)
        .unwrap_or_default()
}

/// **Writes down what this host told each chat waits** ([`dispatchqueue::TOLD_WAITS`]), whole.
/// Where it cannot, a host started since retires what it did not find written: lost, never
/// delivered after its sender was told it was not sent.
fn write_told_waits(socket: &Path, told: &Told) {
    let at = dispatchqueue::told_waits_for(socket);
    let written = (|| {
        let dir = at
            .parent()
            .ok_or_else(|| std::io::Error::other("no folder"))?;
        crate::secrets::make_private_dir(dir)?;
        let bytes = serde_json::to_vec(&ToldFile {
            v: TOLD_VERSION,
            chats: told.clone(),
        })
        .map_err(std::io::Error::other)?;
        crate::rewrite::replace(dir, &at, &bytes, crate::rewrite::Mode::Private)
    })();
    if let Err(why) = written {
        tracing::warn!(
            "purlisd: what this host told its chats waits was not written down, so what they \
             queue while the link is down is retired if this host restarts ({}): {why}",
            at.display()
        );
    }
}

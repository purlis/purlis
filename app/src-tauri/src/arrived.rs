// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of [`purlis_core::sandbox::arrived`]: when a sandboxed chat's turn ends, what
//! has appeared in its folders is looked at, off the hook channel's thread and the window's, so a
//! turn's end is never held up by it. The Inbox reads what it found from the core's record
//! ([`crate::alerts`]).
//!
//! **One look at a time, per chat.** A turn that ends while its chat's look runs asks for one more
//! after it: that one sees what the turn left, and any number of ends in between cost one look.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::sandbox::arrived;

use crate::planes::Held;

/// **Chat `id` is starting**: what is in `watched` now is recorded before its program starts,
/// unless that takes longer than a start may wait. Then it is finished beside the chat's first
/// moments, and the app's start goes on: a very large folder must not hold a chat up. What can't be
/// recorded is told to the log: a chat with no record is not told of.
pub fn began(root: &Path, id: &str, name: &str, watched: arrived::Watched) {
    let (done, finished) = std::sync::mpsc::channel();
    let (root, thread_id, name) = (root.to_path_buf(), id.to_owned(), name.to_owned());
    let spawned = std::thread::Builder::new()
        .name("arrived-begin".to_owned())
        .spawn(move || {
            if let Err(why) = arrived::begin(&root, &thread_id, Some(&name), &watched) {
                tracing::warn!("purlis: chat {thread_id}'s folders were not recorded ({why})");
            }
            let _ = done.send(());
        });
    match spawned {
        Ok(_) => {
            let _ = finished.recv_timeout(arrived::BEGIN_WAITS_AT_MOST);
        }
        Err(why) => tracing::warn!("purlis: no thread to record chat {id}'s folders ({why})"),
    }
}

/// What a chat's looks are at, by its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Running,
    AndOneMore,
}

static LOOKS: Mutex<Option<HashMap<String, Look>>> = Mutex::new(None);

/// Whether a look must start: `false` where one runs, which is told to go again.
fn asks_for_one(chat: &str) -> bool {
    let mut looks = LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    let looks = looks.get_or_insert_with(HashMap::new);
    match looks.get_mut(chat) {
        Some(look) => {
            *look = Look::AndOneMore;
            false
        }
        None => {
            looks.insert(chat.to_owned(), Look::Running);
            true
        }
    }
}

/// Whether another look is owed now one has ended; where none is, the chat has none running.
fn one_ended(chat: &str) -> bool {
    let mut looks = LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    let looks = looks.get_or_insert_with(HashMap::new);
    if looks.get(chat) == Some(&Look::AndOneMore) {
        looks.insert(chat.to_owned(), Look::Running);
        true
    } else {
        looks.remove(chat);
        false
    }
}

/// **Chat `chat`'s turn has ended.** A chat with no record is one with no sandbox, and its look
/// finds that and ends.
pub fn turn_ended(held: &Arc<Held>, chat: u32) {
    let Some(id) = held.chats().id_of(chat) else {
        return;
    };
    if !asks_for_one(&id) {
        return;
    }
    let weak = Arc::downgrade(held);
    let name = held.chats().shown_name(chat);
    let spawned = std::thread::Builder::new()
        .name("arrived-look".to_owned())
        .spawn({
            let id = id.clone();
            move || {
                loop {
                    let Some(held) = weak.upgrade() else {
                        one_ended(&id);
                        return;
                    };
                    look(&held, chat, &id, name.as_deref());
                    drop(held);
                    if !one_ended(&id) {
                        return;
                    }
                }
            }
        });
    if let Err(why) = spawned {
        one_ended(&id);
        tracing::warn!("purlis: no thread to look through chat {chat}'s folders ({why})");
    }
}

/// One look, its finding in the audit. A look that fails says so in the log: it tells the person
/// nothing, and the next turn's looks again.
fn look(held: &Held, chat: u32, id: &str, name: Option<&str>) {
    match arrived::look(held.root(), id, name) {
        Ok(Some(looked)) if looked.says_anything() => {
            if let Err(why) = held.hooks().record_arrived(held.root(), chat, &looked) {
                tracing::warn!("purlis: what appeared in chat {chat}'s folders: {why}");
            }
        }
        Ok(_) => {}
        Err(why) => tracing::warn!("purlis: chat {chat}'s folders were not looked at ({why})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_that_end_while_a_look_runs_cost_one_more_look() {
        let chat = "01M6M3VNN8GNE85XEV3194WJE6";
        assert!(asks_for_one(chat), "the first starts one");
        assert!(!asks_for_one(chat), "one runs");
        assert!(!asks_for_one(chat), "and still only one more");
        assert!(one_ended(chat), "the one more");
        assert!(!one_ended(chat), "and no other");
        assert!(asks_for_one(chat), "none runs now");
        assert!(!one_ended(chat));
    }
}

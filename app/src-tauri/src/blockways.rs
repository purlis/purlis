// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A block's Tell the chat…** (T1-33; R-189, R-192; Q267): the one way out of a block that
//! reaches the chat. On the person's second press, purlis types one line of its own into the
//! chat ([`TELL_LINE`]): that the call stays blocked, and not to try it again.
//!
//! **One way in, T1-36's** (Q267): when it is typed is `pausing::telling`'s answer, from the
//! board as the app has it now (`dispatched::seen_now`) and a stop's last turn; it is typed
//! through `Held::type_in_turn`, the way Smart close and Stop type. It types only into a chat
//! that takes keys and only while it waits (`Seen::takes_a_line`): never mid-turn and never at
//! a prompt. **Nothing is kept for later**: where it can't be typed now, the person is told why,
//! and presses again.
//!
//! **It widens nothing** (Q3, Q139): the line allows nothing, names nothing the chat or its
//! harness said (Q259), and no answer or grant is written.

use crate::pausing::Telling;
use crate::planes::{PlaneId, Planes};

/// **What Tell the chat… types** (R-189, R-192): purlis's own words, naming no command, no
/// target and nothing the chat said, as the event log keeps none (R-220).
pub const TELL_LINE: &str = "purlis: the person asks you not to try again what was just \
    blocked. It stays blocked. Don't try it again, in these words or others: go on without it, \
    or say what you needed it for and stop.";

/// **What is typed for [`TELL_LINE`]**: the line as one bracketed paste, then Enter, the way
/// every harness purlis types into takes a line (as `pausing::stop_trying_typed`).
pub fn typed() -> String {
    format!("{}\r", crate::curation::bracketed(TELL_LINE))
}

/// **Tells chat `who` not to try again** as `look` says it stands, typing [`typed`] through
/// `send` only where it takes a line now. Where it does not, nothing is typed or kept, and the
/// answer says why and what to do.
pub(crate) fn tell(
    who: &str,
    look: impl FnOnce() -> Telling,
    send: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    match look() {
        Telling::Now => {
            send(typed().as_bytes()).map_err(|why| format!("purlis could not type in {who}: {why}"))
        }
        Telling::AtTurnEnd => Err(format!(
            "{who} isn't waiting for you, so nothing was typed: a chat is typed into only while \
             it waits. Press again once it does."
        )),
        Telling::Never => Err(format!(
            "purlis can't type into {who} now, so nothing was typed: it has ended, is stopping, \
             or runs a harness purlis can't type into. Go to the chat and tell it there."
        )),
    }
}

/// **Tell the chat…, pressed twice** (T1-33, Q267): chat `session` is typed [`TELL_LINE`] where
/// it waits, through T1-36's way in. Answers why where nothing was typed.
#[tauri::command]
#[specta::specta]
pub fn tell_chat_blocked(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let who = held
        .chats()
        .shown_name(session)
        .unwrap_or_else(|| format!("chat {session}"));
    tell(
        &who,
        || {
            crate::pausing::telling(
                crate::dispatched::seen_now(&held, session),
                held.stopping().in_its_last_turn(session),
            )
        },
        |bytes| held.type_in_turn(session, bytes),
    )
}

#[cfg(test)]
#[path = "blockways/tests.rs"]
mod tests;

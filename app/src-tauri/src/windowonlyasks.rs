// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A refusal a chat's own `purlis` handed the app to raise as an ask** (A3, R-328): the app's
//! answer to [`Ask::WindowOnly`], beside [`crate::removalasks`]'s.
//!
//! The chat's `forge_milestones` tool runs in the `purlis mcp` process, which holds no window,
//! so it cannot raise the ask itself ([`purlis_core::windowonly::raise`] is in-memory, by
//! project). It hands the refusal here. The app raises it, by its own record of the chat's
//! workspace and the clone's own origin, and answers with the sentence the chat is told: the
//! person was asked, or nobody was, in which case the block is written to the chat's Chat log
//! by the writer this process registered ([`purlis_core::windowonly::blocking`]). **The app
//! records nothing for it:** only the person's press of Record this repo does.
//!
//! **The line is the chat's own word, so the app checks what it can:** the clone must be one of
//! the chat's workspace that was there when the chat started and whose git directory no chat may
//! write (as the GitHub App mint counts a clone), and the project's policy must enforce the check
//! that refuses an unrecorded clone ([`purlis_core::windowonly::ask_to_record`]). A chat can
//! always make that refusal itself by reading the clone's milestones, so a line for any other
//! clone is refused, and a line for one is no more than that refusal. Each chat has an ask budget
//! of its own ([`budget`]), never its brokered writes'.

use std::time::{Duration, Instant};

use purlis_core::brokered::Rate;
use purlis_core::hookwire::{Answer, WindowOnlyAsk, WindowOnlyRefusal};

use crate::planes::Held;

/// The most window-only asks one chat may hand the app in [`WINDOW_ONLY_ASKS_PER`]: a loop of
/// them is stopped long before it fills the person's Inbox.
pub const WINDOW_ONLY_ASKS_AT_MOST: usize = 12;

/// The while [`WINDOW_ONLY_ASKS_AT_MOST`] is counted over.
pub const WINDOW_ONLY_ASKS_PER: Duration = Duration::from_secs(60);

/// The budget of window-only asks the app holds every chat to, its own and no write's.
pub fn budget() -> Rate {
    Rate::new(WINDOW_ONLY_ASKS_AT_MOST, WINDOW_ONLY_ASKS_PER)
}

/// What the app answers chat `ask.chat`'s refusal: [`Answer::No`] with the sentence the chat is
/// told.
pub fn asked(held: &Held, ask: &WindowOnlyAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    // The workspace is the app's record of the chat, never a word of the line's.
    let Some(workspace) = held.chats().workspace_of(ask.chat) else {
        return no(format!("chat {} is not one this app has open", ask.chat));
    };
    if held
        .window_only_asks()
        .allow(ask.chat, Instant::now())
        .is_err()
    {
        return no(format!(
            "chat {} has handed purlis {WINDOW_ONLY_ASKS_AT_MOST} refusals to ask the person \
             about in the last {} seconds, which is its cap; nothing more was asked. Wait a \
             little",
            ask.chat,
            WINDOW_ONLY_ASKS_PER.as_secs()
        ));
    }
    let Some(workspace) = workspace else {
        return no("A chat at the project's root has no workspace of repos to record.".to_owned());
    };
    // What the chat could write and what its workspace held when it started: the app's own
    // record of the chat, as the mint reads it.
    let Some(confines) = held.chats().confines_of(ask.chat) else {
        return no(
            "purlis holds no record of the clones this chat started with, so it asked \
                   nobody. Record it with `purlis discover`."
                .to_owned(),
        );
    };
    let mut writes = confines.writable.clone();
    if let Some(caches) = &confines.widened.caches {
        writes.extend(caches.trees.iter().chain(caches.bare.iter()).cloned());
    }
    let reach = purlis_core::windowonly::Reach {
        started: Some(&confines.clones),
        writes: &writes,
    };
    let WindowOnlyRefusal::RecordRepo { clone } = &ask.refused;
    match purlis_core::windowonly::ask_to_record(held.root(), ask.chat, &workspace, clone, reach) {
        Ok(said) | Err(said) => no(said),
    }
}

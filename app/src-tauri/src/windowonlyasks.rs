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

use purlis_core::hookwire::{Answer, WindowOnlyAsk, WindowOnlyRefusal};

use crate::planes::Held;

/// What the app answers chat `ask.chat`'s refusal: [`Answer::No`] with the sentence the chat is
/// told. Held to the rate a chat's brokered writes are, so it cannot fill the person's Inbox.
pub fn asked(held: &Held, ask: &WindowOnlyAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    // The workspace is the app's record of the chat, never a word of the line's.
    let Some(workspace) = held.chats().workspace_of(ask.chat) else {
        return no(format!("chat {} is not one this app has open", ask.chat));
    };
    let Some(workspace) = workspace else {
        return no("A chat at the project's root has no workspace of repos to record.".to_owned());
    };
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let WindowOnlyRefusal::RecordRepo { clone } = &ask.refused;
    match purlis_core::windowonly::ask_to_record(held.root(), ask.chat, &workspace, clone) {
        Ok(said) | Err(said) => no(said),
    }
}

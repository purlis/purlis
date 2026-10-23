// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Brokered writes (ADR 0067 §2, #1333): the project's own files a chat the app started asks
//! the app to write, because its sandbox does not let it. The write is the core's
//! ([`purlis_core::brokered::answer`]); what is the app's is who asks.

use purlis_core::active::Place;
use purlis_core::hookwire::{Answer, WriteAsk};

use crate::planes::Held;

/// Chat `ask.chat` asked the app to write one of the project's own files for it.
///
/// **The place and the persona are the app's record of the chat, never the request's**: the
/// workspace its directory was in when it started, and the persona it was started as. The chat the line names is
/// the one whose token it carries, which the listener checks before this is asked. Each chat is
/// held to [`purlis_core::brokered::Rate`], and every write is credited to it in the trace.
pub fn write(held: &Held, ask: &WriteAsk) -> Answer {
    let Some(open) = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == ask.chat)
    else {
        return Answer::No {
            why: format!("chat {} is not one this app has open", ask.chat),
        };
    };
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return Answer::No { why };
    }
    // Resolved once, as the chat started (`chats::Open::workspace`): a link made since cannot
    // move where its writes go.
    let place = open.workspace.map_or(Place::PlaneRoot, Place::Workspace);
    let asker = purlis_core::brokered::Asker {
        chat: ask.chat,
        place,
        persona: open.persona,
    };
    // **A chat on a pool's runner changes nothing here unasked** (#2046, ADR 0078 §7): its write
    // is held for the person as a proposal, and made only on their Apply. A write that came over
    // a pool's link is held by that link alone. One that came over this machine's link is held
    // where the router records its chat on a pool (`pool_of`). That link's lines for a pool's
    // chat are refused before they reach here (`hooks::Relayed`), so this is a second guard.
    let pool = crate::hooks::heard_from_a_pool().or_else(|| held.chats().pool_of(ask.chat));
    if let Some(pool) = pool {
        return crate::runnerwrites::hold(held, &pool, asker, ask.write.clone());
    }
    answered(
        held.root(),
        &asker,
        &ask.write,
        chrono::Local::now().naive_local(),
    )
}

/// The answer to `write`, made for `asker` (the app's record of the chat, never the request's)
/// in the project at `root`: what the window answers a chat here, and what a runner chat's held
/// write makes on the person's Apply (`crate::runnerwrites`).
pub(crate) fn answered(
    root: &std::path::Path,
    asker: &purlis_core::brokered::Asker,
    write: &purlis_core::brokered::Write,
    now: chrono::NaiveDateTime,
) -> Answer {
    let chat = asker.chat;
    match purlis_core::brokered::answer(root, asker, write, now) {
        Ok(purlis_core::brokered::Done::Written(written)) => {
            tracing::info!(
                "purlis: wrote {} for chat {} (a brokered write)",
                written.path,
                chat
            );
            Answer::Written {
                to: written.to,
                path: written.path,
            }
        }
        // A memory verb (#1408): what its command says, which the command prints as its own.
        Ok(purlis_core::brokered::Done::Said { lines, code }) => {
            tracing::info!(
                "purlis: ran a memory verb for chat {chat} (a brokered write, exit {code})"
            );
            Answer::Said { lines, code }
        }
        Err(why) => Answer::No { why },
    }
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's answers to a chat's Asks, told to the chat** (#1987, ruling Q64): the app's
//! side of [`purlis_core::askoutcome`], whose [`Outcome::line`] writes the words.
//!
//! An Allow that restarts a chat tells it in the restart's first message
//! ([`purlis_core::sandbox::grant::told`]); an Allow a chat's proxy takes live, and every Keep
//! blocked, go to the chat's next turn through [`crate::chats::Chats::tell_outcome`]; a vault's
//! Allow is typed into a waiting chat (`crate::vaultroute`). All of them are that one line.
//! [`about`] matches every kind a block Notice answers with no catch-all arm, so a new kind does
//! not build until it says how its chat is told.

use purlis_core::askoutcome::{About, Answer, Outcome};

use crate::sandboxing::GrantWhat;

/// The Ask a block on `target` of kind `what` raised, as its chat is told it; none for a kind
/// no block raises.
pub(crate) fn about(what: GrantWhat, target: &str) -> Option<About> {
    let target = target.to_owned();
    match what {
        GrantWhat::Host => Some(About::Host { host: target }),
        GrantWhat::Write => Some(About::Write { folder: target }),
        GrantWhat::Socket => Some(About::Socket { socket: target }),
        GrantWhat::Service => Some(About::Service { service: target }),
        GrantWhat::Port => Some(About::Port { port: target }),
        GrantWhat::Listen => Some(About::Listen { port: target }),
        GrantWhat::Vault => Some(About::Vault { vault: target }),
        // Allowed from its own Notice or the browser's Ask, which no chat's refusal raises.
        GrantWhat::PersonaHosts | GrantWhat::Browser | GrantWhat::BrowserSite => None,
    }
}

/// An Ask's Keep blocked, with the reason the person typed, if any.
pub(crate) fn kept(about: About, reason: Option<&str>) -> Outcome {
    Outcome {
        about,
        answer: Answer::KeptBlocked {
            reason: purlis_core::askoutcome::reason(reason),
        },
    }
}

/// **The person kept chat `session`'s block blocked**: the chat's next turn is told so in one
/// line, with `reason`.
pub(crate) fn kept_blocked(
    chats: &crate::chats::Chats,
    session: u32,
    (what, target): (GrantWhat, &str),
    reason: Option<&str>,
) {
    if let Some(about) = about(what, target) {
        chats.tell_outcome(session, kept(about, reason));
    }
}

#[cfg(test)]
mod tests;

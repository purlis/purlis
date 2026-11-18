// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The session host's decisions** (M75 slice 04, #7012; spec §6): what the app answers a
//! hook's [`Ask::Decide`](purlis_core::hookwire::Ask::Decide).
//!
//! The app alone loads, verifies and evaluates the policy. A hook names the action and its
//! resource; the chat is the one whose token the line carried, which the listener checked before
//! this is asked, and everything the policy reads of it is this app's record: its workspace and
//! persona, whether it runs sandboxed, whether the person is at it. The policy is the project's
//! as committed at the plane root's `HEAD` ([`purlis_core::policy::decide::in_plane`]), never a
//! file the line names.

use purlis_core::hookwire::{Answer, DecideAsk};
use purlis_core::policy::decide::{self, Asker};
use purlis_core::policy::levels::Chat;

use crate::planes::Held;

/// **Answers chat `asked.chat`'s ask for one decision**: refused for a chat this app does not
/// have open, since nothing of it is known to decide by. A deny is written to the event log here,
/// as the host decided it.
pub fn asked(held: &Held, asked: &DecideAsk) -> Answer {
    let chat = asked.chat;
    let Some(decision) = decision(held, chat, &asked.action, (&asked.kind, &asked.id)) else {
        return Answer::No {
            why: format!("chat {chat} is not one this app has open, so nothing was decided."),
        };
    };
    if decision.after_mode == decide::Outcome::Deny {
        blocked(held, chat, &asked.tool, &decision);
    }
    Answer::Decided(Box::new(decision))
}

/// **What the project's policy decides of chat `chat` doing `action` to `resource`**, by this
/// app's record of the chat: none for a chat this app does not have open.
pub fn decision(
    held: &Held,
    chat: u32,
    action: &str,
    resource: (&str, &str),
) -> Option<decide::Decision> {
    let chats = held.chats();
    let workspace = chats.workspace_of(chat)?;
    let persona = chats
        .persona_of(chat)
        .and_then(|persona| persona.started_as);
    let attended = chats.attendance_of(chat).is_some_and(|fact| {
        fact.hearing(false, None) == purlis_core::dispatchunattended::Attendance::Attended
    });
    // Which of the two levels a sandboxed chat runs at is slice 11's to say: `floor` never
    // claims more confinement than the chat has.
    let sandbox = if chats.runs_sandboxed(chat) {
        "floor"
    } else {
        "off"
    };
    let asker = Asker {
        chat,
        of: Chat {
            workspace: workspace.as_deref(),
            persona: persona.as_deref(),
        },
        sandbox,
        attended,
        harness_mode: "default",
        depth: 0,
        via: "hook",
    };
    Some(decide::in_plane(held.root(), &asker, action, resource))
}

/// **Writes the block a deny is**, as the host decided it (Q313.3): source `policy`, aimed at
/// the first deciding rule, with every deciding rule, its level, the family and both outcomes.
/// Written here, not from the chat's own line, which a chat at the sandbox level `off` can reach.
fn blocked(held: &Held, chat: u32, tool: &str, decision: &decide::Decision) {
    use purlis_core::eventlog::stops::{Block, Decided, Source, Stop, TARGET_CHARS, Target};
    let stop = Stop::Block(Block {
        tool: purlis_core::shown::readable(tool, TARGET_CHARS),
        source: Source::Policy,
        qualifier: None,
        target: decision
            .deciding
            .first()
            .and_then(|rule| Target::guard_arm(&rule.id).ok()),
        args: None,
        policy: Some(Decided::from(decision)),
    });
    if let Err(why) = held.hooks().write_stop(chat, stop.clone()) {
        tracing::warn!("purlis: chat {chat}'s policy block was not recorded: {why}");
    }
    crate::pausing::stopped(held, chat, &stop, None);
}

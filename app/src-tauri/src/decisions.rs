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
use purlis_core::policy::decide;

use crate::planes::Held;

/// **Answers chat `asked.chat`'s ask for one decision**: refused for a chat this app does not
/// have open, since nothing of it is known to decide by. A deny is written to the event log here,
/// as the host decided it.
pub fn asked(held: &Held, asked: &DecideAsk) -> Answer {
    let chat = asked.chat;
    // Only what an enforcement point is wired for: a chat names nothing else for the host to
    // decide, record as a block or pause itself on (M75-04 round 2).
    if !decide::is_asked_by_an_enforcement_point(&asked.action, &asked.kind) {
        return Answer::No {
            why: format!(
                "nothing asks the host to decide {} on a {}.",
                purlis_core::shown::readable(&asked.action, 40),
                purlis_core::shown::readable(&asked.kind, 40)
            ),
        };
    }
    let id = with_its_repo(held, asked);
    let Some(decision) = decision(held, chat, &asked.action, (&asked.kind, &id)) else {
        return Answer::No {
            why: format!("chat {chat} is not one this app has open, so nothing was decided."),
        };
    };
    if decision.after_mode == decide::Outcome::Deny {
        blocked(held, chat, &asked.tool, &decision);
    }
    Answer::Decided(Box::new(decision))
}

/// **The resource's id with its repo found**, as the merge the host then raises decides it: a
/// merge the command gave no repo (`#12`) is of the repo of the clone the hook ran in
/// ([`crate::mergeholds::found`]). The id as it came where it cannot be found: the merge is
/// refused later all the same, and nothing here decides on a guess.
fn with_its_repo(held: &Held, asked: &DecideAsk) -> String {
    use purlis_core::hookwire::MergeAsk;
    let number = asked
        .id
        .strip_prefix('#')
        .and_then(|n| n.parse::<u64>().ok());
    let (Some(number), Some(cwd), "merge") = (number, &asked.cwd, asked.action.as_str()) else {
        return asked.id.clone();
    };
    let ask = MergeAsk {
        chat: asked.chat,
        request: purlis_core::mergehold::Request { repo: None, number },
        cwd: cwd.clone(),
    };
    match crate::mergeholds::found(held.root(), &ask) {
        Ok((_, repo)) => format!("{}#{number}", repo.as_str()),
        Err(_) => asked.id.clone(),
    }
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
    let facts = chats.workspace_of(chat).map(|workspace| decide::ChatFacts {
        workspace,
        persona: chats
            .persona_of(chat)
            .and_then(|persona| persona.started_as),
        sandboxed: chats.runs_sandboxed(chat),
        attended: chats.attendance_of(chat).is_some_and(|fact| {
            fact.hearing(false, None) == purlis_core::dispatchunattended::Attendance::Attended
        }),
    });
    // The accepted floor and the machine's locks, until slices 06 and 14 read them in the loader.
    let floor = purlis_core::enforcement::in_force(held.root());
    // What the app's sealed store says now: the deny state refuses every action but `start`, and
    // `context.machine` is the device id the person confirmed (M75-07b).
    let sealed = decide::Sealed::of(held.root());
    decide::for_chat(
        held.root(),
        chat,
        facts.as_ref(),
        &floor,
        &sealed,
        action,
        resource,
    )
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

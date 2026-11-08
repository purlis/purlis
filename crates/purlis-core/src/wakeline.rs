// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The wake line** (spec #5743; ADR 0068 as amended 2026-11-08, "A reopened chat is told
//! what the restart ended"): the one message purlis types into a chat it brings back on its
//! conversation after a restart ended the chat's program.
//!
//! A chat whose turn ended on background work of its own waits for that work to wake it. A
//! restart that ends the harness ends the work with it, and the resumed harness never hears
//! of it, so nothing would ever wake the chat. The wake line tells it what ended; **the chat
//! decides what to start again**, and purlis starts none of it.
//!
//! It is purlis's own sentence, made only from counts the app recorded
//! ([`crate::reopen::Chat::own_work`]) and from whether the chat is a task told to carry on
//! ([`crate::dispatchrestart::CARRY_ON`], which it absorbs, so a task gets one message and
//! not two). It names no chat, no command and no path, so nothing read off a file a chat can
//! write reaches a line a harness is started with.
//!
//! **The tracer** (#5747) names the kinds of work in flight and how many of each. What each
//! one last printed, the wake-ups and crons that were lost, a turn cut mid-way and the news of
//! the chat's tasks are its siblings under #5743.

use crate::dispatchrestart::CARRY_ON;
use crate::ownwork::OwnWork;

/// What the chat is told after the work it had left running is listed: that none of it runs
/// now, and that starting it again is the chat's to decide.
const NONE_OF_IT_RUNS: &str = "None of it is running now, and nothing will come back from \
    it: start again whatever is still needed.";

/// **The wake line for a chat brought back on its conversation after a restart**, or none
/// where there is nothing to tell it.
///
/// `own_work` is what the chat had left running of its own as the restart ended it, and
/// `carry_on` whether it is a task that still owes its report. A chat with neither is told
/// nothing. A task with no own work is told [`CARRY_ON`] alone, word for word.
pub fn of(own_work: Option<&OwnWork>, carry_on: bool) -> Option<String> {
    let ended = own_work.and_then(ended);
    match (carry_on, ended) {
        (false, None) => None,
        (true, None) => Some(CARRY_ON.to_owned()),
        (false, Some(ended)) => Some(format!(
            "purlis: purlis was restarted, and this is this chat's conversation brought back. \
             The restart ended what this chat had left running in the background: {ended}. \
             {NONE_OF_IT_RUNS}"
        )),
        (true, Some(ended)) => Some(format!(
            "{CARRY_ON} The restart also ended what this chat had left running in the \
             background: {ended}. {NONE_OF_IT_RUNS}"
        )),
    }
}

/// **The wake line for a chat restarted on its own while purlis runs on** (#5753): Restart
/// now, a restart without the sandbox, a restart that contains its project's settings, or its
/// move to or from a runner, each of which ends its program with what it had left running. Only where it had
/// something of its own running; the rest of what the restart tells it is the restart's own.
pub fn of_the_chat_restarted(own_work: Option<&OwnWork>) -> Option<String> {
    own_work.and_then(ended).map(|ended| {
        format!(
            "purlis: this chat was restarted on its conversation, and the restart ended what it \
             had left running in the background: {ended}. {NONE_OF_IT_RUNS}"
        )
    })
}

/// The work in flight `own_work` names, as a list: "1 background shell, 2 monitors". None
/// where none is in flight.
fn ended(own_work: &OwnWork) -> Option<String> {
    let kinds = [
        (own_work.shells, "background shell", "background shells"),
        (own_work.monitors, "monitor", "monitors"),
        (own_work.helpers, "subagent", "subagents"),
        (own_work.workflows, "workflow", "workflows"),
    ];
    let named: Vec<String> = kinds
        .into_iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, one, many)| format!("{count} {}", if count == 1 { one } else { many }))
        .collect();
    (!named.is_empty()).then(|| named.join(", "))
}

#[cfg(test)]
#[path = "wakeline_tests.rs"]
mod tests;

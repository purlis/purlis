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
//! It is purlis's own sentence, made from what the app recorded of the chat
//! ([`crate::reopen::Chat::own_work`], [`crate::reopen::Chat::own_pieces`],
//! [`crate::reopen::Chat::mid_turn`]) and from whether the chat is a task told to carry on
//! ([`crate::dispatchrestart::CARRY_ON`], which it absorbs, so a task gets one message and
//! not two). **What the chat said of its own work is quoted as data**: each piece's words were
//! held to one line of [`crate::ownwork::pieces::MOST_TEXT_BYTES`] with no control character as
//! they were recorded, and the whole line to [`MOST_BYTES`], so it is always a first message a
//! harness can be started with.
//!
//! **What it says** (#5747, #5750): that the turn under way was cut mid-way, and each piece of
//! own work the restart ended, by kind and by what it was; by count where the record names no
//! piece. The rest of the spec's list (each piece's last output, a missed wake-up, the news of
//! the chat's tasks) is its siblings under #5743.

use crate::dispatchrestart::CARRY_ON;
use crate::ownwork::OwnWork;
use crate::ownwork::pieces::{Kind, Piece};

/// The most bytes a wake line is: under a first message's bound
/// ([`crate::handoff::FIRST_MESSAGE_MAX_BYTES`]), with room left for the start's own words.
pub const MOST_BYTES: usize = 10 * 1024;

/// What the chat is told after the work it had left running is listed: that none of it runs
/// now, and that starting it again is the chat's to decide.
const NONE_OF_IT_RUNS: &str = "None of it is running now, and nothing will come back from \
    it: start again whatever is still needed.";

/// What a chat cut off mid-turn is told.
const CUT_MID_WAY: &str = "The restart ended this chat's last turn mid-way: check what it had \
    half done before going on.";

/// **What the restart ended of a chat**, as its record says: what it had left running of its
/// own, each piece by name, and whether a turn of its own was under way.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ended<'a> {
    /// What its last turn's end said it left running, by count.
    pub own_work: Option<&'a OwnWork>,
    /// Each piece of its own work, by name.
    pub pieces: &'a [Piece],
    /// Whether a turn of its own was under way.
    pub mid_turn: bool,
}

/// **The wake line for a chat brought back on its conversation after a restart**, or none
/// where there is nothing to tell it.
///
/// `ended` is what the restart ended of the chat, and `carry_on` whether it is a task that still
/// owes its report. A chat with neither is told nothing. A task with nothing ended is told
/// [`CARRY_ON`] alone, word for word.
pub fn of(ended: &Ended<'_>, carry_on: bool) -> Option<String> {
    let listed = listed(ended);
    if !carry_on && !ended.mid_turn && listed.is_none() {
        return None;
    }
    let mut line = if carry_on {
        CARRY_ON.to_owned()
    } else {
        "purlis: purlis was restarted, and this is this chat's conversation brought back."
            .to_owned()
    };
    if ended.mid_turn {
        line.push(' ');
        line.push_str(CUT_MID_WAY);
    }
    if let Some(listed) = listed {
        let also = if carry_on { " also" } else { "" };
        line.push_str(&format!(
            " The restart{also} ended what this chat had left running in the background: \
             {listed}. {NONE_OF_IT_RUNS}"
        ));
    }
    Some(line)
}

/// The work `ended` names, as a list: each piece where the record names any, else the counts
/// ("1 background shell, 2 monitors"). None where nothing is in flight. Held to what fits in
/// [`MOST_BYTES`]: the pieces past it are counted, not named.
fn listed(ended: &Ended<'_>) -> Option<String> {
    if ended.pieces.is_empty() {
        return ended.own_work.and_then(counted);
    }
    let room = MOST_BYTES - CARRY_ON.len() - CUT_MID_WAY.len() - NONE_OF_IT_RUNS.len() - 200;
    let mut named: Vec<String> = Vec::new();
    let mut used = 0;
    for (at, piece) in ended.pieces.iter().enumerate() {
        let one = named_piece(piece);
        if used + one.len() > room {
            named.push(format!("and {} more", ended.pieces.len() - at));
            break;
        }
        used += one.len() + 2;
        named.push(one);
    }
    Some(named.join("; "))
}

/// One piece, by its kind and what it was: `a background shell "npm test"`.
fn named_piece(piece: &Piece) -> String {
    let kind = match piece.kind {
        Kind::Shell => "a background shell",
        Kind::Monitor => "a monitor",
        Kind::Subagent => "a subagent",
        Kind::Workflow => "a workflow",
        Kind::Cron => "a cron",
        Kind::WakeUp => "a wake-up",
    };
    let mut said = kind.to_owned();
    if !piece.what.is_empty() {
        said.push_str(&format!(" \"{}\"", piece.what));
    }
    if let Some(schedule) = &piece.schedule {
        said.push_str(&format!(" on \"{schedule}\""));
    }
    if let Some(prompt) = &piece.prompt {
        said.push_str(&format!(" with the prompt \"{prompt}\""));
    }
    said
}

/// The work in flight `own_work` counts, as a list: "1 background shell, 2 monitors". None
/// where none is in flight.
fn counted(own_work: &OwnWork) -> Option<String> {
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

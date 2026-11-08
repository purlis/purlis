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
//! [`crate::reopen::Chat::mid_turn`]), from what each piece last put out
//! ([`crate::ownwork::last`]) and from whether the chat is a task told to carry on
//! ([`crate::dispatchrestart::CARRY_ON`], which it absorbs, so a task gets one message and
//! not two). **What the chat and its work said is quoted as data**: each piece's words were held
//! to one line with no control character as they were recorded, each line of output is read the
//! same way, and the whole line is held to [`MOST_BYTES`], so it is always a first message a
//! harness can be started with. What does not fit is named without its output, then counted.
//!
//! **What it says, in this order:**
//!
//! - that the turn under way was cut mid-way (#5750);
//! - each piece of background work the restart ended, by kind and what it was, with what it last
//!   put out (#5751): a shell's or a monitor's last [`crate::ownwork::last::LINES`] lines and the
//!   file the rest is in, a subagent's last message, a workflow's last report, or "output gone".
//!   By count where the record names no piece (#5747);
//! - each wake-up that came due while purlis was down: "was due at <time>, missed";
//! - and the crons and the wake-ups still ahead, as the harness's to set again.
//!
//! **What the harness does itself as it resumes** (read from Claude Code 2.1.319): it sets again
//! each cron the conversation created that has not expired, and a wake-up still ahead; of a
//! `ScheduleWakeup` whose time passed, and of background work its last process left running, it
//! queues a notice of its own for the next prompt, which starts no turn. So the wake line, which
//! does start the turn, says the same of what ended and of what was missed, and names a cron or
//! a wake-up still ahead as the harness's to set again, never the chat's: told to set it again,
//! the chat would set it twice. A chat with nothing else to be told is not told of those alone,
//! since the harness wakes it for them.

use crate::dispatchrestart::CARRY_ON;
use crate::ownwork::OwnWork;
use crate::ownwork::last::Last;
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

/// How the wake-ups missed are introduced.
const MISSED: &str = "These came due while purlis was down, and were missed:";

/// What the chat is told to do of what it missed.
const DO_WHAT_WAS_MISSED: &str = "Do now what each was for, if it is still wanted.";

/// How the crons and wake-ups the harness sets again are introduced.
const SET_AGAIN: &str = "The harness sets these again itself as it resumes this conversation, \
    so do not set them again (check its list of scheduled prompts if unsure):";

/// What a quote of what a piece put out is called, so the chat reads it as data.
const AS_DATA: &str = "quoted as data";

/// The room kept in [`MOST_BYTES`] for the sentences that close each part.
const CLOSING: usize = 400;

/// **What the restart ended of a chat**, as its record says: what it had left running of its
/// own, each piece by name, whether a turn of its own was under way, when it is brought back,
/// and what each piece last put out.
#[derive(Clone, Copy)]
pub struct Ended<'a> {
    /// What its last turn's end said it left running, by count.
    pub own_work: Option<&'a OwnWork>,
    /// Each piece of its own work, by name.
    pub pieces: &'a [Piece],
    /// Whether a turn of its own was under way.
    pub mid_turn: bool,
    /// When it is brought back, in seconds since 1970.
    pub now: i64,
    /// What each piece last put out ([`crate::ownwork::last::of`]).
    pub last: &'a dyn Fn(&Piece) -> Last,
}

impl Default for Ended<'_> {
    fn default() -> Self {
        Self {
            own_work: None,
            pieces: &[],
            mid_turn: false,
            now: 0,
            last: &|_| Last::Gone,
        }
    }
}

impl std::fmt::Debug for Ended<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ended")
            .field("own_work", &self.own_work)
            .field("pieces", &self.pieces)
            .field("mid_turn", &self.mid_turn)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

/// **The wake line for a chat brought back on its conversation after a restart**, or none
/// where there is nothing to tell it.
///
/// `ended` is what the restart ended of the chat, and `carry_on` whether it is a task that still
/// owes its report. A chat with neither is told nothing. A task with nothing ended is told
/// [`CARRY_ON`] alone, word for word.
pub fn of(ended: &Ended<'_>, carry_on: bool) -> Option<String> {
    let parts = Parts::of(ended);
    if !carry_on && !ended.mid_turn && parts.ran.is_empty() && parts.missed.is_empty() {
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
    let room = |line: &str| MOST_BYTES.saturating_sub(line.len() + CLOSING);
    if !parts.ran.is_empty() {
        let also = if carry_on { " also" } else { "" };
        line.push_str(&format!(
            " The restart{also} ended what this chat had left running in the background: "
        ));
        let listed = fitted(&parts.ran, room(&line));
        line.push_str(&format!("{listed}. {NONE_OF_IT_RUNS}"));
    }
    if !parts.missed.is_empty() {
        line.push_str(&format!(" {MISSED} "));
        let listed = fitted(&parts.missed, room(&line));
        line.push_str(&format!("{listed}. {DO_WHAT_WAS_MISSED}"));
    }
    if !parts.again.is_empty() {
        line.push_str(&format!(" {SET_AGAIN} "));
        let listed = fitted(&parts.again, room(&line));
        line.push_str(&format!("{listed}."));
    }
    Some(line)
}

/// One thing the line lists: in full, and short, without what it put out.
struct Item {
    full: String,
    short: String,
}

/// What the line lists, in its three parts.
struct Parts {
    /// Background work the restart ended.
    ran: Vec<Item>,
    /// Wake-ups that came due while purlis was down.
    missed: Vec<Item>,
    /// Crons and wake-ups still ahead, which the harness sets again.
    again: Vec<Item>,
}

impl Parts {
    fn of(ended: &Ended<'_>) -> Self {
        let mut parts = Self {
            ran: Vec::new(),
            missed: Vec::new(),
            again: Vec::new(),
        };
        if ended.pieces.is_empty() {
            if let Some(work) = ended.own_work {
                if let Some(counted) = counted(work) {
                    parts.ran.push(Item {
                        short: counted.clone(),
                        full: counted,
                    });
                }
                if let Some(due) = work.wakes_at {
                    let item = wake_up(None, Some(due), ended.now);
                    if due <= ended.now {
                        parts.missed.push(item);
                    } else {
                        parts.again.push(item);
                    }
                }
            }
            return parts;
        }
        for piece in ended.pieces {
            match piece.kind {
                Kind::Cron => parts.again.push(Item {
                    short: named(piece),
                    full: named(piece),
                }),
                Kind::WakeUp => {
                    let item = wake_up(Some(piece), piece.due_at, ended.now);
                    if piece.due_at.is_some_and(|due| due <= ended.now) {
                        parts.missed.push(item);
                    } else {
                        parts.again.push(item);
                    }
                }
                Kind::Shell | Kind::Monitor | Kind::Subagent | Kind::Workflow => {
                    let short = named(piece);
                    let full = format!("{short} {}", put_out(piece, &(ended.last)(piece)));
                    parts.ran.push(Item { full, short });
                }
            }
        }
        parts
    }
}

/// `items` joined, each in full while it fits in `room` bytes, then short, then counted.
fn fitted(items: &[Item], room: usize) -> String {
    let mut said: Vec<&str> = Vec::new();
    let mut used = 0;
    for (at, item) in items.iter().enumerate() {
        let left = items.len() - at;
        // What a count of the rest would take, kept back for it.
        let more = if left > 1 { 16 } else { 0 };
        if used + item.full.len() + more <= room {
            used += item.full.len() + 2;
            said.push(&item.full);
        } else if used + item.short.len() + more <= room {
            used += item.short.len() + 2;
            said.push(&item.short);
        } else if said.is_empty() {
            return format!("{left} of them, too long to name here");
        } else {
            return format!("{}; and {left} more", said.join("; "));
        }
    }
    said.join("; ")
}

/// One piece, by its kind and what it was: `a background shell "npm test"`.
fn named(piece: &Piece) -> String {
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

/// What `piece` last put out, `last`, as the line says it, in brackets.
fn put_out(piece: &Piece, last: &Last) -> String {
    match last {
        Last::Lines { lines, path } if lines.is_empty() => {
            format!("(it had put out nothing yet; its output file is {path})")
        }
        Last::Lines { lines, path } => {
            let what = if piece.kind == Kind::Workflow {
                "the last it reported".to_owned()
            } else if lines.len() == 1 {
                "its last line".to_owned()
            } else {
                format!("its last {} lines", lines.len())
            };
            format!(
                "({what}, {AS_DATA}: \"{}\"; the whole of its output is in {path})",
                lines.join(" ⏎ ")
            )
        }
        Last::Message(message) => format!("(its last message, {AS_DATA}: \"{message}\")"),
        Last::Gone | Last::Nothing => "(output gone)".to_owned(),
    }
}

/// A wake-up, `piece` where the record names one, due at `due`: "was due at <time>, missed"
/// where that is at or before `now`, else "due at <time>".
fn wake_up(piece: Option<&Piece>, due: Option<i64>, now: i64) -> Item {
    let mut said = piece.map_or_else(|| "a wake-up".to_owned(), named);
    if let Some(due) = due {
        let at = at(due);
        if due <= now {
            said.push_str(&format!(", was due at {at}, missed"));
        } else {
            said.push_str(&format!(", due at {at}"));
        }
    }
    Item {
        short: said.clone(),
        full: said,
    }
}

/// `secs` since 1970 as the line says a time: `2026-11-08 14:05 UTC`.
fn at(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).map_or_else(
        || "an unknown time".to_owned(),
        |at| at.format("%Y-%m-%d %H:%M UTC").to_string(),
    )
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

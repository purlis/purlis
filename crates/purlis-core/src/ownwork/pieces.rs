// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each piece of a chat's own work, by name** (#5750, spec #5743): what the wake line lists
//! ([`crate::wakeline`]) where [`super::OwnWork`] holds only counts.
//!
//! **Two sources, one record** (ADR 0068 as amended 2026-11-08):
//!
//! - **A piece is recorded as its tool starts it** ([`started`]), from the tool's hook: a
//!   background shell, a monitor, a background subagent, a workflow, a cron or a wake-up. So
//!   work started in a turn a kill cut mid-way is on the record though the turn never ended.
//! - **The turn's `Stop` is the authority** ([`at_stop`]): what it says is in flight is the
//!   record, and a piece it does not name has ended. What the start knew and the `Stop` does not
//!   say (where the output is, when a wake-up is due) is kept.
//!
//! A harness's adapter reads its own words into pieces (`crate::harness::claude::ownwork`).
//! Every piece is held to a shape ([`Piece::sound`]) wherever it comes from, the chat's own line
//! or the record on disk: an id of [`MOST_ID_BYTES`] of letters, digits, `-` and `_`, words cut
//! to [`MOST_TEXT_BYTES`] with no control character, and at most [`MOST_PIECES`] of them.

/// The most pieces a chat's record holds: the latest started.
pub const MOST_PIECES: usize = 16;

/// The most bytes of a piece's words (its description, command, schedule or prompt).
pub const MOST_TEXT_BYTES: usize = 300;

/// The most bytes of a piece's id.
pub const MOST_ID_BYTES: usize = 64;

/// The most bytes of the path to a piece's output.
pub const MOST_PATH_BYTES: usize = 1024;

/// **What kind of own work a piece is.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A command left running in the background.
    Shell,
    /// A monitor watching for something to wake the chat.
    Monitor,
    /// A helper agent working in the background.
    Subagent,
    /// A workflow of helpers.
    Workflow,
    /// A prompt scheduled to run again and again.
    Cron,
    /// A prompt scheduled to run once.
    WakeUp,
}

/// **One piece of a chat's own work.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Piece {
    pub kind: Kind,
    /// The harness's id for it: a task's id, a cron's id, or the call that started it.
    pub id: String,
    /// Its description, or the command it runs.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub what: String,
    /// Where the harness writes its output, as the harness named it: read only at a reopen, and
    /// only where it is that chat's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// A cron's five-field schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// The prompt a cron or a wake-up runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// When a wake-up is due, in seconds since 1970 by the clock of the machine the chat runs
    /// on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<i64>,
}

impl Piece {
    /// A piece of `kind` with the id `id`, doing `what`: none where `id` is not one.
    pub fn new(kind: Kind, id: &str, what: &str) -> Option<Self> {
        Self {
            kind,
            id: id.to_owned(),
            what: what.to_owned(),
            output: None,
            schedule: None,
            prompt: None,
            due_at: None,
        }
        .sound()
    }

    /// **It, held to its shape**, or none where its id is not one: its words cut to
    /// [`MOST_TEXT_BYTES`] with every control character a space, and an output path kept only
    /// where it is absolute and of [`MOST_PATH_BYTES`] at most.
    pub fn sound(self) -> Option<Self> {
        if !an_id(&self.id) {
            return None;
        }
        let words = |text: Option<String>| {
            text.map(|text| cut(&text, MOST_TEXT_BYTES))
                .filter(|text| !text.is_empty())
        };
        Some(Self {
            what: cut(&self.what, MOST_TEXT_BYTES),
            output: self.output.filter(|path| {
                path.len() <= MOST_PATH_BYTES
                    && std::path::Path::new(path).is_absolute()
                    && !path.chars().any(char::is_control)
            }),
            schedule: words(self.schedule),
            prompt: words(self.prompt),
            ..self
        })
    }
}

/// Whether `id` is a piece's id: 1 to [`MOST_ID_BYTES`] letters, digits, `-` and `_`.
pub fn an_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MOST_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `text` on one line, trimmed, every control character a space, cut on a character to `most`
/// bytes.
pub fn cut(text: &str, most: usize) -> String {
    let line: String = text
        .trim()
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    let mut end = line.len().min(most);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].trim_end().to_owned()
}

/// **The record once `piece` has started**: it in place of one of the same id, else added at
/// the end, the earliest dropped past [`MOST_PIECES`].
pub fn started(pieces: &mut Vec<Piece>, piece: Piece) {
    if let Some(at) = pieces.iter().position(|one| one.id == piece.id) {
        pieces[at] = piece;
        return;
    }
    pieces.push(piece);
    if pieces.len() > MOST_PIECES {
        let over = pieces.len() - MOST_PIECES;
        pieces.drain(..over);
    }
}

/// **The record once a turn's end said `said` is in flight**: exactly what it said, the authority
/// on what still runs, with what `recorded` knew of each piece and it did not say.
pub fn at_stop(recorded: &[Piece], said: Vec<Piece>) -> Vec<Piece> {
    let mut now: Vec<Piece> = Vec::new();
    for piece in said {
        let piece = match recorded.iter().find(|one| one.id == piece.id) {
            Some(known) => Piece {
                what: if piece.what.is_empty() {
                    known.what.clone()
                } else {
                    piece.what
                },
                output: piece.output.or_else(|| known.output.clone()),
                schedule: piece.schedule.or_else(|| known.schedule.clone()),
                prompt: piece.prompt.or_else(|| known.prompt.clone()),
                due_at: known.due_at.or(piece.due_at),
                ..piece
            },
            None => piece,
        };
        started(&mut now, piece);
    }
    now
}

#[cfg(test)]
#[path = "pieces_tests.rs"]
mod tests;

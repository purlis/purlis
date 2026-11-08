// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The turns of the chats a runner's host runs** (#5427; session protocol 1.78, ADR 0068 as
//! amended 2026-11-08): what its hook channel does with them.
//!
//! - **Recorded where they ran.** Each line of a chat's own harness the channel admits is the
//!   boundary of a turn the core's [`Recorder`] records, as the window records a chat of its own:
//!   `refs/purlis/turns/<chat>/<n>/…` in the runner's folder, under the chat's id and folder as
//!   this host composed it ([`crate::hostedchats::place`]). The desktop fetches them with the
//!   runner's code ([`crate::codelink::give_with_turns`]).
//! - **Undone where they ran** ([`HostedTurns::undo`]): the window's **Undo a turn** of such a
//!   chat comes down the link, and is done here under this host's own idle check, with the
//!   window's refusals: a chat working in the folder that is not idle, or still recording its
//!   last turn, and a file changed since the turn ended. A prompt heard meanwhile waits until the
//!   undo is done, as it waits on the window's board for one of the window's own.
//! - **Gone with the chat** ([`HostedTurns::removed`]).

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use purlis_host::TurnToUndo;
use purlis_host::protocol::session::{self, Answer, Refusal, UndoWaitsOn};

use crate::files::turns::{Place, Recorder, place_of};
use crate::files::{Branch, Reader, TurnKey};
use crate::hookwire::relay::Heard;
use crate::state::{Board, State};

/// The turns of one held project's chats, in the host that runs them.
pub struct HostedTurns {
    /// Where the project is on this machine.
    root: PathBuf,
    recorder: Recorder,
    /// The bounded reader an undo reads and writes the folder through.
    reader: Reader,
    /// Held through an undo: a boundary heard meanwhile waits for it.
    undoing: Mutex<()>,
}

impl HostedTurns {
    /// The turns of the project at `root`, undone through `reader`.
    pub fn new(root: PathBuf, reader: Reader) -> HostedTurns {
        HostedTurns {
            root,
            recorder: Recorder::default(),
            reader,
            undoing: Mutex::new(()),
        }
    }

    /// Chat `number`'s line `heard` was admitted: a boundary of its turn where it is its own
    /// harness's prompt or stop, and nothing otherwise, nor for a chat this host did not compose.
    pub fn heard(&self, number: u32, heard: &Heard) {
        let Heard::Report(report) = heard else {
            return;
        };
        let Some((id, cwd)) = crate::hostedchats::place(&self.root, number) else {
            return;
        };
        let _undone_first = self.undoing.lock().unwrap_or_else(PoisonError::into_inner);
        self.recorder.heard(&self.root, &id, &cwd, report);
    }

    /// Chat `number`'s program ended: a turn still open ended with it.
    pub fn program_ended(&self, number: u32) {
        if let Some((id, cwd)) = crate::hostedchats::place(&self.root, number) {
            self.recorder.program_ended(&self.root, &id, &cwd);
        }
    }

    /// Chat `number` is removed: its turns' refs go, and where it worked is forgotten.
    pub fn removed(&self, number: u32) {
        if let Some((id, cwd)) = crate::hostedchats::place(&self.root, number) {
            self.recorder.removed(&self.root, &id, &cwd);
        }
        crate::hostedchats::forget_place(&self.root, number);
    }

    /// Whether a boundary of chat `id`'s turn is still being recorded.
    pub fn recording(&self, id: &str) -> bool {
        self.recorder.recording(id)
    }

    /// **Undo `turn`**, where `board` is how this host's chats stand, read while it is held: every
    /// file the turn changed put back as its start had it, and the turn marked undone. Answered
    /// [`Answer::UndoWaits`], with nothing written, while a chat working in the folder is not
    /// idle or is still recording its last turn; refused [`session::NOT_ALLOWED`] for a turn the
    /// core will not undo, in its words.
    pub fn undo(&self, board: &Board, turn: &TurnToUndo) -> Result<Answer, Refusal> {
        let place = Place {
            ws: turn.workspace.clone(),
            repo: turn.repo.clone(),
            piece: turn.piece.clone(),
        };
        let branch = match turn.piece.as_deref() {
            Some(piece) => Branch::piece(&turn.workspace, &turn.repo, piece),
            None => Branch::repo(&turn.workspace, &turn.repo),
        };
        let _held = self.undoing.lock().unwrap_or_else(PoisonError::into_inner);
        let chats: Vec<UndoWaitsOn> = crate::hostedchats::placed_in(&self.root)
            .into_iter()
            .filter(|(_, _, folder)| place_of(&self.root, folder).as_ref() == Some(&place))
            .filter_map(|(number, id, _)| {
                waits_on(board, number, self.recorder.recording(&id))
                    .map(|state| UndoWaitsOn { number, state })
            })
            .collect();
        if !chats.is_empty() {
            return Ok(Answer::UndoWaits { chats });
        }
        let key = TurnKey {
            chat: turn.chat.clone(),
            number: turn.number,
        };
        let undone =
            crate::files::undo_turn(&self.reader, &self.root, branch, &key).map_err(|refused| {
                Refusal {
                    code: session::NOT_ALLOWED.into(),
                    why: refused.to_string(),
                }
            })?;
        let edit = serde_json::to_value(&undone.edit).map_err(|why| Refusal {
            code: session::NOT_STARTED.into(),
            why: format!("the files are back, but the record of it could not be written: {why}"),
        })?;
        Ok(Answer::TurnUndone {
            edit,
            unmarked: undone.unmarked,
        })
    }
}

/// How a sentence says what an undo waits on in chat `number`, as the window says it of one of
/// its own; `None` for a chat that is idle and has nothing left to record.
fn waits_on(board: &Board, number: u32, recording: bool) -> Option<String> {
    let mid_turn = match board.state(number) {
        State::Running | State::Unknown => true,
        State::Waiting => {
            board.asking(number)
                || board.its_prompt(number).is_some()
                || board.own_work(number).is_some()
        }
        State::Done | State::Failed => false,
    };
    let state = mid_turn.then(|| match board.state(number) {
        State::Running => "is running",
        State::Waiting if board.asking(number) => "is asking you something",
        State::Waiting if board.its_prompt(number).is_some() => "is stopped on a prompt",
        State::Waiting => "is waiting on work of its own",
        _ => "has not been heard from yet",
    });
    state
        .map(str::to_owned)
        .or_else(|| recording.then(|| "is still recording its last turn".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_still_recording_its_last_turn_holds_an_undo_back_and_says_so() {
        let mut board = Board::new();
        board.opened(7, None, None);
        board.exited(7, Some(0));

        assert_eq!(waits_on(&board, 7, false), None);
        assert_eq!(
            waits_on(&board, 7, true).as_deref(),
            Some("is still recording its last turn")
        );
    }

    #[test]
    fn a_chat_never_heard_from_holds_an_undo_back() {
        let board = Board::new();
        assert_eq!(
            waits_on(&board, 9, false).as_deref(),
            Some("has not been heard from yet")
        );
    }
}

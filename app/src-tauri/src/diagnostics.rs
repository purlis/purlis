// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What Copy diagnostics asks the core** (#4774): the facts about this machine and this app's
//! chats that the window cannot read itself. The window puts the report together, beside the
//! version it already asked for and each harness's installed version, and puts it on the
//! clipboard; this only answers.
//!
//! Nothing here is a secret or a path: the system's name and release, and two counts.

use purlis_core::state::State;

use crate::planes::Planes;

/// What the report says about where purlis runs and how many chats it holds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DiagnosticsHere {
    /// The system, as [`purlis_core::adopt::system`] says it: `Darwin 27.2.0, aarch64`.
    pub os: String,
    /// The chats open in every project this app holds, a shell tab not among them.
    pub open: u32,
    /// The open chats whose run has a program (the glossary's **hot chat**): any not done or
    /// failed.
    pub hot: u32,
}

/// The open and hot chats among chats in `states`, one state for each open chat.
fn counted(states: impl IntoIterator<Item = State>) -> (u32, u32) {
    states.into_iter().fold((0, 0), |(open, hot), state| {
        let has_a_program = !matches!(state, State::Done | State::Failed);
        (open + 1, hot + u32::from(has_a_program))
    })
}

/// Copy diagnostics' answer: the system, and the open and hot chats of every project held.
/// Read from memory only, so it costs nothing on the window's thread.
#[tauri::command]
#[specta::specta]
pub fn diagnostics_here(planes: tauri::State<'_, Planes>) -> DiagnosticsHere {
    let states = planes.every_held().into_iter().flat_map(|held| {
        held.chats()
            .open_now()
            .into_iter()
            .filter(|open| open.harness.is_some())
            .map(|open| held.board().glance(open.session).state)
            .collect::<Vec<_>>()
    });
    let (open, hot) = counted(states);
    DiagnosticsHere {
        os: purlis_core::adopt::system(),
        open,
        hot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_whose_program_ended_is_open_and_not_hot() {
        let states = [
            State::Unknown,
            State::Running,
            State::Waiting,
            State::Done,
            State::Failed,
        ];
        assert_eq!(counted(states), (5, 3));
    }

    #[test]
    fn no_chats_count_nothing() {
        assert_eq!(counted([]), (0, 0));
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat whose program ended with its session host** (#2007, ADR 0068 §3).
//!
//! When the session host the window's chats run in ends unexpectedly, every chat it ran ends
//! with it: its terminal closes, and its program with it. The window hears the link end, and
//! each chat ends as a program that ended does, but not as one that ended by itself: nothing
//! is told a task failed, and a task chat reports when it is resumed, as after a quit.
//!
//! **What its run is left in** is ADR 0076's table for a host crash, read from what the board
//! said of the chat just before ([`after_the_host`]): a chat in the middle of a turn, or
//! stopped on an ask, has **failed** (its turn is lost); one idle at a turn's end, with a
//! conversation to resume, is **hibernated** (nothing is lost). The window keeps that for the
//! chat ([`Lost`]) and says it on the chat's pane, with **Resume**: the chat's restart on its
//! conversation (`ask_chat_restart`), one click. In memory only: a relaunch starts with none,
//! and a closed chat's goes with it.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::state::State;
use purlis_core::state::run::{Facts, Level, Reason, Run, RunState};

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat's program ended with its session host.
pub const LOST: &str = "chat-lost-with-the-host";

/// A chat whose program ended with its session host, as the Notice on its pane shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LostChat {
    pub plane: PlaneId,
    pub session: u32,
    /// Its run is hibernated: it was idle at a turn's end, and nothing was lost. Otherwise it
    /// failed, and the turn it was in is lost.
    pub hibernated: bool,
    /// It has a conversation to resume, so Resume puts it back.
    pub resumable: bool,
}

/// Told each chat whose program ended with its host. The event carries its project.
pub type Teller = std::sync::Arc<dyn Fn(LostChat) + Send + Sync + 'static>;

/// What a host crash left a chat's run in, from what the board said of the chat as it went:
/// `Some(true)` hibernated, `Some(false)` failed, and `None` for a chat whose program had
/// already ended. `resumable` is whether it has a conversation to resume, which is what lets
/// an idle chat be hibernated (ADR 0076 §5).
pub fn after_the_host(state: State, asking: bool, resumable: bool) -> Option<bool> {
    let run = match state {
        State::Done | State::Failed => return None,
        State::Unknown => RunState::Starting,
        State::Running => RunState::Working,
        State::Waiting if asking => RunState::InputRequired(Reason::Asked),
        State::Waiting => RunState::InputRequired(Reason::TurnEnded),
    };
    let facts = Facts {
        level: Level::Two,
        reports_start: true,
        resumes_natively: resumable,
    };
    Some(Run::after_a_host_crash(run, facts) == RunState::Hibernated)
}

/// The chats of a project whose program ended with their session host, by session, until the
/// person resumes, dismisses or closes each.
#[derive(Debug, Default)]
pub struct Lost(Mutex<HashMap<u32, Kept>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Kept {
    hibernated: bool,
    resumable: bool,
}

impl Lost {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Kept>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `session`'s program ended with its host, leaving its run hibernated or failed.
    pub fn note(&self, session: u32, hibernated: bool, resumable: bool) {
        self.held().insert(
            session,
            Kept {
                hibernated,
                resumable,
            },
        );
    }

    /// What chat `session` was left in, as `(hibernated, resumable)`, until it is put away.
    pub fn of(&self, session: u32) -> Option<(bool, bool)> {
        self.held()
            .get(&session)
            .map(|kept| (kept.hibernated, kept.resumable))
    }

    /// The person resumed or dismissed it, or the chat closed.
    pub fn forget(&self, session: u32) {
        self.held().remove(&session);
    }
}

/// Chat `session`, where its program ended with its session host and the person has not put
/// it away.
#[tauri::command]
#[specta::specta]
pub fn chat_lost_with_the_host(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<LostChat>, String> {
    let held = planes.held(&plane)?;
    Ok(held
        .lost_with_the_host()
        .of(session)
        .map(|(hibernated, resumable)| LostChat {
            plane: plane.clone(),
            session,
            hibernated,
            resumable,
        }))
}

/// **Dismiss** on the Notice, and what Resume does once it has asked for the chat's restart:
/// puts it away, and changes nothing else.
#[tauri::command]
#[specta::specta]
pub fn put_away_lost_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<(), String> {
    planes.held(&plane)?.lost_with_the_host().forget(session);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_mid_turn_or_on_an_ask_fails_with_its_host_and_an_idle_one_hibernates() {
        assert_eq!(after_the_host(State::Running, false, true), Some(false));
        assert_eq!(after_the_host(State::Unknown, false, true), Some(false));
        assert_eq!(
            after_the_host(State::Waiting, true, true),
            Some(false),
            "the ask is lost"
        );
        assert_eq!(after_the_host(State::Waiting, false, true), Some(true));
    }

    #[test]
    fn an_idle_chat_with_no_conversation_to_resume_fails_with_its_host() {
        assert_eq!(after_the_host(State::Waiting, false, false), Some(false));
    }

    #[test]
    fn a_chat_whose_program_had_already_ended_is_not_lost_with_its_host() {
        assert_eq!(after_the_host(State::Done, false, true), None);
        assert_eq!(after_the_host(State::Failed, false, true), None);
    }

    #[test]
    fn a_lost_chat_is_kept_until_put_away() {
        let lost = Lost::default();
        lost.note(3, true, true);
        lost.note(4, false, false);

        lost.forget(3);

        assert_eq!(lost.of(3), None);
        assert_eq!(lost.of(4), Some((false, false)));
    }
}

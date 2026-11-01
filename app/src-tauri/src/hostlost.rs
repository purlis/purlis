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
//! stopped on an ask, has **failed** (its turn is lost); one idle at a turn's end, on a harness
//! that picks a conversation back up by its id (`resumes_by_id`, ADR 0076 §5), is
//! **hibernated** (nothing is lost). The window keeps that for the chat ([`Lost`]) and says it
//! on the chat's pane, with **Resume** where it has a conversation to resume: the chat's restart
//! on its conversation (`ask_chat_restart`), one click. In memory only: a relaunch starts with
//! none, and a closed chat's goes with it.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::state::State;
use purlis_core::state::run::{Facts, Level, Reason, Run, RunState};

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat's program ended with its session host.
pub const LOST: &str = "chat-lost-with-the-host";

/// What a host crash left a chat's run in (ADR 0076 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum LeftIn {
    /// It was in the middle of a turn, or stopped on an ask: that turn is lost.
    Failed,
    /// It was idle at a turn's end on a harness that resumes by id: nothing is lost.
    Hibernated,
}

/// A chat whose program ended with its session host, as the Notice on its pane shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LostChat {
    pub plane: PlaneId,
    pub session: u32,
    /// What its run was left in.
    pub left_in: LeftIn,
    /// It has a conversation to resume, so Resume puts it back.
    pub resumable: bool,
    /// Its host was a pool's runner, gone for good, and its conversation went with that runner's
    /// home (#4350).
    pub with_the_runner: bool,
}

/// Told each chat whose program ended with its host. The event carries its project.
pub type Teller = std::sync::Arc<dyn Fn(LostChat) + Send + Sync + 'static>;

/// What a host crash left a chat's run in, from what the board said of the chat as it went;
/// `None` for a chat whose program had already ended. An idle run is hibernated only where it
/// can be picked up again: its harness resumes by id (`resumes_by_id`), and it has a
/// conversation to resume (`has_a_conversation`).
pub fn after_the_host(
    state: State,
    asking: bool,
    resumes_by_id: bool,
    has_a_conversation: bool,
) -> Option<LeftIn> {
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
        resumes_natively: resumes_by_id && has_a_conversation,
    };
    Some(match Run::after_a_host_crash(run, facts) {
        RunState::Hibernated => LeftIn::Hibernated,
        _ => LeftIn::Failed,
    })
}

/// How one chat was left by its host's end, kept until the person answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LostRun {
    pub left_in: LeftIn,
    pub resumable: bool,
    pub with_the_runner: bool,
}

/// The chats of a project whose program ended with their session host, by session, until the
/// person resumes, dismisses or closes each.
#[derive(Debug, Default)]
pub struct Lost(Mutex<HashMap<u32, LostRun>>);

impl Lost {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<u32, LostRun>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `session`'s program ended with its host, leaving its run as `lost` says.
    pub fn note(&self, session: u32, lost: LostRun) {
        self.held().insert(session, lost);
    }

    /// How chat `session` was left, until it is put away.
    pub fn of(&self, session: u32) -> Option<LostRun> {
        self.held().get(&session).copied()
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
    Ok(held.lost_with_the_host().of(session).map(|lost| LostChat {
        plane: plane.clone(),
        session,
        left_in: lost.left_in,
        resumable: lost.resumable,
        with_the_runner: lost.with_the_runner,
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
        assert_eq!(
            after_the_host(State::Running, false, true, true),
            Some(LeftIn::Failed)
        );
        assert_eq!(
            after_the_host(State::Unknown, false, true, true),
            Some(LeftIn::Failed)
        );
        assert_eq!(
            after_the_host(State::Waiting, true, true, true),
            Some(LeftIn::Failed),
            "the ask is lost"
        );
        assert_eq!(
            after_the_host(State::Waiting, false, true, true),
            Some(LeftIn::Hibernated)
        );
    }

    #[test]
    fn an_idle_chat_on_a_harness_that_cannot_resume_by_id_fails_with_its_host() {
        assert_eq!(
            after_the_host(State::Waiting, false, false, true),
            Some(LeftIn::Failed)
        );
    }

    #[test]
    fn an_idle_chat_with_no_conversation_to_resume_fails_with_its_host() {
        assert_eq!(
            after_the_host(State::Waiting, false, true, false),
            Some(LeftIn::Failed),
            "nothing to pick back up"
        );
    }

    #[test]
    fn a_chat_whose_program_had_already_ended_is_not_lost_with_its_host() {
        assert_eq!(after_the_host(State::Done, false, true, true), None);
        assert_eq!(after_the_host(State::Failed, false, true, true), None);
    }

    #[test]
    fn a_lost_chat_is_kept_until_put_away() {
        let lost = Lost::default();
        let hibernated = LostRun {
            left_in: LeftIn::Hibernated,
            resumable: true,
            with_the_runner: false,
        };
        let failed = LostRun {
            left_in: LeftIn::Failed,
            resumable: false,
            with_the_runner: false,
        };
        lost.note(3, hibernated);
        lost.note(4, failed);

        lost.forget(3);

        assert_eq!(lost.of(3), None);
        assert_eq!(lost.of(4), Some(failed));
    }
}

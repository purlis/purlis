// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat the person hibernated by hand, and the Resume that wakes it** (SC-4a, #7630, spec
//! #1391, ADR 0076 as amended 2026-11-25: the run's `hibernated | operator` row).
//!
//! Hibernating ends a chat's program at a turn boundary and drops its engine in the session host
//! ([`purlis_session_protocol::session::Command::Hibernate`]); its conversation is the harness's,
//! resumed by id. **The decision and the act are the app's.** Nothing here reads a harness's
//! output: whether a chat may be hibernated is read from the board, the harness's declared
//! abilities and the app's own records ([`Facts`]).
//!
//! This is the tracer: one chat, by hand, in memory. **Nothing is automatic, and the flag does
//! not survive a restart** (SC-4e, #7634). What a ruling on the spec could change sits in two
//! places: [`refusal`], the one pure answer to *may it be hibernated*, and [`Hibernated`], the
//! one place that says which chats are.
//!
//! **The five-word board `State` gains no value.** A hibernated chat is a per-chat flag read
//! where the board is read, as [`crate::hostlost::Lost`] is.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::state::State;

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat is hibernated or woken.
pub const CHANGED: &str = "chat-hibernation-changed";

/// Why a Hibernate pressed while one is already under way does nothing: the first acts.
pub const ALREADY_HIBERNATING: &str =
    "Not hibernated again: purlis is already hibernating this chat.";

/// Why a Hibernate on a hibernated chat does nothing.
pub const ALREADY_HIBERNATED: &str =
    "Not hibernated: this chat is already hibernated. Resume it to go on.";

/// Why a Resume pressed while the chat is still being hibernated starts nothing: a second
/// program would start beside the one ending.
pub const STILL_HIBERNATING: &str =
    "Not resumed: purlis is still hibernating this chat. Resume it once it is hibernated.";

/// Why a hibernation did not take: the chat's program ended another way as it ended.
pub const ENDED_ANOTHER_WAY: &str =
    "Not hibernated: this chat ended another way while purlis hibernated it.";

/// What is read of one chat to decide whether it may be hibernated by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    /// What the board says of it.
    pub state: Option<State>,
    /// It is stopped on an ask of the person's.
    pub asking: bool,
    /// Its program is held still (a pause).
    pub paused: bool,
    /// Its harness picks a conversation back up by its id, and it has one.
    pub resumes_by_id: bool,
    /// A remote chat purlis only watches (ADR 0076 §7).
    pub observed: bool,
    /// It has something running of its own, or its turn waits on it.
    pub own_work: bool,
    /// It dispatched a task that is open.
    pub has_tasks: bool,
    /// It is itself a dispatched task.
    pub is_a_task: bool,
}

/// Why a chat cannot be hibernated by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Its program is not at a turn boundary: it is starting or in a turn.
    Working,
    /// It is stopped on an ask: the ask would be lost with its program.
    Asking,
    /// It is paused.
    Paused,
    /// Its program has ended.
    Ended,
    /// Its harness cannot pick its conversation up again by itself.
    CannotResume,
    /// It is an observed chat.
    Observed,
    /// It has something running of its own.
    OwnWork,
    /// It dispatched a task that is still open.
    HasTasks,
    /// It is a dispatched task.
    IsATask,
}

/// **Whether a chat may be hibernated by hand, and if not, why.** The tracer is conservative:
/// own work, a dispatched task of its own, and being a task are refused until the act holds its
/// latch and a tell wakes a task (SC-4d, #7633; SC-4j, #7639). Spec rulings 2 and 4 may loosen
/// these, here and nowhere else.
pub fn refusal(facts: &Facts) -> Option<Refusal> {
    if facts.observed {
        return Some(Refusal::Observed);
    }
    match facts.state {
        Some(State::Done | State::Failed) => return Some(Refusal::Ended),
        Some(State::Waiting) => {}
        _ => return Some(Refusal::Working),
    }
    if facts.paused {
        return Some(Refusal::Paused);
    }
    if facts.asking {
        return Some(Refusal::Asking);
    }
    if !facts.resumes_by_id {
        return Some(Refusal::CannotResume);
    }
    if facts.own_work {
        return Some(Refusal::OwnWork);
    }
    if facts.has_tasks {
        return Some(Refusal::HasTasks);
    }
    if facts.is_a_task {
        return Some(Refusal::IsATask);
    }
    None
}

impl Refusal {
    /// What happened and what to do, in the window's words (`docs/ui-copy.md`).
    pub fn says(self) -> &'static str {
        match self {
            Self::Working => {
                "Not hibernated: this chat is working. Hibernate it once its turn ends."
            }
            Self::Asking => "Not hibernated: this chat is asking you something. Answer it first.",
            Self::Paused => "Not hibernated: this chat is paused. Let it go on first.",
            Self::Ended => "Not hibernated: this chat has ended.",
            Self::CannotResume => {
                "Not hibernated: this chat's harness cannot resume a conversation by itself."
            }
            Self::Observed => "Not hibernated: purlis only watches this chat, it does not run it.",
            Self::OwnWork => {
                "Not hibernated: this chat has work running of its own. Wait for it to finish."
            }
            Self::HasTasks => {
                "Not hibernated: this chat dispatched a task that is still open. Wait for its report."
            }
            Self::IsATask => "Not hibernated: this chat is a dispatched task.",
        }
    }
}

/// How a chat that is hibernated was put to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum By {
    /// The person did, from its row.
    Operator,
}

/// One chat's hibernation, from the person's press until it is woken or ended another way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Held {
    by: By,
    /// The host has answered that its program ended as a hibernation. Until then it is in
    /// flight: neither Resume nor a second Hibernate is taken.
    settled: bool,
    /// Its program's end was heard while it was in flight, and read as the hibernation.
    ended: bool,
}

/// **The chats of a project that are hibernated, or being hibernated, by session**, in memory
/// (SC-4e makes it durable). The one answer to *is it hibernated*: the program-end path, the
/// pane, the row and the wake read it here.
#[derive(Debug, Default)]
pub struct Hibernated(Mutex<HashMap<u32, Held>>);

impl Hibernated {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Held>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The host is about to end chat `session`'s program as a hibernation, by `by`: written
    /// first, in flight, so the exit that follows is read as one. False, with nothing changed,
    /// where it is already hibernated or being hibernated: the first press is the one that acts.
    pub fn intend(&self, session: u32, by: By) -> bool {
        let mut held = self.held();
        if held.contains_key(&session) {
            return false;
        }
        held.insert(
            session,
            Held {
                by,
                settled: false,
                ended: false,
            },
        );
        true
    }

    /// The host answered that chat `session`'s program ended as a hibernation: it is
    /// hibernated from now on. False where nothing was in flight for it any more: another end
    /// won, and the chat is not hibernated.
    pub fn settle(&self, session: u32) -> bool {
        self.held().get_mut(&session).is_some_and(|held| {
            held.settled = true;
            true
        })
    }

    /// The act on chat `session` failed. Where its program's end was already heard as the
    /// hibernation it is hibernated all the same, and true; otherwise it is not, and false.
    pub fn failed(&self, session: u32) -> bool {
        let mut held = self.held();
        match held.get_mut(&session) {
            Some(one) if one.ended => {
                one.settled = true;
                true
            }
            Some(_) => {
                held.remove(&session);
                false
            }
            None => false,
        }
    }

    /// Chat `session`'s program ended. True where that end is a hibernation, in flight or
    /// already answered: the end the program-end path reads as no end of the chat.
    pub fn ends_as_one(&self, session: u32) -> bool {
        self.held().get_mut(&session).is_some_and(|held| {
            held.ended = true;
            true
        })
    }

    /// How chat `session` is hibernated, once the host has answered.
    pub fn of(&self, session: u32) -> Option<By> {
        self.held()
            .get(&session)
            .filter(|held| held.settled)
            .map(|held| held.by)
    }

    /// Whether chat `session` is being hibernated, and the host has not answered yet.
    pub fn in_flight(&self, session: u32) -> bool {
        self.held().get(&session).is_some_and(|held| !held.settled)
    }

    /// The chats that are hibernated, in the order of their numbers.
    pub fn now(&self) -> Vec<u32> {
        let mut now: Vec<u32> = self
            .held()
            .iter()
            .filter(|(_, held)| held.settled)
            .map(|(session, _)| *session)
            .collect();
        now.sort_unstable();
        now
    }

    /// It is awake, was ended another way, or is closed.
    pub fn forget(&self, session: u32) {
        self.held().remove(&session);
    }
}

/// What is read of chat `session` to decide whether it may be hibernated: the board, its
/// harness, its pauses and the app's records of what it dispatched and who dispatched it.
/// Nothing a harness printed. Observed chats are not yet a thing the app keeps for a local
/// chat, so that one is read as false until a source for it exists.
///
/// **Own work counts while it is out, lapsed or not** (spec #1391): a wait on it that passed its
/// limit leaves the work running, and its wake would find the program gone
/// ([`purlis_core::state::Chat::wakes_on_its_own_work`]).
pub fn facts_of(held: &crate::planes::Held, session: u32) -> Facts {
    let glance = held.board().glance(session);
    let resumes = held
        .chats()
        .harness(session)
        .is_some_and(purlis_core::harness::Harness::resumes_by_id);
    let task = held
        .chats()
        .recorded_chat(session)
        .and_then(|chat| chat.from)
        .is_some_and(|from| from.mode == purlis_core::reopen::Mode::Task);
    Facts {
        state: Some(glance.state),
        asking: glance.asking,
        paused: held.pauses().holds(session),
        resumes_by_id: resumes && held.board().conversation(session).is_some(),
        observed: false,
        own_work: glance.own_work || glance.wakes_on_its_own_work,
        has_tasks: !held.chats().tasks_of(session).is_empty(),
        is_a_task: task,
    }
}

/// **Whether chat `session`'s row offers Hibernate** (#7630): its harness picks a conversation
/// up by its id, and its session host can hibernate it (protocol 1.96; never the in-process
/// host). Whether it may be hibernated now is [`refusal`]'s answer, said when it is pressed.
pub fn offered(held: &crate::planes::Held, session: u32) -> bool {
    held.chats()
        .harness(session)
        .is_some_and(purlis_core::harness::Harness::resumes_by_id)
        && held.chats().sessions().can_hibernate(session)
}

/// The chats of `plane` that are hibernated: what the window reads its rows by as it opens.
#[tauri::command]
#[specta::specta]
pub fn hibernated_chats(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<u32>, String> {
    Ok(planes.held(&plane)?.hibernated().now())
}

/// Chat `session` as the window reads its hibernation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatHibernation {
    pub by: By,
}

/// Chat `session`, where it is hibernated.
#[tauri::command]
#[specta::specta]
pub fn chat_hibernated(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<ChatHibernation>, String> {
    let held = planes.held(&plane)?;
    Ok(held
        .hibernated()
        .of(session)
        .map(|by| ChatHibernation { by }))
}

/// Hibernate on a chat's row (SC-4a, #7630): the person's act. Refused, in a sentence that
/// says why and what to do, where the chat may not be hibernated; otherwise its program ends and
/// the window is told. The window's alone (`WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn hibernate_chat(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<(), String> {
    use tauri::Emitter;
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || held.hibernate_chat(session))
        .await
        .map_err(|err| format!("purlis could not hibernate the chat: {err}"))??;
    let _ = app.emit(
        CHANGED,
        Changed {
            plane,
            session,
            hibernated: true,
        },
    );
    Ok(())
}

/// What the window is told when a chat is hibernated or woken.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Changed {
    pub plane: PlaneId,
    pub session: u32,
    /// Whether it is hibernated now.
    pub hibernated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chat at a turn boundary that may be hibernated.
    fn at_rest() -> Facts {
        Facts {
            state: Some(State::Waiting),
            resumes_by_id: true,
            ..Facts::default()
        }
    }

    #[test]
    fn a_chat_at_a_turn_boundary_on_a_harness_that_resumes_by_id_may_be_hibernated() {
        assert_eq!(refusal(&at_rest()), None);
    }

    #[test]
    fn each_thing_that_stands_in_the_way_is_refused_alone_in_its_own_words() {
        let cases = [
            (
                Facts {
                    state: Some(State::Running),
                    ..at_rest()
                },
                Refusal::Working,
            ),
            (
                Facts {
                    state: Some(State::Unknown),
                    ..at_rest()
                },
                Refusal::Working,
            ),
            (
                Facts {
                    state: Some(State::Done),
                    ..at_rest()
                },
                Refusal::Ended,
            ),
            (
                Facts {
                    state: Some(State::Failed),
                    ..at_rest()
                },
                Refusal::Ended,
            ),
            (
                Facts {
                    asking: true,
                    ..at_rest()
                },
                Refusal::Asking,
            ),
            (
                Facts {
                    paused: true,
                    ..at_rest()
                },
                Refusal::Paused,
            ),
            (
                Facts {
                    resumes_by_id: false,
                    ..at_rest()
                },
                Refusal::CannotResume,
            ),
            (
                Facts {
                    observed: true,
                    ..at_rest()
                },
                Refusal::Observed,
            ),
            (
                Facts {
                    own_work: true,
                    ..at_rest()
                },
                Refusal::OwnWork,
            ),
            (
                Facts {
                    has_tasks: true,
                    ..at_rest()
                },
                Refusal::HasTasks,
            ),
            (
                Facts {
                    is_a_task: true,
                    ..at_rest()
                },
                Refusal::IsATask,
            ),
        ];
        let mut said = std::collections::HashMap::new();
        for (facts, why) in cases {
            assert_eq!(refusal(&facts), Some(why), "{facts:?}");
            assert!(why.says().starts_with("Not hibernated: "), "{why:?}");
            // Two chats refused for one reason are told it in one set of words, and no two
            // reasons share them.
            let first = said.entry(why.says()).or_insert(why);
            assert_eq!(*first, why, "each reason has its own words");
        }
    }

    #[test]
    fn a_chat_with_no_board_entry_is_not_at_a_turn_boundary() {
        assert_eq!(
            refusal(&Facts {
                state: None,
                resumes_by_id: true,
                ..Facts::default()
            }),
            Some(Refusal::Working)
        );
    }

    #[test]
    fn a_hibernated_chat_is_kept_until_it_is_forgotten() {
        let hibernated = Hibernated::default();
        assert!(hibernated.intend(3, By::Operator));
        assert!(hibernated.settle(3));
        assert_eq!(hibernated.of(3), Some(By::Operator));
        assert_eq!(hibernated.of(4), None);
        assert_eq!(hibernated.now(), [3]);
        hibernated.forget(3);
        assert_eq!(hibernated.of(3), None);
    }

    #[test]
    fn a_chat_is_hibernated_only_once_the_host_has_answered() {
        let hibernated = Hibernated::default();
        assert!(hibernated.intend(3, By::Operator));
        assert!(hibernated.in_flight(3));
        assert_eq!(hibernated.of(3), None, "no Resume while it is in flight");
        assert_eq!(hibernated.now(), Vec::<u32>::new());
        assert!(
            hibernated.ends_as_one(3),
            "its end is read as the hibernation"
        );
        assert!(hibernated.settle(3));
        assert!(!hibernated.in_flight(3));
        assert_eq!(hibernated.of(3), Some(By::Operator));
    }

    #[test]
    fn a_second_intent_leaves_the_first_in_flight() {
        let hibernated = Hibernated::default();
        assert!(hibernated.intend(3, By::Operator));
        assert!(!hibernated.intend(3, By::Operator), "the first press acts");
        assert!(hibernated.in_flight(3));
    }

    #[test]
    fn a_failed_act_is_forgotten_unless_its_program_already_ended_as_one() {
        let hibernated = Hibernated::default();
        assert!(hibernated.intend(3, By::Operator));
        assert!(!hibernated.failed(3));
        assert!(!hibernated.in_flight(3));
        assert_eq!(hibernated.of(3), None);

        assert!(hibernated.intend(4, By::Operator));
        assert!(hibernated.ends_as_one(4));
        assert!(hibernated.failed(4), "its program ended as a hibernation");
        assert_eq!(hibernated.of(4), Some(By::Operator));
    }

    #[test]
    fn an_end_that_won_leaves_nothing_to_settle() {
        let hibernated = Hibernated::default();
        assert!(hibernated.intend(3, By::Operator));
        hibernated.forget(3);
        assert!(!hibernated.settle(3));
        assert_eq!(hibernated.of(3), None);
    }
}
